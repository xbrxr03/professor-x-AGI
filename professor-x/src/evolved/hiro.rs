/// HIRO benchmark runner.
///
/// 60 tasks across 3 categories (20 each):
/// - tool_use:        basic tool execution (p_tool)
/// - planning:        multi-step decomposition (p_plan)
/// - self_correction: detect + recover from wrong first move (p_correct)
///
/// Pass@3 semantics: each task gets max_attempts=3.
/// One round = all 60 tasks run sequentially.
/// HIRO score = (P_N - P_0) / N where P_k = pass@3 at round k.
///
/// Tasks are loaded from hiro/tasks.json relative to the binary's working directory,
/// or from HIRO_TASKS_PATH env var.
use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::params;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

use crate::agentd::graph::{TaskNode, TaskType};
use crate::agentd::react::ReactLoop;
use crate::evolved::bf::BfTracker;
use crate::evolved::lcap::LcapPolicy;
use crate::memd::MemoryManager;
use crate::ollama::OllamaClient;
use crate::policyd::PolicyEngine;
use crate::toolbridge::ToolRegistry;
use tokio_util::sync::CancellationToken;

// ── Task types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HiroCategory {
    ToolUse,
    Planning,
    SelfCorrection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HiroTask {
    pub id: String,
    pub category: HiroCategory,
    pub description: String,
    #[allow(dead_code)]
    pub difficulty: String,
    #[serde(default)]
    pub evaluator: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TaskFile {
    tasks: Vec<HiroTask>,
}

#[derive(Debug)]
pub struct HiroRoundResult {
    pub round: u32,
    pub p_tool: f32,
    pub p_plan: f32,
    pub p_correct: f32,
    pub pass_at_3: f32,
    pub task_count: usize,
    pub successes: usize,
}

#[derive(Debug, Clone)]
pub struct HiroAttemptResult {
    pub task_id: String,
    pub category: HiroCategory,
    pub attempt: u8,
    pub passed: bool,
    pub failure_reason: Option<String>,
    pub output_hash: String,
    pub duration_ms: u64,
}

// ── Runner ────────────────────────────────────────────────────────────────────

pub struct HiroRunner {
    ollama: Arc<OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    cancel: CancellationToken,
    /// Shared LCAP policy across all tasks in a round — UCB1 state accumulates per round.
    lcap: Arc<std::sync::Mutex<LcapPolicy>>,
}

impl HiroRunner {
    pub fn new(
        ollama: Arc<OllamaClient>,
        registry: Arc<std::sync::RwLock<ToolRegistry>>,
        policy: Arc<PolicyEngine>,
        memory: Arc<MemoryManager>,
        cancel: CancellationToken,
    ) -> Self {
        let lcap = LcapPolicy::load_from_db(&memory.db).unwrap_or_else(|_| LcapPolicy::new());
        Self {
            ollama,
            registry,
            policy,
            memory,
            cancel,
            lcap: Arc::new(std::sync::Mutex::new(lcap)),
        }
    }

    /// Run the full 60-task benchmark for a given round.
    /// Runs tasks sequentially (Ollama is single-GPU, parallel won't help throughput).
    pub async fn run_benchmark(&self, round: u32) -> Result<HiroRoundResult> {
        self.run_benchmark_labeled(round, None).await
    }

    pub async fn run_benchmark_labeled(
        &self,
        round: u32,
        component_modified: Option<&str>,
    ) -> Result<HiroRoundResult> {
        self.run_benchmark_labeled_with_limit(round, component_modified, None)
            .await
    }

    pub async fn run_benchmark_labeled_with_limit(
        &self,
        round: u32,
        component_modified: Option<&str>,
        task_limit: Option<usize>,
    ) -> Result<HiroRoundResult> {
        let mut tasks = load_tasks()?;
        if let Some(limit) = task_limit {
            tasks.truncate(limit);
        }
        let harness_commit = current_harness_commit().unwrap_or_else(|e| {
            warn!("hiro: failed to read harness commit: {e}");
            "unknown".to_string()
        });
        info!("hiro: starting round {} with {} tasks", round, tasks.len());

        let mut tool_pass = 0u32;
        let mut tool_total = 0u32;
        let mut plan_pass = 0u32;
        let mut plan_total = 0u32;
        let mut corr_pass = 0u32;
        let mut corr_total = 0u32;

        for (idx, task) in tasks.iter().enumerate() {
            if self.cancel.is_cancelled() {
                warn!(
                    "hiro: benchmark cancelled at task {}/{}",
                    idx + 1,
                    tasks.len()
                );
                break;
            }

            info!(
                "hiro: [{}/{}] {} — {}",
                idx + 1,
                tasks.len(),
                task.id,
                &task.description.chars().take(60).collect::<String>()
            );

            let success = match self.run_task(task, round).await {
                Ok((success, attempts)) => {
                    if let Err(e) = self.record_attempts(round, &harness_commit, &attempts) {
                        warn!("hiro: failed to record attempts for task {}: {e}", task.id);
                    }
                    success
                }
                Err(e) => {
                    warn!("hiro: task {} error: {e}", task.id);
                    let attempt = HiroAttemptResult {
                        task_id: task.id.clone(),
                        category: task.category.clone(),
                        attempt: 1,
                        passed: false,
                        failure_reason: Some(e.to_string()),
                        output_hash: hash_output(&e.to_string()),
                        duration_ms: 0,
                    };
                    if let Err(record_err) =
                        self.record_attempts(round, &harness_commit, &[attempt])
                    {
                        warn!(
                            "hiro: failed to record error attempt for task {}: {record_err}",
                            task.id
                        );
                    }
                    false
                }
            };

            match task.category {
                HiroCategory::ToolUse => {
                    tool_total += 1;
                    if success {
                        tool_pass += 1;
                    }
                }
                HiroCategory::Planning => {
                    plan_total += 1;
                    if success {
                        plan_pass += 1;
                    }
                }
                HiroCategory::SelfCorrection => {
                    corr_total += 1;
                    if success {
                        corr_pass += 1;
                    }
                }
            }
        }

        let p_tool = div_safe(tool_pass, tool_total);
        let p_plan = div_safe(plan_pass, plan_total);
        let p_correct = div_safe(corr_pass, corr_total);
        let total_pass = tool_pass + plan_pass + corr_pass;
        let total_tasks = tool_total + plan_total + corr_total;
        let pass_at_3 = div_safe(total_pass, total_tasks);

        info!(
            "hiro: round {} complete — p_tool={:.3} p_plan={:.3} p_correct={:.3} pass@3={:.3}",
            round, p_tool, p_plan, p_correct, pass_at_3
        );

        // Record round in hiro_rounds table via BfTracker
        let bf = BfTracker::new(Arc::clone(&self.memory.db));
        if let Err(e) = bf.record_round(
            round,
            p_tool,
            p_plan,
            p_correct,
            component_modified,
            Some(&harness_commit),
        ) {
            warn!("hiro: failed to record round {}: {e}", round);
        }

        // Persist LCAP arm state so UCB1 learning carries over to future rounds
        {
            let lc = self.lcap.lock().unwrap();
            if let Err(e) = lc.save_to_db(&self.memory.db) {
                warn!("hiro: failed to persist LCAP state: {e}");
            }
        }

        Ok(HiroRoundResult {
            round,
            p_tool,
            p_plan,
            p_correct,
            pass_at_3,
            task_count: total_tasks as usize,
            successes: total_pass as usize,
        })
    }

    async fn run_task(
        &self,
        hiro_task: &HiroTask,
        round: u32,
    ) -> Result<(bool, Vec<HiroAttemptResult>)> {
        let react = ReactLoop::new(
            Arc::clone(&self.ollama),
            Arc::clone(&self.registry),
            Arc::clone(&self.policy),
            Arc::clone(&self.memory),
            self.cancel.clone(),
        )
        .with_lcap(Arc::clone(&self.lcap), round);

        let mut task = TaskNode::new(hiro_task.description.clone(), TaskType::Research, 50);
        task.max_attempts = 3; // pass@3
        let start = Instant::now();
        let outcome = react.run(&mut task).await?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let evaluation = evaluate_task(hiro_task, &task, outcome.success);
        let attempts = summarize_attempts(
            hiro_task,
            &task,
            evaluation.passed,
            evaluation.failure_reason.clone(),
            duration_ms,
        );
        Ok((evaluation.passed, attempts))
    }

    fn record_attempts(
        &self,
        round: u32,
        harness_commit: &str,
        attempts: &[HiroAttemptResult],
    ) -> Result<()> {
        let db = self.memory.db.lock().unwrap();
        for attempt in attempts {
            db.execute(
                "INSERT OR REPLACE INTO hiro_attempts
                 (round, harness_commit, task_id, category, attempt, passed,
                  failure_reason, output_hash, duration_ms, recorded_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![
                    round,
                    harness_commit,
                    attempt.task_id.as_str(),
                    hiro_category_name(&attempt.category),
                    attempt.attempt as i64,
                    attempt.passed as i64,
                    attempt.failure_reason.as_deref(),
                    attempt.output_hash.as_str(),
                    attempt.duration_ms as i64,
                    Utc::now().to_rfc3339(),
                ],
            )?;
        }
        Ok(())
    }
}

// ── Task loading ──────────────────────────────────────────────────────────────

fn load_tasks() -> Result<Vec<HiroTask>> {
    // 1. Explicit env override
    // 2. Relative to the running binary's directory (for installed deployments)
    // 3. CWD-relative (for `cargo run`)
    let candidates: Vec<std::path::PathBuf> = if let Ok(p) = std::env::var("HIRO_TASKS_PATH") {
        vec![std::path::PathBuf::from(p)]
    } else {
        let mut paths = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                paths.push(dir.join("hiro/tasks.json"));
            }
        }
        paths.push(std::path::PathBuf::from("hiro/tasks.json"));
        paths
    };

    let path = candidates.iter().find(|p| p.exists()).ok_or_else(|| {
        anyhow::anyhow!(
            "HIRO tasks file not found. Tried: {:?}. \
             Set HIRO_TASKS_PATH or run from the professor-x/ directory.",
            candidates
        )
    })?;

    let json = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read HIRO tasks from '{}': {e}", path.display()))?;

    let file: TaskFile = serde_json::from_str(&json)
        .map_err(|e| anyhow::anyhow!("cannot parse HIRO tasks JSON: {e}"))?;

    if file.tasks.is_empty() {
        bail!("HIRO tasks file is empty");
    }

    Ok(file.tasks)
}

fn div_safe(pass: u32, total: u32) -> f32 {
    if total == 0 {
        0.0
    } else {
        pass as f32 / total as f32
    }
}

fn summarize_attempts(
    hiro_task: &HiroTask,
    task: &TaskNode,
    success: bool,
    evaluated_failure_reason: Option<String>,
    total_duration_ms: u64,
) -> Vec<HiroAttemptResult> {
    let attempts = task.attempt_count.max(1);
    let per_attempt_ms = total_duration_ms / attempts as u64;
    (1..=attempts)
        .map(|attempt| {
            let passed = success && attempt == attempts;
            let failure_reason = if passed {
                None
            } else if attempt == attempts {
                evaluated_failure_reason.clone().or_else(|| {
                    Some(
                        task.steps
                            .last()
                            .and_then(|step| step.observation.error.clone())
                            .or_else(|| task.reflections.back().cloned())
                            .unwrap_or_else(|| "task failed".to_string()),
                    )
                })
            } else if success {
                Some("attempt did not complete before retry".to_string())
            } else {
                Some(
                    task.steps
                        .last()
                        .and_then(|step| step.observation.error.clone())
                        .or_else(|| task.reflections.back().cloned())
                        .unwrap_or_else(|| "task failed".to_string()),
                )
            };
            let output = if passed {
                task.steps_text()
            } else {
                failure_reason.clone().unwrap_or_default()
            };
            HiroAttemptResult {
                task_id: hiro_task.id.clone(),
                category: hiro_task.category.clone(),
                attempt,
                passed,
                failure_reason,
                output_hash: hash_output(&output),
                duration_ms: per_attempt_ms,
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
struct HiroEvaluation {
    passed: bool,
    failure_reason: Option<String>,
}

fn evaluate_task(hiro_task: &HiroTask, task: &TaskNode, react_success: bool) -> HiroEvaluation {
    if !react_success {
        return HiroEvaluation {
            passed: false,
            failure_reason: Some("ReAct task failed before evaluator checks".to_string()),
        };
    }

    let evaluator = hiro_task.evaluator.as_deref().unwrap_or("category_trace");
    match evaluator {
        "category_trace" => evaluate_category_trace(hiro_task, task),
        "finish_only" => HiroEvaluation {
            passed: true,
            failure_reason: None,
        },
        other => HiroEvaluation {
            passed: false,
            failure_reason: Some(format!("unknown HIRO evaluator '{other}'")),
        },
    }
}

fn evaluate_category_trace(hiro_task: &HiroTask, task: &TaskNode) -> HiroEvaluation {
    if task.steps.is_empty() {
        return HiroEvaluation {
            passed: false,
            failure_reason: Some("no tool trace before completion".to_string()),
        };
    }

    match hiro_task.category {
        HiroCategory::ToolUse => {
            if successful_tool_steps(task) == 0 {
                return HiroEvaluation {
                    passed: false,
                    failure_reason: Some(
                        "tool-use task completed without a successful tool call".to_string(),
                    ),
                };
            }
        }
        HiroCategory::Planning => {
            if successful_tool_steps(task) == 0 && task.steps.len() < 2 {
                return HiroEvaluation {
                    passed: false,
                    failure_reason: Some(
                        "planning task lacks multi-step or successful-tool evidence".to_string(),
                    ),
                };
            }
        }
        HiroCategory::SelfCorrection => {
            let has_recovery_evidence = task.attempt_count > 1
                || !task.reflections.is_empty()
                || task.steps.iter().any(|step| !step.observation.success);
            if !has_recovery_evidence {
                return HiroEvaluation {
                    passed: false,
                    failure_reason: Some(
                        "self-correction task completed without retry/reflection/recovery evidence"
                            .to_string(),
                    ),
                };
            }
        }
    }

    HiroEvaluation {
        passed: true,
        failure_reason: None,
    }
}

fn successful_tool_steps(task: &TaskNode) -> usize {
    task.steps
        .iter()
        .filter(|step| {
            step.observation.success
                && !matches!(step.action.tool_name.as_str(), "finish" | "done" | "fail")
        })
        .count()
}

fn hiro_category_name(category: &HiroCategory) -> &'static str {
    match category {
        HiroCategory::ToolUse => "tool_use",
        HiroCategory::Planning => "planning",
        HiroCategory::SelfCorrection => "self_correction",
    }
}

fn hash_output(output: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(output.as_bytes());
    hex::encode(hasher.finalize())
}

fn current_harness_commit() -> Result<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentd::graph::ExecutionStep;
    use crate::toolbridge::executor::{Action, Observation};

    fn task(category: HiroCategory) -> HiroTask {
        HiroTask {
            id: "t1".to_string(),
            category,
            description: "test task".to_string(),
            difficulty: "medium".to_string(),
            evaluator: None,
        }
    }

    fn task_node_with_steps(steps: Vec<ExecutionStep>) -> TaskNode {
        let mut task = TaskNode::new("test task".to_string(), TaskType::Research, 50);
        task.attempt_count = 1;
        task.steps = steps;
        task
    }

    fn step(tool_name: &str, success: bool) -> ExecutionStep {
        ExecutionStep {
            index: 1,
            thought: "test".to_string(),
            action: Action {
                tool_name: tool_name.to_string(),
                params: serde_json::json!({}),
                risk_score: 0,
            },
            observation: Observation {
                success,
                output: if success { "ok" } else { "" }.to_string(),
                error: if success {
                    None
                } else {
                    Some("failed".to_string())
                },
                tokens_used: 0,
                execution_ms: 1,
                artifacts: Vec::new(),
            },
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn tool_use_evaluator_rejects_finish_without_tools() {
        let task = task(HiroCategory::ToolUse);
        let node = task_node_with_steps(Vec::new());

        let evaluation = evaluate_task(&task, &node, true);

        assert!(!evaluation.passed);
        assert!(evaluation.failure_reason.unwrap().contains("no tool trace"));
    }

    #[test]
    fn tool_use_evaluator_accepts_successful_tool_trace() {
        let task = task(HiroCategory::ToolUse);
        let node = task_node_with_steps(vec![step("fs.read", true)]);

        let evaluation = evaluate_task(&task, &node, true);

        assert!(evaluation.passed);
    }

    #[test]
    fn self_correction_requires_recovery_evidence() {
        let task = task(HiroCategory::SelfCorrection);
        let node = task_node_with_steps(vec![step("fs.read", true)]);

        let evaluation = evaluate_task(&task, &node, true);

        assert!(!evaluation.passed);
    }

    #[test]
    fn self_correction_accepts_retry_evidence() {
        let task = task(HiroCategory::SelfCorrection);
        let mut node = task_node_with_steps(vec![step("fs.read", true)]);
        node.attempt_count = 2;

        let evaluation = evaluate_task(&task, &node, true);

        assert!(evaluation.passed);
    }
}
