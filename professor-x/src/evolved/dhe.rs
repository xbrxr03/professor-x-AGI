/// DHE — Diagnostic Harness Evolution.
///
/// 5-layer failure attribution probe (ARCHITECTURE.md Section 14.1):
///   Layer 1: Retrieval presence    — was the right memory retrieved?
///   Layer 2: Context construction  — was the retrieved content used correctly?
///   Layer 3: Tool dispatch          — did the agent call the right tool?
///   Layer 4: Tool execution         — did the tool return the right output?
///   Layer 5: Reasoning              — did the model reason correctly over the output?
///
/// Attribution determines which MHE lever to pull:
///   Layers 1-2 → Lever 2 (contextual) + LCAP
///   Layers 3-4 → Lever 3 (structural harness change)
///   Layer 5    → Lever 1 (parametric, if pattern is pervasive)
///
/// Target: ≥60% fix-prediction precision vs AHE baseline of 33.7% (H10).
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::agentd::graph::TaskNode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerResult {
    pub layer: u8,
    pub passed: bool,
    pub evidence: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticTrace {
    pub task_id: u64,
    /// 1-5 (or 0 if all layers pass — no failure to attribute)
    pub failed_layer: u8,
    pub evidence: String,
    pub confidence: f32,
    pub probe_results: Vec<LayerResult>,
    /// Which MHE lever this attribution recommends: 1, 2, or 3
    pub recommended_lever: u8,
}

pub struct Dhe;

impl Dhe {
    /// Async variant — uses an LLM-as-judge for Layer 5 (reasoning quality).
    /// Layers 1-4 are heuristic and run synchronously; Layer 5 calls Ollama.
    /// Falls back to the heuristic circular-check on Ollama error.
    pub async fn diagnose_async(
        task: &TaskNode,
        ollama: &crate::ollama::OllamaClient,
    ) -> DiagnosticTrace {
        let probes = vec![
            Self::probe_layer1(task),
            Self::probe_layer2(task),
            Self::probe_layer3(task),
            Self::probe_layer4(task),
            Self::probe_layer5_llm(task, ollama).await,
        ];
        Self::select_trace(probes)
    }

    /// Run all 5 layers synchronously (heuristic-only, no LLM call).
    /// Use `diagnose_async` when Ollama is available for better Layer 5 precision.
    pub fn diagnose(task: &TaskNode) -> DiagnosticTrace {
        let probes = vec![
            Self::probe_layer1(task),
            Self::probe_layer2(task),
            Self::probe_layer3(task),
            Self::probe_layer4(task),
            Self::probe_layer5(task),
        ];
        Self::select_trace(probes)
    }

    fn select_trace(probes: Vec<LayerResult>) -> DiagnosticTrace {
        let first_fail = probes.iter().find(|p| !p.passed);
        match first_fail {
            None => DiagnosticTrace {
                task_id: 0,
                failed_layer: 0,
                evidence: "all layers passed — failure source unclear".to_string(),
                confidence: 0.3,
                probe_results: probes,
                recommended_lever: 3,
            },
            Some(fail) => {
                let lever = match fail.layer {
                    1 | 2 => 2,
                    3 | 4 => 3,
                    5 => 1,
                    _ => 3,
                };
                DiagnosticTrace {
                    task_id: 0,
                    failed_layer: fail.layer,
                    evidence: fail.evidence.clone(),
                    confidence: fail.confidence,
                    probe_results: probes,
                    recommended_lever: lever,
                }
            }
        }
    }

    /// Layer 5 (async): LLM-as-judge for reasoning quality.
    /// Runs circular check first (fast path). If that passes, calls Ollama
    /// to assess whether the model's reasoning used observations correctly.
    async fn probe_layer5_llm(
        task: &TaskNode,
        ollama: &crate::ollama::OllamaClient,
    ) -> LayerResult {
        // Fast path: catch obvious circular reasoning without an LLM call
        let heuristic = Self::probe_layer5(task);
        if !heuristic.passed {
            return heuristic;
        }

        if task.steps.is_empty() {
            return LayerResult {
                layer: 5,
                passed: true,
                evidence: "no steps to evaluate".to_string(),
                confidence: 0.5,
            };
        }

        // Build a compact context: last 3 steps, truncated
        let steps_context = task
            .steps
            .iter()
            .rev()
            .take(3)
            .map(|s| {
                format!(
                    "Thought: {}\nObservation: {}",
                    s.thought.chars().take(150).collect::<String>(),
                    s.observation.output.chars().take(200).collect::<String>(),
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let prompt = format!(
            "Task: {}\n\nFinal steps:\n{}\n\n\
             Did the reasoning correctly use the tool observations to make \
             progress toward the task? Answer YES or NO only.",
            task.description.chars().take(200).collect::<String>(),
            steps_context,
        );

        let resp = ollama
            .generate(
                &prompt,
                Some("You are a reasoning quality evaluator. Answer with only YES or NO."),
                Some(crate::ollama::ModelOptions {
                    temperature: Some(0.0),
                    num_ctx: Some(2048),
                    top_p: None,
                    stop: None,
                    think: Some(false),
                }),
            )
            .await;

        match resp {
            Ok(r) => {
                let (_, answer) = r.split_thinking();
                let passed = answer.trim().to_uppercase().starts_with("YES");
                LayerResult {
                    layer: 5,
                    passed,
                    evidence: if passed {
                        "LLM judge: reasoning used observations correctly".to_string()
                    } else {
                        "LLM judge: reasoning failed to use observations toward the goal"
                            .to_string()
                    },
                    confidence: 0.85,
                }
            }
            Err(e) => {
                warn!("dhe: LLM-as-judge failed, falling back to heuristic: {e}");
                Self::probe_layer5(task)
            }
        }
    }

    /// Layer 1: Was relevant memory retrieved? (proxy: did steps reference memory.read?)
    fn probe_layer1(task: &TaskNode) -> LayerResult {
        let used_memory = task
            .steps
            .iter()
            .any(|s| s.action.tool_name.starts_with("memory."));

        // If task requires recall and no memory was used, Layer 1 failed
        let needs_memory = task.description.to_lowercase().contains("previous")
            || task.description.to_lowercase().contains("last time")
            || task.description.to_lowercase().contains("recall");

        let passed = !needs_memory || used_memory;

        LayerResult {
            layer: 1,
            passed,
            evidence: if passed {
                "memory retrieval not required or was attempted".to_string()
            } else {
                "task needed episodic recall but no memory.read was called".to_string()
            },
            confidence: 0.7,
        }
    }

    /// Layer 2: Was retrieved context used? (proxy: observation truncated or ignored)
    fn probe_layer2(task: &TaskNode) -> LayerResult {
        let context_overload = task.steps.iter().any(|s| {
            s.observation.output.contains("truncated") || s.observation.output.len() > 6000
        });

        LayerResult {
            layer: 2,
            passed: !context_overload,
            evidence: if context_overload {
                "observation output was truncated — model may have ignored relevant context"
                    .to_string()
            } else {
                "context size within bounds".to_string()
            },
            confidence: 0.6,
        }
    }

    /// Layer 3: Did the agent call the right tools in the right order?
    fn probe_layer3(task: &TaskNode) -> LayerResult {
        // Wrong tool: if ALL tool calls in the task were denied or failed, dispatch is broken
        let total = task.steps.len();
        let denied = task
            .steps
            .iter()
            .filter(|s| {
                s.observation
                    .error
                    .as_deref()
                    .unwrap_or("")
                    .contains("policy denied")
            })
            .count();

        let mostly_denied = total > 0 && (denied as f32 / total as f32) > 0.6;

        LayerResult {
            layer: 3,
            passed: !mostly_denied,
            evidence: if mostly_denied {
                format!(
                    "{denied}/{total} tool calls were denied — agent is not using permitted tools"
                )
            } else {
                "tool dispatch appears correct".to_string()
            },
            confidence: 0.75,
        }
    }

    /// Layer 4: Did tools return useful output?
    fn probe_layer4(task: &TaskNode) -> LayerResult {
        let total = task.steps.len();
        let failures = task.steps.iter().filter(|s| !s.observation.success).count();

        let high_failure_rate = total > 0 && (failures as f32 / total as f32) > 0.5;

        LayerResult {
            layer: 4,
            passed: !high_failure_rate,
            evidence: if high_failure_rate {
                format!("{failures}/{total} tool executions failed — tools may be broken or unavailable")
            } else {
                "tool execution success rate acceptable".to_string()
            },
            confidence: 0.8,
        }
    }

    /// Layer 5: Did the model reason correctly? (proxy: circular steps without progress)
    fn probe_layer5(task: &TaskNode) -> LayerResult {
        // Check for circular reasoning: same tool called with same params multiple times
        let mut seen_actions = std::collections::HashSet::new();
        let mut circular = false;

        for step in &task.steps {
            let key = format!(
                "{}:{}",
                step.action.tool_name,
                step.action
                    .params
                    .to_string()
                    .chars()
                    .take(100)
                    .collect::<String>()
            );
            if !seen_actions.insert(key) {
                circular = true;
                break;
            }
        }

        LayerResult {
            layer: 5,
            passed: !circular,
            evidence: if circular {
                "detected circular tool calls — model reasoning is stuck in a loop".to_string()
            } else {
                "no circular reasoning detected".to_string()
            },
            confidence: 0.65,
        }
    }
}
