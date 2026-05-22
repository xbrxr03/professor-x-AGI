/// Per-session permission scope.
/// granted_tools wired to SKILL.md `allowed-tools` field at skill load time.
#[derive(Debug, Clone)]
pub struct PermissionScope {
    pub granted_tools: Vec<String>,
    pub blocked_paths: Vec<String>,
    pub allowed_url_schemes: Vec<String>,
    pub blocked_url_patterns: Vec<String>,
    pub max_risk_score: u8,
    /// Risk >= this threshold → queued for human approval (default 50).
    pub approval_threshold: u8,
}

impl Default for PermissionScope {
    fn default() -> Self {
        Self {
            granted_tools: vec![
                "fs.read".to_string(),
                "fs.list".to_string(),
                "fs.search".to_string(),
                "web.search".to_string(),
                "web.fetch".to_string(),
                "memory.read".to_string(),
                "memory.write".to_string(),
                "ollama.complete".to_string(),
            ],
            blocked_paths: vec![
                "~/.jarvis/vault.key".to_string(),
                "~/.jarvis/vault.enc".to_string(),
                "/etc/passwd".to_string(),
                "/etc/shadow".to_string(),
            ],
            allowed_url_schemes: vec!["http".to_string(), "https".to_string()],
            blocked_url_patterns: vec![
                "169.254.169.254".to_string(),       // AWS metadata
                "metadata.google.internal".to_string(),
                "metadata.azure.com".to_string(),
            ],
            max_risk_score: 100,
            approval_threshold: 50,
        }
    }
}
