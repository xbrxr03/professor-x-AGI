use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

use crate::agentd::graph::{TaskNode, TaskType};

#[derive(Clone)]
pub struct ArtifactValidator {
    report_dir: PathBuf,
}

impl ArtifactValidator {
    pub fn new(report_dir: PathBuf) -> Self {
        Self { report_dir }
    }

    pub fn validate_task(&self, task: &TaskNode) -> Result<Option<ArtifactValidationReport>> {
        if task.task_type != TaskType::Scheduled {
            return Ok(None);
        }

        let mut checks = Vec::new();
        let mut artifacts = Vec::new();

        let nested_ok = !std::path::Path::new("professor-x").exists();
        checks.push(ArtifactCheck {
            name: "no_nested_professor_x_dir".to_string(),
            passed: nested_ok,
            detail: if nested_ok {
                "no nested professor-x/ directory found inside crate".to_string()
            } else {
                "nested professor-x/ directory found inside crate".to_string()
            },
        });

        if let Some(job_id) = scheduled_job_id(&task.description) {
            if job_id.contains("daily-update") {
                let today = Local::now().format("%Y-%m-%d").to_string();
                let expected = PathBuf::from("ops/daily").join(format!("{today}.md"));
                let exists = expected.exists();
                if exists {
                    artifacts.push(expected.to_string_lossy().to_string());
                }
                checks.push(ArtifactCheck {
                    name: "daily_note_written_for_today".to_string(),
                    passed: exists,
                    detail: format!("expected {}", expected.display()),
                });
            }
        }

        let passed = checks.iter().all(|check| check.passed);
        let report = ArtifactValidationReport {
            id: Uuid::new_v4(),
            task_id: task.id,
            task_description: task.description.clone(),
            passed,
            checks,
            artifacts,
            report_path: None,
            recorded_at: Utc::now(),
        };
        Ok(Some(report))
    }

    pub fn write_report(&self, report: &mut ArtifactValidationReport) -> Result<PathBuf> {
        let dir = self.report_dir.join(Utc::now().format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", report.task_id));
        report.report_path = Some(path.to_string_lossy().to_string());
        let json = serde_json::to_string_pretty(report)?;
        let mut file = std::fs::File::create(&path)?;
        writeln!(file, "{json}")?;
        Ok(path)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactValidationReport {
    pub id: Uuid,
    pub task_id: Uuid,
    pub task_description: String,
    pub passed: bool,
    pub checks: Vec<ArtifactCheck>,
    pub artifacts: Vec<String>,
    pub report_path: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

impl ArtifactValidationReport {
    pub fn failure_reason(&self) -> Option<String> {
        if self.passed {
            return None;
        }
        Some(
            self.checks
                .iter()
                .filter(|check| !check.passed)
                .map(|check| format!("{}: {}", check.name, check.detail))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

fn scheduled_job_id(description: &str) -> Option<String> {
    let marker = "scheduled daily job '";
    let start = description.find(marker)? + marker.len();
    let rest = &description[start..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_scheduled_tasks_do_not_need_artifact_validation() {
        let validator = ArtifactValidator::new(PathBuf::from("artifacts/validation"));
        let task = TaskNode::new("hello".to_string(), TaskType::UserRequest, 100);
        assert!(validator.validate_task(&task).unwrap().is_none());
    }
}
