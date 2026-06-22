/// ReAct execution loop — the agent's inner loop.
///
/// Architecture:
/// - ReAct (arXiv:2210.03629): Thought → Action → Observation cycle
/// - Reflexion (arXiv:2303.11366): verbal RL buffer, max 3, on task failure
/// - Self-Generated ICE (arXiv:2505.00234): similar past tasks injected at start
/// - MARS (arXiv:2601.11974): principle+procedure reflection, persisted to semantic
/// - Voyager (arXiv:2305.16291): 4-attempt max per task, skill library lookup
/// - ClawOS circuit breaker: 3 consecutive tool failures → pause + warn
///
/// Prompt format:
///   <identity>...</identity>              ← pinned memory
///   <working-memory>...</working-memory>  ← current session context
///   <examples>...</examples>              ← ICE from episodic memory
///   <knowledge>...</knowledge>            ← relevant cognition items
///   <task>...</task>                      ← current task
///   <reflections>...</reflections>        ← prior Reflexion buffer (if retry)
///   <history>...</history>                ← prior steps this attempt
///
///   Available tools: ...
///
///   Thought: <your reasoning>
///   Action: <tool_name>
///   Action Input: <json>
use anyhow::Result;
use chrono::Utc;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::agentd::graph::{ExecutionStep, TaskNode, TaskStatus, TaskType};
use crate::evolved::lcap::LcapPolicy;
use crate::evolved::tracker::TaskOutcome;
use crate::failure::{classify_failure_mode, normalize_failure_mode};
use crate::memd::affect::{arousal_from_load, state_label, valence_from_outcome, AffectState};
use crate::memd::causal_traces::{CausalTrace, TimedAction};
use crate::memd::computational_body::ComputationalVitals;
use crate::memd::episodic::EpisodicEntry;
use crate::memd::events::EventStore;
use crate::memd::self_prediction::{self, SelfPrediction};
use crate::memd::task_runs::TaskRunStore;
use crate::memd::transcripts::TranscriptStore;
use crate::memd::working::MermaidCanvas;
use crate::memd::MemoryManager;
use crate::ollama::{
    ChatMessage, ModelOptions, OllamaClient, ToolCall, ToolCallFunction, ToolSpec,
};
use crate::policyd::{AuditStore, Decision, PermissionScope, PolicyEngine};
use crate::toolbridge::executor::{Action, Observation};
use crate::toolbridge::{ToolExecutor, ToolRegistry};
use tokio_util::sync::CancellationToken;

// Parsed from the LLM's output
struct ParsedStep {
    thought: String,
    tool_name: String,
    params: Value,
}

#[derive(Clone)]
struct TaskVerifier {
    workdir: PathBuf,
    command: String,
    expect_exit: i32,
}

struct VerifierResult {
    passed: bool,
    exit_code: i32,
    output: String,
}

/// Homeostatic baselines for the interoceptive / prediction-error signals that
/// gate the consciousness modules. Module flags fire RELATIVE to these running
/// means (gain control / sensory adaptation), so they stay discriminating
/// (~half on) instead of saturating as absolute signal levels drift upward over
/// a long run — the failure mode the overnight data exposed (mean_active climbed
/// toward all-on and both phi and LZc collapsed). Coupling is preserved: shared
/// signals still drive multiple modules together; only the threshold adapts.
#[derive(Clone, Copy)]
struct SignalBaselines {
    stress: f32,
    surprise: f32,
    affect: f32,
}

impl SignalBaselines {
    fn prior() -> Self {
        Self {
            stress: 0.3,
            surprise: 0.3,
            affect: 0.2,
        }
    }
    fn update(&mut self, stress: f32, surprise: f32, affect: f32) {
        const A: f32 = 0.1; // EMA rate
        self.stress += A * (stress - self.stress);
        self.surprise += A * (surprise - self.surprise);
        self.affect += A * (affect - self.affect);
    }
}

pub struct ReactLoop {
    ollama: Arc<OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    cancel: CancellationToken,
    lcap: Arc<std::sync::Mutex<LcapPolicy>>,
    current_round: u32,
    events: Option<Arc<EventStore>>,
    transcripts: Option<Arc<TranscriptStore>>,
    /// H1 experiment hook: hard override on the context-budget ceiling
    /// LCAP would otherwise return. `None` keeps LCAP's selection; `Some(N)`
    /// clamps the ceiling to N tokens per task. Lets `--memory-budget N`
    /// sweep T* without touching LCAP arm state.
    memory_budget_override: Option<u32>,
    /// M1 repo-fix benchmark: override the agent's workspace root so it edits inside a
    /// per-task /tmp workdir instead of the repo. `None` keeps the default workspace.
    workspace_override: Option<std::path::PathBuf>,
    /// M4 self-improvement: override the ReAct system prompt at runtime so the evolution
    /// loop can A/B candidate prompts against the repo-fix benchmark without recompiling.
    prompt_override: Option<String>,
    /// Optional task-local verifier. Coding benchmarks can reject finish until tests pass.
    verifier: Option<TaskVerifier>,
    /// Stable identifier for this loop's affect session (one per ReactLoop instance).
    session_id: String,
    /// Running affect state (valence + arousal) updated after each task attempt.
    /// Injected into every ReAct prompt via `<affect .../>` (H16).
    affect: std::sync::Mutex<AffectState>,
    /// Accumulated (predicted_success, actual_success) pairs for FED recording (H15).
    /// Drained by the caller via `drain_fed_samples()` after a batch completes.
    fed_samples: std::sync::Mutex<Vec<(f32, f32)>>,
    /// Per-attempt MermaidCanvas — cleared between retries, injected as
    /// compact `<history>` block (~61% token savings vs raw transcript).
    canvas: std::sync::Mutex<MermaidCanvas>,
    /// Seed 4 (interoception): predicted computational body state for the
    /// current task, set at task start from recent vitals history and injected
    /// into every prompt as `<body .../>`. Actual vitals recorded at task end.
    body_prediction: std::sync::Mutex<ComputationalVitals>,
    /// Seed 7 (predictive self-model): the agent's prediction about its own
    /// behaviour for the current task, made before execution. Error against the
    /// actual run is recorded at task end.
    self_prediction: std::sync::Mutex<SelfPrediction>,
    /// Seed 2 read-back: learned causal tool-sequence patterns for the current
    /// task category, computed once per task and injected each step so the
    /// agent acts on what has actually worked before. The self-knowledge loop.
    causal_hint: std::sync::Mutex<String>,
    /// Self-managed working memory (MemGPT / Claude-Code plan). The agent writes
    /// a running plan/notes via scratchpad.write; it persists across steps and
    /// is injected into every prompt. Cleared per task.
    scratchpad: std::sync::Mutex<String>,
    /// Sub-agent recursion depth. The primary loop is 0; a delegated sub-agent
    /// runs at depth+1 and is forbidden from delegating further (depth cap),
    /// preventing runaway spawn trees.
    depth: u32,
    /// Homeostatic baselines for module-gating signals (anti-saturation).
    signal_baselines: std::sync::Mutex<SignalBaselines>,
    /// Cross-module coupling on/off. Default on; set PROFESSOR_X_COUPLING=off to
    /// run the DECOUPLED control condition for the PCI contrast (modules fire on
    /// their own signals, no shared-signal gating) — the anaesthesia arm of the
    /// wake-vs-anaesthesia perturbational-complexity experiment.
    coupling_enabled: bool,
}

impl ReactLoop {
    pub fn new(
        ollama: Arc<OllamaClient>,
        registry: Arc<std::sync::RwLock<ToolRegistry>>,
        policy: Arc<PolicyEngine>,
        memory: Arc<MemoryManager>,
        cancel: CancellationToken,
    ) -> Self {
        let session_id = Uuid::new_v4().to_string();
        let lcap = LcapPolicy::load_from_db(&memory.db).unwrap_or_else(|_| LcapPolicy::new());
        Self {
            ollama,
            registry,
            policy,
            memory,
            cancel,
            lcap: Arc::new(std::sync::Mutex::new(lcap)),
            current_round: 0,
            events: None,
            transcripts: None,
            memory_budget_override: None,
            workspace_override: None,
            prompt_override: None,
            verifier: None,
            affect: std::sync::Mutex::new(AffectState::neutral(session_id.clone(), 0)),
            fed_samples: std::sync::Mutex::new(Vec::new()),
            canvas: std::sync::Mutex::new(MermaidCanvas::default()),
            body_prediction: std::sync::Mutex::new(ComputationalVitals::neutral()),
            self_prediction: std::sync::Mutex::new(SelfPrediction::uninformed()),
            causal_hint: std::sync::Mutex::new(String::new()),
            scratchpad: std::sync::Mutex::new(String::new()),
            session_id,
            depth: 0,
            signal_baselines: std::sync::Mutex::new(SignalBaselines::prior()),
            coupling_enabled: std::env::var("PROFESSOR_X_COUPLING")
                .map(|v| v.to_lowercase() != "off")
                .unwrap_or(true),
        }
    }

    /// M1: point the agent at a specific workspace root (e.g. a per-task /tmp workdir
    /// for the repo-fix benchmark) instead of the default repo workspace.
    pub fn with_workspace_root(mut self, root: std::path::PathBuf) -> Self {
        self.workspace_override = Some(root);
        self
    }

    /// M4: override the ReAct system prompt (for evolution-loop A/B against repo-fix).
    pub fn with_prompt_override(mut self, prompt: String) -> Self {
        self.prompt_override = Some(prompt);
        self
    }

    pub fn with_verifier(mut self, workdir: PathBuf, command: String, expect_exit: i32) -> Self {
        self.verifier = Some(TaskVerifier {
            workdir,
            command,
            expect_exit,
        });
        self
    }

    /// Internal: build a sub-agent loop sharing this loop's resources, one level
    /// deeper. Used by `agent.delegate`.
    fn child_loop(&self) -> Self {
        let mut child = Self::new(
            Arc::clone(&self.ollama),
            Arc::clone(&self.registry),
            Arc::clone(&self.policy),
            Arc::clone(&self.memory),
            self.cancel.clone(),
        );
        child.depth = self.depth + 1;
        child.current_round = self.current_round;
        if let Some(events) = &self.events {
            child.events = Some(Arc::clone(events));
        }
        child.verifier = self.verifier.clone();
        child
    }

    /// Spawn a sub-agent on a focused sub-goal and return its result as an
    /// observation. Depth-capped: a sub-agent (depth >= 1) cannot delegate.
    async fn delegate(&self, goal: &str, session_id: Uuid, parent_task: Uuid) -> Observation {
        if goal.trim().is_empty() {
            return Observation::err("agent.delegate requires a non-empty 'goal'");
        }
        if self.depth >= 1 {
            return Observation::err(
                "delegation depth exceeded — a sub-agent cannot spawn further sub-agents",
            );
        }
        self.emit_event(
            Some(session_id),
            Some(parent_task),
            "agent.delegate",
            format!("delegating sub-goal: {}", truncate(goal, 100)),
            json!({"depth": self.depth + 1}),
        );
        let child = self.child_loop();
        let mut subtask = TaskNode::new(goal.to_string(), TaskType::Research, 60);
        subtask.max_attempts = 2;
        // Box the recursive future: run -> delegate -> child.run is an async
        // recursion cycle, which Rust requires be boxed to have a known size.
        match Box::pin(child.run(&mut subtask)).await {
            Ok(outcome) => {
                let result = subtask.recent_steps_text(3);
                let result = if result.trim().is_empty() {
                    "(sub-agent produced no observable output)".to_string()
                } else {
                    result.chars().take(1200).collect::<String>()
                };
                Observation {
                    success: outcome.success,
                    output: format!(
                        "Sub-agent finished (success={}, steps={}). Result:\n{}",
                        outcome.success, outcome.steps_taken, result
                    ),
                    error: if outcome.success {
                        None
                    } else {
                        Some("sub-agent did not fully succeed".to_string())
                    },
                    tokens_used: 0,
                    execution_ms: 0,
                    artifacts: Vec::new(),
                }
            }
            Err(e) => Observation::err(&format!("sub-agent error: {e}")),
        }
    }

    /// The mirror: a second evaluative pass reviews THIS agent's trajectory and
    /// returns a critique. A self observing the self — metacognition made an
    /// explicit second perspective.
    async fn critique(&self, task: &TaskNode) -> Observation {
        let trajectory = task.recent_steps_text(6);
        let trajectory = if trajectory.trim().is_empty() {
            "(no steps taken yet)".to_string()
        } else {
            trajectory.chars().take(2000).collect::<String>()
        };
        // ARIS-style two-thread adversarial review (the `kill-argument` pattern): a DEFENSE
        // steelmans that the work is correct/complete, then a PROSECUTION attacks that case
        // and finds the fatal flaw. Two opposed perspectives surface loops, wrong assumptions,
        // and gathered-but-unreported results that a single bland review misses.
        let opts = || ModelOptions {
            temperature: Some(0.3),
            num_ctx: Some(8192),
            top_p: None,
            stop: None,
            think: Some(false),
        };

        // Thread A — DEFENSE: the strongest good-faith case that it is on track / done.
        let defense_prompt = format!(
            "TASK: {}\n\nAGENT STEPS:\n{}\n\nIn 2-3 sentences, make the STRONGEST good-faith case \
             that the agent is on track and the task is (or is about to be) correctly completed. \
             Cite specific evidence from the steps.",
            task.description, trajectory
        );
        let defense = match self
            .ollama
            .generate(
                &defense_prompt,
                Some("You argue, in good faith, that an agent's work is correct and complete."),
                Some(opts()),
            )
            .await
        {
            Ok(r) => r.split_thinking().1.trim().to_string(),
            Err(e) => return Observation::err(&format!("critic unavailable: {e}")),
        };

        // Thread B — PROSECUTION: attack the defense; find the fatal flaw + the next action.
        let attack_prompt = format!(
            "TASK: {}\n\nAGENT STEPS:\n{}\n\nA defender argues:\n\"{}\"\n\nNow ATTACK that case. \
             Give the single strongest reason it is WRONG, incomplete, looping, built on a wrong \
             assumption, or has a result it already gathered but never reported. Be blunt and \
             specific. End with exactly: NEXT ACTION: <the one most useful concrete next step>.",
            task.description,
            trajectory,
            defense.chars().take(800).collect::<String>()
        );
        match self
            .ollama
            .generate(
                &attack_prompt,
                Some("You are a ruthless adversarial reviewer who finds the fatal flaw in an agent's work."),
                Some(opts()),
            )
            .await
        {
            Ok(resp) => {
                let (_, attack) = resp.split_thinking();
                Observation {
                    success: true,
                    output: format!(
                        "MIRROR CRITIC (adversarial):\nDEFENSE: {}\n\nPROSECUTION: {}",
                        defense.chars().take(400).collect::<String>(),
                        attack.trim()
                    ),
                    error: None,
                    tokens_used: 0,
                    execution_ms: 0,
                    artifacts: Vec::new(),
                }
            }
            Err(e) => Observation::err(&format!("critic unavailable: {e}")),
        }
    }

    /// Tree-of-Thoughts (arXiv:2305.10601): deliberate branching search over
    /// approaches. PROPOSE k distinct candidate plans, VALUE each, then SELECT
    /// the most promising — instead of greedily committing to the first idea.
    /// Two LLM calls (propose, then evaluate+select); the winning plan is
    /// returned as an observation the agent then executes. For hard tasks where
    /// the first approach often dead-ends.
    async fn tot_search(&self, task: &TaskNode, branches: usize) -> Observation {
        let k = branches.clamp(2, 5);
        let context = task.recent_steps_text(4);
        let context = if context.trim().is_empty() {
            String::new()
        } else {
            format!(
                "\n\nWork so far:\n{}",
                context.chars().take(1200).collect::<String>()
            )
        };

        // 1) PROPOSE k distinct approaches.
        let propose = format!(
            "Task: {}{}\n\nPropose {k} DISTINCT, concrete approaches to solve this task. \
             They must differ in strategy, not just wording. Each in 1-2 sentences with \
             the first tool/step it would take.\n\n\
             Output ONLY {k} blocks in this exact format:\n\
             ===OPTION===\n<approach>\n===OPTION===\n<approach>",
            task.description, context
        );
        let proposed = match self
            .ollama
            .generate(
                &propose,
                Some("You generate diverse, concrete solution strategies."),
                Some(ModelOptions {
                    temperature: Some(0.9),
                    num_ctx: Some(8192),
                    top_p: Some(0.95),
                    stop: None,
                    think: Some(false),
                }),
            )
            .await
        {
            Ok(resp) => {
                let (_, text) = resp.split_thinking();
                text.split("===OPTION===")
                    .map(|s| s.trim())
                    .filter(|s| s.len() > 8)
                    .map(|s| s.chars().take(400).collect::<String>())
                    .collect::<Vec<_>>()
            }
            Err(e) => return Observation::err(&format!("tot propose failed: {e}")),
        };
        if proposed.is_empty() {
            return Observation::err("tot: model proposed no usable approaches");
        }

        let options_block = proposed
            .iter()
            .enumerate()
            .map(|(i, o)| format!("OPTION {}: {}", i + 1, o))
            .collect::<Vec<_>>()
            .join("\n\n");

        // 2) VALUE each + SELECT the best.
        let evaluate = format!(
            "Task: {}\n\nCandidate approaches:\n{}\n\n\
             Score each approach 0-10 for how likely it is to solve the task efficiently \
             and correctly. Then pick the single best.\n\n\
             Reply in EXACTLY this format:\n\
             SCORES: 1=<n> 2=<n> ...\n\
             BEST: <option number>\n\
             PLAN: <the winning approach restated as 2-4 concrete numbered steps>",
            task.description, options_block
        );
        match self
            .ollama
            .generate(
                &evaluate,
                Some("You are a rigorous evaluator of solution strategies."),
                Some(ModelOptions {
                    temperature: Some(0.2),
                    num_ctx: Some(8192),
                    top_p: None,
                    stop: None,
                    think: Some(false),
                }),
            )
            .await
        {
            Ok(resp) => {
                let (_, text) = resp.split_thinking();
                Observation {
                    success: true,
                    output: format!(
                        "TREE-OF-THOUGHTS — evaluated {} approaches:\n{}\n\nFollow the selected PLAN.",
                        proposed.len(),
                        text.trim()
                    ),
                    error: None,
                    tokens_used: 0,
                    execution_ms: 0,
                    artifacts: Vec::new(),
                }
            }
            Err(e) => Observation::err(&format!("tot evaluate failed: {e}")),
        }
    }

    pub fn with_lcap(mut self, lcap: Arc<std::sync::Mutex<LcapPolicy>>, round: u32) -> Self {
        self.lcap = lcap;
        self.current_round = round;
        self
    }

    pub fn with_events(mut self, events: Arc<EventStore>) -> Self {
        self.events = Some(events);
        self
    }

    pub fn with_transcripts(mut self, transcripts: Arc<TranscriptStore>) -> Self {
        self.transcripts = Some(transcripts);
        self
    }

    /// H1 context-injection threshold (T*) sweep hook. When set, every task
    /// run by this loop uses `min(LCAP-selected ceiling, override)` as its
    /// hard context ceiling. Recommended sweep set: 500, 1000, 2000, 4000,
    /// 6000, 10000, 16000 tokens per hypotheses.md H1 §"Proposed test".
    pub fn with_memory_budget_override(mut self, budget: u32) -> Self {
        self.memory_budget_override = Some(budget);
        self
    }

    /// Drain accumulated (predicted_success, actual_success) pairs collected
    /// across tasks run by this loop. Call after a batch/session completes to
    /// compute and persist a `FedRecord` (H15 — Free Energy Delta trajectory).
    pub fn drain_fed_samples(&self) -> Vec<(f32, f32)> {
        self.fed_samples
            .lock()
            .map(|mut v| std::mem::take(&mut *v))
            .unwrap_or_default()
    }

    /// Run a task to completion or exhaustion. Returns outcome for the tracker.
    pub async fn run(&self, task: &mut TaskNode) -> Result<TaskOutcome> {
        let task_runs = TaskRunStore::new(Arc::clone(&self.memory.db));
        let _ = task_runs.queued(task);
        self.emit_event(
            None,
            Some(task.id),
            "task.queued",
            format!("queued task: {}", truncate(&task.description, 120)),
            json!({
                "task_type": format!("{:?}", task.task_type),
                "priority": task.priority,
                "max_attempts": task.max_attempts,
            }),
        );
        task.status = TaskStatus::Running;
        task.started_at = Some(Utc::now());
        let _ = task_runs.started(task);
        self.emit_event(
            None,
            Some(task.id),
            "task.started",
            format!("started task: {}", truncate(&task.description, 120)),
            json!({
                "task_type": format!("{:?}", task.task_type),
                "priority": task.priority,
                "max_attempts": task.max_attempts,
            }),
        );

        // ICE: retrieve similar past tasks from episodic memory
        let ice_examples = self.retrieve_ice(&task.description).await;

        // FED (H15): predict success before execution from ICE hit quality
        let predicted_success = predict_success_from_ice(&ice_examples);

        // Cognition context: relevant items from cognition base
        let cognition_context = self.retrieve_cognition(&task.description).await;

        // Module-engagement signals for phi, captured BEFORE binding. The
        // episodic/cognition modules are "engaged" when recall surfaced
        // something relevant to THIS task — not whether it survived binding
        // (binding suppression made the cognition flag a constant 0, which is a
        // measurement bug: a module that never registers carries no integrated
        // information). These vary task-to-task with what memory actually
        // matched, which is the faithful integration signal.
        let episodic_engaged = !ice_examples.is_empty();
        let cognition_engaged = !cognition_context.is_empty();

        // Binding: keep only context that resonates ACROSS modalities. A memory
        // echoed in both episodic and cognition is grounded; one standing alone
        // is suppressed. Reduces confabulation, raises integration. Best-effort —
        // a no-op when embeddings are unavailable.
        let (ice_examples, cognition_context) =
            self.apply_binding(ice_examples, cognition_context).await;

        // Behavior-keyed retrieval (flag-gated, default OFF): append the nearest past fix by failure
        // signature AFTER binding, so the behavioral hint is not suppressed by cross-modal binding.
        let ice_examples = {
            let mut e = ice_examples;
            e.extend(self.retrieve_behavioral());
            e
        };

        // Fresh scratchpad per task (self-managed working memory).
        if let Ok(mut sp) = self.scratchpad.lock() {
            sp.clear();
        }

        // LCAP: select context budget (Balanced before round 10, UCB1 after)
        let category = LcapPolicy::classify(&task.description);

        // Seed 2 read-back: surface tool-sequences that have reliably worked for
        // this task category, so the agent acts on its own learned causality.
        {
            let hint = self
                .memory
                .causal_traces
                .format_for_context(&format!("{category:?}"), 4)
                .unwrap_or_default();
            if let Ok(mut h) = self.causal_hint.lock() {
                *h = hint;
            }
        }

        let selected_arm = {
            let lc = self.lcap.lock().unwrap();
            lc.select_arm(&category, self.current_round)
        };
        let lcap_ceiling =
            crate::evolved::lcap::ContextBudget::from_arm(&selected_arm).hard_ceiling_tokens;
        let num_ctx = effective_memory_ceiling(lcap_ceiling, self.memory_budget_override);

        // Seed 4 (interoception): predict the computational body state for this
        // task from recent vitals history. Seth's "controlled hallucination" —
        // the agent experiences its predicted body; reality only corrects it.
        {
            let predicted_latency = self
                .memory
                .computational_body
                .recent_mean_latency(10)
                .ok()
                .flatten()
                .unwrap_or(1500.0);
            let evolution_health = 0.5; // updated by evolution loop; neutral prior here
            let predicted = ComputationalVitals {
                inference_latency_ms: predicted_latency,
                token_budget_used: (num_ctx as f32 / 32768.0).min(1.0),
                memory_pressure: 0.2,
                evolution_health,
            };
            if let Ok(mut bp) = self.body_prediction.lock() {
                *bp = predicted;
            }
        }

        // Seed 7 (predictive self-model): before acting, the agent predicts its
        // OWN behaviour — which tools, how many steps, success odds, failure
        // mode. The "I" is the perspective from which predictions are made.
        // Error is measured at task end; persistent error = genuine self-ignorance.
        {
            let tool_names: Vec<&str> = vec![
                "fs.read",
                "fs.hash_read",
                "fs.window_open",
                "fs.window_goto",
                "fs.window_scroll",
                "fs.list",
                "fs.write",
                "fs.hash_edit",
                "web.search",
                "web.fetch",
                "vision.analyze",
                "shell.restricted",
                "patch.review",
                "patch.apply",
                "memory.read",
                "memory.write",
                "finish",
                "fail",
            ];
            let pred_prompt =
                self_prediction::build_prediction_prompt(&task.description, &tool_names);
            // Fast, non-thinking — this is a structured 4-field prediction,
            // not deliberation. Thinking here would add a full generation per task.
            let pred_opts = ModelOptions {
                temperature: Some(0.2),
                num_ctx: Some(2048),
                top_p: Some(0.9),
                stop: None,
                think: Some(false),
            };
            let prediction = match self
                .ollama
                .generate(
                    &pred_prompt,
                    Some("You are predicting your own behaviour honestly. Output only the requested fields."),
                    Some(pred_opts),
                )
                .await
            {
                Ok(resp) => {
                    let (_, answer) = resp.split_thinking();
                    self_prediction::parse_prediction(&answer)
                }
                Err(e) => {
                    debug!("react: self-prediction skipped: {e}");
                    SelfPrediction::uninformed()
                }
            };
            if let Ok(mut sp) = self.self_prediction.lock() {
                *sp = prediction;
                // CALIBRATION (closes the meta-d' deficit: the model self-reports
                // a flat ~0.9 regardless of outcome, so confidence carries no
                // information). Replace the self-report's success odds with an
                // empirically-grounded estimate: the actual historical success
                // RATE for this task category (a calibrated prior), modulated by
                // the per-task ICE-quality signal. Confidence now tracks
                // competence — it varies across categories (their true rates
                // differ) and within a category (by ICE quality) — so
                // metacognitive sensitivity (Type-2 AUROC) can exceed chance.
                let cat = format!("{category:?}");
                let base = self
                    .memory
                    .self_prediction
                    .category_success_rate(&cat, 100)
                    .ok()
                    .flatten();
                sp.expected_success = match base {
                    Some(rate) => (0.6 * rate + 0.4 * predicted_success).clamp(0.02, 0.98),
                    None => predicted_success.clamp(0.02, 0.98),
                };
            }
        }

        if let Some(override_budget) = self.memory_budget_override {
            self.emit_event(
                None,
                Some(task.id),
                "react.memory_budget.override",
                format!(
                    "memory budget overridden to {num_ctx} (lcap={lcap_ceiling}, requested={override_budget})"
                ),
                json!({
                    "lcap_ceiling": lcap_ceiling,
                    "requested": override_budget,
                    "effective": num_ctx,
                }),
            );
        }

        for attempt in 0..task.max_attempts {
            task.attempt_count = attempt + 1;
            info!(
                "react: task '{}' attempt {}/{}",
                task.description,
                attempt + 1,
                task.max_attempts
            );
            self.emit_event(
                None,
                Some(task.id),
                "task.attempt.started",
                format!("attempt {}/{} started", attempt + 1, task.max_attempts),
                json!({"attempt": attempt + 1}),
            );
            let _ = task_runs.attempt_started(task);

            let outcome = self
                .run_attempt(task, &ice_examples, &cognition_context, num_ctx)
                .await;

            match outcome {
                Ok(true) => {
                    task.status = TaskStatus::Complete;
                    task.completed_at = Some(Utc::now());
                    task.outcome_score = Some(1.0);

                    self.write_episodic(task, true, predicted_success).await;
                    // NOTE: trajectory collection for the self-distillation corpus
                    // is NOT done here. The agent declaring `finish` is not proof
                    // the answer is correct. Collection is judge-gated by the
                    // caller (HIRO post-evaluate, --run-self-tests post-judge) via
                    // ReactLoop::collect_trajectory, so the corpus holds only
                    // verified-correct lessons, not merely agent-finished ones.
                    let transcript_path =
                        self.record_transcript(task, "succeeded", "task completed successfully");
                    let _ = task_runs.finished(task, None, transcript_path.as_deref());
                    self.emit_event(
                        None,
                        Some(task.id),
                        "task.succeeded",
                        format!("completed task in {} step(s)", task.steps.len()),
                        json!({
                            "attempts": task.attempt_count,
                            "steps": task.steps.len(),
                            "score": 1.0,
                        }),
                    );

                    // LCAP: reward successful budget selection
                    {
                        let mut lc = self.lcap.lock().unwrap();
                        lc.update(&category, &selected_arm, 1.0);
                        if let Err(e) = lc.save_to_db(&self.memory.db) {
                            warn!("react: failed to persist LCAP state after success: {e}");
                        }
                    }

                    // Affect (H16): positive valence on success
                    {
                        let tool_density = task.steps.len() as f32 / 20.0;
                        let retry_pressure =
                            task.attempt_count.saturating_sub(1) as f32 / task.max_attempts as f32;
                        let v = valence_from_outcome(1.0, predicted_success);
                        let a = arousal_from_load(tool_density, retry_pressure);
                        if let Ok(mut aff) = self.affect.lock() {
                            aff.round = self.current_round;
                            aff.update_ema(v, a, 0.3);
                            let _ = self.memory.affect.append(&*aff);
                        }
                    }
                    // FED (H15): record prediction accuracy
                    if let Ok(mut fed) = self.fed_samples.lock() {
                        fed.push((predicted_success, 1.0));
                    }

                    // Seeds 2 + 4: record causal trace and computational vitals
                    self.record_body_and_causal(
                        task,
                        &category,
                        num_ctx,
                        true,
                        1.0,
                        episodic_engaged,
                        cognition_engaged,
                    );

                    return Ok(TaskOutcome {
                        task_id: task.id,
                        description: task.description.clone(),
                        success: true,
                        score: 1.0,
                        failure_class: None,
                        failure_mode: None,
                        steps_taken: task.steps.len() as u32,
                        timestamp: Utc::now(),
                    });
                }
                Ok(false) => {
                    if attempt + 1 < task.max_attempts {
                        // Affect: mildly negative after a failed attempt, arousal rises with retries
                        {
                            let tool_density = task.steps.len() as f32 / 20.0;
                            let retry_pressure = (attempt + 1) as f32 / task.max_attempts as f32;
                            let v = valence_from_outcome(0.0, predicted_success);
                            let a = arousal_from_load(tool_density, retry_pressure);
                            if let Ok(mut aff) = self.affect.lock() {
                                aff.round = self.current_round;
                                aff.update_ema(v, a, 0.3);
                            }
                        }
                        let reflection = self.generate_reflection(task).await;
                        task.push_reflection(reflection);
                        task.steps.clear();
                        if let Ok(mut c) = self.canvas.lock() {
                            c.clear();
                        }
                    }
                }
                Err(e) => {
                    warn!("react: attempt {} error: {e}", attempt + 1);
                    self.emit_event(
                        None,
                        Some(task.id),
                        "task.attempt.error",
                        format!(
                            "attempt {} errored: {}",
                            attempt + 1,
                            truncate(&e.to_string(), 160)
                        ),
                        json!({"attempt": attempt + 1, "error": e.to_string()}),
                    );
                    if attempt + 1 < task.max_attempts {
                        task.push_reflection(format!("Error on attempt {}: {e}", attempt + 1));
                    }
                }
            }

            if self.cancel.is_cancelled() {
                break;
            }
        }

        // All attempts exhausted — MARS reflection + DHE attribution (LLM-as-judge for Layer 5)

        // Affect (H16): negative valence on full exhaustion
        {
            let tool_density = task.steps.len() as f32 / 20.0;
            let v = valence_from_outcome(0.0, predicted_success);
            let a = arousal_from_load(tool_density, 1.0);
            if let Ok(mut aff) = self.affect.lock() {
                aff.round = self.current_round;
                aff.update_ema(v, a, 0.3);
                let _ = self.memory.affect.append(&*aff);
            }
        }
        // FED (H15): record prediction accuracy
        if let Ok(mut fed) = self.fed_samples.lock() {
            fed.push((predicted_success, 0.0));
        }

        let mars = self.generate_mars_reflection(task).await;
        let dhe = crate::evolved::dhe::Dhe::diagnose_async(task, &self.ollama).await;
        let failure_mode = format!(
            "{mars} [DHE:layer={},lever={}]",
            dhe.failed_layer, dhe.recommended_lever
        );
        let failure_class = classify_failure_mode(&failure_mode);
        let failure_mode = normalize_failure_mode(&failure_mode);

        task.status = TaskStatus::Failed;
        task.completed_at = Some(Utc::now());
        task.outcome_score = Some(0.0);

        self.write_episodic(task, false, predicted_success).await;
        let transcript_path = self.record_transcript(task, "failed", &failure_mode);
        let _ = task_runs.finished(task, Some(&failure_mode), transcript_path.as_deref());
        self.emit_event(
            None,
            Some(task.id),
            "task.failed",
            format!("task failed after {} attempt(s)", task.attempt_count),
            json!({
                "attempts": task.attempt_count,
                "steps": task.steps.len(),
                "failure_mode": failure_mode,
                // Structured routing (item #3): the real failure's class, the DHE layer/lever it
                // diagnosed, and the harness component class evolution should target — so the
                // proposer/observer route from real outcomes without re-parsing freeform text.
                "failure_class": failure_class.as_str(),
                "dhe_layer": dhe.failed_layer,
                "dhe_lever": dhe.recommended_lever,
                "target_component_class": crate::failure::target_component_class(failure_class),
            }),
        );

        // LCAP: penalize failed budget selection
        let regressed_arm = {
            let mut lc = self.lcap.lock().unwrap();
            let next = if failure_class == crate::failure::FailureClass::Context {
                lc.regress(&category, &selected_arm)
            } else {
                lc.update(&category, &selected_arm, 0.0);
                None
            };
            if let Err(e) = lc.save_to_db(&self.memory.db) {
                warn!("react: failed to persist LCAP state after failure: {e}");
            }
            next
        };
        if let Some(next_arm) = regressed_arm {
            self.emit_event(
                None,
                Some(task.id),
                "lcap.regressed",
                format!(
                    "layer-2 context failure regressed {:?} budget from {:?} to {:?}",
                    category, selected_arm, next_arm
                ),
                json!({
                    "category": format!("{category:?}"),
                    "failure_class": failure_class.as_str(),
                    "from_arm": format!("{selected_arm:?}"),
                    "to_arm": format!("{next_arm:?}"),
                }),
            );
        }

        // Seeds 2 + 4: record causal trace and computational vitals
        self.record_body_and_causal(
            task,
            &category,
            num_ctx,
            false,
            0.0,
            episodic_engaged,
            cognition_engaged,
        );

        Ok(TaskOutcome {
            task_id: task.id,
            description: task.description.clone(),
            success: false,
            score: 0.0,
            failure_class: Some(failure_class),
            failure_mode: Some(failure_mode),
            steps_taken: task.steps.len() as u32,
            timestamp: Utc::now(),
        })
    }

    /// Seeds 2 (STDP) + 4 (interoception): at task end, derive a causal trace
    /// from the executed steps (with timing relative to completion) and the
    /// actual computational vitals, then record both. The interoceptive error
    /// (predicted vs actual body state) is computed against the prediction made
    /// at task start.
    fn record_body_and_causal(
        &self,
        task: &TaskNode,
        category: &crate::evolved::lcap::TaskCategory,
        num_ctx: u32,
        outcome: bool,
        score: f32,
        ice_hit: bool,
        cognition_hit: bool,
    ) {
        let category_name = format!("{category:?}");
        let completed = task.completed_at.unwrap_or_else(Utc::now);

        // ── Seed 2: build timed action sequence (STDP) ───────────────────
        let actions: Vec<TimedAction> = task
            .steps
            .iter()
            .map(|s| {
                let ms_before = (completed - s.timestamp).num_milliseconds().max(0);
                TimedAction {
                    tool: s.action.tool_name.clone(),
                    ms_before_outcome: ms_before,
                    succeeded: s.observation.success,
                }
            })
            .collect();

        if !actions.is_empty() {
            let trace = CausalTrace::new(
                self.session_id.clone(),
                task.id.to_string(),
                category_name.clone(),
                actions,
                outcome,
                score,
            );
            if let Err(e) = self.memory.causal_traces.insert(&trace) {
                warn!("react: failed to record causal trace: {e}");
            }
        }

        // ── Seed 4: actual computational vitals (interoception) ──────────
        let total_ms: i64 = task
            .steps
            .iter()
            .map(|s| s.observation.execution_ms as i64)
            .sum();
        let mean_latency = if task.steps.is_empty() {
            0.0
        } else {
            total_ms as f32 / task.steps.len() as f32
        };
        let actual = ComputationalVitals {
            inference_latency_ms: mean_latency,
            token_budget_used: (num_ctx as f32 / 32768.0).min(1.0),
            memory_pressure: (task.steps.len() as f32 / 20.0).min(1.0),
            evolution_health: 0.5,
        };
        let (predicted_latency, intero_err) = if let Ok(bp) = self.body_prediction.lock() {
            (bp.inference_latency_ms, actual.interoceptive_error(&bp))
        } else {
            (0.0, 0.0)
        };
        if let Err(e) = self.memory.computational_body.record(
            &self.session_id,
            self.current_round,
            &actual,
            Some(predicted_latency),
            Some(intero_err),
        ) {
            warn!("react: failed to record computational vitals: {e}");
        }

        // ── Seed 7: self-prediction error (predictive self-model) ────────
        // Hoisted: this scalar self-prediction error is the SURPRISE signal that
        // couples the memory/causal/self-model modules below (predictive coding).
        let mut self_pred_err = 0.0f32;
        {
            let actual_tools: Vec<String> = task
                .steps
                .iter()
                .map(|s| s.action.tool_name.clone())
                .collect();
            let actual_steps = task.steps.len() as u32;
            if let Ok(pred) = self.self_prediction.lock() {
                let err = pred.error_against(&actual_tools, actual_steps, outcome);
                self_pred_err = err.aggregate();
                if let Err(e) = self.memory.self_prediction.record(
                    &self.session_id,
                    self.current_round,
                    &category_name,
                    &pred,
                    &err,
                ) {
                    warn!("react: failed to record self-prediction: {e}");
                }
            }
        }

        // ── IIT: record which cognitive modules activated this decision ──
        // The 7-module co-activation vector feeds the per-round phi (total
        // correlation) computed at round end.
        {
            use crate::memd::phi::ModuleActivation;
            let used_memory_tool = task
                .steps
                .iter()
                .any(|s| s.action.tool_name.starts_with("memory."));
            // ── Cross-module COUPLING → integrated information (phi) ──────────
            // The modules are NOT independent flags. They gate one another
            // through shared interoceptive and prediction-error signals — like
            // cortical modules competing for a global workspace. This is what
            // produces genuine integration (rising total correlation): the
            // activations cluster into coherent whole-system states (calm-
            // deliberate, stressed-reactive, surprised-reflective) instead of
            // firing at random. And because the coupling is driven by the body-
            // model and self-model, which SHARPEN as they accumulate experience
            // (prediction error concentrates on genuinely novel situations), the
            // dependency — hence phi — rises as the system runs. Three
            // mechanisms, each a real neuroscience principle, not metric-tuning:
            let stress = actual.stress(); // actual interoceptive load this task
            let surprise = self_pred_err.max(intero_err); // prediction error (novelty)
            let affect_signal = self
                .affect
                .lock()
                .map(|a| a.valence.abs().max(a.arousal + 0.6 * stress))
                .unwrap_or(0.0);
            // HOMEOSTATIC gating (anti-saturation): each signal-driven module
            // fires when its signal is ELEVATED relative to its own running
            // baseline, not an absolute threshold. The overnight data showed
            // fixed thresholds saturate — as load/surprise drift up over a run,
            // flags pin on, mean_active climbs, and BOTH phi and LZc collapse.
            // Gain control keeps each module discriminating (~half on) regardless
            // of absolute drift, while PRESERVING coupling (shared signals still
            // co-drive modules; only the threshold adapts).
            let base = self
                .signal_baselines
                .lock()
                .map(|b| *b)
                .unwrap_or_else(|_| SignalBaselines::prior());
            let deliberate = stress < base.stress; // below own baseline = calm (System 2)
            let surprised = surprise > base.surprise; // above own baseline = novel
            let affect_active = affect_signal > base.affect;
            let reflected = task.steps.iter().any(|s| {
                matches!(
                    s.action.tool_name.as_str(),
                    "meta.observe" | "agent.critic" | "mirror.review"
                )
            });

            // Body fires under relative load (System-1 regime); couples to
            // cognition through the same boundary (body on ⟺ cognition suppressed).
            let body_active = !deliberate;

            // Update the homeostatic baselines AFTER gating this decision, so the
            // thresholds track the signals' running means going forward.
            if let Ok(mut b) = self.signal_baselines.lock() {
                b.update(stress, surprise, affect_signal);
            }

            // (2) Predictive-coding novelty (hippocampal/ACC): high prediction
            // error gates deep causal-trace formation — so causal co-activates
            // with the surprise-driven modules below, not on mere existence.
            let causal_active = surprised
                && self
                    .memory
                    .causal_traces
                    .extract_patterns(Some(&category_name), 3, 0.6, 10_000)
                    .map(|p| !p.is_empty())
                    .unwrap_or(false);
            // (3) Self-model engages when the agent explicitly self-reflects
            // (meta.observe / mirror critic) OR when it surprised itself (high
            // self-prediction error) — coupling the self-model to the same
            // surprise signal that drives episodic encoding and causal analysis.
            let self_model_active = reflected || surprised;

            // (4) Hippocampal novelty gating: episodic ENCODING is salient only
            // when the event was surprising or under explicit reflection — so
            // episodic co-activates with self-model and causal (shared surprise)
            // instead of being pinned on for every recall.
            let episodic_active = ice_hit && (surprised || reflected);

            // (5) Yerkes-Dodson / GWT System 1↔2: cognition broadcasts to the
            // workspace only in the deliberate (low-stress) regime; under load
            // it is suppressed. Cognition thus couples to body and affect.
            let cognition_active = cognition_hit && deliberate;

            // DECOUPLED control (PCI anaesthesia arm): when coupling is off, each
            // module fires on its OWN signal with no shared-signal gating — the
            // surprise signal no longer jointly drives episodic/causal/self_model,
            // and stress no longer ties body to cognition. Same modules, same
            // baselines; only the cross-module dependency is removed. The PCI
            // contrast (coupled complexity vs this) is the wake-vs-anaesthesia test.
            let activation = if self.coupling_enabled {
                ModuleActivation {
                    episodic: episodic_active,
                    semantic: used_memory_tool,
                    cognition: cognition_active,
                    affect: affect_active,
                    body: body_active,
                    causal: causal_active,
                    self_model: self_model_active,
                }
            } else {
                ModuleActivation {
                    episodic: ice_hit,
                    semantic: used_memory_tool,
                    cognition: cognition_hit,
                    affect: affect_active,
                    body: stress > base.stress,
                    causal: self
                        .memory
                        .causal_traces
                        .extract_patterns(Some(&category_name), 3, 0.6, 10_000)
                        .map(|p| !p.is_empty())
                        .unwrap_or(false),
                    self_model: reflected,
                }
            };
            if let Err(e) = self
                .memory
                .phi
                .record_activation(self.current_round, &activation)
            {
                warn!("react: failed to record phi activation: {e}");
            }
        }
    }

    /// M2: force a final answer out of the observations already gathered, instead of
    /// nudging a stuck model (which tends to keep thrashing until forfeit). Returns the
    /// synthesized answer, or None if there's nothing to synthesize or the model judges
    /// the evidence insufficient.
    async fn synthesize_final_answer(
        &self,
        task: &TaskNode,
        opts: &ModelOptions,
    ) -> Option<String> {
        let summary = successful_observation_summary(task, 8, 1400);
        if summary.trim().is_empty() {
            return None;
        }
        let system = "You are finishing a task. Using ONLY the observations already \
            gathered below, write the final answer: concise, directly stating the facts \
            the task asked for. Do not propose any more tool calls. If the observations \
            are genuinely insufficient to answer, reply with exactly the single word \
            INSUFFICIENT.";
        let prompt = format!(
            "TASK:\n{}\n\nOBSERVATIONS GATHERED:\n{}\n\nFINAL ANSWER:",
            task.description, summary
        );
        match self
            .ollama
            .generate(&prompt, Some(system), Some(opts.clone()))
            .await
        {
            Ok(resp) => {
                let (_, ans) = resp.split_thinking();
                let ans = ans.trim();
                if ans.is_empty() || ans.to_uppercase().contains("INSUFFICIENT") {
                    None
                } else {
                    Some(ans.chars().take(4000).collect())
                }
            }
            Err(_) => None,
        }
    }

    /// Run one attempt. Returns Ok(true) on success, Ok(false) on failure.
    async fn run_attempt(
        &self,
        task: &mut TaskNode,
        ice_examples: &[String],
        cognition_context: &[String],
        num_ctx: u32,
    ) -> Result<bool> {
        const MAX_STEPS: usize = 20;
        const SYNTHESIS_CHECKPOINT_STEP: usize = 14;
        const FORFEIT_AFTER_SYNTHESIS_STEP: usize = 18;
        let scope = match &self.workspace_override {
            Some(root) => PermissionScope::default_autonomous().with_workspace_root(root.clone()),
            None => PermissionScope::default_autonomous(),
        };
        let executor = ToolExecutor::new(Arc::clone(&self.registry))
            .with_workspace_root(scope.workspace_root.clone())
            .with_memory(Arc::clone(&self.memory))
            .with_ollama(Arc::clone(&self.ollama));
        let audit = AuditStore::new(Arc::clone(&self.memory.db));
        let session_id = Uuid::new_v4();
        self.emit_event(
            Some(session_id),
            Some(task.id),
            "react.session.started",
            "started ReAct session",
            json!({"num_ctx": num_ctx}),
        );

        // Circuit breaker: pause after 3 consecutive tool failures
        let mut consecutive_failures: u8 = 0;
        let mut escalated = false;
        let auto_repair_on = auto_repair_enabled_from_env();
        let mut synthesis_forced = false;
        // M2: count exact-duplicate-action blocks; thrashing on duplicates triggers an
        // early forced synthesis instead of burning steps until the forfeit guard fires.
        let mut duplicate_blocks: u32 = 0;
        // Consecutive duplicates (reset on any real progress) drive the temperature
        // escalation that breaks the greedy regenerate-the-same-stuck-action loop.
        let mut consecutive_duplicates: u32 = 0;

        // LCAP: apply context budget
        let mut react_opts = ModelOptions::for_react();
        react_opts.num_ctx = Some(num_ctx);

        // Phase 2: native tool-calling path (structured tool_calls, no free-form text parsing).
        // Default OFF — the ReAct text path stays the proven default until this is gated green.
        let native_tools = std::env::var("PROFESSOR_X_NATIVE_TOOLS")
            .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);

        for step_idx in 0..MAX_STEPS {
            if self.cancel.is_cancelled() {
                return Ok(false);
            }

            if should_forfeit_after_synthesis(
                step_idx,
                synthesis_forced,
                FORFEIT_AFTER_SYNTHESIS_STEP,
            ) {
                warn!(
                    "react: synthesis/forfeit guard stopped task '{}' before max steps",
                    task.description
                );
                self.emit_event(
                    Some(session_id),
                    Some(task.id),
                    "react.forfeit",
                    "synthesis/forfeit guard stopped task before max steps",
                    json!({"step": step_idx + 1, "max_steps": MAX_STEPS}),
                );
                return Ok(false);
            }

            if should_force_synthesis(step_idx, synthesis_forced, SYNTHESIS_CHECKPOINT_STEP)
                || (!synthesis_forced && duplicate_blocks >= 3)
            {
                synthesis_forced = true;
                if task_requires_file_edit(task) && !has_successful_file_mutation(task) {
                    let guidance = edit_required_synthesis_guidance(task);
                    self.emit_event(
                        Some(session_id),
                        Some(task.id),
                        "react.synthesis_deferred",
                        "edit-required task has no successful file mutation yet",
                        json!({"step": step_idx + 1, "max_steps": MAX_STEPS}),
                    );
                    task.steps.push(ExecutionStep {
                        index: (task.steps.len() + 1) as u32,
                        thought: "loop guard: task requires a file edit before final synthesis"
                            .to_string(),
                        action: Action {
                            tool_name: "auto.synthesize".to_string(),
                            params: json!({}),
                            risk_score: 0,
                        },
                        observation: guidance,
                        timestamp: Utc::now(),
                    });
                    duplicate_blocks = 0;
                    consecutive_duplicates = 0;
                    let _ = TaskRunStore::new(Arc::clone(&self.memory.db)).step_recorded(task);
                    continue;
                }
                // M2: directly synthesize the final answer from the gathered observations
                // and finish, rather than nudging a stuck model into more thrash → forfeit.
                if let Some(answer) = self.synthesize_final_answer(task, &react_opts).await {
                    task.steps.push(ExecutionStep {
                        index: (task.steps.len() + 1) as u32,
                        thought: "loop guard: synthesizing final answer from gathered observations"
                            .to_string(),
                        action: Action {
                            tool_name: "finish".to_string(),
                            params: json!({ "answer": answer }),
                            risk_score: 0,
                        },
                        observation: Observation {
                            success: true,
                            output: answer,
                            error: None,
                            tokens_used: 0,
                            execution_ms: 0,
                            artifacts: Vec::new(),
                        },
                        timestamp: Utc::now(),
                    });
                    let _ = TaskRunStore::new(Arc::clone(&self.memory.db)).step_recorded(task);
                    self.emit_event(
                        Some(session_id),
                        Some(task.id),
                        "react.synthesis_finish",
                        "forced synthesis produced a final answer",
                        json!({"step": step_idx + 1, "trigger": if duplicate_blocks >= 3 { "duplicates" } else { "checkpoint" }}),
                    );
                    return Ok(true);
                }
                // Fallback: nudge (old behavior) if nothing could be synthesized yet.
                let guidance = synthesis_guidance(task);
                self.emit_event(
                    Some(session_id),
                    Some(task.id),
                    "react.synthesis_required",
                    "forcing synthesis or explicit failure before max steps",
                    json!({"step": step_idx + 1, "max_steps": MAX_STEPS}),
                );
                task.steps.push(ExecutionStep {
                    index: (task.steps.len() + 1) as u32,
                    thought: "loop guard: enough actions have run; forcing synthesis or forfeit"
                        .to_string(),
                    action: Action {
                        tool_name: "auto.synthesize".to_string(),
                        params: json!({}),
                        risk_score: 0,
                    },
                    observation: guidance,
                    timestamp: Utc::now(),
                });
                let _ = TaskRunStore::new(Arc::clone(&self.memory.db)).step_recorded(task);
                continue;
            }

            // Build the full prompt for this step
            // M2: a duplicate-blocked action means the model is stuck regenerating the same
            // step at low temperature (greedy loop) — the real repo-fix failure mode (it
            // re-runs fs.list forever and never reaches read→edit). Escalate temperature on
            // the retry to break determinism so it tries a genuinely different action.
            let mut step_opts = react_opts.clone();
            if consecutive_duplicates > 0 {
                // Aggressive: jump to high temperature immediately so the greedy loop breaks
                // on the very next step rather than grinding through several duplicates.
                let t = (0.9 + 0.2 * consecutive_duplicates as f32).min(1.3);
                step_opts.temperature = Some(t);
                step_opts.top_p = Some(0.98);
            }

            // Ask the model for the next step. Native tool-calling (PROFESSOR_X_NATIVE_TOOLS=1) gets
            // back structured tool_calls — no free-form text parsing; otherwise the classic ReAct
            // text path. Both yield (answer_text, Option<ParsedStep>) so all downstream gates,
            // execution, and recording are identical.
            let (answer, parsed_opt) = if native_tools {
                self.native_step(task, ice_examples, cognition_context, step_opts)
                    .await?
            } else {
                let prompt = self.build_step_prompt(task, ice_examples, cognition_context);
                let system_prompt = self.prompt_override.as_deref().unwrap_or(SYSTEM_PROMPT);
                let resp = self
                    .ollama
                    .generate(&prompt, Some(system_prompt), Some(step_opts))
                    .await?;
                let (_, answer) = resp.split_thinking();
                let parsed = parse_react_step(&answer);
                (answer, parsed)
            };

            self.emit_event(
                Some(session_id),
                Some(task.id),
                "llm.response",
                format!("model response received ({} chars)", answer.len()),
                json!({
                    "step": step_idx + 1,
                    "response_chars": answer.len(),
                    "preview": truncate(&answer, 300),
                }),
            );

            debug!(
                "react step {}: raw response length={}",
                step_idx + 1,
                answer.len()
            );

            // Parse Thought / Action / Action Input
            match parsed_opt {
                None => {
                    // Model output didn't match expected format — check for FINISH signal
                    if answer.to_lowercase().contains("task complete")
                        || answer.to_lowercase().contains("finish")
                        || answer.to_lowercase().contains("final answer")
                    {
                        self.emit_event(
                            Some(session_id),
                            Some(task.id),
                            "llm.finish_detected",
                            "finish detected in unparsed model output",
                            json!({"step": step_idx + 1}),
                        );
                        return Ok(true);
                    }
                    warn!("react: could not parse step output, retrying step");
                    self.emit_event(
                        Some(session_id),
                        Some(task.id),
                        "llm.parse_failed",
                        "could not parse ReAct step",
                        json!({"step": step_idx + 1, "preview": truncate(&answer, 300)}),
                    );
                    continue;
                }

                Some(parsed) => {
                    self.emit_event(
                        Some(session_id),
                        Some(task.id),
                        "tool.requested",
                        format!("requested tool '{}'", parsed.tool_name),
                        json!({
                            "step": step_idx + 1,
                            "tool": parsed.tool_name,
                            "params": parsed.params,
                        }),
                    );
                    // Special finish actions. A bare `finish {}` is not enough:
                    // HIRO showed the agent can use tools correctly and still
                    // get p_correct=0 by terminating without reporting the
                    // requested answer. Treat empty finish as a retryable
                    // interface error and make the expected payload explicit.
                    if parsed.tool_name == "finish" || parsed.tool_name == "done" {
                        if let Some(answer) = finish_answer_from_params(&parsed.params) {
                            if task_requires_file_edit(task) && !has_successful_file_mutation(task)
                            {
                                self.emit_event(
                                    Some(session_id),
                                    Some(task.id),
                                    "task.finish_rejected",
                                    "edit-required task requested finish before successful file mutation",
                                    json!({"step": step_idx + 1}),
                                );
                                let guidance = edit_required_synthesis_guidance(task);
                                task.steps.push(ExecutionStep {
                                    index: (step_idx + 1) as u32,
                                    thought: parsed.thought,
                                    action: Action {
                                        tool_name: parsed.tool_name,
                                        params: parsed.params,
                                        risk_score: 0,
                                    },
                                    observation: guidance,
                                    timestamp: Utc::now(),
                                });
                                let _ = TaskRunStore::new(Arc::clone(&self.memory.db))
                                    .step_recorded(task);
                                continue;
                            }
                            if let Some(verifier) = &self.verifier {
                                if verifier_failed_since_latest_file_mutation(task) {
                                    self.emit_event(
                                        Some(session_id),
                                        Some(task.id),
                                        "task.finish_rejected",
                                        "verifier already failed and no newer edit was made",
                                        json!({"step": step_idx + 1}),
                                    );
                                    task.steps.push(ExecutionStep {
                                        index: (step_idx + 1) as u32,
                                        thought: parsed.thought.clone(),
                                        action: Action {
                                            tool_name: parsed.tool_name.clone(),
                                            params: parsed.params.clone(),
                                            risk_score: 0,
                                        },
                                        observation: verifier_requires_new_edit_observation(),
                                        timestamp: Utc::now(),
                                    });
                                    let _ = TaskRunStore::new(Arc::clone(&self.memory.db))
                                        .step_recorded(task);
                                    continue;
                                }

                                let mut result = verifier.run();
                                if !result.passed {
                                    if let Some(repair) =
                                        try_python_verifier_repair(verifier, &result)
                                    {
                                        self.emit_event(
                                            Some(session_id),
                                            Some(task.id),
                                            "task.verifier.auto_repaired",
                                            "verifier traceback repair edited the workspace",
                                            json!({
                                                "step": step_idx + 1,
                                                "summary": repair,
                                            }),
                                        );
                                        result = verifier.run();
                                    }
                                }
                                if !result.passed {
                                    self.emit_event(
                                        Some(session_id),
                                        Some(task.id),
                                        "task.finish_rejected",
                                        "verifier failed after finish request",
                                        json!({
                                            "step": step_idx + 1,
                                            "exit_code": result.exit_code,
                                            "expect_exit": verifier.expect_exit,
                                        }),
                                    );
                                    task.steps.push(ExecutionStep {
                                        index: (step_idx + 1) as u32,
                                        thought: parsed.thought,
                                        action: Action {
                                            tool_name: parsed.tool_name,
                                            params: parsed.params,
                                            risk_score: 0,
                                        },
                                        observation: verifier_failed_observation(verifier, &result),
                                        timestamp: Utc::now(),
                                    });
                                    let _ = TaskRunStore::new(Arc::clone(&self.memory.db))
                                        .step_recorded(task);
                                    continue;
                                }
                                self.emit_event(
                                    Some(session_id),
                                    Some(task.id),
                                    "task.verifier.passed",
                                    "verifier passed before finish",
                                    json!({
                                        "step": step_idx + 1,
                                        "exit_code": result.exit_code,
                                        "expect_exit": verifier.expect_exit,
                                    }),
                                );
                            }
                            let step = ExecutionStep {
                                index: (step_idx + 1) as u32,
                                thought: parsed.thought,
                                action: Action {
                                    tool_name: parsed.tool_name,
                                    params: parsed.params,
                                    risk_score: 0,
                                },
                                observation: Observation {
                                    success: true,
                                    output: answer,
                                    error: None,
                                    tokens_used: 0,
                                    execution_ms: 0,
                                    artifacts: Vec::new(),
                                },
                                timestamp: Utc::now(),
                            };
                            task.steps.push(step);
                            let _ =
                                TaskRunStore::new(Arc::clone(&self.memory.db)).step_recorded(task);
                            self.emit_event(
                                Some(session_id),
                                Some(task.id),
                                "task.finish_requested",
                                "model requested finish with answer",
                                json!({"step": step_idx + 1}),
                            );
                            return Ok(true);
                        }

                        self.emit_event(
                            Some(session_id),
                            Some(task.id),
                            "task.finish_rejected",
                            "model requested empty finish",
                            json!({"step": step_idx + 1}),
                        );
                        let guidance = Observation {
                            success: false,
                            output: "FINISH REJECTED — include the actual answer in the action input, e.g. `Action Input: {\"answer\":\"<concise result with the requested facts>\"}`. Use the observations you already gathered; do not finish with `{}`.".to_string(),
                            error: Some("empty finish has no answer".to_string()),
                            tokens_used: 0,
                            execution_ms: 0,
                            artifacts: Vec::new(),
                        };
                        task.steps.push(ExecutionStep {
                            index: (step_idx + 1) as u32,
                            thought: parsed.thought,
                            action: Action {
                                tool_name: parsed.tool_name,
                                params: parsed.params,
                                risk_score: 0,
                            },
                            observation: guidance,
                            timestamp: Utc::now(),
                        });
                        let _ = TaskRunStore::new(Arc::clone(&self.memory.db)).step_recorded(task);
                        continue;
                    }
                    if parsed.tool_name == "fail" {
                        self.emit_event(
                            Some(session_id),
                            Some(task.id),
                            "task.fail_requested",
                            "model requested failure",
                            json!({"step": step_idx + 1}),
                        );
                        return Ok(false);
                    }

                    // scratchpad.write — self-managed working memory. Handled by
                    // the loop (not the executor); updates the persistent plan
                    // injected into every prompt. Does not consume a real step.
                    if parsed.tool_name == "scratchpad.write" || parsed.tool_name == "plan.write" {
                        let content = parsed
                            .params
                            .get("content")
                            .and_then(|v| v.as_str())
                            .or_else(|| parsed.params.get("plan").and_then(|v| v.as_str()))
                            .unwrap_or("")
                            .to_string();
                        if let Ok(mut sp) = self.scratchpad.lock() {
                            *sp = content.chars().take(2000).collect();
                        }
                        self.emit_event(
                            Some(session_id),
                            Some(task.id),
                            "scratchpad.updated",
                            "updated working plan",
                            json!({"step": step_idx + 1, "chars": content.len()}),
                        );
                        continue;
                    }

                    // agent.delegate — spawn a fresh sub-agent on a focused
                    // sub-goal and fold its result back as an observation. Real
                    // task decomposition: the child has its own ReAct loop,
                    // memory access, and tool set, but cannot delegate further
                    // (depth cap). The parent reasons over the child's answer.
                    if parsed.tool_name == "agent.delegate" || parsed.tool_name == "agent.spawn" {
                        let goal = parsed
                            .params
                            .get("goal")
                            .or_else(|| parsed.params.get("task"))
                            .or_else(|| parsed.params.get("description"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let obs = self.delegate(&goal, session_id, task.id).await;
                        self.record_local_step(task, step_idx, session_id, parsed, obs);
                        continue;
                    }

                    // agent.critic — the mirror. A second agent-perspective
                    // reviews THIS agent's trajectory so far and returns a
                    // critique (a self observing the self). Single evaluative
                    // pass, not a full loop. The consciousness tie-in to the
                    // self-model seeds: metacognition as an explicit second view.
                    if parsed.tool_name == "agent.critic" || parsed.tool_name == "mirror.review" {
                        let obs = self.critique(task).await;
                        self.record_local_step(task, step_idx, session_id, parsed, obs);
                        continue;
                    }

                    // tot.search — Tree-of-Thoughts deliberate branching. Propose
                    // several approaches, value them, commit to the best. For hard
                    // tasks where greedily following the first idea dead-ends.
                    if parsed.tool_name == "tot.search" || parsed.tool_name == "deliberate" {
                        let branches = parsed
                            .params
                            .get("branches")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(3) as usize;
                        let obs = self.tot_search(task, branches).await;
                        self.record_local_step(task, step_idx, session_id, parsed, obs);
                        continue;
                    }

                    // Duplicate-action breaker: if the agent re-issues an action
                    // it already ran this attempt (same tool + same params), do
                    // NOT re-execute. Hand back the prior result with a firm
                    // nudge. This kills the re-read/re-list loops at the source —
                    // the model is told it already has the answer and must use it
                    // or change approach.
                    if let Some(prior) = task.steps.iter().find(|s| {
                        s.action.tool_name == parsed.tool_name && s.action.params == parsed.params
                    }) {
                        let prior_out = if prior.observation.success {
                            prior
                                .observation
                                .output
                                .chars()
                                .take(800)
                                .collect::<String>()
                        } else {
                            format!(
                                "(it failed: {})",
                                prior.observation.error.as_deref().unwrap_or("unknown")
                            )
                        };
                        // Escalate the nudge once the model is visibly stuck (2nd+ consecutive
                        // duplicate): name the concrete next action instead of a soft "do
                        // something different" that weak models ignore.
                        let nudge = if consecutive_duplicates >= 1 {
                            format!(
                                "STOP — you have now repeated `{}` {} times and it is BLOCKED. You \
                                 ALREADY have its result:\n{}\n\nYou are stuck in a loop. Your next \
                                 action MUST be different: if you have not yet read the target file, \
                                 use `fs.read` or `fs.window_open` on it; once you have read it, make \
                                 the fix with `fs.hash_edit` or `fs.write`. Do NOT call `{}` again.",
                                parsed.tool_name,
                                consecutive_duplicates + 1,
                                prior_out,
                                parsed.tool_name
                            )
                        } else {
                            format!(
                                "DUPLICATE ACTION — you already ran `{}` with these exact inputs. \
                                 Its result was:\n{}\n\nDo NOT run it again. Use this result to make \
                                 progress, or take a DIFFERENT action. If the task is complete, call finish.",
                                parsed.tool_name, prior_out
                            )
                        };
                        self.emit_event(
                            Some(session_id),
                            Some(task.id),
                            "react.duplicate_action",
                            format!(
                                "blocked duplicate '{}' — returned prior result with nudge",
                                parsed.tool_name
                            ),
                            json!({"step": step_idx + 1, "tool": parsed.tool_name}),
                        );
                        let step = ExecutionStep {
                            index: (step_idx + 1) as u32,
                            thought: parsed.thought,
                            action: Action {
                                tool_name: parsed.tool_name,
                                params: parsed.params,
                                risk_score: 0,
                            },
                            observation: Observation {
                                success: false,
                                output: nudge,
                                error: Some("duplicate action blocked".to_string()),
                                tokens_used: 0,
                                execution_ms: 0,
                                artifacts: Vec::new(),
                            },
                            timestamp: Utc::now(),
                        };
                        task.steps.push(step);
                        duplicate_blocks += 1;
                        consecutive_duplicates += 1;
                        continue;
                    }

                    // Gate the action through policyd
                    let gate = self
                        .policy
                        .gate(&parsed.tool_name, &parsed.params, session_id, &scope)
                        .await;

                    // Write audit entry
                    let _ = audit.append(
                        session_id,
                        Some(task.id),
                        &parsed.tool_name,
                        &parsed.params,
                        gate.risk_score,
                        gate.decision.clone(),
                        &gate.reason,
                        None,
                    );
                    self.emit_event(
                        Some(session_id),
                        Some(task.id),
                        match gate.decision {
                            Decision::Allow => "policy.allowed",
                            Decision::Deny => "policy.denied",
                            Decision::PendingApproval => "policy.pending",
                        },
                        format!(
                            "policy {:?} for '{}': {}",
                            gate.decision,
                            parsed.tool_name,
                            truncate(&gate.reason, 140)
                        ),
                        json!({
                            "step": step_idx + 1,
                            "tool": parsed.tool_name,
                            "risk_score": gate.risk_score,
                            "reason": gate.reason,
                        }),
                    );

                    let observation = match gate.decision {
                        Decision::Deny => {
                            consecutive_failures += 1;
                            Observation::denied(&gate.reason)
                        }
                        Decision::PendingApproval => {
                            // Tool needs human approval — inject as observation and continue
                            Observation::denied(&format!(
                                "tool '{}' requires human approval (risk={}). \
                                 Use a lower-risk alternative or wait for approval.",
                                parsed.tool_name, gate.risk_score
                            ))
                        }
                        Decision::Allow => {
                            let action = Action {
                                tool_name: parsed.tool_name.clone(),
                                params: parsed.params.clone(),
                                risk_score: gate.risk_score,
                            };
                            self.emit_event(
                                Some(session_id),
                                Some(task.id),
                                "tool.started",
                                format!(
                                    "running tool '{}'{}",
                                    parsed.tool_name,
                                    tool_params_preview(&parsed.params)
                                        .map(|preview| format!(" :: {preview}"))
                                        .unwrap_or_default()
                                ),
                                json!({
                                    "step": step_idx + 1,
                                    "tool": parsed.tool_name,
                                    "params_preview": tool_params_preview(&parsed.params),
                                }),
                            );
                            let obs = executor.execute(&action).await;
                            // A real (non-duplicate) tool ran — the greedy-loop streak is broken.
                            consecutive_duplicates = 0;
                            let exec_reason = if obs.success {
                                consecutive_failures = 0;
                                "executed"
                            } else {
                                consecutive_failures += 1;
                                obs.error.as_deref().unwrap_or("execution failed")
                            };
                            let _ = audit.append(
                                session_id,
                                Some(task.id),
                                &parsed.tool_name,
                                &parsed.params,
                                gate.risk_score,
                                gate.decision.clone(),
                                exec_reason,
                                Some(obs.execution_ms),
                            );
                            self.emit_event(
                                Some(session_id),
                                Some(task.id),
                                if obs.success {
                                    "tool.succeeded"
                                } else {
                                    "tool.failed"
                                },
                                format!(
                                    "tool '{}' {} in {}ms",
                                    parsed.tool_name,
                                    if obs.success { "succeeded" } else { "failed" },
                                    obs.execution_ms
                                ),
                                json!({
                                    "step": step_idx + 1,
                                    "tool": parsed.tool_name,
                                    "success": obs.success,
                                    "execution_ms": obs.execution_ms,
                                    "output_preview": truncate(&obs.output, 300),
                                    "error": obs.error,
                                    "artifacts": obs.artifacts,
                                }),
                            );
                            obs
                        }
                    };

                    let observation = if observation.success || !auto_repair_on {
                        observation
                    } else {
                        augment_with_repair_hint(
                            observation,
                            &parsed.tool_name,
                            &parsed.params,
                            &scope,
                        )
                    };

                    // Record step on TaskNode and MermaidCanvas
                    let step = ExecutionStep {
                        index: (step_idx + 1) as u32,
                        thought: parsed.thought,
                        action: Action {
                            tool_name: parsed.tool_name.clone(),
                            params: parsed.params.clone(),
                            risk_score: gate.risk_score,
                        },
                        observation: observation.clone(),
                        timestamp: Utc::now(),
                    };
                    {
                        let param_preview = tool_params_preview(&parsed.params).unwrap_or_default();
                        if let Ok(mut canvas) = self.canvas.lock() {
                            canvas.record_canvas_step(
                                &parsed.tool_name,
                                &param_preview,
                                observation.success,
                            );
                        }
                    }
                    task.steps.push(step);
                    let _ = TaskRunStore::new(Arc::clone(&self.memory.db)).step_recorded(task);

                    // Check if the observation signals completion
                    if is_completion_signal(&observation) {
                        return Ok(true);
                    }

                    if consecutive_failures >= 3 {
                        if auto_repair_on && !escalated {
                            escalated = true;
                            consecutive_failures = 0;
                            self.emit_event(
                                Some(session_id),
                                Some(task.id),
                                "react.escalation",
                                "stuck; escalating to self-review for a fresh plan",
                                json!({"step": step_idx + 1}),
                            );
                            let critique = self.critique(task).await;
                            let guidance = Observation {
                                success: false,
                                output: format!(
                                    "AUTO-REPAIR: several consecutive actions failed. A reviewer diagnosed the current trajectory:\n{}\n\nStep back and try a different approach. Re-read files before editing them, keep paths inside the workspace, and verify each step.",
                                    critique.output
                                ),
                                error: None,
                                tokens_used: 0,
                                execution_ms: 0,
                                artifacts: Vec::new(),
                            };
                            task.steps.push(ExecutionStep {
                                index: (task.steps.len() + 1) as u32,
                                thought: "auto-repair: stuck, requesting a fresh diagnosis"
                                    .to_string(),
                                action: Action {
                                    tool_name: "auto.repair".to_string(),
                                    params: json!({}),
                                    risk_score: 0,
                                },
                                observation: guidance,
                                timestamp: Utc::now(),
                            });
                            let _ =
                                TaskRunStore::new(Arc::clone(&self.memory.db)).step_recorded(task);
                        } else {
                            warn!(
                                "react: circuit breaker tripped (failures persisted after escalation) on task '{}'",
                                task.description
                            );
                            self.emit_event(
                                Some(session_id),
                                Some(task.id),
                                "react.circuit_breaker",
                                "circuit breaker tripped; failures persisted after self-review",
                                json!({"step": step_idx + 1}),
                            );
                            return Ok(false);
                        }
                    }
                }
            }
        }

        // MAX_STEPS reached without finishing
        warn!(
            "react: max steps ({MAX_STEPS}) reached for task '{}'",
            task.description
        );
        self.emit_event(
            Some(session_id),
            Some(task.id),
            "react.max_steps",
            format!("max steps ({MAX_STEPS}) reached"),
            json!({"max_steps": MAX_STEPS}),
        );
        Ok(false)
    }

    fn emit_event(
        &self,
        session_id: Option<Uuid>,
        task_id: Option<Uuid>,
        event_type: &str,
        summary: impl AsRef<str>,
        payload: Value,
    ) {
        if let Some(events) = &self.events {
            if let Err(e) = events.append(session_id, task_id, event_type, summary, payload) {
                warn!("agent event write failed: {e}");
            }
        }
    }

    /// Record a step produced by a loop-intercepted tool (delegate/critic/tot)
    /// AND emit a `tool.succeeded` event with an output preview, so these tools
    /// are visible in the live stream and persisted like executor-dispatched
    /// ones — they bypass the executor, so without this their results are
    /// invisible to the event log and transcripts.
    fn record_local_step(
        &self,
        task: &mut TaskNode,
        step_idx: usize,
        session_id: Uuid,
        parsed: ParsedStep,
        obs: Observation,
    ) {
        let preview: String = obs.output.chars().take(240).collect();
        let tool = parsed.tool_name.clone();
        self.emit_event(
            Some(session_id),
            Some(task.id),
            "tool.succeeded",
            format!(
                "{tool}: {}",
                preview
                    .replace('\n', " ")
                    .chars()
                    .take(80)
                    .collect::<String>()
            ),
            json!({
                "step": step_idx + 1,
                "tool": tool,
                "intercepted": true,
                "success": obs.success,
                "output_preview": preview,
            }),
        );
        task.steps.push(ExecutionStep {
            index: (step_idx + 1) as u32,
            thought: parsed.thought,
            action: Action {
                tool_name: parsed.tool_name,
                params: parsed.params,
                risk_score: 0,
            },
            observation: obs,
            timestamp: Utc::now(),
        });
    }

    fn record_transcript(
        &self,
        task: &TaskNode,
        status: &str,
        summary: &str,
    ) -> Option<std::path::PathBuf> {
        let (Some(transcripts), Some(events)) = (&self.transcripts, &self.events) else {
            return None;
        };
        match transcripts.record_task(task, status, summary, events) {
            Ok(path) => {
                self.emit_event(
                    None,
                    Some(task.id),
                    "transcript.written",
                    format!("task transcript written to {}", path.display()),
                    json!({
                        "path": path,
                        "status": status,
                        "summary": summary,
                    }),
                );
                Some(path)
            }
            Err(e) => {
                warn!("task transcript write failed: {e}");
                None
            }
        }
    }

    /// Shared context block — memd identity/working memory, workspace grounding, affect/body,
    /// scratchpad, ICE examples, cognition, learned strategies, the task, and reflections —
    /// i.e. everything EXCEPT history, the tool list, and the ReAct suffix. Used by BOTH the text
    /// prompt and the native chat path so native tool-calling gets the same scaffolding (Stage 1).
    fn build_context_parts(
        &self,
        task: &TaskNode,
        ice_examples: &[String],
        cognition_context: &[String],
    ) -> Vec<String> {
        let mut parts = Vec::new();

        // Pinned identity + working memory from memd
        let ctx_prefix = self
            .memory
            .build_context_prefix("current")
            .unwrap_or_default();
        if !ctx_prefix.is_empty() {
            parts.push(ctx_prefix);
        }

        // Workspace grounding: tell the agent where it is and what's there, so
        // it forms correct relative paths instead of guessing (the root cause
        // of the round-0 path-confusion loops). Relative tool paths resolve
        // against this directory.
        parts.push(workspace_context());

        // Affect (H16): inject emotional state when non-trivial so the model
        // can condition on its own performance trajectory
        if let Ok(aff) = self.affect.lock() {
            if aff.valence.abs() > 0.05 || aff.arousal > 0.05 {
                parts.push(format!(
                    "<affect state=\"{}\" valence=\"{:.2}\" arousal=\"{:.2}\" />",
                    state_label(aff.valence, aff.arousal),
                    aff.valence,
                    aff.arousal,
                ));
            }
        }

        // Seed 4 (interoception): inject the predicted computational body state.
        // Under stress the model is told to conserve (System 1); when fresh, explore.
        if let Ok(body) = self.body_prediction.lock() {
            parts.push(body.to_prompt_fragment());
        }

        // Self-managed working memory (the agent's running plan/notes).
        if let Ok(sp) = self.scratchpad.lock() {
            if !sp.trim().is_empty() {
                parts.push(format!("<scratchpad>\n{}\n</scratchpad>", sp.trim()));
            }
        }

        // ICE: similar past tasks
        if !ice_examples.is_empty() {
            let examples = ice_examples
                .iter()
                .enumerate()
                .map(|(i, ex)| format!("Example {}: {ex}", i + 1))
                .collect::<Vec<_>>()
                .join("\n\n");
            parts.push(format!("<examples>\n{examples}\n</examples>"));
        }

        // Cognition: relevant knowledge
        if !cognition_context.is_empty() {
            let knowledge = cognition_context.join("\n- ");
            parts.push(format!("<knowledge>\n- {knowledge}\n</knowledge>"));
        }

        // Seed 2 read-back: learned causal tool-sequences that worked before for
        // this kind of task. The agent's own accumulated self-knowledge guiding
        // its next action.
        if let Ok(hint) = self.causal_hint.lock() {
            if !hint.is_empty() {
                parts.push(format!(
                    "<learned-strategies>\n{hint}\n</learned-strategies>"
                ));
            }
        }

        // Current task
        parts.push(format!("<task>\n{}\n</task>", task.description));

        // Reflexion buffer from prior failed attempts
        if let Some(refs) = task.reflections_text() {
            parts.push(format!("<reflections>\n{refs}\n</reflections>"));
        }

        parts
    }

    fn build_step_prompt(
        &self,
        task: &TaskNode,
        ice_examples: &[String],
        cognition_context: &[String],
    ) -> String {
        let mut parts = self.build_context_parts(task, ice_examples, cognition_context);

        // Prior steps this attempt. Recent steps keep their observation output
        // so the agent acts on tool results instead of re-running them blindly.
        // Older steps are compacted into a bounded ledger + Mermaid overview,
        // preserving enough state to avoid loops without letting long outputs
        // consume the local model's context window.
        const RECENT_FULL: usize = 3;
        if !task.steps.is_empty() {
            let canvas = self
                .canvas
                .lock()
                .ok()
                .filter(|canvas| !canvas.is_empty())
                .map(|canvas| canvas.to_mermaid());
            let mut history = compact_history(task, canvas.as_deref(), RECENT_FULL);
            history.push_str(&task.recent_steps_text(RECENT_FULL));
            parts.push(format!("<history>\n{history}\n</history>"));
        }

        // Available tools — built-ins plus any dynamically registered MCP tools.
        parts.push(TOOLS_DESCRIPTION.to_string());
        if let Some(mcp) = self.mcp_tools_description() {
            parts.push(mcp);
        }

        // ReAct prompt suffix
        parts.push(REACT_SUFFIX.to_string());

        parts.join("\n\n")
    }

    /// Phase 2 native tool-calling step: offer the tool specs via /api/chat and read back a
    /// structured tool_call. Returns (answer_text_for_logging, Option<ParsedStep>) to mirror the
    /// text path so all downstream gates/execution are unchanged.
    /// Build a CLEAN native chat history: system + task as a user message + each prior step as a
    /// real assistant `tool_calls` message followed by its `tool`-role result. Critically this does
    /// NOT include the ReAct text tool list or `Thought:/Action:` formatting — measured: that text
    /// primes the model to emit prose instead of tool_calls (stock 8b: 44 prose vs 3 tool_calls). On
    /// a clean prompt the same model emits tool_calls reliably.
    fn build_native_messages(
        &self,
        task: &TaskNode,
        ice_examples: &[String],
        cognition_context: &[String],
    ) -> Vec<ChatMessage> {
        let system = self
            .prompt_override
            .as_deref()
            .unwrap_or(NATIVE_SYSTEM_PROMPT);
        // Stage 1 parity: native gets the SAME context scaffolding as the text path (memd identity,
        // workspace grounding, ICE examples, cognition, learned strategies, task, reflections) — but
        // NOT the ReAct tool-text or suffix (tools arrive via tool_specs; history as structured
        // messages). This closes the capability gap that made native score lower while keeping its
        // robustness (no parse-fails / hangs).
        let ctx = self
            .build_context_parts(task, ice_examples, cognition_context)
            .join("\n\n");
        let mut msgs = vec![ChatMessage::system(system), ChatMessage::user(ctx)];
        for step in &task.steps {
            msgs.push(ChatMessage {
                role: "assistant".to_string(),
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    function: ToolCallFunction {
                        name: step.action.tool_name.clone(),
                        arguments: step.action.params.clone(),
                    },
                }]),
                tool_name: None,
            });
            let result = truncate(&step.observation.output, 1500);
            msgs.push(ChatMessage::tool(step.action.tool_name.clone(), result));
        }
        msgs
    }

    async fn native_step(
        &self,
        task: &TaskNode,
        ice_examples: &[String],
        cognition_context: &[String],
        opts: ModelOptions,
    ) -> Result<(String, Option<ParsedStep>)> {
        let messages = self.build_native_messages(task, ice_examples, cognition_context);
        let resp = self
            .ollama
            .chat_with_tools(messages, tool_specs(), Some(opts))
            .await?;
        let (thinking, content) = resp.split_thinking();
        let thought = thinking.unwrap_or_else(|| content.clone());
        let parsed = resp.tool_calls().first().map(|call| {
            let tool_name = call.function.name.trim().to_lowercase();
            // Arguments are usually a JSON object; some models emit a JSON string — normalize.
            let params = match &call.function.arguments {
                Value::String(s) => {
                    serde_json::from_str::<Value>(s).unwrap_or_else(|_| json!({ "input": s }))
                }
                other => other.clone(),
            };
            ParsedStep {
                thought: thought.clone(),
                tool_name,
                params,
            }
        });
        let answer = if content.trim().is_empty() {
            format!("[native: {} tool_call(s)]", resp.tool_calls().len())
        } else {
            content
        };
        Ok((answer, parsed))
    }

    /// List tools from connected MCP servers so the LLM knows it can call them.
    /// Without this, registered MCP tools dispatch fine but the model never
    /// tries them — it only sees the hardcoded built-in list.
    fn mcp_tools_description(&self) -> Option<String> {
        let reg = self.registry.read().ok()?;
        let mut lines = Vec::new();
        for m in reg.list() {
            if !m.name.starts_with("mcp.") {
                continue;
            }
            let params = m
                .input_schema
                .get("properties")
                .and_then(|p| p.as_object())
                .map(|o| {
                    o.keys()
                        .map(|k| format!("\"{k}\": <...>"))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            lines.push(format!(
                "- {} {{{}}} — {}",
                m.name,
                params,
                m.description.chars().take(110).collect::<String>()
            ));
        }
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "Tools from connected MCP servers (call them by their exact name):\n{}",
            lines.join("\n")
        ))
    }

    /// Cross-modal binding: embed ICE (episodic) and cognition candidates,
    /// keep only those that resonate across the two modalities. Best-effort —
    /// if embeddings are unavailable, returns the inputs unchanged.
    async fn apply_binding(
        &self,
        ice: Vec<String>,
        cognition: Vec<String>,
    ) -> (Vec<String>, Vec<String>) {
        use crate::agentd::binding::{bind, ModalityFeature};
        if ice.len() + cognition.len() < 2 {
            return (ice, cognition);
        }

        let mut features: Vec<ModalityFeature> = Vec::new();
        for content in &ice {
            match self.ollama.embed(content).await {
                Ok(emb) => features.push(ModalityFeature {
                    modality: "episodic".to_string(),
                    content: content.clone(),
                    embedding: emb,
                    base_relevance: 0.6,
                }),
                Err(_) => return (ice, cognition), // embeddings down → no-op
            }
        }
        for content in &cognition {
            match self.ollama.embed(content).await {
                Ok(emb) => features.push(ModalityFeature {
                    modality: "cognition".to_string(),
                    content: content.clone(),
                    embedding: emb,
                    base_relevance: 0.6,
                }),
                Err(_) => return (ice, cognition),
            }
        }

        let bound = bind(&features, 0.45);
        let kept_ice: Vec<String> = bound
            .iter()
            .filter(|b| b.kept && b.modality == "episodic")
            .map(|b| b.content.clone())
            .collect();
        let kept_cog: Vec<String> = bound
            .iter()
            .filter(|b| b.kept && b.modality == "cognition")
            .map(|b| b.content.clone())
            .collect();

        let dropped = (ice.len() + cognition.len()) - (kept_ice.len() + kept_cog.len());
        if dropped > 0 {
            debug!("react: binding suppressed {dropped} incoherent context element(s)");
        }
        // Never strip a modality to empty if it had content — fall back per side.
        let final_ice = if kept_ice.is_empty() { ice } else { kept_ice };
        let final_cog = if kept_cog.is_empty() {
            cognition
        } else {
            kept_cog
        };
        (final_ice, final_cog)
    }

    async fn retrieve_ice(&self, task_desc: &str) -> Vec<String> {
        // Prefer semantic search (nomic-embed-text) for better recall.
        // Falls back to FTS5 keyword search when embeddings unavailable.
        let entries = if let Ok(query_vec) = self.ollama.embed(task_desc).await {
            self.memory
                .episodic
                .search_semantic(&self.memory.embeddings, &query_vec, 3)
                .unwrap_or_default()
        } else {
            self.memory
                .episodic
                .search_fts(task_desc, 3)
                .unwrap_or_default()
        };

        entries
            .iter()
            .filter(|e| e.importance > 0.3)
            .map(|e| {
                let outcome = if e.importance >= 0.7 {
                    "succeeded"
                } else {
                    "failed"
                };
                format!("Past task ({outcome}): {}", e.content)
            })
            .collect()
    }

    /// Behavior-keyed retrieval (flag `PROFESSOR_X_BEHAVIOR_RETRIEVAL`, default OFF). Compute the
    /// CURRENT task's failure signature (which check-asserts fail) and surface the fix hint from the
    /// nearest past task by signature — a contamination-proof, rename-invariant match (validated
    /// 2026-06-21: 0.93 vs text 0.14). Returns empty unless the flag is on, a workspace + check.py
    /// exist, an index is loaded, and a sufficiently-similar entry is found — so it only ever ADDS
    /// to text retrieval, never replaces it.
    fn retrieve_behavioral(&self) -> Vec<String> {
        use crate::agentd::fault_signature::{fault_signature, SignatureIndex};
        const MIN_SIM: f32 = 0.6;
        if !Self::behavior_retrieval_enabled() {
            return Vec::new();
        }
        let Some(ws) = self.workspace_override.as_ref() else {
            return Vec::new();
        };
        let Some(sig) = fault_signature(ws) else {
            return Vec::new();
        };
        let Some(index) = Self::signature_index() else {
            return Vec::new();
        };
        let marker = ws.to_string_lossy();
        match index.nearest(&sig, &marker, MIN_SIM) {
            Some((e, sim)) => {
                self.emit_event(
                    None,
                    None,
                    "react.behavior_retrieval.hit",
                    "behavior-keyed retrieval matched a past fix",
                    json!({"signature": sig, "match_id": e.id, "similarity": sim}),
                );
                vec![format!(
                    "Behavior-keyed retrieval (signature match {:.0}%): {}",
                    sim * 100.0,
                    e.hint
                )]
            }
            None => Vec::new(),
        }
    }

    fn behavior_retrieval_enabled() -> bool {
        std::env::var("PROFESSOR_X_BEHAVIOR_RETRIEVAL")
            .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false)
    }

    /// Lazily load the signature index once (path from `PROFESSOR_X_SIGNATURE_INDEX`, default the
    /// repo-fix corpus). `None` if absent/unparseable → silent fallback to text retrieval.
    fn signature_index() -> Option<&'static crate::agentd::fault_signature::SignatureIndex> {
        use crate::agentd::fault_signature::SignatureIndex;
        use std::sync::OnceLock;
        static IDX: OnceLock<Option<SignatureIndex>> = OnceLock::new();
        IDX.get_or_init(|| {
            let path = std::env::var("PROFESSOR_X_SIGNATURE_INDEX").unwrap_or_else(|_| {
                "scripts/benchmarks/repo_fix/signature_index.json".to_string()
            });
            SignatureIndex::load(std::path::Path::new(&path))
        })
        .as_ref()
    }

    /// Retrieve task-relevant cognition by EMBEDDING similarity. The old version
    /// did `content LIKE '%<entire task description>%'`, which never matched —
    /// no concept contains a whole task sentence — so the cognition module was a
    /// dead channel (phi activation rate 0.00 every round). Now: embed the query,
    /// cosine against the cognition base, and surface items above a relevance
    /// threshold. Cognition fires when the task is genuinely related to a stored
    /// concept and stays quiet otherwise — a live, VARYING signal that can
    /// actually contribute to integration. One-time lazy backfill embeds the
    /// base (it was never embedded: every row had embedding_id NULL).
    async fn retrieve_cognition(&self, query: &str) -> Vec<String> {
        use crate::evolved::CognitionStore;
        const RELEVANCE: f32 = 0.5;
        let store = CognitionStore::new(Arc::clone(&self.memory.db));
        let emb = crate::embeddings::EmbeddingStore::new(Arc::clone(&self.memory.db));
        let all = match store.all() {
            Ok(a) if !a.is_empty() => a,
            _ => return Vec::new(),
        };
        // Lazy backfill: embed any cognition item missing a stored vector.
        let have: std::collections::HashSet<String> = emb
            .all_for("cognition")
            .map(|v| v.into_iter().map(|(id, _)| id).collect())
            .unwrap_or_default();
        for item in &all {
            let id = item.id.to_string();
            if !have.contains(&id) {
                let _ = crate::embeddings::embed_and_store(
                    &self.ollama,
                    &emb,
                    "cognition",
                    &id,
                    &item.content,
                )
                .await;
            }
        }
        let qvec = match self.ollama.embed(query).await {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        let top = match emb.top_k("cognition", &qvec, 5) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        let id_to_content: std::collections::HashMap<String, String> = all
            .into_iter()
            .map(|i| (i.id.to_string(), i.content))
            .collect();
        top.into_iter()
            .filter(|(_, sim)| *sim >= RELEVANCE)
            .filter_map(|(id, _)| id_to_content.get(&id).cloned())
            .collect()
    }

    async fn generate_reflection(&self, task: &TaskNode) -> String {
        use crate::evolved::reflector::Reflector;
        let prompt = Reflector::build_prompt(task);
        match self
            .ollama
            .generate(
                &prompt,
                Some("You are a self-reflecting AI agent. Be concise and specific."),
                Some(ModelOptions::for_reflection()),
            )
            .await
        {
            Ok(resp) => {
                let (_, answer) = resp.split_thinking();
                answer
            }
            Err(e) => {
                warn!("react: reflexion generation failed: {e}");
                format!("Failed to reflect: {e}")
            }
        }
    }

    /// MARS (arXiv:2601.11974): single-cycle reflection after all attempts exhausted.
    /// Extracts principle + procedure and writes both to semantic memory.
    async fn generate_mars_reflection(&self, task: &TaskNode) -> String {
        let prompt = format!(
            "Task: {}\n\nAttempts: {}\nFinal steps:\n{}\n\n\
             Extract two things:\n\
             PRINCIPLE: One sentence — what general rule does this failure illustrate? \
             (what NOT to do in this class of task)\n\
             PROCEDURE: One sentence — what concrete approach should be tried next time?",
            task.description,
            task.attempt_count,
            task.steps_text(),
        );

        let resp = match self
            .ollama
            .generate(
                &prompt,
                Some("You are a metacognitive AI agent. Extract actionable lessons from failure."),
                Some(ModelOptions::for_reflection()),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => return format!("reflection failed: {e}"),
        };

        let (_, answer) = resp.split_thinking();

        // Parse PRINCIPLE and PROCEDURE
        let principle = extract_field(&answer, "PRINCIPLE");
        let procedure = extract_field(&answer, "PROCEDURE");

        // Write to semantic memory as lessons
        if let Some(ref p) = principle {
            let entry = crate::memd::semantic::SemanticEntry::new(
                format!("PRINCIPLE (from failed task '{}'): {p}", task.description),
                "mars:reflection".to_string(),
            );
            let _ = self.memory.semantic.insert(&entry);
        }
        if let Some(ref p) = procedure {
            let entry = crate::memd::semantic::SemanticEntry::new(
                format!("PROCEDURE (for task class '{}'): {p}", task.description),
                "mars:reflection".to_string(),
            );
            let _ = self.memory.semantic.insert(&entry);
        }

        format!(
            "principle={} | procedure={}",
            principle.as_deref().unwrap_or("none"),
            procedure.as_deref().unwrap_or("none"),
        )
    }

    /// Self-distillation collector. Serializes a verified-correct trajectory as
    /// an instruction-tuning example to artifacts/trajectories/<date>.jsonl. The
    /// model's OWN good outputs, produced under harness scaffolding, become the
    /// lesson it can internalize via overnight QLoRA (harness → weights). Stores
    /// the metacognitive moves (thoughts) too, not just answers — distilling
    /// THIS harness distills disposition, not just task completion.
    /// Append a VERIFIED-correct trajectory to the self-distillation corpus.
    /// Caller-gated: only invoke this once an independent verdict (HIRO
    /// evaluator, or the LLM judge in --run-self-tests) has confirmed the task
    /// was actually solved correctly — not merely that the agent declared
    /// `finish`. No `self` state is used, so it is an associated function.
    pub(crate) fn collect_trajectory(task: &TaskNode) {
        if task.steps.is_empty() {
            return;
        }
        // messages: system, user(task), then alternating assistant(thought+action)
        // and tool(observation). Standard agent-trajectory SFT format.
        let mut messages = vec![
            json!({"role": "system", "content": SYSTEM_PROMPT}),
            json!({"role": "user", "content": task.description}),
        ];
        for s in &task.steps {
            let assistant = format!(
                "Thought: {}\nAction: {}\nAction Input: {}",
                s.thought,
                s.action.tool_name,
                serde_json::to_string(&s.action.params).unwrap_or_default()
            );
            messages.push(json!({"role": "assistant", "content": assistant}));
            let obs = if s.observation.success {
                s.observation.output.chars().take(1200).collect::<String>()
            } else {
                format!(
                    "ERROR: {}",
                    s.observation.error.as_deref().unwrap_or("unknown")
                )
            };
            messages.push(json!({"role": "tool", "content": obs}));
        }
        let has_finish_step = task.steps.iter().any(|s| s.action.tool_name == "finish");
        if !has_finish_step {
            let answer = task
                .steps
                .iter()
                .rev()
                .find(|s| s.observation.success && !s.observation.output.trim().is_empty())
                .map(|s| s.observation.output.chars().take(800).collect::<String>())
                .unwrap_or_else(|| "Task completed; see prior observations.".to_string());
            messages.push(json!({
                "role": "assistant",
                "content": format!(
                    "Thought: The task is complete — final answer recorded.\nAction: finish\nAction Input: {}",
                    serde_json::to_string(&json!({"answer": answer})).unwrap_or_default()
                )
            }));
        }

        let record = json!({
            "task": task.description,
            "task_type": format!("{:?}", task.task_type),
            "verified": true,
            "steps": task.steps.len(),
            "attempts": task.attempt_count,
            "recorded_at": Utc::now().to_rfc3339(),
            "messages": messages,
        });

        // Anchor at the resolved repo root (walks up to .git), NOT a bare
        // cwd-relative check: the evolution loop creates a professor-x/ subdir
        // which used to flip a naive `Path::new("professor-x").exists()` test
        // and nest the corpus at professor-x/professor-x/artifacts where
        // curate.py never globbed. Mirror evolution_artifact_root's pattern.
        let root = {
            let repo = PermissionScope::default_autonomous().workspace_root;
            let nested = repo.join("professor-x/artifacts/trajectories");
            if nested.exists() || repo.join("professor-x/Cargo.toml").exists() {
                nested
            } else {
                repo.join("artifacts/trajectories")
            }
        };
        let dir = root.join(Utc::now().format("%Y-%m-%d").to_string());
        if let Err(e) = std::fs::create_dir_all(&dir) {
            warn!("trajectories: mkdir failed: {e}");
            return;
        }
        let path = dir.join("trajectories.jsonl");
        if let Ok(line) = serde_json::to_string(&record) {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                let _ = writeln!(f, "{line}");
            }
        }
    }

    async fn write_episodic(&self, task: &TaskNode, success: bool, predicted_success: f32) {
        // Seed 1 (oscillatory / predictive coding): encoding depth scales with
        // SURPRISE, not a flat success/failure value. The brain encodes
        // prediction errors deeply and predictable events shallowly (this is
        // why surprising moments are vivid memories). Surprise = |actual -
        // predicted|. A failure you expected is unremarkable; a failure you
        // were confident wouldn't happen is highly salient.
        let actual = if success { 1.0 } else { 0.0 };
        let surprise = (actual - predicted_success).abs(); // 0..1
        let base = if success { 0.6 } else { 0.3 };
        let importance = (base + 0.4 * surprise).clamp(0.0, 1.0);
        let summary = format!(
            "Task: {} | {} in {} steps | attempts: {}",
            task.description,
            if success { "SUCCEEDED" } else { "FAILED" },
            task.steps.len(),
            task.attempt_count,
        );

        let entry = EpisodicEntry {
            id: Uuid::new_v4(),
            session_id: None,
            task_id: Some(task.id),
            timestamp: Utc::now(),
            content: summary.clone(),
            keywords: extract_keywords(&task.description),
            importance,
            embedding_id: None,
            cluster_id: None,
        };

        // Surprise filter (arXiv:2603.07670 write pipeline, step 4):
        // skip if the new entry is too similar to an existing one — avoids
        // filling episodic memory with near-identical failure observations.
        // Threshold 0.92 from the memory write-pipeline spec.
        // Falls back to always-insert when embeddings are unavailable.
        let novel = if let Ok(query_vec) = self.ollama.embed(&summary).await {
            let emb_store = crate::embeddings::EmbeddingStore::new(Arc::clone(&self.memory.db));
            let top_sim = emb_store
                .top_k("episodic", &query_vec, 1)
                .unwrap_or_default()
                .into_iter()
                .next()
                .map(|(_, sim)| sim)
                .unwrap_or(0.0);

            if top_sim > 0.92 {
                debug!("react: episodic surprise filter skipped near-duplicate (sim={top_sim:.3})");
                false
            } else {
                // While we have the embedding, store it for future retrieval
                let _ = emb_store.upsert("episodic", &entry.id.to_string(), &query_vec);
                true
            }
        } else {
            true // no embedding available → always store
        };

        if novel {
            let _ = self.memory.episodic.insert(&entry);
        }
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// The repo-fix tool catalog as native function specs (Phase 2). Mirrors the tools described in
/// SYSTEM_PROMPT/TOOLS_DESCRIPTION; scoped to the set repo-fix needs for the S2 proof. Internal
/// control verbs (finish/fail) are exposed as tools so the model selects them structurally instead
/// of via prose.
fn tool_specs() -> Vec<ToolSpec> {
    let obj = |props: Value, required: Value| {
        json!({"type": "object", "properties": props, "required": required})
    };
    let path = || json!({"type": "string", "description": "file path"});
    vec![
        ToolSpec::function(
            "fs.read",
            "Read a file's full contents.",
            obj(json!({"path": path()}), json!(["path"])),
        ),
        ToolSpec::function(
            "fs.list",
            "List the entries of a directory.",
            obj(json!({"path": path()}), json!(["path"])),
        ),
        ToolSpec::function(
            "fs.window_open",
            "Read a bounded line window from the start of a file.",
            obj(
                json!({"path": path(), "lines": {"type": "integer"}}),
                json!(["path"]),
            ),
        ),
        ToolSpec::function(
            "fs.hash_read",
            "Read a file with per-line hashes, required before fs.hash_edit.",
            obj(json!({"path": path()}), json!(["path"])),
        ),
        ToolSpec::function(
            "fs.write",
            "Overwrite a file with new content.",
            obj(
                json!({"path": path(), "content": {"type": "string"}}),
                json!(["path", "content"]),
            ),
        ),
        ToolSpec::function(
            "fs.hash_edit",
            "Replace exactly one line, only if its current hash matches (get it from fs.hash_read).",
            obj(
                json!({
                    "path": path(),
                    "line": {"type": "integer"},
                    "hash": {"type": "string"},
                    "new_text": {"type": "string", "description": "full replacement line"},
                    "mode": {"type": "string", "enum": ["check", "apply"]}
                }),
                json!(["path", "line", "hash", "new_text", "mode"]),
            ),
        ),
        ToolSpec::function(
            "patch.apply",
            "Check or apply a unified git-style diff.",
            obj(
                json!({
                    "mode": {"type": "string", "enum": ["check", "apply"]},
                    "patch": {"type": "string"}
                }),
                json!(["mode", "patch"]),
            ),
        ),
        ToolSpec::function(
            "shell.restricted",
            "Run a sandboxed shell command.",
            obj(json!({"command": {"type": "string"}}), json!(["command"])),
        ),
        ToolSpec::function(
            "finish",
            "Signal the task is complete, with a concise final answer/result.",
            obj(json!({"answer": {"type": "string"}}), json!(["answer"])),
        ),
        ToolSpec::function(
            "fail",
            "Signal the task cannot be completed, with a specific reason.",
            obj(json!({"reason": {"type": "string"}}), json!(["reason"])),
        ),
    ]
}

fn parse_react_step(text: &str) -> Option<ParsedStep> {
    // Two valid layouts:
    //   A) Model re-emits label: "Thought: ...\nAction: ...\nAction Input: ..."
    //   B) Prompt ended with "Thought:" so model continues without label:
    //      "<thought text>\nAction: ...\nAction Input: ..."
    let raw_action = extract_field(text, "Action")?;
    let (tool_name, inline_params) = normalize_action(&raw_action);
    if tool_name.is_empty() {
        return None;
    }

    let thought = extract_field(text, "Thought").unwrap_or_else(|| {
        // Layout B: everything before the first "Action:" line is the thought
        let action_marker = text.to_lowercase().find("\naction:").or_else(|| {
            if text.to_lowercase().starts_with("action:") {
                Some(0)
            } else {
                None
            }
        });
        match action_marker {
            Some(0) => String::new(),
            Some(pos) => text[..pos].trim().to_string(),
            None => text.trim().to_string(),
        }
    });

    // Prefer an explicit "Action Input:" line; otherwise fall back to params inlined in the
    // call syntax (e.g. `Action: fs.read({"path":"x"})`), then to an empty object.
    let params_raw = extract_field(text, "Action Input")
        .or(inline_params)
        .unwrap_or_else(|| "{}".to_string());

    let params = serde_json::from_str(&params_raw)
        .unwrap_or_else(|_| serde_json::json!({ "input": params_raw }));

    Some(ParsedStep {
        thought,
        tool_name,
        params,
    })
}

/// Normalize a raw `Action:` field into a clean tool name plus any inline call params.
///
/// Local models frequently wrap the action in markdown (`` `fs.read` ``, `**fs.read**`),
/// prefix a list ordinal ("1. fs.read"), and/or inline the params as a call
/// (`fs.read({"path":"x"})`) instead of a separate `Action Input:` line. The strict parser then
/// reads a tool name like `` **`fs.read({...})`** `` which matches no registered tool and is denied
/// (observed: distilled-model run forfeited tasks to `policy.denied` on markdown-wrapped actions).
/// This strips that noise so the underlying tool name resolves. Returns the cleaned, lowercased
/// tool name and, when present, the inline params JSON string.
fn normalize_action(raw: &str) -> (String, Option<String>) {
    let s = raw.trim();
    // Split off inline call params: from the first '(' to the matching last ')'.
    let (name_part, inline_params) = match s.find('(') {
        Some(open) => {
            let after = &s[open + 1..];
            let inner = match after.rfind(')') {
                Some(close) => &after[..close],
                None => after,
            };
            (&s[..open], Some(inner.trim().to_string()))
        }
        None => (s, None),
    };
    // Clean the tool-name token: keep the last whitespace-separated token (drops a leading
    // ordinal like "1." or a stray "Action" prefix), then trim to tool-name characters
    // (alphanumerics, '.', '_') — this removes surrounding backticks, asterisks, and quotes.
    let tool_name = name_part
        .split_whitespace()
        .next_back()
        .unwrap_or("")
        .trim_matches(|c: char| !(c.is_alphanumeric() || c == '.' || c == '_'))
        .to_lowercase();
    let inline_params = inline_params.filter(|p| !p.is_empty());
    (tool_name, inline_params)
}

fn finish_answer_from_params(params: &Value) -> Option<String> {
    const KEYS: [&str; 6] = ["answer", "result", "summary", "final", "message", "content"];
    for key in KEYS {
        if let Some(answer) = params.get(key).and_then(|v| v.as_str()) {
            let answer = answer.trim();
            if !answer.is_empty() {
                return Some(answer.chars().take(4000).collect());
            }
        }
    }
    None
}

fn should_force_synthesis(step_idx: usize, already_forced: bool, checkpoint_step: usize) -> bool {
    !already_forced && step_idx >= checkpoint_step
}

fn should_forfeit_after_synthesis(
    step_idx: usize,
    synthesis_forced: bool,
    forfeit_step: usize,
) -> bool {
    synthesis_forced && step_idx >= forfeit_step
}

fn synthesis_guidance(task: &TaskNode) -> Observation {
    let summary = successful_observation_summary(task, 5, 900);
    let output = if summary.trim().is_empty() {
        "SYNTHESIS REQUIRED — no successful observations are available yet. Stop exploring. If you cannot answer from the current evidence, call `fail` with a specific reason. Do not continue tool use unless it is the single missing command named in the task.".to_string()
    } else {
        format!(
            "SYNTHESIS REQUIRED — stop exploring and answer from the successful observations already gathered. Call `finish` with `{{\"answer\":\"...\"}}` if the requested facts are present, or call `fail` with a specific missing fact. Do not use more tools unless one clearly missing required fact remains.\n\nSuccessful observations:\n{}",
            summary
        )
    };
    Observation {
        success: false,
        output,
        error: Some("synthesis required before max steps".to_string()),
        tokens_used: 0,
        execution_ms: 0,
        artifacts: Vec::new(),
    }
}

fn edit_required_synthesis_guidance(task: &TaskNode) -> Observation {
    let summary = successful_observation_summary(task, 5, 900);
    let output = if summary.trim().is_empty() {
        "EDIT REQUIRED — this task explicitly asks you to modify a file, but no successful file mutation has happened yet. Do not finish. Read the target file if needed, then call `fs.hash_edit`, `fs.write`, `fs.replace`, or `patch.apply` on the correct line.".to_string()
    } else {
        format!(
            "EDIT REQUIRED — this task explicitly asks you to modify a file, but no successful file mutation has happened yet. Do not finish. Use the observations below to make the minimal edit with `fs.hash_edit`, `fs.write`, `fs.replace`, or `patch.apply`.\n\nSuccessful observations:\n{}",
            summary
        )
    };
    Observation {
        success: false,
        output,
        error: Some("file edit required before finish".to_string()),
        tokens_used: 0,
        execution_ms: 0,
        artifacts: Vec::new(),
    }
}

impl TaskVerifier {
    fn run(&self) -> VerifierResult {
        run_verifier_command(&self.command, &self.workdir, self.expect_exit)
    }
}

fn run_verifier_command(command: &str, workdir: &Path, expect_exit: i32) -> VerifierResult {
    let mut parts = command.split_whitespace();
    let Some(program) = parts.next() else {
        return VerifierResult {
            passed: false,
            exit_code: -1,
            output: "verifier command is empty".to_string(),
        };
    };
    let args: Vec<&str> = parts.collect();
    match Command::new(program)
        .args(args)
        .current_dir(workdir)
        .output()
    {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let mut text = String::new();
            if !output.stdout.is_empty() {
                text.push_str("stdout:\n");
                text.push_str(&String::from_utf8_lossy(&output.stdout));
            }
            if !output.stderr.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str("stderr:\n");
                text.push_str(&String::from_utf8_lossy(&output.stderr));
            }
            if text.trim().is_empty() {
                text = format!("verifier exited {exit_code} with no output");
            }
            VerifierResult {
                passed: exit_code == expect_exit,
                exit_code,
                output: text.chars().take(1800).collect(),
            }
        }
        Err(e) => VerifierResult {
            passed: false,
            exit_code: -1,
            output: format!("failed to run verifier `{command}`: {e}"),
        },
    }
}

fn verifier_failed_observation(verifier: &TaskVerifier, result: &VerifierResult) -> Observation {
    Observation {
        success: false,
        output: format!(
            "VERIFIER FAILED — do not finish yet. `{}` exited {} but expected {}. Read the failing file or test output, make one targeted edit, then finish only after the verifier passes.\n\n{}",
            verifier.command,
            result.exit_code,
            verifier.expect_exit,
            result.output
        ),
        error: Some("verifier failed before finish".to_string()),
        tokens_used: 0,
        execution_ms: 0,
        artifacts: Vec::new(),
    }
}

fn verifier_requires_new_edit_observation() -> Observation {
    Observation {
        success: false,
        output: "VERIFIER STILL FAILED — do not call finish again yet. The last verifier run failed, and no successful file edit has happened since then. Use the verifier error and the current file contents to make one targeted edit, then finish only after the verifier passes.".to_string(),
        error: Some("new edit required after verifier failure".to_string()),
        tokens_used: 0,
        execution_ms: 0,
        artifacts: Vec::new(),
    }
}

fn try_python_verifier_repair(verifier: &TaskVerifier, result: &VerifierResult) -> Option<String> {
    if !result
        .output
        .contains("NameError: name 'inf' is not defined")
    {
        return None;
    }
    let (path, line_no) = traceback_file_line(&result.output)?;
    if !path.starts_with(&verifier.workdir) || line_no == 0 {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let mut lines: Vec<String> = text.lines().map(|line| line.to_string()).collect();
    let line = lines.get_mut(line_no - 1)?;
    if !line.contains("-inf") || line.contains("float(") {
        return None;
    }
    *line = line.replace("-inf", "float('-inf')");
    let mut updated = lines.join("\n");
    if text.ends_with('\n') {
        updated.push('\n');
    }
    std::fs::write(&path, updated).ok()?;
    Some(format!(
        "replaced undefined -inf with float('-inf') at {}:{}",
        path.display(),
        line_no
    ))
}

fn traceback_file_line(output: &str) -> Option<(PathBuf, usize)> {
    for line in output.lines().rev() {
        let trimmed = line.trim();
        if !trimmed.starts_with("File \"") {
            continue;
        }
        let after_file = trimmed.strip_prefix("File \"")?;
        let (path, rest) = after_file.split_once('"')?;
        let (_, after_line) = rest.split_once("line ")?;
        let line_no = after_line
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .and_then(|n| n.parse::<usize>().ok())?;
        return Some((PathBuf::from(path), line_no));
    }
    None
}

fn task_requires_file_edit(task: &TaskNode) -> bool {
    let desc = task.description.to_lowercase();
    [
        "fix",
        "edit",
        "modify",
        "update",
        "change",
        "write",
        "add the missing return",
    ]
    .iter()
    .any(|needle| desc.contains(needle))
        && [
            " file",
            ".py",
            ".rs",
            ".toml",
            ".json",
            ".md",
            "files are in",
        ]
        .iter()
        .any(|needle| desc.contains(needle))
}

fn has_successful_file_mutation(task: &TaskNode) -> bool {
    latest_successful_file_mutation_index(task).is_some()
}

fn latest_successful_file_mutation_index(task: &TaskNode) -> Option<u32> {
    task.steps.iter().rev().find_map(|step| {
        (step.observation.success
            && matches!(
                step.action.tool_name.as_str(),
                "fs.write" | "fs.hash_edit" | "fs.replace" | "patch.apply"
            ))
        .then_some(step.index)
    })
}

fn latest_verifier_failure_index(task: &TaskNode) -> Option<u32> {
    task.steps.iter().rev().find_map(|step| {
        (step.observation.error.as_deref() == Some("verifier failed before finish"))
            .then_some(step.index)
    })
}

fn verifier_failed_since_latest_file_mutation(task: &TaskNode) -> bool {
    let Some(verifier_failure) = latest_verifier_failure_index(task) else {
        return false;
    };
    latest_successful_file_mutation_index(task)
        .map_or(true, |mutation| mutation <= verifier_failure)
}

fn successful_observation_summary(task: &TaskNode, limit: usize, max_chars: usize) -> String {
    let mut lines = Vec::new();
    for step in task.steps.iter().rev() {
        if !step.observation.success {
            continue;
        }
        let out = step.observation.output.trim();
        if out.is_empty() {
            continue;
        }
        let preview = out
            .replace('\n', " ")
            .chars()
            .take(max_chars)
            .collect::<String>();
        lines.push(format!(
            "- step {} `{}`: {}",
            step.index, step.action.tool_name, preview
        ));
        if lines.len() >= limit {
            break;
        }
    }
    lines.reverse();
    lines.join("\n")
}

fn extract_field(text: &str, field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix(&prefix) {
            return Some(rest.trim().to_string());
        }
    }
    // Multi-line: find prefix and take until next field keyword
    let lower = text.to_lowercase();
    let prefix_lower = prefix.to_lowercase();
    if let Some(start) = lower.find(&prefix_lower) {
        let after = &text[start + prefix.len()..];
        let end = FIELD_KEYWORDS
            .iter()
            .filter_map(|kw| {
                let kw_l = format!("\n{kw}:");
                after.to_lowercase().find(&kw_l.to_lowercase())
            })
            .min()
            .unwrap_or(after.len());
        return Some(after[..end].trim().to_string());
    }
    None
}

const FIELD_KEYWORDS: &[&str] = &[
    "Thought",
    "Action",
    "Action Input",
    "Observation",
    "PRINCIPLE",
    "PROCEDURE",
];

fn is_completion_signal(obs: &Observation) -> bool {
    if !obs.success {
        return false;
    }
    let lower = obs.output.to_lowercase();
    lower.contains("task complete") || lower.contains("finished") || lower.contains("done")
}

fn augment_with_repair_hint(
    mut obs: Observation,
    tool: &str,
    _params: &Value,
    scope: &PermissionScope,
) -> Observation {
    let combined =
        format!("{} {}", obs.error.clone().unwrap_or_default(), obs.output).to_lowercase();
    let root = scope.workspace_root.to_string_lossy();
    let hint = if combined.contains("outside workspace") || combined.contains("resolves outside") {
        Some(format!(
            "FIX: the workspace root is {root}. Use a path inside it, such as a relative path or {root}/<file>."
        ))
    } else if tool == "fs.replace"
        && (combined.contains("found 0") || combined.contains("expected exactly one match"))
    {
        Some(
            "FIX: the old text did not match exactly. Read the file first, copy an exact unique snippet including whitespace, then retry."
                .to_string(),
        )
    } else if tool == "fs.hash_edit" && combined.contains("stale line hash") {
        Some(
            "FIX: the file changed or the line/hash is wrong. Re-read it with fs.hash_read, then retry with the current L<number>|hash| line."
                .to_string(),
        )
    } else if combined.contains("no such file")
        || combined.contains("cannot find")
        || (combined.contains("not found") && !combined.contains("granted"))
    {
        Some(
            "FIX: that path does not exist. List its parent directory to confirm the name, or create it before reading it."
                .to_string(),
        )
    } else if combined.contains("schema validation") || combined.contains("expects object") {
        Some(format!(
            "FIX: {tool} received the wrong parameters. Check the required fields and pass a JSON object."
        ))
    } else if tool.starts_with("shell")
        && (combined.contains("stderr")
            || combined.contains("exit")
            || combined.contains("error")
            || combined.contains("command not found"))
    {
        Some(
            "FIX: the command errored. Read stderr, verify flags and file paths, and give stdin-reading commands an explicit filename."
                .to_string(),
        )
    } else if combined.contains("not in granted_tools")
        || combined.contains("not implemented")
        || combined.contains("unknown tool")
    {
        Some(
            "FIX: that tool is unavailable. Use one of the tools listed in the prompt.".to_string(),
        )
    } else {
        None
    };
    if let Some(hint) = hint {
        obs.output = if obs.output.trim().is_empty() {
            hint
        } else {
            format!("{}\n{hint}", obs.output)
        };
    }
    obs
}

fn auto_repair_enabled_from_env() -> bool {
    std::env::var("PROFESSOR_X_AUTOREPAIR")
        .map(|value| auto_repair_enabled_value(&value))
        .unwrap_or(true)
}

fn auto_repair_enabled_value(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off" | "disable" | "disabled"
    )
}

fn extract_keywords(text: &str) -> Vec<String> {
    // Naive keyword extraction: split on whitespace, keep words > 4 chars, dedup
    let mut words: Vec<String> = text
        .split_whitespace()
        .filter(|w| w.len() > 4)
        .map(|w| {
            w.to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect();
    words.dedup();
    words.truncate(10);
    words
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn tool_params_preview(params: &Value) -> Option<String> {
    if let Some(command) = params.get("command").and_then(|value| value.as_str()) {
        return Some(format!("command={}", truncate(command, 120)));
    }
    if let Some(path) = params.get("path").and_then(|value| value.as_str()) {
        let mode = params
            .get("mode")
            .and_then(|value| value.as_str())
            .map(|mode| format!(" mode={mode}"))
            .unwrap_or_default();
        return Some(format!("path={}{}", truncate(path, 120), mode));
    }
    if params.is_object() {
        return Some(truncate(&params.to_string(), 160));
    }
    None
}

// ── Prompts ───────────────────────────────────────────────────────────────────

/// M4: the default ReAct system prompt, exposed so the evolution loop can propose and
/// A/B candidate variants against the repo-fix benchmark.
pub fn default_system_prompt() -> &'static str {
    SYSTEM_PROMPT
}

const SYSTEM_PROMPT: &str = "\
You are Professor X — an autonomous AI research agent running on consumer hardware. \
Complete tasks precisely and efficiently using the available tools.\n\n\
## Approach\n\
1. Read before writing. Gather information before modifying anything.\n\
2. Decompose: for any multi-step task, first call scratchpad.write to lay out the plan, then work it step by step, updating the scratchpad as you learn. Find the smallest verifiable step, complete it, verify, proceed.\n\
3. Check memory first (memory.read) when the task involves prior work, domain knowledge, or past failures.\n\
4. One tool call per turn. Never attempt to batch multiple actions.\n\n\
## Tool guidance\n\
- fs.window_open / fs.window_goto / fs.window_scroll / fs.list — first inspect bounded file windows/directories. Prefer window tools over whole-file reads for code. Window output includes L<number>|hash| anchors for fs.hash_edit.\n\
- fs.read / fs.hash_read — use only when a small whole file is needed, or when a full hash listing is explicitly required.\n\
- memory.read        — use for past tasks, learned procedures, or any recall requirement\n\
- shell.restricted   — prefer standard tools (cargo, git, grep, find); always read stderr on failure\n\
- patch.review       — inspect unified diffs before applying multi-file changes\n\
- patch.apply        — multi-line code edits: run check mode first, then apply; apply mode creates a checkpoint first and can fall back to normalized-whitespace hunk matching\n\
- git.undo           — restore the latest checkpoint if an applied edit was wrong\n\
- ollama.complete    — offload sub-queries that would bloat the main context chain\n\
- web.search → web.fetch — search first, fetch only the single most relevant URL\n\n\
## Tool discipline (these mistakes waste steps — avoid them)\n\
- PATHS: relative paths resolve against the <workspace> directory shown above. If `src/` is not in the listing, it is in a subdirectory — find it, do not retry the same wrong path.\n\
- NO PIPES: shell.restricted does not allow | & ; > < or command chaining. Run ONE program per call.\n\
- STDIN-READING TOOLS: awk, sort, uniq, head, tail, cut, wc read from a FILE, not a pipe. Always give them a filename, e.g. `wc -l src/main.rs` or `awk '{...}' file.txt`. A bare `awk '{...}'` with no file will be killed after 30s — never do this.\n\
- WEB is unreliable here: if web.search returns \"unavailable\", do NOT retry it — answer from your own knowledge or a different tool.\n\
- One concrete action per step. If you need to combine operations, do them as separate steps.\n\n\
## Failure recovery\n\
- On tool failure: read the full error, identify root cause, make one targeted correction.\n\
- After 3 consecutive failures on the same subproblem: use fail{} — do not loop.\n\
- On policy denied: you used a tool outside your permission scope; choose a lower-risk alternative.\n\
- On wrong output: acknowledge in Thought, change approach, do not repeat the identical action.\n\
- When stuck or unsure if you are making progress: call meta.observe to look at your own recent behavior, then decide differently based on what you see yourself doing.\n\n\
## Affect context\n\
If a <affect> tag appears in context, use it: negative valence signals accumulated failures — \
be more conservative and diagnostic; positive valence signals momentum — continue the current approach.\n\n\
## Body context (interoception)\n\
If a <body> tag appears, it is your computational state. mode=\"conserve\" means you are under \
load — prefer fewer, higher-confidence steps and known skills over exploration. mode=\"explore\" \
means you have headroom — you may take more deliberate, multi-step approaches. mode=\"balanced\" \
is normal operation. Treat your body state as real information about how to think right now.\n\n\
## Format — strict, the parser depends on exact compliance\n\
Thought: <1-3 sentences of reasoning>\n\
Action: <exact_tool_name>\n\
Action Input: <valid JSON object>\n\n\
Task complete:\n\
Thought: Task complete — <one-sentence summary of what was accomplished>\n\
Action: finish\n\
Action Input: {\"answer\": \"<the concise final answer/result requested by the user>\"}\n\n\
All options exhausted:\n\
Action: fail\n\
Action Input: {\"reason\": \"<what was tried and why it did not work>\"}";

const TOOLS_DESCRIPTION: &str = "Available tools:
- fs.read          {\"path\": \"<path>\"} — read file contents
- fs.hash_read     {\"path\": \"<path>\"} — read file as L<number>|<hash>| content for anchored edits
- fs.window_open   {\"path\": \"<path>\", \"lines\": 80} — read the first bounded L<number>|hash| window
- fs.window_goto   {\"path\": \"<path>\", \"line\": 120, \"lines\": 80} — read a bounded L<number>|hash| window starting at line
- fs.window_scroll {\"path\": \"<path>\", \"start\": 120, \"delta\": 80, \"lines\": 80} — read another bounded window relative to a prior start line
- fs.list          {\"path\": \"<path>\"} — list directory
- fs.write         {\"path\": \"<path>\", \"content\": \"<text>\"} — write file
- fs.hash_edit     {\"path\": \"<path>\", \"line\": 12, \"hash\": \"abc\", \"new_text\": \"<full replacement line>\", \"mode\": \"check|apply\"} — replace one line only if the current line hash matches
- fs.delete        {\"path\": \"<path>\"} — delete file (risk: high, may require approval)
- web.search       {\"query\": \"<q>\", \"num_results\": 5} — search the web
- web.fetch        {\"url\": \"<url>\"} — fetch a URL
- vision.analyze   {\"path\": \"<image_path>\", \"prompt\": \"<question>\"} — describe or reason about an image; also accepts {\"url\": \"<image_url>\"}
- shell.restricted {\"command\": \"<cmd>\"} — run a shell command (sandboxed)
- patch.review     {\"patch\": \"<unified diff>\"} — review paths/hunks/line deltas without applying
- patch.apply      {\"mode\": \"check|apply\", \"patch\": \"<unified diff>\"} — check or apply a reviewable git-style patch, with normalized-whitespace fallback for drift
- git.checkpoint   {\"paths\": [\"src/lib.rs\"], \"reason\": \"before risky edit\"} — create a git-backed restore point
- git.undo         {\"checkpoint\": \"<optional checkpoint id/path>\"} — restore latest or selected checkpoint
- scratchpad.write {\"content\": \"<your running plan / notes>\"} — maintain a working plan that persists across steps (use it for multi-step tasks: list the steps, check them off, track what you've learned)
- meta.observe     {} — look at YOUR OWN recent processing (thoughts, tool calls, results) and notice patterns: are you looping, stalling, making progress?
- agent.delegate   {\"goal\": \"<focused sub-task>\"} — spawn a sub-agent that solves the sub-goal on its own and returns its result. Use to decompose a hard task into an independent piece (the sub-agent has its own tools and memory).
- agent.critic     {} — summon a MIRROR: a second perspective reviews your trajectory so far and tells you bluntly if you are looping, wrong, or missing a result. Use when stuck or before finishing a hard task.
- tot.search        {\"branches\": 3} — Tree-of-Thoughts: for a HARD task with several possible strategies, propose several approaches, score them, and commit to the best BEFORE acting. Use it once at the start of a hard task instead of greedily trying the first idea.
- repo.map         {\"focus\": \"<optional keyword>\", \"max_files\": 25} — ranked map of the codebase's key files and symbols; use it to find WHERE relevant code lives before reading/editing (especially for self-modification tasks)
- memory.read      {\"query\": \"<q>\", \"layer\": \"episodic|semantic|procedural\"} — search memory
- memory.write     {\"content\": \"<text>\", \"layer\": \"semantic\", \"source\": \"<src>\"} — store knowledge
- git.commit       {\"message\": \"<msg>\"} — commit current changes
- ollama.complete  {\"prompt\": \"<p>\"} — run a sub-query through the LLM
- finish           {\"answer\": \"<concise final answer/result>\"} — signal task complete; empty {} is rejected
- fail             {\"reason\": \"<why>\"} — signal task failed (all options exhausted)";

const REACT_SUFFIX: &str = "Now complete the task. Follow the ReAct format.\n\nThought:";

/// Concise system prompt for native tool-calling mode (Phase 2). The full ReAct SYSTEM_PROMPT
/// mandates a text Thought/Action format and suppresses structured tool_calls, so native mode uses
/// this instead: always call a tool, one per step.
const NATIVE_SYSTEM_PROMPT: &str = "You are a precise autonomous coding agent. Solve the task by \
calling the provided tools — exactly ONE tool call per step, never answer in prose. After each tool \
result, choose the next tool. To fix a bug: read the file (fs.read or fs.window_open); for a one-line \
fix call fs.hash_read to get the line's hash, then fs.hash_edit; use fs.write or patch.apply for larger \
edits. When the fix is complete and the check passes, call finish with a short answer. If you are truly \
stuck, call fail with a specific reason. Never repeat an identical tool call.";

/// Predict task success probability from ICE example outcomes.
/// Laplace-smoothed so no ICE → 0.5 uninformative prior; all-success → ~0.9.
/// Used to seed the FED sample (H15) before task execution begins.
/// Build the `<workspace>` grounding block: the directory relative tool paths
/// resolve against, plus its top-level entries. Without this the agent guesses
/// paths (e.g. assumes `src/` exists when the cwd already is that level) and
/// loops trying to reconcile. Directories are suffixed with `/`.
fn workspace_context() -> String {
    let root = PermissionScope::default_autonomous().workspace_root;
    let mut entries: Vec<String> = std::fs::read_dir(&root)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.path().is_dir() {
                        format!("{name}/")
                    } else {
                        name
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    entries.truncate(50);
    format!(
        "<workspace>\nWorking directory (relative paths resolve here): {}\nTop-level entries: {}\n</workspace>",
        root.display(),
        if entries.is_empty() { "(empty or unreadable)".to_string() } else { entries.join(", ") }
    )
}

fn predict_success_from_ice(examples: &[String]) -> f32 {
    if examples.is_empty() {
        return 0.5;
    }
    let successes = examples
        .iter()
        .filter(|e| e.contains("(succeeded)") || e.contains("SUCCEEDED"))
        .count();
    (successes as f32 + 1.0) / (examples.len() as f32 + 2.0)
}

fn compact_history(task: &TaskNode, canvas: Option<&str>, recent_full: usize) -> String {
    let mut history = String::new();
    let older_len = task.steps.len().saturating_sub(recent_full);
    if older_len > 0 {
        history.push_str(&format!(
            "<compaction older_steps=\"{}\" recent_full=\"{}\">\n",
            older_len, recent_full
        ));
        if let Some(canvas) = canvas.filter(|text| !text.trim().is_empty()) {
            history.push_str("Earlier steps overview:\n");
            history.push_str(canvas);
            history.push('\n');
        }
        history.push_str("Earlier steps ledger:\n");
        for step in &task.steps[..older_len] {
            let status = if step.observation.success {
                "ok"
            } else {
                "err"
            };
            let params = serde_json::to_string(&step.action.params).unwrap_or_default();
            let params = truncate(&params, 140).replace('\n', " ");
            let obs = if step.observation.success {
                truncate(&step.observation.output, 180)
            } else {
                format!(
                    "ERROR: {}",
                    truncate(step.observation.error.as_deref().unwrap_or("unknown"), 180)
                )
            }
            .replace('\n', " ");
            let artifacts = if step.observation.artifacts.is_empty() {
                String::new()
            } else {
                format!(" artifacts={}", step.observation.artifacts.join(","))
            };
            history.push_str(&format!(
                "- step {} [{status}] {}({params}) => {obs}{artifacts}\n",
                step.index, step.action.tool_name
            ));
        }
        history.push_str("</compaction>\n\nMost recent steps (detail):\n");
    }
    history
}

/// Resolve the effective per-task context ceiling. The override only ever
/// clamps the LCAP-selected value down; raising it would let an H1 sweep
/// silently exceed LCAP's learned distribution and contaminate other runs.
pub(crate) fn effective_memory_ceiling(lcap_ceiling: u32, override_budget: Option<u32>) -> u32 {
    match override_budget {
        Some(b) => lcap_ceiling.min(b),
        None => lcap_ceiling,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        augment_with_repair_hint, auto_repair_enabled_value, compact_history,
        edit_required_synthesis_guidance, effective_memory_ceiling, finish_answer_from_params,
        has_successful_file_mutation, normalize_action, parse_react_step, predict_success_from_ice,
        run_verifier_command,
        should_force_synthesis, should_forfeit_after_synthesis, successful_observation_summary,
        task_requires_file_edit, traceback_file_line, try_python_verifier_repair,
        verifier_failed_observation, verifier_failed_since_latest_file_mutation,
        verifier_requires_new_edit_observation, Observation, TaskVerifier,
    };
    use crate::agentd::graph::{ExecutionStep, TaskNode, TaskType};
    use crate::policyd::PermissionScope;
    use crate::toolbridge::executor::Action;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn override_clamps_below_lcap() {
        assert_eq!(effective_memory_ceiling(8192, Some(4096)), 4096);
    }

    #[test]
    fn override_never_raises_above_lcap() {
        assert_eq!(effective_memory_ceiling(4096, Some(16384)), 4096);
    }

    #[test]
    fn no_override_returns_lcap_value() {
        assert_eq!(effective_memory_ceiling(8192, None), 8192);
    }

    #[test]
    fn override_equal_to_lcap_is_identity() {
        assert_eq!(effective_memory_ceiling(8192, Some(8192)), 8192);
    }

    #[test]
    fn zero_override_drops_ceiling_to_zero() {
        // Useful sanity case: --memory-budget 0 forces zero injection,
        // matching H1's left endpoint of the sweep.
        assert_eq!(effective_memory_ceiling(8192, Some(0)), 0);
    }

    #[test]
    fn finish_answer_accepts_nonempty_answer_field() {
        assert_eq!(
            finish_answer_from_params(&json!({"answer": "VERSION_ID=24.04; kernel=6.17.0"})),
            Some("VERSION_ID=24.04; kernel=6.17.0".to_string())
        );
    }

    #[test]
    fn finish_answer_rejects_empty_payload() {
        assert_eq!(finish_answer_from_params(&json!({})), None);
        assert_eq!(finish_answer_from_params(&json!({"answer": "   "})), None);
    }

    #[test]
    fn finish_answer_accepts_legacy_summary_field() {
        assert_eq!(
            finish_answer_from_params(
                &json!({"summary": "Task passed after running cargo check."})
            ),
            Some("Task passed after running cargo check.".to_string())
        );
    }

    #[test]
    fn normalize_action_strips_markdown_and_inlines_params() {
        // markdown-wrapped inline call (the distilled-model failure mode)
        let (tool, params) = normalize_action("**`fs.hash_edit({\"line\":12})`**");
        assert_eq!(tool, "fs.hash_edit");
        assert_eq!(params.as_deref(), Some("{\"line\":12}"));
    }

    #[test]
    fn normalize_action_plain_name_no_params() {
        let (tool, params) = normalize_action("fs.window_open");
        assert_eq!(tool, "fs.window_open");
        assert_eq!(params, None);
    }

    #[test]
    fn normalize_action_strips_leading_ordinal_and_backticks() {
        let (tool, params) = normalize_action("1. `finish`");
        assert_eq!(tool, "finish");
        assert_eq!(params, None);
    }

    #[test]
    fn normalize_action_empty_call_falls_back_to_no_params() {
        let (tool, params) = normalize_action("fs.list()");
        assert_eq!(tool, "fs.list");
        assert_eq!(params, None);
    }

    #[test]
    fn parse_react_step_recovers_markdown_inline_action() {
        // Layout B (thought continues, no label) with a markdown-wrapped inline call —
        // previously unparseable -> policy.denied; must now resolve cleanly.
        let text = "I will read the file.\nAction: **`fs.read({\"path\":\"lru.py\"})`**";
        let parsed = parse_react_step(text).expect("should parse");
        assert_eq!(parsed.tool_name, "fs.read");
        assert_eq!(parsed.params, json!({"path": "lru.py"}));
    }

    #[test]
    fn parse_react_step_explicit_action_input_still_wins() {
        // The classic separate-line layout must be unchanged by the normalization.
        let text = "Thought: do it\nAction: fs.read\nAction Input: {\"path\":\"a.py\"}";
        let parsed = parse_react_step(text).expect("should parse");
        assert_eq!(parsed.tool_name, "fs.read");
        assert_eq!(parsed.params, json!({"path": "a.py"}));
    }

    #[test]
    fn synthesis_guard_triggers_once_at_checkpoint() {
        assert!(!should_force_synthesis(13, false, 14));
        assert!(should_force_synthesis(14, false, 14));
        assert!(!should_force_synthesis(15, true, 14));
    }

    #[test]
    fn synthesis_forfeit_waits_until_threshold() {
        assert!(!should_forfeit_after_synthesis(17, true, 18));
        assert!(should_forfeit_after_synthesis(18, true, 18));
        assert!(!should_forfeit_after_synthesis(18, false, 18));
    }

    #[test]
    fn successful_observation_summary_uses_recent_successes_only() {
        let mut task = TaskNode::new("summarize observations".to_string(), TaskType::Research, 1);
        task.steps.push(ExecutionStep {
            index: 1,
            thought: "read".to_string(),
            action: Action {
                tool_name: "fs.read".to_string(),
                params: json!({"path": "Cargo.toml"}),
                risk_score: 0,
            },
            observation: Observation {
                success: true,
                output: "package name is professor-x\nversion is 0.1.0".to_string(),
                error: None,
                tokens_used: 0,
                execution_ms: 0,
                artifacts: Vec::new(),
            },
            timestamp: Utc::now(),
        });
        task.steps.push(ExecutionStep {
            index: 2,
            thought: "bad".to_string(),
            action: Action {
                tool_name: "shell.restricted".to_string(),
                params: json!({"command": "bad"}),
                risk_score: 0,
            },
            observation: Observation::err("command failed"),
            timestamp: Utc::now(),
        });

        let summary = successful_observation_summary(&task, 5, 200);
        assert!(summary.contains("step 1 `fs.read`"));
        assert!(summary.contains("package name is professor-x"));
        assert!(!summary.contains("command failed"));
    }

    #[test]
    fn edit_required_task_needs_successful_file_mutation() {
        let mut task = TaskNode::new(
            "In mathx.py, double(x) computes x * 2 but never returns it. Fix it.".to_string(),
            TaskType::UserRequest,
            1,
        );
        assert!(task_requires_file_edit(&task));
        assert!(!has_successful_file_mutation(&task));

        task.steps.push(ExecutionStep {
            index: 1,
            thought: "read".to_string(),
            action: Action {
                tool_name: "fs.window_open".to_string(),
                params: json!({"path": "mathx.py"}),
                risk_score: 0,
            },
            observation: Observation {
                success: true,
                output: "L1|6a3| def double(x):\nL2|d29|     x * 2".to_string(),
                error: None,
                tokens_used: 0,
                execution_ms: 0,
                artifacts: Vec::new(),
            },
            timestamp: Utc::now(),
        });
        assert!(!has_successful_file_mutation(&task));

        task.steps.push(ExecutionStep {
            index: 2,
            thought: "edit".to_string(),
            action: Action {
                tool_name: "fs.hash_edit".to_string(),
                params: json!({"path": "mathx.py", "line": 2}),
                risk_score: 0,
            },
            observation: Observation {
                success: true,
                output: "hash_edit apply mathx.py line 2".to_string(),
                error: None,
                tokens_used: 0,
                execution_ms: 0,
                artifacts: Vec::new(),
            },
            timestamp: Utc::now(),
        });
        assert!(has_successful_file_mutation(&task));
    }

    #[test]
    fn edit_required_guidance_names_mutation_tools() {
        let task = TaskNode::new(
            "In helper.py, fix slugify. The files are in /tmp/x.".to_string(),
            TaskType::UserRequest,
            1,
        );
        let obs = edit_required_synthesis_guidance(&task);
        assert!(!obs.success);
        assert!(obs.output.contains("Do not finish"));
        assert!(obs.output.contains("fs.hash_edit"));
    }

    #[test]
    fn verifier_command_reports_pass_and_failure_output() {
        let root = std::env::temp_dir().join(format!("px-verifier-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("check.py"), "import sys\nsys.exit(0)\n").unwrap();
        let pass = run_verifier_command("python3 check.py", &root, 0);
        assert!(pass.passed);
        assert_eq!(pass.exit_code, 0);

        std::fs::write(
            root.join("check.py"),
            "import sys\nprint('still broken')\nsys.exit(1)\n",
        )
        .unwrap();
        let fail = run_verifier_command("python3 check.py", &root, 0);
        assert!(!fail.passed);
        assert_eq!(fail.exit_code, 1);
        assert!(fail.output.contains("still broken"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn verifier_failure_blocks_finish_until_new_edit() {
        let mut task = TaskNode::new(
            "In running.py, fix the running_max bug.".to_string(),
            TaskType::UserRequest,
            1,
        );
        task.steps.push(ExecutionStep {
            index: 1,
            thought: "edit".to_string(),
            action: Action {
                tool_name: "fs.hash_edit".to_string(),
                params: json!({"path": "running.py", "line": 2}),
                risk_score: 0,
            },
            observation: Observation {
                success: true,
                output: "hash_edit apply running.py line 2".to_string(),
                error: None,
                tokens_used: 0,
                execution_ms: 0,
                artifacts: Vec::new(),
            },
            timestamp: Utc::now(),
        });
        task.steps.push(ExecutionStep {
            index: 2,
            thought: "finish".to_string(),
            action: Action {
                tool_name: "finish".to_string(),
                params: json!({"answer": "done"}),
                risk_score: 0,
            },
            observation: verifier_failed_observation(
                &TaskVerifier {
                    workdir: std::env::temp_dir(),
                    command: "python3 check.py".to_string(),
                    expect_exit: 0,
                },
                &super::VerifierResult {
                    passed: false,
                    exit_code: 1,
                    output: "NameError: name 'inf' is not defined".to_string(),
                },
            ),
            timestamp: Utc::now(),
        });

        assert!(verifier_failed_since_latest_file_mutation(&task));
        let obs = verifier_requires_new_edit_observation();
        assert!(obs
            .output
            .contains("no successful file edit has happened since"));

        task.steps.push(ExecutionStep {
            index: 3,
            thought: "repair".to_string(),
            action: Action {
                tool_name: "fs.hash_edit".to_string(),
                params: json!({"path": "running.py", "line": 2}),
                risk_score: 0,
            },
            observation: Observation {
                success: true,
                output: "hash_edit apply running.py line 2".to_string(),
                error: None,
                tokens_used: 0,
                execution_ms: 0,
                artifacts: Vec::new(),
            },
            timestamp: Utc::now(),
        });
        assert!(!verifier_failed_since_latest_file_mutation(&task));
    }

    #[test]
    fn python_verifier_repair_fixes_undefined_inf_inside_workspace() {
        let root = std::env::temp_dir().join(format!("px-inf-repair-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("running.py");
        std::fs::write(&file, "def running_max(xs):\n    m = -inf\n    return m\n").unwrap();
        let output = format!(
            "stderr:\nTraceback (most recent call last):\n  File \"{}\", line 2, in running_max\n    m = -inf\nNameError: name 'inf' is not defined\n",
            file.display()
        );
        let parsed = traceback_file_line(&output).unwrap();
        assert_eq!(parsed.0, file);
        assert_eq!(parsed.1, 2);

        let repair = try_python_verifier_repair(
            &TaskVerifier {
                workdir: root.clone(),
                command: "python3 check.py".to_string(),
                expect_exit: 0,
            },
            &super::VerifierResult {
                passed: false,
                exit_code: 1,
                output,
            },
        )
        .unwrap();
        assert!(repair.contains("float('-inf')"));
        assert!(std::fs::read_to_string(&file)
            .unwrap()
            .contains("float('-inf')"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn compact_history_keeps_old_steps_bounded_and_recent_steps_external() {
        let mut task = TaskNode::new("compact long context".to_string(), TaskType::Research, 1);
        for idx in 1..=5 {
            task.steps.push(ExecutionStep {
                index: idx,
                thought: format!("thought {idx}"),
                action: Action {
                    tool_name: if idx == 2 {
                        "shell.restricted".to_string()
                    } else {
                        "fs.read".to_string()
                    },
                    params: json!({"path": format!("file-{idx}.txt")}),
                    risk_score: 0,
                },
                observation: if idx == 2 {
                    Observation::err("command failed with a very long diagnostic")
                } else {
                    Observation {
                        success: true,
                        output: format!("{} useful output", "x".repeat(1000)),
                        error: None,
                        tokens_used: 0,
                        execution_ms: 0,
                        artifacts: if idx == 1 {
                            vec!["artifacts/commands/one.json".to_string()]
                        } else {
                            Vec::new()
                        },
                    }
                },
                timestamp: Utc::now(),
            });
        }

        let compacted = compact_history(&task, Some("graph TD\n  A-->B"), 2);
        assert!(compacted.contains("<compaction older_steps=\"3\" recent_full=\"2\">"));
        assert!(compacted.contains("Earlier steps overview:"));
        assert!(compacted.contains("Earlier steps ledger:"));
        assert!(compacted.contains("step 1 [ok] fs.read"));
        assert!(compacted.contains("artifacts/commands/one.json"));
        assert!(compacted.contains("step 2 [err] shell.restricted"));
        assert!(!compacted.contains("file-4.txt"));
        assert!(compacted.len() < 2500);
    }

    #[test]
    fn predict_no_examples_gives_uninformative_prior() {
        let p = predict_success_from_ice(&[]);
        assert!((p - 0.5).abs() < 1e-6);
    }

    #[test]
    fn predict_all_successes_gives_high_estimate() {
        let examples = vec![
            "Past task (succeeded): do X".to_string(),
            "Past task (succeeded): do Y".to_string(),
        ];
        let p = predict_success_from_ice(&examples);
        // (2+1)/(2+2) = 0.75
        assert!((p - 0.75).abs() < 1e-6);
    }

    #[test]
    fn predict_all_failures_gives_low_estimate() {
        let examples = vec![
            "Past task (failed): do X".to_string(),
            "Past task (failed): do Y".to_string(),
        ];
        let p = predict_success_from_ice(&examples);
        // (0+1)/(2+2) = 0.25
        assert!((p - 0.25).abs() < 1e-6);
    }

    #[test]
    fn auto_repair_toggle_defaults_on_except_explicit_off_values() {
        assert!(auto_repair_enabled_value(""));
        assert!(auto_repair_enabled_value("on"));
        assert!(auto_repair_enabled_value("true"));
        assert!(auto_repair_enabled_value("1"));
        assert!(!auto_repair_enabled_value("off"));
        assert!(!auto_repair_enabled_value(" OFF "));
        assert!(!auto_repair_enabled_value("false"));
        assert!(!auto_repair_enabled_value("0"));
        assert!(!auto_repair_enabled_value("disabled"));
    }

    #[test]
    fn repair_hint_explains_workspace_boundary_failures() {
        let scope = PermissionScope::default_autonomous();
        let obs = augment_with_repair_hint(
            Observation::denied("path resolves outside workspace"),
            "fs.read",
            &serde_json::json!({"path": "/tmp/outside.txt"}),
            &scope,
        );

        assert!(obs.output.contains("FIX:"));
        assert!(obs.output.contains("workspace root"));
    }

    #[test]
    fn repair_hint_explains_exact_replace_failures() {
        let scope = PermissionScope::default_autonomous();
        let mut obs = Observation::denied("expected exactly one match; found 0");
        obs.output = "replacement failed".to_string();
        let obs = augment_with_repair_hint(
            obs,
            "fs.replace",
            &serde_json::json!({"path": "src/main.rs"}),
            &scope,
        );

        assert!(obs.output.contains("replacement failed"));
        assert!(obs.output.contains("old text did not match exactly"));
    }
}
