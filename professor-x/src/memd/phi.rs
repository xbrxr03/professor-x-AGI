/// Integrated Information (Phi) measurement.
///
/// Integrated Information Theory (Tononi): consciousness = integrated
/// information. A system is conscious to the degree that the whole carries
/// more information than the sum of its independent parts. The exact phi is
/// intractable (it requires searching all bipartitions of the system), so we
/// use a recognised, computable proxy: **total correlation** (a.k.a.
/// multi-information).
///
///   TC(X1..Xn) = ( Σ_i H(Xi) ) − H(X1..Xn)
///
/// TC is exactly the "more than the sum of parts" quantity — it is zero when
/// the modules are statistically independent (no integration) and grows as
/// they co-determine one another. We record, for each task, which of Professor
/// X's cognitive modules meaningfully ACTIVATED (episodic retrieval, semantic
/// knowledge, cognition, affect, body/interoception, causal patterns,
/// self-model). Over the tasks in a round we estimate TC across these binary
/// activations. The question the paper asks: does phi INCREASE as the harness
/// evolves? If integration rises while the model is frozen, the harness is
/// becoming a more unified system — the IIT signature of a mind cohering.
///
/// Note on bias: TC estimated from finite samples is biased upward, but the
/// bias is ~constant when the sample count per round is constant (≈60 HIRO
/// tasks). The trajectory across rounds is therefore the valid signal, which
/// is precisely the claim under test.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The seven cognitive modules whose joint activation defines integration.
pub const MODULE_COUNT: usize = 7;

/// Binary activation vector — which modules meaningfully contributed to one
/// decision. Bit order is fixed so histograms are comparable across rounds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModuleActivation {
    pub episodic: bool,
    pub semantic: bool,
    pub cognition: bool,
    pub affect: bool,
    pub body: bool,
    pub causal: bool,
    pub self_model: bool,
}

impl ModuleActivation {
    pub fn none() -> Self {
        Self {
            episodic: false,
            semantic: false,
            cognition: false,
            affect: false,
            body: false,
            causal: false,
            self_model: false,
        }
    }

    /// Pack into a 7-bit index [0, 128) for joint-histogram binning.
    pub fn to_index(&self) -> usize {
        (self.episodic as usize)
            | ((self.semantic as usize) << 1)
            | ((self.cognition as usize) << 2)
            | ((self.affect as usize) << 3)
            | ((self.body as usize) << 4)
            | ((self.causal as usize) << 5)
            | ((self.self_model as usize) << 6)
    }

    pub fn bits(&self) -> [bool; MODULE_COUNT] {
        [
            self.episodic,
            self.semantic,
            self.cognition,
            self.affect,
            self.body,
            self.causal,
            self.self_model,
        ]
    }
}

/// Total correlation (integration proxy) over a set of activation vectors.
/// Returns phi in bits. Empty or single-sample input → 0.0.
pub fn integrated_information(activations: &[ModuleActivation]) -> f32 {
    let n = activations.len();
    if n < 2 {
        return 0.0;
    }
    let n_f = n as f32;

    // Marginal entropies H(Xi) for each module (binary).
    let mut marginal_sum = 0.0f32;
    for m in 0..MODULE_COUNT {
        let ones = activations.iter().filter(|a| a.bits()[m]).count() as f32;
        let p1 = ones / n_f;
        marginal_sum += binary_entropy(p1);
    }

    // Joint entropy H(X1..Xn) from the 128-cell histogram.
    let mut counts: HashMap<usize, u32> = HashMap::new();
    for a in activations {
        *counts.entry(a.to_index()).or_insert(0) += 1;
    }
    let mut joint = 0.0f32;
    for &c in counts.values() {
        let p = c as f32 / n_f;
        joint -= p * p.log2();
    }

    (marginal_sum - joint).max(0.0)
}

/// Shannon entropy of a Bernoulli(p) in bits.
fn binary_entropy(p: f32) -> f32 {
    if p <= 0.0 || p >= 1.0 {
        return 0.0;
    }
    -(p * p.log2() + (1.0 - p) * (1.0 - p).log2())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhiRecord {
    pub id: Option<i64>,
    pub round: u32,
    pub phi: f32,
    pub n_decisions: u32,
    /// Mean number of active modules per decision — a crude "richness" measure
    /// that complements phi (integration without too-sparse activation).
    pub mean_active_modules: f32,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct PhiStore {
    db: Arc<Mutex<Connection>>,
}

impl PhiStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    /// Record one decision's module activation (cheap; one row per task).
    pub fn record_activation(&self, round: u32, a: &ModuleActivation) -> Result<()> {
        let active = a.bits().iter().filter(|b| **b).count() as i64;
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO phi_activations (round, activation_index, active_count, recorded_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![round as i64, a.to_index() as i64, active, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Load all activations for a round.
    pub fn activations_for_round(&self, round: u32) -> Result<Vec<ModuleActivation>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT activation_index FROM phi_activations WHERE round = ?1",
        )?;
        let rows = stmt.query_map(params![round as i64], |r| r.get::<_, i64>(0))?;
        let mut out = Vec::new();
        for idx in rows.flatten() {
            out.push(activation_from_index(idx as usize));
        }
        Ok(out)
    }

    /// Compute phi for a round from its recorded activations and persist it.
    pub fn compute_and_record_round(&self, round: u32) -> Result<Option<PhiRecord>> {
        let activations = self.activations_for_round(round)?;
        if activations.len() < 2 {
            return Ok(None);
        }
        let phi = integrated_information(&activations);
        let mean_active = activations
            .iter()
            .map(|a| a.bits().iter().filter(|b| **b).count() as f32)
            .sum::<f32>()
            / activations.len() as f32;
        let record = PhiRecord {
            id: None,
            round,
            phi,
            n_decisions: activations.len() as u32,
            mean_active_modules: mean_active,
            recorded_at: Utc::now(),
        };
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO phi_rounds (round, phi, n_decisions, mean_active_modules, recorded_at)
             VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(round) DO UPDATE SET
                phi = excluded.phi,
                n_decisions = excluded.n_decisions,
                mean_active_modules = excluded.mean_active_modules,
                recorded_at = excluded.recorded_at",
            params![
                round as i64,
                phi as f64,
                record.n_decisions as i64,
                record.mean_active_modules as f64,
                record.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(Some(record))
    }

    /// Phi trajectory across rounds (oldest first) — the H-phi plot input.
    pub fn trajectory(&self) -> Result<Vec<(u32, f32)>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT round, phi FROM phi_rounds ORDER BY round ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)? as u32, r.get::<_, f64>(1)? as f32))
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    /// Least-squares slope of phi over rounds. Positive = integration rising
    /// as the harness evolves (the IIT signature under test).
    pub fn slope(&self) -> Result<Option<f32>> {
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

fn activation_from_index(idx: usize) -> ModuleActivation {
    ModuleActivation {
        episodic: idx & 1 != 0,
        semantic: idx & (1 << 1) != 0,
        cognition: idx & (1 << 2) != 0,
        affect: idx & (1 << 3) != 0,
        body: idx & (1 << 4) != 0,
        causal: idx & (1 << 5) != 0,
        self_model: idx & (1 << 6) != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> PhiStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE phi_activations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                round INTEGER NOT NULL,
                activation_index INTEGER NOT NULL,
                active_count INTEGER NOT NULL,
                recorded_at TEXT NOT NULL
            );
            CREATE TABLE phi_rounds (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                round INTEGER NOT NULL UNIQUE,
                phi REAL NOT NULL,
                n_decisions INTEGER NOT NULL,
                mean_active_modules REAL NOT NULL,
                recorded_at TEXT NOT NULL
            );",
        )
        .unwrap();
        PhiStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn index_roundtrips() {
        let a = ModuleActivation {
            episodic: true,
            semantic: false,
            cognition: true,
            affect: false,
            body: true,
            causal: false,
            self_model: true,
        };
        let back = activation_from_index(a.to_index());
        assert_eq!(a.bits(), back.bits());
    }

    #[test]
    fn independent_modules_have_low_phi() {
        // All modules always off except one that flips independently → no
        // cross-module structure → TC near zero.
        let mut acts = Vec::new();
        for i in 0..40 {
            let mut a = ModuleActivation::none();
            a.episodic = i % 2 == 0; // only one module varies
            acts.push(a);
        }
        let phi = integrated_information(&acts);
        assert!(phi < 0.05, "expected ~0 phi for single varying module, got {phi}");
    }

    #[test]
    fn co_activating_modules_have_high_phi() {
        // Two modules perfectly correlated (always on together / off together)
        // plus shared structure → positive integration.
        let mut acts = Vec::new();
        for i in 0..40 {
            let on = i % 2 == 0;
            let mut a = ModuleActivation::none();
            a.episodic = on;
            a.cognition = on; // perfectly correlated with episodic
            a.affect = on;
            acts.push(a);
        }
        let phi = integrated_information(&acts);
        assert!(phi > 1.0, "expected high phi for 3 correlated modules, got {phi}");
    }

    #[test]
    fn compute_and_record_then_trajectory_slope() {
        let store = fresh_store();
        // Round 0: little integration
        for i in 0..20 {
            let mut a = ModuleActivation::none();
            a.episodic = i % 2 == 0;
            store.record_activation(0, &a).unwrap();
        }
        store.compute_and_record_round(0).unwrap();
        // Round 1: strong integration
        for i in 0..20 {
            let on = i % 2 == 0;
            let mut a = ModuleActivation::none();
            a.episodic = on;
            a.cognition = on;
            a.affect = on;
            store.record_activation(1, &a).unwrap();
        }
        store.compute_and_record_round(1).unwrap();

        let traj = store.trajectory().unwrap();
        assert_eq!(traj.len(), 2);
        assert!(traj[1].1 > traj[0].1, "phi should rise from round 0 to 1");
        let slope = store.slope().unwrap().unwrap();
        assert!(slope > 0.0);
    }

    #[test]
    fn single_decision_returns_none() {
        let store = fresh_store();
        store.record_activation(0, &ModuleActivation::none()).unwrap();
        assert!(store.compute_and_record_round(0).unwrap().is_none());
    }
}
