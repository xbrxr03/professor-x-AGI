use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AgentEvent {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub event_type: String,
    pub summary: String,
    pub payload: Value,
}

#[derive(Clone)]
pub struct EventStore {
    db: Arc<Mutex<Connection>>,
    jsonl_dir: Option<PathBuf>,
}

impl EventStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self {
            db,
            jsonl_dir: None,
        }
    }

    pub fn with_jsonl_mirror(mut self, dir: PathBuf) -> Self {
        self.jsonl_dir = Some(dir);
        self
    }

    pub fn append(
        &self,
        session_id: Option<Uuid>,
        task_id: Option<Uuid>,
        event_type: &str,
        summary: impl AsRef<str>,
        payload: Value,
    ) -> Result<()> {
        let timestamp = Utc::now();
        let session_id = session_id.map(|id| id.to_string());
        let task_id = task_id.map(|id| id.to_string());
        let payload_raw = payload.to_string();
        let id = {
            let db = self.db.lock().unwrap();
            db.execute(
                "INSERT INTO agent_events
                 (timestamp, session_id, task_id, event_type, summary, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    timestamp.to_rfc3339(),
                    session_id,
                    task_id,
                    event_type,
                    summary.as_ref(),
                    payload_raw,
                ],
            )?;
            db.last_insert_rowid()
        };
        self.append_jsonl(id, timestamp, session_id, task_id, event_type, summary.as_ref(), &payload)?;
        Ok(())
    }

    pub fn tail(&self, limit: usize) -> Result<Vec<AgentEvent>> {
        let limit = limit.clamp(1, 500) as i64;
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, timestamp, session_id, task_id, event_type, summary, payload
             FROM agent_events
             ORDER BY id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], parse_event)?;
        let mut events: Vec<AgentEvent> = rows.map(|r| r.map_err(Into::into)).collect::<Result<_>>()?;
        events.reverse();
        Ok(events)
    }

    pub fn work_tail(&self, limit: usize) -> Result<Vec<AgentEvent>> {
        let limit = limit.clamp(1, 500) as i64;
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(&format!(
            "SELECT id, timestamp, session_id, task_id, event_type, summary, payload
             FROM agent_events
             WHERE {}
             ORDER BY id DESC
             LIMIT ?1",
            work_event_where_clause()
        ))?;
        let rows = stmt.query_map(params![limit], parse_event)?;
        let mut events: Vec<AgentEvent> = rows.map(|r| r.map_err(Into::into)).collect::<Result<_>>()?;
        events.reverse();
        Ok(events)
    }

    pub fn after_id(&self, last_id: i64, limit: usize) -> Result<Vec<AgentEvent>> {
        let limit = limit.clamp(1, 500) as i64;
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, timestamp, session_id, task_id, event_type, summary, payload
             FROM agent_events
             WHERE id > ?1
             ORDER BY id ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![last_id, limit], parse_event)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn work_after_id(&self, last_id: i64, limit: usize) -> Result<Vec<AgentEvent>> {
        let limit = limit.clamp(1, 500) as i64;
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(&format!(
            "SELECT id, timestamp, session_id, task_id, event_type, summary, payload
             FROM agent_events
             WHERE id > ?1 AND ({})
             ORDER BY id ASC
             LIMIT ?2",
            work_event_where_clause()
        ))?;
        let rows = stmt.query_map(params![last_id, limit], parse_event)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn for_task(&self, task_id: Uuid, limit: usize) -> Result<Vec<AgentEvent>> {
        let limit = limit.clamp(1, 2000) as i64;
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, timestamp, session_id, task_id, event_type, summary, payload
             FROM agent_events
             WHERE task_id = ?1
             ORDER BY id ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![task_id.to_string(), limit], parse_event)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    fn append_jsonl(
        &self,
        id: i64,
        timestamp: DateTime<Utc>,
        session_id: Option<String>,
        task_id: Option<String>,
        event_type: &str,
        summary: &str,
        payload: &Value,
    ) -> Result<()> {
        let Some(dir) = &self.jsonl_dir else {
            return Ok(());
        };
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{}.jsonl", timestamp.format("%Y-%m-%d")));
        let record = serde_json::json!({
            "id": id,
            "timestamp": timestamp.to_rfc3339(),
            "session_id": session_id,
            "task_id": task_id,
            "event_type": event_type,
            "summary": summary,
            "payload": payload,
        });
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{}", record)?;
        Ok(())
    }
}

fn work_event_where_clause() -> &'static str {
    "event_type LIKE 'task.%'
      OR event_type LIKE 'tool.%'
      OR event_type LIKE 'policy.%'
      OR event_type LIKE 'react.%'
      OR event_type LIKE 'coding.smoke.%'
      OR event_type LIKE 'evolution.%'
      OR event_type LIKE 'work_loop.%'
      OR event_type = 'transcript.written'"
}

fn parse_event(row: &rusqlite::Row) -> rusqlite::Result<AgentEvent> {
    let timestamp_raw: String = row.get(1)?;
    let timestamp = DateTime::parse_from_rfc3339(&timestamp_raw)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let payload_raw: String = row.get(6)?;
    let payload = serde_json::from_str(&payload_raw).unwrap_or(Value::Null);

    Ok(AgentEvent {
        id: row.get(0)?,
        timestamp,
        session_id: row.get(2)?,
        task_id: row.get(3)?,
        event_type: row.get(4)?,
        summary: row.get(5)?,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_tails_events() {
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
        let store = EventStore::new(db);

        store
            .append(None, None, "daemon.started", "started", serde_json::json!({}))
            .unwrap();
        store
            .append(None, None, "task.queued", "queued", serde_json::json!({"priority": 100}))
            .unwrap();

        let events = store.tail(10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "daemon.started");
        assert_eq!(events[1].payload["priority"], 100);
        assert_eq!(store.after_id(events[0].id, 10).unwrap().len(), 1);
        assert_eq!(store.work_tail(10).unwrap().len(), 1);
        assert_eq!(store.work_after_id(0, 10).unwrap().len(), 1);
    }
}
