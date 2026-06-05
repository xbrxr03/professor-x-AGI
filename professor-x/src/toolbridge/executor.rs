/// Tool executor — single dispatch point for all tool calls.
///
/// All tools flow through here after policyd approves them.
/// Circuit breaker lives in the ReAct loop — executor is pure dispatch.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, warn};

use crate::memd::semantic::SemanticEntry;
use crate::memd::MemoryManager;
use crate::ollama::OllamaClient;
use crate::toolbridge::ToolRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub tool_name: String,
    pub params: serde_json::Value,
    pub risk_score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub tokens_used: u32,
    pub execution_ms: u64,
    #[serde(default)]
    pub artifacts: Vec<String>,
}

impl Observation {
    pub fn denied(reason: &str) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(format!("policy denied: {reason}")),
            tokens_used: 0,
            execution_ms: 0,
            artifacts: Vec::new(),
        }
    }
    pub fn err(msg: &str) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(msg.to_string()),
            tokens_used: 0,
            execution_ms: 0,
            artifacts: Vec::new(),
        }
    }
}

struct ToolDispatch {
    output: String,
    tokens_used: u32,
    artifacts: Vec<String>,
}

impl ToolDispatch {
    fn output(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            tokens_used: 0,
            artifacts: Vec::new(),
        }
    }

    fn with_tokens(output: impl Into<String>, tokens_used: u32) -> Self {
        Self {
            output: output.into(),
            tokens_used,
            artifacts: Vec::new(),
        }
    }

    fn with_artifact(output: impl Into<String>, artifact: PathBuf) -> Self {
        Self {
            output: output.into(),
            tokens_used: 0,
            artifacts: vec![artifact.to_string_lossy().to_string()],
        }
    }
}

pub struct ToolExecutor {
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    memory: Option<Arc<MemoryManager>>,
    ollama: Option<Arc<OllamaClient>>,
    workspace_root: PathBuf,
}

#[derive(Debug, Serialize)]
struct CommandOutputArtifact<'a> {
    command: &'a str,
    exit_code: Option<i32>,
    success: bool,
    stdout: &'a str,
    stderr: &'a str,
    stdout_bytes: usize,
    stderr_bytes: usize,
    recorded_at: String,
}

impl ToolExecutor {
    pub fn new(registry: Arc<std::sync::RwLock<ToolRegistry>>) -> Self {
        Self {
            registry,
            memory: None,
            ollama: None,
            workspace_root: default_workspace_root(),
        }
    }
    pub fn with_workspace_root(mut self, workspace_root: PathBuf) -> Self {
        self.workspace_root = workspace_root;
        self
    }
    pub fn with_memory(mut self, memory: Arc<MemoryManager>) -> Self {
        self.memory = Some(memory);
        self
    }
    pub fn with_ollama(mut self, ollama: Arc<OllamaClient>) -> Self {
        self.ollama = Some(ollama);
        self
    }

    pub async fn execute(&self, action: &Action) -> Observation {
        let start = Instant::now();
        {
            let reg = self.registry.read().unwrap();
            if let Err(e) = reg.validate_params(&action.tool_name, &action.params) {
                return Observation::err(&format!("schema validation failed: {e}"));
            }
        }
        let result = self.dispatch(action).await;
        let elapsed = start.elapsed().as_millis() as u64;
        let observation = match result {
            Ok(result) => Observation {
                success: true,
                output: result.output,
                error: None,
                tokens_used: result.tokens_used,
                execution_ms: elapsed,
                artifacts: result.artifacts,
            },
            Err(e) => Observation {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
                tokens_used: 0,
                execution_ms: elapsed,
                artifacts: Vec::new(),
            },
        };

        // Voyager skill quality (arXiv:2305.16291) + EvolveR (arXiv:2510.16079):
        // if `action.tool_name` resolves to a known procedural entry, record
        // the outcome so its running quality score (verification_score) drifts
        // toward the empirical success rate. Failures here only warn — they
        // must not block the agent's main loop.
        self.record_skill_outcome_if_skill(&action.tool_name, observation.success);

        observation
    }

    fn record_skill_outcome_if_skill(&self, tool_name: &str, success: bool) {
        let Some(memory) = self.memory.as_ref() else {
            return;
        };
        // Cheap path: known built-in tool prefixes are never skills. Avoids
        // a DB roundtrip on every tool call.
        if is_known_builtin_tool(tool_name) {
            return;
        }
        match memory.procedural.is_skill(tool_name) {
            Ok(true) => {
                if let Err(e) = memory.procedural.record_outcome(tool_name, success) {
                    warn!("procedural: failed to record outcome for '{tool_name}': {e}");
                }
            }
            Ok(false) => {}
            Err(e) => warn!("procedural: skill lookup for '{tool_name}' failed: {e}"),
        }
    }

    async fn dispatch(&self, action: &Action) -> Result<ToolDispatch> {
        match action.tool_name.as_str() {
            "fs.read" => {
                use std::io::Read;
                const MAX_READ: u64 = 8192;
                let path = req_str(&action.params, "path")?;
                let file = std::fs::File::open(path)?;
                let mut buf = Vec::with_capacity(MAX_READ as usize + 1);
                let n = file.take(MAX_READ + 1).read_to_end(&mut buf)?;
                let truncated = n > MAX_READ as usize;
                if truncated {
                    buf.truncate(MAX_READ as usize);
                }
                let text = String::from_utf8_lossy(&buf).into_owned();
                let out = if truncated {
                    format!("{text}\n[... truncated at {MAX_READ} bytes]")
                } else {
                    text
                };
                Ok(ToolDispatch::output(out))
            }
            "fs.list" => {
                let path = req_str(&action.params, "path")?;
                let entries: Vec<String> = std::fs::read_dir(path)?
                    .flatten()
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if e.path().is_dir() {
                            format!("{name}/")
                        } else {
                            name
                        }
                    })
                    .collect();
                Ok(ToolDispatch::output(entries.join("\n")))
            }
            "fs.write" => {
                let path = req_str(&action.params, "path")?;
                let content = req_str(&action.params, "content")?;
                if let Some(p) = std::path::Path::new(path).parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::write(path, content)?;
                Ok(ToolDispatch::output(format!(
                    "wrote {} bytes to {path}",
                    content.len()
                )))
            }
            "fs.replace" => {
                let path = req_str(&action.params, "path")?;
                let old = req_str(&action.params, "old")?;
                let new = req_str(&action.params, "new")?;
                if old.is_empty() {
                    anyhow::bail!("fs.replace requires non-empty 'old' text");
                }
                let mode = action.params["mode"].as_str().unwrap_or("apply");
                if !matches!(mode, "check" | "apply") {
                    anyhow::bail!("fs.replace mode must be 'check' or 'apply'");
                }
                let path_ref = std::path::Path::new(path);
                let resolved_path = if path_ref.is_absolute() {
                    path_ref.to_path_buf()
                } else {
                    self.workspace_root.join(path_ref)
                };
                let original = std::fs::read_to_string(&resolved_path)?;
                let matches = original.match_indices(old).count();
                if matches != 1 {
                    anyhow::bail!(
                        "fs.replace expected exactly one match for old text, found {matches}"
                    );
                }
                let updated = original.replacen(old, new, 1);
                let diff_artifact = self.write_replace_artifact(path, &original, &updated)?;
                if mode == "apply" {
                    std::fs::write(&resolved_path, updated)?;
                }
                Ok(ToolDispatch::with_artifact(
                    format!(
                        "replace {mode} succeeded for {path}; old_bytes={} new_bytes={}; artifact={}",
                        old.len(),
                        new.len(),
                        diff_artifact.display()
                    ),
                    diff_artifact,
                ))
            }
            "fs.delete" => {
                let path = req_str(&action.params, "path")?;
                let p = std::path::Path::new(path);
                if p.is_dir() {
                    std::fs::remove_dir_all(p)?;
                } else {
                    std::fs::remove_file(p)?;
                }
                Ok(ToolDispatch::output(format!("deleted {path}")))
            }
            "shell.restricted" => {
                let cmd = req_str(&action.params, "command")?;
                debug!("shell.restricted: {cmd}");
                // stdin = /dev/null so commands that read stdin (awk/sort/cat
                // with no file arg) get immediate EOF instead of blocking
                // forever — this hung a 14h baseline run on bare `awk '...'`.
                // Hard 30s timeout so NO command can ever freeze the agent.
                let child = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .current_dir(&self.workspace_root)
                    .stdin(std::process::Stdio::null())
                    .output();
                let out = match tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    child,
                )
                .await
                {
                    Ok(result) => result?,
                    Err(_) => {
                        anyhow::bail!(
                            "shell command timed out after 30s (did it wait on stdin or block?): {}",
                            truncate_text(cmd, 200)
                        );
                    }
                };
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let artifact_path = self.write_command_artifact(
                    cmd,
                    out.status.code(),
                    out.status.success(),
                    &stdout,
                    &stderr,
                )?;
                if !out.status.success() {
                    anyhow::bail!(
                        "exit {}: {}; artifact={}",
                        out.status.code().unwrap_or(-1),
                        truncate_text(&stderr, 4000),
                        artifact_path.display()
                    );
                }
                let preview = if stderr.is_empty() {
                    truncate_text(&stdout, 8000)
                } else {
                    format!(
                        "{}\nstderr: {}",
                        truncate_text(&stdout, 6000),
                        truncate_text(&stderr, 2000)
                    )
                };
                Ok(ToolDispatch::with_artifact(
                    format!("{preview}\n[full output: {}]", artifact_path.display()),
                    artifact_path,
                ))
            }
            "patch.apply" => {
                let patch = req_str(&action.params, "patch")?;
                let mode = action.params["mode"].as_str().unwrap_or("check");
                if !matches!(mode, "check" | "apply") {
                    anyhow::bail!("patch.apply mode must be 'check' or 'apply'");
                }
                let paths = validate_patch_paths(patch)?;
                let artifact_path = self.write_patch_artifact(patch)?;
                let check = tokio::process::Command::new("git")
                    .args(["apply", "--check"])
                    .arg(&artifact_path)
                    .current_dir(&self.workspace_root)
                    .output()
                    .await?;
                if !check.status.success() {
                    anyhow::bail!(
                        "git apply --check failed: {}",
                        String::from_utf8_lossy(&check.stderr)
                    );
                }
                if mode == "apply" {
                    let apply = tokio::process::Command::new("git")
                        .arg("apply")
                        .arg(&artifact_path)
                        .current_dir(&self.workspace_root)
                        .output()
                        .await?;
                    if !apply.status.success() {
                        anyhow::bail!(
                            "git apply failed: {}",
                            String::from_utf8_lossy(&apply.stderr)
                        );
                    }
                }
                Ok(ToolDispatch::with_artifact(
                    format!(
                        "patch {mode} succeeded for {} path(s); artifact={}",
                        paths.len(),
                        artifact_path.display()
                    ),
                    artifact_path,
                ))
            }
            "web.search" => {
                let query = req_str(&action.params, "query")?;
                let n = action.params["num_results"].as_u64().unwrap_or(5) as usize;
                Ok(ToolDispatch::output(web_search(query, n).await?))
            }
            "web.fetch" => {
                let url = req_str(&action.params, "url")?;
                let body = web_fetch(url).await?;
                let out = if body.len() > 16000 {
                    format!(
                        "{}\n[... {} bytes truncated]",
                        &body[..16000],
                        body.len() - 16000
                    )
                } else {
                    body
                };
                Ok(ToolDispatch::output(out))
            }
            "meta.observe" => {
                // Recursive self-perception. The agent reads its OWN recent
                // processing stream and is asked to form a higher-order
                // representation of what it is doing — the strange loop made
                // literal (Hofstadter; Higher-Order Theory; Global Workspace).
                // The event stream is the system's own broadcast; this tool is
                // the spotlight reading it back into the loop.
                let mem = self
                    .memory
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("meta.observe requires memory"))?;
                let store = crate::memd::events::EventStore::new(Arc::clone(&mem.db));
                let recent = store.tail(24).unwrap_or_default();
                let trace: Vec<&crate::memd::events::AgentEvent> = recent
                    .iter()
                    .filter(|e| {
                        matches!(
                            e.event_type.as_str(),
                            "llm.response"
                                | "tool.started"
                                | "tool.succeeded"
                                | "tool.failed"
                                | "react.duplicate_action"
                                | "react.circuit_breaker"
                                | "policy.denied"
                        )
                    })
                    .collect();
                let tail: Vec<&&crate::memd::events::AgentEvent> =
                    trace.iter().rev().take(12).rev().collect();
                if tail.is_empty() {
                    return Ok(ToolDispatch::output(
                        "No processing to observe yet — this is your first action.".to_string(),
                    ));
                }
                // A light computed signal: which tool have you leaned on most?
                let mut counts: std::collections::HashMap<String, u32> =
                    std::collections::HashMap::new();
                for e in &tail {
                    if e.event_type == "tool.started" {
                        let tool = e
                            .summary
                            .split('\'')
                            .nth(1)
                            .unwrap_or("?")
                            .to_string();
                        *counts.entry(tool).or_insert(0) += 1;
                    }
                }
                let mut top: Vec<_> = counts.into_iter().collect();
                top.sort_by(|a, b| b.1.cmp(&a.1));
                let pattern = top
                    .first()
                    .filter(|(_, n)| *n >= 3)
                    .map(|(t, n)| format!("\nYou have called '{t}' {n} times recently — are you making progress or repeating yourself?"))
                    .unwrap_or_default();

                let lines: Vec<String> = tail
                    .iter()
                    .map(|e| format!("  {}: {}", e.event_type, truncate_text(&e.summary, 110)))
                    .collect();
                Ok(ToolDispatch::output(format!(
                    "This is YOUR OWN recent processing. Step back and observe yourself: \
                     what are you actually doing, is it working, are you looping or \
                     stalling, and what should you do differently?\n{}{}",
                    lines.join("\n"),
                    pattern
                )))
            }
            "vision.analyze" => {
                // Multimodal perception — describe or reason about an image file.
                // Routes to the primary model (llama4:scout supports vision natively).
                // Usage: {"path": "/path/to/image.png", "prompt": "what do you see?"}
                // Also accepts {"url": "https://..."} for web images (fetched first).
                let prompt = action.params["prompt"]
                    .as_str()
                    .unwrap_or("Describe this image in detail.");
                let ollama = self
                    .ollama
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("vision.analyze requires ollama client"))?;

                let result = if let Some(path) = action.params["path"].as_str() {
                    // Local file
                    let resp = ollama
                        .vision_generate(prompt, &[path], None)
                        .await?;
                    let (_, answer) = resp.split_thinking();
                    answer
                } else if let Some(url) = action.params["url"].as_str() {
                    // Fetch remote image, write to temp file, then analyze
                    let bytes = reqwest::get(url).await?.bytes().await?;
                    let tmp = std::env::temp_dir()
                        .join(format!("px-vision-{}.bin", uuid::Uuid::new_v4()));
                    std::fs::write(&tmp, &bytes)?;
                    let resp = ollama
                        .vision_generate(prompt, &[tmp.to_str().unwrap_or("")], None)
                        .await?;
                    let _ = std::fs::remove_file(&tmp);
                    let (_, answer) = resp.split_thinking();
                    answer
                } else {
                    anyhow::bail!("vision.analyze requires 'path' or 'url' param");
                };

                Ok(ToolDispatch::output(result))
            }
            "memory.read" => {
                let mem = self
                    .memory
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("memory unavailable"))?;
                let query = req_str(&action.params, "query")?;
                let layer = action.params["layer"].as_str().unwrap_or("episodic");
                let out = match layer {
                    "episodic" => {
                        // Prefer semantic search; fall back to FTS when embedding unavailable
                        let entries = if let Some(ollama) = self.ollama.as_ref() {
                            if let Ok(vec) = ollama.embed(query).await {
                                let emb_store = crate::embeddings::EmbeddingStore::new(
                                    Arc::clone(&mem.db),
                                );
                                mem.episodic
                                    .search_semantic(&emb_store, &vec, 5)
                                    .unwrap_or_else(|_| {
                                        mem.episodic.search_fts(query, 5).unwrap_or_default()
                                    })
                            } else {
                                mem.episodic.search_fts(query, 5).unwrap_or_default()
                            }
                        } else {
                            mem.episodic.search_fts(query, 5).unwrap_or_default()
                        };
                        entries
                            .iter()
                            .map(|e| {
                                format!("[{}] {}", e.timestamp.format("%Y-%m-%d"), e.content)
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                    "semantic" => {
                        let words: Vec<String> =
                            query.split_whitespace().map(String::from).collect();
                        mem.semantic
                            .search_keywords(&words, 5)?
                            .iter()
                            .map(|e| format!("[q={:.2}] {}", e.quality, e.content))
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                    "procedural" => mem
                        .procedural
                        .list_by_quality(0, 10)?
                        .iter()
                        .map(|e| {
                            format!(
                                "[{} q={:.2} uses={}] {}",
                                e.name,
                                e.verification_score,
                                e.times_used,
                                e.description
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => anyhow::bail!("unknown layer '{layer}'"),
                };
                let result = if out.is_empty() {
                    format!("no results in {layer} for '{query}'")
                } else {
                    format!("{layer} results for '{query}':\n{out}")
                };
                Ok(ToolDispatch::output(result))
            }
            "memory.write" => {
                let mem = self
                    .memory
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("memory unavailable"))?;
                let content = req_str(&action.params, "content")?;
                let source = action.params["source"].as_str().unwrap_or("agent");
                let entry = SemanticEntry::new(content.to_string(), source.to_string());
                let id = entry.id;
                mem.semantic.insert(&entry)?;
                Ok(ToolDispatch::output(format!(
                    "stored in semantic memory (id={id})"
                )))
            }
            "git.commit" => {
                let message = req_str(&action.params, "message")?;
                let add = tokio::process::Command::new("git")
                    .args(["add", "-A"])
                    .current_dir(&self.workspace_root)
                    .output()
                    .await?;
                if !add.status.success() {
                    anyhow::bail!("git add: {}", String::from_utf8_lossy(&add.stderr));
                }
                let commit = tokio::process::Command::new("git")
                    .args(["commit", "-m", message])
                    .current_dir(&self.workspace_root)
                    .output()
                    .await?;
                if !commit.status.success() {
                    let err = String::from_utf8_lossy(&commit.stderr);
                    if err.contains("nothing to commit") {
                        return Ok(ToolDispatch::output("nothing to commit"));
                    }
                    anyhow::bail!("git commit: {err}");
                }
                Ok(ToolDispatch::output(
                    String::from_utf8_lossy(&commit.stdout).to_string(),
                ))
            }
            "ollama.complete" => {
                let ollama = self
                    .ollama
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("ollama unavailable"))?;
                let prompt = req_str(&action.params, "prompt")?;
                let resp = ollama.generate(prompt, None, None).await?;
                let (_, answer) = resp.split_thinking();
                Ok(ToolDispatch::with_tokens(answer, resp.tokens_used()))
            }
            _ => {
                // Cerebellum bypass (Voyager arXiv:2305.16291):
                // If this is a known procedural skill, serve it without a
                // full LLM reasoning cycle. High-quality skills (score > 0.85,
                // ≥ 3 uses) get direct shell execution; others get skill body
                // returned as context for the next ReAct step.
                let Some(mem) = self.memory.as_ref() else {
                    warn!("unimplemented tool: {}", action.tool_name);
                    anyhow::bail!("tool '{}' not implemented", action.tool_name);
                };

                let skill = mem.procedural.get_by_name(&action.tool_name)?;
                match skill {
                    None => {
                        warn!("unimplemented tool: {}", action.tool_name);
                        anyhow::bail!("tool '{}' not implemented", action.tool_name);
                    }
                    Some(entry) => {
                        // High-quality skill: direct execution without extra LLM step
                        if entry.verification_score > 0.85 && entry.times_used >= 3 {
                            if let Some(cmd) = extract_skill_command(&entry.skill_body) {
                                debug!(
                                    "cerebellum: directly executing skill '{}' (score={:.2}, uses={}): {}",
                                    entry.name,
                                    entry.verification_score,
                                    entry.times_used,
                                    cmd.chars().take(80).collect::<String>()
                                );
                                let output = tokio::process::Command::new("sh")
                                    .args(["-c", &cmd])
                                    .current_dir(&self.workspace_root)
                                    .output()
                                    .await?;
                                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                let combined = if stderr.is_empty() {
                                    stdout
                                } else {
                                    format!("{stdout}\nstderr: {stderr}")
                                };
                                return Ok(ToolDispatch::output(format!(
                                    "cerebellum: skill '{}' executed directly\n{}",
                                    entry.name,
                                    combined.chars().take(4096).collect::<String>()
                                )));
                            }
                        }
                        // Lower-confidence skill: return body as LLM context
                        Ok(ToolDispatch::output(format!(
                            "Skill '{}' (score={:.2}, uses={}):\n{}\n\n{}",
                            entry.name,
                            entry.verification_score,
                            entry.times_used,
                            entry.description,
                            entry.skill_body.chars().take(2048).collect::<String>()
                        )))
                    }
                }
            }
        }
    }

    fn write_patch_artifact(&self, patch: &str) -> Result<PathBuf> {
        let dir = artifact_root(&self.workspace_root)
            .join("patches")
            .join(chrono::Utc::now().format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.diff", uuid::Uuid::new_v4()));
        let mut file = std::fs::File::create(&path)?;
        writeln!(file, "{patch}")?;
        Ok(path)
    }

    fn write_replace_artifact(&self, path: &str, before: &str, after: &str) -> Result<PathBuf> {
        let dir = artifact_root(&self.workspace_root)
            .join("replacements")
            .join(chrono::Utc::now().format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&dir)?;
        let path_out = dir.join(format!("{}.diff", uuid::Uuid::new_v4()));
        let mut file = std::fs::File::create(&path_out)?;
        writeln!(file, "--- {path}.before")?;
        writeln!(file, "+++ {path}.after")?;
        writeln!(file, "@@ exact replacement preview @@")?;
        writeln!(file, "{}", text_preview_diff(before, after))?;
        Ok(path_out)
    }

    fn write_command_artifact(
        &self,
        command: &str,
        exit_code: Option<i32>,
        success: bool,
        stdout: &str,
        stderr: &str,
    ) -> Result<PathBuf> {
        let dir = artifact_root(&self.workspace_root)
            .join("commands")
            .join(chrono::Utc::now().format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", uuid::Uuid::new_v4()));
        let artifact = CommandOutputArtifact {
            command,
            exit_code,
            success,
            stdout,
            stderr,
            stdout_bytes: stdout.len(),
            stderr_bytes: stderr.len(),
            recorded_at: chrono::Utc::now().to_rfc3339(),
        };
        let mut file = std::fs::File::create(&path)?;
        writeln!(file, "{}", serde_json::to_string_pretty(&artifact)?)?;
        Ok(path)
    }
}

fn artifact_root(workspace_root: &std::path::Path) -> PathBuf {
    let nested = workspace_root.join("professor-x/artifacts");
    if nested.exists() {
        nested
    } else {
        workspace_root.join("artifacts")
    }
}

fn validate_patch_paths(patch: &str) -> Result<Vec<String>> {
    let mut paths = patch_touched_paths(patch);
    if paths.is_empty() {
        anyhow::bail!("patch contains no file paths");
    }
    for path in &paths {
        if path == "/dev/null" {
            continue;
        }
        if path.starts_with('/') || path.contains('\0') {
            anyhow::bail!("patch path '{path}' is not relative");
        }
        if path.split('/').any(|part| part == ".." || part == ".git") {
            anyhow::bail!("patch path '{path}' contains blocked component");
        }
    }
    paths.retain(|path| path != "/dev/null");
    Ok(paths)
}

fn patch_touched_paths(patch: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let mut parts = rest.split_whitespace();
            for raw in [parts.next(), parts.next()].into_iter().flatten() {
                if let Some(path) = raw.strip_prefix("a/").or_else(|| raw.strip_prefix("b/")) {
                    paths.push(path.to_string());
                }
            }
        } else if let Some(raw) = line.strip_prefix("+++ ") {
            if let Some(path) = clean_patch_header_path(raw) {
                paths.push(path);
            }
        } else if let Some(raw) = line.strip_prefix("--- ") {
            if let Some(path) = clean_patch_header_path(raw) {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn clean_patch_header_path(raw: &str) -> Option<String> {
    let path = raw.split_whitespace().next()?;
    if path == "/dev/null" {
        return Some(path.to_string());
    }
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .map(ToString::to_string)
}

async fn web_search(query: &str, n: usize) -> Result<String> {
    // Try the lite endpoint first (simpler HTML, more scrape-friendly), then
    // the html endpoint. Short 8s timeout so a stall doesn't block the agent.
    // CRUCIAL: on total failure we return a usable MESSAGE (Ok), not an error —
    // a hard error makes the agent retry-loop; a clear "search unavailable,
    // proceed without it" observation makes it adapt and move on.
    let endpoints = [
        format!("https://lite.duckduckgo.com/lite/?q={}", url_encode(query)),
        format!("https://html.duckduckgo.com/html/?q={}", url_encode(query)),
    ];
    for url in &endpoints {
        match try_web_search(url, n).await {
            Ok(Some(text)) => return Ok(text),
            Ok(None) => continue,        // reachable but empty → try fallback
            Err(_) => break,             // network/timeout → no point retrying same network
        }
    }
    Ok(format!(
        "web search is currently unavailable or returned no results for '{query}'. \
         Do NOT repeat this search. Proceed using your existing knowledge, or take \
         a different action toward the task."
    ))
}

/// Single search attempt against one endpoint. Returns Ok(Some(results)) on
/// hits, Ok(None) when reachable but empty, Err on network/timeout.
async fn try_web_search(url: &str, n: usize) -> Result<Option<String>> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:121.0) Gecko/20100101 Firefox/121.0")
        .timeout(std::time::Duration::from_secs(8))
        .build()?;
    let body = client.get(url).send().await?.text().await?;
    let mut results = Vec::new();
    // Both DDG variants delimit results with one of these markers.
    for marker in ["result__body", "result-snippet", "result-link"] {
        if !body.contains(marker) {
            continue;
        }
        for chunk in body.split(marker).skip(1) {
            if results.len() >= n {
                break;
            }
            let text = strip_html(chunk);
            let t = text.trim();
            if t.len() > 30 {
                results.push(t.chars().take(300).collect::<String>());
            }
        }
        if !results.is_empty() {
            break;
        }
    }
    if results.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        results
            .iter()
            .enumerate()
            .map(|(i, r)| format!("{}. {r}", i + 1))
            .collect::<Vec<_>>()
            .join("\n\n"),
    ))
}

async fn web_fetch(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:121.0) Gecko/20100101 Firefox/121.0")
        .timeout(std::time::Duration::from_secs(12))
        .build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} for {url}", resp.status());
    }
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = resp.text().await?;
    Ok(if ct.contains("html") {
        strip_html(&body)
    } else {
        body
    })
}

fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .flat_map(|b| -> Vec<char> {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                vec![b as char]
            } else if b == b' ' {
                vec!['+']
            } else {
                format!("%{b:02X}").chars().collect()
            }
        })
        .collect()
}

fn req_str<'a>(p: &'a serde_json::Value, key: &str) -> Result<&'a str> {
    p[key]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing param '{key}'"))
}

/// Extract the primary shell command from a skill body for cerebellum bypass.
/// Looks for the first non-comment line inside a ```bash/```sh block,
/// or the first line prefixed with `$ `.
fn extract_skill_command(skill_body: &str) -> Option<String> {
    let mut in_bash = false;
    for line in skill_body.lines() {
        let trimmed = line.trim();
        if trimmed == "```bash" || trimmed == "```sh" || trimmed == "```shell" {
            in_bash = true;
            continue;
        }
        if trimmed == "```" && in_bash {
            in_bash = false;
            continue;
        }
        if in_bash && !trimmed.is_empty() && !trimmed.starts_with('#') {
            return Some(trimmed.to_string());
        }
        if let Some(cmd) = trimmed.strip_prefix("$ ") {
            if !cmd.is_empty() {
                return Some(cmd.to_string());
            }
        }
    }
    None
}

/// Built-in tool prefixes the executor dispatches itself. Any name not in this
/// allow-list might be a SKILL.md-loaded procedural entry; the executor
/// consults `procedural.is_skill` for those. Centralising the list keeps the
/// skill-outcome hook from doing a DB query on every `fs.read` / `shell.*`
/// call.
fn is_known_builtin_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "fs.read"
            | "fs.list"
            | "fs.write"
            | "shell.restricted"
            | "scratchpad.write"
            | "plan.write"
            | "meta.observe"
            | "vision.analyze"
            | "memory.read"
            | "memory.write"
            | "web.fetch"
            | "web.search"
            | "patch.apply"
            | "git.diff"
            | "git.status"
            | "git.log"
            | "ollama.generate"
    )
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    out.push_str("\n[... truncated; full output saved as artifact]");
    out
}

fn text_preview_diff(before: &str, after: &str) -> String {
    let before = truncate_text(before, 2000);
    let after = truncate_text(after, 2000);
    format!("- before:\n{before}\n+ after:\n{after}")
}

fn default_workspace_root() -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_workspace() -> PathBuf {
        let root = std::env::temp_dir().join(format!("px-patch-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn x() {}\n").unwrap();
        let init = std::process::Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
        root
    }

    fn patch_action(mode: &str) -> Action {
        Action {
            tool_name: "patch.apply".to_string(),
            params: json!({
                "mode": mode,
                "patch": "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-pub fn x() {}\n+pub fn x() { }\n",
            }),
            risk_score: 62,
        }
    }

    fn shell_action(command: &str) -> Action {
        Action {
            tool_name: "shell.restricted".to_string(),
            params: json!({"command": command}),
            risk_score: 60,
        }
    }

    #[test]
    fn known_builtin_tools_skip_procedural_lookup() {
        for name in [
            "fs.read",
            "fs.write",
            "shell.restricted",
            "memory.read",
            "memory.write",
            "web.fetch",
            "patch.apply",
        ] {
            assert!(
                is_known_builtin_tool(name),
                "{name} should be a known builtin"
            );
        }
    }

    #[test]
    fn skill_named_tools_consult_procedural_lookup() {
        for name in [
            "px-experiment-runner",
            "px-literature-search",
            "RetryPlanGeneration",
        ] {
            assert!(
                !is_known_builtin_tool(name),
                "{name} should not be misclassified as a builtin"
            );
        }
    }

    fn replace_action(mode: &str, old: &str, new: &str) -> Action {
        Action {
            tool_name: "fs.replace".to_string(),
            params: json!({
                "path": "src/lib.rs",
                "old": old,
                "new": new,
                "mode": mode,
            }),
            risk_score: 42,
        }
    }

    #[tokio::test]
    async fn patch_apply_checks_and_applies_reviewable_diff() {
        let root = temp_workspace();
        let registry = Arc::new(std::sync::RwLock::new(ToolRegistry::new()));
        let executor = ToolExecutor::new(registry).with_workspace_root(root.clone());

        let check = executor.execute(&patch_action("check")).await;
        assert!(check.success, "{:?}", check.error);
        assert!(check.output.contains("patch check succeeded"));
        assert_eq!(check.artifacts.len(), 1);
        assert_eq!(
            std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
            "pub fn x() {}\n"
        );

        let apply = executor.execute(&patch_action("apply")).await;
        assert!(apply.success, "{:?}", apply.error);
        assert!(apply.output.contains("patch apply succeeded"));
        assert_eq!(apply.artifacts.len(), 1);
        assert_eq!(
            std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
            "pub fn x() { }\n"
        );
        assert!(root.join("artifacts/patches").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn shell_restricted_writes_full_output_artifact() {
        let root = temp_workspace();
        let registry = Arc::new(std::sync::RwLock::new(ToolRegistry::new()));
        let executor = ToolExecutor::new(registry).with_workspace_root(root.clone());

        let obs = executor
            .execute(&shell_action("printf 'hello professor x'"))
            .await;
        assert!(obs.success, "{:?}", obs.error);
        assert!(obs.output.contains("hello professor x"));
        assert!(obs.output.contains("[full output:"));
        assert_eq!(obs.artifacts.len(), 1);

        let artifacts = root.join("artifacts/commands");
        assert!(artifacts.exists());
        let files = collect_json_files(&artifacts);
        assert_eq!(files.len(), 1);
        assert_eq!(obs.artifacts[0], files[0].to_string_lossy());
        let artifact: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&files[0]).unwrap()).unwrap();
        assert_eq!(artifact["command"], "printf 'hello professor x'");
        assert_eq!(artifact["success"], true);
        assert_eq!(artifact["stdout"], "hello professor x");

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn fs_replace_requires_exactly_one_match_and_writes_artifact() {
        let root = temp_workspace();
        let registry = Arc::new(std::sync::RwLock::new(ToolRegistry::new()));
        let executor = ToolExecutor::new(registry).with_workspace_root(root.clone());

        let check = executor
            .execute(&replace_action("check", "pub fn x() {}", "pub fn x() { }"))
            .await;
        assert!(check.success, "{:?}", check.error);
        assert!(check.output.contains("replace check succeeded"));
        assert_eq!(
            std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
            "pub fn x() {}\n"
        );

        let apply = executor
            .execute(&replace_action("apply", "pub fn x() {}", "pub fn x() { }"))
            .await;
        assert!(apply.success, "{:?}", apply.error);
        assert!(apply.output.contains("replace apply succeeded"));
        assert_eq!(apply.artifacts.len(), 1);
        assert_eq!(
            std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
            "pub fn x() { }\n"
        );

        std::fs::write(root.join("src/lib.rs"), "same\nsame\n").unwrap();
        let ambiguous = executor
            .execute(&replace_action("apply", "same", "changed"))
            .await;
        assert!(!ambiguous.success);
        assert!(ambiguous
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("expected exactly one match"));

        let _ = std::fs::remove_dir_all(root);
    }

    fn collect_json_files(root: &std::path::Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(root).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_json_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "json") {
                files.push(path);
            }
        }
        files
    }
}
