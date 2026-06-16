use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    Retrieval,
    Context,
    ToolSelection,
    ToolExecution,
    Reasoning,
    MaxSteps,
    AnswerMissing,
    PolicyDenied,
    ArtifactValidation,
    Verification,
    Cancelled,
    Unknown,
}

impl FailureClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Retrieval => "retrieval",
            Self::Context => "context",
            Self::ToolSelection => "tool_selection",
            Self::ToolExecution => "tool_execution",
            Self::Reasoning => "reasoning",
            Self::MaxSteps => "max_steps",
            Self::AnswerMissing => "answer_missing",
            Self::PolicyDenied => "policy_denied",
            Self::ArtifactValidation => "artifact_validation",
            Self::Verification => "verification",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }
}

pub fn parse_failure_class(raw: &str) -> Option<FailureClass> {
    let normalized = raw.trim().to_ascii_lowercase();
    Some(match normalized.as_str() {
        "retrieval" => FailureClass::Retrieval,
        "context" => FailureClass::Context,
        "tool_selection" => FailureClass::ToolSelection,
        "tool_execution" => FailureClass::ToolExecution,
        "reasoning" => FailureClass::Reasoning,
        "max_steps" => FailureClass::MaxSteps,
        "answer_missing" => FailureClass::AnswerMissing,
        "policy_denied" => FailureClass::PolicyDenied,
        "artifact_validation" => FailureClass::ArtifactValidation,
        "verification" => FailureClass::Verification,
        "cancelled" => FailureClass::Cancelled,
        "unknown" => FailureClass::Unknown,
        _ => return None,
    })
}

pub fn classify_failure_mode(raw: &str) -> FailureClass {
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.contains("[dhe:layer=1") {
        return FailureClass::Retrieval;
    }
    if normalized.contains("[dhe:layer=2") {
        return FailureClass::Context;
    }
    if normalized.contains("[dhe:layer=3") {
        return FailureClass::ToolSelection;
    }
    if normalized.contains("[dhe:layer=4") {
        return FailureClass::ToolExecution;
    }
    if normalized.contains("[dhe:layer=5") {
        return FailureClass::Reasoning;
    }
    if normalized.contains("max-step")
        || normalized.contains("max step")
        || normalized.contains("step limit")
        || normalized.contains("loop exhaustion")
    {
        return FailureClass::MaxSteps;
    }
    if normalized.contains("empty final answer")
        || normalized.contains("finish {}")
        || normalized.contains("missing final answer")
        || normalized.contains("no final answer")
        || normalized.contains("answer-bearing")
    {
        return FailureClass::AnswerMissing;
    }
    if normalized.contains("policy denied")
        || normalized.contains("permission denied")
        || normalized.contains("approval required")
        || normalized.contains("risk score")
    {
        return FailureClass::PolicyDenied;
    }
    if normalized.contains("artifact validation")
        || normalized.contains("field:")
        || normalized.contains("artifact.")
        || normalized.contains("invalid artifact")
    {
        return FailureClass::ArtifactValidation;
    }
    if normalized.contains("verifier")
        || normalized.contains("verification")
        || normalized.contains("llm-judge")
        || normalized.contains("judge")
        || normalized.contains("check.py")
    {
        return FailureClass::Verification;
    }
    if normalized.contains("cancelled") || normalized.contains("canceled") {
        return FailureClass::Cancelled;
    }
    if normalized.contains("tool ")
        || normalized.contains("tool:")
        || normalized.contains("shell.")
        || normalized.contains("fs.")
        || normalized.contains("patch")
        || normalized.contains("git apply")
    {
        return FailureClass::ToolExecution;
    }
    FailureClass::Unknown
}

pub fn extract_failure_class(raw: &str) -> Option<FailureClass> {
    let start = raw.find("[failure:")?;
    let end = raw[start..].find(']')? + start;
    parse_failure_class(&raw[start + "[failure:".len()..end])
}

/// The harness component class a failure should target for evolution — the structured answer to
/// "failures directly target the right component class". Consistent with the DHE layer routing
/// (`failure_class_to_dhe`) and the per-layer component hints in loop_runner: retrieval/reasoning
/// failures want global guidance (SystemPrompt), context/budget failures want HarnessConfig, and
/// tool-choice/tool-execution failures want a reusable SkillDefinition.
pub fn target_component_class(class: FailureClass) -> &'static str {
    match class {
        FailureClass::Context => "HarnessConfig",
        FailureClass::ToolSelection
        | FailureClass::ToolExecution
        | FailureClass::PolicyDenied => "SkillDefinition",
        FailureClass::Retrieval
        | FailureClass::Reasoning
        | FailureClass::MaxSteps
        | FailureClass::AnswerMissing
        | FailureClass::ArtifactValidation
        | FailureClass::Verification
        | FailureClass::Cancelled
        | FailureClass::Unknown => "SystemPrompt",
    }
}

pub fn normalize_failure_mode(raw: &str) -> String {
    let trimmed = strip_failure_prefix(raw).trim();
    let class = classify_failure_mode(trimmed);
    format!("[failure:{}] {}", class.as_str(), trimmed)
}

fn strip_failure_prefix(raw: &str) -> &str {
    if raw.starts_with("[failure:") {
        if let Some(end) = raw.find(']') {
            return &raw[end + 1..];
        }
    }
    raw
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dhe_layer_maps_to_structured_class() {
        assert_eq!(
            classify_failure_mode("reflection text [DHE:layer=4,lever=3]"),
            FailureClass::ToolExecution
        );
    }

    #[test]
    fn normalizer_adds_single_prefix() {
        let normalized = normalize_failure_mode("field:recorded_at missing");
        assert_eq!(
            normalized,
            "[failure:artifact_validation] field:recorded_at missing"
        );
        assert_eq!(normalize_failure_mode(&normalized), normalized);
    }

    #[test]
    fn prefix_round_trips_to_enum() {
        let normalized = normalize_failure_mode("policy denied: write outside workspace");
        assert_eq!(
            extract_failure_class(&normalized),
            Some(FailureClass::PolicyDenied)
        );
    }

    #[test]
    fn failure_class_routes_to_the_right_component_class() {
        // Tool-choice/execution failures want a reusable skill; budget failures want config;
        // everything else wants global system-prompt guidance.
        assert_eq!(target_component_class(FailureClass::ToolExecution), "SkillDefinition");
        assert_eq!(target_component_class(FailureClass::ToolSelection), "SkillDefinition");
        assert_eq!(target_component_class(FailureClass::Context), "HarnessConfig");
        assert_eq!(target_component_class(FailureClass::Reasoning), "SystemPrompt");
        assert_eq!(target_component_class(FailureClass::MaxSteps), "SystemPrompt");
    }
}
