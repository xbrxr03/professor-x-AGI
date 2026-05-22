use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, warn};

use crate::toolbridge::ToolRegistry;

/// ReAct Action (arXiv:2210.03629)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub tool_name: String,
    pub params: serde_json::Value,
    pub risk_score: u8,
}

/// ReAct Observation (arXiv:2210.03629)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub tokens_used: u32,
    pub execution_ms: u64,
}

impl Observation {
    pub fn denied(reason: &str) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(format!("policy denied: {reason}")),
            tokens_used: 0,
            execution_ms: 0,
        }
    }

    pub fn err(msg: &str) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(msg.to_string()),
            tokens_used: 0,
            execution_ms: 0,
        }
    }
}

pub struct ToolExecutor {
    registry: std::sync::Arc<std::sync::RwLock<ToolRegistry>>,
}

impl ToolExecutor {
    pub fn new(registry: std::sync::Arc<std::sync::RwLock<ToolRegistry>>) -> Self {
        Self { registry }
    }

    /// Execute an action after policyd has approved it.
    /// Returns Observation with timing.
    pub async fn execute(&self, action: &Action) -> Observation {
        let start = Instant::now();

        // Validate params against schema before executing.
        {
            let reg = self.registry.read().unwrap();
            if let Err(e) = reg.validate_params(&action.tool_name, &action.params) {
                return Observation::err(&format!("schema validation failed: {e}"));
            }
        }

        let result = self.dispatch(action).await;
        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => Observation {
                success: true,
                output,
                error: None,
                tokens_used: 0,
                execution_ms: elapsed,
            },
            Err(e) => Observation {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
                tokens_used: 0,
                execution_ms: elapsed,
            },
        }
    }

    async fn dispatch(&self, action: &Action) -> Result<String> {
        match action.tool_name.as_str() {
            "fs.read" => {
                let path = action.params["path"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("fs.read requires 'path'"))?;
                Ok(std::fs::read_to_string(path)?)
            }
            "fs.list" => {
                let path = action.params["path"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("fs.list requires 'path'"))?;
                let entries: Vec<String> = std::fs::read_dir(path)?
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                Ok(entries.join("\n"))
            }
            "fs.write" => {
                let path = action.params["path"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("fs.write requires 'path'"))?;
                let content = action.params["content"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("fs.write requires 'content'"))?;
                if let Some(parent) = std::path::Path::new(path).parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, content)?;
                Ok(format!("wrote {} bytes to {path}", content.len()))
            }
            "shell.restricted" => {
                let cmd = action.params["command"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("shell.restricted requires 'command'"))?;
                debug!("shell.restricted: {cmd}");
                let output = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .output()
                    .await?;
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if !output.status.success() {
                    anyhow::bail!("command failed: {stderr}");
                }
                Ok(if stderr.is_empty() { stdout } else { format!("{stdout}\nstderr: {stderr}") })
            }
            _ => {
                warn!("unimplemented tool: {}", action.tool_name);
                anyhow::bail!("tool '{}' not yet implemented", action.tool_name)
            }
        }
    }
}
