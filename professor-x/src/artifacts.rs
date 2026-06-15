//! Phase B artifact-truth layer.
//!
//! Every artifact-producing task declares an `ArtifactKind`. The validator
//! checks the file the task wrote against the per-kind schema, fails the task
//! if required fields are missing, and emits `artifact.{kind}.{valid,invalid}`
//! events the observer can render.
//!
//! Lineage:
//! - AHE (arXiv:2604.25850): change-manifest field discipline.
//! - MOSS / Phase C: "invalid until proven valid" inversion.
//! - Codex / Claude Code: run_id + harness_commit on every record.
//! - Scientific-agent repos: source citations are required, not optional.

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::agentd::graph::{TaskNode, TaskType};

// ── ArtifactKind ──────────────────────────────────────────────────────────────

/// Every artifact-producing task declares the kind of artifact it should leave
/// behind. Validators dispatch on this. Adding a variant requires implementing
/// `required_fields` and `validate_file` for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
pub enum ArtifactKind {
    DailyUpdate,
    LiteratureNote,
    ExperimentResult,
    HiroRun,
    HiroNullBaseline,
    EvolutionProposal,
    EvolutionRejection,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DailyUpdate => "daily_update",
            Self::LiteratureNote => "literature_note",
            Self::ExperimentResult => "experiment_result",
            Self::HiroRun => "hiro_run",
            Self::HiroNullBaseline => "hiro_null_baseline",
            Self::EvolutionProposal => "evolution_proposal",
            Self::EvolutionRejection => "evolution_rejection",
        }
    }

    pub fn from_str(raw: &str) -> Option<Self> {
        match raw {
            "daily_update" | "DailyUpdate" => Some(Self::DailyUpdate),
            "literature_note" | "LiteratureNote" => Some(Self::LiteratureNote),
            "experiment_result" | "ExperimentResult" => Some(Self::ExperimentResult),
            "hiro_run" | "HiroRun" => Some(Self::HiroRun),
            "hiro_null_baseline" | "HiroNullBaseline" => Some(Self::HiroNullBaseline),
            "evolution_proposal" | "EvolutionProposal" => Some(Self::EvolutionProposal),
            "evolution_rejection" | "EvolutionRejection" => Some(Self::EvolutionRejection),
            _ => None,
        }
    }

    /// Required fields. For JSON kinds these are top-level keys; for markdown
    /// kinds these are YAML-frontmatter keys.
    pub fn required_fields(self) -> &'static [&'static str] {
        match self {
            Self::DailyUpdate => &["date", "recorded_at"],
            Self::LiteratureNote => &["title", "citations", "recorded_at"],
            Self::ExperimentResult => &["run_id", "harness_commit", "method", "recorded_at"],
            Self::HiroRun => &[
                "run_id",
                "round",
                "harness_commit",
                "p_tool",
                "p_plan",
                "p_correct",
                "pass_at_3",
                "recorded_at",
            ],
            Self::HiroNullBaseline => &[
                "run_id",
                "harness_commit",
                "rounds",
                "frozen_harness",
                "recorded_at",
            ],
            Self::EvolutionProposal => {
                &["target_component", "motivation", "manifest", "generated_at"]
            }
            Self::EvolutionRejection => &["target_component", "reason", "generated_at"],
        }
    }

    /// Expected file format for parsing.
    fn format(self) -> ArtifactFormat {
        match self {
            Self::DailyUpdate | Self::LiteratureNote | Self::ExperimentResult => {
                ArtifactFormat::MarkdownFrontmatter
            }
            Self::HiroRun
            | Self::HiroNullBaseline
            | Self::EvolutionProposal
            | Self::EvolutionRejection => ArtifactFormat::Json,
        }
    }

    /// Directory under repo root where this kind is allowed to live.
    /// Used by `validate_path_root`.
    pub fn allowed_root(self) -> &'static str {
        match self {
            Self::DailyUpdate => "professor-x/ops/daily",
            Self::LiteratureNote => "brain/literature",
            Self::ExperimentResult => "brain/experiments",
            Self::HiroRun => "professor-x/artifacts/hiro/rounds",
            Self::HiroNullBaseline => "professor-x/artifacts/hiro/null-baselines",
            Self::EvolutionProposal => "professor-x/artifacts/evolution/proposals",
            Self::EvolutionRejection => "professor-x/artifacts/evolution/rejections",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ArtifactFormat {
    Json,
    MarkdownFrontmatter,
}

// ── Validation outcome ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

impl ArtifactCheck {
    fn pass(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            detail: detail.into(),
        }
    }

    fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactValidationReport {
    pub id: Uuid,
    pub task_id: Uuid,
    pub task_description: String,
    pub kind: Option<String>,
    pub passed: bool,
    pub checks: Vec<ArtifactCheck>,
    pub artifacts: Vec<String>,
    pub report_path: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

impl ArtifactValidationReport {
    pub fn failure_reason(&self) -> Option<String> {
        if self.passed {
            return None;
        }
        Some(
            self.checks
                .iter()
                .filter(|check| !check.passed)
                .map(|check| format!("{}: {}", check.name, check.detail))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }
}

// ── ArtifactValidator ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ArtifactValidator {
    report_dir: PathBuf,
}

impl ArtifactValidator {
    pub fn new(report_dir: PathBuf) -> Self {
        Self { report_dir }
    }

    /// Per-task validation invoked by the scheduler/React loop after a task
    /// completes. Returns `Ok(None)` if the task did not declare an
    /// `expected_artifact_kind` and is not a daily-cycle job — we don't gate
    /// arbitrary user tasks.
    pub fn validate_task(&self, task: &TaskNode) -> Result<Option<ArtifactValidationReport>> {
        // Parse the declared kind, if any. Unknown values become an explicit
        // failure so a typo in daily-cycle.toml can't silently bypass the gate.
        let declared = match task.expected_artifact_kind.as_deref() {
            Some(raw) => match ArtifactKind::from_str(raw) {
                Some(k) => Some((k, false)),
                None => {
                    let report = ArtifactValidationReport {
                        id: Uuid::new_v4(),
                        task_id: task.id,
                        task_description: task.description.clone(),
                        kind: Some(raw.to_string()),
                        passed: false,
                        checks: vec![ArtifactCheck::fail(
                            "kind_recognized",
                            format!("unknown expected_artifact_kind '{raw}'"),
                        )],
                        artifacts: Vec::new(),
                        report_path: None,
                        recorded_at: Utc::now(),
                    };
                    return Ok(Some(report));
                }
            },
            None => None,
        };

        let kind = declared.map(|(k, _)| k).or_else(|| {
            // Back-compat: scheduled daily-update jobs default to DailyUpdate
            // even when the schedule config does not declare a kind.
            if task.task_type == TaskType::Scheduled {
                scheduled_job_id(&task.description)
                    .filter(|job_id| job_id.contains("daily-update"))
                    .map(|_| ArtifactKind::DailyUpdate)
            } else {
                None
            }
        });

        let Some(kind) = kind else {
            return Ok(None);
        };

        let mut checks = Vec::new();
        let mut artifacts = Vec::new();

        // Cross-cutting structural checks run for every kind.
        checks.push(check_no_nested_pxpx());
        checks.push(check_no_nested_brain_writes());

        // Locate the expected file. For DailyUpdate we use today's date; for
        // other kinds we look in `allowed_root()` for the most recently
        // modified file. A separate path-aware API can be added later for
        // operator commits.
        let candidate = locate_artifact(kind);
        match candidate {
            Some(path) => {
                artifacts.push(path.to_string_lossy().to_string());
                checks.push(check_path_root(kind, &path));
                checks.extend(validate_required_fields(kind, &path));
            }
            None => {
                checks.push(ArtifactCheck::fail(
                    format!("{}_artifact_present", kind.as_str()),
                    format!(
                        "no artifact of kind {} found under {}",
                        kind.as_str(),
                        kind.allowed_root()
                    ),
                ));
            }
        }

        let passed = checks.iter().all(|c| c.passed);
        Ok(Some(ArtifactValidationReport {
            id: Uuid::new_v4(),
            task_id: task.id,
            task_description: task.description.clone(),
            kind: Some(kind.as_str().to_string()),
            passed,
            checks,
            artifacts,
            report_path: None,
            recorded_at: Utc::now(),
        }))
    }

    pub fn write_report(&self, report: &mut ArtifactValidationReport) -> Result<PathBuf> {
        let dir = self
            .report_dir
            .join(Utc::now().format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", report.task_id));
        report.report_path = Some(path.to_string_lossy().to_string());
        let json = serde_json::to_string_pretty(report)?;
        let mut file = std::fs::File::create(&path)?;
        writeln!(file, "{json}")?;
        Ok(path)
    }

    /// One-shot scan invoked by `--validate-artifacts`. Walks the repo,
    /// matches each file to its kind by path, validates it, and returns a
    /// summary across all kinds.
    pub fn scan_repo(&self, repo_root: &Path) -> ScanReport {
        let mut entries: Vec<(ArtifactKind, PathBuf, ArtifactCheckBundle)> = Vec::new();

        for kind in [
            ArtifactKind::DailyUpdate,
            ArtifactKind::LiteratureNote,
            ArtifactKind::ExperimentResult,
            ArtifactKind::HiroRun,
            ArtifactKind::HiroNullBaseline,
            ArtifactKind::EvolutionProposal,
            ArtifactKind::EvolutionRejection,
        ] {
            for root in repo_scan_roots(repo_root, kind) {
                if !root.exists() {
                    continue;
                }
                for path in walk_dir(&root)
                    .into_iter()
                    .filter(|path| is_artifact_file(path))
                {
                    let mut checks = vec![check_path_root(kind, &path)];
                    checks.extend(validate_required_fields(kind, &path));
                    checks.push(check_no_nested_pxpx_for_path(&path));
                    let passed = checks.iter().all(|c| c.passed);
                    entries.push((kind, path, ArtifactCheckBundle { passed, checks }));
                }
            }
        }

        let total = entries.len();
        let failed: Vec<_> = entries.iter().filter(|(_, _, b)| !b.passed).collect();
        let failed_count = failed.len();
        ScanReport {
            generated_at: Utc::now(),
            total,
            failed: failed_count,
            entries: entries
                .iter()
                .map(|(kind, path, bundle)| ScanEntry {
                    kind: kind.as_str().to_string(),
                    path: path.to_string_lossy().to_string(),
                    passed: bundle.passed,
                    checks: bundle.checks.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug)]
struct ArtifactCheckBundle {
    passed: bool,
    checks: Vec<ArtifactCheck>,
}

#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub generated_at: DateTime<Utc>,
    pub total: usize,
    pub failed: usize,
    pub entries: Vec<ScanEntry>,
}

#[derive(Debug, Serialize)]
pub struct ScanEntry {
    pub kind: String,
    pub path: String,
    pub passed: bool,
    pub checks: Vec<ArtifactCheck>,
}

impl ScanReport {
    pub fn print_human(&self) {
        println!(
            "scanned {} artifact(s) — {} failed",
            self.total, self.failed
        );
        for entry in &self.entries {
            let mark = if entry.passed { "OK  " } else { "FAIL" };
            println!("  [{mark}] {} {}", entry.kind, entry.path);
            if !entry.passed {
                for check in &entry.checks {
                    if !check.passed {
                        println!("         - {}: {}", check.name, check.detail);
                    }
                }
            }
        }
    }
}

// ── Cross-cutting structural checks ───────────────────────────────────────────

fn check_no_nested_pxpx() -> ArtifactCheck {
    // The bad pattern from REPO_STRUCTURE.md: outputs landing at
    // `professor-x/professor-x/professor-x/...`. The check runs from cwd; if
    // the agent is in the Rust crate dir, this catches accidental nested
    // writes one level below.
    let bad = Path::new("professor-x").join("professor-x");
    let ok = !bad.exists();
    if ok {
        ArtifactCheck::pass(
            "no_nested_professor_x_dir",
            "no nested professor-x/professor-x dir at cwd",
        )
    } else {
        ArtifactCheck::fail(
            "no_nested_professor_x_dir",
            format!(
                "nested {} exists; outputs should not double-prefix",
                bad.display()
            ),
        )
    }
}

fn check_no_nested_pxpx_for_path(path: &Path) -> ArtifactCheck {
    let s = path.to_string_lossy();
    if s.contains("professor-x/professor-x/professor-x")
        || s.contains("professor-x\\professor-x\\professor-x")
    {
        return ArtifactCheck::fail(
            "no_nested_professor_x_dir",
            format!("artifact path '{}' contains triple professor-x nesting", s),
        );
    }
    ArtifactCheck::pass("no_nested_professor_x_dir", "path root is well-formed")
}

fn check_no_nested_brain_writes() -> ArtifactCheck {
    // The split-brain failure mode from DE-1: agent writes to
    // `professor-x/brain/*` instead of repo-root `brain/`. After the
    // gitignore tripwire lands, any write here is local-only, but the
    // validator still flags it as a misroute.
    let nested = Path::new("professor-x").join("brain");
    if !nested.exists() {
        return ArtifactCheck::pass(
            "no_nested_brain_writes",
            "no professor-x/brain/ contents at cwd",
        );
    }
    // STUB.md is the allowed tripwire file.
    let mut bad: Vec<String> = Vec::new();
    for entry in walk_dir(&nested) {
        let name = entry
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name == "STUB.md" {
            continue;
        }
        bad.push(entry.to_string_lossy().to_string());
    }
    if bad.is_empty() {
        ArtifactCheck::pass(
            "no_nested_brain_writes",
            "professor-x/brain/ only contains STUB.md",
        )
    } else {
        ArtifactCheck::fail(
            "no_nested_brain_writes",
            format!(
                "professor-x/brain/ contains non-stub files: {}",
                bad.join(", ")
            ),
        )
    }
}

fn check_path_root(kind: ArtifactKind, path: &Path) -> ArtifactCheck {
    let s = path.to_string_lossy().replace('\\', "/");
    let allowed = kind.allowed_root();
    let crate_relative = allowed.strip_prefix("professor-x/").unwrap_or(allowed);
    if s.contains(allowed) || s.contains(crate_relative) {
        ArtifactCheck::pass("path_root", format!("{} under {}", kind.as_str(), allowed))
    } else {
        ArtifactCheck::fail(
            "path_root",
            format!(
                "{} should live under {}, found at {}",
                kind.as_str(),
                allowed,
                s
            ),
        )
    }
}

// ── Per-kind field validation ─────────────────────────────────────────────────

fn validate_required_fields(kind: ArtifactKind, path: &Path) -> Vec<ArtifactCheck> {
    let raw = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return vec![ArtifactCheck::fail(
                "readable",
                format!("cannot read {}: {e}", path.display()),
            )];
        }
    };

    let mut fields = match kind.format() {
        ArtifactFormat::Json => match parse_json_fields(&raw) {
            Ok(fields) => fields,
            Err(e) => {
                return vec![ArtifactCheck::fail(
                    "parseable",
                    format!("invalid JSON at {}: {e}", path.display()),
                )];
            }
        },
        ArtifactFormat::MarkdownFrontmatter => match parse_frontmatter_fields(&raw) {
            Ok(fields) => fields,
            Err(e) => {
                return vec![ArtifactCheck::fail(
                    "parseable",
                    format!(
                        "missing or malformed frontmatter at {}: {e}",
                        path.display()
                    ),
                )];
            }
        },
    };
    apply_artifact_schema_aliases(kind, &mut fields);

    let mut checks = vec![ArtifactCheck::pass(
        "parseable",
        format!("{} parsed cleanly", path.display()),
    )];
    for required in kind.required_fields() {
        match fields.get(*required) {
            Some(value) if !value.trim().is_empty() => {
                checks.push(ArtifactCheck::pass(
                    format!("field:{required}"),
                    "present".to_string(),
                ));
            }
            Some(_) => {
                checks.push(ArtifactCheck::fail(
                    format!("field:{required}"),
                    "present but empty".to_string(),
                ));
            }
            None => {
                checks.push(ArtifactCheck::fail(
                    format!("field:{required}"),
                    "missing".to_string(),
                ));
            }
        }
    }
    checks.extend(validate_semantic_fields(kind, path, &fields));
    checks
}

fn apply_artifact_schema_aliases(kind: ArtifactKind, fields: &mut BTreeMap<String, String>) {
    match kind {
        ArtifactKind::EvolutionProposal => {
            let is_dry_run = fields.get("mode").is_some_and(|mode| mode == "dry_run");
            let has_dry_run_evidence = ["reason", "checks", "diff_hash", "harness_commit"]
                .iter()
                .all(|field| {
                    fields
                        .get(*field)
                        .is_some_and(|value| !value.trim().is_empty())
                });
            if is_dry_run && has_dry_run_evidence && !fields.contains_key("manifest") {
                fields.insert(
                    "manifest".to_string(),
                    "dry_run verification report with checks, reason, diff_hash, and harness_commit"
                        .to_string(),
                );
            }
        }
        ArtifactKind::EvolutionRejection => {
            if !fields.contains_key("reason") {
                if let Some(analysis) = fields
                    .get("analysis")
                    .filter(|analysis| !analysis.trim().is_empty())
                    .cloned()
                {
                    fields.insert("reason".to_string(), analysis);
                }
            }
        }
        _ => {}
    }
}

fn validate_semantic_fields(
    kind: ArtifactKind,
    path: &Path,
    fields: &BTreeMap<String, String>,
) -> Vec<ArtifactCheck> {
    let mut checks = Vec::new();

    for required in kind.required_fields() {
        if let Some(value) = fields.get(*required) {
            checks.push(check_not_placeholder(required, value));
        }
    }

    match kind {
        ArtifactKind::DailyUpdate => {
            if let Some(date) = fields.get("date") {
                checks.push(check_date_field("date", date));
                checks.push(check_filename_matches_date(path, date));
            }
            if let Some(recorded_at) = fields.get("recorded_at") {
                checks.push(check_timestamp_field("recorded_at", recorded_at));
            }
        }
        ArtifactKind::LiteratureNote => {
            if let Some(citations) = fields.get("citations") {
                checks.push(check_citations_field(citations));
            }
            if let Some(recorded_at) = fields.get("recorded_at") {
                checks.push(check_timestamp_field("recorded_at", recorded_at));
            }
        }
        ArtifactKind::ExperimentResult => {
            if let Some(run_id) = fields.get("run_id") {
                checks.push(check_run_id_field(run_id));
            }
            if let Some(commit) = fields.get("harness_commit") {
                checks.push(check_commit_field("harness_commit", commit));
            }
            if let Some(recorded_at) = fields.get("recorded_at") {
                checks.push(check_timestamp_field("recorded_at", recorded_at));
            }
        }
        ArtifactKind::HiroRun => {
            if let Some(run_id) = fields.get("run_id") {
                checks.push(check_run_id_field(run_id));
            }
            if let Some(commit) = fields.get("harness_commit") {
                checks.push(check_commit_field("harness_commit", commit));
            }
            if let Some(recorded_at) = fields.get("recorded_at") {
                checks.push(check_timestamp_field("recorded_at", recorded_at));
            }
        }
        ArtifactKind::HiroNullBaseline => {
            if let Some(run_id) = fields.get("run_id") {
                checks.push(check_run_id_field(run_id));
            }
            if let Some(commit) = fields.get("harness_commit") {
                checks.push(check_commit_field("harness_commit", commit));
            }
            if let Some(recorded_at) = fields.get("recorded_at") {
                checks.push(check_timestamp_field("recorded_at", recorded_at));
            }
        }
        ArtifactKind::EvolutionProposal | ArtifactKind::EvolutionRejection => {
            if let Some(generated_at) = fields.get("generated_at") {
                checks.push(check_timestamp_field("generated_at", generated_at));
            }
        }
    }

    checks
}

fn check_not_placeholder(field: &str, value: &str) -> ArtifactCheck {
    let normalized = value.trim().trim_matches('"').to_ascii_lowercase();
    let bad = [
        "todo",
        "tbd",
        "none",
        "n/a",
        "na",
        "unknown",
        "placeholder",
        "fake",
        "dummy",
        "example",
    ];
    if bad
        .iter()
        .any(|bad| normalized == *bad || normalized.contains(&format!("<{bad}>")))
    {
        ArtifactCheck::fail(
            format!("field:{field}:not_placeholder"),
            format!("'{value}' is placeholder metadata"),
        )
    } else {
        ArtifactCheck::pass(format!("field:{field}:not_placeholder"), "not placeholder")
    }
}

fn check_date_field(field: &str, value: &str) -> ArtifactCheck {
    if chrono::NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").is_ok() {
        ArtifactCheck::pass(format!("field:{field}:date"), "valid YYYY-MM-DD date")
    } else {
        ArtifactCheck::fail(
            format!("field:{field}:date"),
            format!("'{value}' is not YYYY-MM-DD"),
        )
    }
}

fn check_timestamp_field(field: &str, value: &str) -> ArtifactCheck {
    let value = value.trim();
    let valid = DateTime::parse_from_rfc3339(value).is_ok()
        || chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok();
    if valid {
        ArtifactCheck::pass(
            format!("field:{field}:timestamp"),
            "valid RFC3339 timestamp or YYYY-MM-DD date",
        )
    } else {
        ArtifactCheck::fail(
            format!("field:{field}:timestamp"),
            format!("'{value}' is not a timestamp/date"),
        )
    }
}

fn check_filename_matches_date(path: &Path, date: &str) -> ArtifactCheck {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if stem == date.trim() {
        ArtifactCheck::pass(
            "filename_matches_date",
            "daily update filename matches date",
        )
    } else {
        ArtifactCheck::fail(
            "filename_matches_date",
            format!("daily update filename '{stem}' does not match date '{date}'"),
        )
    }
}

fn check_run_id_field(value: &str) -> ArtifactCheck {
    let value = value.trim();
    let uuid_like = Uuid::parse_str(value).is_ok();
    let compact_id = value.len() >= 8
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
    if uuid_like || compact_id {
        ArtifactCheck::pass("field:run_id:shape", "run_id is reviewable")
    } else {
        ArtifactCheck::fail(
            "field:run_id:shape",
            format!("run_id '{value}' is too weak to audit"),
        )
    }
}

fn check_commit_field(field: &str, value: &str) -> ArtifactCheck {
    let value = value.trim();
    let ok = (7..=40).contains(&value.len()) && value.chars().all(|ch| ch.is_ascii_hexdigit());
    if ok {
        ArtifactCheck::pass(format!("field:{field}:commit"), "git commit hash shape")
    } else {
        ArtifactCheck::fail(
            format!("field:{field}:commit"),
            format!("'{value}' is not a 7-40 char hex commit id"),
        )
    }
}

fn check_citations_field(value: &str) -> ArtifactCheck {
    let lower = value.to_ascii_lowercase();
    let source_markers = ["arxiv", "doi:", "http://", "https://", "paper:", "isbn:"];
    if source_markers.iter().any(|marker| lower.contains(marker)) {
        ArtifactCheck::pass(
            "field:citations:sourced",
            "citations include source identifiers",
        )
    } else {
        ArtifactCheck::fail(
            "field:citations:sourced",
            "citations must include arxiv/doi/http/paper/isbn source identifiers",
        )
    }
}

/// Pull top-level scalar fields out of a JSON object. Nested objects are
/// flattened to JSON text — what we care about for required-field presence is
/// only "does the key exist with non-empty value."
fn parse_json_fields(raw: &str) -> Result<BTreeMap<String, String>> {
    let value: serde_json::Value = serde_json::from_str(raw)?;
    let mut out = BTreeMap::new();
    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            out.insert(k.clone(), json_value_as_text(v));
        }
    }
    Ok(out)
}

fn json_value_as_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => {
            if a.is_empty() {
                String::new()
            } else {
                serde_json::to_string(a).unwrap_or_default()
            }
        }
        serde_json::Value::Object(o) => {
            if o.is_empty() {
                String::new()
            } else {
                serde_json::to_string(o).unwrap_or_default()
            }
        }
    }
}

/// Parse a YAML-style frontmatter block delimited by `---` lines at the top
/// of a markdown file. We support only `key: value` pairs (scalar) and
/// `key:` followed by `- item` lines (list collapsed to a non-empty string).
/// Nested mappings are not supported — keep frontmatter shallow.
fn parse_frontmatter_fields(raw: &str) -> Result<BTreeMap<String, String>> {
    let rest = raw
        .strip_prefix("---\n")
        .or_else(|| raw.strip_prefix("---\r\n"))
        .ok_or_else(|| anyhow::anyhow!("file does not start with `---` frontmatter delimiter"))?;
    let end = rest
        .find("\n---\n")
        .or_else(|| rest.find("\r\n---\r\n"))
        .ok_or_else(|| anyhow::anyhow!("frontmatter has no closing `---` delimiter"))?;
    let block = &rest[..end];

    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let mut current_key: Option<String> = None;
    let mut current_list: Vec<String> = Vec::new();

    for line in block.lines() {
        let trimmed_end = line.trim_end();
        if trimmed_end.is_empty() {
            continue;
        }
        if let Some(stripped) = trimmed_end.trim_start().strip_prefix("- ") {
            // List item under the most recent key.
            if current_key.is_some() {
                current_list.push(stripped.trim().to_string());
            }
            continue;
        }
        if let Some((key, value)) = trimmed_end.split_once(':') {
            // Flush previous list-valued key.
            if let Some(prev_key) = current_key.take() {
                if !current_list.is_empty() {
                    out.insert(prev_key, current_list.join(","));
                    current_list.clear();
                }
            }
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if value.is_empty() {
                // Multi-line list value follows.
                current_key = Some(key);
            } else {
                out.insert(key, value);
            }
        }
    }
    // Flush trailing list-valued key.
    if let Some(prev_key) = current_key.take() {
        if !current_list.is_empty() {
            out.insert(prev_key, current_list.join(","));
        }
    }
    Ok(out)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn locate_artifact(kind: ArtifactKind) -> Option<PathBuf> {
    for root in cwd_candidate_roots(kind) {
        if !root.exists() {
            continue;
        }
        match kind {
            ArtifactKind::DailyUpdate => {
                let today = Local::now().format("%Y-%m-%d").to_string();
                let candidate = root.join(format!("{today}.md"));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
            _ => {
                // Most-recently-modified file under the allowed root.
                let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
                for path in walk_dir(&root)
                    .into_iter()
                    .filter(|path| is_artifact_file(path))
                {
                    let Ok(meta) = std::fs::metadata(&path) else {
                        continue;
                    };
                    let Ok(modified) = meta.modified() else {
                        continue;
                    };
                    if best.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
                        best = Some((modified, path));
                    }
                }
                if let Some((_, path)) = best {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn repo_scan_roots(repo_root: &Path, kind: ArtifactKind) -> Vec<PathBuf> {
    vec![repo_root.join(kind.allowed_root())]
}

fn cwd_candidate_roots(kind: ArtifactKind) -> Vec<PathBuf> {
    let allowed = kind.allowed_root();
    let mut roots = vec![PathBuf::from(allowed)];
    if let Some(stripped) = allowed.strip_prefix("professor-x/") {
        roots.push(PathBuf::from(stripped));
    }
    roots
}

fn is_artifact_file(path: &Path) -> bool {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == ".gitkeep")
    {
        return false;
    }
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("json" | "md")
    )
}

fn walk_dir(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                stack.push(path);
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name == ".gitkeep" || name == "STUB.md" {
                continue;
            }
            out.push(path);
        }
    }
    out
}

fn scheduled_job_id(description: &str) -> Option<String> {
    let marker = "scheduled daily job '";
    let start = description.find(marker)? + marker.len();
    let rest = &description[start..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn tmp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("px-art-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn non_declared_user_request_not_validated() {
        let validator = ArtifactValidator::new(tmp_dir("nodecl"));
        let task = TaskNode::new("ad-hoc".to_string(), TaskType::UserRequest, 50);
        assert!(validator.validate_task(&task).unwrap().is_none());
    }

    #[test]
    fn hiro_run_required_fields_match_persisted_shape() {
        for f in ArtifactKind::HiroRun.required_fields() {
            assert!(!f.is_empty(), "required field name must be non-empty");
        }
    }

    #[test]
    fn parse_json_fields_extracts_scalars_and_arrays() {
        let json = r#"{"run_id":"abc","round":3,"citations":["x","y"],"empty":[]}"#;
        let fields = parse_json_fields(json).unwrap();
        assert_eq!(fields.get("run_id").unwrap(), "abc");
        assert_eq!(fields.get("round").unwrap(), "3");
        assert!(fields.get("citations").unwrap().contains("x"));
        assert!(fields.get("empty").unwrap().is_empty());
    }

    #[test]
    fn parse_frontmatter_handles_scalars_and_lists() {
        let md = "---\ntitle: ICS\nrecorded_at: 2026-05-28\nsources:\n  - arXiv:1\n  - arXiv:2\n---\nbody";
        let fields = parse_frontmatter_fields(md).unwrap();
        assert_eq!(fields.get("title").unwrap(), "ICS");
        assert!(fields.get("sources").unwrap().contains("arXiv:1"));
    }

    #[test]
    fn frontmatter_without_delimiters_errors() {
        let md = "# Heading\nno frontmatter";
        assert!(parse_frontmatter_fields(md).is_err());
    }

    #[test]
    fn hiro_run_json_validates_required_fields() {
        let dir = tmp_dir("hiro-rt");
        // Pretend allowed_root for the test by writing the path the validator
        // expects relative to cwd. We bypass the full dispatcher and call the
        // field check directly.
        let path = dir.join("run.json");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            "{}",
            r#"{
                "run_id":"12345678","round":1,"harness_commit":"abcdef0",
                "p_tool":0.5,"p_plan":0.4,"p_correct":0.3,"pass_at_3":0.4,
                "recorded_at":"2026-05-28T10:00:00Z"
            }"#
        )
        .unwrap();
        let checks = validate_required_fields(ArtifactKind::HiroRun, &path);
        assert!(checks.iter().all(|c| c.passed), "checks: {:?}", checks);
    }

    #[test]
    fn hiro_run_json_missing_run_id_fails() {
        let dir = tmp_dir("hiro-bad");
        let path = dir.join("run.json");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            "{}",
            r#"{"round":1,"harness_commit":"a","p_tool":0,"p_plan":0,"p_correct":0,"pass_at_3":0,"recorded_at":"x"}"#
        )
        .unwrap();
        let checks = validate_required_fields(ArtifactKind::HiroRun, &path);
        assert!(
            checks.iter().any(|c| !c.passed && c.name == "field:run_id"),
            "expected run_id failure, got {:?}",
            checks
        );
    }

    #[test]
    fn experiment_result_markdown_missing_run_id_fails() {
        let dir = tmp_dir("expres");
        let path = dir.join("e.md");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            "{}",
            "---\nmethod: A vs B\nrecorded_at: 2026-05-28\n---\n# body"
        )
        .unwrap();
        let checks = validate_required_fields(ArtifactKind::ExperimentResult, &path);
        assert!(
            checks.iter().any(|c| !c.passed && c.name == "field:run_id"),
            "expected run_id failure, got {:?}",
            checks
        );
    }

    #[test]
    fn experiment_result_markdown_with_all_fields_passes() {
        let dir = tmp_dir("expres-ok");
        let path = dir.join("e.md");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            "{}",
            "---\nrun_id: 12345678\nharness_commit: abcdef0\nmethod: A vs B\nrecorded_at: 2026-05-28\n---\n# body"
        )
        .unwrap();
        let checks = validate_required_fields(ArtifactKind::ExperimentResult, &path);
        assert!(checks.iter().all(|c| c.passed), "checks: {:?}", checks);
    }

    #[test]
    fn experiment_result_placeholder_commit_fails() {
        let dir = tmp_dir("expres-placeholder");
        let path = dir.join("e.md");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            "{}",
            "---\nrun_id: 12345678\nharness_commit: TODO\nmethod: A vs B\nrecorded_at: 2026-05-28\n---\n# body"
        )
        .unwrap();
        let checks = validate_required_fields(ArtifactKind::ExperimentResult, &path);
        assert!(
            checks
                .iter()
                .any(|c| !c.passed && c.name == "field:harness_commit:not_placeholder"),
            "expected placeholder failure, got {:?}",
            checks
        );
        assert!(
            checks
                .iter()
                .any(|c| !c.passed && c.name == "field:harness_commit:commit"),
            "expected commit-shape failure, got {:?}",
            checks
        );
    }

    #[test]
    fn literature_note_requires_sourced_citations() {
        let dir = tmp_dir("lit-unsourced");
        let path = dir.join("note.md");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            "{}",
            "---\ntitle: Attention schema\ncitations:\n  - some blog\nrecorded_at: 2026-05-28\n---\n# body"
        )
        .unwrap();
        let checks = validate_required_fields(ArtifactKind::LiteratureNote, &path);
        assert!(
            checks
                .iter()
                .any(|c| !c.passed && c.name == "field:citations:sourced"),
            "expected sourced-citation failure, got {:?}",
            checks
        );
    }

    #[test]
    fn daily_update_filename_must_match_frontmatter_date() {
        let dir = tmp_dir("daily-date");
        let path = dir.join("2026-05-29.md");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            "{}",
            "---\ndate: 2026-05-28\nrecorded_at: 2026-05-28\n---\n# body"
        )
        .unwrap();
        let checks = validate_required_fields(ArtifactKind::DailyUpdate, &path);
        assert!(
            checks
                .iter()
                .any(|c| !c.passed && c.name == "filename_matches_date"),
            "expected filename/date failure, got {:?}",
            checks
        );
    }

    #[test]
    fn dry_run_proposal_report_satisfies_manifest_contract_with_verification_evidence() {
        let dir = tmp_dir("dry-run-proposal");
        let path = dir.join("proposal.json");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            "{}",
            r#"{
                "generated_at":"2026-06-10T07:54:06Z",
                "mode":"dry_run",
                "harness_commit":"abcdef0",
                "target_component":"SkillDefinition(\"px-test\")",
                "motivation":"verify the proposal without applying it",
                "reason":"sandbox verification passed",
                "checks":["reward_hacking_scan","sandbox_worktree"],
                "diff_hash":"854541f760360e0ee79edb8f129865fc81e33d7d975a300528bf05cae21168a5",
                "diff_bytes":1308
            }"#
        )
        .unwrap();
        let checks = validate_required_fields(ArtifactKind::EvolutionProposal, &path);
        assert!(
            checks.iter().all(|c| c.passed),
            "dry-run proposal should validate, got {:?}",
            checks
        );
    }

    #[test]
    fn rejection_report_can_use_analysis_as_reason_alias() {
        let dir = tmp_dir("reject-alias");
        let path = dir.join("rejection.json");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            "{}",
            r#"{
                "generated_at":"2026-06-04T00:01:32Z",
                "target_component":"SystemPrompt",
                "status":"Rejected",
                "analysis":"main worktree is dirty; refusing autonomous mutation"
            }"#
        )
        .unwrap();
        let checks = validate_required_fields(ArtifactKind::EvolutionRejection, &path);
        assert!(
            checks.iter().all(|c| c.passed),
            "rejection alias should validate, got {:?}",
            checks
        );
    }

    #[test]
    fn artifact_kind_roundtrip() {
        for k in [
            ArtifactKind::DailyUpdate,
            ArtifactKind::LiteratureNote,
            ArtifactKind::ExperimentResult,
            ArtifactKind::HiroRun,
            ArtifactKind::HiroNullBaseline,
            ArtifactKind::EvolutionProposal,
            ArtifactKind::EvolutionRejection,
        ] {
            assert_eq!(ArtifactKind::from_str(k.as_str()), Some(k));
        }
    }
}
