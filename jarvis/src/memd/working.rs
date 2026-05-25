use std::collections::VecDeque;
use uuid::Uuid;

/// In-process only. Not persisted to SQLite. Reset each session.
pub struct WorkingMemory {
    pub session_id: Uuid,
    pub context_budget: u32,
    pub active_task_id: Option<Uuid>,
    /// Last N Thought/Action/Observation triples (kept as formatted strings for quick injection).
    pub recent_steps: VecDeque<String>,
    /// Reflexion verbal RL buffer — max 3, oldest evicted.
    /// Source: Reflexion paper (arXiv:2303.11366), Algorithm 1.
    pub reflections: VecDeque<String>,
}

impl WorkingMemory {
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4(),
            context_budget: 32768,
            active_task_id: None,
            recent_steps: VecDeque::with_capacity(20),
            reflections: VecDeque::with_capacity(3),
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

    /// Serialize current working state for context injection.
    pub fn summarize(&self) -> String {
        let mut parts = Vec::new();

        if let Some(task_id) = &self.active_task_id {
            parts.push(format!("Active task: {task_id}"));
        }

        if !self.reflections.is_empty() {
            let r = self.reflections.iter()
                .enumerate()
                .map(|(i, r)| format!("  [{}] {}", i + 1, r))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("Prior reflections:\n{r}"));
        }

        parts.join("\n")
    }

    pub fn reset(&mut self) {
        self.session_id = Uuid::new_v4();
        self.active_task_id = None;
        self.recent_steps.clear();
        self.reflections.clear();
    }
}
