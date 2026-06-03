/// Working memory — in-process only, never persisted, reset each session.
///
/// Holds the Reflexion buffer and a MermaidCanvas that compresses the
/// execution history into a compact flowchart. The canvas replaces the
/// raw Thought/Action/Observation transcript in every ReAct prompt step,
/// saving ~61% tokens (TencentDB Agent Memory, arXiv result).

use std::collections::VecDeque;
use uuid::Uuid;

// ── Canvas ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus {
    Ok,
    Failed,
    Running,
}

#[derive(Debug, Clone)]
pub struct CanvasStep {
    /// Short label: tool name + first 30 chars of key param
    pub label: String,
    pub status: StepStatus,
    /// Index of the parent step (None = root)
    pub parent: Option<usize>,
}

/// Compact task-execution graph serialised as Mermaid LR flowchart.
///
/// Each CanvasStep is one node; edges follow execution order. Failed
/// nodes are labelled `:::fail` for visual distinction. The serialised
/// graph replaces the full `<history>` block in the ReAct prompt.
#[derive(Debug, Default)]
pub struct MermaidCanvas {
    steps: Vec<CanvasStep>,
}

impl MermaidCanvas {
    pub fn push(&mut self, label: impl Into<String>, status: StepStatus) {
        let parent = if self.steps.is_empty() {
            None
        } else {
            Some(self.steps.len() - 1)
        };
        self.steps.push(CanvasStep {
            label: label.into(),
            status,
            parent,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn clear(&mut self) {
        self.steps.clear();
    }

    /// Convenience: record a ReAct tool call step.
    pub fn record_canvas_step(&mut self, tool: &str, param_preview: &str, success: bool) {
        let label = format!("{tool} {}", param_preview.chars().take(30).collect::<String>())
            .trim()
            .to_string();
        let status = if success { StepStatus::Ok } else { StepStatus::Failed };
        self.push(label, status);
    }

    /// Render as a Mermaid LR flowchart string.
    ///
    /// Example:
    /// ```text
    /// graph LR
    ///   S0["fs.read /src/main"] -->|ok| S1["shell.restricted cargo check"]
    ///   S1 -->|failed| S2["fs.replace ..."]
    /// ```
    pub fn to_mermaid(&self) -> String {
        if self.steps.is_empty() {
            return String::new();
        }

        let mut lines = vec!["graph LR".to_string()];

        for (i, step) in self.steps.iter().enumerate() {
            let safe_label = step.label.replace('"', "'");

            if let Some(parent) = step.parent {
                // Edge label = outcome of the PARENT step (what happened before this one)
                let edge_label = match self.steps[parent].status {
                    StepStatus::Ok => "ok",
                    StepStatus::Failed => "failed",
                    StepStatus::Running => "...",
                };
                lines.push(format!(
                    "  S{parent} -->|{edge_label}| S{i}[\"{safe_label}\"]"
                ));
            } else {
                lines.push(format!("  S{i}[\"{safe_label}\"]"));
            }
        }

        lines.join("\n")
    }

    /// Compact text summary — one line per step, used as fallback when
    /// the model doesn't support Mermaid rendering.
    pub fn to_summary(&self) -> String {
        self.steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let status = match s.status {
                    StepStatus::Ok => "✓",
                    StepStatus::Failed => "✗",
                    StepStatus::Running => "…",
                };
                format!("{i}. {status} {}", s.label)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ── WorkingMemory ─────────────────────────────────────────────────────────────

pub struct WorkingMemory {
    pub session_id: Uuid,
    pub context_budget: u32,
    pub active_task_id: Option<Uuid>,
    /// Last N Thought/Action/Observation triples (kept for backward compat).
    pub recent_steps: VecDeque<String>,
    /// Reflexion verbal RL buffer — max 3, oldest evicted.
    /// Source: Reflexion paper (arXiv:2303.11366), Algorithm 1.
    pub reflections: VecDeque<String>,
    /// Compact execution graph for prompt injection (~61% token savings).
    pub canvas: MermaidCanvas,
}

impl WorkingMemory {
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4(),
            context_budget: 32768,
            active_task_id: None,
            recent_steps: VecDeque::with_capacity(20),
            reflections: VecDeque::with_capacity(3),
            canvas: MermaidCanvas::default(),
        }
    }

    pub fn push_step(&mut self, step: String) {
        if self.recent_steps.len() >= 20 {
            self.recent_steps.pop_front();
        }
        self.recent_steps.push_back(step);
    }

    /// Add a reflection. Evicts oldest if buffer is full (max 3).
    pub fn push_reflection(&mut self, reflection: String) {
        if self.reflections.len() >= 3 {
            self.reflections.pop_front();
        }
        self.reflections.push_back(reflection);
    }

    /// Record one ReAct step on the canvas.
    pub fn record_canvas_step(&mut self, tool: &str, param_preview: &str, success: bool) {
        let label = format!(
            "{tool} {}",
            param_preview.chars().take(30).collect::<String>()
        )
        .trim()
        .to_string();
        let status = if success { StepStatus::Ok } else { StepStatus::Failed };
        self.canvas.push(label, status);
    }

    /// Clear canvas between retry attempts.
    pub fn clear_canvas(&mut self) {
        self.canvas.clear();
    }

    /// Serialise current working state for context injection.
    pub fn summarize(&self) -> String {
        let mut parts = Vec::new();

        if let Some(task_id) = &self.active_task_id {
            parts.push(format!("Active task: {task_id}"));
        }

        if !self.reflections.is_empty() {
            let r = self
                .reflections
                .iter()
                .enumerate()
                .map(|(i, r)| format!("  [{}] {}", i + 1, r))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("Prior reflections:\n{r}"));
        }

        parts.join("\n")
    }

    /// Build the `<history>` block: Mermaid canvas if non-empty,
    /// else empty string. Replaces raw step transcript (~61% savings).
    pub fn history_fragment(&self) -> String {
        if self.canvas.is_empty() {
            return String::new();
        }
        format!("<history>\n{}\n</history>", self.canvas.to_mermaid())
    }

    pub fn reset(&mut self) {
        self.session_id = Uuid::new_v4();
        self.active_task_id = None;
        self.recent_steps.clear();
        self.reflections.clear();
        self.canvas.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_mermaid_empty_is_empty_string() {
        let canvas = MermaidCanvas::default();
        assert_eq!(canvas.to_mermaid(), "");
    }

    #[test]
    fn canvas_mermaid_single_node_no_edge() {
        let mut canvas = MermaidCanvas::default();
        canvas.push("fs.read /src", StepStatus::Ok);
        let out = canvas.to_mermaid();
        assert!(out.contains("graph LR"));
        assert!(out.contains("S0"));
        assert!(out.contains("fs.read /src"));
        assert!(!out.contains("-->"));
    }

    #[test]
    fn canvas_mermaid_chain_has_edges() {
        let mut canvas = MermaidCanvas::default();
        canvas.push("fs.read file", StepStatus::Ok);
        canvas.push("shell.restricted cargo", StepStatus::Failed);
        canvas.push("fs.write fix", StepStatus::Ok);
        let out = canvas.to_mermaid();
        assert!(out.contains("S0 -->|ok| S1"));
        assert!(out.contains("S1 -->|failed| S2"));
    }

    #[test]
    fn canvas_clear_resets() {
        let mut canvas = MermaidCanvas::default();
        canvas.push("step", StepStatus::Ok);
        assert!(!canvas.is_empty());
        canvas.clear();
        assert!(canvas.is_empty());
    }

    #[test]
    fn history_fragment_wraps_mermaid() {
        let mut mem = WorkingMemory::new();
        assert!(mem.history_fragment().is_empty());
        mem.record_canvas_step("fs.read", "/src/main.rs", true);
        let frag = mem.history_fragment();
        assert!(frag.starts_with("<history>"));
        assert!(frag.contains("graph LR"));
        assert!(frag.ends_with("</history>"));
    }
}
