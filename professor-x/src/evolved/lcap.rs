/// LCAP — Learned Context Allocation Policy.
///
/// UCB1 bandit over 5 context budget arms per task type.
/// c = 1.414 (ASI-Evolve standard).
///
/// After round 10, replaces the static T*-optimal budget with learned budgets.
/// Claim: ≥3pp gain over static allocation (H12).
///
/// ARCHITECTURE.md Section 14.3

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const UCB1_C: f64 = 1.414;

/// Pre-defined context budget arms (from ARCHITECTURE.md).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BudgetArm {
    Sparse,
    Conservative,
    Balanced,
    Rich,
    MemoryHeavy,
}

/// Token budget allocation for one task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    pub episodic_slots:       u8,
    pub semantic_slots:       u8,
    pub tool_depth:           u8,   // max tool call chain depth
    pub system_prompt_tokens: u16,
    pub hard_ceiling_tokens:  u32,
}

impl ContextBudget {
    pub fn from_arm(arm: &BudgetArm) -> Self {
        match arm {
            BudgetArm::Sparse => Self {
                episodic_slots: 1, semantic_slots: 1, tool_depth: 2,
                system_prompt_tokens: 256, hard_ceiling_tokens: 4096,
            },
            BudgetArm::Conservative => Self {
                episodic_slots: 2, semantic_slots: 2, tool_depth: 3,
                system_prompt_tokens: 512, hard_ceiling_tokens: 8192,
            },
            BudgetArm::Balanced => Self {
                episodic_slots: 3, semantic_slots: 3, tool_depth: 4,
                system_prompt_tokens: 768, hard_ceiling_tokens: 12288,
            },
            BudgetArm::Rich => Self {
                episodic_slots: 5, semantic_slots: 4, tool_depth: 6,
                system_prompt_tokens: 1024, hard_ceiling_tokens: 16384,
            },
            BudgetArm::MemoryHeavy => Self {
                episodic_slots: 8, semantic_slots: 6, tool_depth: 4,
                system_prompt_tokens: 1024, hard_ceiling_tokens: 24576,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmState {
    pub arm:           BudgetArm,
    pub pull_count:    u32,
    pub total_reward:  f32,
}

impl ArmState {
    fn new(arm: BudgetArm) -> Self {
        Self { arm, pull_count: 0, total_reward: 0.0 }
    }

    fn mean_reward(&self) -> f64 {
        if self.pull_count == 0 { 0.0 } else { self.total_reward as f64 / self.pull_count as f64 }
    }

    fn ucb1(&self, total_pulls: u32) -> f64 {
        if self.pull_count == 0 {
            return f64::MAX; // Unvisited arms sampled first
        }
        let n = total_pulls as f64;
        let ni = self.pull_count as f64;
        self.mean_reward() + UCB1_C * (n.ln() / ni).sqrt()
    }
}

/// Task type classifier — maps task descriptions to arm selection.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskCategory {
    ToolUse,
    Planning,
    SelfCorrection,
    Research,
    Other,
}

pub struct LcapPolicy {
    /// One bandit per task category, each with 5 arms.
    arms: HashMap<TaskCategory, Vec<ArmState>>,
    /// Only activate after round 10 (need enough data to learn from).
    active_round: u32,
}

impl LcapPolicy {
    pub fn new() -> Self {
        let all_arms = vec![
            ArmState::new(BudgetArm::Sparse),
            ArmState::new(BudgetArm::Conservative),
            ArmState::new(BudgetArm::Balanced),
            ArmState::new(BudgetArm::Rich),
            ArmState::new(BudgetArm::MemoryHeavy),
        ];

        let mut arms = HashMap::new();
        arms.insert(TaskCategory::ToolUse,       all_arms.clone());
        arms.insert(TaskCategory::Planning,       all_arms.clone());
        arms.insert(TaskCategory::SelfCorrection, all_arms.clone());
        arms.insert(TaskCategory::Research,       all_arms.clone());
        arms.insert(TaskCategory::Other,          all_arms);

        Self { arms, active_round: 10 }
    }

    /// Select the best budget for a task category via UCB1.
    /// Before round 10 (or during cold start), returns Balanced as default.
    pub fn select(&self, category: &TaskCategory, current_round: u32) -> ContextBudget {
        if current_round < self.active_round {
            return ContextBudget::from_arm(&BudgetArm::Balanced);
        }

        let arms = match self.arms.get(category) {
            Some(a) => a,
            None    => return ContextBudget::from_arm(&BudgetArm::Balanced),
        };

        let total_pulls: u32 = arms.iter().map(|a| a.pull_count).sum();
        let best = arms.iter()
            .max_by(|a, b| a.ucb1(total_pulls).partial_cmp(&b.ucb1(total_pulls))
                .unwrap_or(std::cmp::Ordering::Equal));

        match best {
            Some(arm) => ContextBudget::from_arm(&arm.arm),
            None      => ContextBudget::from_arm(&BudgetArm::Balanced),
        }
    }

    /// Update arm reward after a task completes.
    /// reward = delta_pass_at_3 for this task type in this round.
    pub fn update(&mut self, category: &TaskCategory, arm: &BudgetArm, reward: f32) {
        if let Some(arms) = self.arms.get_mut(category) {
            if let Some(state) = arms.iter_mut().find(|a| &a.arm == arm) {
                state.pull_count  += 1;
                state.total_reward += reward;
            }
        }
    }

    /// Classify task description into a category.
    pub fn classify(description: &str) -> TaskCategory {
        let lower = description.to_lowercase();
        if lower.contains("tool") || lower.contains("search") || lower.contains("fetch") ||
           lower.contains("read") || lower.contains("write") || lower.contains("execute") {
            TaskCategory::ToolUse
        } else if lower.contains("plan") || lower.contains("strateg") || lower.contains("sequence") ||
                  lower.contains("steps") {
            TaskCategory::Planning
        } else if lower.contains("fix") || lower.contains("correct") || lower.contains("error") ||
                  lower.contains("wrong") || lower.contains("debug") {
            TaskCategory::SelfCorrection
        } else if lower.contains("research") || lower.contains("study") || lower.contains("analyze") ||
                  lower.contains("hypothesis") || lower.contains("paper") {
            TaskCategory::Research
        } else {
            TaskCategory::Other
        }
    }
}

impl Default for LcapPolicy {
    fn default() -> Self { Self::new() }
}
