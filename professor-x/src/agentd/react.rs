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
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::agentd::graph::{ExecutionStep, TaskNode, TaskStatus};
use crate::evolved::lcap::LcapPolicy;
use crate::evolved::tracker::TaskOutcome;
use crate::memd::affect::{arousal_from_load, state_label, valence_from_outcome, AffectState};
use crate::memd::episodic::EpisodicEntry;
use crate::memd::events::EventStore;
use crate::memd::task_runs::TaskRunStore;
use crate::memd::transcripts::TranscriptStore;
use crate::memd::MemoryManager;
use crate::ollama::{ModelOptions, OllamaClient};
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
    /// Stable identifier for this loop's affect session (one per ReactLoop instance).
    session_id: String,
    /// Running affect state (valence + arousal) updated after each task attempt.
    /// Injected into every ReAct prompt via `<affect .../>` (H16).
    affect: std::sync::Mutex<AffectState>,
    /// Accumulated (predicted_success, actual_success) pairs for FED recording (H15).
    /// Drained by the caller via `drain_fed_samples()` after a batch completes.
    fed_samples: std::sync::Mutex<Vec<(f32, f32)>>,
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
        Self {
            ollama,
            registry,
            policy,
            memory,
            cancel,
            lcap: Arc::new(std::sync::Mutex::new(LcapPolicy::new())),
            current_round: 0,
            events: None,
            transcripts: None,
            memory_budget_override: None,
            affect: std::sync::Mutex::new(AffectState::neutral(session_id.clone(), 0)),
            fed_samples: std::sync::Mutex::new(Vec::new()),
            session_id,
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
        let cognition_context = self.retrieve_cognition(&task.description);

        // LCAP: select context budget (Balanced before round 10, UCB1 after)
        let category = LcapPolicy::classify(&task.description);
        let lcap_ceiling = {
            let lc = self.lcap.lock().unwrap();
            lc.select(&category, self.current_round).hard_ceiling_tokens
        };
        let num_ctx = effective_memory_ceiling(lcap_ceiling, self.memory_budget_override);
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

                    self.write_episodic(task, true).await;
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
                        let arm = arm_for_ctx(num_ctx);
                        lc.update(&category, &arm, 1.0);
                    }

                    // Affect (H16): positive valence on success
                    {
                        let tool_density = task.steps.len() as f32 / 20.0;
                        let retry_pressure =
                            task.attempt_count.saturating_sub(1) as f32
                            / task.max_attempts as f32;
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

                    return Ok(TaskOutcome {
                        task_id: task.id,
                        description: task.description.clone(),
                        success: true,
                        score: 1.0,
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
                            let retry_pressure =
                                (attempt + 1) as f32 / task.max_attempts as f32;
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

        // All attempts exhausted — MARS reflection + DHE attribution

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
        let dhe = crate::evolved::dhe::Dhe::diagnose(task);
        let failure_mode = format!(
            "{mars} [DHE:layer={},lever={}]",
            dhe.failed_layer, dhe.recommended_lever
        );

        task.status = TaskStatus::Failed;
        task.completed_at = Some(Utc::now());
        task.outcome_score = Some(0.0);

        self.write_episodic(task, false).await;
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
            }),
        );

        // LCAP: penalize failed budget selection
        {
            let mut lc = self.lcap.lock().unwrap();
            let arm = arm_for_ctx(num_ctx);
            lc.update(&category, &arm, 0.0);
        }

        Ok(TaskOutcome {
            task_id: task.id,
            description: task.description.clone(),
            success: false,
            score: 0.0,
            failure_mode: Some(failure_mode),
            steps_taken: task.steps.len() as u32,
            timestamp: Utc::now(),
        })
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
        let scope = PermissionScope::default_autonomous();
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

        // LCAP: apply context budget
        let mut react_opts = ModelOptions::for_react();
        react_opts.num_ctx = Some(num_ctx);

        for step_idx in 0..MAX_STEPS {
            if self.cancel.is_cancelled() {
                return Ok(false);
            }

            // Build the full prompt for this step
            let prompt = self.build_step_prompt(task, ice_examples, cognition_context);

            // Ask the model for the next Thought + Action
            let resp = self
                .ollama
                .generate(&prompt, Some(SYSTEM_PROMPT), Some(react_opts.clone()))
                .await?;

            let (_, answer) = resp.split_thinking();
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
            match parse_react_step(&answer) {
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
                    // Special finish actions
                    if parsed.tool_name == "finish" || parsed.tool_name == "done" {
                        self.emit_event(
                            Some(session_id),
                            Some(task.id),
                            "task.finish_requested",
                            "model requested finish",
                            json!({"step": step_idx + 1}),
                        );
                        return Ok(true);
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

                    // ClawOS circuit breaker: 3 consecutive failures → pause
                    if consecutive_failures >= 3 {
                        warn!(
                            "react: circuit breaker tripped (3 consecutive failures) on task '{}'",
                            task.description
                        );
                        self.emit_event(
                            Some(session_id),
                            Some(task.id),
                            "react.circuit_breaker",
                            "circuit breaker tripped after 3 consecutive failures",
                            json!({"step": step_idx + 1}),
                        );
                        return Ok(false);
                    }

                    // Record the step
                    let step = ExecutionStep {
                        index: (step_idx + 1) as u32,
                        thought: parsed.thought,
                        action: Action {
                            tool_name: parsed.tool_name,
                            params: parsed.params,
                            risk_score: gate.risk_score,
                        },
                        observation: observation.clone(),
                        timestamp: Utc::now(),
                    };
                    task.steps.push(step);
                    let _ = TaskRunStore::new(Arc::clone(&self.memory.db)).step_recorded(task);

                    // Check if the observation signals completion
                    if is_completion_signal(&observation) {
                        return Ok(true);
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

    fn build_step_prompt(
        &self,
        task: &TaskNode,
        ice_examples: &[String],
        cognition_context: &[String],
    ) -> String {
        let mut parts = Vec::new();

        // Pinned identity + working memory from memd
        let ctx_prefix = self
            .memory
            .build_context_prefix("current")
            .unwrap_or_default();
        if !ctx_prefix.is_empty() {
            parts.push(ctx_prefix);
        }

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

        // Current task
        parts.push(format!("<task>\n{}\n</task>", task.description));

        // Reflexion buffer from prior failed attempts
        if let Some(refs) = task.reflections_text() {
            parts.push(format!("<reflections>\n{refs}\n</reflections>"));
        }

        // Prior steps this attempt
        if !task.steps.is_empty() {
            parts.push(format!("<history>\n{}\n</history>", task.steps_text()));
        }

        // Available tools
        parts.push(TOOLS_DESCRIPTION.to_string());

        // ReAct prompt suffix
        parts.push(REACT_SUFFIX.to_string());

        parts.join("\n\n")
    }

    async fn retrieve_ice(&self, task_desc: &str) -> Vec<String> {
        match self.memory.episodic.search_fts(task_desc, 3) {
            Ok(entries) => entries
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
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn retrieve_cognition(&self, query: &str) -> Vec<String> {
        use crate::evolved::CognitionStore;
        let store = CognitionStore::new(Arc::clone(&self.memory.db));
        match store.query_top_k(query, 5) {
            Ok(items) => items
                .iter()
                .filter(|i| i.quality > 0.4)
                .map(|i| i.content.clone())
                .collect(),
            Err(_) => Vec::new(),
        }
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

    async fn write_episodic(&self, task: &TaskNode, success: bool) {
        let importance = if success { 0.8 } else { 0.4 };
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
            content: summary,
            keywords: extract_keywords(&task.description),
            importance,
            embedding_id: None,
            cluster_id: None,
        };

        let _ = self.memory.episodic.insert(&entry);
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

fn parse_react_step(text: &str) -> Option<ParsedStep> {
    // Two valid layouts:
    //   A) Model re-emits label: "Thought: ...\nAction: ...\nAction Input: ..."
    //   B) Prompt ended with "Thought:" so model continues without label:
    //      "<thought text>\nAction: ...\nAction Input: ..."
    let tool_name =
        extract_field(text, "Action").map(|s| s.trim().to_lowercase().replace(' ', "_"))?;

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

    let params_raw = extract_field(text, "Action Input").unwrap_or_else(|| "{}".to_string());

    let params = serde_json::from_str(&params_raw)
        .unwrap_or_else(|_| serde_json::json!({ "input": params_raw }));

    Some(ParsedStep {
        thought,
        tool_name,
        params,
    })
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

/// Map a num_ctx token ceiling back to the nearest BudgetArm for LCAP reward tracking.
fn arm_for_ctx(num_ctx: u32) -> crate::evolved::lcap::BudgetArm {
    use crate::evolved::lcap::BudgetArm;
    match num_ctx {
        0..=4096 => BudgetArm::Sparse,
        4097..=8192 => BudgetArm::Conservative,
        8193..=12288 => BudgetArm::Balanced,
        12289..=16384 => BudgetArm::Rich,
        _ => BudgetArm::MemoryHeavy,
    }
}

// ── Prompts ───────────────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = "You are Professor X, an autonomous AI research agent. \
Complete tasks using the available tools. Reply ONLY in this exact format — no JSON, no markdown:\n\n\
Thought: <your reasoning>\n\
Action: <tool_name>\n\
Action Input: <json params>\n\n\
Example turn:\n\
Thought: I need to read the file to get its contents.\n\
Action: fs.read\n\
Action Input: {\"path\": \"/etc/os-release\"}\n\n\
When done:\n\
Thought: The task is complete.\n\
Action: finish\n\
Action Input: {}";

const TOOLS_DESCRIPTION: &str = "Available tools:
- fs.read       {\"path\": \"<path>\"} — read file contents
- fs.list       {\"path\": \"<path>\"} — list directory
- fs.write      {\"path\": \"<path>\", \"content\": \"<text>\"} — write file
- fs.replace    {\"path\": \"<path>\", \"old\": \"<exact text>\", \"new\": \"<replacement>\", \"mode\": \"check|apply\"} — replace exactly one matching text span
- fs.delete     {\"path\": \"<path>\"} — delete file (risk: high, may require approval)
- web.search    {\"query\": \"<q>\", \"num_results\": 5} — search the web
- web.fetch     {\"url\": \"<url>\"} — fetch a URL
- shell.restricted {\"command\": \"<cmd>\"} — run a shell command (sandboxed)
- patch.apply  {\"mode\": \"check|apply\", \"patch\": \"<unified diff>\"} — check or apply a reviewable git-style patch
- memory.read   {\"query\": \"<q>\", \"layer\": \"episodic|semantic|procedural\"} — search memory
- memory.write  {\"content\": \"<text>\", \"layer\": \"semantic\", \"source\": \"<src>\"} — store knowledge
- git.commit    {\"message\": \"<msg>\"} — commit current changes
- ollama.complete {\"prompt\": \"<p>\"} — run a sub-query through the LLM
- finish        {} — signal task complete
- fail          {\"reason\": \"<why>\"} — signal task failed (all options exhausted)";

const REACT_SUFFIX: &str = "Now complete the task. Follow the ReAct format.\n\nThought:";

/// Predict task success probability from ICE example outcomes.
/// Laplace-smoothed so no ICE → 0.5 uninformative prior; all-success → ~0.9.
/// Used to seed the FED sample (H15) before task execution begins.
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
    use super::{effective_memory_ceiling, predict_success_from_ice};

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
}
