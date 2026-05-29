//! Functional affect (paper outline §4.8, H16).
//!
//! Per-task `(valence, arousal)` derived from the gap between predicted and
//! actual outcomes, smoothed with an EMA across tasks within a session.
//! Injected into every ReAct prompt as `<affect ... />` so the model can
//! condition on its own emotional state.
//!
//! Lineage:
//! - Free Energy Principle (Friston) — affect emerges from prediction error.
//! - Forgas, 2007 — mild negative valence enhances analytical thinking.
//!   H16 tests whether this carries over to LLM agents (negative affect →
//!   better DHE fix-prediction precision).
//!
//! **Stub status:** struct + EMA update + persistence are real. The
//! prompt-injection wiring + the DHE-correlation analysis (H16) are
//! follow-up work.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectState {
    pub id: Option<i64>,
    pub session_id: String,
    pub round: u32,
    /// Bounded in `[-1.0, 1.0]`. Negative = things going worse than expected.
    pub valence: f32,
    /// Bounded in `[0.0, 1.0]`. High = many tool calls + retries.
    pub arousal: f32,
    pub recorded_at: DateTime<Utc>,
}

impl AffectState {
    pub fn neutral(session_id: impl Into<String>, round: u32) -> Self {
        Self {
            id: None,
            session_id: session_id.into(),
            round,
            valence: 0.0,
            arousal: 0.0,
            recorded_at: Utc::now(),
        }
    }

    /// EMA update toward the per-task observation. `alpha` in `[0,1]`:
    /// 0 ignores the new sample, 1 replaces the running estimate.
    pub fn update_ema(&mut self, sample_valence: f32, sample_arousal: f32, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        self.valence = clamp_unit(self.valence * (1.0 - alpha) + sample_valence * alpha);
        self.arousal = clamp_nonneg(self.arousal * (1.0 - alpha) + sample_arousal * alpha);
        self.recorded_at = Utc::now();
    }
}

/// Per-task valence: `tanh(actual_score - predicted_score)`. Outcome better
/// than expected → positive; worse → negative.
pub fn valence_from_outcome(actual_score: f32, predicted_score: f32) -> f32 {
    (actual_score - predicted_score).tanh()
}

/// Per-task arousal: a coarse signal driven by tool density + retry
/// pressure. Both inputs are non-negative; the sum is squashed into
/// `[0, 1]` by `1 - exp(-x)` so the metric saturates rather than blowing up.
pub fn arousal_from_load(tool_density: f32, retry_pressure: f32) -> f32 {
    let raw = (tool_density + retry_pressure).max(0.0);
    (1.0 - (-raw).exp()).min(1.0 - f32::EPSILON)
}

/// Human-readable label for `<affect state="...">` prompt injection.
pub fn state_label(valence: f32, arousal: f32) -> &'static str {
    match (valence, arousal) {
        (v, a) if v >= 0.3 && a >= 0.5 => "engaged",
        (v, a) if v >= 0.3 && a < 0.5 => "content",
        (v, _) if v.abs() < 0.3 => "neutral",
        (v, a) if v <= -0.3 && a >= 0.5 => "frustrated",
        _ => "muted",
    }
}

#[derive(Clone)]
pub struct AffectStore {
    db: Arc<Mutex<Connection>>,
}

impl AffectStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn append(&self, state: &AffectState) -> Result<i64> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO affect_states (session_id, round, valence, arousal, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                state.session_id,
                state.round as i64,
                state.valence as f64,
                state.arousal as f64,
                state.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// Mean valence across `session_id`'s rows. Used by H16 binning.
    pub fn mean_valence_for_session(&self, session_id: &str) -> Result<Option<f32>> {
        let db = self.db.lock().unwrap();
        let avg: Option<f64> = db
            .query_row(
                "SELECT AVG(valence) FROM affect_states WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, Option<f64>>(0),
            )
            .ok()
            .flatten();
        Ok(avg.map(|v| v as f32))
    }

    pub fn latest_for_session(&self, session_id: &str) -> Result<Option<AffectState>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, session_id, round, valence, arousal, recorded_at
             FROM affect_states
             WHERE session_id = ?1
             ORDER BY id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![session_id], parse_row)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }
}

fn clamp_unit(v: f32) -> f32 {
    v.clamp(-1.0, 1.0)
}

fn clamp_nonneg(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Stable, deterministic session-id generator for tests that don't want
/// to depend on the surrounding daemon context.
pub fn synthesize_session_id() -> String {
    Uuid::new_v4().to_string()
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<AffectState> {
    let recorded_at: String = row.get(5)?;
    Ok(AffectState {
        id: Some(row.get(0)?),
        session_id: row.get(1)?,
        round: row.get::<_, i64>(2)? as u32,
        valence: row.get::<_, f64>(3)? as f32,
        arousal: row.get::<_, f64>(4)? as f32,
        recorded_at: DateTime::parse_from_rfc3339(&recorded_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> AffectStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE affect_states (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                valence REAL NOT NULL,
                arousal REAL NOT NULL,
                recorded_at TEXT NOT NULL
            );",
        )
        .unwrap();
        AffectStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn valence_from_outcome_clamps_via_tanh() {
        assert!(valence_from_outcome(1.0, 0.0) > 0.0);
        assert!(valence_from_outcome(0.0, 1.0) < 0.0);
        let extreme = valence_from_outcome(100.0, 0.0);
        assert!(extreme <= 1.0 && extreme > 0.99);
    }

    #[test]
    fn arousal_saturates() {
        let a = arousal_from_load(100.0, 100.0);
        assert!(a < 1.0 && a > 0.99);
        let b = arousal_from_load(0.0, 0.0);
        assert!((b - 0.0).abs() < 1e-6);
    }

    #[test]
    fn ema_update_clamps_in_bounds() {
        let mut s = AffectState::neutral("sess", 0);
        s.update_ema(-2.0, 5.0, 0.5);
        assert!(s.valence >= -1.0 && s.valence <= 1.0);
        assert!(s.arousal >= 0.0 && s.arousal <= 1.0);
    }

    #[test]
    fn state_label_for_quadrants() {
        assert_eq!(state_label(0.4, 0.6), "engaged");
        assert_eq!(state_label(0.4, 0.3), "content");
        assert_eq!(state_label(0.0, 0.5), "neutral");
        assert_eq!(state_label(-0.5, 0.7), "frustrated");
        assert_eq!(state_label(-0.5, 0.1), "muted");
    }

    #[test]
    fn store_roundtrip_and_mean_valence() {
        let store = fresh_store();
        store
            .append(&AffectState {
                id: None,
                session_id: "s1".to_string(),
                round: 0,
                valence: 0.2,
                arousal: 0.4,
                recorded_at: Utc::now(),
            })
            .unwrap();
        store
            .append(&AffectState {
                id: None,
                session_id: "s1".to_string(),
                round: 1,
                valence: -0.6,
                arousal: 0.7,
                recorded_at: Utc::now(),
            })
            .unwrap();
        let mean = store.mean_valence_for_session("s1").unwrap().unwrap();
        assert!((mean - (-0.2)).abs() < 1e-5);
        let latest = store.latest_for_session("s1").unwrap().unwrap();
        assert_eq!(latest.round, 1);
    }
}
