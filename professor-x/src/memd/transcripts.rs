use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::agentd::graph::{ExecutionStep, TaskNode};
use crate::memd::events::{AgentEvent, EventStore};

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
    events: Vec<TranscriptEvent>,
    steps: Vec<TranscriptStep>,
    recorded_at: DateTime<Utc>,
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
