use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Verified skills. Schema from Voyager (arXiv:2305.16291).
/// Stored as SKILL.md-compatible bodies indexed by embedding on description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralEntry {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    /// Full SKILL.md body or code block.
    pub skill_body: String,
    pub verified: bool,
    pub verification_score: f32,
    pub times_used: u32,
    pub times_succeeded: u32,
    pub embedding_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub source_task_id: Option<Uuid>,
}

impl ProceduralEntry {
    pub fn new(name: String, description: String, skill_body: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            skill_body,
            verified: false,
            // Laplace prior — un-used skills start at 0.5 so the bandit doesn't
            // starve newcomers. See cognition_base.rs for the same pattern.
            verification_score: 0.5,
            times_used: 0,
            times_succeeded: 0,
            embedding_id: None,
            created_at: Utc::now(),
            source_task_id: None,
        }
    }

    /// EvolveR quality score (arXiv:2510.16079): `(success+1)/(use+2)`.
    /// Laplace-smoothed so new entries start at 0.5 (uninformative prior) and
    /// the formula avoids division by zero. Mirrors `CognitionItem::recompute_quality`.
    pub fn quality_score(&self) -> f32 {
        (self.times_succeeded as f32 + 1.0) / (self.times_used as f32 + 2.0)
    }

    /// Recompute `verification_score` from current use stats. The Voyager
    /// pattern (arXiv:2305.16291) treats verification_score as the running
    /// quality estimate; LCAP / retrieval ordering reads this field.
    pub fn recompute_quality(&mut self) {
        self.verification_score = self.quality_score();
    }
}

pub struct ProceduralStore {
    db: Arc<Mutex<Connection>>,
}

impl ProceduralStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn upsert(&self, entry: &ProceduralEntry) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO procedural
             (id, name, description, skill_body, verified, verification_score,
              times_used, times_succeeded, embedding_id, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(name) DO UPDATE SET
               description=excluded.description,
               skill_body=excluded.skill_body,
               verified=excluded.verified,
               verification_score=excluded.verification_score",
            params![
                entry.id.to_string(),
                entry.name,
                entry.description,
                entry.skill_body,
                entry.verified as i32,
                entry.verification_score,
                entry.times_used,
                entry.times_succeeded,
                entry.embedding_id,
                entry.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_by_name(&self, name: &str) -> Result<Option<ProceduralEntry>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, name, description, skill_body, verified, verification_score,
                    times_used, times_succeeded, embedding_id, created_at
             FROM procedural WHERE name = ?1",
        )?;
        let mut rows = stmt.query_map(params![name], parse_row)?;
        Ok(rows.next().transpose()?.map(|r| r))
    }

    pub fn list_verified(&self, limit: usize) -> Result<Vec<ProceduralEntry>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, name, description, skill_body, verified, verification_score,
                    times_used, times_succeeded, embedding_id, created_at
             FROM procedural WHERE verified = 1
             ORDER BY verification_score DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    /// Voyager-style ranked list of skills by running quality. Unlike
    /// `list_verified`, this returns unverified entries too so the agent can
    /// trial promising candidates. `min_uses` lets callers exclude cold-start
    /// noise (e.g. ask for skills with at least 2 prior invocations).
    pub fn list_by_quality(&self, min_uses: u32, limit: usize) -> Result<Vec<ProceduralEntry>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, name, description, skill_body, verified, verification_score,
                    times_used, times_succeeded, embedding_id, created_at
             FROM procedural
             WHERE times_used >= ?1
             ORDER BY verification_score DESC, times_used DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![min_uses as i64, limit as i64], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn record_use(&self, name: &str, success: bool) -> Result<()> {
        self.record_outcome(name, success)
    }

    /// Record a skill invocation outcome and recompute `verification_score`
    /// in SQL using the EvolveR formula (arXiv:2510.16079):
    ///     `quality = (success_count + 1) / (use_count + 2)`
    /// after the increment. Treating verification_score as the running
    /// quality estimate keeps ORDER BY ranking fresh without a separate
    /// recompute pass. The expression mirrors `CognitionStore::record_use`.
    pub fn record_outcome(&self, name: &str, success: bool) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE procedural SET
                times_used = times_used + 1,
                times_succeeded = times_succeeded + ?1,
                verification_score =
                    (CAST(times_succeeded + ?1 + 1 AS REAL))
                  / (CAST(times_used + 1 + 2 AS REAL))
             WHERE name = ?2",
            params![success as i32, name],
        )?;
        Ok(())
    }

    /// Voyager skill-name lookup: does `name` resolve to a known skill?
    /// Cheap existence check used by the toolbridge executor to decide
    /// whether a tool-call is in fact a skill invocation worth recording.
    pub fn is_skill(&self, name: &str) -> Result<bool> {
        let db = self.db.lock().unwrap();
        let n: i64 = db.query_row(
            "SELECT COUNT(*) FROM procedural WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Ratchet-style skill retirement (arXiv:2605.22148).
    ///
    /// Marks skills as `verified = false` when their EvolveR quality score
    /// falls below `threshold` after at least `min_uses` invocations.
    /// Returns the names of skills that were retired.
    ///
    /// Soft-retire (not delete) preserves history and allows a skill to be
    /// re-verified if it improves. Without retirement: +0.0pp over the
    /// no-skill baseline. With it: +0.328pp (Ratchet paper Table 2).
    ///
    /// Suggested defaults: `min_uses = 5, threshold = 0.30`.
    pub fn retire_low_quality(&self, min_uses: u32, threshold: f32) -> Result<Vec<String>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT name FROM procedural
             WHERE verified = 1
               AND times_used >= ?1
               AND (CAST(times_succeeded + 1 AS REAL) / CAST(times_used + 2 AS REAL)) < ?2",
        )?;
        let names: Vec<String> = stmt
            .query_map(params![min_uses as i64, threshold as f64], |row| {
                row.get(0)
            })?
            .filter_map(|r| r.ok())
            .collect();

        for name in &names {
            db.execute(
                "UPDATE procedural SET verified = 0 WHERE name = ?1",
                params![name],
            )?;
        }

        Ok(names)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM procedural WHERE name = ?1", params![name])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> ProceduralStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE procedural (
                id TEXT PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                description TEXT NOT NULL,
                skill_body TEXT NOT NULL,
                verified INTEGER NOT NULL DEFAULT 0,
                verification_score REAL NOT NULL DEFAULT 0.0,
                times_used INTEGER NOT NULL DEFAULT 0,
                times_succeeded INTEGER NOT NULL DEFAULT 0,
                embedding_id INTEGER,
                created_at TEXT NOT NULL
            );",
        )
        .unwrap();
        ProceduralStore::new(Arc::new(Mutex::new(conn)))
    }

    fn seed(store: &ProceduralStore, name: &str) {
        let entry = ProceduralEntry::new(
            name.to_string(),
            format!("desc-{name}"),
            "body".to_string(),
        );
        store.upsert(&entry).unwrap();
    }

    #[test]
    fn quality_score_starts_at_laplace_prior() {
        let entry = ProceduralEntry::new("s".to_string(), "d".to_string(), "b".to_string());
        // (0+1)/(0+2) = 0.5
        assert!((entry.quality_score() - 0.5).abs() < 1e-6);
        assert!((entry.verification_score - 0.5).abs() < 1e-6);
    }

    #[test]
    fn quality_score_monotonic_with_success_rate() {
        let mut entry = ProceduralEntry::new("s".to_string(), "d".to_string(), "b".to_string());
        entry.times_used = 4;
        entry.times_succeeded = 4;
        // (4+1)/(4+2) = 0.833
        assert!(entry.quality_score() > 0.8);
        entry.times_succeeded = 0;
        // (0+1)/(4+2) = 0.166
        assert!(entry.quality_score() < 0.2);
    }

    #[test]
    fn record_outcome_increments_and_recomputes_in_sql() {
        let store = fresh_store();
        seed(&store, "skill_a");
        store.record_outcome("skill_a", true).unwrap();
        store.record_outcome("skill_a", true).unwrap();
        store.record_outcome("skill_a", false).unwrap();
        let entry = store.get_by_name("skill_a").unwrap().unwrap();
        assert_eq!(entry.times_used, 3);
        assert_eq!(entry.times_succeeded, 2);
        // (2+1)/(3+2) = 0.6
        assert!(
            (entry.verification_score - 0.6).abs() < 1e-5,
            "expected 0.6, got {}",
            entry.verification_score
        );
    }

    #[test]
    fn list_by_quality_orders_descending_and_respects_min_uses() {
        let store = fresh_store();
        seed(&store, "hot");
        seed(&store, "warm");
        seed(&store, "cold");
        for _ in 0..5 {
            store.record_outcome("hot", true).unwrap();
        }
        store.record_outcome("warm", true).unwrap();
        store.record_outcome("warm", false).unwrap();
        // "cold" never used — should be excluded by min_uses=1

        let ranked = store.list_by_quality(1, 10).unwrap();
        let names: Vec<_> = ranked.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["hot", "warm"]);
        assert!(!names.contains(&"cold"));
    }

    #[test]
    fn is_skill_detects_existing_entries() {
        let store = fresh_store();
        seed(&store, "px-experiment-runner");
        assert!(store.is_skill("px-experiment-runner").unwrap());
        assert!(!store.is_skill("fs.read").unwrap());
    }

    #[test]
    fn retire_low_quality_soft_retires_failing_skills() {
        let store = fresh_store();
        seed(&store, "good");
        seed(&store, "bad");
        // Mark both as verified
        store.record_outcome("good", true).unwrap();
        store.record_outcome("good", true).unwrap();
        store.record_outcome("good", true).unwrap();
        store.record_outcome("good", true).unwrap();
        store.record_outcome("good", true).unwrap();
        // bad: 5 uses, 0 successes → quality = 1/7 ≈ 0.14
        for _ in 0..5 {
            store.record_outcome("bad", false).unwrap();
        }
        // Manually mark both verified
        {
            let db = store.db.lock().unwrap();
            db.execute("UPDATE procedural SET verified = 1", []).unwrap();
        }

        let retired = store.retire_low_quality(5, 0.30).unwrap();
        assert_eq!(retired, vec!["bad".to_string()]);

        // "bad" is now unverified; "good" stays verified
        let still_good = store.get_by_name("good").unwrap().unwrap();
        assert!(still_good.verified);
        let now_retired = store.get_by_name("bad").unwrap().unwrap();
        assert!(!now_retired.verified);
    }

    #[test]
    fn retire_low_quality_respects_min_uses() {
        let store = fresh_store();
        seed(&store, "newskill");
        // Only 2 uses — below min_uses=5, should NOT be retired even if quality is low
        store.record_outcome("newskill", false).unwrap();
        store.record_outcome("newskill", false).unwrap();
        {
            let db = store.db.lock().unwrap();
            db.execute("UPDATE procedural SET verified = 1", []).unwrap();
        }

        let retired = store.retire_low_quality(5, 0.30).unwrap();
        assert!(retired.is_empty());
    }
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<ProceduralEntry> {
    let id: String = row.get(0)?;
    let created_at: String = row.get(9)?;
    Ok(ProceduralEntry {
        id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
        name: row.get(1)?,
        description: row.get(2)?,
        skill_body: row.get(3)?,
        verified: row.get::<_, i32>(4)? != 0,
        verification_score: row.get(5)?,
        times_used: row.get::<_, i64>(6)? as u32,
        times_succeeded: row.get::<_, i64>(7)? as u32,
        embedding_id: row.get(8)?,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        source_task_id: None,
    })
}
