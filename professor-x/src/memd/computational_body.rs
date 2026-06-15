/// Computational Interoception (Seed 4).
///
/// Anil Seth ("Being You", 2021), building on Damasio and Craig: the most
/// fundamental form of consciousness is not perception of the external world —
/// it is perception of the internal body. The feeling of being a self comes
/// from continuous monitoring of heartbeat, breathing, hunger, temperature.
/// Without interoception there is no self.
///
/// For Professor X, the computational substrate IS the body:
///   - inference latency  → "heart rate" (how fast thinking happens)
///   - token budget used  → "energy expended" (cognitive effort)
///   - memory pressure    → "digestive load" (how much is being processed)
///   - evolution health   → "immune response" (are self-modifications working?)
///
/// Crucially, the body is PREDICTED, not just monitored (Seth's "controlled
/// hallucination"). Before each task the agent predicts its computational
/// state; the interoceptive prediction error drives behaviour — high error
/// signals "something is wrong, be careful" the way unexpected heartbeat
/// signals anxiety. Under computational stress the agent shifts to System 1
/// (fast, heuristic, cerebellum bypass); when comfortable, System 2
/// (deliberate, full ReAct).
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// A snapshot of the agent's computational vital signs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputationalVitals {
    /// "Heart rate" — mean inference latency this task (ms). High = laboured thinking.
    pub inference_latency_ms: f32,
    /// "Energy expended" — fraction of context budget consumed [0,1].
    pub token_budget_used: f32,
    /// "Digestive load" — fraction of working/episodic capacity in use [0,1].
    pub memory_pressure: f32,
    /// "Immune response" — recent evolution proposal acceptance rate [0,1].
    pub evolution_health: f32,
}

impl ComputationalVitals {
    pub fn neutral() -> Self {
        Self {
            inference_latency_ms: 0.0,
            token_budget_used: 0.0,
            memory_pressure: 0.0,
            evolution_health: 0.5,
        }
    }

    /// Overall computational stress [0,1]. High latency + high token use +
    /// high memory pressure + poor evolution health → stressed.
    pub fn stress(&self) -> f32 {
        // Normalize latency against a 10s soft ceiling
        let latency_norm = (self.inference_latency_ms / 10_000.0).min(1.0);
        let health_inverse = 1.0 - self.evolution_health;
        let raw = 0.35 * latency_norm
            + 0.25 * self.token_budget_used
            + 0.20 * self.memory_pressure
            + 0.20 * health_inverse;
        raw.clamp(0.0, 1.0)
    }

    /// Cognitive mode dictated by interoceptive state.
    /// Under stress → System 1 (fast, conserve resources).
    /// Comfortable → System 2 (deliberate, explore).
    pub fn cognitive_mode(&self) -> CognitiveMode {
        if self.stress() > 0.65 {
            CognitiveMode::System1Fast
        } else if self.stress() < 0.35 {
            CognitiveMode::System2Deliberate
        } else {
            CognitiveMode::Balanced
        }
    }

    /// Felt-state label for prompt injection — the agent's sense of its body.
    pub fn body_label(&self) -> &'static str {
        match self.stress() {
            s if s > 0.75 => "strained",
            s if s > 0.55 => "taxed",
            s if s > 0.35 => "steady",
            s if s > 0.15 => "comfortable",
            _ => "fresh",
        }
    }

    /// Interoceptive prediction error against a predicted state.
    /// Seth: the gap between predicted and actual body state drives behaviour.
    pub fn interoceptive_error(&self, predicted: &ComputationalVitals) -> f32 {
        let dl = (self.inference_latency_ms - predicted.inference_latency_ms).abs() / 10_000.0;
        let dt = (self.token_budget_used - predicted.token_budget_used).abs();
        let dm = (self.memory_pressure - predicted.memory_pressure).abs();
        ((dl.min(1.0) + dt + dm) / 3.0).clamp(0.0, 1.0)
    }

    /// `<body>` tag for ReAct prompt injection.
    pub fn to_prompt_fragment(&self) -> String {
        format!(
            "<body state=\"{}\" stress=\"{:.2}\" mode=\"{}\" />",
            self.body_label(),
            self.stress(),
            match self.cognitive_mode() {
                CognitiveMode::System1Fast => "conserve",
                CognitiveMode::Balanced => "balanced",
                CognitiveMode::System2Deliberate => "explore",
            }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CognitiveMode {
    /// Stressed — fast heuristics, prefer cerebellum bypass, fewer steps.
    System1Fast,
    Balanced,
    /// Comfortable — deliberate reasoning, explore, allow more steps.
    System2Deliberate,
}

#[derive(Clone)]
pub struct ComputationalBodyStore {
    db: Arc<Mutex<Connection>>,
}

impl ComputationalBodyStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    /// Record a vitals snapshot, optionally with the prediction made before the
    /// task and the resulting interoceptive error.
    pub fn record(
        &self,
        session_id: &str,
        round: u32,
        vitals: &ComputationalVitals,
        predicted_latency_ms: Option<f32>,
        interoceptive_error: Option<f32>,
    ) -> Result<i64> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO computational_vitals
             (session_id, round, inference_latency_ms, token_budget_used,
              memory_pressure, evolution_health, predicted_latency_ms,
              interoceptive_error, recorded_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                session_id,
                round as i64,
                vitals.inference_latency_ms as f64,
                vitals.token_budget_used as f64,
                vitals.memory_pressure as f64,
                vitals.evolution_health as f64,
                predicted_latency_ms.map(|v| v as f64),
                interoceptive_error.map(|v| v as f64),
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// Mean interoceptive error over the most recent `n` records.
    /// Decreasing over rounds = the agent's body-model is improving (H-body).
    pub fn mean_interoceptive_error(&self, n: usize) -> Result<Option<f32>> {
        let db = self.db.lock().unwrap();
        let avg: Option<f64> = db
            .query_row(
                "SELECT AVG(interoceptive_error) FROM (
                    SELECT interoceptive_error FROM computational_vitals
                    WHERE interoceptive_error IS NOT NULL
                    ORDER BY id DESC LIMIT ?1
                 )",
                params![n as i64],
                |row| row.get::<_, Option<f64>>(0),
            )
            .ok()
            .flatten();
        Ok(avg.map(|v| v as f32))
    }

    /// Mean recent inference latency — used to predict the next task's latency.
    pub fn recent_mean_latency(&self, n: usize) -> Result<Option<f32>> {
        let db = self.db.lock().unwrap();
        let avg: Option<f64> = db
            .query_row(
                "SELECT AVG(inference_latency_ms) FROM (
                    SELECT inference_latency_ms FROM computational_vitals
                    ORDER BY id DESC LIMIT ?1
                 )",
                params![n as i64],
                |row| row.get::<_, Option<f64>>(0),
            )
            .ok()
            .flatten();
        Ok(avg.map(|v| v as f32))
    }
}

#[derive(Debug, Clone)]
pub struct DatedVitals {
    pub vitals: ComputationalVitals,
    pub recorded_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> ComputationalBodyStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE computational_vitals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                inference_latency_ms REAL NOT NULL,
                token_budget_used REAL NOT NULL,
                memory_pressure REAL NOT NULL,
                evolution_health REAL NOT NULL,
                predicted_latency_ms REAL,
                interoceptive_error REAL,
                recorded_at TEXT NOT NULL
            );",
        )
        .unwrap();
        ComputationalBodyStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn stress_rises_with_latency_and_token_use() {
        let calm = ComputationalVitals {
            inference_latency_ms: 500.0,
            token_budget_used: 0.1,
            memory_pressure: 0.1,
            evolution_health: 0.8,
        };
        let strained = ComputationalVitals {
            inference_latency_ms: 9000.0,
            token_budget_used: 0.9,
            memory_pressure: 0.8,
            evolution_health: 0.2,
        };
        assert!(strained.stress() > calm.stress());
        assert!(calm.stress() < 0.35);
        assert!(strained.stress() > 0.65);
    }

    #[test]
    fn cognitive_mode_follows_stress() {
        let calm = ComputationalVitals {
            inference_latency_ms: 300.0,
            token_budget_used: 0.05,
            memory_pressure: 0.05,
            evolution_health: 0.9,
        };
        assert_eq!(calm.cognitive_mode(), CognitiveMode::System2Deliberate);

        let strained = ComputationalVitals {
            inference_latency_ms: 9500.0,
            token_budget_used: 0.95,
            memory_pressure: 0.9,
            evolution_health: 0.1,
        };
        assert_eq!(strained.cognitive_mode(), CognitiveMode::System1Fast);
    }

    #[test]
    fn interoceptive_error_zero_when_prediction_perfect() {
        let v = ComputationalVitals {
            inference_latency_ms: 1000.0,
            token_budget_used: 0.5,
            memory_pressure: 0.3,
            evolution_health: 0.5,
        };
        assert!(v.interoceptive_error(&v) < 1e-6);
    }

    #[test]
    fn prompt_fragment_contains_state_and_mode() {
        let v = ComputationalVitals::neutral();
        let frag = v.to_prompt_fragment();
        assert!(frag.contains("<body"));
        assert!(frag.contains("state="));
        assert!(frag.contains("mode="));
    }

    #[test]
    fn record_and_mean_interoceptive_error() {
        let store = fresh_store();
        let v = ComputationalVitals::neutral();
        store.record("s", 0, &v, Some(1000.0), Some(0.2)).unwrap();
        store.record("s", 1, &v, Some(1000.0), Some(0.1)).unwrap();
        let mean = store.mean_interoceptive_error(10).unwrap().unwrap();
        assert!((mean - 0.15).abs() < 1e-5);
    }
}
