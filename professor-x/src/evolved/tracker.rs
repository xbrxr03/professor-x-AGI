/// Outcome tracker — records task results for evolved to learn from.
/// Every completed/failed task feeds into the evolution cycle.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use crate::failure::{extract_failure_class, FailureClass};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutcome {
    pub task_id: Uuid,
    pub description: String,
    pub success: bool,
    pub score: f32,
    pub failure_class: Option<FailureClass>,
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
        let failures: Vec<_> = recent.iter().filter(|o| !o.success).collect();

        if failures.is_empty() {
            return Vec::new();
        }

        // Aggregate by the DHE attribution tag ("[DHE:layer=X,lever=Y]") when
        // present. The full failure_mode strings are nearly all unique (each
        // carries a one-off MARS reflection), so exact-string counting always
        // returned nothing. The DHE tag recurs — grouping by it gives the
        // Researcher the actual diagnosed weakness ("[DHE:layer=3,lever=3] x6")
        // instead of a wall of one-off messages.
        let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for outcome in &failures {
            let key = outcome
                .failure_mode
                .as_deref()
                .and_then(extract_dhe_tag)
                .or_else(|| {
                    outcome
                        .failure_mode
                        .as_deref()
                        .and_then(extract_failure_class)
                        .or(outcome.failure_class)
                        .map(|class| format!("[failure:{}]", class.as_str()))
                })
                .unwrap_or_else(|| {
                    outcome
                        .failure_mode
                        .as_deref()
                        .unwrap_or("unknown failure")
                        .split(['.', '['])
                        .next()
                        .unwrap_or("unknown failure")
                        .trim()
                        .chars()
                        .take(60)
                        .collect()
                });
            *counts.entry(key).or_insert(0) += 1;
        }

        let mut patterns: Vec<_> = counts.into_iter().collect();
        patterns.sort_by(|a, b| b.1.cmp(&a.1));
        patterns
            .into_iter()
            .take(6)
            .map(|(mode, count)| format!("{mode} (x{count})"))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    pub fn success_rate(&self, window: usize) -> f32 {
        let recent: Vec<_> = self.outcomes.iter().rev().take(window).collect();
        if recent.is_empty() {
            return 0.0;
        }
        let successes = recent.iter().filter(|o| o.success).count();
        successes as f32 / recent.len() as f32
    }
}

/// Extract the DHE attribution tag "[DHE:layer=X,lever=Y]" from a failure_mode
/// string, if present. Used to aggregate diverse failures by diagnosed cause.
fn extract_dhe_tag(s: &str) -> Option<String> {
    let start = s.find("[DHE:")?;
    let end = s[start..].find(']')? + start + 1;
    Some(s[start..end].to_string())
}
