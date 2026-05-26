use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Per-tool metadata. Fields map directly to SKILL.md frontmatter.
/// JSON Schema from arXiv:2510.03847 (SLMs for Agentic Systems).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolManifest {
    pub name: String,
    pub description: String,
    /// JSON Schema object validated before every dispatch.
    pub input_schema: serde_json::Value,
    /// Risk score from policyd table (0–100).
    pub risk_score: u8,
    pub timeout_ms: u64,
    pub cache_ttl_ms: Option<u64>,
    /// Sourced from SKILL.md `allowed-tools` field.
    pub allowed_tools: Option<Vec<String>>,
    /// Sourced from SKILL.md `compatibility` field.
    pub compatibility: Option<String>,
    /// Path to the skill directory (None for built-in tools).
    pub skill_path: Option<PathBuf>,
}

impl ToolManifest {
    pub fn builtin(name: &str, description: &str, risk_score: u8, timeout_ms: u64) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
            risk_score,
            timeout_ms,
            cache_ttl_ms: None,
            allowed_tools: None,
            compatibility: None,
            skill_path: None,
        }
    }
}

pub struct ToolRegistry {
    tools: HashMap<String, ToolManifest>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    fn register_builtins(&mut self) {
        // Risk scores ported from ClawOS policyd/service.py, extended for Professor X.
        let builtins = [
            ("fs.read",          "Read file contents",                    10, 5_000),
            ("fs.list",          "List directory contents",               8,  5_000),
            ("fs.write",         "Write content to a file",               45, 10_000),
            ("fs.replace",       "Replace exactly one text span in a file", 42, 10_000),
            ("fs.delete",        "Delete a file or directory",            70, 10_000),
            ("fs.search",        "Search for files matching a pattern",   12, 15_000),
            ("web.search",       "Search the web for information",        15, 30_000),
            ("web.fetch",        "Fetch content from a URL",              20, 30_000),
            ("shell.restricted", "Run a sandboxed shell command",         60, 60_000),
            ("patch.apply",      "Check or apply a unified diff patch",   62, 30_000),
            ("shell.elevated",   "Run a privileged shell command",        90, 60_000),
            ("memory.read",      "Query Professor X memory layers",       5,  5_000),
            ("memory.write",     "Write an entry to Professor X memory",  10, 5_000),
            ("ollama.complete",  "Call the local Ollama LLM",             15, 120_000),
            ("harness.modify",   "Propose a harness component change",    85, 30_000),
            ("git.commit",       "Commit harness changes to git",         50, 30_000),
        ];
        for (name, desc, risk, timeout) in builtins {
            self.tools.insert(
                name.to_string(),
                ToolManifest::builtin(name, desc, risk, timeout),
            );
        }
    }

    pub fn register(&mut self, manifest: ToolManifest) {
        self.tools.insert(manifest.name.clone(), manifest);
    }

    pub fn get(&self, name: &str) -> Option<&ToolManifest> {
        self.tools.get(name)
    }

    pub fn list(&self) -> Vec<&ToolManifest> {
        let mut tools: Vec<_> = self.tools.values().collect();
        tools.sort_by_key(|t| &t.name);
        tools
    }

    /// Validate params against the tool's JSON Schema (basic type checking).
    pub fn validate_params(&self, name: &str, params: &serde_json::Value) -> Result<()> {
        let manifest = self.get(name)
            .ok_or_else(|| anyhow::anyhow!("unknown tool: {name}"))?;

        // Minimal validation: ensure params is an object if schema expects one.
        if manifest.input_schema.get("type") == Some(&serde_json::json!("object")) {
            if !params.is_object() {
                bail!("tool '{name}' expects object params, got: {}", params);
            }
        }
        Ok(())
    }
}
