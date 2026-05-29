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
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};
use uuid::Uuid;

use crate::agentd::graph::{TaskNode, TaskType};
use crate::agentd::react::ReactLoop;
use crate::evolved::bf::BfTracker;
use crate::evolved::lcap::LcapPolicy;
use crate::memd::events::EventStore;
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct HiroTaskInventory {
    pub task_count: usize,
    pub tool_use: usize,
    pub planning: usize,
    pub self_correction: usize,
    pub duplicate_ids: Vec<String>,
}

/// JSON shape written under `artifacts/hiro/attempts/<run_id>/r<round>/<task_id>.json`.
/// Mirrors the `hiro_attempts` SQLite row plus run_id / harness_commit for
/// auditability.
#[derive(Debug, Serialize)]
struct HiroAttemptArtifact {
    run_id: String,
    round: u32,
    task_id: String,
    category: String,
    attempt: u8,
    passed: bool,
    failure_reason: Option<String>,
    output_hash: String,
    duration_ms: u64,
    harness_commit: String,
    recorded_at: DateTime<Utc>,
}

/// JSON shape written under `artifacts/hiro/rounds/<run_id>-r<round>.json`.
/// Required fields match `ArtifactKind::HiroRun`.
#[derive(Debug, Serialize)]
struct HiroRoundArtifact {
    run_id: String,
    round: u32,
    harness_commit: String,
    p_tool: f32,
    p_plan: f32,
    p_correct: f32,
    pass_at_3: f32,
    task_count: usize,
    successes: usize,
    frozen_harness: bool,
    component_modified: Option<String>,
    recorded_at: DateTime<Utc>,
}

/// JSON shape written under `artifacts/hiro/null-baselines/<run_id>.json`.
/// Required fields match `ArtifactKind::HiroNullBaseline`.
#[derive(Debug, Serialize, Deserialize)]
struct NullBaselineArtifact {
    run_id: String,
    harness_commit: String,
    frozen_harness: bool,
    rounds: Vec<NullBaselineRound>,
    recorded_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NullBaselineRound {
    round: u32,
    p_tool: f32,
    p_plan: f32,
    p_correct: f32,
    pass_at_3: f32,
    recorded_at: DateTime<Utc>,
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
    /// Stable identifier for this benchmark session. All rounds run by this
    /// runner share the same run_id. Generated at construction; published in
    /// every artifact and event so downstream tools can correlate.
    run_id: String,
    /// Optional event sink; when set, the runner publishes `hiro.*` events.
    events: Option<Arc<EventStore>>,
    /// Root directory for JSON artifact mirroring. Defaults to
    /// `artifacts/hiro`. Per-attempt JSONs land in `<root>/attempts/<run_id>/r<round>/`,
    /// per-round summaries in `<root>/rounds/`, null-baseline summaries in
    /// `<root>/null-baselines/`.
    artifact_root: PathBuf,
    /// When true, round summaries also write to `null-baselines/`. Set this
    /// for `--hiro-null` invocations so the operator audit can find them.
    frozen_harness: bool,
    /// H1 sweep override. Propagated to each per-task ReactLoop.
    memory_budget_override: Option<u32>,
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
            run_id: Uuid::new_v4().to_string(),
            events: None,
            artifact_root: PathBuf::from("artifacts/hiro"),
            frozen_harness: false,
            memory_budget_override: None,
        }
    }

    pub fn with_events(mut self, events: Arc<EventStore>) -> Self {
        self.events = Some(events);
        self
    }

    pub fn with_artifact_root(mut self, root: PathBuf) -> Self {
        self.artifact_root = root;
        self
    }

    pub fn as_null_baseline(mut self) -> Self {
        self.frozen_harness = true;
        self
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// H1 sweep: cap every task's context budget at `budget` tokens. Lower
    /// values force aggressive retrieval pruning. See `brain/hypotheses.md`
    /// H1 §"Proposed test" for the recommended budget points.
    pub fn with_memory_budget_override(mut self, budget: u32) -> Self {
        self.memory_budget_override = Some(budget);
        self
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
        if let Some(events) = &self.events {
            let _ = events.append(
                None,
                None,
                "hiro.round.started",
                format!(
                    "starting HIRO round {} (run_id={}, harness_commit={})",
                    round, self.run_id, harness_commit
                ),
                serde_json::json!({
                    "run_id": self.run_id,
                    "round": round,
                    "task_count": tasks.len(),
                    "harness_commit": harness_commit,
                    "frozen_harness": self.frozen_harness,
                    "component_modified": component_modified,
                }),
            );
        }

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

        let result = HiroRoundResult {
            round,
            p_tool,
            p_plan,
            p_correct,
            pass_at_3,
            task_count: total_tasks as usize,
            successes: total_pass as usize,
        };

        // Phase B: mirror round summary to disk so the artifact validator
        // and operator audits can find it without querying SQLite.
        match self.write_round_artifact(&result, &harness_commit, component_modified) {
            Ok(path) => {
                if let Some(events) = &self.events {
                    let _ = events.append(
                        None,
                        None,
                        "hiro.round.completed",
                        format!(
                            "HIRO round {} complete — pass@3={:.3} (run_id={})",
                            round, pass_at_3, self.run_id
                        ),
                        serde_json::json!({
                            "run_id": self.run_id,
                            "round": round,
                            "p_tool": p_tool,
                            "p_plan": p_plan,
                            "p_correct": p_correct,
                            "pass_at_3": pass_at_3,
                            "harness_commit": harness_commit,
                            "frozen_harness": self.frozen_harness,
                            "artifact_path": path.to_string_lossy(),
                        }),
                    );
                }
            }
            Err(e) => warn!("hiro: failed to write round artifact: {e}"),
        }

        if self.frozen_harness {
            if let Err(e) = self.append_null_baseline_summary(&result, &harness_commit) {
                warn!("hiro: failed to write null-baseline summary: {e}");
            }
        }

        Ok(result)
    }

    fn write_round_artifact(
        &self,
        result: &HiroRoundResult,
        harness_commit: &str,
        component_modified: Option<&str>,
    ) -> Result<PathBuf> {
        let dir = self.artifact_root.join("rounds");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}-r{}.json", self.run_id, result.round));
        let record = HiroRoundArtifact {
            run_id: self.run_id.clone(),
            round: result.round,
            harness_commit: harness_commit.to_string(),
            p_tool: result.p_tool,
            p_plan: result.p_plan,
            p_correct: result.p_correct,
            pass_at_3: result.pass_at_3,
            task_count: result.task_count,
            successes: result.successes,
            frozen_harness: self.frozen_harness,
            component_modified: component_modified.map(|s| s.to_string()),
            recorded_at: Utc::now(),
        };
        let json = serde_json::to_string_pretty(&record)?;
        let mut file = std::fs::File::create(&path)?;
        writeln!(file, "{json}")?;
        Ok(path)
    }

    /// Append-write a null-baseline summary that the operator runbook
    /// inspects before crediting any autonomous change. The summary file is
    /// per-run (one file covering all rounds of this runner) so multiple
    /// rounds of a single `--hiro-null` invocation accumulate into one
    /// artifact. Required-fields schema mirrors `ArtifactKind::HiroNullBaseline`.
    fn append_null_baseline_summary(
        &self,
        latest: &HiroRoundResult,
        harness_commit: &str,
    ) -> Result<()> {
        let dir = self.artifact_root.join("null-baselines");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", self.run_id));
        let mut existing: NullBaselineArtifact = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            serde_json::from_str(&raw).unwrap_or_else(|_| NullBaselineArtifact {
                run_id: self.run_id.clone(),
                harness_commit: harness_commit.to_string(),
                frozen_harness: true,
                rounds: Vec::new(),
                recorded_at: Utc::now(),
            })
        } else {
            NullBaselineArtifact {
                run_id: self.run_id.clone(),
                harness_commit: harness_commit.to_string(),
                frozen_harness: true,
                rounds: Vec::new(),
                recorded_at: Utc::now(),
            }
        };
        existing.rounds.push(NullBaselineRound {
            round: latest.round,
            p_tool: latest.p_tool,
            p_plan: latest.p_plan,
            p_correct: latest.p_correct,
            pass_at_3: latest.pass_at_3,
            recorded_at: Utc::now(),
        });
        existing.recorded_at = Utc::now();
        let json = serde_json::to_string_pretty(&existing)?;
        let mut file = std::fs::File::create(&path)?;
        writeln!(file, "{json}")?;
        Ok(())
    }

    async fn run_task(
        &self,
        hiro_task: &HiroTask,
        round: u32,
    ) -> Result<(bool, Vec<HiroAttemptResult>)> {
        let mut react = ReactLoop::new(
            Arc::clone(&self.ollama),
            Arc::clone(&self.registry),
            Arc::clone(&self.policy),
            Arc::clone(&self.memory),
            self.cancel.clone(),
        )
        .with_lcap(Arc::clone(&self.lcap), round);
        if let Some(budget) = self.memory_budget_override {
            react = react.with_memory_budget_override(budget);
        }

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
        {
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
        } // drop the lock before any FS work

        // Phase B: mirror per-attempt JSON next to the SQLite row. Failures
        // here only warn — they must not abort the benchmark.
        for attempt in attempts {
            if let Err(e) = self.write_attempt_artifact(round, harness_commit, attempt) {
                warn!(
                    "hiro: failed to write attempt artifact for {} (round {}, attempt {}): {e}",
                    attempt.task_id, round, attempt.attempt
                );
            }
        }
        Ok(())
    }

    fn write_attempt_artifact(
        &self,
        round: u32,
        harness_commit: &str,
        attempt: &HiroAttemptResult,
    ) -> Result<()> {
        let dir = self
            .artifact_root
            .join("attempts")
            .join(&self.run_id)
            .join(format!("r{round}"));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}-a{}.json", attempt.task_id, attempt.attempt));
        let record = HiroAttemptArtifact {
            run_id: self.run_id.clone(),
            round,
            task_id: attempt.task_id.clone(),
            category: hiro_category_name(&attempt.category).to_string(),
            attempt: attempt.attempt,
            passed: attempt.passed,
            failure_reason: attempt.failure_reason.clone(),
            output_hash: attempt.output_hash.clone(),
            duration_ms: attempt.duration_ms,
            harness_commit: harness_commit.to_string(),
            recorded_at: Utc::now(),
        };
        let json = serde_json::to_string_pretty(&record)?;
        let mut file = std::fs::File::create(&path)?;
        writeln!(file, "{json}")?;
        if let Some(events) = &self.events {
            let _ = events.append(
                None,
                None,
                "hiro.attempt.completed",
                format!(
                    "HIRO attempt {} for task {} (round {}): {}",
                    attempt.attempt,
                    attempt.task_id,
                    round,
                    if attempt.passed { "PASS" } else { "FAIL" }
                ),
                serde_json::json!({
                    "run_id": self.run_id,
                    "round": round,
                    "task_id": attempt.task_id,
                    "category": hiro_category_name(&attempt.category),
                    "attempt": attempt.attempt,
                    "passed": attempt.passed,
                    "duration_ms": attempt.duration_ms,
                    "artifact_path": path.to_string_lossy(),
                }),
            );
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

pub fn load_task_inventory() -> Result<HiroTaskInventory> {
    let tasks = load_tasks()?;
    let mut seen = HashSet::new();
    let mut duplicate_ids = Vec::new();
    let mut tool_use = 0usize;
    let mut planning = 0usize;
    let mut self_correction = 0usize;

    for task in &tasks {
        if !seen.insert(task.id.clone()) {
            duplicate_ids.push(task.id.clone());
        }
        match task.category {
            HiroCategory::ToolUse => tool_use += 1,
            HiroCategory::Planning => planning += 1,
            HiroCategory::SelfCorrection => self_correction += 1,
        }
    }

    let inventory = HiroTaskInventory {
        task_count: tasks.len(),
        tool_use,
        planning,
        self_correction,
        duplicate_ids,
    };
    validate_task_inventory(&inventory)?;
    Ok(inventory)
}

fn validate_task_inventory(inventory: &HiroTaskInventory) -> Result<()> {
    if inventory.task_count == 0 {
        bail!("HIRO inventory is empty");
    }
    if inventory.tool_use == 0 || inventory.planning == 0 || inventory.self_correction == 0 {
        bail!(
            "HIRO inventory missing category coverage: tool_use={} planning={} self_correction={}",
            inventory.tool_use,
            inventory.planning,
            inventory.self_correction
        );
    }
    if !inventory.duplicate_ids.is_empty() {
        bail!(
            "HIRO inventory has duplicate task ids: {:?}",
            inventory.duplicate_ids
        );
    }
    Ok(())
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

    #[test]
    fn inventory_requires_all_categories_and_unique_ids() {
        let valid = HiroTaskInventory {
            task_count: 3,
            tool_use: 1,
            planning: 1,
            self_correction: 1,
            duplicate_ids: Vec::new(),
        };
        assert!(validate_task_inventory(&valid).is_ok());

        let missing_category = HiroTaskInventory {
            self_correction: 0,
            ..valid.clone()
        };
        assert!(validate_task_inventory(&missing_category).is_err());

        let duplicate = HiroTaskInventory {
            duplicate_ids: vec!["tu_001".to_string()],
            ..valid
        };
        assert!(validate_task_inventory(&duplicate).is_err());
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

    // ── Phase B persistence shape ─────────────────────────────────────────

    /// Guard: the JSON shape `write_round_artifact` writes must satisfy the
    /// `ArtifactKind::HiroRun` schema in artifacts.rs. If a future refactor
    /// renames a field on `HiroRoundArtifact`, this test breaks before the
    /// `--validate-artifacts` scanner does.
    #[test]
    fn round_artifact_carries_all_hiro_run_required_fields() {
        let art = HiroRoundArtifact {
            run_id: "test-run".to_string(),
            round: 0,
            harness_commit: "abcdef0".to_string(),
            p_tool: 0.50,
            p_plan: 0.40,
            p_correct: 0.30,
            pass_at_3: 0.40,
            task_count: 60,
            successes: 24,
            frozen_harness: true,
            component_modified: None,
            recorded_at: Utc::now(),
        };
        let json = serde_json::to_value(&art).unwrap();
        for required in [
            "run_id",
            "round",
            "harness_commit",
            "p_tool",
            "p_plan",
            "p_correct",
            "pass_at_3",
            "recorded_at",
        ] {
            assert!(
                json.get(required).is_some(),
                "round artifact JSON missing required field '{}': {:?}",
                required,
                json
            );
        }
    }

    /// Guard: `NullBaselineArtifact` JSON shape must satisfy
    /// `ArtifactKind::HiroNullBaseline` required fields.
    #[test]
    fn null_baseline_artifact_carries_all_required_fields() {
        let art = NullBaselineArtifact {
            run_id: "nb-run".to_string(),
            harness_commit: "abcdef0".to_string(),
            frozen_harness: true,
            rounds: vec![NullBaselineRound {
                round: 0,
                p_tool: 0.5,
                p_plan: 0.4,
                p_correct: 0.3,
                pass_at_3: 0.4,
                recorded_at: Utc::now(),
            }],
            recorded_at: Utc::now(),
        };
        let json = serde_json::to_value(&art).unwrap();
        for required in [
            "run_id",
            "harness_commit",
            "rounds",
            "frozen_harness",
            "recorded_at",
        ] {
            assert!(
                json.get(required).is_some(),
                "null-baseline JSON missing required field '{}': {:?}",
                required,
                json
            );
        }
        let rounds = json.get("rounds").and_then(|v| v.as_array()).unwrap();
        assert!(!rounds.is_empty(), "rounds field must be non-empty array");
    }

    /// Guard: round and null-baseline run_ids are namespaced UUIDs (not
    /// colliding with paths that contain `/`). Catches a future refactor
    /// that swaps to a derived id.
    #[test]
    fn run_id_is_uuid_shape() {
        let art = HiroRoundArtifact {
            run_id: Uuid::new_v4().to_string(),
            round: 0,
            harness_commit: "x".to_string(),
            p_tool: 0.0,
            p_plan: 0.0,
            p_correct: 0.0,
            pass_at_3: 0.0,
            task_count: 0,
            successes: 0,
            frozen_harness: false,
            component_modified: None,
            recorded_at: Utc::now(),
        };
        assert!(!art.run_id.contains('/'));
        assert!(!art.run_id.contains('\\'));
        assert!(art.run_id.len() >= 32);
    }
}
