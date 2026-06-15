/// STDP-inspired causal trace store.
///
/// Spike-Timing Dependent Plasticity (Markram, 1997): the brain learns causal
/// sequences, not correlations. Neurons that fire BEFORE an outcome strengthen
/// their causal connection; those that fire after weaken it. The key is timing.
///
/// For Professor X: instead of learning "memory.read correlates with success,"
/// we learn "memory.read called 3 steps before the decisive fs.write in planning
/// tasks reliably predicts success." Order and timing matter.
///
/// After N rounds, `extract_patterns` returns the causal sequences that reliably
/// precede success across task categories — these feed the Researcher as context
/// and inform LCAP arm selection.
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// One action in a causal sequence, with timing relative to task completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimedAction {
    /// Tool name called
    pub tool: String,
    /// Milliseconds before task outcome (positive = before completion)
    pub ms_before_outcome: i64,
    /// Whether the individual tool call succeeded
    pub succeeded: bool,
}

/// A complete causal trace for one task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalTrace {
    pub id: Option<i64>,
    pub session_id: String,
    pub task_id: String,
    pub task_category: String,
    /// Actions in execution order, with timing relative to completion
    pub actions: Vec<TimedAction>,
    /// Did the task ultimately succeed?
    pub outcome: bool,
    pub outcome_score: f32,
    pub created_at: DateTime<Utc>,
}

impl CausalTrace {
    pub fn new(
        session_id: impl Into<String>,
        task_id: impl Into<String>,
        task_category: impl Into<String>,
        actions: Vec<TimedAction>,
        outcome: bool,
        outcome_score: f32,
    ) -> Self {
        Self {
            id: None,
            session_id: session_id.into(),
            task_id: task_id.into(),
            task_category: task_category.into(),
            actions,
            outcome,
            outcome_score,
            created_at: Utc::now(),
        }
    }

    /// Extract the STDP window: tools called within `window_ms` of the outcome.
    /// These are the causally relevant actions — the ones that "fired before" success.
    pub fn stdp_window(&self, window_ms: i64) -> Vec<&TimedAction> {
        self.actions
            .iter()
            .filter(|a| a.ms_before_outcome >= 0 && a.ms_before_outcome <= window_ms)
            .collect()
    }
}

/// A causal pattern extracted from many traces: a tool sequence that reliably
/// precedes success in a specific task category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalPattern {
    pub category: String,
    /// The tool sequence (order matters — this is the causal chain)
    pub sequence: Vec<String>,
    /// How many traces contain this sequence
    pub occurrence_count: u32,
    /// Fraction of occurrences that led to success
    pub success_rate: f32,
    /// Mean time-before-outcome for the decisive action (ms)
    pub mean_timing_ms: f32,
}

impl CausalPattern {
    /// STDP-weighted strength: combines frequency, success rate, and timing proximity.
    /// Patterns that fire close to the outcome and reliably succeed are strongest.
    pub fn strength(&self) -> f32 {
        let frequency = (self.occurrence_count as f32).ln() + 1.0;
        let timing_weight = 1.0 / (1.0 + self.mean_timing_ms / 5000.0); // decay over 5s
        frequency * self.success_rate * timing_weight
    }
}

#[derive(Clone)]
pub struct CausalTraceStore {
    db: Arc<Mutex<Connection>>,
}

impl CausalTraceStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn insert(&self, trace: &CausalTrace) -> Result<i64> {
        let actions_json = serde_json::to_string(&trace.actions)?;
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO causal_traces
             (session_id, task_id, task_category, actions, outcome, outcome_score, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                trace.session_id,
                trace.task_id,
                trace.task_category,
                actions_json,
                trace.outcome as i64,
                trace.outcome_score as f64,
                trace.created_at.to_rfc3339(),
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// Extract causal patterns from traces: tool sequences that reliably precede
    /// success within the STDP window.
    ///
    /// `min_occurrences`: only patterns seen this many times qualify
    /// `min_success_rate`: only reliable patterns qualify
    /// `stdp_window_ms`: how close to outcome counts as "causal"
    pub fn extract_patterns(
        &self,
        category: Option<&str>,
        min_occurrences: u32,
        min_success_rate: f32,
        stdp_window_ms: i64,
    ) -> Result<Vec<CausalPattern>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT task_category, actions, outcome, outcome_score
             FROM causal_traces
             ORDER BY created_at DESC
             LIMIT 500",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, f64>(3)? as f32,
            ))
        })?;

        // Accumulate bigrams (pairs of consecutive tools) within STDP window
        // keyed by (category, tool_a, tool_b)
        let mut pattern_stats: HashMap<(String, String, String), (u32, u32)> = HashMap::new();
        let mut pattern_timing: HashMap<(String, String, String), Vec<f32>> = HashMap::new();

        for row in rows.flatten() {
            let (cat, actions_json, outcome, _score) = row;
            if let Some(filter) = category {
                if cat != filter {
                    continue;
                }
            }
            let actions: Vec<TimedAction> = serde_json::from_str(&actions_json).unwrap_or_default();

            // Extract tools in STDP window
            let window_actions: Vec<&TimedAction> = actions
                .iter()
                .filter(|a| a.ms_before_outcome >= 0 && a.ms_before_outcome <= stdp_window_ms)
                .collect();

            for pair in window_actions.windows(2) {
                let key = (cat.clone(), pair[0].tool.clone(), pair[1].tool.clone());
                let entry = pattern_stats.entry(key.clone()).or_insert((0, 0));
                entry.0 += 1;
                if outcome {
                    entry.1 += 1;
                }
                pattern_timing
                    .entry(key)
                    .or_default()
                    .push(pair[1].ms_before_outcome as f32);
            }
        }

        let mut patterns: Vec<CausalPattern> = pattern_stats
            .into_iter()
            .filter(|(_, (total, _))| *total >= min_occurrences)
            .filter(|(_, (total, successes))| *successes as f32 / *total as f32 >= min_success_rate)
            .map(|((cat, tool_a, tool_b), (total, successes))| {
                let key = (cat.clone(), tool_a.clone(), tool_b.clone());
                let timings = pattern_timing.get(&key).cloned().unwrap_or_default();
                let mean_timing = if timings.is_empty() {
                    0.0
                } else {
                    timings.iter().sum::<f32>() / timings.len() as f32
                };
                CausalPattern {
                    category: cat,
                    sequence: vec![tool_a, tool_b],
                    occurrence_count: total,
                    success_rate: successes as f32 / total as f32,
                    mean_timing_ms: mean_timing,
                }
            })
            .collect();

        // Sort by STDP strength (frequency × success × timing proximity)
        patterns.sort_by(|a, b| {
            b.strength()
                .partial_cmp(&a.strength())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(patterns)
    }

    /// Format top patterns as a compact string for Researcher context injection.
    pub fn format_for_context(&self, category: &str, top_n: usize) -> Result<String> {
        let patterns = self.extract_patterns(Some(category), 3, 0.6, 10_000)?;
        if patterns.is_empty() {
            return Ok(String::new());
        }
        let lines: Vec<String> = patterns
            .into_iter()
            .take(top_n)
            .map(|p| {
                format!(
                    "  {} → {} (success={:.0}%, n={}, strength={:.2})",
                    p.sequence[0],
                    p.sequence[1],
                    p.success_rate * 100.0,
                    p.occurrence_count,
                    p.strength()
                )
            })
            .collect();
        Ok(format!(
            "Causal patterns for {} tasks:\n{}",
            category,
            lines.join("\n")
        ))
    }

    pub fn count(&self) -> Result<i64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT COUNT(*) FROM causal_traces", [], |r| r.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> CausalTraceStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE causal_traces (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                task_category TEXT NOT NULL,
                actions TEXT NOT NULL,
                outcome INTEGER NOT NULL,
                outcome_score REAL NOT NULL,
                created_at TEXT NOT NULL
            );",
        )
        .unwrap();
        CausalTraceStore::new(Arc::new(Mutex::new(conn)))
    }

    fn make_trace(category: &str, tools: &[&str], outcome: bool) -> CausalTrace {
        let now = Utc::now();
        let actions = tools
            .iter()
            .enumerate()
            .map(|(i, t)| TimedAction {
                tool: t.to_string(),
                ms_before_outcome: ((tools.len() - i) as i64 * 1000),
                succeeded: true,
            })
            .collect();
        CausalTrace::new(
            "sess",
            "task",
            category,
            actions,
            outcome,
            if outcome { 1.0 } else { 0.0 },
        )
    }

    #[test]
    fn insert_and_count() {
        let store = fresh_store();
        let trace = make_trace("planning", &["memory.read", "fs.read", "fs.write"], true);
        store.insert(&trace).unwrap();
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn stdp_window_filters_by_timing() {
        let trace = make_trace("planning", &["memory.read", "fs.read", "fs.write"], true);
        // tools are at 3000ms, 2000ms, 1000ms before outcome
        let window_5s = trace.stdp_window(5000);
        assert_eq!(window_5s.len(), 3);
        let window_1500ms = trace.stdp_window(1500);
        assert_eq!(window_1500ms.len(), 1); // only fs.write at 1000ms
    }

    #[test]
    fn extract_patterns_finds_reliable_sequences() {
        let store = fresh_store();
        // Insert 5 successful traces with same pattern
        for _ in 0..5 {
            let t = make_trace("planning", &["memory.read", "fs.write"], true);
            store.insert(&t).unwrap();
        }
        // Insert 1 failure
        let t = make_trace("planning", &["memory.read", "fs.write"], false);
        store.insert(&t).unwrap();

        let patterns = store
            .extract_patterns(Some("planning"), 3, 0.6, 10_000)
            .unwrap();
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].sequence, vec!["memory.read", "fs.write"]);
        assert!((patterns[0].success_rate - 5.0 / 6.0).abs() < 0.01);
    }

    #[test]
    fn pattern_strength_prefers_high_success_and_close_timing() {
        let high = CausalPattern {
            category: "c".to_string(),
            sequence: vec!["a".to_string(), "b".to_string()],
            occurrence_count: 10,
            success_rate: 0.9,
            mean_timing_ms: 500.0,
        };
        let low = CausalPattern {
            category: "c".to_string(),
            sequence: vec!["a".to_string(), "b".to_string()],
            occurrence_count: 10,
            success_rate: 0.3,
            mean_timing_ms: 8000.0,
        };
        assert!(high.strength() > low.strength());
    }
}
