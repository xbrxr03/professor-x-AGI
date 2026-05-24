/// Outcome tracker — records task results for evolved to learn from.
/// Every completed/failed task feeds into the evolution cycle.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutcome {
    pub task_id: Uuid,
    pub description: String,
    pub success: bool,
    pub score: f32,
    pub failure_mode: Option<String>,
    pub steps_taken: u32,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone)]
pub struct OutcomeTracker {
    /// Bounded ring buffer — last 100 outcomes kept in memory.
    outcomes: VecDeque<TaskOutcome>,
    capacity: usize,
}

impl OutcomeTracker {
    pub fn new() -> Self {
        Self {
            outcomes: VecDeque::with_capacity(100),
            capacity: 100,
        }
    }

    pub fn record(&mut self, outcome: TaskOutcome) {
        if self.outcomes.len() >= self.capacity {
            self.outcomes.pop_front();
        }
        self.outcomes.push_back(outcome);
    }

    /// Return recent outcomes for the evolution cycle to analyze.
    pub fn recent(&self, n: usize) -> Vec<&TaskOutcome> {
        self.outcomes.iter().rev().take(n).collect()
    }

    /// Identify recurring failure modes across recent outcomes.
    pub fn failure_patterns(&self, window: usize) -> Vec<String> {
        let recent: Vec<_> = self.outcomes.iter().rev().take(window).collect();
        let failures: Vec<_> = recent.iter()
            .filter(|o| !o.success)
            .filter_map(|o| o.failure_mode.as_deref())
            .collect();

        if failures.is_empty() {
            return Vec::new();
        }

        // Count frequency of each failure mode
        let mut counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
        for f in &failures {
            *counts.entry(f).or_insert(0) += 1;
        }

        let mut patterns: Vec<_> = counts.iter().collect();
        patterns.sort_by(|a, b| b.1.cmp(a.1));
        patterns.iter()
            .filter(|(_, count)| **count >= 2)
            .map(|(mode, count)| format!("{mode} (x{count})"))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    pub fn success_rate(&self, window: usize) -> f32 {
        let recent: Vec<_> = self.outcomes.iter().rev().take(window).collect();
        if recent.is_empty() { return 0.0; }
        let successes = recent.iter().filter(|o| o.success).count();
        successes as f32 / recent.len() as f32
    }
}
