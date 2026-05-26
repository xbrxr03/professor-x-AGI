use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::agentd::graph::{TaskNode, TaskStatus};

#[derive(Debug, Clone)]
pub struct TaskRun {
    pub task_id: String,
    pub description: String,
    pub task_type: String,
    pub status: String,
    pub priority: u8,
    pub attempt_count: u8,
    pub step_count: usize,
    pub last_tool: Option<String>,
    pub last_summary: String,
    pub last_output_preview: Option<String>,
    pub last_error: Option<String>,
    pub last_artifacts: Vec<String>,
    pub verification_summary: String,
    pub verification_artifacts: Vec<String>,
    pub outcome_score: Option<f32>,
    pub failure_mode: Option<String>,
    pub transcript_path: Option<String>,
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct TaskRunStore {
    db: Arc<Mutex<Connection>>,
}

impl TaskRunStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn queued(&self, task: &TaskNode) -> Result<()> {
        let now = Utc::now();
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO task_runs
             (task_id, description, task_type, status, priority, attempt_count, step_count,
              last_summary, queued_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(task_id) DO UPDATE SET
              description = excluded.description,
              task_type = excluded.task_type,
              status = excluded.status,
              priority = excluded.priority,
              attempt_count = excluded.attempt_count,
              step_count = excluded.step_count,
              last_summary = excluded.last_summary,
              updated_at = excluded.updated_at",
            params![
                task.id.to_string(),
                task.description,
                format!("{:?}", task.task_type),
                format!("{:?}", task.status),
                task.priority,
                task.attempt_count,
                task.steps.len() as i64,
                "queued",
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn started(&self, task: &TaskNode) -> Result<()> {
        self.update_task(task, None, "running")
    }

    pub fn attempt_started(&self, task: &TaskNode) -> Result<()> {
        self.update_task(
            task,
            None,
            format!("attempt {} started", task.attempt_count),
        )
    }

    pub fn step_recorded(&self, task: &TaskNode) -> Result<()> {
        let last = task.steps.last();
        let last_tool = last.map(|step| step.action.tool_name.as_str());
        let summary = last
            .map(step_summary)
            .unwrap_or_else(|| "waiting for first step".to_string());
        let output_preview = last.and_then(|step| {
            if step.observation.output.is_empty() {
                None
            } else {
                Some(truncate(&step.observation.output, 360))
            }
        });
        let error = last.and_then(|step| step.observation.error.as_deref());
        let artifacts = last
            .map(|step| step.observation.artifacts.clone())
            .unwrap_or_default();
        self.update_task_detail(
            task,
            last_tool,
            summary,
            output_preview.as_deref(),
            error,
            &artifacts,
        )
    }

    pub fn finished(
        &self,
        task: &TaskNode,
        failure_mode: Option<&str>,
        transcript_path: Option<&Path>,
    ) -> Result<()> {
        let now = Utc::now();
        let (verification_summary, verification_artifacts) =
            verification_for_task(task, transcript_path);
        let verification_artifacts_raw = serde_json::to_string(&verification_artifacts)?;
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE task_runs
             SET status = ?2,
                 attempt_count = ?3,
                 step_count = ?4,
                 last_summary = ?5,
                 outcome_score = ?6,
                 failure_mode = ?7,
                 transcript_path = ?8,
                 verification_summary = ?9,
                 verification_artifacts = ?10,
                 updated_at = ?11,
                 completed_at = ?12
             WHERE task_id = ?1",
            params![
                task.id.to_string(),
                format!("{:?}", task.status),
                task.attempt_count,
                task.steps.len() as i64,
                match task.status {
                    TaskStatus::Complete => "completed",
                    TaskStatus::Failed => "failed",
                    TaskStatus::Cancelled => "cancelled",
                    TaskStatus::Blocked => "blocked",
                    _ => "stopped",
                },
                task.outcome_score,
                failure_mode,
                transcript_path.map(|path| path.display().to_string()),
                verification_summary,
                verification_artifacts_raw,
                now.to_rfc3339(),
                task.completed_at.unwrap_or(now).to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn latest(&self) -> Result<Option<TaskRun>> {
        Ok(self.recent(1)?.into_iter().next())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<TaskRun>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT task_id, description, task_type, status, priority, attempt_count, step_count,
                    last_tool, last_summary, last_output_preview, last_error, last_artifacts,
                    verification_summary, verification_artifacts,
                    outcome_score, failure_mode, transcript_path,
                    queued_at, started_at, updated_at, completed_at
             FROM task_runs
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit.max(1) as i64], parse_run)?;
        let mut runs = Vec::new();
        for row in rows {
            runs.push(row?);
        }
        Ok(runs)
    }

    fn update_task(
        &self,
        task: &TaskNode,
        last_tool: Option<&str>,
        last_summary: impl AsRef<str>,
    ) -> Result<()> {
        let now = Utc::now();
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE task_runs
             SET status = ?2,
                 attempt_count = ?3,
                 step_count = ?4,
                 last_tool = COALESCE(?5, last_tool),
                 last_summary = ?6,
                 started_at = COALESCE(started_at, ?7),
                 updated_at = ?8
             WHERE task_id = ?1",
            params![
                task.id.to_string(),
                format!("{:?}", task.status),
                task.attempt_count,
                task.steps.len() as i64,
                last_tool,
                last_summary.as_ref(),
                task.started_at.unwrap_or(now).to_rfc3339(),
                now.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn update_task_detail(
        &self,
        task: &TaskNode,
        last_tool: Option<&str>,
        last_summary: impl AsRef<str>,
        output_preview: Option<&str>,
        error: Option<&str>,
        artifacts: &[String],
    ) -> Result<()> {
        let now = Utc::now();
        let artifacts_raw = serde_json::to_string(artifacts)?;
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE task_runs
             SET status = ?2,
                 attempt_count = ?3,
                 step_count = ?4,
                 last_tool = COALESCE(?5, last_tool),
                 last_summary = ?6,
                 last_output_preview = ?7,
                 last_error = ?8,
                 last_artifacts = ?9,
                 started_at = COALESCE(started_at, ?10),
                 updated_at = ?11
             WHERE task_id = ?1",
            params![
                task.id.to_string(),
                format!("{:?}", task.status),
                task.attempt_count,
                task.steps.len() as i64,
                last_tool,
                last_summary.as_ref(),
                output_preview,
                error,
                artifacts_raw,
                task.started_at.unwrap_or(now).to_rfc3339(),
                now.to_rfc3339(),
            ],
        )?;
        Ok(())
    }
}

fn parse_run(row: &rusqlite::Row) -> rusqlite::Result<TaskRun> {
    let artifacts_raw: String = row.get(11)?;
    let verification_artifacts_raw: String = row.get(13)?;
    let queued_at_raw: String = row.get(17)?;
    let started_at_raw: Option<String> = row.get(18)?;
    let updated_at_raw: String = row.get(19)?;
    let completed_at_raw: Option<String> = row.get(20)?;
    Ok(TaskRun {
        task_id: row.get(0)?,
        description: row.get(1)?,
        task_type: row.get(2)?,
        status: row.get(3)?,
        priority: row.get::<_, i64>(4)? as u8,
        attempt_count: row.get::<_, i64>(5)? as u8,
        step_count: row.get::<_, i64>(6)? as usize,
        last_tool: row.get(7)?,
        last_summary: row.get(8)?,
        last_output_preview: row.get(9)?,
        last_error: row.get(10)?,
        last_artifacts: serde_json::from_str(&artifacts_raw).unwrap_or_default(),
        verification_summary: row.get(12)?,
        verification_artifacts: serde_json::from_str(&verification_artifacts_raw)
            .unwrap_or_default(),
        outcome_score: row.get(14)?,
        failure_mode: row.get(15)?,
        transcript_path: row.get(16)?,
        queued_at: parse_time(&queued_at_raw),
        started_at: started_at_raw.as_deref().map(parse_time),
        updated_at: parse_time(&updated_at_raw),
        completed_at: completed_at_raw.as_deref().map(parse_time),
    })
}

fn verification_for_task(task: &TaskNode, transcript_path: Option<&Path>) -> (String, Vec<String>) {
    let succeeded = task
        .steps
        .iter()
        .filter(|step| step.observation.success)
        .count();
    let failed = task.steps.len().saturating_sub(succeeded);
    let mut artifacts = task
        .steps
        .iter()
        .flat_map(|step| step.observation.artifacts.iter().cloned())
        .collect::<Vec<_>>();
    if let Some(path) = transcript_path {
        artifacts.push(path.display().to_string());
    }
    artifacts.sort();
    artifacts.dedup();
    let transcript_status = if transcript_path.is_some() {
        "transcript recorded"
    } else {
        "no transcript"
    };
    (
        format!(
            "{} step(s): {} succeeded, {} failed; {} artifact(s); {}",
            task.steps.len(),
            succeeded,
            failed,
            artifacts.len(),
            transcript_status
        ),
        artifacts,
    )
}

fn step_summary(step: &crate::agentd::graph::ExecutionStep) -> String {
    if step.observation.success {
        format!("step {}: {} succeeded", step.index, step.action.tool_name)
    } else {
        format!("step {}: {} failed", step.index, step.action.tool_name)
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}

fn parse_time(raw: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentd::graph::{ExecutionStep, TaskType};
    use crate::toolbridge::executor::{Action, Observation};
    use serde_json::json;

    #[test]
    fn records_latest_task_run_lifecycle() {
        let db = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE task_runs (
                    task_id TEXT PRIMARY KEY,
                    description TEXT NOT NULL,
                    task_type TEXT NOT NULL,
                    status TEXT NOT NULL,
                    priority INTEGER NOT NULL DEFAULT 0,
                    attempt_count INTEGER NOT NULL DEFAULT 0,
                    step_count INTEGER NOT NULL DEFAULT 0,
                    last_tool TEXT,
                    last_summary TEXT NOT NULL DEFAULT '',
                    last_output_preview TEXT,
                    last_error TEXT,
                    last_artifacts TEXT NOT NULL DEFAULT '[]',
                    verification_summary TEXT NOT NULL DEFAULT '',
                    verification_artifacts TEXT NOT NULL DEFAULT '[]',
                    outcome_score REAL,
                    failure_mode TEXT,
                    transcript_path TEXT,
                    queued_at TEXT NOT NULL,
                    started_at TEXT,
                    updated_at TEXT NOT NULL,
                    completed_at TEXT
                );",
            )
            .unwrap();

        let store = TaskRunStore::new(db);
        let mut task = TaskNode::new("inspect workspace".to_string(), TaskType::UserRequest, 100);
        store.queued(&task).unwrap();
        task.status = TaskStatus::Running;
        task.started_at = Some(Utc::now());
        task.attempt_count = 1;
        store.started(&task).unwrap();
        store.attempt_started(&task).unwrap();
        task.steps.push(ExecutionStep {
            index: 1,
            thought: "list files".to_string(),
            action: Action {
                tool_name: "shell.restricted".to_string(),
                params: json!({"cmd": "git status --short"}),
                risk_score: 10,
            },
            observation: Observation {
                success: true,
                output: "clean".to_string(),
                error: None,
                tokens_used: 0,
                execution_ms: 2,
                artifacts: Vec::new(),
            },
            timestamp: Utc::now(),
        });
        store.step_recorded(&task).unwrap();
        task.status = TaskStatus::Complete;
        task.completed_at = Some(Utc::now());
        task.outcome_score = Some(1.0);
        store
            .finished(
                &task,
                None,
                Some(Path::new("artifacts/transcripts/task.json")),
            )
            .unwrap();

        let latest = store.latest().unwrap().unwrap();
        assert_eq!(latest.description, "inspect workspace");
        assert_eq!(latest.status, "Complete");
        assert_eq!(latest.attempt_count, 1);
        assert_eq!(latest.step_count, 1);
        assert_eq!(latest.last_tool.as_deref(), Some("shell.restricted"));
        assert_eq!(latest.last_output_preview.as_deref(), Some("clean"));
        assert!(latest.last_artifacts.is_empty());
        assert!(latest.verification_summary.contains("1 step(s)"));
        assert!(latest.verification_summary.contains("transcript recorded"));
        assert_eq!(
            latest.verification_artifacts,
            vec!["artifacts/transcripts/task.json".to_string()]
        );
        assert_eq!(
            latest.transcript_path.as_deref(),
            Some("artifacts/transcripts/task.json")
        );

        let recent = store.recent(5).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].task_id, latest.task_id);
    }
}
