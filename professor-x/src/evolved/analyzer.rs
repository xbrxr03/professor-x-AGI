/// Analyzer module — distills experimental outcomes into reusable insights.
/// Source: ASI-Evolve pipeline/analyzer/analyzer.py

use crate::evolved::cognition_base::CognitionItem;

pub struct Analyzer;

impl Analyzer {
    /// Build the analyzer prompt for the LLM.
    /// Input: evolution node description + experimental results.
    /// Output: analysis string + new CognitionItem to write to cognition store.
    pub fn build_prompt(
        motivation: &str,
        diff_applied: &str,
        results_json: &str,
    ) -> String {
        format!(
            "You are the Analyzer agent in a self-evolving AI harness.\n\n\
             Evolution proposal:\n{motivation}\n\n\
             Change applied:\n```\n{diff_applied}\n```\n\n\
             Experimental results:\n{results_json}\n\n\
             Your task:\n\
             1. Write a 2-4 sentence analysis of what the results show.\n\
             2. Extract one reusable lesson (1-2 sentences) for the cognition base.\n\
             Format your response as:\n\
             ANALYSIS: <your analysis>\n\
             LESSON: <the reusable lesson>",
            motivation = motivation,
            diff_applied = diff_applied,
            results_json = results_json,
        )
    }

    /// Parse LLM response into (analysis, lesson) strings.
    pub fn parse_response(response: &str) -> (String, String) {
        let mut analysis = String::new();
        let mut lesson = String::new();

        for line in response.lines() {
            if let Some(rest) = line.strip_prefix("ANALYSIS:") {
                analysis = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("LESSON:") {
                lesson = rest.trim().to_string();
            }
        }

        // Fallback: use entire response as analysis if parsing fails
        if analysis.is_empty() {
            analysis = response.trim().to_string();
        }

        (analysis, lesson)
    }

    /// Create a CognitionItem from an analyzer lesson.
    pub fn to_cognition_item(lesson: &str, evolution_node_id: u64) -> CognitionItem {
        let mut item = CognitionItem::new(
            lesson.to_string(),
            format!("evolution-node-{evolution_node_id}"),
        );
        item.keywords = extract_keywords(lesson);
        item
    }
}

fn extract_keywords(text: &str) -> Vec<String> {
    // Simple keyword extraction: words longer than 5 chars, deduplicated.
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace()
        .filter(|w| w.len() > 5)
        .map(|w| w.to_lowercase().trim_matches(|c: char| !c.is_alphabetic()).to_string())
        .filter(|w| !w.is_empty() && seen.insert(w.clone()))
        .take(5)
        .collect()
}
