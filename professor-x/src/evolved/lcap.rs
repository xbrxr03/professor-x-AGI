/// LCAP — Learned Context Allocation Policy.
///
/// UCB1 bandit over 5 context budget arms per task type.
/// c = 1.414 (ASI-Evolve standard).
///
/// After round 10, replaces the static T*-optimal budget with learned budgets.
/// Claim: ≥3pp gain over static allocation (H12).
///
/// ARCHITECTURE.md Section 14.3
use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const UCB1_C: f64 = 1.414;

/// Pre-defined context budget arms (from ARCHITECTURE.md).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
    pub episodic_slots: u8,
    pub semantic_slots: u8,
    pub tool_depth: u8, // max tool call chain depth
    pub system_prompt_tokens: u16,
    pub hard_ceiling_tokens: u32,
}

impl ContextBudget {
    pub fn from_arm(arm: &BudgetArm) -> Self {
        match arm {
            BudgetArm::Sparse => Self {
                episodic_slots: 1,
                semantic_slots: 1,
                tool_depth: 2,
                system_prompt_tokens: 256,
                hard_ceiling_tokens: 4096,
            },
            BudgetArm::Conservative => Self {
                episodic_slots: 2,
                semantic_slots: 2,
                tool_depth: 3,
                system_prompt_tokens: 512,
                hard_ceiling_tokens: 8192,
            },
            BudgetArm::Balanced => Self {
                episodic_slots: 3,
                semantic_slots: 3,
                tool_depth: 4,
                system_prompt_tokens: 768,
                hard_ceiling_tokens: 12288,
            },
            BudgetArm::Rich => Self {
                episodic_slots: 5,
                semantic_slots: 4,
                tool_depth: 6,
                system_prompt_tokens: 1024,
                hard_ceiling_tokens: 16384,
            },
            BudgetArm::MemoryHeavy => Self {
                episodic_slots: 8,
                semantic_slots: 6,
                tool_depth: 4,
                system_prompt_tokens: 1024,
                hard_ceiling_tokens: 24576,
            },
        }
    }
}

impl BudgetArm {
    pub fn leaner(self) -> Option<Self> {
        match self {
            Self::Sparse => None,
            Self::Conservative => Some(Self::Sparse),
            Self::Balanced => Some(Self::Conservative),
            Self::Rich => Some(Self::Balanced),
            Self::MemoryHeavy => Some(Self::Rich),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmState {
    pub arm: BudgetArm,
    pub pull_count: u32,
    pub total_reward: f32,
}

impl ArmState {
    fn new(arm: BudgetArm) -> Self {
        Self {
            arm,
            pull_count: 0,
            total_reward: 0.0,
        }
    }

    fn mean_reward(&self) -> f64 {
        if self.pull_count == 0 {
            0.0
        } else {
            self.total_reward as f64 / self.pull_count as f64
        }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Immediate one-step correction path from DHE Layer 2. This is a live
    /// routing hint for the next task of a category, cleared once that arm is
    /// actually updated from a real task outcome.
    forced_next: HashMap<TaskCategory, BudgetArm>,
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
        arms.insert(TaskCategory::ToolUse, all_arms.clone());
        arms.insert(TaskCategory::Planning, all_arms.clone());
        arms.insert(TaskCategory::SelfCorrection, all_arms.clone());
        arms.insert(TaskCategory::Research, all_arms.clone());
        arms.insert(TaskCategory::Other, all_arms);

        Self {
            arms,
            active_round: 10,
            forced_next: HashMap::new(),
        }
    }

    /// Select the best budget for a task category via UCB1.
    /// Before round 10 (or during cold start), returns Balanced as default.
    pub fn select(&self, category: &TaskCategory, current_round: u32) -> ContextBudget {
        ContextBudget::from_arm(&self.select_arm(category, current_round))
    }

    pub fn select_arm(&self, category: &TaskCategory, current_round: u32) -> BudgetArm {
        if current_round < self.active_round && self.total_pulls() < self.active_round {
            return BudgetArm::Balanced;
        }
        if let Some(arm) = self.forced_next.get(category).copied() {
            return arm;
        }

        let arms = match self.arms.get(category) {
            Some(a) => a,
            None => return BudgetArm::Balanced,
        };

        let total_pulls: u32 = arms.iter().map(|a| a.pull_count).sum();
        let best = arms.iter().max_by(|a, b| {
            a.ucb1(total_pulls)
                .partial_cmp(&b.ucb1(total_pulls))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        match best {
            Some(arm) => arm.arm,
            None => BudgetArm::Balanced,
        }
    }

    /// Update arm reward after a task completes.
    /// reward = delta_pass_at_3 for this task type in this round.
    pub fn update(&mut self, category: &TaskCategory, arm: &BudgetArm, reward: f32) {
        self.forced_next.remove(category);
        if let Some(arms) = self.arms.get_mut(category) {
            if let Some(state) = arms.iter_mut().find(|a| &a.arm == arm) {
                state.pull_count += 1;
                state.total_reward += reward;
            }
        }
    }

    /// Direct contextual feedback path from DHE Layer 2.
    /// Penalize the current arm, then bias one step leaner so the next task of
    /// this type explores a smaller context budget immediately instead of
    /// waiting for a full evolution round.
    pub fn regress(
        &mut self,
        category: &TaskCategory,
        current_arm: &BudgetArm,
    ) -> Option<BudgetArm> {
        let leaner = current_arm.leaner()?;
        self.update(category, current_arm, -0.5);
        self.forced_next.insert(*category, leaner);
        Some(leaner)
    }

    fn total_pulls(&self) -> u32 {
        self.arms
            .values()
            .flat_map(|arms| arms.iter())
            .map(|state| state.pull_count)
            .sum()
    }

    /// Classify task description into a category.
    pub fn classify(description: &str) -> TaskCategory {
        let lower = description.to_lowercase();
        if lower.contains("tool")
            || lower.contains("search")
            || lower.contains("fetch")
            || lower.contains("read")
            || lower.contains("write")
            || lower.contains("execute")
        {
            TaskCategory::ToolUse
        } else if lower.contains("plan")
            || lower.contains("strateg")
            || lower.contains("sequence")
            || lower.contains("steps")
        {
            TaskCategory::Planning
        } else if lower.contains("fix")
            || lower.contains("correct")
            || lower.contains("error")
            || lower.contains("wrong")
            || lower.contains("debug")
        {
            TaskCategory::SelfCorrection
        } else if lower.contains("research")
            || lower.contains("study")
            || lower.contains("analyze")
            || lower.contains("hypothesis")
            || lower.contains("paper")
        {
            TaskCategory::Research
        } else {
            TaskCategory::Other
        }
    }
}

impl Default for LcapPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl LcapPolicy {
    /// Load accumulated arm state from DB, merging into a fresh policy.
    pub fn load_from_db(db: &Arc<Mutex<Connection>>) -> Result<Self> {
        let mut policy = Self::new();
        let conn = db.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT category, arm, pull_count, total_reward FROM lcap_arms")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, f32>(3)?,
            ))
        })?;
        for row in rows.flatten() {
            let (cat_str, arm_str, pulls, reward) = row;
            let category = parse_category(&cat_str);
            let arm = parse_arm(&arm_str);
            if let Some(arms) = policy.arms.get_mut(&category) {
                if let Some(state) = arms.iter_mut().find(|a| a.arm == arm) {
                    state.pull_count = pulls;
                    state.total_reward = reward;
                }
            }
        }
        Ok(policy)
    }

    /// Persist all arm state to DB (upsert).
    pub fn save_to_db(&self, db: &Arc<Mutex<Connection>>) -> Result<()> {
        let conn = db.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        for (category, arms) in &self.arms {
            let cat_str = format!("{category:?}");
            for state in arms {
                let arm_str = format!("{:?}", state.arm);
                conn.execute(
                    "INSERT INTO lcap_arms (category, arm, pull_count, total_reward, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(category, arm) DO UPDATE SET
                         pull_count   = excluded.pull_count,
                         total_reward = excluded.total_reward,
                         updated_at   = excluded.updated_at",
                    rusqlite::params![
                        cat_str,
                        arm_str,
                        state.pull_count,
                        state.total_reward as f64,
                        now,
                    ],
                )?;
            }
        }
        Ok(())
    }
}

fn parse_category(s: &str) -> TaskCategory {
    match s {
        "ToolUse" => TaskCategory::ToolUse,
        "Planning" => TaskCategory::Planning,
        "SelfCorrection" => TaskCategory::SelfCorrection,
        "Research" => TaskCategory::Research,
        _ => TaskCategory::Other,
    }
}

fn parse_arm(s: &str) -> BudgetArm {
    match s {
        "Sparse" => BudgetArm::Sparse,
        "Conservative" => BudgetArm::Conservative,
        "Balanced" => BudgetArm::Balanced,
        "Rich" => BudgetArm::Rich,
        _ => BudgetArm::MemoryHeavy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_arm_defaults_to_balanced_before_activation() {
        let policy = LcapPolicy::new();

        assert_eq!(
            policy.select_arm(&TaskCategory::ToolUse, 0),
            BudgetArm::Balanced
        );
        assert_eq!(
            policy.select_arm(&TaskCategory::Planning, 9),
            BudgetArm::Balanced
        );
    }

    #[test]
    fn regress_penalizes_current_arm_and_promotes_leaner_one() {
        let mut policy = LcapPolicy::new();
        let category = TaskCategory::Planning;

        {
            let arms = policy.arms.get_mut(&category).unwrap();
            for state in arms.iter_mut() {
                state.pull_count = 4;
                state.total_reward = 2.0;
            }
        }

        let next = policy.regress(&category, &BudgetArm::Rich);

        assert_eq!(next, Some(BudgetArm::Balanced));

        let arms = policy.arms.get(&category).unwrap();
        let rich = arms
            .iter()
            .find(|state| state.arm == BudgetArm::Rich)
            .unwrap();
        let balanced = arms
            .iter()
            .find(|state| state.arm == BudgetArm::Balanced)
            .unwrap();

        assert_eq!(rich.pull_count, 5);
        assert!((rich.total_reward - 1.5).abs() < f32::EPSILON);
        assert_eq!(balanced.pull_count, 4);
        assert!((balanced.total_reward - 2.0).abs() < 1e-6);
        assert_eq!(policy.select_arm(&category, 10), BudgetArm::Balanced);
    }

    #[test]
    fn save_and_load_round_trips_regressed_state() {
        let db = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE lcap_arms (
                    category TEXT NOT NULL,
                    arm TEXT NOT NULL,
                    pull_count INTEGER NOT NULL DEFAULT 0,
                    total_reward REAL NOT NULL DEFAULT 0,
                    updated_at TEXT NOT NULL,
                    PRIMARY KEY(category, arm)
                );",
            )
            .unwrap();

        let mut policy = LcapPolicy::new();
        let category = TaskCategory::Research;
        policy.regress(&category, &BudgetArm::MemoryHeavy);
        policy.save_to_db(&db).unwrap();

        let loaded = LcapPolicy::load_from_db(&db).unwrap();
        let arms = loaded.arms.get(&category).unwrap();
        let memory_heavy = arms
            .iter()
            .find(|state| state.arm == BudgetArm::MemoryHeavy)
            .unwrap();
        let rich = arms
            .iter()
            .find(|state| state.arm == BudgetArm::Rich)
            .unwrap();

        assert_eq!(memory_heavy.pull_count, 1);
        assert!((memory_heavy.total_reward + 0.5).abs() < 1e-6);
        assert_eq!(rich.pull_count, 0);
        assert!((rich.total_reward - 0.0).abs() < 1e-6);
    }

    #[test]
    fn learned_policy_activates_for_general_loops_once_history_exists() {
        let mut policy = LcapPolicy::new();
        let category = TaskCategory::ToolUse;

        {
            let arms = policy.arms.get_mut(&category).unwrap();
            for state in arms.iter_mut() {
                state.pull_count = 3;
                state.total_reward = 0.0;
            }
            let rich = arms
                .iter_mut()
                .find(|state| state.arm == BudgetArm::Rich)
                .unwrap();
            rich.total_reward = 3.0;
        }

        assert_eq!(policy.total_pulls(), 15);
        assert_eq!(policy.select_arm(&category, 0), BudgetArm::Rich);
    }
}
