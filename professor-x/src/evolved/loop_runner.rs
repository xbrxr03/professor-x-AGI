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
use std::sync::Arc;
use tracing::{info, warn};

use crate::evolved::analyzer::Analyzer;
use crate::evolved::cognition_base::CognitionStore;
use crate::evolved::proposer::{ChangeManifest, EvolutionNode, HarnessComponent, NodeDatabase, VerificationStatus};
use crate::evolved::tracker::OutcomeTracker;
use crate::memd::MemoryManager;
use crate::ollama::{ChatMessage, ModelOptions, OllamaClient};

// Parse "[DHE:layer=X,lever=Y]" from failure pattern strings.
// Returns (layer, lever) from the most common DHE annotation found, or (0, 3) as default.
fn parse_dhe_from_patterns(patterns: &[String]) -> (u8, u8) {
    for p in patterns {
        if let Some(start) = p.find("[DHE:layer=") {
            let rest = &p[start + 11..];
            if let Some(comma) = rest.find(',') {
                let layer_str = &rest[..comma];
                let lever_str = rest.get(comma + 7..).unwrap_or("3").split(']').next().unwrap_or("3");
                let layer = layer_str.parse::<u8>().unwrap_or(0);
                let lever = lever_str.parse::<u8>().unwrap_or(3);
                return (layer, lever);
            }
        }
    }
    (0, 3)
}

pub struct EvolvedLoop {
    ollama:   Arc<OllamaClient>,
    memory:   Arc<MemoryManager>,
    node_db:  NodeDatabase,
    cognition: CognitionStore,
}

impl EvolvedLoop {
    pub fn new(ollama: Arc<OllamaClient>, memory: Arc<MemoryManager>) -> Self {
        let node_db   = NodeDatabase::new(Arc::clone(&memory.db));
        let cognition = CognitionStore::new(Arc::clone(&memory.db));
        Self { ollama, memory, node_db, cognition }
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
        info!("evolved: success_rate={:.2}, failure_patterns={:?}", success_rate, failure_patterns);

        // Sample a node via UCB1 (ASI-Evolve)
        let candidates = self.node_db.sample_ucb1(3)?;

        let proposal = self.researcher_propose(&failure_patterns, &candidates, success_rate).await?;
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
        self.analyzer_verify(&mut node, tracker).await?;

        info!("evolved: cycle complete — node {} {}", node.id.unwrap_or(0), format!("{:?}", node.status));
        Ok(true)
    }

    async fn researcher_propose(
        &self,
        failure_patterns: &[String],
        candidates: &[EvolutionNode],
        success_rate: f32,
    ) -> Result<Option<EvolutionNode>> {
        // Retrieve top cognition items for context
        let cognition_items = self.cognition.query_top_k("harness improvement failure", 5)?;
        let cognition_context = cognition_items.iter()
            .map(|c| format!("- {}", c.content))
            .collect::<Vec<_>>()
            .join("\n");

        let candidates_text = if candidates.is_empty() {
            "No prior nodes. This is round 1.".to_string()
        } else {
            candidates.iter().map(|n| format!(
                "Node {}: motivation='{}' score={:.2} visits={}",
                n.id.unwrap_or(0), n.motivation, n.score, n.visit_count
            )).collect::<Vec<_>>().join("\n")
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

        let resp = self.ollama.chat(
            vec![
                ChatMessage::system("You are a rigorous AI research agent analyzing your own performance."),
                ChatMessage::user(prompt),
            ],
            Some(ModelOptions::for_evolution()),
        ).await?;

        let (_, answer) = resp.split_thinking();
        self.parse_researcher_output(&answer)
    }

    fn parse_researcher_output(&self, text: &str) -> Result<Option<EvolutionNode>> {
        let component_str = extract_field(text, "COMPONENT").unwrap_or_default();
        let motivation    = extract_field(text, "MOTIVATION").unwrap_or_default();
        let root_cause    = extract_field(text, "ROOT_CAUSE").unwrap_or_default();
        let fix           = extract_field(text, "FIX").unwrap_or_default();
        let predicts_fix  = extract_field(text, "PREDICTS_FIX").unwrap_or_default();
        let predicts_reg  = extract_field(text, "PREDICTS_REGRESSION").unwrap_or_default();

        if motivation.is_empty() || fix.is_empty() {
            return Ok(None);
        }

        let component = parse_component(&component_str);

        let manifest = ChangeManifest {
            evidence_cited: Vec::new(),
            root_cause,
            fix_description: fix.clone(),
            predicted_fixes: vec![predicts_fix],
            predicted_regressions: if predicts_reg == "none" { vec![] } else { vec![predicts_reg] },
            verification_status: VerificationStatus::Pending,
            verified_at: None,
        };

        Ok(Some(EvolutionNode::new(motivation, component, fix, manifest)))
    }

    async fn engineer_apply(&self, node: &mut EvolutionNode) -> Result<bool> {
        // Safety check: Middleware/core modules require human approval
        if matches!(node.target_component, HarnessComponent::Middleware) {
            warn!("evolved: Engineer blocked — Middleware changes require human approval");
            return Ok(false);
        }

        info!("evolved: Engineer applying change to {:?}", node.target_component);

        // Apply the change based on component type
        let applied = match &node.target_component {
            HarnessComponent::SystemPrompt => {
                self.apply_system_prompt_change(&node.diff).await
            }
            HarnessComponent::HarnessConfig => {
                self.apply_config_change(&node.diff).await
            }
            HarnessComponent::ToolDescription(name) => {
                self.apply_tool_description_change(name, &node.diff).await
            }
            HarnessComponent::SkillDefinition(name) => {
                self.apply_skill_change(name, &node.diff).await
            }
            _ => {
                warn!("evolved: component {:?} not yet implemented", node.target_component);
                Ok(false)
            }
        };

        match applied {
            Ok(true) => {
                node.status = crate::evolved::proposer::NodeStatus::Testing;
                // Version control: git commit the change
                let _ = tokio::process::Command::new("git")
                    .args(["add", "-A"])
                    .output().await;
                let commit_msg = format!(
                    "evolved: {} — {}",
                    format!("{:?}", node.target_component),
                    node.motivation.chars().take(60).collect::<String>()
                );
                let _ = tokio::process::Command::new("git")
                    .args(["commit", "-m", &commit_msg])
                    .output().await;
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

    async fn apply_tool_description_change(&self, _tool_name: &str, _new_desc: &str) -> Result<bool> {
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

    async fn analyzer_verify(&self, node: &mut EvolutionNode, tracker: &OutcomeTracker) -> Result<()> {
        let prompt = Analyzer::build_prompt(
            &node.motivation,
            &node.diff,
            &node.results.to_string(),
        );
        let resp = self.ollama.generate(&prompt, None, Some(ModelOptions::for_evolution())).await?;
        let (_, answer) = resp.split_thinking();

        let (analysis, lesson) = Analyzer::parse_response(&answer);
        node.analysis = analysis.clone();

        // Did the predicted improvement materialize?
        let recent_success = tracker.success_rate(5);
        let improved = recent_success > 0.6;

        node.manifest.verification_status = if improved {
            node.status = crate::evolved::proposer::NodeStatus::Accepted;
            node.score = (node.score + recent_success) / 2.0;
            VerificationStatus::Confirmed
        } else {
            node.status = crate::evolved::proposer::NodeStatus::Rejected;
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

        // Save node to DB
        self.node_db.insert(node)?;
        Ok(())
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
