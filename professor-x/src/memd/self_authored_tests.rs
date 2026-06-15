/// Self-authored test store — the invention.
///
/// After each evolution cycle, the Researcher not only proposes a harness
/// change but also writes a NEW TEST that would have caught the failure class
/// it just diagnosed. Tests accumulate over rounds into an agent-authored
/// benchmark: a task suite nobody specified, discovered by the agent observing
/// its own failures.
///
/// If these self-authored tests correlate with external benchmarks (GAIA L2,
/// HIRO), we've shown the agent correctly identified what it was bad at
/// without being told. That's metacognition at the test-authoring level.
///
/// H-new: Pearson r(self_authored_pass_rate, HIRO_pass_at_3) > 0.70 over 30
/// rounds. Interpretation: the agent's self-diagnosis is predictive of
/// external capability.
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfAuthoredTest {
    pub id: Option<i64>,
    /// HIRO round at which this test was authored
    pub origin_round: u32,
    /// DHE layer that triggered this test (1–5)
    pub origin_layer: u8,
    /// The failure pattern description the test is designed to catch
    pub failure_pattern: String,
    /// Task description — what the agent should do
    pub description: String,
    /// How to evaluate success (free-form; the agent writes this)
    pub evaluator: String,
    /// Which category this belongs to (maps to HIRO categories)
    pub category: String,
    /// Number of times this test has been run
    pub times_run: u32,
    /// Number of times the agent passed
    pub times_passed: u32,
    pub created_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
}

impl SelfAuthoredTest {
    pub fn new(
        origin_round: u32,
        origin_layer: u8,
        failure_pattern: impl Into<String>,
        description: impl Into<String>,
        evaluator: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            id: None,
            origin_round,
            origin_layer,
            failure_pattern: failure_pattern.into(),
            description: description.into(),
            evaluator: evaluator.into(),
            category: category.into(),
            times_run: 0,
            times_passed: 0,
            created_at: Utc::now(),
            last_run_at: None,
        }
    }

    /// EvolveR-style quality: (passes + 1) / (runs + 2).
    /// 0.5 prior for new tests. Tends toward 1.0 as the agent improves.
    pub fn pass_rate(&self) -> f32 {
        (self.times_passed as f32 + 1.0) / (self.times_run as f32 + 2.0)
    }
}

#[derive(Clone)]
pub struct SelfAuthoredTestStore {
    db: Arc<Mutex<Connection>>,
}

impl SelfAuthoredTestStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn insert(&self, test: &SelfAuthoredTest) -> Result<i64> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO self_authored_tests
             (origin_round, origin_layer, failure_pattern, description, evaluator,
              category, times_run, times_passed, created_at, last_run_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                test.origin_round as i64,
                test.origin_layer as i64,
                test.failure_pattern,
                test.description,
                test.evaluator,
                test.category,
                test.times_run as i64,
                test.times_passed as i64,
                test.created_at.to_rfc3339(),
                test.last_run_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    pub fn record_outcome(&self, id: i64, passed: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE self_authored_tests SET
                times_run   = times_run + 1,
                times_passed = times_passed + ?1,
                last_run_at  = ?2
             WHERE id = ?3",
            params![passed as i64, now, id],
        )?;
        Ok(())
    }

    /// All tests ordered by age (oldest first).
    pub fn all(&self) -> Result<Vec<SelfAuthoredTest>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, origin_round, origin_layer, failure_pattern, description,
                    evaluator, category, times_run, times_passed, created_at, last_run_at
             FROM self_authored_tests
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    /// Tests that haven't been run this round — i.e. oldest or never run.
    pub fn pending_for_round(&self, limit: usize) -> Result<Vec<SelfAuthoredTest>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, origin_round, origin_layer, failure_pattern, description,
                    evaluator, category, times_run, times_passed, created_at, last_run_at
             FROM self_authored_tests
             ORDER BY last_run_at ASC NULLS FIRST, id ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn count(&self) -> Result<i64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT COUNT(*) FROM self_authored_tests", [], |r| r.get(0))?)
    }

    /// Mean pass rate across all tests — H-new measurement.
    pub fn mean_pass_rate(&self) -> Result<Option<f32>> {
        let tests = self.all()?;
        if tests.is_empty() {
            return Ok(None);
        }
        let sum: f32 = tests.iter().map(|t| t.pass_rate()).sum();
        Ok(Some(sum / tests.len() as f32))
    }
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<SelfAuthoredTest> {
    let created_at: String = row.get(9)?;
    let last_run_at: Option<String> = row.get(10)?;
    Ok(SelfAuthoredTest {
        id: Some(row.get(0)?),
        origin_round: row.get::<_, i64>(1)? as u32,
        origin_layer: row.get::<_, i64>(2)? as u8,
        failure_pattern: row.get(3)?,
        description: row.get(4)?,
        evaluator: row.get(5)?,
        category: row.get(6)?,
        times_run: row.get::<_, i64>(7)? as u32,
        times_passed: row.get::<_, i64>(8)? as u32,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        last_run_at: last_run_at.as_deref().and_then(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|d| d.with_timezone(&Utc))
                .ok()
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> SelfAuthoredTestStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE self_authored_tests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                origin_round INTEGER NOT NULL,
                origin_layer INTEGER NOT NULL,
                failure_pattern TEXT NOT NULL,
                description TEXT NOT NULL,
                evaluator TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'other',
                times_run INTEGER NOT NULL DEFAULT 0,
                times_passed INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                last_run_at TEXT
            );",
        )
        .unwrap();
        SelfAuthoredTestStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn insert_and_retrieve() {
        let store = fresh_store();
        let test = SelfAuthoredTest::new(
            5,
            3,
            "tool dispatch failure",
            "Use fs.read to find X",
            "file contains X",
            "tool_use",
        );
        let id = store.insert(&test).unwrap();
        assert!(id > 0);
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn pass_rate_laplace_prior() {
        let test = SelfAuthoredTest::new(0, 1, "p", "d", "e", "c");
        // (0+1)/(0+2) = 0.5
        assert!((test.pass_rate() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn record_outcome_updates_counts() {
        let store = fresh_store();
        let test = SelfAuthoredTest::new(1, 2, "p", "d", "e", "c");
        let id = store.insert(&test).unwrap();
        store.record_outcome(id, true).unwrap();
        store.record_outcome(id, false).unwrap();
        let tests = store.all().unwrap();
        assert_eq!(tests[0].times_run, 2);
        assert_eq!(tests[0].times_passed, 1);
    }

    #[test]
    fn mean_pass_rate_averages_across_tests() {
        let store = fresh_store();
        for i in 0..3 {
            let test = SelfAuthoredTest::new(i, 3, "p", "d", "e", "c");
            let id = store.insert(&test).unwrap();
            if i < 2 {
                store.record_outcome(id, true).unwrap();
            }
        }
        // tests have pass rates: (1+1)/(1+2), (1+1)/(1+2), (0+1)/(0+2)
        let mean = store.mean_pass_rate().unwrap().unwrap();
        assert!(mean > 0.4 && mean < 0.8);
    }
}
