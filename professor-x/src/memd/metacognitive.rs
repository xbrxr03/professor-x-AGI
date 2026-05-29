//! Metacognitive self-model store (ARCHITECTURE.md §14).
//!
//! Records the agent's per-evolution predictions about which DHE layer and
//! which lever should fix a failure, then verifies after the next HIRO round
//! whether the prediction held. The fraction of correct attributions over a
//! rolling window is **MCA — Metacognitive Calibration Accuracy** — the core
//! empirical claim of MHE (H13: Pearson r(MCA, IR) > 0.70).
//!
//! Schema is owned by `memd::mod` (`metacognitive` table). This module owns
//! the store wrapper, the typed entry struct, and the verification logic.
//! See `evolved::loop_runner` for the append site and `evolved::hiro` for
//! the verification site.
//!
//! Lineage:
//! - "Truly Self-Improving Agents Require Intrinsic Metacognitive Learning"
//!   (arXiv:2506.05109) — names metacognitive evaluation as a required loop.
//! - Meta-Harness (arXiv:2603.28052) — better diagnostic access → better
//!   proposals. MCA operationalises "self-knowledge quality" as a measurable
//!   driver of improvement rate.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// One row in the `metacognitive` table. Created at proposal/attribution
/// time with `attribution_correct = false` and `actual_improvement = 0.0`,
/// then updated by `verify_round` after the next HIRO round records its
/// fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetacognitiveEntry {
    pub id: Option<i64>,
    /// The HIRO round at which the prediction was made. Verification looks
    /// at the round that ran *after* this one.
    pub round: u32,
    /// Free-form label — usually the harness component being modified
    /// (`SkillDefinition("X")`, `ToolDescription("Y")`, etc.). Used to bin
    /// attributions for per-component MCA breakdowns.
    pub task_type: String,
    /// DHE layer the prediction targets (1–5).
    pub predicted_layer: u8,
    /// Improvement lever the prediction targets (1=parametric,
    /// 2=contextual, 3=structural).
    pub predicted_lever: u8,
    /// Set by `verify_round`. The pass@3 delta from the round the entry
    /// was attached to versus the next round.
    pub actual_improvement: f32,
    /// Set by `verify_round`. True iff `actual_improvement` exceeded
    /// the threshold passed to verify.
    pub attribution_correct: bool,
    /// The agent's stated confidence at attribution time (0–1).
    pub confidence: f32,
    pub recorded_at: DateTime<Utc>,
}

impl MetacognitiveEntry {
    pub fn new(
        round: u32,
        task_type: impl Into<String>,
        predicted_layer: u8,
        predicted_lever: u8,
        confidence: f32,
    ) -> Self {
        Self {
            id: None,
            round,
            task_type: task_type.into(),
            predicted_layer,
            predicted_lever,
            actual_improvement: 0.0,
            attribution_correct: false,
            confidence,
            recorded_at: Utc::now(),
        }
    }
}

#[derive(Clone)]
pub struct MetacognitiveStore {
    db: Arc<Mutex<Connection>>,
}

impl MetacognitiveStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    /// Persist a new attribution. The entry's `id` is set on the returned
    /// copy so callers can reference it later for verification.
    pub fn append(&self, entry: &MetacognitiveEntry) -> Result<i64> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO metacognitive
             (round, task_type, predicted_layer, predicted_lever,
              actual_improvement, attribution_correct, confidence, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                entry.round as i64,
                entry.task_type,
                entry.predicted_layer as i64,
                entry.predicted_lever as i64,
                entry.actual_improvement as f64,
                entry.attribution_correct as i64,
                entry.confidence as f64,
                entry.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// Entries that have not yet been verified. Used by the HIRO runner
    /// after a round completes — it compares the new pass@3 to the prior
    /// round's and flips `attribution_correct` for any pending entry whose
    /// round equals `prior_round`.
    pub fn pending_for_round(&self, round: u32) -> Result<Vec<MetacognitiveEntry>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, round, task_type, predicted_layer, predicted_lever,
                    actual_improvement, attribution_correct, confidence, recorded_at
             FROM metacognitive
             WHERE round = ?1 AND attribution_correct = 0 AND actual_improvement = 0.0",
        )?;
        let rows = stmt.query_map(params![round as i64], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    /// Mark an attribution as verified. Idempotent — calling it twice on
    /// the same `id` is harmless; the latter call wins.
    pub fn verify_attribution(
        &self,
        id: i64,
        actual_improvement: f32,
        attribution_correct: bool,
    ) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE metacognitive
             SET actual_improvement = ?1, attribution_correct = ?2
             WHERE id = ?3",
            params![actual_improvement as f64, attribution_correct as i64, id],
        )?;
        Ok(())
    }

    /// Coarse verification driver: a single `pass@3` delta verdict for every
    /// pending entry from `prior_round`. If `current_pass_at_3 -
    /// prior_pass_at_3 >= delta_threshold`, the attribution is credited.
    ///
    /// Kept for callers that don't have a fingerprint handy. Prefer
    /// `verify_round_lever_specific` when the per-category fingerprint is
    /// available — it credits attributions only when the predicted layer's
    /// targeted category actually improves, which is what H13 needs.
    pub fn verify_round(
        &self,
        prior_round: u32,
        prior_pass_at_3: f32,
        current_pass_at_3: f32,
        delta_threshold: f32,
    ) -> Result<usize> {
        let pending = self.pending_for_round(prior_round)?;
        let delta = current_pass_at_3 - prior_pass_at_3;
        let credit = delta >= delta_threshold;
        let n = pending.len();
        for entry in &pending {
            if let Some(id) = entry.id {
                self.verify_attribution(id, delta, credit)?;
            }
        }
        Ok(n)
    }

    /// Lever-specific verification driver. Each pending entry's
    /// `predicted_layer` selects which of the per-category deltas the
    /// verifier inspects — a Layer-3 (tool dispatch) attribution only earns
    /// credit when `p_tool` improves; a Layer-5 (reasoning) attribution
    /// needs `p_plan` + `p_correct` to move; etc. Cross-cutting layers
    /// (1 retrieval, 2 context) fall back to the overall pass@3 delta.
    /// Unknown layers (e.g. `predicted_layer = 0` from the parser default)
    /// also use overall.
    ///
    /// This refines the coarse `verify_round` so a tool-only improvement
    /// doesn't accidentally credit a reasoning attribution and vice versa.
    /// MCA computed from these credits is what `H13 Pearson r(MCA, IR)`
    /// expects.
    pub fn verify_round_lever_specific(
        &self,
        prior_round: u32,
        prior_fp: [f32; 3],
        curr_fp: [f32; 3],
        delta_threshold: f32,
    ) -> Result<usize> {
        let pending = self.pending_for_round(prior_round)?;
        let n = pending.len();
        for entry in &pending {
            let delta = relevant_delta_for_layer(entry.predicted_layer, prior_fp, curr_fp);
            let credit = delta >= delta_threshold;
            if let Some(id) = entry.id {
                self.verify_attribution(id, delta, credit)?;
            }
        }
        Ok(n)
    }

    /// Most recent N entries, newest first. For observer panels.
    pub fn recent(&self, limit: usize) -> Result<Vec<MetacognitiveEntry>> {
        let limit = limit.clamp(1, 500) as i64;
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, round, task_type, predicted_layer, predicted_lever,
                    actual_improvement, attribution_correct, confidence, recorded_at
             FROM metacognitive
             ORDER BY id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    /// Metacognitive Calibration Accuracy over the rounds in
    /// `[start_round, end_round]` inclusive. Returns `(mca, sample_size)`.
    /// When `sample_size == 0`, the returned mca is 0.0 — callers should
    /// check the size before reporting.
    pub fn mca_for_window(&self, start_round: u32, end_round: u32) -> Result<(f32, usize)> {
        let db = self.db.lock().unwrap();
        let row: Option<(i64, i64)> = db
            .query_row(
                "SELECT
                    SUM(CASE WHEN attribution_correct = 1 THEN 1 ELSE 0 END),
                    COUNT(*)
                 FROM metacognitive
                 WHERE round BETWEEN ?1 AND ?2",
                params![start_round as i64, end_round as i64],
                |row| Ok((row.get::<_, Option<i64>>(0)?.unwrap_or(0), row.get(1)?)),
            )
            .ok();
        let Some((correct, total)) = row else {
            return Ok((0.0, 0));
        };
        if total == 0 {
            return Ok((0.0, 0));
        }
        Ok((correct as f32 / total as f32, total as usize))
    }

    /// Rolling MCA over the most recent `window_rounds` rounds anchored at
    /// `current_round`. H13's primary accessor.
    pub fn mca_rolling(&self, current_round: u32, window_rounds: u32) -> Result<(f32, usize)> {
        let start = current_round.saturating_sub(window_rounds.saturating_sub(1));
        self.mca_for_window(start, current_round)
    }
}

/// Index into `[p_tool, p_plan, p_correct]` fingerprints.
const P_TOOL: usize = 0;
const P_PLAN: usize = 1;
const P_CORRECT: usize = 2;

/// Pick the relevant per-category delta for a DHE layer.
///
/// Mapping (from ARCHITECTURE.md §14 + paper outline §6.1):
/// - Layer 1 retrieval, Layer 2 context: cross-cutting — overall mean delta.
/// - Layer 3 tool dispatch: `p_tool` only.
/// - Layer 4 tool execution: `p_tool` only (tool category dominates).
/// - Layer 5 reasoning: average of `p_plan` and `p_correct`.
/// - Layer 0 (unknown / parser default): overall mean delta.
pub(crate) fn relevant_delta_for_layer(
    layer: u8,
    prior_fp: [f32; 3],
    curr_fp: [f32; 3],
) -> f32 {
    let d_tool = curr_fp[P_TOOL] - prior_fp[P_TOOL];
    let d_plan = curr_fp[P_PLAN] - prior_fp[P_PLAN];
    let d_correct = curr_fp[P_CORRECT] - prior_fp[P_CORRECT];
    match layer {
        3 | 4 => d_tool,
        5 => (d_plan + d_correct) / 2.0,
        // Layers 1, 2, and 0 (unknown) — cross-cutting overall.
        _ => (d_tool + d_plan + d_correct) / 3.0,
    }
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<MetacognitiveEntry> {
    let recorded_at: String = row.get(8)?;
    Ok(MetacognitiveEntry {
        id: Some(row.get(0)?),
        round: row.get::<_, i64>(1)? as u32,
        task_type: row.get(2)?,
        predicted_layer: row.get::<_, i64>(3)? as u8,
        predicted_lever: row.get::<_, i64>(4)? as u8,
        actual_improvement: row.get::<_, f64>(5)? as f32,
        attribution_correct: row.get::<_, i64>(6)? != 0,
        confidence: row.get::<_, f64>(7)? as f32,
        recorded_at: DateTime::parse_from_rfc3339(&recorded_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> MetacognitiveStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE metacognitive (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                round INTEGER NOT NULL,
                task_type TEXT NOT NULL,
                predicted_layer INTEGER NOT NULL,
                predicted_lever INTEGER NOT NULL,
                actual_improvement REAL NOT NULL DEFAULT 0.0,
                attribution_correct INTEGER NOT NULL DEFAULT 0,
                confidence REAL NOT NULL DEFAULT 0.0,
                recorded_at TEXT NOT NULL
            );",
        )
        .unwrap();
        MetacognitiveStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn append_assigns_rowid_and_persists() {
        let store = fresh_store();
        let id = store
            .append(&MetacognitiveEntry::new(3, "SkillDefinition(\"x\")", 3, 3, 0.6))
            .unwrap();
        assert!(id > 0);
        let recent = store.recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].round, 3);
        assert_eq!(recent[0].predicted_lever, 3);
        assert!(!recent[0].attribution_correct);
    }

    #[test]
    fn verify_round_credits_when_delta_positive() {
        let store = fresh_store();
        store
            .append(&MetacognitiveEntry::new(5, "x", 3, 3, 0.7))
            .unwrap();
        store
            .append(&MetacognitiveEntry::new(5, "y", 2, 2, 0.5))
            .unwrap();
        // Round 5 → round 6 improved by 0.05; threshold 0.02.
        let n = store.verify_round(5, 0.30, 0.35, 0.02).unwrap();
        assert_eq!(n, 2);
        let recent = store.recent(10).unwrap();
        assert!(recent.iter().all(|e| e.attribution_correct));
    }

    #[test]
    fn verify_round_rejects_when_delta_below_threshold() {
        let store = fresh_store();
        store
            .append(&MetacognitiveEntry::new(7, "x", 3, 3, 0.7))
            .unwrap();
        let n = store.verify_round(7, 0.50, 0.501, 0.02).unwrap();
        assert_eq!(n, 1);
        let recent = store.recent(10).unwrap();
        assert!(!recent[0].attribution_correct);
        // actual_improvement still records the delta even when not credited.
        assert!((recent[0].actual_improvement - 0.001).abs() < 1e-3);
    }

    #[test]
    fn verify_round_skips_already_verified_entries() {
        let store = fresh_store();
        let id = store
            .append(&MetacognitiveEntry::new(9, "x", 3, 3, 0.7))
            .unwrap();
        store.verify_attribution(id, 0.1, true).unwrap();
        // Now verify_round on the same round should not find this pending.
        let n = store.verify_round(9, 0.4, 0.5, 0.02).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn mca_for_window_counts_correctly() {
        let store = fresh_store();
        for round in 0..5 {
            store
                .append(&MetacognitiveEntry::new(round, "x", 3, 3, 0.5))
                .unwrap();
        }
        // Rounds 0,1,2 verified-correct; round 3 verified-incorrect; round 4 still pending.
        for round in 0..3 {
            store.verify_round(round, 0.0, 0.1, 0.02).unwrap();
        }
        store.verify_round(3, 0.0, 0.0, 0.02).unwrap();
        let (mca, n) = store.mca_for_window(0, 4).unwrap();
        assert_eq!(n, 5);
        // 3 of 5 correct.
        assert!((mca - 0.6).abs() < 1e-5, "expected 0.6, got {mca}");
    }

    #[test]
    fn mca_rolling_window_anchors_at_current_round() {
        let store = fresh_store();
        for round in 0..10 {
            store
                .append(&MetacognitiveEntry::new(round, "x", 3, 3, 0.5))
                .unwrap();
        }
        for round in 0..5 {
            store.verify_round(round, 0.0, 0.1, 0.02).unwrap();
        }
        for round in 5..10 {
            store.verify_round(round, 0.0, 0.0, 0.02).unwrap();
        }
        // Last 5 rounds: all incorrect.
        let (mca, n) = store.mca_rolling(9, 5).unwrap();
        assert_eq!(n, 5);
        assert!(mca < 0.01);
    }

    // ── Lever-specific verification ───────────────────────────────────────

    #[test]
    fn layer3_attribution_credited_only_on_tool_delta() {
        // Layer 3 = tool dispatch — should care only about p_tool.
        let prior = [0.30, 0.40, 0.50];
        let curr_tool_only = [0.50, 0.40, 0.50];
        let d = relevant_delta_for_layer(3, prior, curr_tool_only);
        assert!((d - 0.20).abs() < 1e-5);

        // Same overall pass@3 (mean), but the improvement is on planning,
        // not tool. Layer-3 verdict should see ~zero delta.
        let curr_plan_only = [0.30, 0.60, 0.50];
        let d_plan = relevant_delta_for_layer(3, prior, curr_plan_only);
        assert!(d_plan.abs() < 1e-5);
    }

    #[test]
    fn layer5_attribution_uses_plan_and_correct_average() {
        let prior = [0.30, 0.40, 0.50];
        let curr = [0.30, 0.50, 0.60];
        // Layer 5 — reasoning. p_plan +0.10, p_correct +0.10 → avg 0.10.
        let d = relevant_delta_for_layer(5, prior, curr);
        assert!((d - 0.10).abs() < 1e-5);
        // p_tool moving alone shouldn't change Layer-5's verdict.
        let curr_tool = [0.99, 0.40, 0.50];
        let d_tool = relevant_delta_for_layer(5, prior, curr_tool);
        assert!(d_tool.abs() < 1e-5);
    }

    #[test]
    fn cross_cutting_layers_use_overall_mean() {
        let prior = [0.30, 0.40, 0.50];
        let curr = [0.40, 0.50, 0.60];
        // Mean delta = (0.10 + 0.10 + 0.10)/3 = 0.10.
        for layer in [0u8, 1, 2] {
            let d = relevant_delta_for_layer(layer, prior, curr);
            assert!(
                (d - 0.10).abs() < 1e-5,
                "layer {layer} expected 0.10 mean delta, got {d}"
            );
        }
    }

    #[test]
    fn verify_round_lever_specific_credits_per_layer() {
        let store = fresh_store();
        // Two attributions on the same round: Layer 3 (tool-only) and
        // Layer 5 (reasoning).
        let _ = store
            .append(&MetacognitiveEntry::new(1, "ToolDescription(\"x\")", 3, 3, 0.7))
            .unwrap();
        let _ = store
            .append(&MetacognitiveEntry::new(1, "SystemPrompt", 5, 3, 0.7))
            .unwrap();
        // Only p_tool improved. Layer-3 should be credited; Layer-5 not.
        let prior = [0.30, 0.40, 0.50];
        let curr = [0.50, 0.40, 0.50];
        let n = store
            .verify_round_lever_specific(1, prior, curr, 0.05)
            .unwrap();
        assert_eq!(n, 2);
        let entries = store.recent(10).unwrap();
        let layer3 = entries.iter().find(|e| e.predicted_layer == 3).unwrap();
        let layer5 = entries.iter().find(|e| e.predicted_layer == 5).unwrap();
        assert!(layer3.attribution_correct);
        assert!(!layer5.attribution_correct);
    }

    #[test]
    fn verify_round_lever_specific_records_per_entry_delta() {
        let store = fresh_store();
        store
            .append(&MetacognitiveEntry::new(2, "ToolDescription(\"x\")", 3, 3, 0.7))
            .unwrap();
        store
            .append(&MetacognitiveEntry::new(2, "SystemPrompt", 5, 3, 0.7))
            .unwrap();
        let prior = [0.30, 0.40, 0.50];
        let curr = [0.50, 0.50, 0.60]; // tool +0.20, plan +0.10, correct +0.10
        store
            .verify_round_lever_specific(2, prior, curr, 0.05)
            .unwrap();
        let entries = store.recent(10).unwrap();
        let layer3 = entries.iter().find(|e| e.predicted_layer == 3).unwrap();
        let layer5 = entries.iter().find(|e| e.predicted_layer == 5).unwrap();
        assert!((layer3.actual_improvement - 0.20).abs() < 1e-5);
        // Layer 5: avg(0.10, 0.10) = 0.10
        assert!((layer5.actual_improvement - 0.10).abs() < 1e-5);
    }
}
