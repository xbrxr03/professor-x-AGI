use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

use crate::memd::events::EventStore;

pub const STALE_CODING_SESSION_GRACE_MINUTES: i64 = 30;
const STALE_CODING_SESSION_RESTART_THRESHOLD: i64 = 2;

#[derive(Debug, Clone)]
pub struct CodingSessionRecord {
    pub id: String,
    pub generated_at: DateTime<Utc>,
    pub goal: String,
    pub exercise: String,
    pub status: String,
    pub workspace: Option<String>,
    pub smoke_id: Option<i64>,
    pub smoke_report_path: Option<String>,
    pub session_report_path: String,
    pub transcript_path: Option<String>,
    pub artifacts: Vec<String>,
    pub checks: Vec<String>,
    pub plan_steps: Vec<String>,
    pub step_outcomes: Vec<String>,
    pub failure_reason: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CodingSessionStaleCandidate {
    pub session_id: String,
    pub last_activity_at: DateTime<Utc>,
    pub idle_minutes: i64,
    pub newer_process_starts: i64,
    pub reason: String,
}

#[derive(Clone)]
pub struct CodingSessionStore {
    db: Arc<Mutex<Connection>>,
}

impl CodingSessionStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn insert(&self, record: &CodingSessionRecord) -> Result<()> {
        let artifacts = serde_json::to_string(&record.artifacts)?;
        let checks = serde_json::to_string(&record.checks)?;
        let plan_steps = serde_json::to_string(&record.plan_steps)?;
        let step_outcomes = serde_json::to_string(&record.step_outcomes)?;
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO coding_sessions
             (id, generated_at, goal, exercise, status, workspace, smoke_id, smoke_report_path,
              session_report_path, transcript_path, artifacts, checks, plan_steps, step_outcomes,
              failure_reason, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                record.id,
                record.generated_at.to_rfc3339(),
                record.goal,
                record.exercise,
                record.status,
                record.workspace,
                record.smoke_id,
                record.smoke_report_path,
                record.session_report_path,
                record.transcript_path,
                artifacts,
                checks,
                plan_steps,
                step_outcomes,
                record.failure_reason,
                record.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn latest(&self) -> Result<Option<CodingSessionRecord>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, generated_at, goal, status, workspace, smoke_id, smoke_report_path,
                    session_report_path, transcript_path, artifacts, checks, failure_reason,
                    recorded_at, exercise, plan_steps, step_outcomes
             FROM coding_sessions
             ORDER BY generated_at DESC, recorded_at DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(parse_record(row)?))
    }

    pub fn get_by_ref(&self, session_ref: &str) -> Result<Option<CodingSessionRecord>> {
        let session_ref = session_ref.trim();
        if session_ref.is_empty() || session_ref == "latest" {
            return self.latest();
        }

        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, generated_at, goal, status, workspace, smoke_id, smoke_report_path,
                    session_report_path, transcript_path, artifacts, checks, failure_reason,
                    recorded_at, exercise, plan_steps, step_outcomes
             FROM coding_sessions
             WHERE id = ?1 OR id LIKE ?2
             ORDER BY generated_at DESC, recorded_at DESC
             LIMIT 2",
        )?;
        let prefix = format!("{session_ref}%");
        let rows = stmt.query_map(params![session_ref, prefix], parse_record)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        if records.len() > 1 {
            anyhow::bail!("coding session reference '{session_ref}' is ambiguous");
        }
        Ok(records.pop())
    }

    pub fn count(&self) -> Result<i64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT COUNT(*) FROM coding_sessions", [], |row| row.get(0))?)
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<CodingSessionRecord>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, generated_at, goal, status, workspace, smoke_id, smoke_report_path,
                    session_report_path, transcript_path, artifacts, checks, failure_reason,
                    recorded_at, exercise, plan_steps, step_outcomes
             FROM coding_sessions
             ORDER BY generated_at DESC, recorded_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], parse_record)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn running(&self, limit: usize) -> Result<Vec<CodingSessionRecord>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, generated_at, goal, status, workspace, smoke_id, smoke_report_path,
                    session_report_path, transcript_path, artifacts, checks, failure_reason,
                    recorded_at, exercise, plan_steps, step_outcomes
             FROM coding_sessions
             WHERE status = 'running'
             ORDER BY generated_at DESC, recorded_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], parse_record)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }
}

pub fn is_pending_running(session: &CodingSessionRecord) -> bool {
    session.status == "running" && session.session_report_path == "pending"
}

pub fn display_status<'a>(
    session: &'a CodingSessionRecord,
    stale: Option<&CodingSessionStaleCandidate>,
) -> &'a str {
    if stale.is_some() {
        "stale"
    } else {
        session.status.as_str()
    }
}

pub fn stale_candidate(
    events: &EventStore,
    session: &CodingSessionRecord,
    now: DateTime<Utc>,
) -> Result<Option<CodingSessionStaleCandidate>> {
    if !is_pending_running(session) {
        return Ok(None);
    }

    let last_activity_at = events
        .latest_for_session(&session.id)?
        .map(|event| event.timestamp)
        .unwrap_or(session.recorded_at.max(session.generated_at));
    let idle = now.signed_duration_since(last_activity_at);
    if idle < Duration::minutes(STALE_CODING_SESSION_GRACE_MINUTES) {
        return Ok(None);
    }

    let newer_process_starts =
        events.count_type_after_timestamp("daemon.started", last_activity_at)?;
    if newer_process_starts < STALE_CODING_SESSION_RESTART_THRESHOLD {
        return Ok(None);
    }

    let idle_minutes = idle.num_minutes().max(0);
    Ok(Some(CodingSessionStaleCandidate {
        session_id: session.id.clone(),
        last_activity_at,
        idle_minutes,
        newer_process_starts,
        reason: format!(
            "{newer_process_starts} later Professor X process starts were recorded after last session activity at {} and no terminal report was written for {idle_minutes} minute(s)",
            last_activity_at.to_rfc3339()
        ),
    }))
}

fn parse_record(row: &rusqlite::Row) -> rusqlite::Result<CodingSessionRecord> {
    let generated_at_raw: String = row.get(1)?;
    let artifacts_raw: String = row.get(9)?;
    let checks_raw: String = row.get(10)?;
    let recorded_at_raw: String = row.get(12)?;
    let plan_steps_raw: String = row.get(14)?;
    let step_outcomes_raw: String = row.get(15)?;
    Ok(CodingSessionRecord {
        id: row.get(0)?,
        generated_at: parse_time(&generated_at_raw),
        goal: row.get(2)?,
        exercise: row.get(13)?,
        status: row.get(3)?,
        workspace: row.get(4)?,
        smoke_id: row.get(5)?,
        smoke_report_path: row.get(6)?,
        session_report_path: row.get(7)?,
        transcript_path: row.get(8)?,
        artifacts: serde_json::from_str(&artifacts_raw).unwrap_or_default(),
        checks: serde_json::from_str(&checks_raw).unwrap_or_default(),
        plan_steps: serde_json::from_str(&plan_steps_raw).unwrap_or_default(),
        step_outcomes: serde_json::from_str(&step_outcomes_raw).unwrap_or_default(),
        failure_reason: row.get(11)?,
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
    fn records_recent_coding_sessions() {
        let db = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE coding_sessions (
                    id TEXT PRIMARY KEY,
                    generated_at TEXT NOT NULL,
                    goal TEXT NOT NULL,
                    exercise TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL,
                    workspace TEXT,
                    smoke_id INTEGER,
                    smoke_report_path TEXT,
                    session_report_path TEXT NOT NULL,
                    transcript_path TEXT,
                    artifacts TEXT NOT NULL DEFAULT '[]',
                    checks TEXT NOT NULL DEFAULT '[]',
                    plan_steps TEXT NOT NULL DEFAULT '[]',
                    step_outcomes TEXT NOT NULL DEFAULT '[]',
                    failure_reason TEXT,
                    recorded_at TEXT NOT NULL
                );",
            )
            .unwrap();

        let now = Utc::now();
        let store = CodingSessionStore::new(db);
        store
            .insert(&CodingSessionRecord {
                id: "session-1".to_string(),
                generated_at: now,
                goal: "fix a failing Rust test".to_string(),
                exercise: "add_i32".to_string(),
                status: "passed".to_string(),
                workspace: Some("/tmp/px".to_string()),
                smoke_id: Some(7),
                smoke_report_path: Some("artifacts/coding-smoke/report.json".to_string()),
                session_report_path: "artifacts/coding-sessions/session.json".to_string(),
                transcript_path: Some("artifacts/transcripts/task.json".to_string()),
                artifacts: vec!["artifacts/commands/cargo-test.json".to_string()],
                checks: vec![
                    "initial cargo test failed".to_string(),
                    "final cargo test passed".to_string(),
                ],
                plan_steps: vec![
                    "run tests before editing".to_string(),
                    "apply exact patch".to_string(),
                    "run tests after editing".to_string(),
                ],
                step_outcomes: vec![
                    "initial test failed".to_string(),
                    "patch applied".to_string(),
                    "final test passed".to_string(),
                ],
                failure_reason: None,
                recorded_at: now,
            })
            .unwrap();

        let latest = store.latest().unwrap().unwrap();
        assert_eq!(latest.id, "session-1");
        assert_eq!(latest.exercise, "add_i32");
        assert_eq!(latest.status, "passed");
        assert_eq!(latest.plan_steps.len(), 3);
        assert_eq!(latest.step_outcomes.len(), 3);
        assert_eq!(latest.smoke_id, Some(7));
        assert_eq!(
            store.get_by_ref("session-1").unwrap().unwrap().id,
            "session-1"
        );
        assert_eq!(store.get_by_ref("latest").unwrap().unwrap().id, "session-1");
        assert_eq!(store.recent(5).unwrap().len(), 1);
        assert_eq!(store.running(5).unwrap().len(), 0);
    }

    #[test]
    fn stale_candidate_requires_pending_running_row_and_multiple_restarts() {
        let db = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE coding_sessions (
                    id TEXT PRIMARY KEY,
                    generated_at TEXT NOT NULL,
                    goal TEXT NOT NULL,
                    exercise TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL,
                    workspace TEXT,
                    smoke_id INTEGER,
                    smoke_report_path TEXT,
                    session_report_path TEXT NOT NULL,
                    transcript_path TEXT,
                    artifacts TEXT NOT NULL DEFAULT '[]',
                    checks TEXT NOT NULL DEFAULT '[]',
                    plan_steps TEXT NOT NULL DEFAULT '[]',
                    step_outcomes TEXT NOT NULL DEFAULT '[]',
                    failure_reason TEXT,
                    recorded_at TEXT NOT NULL
                );
                CREATE TABLE agent_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp TEXT NOT NULL,
                    session_id TEXT,
                    task_id TEXT,
                    event_type TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    payload TEXT NOT NULL DEFAULT '{}'
                );",
            )
            .unwrap();
        let store = CodingSessionStore::new(Arc::clone(&db));
        let events = EventStore::new(Arc::clone(&db));
        let generated_at = Utc::now() - Duration::minutes(90);
        let session = CodingSessionRecord {
            id: "session-stale".to_string(),
            generated_at,
            goal: "repair stale row".to_string(),
            exercise: "repo_patch_apply_commit".to_string(),
            status: "running".to_string(),
            workspace: Some("repo-root".to_string()),
            smoke_id: None,
            smoke_report_path: None,
            session_report_path: "pending".to_string(),
            transcript_path: None,
            artifacts: Vec::new(),
            checks: Vec::new(),
            plan_steps: Vec::new(),
            step_outcomes: Vec::new(),
            failure_reason: None,
            recorded_at: generated_at,
        };
        store.insert(&session).unwrap();
        db.lock()
            .unwrap()
            .execute(
                "INSERT INTO agent_events
                 (timestamp, session_id, task_id, event_type, summary, payload)
                 VALUES (?1, NULL, NULL, ?2, ?3, ?4)",
                params![
                    generated_at.to_rfc3339(),
                    "coding.session.started",
                    "started",
                    serde_json::json!({"session_id": "session-stale"}).to_string(),
                ],
            )
            .unwrap();
        events
            .append(
                None,
                None,
                "daemon.started",
                "restart 1",
                serde_json::json!({}),
            )
            .unwrap();
        events
            .append(
                None,
                None,
                "daemon.started",
                "restart 2",
                serde_json::json!({}),
            )
            .unwrap();

        let candidate = stale_candidate(&events, &session, Utc::now()).unwrap();
        assert!(candidate.is_some());
        assert_eq!(candidate.unwrap().session_id, "session-stale");
    }

    #[test]
    fn stale_candidate_ignores_single_restart() {
        let db = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE agent_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp TEXT NOT NULL,
                    session_id TEXT,
                    task_id TEXT,
                    event_type TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    payload TEXT NOT NULL DEFAULT '{}'
                );",
            )
            .unwrap();
        let events = EventStore::new(Arc::clone(&db));
        let session = CodingSessionRecord {
            id: "session-fresh".to_string(),
            generated_at: Utc::now() - Duration::minutes(90),
            goal: "still plausible".to_string(),
            exercise: "repo_patch_apply_commit".to_string(),
            status: "running".to_string(),
            workspace: None,
            smoke_id: None,
            smoke_report_path: None,
            session_report_path: "pending".to_string(),
            transcript_path: None,
            artifacts: Vec::new(),
            checks: Vec::new(),
            plan_steps: Vec::new(),
            step_outcomes: Vec::new(),
            failure_reason: None,
            recorded_at: Utc::now() - Duration::minutes(90),
        };
        db.lock()
            .unwrap()
            .execute(
                "INSERT INTO agent_events
                 (timestamp, session_id, task_id, event_type, summary, payload)
                 VALUES (?1, NULL, NULL, ?2, ?3, ?4)",
                params![
                    session.recorded_at.to_rfc3339(),
                    "coding.session.started",
                    "started",
                    serde_json::json!({"session_id": "session-fresh"}).to_string(),
                ],
            )
            .unwrap();
        events
            .append(
                None,
                None,
                "daemon.started",
                "restart 1",
                serde_json::json!({}),
            )
            .unwrap();

        assert!(stale_candidate(&events, &session, Utc::now())
            .unwrap()
            .is_none());
    }
}
