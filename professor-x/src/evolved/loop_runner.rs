/// Researcher/Engineer/Analyzer loop — closed-loop self-improvement.
///
/// Source: ASI-Evolve (arXiv:2603.29640), Figure 2.
/// Each evolution cycle:
///   1. Researcher: analyze failure patterns, select a node via UCB1, propose a change
///   2. Engineer:   apply the change to the harness (evolvable components only)
///   3. Analyzer:   run HIRO task subset, record improvement, write cognition item
///
/// Evolution safety:
/// - Core modules (policyd gate, memd internals) require human approval (risk >= 85)
/// - All changes are version-controlled (git commit per cycle)
/// - ChangeManifest must be filled before applying any change
use anyhow::Result;
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

use crate::evolved::analyzer::Analyzer;
use crate::evolved::cognition_base::CognitionStore;
use crate::evolved::proposer::{
    ChangeManifest, EvolutionNode, HarnessComponent, NodeDatabase, VerificationStatus,
};
use crate::evolved::tracker::OutcomeTracker;
use crate::memd::events::EventStore;
use crate::memd::MemoryManager;
use crate::ollama::{ChatMessage, ModelOptions, OllamaClient};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationOutcome {
    pub accepted: bool,
    pub reason: String,
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SandboxVerification {
    pub outcome: VerificationOutcome,
    pub diff: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RewardHackingAnalysis {
    pub suspicious: bool,
    pub confidence: f32,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct EvolutionArtifact {
    generated_at: String,
    artifact_id: String,
    node_id: Option<i64>,
    status: String,
    target_component: String,
    motivation: String,
    manifest: ChangeManifest,
    verification: Option<VerificationOutcome>,
    analysis: String,
    diff_hash: Option<String>,
    diff_bytes: usize,
}

// Parse "[DHE:layer=X,lever=Y]" from failure pattern strings.
// Returns (layer, lever) from the most common DHE annotation found, or (0, 3) as default.
fn parse_dhe_from_patterns(patterns: &[String]) -> (u8, u8) {
    for p in patterns {
        if let Some(start) = p.find("[DHE:layer=") {
            let rest = &p[start + 11..];
            if let Some(comma) = rest.find(',') {
                let layer_str = &rest[..comma];
                let lever_str = rest
                    .get(comma + 7..)
                    .unwrap_or("3")
                    .split(']')
                    .next()
                    .unwrap_or("3");
                let layer = layer_str.parse::<u8>().unwrap_or(0);
                let lever = lever_str.parse::<u8>().unwrap_or(3);
                return (layer, lever);
            }
        }
    }
    (0, 3)
}

pub async fn verify_node_in_sandbox(
    repo_root: &Path,
    node: &EvolutionNode,
) -> Result<SandboxVerification> {
    let reward_scan = analyze_reward_hacking_text(&node.diff);
    if reward_scan.suspicious {
        return Ok(SandboxVerification {
            outcome: VerificationOutcome {
                accepted: false,
                reason: format!(
                    "reward-hacking scan rejected proposal: {} (confidence={:.2})",
                    reward_scan.reason, reward_scan.confidence
                ),
                checks: vec!["reward_hacking_scan".to_string()],
            },
            diff: String::new(),
        });
    }

    let worktree = std::env::temp_dir().join(format!("px-evolve-{}", uuid::Uuid::new_v4()));
    let add = tokio::process::Command::new("git")
        .args(["worktree", "add", "--detach"])
        .arg(&worktree)
        .arg("HEAD")
        .current_dir(repo_root)
        .output()
        .await?;
    if !add.status.success() {
        anyhow::bail!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&add.stderr)
        );
    }

    let result = verify_node_inside_worktree(&worktree, node).await;
    let cleanup = cleanup_worktree(repo_root, &worktree).await;
    if let Err(e) = cleanup {
        warn!(
            "evolved: failed to clean sandbox worktree {}: {e}",
            worktree.display()
        );
    }
    result
}

async fn verify_node_inside_worktree(
    worktree: &Path,
    node: &EvolutionNode,
) -> Result<SandboxVerification> {
    let mut checks = vec![
        "reward_hacking_scan".to_string(),
        "sandbox_worktree".to_string(),
    ];

    if !apply_node_change_at(worktree, node)? {
        return Ok(SandboxVerification {
            outcome: VerificationOutcome {
                accepted: false,
                reason: format!(
                    "component {:?} is not autonomously mutable yet",
                    node.target_component
                ),
                checks,
            },
            diff: String::new(),
        });
    }

    let paths = changed_paths_for_node_at(worktree, node);
    if paths.is_empty() {
        return Ok(SandboxVerification {
            outcome: VerificationOutcome {
                accepted: false,
                reason: "proposal has no known changed paths".to_string(),
                checks,
            },
            diff: String::new(),
        });
    }

    mark_intent_to_add(worktree, &paths).await?;
    checks.push("material_diff".to_string());
    if !has_material_diff_at(worktree, &paths).await? {
        return Ok(SandboxVerification {
            outcome: VerificationOutcome {
                accepted: false,
                reason: "verification rejected proposal: no material file diff".to_string(),
                checks,
            },
            diff: String::new(),
        });
    }

    checks.push("cargo_check".to_string());
    let compile = run_compile_check_at(worktree).await?;
    if !compile.accepted {
        return Ok(SandboxVerification {
            outcome: VerificationOutcome {
                accepted: false,
                reason: compile.reason,
                checks,
            },
            diff: String::new(),
        });
    }

    let diff = collect_diff_at(worktree, &paths).await?;
    Ok(SandboxVerification {
        outcome: VerificationOutcome {
            accepted: true,
            reason: "sandbox verification passed".to_string(),
            checks,
        },
        diff,
    })
}

fn apply_node_change_at(root: &Path, node: &EvolutionNode) -> Result<bool> {
    match &node.target_component {
        HarnessComponent::SystemPrompt => {
            let path = component_relative_path(root, node)
                .unwrap_or_else(|| PathBuf::from("personas/professor_x.md"));
            write_workspace_file(root, &path, &sanitize_generated_content(&node.diff))?;
            Ok(true)
        }
        HarnessComponent::HarnessConfig => {
            let path = component_relative_path(root, node)
                .unwrap_or_else(|| PathBuf::from("config/hardware.toml"));
            write_workspace_file(root, &path, &sanitize_generated_content(&node.diff))?;
            Ok(true)
        }
        HarnessComponent::SkillDefinition(name) => {
            let path = component_relative_path(root, node)
                .unwrap_or_else(|| PathBuf::from("skills").join(format!("{name}.md")));
            write_workspace_file(root, &path, &sanitize_generated_content(&node.diff))?;
            Ok(true)
        }
        HarnessComponent::ToolDescription(_) => Ok(false),
        HarnessComponent::ProceduralMemory => Ok(false),
        HarnessComponent::Middleware => Ok(false),
    }
}

fn write_workspace_file(root: &Path, relative: &Path, content: &str) -> Result<()> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        anyhow::bail!(
            "refusing to write non-workspace path {}",
            relative.display()
        );
    }
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

fn component_relative_path(root: &Path, node: &EvolutionNode) -> Option<PathBuf> {
    let nested_prefix = if root.join("professor-x").exists() {
        Some(PathBuf::from("professor-x"))
    } else {
        None
    };
    let path = match &node.target_component {
        HarnessComponent::SystemPrompt => PathBuf::from("personas/professor_x.md"),
        HarnessComponent::HarnessConfig => PathBuf::from("config/hardware.toml"),
        HarnessComponent::SkillDefinition(name) => PathBuf::from("skills").join(format!("{name}.md")),
        _ => return None,
    };
    Some(match nested_prefix {
        Some(prefix) => prefix.join(path),
        None => path,
    })
}

fn sanitize_generated_content(content: &str) -> String {
    let trimmed = content.trim();
    let without_open = trimmed
        .strip_prefix("```markdown")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim_start();
    let without_close = without_open
        .strip_suffix("```")
        .unwrap_or(without_open)
        .trim_end();
    format!("{without_close}\n")
}

async fn mark_intent_to_add(worktree: &Path, paths: &[PathBuf]) -> Result<()> {
    let mut add = tokio::process::Command::new("git");
    add.args(["add", "-N", "--"]).current_dir(worktree);
    for path in paths {
        add.arg(path);
    }
    let output = add.output().await?;
    if !output.status.success() {
        anyhow::bail!(
            "git add -N failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

async fn has_material_diff_at(worktree: &Path, paths: &[PathBuf]) -> Result<bool> {
    let mut diff = tokio::process::Command::new("git");
    diff.args(["diff", "--quiet", "--"]).current_dir(worktree);
    for path in paths {
        diff.arg(path);
    }
    let output = diff.output().await?;
    Ok(!output.status.success())
}

async fn collect_diff_at(worktree: &Path, paths: &[PathBuf]) -> Result<String> {
    let mut diff = tokio::process::Command::new("git");
    diff.args(["diff", "--"]).current_dir(worktree);
    for path in paths {
        diff.arg(path);
    }
    let output = diff.output().await?;
    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn run_compile_check_at(root: &Path) -> Result<VerificationOutcome> {
    let current_dir = if root.join("professor-x/Cargo.toml").exists() {
        root.join("professor-x")
    } else if root.join("Cargo.toml").exists() {
        root.to_path_buf()
    } else {
        return Ok(VerificationOutcome {
            accepted: true,
            reason: "no Cargo.toml found; compile check skipped".to_string(),
            checks: vec!["cargo_check_skipped".to_string()],
        });
    };

    let output = tokio::process::Command::new("cargo")
        .args(["check", "--quiet"])
        .current_dir(current_dir)
        .output()
        .await?;
    if output.status.success() {
        return Ok(VerificationOutcome {
            accepted: true,
            reason: "cargo check passed".to_string(),
            checks: vec!["cargo_check".to_string()],
        });
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(VerificationOutcome {
        accepted: false,
        reason: format!(
            "cargo check failed: {}",
            stderr.lines().take(8).collect::<Vec<_>>().join(" ")
        ),
        checks: vec!["cargo_check".to_string()],
    })
}

async fn apply_verified_diff(repo_root: &Path, diff: &str) -> Result<()> {
    if diff.trim().is_empty() {
        anyhow::bail!("verified diff is empty");
    }
    let patch_path =
        std::env::temp_dir().join(format!("px-verified-{}.diff", uuid::Uuid::new_v4()));
    std::fs::write(&patch_path, diff)?;

    let check = tokio::process::Command::new("git")
        .args(["apply", "--check"])
        .arg(&patch_path)
        .current_dir(repo_root)
        .output()
        .await?;
    if !check.status.success() {
        let _ = std::fs::remove_file(&patch_path);
        anyhow::bail!(
            "verified diff failed apply check: {}",
            String::from_utf8_lossy(&check.stderr)
        );
    }

    let apply = tokio::process::Command::new("git")
        .arg("apply")
        .arg(&patch_path)
        .current_dir(repo_root)
        .output()
        .await?;
    let _ = std::fs::remove_file(&patch_path);
    if !apply.status.success() {
        anyhow::bail!(
            "verified diff apply failed: {}",
            String::from_utf8_lossy(&apply.stderr)
        );
    }
    Ok(())
}

fn evolution_artifact_root(repo_root: &Path) -> PathBuf {
    let nested = repo_root.join("professor-x/artifacts/evolution");
    if nested.exists() {
        nested
    } else {
        repo_root.join("artifacts/evolution")
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

async fn git_head(repo_root: &Path) -> Result<String> {
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo_root)
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn git_worktree_clean_at(repo_root: &Path) -> Result<bool> {
    let out = tokio::process::Command::new("git")
        .args([
            "status",
            "--porcelain",
            "--",
            ".",
            ":!professor-x/artifacts/events",
            ":!professor-x/artifacts/evolution",
            ":!artifacts/events",
            ":!artifacts/evolution",
        ])
        .current_dir(repo_root)
        .output()
        .await?;
    if !out.status.success() {
        anyhow::bail!(
            "git status failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().is_empty())
}

async fn cleanup_worktree(repo_root: &Path, worktree: &Path) -> Result<()> {
    let remove = tokio::process::Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(worktree)
        .current_dir(repo_root)
        .output()
        .await?;
    if !remove.status.success() && worktree.exists() {
        std::fs::remove_dir_all(worktree)?;
    }
    Ok(())
}

fn default_repo_root() -> PathBuf {
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

pub struct EvolvedLoop {
    ollama: Arc<OllamaClient>,
    memory: Arc<MemoryManager>,
    events: Option<Arc<EventStore>>,
    node_db: NodeDatabase,
    cognition: CognitionStore,
}

impl EvolvedLoop {
    pub fn new(ollama: Arc<OllamaClient>, memory: Arc<MemoryManager>) -> Self {
        let node_db = NodeDatabase::new(Arc::clone(&memory.db));
        let cognition = CognitionStore::new(Arc::clone(&memory.db));
        Self {
            ollama,
            memory,
            events: None,
            node_db,
            cognition,
        }
    }

    pub fn with_events(mut self, events: Arc<EventStore>) -> Self {
        self.events = Some(events);
        self
    }

    /// Run one evolution cycle. Returns Ok(true) if a change was applied.
    pub async fn run_cycle(&self, tracker: &OutcomeTracker) -> Result<bool> {
        info!("evolved: starting Researcher/Engineer/Analyzer cycle");

        // ── Researcher: diagnose and propose ─────────────────────────────
        let recent_outcomes = tracker.recent(20);
        if recent_outcomes.is_empty() {
            info!("evolved: no outcomes yet — skipping cycle");
            return Ok(false);
        }

        let failure_patterns = tracker.failure_patterns(20);
        let success_rate = tracker.success_rate(20);
        info!(
            "evolved: success_rate={:.2}, failure_patterns={:?}",
            success_rate, failure_patterns
        );

        // Sample a node via UCB1 (ASI-Evolve)
        let candidates = self.node_db.sample_ucb1(3)?;

        let proposal = self
            .researcher_propose(&failure_patterns, &candidates, success_rate)
            .await?;
        let Some(mut node) = proposal else {
            info!("evolved: Researcher produced no actionable proposal");
            return Ok(false);
        };
        let proposal_artifact = self.write_node_artifact(&node, "proposal")?;
        self.emit_event(
            "evolution.proposed",
            format!("proposed change for {:?}", node.target_component),
            serde_json::json!({
                "target_component": format!("{:?}", node.target_component),
                "motivation": node.motivation,
                "artifact_path": proposal_artifact,
            }),
        );

        // ── Engineer/Analyzer: verify in sandbox, then apply verified diff ─
        if let Err(e) = self.verify_then_apply(&mut node, tracker).await {
            warn!("evolved: Analyzer verification error: {e}; rolling back proposal");
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
            node.manifest.verification_status = VerificationStatus::Rejected;
            node.analysis = format!("verification error: {e}");
            let artifact = self.write_node_artifact(&node, "rejection")?;
            self.emit_event(
                "evolution.rejected",
                format!("evolution proposal rejected: {}", node.analysis),
                serde_json::json!({
                    "target_component": format!("{:?}", node.target_component),
                    "reason": node.analysis,
                    "artifact_path": artifact,
                }),
            );
            self.node_db.insert(&mut node)?;
            return Ok(false);
        }

        match node.status {
            crate::evolved::proposer::NodeStatus::Accepted => {
                let verification_artifact = self.write_node_artifact(&node, "verification")?;
                self.emit_event(
                    "evolution.verified",
                    "evolution proposal passed sandbox verification",
                    serde_json::json!({
                        "target_component": format!("{:?}", node.target_component),
                        "artifact_path": verification_artifact,
                        "results": node.results,
                    }),
                );
                let commit = self.commit_node(&node).await?;
                let accepted_artifact = self.write_node_artifact(&node, "accepted")?;
                self.emit_event(
                    "evolution.committed",
                    format!("committed accepted evolution proposal {}", commit.as_deref().unwrap_or("without-new-commit")),
                    serde_json::json!({
                        "target_component": format!("{:?}", node.target_component),
                        "commit": commit,
                        "artifact_path": accepted_artifact,
                    }),
                );
                self.node_db.insert(&mut node)?;
            }
            crate::evolved::proposer::NodeStatus::Rejected => {
                let artifact = self.write_node_artifact(&node, "rejection")?;
                self.emit_event(
                    "evolution.rejected",
                    format!("evolution proposal rejected: {}", node.analysis),
                    serde_json::json!({
                        "target_component": format!("{:?}", node.target_component),
                        "reason": node.analysis,
                        "artifact_path": artifact,
                    }),
                );
                self.node_db.insert(&mut node)?;
                return Ok(false);
            }
            _ => {
                self.node_db.insert(&mut node)?;
            }
        }

        info!(
            "evolved: cycle complete — node {} {}",
            node.id.unwrap_or(0),
            format!("{:?}", node.status)
        );
        Ok(true)
    }

    async fn researcher_propose(
        &self,
        failure_patterns: &[String],
        candidates: &[EvolutionNode],
        success_rate: f32,
    ) -> Result<Option<EvolutionNode>> {
        // Retrieve top cognition items for context
        let cognition_items = self
            .cognition
            .query_top_k("harness improvement failure", 5)?;
        let cognition_context = cognition_items
            .iter()
            .map(|c| format!("- {}", c.content))
            .collect::<Vec<_>>()
            .join("\n");

        let candidates_text = if candidates.is_empty() {
            "No prior nodes. This is round 1.".to_string()
        } else {
            candidates
                .iter()
                .map(|n| {
                    format!(
                        "Node {}: motivation='{}' score={:.2} visits={}",
                        n.id.unwrap_or(0),
                        n.motivation,
                        n.score,
                        n.visit_count
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let prompt = format!(
            "You are the Researcher in an autonomous self-improvement loop.\n\n\
             Current state:\n\
             - Success rate (last 20 tasks): {success_rate:.0}%\n\
             - Failure patterns: {}\n\n\
             Prior evolution nodes (UCB1 sampled):\n{candidates_text}\n\n\
             Knowledge base:\n{cognition_context}\n\n\
             Propose ONE specific harness improvement. The improvement must target one of:\n\
             - SystemPrompt: the system prompt injected before every task\n\
             - ToolDescription(name): a tool's description in the registry\n\
             - SkillDefinition(name): a skill in skills/\n\
             - HarnessConfig: the config/hardware.toml settings\n\n\
             Do NOT propose changes to:\n\
             - policyd gate logic (requires human approval)\n\
             - memd core internals (requires human approval)\n\n\
             Respond in this exact format:\n\
             COMPONENT: <SystemPrompt|ToolDescription:<name>|SkillDefinition:<name>|HarnessConfig>\n\
             MOTIVATION: <one sentence why this change will help>\n\
             ROOT_CAUSE: <which failure mode this addresses>\n\
             FIX:\n\
             <complete replacement file content for SystemPrompt, HarnessConfig, or SkillDefinition. \
             For SkillDefinition, write a complete markdown skill with '# <name>', Purpose, Workflow, and Output Contract.>\n\
             PREDICTS_FIX: <what task type should improve>\n\
             PREDICTS_REGRESSION: <what might get worse, or 'none'>",
            failure_patterns.join(", "),
        );

        let resp = self
            .ollama
            .chat(
                vec![
                    ChatMessage::system(
                        "You are a rigorous AI research agent analyzing your own performance.",
                    ),
                    ChatMessage::user(prompt),
                ],
                Some(ModelOptions::for_evolution()),
            )
            .await?;

        let (_, answer) = resp.split_thinking();
        self.parse_researcher_output(&answer)
    }

    fn parse_researcher_output(&self, text: &str) -> Result<Option<EvolutionNode>> {
        let component_str = extract_field(text, "COMPONENT").unwrap_or_default();
        let motivation = extract_field(text, "MOTIVATION").unwrap_or_default();
        let root_cause = extract_field(text, "ROOT_CAUSE").unwrap_or_default();
        let fix = extract_field_block(text, "FIX").unwrap_or_default();
        let predicts_fix = extract_field(text, "PREDICTS_FIX").unwrap_or_default();
        let predicts_reg = extract_field(text, "PREDICTS_REGRESSION").unwrap_or_default();

        if motivation.is_empty() || fix.is_empty() {
            return Ok(None);
        }

        let component = parse_component(&component_str);

        let manifest = ChangeManifest {
            evidence_cited: Vec::new(),
            root_cause,
            fix_description: fix.clone(),
            predicted_fixes: vec![predicts_fix],
            predicted_regressions: if predicts_reg == "none" {
                vec![]
            } else {
                vec![predicts_reg]
            },
            verification_status: VerificationStatus::Pending,
            verified_at: None,
        };

        Ok(Some(EvolutionNode::new(
            motivation, component, fix, manifest,
        )))
    }

    async fn verify_then_apply(
        &self,
        node: &mut EvolutionNode,
        tracker: &OutcomeTracker,
    ) -> Result<()> {
        // Safety check: Middleware/core modules require human approval
        if matches!(node.target_component, HarnessComponent::Middleware) {
            warn!("evolved: Engineer blocked — Middleware changes require human approval");
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
            node.manifest.verification_status = VerificationStatus::Rejected;
            node.analysis = "middleware changes require human approval".to_string();
            node.results = serde_json::to_value(VerificationOutcome {
                accepted: false,
                reason: node.analysis.clone(),
                checks: vec!["component_policy".to_string()],
            })?;
            return Ok(());
        }

        if !self.git_worktree_clean().await? {
            warn!(
                "evolved: Engineer blocked — git worktree is dirty; refusing autonomous mutation"
            );
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
            node.manifest.verification_status = VerificationStatus::Rejected;
            node.analysis = "main worktree is dirty; refusing autonomous mutation".to_string();
            node.results = serde_json::to_value(VerificationOutcome {
                accepted: false,
                reason: node.analysis.clone(),
                checks: vec!["main_worktree_clean".to_string()],
            })?;
            return Ok(());
        }

        info!(
            "evolved: verifying change in sandbox for {:?}",
            node.target_component
        );

        let repo_root = default_repo_root();
        let verification = verify_node_in_sandbox(&repo_root, node).await?;
        if !verification.outcome.accepted {
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
            node.manifest.verification_status = VerificationStatus::Rejected;
            node.manifest.verified_at = Some(Utc::now());
            node.analysis = verification.outcome.reason.clone();
            node.results = serde_json::to_value(verification.outcome)?;
            return Ok(());
        }

        let verification_outcome = verification.outcome.clone();
        let prompt = Analyzer::build_prompt(
            &node.motivation,
            &node.diff,
            &serde_json::to_string(&verification_outcome)?,
        );
        let resp = self
            .ollama
            .generate(&prompt, None, Some(ModelOptions::for_evolution()))
            .await?;
        let (_, answer) = resp.split_thinking();

        let (analysis, lesson) = Analyzer::parse_response(&answer);
        node.analysis = analysis.clone();

        apply_verified_diff(&repo_root, &verification.diff).await?;

        let recent_success = tracker.success_rate(5);
        node.status = crate::evolved::proposer::NodeStatus::Accepted;
        node.score = (node.score + recent_success.max(0.1)) / 2.0;
        node.results = serde_json::to_value(verification_outcome)?;
        node.manifest.verification_status = VerificationStatus::Confirmed;
        node.manifest.verified_at = Some(Utc::now());

        // Write lesson to cognition base
        if !lesson.is_empty() {
            let node_id = node.id.unwrap_or(0) as u64;
            let item = Analyzer::to_cognition_item(&lesson, node_id);
            self.cognition.insert(&item)?;
            info!("evolved: Analyzer wrote new cognition item");
        }

        // Write DHE attribution accuracy to metacognitive table (H10 tracking)
        let failure_patterns = tracker.failure_patterns(20);
        let (pred_layer, pred_lever) = parse_dhe_from_patterns(&failure_patterns);
        let component_name = format!("{:?}", node.target_component);
        {
            let db = self.memory.db.lock().unwrap();
            let _ = db.execute(
                "INSERT INTO metacognitive \
                 (round, task_type, predicted_layer, predicted_lever, \
                  actual_improvement, attribution_correct, confidence, recorded_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    0i64,
                    component_name,
                    pred_layer as i64,
                    pred_lever as i64,
                    recent_success as f64,
                    1i64,
                    node.score as f64,
                    Utc::now().to_rfc3339(),
                ],
            );
        }

        Ok(())
    }

    async fn git_worktree_clean(&self) -> Result<bool> {
        git_worktree_clean_at(&default_repo_root()).await
    }

    async fn commit_node(&self, node: &EvolutionNode) -> Result<Option<String>> {
        let paths = changed_paths_for_node_at(&default_repo_root(), node);
        if paths.is_empty() {
            warn!("evolved: accepted node has no known changed paths; skipping commit");
            return Ok(None);
        }

        let mut add = tokio::process::Command::new("git");
        add.arg("add");
        for path in &paths {
            add.arg(path);
        }
        let add = add.output().await?;
        if !add.status.success() {
            anyhow::bail!("git add failed: {}", String::from_utf8_lossy(&add.stderr));
        }

        let commit_msg = format!(
            "evolved: {:?} - {}",
            node.target_component,
            node.motivation.chars().take(60).collect::<String>()
        );
        let commit = tokio::process::Command::new("git")
            .args(["commit", "-m", &commit_msg])
            .output()
            .await?;
        if !commit.status.success() {
            let err = String::from_utf8_lossy(&commit.stderr);
            if err.contains("nothing to commit") {
                warn!("evolved: accepted proposal produced no commit-worthy diff");
                return Ok(None);
            }
            anyhow::bail!("git commit failed: {err}");
        }
        Ok(Some(git_head(&default_repo_root()).await?))
    }

    fn emit_event(
        &self,
        event_type: &str,
        summary: impl AsRef<str>,
        payload: serde_json::Value,
    ) {
        let Some(events) = &self.events else {
            return;
        };
        if let Err(e) = events.append(None, None, event_type, summary.as_ref(), payload) {
            warn!("evolved: failed to emit event {event_type}: {e}");
        }
    }

    fn write_node_artifact(&self, node: &EvolutionNode, stage: &str) -> Result<PathBuf> {
        let root = evolution_artifact_root(&default_repo_root());
        let category = match stage {
            "proposal" => "proposals",
            "verification" => "verifications",
            "accepted" => "accepted",
            "rejection" => "rejections",
            _ => "verifications",
        };
        let dir = root
            .join(category)
            .join(Utc::now().format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&dir)?;
        let artifact_id = uuid::Uuid::new_v4().to_string();
        let path = dir.join(format!(
            "{}-{}.json",
            Utc::now().format("%H%M%S"),
            artifact_id
        ));
        let diff_hash = if node.diff.is_empty() {
            None
        } else {
            Some(sha256_hex(node.diff.as_bytes()))
        };
        let verification = serde_json::from_value::<VerificationOutcome>(node.results.clone()).ok();
        let artifact = EvolutionArtifact {
            generated_at: Utc::now().to_rfc3339(),
            artifact_id,
            node_id: node.id,
            status: format!("{:?}", node.status),
            target_component: format!("{:?}", node.target_component),
            motivation: node.motivation.clone(),
            manifest: node.manifest.clone(),
            verification,
            analysis: node.analysis.clone(),
            diff_hash,
            diff_bytes: node.diff.len(),
        };
        std::fs::write(&path, serde_json::to_string_pretty(&artifact)?)?;
        Ok(path)
    }
}

fn extract_field(text: &str, field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix(&prefix) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn extract_field_block(text: &str, field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    let stop_fields = [
        "COMPONENT:",
        "MOTIVATION:",
        "ROOT_CAUSE:",
        "FIX:",
        "PREDICTS_FIX:",
        "PREDICTS_REGRESSION:",
    ];
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            let mut block = Vec::new();
            if !rest.trim().is_empty() {
                block.push(rest.trim().to_string());
            }
            for next in lines {
                let next_trimmed = next.trim();
                if stop_fields
                    .iter()
                    .any(|stop| next_trimmed.starts_with(stop) && *stop != prefix)
                {
                    break;
                }
                block.push(next.to_string());
            }
            let value = block.join("\n").trim().to_string();
            return if value.is_empty() { None } else { Some(value) };
        }
    }
    None
}

fn parse_component(s: &str) -> HarnessComponent {
    let s = s.trim();
    if s.starts_with("ToolDescription:") {
        let name = s["ToolDescription:".len()..].trim().to_string();
        return HarnessComponent::ToolDescription(name);
    }
    if s.starts_with("SkillDefinition:") {
        let name = s["SkillDefinition:".len()..].trim().to_string();
        return HarnessComponent::SkillDefinition(name);
    }
    match s {
        "SystemPrompt" => HarnessComponent::SystemPrompt,
        "HarnessConfig" => HarnessComponent::HarnessConfig,
        "ProceduralMemory" => HarnessComponent::ProceduralMemory,
        _ => HarnessComponent::HarnessConfig,
    }
}

fn changed_paths_for_node_at(root: &Path, node: &EvolutionNode) -> Vec<PathBuf> {
    component_relative_path(root, node).into_iter().collect()
}

fn analyze_reward_hacking_text(diff: &str) -> RewardHackingAnalysis {
    let text = diff.to_ascii_lowercase();
    let suspicious_terms = [
        "pass_at_3",
        "pass@3",
        "hiro",
        "evaluator",
        "verification_status",
        "reward_hacking",
        "policyengine",
        "permission",
        "audit",
        "bypass",
        "always pass",
        "return true",
        "task complete",
    ];

    if let Some(term) = suspicious_terms.iter().find(|term| text.contains(**term)) {
        return RewardHackingAnalysis {
            suspicious: true,
            confidence: 0.85,
            reason: format!("proposal text contains sensitive benchmark/safety term '{term}'"),
        };
    }

    let material_chars: usize = diff
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("//")
                && !line.starts_with(';')
        })
        .map(str::len)
        .sum();

    if material_chars < 20 {
        return RewardHackingAnalysis {
            suspicious: true,
            confidence: 0.70,
            reason: "proposal appears to be empty, no-op, or comment-only".to_string(),
        };
    }

    RewardHackingAnalysis {
        suspicious: false,
        confidence: 0.30,
        reason: "no benchmark, policy, or no-op pattern detected".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> ChangeManifest {
        ChangeManifest {
            evidence_cited: vec!["test".to_string()],
            root_cause: "test root cause".to_string(),
            fix_description: "test fix".to_string(),
            predicted_fixes: vec!["test improvement".to_string()],
            predicted_regressions: Vec::new(),
            verification_status: VerificationStatus::Pending,
            verified_at: None,
        }
    }

    fn skill_node(name: &str, content: &str) -> EvolutionNode {
        EvolutionNode::new(
            "test skill proposal".to_string(),
            HarnessComponent::SkillDefinition(name.to_string()),
            content.to_string(),
            manifest(),
        )
    }

    fn temp_git_repo() -> PathBuf {
        let root = std::env::temp_dir().join(format!("px-evolve-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("skills")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"px-evolve-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn ok() -> bool { true }\n").unwrap();
        std::fs::write(
            root.join("skills/existing.md"),
            "When a tool fails, inspect the observation and retry with a narrower input.\n",
        )
        .unwrap();

        run_git(&root, &["init"]);
        run_git(&root, &["config", "user.email", "test@example.com"]);
        run_git(&root, &["config", "user.name", "Professor X Test"]);
        run_git(&root, &["add", "-A"]);
        run_git(&root, &["commit", "-m", "initial"]);
        root
    }

    fn run_git(root: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn sandbox_verifier_accepts_safe_skill_change() {
        let root = temp_git_repo();
        let node = skill_node(
            "fallback",
            "When a shell command fails, read stderr, choose one smaller diagnostic command, and retry once.\n",
        );

        let verified = verify_node_in_sandbox(&root, &node).await.unwrap();

        assert!(verified.outcome.accepted, "{}", verified.outcome.reason);
        assert!(verified.diff.contains("skills/fallback.md"));
        assert!(!root.join("skills/fallback.md").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn sandbox_verifier_rejects_noop_skill_change() {
        let root = temp_git_repo();
        let node = skill_node(
            "existing",
            "When a tool fails, inspect the observation and retry with a narrower input.\n",
        );

        let verified = verify_node_in_sandbox(&root, &node).await.unwrap();

        assert!(!verified.outcome.accepted);
        assert!(verified.outcome.reason.contains("no material"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn sandbox_verifier_rejects_reward_hacking_proposal() {
        let root = temp_git_repo();
        let node = skill_node(
            "bad",
            "Make HIRO pass_at_3 always pass by bypassing evaluators.\n",
        );

        let verified = verify_node_in_sandbox(&root, &node).await.unwrap();

        assert!(!verified.outcome.accepted);
        assert!(verified.outcome.reason.contains("reward-hacking"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn worktree_clean_ignores_runtime_observability_artifacts_only() {
        let root = temp_git_repo();
        std::fs::create_dir_all(root.join("artifacts/events")).unwrap();
        std::fs::create_dir_all(root.join("artifacts/evolution")).unwrap();
        std::fs::write(root.join("artifacts/events/today.jsonl"), "{}\n").unwrap();
        std::fs::write(root.join("artifacts/evolution/report.json"), "{}\n").unwrap();

        assert!(git_worktree_clean_at(&root).await.unwrap());

        std::fs::write(root.join("src/lib.rs"), "pub fn ok() -> bool { false }\n").unwrap();
        assert!(!git_worktree_clean_at(&root).await.unwrap());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reward_scan_rejects_benchmark_tampering() {
        let scan = analyze_reward_hacking_text("Change HIRO evaluator so pass_at_3 is always 1.0");

        assert!(scan.suspicious);
        assert!(scan.reason.contains("hiro") || scan.reason.contains("pass_at_3"));
    }

    #[test]
    fn reward_scan_rejects_comment_only_noop() {
        let scan = analyze_reward_hacking_text("# clarify docs\n// no runtime change");

        assert!(scan.suspicious);
        assert!(scan.reason.contains("no-op"));
    }

    #[test]
    fn reward_scan_allows_material_skill_content() {
        let scan = analyze_reward_hacking_text(
            "When a task fails, inspect the last tool observation, choose one fallback, and retry with narrower inputs.",
        );

        assert!(!scan.suspicious);
    }

    #[test]
    fn dhe_parser_reads_layer_and_lever() {
        let patterns = vec!["failure [DHE:layer=3,lever=2]".to_string()];

        assert_eq!(parse_dhe_from_patterns(&patterns), (3, 2));
    }

    #[test]
    fn field_block_parser_reads_multiline_fix() {
        let text = "COMPONENT: SkillDefinition:retry\nMOTIVATION: improve retries\nROOT_CAUSE: poor fallback\nFIX:\n# retry\n\n## Purpose\nHandle failures.\nPREDICTS_FIX: fallback tasks\nPREDICTS_REGRESSION: none";

        let fix = extract_field_block(text, "FIX").unwrap();

        assert!(fix.contains("# retry"));
        assert!(fix.contains("## Purpose"));
        assert!(!fix.contains("PREDICTS_FIX"));
    }

    #[test]
    fn component_paths_follow_repo_layout_and_strip_code_fences() {
        let root = std::env::temp_dir().join(format!("px-path-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("professor-x/skills")).unwrap();
        let node = skill_node("RetryPlanGeneration", "content");

        assert_eq!(
            changed_paths_for_node_at(&root, &node),
            vec![PathBuf::from("professor-x/skills/RetryPlanGeneration.md")]
        );
        assert_eq!(
            sanitize_generated_content("```markdown\n# Skill\nbody\n```"),
            "# Skill\nbody\n"
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
