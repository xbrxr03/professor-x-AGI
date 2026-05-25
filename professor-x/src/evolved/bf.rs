/// BF — Behavioral Fingerprinting.
///
/// F(H_k) = [p_tool, p_plan, p_correct] at every HIRO round.
/// Tracks per-category improvement over 30 rounds to confirm H11:
/// non-uniform improvement across task categories.
///
/// ARCHITECTURE.md Section 14.2

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use chrono::Utc;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralFingerprint {
    pub round:     u32,
    pub p_tool:    f32,
    pub p_plan:    f32,
    pub p_correct: f32,
    /// delta from previous round per component
    pub delta:     Option<[f32; 3]>,
}

impl BehavioralFingerprint {
    pub fn as_array(&self) -> [f32; 3] {
        [self.p_tool, self.p_plan, self.p_correct]
    }

    /// Check H11: is improvement non-uniform?
    /// Returns true if max(|Δ_i|) > 0.10 and range(Δ_i) > 0.07
    pub fn is_nonuniform(&self) -> bool {
        let Some(d) = self.delta else { return false; };
        let abs_max = d.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        let max_d   = d.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let min_d   = d.iter().copied().fold(f32::INFINITY, f32::min);
        abs_max > 0.10 && (max_d - min_d) > 0.07
    }
}

pub struct BfTracker {
    db: Arc<Mutex<Connection>>,
}

impl BfTracker {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    /// Record the fingerprint for a completed HIRO round.
    pub fn record_round(
        &self,
        round:     u32,
        p_tool:    f32,
        p_plan:    f32,
        p_correct: f32,
        component_modified: Option<&str>,
        harness_commit:     Option<&str>,
    ) -> Result<()> {
        let db = self.db.lock().unwrap();
        let pass_at_3 = (p_tool + p_plan + p_correct) / 3.0;
        db.execute(
            "INSERT OR REPLACE INTO hiro_rounds
             (round, p_tool, p_plan, p_correct, pass_at_3,
              component_modified, harness_commit, recorded_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                round,
                p_tool,
                p_plan,
                p_correct,
                pass_at_3,
                component_modified,
                harness_commit,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Get fingerprint for a specific round, with delta from previous round.
    pub fn get_round(&self, round: u32) -> Result<Option<BehavioralFingerprint>> {
        let db = self.db.lock().unwrap();

        let current: Option<(f32, f32, f32)> = {
            let mut stmt = db.prepare(
                "SELECT p_tool, p_plan, p_correct FROM hiro_rounds WHERE round = ?1"
            )?;
            stmt.query_row(params![round], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .ok()
        };

        let Some((p_tool, p_plan, p_correct)) = current else {
            return Ok(None);
        };

        let prev: Option<(f32, f32, f32)> = if round > 0 {
            let mut stmt = db.prepare(
                "SELECT p_tool, p_plan, p_correct FROM hiro_rounds WHERE round = ?1"
            )?;
            stmt.query_row(params![round - 1], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .ok()
        } else {
            None
        };

        let delta = prev.map(|(pt, pp, pc)| [p_tool - pt, p_plan - pp, p_correct - pc]);

        Ok(Some(BehavioralFingerprint { round, p_tool, p_plan, p_correct, delta }))
    }

    /// Compute HIRO(N) = (P_N - P_0) / N
    pub fn hiro_score(&self, n: u32) -> Result<f32> {
        let db = self.db.lock().unwrap();
        let p0: Option<f32> = db.query_row(
            "SELECT pass_at_3 FROM hiro_rounds WHERE round = 0", [], |r| r.get(0)
        ).ok();
        let pn: Option<f32> = db.query_row(
            "SELECT pass_at_3 FROM hiro_rounds WHERE round = ?1", params![n], |r| r.get(0)
        ).ok();

        match (p0, pn) {
            (Some(p0), Some(pn)) if n > 0 => Ok((pn - p0) / n as f32),
            _ => Ok(0.0),
        }
    }

    /// Get all rounds for plotting.
    pub fn all_rounds(&self) -> Result<Vec<BehavioralFingerprint>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT round, p_tool, p_plan, p_correct FROM hiro_rounds ORDER BY round ASC"
        )?;
        let rows: Vec<(u32, f32, f32, f32)> = stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?.filter_map(|r| r.ok()).collect();

        let mut result = Vec::new();
        for (i, (round, p_tool, p_plan, p_correct)) in rows.iter().enumerate() {
            let delta = if i > 0 {
                let (_, pt, pp, pc) = rows[i - 1];
                Some([p_tool - pt, p_plan - pp, p_correct - pc])
            } else {
                None
            };
            result.push(BehavioralFingerprint {
                round: *round, p_tool: *p_tool, p_plan: *p_plan,
                p_correct: *p_correct, delta,
            });
        }
        Ok(result)
    }
}
