//! Model Context Protocol (MCP) client.
//!
//! Lets Professor X connect to external MCP servers (filesystem, GitHub, Slack,
//! Postgres, …) over the stdio transport and expose their tools as first-class
//! entries in the ToolRegistry — so the ReAct loop and the self-authored
//! curriculum can use the entire MCP ecosystem, not just the built-ins.
//!
//! Transport: newline-delimited JSON-RPC 2.0 over a child process's stdin/stdout
//! (the MCP stdio transport). Calls are serialized per server behind a Mutex —
//! a single agent makes one tool call at a time, so this is simple and correct
//! without a background reader task.
//!
//! Config (`.mcp.json` at the repo root, Claude-compatible):
//! ```json
//! { "mcpServers": {
//!     "filesystem": { "command": "npx",
//!       "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"] }
//! }}
//! ```
//! Tools are registered as `mcp.<server>.<tool>` so they never collide with
//! built-ins and the source server is obvious in a transcript.

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::toolbridge::registry::{ToolManifest, ToolRegistry};

const PROTOCOL_VERSION: &str = "2024-11-05";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
const INIT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct McpFile {
    #[serde(rename = "mcpServers", default)]
    mcp_servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct McpServerConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

/// A live stdio JSON-RPC connection to one MCP server.
struct McpServerConn {
    name: String,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpServerConn {
    /// Spawn the server process and perform the initialize handshake.
    async fn spawn(name: &str, cfg: &McpServerConfig) -> Result<Self> {
        let mut cmd = Command::new(&cfg.command);
        cmd.args(&cfg.args)
            .envs(&cfg.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null()) // server logs to stderr; we don't multiplex it
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .with_context(|| format!("spawning MCP server '{name}' ({})", cfg.command))?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = BufReader::new(child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?);
        let mut conn = Self {
            name: name.to_string(),
            child,
            stdin,
            stdout,
            next_id: 0,
        };

        // initialize → initialized
        let init = tokio::time::timeout(
            INIT_TIMEOUT,
            conn.request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "professor-x", "version": env!("CARGO_PKG_VERSION")}
                }),
            ),
        )
        .await
        .map_err(|_| anyhow!("MCP '{name}' initialize timed out"))??;
        let _ = init; // server capabilities ignored for now
        conn.notify("notifications/initialized", json!({})).await?;
        Ok(conn)
    }

    /// Send a JSON-RPC request and read until the matching response arrives,
    /// skipping any interleaved notifications/log messages.
    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let line = serde_json::to_string(&req)?;
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        loop {
            let mut buf = String::new();
            let n = self.stdout.read_line(&mut buf).await?;
            if n == 0 {
                bail!("MCP '{}' closed the connection", self.name);
            }
            let trimmed = buf.trim();
            if trimmed.is_empty() {
                continue;
            }
            let msg: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => continue, // non-JSON server chatter on stdout — skip
            };
            // Only a response carries our id; notifications have none.
            if msg.get("id").and_then(|v| v.as_u64()) != Some(id) {
                continue;
            }
            if let Some(err) = msg.get("error") {
                bail!("MCP '{}' error: {}", self.name, err);
            }
            return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        let req = json!({"jsonrpc": "2.0", "method": method, "params": params});
        let line = serde_json::to_string(&req)?;
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn list_tools(&mut self) -> Result<Vec<McpToolDef>> {
        let result = self.request("tools/list", json!({})).await?;
        let tools = result
            .get("tools")
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::new();
        for t in tools {
            let Some(name) = t.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            out.push(McpToolDef {
                name: name.to_string(),
                description: t
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                input_schema: t
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object", "properties": {}})),
            });
        }
        Ok(out)
    }

    async fn call_tool(&mut self, tool: &str, arguments: Value) -> Result<String> {
        let result = self
            .request("tools/call", json!({"name": tool, "arguments": arguments}))
            .await?;
        // result.content is an array of {type, text}; concatenate text blocks.
        let mut text = String::new();
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            for block in content {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(t);
                }
            }
        }
        if text.is_empty() {
            // Fall back to the raw result for non-text content.
            text = serde_json::to_string(&result).unwrap_or_default();
        }
        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_error {
            bail!("{text}");
        }
        Ok(text)
    }
}

struct McpToolDef {
    name: String,
    description: String,
    input_schema: Value,
}

/// Routes `mcp.<server>.<tool>` calls to the owning server connection.
pub struct McpManager {
    servers: HashMap<String, Mutex<McpServerConn>>,
    /// registered tool name -> (server, original tool name)
    routes: HashMap<String, (String, String)>,
}

impl McpManager {
    pub fn tool_count(&self) -> usize {
        self.routes.len()
    }

    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    /// Dispatch a registered `mcp.*` tool call. Returns the server's text output.
    pub async fn call(&self, registered_name: &str, arguments: &Value) -> Result<String> {
        let (server, tool) = self
            .routes
            .get(registered_name)
            .ok_or_else(|| anyhow!("unknown MCP tool: {registered_name}"))?;
        let conn = self
            .servers
            .get(server)
            .ok_or_else(|| anyhow!("MCP server '{server}' not connected"))?;
        let mut guard = conn.lock().await;
        tokio::time::timeout(REQUEST_TIMEOUT, guard.call_tool(tool, arguments.clone()))
            .await
            .map_err(|_| anyhow!("MCP tool '{registered_name}' timed out"))?
    }
}

static GLOBAL: OnceLock<Arc<McpManager>> = OnceLock::new();

/// The process-wide MCP manager, if servers were configured and connected.
pub fn global() -> Option<Arc<McpManager>> {
    GLOBAL.get().cloned()
}

/// True if a tool name is routed to MCP.
pub fn is_mcp_tool(name: &str) -> bool {
    name.starts_with("mcp.")
}

fn config_path(repo_root: &Path) -> std::path::PathBuf {
    // Prefer the canonical professor-x/.mcp.json, fall back to repo-root.
    let nested = repo_root.join("professor-x/.mcp.json");
    if nested.exists() {
        nested
    } else {
        repo_root.join(".mcp.json")
    }
}

/// Load `.mcp.json`, spawn every configured server, register its tools into the
/// shared registry, and install the process-global manager. Idempotent: a
/// second call is a no-op. Returns (servers_connected, tools_registered).
/// Never fails the whole boot — a broken server is logged and skipped.
pub async fn init_global_mcp(
    repo_root: &Path,
    registry: &Arc<std::sync::RwLock<ToolRegistry>>,
) -> (usize, usize) {
    if GLOBAL.get().is_some() {
        return (0, 0);
    }
    let path = config_path(repo_root);
    if !path.exists() {
        return (0, 0);
    }
    let raw = match std::fs::read_to_string(&path) {
        Ok(r) => r,
        Err(e) => {
            warn!("mcp: cannot read {}: {e}", path.display());
            return (0, 0);
        }
    };
    let file: McpFile = match serde_json::from_str(&raw) {
        Ok(f) => f,
        Err(e) => {
            warn!("mcp: invalid {}: {e}", path.display());
            return (0, 0);
        }
    };
    if file.mcp_servers.is_empty() {
        return (0, 0);
    }

    let mut servers = HashMap::new();
    let mut routes = HashMap::new();
    let mut tools_registered = 0usize;

    for (name, cfg) in &file.mcp_servers {
        match McpServerConn::spawn(name, cfg).await {
            Ok(mut conn) => {
                let tools = match conn.list_tools().await {
                    Ok(t) => t,
                    Err(e) => {
                        warn!("mcp: '{name}' tools/list failed: {e}; skipping server");
                        continue;
                    }
                };
                {
                    let mut reg = registry.write().unwrap();
                    for t in &tools {
                        let registered = format!("mcp.{name}.{}", t.name);
                        let manifest = ToolManifest {
                            name: registered.clone(),
                            description: format!("[MCP:{name}] {}", t.description),
                            input_schema: t.input_schema.clone(),
                            // External tools are untrusted: high risk so the
                            // policy layer gates them like other powerful tools.
                            risk_score: 65,
                            timeout_ms: REQUEST_TIMEOUT.as_millis() as u64,
                            cache_ttl_ms: None,
                            allowed_tools: None,
                            compatibility: None,
                            skill_path: None,
                        };
                        reg.register(manifest);
                        routes.insert(registered, (name.clone(), t.name.clone()));
                        tools_registered += 1;
                    }
                }
                info!("mcp: connected '{name}' ({} tools)", tools.len());
                servers.insert(name.clone(), Mutex::new(conn));
            }
            Err(e) => warn!("mcp: failed to start '{name}': {e}"),
        }
    }

    if servers.is_empty() {
        return (0, 0);
    }
    let server_count = servers.len();
    let manager = Arc::new(McpManager { servers, routes });
    let _ = GLOBAL.set(manager);
    (server_count, tools_registered)
}
