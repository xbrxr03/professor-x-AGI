use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct CodingSmokeRecord {
    pub id: Option<i64>,
    pub generated_at: DateTime<Utc>,
    pub workspace: String,
    pub passed: bool,
    pub initial_test_failed: bool,
    pub edit_applied: bool,
    pub final_test_passed: bool,
    pub report_path: String,
    pub transcript_path: Option<String>,
    pub artifacts: Vec<String>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct CodingSmokeStore {
    db: Arc<Mutex<Connection>>,
}

impl CodingSmokeStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn insert(&self, record: &CodingSmokeRecord) -> Result<()> {
        let artifacts = serde_json::to_string(&record.artifacts)?;
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO coding_smokes
             (generated_at, workspace, passed, initial_test_failed, edit_applied,
              final_test_passed, report_path, transcript_path, artifacts, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.generated_at.to_rfc3339(),
                record.workspace,
                record.passed as i64,
                record.initial_test_failed as i64,
                record.edit_applied as i64,
                record.final_test_passed as i64,
                record.report_path,
                record.transcript_path,
                artifacts,
                record.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn count(&self) -> Result<i64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT COUNT(*) FROM coding_smokes", [], |row| row.get(0))?)
    }

    pub fn pass_count(&self) -> Result<i64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row(
            "SELECT COUNT(*) FROM coding_smokes WHERE passed = 1",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn latest(&self) -> Result<Option<CodingSmokeRecord>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, generated_at, workspace, passed, initial_test_failed, edit_applied,
                    final_test_passed, report_path, transcript_path, artifacts, recorded_at
             FROM coding_smokes
             ORDER BY generated_at DESC, id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(parse_record(row)?))
    }
}

fn parse_record(row: &rusqlite::Row) -> rusqlite::Result<CodingSmokeRecord> {
    let generated_at_raw: String = row.get(1)?;
    let artifacts_raw: String = row.get(9)?;
    let recorded_at_raw: String = row.get(10)?;
    Ok(CodingSmokeRecord {
        id: row.get(0)?,
        generated_at: parse_time(&generated_at_raw),
        workspace: row.get(2)?,
        passed: row.get::<_, i64>(3)? != 0,
        initial_test_failed: row.get::<_, i64>(4)? != 0,
        edit_applied: row.get::<_, i64>(5)? != 0,
        final_test_passed: row.get::<_, i64>(6)? != 0,
        report_path: row.get(7)?,
        transcript_path: row.get(8)?,
        artifacts: serde_json::from_str(&artifacts_raw).unwrap_or_default(),
        recorded_at: parse_time(&recorded_at_raw),
    })
}

fn parse_time(raw: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_latest_coding_smoke() {
        let db = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE coding_smokes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    generated_at TEXT NOT NULL,
                    workspace TEXT NOT NULL,
                    passed INTEGER NOT NULL DEFAULT 0,
                    initial_test_failed INTEGER NOT NULL DEFAULT 0,
                    edit_applied INTEGER NOT NULL DEFAULT 0,
                    final_test_passed INTEGER NOT NULL DEFAULT 0,
                    report_path TEXT NOT NULL,
                    transcript_path TEXT,
                    artifacts TEXT NOT NULL DEFAULT '[]',
                    recorded_at TEXT NOT NULL
                );",
            )
            .unwrap();

        let store = CodingSmokeStore::new(db);
        let now = Utc::now();
        let record = CodingSmokeRecord {
            id: None,
            generated_at: now,
            workspace: "/tmp/px".to_string(),
            passed: true,
            initial_test_failed: true,
            edit_applied: true,
            final_test_passed: true,
            report_path: "artifacts/coding-smoke/report.json".to_string(),
            transcript_path: Some("artifacts/transcripts/task.json".to_string()),
            artifacts: vec!["artifacts/commands/cargo-test.json".to_string()],
            recorded_at: now,
        };
        store.insert(&record).unwrap();

        assert_eq!(store.count().unwrap(), 1);
        assert_eq!(store.pass_count().unwrap(), 1);
        let latest = store.latest().unwrap().unwrap();
        assert_eq!(latest.workspace, "/tmp/px");
        assert!(latest.passed);
        assert_eq!(
            latest.transcript_path.as_deref(),
            Some("artifacts/transcripts/task.json")
        );
        assert_eq!(latest.artifacts.len(), 1);
        assert!(latest.id.is_some());
    }
}
