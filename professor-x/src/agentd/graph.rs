/// Task DAG node. Execution trace follows ReAct format (arXiv:2210.03629).
/// Reflexion buffer from arXiv:2303.11366 (max 3 reflections, oldest evicted).
/// Max attempts default 4 from Voyager's 4-round timeout pattern (arXiv:2305.16291).
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use crate::toolbridge::executor::{Action, Observation};

/// ReAct Thought/Action/Observation triple (arXiv:2210.03629, Algorithm 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// 1-indexed within the task.
    pub index: u32,
    pub thought: String,
    pub action: Action,
    pub observation: Observation,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskType {
    Research,
    Skill,
    Evolution,
    Scheduled,
    UserRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Complete,
    Failed,
    Blocked,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: Uuid,
    pub description: String,
    pub task_type: TaskType,
    pub status: TaskStatus,
    /// 0–255 priority.
    pub priority: u8,
    /// Wait for all parent tasks to complete before running.
    pub parent_ids: Vec<Uuid>,
    pub child_ids: Vec<Uuid>,
    pub steps: Vec<ExecutionStep>,
    /// Reflexion verbal RL buffer — max 3, oldest evicted (arXiv:2303.11366).
    pub reflections: VecDeque<String>,
    pub attempt_count: u8,
    /// Default 4 (Voyager pattern).
    pub max_attempts: u8,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    /// 0.0–1.0, set by evolved on completion.
    pub outcome_score: Option<f32>,
    /// Phase B truth gate: if Some, the artifact validator must find an
    /// artifact of this kind for the task. Stored as a string to avoid a
    /// module cycle with `artifacts::ArtifactKind`; parsed lazily by the
    /// validator. Unrecognized values fail the task with a clear error.
    #[serde(default)]
    pub expected_artifact_kind: Option<String>,
}

impl TaskNode {
    pub fn new(description: String, task_type: TaskType, priority: u8) -> Self {
        Self {
            id: Uuid::new_v4(),
            description,
            task_type,
            status: TaskStatus::Pending,
            priority,
            parent_ids: Vec::new(),
            child_ids: Vec::new(),
            steps: Vec::new(),
            reflections: VecDeque::new(),
            attempt_count: 0,
            max_attempts: 4,
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            outcome_score: None,
            expected_artifact_kind: None,
        }
    }

    /// Declare what artifact kind this task should produce. Validated by
    /// `ArtifactValidator::validate_task` after the React loop returns.
    pub fn with_expected_artifact_kind(mut self, kind: impl Into<String>) -> Self {
        self.expected_artifact_kind = Some(kind.into());
        self
    }

    pub fn push_reflection(&mut self, reflection: String) {
        if self.reflections.len() >= 3 {
            self.reflections.pop_front();
        }
        self.reflections.push_back(reflection);
    }

    /// Format reflection buffer for context injection.
    pub fn reflections_text(&self) -> Option<String> {
        if self.reflections.is_empty() {
            return None;
        }
        Some(
            self.reflections
                .iter()
                .enumerate()
                .map(|(i, r)| format!("Reflection {}: {r}", i + 1))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    /// Format execution steps as ReAct trace for context injection.
    /// Full thought/action/observation text for the most recent `n` steps.
    /// Unlike the Mermaid canvas overview, this INCLUDES the observation output
    /// so the agent can actually act on what its tools returned (prevents the
    /// loop where it re-runs a tool because it can't see the prior result).
    pub fn recent_steps_text(&self, n: usize) -> String {
        let start = self.steps.len().saturating_sub(n);
        self.steps[start..]
            .iter()
            .map(|s| {
                format!(
                    "Thought {}: {}\nAction {}: {}({})\nObservation {}: {}",
                    s.index,
                    s.thought,
                    s.index,
                    s.action.tool_name,
                    serde_json::to_string(&s.action.params).unwrap_or_default(),
                    s.index,
                    if s.observation.success {
                        s.observation.output.chars().take(800).collect::<String>()
                    } else {
                        format!(
                            "ERROR: {}",
                            s.observation.error.as_deref().unwrap_or("unknown")
                        )
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn steps_text(&self) -> String {
        self.steps
            .iter()
            .map(|s| {
                format!(
                    "Thought {}: {}\nAction {}: {}({})\nObservation {}: {}",
                    s.index,
                    s.thought,
                    s.index,
                    s.action.tool_name,
                    serde_json::to_string(&s.action.params).unwrap_or_default(),
                    s.index,
                    if s.observation.success {
                        s.observation.output.chars().take(500).collect::<String>()
                    } else {
                        format!(
                            "ERROR: {}",
                            s.observation.error.as_deref().unwrap_or("unknown")
                        )
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
