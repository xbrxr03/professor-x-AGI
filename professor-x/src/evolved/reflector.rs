/// Reflexion module — verbal self-reflection after task failure.
/// Source: Reflexion paper (arXiv:2303.11366), Algorithm 1.
/// Prompt skeleton from Architecture doc Section 7.
use crate::agentd::graph::TaskNode;

pub struct Reflector;

impl Reflector {
    /// Build the Reflexion prompt for the LLM to generate a reflection.
    /// Output is appended to task.reflections (max 3, oldest evicted).
    pub fn build_prompt(task: &TaskNode) -> String {
        let steps_text = task.steps_text();
        let prior_reflections = task
            .reflections_text()
            .unwrap_or_else(|| "none".to_string());

        format!(
            "You attempted the following task and failed.\n\
             Task: {description}\n\n\
             Your steps:\n{steps_text}\n\n\
             Previous reflections: {prior_reflections}\n\n\
             In 2-4 sentences: what went wrong, and what will you do differently next attempt?",
            description = task.description,
            steps_text = steps_text,
            prior_reflections = prior_reflections,
        )
    }
}
