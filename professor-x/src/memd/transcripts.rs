use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::agentd::graph::{ExecutionStep, TaskNode};
use crate::memd::events::{AgentEvent, EventStore};

#[derive(Debug, Clone)]
pub struct TranscriptSummary {
    pub id: String,
    pub task_id: String,
    pub task_description: String,
    pub status: String,
    pub attempt_count: u8,
    pub step_count: usize,
    pub transcript_path: String,
    pub summary: String,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct TranscriptStore {
    db: Arc<Mutex<Connection>>,
    transcript_dir: PathBuf,
}

impl TranscriptStore {
    pub fn new(db: Arc<Mutex<Connection>>, transcript_dir: PathBuf) -> Self {
        Self { db, transcript_dir }
    }

    pub fn record_task(
        &self,
        task: &TaskNode,
        status: &str,
        summary: &str,
        events: &EventStore,
    ) -> Result<PathBuf> {
        let task_events = events.for_task(task.id, 2000)?;
        let session_ids = task_events
            .iter()
            .filter_map(|event| event.session_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let now = Utc::now();
        let dir = self.transcript_dir.join(now.format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", task.id));
        let repo = repo_root();
        let git_status = git_output_lines(&repo, &["status", "--short"]);
        let changed_files = git_output_lines(&repo, &["diff", "--name-only", "HEAD"]);
        let (git_diff, git_diff_truncated) = git_diff_snapshot(&repo);
        let tool_artifacts = collect_tool_artifacts(task);
        let transcript = TaskTranscript {
            id: Uuid::new_v4(),
            task_id: task.id,
            session_ids: session_ids.clone(),
            task_description: task.description.clone(),
            task_type: format!("{:?}", task.task_type),
            status: status.to_string(),
            started_at: task.started_at,
            completed_at: task.completed_at,
            attempt_count: task.attempt_count,
            max_attempts: task.max_attempts,
            step_count: task.steps.len(),
            summary: summary.to_string(),
            review: TaskReview {
                tool_artifacts,
                changed_files,
                git_status,
                git_diff,
                git_diff_truncated,
            },
            events: task_events.iter().map(TranscriptEvent::from).collect(),
            steps: task.steps.iter().map(TranscriptStep::from).collect(),
            recorded_at: now,
        };

        let json = serde_json::to_string_pretty(&transcript)?;
        let mut file = std::fs::File::create(&path)?;
        writeln!(file, "{json}")?;

        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO task_transcripts
             (id, task_id, session_ids, task_description, status, started_at, completed_at,
              attempt_count, step_count, transcript_path, summary, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                transcript.id.to_string(),
                task.id.to_string(),
                serde_json::to_string(&session_ids)?,
                task.description,
                status,
                task.started_at.map(|dt| dt.to_rfc3339()),
                task.completed_at.map(|dt| dt.to_rfc3339()),
                task.attempt_count as i64,
                task.steps.len() as i64,
                path.to_string_lossy(),
                summary,
                now.to_rfc3339(),
            ],
        )?;

        Ok(path)
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<TranscriptSummary>> {
        let limit = limit.clamp(1, 100) as i64;
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, task_id, task_description, status, attempt_count, step_count,
                    transcript_path, summary, recorded_at
             FROM task_transcripts
             ORDER BY recorded_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], parse_summary)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn latest(&self) -> Result<Option<TranscriptSummary>> {
        Ok(self.recent(1)?.into_iter().next())
    }

    pub fn get_by_task_prefix(&self, task_ref: &str) -> Result<Option<TranscriptSummary>> {
        let db = self.db.lock().unwrap();
        let pattern = format!("{task_ref}%");
        let mut stmt = db.prepare(
            "SELECT id, task_id, task_description, status, attempt_count, step_count,
                    transcript_path, summary, recorded_at
             FROM task_transcripts
             WHERE task_id LIKE ?1
             ORDER BY recorded_at DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![pattern])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(parse_summary(row)?))
    }
}

fn parse_summary(row: &rusqlite::Row) -> rusqlite::Result<TranscriptSummary> {
    let recorded_at_raw: String = row.get(8)?;
    Ok(TranscriptSummary {
        id: row.get(0)?,
        task_id: row.get(1)?,
        task_description: row.get(2)?,
        status: row.get(3)?,
        attempt_count: row.get::<_, i64>(4)? as u8,
        step_count: row.get::<_, i64>(5)? as usize,
        transcript_path: row.get(6)?,
        summary: row.get(7)?,
        recorded_at: parse_time(&recorded_at_raw),
    })
}

#[derive(Debug, Serialize)]
struct TaskTranscript {
    id: Uuid,
    task_id: Uuid,
    session_ids: Vec<String>,
    task_description: String,
    task_type: String,
    status: String,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    attempt_count: u8,
    max_attempts: u8,
    step_count: usize,
    summary: String,
    review: TaskReview,
    events: Vec<TranscriptEvent>,
    steps: Vec<TranscriptStep>,
    recorded_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct TaskReview {
    tool_artifacts: Vec<String>,
    changed_files: Vec<String>,
    git_status: Vec<String>,
    git_diff: String,
    git_diff_truncated: bool,
}

#[derive(Debug, Serialize)]
struct TranscriptEvent {
    id: i64,
    timestamp: DateTime<Utc>,
    session_id: Option<String>,
    event_type: String,
    summary: String,
    payload: Value,
}

impl From<&AgentEvent> for TranscriptEvent {
    fn from(event: &AgentEvent) -> Self {
        Self {
            id: event.id,
            timestamp: event.timestamp,
            session_id: event.session_id.clone(),
            event_type: event.event_type.clone(),
            summary: event.summary.clone(),
            payload: event.payload.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
struct TranscriptStep {
    index: u32,
    thought: String,
    tool_name: String,
    params: Value,
    risk_score: u8,
    observation_success: bool,
    observation_output: String,
    observation_error: Option<String>,
    observation_artifacts: Vec<String>,
    execution_ms: u64,
    timestamp: DateTime<Utc>,
}

impl From<&ExecutionStep> for TranscriptStep {
    fn from(step: &ExecutionStep) -> Self {
        Self {
            index: step.index,
            thought: step.thought.clone(),
            tool_name: step.action.tool_name.clone(),
            params: step.action.params.clone(),
            risk_score: step.action.risk_score,
            observation_success: step.observation.success,
            observation_output: step.observation.output.clone(),
            observation_error: step.observation.error.clone(),
            observation_artifacts: step.observation.artifacts.clone(),
            execution_ms: step.observation.execution_ms,
            timestamp: step.timestamp,
        }
    }
}

fn collect_tool_artifacts(task: &TaskNode) -> Vec<String> {
    let mut artifacts = task
        .steps
        .iter()
        .flat_map(|step| step.observation.artifacts.clone())
        .collect::<Vec<_>>();
    artifacts.sort();
    artifacts.dedup();
    artifacts
}

fn repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        }
    }
}

fn git_output(repo: &std::path::Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_output_lines(repo: &std::path::Path, args: &[&str]) -> Vec<String> {
    git_output(repo, args)
        .unwrap_or_default()
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn git_diff_snapshot(repo: &std::path::Path) -> (String, bool) {
    const MAX_DIFF_CHARS: usize = 32_000;
    let diff = git_output(repo, &["diff", "--no-ext-diff", "--", "."]).unwrap_or_default();
    if diff.chars().count() <= MAX_DIFF_CHARS {
        return (diff, false);
    }
    let mut truncated = diff.chars().take(MAX_DIFF_CHARS).collect::<String>();
    truncated.push_str("\n[... git diff truncated in transcript]");
    (truncated, true)
}

fn parse_time(raw: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentd::graph::{ExecutionStep, TaskStatus, TaskType};
    use crate::memd::events::EventStore;
    use crate::toolbridge::executor::{Action, Observation};
    use serde_json::json;

    #[test]
    fn transcript_contains_review_bundle() {
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
                );
                CREATE TABLE task_transcripts (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    session_ids TEXT NOT NULL DEFAULT '[]',
                    task_description TEXT NOT NULL,
                    status TEXT NOT NULL,
                    started_at TEXT,
                    completed_at TEXT,
                    attempt_count INTEGER NOT NULL DEFAULT 0,
                    step_count INTEGER NOT NULL DEFAULT 0,
                    transcript_path TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    recorded_at TEXT NOT NULL
                );",
            )
            .unwrap();
        let events = EventStore::new(Arc::clone(&db));
        let transcript_dir =
            std::env::temp_dir().join(format!("px-transcript-test-{}", Uuid::new_v4()));
        let store = TranscriptStore::new(db, transcript_dir);

        let mut task = TaskNode::new("fix failing test".to_string(), TaskType::UserRequest, 100);
        task.status = TaskStatus::Complete;
        task.started_at = Some(Utc::now());
        task.completed_at = Some(Utc::now());
        task.attempt_count = 1;
        task.steps.push(ExecutionStep {
            index: 1,
            thought: "run tests".to_string(),
            action: Action {
                tool_name: "shell.restricted".to_string(),
                params: json!({"command": "cargo test"}),
                risk_score: 60,
            },
            observation: Observation {
                success: true,
                output: "ok".to_string(),
                error: None,
                tokens_used: 0,
                execution_ms: 12,
                artifacts: vec!["artifacts/commands/test.json".to_string()],
            },
            timestamp: Utc::now(),
        });
        events
            .append(
                None,
                Some(task.id),
                "tool.succeeded",
                "cargo test succeeded",
                json!({"artifacts": ["artifacts/commands/test.json"]}),
            )
            .unwrap();

        let path = store.record_task(&task, "succeeded", "done", &events).unwrap();
        let raw = std::fs::read_to_string(path).unwrap();
        let transcript: serde_json::Value = serde_json::from_str(&raw).unwrap();

        assert_eq!(
            transcript["review"]["tool_artifacts"][0],
            "artifacts/commands/test.json"
        );
        assert!(transcript["review"]["git_status"].is_array());
        assert!(transcript["review"]["changed_files"].is_array());
        assert!(transcript["review"]["git_diff"].is_string());
        assert!(transcript["steps"].as_array().unwrap().len() == 1);

        let recent = store.recent(5).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].task_description, "fix failing test");
        assert_eq!(recent[0].status, "succeeded");
        let by_prefix = store
            .get_by_task_prefix(&task.id.to_string()[..8])
            .unwrap()
            .unwrap();
        assert_eq!(by_prefix.task_id, task.id.to_string());
    }
}
