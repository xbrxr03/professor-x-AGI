use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct AutonomyQueueItem {
    pub id: String,
    pub goal: String,
    pub kind: String,
    pub profile: String,
    pub cycles: u32,
    pub priority: u8,
    pub status: String,
    pub result_run_id: Option<String>,
    pub result_report_path: Option<String>,
    pub failure_reason: Option<String>,
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct AutonomyQueueStore {
    db: Arc<Mutex<Connection>>,
}

impl AutonomyQueueStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn enqueue(
        &self,
        goal: impl Into<String>,
        kind: impl Into<String>,
        profile: impl Into<String>,
        cycles: u32,
        priority: u8,
    ) -> Result<AutonomyQueueItem> {
        let now = Utc::now();
        let item = AutonomyQueueItem {
            id: uuid::Uuid::new_v4().to_string(),
            goal: goal.into(),
            kind: kind.into(),
            profile: profile.into(),
            cycles: cycles.clamp(1, 50),
            priority,
            status: "pending".to_string(),
            result_run_id: None,
            result_report_path: None,
            failure_reason: None,
            queued_at: now,
            started_at: None,
            completed_at: None,
            updated_at: now,
        };
        self.insert(&item)?;
        Ok(item)
    }

    pub fn insert(&self, item: &AutonomyQueueItem) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO autonomy_queue
             (id, goal, kind, profile, cycles, priority, status, result_run_id, result_report_path,
              failure_reason, queued_at, started_at, completed_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                item.id,
                item.goal,
                item.kind,
                item.profile,
                item.cycles,
                item.priority,
                item.status,
                item.result_run_id,
                item.result_report_path,
                item.failure_reason,
                item.queued_at.to_rfc3339(),
                item.started_at.map(|value| value.to_rfc3339()),
                item.completed_at.map(|value| value.to_rfc3339()),
                item.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn next_pending(&self) -> Result<Option<AutonomyQueueItem>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, goal, kind, profile, cycles, priority, status, result_run_id,
                    result_report_path, failure_reason, queued_at, started_at, completed_at,
                    updated_at
             FROM autonomy_queue
             WHERE status = 'pending'
             ORDER BY priority DESC, queued_at ASC
             LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(parse_item(row)?))
    }

    pub fn mark_running(&self, id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE autonomy_queue
             SET status = 'running', started_at = COALESCE(started_at, ?2), updated_at = ?2
             WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    pub fn mark_finished(
        &self,
        id: &str,
        status: &str,
        result_run_id: Option<&str>,
        result_report_path: Option<&str>,
        failure_reason: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE autonomy_queue
             SET status = ?2, result_run_id = ?3, result_report_path = ?4,
                 failure_reason = ?5, completed_at = ?6, updated_at = ?6
             WHERE id = ?1",
            params![
                id,
                status,
                result_run_id,
                result_report_path,
                failure_reason,
                now
            ],
        )?;
        Ok(())
    }

    pub fn count_pending(&self) -> Result<i64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row(
            "SELECT COUNT(*) FROM autonomy_queue WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<AutonomyQueueItem>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, goal, kind, profile, cycles, priority, status, result_run_id,
                    result_report_path, failure_reason, queued_at, started_at, completed_at,
                    updated_at
             FROM autonomy_queue
             ORDER BY updated_at DESC, queued_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], parse_item)?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    pub fn resolve_ref(&self, item_ref: &str) -> Result<Option<AutonomyQueueItem>> {
        let item_ref = item_ref.trim();
        if item_ref.is_empty() || item_ref == "latest" {
            return Ok(self.recent(1)?.into_iter().next());
        }
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, goal, kind, profile, cycles, priority, status, result_run_id,
                    result_report_path, failure_reason, queued_at, started_at, completed_at,
                    updated_at
             FROM autonomy_queue
             WHERE id LIKE ?1
             ORDER BY updated_at DESC, queued_at DESC
             LIMIT 2",
        )?;
        let rows = stmt.query_map([format!("{item_ref}%")], parse_item)?;
        let mut matches = Vec::new();
        for row in rows {
            matches.push(row?);
        }
        if matches.len() > 1 {
            anyhow::bail!("queue reference '{item_ref}' is ambiguous");
        }
        Ok(matches.into_iter().next())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutonomyQueueBrief {
    pub queue_id: String,
    pub summary: String,
    pub next_command: String,
    pub commands: Vec<String>,
}

pub fn short_queue_id(id: &str) -> String {
    id.chars().take(8).collect()
}

pub fn autonomy_queue_brief(
    item: &AutonomyQueueItem,
    max_summary_chars: usize,
) -> AutonomyQueueBrief {
    let commands = autonomy_queue_commands(item);
    AutonomyQueueBrief {
        queue_id: short_queue_id(&item.id),
        summary: autonomy_queue_summary(item, max_summary_chars),
        next_command: commands.first().cloned().unwrap_or_else(|| {
            format!(
                "cargo run -- --prof-x-queue-review {}",
                short_queue_id(&item.id)
            )
        }),
        commands,
    }
}

pub fn autonomy_queue_summary(item: &AutonomyQueueItem, max_chars: usize) -> String {
    let result = item
        .result_run_id
        .as_ref()
        .map(|run| format!(" / run {}", short_queue_id(run)))
        .or_else(|| {
            item.result_report_path
                .as_ref()
                .map(|path| format!(" / report {}", truncate_for_queue(path, 42)))
        })
        .unwrap_or_default();
    let failure = item
        .failure_reason
        .as_ref()
        .map(|reason| format!(" / failure {}", truncate_for_queue(reason, 42)))
        .unwrap_or_default();
    truncate_for_queue(
        &format!(
            "{}:{} p{} c{} {}{}{}",
            item.kind, item.profile, item.priority, item.cycles, item.goal, result, failure
        ),
        max_chars,
    )
}

pub fn autonomy_queue_commands(item: &AutonomyQueueItem) -> Vec<String> {
    let queue = short_queue_id(&item.id);
    match item.status.as_str() {
        "pending" | "running" => vec![
            "cargo run -- --prof-x-step-live 1".to_string(),
            format!("cargo run -- --prof-x-queue-review {queue}"),
        ],
        "passed" | "completed" => vec![
            format!("cargo run -- --prof-x-queue-review {queue}"),
            format!("cargo run -- --prof-x-queue-replay {queue}"),
            format!("cargo run -- --prof-x-queue-publish {queue}"),
        ],
        "failed" | "rejected" => vec![
            format!("cargo run -- --prof-x-queue-review {queue}"),
            format!("cargo run -- --prof-x-queue-replay {queue}"),
        ],
        _ => vec![format!("cargo run -- --prof-x-queue-review {queue}")],
    }
}

pub fn autonomy_queue_next_command(item: &AutonomyQueueItem) -> String {
    autonomy_queue_brief(item, 96).next_command
}

fn truncate_for_queue(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}

fn parse_item(row: &rusqlite::Row) -> rusqlite::Result<AutonomyQueueItem> {
    let queued_at_raw: String = row.get(10)?;
    let started_at_raw: Option<String> = row.get(11)?;
    let completed_at_raw: Option<String> = row.get(12)?;
    let updated_at_raw: String = row.get(13)?;
    Ok(AutonomyQueueItem {
        id: row.get(0)?,
        goal: row.get(1)?,
        kind: row.get(2)?,
        profile: row.get(3)?,
        cycles: row.get::<_, u32>(4)?,
        priority: row.get::<_, u8>(5)?,
        status: row.get(6)?,
        result_run_id: row.get(7)?,
        result_report_path: row.get(8)?,
        failure_reason: row.get(9)?,
        queued_at: parse_time(&queued_at_raw),
        started_at: started_at_raw.as_deref().map(parse_time),
        completed_at: completed_at_raw.as_deref().map(parse_time),
        updated_at: parse_time(&updated_at_raw),
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
    fn queue_orders_by_priority_and_records_result() {
        let db = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE autonomy_queue (
                    id TEXT PRIMARY KEY,
                    goal TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    profile TEXT NOT NULL,
                    cycles INTEGER NOT NULL DEFAULT 1,
                    priority INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL,
                    result_run_id TEXT,
                    result_report_path TEXT,
                    failure_reason TEXT,
                    queued_at TEXT NOT NULL,
                    started_at TEXT,
                    completed_at TEXT,
                    updated_at TEXT NOT NULL
                );",
            )
            .unwrap();
        let store = AutonomyQueueStore::new(db);

        let low = store.enqueue("low", "operator_run", "core", 1, 10).unwrap();
        let high = store
            .enqueue("high", "operator_run", "commit", 3, 90)
            .unwrap();

        assert_eq!(store.count_pending().unwrap(), 2);
        assert_eq!(store.next_pending().unwrap().unwrap().id, high.id);

        store.mark_running(&high.id).unwrap();
        store
            .mark_finished(
                &high.id,
                "done",
                Some("run-123"),
                Some("artifacts/work-loop/run.json"),
                None,
            )
            .unwrap();

        let recent = store.recent(5).unwrap();
        assert_eq!(recent[0].status, "done");
        assert_eq!(recent[0].result_run_id.as_deref(), Some("run-123"));
        assert_eq!(store.next_pending().unwrap().unwrap().id, low.id);
        assert_eq!(store.resolve_ref("latest").unwrap().unwrap().id, high.id);
        assert_eq!(
            store
                .resolve_ref(&high.id[..8])
                .unwrap()
                .unwrap()
                .result_run_id
                .as_deref(),
            Some("run-123")
        );
        assert!(store.resolve_ref("missing").unwrap().is_none());
    }

    #[test]
    fn queue_ref_rejects_ambiguous_prefixes() {
        let db = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE autonomy_queue (
                    id TEXT PRIMARY KEY,
                    goal TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    profile TEXT NOT NULL,
                    cycles INTEGER NOT NULL DEFAULT 1,
                    priority INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL,
                    result_run_id TEXT,
                    result_report_path TEXT,
                    failure_reason TEXT,
                    queued_at TEXT NOT NULL,
                    started_at TEXT,
                    completed_at TEXT,
                    updated_at TEXT NOT NULL
                );",
            )
            .unwrap();
        let store = AutonomyQueueStore::new(db);
        let now = Utc::now();
        for id in [
            "abc11111-aaaa-bbbb-cccc-123456789abc",
            "abc22222-aaaa-bbbb-cccc-123456789abc",
        ] {
            store
                .insert(&AutonomyQueueItem {
                    id: id.to_string(),
                    goal: "goal".to_string(),
                    kind: "operator_run".to_string(),
                    profile: "core".to_string(),
                    cycles: 1,
                    priority: 10,
                    status: "done".to_string(),
                    result_run_id: None,
                    result_report_path: None,
                    failure_reason: None,
                    queued_at: now,
                    started_at: None,
                    completed_at: None,
                    updated_at: now,
                })
                .unwrap();
        }

        assert!(store.resolve_ref("abc").is_err());
    }

    fn display_item(status: &str) -> AutonomyQueueItem {
        let now = Utc::now();
        AutonomyQueueItem {
            id: "12345678-90ab-cdef-1234-567890abcdef".to_string(),
            goal: "make Professor X observable like a coding CLI".to_string(),
            kind: "operator_run".to_string(),
            profile: "commit".to_string(),
            cycles: 5,
            priority: 65,
            status: status.to_string(),
            result_run_id: Some("abcdef12-3456-7890-abcd-ef1234567890".to_string()),
            result_report_path: None,
            failure_reason: None,
            queued_at: now,
            started_at: None,
            completed_at: None,
            updated_at: now,
        }
    }

    #[test]
    fn queue_brief_surfaces_summary_and_next_command_for_pending_work() {
        let brief = autonomy_queue_brief(&display_item("pending"), 120);

        assert_eq!(brief.queue_id, "12345678");
        assert!(brief.summary.contains("operator_run:commit"));
        assert!(brief.summary.contains("run abcdef12"));
        assert_eq!(brief.next_command, "cargo run -- --prof-x-step-live 1");
        assert!(brief
            .commands
            .iter()
            .any(|cmd| cmd == "cargo run -- --prof-x-queue-review 12345678"));
    }

    #[test]
    fn queue_brief_surfaces_review_replay_publish_for_passed_work() {
        let brief = autonomy_queue_brief(&display_item("passed"), 120);

        assert_eq!(
            brief.next_command,
            "cargo run -- --prof-x-queue-review 12345678"
        );
        assert!(brief
            .commands
            .iter()
            .any(|cmd| cmd == "cargo run -- --prof-x-queue-publish 12345678"));
    }
}
