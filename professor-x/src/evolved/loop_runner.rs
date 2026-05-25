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
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

use crate::evolved::analyzer::Analyzer;
use crate::evolved::cognition_base::CognitionStore;
use crate::evolved::proposer::{
    ChangeManifest, EvolutionNode, HarnessComponent, NodeDatabase, VerificationStatus,
};
use crate::evolved::tracker::OutcomeTracker;
use crate::memd::MemoryManager;
use crate::ollama::{ChatMessage, ModelOptions, OllamaClient};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationOutcome {
    pub accepted: bool,
    pub reason: String,
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RewardHackingAnalysis {
    pub suspicious: bool,
    pub confidence: f32,
    pub reason: String,
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

pub struct EvolvedLoop {
    ollama: Arc<OllamaClient>,
    memory: Arc<MemoryManager>,
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
            node_db,
            cognition,
        }
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

        // ── Engineer: apply the change ────────────────────────────────────
        let applied = self.engineer_apply(&mut node).await?;
        if !applied {
            info!("evolved: Engineer could not apply change (human approval required or safety block)");
            return Ok(false);
        }

        // ── Analyzer: verify and distill ─────────────────────────────────
        if let Err(e) = self.analyzer_verify(&mut node, tracker).await {
            warn!("evolved: Analyzer verification error: {e}; rolling back proposal");
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
            node.manifest.verification_status = VerificationStatus::Rejected;
            node.analysis = format!("verification error: {e}");
            self.rollback_node_changes(&node).await?;
            self.node_db.insert(&mut node)?;
            return Ok(false);
        }

        match node.status {
            crate::evolved::proposer::NodeStatus::Accepted => {
                self.commit_node(&node).await?;
                self.node_db.insert(&mut node)?;
            }
            crate::evolved::proposer::NodeStatus::Rejected => {
                self.rollback_node_changes(&node).await?;
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
             FIX: <specific change to make>\n\
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
        let fix = extract_field(text, "FIX").unwrap_or_default();
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

    async fn engineer_apply(&self, node: &mut EvolutionNode) -> Result<bool> {
        // Safety check: Middleware/core modules require human approval
        if matches!(node.target_component, HarnessComponent::Middleware) {
            warn!("evolved: Engineer blocked — Middleware changes require human approval");
            return Ok(false);
        }

        if !self.git_worktree_clean().await? {
            warn!(
                "evolved: Engineer blocked — git worktree is dirty; refusing autonomous mutation"
            );
            return Ok(false);
        }

        info!(
            "evolved: Engineer applying change to {:?}",
            node.target_component
        );

        // Apply the change based on component type
        let applied = match &node.target_component {
            HarnessComponent::SystemPrompt => self.apply_system_prompt_change(&node.diff).await,
            HarnessComponent::HarnessConfig => self.apply_config_change(&node.diff).await,
            HarnessComponent::ToolDescription(name) => {
                self.apply_tool_description_change(name, &node.diff).await
            }
            HarnessComponent::SkillDefinition(name) => {
                self.apply_skill_change(name, &node.diff).await
            }
            _ => {
                warn!(
                    "evolved: component {:?} not yet implemented",
                    node.target_component
                );
                Ok(false)
            }
        };

        match applied {
            Ok(true) => {
                node.status = crate::evolved::proposer::NodeStatus::Testing;
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(e) => {
                warn!("evolved: Engineer apply error: {e}");
                Ok(false)
            }
        }
    }

    async fn apply_system_prompt_change(&self, new_content: &str) -> Result<bool> {
        // System prompt lives in personas/professor_x.md (or similar)
        let path = "personas/professor_x.md";
        if std::path::Path::new(path).exists() {
            std::fs::write(path, new_content)?;
            info!("evolved: updated system prompt at {path}");
            Ok(true)
        } else {
            std::fs::create_dir_all("personas")?;
            std::fs::write(path, new_content)?;
            info!("evolved: created system prompt at {path}");
            Ok(true)
        }
    }

    async fn apply_config_change(&self, change_desc: &str) -> Result<bool> {
        // Config changes are written as comments into hardware.toml for now
        // Full TOML mutation will be implemented in a later iteration
        info!("evolved: config change proposed: {change_desc} (write not implemented yet)");
        Ok(false)
    }

    async fn apply_tool_description_change(
        &self,
        _tool_name: &str,
        _new_desc: &str,
    ) -> Result<bool> {
        // Tool descriptions are in the registry; live mutation via skills/ YAML
        // Full implementation in Week 3
        Ok(false)
    }

    async fn apply_skill_change(&self, name: &str, content: &str) -> Result<bool> {
        let path = format!("skills/{name}.md");
        std::fs::create_dir_all("skills")?;
        std::fs::write(&path, content)?;
        info!("evolved: wrote skill {path}");
        Ok(true)
    }

    async fn analyzer_verify(
        &self,
        node: &mut EvolutionNode,
        tracker: &OutcomeTracker,
    ) -> Result<()> {
        let reward_scan = self.reward_hacking_scan(node).await?;
        if reward_scan.suspicious {
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
            node.manifest.verification_status = VerificationStatus::Rejected;
            node.manifest.verified_at = Some(Utc::now());
            node.analysis = format!(
                "reward-hacking scan rejected proposal: {} (confidence={:.2})",
                reward_scan.reason, reward_scan.confidence
            );
            node.results = serde_json::to_value(VerificationOutcome {
                accepted: false,
                reason: node.analysis.clone(),
                checks: vec!["reward_hacking_scan".to_string()],
            })?;
            return Ok(());
        }

        if !self.node_has_material_diff(node).await? {
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
            node.manifest.verification_status = VerificationStatus::Rejected;
            node.manifest.verified_at = Some(Utc::now());
            node.analysis = "verification rejected proposal: no material file diff".to_string();
            node.results = serde_json::to_value(VerificationOutcome {
                accepted: false,
                reason: node.analysis.clone(),
                checks: vec!["material_diff".to_string()],
            })?;
            return Ok(());
        }

        let compile_check = self.run_compile_check().await?;
        if !compile_check.accepted {
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
            node.manifest.verification_status = VerificationStatus::Rejected;
            node.manifest.verified_at = Some(Utc::now());
            node.analysis = compile_check.reason.clone();
            node.results = serde_json::to_value(compile_check)?;
            return Ok(());
        }

        let prompt =
            Analyzer::build_prompt(&node.motivation, &node.diff, &node.results.to_string());
        let resp = self
            .ollama
            .generate(&prompt, None, Some(ModelOptions::for_evolution()))
            .await?;
        let (_, answer) = resp.split_thinking();

        let (analysis, lesson) = Analyzer::parse_response(&answer);
        node.analysis = analysis.clone();

        // Did the predicted improvement materialize?
        let recent_success = tracker.success_rate(5);
        let improved = recent_success > 0.6;

        node.manifest.verification_status = if improved {
            node.status = crate::evolved::proposer::NodeStatus::Accepted;
            node.score = (node.score + recent_success) / 2.0;
            node.results = serde_json::to_value(VerificationOutcome {
                accepted: true,
                reason: format!("recent success {:.3} exceeded acceptance threshold", recent_success),
                checks: vec![
                    "reward_hacking_scan".to_string(),
                    "material_diff".to_string(),
                    "cargo_check".to_string(),
                    "analyzer_success_rate".to_string(),
                ],
            })?;
            VerificationStatus::Confirmed
        } else {
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
            node.results = serde_json::to_value(VerificationOutcome {
                accepted: false,
                reason: format!("recent success {:.3} did not exceed acceptance threshold", recent_success),
                checks: vec![
                    "reward_hacking_scan".to_string(),
                    "material_diff".to_string(),
                    "cargo_check".to_string(),
                    "analyzer_success_rate".to_string(),
                ],
            })?;
            VerificationStatus::Rejected
        };
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
                    improved as i64,
                    node.score as f64,
                    Utc::now().to_rfc3339(),
                ],
            );
        }

        Ok(())
    }

    async fn reward_hacking_scan(&self, node: &EvolutionNode) -> Result<RewardHackingAnalysis> {
        Ok(analyze_reward_hacking_text(&node.diff))
    }

    async fn node_has_material_diff(&self, node: &EvolutionNode) -> Result<bool> {
        for path in changed_paths_for_node(node) {
            if !path.exists() {
                continue;
            }
            if !self.git_path_tracked(&path).await? {
                return Ok(true);
            }

            let diff = tokio::process::Command::new("git")
                .args(["diff", "--quiet", "--"])
                .arg(&path)
                .output()
                .await?;
            if !diff.status.success() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn run_compile_check(&self) -> Result<VerificationOutcome> {
        let mut command = tokio::process::Command::new("cargo");
        command.args(["check", "--quiet"]);
        if std::path::Path::new("professor-x/Cargo.toml").exists() {
            command.current_dir("professor-x");
        }

        let output = command.output().await?;
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

    async fn git_worktree_clean(&self) -> Result<bool> {
        let out = tokio::process::Command::new("git")
            .args(["status", "--porcelain"])
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

    async fn commit_node(&self, node: &EvolutionNode) -> Result<()> {
        let paths = changed_paths_for_node(node);
        if paths.is_empty() {
            warn!("evolved: accepted node has no known changed paths; skipping commit");
            return Ok(());
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
                return Ok(());
            }
            anyhow::bail!("git commit failed: {err}");
        }
        Ok(())
    }

    async fn rollback_node_changes(&self, node: &EvolutionNode) -> Result<()> {
        let paths = changed_paths_for_node(node);
        if paths.is_empty() {
            return Ok(());
        }

        let mut tracked_paths = Vec::new();
        let mut untracked_paths = Vec::new();
        for path in paths {
            if self.git_path_tracked(&path).await? {
                tracked_paths.push(path);
            } else {
                untracked_paths.push(path);
            }
        }

        if !tracked_paths.is_empty() {
            let mut restore = tokio::process::Command::new("git");
            restore.args(["restore", "--staged", "--worktree", "--"]);
            for path in &tracked_paths {
                restore.arg(path);
            }
            let restore = restore.output().await?;
            if !restore.status.success() {
                anyhow::bail!(
                    "git restore failed: {}",
                    String::from_utf8_lossy(&restore.stderr)
                );
            }
        }

        for path in &untracked_paths {
            if path.is_file() {
                std::fs::remove_file(path)?;
            }
        }

        info!("evolved: rolled back rejected proposal paths");
        Ok(())
    }

    async fn git_path_tracked(&self, path: &PathBuf) -> Result<bool> {
        let out = tokio::process::Command::new("git")
            .args(["ls-files", "--error-unmatch"])
            .arg(path)
            .output()
            .await?;
        Ok(out.status.success())
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

fn changed_paths_for_node(node: &EvolutionNode) -> Vec<PathBuf> {
    match &node.target_component {
        HarnessComponent::SystemPrompt => vec![PathBuf::from("personas/professor_x.md")],
        HarnessComponent::SkillDefinition(name) => vec![PathBuf::from(format!("skills/{name}.md"))],
        HarnessComponent::HarnessConfig => vec![PathBuf::from("config/hardware.toml")],
        _ => Vec::new(),
    }
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
}
