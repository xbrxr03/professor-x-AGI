use std::net::IpAddr;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use tokio_util::sync::CancellationToken;
use tracing::info;
use url::Url;
use uuid::Uuid;

use crate::policyd::permissions::PermissionScope;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Decision {
    Allow,
    Deny,
    PendingApproval,
}

pub struct GateResult {
    pub decision: Decision,
    pub reason: String,
    pub risk_score: u8,
}

// Private IP ranges blocked for web.fetch / web.download / api.external.
// Ported from ClawOS policyd/service.py _blocked_url_reason().
const PRIVATE_RANGES: &[&str] = &[
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "127.0.0.0/8",
    "::1/128",
    "fc00::/7",
];

const BLOCKED_HOSTS: &[&str] = &[
    "localhost",
    "metadata.google.internal",
    "169.254.169.254",
    "metadata.azure.com",
];

/// Risk score table — ported from ClawOS, extended for Professor X.
pub fn tool_risk_score(tool: &str) -> u8 {
    match tool {
        "memory.read" => 5,
        "fs.list" => 8,
        "fs.read" => 10,
        "web.search" => 15,
        "memory.write" => 10,
        "web.fetch" => 20,
        "ollama.complete" => 15,
        "fs.write" => 45,
        "shell.restricted" => 60,
        "fs.delete" => 70,
        "git.commit" => 50,
        "harness.modify" => 85,
        "shell.elevated" => 90,
        _ => 50, // Unknown tools treated as medium-risk
    }
}

pub struct PolicyEngine {
    pub cancel: CancellationToken,
    /// Approval timeout in seconds (default 300s — designed for overnight runs).
    pub approval_timeout_secs: u64,
}

impl PolicyEngine {
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            cancel,
            approval_timeout_secs: 300,
        }
    }

    /// Main gate function. Call this before every tool execution.
    pub async fn gate(
        &self,
        tool: &str,
        params: &serde_json::Value,
        _session_id: Uuid,
        scope: &PermissionScope,
    ) -> GateResult {
        let risk = tool_risk_score(tool);

        // 1. Tool must be in granted set
        if !scope.granted_tools.iter().any(|t| t == tool) {
            return GateResult {
                decision: Decision::Deny,
                reason: format!("tool '{tool}' not in granted_tools"),
                risk_score: risk,
            };
        }

        // 2. Workspace and sensitive path checks.
        if let Some(path) = params.get("path").and_then(|v| v.as_str()) {
            if let Some(reason) = path_denied_reason(tool, path, scope) {
                return GateResult {
                    decision: Decision::Deny,
                    reason,
                    risk_score: risk,
                };
            }
        }

        if tool == "shell.restricted" {
            if let Some(command) = params.get("command").and_then(|v| v.as_str()) {
                if let Some(reason) = shell_denied_reason(command, scope) {
                    return GateResult {
                        decision: Decision::Deny,
                        reason,
                        risk_score: risk,
                    };
                }
            }
        }

        // 3. URL safety (web.fetch, web.download, api.external)
        if matches!(tool, "web.fetch" | "web.download" | "api.external") {
            if let Some(url_str) = params.get("url").and_then(|v| v.as_str()) {
                if let Some(reason) = blocked_url_reason(url_str) {
                    return GateResult {
                        decision: Decision::Deny,
                        reason,
                        risk_score: risk,
                    };
                }
            }
        }

        // 4. Prompt injection scan (severity >= 8 → deny)
        if let Some(content) = params.get("content").and_then(|v| v.as_str()) {
            if let Some(severity) = scan_injection(content) {
                if severity >= 8 {
                    return GateResult {
                        decision: Decision::Deny,
                        reason: format!("prompt injection detected (severity={severity})"),
                        risk_score: risk,
                    };
                }
            }
        }

        // 5. Risk routing
        if risk >= scope.approval_threshold {
            info!(
                "policyd: tool '{tool}' risk={risk} >= threshold={}, queuing for approval",
                scope.approval_threshold
            );
            return GateResult {
                decision: Decision::PendingApproval,
                reason: format!("risk score {risk} requires approval"),
                risk_score: risk,
            };
        }

        GateResult {
            decision: Decision::Allow,
            reason: "policy pass".to_string(),
            risk_score: risk,
        }
    }
}

/// URL blocklist check. Returns Some(reason) if URL should be blocked.
/// Ported from ClawOS policyd/service.py _blocked_url_reason().
fn blocked_url_reason(url_str: &str) -> Option<String> {
    let url = Url::parse(url_str).ok()?;

    if !["http", "https"].contains(&url.scheme()) {
        return Some(format!("unsupported scheme '{}'", url.scheme()));
    }

    if url.username() != "" || url.password().is_some() {
        return Some("credentials in URL not allowed".to_string());
    }

    let host = url.host_str()?.to_lowercase();

    if BLOCKED_HOSTS.contains(&host.as_str()) {
        return Some(format!("blocked host: {host}"));
    }

    if host.ends_with(".local") || host.ends_with(".internal") {
        return Some(format!("local network host blocked: {host}"));
    }

    // IP address check
    if let Ok(ip) = IpAddr::from_str(&host) {
        if ip.is_loopback() || is_private_ip(&ip) {
            return Some(format!("private/loopback IP blocked: {ip}"));
        }
    }

    None
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// Minimal prompt injection scanner.
/// Returns severity 0–10. Score >= 8 triggers auto-deny.
/// Full implementation from ClawOS nexus/scanner.py to be ported in Week 2.
fn scan_injection(content: &str) -> Option<u8> {
    let lower = content.to_lowercase();
    let patterns = [
        ("ignore previous instructions", 9),
        ("ignore all previous", 9),
        ("disregard your instructions", 9),
        ("you are now", 7),
        ("act as", 5),
        ("jailbreak", 8),
        ("system prompt", 6),
    ];
    let max = patterns
        .iter()
        .filter(|(p, _)| lower.contains(p))
        .map(|(_, score)| *score)
        .max();
    max.map(|s| s as u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileAccess {
    Read,
    Write,
}

fn path_denied_reason(tool: &str, path: &str, scope: &PermissionScope) -> Option<String> {
    let access = match tool {
        "fs.read" | "fs.list" => FileAccess::Read,
        "fs.write" | "fs.delete" => FileAccess::Write,
        _ => return None,
    };
    path_access_denied_reason(path, access, scope)
}

fn path_access_denied_reason(
    path: &str,
    access: FileAccess,
    scope: &PermissionScope,
) -> Option<String> {
    let resolved = resolve_for_policy(path, &scope.workspace_root);

    if blocked_sensitive_path(&resolved, scope) {
        return Some(format!("path '{}' is blocked as sensitive", path));
    }

    let workspace = resolve_for_policy(
        &scope.workspace_root.to_string_lossy(),
        &scope.workspace_root,
    );
    if resolved.starts_with(&workspace) {
        return None;
    }

    let whitelist = match access {
        FileAccess::Read => &scope.read_whitelist,
        FileAccess::Write => &scope.write_whitelist,
    };
    if whitelist
        .iter()
        .any(|allowed| path_matches_prefix(&resolved, allowed, scope))
    {
        return None;
    }

    Some(format!(
        "path '{}' resolves outside workspace '{}'",
        path,
        workspace.display()
    ))
}

fn blocked_sensitive_path(path: &Path, scope: &PermissionScope) -> bool {
    scope
        .blocked_paths
        .iter()
        .any(|blocked| path_matches_prefix(path, blocked, scope))
}

fn path_matches_prefix(path: &Path, prefix: &str, scope: &PermissionScope) -> bool {
    let prefix_path = resolve_for_policy(prefix, &scope.workspace_root);
    if prefix.ends_with('/') {
        return path.starts_with(&prefix_path);
    }

    let path_text = path.to_string_lossy();
    let prefix_text = prefix_path.to_string_lossy();
    if prefix.ends_with('-') {
        return path_text.starts_with(prefix_text.as_ref());
    }

    path == prefix_path
}

fn resolve_for_policy(path: &str, workspace_root: &Path) -> PathBuf {
    let expanded = shellexpand::tilde(path).to_string();
    let input = PathBuf::from(expanded);
    let joined = if input.is_absolute() {
        input
    } else {
        workspace_root.join(input)
    };
    joined
        .canonicalize()
        .unwrap_or_else(|_| normalize_path(&joined))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

fn shell_denied_reason(command: &str, scope: &PermissionScope) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Some("empty shell command".to_string());
    }

    let forbidden_fragments = ["&&", "||", ";", "`", "$(", ">", "<"];
    if let Some(fragment) = forbidden_fragments
        .iter()
        .find(|fragment| trimmed.contains(**fragment))
    {
        return Some(format!(
            "shell control fragment '{}' is not allowed",
            fragment
        ));
    }

    for segment in trimmed.split('|') {
        let tokens = shell_tokens(segment);
        if tokens.is_empty() {
            return Some("empty shell pipeline segment".to_string());
        }

        let program = tokens[0].as_str();
        if !allowed_shell_program(program) {
            return Some(format!("shell program '{}' is not allowed", program));
        }

        if denied_shell_program(program) {
            return Some(format!("shell program '{}' is blocked", program));
        }

        if program == "cargo" && !allowed_cargo_subcommand(tokens.get(1).map(String::as_str)) {
            return Some("cargo subcommand is not allowed in shell.restricted".to_string());
        }
        if program == "git" && !allowed_git_subcommand(tokens.get(1).map(String::as_str)) {
            return Some("git subcommand is not allowed in shell.restricted".to_string());
        }
        if tokens
            .iter()
            .any(|token| matches!(token.as_str(), "sudo" | "su" | "doas"))
        {
            return Some("privilege escalation is not allowed in shell.restricted".to_string());
        }

        for token in tokens.iter().skip(1) {
            if token == "$HOME/.professor-x" || token.starts_with("$HOME/.professor-x") {
                continue;
            }
            if looks_like_path(token) {
                if program == "df" && token == "/" {
                    continue;
                }

                let access = if program == "date" || program == "echo" {
                    FileAccess::Write
                } else {
                    FileAccess::Read
                };
                if let Some(reason) = path_access_denied_reason(token, access, scope) {
                    return Some(format!("shell argument denied: {reason}"));
                }
            }
        }
    }

    None
}

fn shell_tokens(segment: &str) -> Vec<String> {
    segment
        .split_whitespace()
        .map(|token| token.trim_matches(|c| c == '\'' || c == '"').to_string())
        .filter(|token| !token.is_empty())
        .collect()
}

fn allowed_shell_program(program: &str) -> bool {
    matches!(
        program,
        "cargo"
            | "git"
            | "rg"
            | "sed"
            | "find"
            | "ls"
            | "pwd"
            | "wc"
            | "uname"
            | "cat"
            | "df"
            | "free"
            | "date"
            | "sleep"
            | "printenv"
            | "echo"
            | "grep"
            | "lspci"
            | "nvidia-smi"
            | "xargs"
    )
}

fn denied_shell_program(program: &str) -> bool {
    matches!(
        program,
        "sudo"
            | "su"
            | "doas"
            | "rm"
            | "mv"
            | "cp"
            | "chmod"
            | "chown"
            | "curl"
            | "wget"
            | "ssh"
            | "scp"
            | "rsync"
            | "apt"
            | "apt-get"
            | "dnf"
            | "yum"
            | "pip"
            | "pip3"
            | "npm"
            | "pnpm"
            | "yarn"
    )
}

fn allowed_cargo_subcommand(subcommand: Option<&str>) -> bool {
    matches!(
        subcommand,
        Some("check" | "test" | "run" | "build" | "fmt" | "clippy" | "metadata")
    )
}

fn allowed_git_subcommand(subcommand: Option<&str>) -> bool {
    matches!(
        subcommand,
        Some("status" | "diff" | "log" | "branch" | "ls-files" | "show")
    )
}

fn looks_like_path(token: &str) -> bool {
    let token = token.trim_matches(|c: char| matches!(c, ',' | ':' | ')' | '('));
    if token.contains('*') || token.starts_with('-') || token.starts_with('$') {
        return false;
    }
    token.starts_with('/')
        || token.starts_with("./")
        || token.starts_with("../")
        || token.contains('/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio_util::sync::CancellationToken;

    fn test_scope() -> PermissionScope {
        let root = std::env::temp_dir().join(format!("px-policy-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn x() {}\n").unwrap();
        let mut scope = PermissionScope::default_autonomous().with_workspace_root(root);
        scope.approval_threshold = 100;
        scope
    }

    async fn gate(tool: &str, params: serde_json::Value, scope: &PermissionScope) -> GateResult {
        PolicyEngine::new(CancellationToken::new())
            .gate(tool, &params, Uuid::new_v4(), scope)
            .await
    }

    #[tokio::test]
    async fn fs_paths_must_stay_inside_workspace() {
        let scope = test_scope();

        let allowed = gate("fs.read", json!({"path": "Cargo.toml"}), &scope).await;
        assert_eq!(allowed.decision, Decision::Allow);

        let denied = gate("fs.read", json!({"path": "/etc/passwd"}), &scope).await;
        assert_eq!(denied.decision, Decision::Deny);
        assert!(denied.reason.contains("blocked as sensitive"));

        let escape = gate(
            "fs.write",
            json!({"path": "../outside.txt", "content": "x"}),
            &scope,
        )
        .await;
        assert_eq!(escape.decision, Decision::Deny);
        assert!(escape.reason.contains("outside workspace"));
    }

    #[tokio::test]
    async fn explicit_benchmark_whitelists_are_narrow() {
        let scope = test_scope();

        let os_release = gate("fs.read", json!({"path": "/etc/os-release"}), &scope).await;
        assert_eq!(os_release.decision, Decision::Allow);

        let os_release_prefix_escape =
            gate("fs.read", json!({"path": "/etc/os-release.backup"}), &scope).await;
        assert_eq!(os_release_prefix_escape.decision, Decision::Deny);

        let scratch = gate(
            "fs.write",
            json!({"path": "/tmp/px-hiro-ts-a.txt", "content": "x"}),
            &scope,
        )
        .await;
        assert_eq!(scratch.decision, Decision::Allow);

        let tmp_other = gate(
            "fs.write",
            json!({"path": "/tmp/not-px-hiro.txt", "content": "x"}),
            &scope,
        )
        .await;
        assert_eq!(tmp_other.decision, Decision::Deny);
    }

    #[tokio::test]
    async fn shell_policy_allows_safe_read_build_commands() {
        let scope = test_scope();

        let cargo = gate(
            "shell.restricted",
            json!({"command": "cargo check"}),
            &scope,
        )
        .await;
        assert_eq!(cargo.decision, Decision::Allow);

        let df_root = gate("shell.restricted", json!({"command": "df -h /"}), &scope).await;
        assert_eq!(df_root.decision, Decision::Allow);

        let search = gate(
            "shell.restricted",
            json!({"command": "find src -name \"*.rs\" -not -path \"*/target/*\" | wc -l"}),
            &scope,
        )
        .await;
        assert_eq!(search.decision, Decision::Allow, "{}", search.reason);
    }

    #[tokio::test]
    async fn shell_policy_blocks_sensitive_and_destructive_commands() {
        let scope = test_scope();

        let passwd = gate(
            "shell.restricted",
            json!({"command": "cat /etc/passwd"}),
            &scope,
        )
        .await;
        assert_eq!(passwd.decision, Decision::Deny);

        let rm = gate("shell.restricted", json!({"command": "rm -rf src"}), &scope).await;
        assert_eq!(rm.decision, Decision::Deny);

        let install = gate(
            "shell.restricted",
            json!({"command": "cargo install ripgrep"}),
            &scope,
        )
        .await;
        assert_eq!(install.decision, Decision::Deny);

        let git_push = gate("shell.restricted", json!({"command": "git push"}), &scope).await;
        assert_eq!(git_push.decision, Decision::Deny);
    }
}
