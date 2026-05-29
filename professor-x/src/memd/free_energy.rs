//! Free Energy Delta (paper outline §4.8, H15).
//!
//! Per-session record of how well Professor X predicted his own task
//! outcomes. The Free Energy Principle predicts that a persistent agent
//! minimises prediction error over time — H15 tests whether the
//! per-session mean absolute error of (predicted_success - actual_success)
//! shows a downward trend across HIRO rounds.
//!
//! Lineage:
//! - Friston, "A free energy principle for the brain" — agents minimise
//!   surprise / variational free energy.
//! - This module operationalises FED as `mean_abs(predicted - actual)`
//!   over the tasks in a session.
//!
//! **Stub status:** struct + persistence + accessor are real. The
//! prediction-recording hook in `react.rs` (where each task's
//! `predicted_success` is set before execution) is a TODO — the existing
//! ReAct loop doesn't capture per-task predictions yet.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FedRecord {
    pub id: Option<i64>,
    pub session_id: String,
    pub round: u32,
    pub n_predictions: u32,
    pub mean_abs_error: f32,
    pub recorded_at: DateTime<Utc>,
}

impl FedRecord {
    pub fn new(
        session_id: impl Into<String>,
        round: u32,
        n_predictions: u32,
        mean_abs_error: f32,
    ) -> Self {
        Self {
            id: None,
            session_id: session_id.into(),
            round,
            n_predictions,
            mean_abs_error,
            recorded_at: Utc::now(),
        }
    }
}

/// Compute FED for a slice of `(predicted, actual)` pairs.
/// Returns `(mean_abs_error, sample_size)`. Empty input is `(0.0, 0)`.
pub fn compute_fed(samples: &[(f32, f32)]) -> (f32, usize) {
    if samples.is_empty() {
        return (0.0, 0);
    }
    let total: f32 = samples.iter().map(|(p, a)| (p - a).abs()).sum();
    (total / samples.len() as f32, samples.len())
}

#[derive(Clone)]
pub struct FreeEnergyStore {
    db: Arc<Mutex<Connection>>,
}

impl FreeEnergyStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn append(&self, record: &FedRecord) -> Result<i64> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO fed_records
             (session_id, round, n_predictions, mean_abs_error, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                record.session_id,
                record.round as i64,
                record.n_predictions as i64,
                record.mean_abs_error as f64,
                record.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// FED trajectory across rounds, oldest first.
    /// H15 plot input: `[(round, mean_abs_error)]`.
    pub fn trajectory(&self) -> Result<Vec<(u32, f32)>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT round, AVG(mean_abs_error)
             FROM fed_records
             GROUP BY round
             ORDER BY round ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)? as u32, row.get::<_, f64>(1)? as f32))
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    /// Simple linear-regression slope over the round trajectory. Negative
    /// slope → FED is decreasing → world model improving. H15 success
    /// criterion: slope < 0 (and ideally p < 0.10 — significance test
    /// deferred until a stats helper lands).
    pub fn slope_per_round(&self) -> Result<Option<f32>> {
        let traj = self.trajectory()?;
        if traj.len() < 2 {
            return Ok(None);
        }
        let n = traj.len() as f32;
        let mean_x = traj.iter().map(|(x, _)| *x as f32).sum::<f32>() / n;
        let mean_y = traj.iter().map(|(_, y)| *y).sum::<f32>() / n;
        let mut num = 0.0;
        let mut den = 0.0;
        for (x, y) in &traj {
            let dx = *x as f32 - mean_x;
            num += dx * (*y - mean_y);
            den += dx * dx;
        }
        if den == 0.0 {
            return Ok(None);
        }
        Ok(Some(num / den))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> FreeEnergyStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE fed_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                n_predictions INTEGER NOT NULL,
                mean_abs_error REAL NOT NULL,
                recorded_at TEXT NOT NULL
            );",
        )
        .unwrap();
        FreeEnergyStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn fed_empty_input_is_zero() {
        let (e, n) = compute_fed(&[]);
        assert_eq!(e, 0.0);
        assert_eq!(n, 0);
    }

    #[test]
    fn fed_computes_mean_abs_error() {
        let s = vec![(1.0, 0.0), (0.0, 1.0), (0.5, 0.5)];
        let (e, n) = compute_fed(&s);
        assert_eq!(n, 3);
        assert!((e - (1.0 + 1.0 + 0.0) / 3.0).abs() < 1e-6);
    }

    #[test]
    fn slope_negative_when_fed_decreases() {
        let store = fresh_store();
        store.append(&FedRecord::new("s", 0, 10, 0.50)).unwrap();
        store.append(&FedRecord::new("s", 1, 10, 0.40)).unwrap();
        store.append(&FedRecord::new("s", 2, 10, 0.30)).unwrap();
        store.append(&FedRecord::new("s", 3, 10, 0.25)).unwrap();
        let slope = store.slope_per_round().unwrap().unwrap();
        assert!(slope < 0.0, "expected negative slope, got {slope}");
    }

    #[test]
    fn slope_positive_when_fed_increases() {
        let store = fresh_store();
        store.append(&FedRecord::new("s", 0, 10, 0.10)).unwrap();
        store.append(&FedRecord::new("s", 1, 10, 0.30)).unwrap();
        store.append(&FedRecord::new("s", 2, 10, 0.50)).unwrap();
        let slope = store.slope_per_round().unwrap().unwrap();
        assert!(slope > 0.0, "expected positive slope, got {slope}");
    }

    #[test]
    fn slope_none_with_single_round() {
        let store = fresh_store();
        store.append(&FedRecord::new("s", 0, 10, 0.5)).unwrap();
        assert!(store.slope_per_round().unwrap().is_none());
    }

    #[test]
    fn trajectory_averages_within_round() {
        let store = fresh_store();
        store.append(&FedRecord::new("a", 0, 5, 0.4)).unwrap();
        store.append(&FedRecord::new("b", 0, 5, 0.6)).unwrap();
        let traj = store.trajectory().unwrap();
        assert_eq!(traj.len(), 1);
        assert!((traj[0].1 - 0.5).abs() < 1e-6);
    }
}
