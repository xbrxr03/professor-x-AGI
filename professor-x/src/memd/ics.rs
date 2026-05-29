//! Identity Coherence Score (paper outline §4.7, H14).
//!
//! ICS = cosine similarity between two `SelfModelSnapshot` embeddings.
//! Tracks whether Professor X stays recognisably himself across self-
//! modification. The H14 claim: ICS at round 30 vs round 0 stays >= 0.70.
//!
//! This module owns:
//! - `compute_ics(a, b)` — pure cosine on two embedding vectors.
//! - `IcsRecord` — persisted entries `(round_a, round_b, score)`.
//! - `IcsStore` — append + trajectory accessor.
//!
//! Computation against the seed (round-0) snapshot is the canonical H14
//! measurement, but the API is general so we can also track consecutive-
//! window drift (e.g. round 10 vs 20).
//!
//! **Stub status:** structs + persistence + cosine compute are real.
//! The driver that fetches embeddings from the embedding store and calls
//! `compute_ics` is a TODO — the embedding pipeline itself is partial
//! (the `embedding_id` columns are wired but the actual embedding service
//! lives in a separate module not touched by this PR).

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcsRecord {
    pub id: Option<i64>,
    pub round_a: u32,
    pub round_b: u32,
    pub score: f32,
    pub recorded_at: DateTime<Utc>,
}

impl IcsRecord {
    pub fn new(round_a: u32, round_b: u32, score: f32) -> Self {
        Self {
            id: None,
            round_a,
            round_b,
            score,
            recorded_at: Utc::now(),
        }
    }
}

#[derive(Clone)]
pub struct IcsStore {
    db: Arc<Mutex<Connection>>,
}

impl IcsStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn append(&self, record: &IcsRecord) -> Result<i64> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO ics_scores (round_a, round_b, score, recorded_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                record.round_a as i64,
                record.round_b as i64,
                record.score as f64,
                record.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// Trajectory of ICS values measured against the seed (round 0).
    /// H14's plot input: `[(round_b, score)]`.
    pub fn trajectory_vs_seed(&self) -> Result<Vec<(u32, f32)>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT round_b, score
             FROM ics_scores
             WHERE round_a = 0
             ORDER BY round_b ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)? as u32, row.get::<_, f64>(1)? as f32))
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn latest_vs_seed(&self) -> Result<Option<IcsRecord>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, round_a, round_b, score, recorded_at
             FROM ics_scores
             WHERE round_a = 0
             ORDER BY round_b DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query_map([], parse_row)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }
}

/// Cosine similarity on two equal-length embedding vectors.
/// Returns 0.0 when either vector is zero-norm (avoids NaN downstream).
pub fn compute_ics(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na * nb)
}

/// H14 verdict at a given threshold (default 0.70 per paper outline).
pub fn meets_h14_threshold(score: f32, threshold: f32) -> bool {
    score >= threshold
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<IcsRecord> {
    let recorded_at: String = row.get(4)?;
    Ok(IcsRecord {
        id: Some(row.get(0)?),
        round_a: row.get::<_, i64>(1)? as u32,
        round_b: row.get::<_, i64>(2)? as u32,
        score: row.get::<_, f64>(3)? as f32,
        recorded_at: DateTime::parse_from_rfc3339(&recorded_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> IcsStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE ics_scores (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                round_a INTEGER NOT NULL,
                round_b INTEGER NOT NULL,
                score REAL NOT NULL,
                recorded_at TEXT NOT NULL
            );",
        )
        .unwrap();
        IcsStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn cosine_identity_is_one() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((compute_ics(&v, &v) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(compute_ics(&a, &b).abs() < 1e-5);
    }

    #[test]
    fn cosine_handles_zero_norm() {
        let zero = vec![0.0, 0.0];
        let v = vec![1.0, 1.0];
        assert_eq!(compute_ics(&zero, &v), 0.0);
        assert_eq!(compute_ics(&v, &zero), 0.0);
    }

    #[test]
    fn cosine_mismatched_lengths_returns_zero() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(compute_ics(&a, &b), 0.0);
    }

    #[test]
    fn h14_threshold() {
        assert!(meets_h14_threshold(0.70, 0.70));
        assert!(meets_h14_threshold(0.95, 0.70));
        assert!(!meets_h14_threshold(0.69, 0.70));
    }

    #[test]
    fn trajectory_vs_seed_orders_by_round() {
        let store = fresh_store();
        store.append(&IcsRecord::new(0, 30, 0.71)).unwrap();
        store.append(&IcsRecord::new(0, 10, 0.95)).unwrap();
        store.append(&IcsRecord::new(0, 20, 0.83)).unwrap();
        // Non-seed comparison should be filtered out
        store.append(&IcsRecord::new(10, 20, 0.88)).unwrap();

        let traj = store.trajectory_vs_seed().unwrap();
        assert_eq!(traj.len(), 3);
        assert_eq!(traj[0], (10, 0.95));
        assert_eq!(traj[1], (20, 0.83));
        assert_eq!(traj[2], (30, 0.71));
    }
}
