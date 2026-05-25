/// Tool executor — single dispatch point for all tool calls.
///
/// All tools flow through here after policyd approves them.
/// Circuit breaker lives in the ReAct loop — executor is pure dispatch.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, warn};

use crate::memd::MemoryManager;
use crate::memd::semantic::SemanticEntry;
use crate::ollama::OllamaClient;
use crate::toolbridge::ToolRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub tool_name:  String,
    pub params:     serde_json::Value,
    pub risk_score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub success:       bool,
    pub output:        String,
    pub error:         Option<String>,
    pub tokens_used:   u32,
    pub execution_ms:  u64,
}

impl Observation {
    pub fn denied(reason: &str) -> Self {
        Self { success: false, output: String::new(),
               error: Some(format!("policy denied: {reason}")),
               tokens_used: 0, execution_ms: 0 }
    }
    pub fn err(msg: &str) -> Self {
        Self { success: false, output: String::new(),
               error: Some(msg.to_string()), tokens_used: 0, execution_ms: 0 }
    }
}

pub struct ToolExecutor {
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    memory:   Option<Arc<MemoryManager>>,
    ollama:   Option<Arc<OllamaClient>>,
    workspace_root: PathBuf,
}

impl ToolExecutor {
    pub fn new(registry: Arc<std::sync::RwLock<ToolRegistry>>) -> Self {
        Self { registry, memory: None, ollama: None, workspace_root: default_workspace_root() }
    }
    pub fn with_workspace_root(mut self, workspace_root: PathBuf) -> Self {
        self.workspace_root = workspace_root;
        self
    }
    pub fn with_memory(mut self, memory: Arc<MemoryManager>) -> Self {
        self.memory = Some(memory); self
    }
    pub fn with_ollama(mut self, ollama: Arc<OllamaClient>) -> Self {
        self.ollama = Some(ollama); self
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
        match result {
            Ok((output, tokens)) => Observation {
                success: true, output, error: None,
                tokens_used: tokens, execution_ms: elapsed,
            },
            Err(e) => Observation {
                success: false, output: String::new(),
                error: Some(e.to_string()), tokens_used: 0, execution_ms: elapsed,
            },
        }
    }

    async fn dispatch(&self, action: &Action) -> Result<(String, u32)> {
        match action.tool_name.as_str() {
            "fs.read" => {
                use std::io::Read;
                const MAX_READ: u64 = 8192;
                let path = req_str(&action.params, "path")?;
                let mut file = std::fs::File::open(path)?;
                let mut buf = Vec::with_capacity(MAX_READ as usize + 1);
                let n = file.take(MAX_READ + 1).read_to_end(&mut buf)?;
                let truncated = n > MAX_READ as usize;
                if truncated { buf.truncate(MAX_READ as usize); }
                let text = String::from_utf8_lossy(&buf).into_owned();
                let out = if truncated {
                    format!("{text}\n[... truncated at {MAX_READ} bytes]")
                } else { text };
                Ok((out, 0))
            }
            "fs.list" => {
                let path = req_str(&action.params, "path")?;
                let entries: Vec<String> = std::fs::read_dir(path)?.flatten()
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if e.path().is_dir() { format!("{name}/") } else { name }
                    }).collect();
                Ok((entries.join("\n"), 0))
            }
            "fs.write" => {
                let path    = req_str(&action.params, "path")?;
                let content = req_str(&action.params, "content")?;
                if let Some(p) = std::path::Path::new(path).parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::write(path, content)?;
                Ok((format!("wrote {} bytes to {path}", content.len()), 0))
            }
            "fs.delete" => {
                let path = req_str(&action.params, "path")?;
                let p = std::path::Path::new(path);
                if p.is_dir() { std::fs::remove_dir_all(p)?; }
                else { std::fs::remove_file(p)?; }
                Ok((format!("deleted {path}"), 0))
            }
            "shell.restricted" => {
                let cmd = req_str(&action.params, "command")?;
                debug!("shell.restricted: {cmd}");
                let out = tokio::process::Command::new("sh").arg("-c").arg(cmd)
                    .current_dir(&self.workspace_root)
                    .output().await?;
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if !out.status.success() {
                    anyhow::bail!("exit {}: {stderr}", out.status.code().unwrap_or(-1));
                }
                Ok((if stderr.is_empty() { stdout } else { format!("{stdout}\nstderr: {stderr}") }, 0))
            }
            "web.search" => {
                let query = req_str(&action.params, "query")?;
                let n = action.params["num_results"].as_u64().unwrap_or(5) as usize;
                Ok((web_search(query, n).await?, 0))
            }
            "web.fetch" => {
                let url = req_str(&action.params, "url")?;
                let body = web_fetch(url).await?;
                let out = if body.len() > 16000 {
                    format!("{}\n[... {} bytes truncated]", &body[..16000], body.len()-16000)
                } else { body };
                Ok((out, 0))
            }
            "memory.read" => {
                let mem   = self.memory.as_ref().ok_or_else(|| anyhow::anyhow!("memory unavailable"))?;
                let query = req_str(&action.params, "query")?;
                let layer = action.params["layer"].as_str().unwrap_or("episodic");
                let out = match layer {
                    "episodic" => mem.episodic.search_fts(query, 5)?
                        .iter().map(|e| format!("[{}] {}", e.timestamp.format("%Y-%m-%d"), e.content))
                        .collect::<Vec<_>>().join("\n"),
                    "semantic" => {
                        let words: Vec<String> = query.split_whitespace().map(String::from).collect();
                        mem.semantic.search_keywords(&words, 5)?
                            .iter().map(|e| format!("[q={:.2}] {}", e.quality, e.content))
                            .collect::<Vec<_>>().join("\n")
                    }
                    "procedural" => mem.procedural.list_verified(10)?
                        .iter().map(|e| format!("[{}] {}", e.name, e.description))
                        .collect::<Vec<_>>().join("\n"),
                    _ => anyhow::bail!("unknown layer '{layer}'"),
                };
                let result = if out.is_empty() {
                    format!("no results in {layer} for '{query}'")
                } else {
                    format!("{layer} results for '{query}':\n{out}")
                };
                Ok((result, 0))
            }
            "memory.write" => {
                let mem     = self.memory.as_ref().ok_or_else(|| anyhow::anyhow!("memory unavailable"))?;
                let content = req_str(&action.params, "content")?;
                let source  = action.params["source"].as_str().unwrap_or("agent");
                let entry   = SemanticEntry::new(content.to_string(), source.to_string());
                let id = entry.id;
                mem.semantic.insert(&entry)?;
                Ok((format!("stored in semantic memory (id={id})"), 0))
            }
            "git.commit" => {
                let message = req_str(&action.params, "message")?;
                let add = tokio::process::Command::new("git")
                    .args(["add", "-A"])
                    .current_dir(&self.workspace_root)
                    .output().await?;
                if !add.status.success() {
                    anyhow::bail!("git add: {}", String::from_utf8_lossy(&add.stderr));
                }
                let commit = tokio::process::Command::new("git")
                    .args(["commit", "-m", message])
                    .current_dir(&self.workspace_root)
                    .output().await?;
                if !commit.status.success() {
                    let err = String::from_utf8_lossy(&commit.stderr);
                    if err.contains("nothing to commit") {
                        return Ok(("nothing to commit".to_string(), 0));
                    }
                    anyhow::bail!("git commit: {err}");
                }
                Ok((String::from_utf8_lossy(&commit.stdout).to_string(), 0))
            }
            "ollama.complete" => {
                let ollama = self.ollama.as_ref().ok_or_else(|| anyhow::anyhow!("ollama unavailable"))?;
                let prompt = req_str(&action.params, "prompt")?;
                let resp = ollama.generate(prompt, None, None).await?;
                let (_, answer) = resp.split_thinking();
                Ok((answer, resp.tokens_used()))
            }
            _ => {
                warn!("unimplemented tool: {}", action.tool_name);
                anyhow::bail!("tool '{}' not implemented", action.tool_name)
            }
        }
    }
}

async fn web_search(query: &str, n: usize) -> Result<String> {
    let url = format!("https://html.duckduckgo.com/html/?q={}", url_encode(query));
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; ProfessorX/0.1)")
        .timeout(std::time::Duration::from_secs(15)).build()?;
    let body = client.get(&url).send().await?.text().await?;
    let mut results = Vec::new();
    for chunk in body.split("result__body") {
        if results.len() >= n { break; }
        let text = strip_html(chunk);
        let t = text.trim();
        if t.len() > 30 { results.push(t.chars().take(300).collect::<String>()); }
    }
    if results.is_empty() { return Ok(format!("no results for '{query}'")); }
    Ok(results.iter().enumerate().map(|(i,r)| format!("{}. {r}", i+1)).collect::<Vec<_>>().join("\n\n"))
}

async fn web_fetch(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; ProfessorX/0.1)")
        .timeout(std::time::Duration::from_secs(30)).build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() { anyhow::bail!("HTTP {} for {url}", resp.status()); }
    let ct = resp.headers().get("content-type")
        .and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let body = resp.text().await?;
    Ok(if ct.contains("html") { strip_html(&body) } else { body })
}

fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    for ch in html.chars() {
        match ch { '<' => in_tag=true, '>' => in_tag=false, _ if !in_tag => out.push(ch), _ => {} }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn url_encode(s: &str) -> String {
    s.bytes().flat_map(|b| -> Vec<char> {
        if b.is_ascii_alphanumeric() || matches!(b, b'-'|b'_'|b'.'|b'~') { vec![b as char] }
        else if b == b' ' { vec!['+'] }
        else { format!("%{b:02X}").chars().collect() }
    }).collect()
}

fn req_str<'a>(p: &'a serde_json::Value, key: &str) -> Result<&'a str> {
    p[key].as_str().ok_or_else(|| anyhow::anyhow!("missing param '{key}'"))
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
