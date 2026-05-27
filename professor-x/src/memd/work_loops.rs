use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkLoopSmokeRecord {
    pub cycle: u32,
    #[serde(default = "default_cycle_kind")]
    pub kind: String,
    pub smoke_id: Option<i64>,
    pub passed: bool,
    pub report_path: String,
    pub transcript_path: Option<String>,
    pub workspace: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkLoopPlannedJob {
    pub cycle: u32,
    pub kind: String,
    pub label: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct WorkLoopRunRecord {
    pub id: Option<i64>,
    pub run_id: String,
    pub run_kind: String,
    pub profile: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub requested_cycles: u32,
    pub completed_cycles: u32,
    pub passed_cycles: u32,
    pub failed_cycles: u32,
    pub report_path: String,
    pub planned_jobs: Vec<WorkLoopPlannedJob>,
    pub smoke_records: Vec<WorkLoopSmokeRecord>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct WorkLoopRunStore {
    db: Arc<Mutex<Connection>>,
}

impl WorkLoopRunStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn insert(&self, record: &WorkLoopRunRecord) -> Result<()> {
        let planned_jobs = serde_json::to_string(&record.planned_jobs)?;
        let smoke_records = serde_json::to_string(&record.smoke_records)?;
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO work_loop_runs
             (run_id, run_kind, profile, started_at, completed_at, requested_cycles,
              completed_cycles, passed_cycles, failed_cycles, report_path, planned_jobs,
              smoke_records, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                record.run_id,
                record.run_kind,
                record.profile,
                record.started_at.to_rfc3339(),
                record.completed_at.to_rfc3339(),
                record.requested_cycles as i64,
                record.completed_cycles as i64,
                record.passed_cycles as i64,
                record.failed_cycles as i64,
                record.report_path,
                planned_jobs,
                smoke_records,
                record.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn count(&self) -> Result<i64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT COUNT(*) FROM work_loop_runs", [], |row| row.get(0))?)
    }

    pub fn latest(&self) -> Result<Option<WorkLoopRunRecord>> {
        Ok(self.recent(1)?.into_iter().next())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<WorkLoopRunRecord>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, run_id, run_kind, profile, started_at, completed_at, requested_cycles,
                    completed_cycles, passed_cycles, failed_cycles, report_path, planned_jobs,
                    smoke_records, recorded_at
             FROM work_loop_runs
             ORDER BY recorded_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit.clamp(1, 500) as i64], parse_record)?;
        rows.map(|row| row.map_err(Into::into)).collect()
    }
}

fn parse_record(row: &rusqlite::Row) -> rusqlite::Result<WorkLoopRunRecord> {
    let started_at_raw: String = row.get(4)?;
    let completed_at_raw: String = row.get(5)?;
    let planned_jobs_raw: String = row.get(11)?;
    let smoke_records_raw: String = row.get(12)?;
    let recorded_at_raw: String = row.get(13)?;
    Ok(WorkLoopRunRecord {
        id: row.get(0)?,
        run_id: row.get(1)?,
        run_kind: row.get(2)?,
        profile: row.get(3)?,
        started_at: parse_time(&started_at_raw),
        completed_at: parse_time(&completed_at_raw),
        requested_cycles: row.get::<_, i64>(6)? as u32,
        completed_cycles: row.get::<_, i64>(7)? as u32,
        passed_cycles: row.get::<_, i64>(8)? as u32,
        failed_cycles: row.get::<_, i64>(9)? as u32,
        report_path: row.get(10)?,
        planned_jobs: serde_json::from_str(&planned_jobs_raw).unwrap_or_default(),
        smoke_records: serde_json::from_str(&smoke_records_raw).unwrap_or_default(),
        recorded_at: parse_time(&recorded_at_raw),
    })
}

fn parse_time(raw: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn default_cycle_kind() -> String {
    "coding_smoke".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_recent_work_loop_runs() {
        let db = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE work_loop_runs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    run_id TEXT NOT NULL UNIQUE,
                    run_kind TEXT NOT NULL DEFAULT 'supervised',
                    profile TEXT NOT NULL DEFAULT 'basic',
                    started_at TEXT NOT NULL,
                    completed_at TEXT NOT NULL,
                    requested_cycles INTEGER NOT NULL DEFAULT 0,
                    completed_cycles INTEGER NOT NULL DEFAULT 0,
                    passed_cycles INTEGER NOT NULL DEFAULT 0,
                    failed_cycles INTEGER NOT NULL DEFAULT 0,
                    report_path TEXT NOT NULL,
                    planned_jobs TEXT NOT NULL DEFAULT '[]',
                    smoke_records TEXT NOT NULL DEFAULT '[]',
                    recorded_at TEXT NOT NULL
                );",
            )
            .unwrap();
        let store = WorkLoopRunStore::new(db);
        let now = Utc::now();
        store
            .insert(&WorkLoopRunRecord {
                id: None,
                run_id: "run-1".to_string(),
                run_kind: "operator".to_string(),
                profile: "core".to_string(),
                started_at: now,
                completed_at: now,
                requested_cycles: 1,
                completed_cycles: 1,
                passed_cycles: 1,
                failed_cycles: 0,
                report_path: "artifacts/work-loop/loop.json".to_string(),
                planned_jobs: vec![WorkLoopPlannedJob {
                    cycle: 1,
                    kind: "coding_smoke".to_string(),
                    label: "coding-agent smoke".to_string(),
                    reason: "verify coding toolchain".to_string(),
                }],
                smoke_records: vec![WorkLoopSmokeRecord {
                    cycle: 1,
                    kind: "coding_smoke".to_string(),
                    smoke_id: Some(7),
                    passed: true,
                    report_path: "artifacts/coding-smoke/smoke.json".to_string(),
                    transcript_path: Some("artifacts/transcripts/task.json".to_string()),
                    workspace: "/tmp/px".to_string(),
                    detail: "coding smoke passed".to_string(),
                }],
                recorded_at: now,
            })
            .unwrap();

        assert_eq!(store.count().unwrap(), 1);
        let latest = store.latest().unwrap().unwrap();
        assert_eq!(latest.run_id, "run-1");
        assert_eq!(latest.run_kind, "operator");
        assert_eq!(latest.profile, "core");
        assert_eq!(latest.planned_jobs[0].reason, "verify coding toolchain");
        assert_eq!(latest.passed_cycles, 1);
        assert_eq!(latest.smoke_records[0].smoke_id, Some(7));
    }
}
