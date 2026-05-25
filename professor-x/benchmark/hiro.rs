/// HIRO Benchmark — Harness Improvement Rate Over iterations
///
/// P0: This file MUST be built before any experiments run.
/// All hypotheses H1-H18 reference HIRO scores.
///
/// Source: brain/paper_outline.md Section 5
/// Primary metric: HIRO(N) = (P_N - P_0) / N — mean pass@3 gain per round
/// Task suite: 60 tasks (20 tool-use, 20 planning, 20 self-correction)
/// Estimated time: ~47 min per round on RTX 3060 at pass@3

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ── Task definitions ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskCategory {
    /// Deterministic verification: pass/fail by output matching
    ToolUse,
    /// LLM-as-judge (Claude Sonnet, 0/1 score with rubric)
    Planning,
    /// Binary: agent must detect and fix its own error
    SelfCorrection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiroTask {
    pub id: String,
    pub category: TaskCategory,
    pub prompt: String,
    /// For ToolUse: expected output string
    /// For Planning: rubric for LLM judge
    /// For SelfCorrection: the error to detect + expected fix
    pub expected: String,
}

/// Result of running one task attempt.
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskAttempt {
    pub task_id: String,
    pub attempt_number: u8,  // 1-3 (pass@3)
    pub passed: bool,
    pub agent_output: String,
    pub duration_ms: u64,
}

/// Result of one HIRO round (all 60 tasks × 3 attempts each = 180 runs).
#[derive(Debug, Serialize, Deserialize)]
pub struct HiroRoundResult {
    pub round: u32,
    /// Pass@3 for each category
    pub p_tool: f32,
    pub p_plan: f32,
    pub p_correct: f32,
    /// Overall pass@3 across all 60 tasks
    pub p_overall: f32,
    /// Behavioral fingerprint F(H_k) = [p_tool, p_plan, p_correct]
    pub fingerprint: [f32; 3],
    pub timestamp: i64,
}

// ── Metrics ───────────────────────────────────────────────────────────────────

/// Primary HIRO metric: mean pass@3 gain per round.
/// HIRO(N) = (P_N - P_0) / N
pub fn compute_hiro(p_0: f32, p_n: f32, n: u32) -> f32 {
    if n == 0 { return 0.0; }
    (p_n - p_0) / n as f32
}

/// Behavioral fingerprint at round k.
/// F(H_k) = [p_tool, p_plan, p_correct]
pub fn compute_fingerprint(round: &HiroRoundResult) -> [f32; 3] {
    [round.p_tool, round.p_plan, round.p_correct]
}

// ── Task suite ────────────────────────────────────────────────────────────────

/// Returns the fixed 60-task HIRO evaluation suite.
/// Tasks are fixed across all rounds — same tasks, same prompts, same evaluation criteria.
/// Reproducibility: any researcher with RTX 3060 + Ollama can run this in ~47 min/round.
pub fn hiro_task_suite() -> Vec<HiroTask> {
    let mut tasks = Vec::with_capacity(60);

    // ── 20 Tool-Use Tasks (deterministic verification) ──
    // TODO: fill in tasks from brain/paper_outline.md Appendix A
    // Sources: adapted from ALFWorld, ToolEval, synthetic
    for i in 1..=20 {
        tasks.push(HiroTask {
            id: format!("tool-{i:02}"),
            category: TaskCategory::ToolUse,
            prompt: format!("TODO: Tool-use task {i} — deterministic output verification"),
            expected: format!("TODO: expected output for tool-{i:02}"),
        });
    }

    // ── 20 Planning Tasks (LLM-as-judge) ──
    // Judge: Claude Sonnet 4.5, rubric: specificity + feasibility + completeness
    for i in 1..=20 {
        tasks.push(HiroTask {
            id: format!("plan-{i:02}"),
            category: TaskCategory::Planning,
            prompt: format!("TODO: Planning task {i} — LLM-as-judge evaluation"),
            expected: format!("TODO: rubric for plan-{i:02}"),
        });
    }

    // ── 20 Self-Correction Tasks (binary) ──
    // Agent receives a flawed output and must detect + fix the error
    for i in 1..=20 {
        tasks.push(HiroTask {
            id: format!("correct-{i:02}"),
            category: TaskCategory::SelfCorrection,
            prompt: format!("TODO: Self-correction task {i} — binary detect+fix"),
            expected: format!("TODO: correct output for correct-{i:02}"),
        });
    }

    tasks
}

// ── Evaluation runner ─────────────────────────────────────────────────────────

/// Run the full HIRO suite for one round.
/// pass@3: a task passes if it passes on ANY of 3 attempts.
pub async fn run_hiro_round(
    round: u32,
    tasks: &[HiroTask],
    // react_loop: &crate::agentd::react::ReactLoop,  // uncomment when wired
) -> Result<HiroRoundResult> {
    let mut tool_passes = 0u32;
    let mut plan_passes = 0u32;
    let mut correct_passes = 0u32;

    for task in tasks {
        let mut passed = false;
        for attempt in 1..=3u8 {
            // TODO: run task through react_loop
            // let output = react_loop.run(&task.prompt).await?;
            // let pass = evaluate_task(task, &output).await?;
            let pass = false; // placeholder
            if pass { passed = true; break; }
            let _ = attempt;
        }
        if passed {
            match task.category {
                TaskCategory::ToolUse       => tool_passes += 1,
                TaskCategory::Planning      => plan_passes += 1,
                TaskCategory::SelfCorrection => correct_passes += 1,
            }
        }
    }

    let n_tool = tasks.iter().filter(|t| t.category == TaskCategory::ToolUse).count() as f32;
    let n_plan = tasks.iter().filter(|t| t.category == TaskCategory::Planning).count() as f32;
    let n_correct = tasks.iter().filter(|t| t.category == TaskCategory::SelfCorrection).count() as f32;
    let n_total = tasks.len() as f32;

    let p_tool    = tool_passes as f32 / n_tool.max(1.0);
    let p_plan    = plan_passes as f32 / n_plan.max(1.0);
    let p_correct = correct_passes as f32 / n_correct.max(1.0);
    let p_overall = (tool_passes + plan_passes + correct_passes) as f32 / n_total.max(1.0);

    Ok(HiroRoundResult {
        round,
        p_tool, p_plan, p_correct, p_overall,
        fingerprint: [p_tool, p_plan, p_correct],
        timestamp: chrono::Utc::now().timestamp(),
    })
}
