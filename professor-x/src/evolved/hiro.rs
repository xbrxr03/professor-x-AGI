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
use serde::Deserialize;
use std::sync::Arc;
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
    pub id:          String,
    pub category:    HiroCategory,
    pub description: String,
    #[allow(dead_code)]
    pub difficulty:  String,
}

#[derive(Debug, Deserialize)]
struct TaskFile {
    tasks: Vec<HiroTask>,
}

#[derive(Debug)]
pub struct HiroRoundResult {
    pub round:      u32,
    pub p_tool:     f32,
    pub p_plan:     f32,
    pub p_correct:  f32,
    pub pass_at_3:  f32,
    pub task_count: usize,
    pub successes:  usize,
}

// ── Runner ────────────────────────────────────────────────────────────────────

pub struct HiroRunner {
    ollama:   Arc<OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy:   Arc<PolicyEngine>,
    memory:   Arc<MemoryManager>,
    cancel:   CancellationToken,
    /// Shared LCAP policy across all tasks in a round — UCB1 state accumulates per round.
    lcap:     Arc<std::sync::Mutex<LcapPolicy>>,
}

impl HiroRunner {
    pub fn new(
        ollama:   Arc<OllamaClient>,
        registry: Arc<std::sync::RwLock<ToolRegistry>>,
        policy:   Arc<PolicyEngine>,
        memory:   Arc<MemoryManager>,
        cancel:   CancellationToken,
    ) -> Self {
        let lcap = LcapPolicy::load_from_db(&memory.db)
            .unwrap_or_else(|_| LcapPolicy::new());
        Self {
            ollama, registry, policy, memory, cancel,
            lcap: Arc::new(std::sync::Mutex::new(lcap)),
        }
    }

    /// Run the full 60-task benchmark for a given round.
    /// Runs tasks sequentially (Ollama is single-GPU, parallel won't help throughput).
    pub async fn run_benchmark(&self, round: u32) -> Result<HiroRoundResult> {
        let tasks = load_tasks()?;
        info!("hiro: starting round {} with {} tasks", round, tasks.len());

        let mut tool_pass  = 0u32;
        let mut tool_total = 0u32;
        let mut plan_pass  = 0u32;
        let mut plan_total = 0u32;
        let mut corr_pass  = 0u32;
        let mut corr_total = 0u32;

        for (idx, task) in tasks.iter().enumerate() {
            if self.cancel.is_cancelled() {
                warn!("hiro: benchmark cancelled at task {}/{}", idx + 1, tasks.len());
                break;
            }

            info!(
                "hiro: [{}/{}] {} — {}",
                idx + 1,
                tasks.len(),
                task.id,
                &task.description.chars().take(60).collect::<String>()
            );

            let success = self.run_task(task, round).await.unwrap_or_else(|e| {
                warn!("hiro: task {} error: {e}", task.id);
                false
            });

            match task.category {
                HiroCategory::ToolUse => {
                    tool_total += 1;
                    if success { tool_pass += 1; }
                }
                HiroCategory::Planning => {
                    plan_total += 1;
                    if success { plan_pass += 1; }
                }
                HiroCategory::SelfCorrection => {
                    corr_total += 1;
                    if success { corr_pass += 1; }
                }
            }
        }

        let p_tool    = div_safe(tool_pass, tool_total);
        let p_plan    = div_safe(plan_pass, plan_total);
        let p_correct = div_safe(corr_pass, corr_total);
        let total_pass = tool_pass + plan_pass + corr_pass;
        let total_tasks = tool_total + plan_total + corr_total;
        let pass_at_3  = div_safe(total_pass, total_tasks);

        info!(
            "hiro: round {} complete — p_tool={:.3} p_plan={:.3} p_correct={:.3} pass@3={:.3}",
            round, p_tool, p_plan, p_correct, pass_at_3
        );

        // Record round in hiro_rounds table via BfTracker
        let bf = BfTracker::new(Arc::clone(&self.memory.db));
        if let Err(e) = bf.record_round(round, p_tool, p_plan, p_correct, None, None) {
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
            successes:  total_pass as usize,
        })
    }

    async fn run_task(&self, hiro_task: &HiroTask, round: u32) -> Result<bool> {
        let react = ReactLoop::new(
            Arc::clone(&self.ollama),
            Arc::clone(&self.registry),
            Arc::clone(&self.policy),
            Arc::clone(&self.memory),
            self.cancel.clone(),
        ).with_lcap(Arc::clone(&self.lcap), round);

        let mut task = TaskNode::new(
            hiro_task.description.clone(),
            TaskType::Research,
            50,
        );
        task.max_attempts = 3; // pass@3
        let outcome = react.run(&mut task).await?;
        Ok(outcome.success)
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

    let path = candidates.iter()
        .find(|p| p.exists())
        .ok_or_else(|| anyhow::anyhow!(
            "HIRO tasks file not found. Tried: {:?}. \
             Set HIRO_TASKS_PATH or run from the professor-x/ directory.",
            candidates
        ))?;

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
    if total == 0 { 0.0 } else { pass as f32 / total as f32 }
}

