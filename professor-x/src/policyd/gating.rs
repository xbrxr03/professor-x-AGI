use std::net::IpAddr;
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

/// Risk score table — ported from ClawOS, extended for JARVIS.
pub fn tool_risk_score(tool: &str) -> u8 {
    match tool {
        "memory.read"      => 5,
        "fs.list"          => 8,
        "fs.read"          => 10,
        "web.search"       => 15,
        "memory.write"     => 10,
        "web.fetch"        => 20,
        "ollama.complete"  => 15,
        "fs.write"         => 45,
        "shell.restricted" => 60,
        "fs.delete"        => 70,
        "git.commit"       => 50,
        "harness.modify"   => 85,
        "shell.elevated"   => 90,
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

        // 2. Blocked path check
        if let Some(path) = params.get("path").and_then(|v| v.as_str()) {
            let expanded = shellexpand::tilde(path).to_string();
            for blocked in &scope.blocked_paths {
                let blocked_exp = shellexpand::tilde(blocked).to_string();
                if expanded.starts_with(&blocked_exp) {
                    return GateResult {
                        decision: Decision::Deny,
                        reason: format!("path '{path}' is in blocked_paths"),
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
            info!("policyd: tool '{tool}' risk={risk} >= threshold={}, queuing for approval",
                  scope.approval_threshold);
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
        IpAddr::V4(v4) => {
            v4.is_private() || v4.is_loopback() || v4.is_link_local()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
        }
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
    let max = patterns.iter()
        .filter(|(p, _)| lower.contains(p))
        .map(|(_, score)| *score)
        .max();
    max.map(|s| s as u8)
}
