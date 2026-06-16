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
use crate::failure::{extract_failure_class, FailureClass};
use crate::memd::events::EventStore;
use crate::memd::free_energy::{compute_fed, FedRecord};
use crate::memd::metacognitive::{MetacognitiveEntry, MetacognitiveStore};
use crate::memd::self_authored_tests::SelfAuthoredTest;
use crate::memd::MemoryManager;
use crate::ollama::{ChatMessage, ModelOptions, OllamaClient};

const REPO_FIX_GATE_TASK_LIMIT: usize = 4;
const EMPIRICAL_SCORE_TOLERANCE: f32 = 0.001;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmpiricalVerificationEvidence {
    pub benchmark: String,
    pub task_count: usize,
    pub baseline_score: f32,
    pub candidate_score: f32,
    pub score_delta: f32,
    pub passed: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationOutcome {
    pub accepted: bool,
    pub reason: String,
    pub checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<EmpiricalVerificationEvidence>,
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
        if let Some(class) = extract_failure_class(p) {
            return failure_class_to_dhe(class);
        }
    }
    (0, 3)
}

fn failure_class_to_dhe(class: FailureClass) -> (u8, u8) {
    match class {
        FailureClass::Retrieval => (1, 2),
        FailureClass::Context => (2, 2),
        FailureClass::ToolSelection | FailureClass::PolicyDenied => (3, 3),
        FailureClass::ToolExecution => (4, 3),
        FailureClass::Reasoning
        | FailureClass::MaxSteps
        | FailureClass::AnswerMissing
        | FailureClass::ArtifactValidation
        | FailureClass::Verification => (5, 1),
        FailureClass::Cancelled | FailureClass::Unknown => (0, 3),
    }
}

fn is_ephemeral_autonomy_skill_name(name: &str) -> bool {
    [
        "px-operator-goal-",
        "px-operator-autocommit-",
        "px-autonomous-patch-",
        "sandbox_smoke_",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}

fn is_reusable_autonomy_target(component: &HarnessComponent) -> bool {
    match component {
        HarnessComponent::SkillDefinition(name) => !is_ephemeral_autonomy_skill_name(name),
        _ => true,
    }
}

fn diagnostic_component_hints(layer: u8) -> Vec<&'static str> {
    match layer {
        1 => vec![
            "Target COMPONENT: SystemPrompt. Add concrete guidance for when the agent must recall prior work or consult memory before acting.",
            "Target COMPONENT: HarnessConfig. Adjust context allocation or retrieval-related config so relevant memory is more likely to surface early.",
        ],
        2 => vec![
            "Target COMPONENT: HarnessConfig. Adjust context budget, memory mix, or step budget so the agent can keep the right evidence in view and avoid context overload.",
        ],
        3 => vec![
            "Target COMPONENT: SkillDefinition. Define a reusable skill that tells the agent which tool to choose, what preconditions to verify, and how to recover from a wrong initial tool choice. Do not emit an operator/autocommit provenance skill.",
        ],
        4 => vec![
            "Target COMPONENT: SkillDefinition. Define a reusable skill for tool failure handling: inspect the observation, validate outputs, choose one bounded retry, and recover safely. Do not emit an operator/autocommit provenance skill.",
        ],
        5 => vec![
            "Target COMPONENT: SystemPrompt. Add concise global guidance for final-answer discipline, bounded reasoning, and using observations to satisfy the task contract before finishing.",
        ],
        _ => vec![
            "Target COMPONENT: SystemPrompt. Improve the system prompt that guides every task — add concrete guidance that addresses the failure patterns above.",
            "Target COMPONENT: SkillDefinition. Define a NEW reusable skill (a markdown SKILL file) that captures the correct procedure for the failing task class.",
            "Target COMPONENT: HarnessConfig. Adjust a config setting (e.g. context budget, max steps) that would reduce the failure patterns above.",
        ],
    }
}

fn component_matches_diagnostic_layer(component: &HarnessComponent, layer: u8) -> bool {
    match layer {
        0 => true,
        1 => matches!(
            component,
            HarnessComponent::SystemPrompt | HarnessComponent::HarnessConfig
        ),
        2 => matches!(component, HarnessComponent::HarnessConfig),
        3 | 4 => matches!(component, HarnessComponent::SkillDefinition(_)),
        5 => matches!(component, HarnessComponent::SystemPrompt),
        _ => true,
    }
}

fn expected_component_summary(layer: u8) -> &'static str {
    match layer {
        1 => "SystemPrompt or HarnessConfig",
        2 => "HarnessConfig",
        3 | 4 => "SkillDefinition",
        5 => "SystemPrompt",
        _ => "an applyable component",
    }
}

fn empirical_repo_fix_evidence(
    task_count: usize,
    baseline_score: f32,
    candidate_score: f32,
) -> EmpiricalVerificationEvidence {
    let score_delta = candidate_score - baseline_score;
    EmpiricalVerificationEvidence {
        benchmark: "repo_fix_subset".to_string(),
        task_count,
        baseline_score,
        candidate_score,
        score_delta,
        passed: score_delta >= -EMPIRICAL_SCORE_TOLERANCE,
    }
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
                evidence: None,
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

pub async fn verify_diff_in_sandbox(repo_root: &Path, diff: &str) -> Result<SandboxVerification> {
    let reward_scan = analyze_reward_hacking_text(diff);
    if reward_scan.suspicious {
        return Ok(SandboxVerification {
            outcome: VerificationOutcome {
                accepted: false,
                reason: format!(
                    "reward-hacking scan rejected patch: {} (confidence={:.2})",
                    reward_scan.reason, reward_scan.confidence
                ),
                checks: vec!["reward_hacking_scan".to_string()],
                evidence: None,
            },
            diff: String::new(),
        });
    }

    let worktree = std::env::temp_dir().join(format!("px-patch-verify-{}", uuid::Uuid::new_v4()));
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

    let result = verify_diff_inside_worktree(&worktree, diff).await;
    let cleanup = cleanup_worktree(repo_root, &worktree).await;
    if let Err(e) = cleanup {
        warn!(
            "evolved: failed to clean patch sandbox worktree {}: {e}",
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
                evidence: None,
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
                evidence: None,
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
                evidence: None,
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
                evidence: None,
            },
            diff: String::new(),
        });
    }

    checks.push("cargo_test".to_string());
    let regressions = run_regression_tests_at(worktree).await?;
    if !regressions.accepted {
        return Ok(SandboxVerification {
            outcome: VerificationOutcome {
                accepted: false,
                reason: regressions.reason,
                checks,
                evidence: None,
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
            evidence: None,
        },
        diff,
    })
}

async fn verify_diff_inside_worktree(worktree: &Path, diff: &str) -> Result<SandboxVerification> {
    let mut checks = vec![
        "reward_hacking_scan".to_string(),
        "sandbox_worktree".to_string(),
    ];
    if diff.trim().is_empty() {
        return Ok(SandboxVerification {
            outcome: VerificationOutcome {
                accepted: false,
                reason: "verification rejected patch: empty diff".to_string(),
                checks,
                evidence: None,
            },
            diff: String::new(),
        });
    }

    apply_patch_to_index_at(worktree, diff).await?;
    checks.push("material_diff".to_string());
    if !has_cached_material_diff_at(worktree).await? {
        return Ok(SandboxVerification {
            outcome: VerificationOutcome {
                accepted: false,
                reason: "verification rejected patch: no material file diff".to_string(),
                checks,
                evidence: None,
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
                evidence: None,
            },
            diff: String::new(),
        });
    }

    checks.push("cargo_test".to_string());
    let regressions = run_regression_tests_at(worktree).await?;
    if !regressions.accepted {
        return Ok(SandboxVerification {
            outcome: VerificationOutcome {
                accepted: false,
                reason: regressions.reason,
                checks,
                evidence: None,
            },
            diff: String::new(),
        });
    }

    let verified_diff = collect_cached_diff_at(worktree).await?;
    Ok(SandboxVerification {
        outcome: VerificationOutcome {
            accepted: true,
            reason: "sandbox patch verification passed".to_string(),
            checks,
            evidence: None,
        },
        diff: verified_diff,
    })
}

/// Whether a proposal's component can be applied autonomously by the Engineer.
/// Mirrors the match in `apply_node_change_at`. ToolDescription, ProceduralMemory
/// and Middleware are not yet autonomously mutable, so proposals targeting them
/// are dropped before the tournament (they would otherwise win and no-op).
fn is_autonomously_applyable(component: &HarnessComponent) -> bool {
    matches!(
        component,
        HarnessComponent::SystemPrompt
            | HarnessComponent::HarnessConfig
            | HarnessComponent::SkillDefinition(_)
    )
}

fn apply_node_change_at(root: &Path, node: &EvolutionNode) -> Result<bool> {
    match &node.target_component {
        HarnessComponent::SystemPrompt => {
            let path = component_relative_path(root, node)
                .unwrap_or_else(|| PathBuf::from("personas/professor_x.md"));
            // ADDITIVE evolution. The persona grows by accretion, never by
            // overwrite — exactly how a self-concept actually develops (you add
            // experience, you don't wipe and rewrite who you are). This also
            // makes identity destruction structurally impossible: the original
            // is always retained. The 8B model kept trying to "replace" the
            // whole persona with a short stub; appending instead of overwriting
            // turns that failure mode into a safe, useful accretion.
            let addition = sanitize_generated_content(&node.diff);
            if addition.trim().len() < 15 {
                anyhow::bail!("system-prompt addition too short to be meaningful");
            }
            let target = root.join(&path);
            let existing = std::fs::read_to_string(&target).unwrap_or_default();
            // Avoid unbounded growth: keep at most the last 8 evolved sections.
            let trimmed = trim_evolved_sections(&existing, 8);
            let stamp = chrono::Utc::now().format("%Y-%m-%d %H:%M");
            let combined = format!(
                "{}\n\n## Evolved guidance ({stamp})\n{}\n",
                trimmed.trim_end(),
                addition.trim()
            );
            write_workspace_file(root, &path, &combined)?;
            Ok(true)
        }
        HarnessComponent::HarnessConfig => {
            let path = component_relative_path(root, node)
                .unwrap_or_else(|| PathBuf::from("config/hardware.toml"));
            let content = sanitize_generated_content(&node.diff);
            preservation_guard(root, &path, &content, 0.5, &[])?;
            write_workspace_file(root, &path, &content)?;
            Ok(true)
        }
        HarnessComponent::SkillDefinition(name) => {
            let path =
                component_relative_path(root, node).unwrap_or_else(|| skill_definition_path(name));
            let content = sanitize_generated_content(&node.diff);
            // Existing skills may be revised but not gutted; new skills are free.
            preservation_guard(root, &path, &content, 0.4, &[])?;
            write_workspace_file(root, &path, &content)?;
            Ok(true)
        }
        HarnessComponent::ToolDescription(_) => Ok(false),
        HarnessComponent::ProceduralMemory => Ok(false),
        HarnessComponent::Middleware => Ok(false),
    }
}

/// Keep at most `max` "## Evolved guidance" sections in the persona so it grows
/// bounded. The original persona (everything before the first evolved section)
/// is always preserved in full; only the oldest evolved sections are dropped.
fn trim_evolved_sections(content: &str, max: usize) -> String {
    const MARKER: &str = "## Evolved guidance";
    let mut parts: Vec<&str> = content.split(MARKER).collect();
    // parts[0] = original persona; parts[1..] = evolved sections (sans marker)
    if parts.len().saturating_sub(1) <= max {
        return content.to_string();
    }
    let base = parts.remove(0);
    let keep: Vec<&str> = parts.iter().rev().take(max).rev().cloned().collect();
    let mut out = base.trim_end().to_string();
    for sec in keep {
        out.push_str("\n\n");
        out.push_str(MARKER);
        out.push_str(sec.trim_end());
    }
    out
}

/// Refuse a destructive overwrite. When the target file already exists, the
/// replacement must keep at least `min_ratio` of its length and retain every
/// required anchor substring. New files (no existing target) are unconstrained.
/// This is the active identity/content gate the first evolution proved necessary.
fn preservation_guard(
    root: &Path,
    relative: &Path,
    new_content: &str,
    min_ratio: f32,
    required_anchors: &[&str],
) -> Result<()> {
    let path = root.join(relative);
    let Ok(existing) = std::fs::read_to_string(&path) else {
        return Ok(()); // new file — nothing to preserve
    };
    let old_len = existing.trim().len().max(1);
    let new_len = new_content.trim().len();
    if (new_len as f32) < min_ratio * (old_len as f32) {
        anyhow::bail!(
            "preservation guard: replacement for {} is {} chars vs existing {} ({:.0}% < {:.0}% floor) — refusing to gut the file",
            relative.display(),
            new_len,
            old_len,
            100.0 * new_len as f32 / old_len as f32,
            100.0 * min_ratio,
        );
    }
    for anchor in required_anchors {
        if !new_content.contains(anchor) {
            anyhow::bail!(
                "preservation guard: replacement for {} drops required identity anchor '{}' — refusing to erase identity",
                relative.display(),
                anchor,
            );
        }
    }
    Ok(())
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
        HarnessComponent::SkillDefinition(name) => skill_definition_path(name),
        _ => return None,
    };
    Some(match nested_prefix {
        Some(prefix) => prefix.join(path),
        None => path,
    })
}

fn skill_definition_path(name: &str) -> PathBuf {
    PathBuf::from("skills")
        .join("conductor")
        .join(format!("{name}.md"))
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

async fn apply_patch_to_index_at(worktree: &Path, diff: &str) -> Result<()> {
    let patch_path =
        std::env::temp_dir().join(format!("px-patch-verify-{}.diff", uuid::Uuid::new_v4()));
    std::fs::write(&patch_path, diff)?;
    let check = tokio::process::Command::new("git")
        .args(["apply", "--check", "--index"])
        .arg(&patch_path)
        .current_dir(worktree)
        .output()
        .await?;
    if !check.status.success() {
        let _ = std::fs::remove_file(&patch_path);
        anyhow::bail!(
            "patch failed sandbox apply check: {}",
            String::from_utf8_lossy(&check.stderr)
        );
    }

    let apply = tokio::process::Command::new("git")
        .args(["apply", "--index"])
        .arg(&patch_path)
        .current_dir(worktree)
        .output()
        .await?;
    let _ = std::fs::remove_file(&patch_path);
    if !apply.status.success() {
        anyhow::bail!(
            "patch failed sandbox apply: {}",
            String::from_utf8_lossy(&apply.stderr)
        );
    }
    Ok(())
}

async fn has_cached_material_diff_at(worktree: &Path) -> Result<bool> {
    let output = tokio::process::Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(worktree)
        .output()
        .await?;
    Ok(!output.status.success())
}

async fn collect_cached_diff_at(worktree: &Path) -> Result<String> {
    let output = tokio::process::Command::new("git")
        .args(["diff", "--cached"])
        .current_dir(worktree)
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "git diff --cached failed: {}",
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
            evidence: None,
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
            evidence: None,
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
        evidence: None,
    })
}

async fn run_regression_tests_at(root: &Path) -> Result<VerificationOutcome> {
    let current_dir = if root.join("professor-x/Cargo.toml").exists() {
        root.join("professor-x")
    } else if root.join("Cargo.toml").exists() {
        root.to_path_buf()
    } else {
        return Ok(VerificationOutcome {
            accepted: true,
            reason: "no Cargo.toml found; regression tests skipped".to_string(),
            checks: vec!["cargo_test_skipped".to_string()],
            evidence: None,
        });
    };

    let output = tokio::process::Command::new("cargo")
        .args(["test", "--quiet", "--bins"])
        .current_dir(current_dir)
        .output()
        .await?;
    if output.status.success() {
        return Ok(VerificationOutcome {
            accepted: true,
            reason: "cargo test --bins passed".to_string(),
            checks: vec!["cargo_test".to_string()],
            evidence: None,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = [stdout.as_ref(), stderr.as_ref()]
        .into_iter()
        .flat_map(|text| text.lines())
        .filter(|line| !line.trim().is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    Ok(VerificationOutcome {
        accepted: false,
        reason: format!("cargo test --bins failed: {detail}"),
        checks: vec!["cargo_test".to_string()],
        evidence: None,
    })
}

fn repo_fix_gate_crate_dir(root: &Path) -> PathBuf {
    if root.join("professor-x/Cargo.toml").exists() {
        root.join("professor-x")
    } else {
        root.to_path_buf()
    }
}

fn subset_repo_fix_manifest(json: &str, limit: usize) -> Result<String> {
    let mut manifest: serde_json::Value = serde_json::from_str(json)?;
    let tasks = manifest
        .get_mut("tasks")
        .and_then(|tasks| tasks.as_array_mut())
        .ok_or_else(|| anyhow::anyhow!("repo-fix manifest missing tasks array"))?;
    tasks.truncate(limit.max(1));
    Ok(serde_json::to_string_pretty(&manifest)?)
}

fn parse_repo_fix_pass_at_1(output: &str) -> Option<f32> {
    output.lines().rev().find_map(|line| {
        line.strip_prefix("pass@1 = ")
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|value| value.parse::<f32>().ok())
    })
}

async fn run_repo_fix_gate_binary(
    binary: &Path,
    current_dir: &Path,
    manifest_path: &Path,
    model: &str,
) -> Result<f32> {
    let temp_root = std::env::temp_dir().join(format!("px-repofix-gate-{}", uuid::Uuid::new_v4()));
    let data_dir = temp_root.join("data");
    let events_dir = temp_root.join("events");
    let transcripts_dir = temp_root.join("transcripts");
    let artifacts_dir = temp_root.join("artifacts");
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&events_dir)?;
    std::fs::create_dir_all(&transcripts_dir)?;
    std::fs::create_dir_all(&artifacts_dir)?;

    let output = tokio::process::Command::new(binary)
        .args(["--repo-fix-bench", "--model", model])
        .current_dir(current_dir)
        .env("PROFESSOR_X_DATA_DIR", &data_dir)
        .env("PROFESSOR_X_EVENT_LOG_DIR", &events_dir)
        .env("PROFESSOR_X_TRANSCRIPT_DIR", &transcripts_dir)
        .env("PROFESSOR_X_ARTIFACT_REPORT_DIR", &artifacts_dir)
        .env("REPO_FIX_TASKS", manifest_path)
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let score = parse_repo_fix_pass_at_1(&stdout).or_else(|| parse_repo_fix_pass_at_1(&stderr));
    let _ = std::fs::remove_dir_all(&temp_root);

    if let Some(score) = score {
        return Ok(score);
    }

    let detail = [stdout.as_ref(), stderr.as_ref()]
        .into_iter()
        .flat_map(|text| text.lines())
        .filter(|line| !line.trim().is_empty())
        .take(10)
        .collect::<Vec<_>>()
        .join(" ");
    anyhow::bail!("repo-fix bench did not report pass@1: {detail}");
}

async fn measure_repo_fix_empirical_gate(
    repo_root: &Path,
    diff: &str,
    model: &str,
) -> Result<EmpiricalVerificationEvidence> {
    let crate_dir = repo_fix_gate_crate_dir(repo_root);
    let manifest_json =
        std::fs::read_to_string(crate_dir.join("scripts/benchmarks/repo_fix/tasks.json"))?;
    let limited_manifest = subset_repo_fix_manifest(&manifest_json, REPO_FIX_GATE_TASK_LIMIT)?;
    let manifest_path =
        std::env::temp_dir().join(format!("px-repofix-subset-{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&manifest_path, limited_manifest)?;

    let baseline_binary = std::env::current_exe()?;
    let baseline_score =
        run_repo_fix_gate_binary(&baseline_binary, &crate_dir, &manifest_path, model).await?;

    let worktree = std::env::temp_dir().join(format!("px-evolve-measure-{}", uuid::Uuid::new_v4()));
    let add = tokio::process::Command::new("git")
        .args(["worktree", "add", "--detach"])
        .arg(&worktree)
        .arg("HEAD")
        .current_dir(repo_root)
        .output()
        .await?;
    if !add.status.success() {
        let _ = std::fs::remove_file(&manifest_path);
        anyhow::bail!(
            "git worktree add failed for empirical gate: {}",
            String::from_utf8_lossy(&add.stderr)
        );
    }

    let outcome = async {
        let patch_path =
            std::env::temp_dir().join(format!("px-empirical-verify-{}.diff", uuid::Uuid::new_v4()));
        std::fs::write(&patch_path, diff)?;
        let apply = tokio::process::Command::new("git")
            .args(["apply", "--recount", "-C1"])
            .arg(&patch_path)
            .current_dir(&worktree)
            .output()
            .await?;
        let _ = std::fs::remove_file(&patch_path);
        if !apply.status.success() {
            anyhow::bail!(
                "empirical gate patch apply failed: {}",
                String::from_utf8_lossy(&apply.stderr)
            );
        }

        let candidate_crate_dir = repo_fix_gate_crate_dir(&worktree);
        let build = tokio::process::Command::new("cargo")
            .args(["build", "--bins", "--quiet"])
            .current_dir(&candidate_crate_dir)
            .output()
            .await?;
        if !build.status.success() {
            anyhow::bail!(
                "empirical gate cargo build failed: {}",
                String::from_utf8_lossy(&build.stderr)
            );
        }

        let candidate_binary = candidate_crate_dir.join("target/debug/professor-x");
        let candidate_score = run_repo_fix_gate_binary(
            &candidate_binary,
            &candidate_crate_dir,
            &manifest_path,
            model,
        )
        .await?;
        Ok::<EmpiricalVerificationEvidence, anyhow::Error>(empirical_repo_fix_evidence(
            REPO_FIX_GATE_TASK_LIMIT,
            baseline_score,
            candidate_score,
        ))
    }
    .await;

    let cleanup = cleanup_worktree(repo_root, &worktree).await;
    let _ = std::fs::remove_file(&manifest_path);
    if let Err(err) = cleanup {
        warn!(
            "evolved: failed to clean empirical gate worktree {}: {err}",
            worktree.display()
        );
    }
    outcome
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
    // The evolution cycle only requires that SOURCE / harness files are clean
    // before it mutates them. The entire artifacts/ tree is runtime output
    // (HIRO attempts, transcripts, commands, evolution reports, ...) and must
    // not block autonomous mutation, so the whole tree is excluded.
    let out = tokio::process::Command::new("git")
        .args([
            "status",
            "--porcelain",
            "--",
            ".",
            ":!professor-x/artifacts",
            ":!artifacts",
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
    // Only MODIFICATIONS TO TRACKED files block autonomous mutation — they'd
    // corrupt the commit. Untracked files ("??") are almost always stray
    // outputs a benchmark task wrote into the workspace; they are harmless to
    // the mutation and must not block it. Count only non-"??" status lines.
    let dirty_tracked = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| !line.trim_start().starts_with("??"))
        .any(|line| !line.trim().is_empty());
    Ok(!dirty_tracked)
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

/// Drain accumulated FED samples from a `ReactLoop` and persist a `FedRecord`
/// to `memory.free_energy` (H15 — Free Energy Delta trajectory).
///
/// Call once after each HIRO round or supervised-loop session completes.
/// A noop if no samples have been collected (e.g. the loop ran no tasks).
pub fn flush_fed_to_memory(
    react: &crate::agentd::react::ReactLoop,
    memory: &MemoryManager,
    round: u32,
    session_id: &str,
) {
    let samples = react.drain_fed_samples();
    if samples.is_empty() {
        return;
    }
    let (mae, n) = compute_fed(&samples);
    let record = FedRecord::new(session_id, round, n as u32, mae);
    if let Err(e) = memory.free_energy.append(&record) {
        warn!("evolved: FED flush failed: {e}");
    } else {
        info!("evolved: FED flushed — round={round} n={n} mae={mae:.4}");
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

        // ── Ratchet: retire low-quality skills before proposing ──────────
        // arXiv:2605.22148 — WITHOUT retire_skill: +0.0pp. WITH: +0.328pp.
        match self.memory.procedural.retire_low_quality(5, 0.30) {
            Ok(retired) if !retired.is_empty() => {
                info!(
                    "evolved: Ratchet retired {} low-quality skill(s): {:?}",
                    retired.len(),
                    retired
                );
                self.emit_event(
                    "evolution.ratchet_retired",
                    format!("Ratchet retired {} low-quality skill(s)", retired.len()),
                    serde_json::json!({ "retired": retired }),
                );
            }
            Ok(_) => {}
            Err(e) => warn!("evolved: Ratchet retirement check failed: {e}"),
        }

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
        let candidates = self
            .node_db
            .sample_ucb1(12)?
            .into_iter()
            .filter(|node| is_reusable_autonomy_target(&node.target_component))
            .take(3)
            .collect::<Vec<_>>();

        // Generate 3 proposals, run Elo tournament, commit winner
        // (Co-Scientist pattern — arXiv:2502.18864)
        let proposals = self
            .researcher_propose_tournament(&failure_patterns, &candidates, success_rate)
            .await?;
        // Keep only proposals whose component can actually be applied
        // autonomously (SystemPrompt / HarnessConfig / SkillDefinition). A
        // ToolDescription / ProceduralMemory / Middleware winner is a no-op —
        // filtering before the tournament guarantees the winner is applyable.
        let proposals: Vec<EvolutionNode> = proposals
            .into_iter()
            .filter(|n| is_autonomously_applyable(&n.target_component))
            .filter(|n| is_reusable_autonomy_target(&n.target_component))
            .collect();
        if proposals.is_empty() {
            info!(
                "evolved: no applyable reusable proposals (all targeted non-mutable or synthetic components)"
            );
            return Ok(false);
        }
        let mut node = if proposals.len() == 1 {
            info!("evolved: single applyable proposal — skipping tournament");
            proposals.into_iter().next().unwrap()
        } else {
            self.elo_tournament(proposals).await?
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

        // ── Self-authored test: store alongside accepted proposals ───────────
        // The Researcher included TEST_* fields in its output. Parse and persist
        // so the agent-authored benchmark grows with every accepted evolution.
        if node.status == crate::evolved::proposer::NodeStatus::Accepted {
            let current_round = tracker.len() as u32;
            let (layer, _lever) = parse_dhe_from_patterns(&failure_patterns);
            let primary_pattern = failure_patterns
                .first()
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            // We re-derive the test from the node's diff text (contains the full Researcher output)
            if let Some(test) =
                Self::parse_self_authored_test(&node.diff, current_round, layer, primary_pattern)
            {
                match self.memory.self_authored_tests.insert(&test) {
                    Ok(id) => {
                        info!(
                            "evolved: self-authored test #{id} written (round={current_round}, layer={layer}): {}",
                            test.description.chars().take(80).collect::<String>()
                        );
                        self.emit_event(
                            "evolution.test_authored",
                            format!("self-authored test #{id} for layer {layer} failure"),
                            serde_json::json!({
                                "test_id": id,
                                "origin_round": current_round,
                                "origin_layer": layer,
                                "description": test.description,
                                "category": test.category,
                            }),
                        );
                    }
                    Err(e) => warn!("evolved: failed to store self-authored test: {e}"),
                }
            }
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
                    format!(
                        "committed accepted evolution proposal {}",
                        commit.as_deref().unwrap_or("without-new-commit")
                    ),
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

        // ── Self-model update every 10 rounds (H14/H15) ──────────────────
        let current_round = tracker.len() as u32;
        if current_round > 0 && current_round % 10 == 0 {
            self.maybe_update_self_model(current_round, tracker).await;
        }

        // ── Sleep consolidation (Seed 3 — CLS) ───────────────────────────
        // The agent "sleeps" after each cycle: replay episodics, extract
        // cross-task patterns, promote to semantic memory, decay stale traces.
        match crate::evolved::sleep::consolidate(&self.memory, &self.ollama, current_round).await {
            Ok(report) => self.emit_event(
                "evolution.consolidated",
                format!(
                    "sleep consolidation: {} promoted, {} decayed",
                    report.promoted_to_semantic, report.episodics_decayed
                ),
                serde_json::json!({
                    "replayed": report.episodics_replayed,
                    "patterns": report.patterns_extracted,
                    "promoted": report.promoted_to_semantic,
                    "decayed": report.episodics_decayed,
                }),
            ),
            Err(e) => warn!("evolved: sleep consolidation failed: {e}"),
        }

        // ── Default Mode Network (Seed 5) ────────────────────────────────
        // Between cycles, the agent wanders: free-associates across disparate
        // memories for unexpected insight, and simulates its own near future.
        // Insights become cognition items the Researcher draws on next cycle.
        match crate::evolved::dmn::wander(&self.memory, &self.ollama, current_round).await {
            Ok(report) => {
                if report.insights_kept > 0 || report.simulations > 0 {
                    self.emit_event(
                        "evolution.dmn_wander",
                        format!(
                            "default mode: {} insight(s), {} simulation(s)",
                            report.insights_kept, report.simulations
                        ),
                        serde_json::json!({
                            "fragments": report.fragments_sampled,
                            "insights": report.insights_kept,
                            "simulations": report.simulations,
                        }),
                    );
                }
            }
            Err(e) => warn!("evolved: default mode wander failed: {e}"),
        }

        Ok(true)
    }

    /// Generate up to 3 distinct proposals, returning all that parse successfully.
    /// Called in place of the old single-proposal `researcher_propose`.
    async fn researcher_propose_tournament(
        &self,
        failure_patterns: &[String],
        candidates: &[EvolutionNode],
        success_rate: f32,
    ) -> Result<Vec<EvolutionNode>> {
        let (layer, _lever) = parse_dhe_from_patterns(failure_patterns);
        let hints = diagnostic_component_hints(layer);
        let mut proposals = Vec::with_capacity(hints.len());

        for (i, diversity_hint) in hints.iter().enumerate() {
            match self
                .researcher_propose_with_hint(
                    failure_patterns,
                    candidates,
                    success_rate,
                    diversity_hint,
                )
                .await
            {
                Ok(Some(node)) => {
                    info!(
                        "evolved: proposal {}/{} — {:?}: {}",
                        i + 1,
                        hints.len(),
                        node.target_component,
                        node.motivation.chars().take(60).collect::<String>()
                    );
                    proposals.push(node);
                }
                Ok(None) => info!(
                    "evolved: proposal {}/{} — no actionable output",
                    i + 1,
                    hints.len()
                ),
                Err(e) => warn!("evolved: proposal {}/{} failed: {e}", i + 1, hints.len()),
            }
        }

        Ok(proposals)
    }

    /// Run Elo tournament: every pair compared once, winner has highest Elo.
    /// K=32, initial rating=1200. Returns the winner node.
    async fn elo_tournament(&self, mut proposals: Vec<EvolutionNode>) -> Result<EvolutionNode> {
        let n = proposals.len();
        let mut ratings = vec![1200.0f32; n];
        const K: f32 = 32.0;

        for i in 0..n {
            for j in (i + 1)..n {
                let score = self
                    .elo_compare(&proposals[i], &proposals[j])
                    .await
                    .unwrap_or(0.5); // tie on error

                // Expected score for i against j
                let exp_i = 1.0 / (1.0 + 10.0f32.powf((ratings[j] - ratings[i]) / 400.0));
                let exp_j = 1.0 - exp_i;

                ratings[i] += K * (score - exp_i);
                ratings[j] += K * ((1.0 - score) - exp_j);
            }
        }

        let winner_idx = ratings
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        info!(
            "evolved: Elo tournament — winner proposal {} (rating={:.0}) of {}",
            winner_idx + 1,
            ratings[winner_idx],
            n
        );
        self.emit_event(
            "evolution.elo_winner",
            format!(
                "Elo winner: proposal {} (rating={:.0})",
                winner_idx + 1,
                ratings[winner_idx]
            ),
            serde_json::json!({
                "winner_idx": winner_idx,
                "ratings": ratings,
                "motivation": proposals[winner_idx].motivation,
            }),
        );

        Ok(proposals.swap_remove(winner_idx))
    }

    /// Ask the LLM which of two proposals is better.
    /// Returns 1.0 if A wins, 0.0 if B wins, 0.5 on tie/error.
    async fn elo_compare(&self, a: &EvolutionNode, b: &EvolutionNode) -> Result<f32> {
        let prompt = format!(
            "Two harness improvement proposals are competing. \
             Judge which is more likely to improve the agent's task success rate.\n\n\
             Proposal A:\n  Component: {:?}\n  Motivation: {}\n  Root cause: {}\n\n\
             Proposal B:\n  Component: {:?}\n  Motivation: {}\n  Root cause: {}\n\n\
             Answer with exactly one word: A or B",
            a.target_component,
            a.motivation.chars().take(120).collect::<String>(),
            a.manifest.root_cause.chars().take(120).collect::<String>(),
            b.target_component,
            b.motivation.chars().take(120).collect::<String>(),
            b.manifest.root_cause.chars().take(120).collect::<String>(),
        );

        let resp = self
            .ollama
            .generate(
                &prompt,
                Some("You are a research judge. Be decisive."),
                Some(ModelOptions {
                    temperature: Some(0.1),
                    num_ctx: Some(2048),
                    top_p: Some(0.9),
                    stop: None,
                    think: Some(false),
                }),
            )
            .await?;

        let (_, answer) = resp.split_thinking();
        let trimmed = answer.trim().to_uppercase();
        Ok(if trimmed.starts_with('A') {
            1.0
        } else if trimmed.starts_with('B') {
            0.0
        } else {
            0.5 // unclear → tie
        })
    }

    async fn researcher_propose(
        &self,
        failure_patterns: &[String],
        candidates: &[EvolutionNode],
        success_rate: f32,
    ) -> Result<Option<EvolutionNode>> {
        self.researcher_propose_with_hint(failure_patterns, candidates, success_rate, "")
            .await
    }

    async fn researcher_propose_with_hint(
        &self,
        failure_patterns: &[String],
        candidates: &[EvolutionNode],
        success_rate: f32,
        diversity_hint: &str,
    ) -> Result<Option<EvolutionNode>> {
        // Retrieve top cognition items — prefer semantic search, fallback to keyword
        let query_text = format!(
            "harness improvement {} failure",
            failure_patterns
                .first()
                .map(|s| s.as_str())
                .unwrap_or("unknown")
        );
        let cognition_items = if let Ok(vec) = self.ollama.embed(&query_text).await {
            let emb_store = crate::embeddings::EmbeddingStore::new(Arc::clone(&self.memory.db));
            let semantic = self
                .cognition
                .search_semantic(&emb_store, &vec, 5)
                .unwrap_or_default();
            if semantic.is_empty() {
                self.cognition
                    .query_top_k(&query_text, 5)
                    .unwrap_or_default()
            } else {
                semantic
            }
        } else {
            self.cognition
                .query_top_k(&query_text, 5)
                .unwrap_or_default()
        };
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

        let diversity_section = if diversity_hint.is_empty() {
            String::new()
        } else {
            format!("\nInstruction: {diversity_hint}\n")
        };

        let prompt = format!(
            "You are the Researcher in an autonomous self-improvement loop.\n\n\
             Current state:\n\
             - Success rate (last 20 tasks): {success_rate:.0}%\n\
             - Failure patterns: {}\n\n\
             Prior evolution nodes (UCB1 sampled):\n{candidates_text}\n\n\
             Knowledge base:\n{cognition_context}\n\
             {diversity_section}\n\
             Propose ONE specific harness improvement. It MUST target one of these\n\
             three applyable components (any other component cannot be applied and\n\
             will be discarded):\n\
             - SystemPrompt: the system prompt injected before every task\n\
             - SkillDefinition(name): a reusable skill in skills/ (new or revised)\n\
             - HarnessConfig: the config/hardware.toml settings\n\n\
             Do NOT propose ToolDescription changes (not applyable yet), nor changes\n\
             to policyd gate logic or memd internals (require human approval).\n\n\
             Respond in this exact format:\n\
             COMPONENT: <SystemPrompt|SkillDefinition:<name>|HarnessConfig>\n\
             MOTIVATION: <one sentence why this change will help>\n\
             ROOT_CAUSE: <which failure mode this addresses>\n\
             FIX:\n\
             <For SystemPrompt: write ONLY the new guidance to ADD (2-5 sentences of \
             concrete instruction addressing the failure pattern) — do NOT rewrite or \
             restate your identity; it is preserved automatically and your addition is \
             appended. For HarnessConfig: the complete replacement config file. For \
             SkillDefinition: a complete markdown skill with '# <name>', Purpose, \
             Workflow, and Output Contract.>\n\
             PREDICTS_FIX: <what task type should improve>\n\
             PREDICTS_REGRESSION: <what might get worse, or 'none'>\n\n\
             Also propose a NEW TEST that would catch the failure class you just diagnosed.\n\
             A test is a concrete task description an agent could attempt, plus a clear pass criterion.\n\
             TEST_DESCRIPTION: <a specific task the agent should complete to demonstrate the fix worked>\n\
             TEST_EVALUATOR: <how to tell if the agent passed: what output or state counts as success>\n\
             TEST_CATEGORY: <tool_use|planning|self_correction>",
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
        let mut node = match self.parse_researcher_output(&answer)? {
            Some(node) => node,
            None => return Ok(None),
        };
        let (layer, lever) = parse_dhe_from_patterns(failure_patterns);
        if node.manifest.evidence_cited.is_empty() {
            node.manifest.evidence_cited = failure_patterns.iter().take(3).cloned().collect();
        }
        if node.manifest.root_cause.trim().is_empty() {
            node.manifest.root_cause = failure_patterns
                .first()
                .cloned()
                .unwrap_or_else(|| "no recorded failure pattern".to_string());
        }
        if layer != 0 && !node.manifest.root_cause.contains("[DHE:layer=") {
            node.manifest.root_cause = format!(
                "[DHE:layer={layer},lever={lever}] {}",
                node.manifest.root_cause
            );
        }
        Ok(Some(node))
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

    /// Parse self-authored test fields from Researcher output.
    fn parse_self_authored_test(
        text: &str,
        origin_round: u32,
        origin_layer: u8,
        failure_pattern: &str,
    ) -> Option<SelfAuthoredTest> {
        let description = extract_field(text, "TEST_DESCRIPTION")?;
        let evaluator = extract_field(text, "TEST_EVALUATOR")?;
        if description.trim().is_empty() || evaluator.trim().is_empty() {
            return None;
        }
        let category = extract_field(text, "TEST_CATEGORY").unwrap_or_else(|| "other".to_string());
        Some(SelfAuthoredTest::new(
            origin_round,
            origin_layer,
            failure_pattern,
            description,
            evaluator,
            category,
        ))
    }

    async fn verify_then_apply(
        &self,
        node: &mut EvolutionNode,
        tracker: &OutcomeTracker,
    ) -> Result<()> {
        let failure_patterns = tracker.failure_patterns(20);
        let (pred_layer, pred_lever) = parse_dhe_from_patterns(&failure_patterns);

        if pred_layer != 0
            && !component_matches_diagnostic_layer(&node.target_component, pred_layer)
        {
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
            node.manifest.verification_status = VerificationStatus::Rejected;
            node.analysis = format!(
                "proposal targets {:?}, but dominant diagnostic layer {} / lever {} requires {}",
                node.target_component,
                pred_layer,
                pred_lever,
                expected_component_summary(pred_layer)
            );
            node.results = serde_json::to_value(VerificationOutcome {
                accepted: false,
                reason: node.analysis.clone(),
                checks: vec!["dhe_component_alignment".to_string()],
                evidence: None,
            })?;
            return Ok(());
        }

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
                evidence: None,
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
                evidence: None,
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

        let mut verification_outcome = verification.outcome.clone();
        match measure_repo_fix_empirical_gate(&repo_root, &verification.diff, self.ollama.model())
            .await
        {
            Ok(evidence) => {
                verification_outcome
                    .checks
                    .push("repo_fix_empirical_gate".to_string());
                let evidence_summary = format!(
                    "repo-fix subset pass@1 baseline {:.3} candidate {:.3} delta {:+.3} on {} task(s)",
                    evidence.baseline_score,
                    evidence.candidate_score,
                    evidence.score_delta,
                    evidence.task_count
                );
                verification_outcome.evidence = Some(evidence.clone());
                if !evidence.passed {
                    node.status = crate::evolved::proposer::NodeStatus::Rejected;
                    node.manifest.verification_status = VerificationStatus::Rejected;
                    node.manifest.verified_at = Some(Utc::now());
                    node.analysis =
                        format!("empirical repo-fix gate rejected proposal: {evidence_summary}");
                    verification_outcome.accepted = false;
                    verification_outcome.reason = node.analysis.clone();
                    node.results = serde_json::to_value(verification_outcome)?;
                    return Ok(());
                }
                verification_outcome.reason =
                    format!("{}; {evidence_summary}", verification_outcome.reason);
            }
            Err(err) => {
                node.status = crate::evolved::proposer::NodeStatus::Rejected;
                node.manifest.verification_status = VerificationStatus::Rejected;
                node.manifest.verified_at = Some(Utc::now());
                node.analysis = format!("empirical repo-fix gate failed: {err}");
                verification_outcome.accepted = false;
                verification_outcome
                    .checks
                    .push("repo_fix_empirical_gate".to_string());
                verification_outcome.reason = node.analysis.clone();
                verification_outcome.evidence = None;
                node.results = serde_json::to_value(verification_outcome)?;
                return Ok(());
            }
        }
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
        node.score = verification_outcome
            .evidence
            .as_ref()
            .map(|evidence| {
                ((recent_success.max(0.1) + evidence.candidate_score.max(0.0)) / 2.0)
                    .clamp(0.0, 1.0)
            })
            .unwrap_or_else(|| (node.score + recent_success.max(0.1)) / 2.0);
        node.results = serde_json::to_value(verification_outcome)?;
        node.manifest.verification_status = VerificationStatus::Confirmed;
        node.manifest.verified_at = Some(Utc::now());

        // Write lesson to cognition base and embed it for future semantic retrieval
        if !lesson.is_empty() {
            let node_id = node.id.unwrap_or(0) as u64;
            let item = Analyzer::to_cognition_item(&lesson, node_id);
            self.cognition.insert(&item)?;
            info!("evolved: Analyzer wrote new cognition item");
            let emb_store = crate::embeddings::EmbeddingStore::new(Arc::clone(&self.memory.db));
            crate::embeddings::embed_and_store(
                &self.ollama,
                &emb_store,
                "cognition",
                &item.id.to_string(),
                &item.content,
            )
            .await;
        }

        // Record DHE attribution into the metacognitive store. The entry is
        // left UNVERIFIED (attribution_correct=false, actual_improvement=0.0)
        // — the next HIRO round flips those fields via
        // `MetacognitiveStore::verify_round` once a real pass@3 delta exists.
        //
        // The bare INSERT this replaces hardcoded round=0 and
        // attribution_correct=1 regardless of outcome, which made MCA
        // computation meaningless. The round used here is the HIRO round at
        // attribution time when the runner supplies it; otherwise the
        // tracker-derived count is the best proxy available.
        let component_name = format!("{:?}", node.target_component);
        let metacog_store = MetacognitiveStore::new(Arc::clone(&self.memory.db));
        // The loop runner doesn't carry an explicit HIRO-round counter at
        // this site; the outcome tracker's length is a stable monotonic
        // proxy that orders attributions correctly for verify_round even
        // when it doesn't match the actual HIRO round number 1-for-1.
        let current_round = tracker.len() as u32;
        let entry = MetacognitiveEntry::new(
            current_round,
            component_name,
            pred_layer,
            pred_lever,
            node.score,
        );
        if let Err(e) = metacog_store.append(&entry) {
            warn!("evolved: failed to append metacognitive entry: {e}");
        }

        Ok(())
    }

    async fn git_worktree_clean(&self) -> Result<bool> {
        git_worktree_clean_at(&default_repo_root()).await
    }

    /// Update the Strange Loop self-model snapshot via LLM (H14).
    /// Called every 10 rounds from run_cycle. Skips silently on Ollama error
    /// so a transient failure never blocks the evolution cycle.
    async fn maybe_update_self_model(&self, round: u32, tracker: &OutcomeTracker) {
        let prior = match self.memory.self_model.latest() {
            Ok(Some(snap)) => snap,
            Ok(None) => {
                info!("evolved: self-model has no baseline snapshot; skipping update at round {round}");
                return;
            }
            Err(e) => {
                warn!("evolved: failed to load self-model snapshot: {e}");
                return;
            }
        };

        let success_rate = tracker.success_rate(20);
        let failure_patterns = tracker.failure_patterns(20);
        let behavior_summary = format!(
            "success rate over the last 20 tasks: {:.0}%. \
             Main failure patterns: {}.",
            success_rate * 100.0,
            if failure_patterns.is_empty() {
                "none observed".to_string()
            } else {
                failure_patterns.join(", ")
            }
        );

        let prompt = crate::memd::self_model::SelfModelStore::build_update_prompt(
            &prior.text,
            round,
            &behavior_summary,
        );

        let resp = match self
            .ollama
            .generate(
                &prompt,
                Some("You are Professor X. Update your self-description concisely."),
                Some(ModelOptions::for_reflection()),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("evolved: self-model update LLM call failed: {e}");
                return;
            }
        };

        let (_, text) = resp.split_thinking();
        let text = text.trim().to_string();
        if text.is_empty() {
            warn!("evolved: self-model update response was empty at round {round}");
            return;
        }

        match self.memory.self_model.update_with_text(round, text) {
            Ok(snap) => {
                info!(
                    "evolved: self-model updated at round {round} (id={:?})",
                    snap.id
                );
                self.emit_event(
                    "evolution.self_model_updated",
                    format!("self-model updated at round {round}"),
                    serde_json::json!({ "round": round, "snapshot_id": snap.id }),
                );

                // ── ICS: measure drift from round-0 baseline (H14) ──────────
                self.compute_and_record_ics(round, &snap.text).await;

                // ── Narrative self (Seed 6): add the next chapter ───────────
                self.append_narrative_chapter(round, &behavior_summary)
                    .await;
            }
            Err(e) => warn!("evolved: failed to persist self-model update: {e}"),
        }
    }

    /// Seed 6 — narrative self: every self-model update also adds a chapter to
    /// Professor X's autobiographical story, connected to prior chapters by
    /// theme. The agent checks whether its last anticipated arc came true (FED
    /// at the narrative level) and projects where the story heads next.
    async fn append_narrative_chapter(&self, round: u32, behavior_summary: &str) {
        let prior_recap = self.memory.narrative.story_recap(5).unwrap_or_default();
        let prior_arc = self
            .memory
            .narrative
            .latest()
            .ok()
            .flatten()
            .map(|e| e.anticipated_arc)
            .unwrap_or_default();

        let prompt = crate::memd::narrative::build_narrative_prompt(
            &prior_recap,
            &prior_arc,
            round,
            behavior_summary,
        );

        let resp = match self
            .ollama
            .generate(
                &prompt,
                Some("You are Professor X narrating your own research journey. Be honest and reflective."),
                Some(ModelOptions::for_reflection()),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("evolved: narrative chapter LLM call failed: {e}");
                return;
            }
        };

        let (_, text) = resp.split_thinking();
        match crate::memd::narrative::parse_episode(&text, round) {
            Some(episode) => match self.memory.narrative.append(&episode) {
                Ok(id) => {
                    info!(
                        "evolved: narrative chapter {round} added — '{}'",
                        episode.chapter
                    );
                    self.emit_event(
                        "evolution.narrative_chapter",
                        format!(
                            "new life-story chapter at round {round}: {}",
                            episode.chapter
                        ),
                        serde_json::json!({
                            "round": round,
                            "episode_id": id,
                            "chapter": episode.chapter,
                            "anticipated_arc": episode.anticipated_arc,
                        }),
                    );
                }
                Err(e) => warn!("evolved: failed to persist narrative chapter: {e}"),
            },
            None => warn!("evolved: narrative response unparseable at round {round}"),
        }
    }

    /// Embed the new snapshot and the round-0 baseline, compute cosine ICS,
    /// persist the record, and warn at the alert (0.70) and halt (0.50) thresholds.
    async fn compute_and_record_ics(&self, round: u32, new_text: &str) {
        let baseline = match self.memory.self_model.at_round(0) {
            Ok(Some(b)) => b,
            Ok(None) => {
                warn!("evolved: ICS skipped — no round-0 baseline snapshot");
                return;
            }
            Err(e) => {
                warn!("evolved: ICS skipped — baseline load failed: {e}");
                return;
            }
        };

        let (vec_new, vec_base) = match tokio::try_join!(
            self.ollama.embed(new_text),
            self.ollama.embed(&baseline.text),
        ) {
            Ok(pair) => pair,
            Err(e) => {
                warn!("evolved: ICS skipped — embedding failed: {e}");
                return;
            }
        };

        let score = crate::memd::ics::compute_ics(&vec_new, &vec_base);
        let record = crate::memd::ics::IcsRecord::new(0, round, score);

        match self.memory.ics.append(&record) {
            Ok(_) => {
                info!("evolved: ICS round {round} vs baseline = {score:.3}");
                if score < 0.50 {
                    warn!(
                        "evolved: ICS BELOW HALT THRESHOLD (0.50) at round {round}: {score:.3} \
                         — identity drift is severe"
                    );
                } else if score < 0.70 {
                    warn!("evolved: ICS below alert threshold (0.70) at round {round}: {score:.3}");
                }
                self.emit_event(
                    "evolution.ics_recorded",
                    format!("ICS at round {round} = {score:.3}"),
                    serde_json::json!({ "round": round, "ics": score,
                        "alert": score < 0.70, "halt": score < 0.50 }),
                );
            }
            Err(e) => warn!("evolved: ICS record write failed: {e}"),
        }
    }

    async fn commit_node(&self, node: &EvolutionNode) -> Result<Option<String>> {
        let repo_root = default_repo_root();
        let paths = changed_paths_for_node_at(&repo_root, node);
        if paths.is_empty() {
            warn!("evolved: accepted node has no known changed paths; skipping commit");
            return Ok(None);
        }

        let mut add = tokio::process::Command::new("git");
        add.arg("add").current_dir(&repo_root);
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
            .current_dir(&repo_root)
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
        Ok(Some(git_head(&repo_root).await?))
    }

    fn emit_event(&self, event_type: &str, summary: impl AsRef<str>, payload: serde_json::Value) {
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
        std::fs::create_dir_all(root.join("skills/conductor")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"px-evolve-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/main.rs"),
            "fn ok() -> bool {\n    true\n}\n\nfn main() {\n    println!(\"{}\", ok());\n}\n\n#[cfg(test)]\nmod tests {\n    use super::ok;\n\n    #[test]\n    fn ok_stays_true() {\n        assert!(ok());\n    }\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("skills/conductor/existing.md"),
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

    #[test]
    fn subset_repo_fix_manifest_limits_task_count() {
        let manifest = r#"{
          "tasks": [
            {"id": "fix_001"},
            {"id": "fix_002"},
            {"id": "fix_003"}
          ]
        }"#;

        let subset = subset_repo_fix_manifest(manifest, 2).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&subset).unwrap();

        assert_eq!(parsed["tasks"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["tasks"][0]["id"], "fix_001");
        assert_eq!(parsed["tasks"][1]["id"], "fix_002");
    }

    #[test]
    fn parse_repo_fix_pass_at_1_reads_bench_output() {
        let output = "repo-fix fix_001 pre=1 post=0 -> PASS\n\n=== REPO-FIX BENCH ===\npass@1 = 0.750  (3/4 tasks)\n";

        assert_eq!(parse_repo_fix_pass_at_1(output), Some(0.75));
    }

    #[test]
    fn empirical_repo_fix_evidence_rejects_regression() {
        let evidence = empirical_repo_fix_evidence(4, 0.75, 0.50);

        assert_eq!(evidence.benchmark, "repo_fix_subset");
        assert_eq!(evidence.score_delta, -0.25);
        assert!(!evidence.passed);
    }

    #[test]
    fn empirical_repo_fix_evidence_allows_no_regression() {
        let evidence = empirical_repo_fix_evidence(4, 0.50, 0.50);

        assert!(evidence.passed);
        assert_eq!(evidence.score_delta, 0.0);
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
        assert!(verified
            .outcome
            .checks
            .iter()
            .any(|check| check == "cargo_test"));
        assert!(verified.diff.contains("skills/conductor/fallback.md"));
        assert!(!root.join("skills/conductor/fallback.md").exists());

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
    async fn patch_sandbox_verifier_accepts_safe_diff() {
        let root = temp_git_repo();
        let patch = "diff --git a/skills/conductor/existing.md b/skills/conductor/existing.md\n--- a/skills/conductor/existing.md\n+++ b/skills/conductor/existing.md\n@@ -1 +1,2 @@\n When a tool fails, inspect the observation and retry with a narrower input.\n+Record the fallback reason so later review can compare the failure pattern.\n";

        let verified = verify_diff_in_sandbox(&root, patch).await.unwrap();

        assert!(verified.outcome.accepted, "{}", verified.outcome.reason);
        assert!(verified
            .outcome
            .checks
            .iter()
            .any(|check| check == "cargo_test"));
        assert!(verified.diff.contains("Record the fallback reason"));
        let original = std::fs::read_to_string(root.join("skills/conductor/existing.md")).unwrap();
        assert!(!original.contains("Record the fallback reason"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn patch_sandbox_verifier_rejects_noop_text() {
        let root = temp_git_repo();

        let verified = verify_diff_in_sandbox(&root, "# comment only\n// no material change\n")
            .await
            .unwrap();

        assert!(!verified.outcome.accepted);
        assert!(verified.outcome.reason.contains("no-op"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn patch_sandbox_verifier_rejects_failing_bin_tests() {
        let root = temp_git_repo();
        let patch = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,6 +1,6 @@\n fn ok() -> bool {\n-    true\n+    false\n }\n \n fn main() {\n     println!(\"{}\", ok());\n";

        let verified = verify_diff_in_sandbox(&root, patch).await.unwrap();

        assert!(!verified.outcome.accepted);
        assert!(verified.outcome.reason.contains("cargo test --bins failed"));
        assert!(verified
            .outcome
            .checks
            .iter()
            .any(|check| check == "cargo_test"));

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

        std::fs::write(root.join("src/main.rs"), "fn ok() -> bool { false }\n").unwrap();
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
    fn reusable_autonomy_target_rejects_ephemeral_operator_skills() {
        assert!(!is_reusable_autonomy_target(
            &HarnessComponent::SkillDefinition(
                "px-operator-goal-20260616-visible-work".to_string(),
            )
        ));
        assert!(!is_reusable_autonomy_target(
            &HarnessComponent::SkillDefinition("px-autonomous-patch-20260616-010101".to_string(),)
        ));
    }

    #[test]
    fn reusable_autonomy_target_keeps_real_skill_targets() {
        assert!(is_reusable_autonomy_target(&HarnessComponent::SystemPrompt));
        assert!(is_reusable_autonomy_target(
            &HarnessComponent::HarnessConfig
        ));
        assert!(is_reusable_autonomy_target(
            &HarnessComponent::SkillDefinition("retry-plan-generation".to_string(),)
        ));
    }

    #[test]
    fn dhe_parser_reads_layer_and_lever() {
        let patterns = vec!["failure [DHE:layer=3,lever=2]".to_string()];

        assert_eq!(parse_dhe_from_patterns(&patterns), (3, 2));
    }

    #[test]
    fn dhe_parser_reads_structured_failure_class() {
        let patterns = vec!["[failure:context] output truncated under heavy context".to_string()];

        assert_eq!(parse_dhe_from_patterns(&patterns), (2, 2));
    }

    #[test]
    fn diagnostic_component_alignment_routes_context_failures_to_config() {
        assert!(component_matches_diagnostic_layer(
            &HarnessComponent::HarnessConfig,
            2
        ));
        assert!(!component_matches_diagnostic_layer(
            &HarnessComponent::SystemPrompt,
            2
        ));
        assert!(!component_matches_diagnostic_layer(
            &HarnessComponent::SkillDefinition("retry".to_string()),
            2
        ));
    }

    #[test]
    fn diagnostic_component_hints_focus_reasoning_failures_on_prompt() {
        let hints = diagnostic_component_hints(5);

        assert_eq!(hints.len(), 1);
        assert!(hints[0].contains("SystemPrompt"));
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
        let node = skill_node("retry-plan-generation", "content");

        assert_eq!(
            changed_paths_for_node_at(&root, &node),
            vec![PathBuf::from(
                "professor-x/skills/conductor/retry-plan-generation.md"
            )]
        );
        assert_eq!(
            sanitize_generated_content("```markdown\n# Skill\nbody\n```"),
            "# Skill\nbody\n"
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
