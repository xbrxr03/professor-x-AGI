/// Predictive Self-Model (Seed 7).
///
/// Anil Seth ("Being You", 2021): consciousness IS the running predictive
/// model, not the sensory input. The self is a "controlled hallucination" —
/// the brain predicts its own states and the world; reality only corrects the
/// model at the edges (prediction errors). The "I" is the perspective from
/// which predictions are made.
///
/// The radical move: predicting your own FUTURE BEHAVIOUR. Not "I know what I
/// am" but "I know what I will do." An agent that accurately predicts its own
/// tool calls, step count, and failure modes has a working self-model. The
/// dimensions where prediction stays WRONG are the agent's genuine
/// self-ignorance — the parts of its own processing it does not understand.
///
/// Before each task: predict. After: measure per-dimension error. Track the
/// errors over rounds. Convergence = developing self-knowledge. Persistent
/// divergence on a dimension = a blind spot worth a self-authored test.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// A prediction the agent makes about its own behaviour, before a task runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfPrediction {
    /// Tools the agent expects to call (names only, order-agnostic).
    pub expected_tools: Vec<String>,
    /// Expected number of execution steps.
    pub expected_steps: u32,
    /// Expected probability of success [0,1].
    pub expected_success: f32,
    /// Expected failure mode if it fails (free text, or None if confident).
    pub expected_failure_mode: Option<String>,
}

impl SelfPrediction {
    /// A neutral default used when no prediction could be parsed.
    pub fn uninformed() -> Self {
        Self {
            expected_tools: Vec::new(),
            expected_steps: 5,
            expected_success: 0.5,
            expected_failure_mode: None,
        }
    }

    /// Per-dimension self-prediction error against the actual outcome.
    /// Each component in [0,1]; lower = better self-knowledge.
    pub fn error_against(
        &self,
        actual_tools: &[String],
        actual_steps: u32,
        actual_success: bool,
    ) -> SelfPredictionError {
        // Tool-set Jaccard distance
        let predicted: std::collections::HashSet<&str> =
            self.expected_tools.iter().map(|s| s.as_str()).collect();
        let actual: std::collections::HashSet<&str> =
            actual_tools.iter().map(|s| s.as_str()).collect();
        let tool_err = if predicted.is_empty() && actual.is_empty() {
            0.0
        } else {
            let inter = predicted.intersection(&actual).count() as f32;
            let union = predicted.union(&actual).count() as f32;
            1.0 - (inter / union.max(1.0))
        };

        // Step-count relative error, saturating
        let step_err = {
            let diff = (self.expected_steps as f32 - actual_steps as f32).abs();
            (diff / (actual_steps.max(1) as f32)).min(1.0)
        };

        // Success prediction error
        let actual_s = if actual_success { 1.0 } else { 0.0 };
        let success_err = (self.expected_success - actual_s).abs();

        SelfPredictionError {
            tool_err,
            step_err,
            success_err,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfPredictionError {
    pub tool_err: f32,
    pub step_err: f32,
    pub success_err: f32,
}

impl SelfPredictionError {
    /// Aggregate self-ignorance [0,1] — mean of the three dimensions.
    pub fn aggregate(&self) -> f32 {
        (self.tool_err + self.step_err + self.success_err) / 3.0
    }
}

/// Metacognitive-sensitivity result (Type-2 AUROC + calibration gap).
#[derive(Debug, Clone)]
pub struct MetacogResult {
    /// Type-2 AUROC: P(confidence_correct > confidence_incorrect). 0.5 = none.
    pub auroc: f32,
    pub n: usize,
    pub n_correct: usize,
    pub mean_conf_correct: f32,
    pub mean_conf_incorrect: f32,
}

#[derive(Clone)]
pub struct SelfPredictionStore {
    db: Arc<Mutex<Connection>>,
}

impl SelfPredictionStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn record(
        &self,
        session_id: &str,
        round: u32,
        task_category: &str,
        prediction: &SelfPrediction,
        error: &SelfPredictionError,
    ) -> Result<i64> {
        let tools_json = serde_json::to_string(&prediction.expected_tools)?;
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO self_predictions
             (session_id, round, task_category, expected_tools, expected_steps,
              expected_success, expected_failure_mode, tool_err, step_err,
              success_err, recorded_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                session_id,
                round as i64,
                task_category,
                tools_json,
                prediction.expected_steps as i64,
                prediction.expected_success as f64,
                prediction.expected_failure_mode,
                error.tool_err as f64,
                error.step_err as f64,
                error.success_err as f64,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// Mean aggregate self-prediction error over the most recent `n` records.
    /// Decreasing over rounds = the agent is developing accurate self-knowledge.
    pub fn mean_error(&self, n: usize) -> Result<Option<f32>> {
        let db = self.db.lock().unwrap();
        let avg: Option<f64> = db
            .query_row(
                "SELECT AVG((tool_err + step_err + success_err) / 3.0) FROM (
                    SELECT tool_err, step_err, success_err FROM self_predictions
                    ORDER BY id DESC LIMIT ?1
                 )",
                params![n as i64],
                |row| row.get::<_, Option<f64>>(0),
            )
            .ok()
            .flatten();
        Ok(avg.map(|v| v as f32))
    }

    /// Per-dimension mean error over recent records — surfaces the agent's
    /// specific blind spots (which dimension stays hardest to predict).
    pub fn mean_error_by_dimension(&self, n: usize) -> Result<Option<SelfPredictionError>> {
        let db = self.db.lock().unwrap();
        let row = db
            .query_row(
                "SELECT AVG(tool_err), AVG(step_err), AVG(success_err) FROM (
                    SELECT tool_err, step_err, success_err FROM self_predictions
                    ORDER BY id DESC LIMIT ?1
                 )",
                params![n as i64],
                |row| {
                    Ok((
                        row.get::<_, Option<f64>>(0)?,
                        row.get::<_, Option<f64>>(1)?,
                        row.get::<_, Option<f64>>(2)?,
                    ))
                },
            )
            .ok();
        match row {
            Some((Some(t), Some(s), Some(su))) => Ok(Some(SelfPredictionError {
                tool_err: t as f32,
                step_err: s as f32,
                success_err: su as f32,
            })),
            _ => Ok(None),
        }
    }

    pub fn count(&self) -> Result<i64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT COUNT(*) FROM self_predictions", [], |r| r.get(0))?)
    }

    /// Metacognitive sensitivity — Type-2 AUROC (Fleming & Lau 2014). Measures
    /// how well the agent's PRE-task confidence (expected_success) discriminates
    /// its OWN correct from incorrect outcomes. 0.5 = no metacognition
    /// (confidence is noise); > 0.5 = genuine self-monitoring, the operational
    /// signature of Higher-Order Theories of consciousness. Model-free (no SDT
    /// fit). Actual correctness is recovered from (expected_success, success_err):
    /// success_err = |expected - actual|, so actual=1 ⟺ expected+err≈1.
    pub fn metacognitive_auroc(&self, n: usize) -> Result<Option<MetacogResult>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT expected_success, success_err FROM self_predictions
             ORDER BY id DESC LIMIT ?1",
        )?;
        let rows: Vec<(f64, f64)> = stmt
            .query_map(params![n as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        // Recover (confidence, correct) per trial.
        let mut data: Vec<(f32, bool)> = Vec::with_capacity(rows.len());
        for (conf, serr) in rows {
            let conf = conf as f32;
            let serr = serr as f32;
            // actual=1 ⟹ serr=1-conf (so conf+serr≈1); actual=0 ⟹ serr=conf.
            let correct = (conf + serr - 1.0).abs() < (conf - serr).abs();
            data.push((conf, correct));
        }
        let n_correct = data.iter().filter(|(_, c)| *c).count();
        let n_incorrect = data.len() - n_correct;
        if n_correct == 0 || n_incorrect == 0 {
            return Ok(None); // need both classes to discriminate
        }

        // AUROC via Mann-Whitney U on confidence ranks (average ranks for ties).
        let mut idx: Vec<usize> = (0..data.len()).collect();
        idx.sort_by(|&a, &b| data[a].0.partial_cmp(&data[b].0).unwrap_or(std::cmp::Ordering::Equal));
        let mut ranks = vec![0.0f64; data.len()];
        let mut i = 0;
        while i < idx.len() {
            let mut j = i;
            while j + 1 < idx.len() && (data[idx[j + 1]].0 - data[idx[i]].0).abs() < 1e-9 {
                j += 1;
            }
            // ranks i..=j are tied → average rank (1-based)
            let avg = ((i + 1) + (j + 1)) as f64 / 2.0;
            for k in i..=j {
                ranks[idx[k]] = avg;
            }
            i = j + 1;
        }
        let rank_sum_correct: f64 = data
            .iter()
            .enumerate()
            .filter(|(_, (_, c))| *c)
            .map(|(k, _)| ranks[k])
            .sum();
        let nc = n_correct as f64;
        let ni = n_incorrect as f64;
        let u_correct = rank_sum_correct - nc * (nc + 1.0) / 2.0;
        let auroc = (u_correct / (nc * ni)) as f32;

        // Calibration gap: mean confidence on correct vs incorrect trials.
        let mean_conf = |want: bool| -> f32 {
            let v: Vec<f32> = data.iter().filter(|(_, c)| *c == want).map(|(x, _)| *x).collect();
            if v.is_empty() { 0.0 } else { v.iter().sum::<f32>() / v.len() as f32 }
        };
        Ok(Some(MetacogResult {
            auroc,
            n: data.len(),
            n_correct,
            mean_conf_correct: mean_conf(true),
            mean_conf_incorrect: mean_conf(false),
        }))
    }
}

/// Build the prompt that asks the agent to predict its own behaviour.
pub fn build_prediction_prompt(task_description: &str, available_tools: &[&str]) -> String {
    format!(
        "Before attempting this task, predict your OWN behaviour. Be honest — \
         this measures how well you know yourself.\n\n\
         Task: {task_description}\n\n\
         Available tools: {}\n\n\
         Answer in exactly this format:\n\
         TOOLS: <comma-separated tool names you expect to use>\n\
         STEPS: <integer — how many actions you expect to take>\n\
         SUCCESS: <0.0-1.0 — your probability of succeeding>\n\
         FAILURE_MODE: <if you fail, how? one phrase, or 'none' if confident>",
        available_tools.join(", "),
    )
}

/// Parse the agent's self-prediction from its response.
pub fn parse_prediction(text: &str) -> SelfPrediction {
    let mut pred = SelfPrediction::uninformed();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("TOOLS:") {
            pred.expected_tools = rest
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s.len() < 40)
                .collect();
        } else if let Some(rest) = line.strip_prefix("STEPS:") {
            if let Ok(n) = rest.trim().parse::<u32>() {
                pred.expected_steps = n.min(50);
            }
        } else if let Some(rest) = line.strip_prefix("SUCCESS:") {
            if let Ok(p) = rest.trim().parse::<f32>() {
                pred.expected_success = p.clamp(0.0, 1.0);
            }
        } else if let Some(rest) = line.strip_prefix("FAILURE_MODE:") {
            let fm = rest.trim();
            if !fm.is_empty() && fm.to_lowercase() != "none" {
                pred.expected_failure_mode = Some(fm.to_string());
            }
        }
    }
    pred
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> SelfPredictionStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE self_predictions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                task_category TEXT NOT NULL,
                expected_tools TEXT NOT NULL,
                expected_steps INTEGER NOT NULL,
                expected_success REAL NOT NULL,
                expected_failure_mode TEXT,
                tool_err REAL NOT NULL,
                step_err REAL NOT NULL,
                success_err REAL NOT NULL,
                recorded_at TEXT NOT NULL
            );",
        )
        .unwrap();
        SelfPredictionStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn perfect_prediction_has_zero_error() {
        let pred = SelfPrediction {
            expected_tools: vec!["fs.read".to_string(), "fs.write".to_string()],
            expected_steps: 3,
            expected_success: 1.0,
            expected_failure_mode: None,
        };
        let err = pred.error_against(
            &["fs.read".to_string(), "fs.write".to_string()],
            3,
            true,
        );
        assert!(err.aggregate() < 1e-6);
    }

    #[test]
    fn wrong_tools_raise_tool_error() {
        let pred = SelfPrediction {
            expected_tools: vec!["web.search".to_string()],
            expected_steps: 3,
            expected_success: 0.5,
            expected_failure_mode: None,
        };
        let err = pred.error_against(&["fs.read".to_string()], 3, true);
        assert!(err.tool_err > 0.9); // disjoint sets
    }

    #[test]
    fn step_error_saturates() {
        let pred = SelfPrediction {
            expected_tools: vec![],
            expected_steps: 100,
            expected_success: 0.5,
            expected_failure_mode: None,
        };
        let err = pred.error_against(&[], 2, false);
        assert!(err.step_err <= 1.0);
    }

    #[test]
    fn parse_prediction_reads_all_fields() {
        let text = "TOOLS: fs.read, memory.read\nSTEPS: 4\nSUCCESS: 0.8\nFAILURE_MODE: wrong file path";
        let p = parse_prediction(text);
        assert_eq!(p.expected_tools.len(), 2);
        assert_eq!(p.expected_steps, 4);
        assert!((p.expected_success - 0.8).abs() < 1e-6);
        assert_eq!(p.expected_failure_mode.as_deref(), Some("wrong file path"));
    }

    #[test]
    fn record_and_mean_error() {
        let store = fresh_store();
        let pred = SelfPrediction::uninformed();
        let err = SelfPredictionError { tool_err: 0.2, step_err: 0.4, success_err: 0.3 };
        store.record("s", 0, "planning", &pred, &err).unwrap();
        let mean = store.mean_error(10).unwrap().unwrap();
        assert!((mean - 0.3).abs() < 1e-5); // (0.2+0.4+0.3)/3
    }

    #[test]
    fn mean_error_by_dimension_surfaces_blind_spots() {
        let store = fresh_store();
        let pred = SelfPrediction::uninformed();
        // step_err consistently high — that's the blind spot
        for _ in 0..3 {
            let err = SelfPredictionError { tool_err: 0.1, step_err: 0.9, success_err: 0.1 };
            store.record("s", 0, "planning", &pred, &err).unwrap();
        }
        let dims = store.mean_error_by_dimension(10).unwrap().unwrap();
        assert!(dims.step_err > dims.tool_err);
        assert!(dims.step_err > dims.success_err);
    }
}
