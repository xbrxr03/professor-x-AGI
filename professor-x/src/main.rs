mod agentd;
mod artifacts;
mod evolved;
mod memd;
mod observer;
mod ollama;
mod policyd;
mod toolbridge;

use anyhow::Result;
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use agentd::graph::{ExecutionStep, TaskStatus};
use agentd::react::ReactLoop;
use agentd::{TaskNode, TaskQueue, TaskType};
use artifacts::ArtifactValidator;
use evolved::CognitionStore;
use evolved::cognition_base::CognitionItem;
use evolved::hiro::load_task_inventory;
use evolved::proposer::{ChangeManifest, EvolutionNode, HarnessComponent, VerificationStatus};
use evolved::tracker::{OutcomeTracker, TaskOutcome};
use evolved::verify_diff_in_sandbox;
use evolved::verify_node_in_sandbox;
use evolved::{EvolvedLoop, HiroRunner};
use memd::MemoryManager;
use memd::coding_sessions::{CodingSessionRecord, CodingSessionStore};
use memd::coding_smoke::{CodingSmokeRecord, CodingSmokeStore};
use memd::events::EventStore;
use memd::task_runs::{TaskRun, TaskRunStore};
use memd::transcripts::{TranscriptStore, TranscriptSummary};
use memd::work_loops::{
    WorkLoopGateRecord, WorkLoopGateStore, WorkLoopPlannedJob, WorkLoopRunRecord, WorkLoopRunStore,
    WorkLoopSmokeRecord,
};
use policyd::{AuditStore, Decision, PermissionScope, PolicyEngine};
use toolbridge::executor::{Action, Observation};
use toolbridge::{ToolExecutor, ToolRegistry};

// ── CLI args ──────────────────────────────────────────────────────────────────

struct CliArgs {
    /// Print the practical operator command surface and exit.
    operator_help: bool,
    /// Run a single task immediately and exit.
    task: Option<String>,
    /// Read user tasks interactively from the terminal.
    chat: bool,
    /// Fire the daily cron job immediately (for testing).
    run_now: bool,
    /// Run HIRO benchmark for the given round number and exit.
    hiro_round: Option<u32>,
    /// Limit HIRO to the first N tasks for smoke/regression runs.
    hiro_limit: Option<usize>,
    /// Run N static HIRO null-condition rounds and exit.
    hiro_null_rounds: Option<u32>,
    /// H1 sweep override: hard ceiling on the per-task context budget.
    /// Applies to --hiro and --hiro-null. Recommended sweep set (from
    /// brain/hypotheses.md H1): 500, 1000, 2000, 4000, 6000, 10000, 16000.
    memory_budget: Option<u32>,
    /// Print the ordered daily cycle jobs and exit.
    dry_run_daily: bool,
    /// Print current daemon/scheduler/event status and exit.
    status: bool,
    /// Print the last N agent events and exit.
    events_limit: Option<usize>,
    /// Print the last N work/task/tool events and exit.
    work_feed_limit: Option<usize>,
    /// Print the last N task transcripts and exit.
    transcripts_limit: Option<usize>,
    /// Print the last N task runs and exit.
    task_runs_limit: Option<usize>,
    /// Print the last N supervised work-loop runs and exit.
    work_loops_limit: Option<usize>,
    /// Print the last N work/operator runs as a concise operator log and exit.
    run_log_limit: Option<usize>,
    /// Print a detailed autonomous/work-loop run review by run id prefix, report path, or 'latest'.
    run_review: Option<String>,
    /// Replay a work/operator run timeline by run id prefix, report path, or 'latest'.
    run_replay: Option<String>,
    /// Commit one run's report and ledger artifacts by run id prefix, report path, or 'latest'.
    publish_run: Option<String>,
    /// Print a task transcript review by task id prefix, or 'latest'.
    task_review: Option<String>,
    /// Follow agent events until interrupted.
    watch: bool,
    /// Follow work/task/tool events until interrupted.
    watch_work: bool,
    /// Print a one-shot coding-agent-style cockpit for current Prof X work.
    work_cockpit_once: bool,
    /// Refresh a coding-agent-style terminal cockpit for current Prof X work.
    observe_work_limit: Option<usize>,
    /// Open the full-screen terminal observer.
    observe: bool,
    /// Start the daemon and open the full-screen observer in one process.
    lab: bool,
    /// Run deterministic evolution accept/reject smoke checks and exit.
    evolution_smoke: bool,
    /// Verify one concrete non-committing evolution proposal and exit.
    proposal_dry_run: bool,
    /// Verify one concrete non-committing evolution proposal while streaming the work feed.
    proposal_dry_run_live: bool,
    /// Verify a unified diff patch in an isolated sandbox and exit.
    patch_verify_path: Option<PathBuf>,
    /// Verify a unified diff patch while streaming sandbox verification events.
    patch_verify_live_path: Option<PathBuf>,
    /// Verify, apply, check, and commit a unified diff patch.
    patch_apply_path: Option<PathBuf>,
    /// Verify, apply, check, and commit a unified diff patch while streaming events.
    patch_apply_live_path: Option<PathBuf>,
    /// Validate HIRO task inventory and evaluator substrate and exit.
    hiro_smoke: bool,
    /// Run deterministic local coding-agent edit/verify smoke and exit.
    coding_smoke: bool,
    /// Run a first-class bounded local coding-agent session and exit.
    coding_session: bool,
    /// Run a bounded coding-agent session while streaming the work feed.
    coding_session_live: bool,
    /// Requested goal for --coding-session. Routed only to safe local fixtures for now.
    coding_session_goal: Option<String>,
    /// Verify a repo patch as a coding-agent session without applying it.
    repo_patch_path: Option<PathBuf>,
    /// Verify a repo patch as a coding-agent session while streaming events.
    repo_patch_live_path: Option<PathBuf>,
    /// Verify, apply, and commit a repo patch as a coding-agent session.
    repo_patch_commit_path: Option<PathBuf>,
    /// Verify, apply, and commit a repo patch as a coding-agent session while streaming events.
    repo_patch_commit_live_path: Option<PathBuf>,
    /// Print the last N coding-agent sessions and exit.
    coding_sessions_limit: Option<usize>,
    /// Run N bounded local supervised work-loop cycles and exit.
    supervised_loop_cycles: Option<u32>,
    /// Select supervised loop job mix: basic or core.
    supervised_loop_profile: WorkLoopProfile,
    /// Run N bounded Prof X operator cycles using the core safety profile and exit.
    operator_run_cycles: Option<u32>,
    /// Run N bounded Prof X operator cycles including one commit-capable gate and exit.
    operator_run_commit_cycles: Option<u32>,
    /// Run N commit-capable operator cycles while streaming the work feed.
    operator_run_live_cycles: Option<u32>,
    /// Publish the just-completed work-loop report, ledger, and evidence as a git commit.
    publish_after_run: bool,
    /// Run N bounded autonomous Prof X cycles using the core safety profile and exit.
    autonomous_run_cycles: Option<u32>,
    /// Run N bounded autonomous Prof X cycles including one commit-capable gate and exit.
    autonomous_run_commit_cycles: Option<u32>,
    /// Run one sandbox-verified autonomous commit smoke and exit.
    operator_commit_smoke: bool,
    /// Run one seeded autonomous evolution cycle and exit.
    evolution_cycle: bool,
    /// Phase B truth gate one-shot: scan brain/, artifacts/, ops/daily/ against
    /// the artifact schemas and report. Exit 1 on any failure.
    validate_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkLoopProfile {
    Basic,
    Core,
    Commit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkLoopRunKind {
    Supervised,
    Operator,
}

impl WorkLoopRunKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Supervised => "supervised",
            Self::Operator => "operator",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Supervised => "supervised work loop",
            Self::Operator => "Prof X operator run",
        }
    }
}

impl WorkLoopProfile {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "basic" => Some(Self::Basic),
            "core" => Some(Self::Core),
            "commit" => Some(Self::Commit),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Core => "core",
            Self::Commit => "commit",
        }
    }
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut cli = CliArgs {
        operator_help: false,
        task: None,
        chat: false,
        run_now: false,
        hiro_round: None,
        hiro_limit: None,
        hiro_null_rounds: None,
        memory_budget: None,
        dry_run_daily: false,
        status: false,
        events_limit: None,
        work_feed_limit: None,
        transcripts_limit: None,
        task_runs_limit: None,
        work_loops_limit: None,
        run_log_limit: None,
        run_review: None,
        run_replay: None,
        publish_run: None,
        task_review: None,
        watch: false,
        watch_work: false,
        work_cockpit_once: false,
        observe_work_limit: None,
        observe: false,
        lab: false,
        evolution_smoke: false,
        proposal_dry_run: false,
        proposal_dry_run_live: false,
        patch_verify_path: None,
        patch_verify_live_path: None,
        patch_apply_path: None,
        patch_apply_live_path: None,
        hiro_smoke: false,
        coding_smoke: false,
        coding_session: false,
        coding_session_live: false,
        coding_session_goal: None,
        repo_patch_path: None,
        repo_patch_live_path: None,
        repo_patch_commit_path: None,
        repo_patch_commit_live_path: None,
        coding_sessions_limit: None,
        supervised_loop_cycles: None,
        supervised_loop_profile: WorkLoopProfile::Basic,
        operator_run_cycles: None,
        operator_run_commit_cycles: None,
        operator_run_live_cycles: None,
        publish_after_run: false,
        autonomous_run_cycles: None,
        autonomous_run_commit_cycles: None,
        operator_commit_smoke: false,
        evolution_cycle: false,
        validate_artifacts: false,
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--prof-x-help" | "--operator-help" | "--commands" => {
                cli.operator_help = true;
                i += 1;
            }
            "--task" if i + 1 < args.len() => {
                cli.task = Some(args[i + 1].clone());
                i += 2;
            }
            "--chat" | "--task-interactive" => {
                cli.chat = true;
                i += 1;
            }
            "--run-now" => {
                cli.run_now = true;
                i += 1;
            }
            "--hiro" if i + 1 < args.len() => {
                cli.hiro_round = args[i + 1].parse::<u32>().ok();
                i += 2;
            }
            "--hiro-limit" if i + 1 < args.len() => {
                cli.hiro_limit = args[i + 1].parse::<usize>().ok();
                i += 2;
            }
            "--hiro-null" if i + 1 < args.len() => {
                cli.hiro_null_rounds = args[i + 1].parse::<u32>().ok();
                i += 2;
            }
            "--memory-budget" if i + 1 < args.len() => {
                cli.memory_budget = args[i + 1].parse::<u32>().ok();
                i += 2;
            }
            "--dry-run-daily" => {
                cli.dry_run_daily = true;
                i += 1;
            }
            "--status" => {
                cli.status = true;
                i += 1;
            }
            "--events" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.events_limit = Some(limit.unwrap_or(25));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--work-feed" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.work_feed_limit = Some(limit.unwrap_or(25));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--transcripts" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.transcripts_limit = Some(limit.unwrap_or(10));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--task-runs" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.task_runs_limit = Some(limit.unwrap_or(10));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--work-loops" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.work_loops_limit = Some(limit.unwrap_or(10));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--run-log" | "--operator-log" | "--work-log" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.run_log_limit = Some(limit.unwrap_or(10));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--run-review" | "--loop-review" => {
                let value = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                let has_value = value.is_some();
                cli.run_review = Some(value.unwrap_or_else(|| "latest".to_string()));
                i += if has_value { 2 } else { 1 };
            }
            "--replay" | "--run-replay" | "--loop-replay" => {
                let value = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                let has_value = value.is_some();
                cli.run_replay = Some(value.unwrap_or_else(|| "latest".to_string()));
                i += if has_value { 2 } else { 1 };
            }
            "--publish-run" | "--publish-latest-run" | "--commit-run-log" => {
                let value = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                let has_value = value.is_some();
                cli.publish_run = Some(value.unwrap_or_else(|| "latest".to_string()));
                i += if has_value { 2 } else { 1 };
            }
            "--task-review" if i + 1 < args.len() => {
                cli.task_review = Some(args[i + 1].clone());
                i += 2;
            }
            "--watch" => {
                cli.watch = true;
                i += 1;
            }
            "--watch-work" => {
                cli.watch_work = true;
                i += 1;
            }
            "--cockpit" | "--work-cockpit-once" | "--operator-cockpit" => {
                cli.work_cockpit_once = true;
                i += 1;
            }
            "--observe-work" | "--work-cockpit" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.observe_work_limit = Some(limit.unwrap_or(12));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--observe" => {
                cli.observe = true;
                i += 1;
            }
            "--lab" => {
                cli.lab = true;
                i += 1;
            }
            "--evolution-smoke" => {
                cli.evolution_smoke = true;
                i += 1;
            }
            "--proposal-dry-run" | "--verify-proposal" => {
                cli.proposal_dry_run = true;
                i += 1;
            }
            "--proposal-dry-run-live" | "--verify-proposal-live" => {
                cli.proposal_dry_run_live = true;
                i += 1;
            }
            "--verify-patch" if i + 1 < args.len() => {
                cli.patch_verify_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--verify-patch-live" if i + 1 < args.len() => {
                cli.patch_verify_live_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--apply-verified-patch" | "--apply-patch-commit" if i + 1 < args.len() => {
                cli.patch_apply_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--apply-verified-patch-live" | "--apply-patch-commit-live" if i + 1 < args.len() => {
                cli.patch_apply_live_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--hiro-smoke" => {
                cli.hiro_smoke = true;
                i += 1;
            }
            "--coding-smoke" => {
                cli.coding_smoke = true;
                i += 1;
            }
            "--coding-session" | "--prof-x-code" => {
                cli.coding_session = true;
                let goal = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                cli.coding_session_goal = goal.clone();
                i += if goal.is_some() { 2 } else { 1 };
            }
            "--coding-session-live" | "--prof-x-code-live" => {
                cli.coding_session_live = true;
                let goal = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                cli.coding_session_goal = goal.clone();
                i += if goal.is_some() { 2 } else { 1 };
            }
            "--coding-session-goal" if i + 1 < args.len() => {
                cli.coding_session = true;
                cli.coding_session_goal = Some(args[i + 1].clone());
                i += 2;
            }
            "--repo-patch" | "--prof-x-code-patch" if i + 1 < args.len() => {
                cli.repo_patch_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--repo-patch-live" | "--prof-x-code-patch-live" if i + 1 < args.len() => {
                cli.repo_patch_live_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--repo-patch-commit" | "--prof-x-code-commit" if i + 1 < args.len() => {
                cli.repo_patch_commit_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--repo-patch-commit-live" | "--prof-x-code-commit-live" if i + 1 < args.len() => {
                cli.repo_patch_commit_live_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--coding-sessions" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.coding_sessions_limit = Some(limit.unwrap_or(10));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--supervised-loop" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.supervised_loop_cycles = Some(cycles.unwrap_or(1));
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--supervised-loop-profile" if i + 1 < args.len() => {
                if let Some(profile) = WorkLoopProfile::parse(&args[i + 1]) {
                    cli.supervised_loop_profile = profile;
                }
                i += 2;
            }
            "--operator-run" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.operator_run_cycles = Some(cycles.unwrap_or(4));
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--operator-run-commit" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.operator_run_commit_cycles = Some(cycles.unwrap_or(5));
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--operator-run-publish" | "--operator-run-commit-publish" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.operator_run_commit_cycles = Some(cycles.unwrap_or(5));
                cli.publish_after_run = true;
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--operator-run-live" | "--operator-run-commit-live" | "--prof-x-live" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.operator_run_live_cycles = Some(cycles.unwrap_or(5));
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--operator-run-publish-live"
            | "--operator-run-commit-publish-live"
            | "--prof-x-live-publish" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.operator_run_live_cycles = Some(cycles.unwrap_or(5));
                cli.publish_after_run = true;
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--publish-after-run" => {
                cli.publish_after_run = true;
                i += 1;
            }
            "--autonomous-run" | "--prof-x-run" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.autonomous_run_cycles = Some(cycles.unwrap_or(4));
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--autonomous-run-commit" | "--prof-x-run-commit" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.autonomous_run_commit_cycles = Some(cycles.unwrap_or(5));
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--autonomous-run-publish"
            | "--autonomous-run-commit-publish"
            | "--prof-x-run-publish"
            | "--prof-x-run-commit-publish" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.autonomous_run_commit_cycles = Some(cycles.unwrap_or(5));
                cli.publish_after_run = true;
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--operator-commit-smoke" => {
                cli.operator_commit_smoke = true;
                i += 1;
            }
            "--evolution-cycle" => {
                cli.evolution_cycle = true;
                i += 1;
            }
            "--validate-artifacts" => {
                cli.validate_artifacts = true;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    cli
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = parse_args();
    if cli.operator_help {
        print_operator_help()?;
        return Ok(());
    }

    let inspect_mode = cli.status
        || cli.events_limit.is_some()
        || cli.work_feed_limit.is_some()
        || cli.transcripts_limit.is_some()
        || cli.task_runs_limit.is_some()
        || cli.coding_sessions_limit.is_some()
        || cli.work_loops_limit.is_some()
        || cli.run_log_limit.is_some()
        || cli.run_review.is_some()
        || cli.task_review.is_some()
        || cli.run_replay.is_some()
        || cli.publish_run.is_some()
        || cli.watch
        || cli.watch_work
        || cli.observe_work_limit.is_some()
        || cli.observe
        || cli.lab
        || cli.chat;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            if inspect_mode {
                EnvFilter::new("error")
            } else {
                EnvFilter::new("professor_x=info,warn")
            }
        }))
        .init();

    info!("Professor X starting — single binary, five modules");

    let data_dir =
        PathBuf::from(std::env::var("PROFESSOR_X_DATA_DIR").unwrap_or_else(|_| {
            format!("{}/.professor-x", std::env::var("HOME").unwrap_or_default())
        }));

    // ── memd ──────────────────────────────────────────────────────────────
    let memory = Arc::new(MemoryManager::open(&data_dir)?);
    let event_log_dir = std::env::var("PROFESSOR_X_EVENT_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("artifacts/events"));
    let events = Arc::new(EventStore::new(Arc::clone(&memory.db)).with_jsonl_mirror(event_log_dir));
    let transcript_dir = std::env::var("PROFESSOR_X_TRANSCRIPT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("artifacts/transcripts"));
    let transcripts = Arc::new(TranscriptStore::new(Arc::clone(&memory.db), transcript_dir));
    let artifact_report_dir = std::env::var("PROFESSOR_X_ARTIFACT_REPORT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("artifacts/validation"));
    let artifact_validator = Arc::new(ArtifactValidator::new(artifact_report_dir));
    info!("memd: initialized at {}", data_dir.display());

    if cli.validate_artifacts {
        let repo_root = repo_root_from_cwd();
        let report = artifact_validator.scan_repo(&repo_root);
        report.print_human();
        if report.failed > 0 {
            std::process::exit(1);
        }
        return Ok(());
    }

    if cli.status {
        return observer::print_snapshot(Arc::clone(&memory), Arc::clone(&events));
    }

    if let Some(limit) = cli.events_limit {
        return print_events(Arc::clone(&events), limit);
    }

    if let Some(limit) = cli.work_feed_limit {
        return print_work_feed(Arc::clone(&events), limit);
    }

    if let Some(limit) = cli.transcripts_limit {
        return print_transcripts(Arc::clone(&transcripts), limit);
    }

    if let Some(limit) = cli.task_runs_limit {
        return print_task_runs(Arc::clone(&memory), limit);
    }

    if let Some(limit) = cli.coding_sessions_limit {
        return print_coding_sessions(Arc::clone(&memory), limit);
    }

    if let Some(limit) = cli.work_loops_limit {
        return print_work_loops(Arc::clone(&memory), limit);
    }

    if let Some(limit) = cli.run_log_limit {
        return print_run_log(Arc::clone(&memory), limit);
    }

    if let Some(run_ref) = cli.run_review {
        return print_run_review(Arc::clone(&memory), &run_ref);
    }

    if let Some(run_ref) = cli.run_replay {
        return print_run_replay(Arc::clone(&memory), &run_ref);
    }

    if let Some(run_ref) = cli.publish_run {
        return publish_run_artifacts(Arc::clone(&memory), &run_ref);
    }

    if let Some(task_ref) = cli.task_review {
        return print_task_review(Arc::clone(&transcripts), &task_ref);
    }

    if cli.watch {
        return watch_events(Arc::clone(&events)).await;
    }

    if cli.watch_work {
        return watch_work_feed(Arc::clone(&events)).await;
    }

    if cli.work_cockpit_once {
        return print_work_cockpit(Arc::clone(&memory), Arc::clone(&events), 16);
    }

    if let Some(limit) = cli.observe_work_limit {
        return observe_work_cockpit(Arc::clone(&memory), Arc::clone(&events), limit).await;
    }

    if cli.observe {
        return observer::run_observer(Arc::clone(&memory), Arc::clone(&events));
    }

    // ── tool registry ─────────────────────────────────────────────────────
    let registry = Arc::new(std::sync::RwLock::new(ToolRegistry::new()));
    let skills_dir = PathBuf::from("skills");
    if skills_dir.exists() {
        let skills = toolbridge::skill_loader::scan_skills_dir(&skills_dir);
        info!("toolbridge: loaded {} skill(s) from skills/", skills.len());
        for (skill, path) in &skills {
            info!(
                "toolbridge: skill '{}' — {} ({})",
                skill.name,
                skill.description,
                path.display()
            );
        }
        events.append(
            None,
            None,
            "skills.loaded",
            format!("loaded {} skill(s)", skills.len()),
            serde_json::json!({
                "skills": skills.iter().map(|(skill, _)| &skill.name).collect::<Vec<_>>(),
            }),
        )?;
    }

    if cli.dry_run_daily {
        events.append(
            None,
            None,
            "daily.dry_run",
            "printed daily cycle dry-run",
            serde_json::json!({}),
        )?;
        return dry_run_daily_cycle();
    }

    if cli.evolution_smoke {
        return run_evolution_smoke(Arc::clone(&events)).await;
    }

    if cli.proposal_dry_run {
        return run_evolution_proposal_dry_run(Arc::clone(&events)).await;
    }

    if cli.proposal_dry_run_live {
        return run_evolution_proposal_dry_run_live(Arc::clone(&events)).await;
    }

    if let Some(path) = cli.patch_verify_path {
        return run_patch_verify(Arc::clone(&events), path).await;
    }

    if let Some(path) = cli.patch_verify_live_path {
        return run_patch_verify_live(Arc::clone(&events), path).await;
    }

    if let Some(path) = cli.patch_apply_path {
        return run_patch_apply_commit(Arc::clone(&events), path).await;
    }

    if let Some(path) = cli.patch_apply_live_path {
        return run_patch_apply_commit_live(Arc::clone(&events), path).await;
    }

    if cli.hiro_smoke {
        return run_hiro_inventory_smoke(Arc::clone(&events));
    }

    // ── kill switch ───────────────────────────────────────────────────────
    let cancel = CancellationToken::new();
    setup_signal_handlers(cancel.clone());

    // ── policyd ───────────────────────────────────────────────────────────
    let policy = Arc::new(PolicyEngine::new(cancel.clone()));
    info!("policyd: initialized (approval_threshold=65, timeout=300s)");
    events.append(
        None,
        None,
        "daemon.started",
        "Professor X process started",
        serde_json::json!({
            "data_dir": data_dir,
            "cwd": std::env::current_dir().ok(),
        }),
    )?;

    {
        let audit = AuditStore::new(Arc::clone(&memory.db));
        match audit.verify_chain() {
            Ok(true) => info!("policyd: audit chain intact"),
            Ok(false) => {
                error!("policyd: AUDIT CHAIN TAMPERED — halting");
                std::process::exit(1);
            }
            Err(e) => warn!("policyd: chain verification error: {e}"),
        }
    }

    if cli.coding_smoke {
        return run_coding_smoke(
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
        )
        .await;
    }

    if cli.coding_session {
        return run_coding_session(
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cli.coding_session_goal.clone(),
        )
        .await;
    }

    if cli.coding_session_live {
        return run_coding_session_live(
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cli.coding_session_goal.clone(),
        )
        .await;
    }

    if let Some(path) = cli.repo_patch_path {
        return run_repo_patch_coding_session(
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            path,
        )
        .await;
    }

    if let Some(path) = cli.repo_patch_live_path {
        return run_repo_patch_coding_session_live(
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            path,
        )
        .await;
    }

    if let Some(path) = cli.repo_patch_commit_path {
        return run_repo_patch_commit_coding_session(
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            path,
        )
        .await;
    }

    if let Some(path) = cli.repo_patch_commit_live_path {
        return run_repo_patch_commit_coding_session_live(
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            path,
        )
        .await;
    }

    if let Some(cycles) = cli.supervised_loop_cycles {
        return run_supervised_loop(
            WorkLoopRunKind::Supervised,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cycles,
            cli.supervised_loop_profile,
            false,
        )
        .await;
    }

    if let Some(cycles) = cli.operator_run_cycles {
        return run_supervised_loop(
            WorkLoopRunKind::Operator,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cycles,
            WorkLoopProfile::Core,
            cli.publish_after_run,
        )
        .await;
    }

    if let Some(cycles) = cli.operator_run_commit_cycles {
        return run_supervised_loop(
            WorkLoopRunKind::Operator,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cycles,
            WorkLoopProfile::Commit,
            cli.publish_after_run,
        )
        .await;
    }

    if let Some(cycles) = cli.operator_run_live_cycles {
        return run_supervised_loop_live(
            WorkLoopRunKind::Operator,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cycles,
            WorkLoopProfile::Commit,
            cli.publish_after_run,
        )
        .await;
    }

    if let Some(cycles) = cli.autonomous_run_cycles {
        return run_autonomous_operator_run(
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cycles,
            WorkLoopProfile::Core,
            cli.publish_after_run,
        )
        .await;
    }

    if let Some(cycles) = cli.autonomous_run_commit_cycles {
        return run_autonomous_operator_run(
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cycles,
            WorkLoopProfile::Commit,
            cli.publish_after_run,
        )
        .await;
    }

    if cli.operator_commit_smoke {
        return run_operator_commit_smoke(Arc::clone(&events)).await;
    }

    // ── evolved: seed cognition base ──────────────────────────────────────
    {
        let cognition = CognitionStore::new(Arc::clone(&memory.db));
        cognition.seed_if_empty(seed_cognition_base())?;
        info!("evolved: cognition base has {} items", cognition.count()?);
    }

    // ── ollama health check ───────────────────────────────────────────────
    let ollama = Arc::new(ollama::OllamaClient::new("http://localhost:11434"));
    match ollama.health_check().await {
        Ok(true) => info!("ollama: reachable, model qwen3:8b-q4_k_m ready"),
        Ok(false) => warn!("ollama: reachable but model may not be loaded"),
        Err(e) => warn!("ollama: not reachable ({e}) — tasks will fail until Ollama starts"),
    }

    if cli.evolution_cycle {
        return run_one_evolution_cycle(
            Arc::clone(&ollama),
            Arc::clone(&memory),
            Arc::clone(&events),
        )
        .await;
    }

    // ── one-shot --task mode ──────────────────────────────────────────────
    if let Some(task_desc) = cli.task {
        return run_single_task(
            task_desc,
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cancel,
        )
        .await;
    }

    if cli.chat {
        return run_interactive_tasks(
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cancel,
        )
        .await;
    }

    // ── HIRO benchmark mode ───────────────────────────────────────────────
    if let Some(round) = cli.hiro_round {
        return run_hiro_benchmark(
            round,
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            cancel,
            cli.hiro_limit,
            cli.memory_budget,
        )
        .await;
    }

    if let Some(rounds) = cli.hiro_null_rounds {
        return run_hiro_null_baseline(
            rounds,
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            cancel,
            cli.hiro_limit,
            cli.memory_budget,
        )
        .await;
    }

    if cli.lab {
        return run_lab(
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            Arc::clone(&artifact_validator),
            cancel,
            cli.run_now,
        )
        .await;
    }

    run_daemon(
        ollama,
        registry,
        policy,
        memory,
        events,
        transcripts,
        artifact_validator,
        cancel,
        cli.run_now,
    )
    .await
}

// ── Evolution smoke mode ─────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct EvolutionSmokeCaseReport {
    name: String,
    expected_accepted: bool,
    accepted: bool,
    reason: String,
    checks: Vec<String>,
    diff_hash: Option<String>,
    diff_bytes: usize,
}

#[derive(Debug, serde::Serialize)]
struct EvolutionSmokeReport {
    generated_at: String,
    workspace: String,
    harness_commit: String,
    passed: bool,
    cases: Vec<EvolutionSmokeCaseReport>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct EvolutionProposalDryRunReport {
    generated_at: String,
    mode: String,
    workspace: String,
    harness_commit: String,
    target_component: String,
    motivation: String,
    accepted: bool,
    applied: bool,
    commit: Option<String>,
    reason: String,
    checks: Vec<String>,
    diff_hash: Option<String>,
    diff_bytes: usize,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PatchVerificationReport {
    generated_at: String,
    mode: String,
    patch_path: String,
    workspace: String,
    harness_commit: String,
    accepted: bool,
    applied: bool,
    commit: Option<String>,
    report_commit: Option<String>,
    reason: String,
    checks: Vec<String>,
    diff_hash: Option<String>,
    diff_bytes: usize,
}

#[derive(serde::Serialize)]
struct HiroInventorySmokeReport {
    generated_at: String,
    tasks_path: String,
    harness_commit: String,
    passed: bool,
    task_count: usize,
    tool_use: usize,
    planning: usize,
    self_correction: usize,
    duplicate_ids: Vec<String>,
}

async fn run_evolution_smoke(events: Arc<EventStore>) -> Result<()> {
    let (report, path) = execute_evolution_smoke(events).await?;

    println!(
        "Evolution sandbox smoke: {}",
        if report.passed { "passed" } else { "failed" }
    );
    println!("  report: {}", path.display());
    for case in &report.cases {
        println!(
            "  {}: {} (expected {}) — {}",
            case.name,
            if case.accepted {
                "accepted"
            } else {
                "rejected"
            },
            if case.expected_accepted {
                "accepted"
            } else {
                "rejected"
            },
            case.reason
        );
    }

    if !report.passed {
        anyhow::bail!("evolution sandbox smoke failed");
    }
    Ok(())
}

fn run_hiro_inventory_smoke(events: Arc<EventStore>) -> Result<()> {
    let (report, path) = execute_hiro_inventory_smoke(events)?;
    println!(
        "HIRO inventory smoke: {}",
        if report.passed { "passed" } else { "failed" }
    );
    println!("  report: {}", path.display());
    println!(
        "  tasks: {} (tool_use={}, planning={}, self_correction={})",
        report.task_count, report.tool_use, report.planning, report.self_correction
    );
    Ok(())
}

async fn execute_evolution_smoke(
    events: Arc<EventStore>,
) -> Result<(EvolutionSmokeReport, PathBuf)> {
    let repo_root = default_repo_root();
    let cases = evolution_smoke_cases();
    events.append(
        None,
        None,
        "evolution.smoke.started",
        "starting deterministic evolution sandbox smoke",
        serde_json::json!({
            "workspace": "repo-root",
            "harness_commit": git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
            "cases": cases.iter().map(|case| case.0).collect::<Vec<_>>(),
        }),
    )?;

    let mut reports = Vec::new();
    for (name, expected_accepted, node) in cases {
        let verification = verify_node_in_sandbox(&repo_root, &node).await?;
        let accepted = verification.outcome.accepted;
        let diff_hash = if verification.diff.is_empty() {
            None
        } else {
            Some(sha256_hex(verification.diff.as_bytes()))
        };
        let case_report = EvolutionSmokeCaseReport {
            name: name.to_string(),
            expected_accepted,
            accepted,
            reason: verification.outcome.reason.clone(),
            checks: verification.outcome.checks.clone(),
            diff_hash,
            diff_bytes: verification.diff.len(),
        };
        events.append(
            None,
            None,
            if accepted {
                "evolution.smoke.accepted"
            } else {
                "evolution.smoke.rejected"
            },
            format!(
                "smoke case '{}' {}",
                name,
                if accepted { "accepted" } else { "rejected" }
            ),
            serde_json::to_value(&case_report)?,
        )?;
        reports.push(case_report);
    }

    let passed = reports
        .iter()
        .all(|case| case.accepted == case.expected_accepted);
    let report = EvolutionSmokeReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        workspace: "repo-root".to_string(),
        harness_commit: git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        passed,
        cases: reports,
    };
    let path = write_evolution_smoke_report(&report)?;
    events.append(
        None,
        None,
        if passed {
            "evolution.smoke.passed"
        } else {
            "evolution.smoke.failed"
        },
        format!(
            "evolution sandbox smoke report written to {}",
            path.display()
        ),
        serde_json::json!({
            "passed": passed,
            "report_path": path,
        }),
    )?;
    Ok((report, path))
}

async fn execute_evolution_proposal_dry_run(
    events: Arc<EventStore>,
) -> Result<(EvolutionProposalDryRunReport, PathBuf)> {
    let repo_root = default_repo_root();
    let node = operator_proposal_node("px-operator-proposal-dry-run");
    events.append(
        None,
        None,
        "evolution.proposal_dry_run.started",
        "starting non-committing evolution proposal dry-run",
        serde_json::json!({
            "workspace": "repo-root",
            "harness_commit": git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
            "target_component": format!("{:?}", node.target_component),
            "motivation": node.motivation,
        }),
    )?;
    events.append(
        None,
        None,
        "evolution.proposal_dry_run.verifying",
        "verifying proposal in isolated sandbox worktree",
        serde_json::json!({
            "workspace": "sandbox_worktree",
            "target_component": format!("{:?}", node.target_component),
            "planned_checks": [
                "reward_hacking_scan",
                "sandbox_worktree",
                "material_diff",
                "cargo_check"
            ],
        }),
    )?;

    let verification = verify_node_in_sandbox(&repo_root, &node).await?;
    let diff_hash = if verification.diff.is_empty() {
        None
    } else {
        Some(sha256_hex(verification.diff.as_bytes()))
    };
    let report = EvolutionProposalDryRunReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        mode: "dry_run".to_string(),
        workspace: "repo-root".to_string(),
        harness_commit: git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        target_component: format!("{:?}", node.target_component),
        motivation: node.motivation,
        accepted: verification.outcome.accepted,
        applied: false,
        commit: None,
        reason: verification.outcome.reason,
        checks: verification.outcome.checks,
        diff_hash,
        diff_bytes: verification.diff.len(),
    };
    let path = write_evolution_proposal_dry_run_report(&report)?;
    events.append(
        None,
        None,
        if report.accepted {
            "evolution.proposal_dry_run.accepted"
        } else {
            "evolution.proposal_dry_run.rejected"
        },
        format!(
            "proposal dry-run {} without applying changes; report {}",
            if report.accepted {
                "accepted"
            } else {
                "rejected"
            },
            path.display()
        ),
        serde_json::json!({
            "accepted": report.accepted,
            "applied": report.applied,
            "reason": report.reason,
            "checks": report.checks,
            "diff_hash": report.diff_hash,
            "diff_bytes": report.diff_bytes,
            "report_path": path,
        }),
    )?;
    Ok((report, path))
}

async fn run_evolution_proposal_dry_run(events: Arc<EventStore>) -> Result<()> {
    let (report, path) = execute_evolution_proposal_dry_run(events).await?;
    println!(
        "Evolution proposal dry-run: {}",
        if report.accepted {
            "accepted"
        } else {
            "rejected"
        }
    );
    println!("  report: {}", path.display());
    println!("  target: {}", report.target_component);
    println!("  checks: {}", report.checks.join(", "));
    println!("  diff bytes: {}", report.diff_bytes);
    if let Some(hash) = &report.diff_hash {
        println!("  diff hash: {hash}");
    }
    println!("  reason: {}", report.reason);
    Ok(())
}

async fn run_evolution_proposal_dry_run_live(events: Arc<EventStore>) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live proposal verification");
    println!("Streaming sandbox verification events. No changes will be applied.");
    io::stdout().flush()?;

    let run_events = Arc::clone(&events);
    let mut handle =
        tokio::spawn(async move { execute_evolution_proposal_dry_run(run_events).await });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Live proposal verification interrupted.");
                handle.abort();
                anyhow::bail!("live proposal verification interrupted");
            }
            result = &mut handle => {
                for event in events.work_after_id(last_id, 200)? {
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
                let _ = result??;
                return Ok(());
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(250)) => {
                for event in events.work_after_id(last_id, 100)? {
                    last_id = event.id;
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
            }
        }
    }
}

async fn execute_patch_verify(
    events: Arc<EventStore>,
    patch_path: PathBuf,
) -> Result<(PatchVerificationReport, PathBuf)> {
    let repo_root = default_repo_root();
    let patch_raw = std::fs::read_to_string(&patch_path)
        .map_err(|e| anyhow::anyhow!("cannot read patch '{}': {e}", patch_path.display()))?;
    events.append(
        None,
        None,
        "evolution.patch_verify.started",
        "starting sandbox patch verification",
        serde_json::json!({
            "workspace": "repo-root",
            "harness_commit": git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
            "patch_path": patch_path.display().to_string(),
        }),
    )?;
    events.append(
        None,
        None,
        "evolution.patch_verify.verifying",
        "verifying patch in isolated sandbox worktree",
        serde_json::json!({
            "workspace": "sandbox_worktree",
            "patch_path": patch_path.display().to_string(),
            "planned_checks": [
                "reward_hacking_scan",
                "sandbox_worktree",
                "material_diff",
                "cargo_check"
            ],
        }),
    )?;

    let verification = verify_diff_in_sandbox(&repo_root, &patch_raw).await?;
    let diff_hash = if verification.diff.is_empty() {
        None
    } else {
        Some(sha256_hex(verification.diff.as_bytes()))
    };
    let report = PatchVerificationReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        mode: "patch_verify".to_string(),
        patch_path: patch_path.display().to_string(),
        workspace: "repo-root".to_string(),
        harness_commit: git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        accepted: verification.outcome.accepted,
        applied: false,
        commit: None,
        report_commit: None,
        reason: verification.outcome.reason,
        checks: verification.outcome.checks,
        diff_hash,
        diff_bytes: verification.diff.len(),
    };
    let path = write_patch_verification_report(&report)?;
    events.append(
        None,
        None,
        if report.accepted {
            "evolution.patch_verify.accepted"
        } else {
            "evolution.patch_verify.rejected"
        },
        format!(
            "patch verification {} without applying changes; report {}",
            if report.accepted {
                "accepted"
            } else {
                "rejected"
            },
            path.display()
        ),
        serde_json::json!({
            "accepted": report.accepted,
            "applied": report.applied,
            "reason": report.reason,
            "checks": report.checks,
            "diff_hash": report.diff_hash,
            "diff_bytes": report.diff_bytes,
            "patch_path": report.patch_path,
            "report_path": path,
        }),
    )?;
    Ok((report, path))
}

async fn run_patch_verify(events: Arc<EventStore>, patch_path: PathBuf) -> Result<()> {
    let (report, path) = execute_patch_verify(events, patch_path).await?;
    println!(
        "Patch verification: {}",
        if report.accepted {
            "accepted"
        } else {
            "rejected"
        }
    );
    println!("  report: {}", path.display());
    println!("  patch: {}", report.patch_path);
    println!("  checks: {}", report.checks.join(", "));
    println!("  diff bytes: {}", report.diff_bytes);
    if let Some(hash) = &report.diff_hash {
        println!("  diff hash: {hash}");
    }
    println!("  reason: {}", report.reason);
    Ok(())
}

async fn run_patch_verify_live(events: Arc<EventStore>, patch_path: PathBuf) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live patch verification");
    println!("Streaming sandbox verification events. No changes will be applied.");
    io::stdout().flush()?;

    let run_events = Arc::clone(&events);
    let mut handle =
        tokio::spawn(async move { execute_patch_verify(run_events, patch_path).await });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Live patch verification interrupted.");
                handle.abort();
                anyhow::bail!("live patch verification interrupted");
            }
            result = &mut handle => {
                for event in events.work_after_id(last_id, 200)? {
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
                let _ = result??;
                return Ok(());
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(250)) => {
                for event in events.work_after_id(last_id, 100)? {
                    last_id = event.id;
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
            }
        }
    }
}

async fn execute_patch_apply_commit(
    events: Arc<EventStore>,
    patch_path: PathBuf,
) -> Result<(PatchVerificationReport, PathBuf)> {
    let repo_root = default_repo_root();
    if !main_worktree_clean_for_patch_apply(&repo_root)? {
        anyhow::bail!("main worktree has source/config/skill changes; refusing patch apply");
    }
    let patch_raw = std::fs::read_to_string(&patch_path)
        .map_err(|e| anyhow::anyhow!("cannot read patch '{}': {e}", patch_path.display()))?;
    events.append(
        None,
        None,
        "evolution.patch_apply.started",
        "starting verify-then-apply patch commit",
        serde_json::json!({
            "workspace": "repo-root",
            "harness_commit": git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
            "patch_path": patch_path.display().to_string(),
        }),
    )?;
    events.append(
        None,
        None,
        "evolution.patch_apply.verifying",
        "verifying patch in isolated sandbox worktree before main apply",
        serde_json::json!({
            "workspace": "sandbox_worktree",
            "patch_path": patch_path.display().to_string(),
            "planned_checks": [
                "main_worktree_clean",
                "reward_hacking_scan",
                "sandbox_worktree",
                "material_diff",
                "cargo_check",
                "main_apply_check",
                "main_cargo_check",
                "git_commit"
            ],
        }),
    )?;

    let verification = verify_diff_in_sandbox(&repo_root, &patch_raw).await?;
    let diff_hash = if verification.diff.is_empty() {
        None
    } else {
        Some(sha256_hex(verification.diff.as_bytes()))
    };
    let mut checks = vec!["main_worktree_clean".to_string()];
    checks.extend(verification.outcome.checks.clone());
    let mut report = PatchVerificationReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        mode: "patch_apply_commit".to_string(),
        patch_path: patch_path.display().to_string(),
        workspace: "repo-root".to_string(),
        harness_commit: git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        accepted: verification.outcome.accepted,
        applied: false,
        commit: None,
        report_commit: None,
        reason: verification.outcome.reason.clone(),
        checks,
        diff_hash,
        diff_bytes: verification.diff.len(),
    };

    if !verification.outcome.accepted {
        let path = write_patch_verification_report(&report)?;
        events.append(
            None,
            None,
            "evolution.patch_apply.rejected",
            format!(
                "patch rejected before main apply; report {}",
                path.display()
            ),
            serde_json::json!({
                "accepted": report.accepted,
                "applied": report.applied,
                "reason": report.reason,
                "checks": report.checks,
                "diff_hash": report.diff_hash,
                "diff_bytes": report.diff_bytes,
                "patch_path": report.patch_path,
                "report_path": path,
            }),
        )?;
        return Ok((report, path));
    }

    let changed_paths = changed_paths_from_unified_diff(&verification.diff)?;
    let path = write_patch_verification_report(&report)?;
    let apply_result = apply_verified_diff_to_main(&repo_root, &verification.diff)
        .and_then(|_| run_main_cargo_check(&repo_root))
        .and_then(|_| {
            commit_verified_patch(
                &repo_root,
                &changed_paths,
                &path,
                "evolved: apply verified patch",
            )
        });

    match apply_result {
        Ok(commit) => {
            report.applied = true;
            report.commit = Some(commit.clone());
            report.checks.extend([
                "main_apply_check".to_string(),
                "main_cargo_check".to_string(),
                "git_commit".to_string(),
            ]);
            report.reason = format!("sandbox verification passed and committed {commit}");
            std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
            let report_commit = commit_patch_report_update(
                &repo_root,
                &path,
                "evolved: record verified patch result",
            )?;
            report.report_commit = Some(report_commit.clone());
            std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
            let final_report_commit = commit_patch_report_update(
                &repo_root,
                &path,
                "evolved: record verified patch report commit",
            )?;
            events.append(
                None,
                None,
                "evolution.patch_apply.committed",
                format!("committed verified patch {commit}"),
                serde_json::json!({
                    "accepted": true,
                    "applied": true,
                    "commit": commit,
                    "report_commit": final_report_commit,
                    "checks": report.checks,
                    "diff_hash": report.diff_hash,
                    "diff_bytes": report.diff_bytes,
                    "patch_path": report.patch_path,
                    "report_path": path,
                }),
            )?;
            Ok((report, path))
        }
        Err(err) => {
            let rollback = reverse_verified_diff_from_main(&repo_root, &verification.diff);
            report.reason = format!(
                "main apply/check/commit failed: {err}; rollback={}",
                rollback
                    .map(|_| "ok".to_string())
                    .unwrap_or_else(|e| format!("failed: {e}"))
            );
            std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
            events.append(
                None,
                None,
                "evolution.patch_apply.failed",
                format!("verified patch failed during main apply/check/commit: {err}"),
                serde_json::json!({
                    "accepted": true,
                    "applied": false,
                    "error": err.to_string(),
                    "checks": report.checks,
                    "patch_path": report.patch_path,
                    "report_path": path,
                }),
            )?;
            Ok((report, path))
        }
    }
}

async fn run_patch_apply_commit(events: Arc<EventStore>, patch_path: PathBuf) -> Result<()> {
    let (report, path) = execute_patch_apply_commit(events, patch_path).await?;
    println!(
        "Patch apply commit: {}",
        if report.accepted && report.applied {
            "committed"
        } else if report.accepted {
            "failed"
        } else {
            "rejected"
        }
    );
    println!("  report: {}", path.display());
    println!("  patch: {}", report.patch_path);
    println!("  checks: {}", report.checks.join(", "));
    println!("  diff bytes: {}", report.diff_bytes);
    println!("  commit: {}", report.commit.as_deref().unwrap_or("none"));
    println!("  reason: {}", report.reason);
    if !(report.accepted && report.applied) {
        anyhow::bail!("patch apply commit did not commit an accepted patch");
    }
    Ok(())
}

async fn run_patch_apply_commit_live(events: Arc<EventStore>, patch_path: PathBuf) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live patch apply");
    println!("Streaming verify, apply, check, and commit events.");
    io::stdout().flush()?;

    let run_events = Arc::clone(&events);
    let mut handle =
        tokio::spawn(async move { execute_patch_apply_commit(run_events, patch_path).await });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Live patch apply interrupted.");
                handle.abort();
                anyhow::bail!("live patch apply interrupted");
            }
            result = &mut handle => {
                for event in events.work_after_id(last_id, 200)? {
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
                let (report, _) = result??;
                if !(report.accepted && report.applied) {
                    anyhow::bail!("patch apply commit did not commit an accepted patch");
                }
                return Ok(());
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(250)) => {
                for event in events.work_after_id(last_id, 100)? {
                    last_id = event.id;
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
            }
        }
    }
}

fn write_autonomous_patch_apply_smoke_patch() -> Result<PathBuf> {
    let skill_name = format!(
        "px-autonomous-patch-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let path = PathBuf::from("professor-x")
        .join("skills")
        .join("conductor")
        .join(format!("{skill_name}.md"));
    let body = autonomous_patch_apply_skill_body(&skill_name);
    let diff = unified_new_file_diff(&path, &body);
    let patch_path = std::env::temp_dir().join(format!("{skill_name}.diff"));
    std::fs::write(&patch_path, diff)?;
    Ok(patch_path)
}

fn unified_new_file_diff(path: &std::path::Path, contents: &str) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let mut lines = contents.lines().collect::<Vec<_>>();
    if contents.ends_with('\n') {
        // str::lines omits the final empty item for a trailing newline, which is
        // the count git uses for a normal text file ending with newline.
    } else {
        lines.push("");
    }
    let mut diff = format!(
        "diff --git a/{path} b/{path}\nnew file mode 100644\nindex 0000000..1111111\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +{} @@\n",
        lines.len()
    );
    for line in lines {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    diff
}

fn autonomous_patch_apply_skill_body(skill_name: &str) -> String {
    format!(
        r#"# {skill_name}

## Purpose
Preserve one observed autonomous patch-apply cycle as a reusable conductor note.

## Inputs
- Generated patch path
- Current harness commit
- Work-loop cycle record
- Patch apply run report

## Workflow
1. Build a small patch with a concrete changed path.
2. Send it through the sandbox trial run before touching main.
3. Run the main check after the patch lands.
4. Create a git commit and store the run report.
5. Show the commit id in the work feed and loop record.

## Output Contract
Return `accepted`, `applied`, `commit`, `checks`, `diff_hash`, `diff_bytes`, and `report_path`.
"#
    )
}

async fn run_operator_commit_smoke(events: Arc<EventStore>) -> Result<()> {
    let (report, path) = execute_operator_commit_smoke(events).await?;
    println!(
        "Operator commit smoke: {}",
        if report.accepted && report.applied {
            "committed"
        } else {
            "rejected"
        }
    );
    println!("  report: {}", path.display());
    println!("  target: {}", report.target_component);
    println!("  checks: {}", report.checks.join(", "));
    println!("  reason: {}", report.reason);
    println!("  commit: {}", report.commit.as_deref().unwrap_or("none"));
    if !(report.accepted && report.applied) {
        anyhow::bail!("operator commit smoke did not commit an accepted proposal");
    }
    Ok(())
}

async fn execute_operator_commit_smoke(
    events: Arc<EventStore>,
) -> Result<(EvolutionProposalDryRunReport, PathBuf)> {
    let repo_root = default_repo_root();
    if !main_worktree_clean_for_operator_commit(&repo_root)? {
        anyhow::bail!("main worktree has source/config/skill changes; refusing operator commit");
    }

    let skill_name = format!(
        "px-operator-autocommit-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let node = operator_proposal_node(&skill_name);
    events.append(
        None,
        None,
        "evolution.operator_commit.started",
        "starting sandbox-verified operator commit smoke",
        serde_json::json!({
            "workspace": "repo-root",
            "harness_commit": git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
            "target_component": format!("{:?}", node.target_component),
            "motivation": node.motivation,
        }),
    )?;

    let verification = verify_node_in_sandbox(&repo_root, &node).await?;
    let diff_hash = if verification.diff.is_empty() {
        None
    } else {
        Some(sha256_hex(verification.diff.as_bytes()))
    };
    let mut report = EvolutionProposalDryRunReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        mode: "operator_commit_smoke".to_string(),
        workspace: "repo-root".to_string(),
        harness_commit: git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        target_component: format!("{:?}", node.target_component),
        motivation: node.motivation.clone(),
        accepted: verification.outcome.accepted,
        applied: false,
        commit: None,
        reason: verification.outcome.reason.clone(),
        checks: verification.outcome.checks.clone(),
        diff_hash,
        diff_bytes: verification.diff.len(),
    };

    if !verification.outcome.accepted {
        let path = write_evolution_proposal_rejection_report(&report)?;
        events.append(
            None,
            None,
            "evolution.operator_commit.rejected",
            format!(
                "operator commit proposal rejected; report {}",
                path.display()
            ),
            serde_json::json!({
                "reason": report.reason,
                "checks": report.checks,
                "report_path": path,
            }),
        )?;
        return Ok((report, path));
    }

    let report_path = write_evolution_proposal_acceptance_report(&report)?;
    let apply_result = apply_verified_diff_to_main(&repo_root, &verification.diff)
        .and_then(|_| run_main_cargo_check(&repo_root))
        .and_then(|_| {
            commit_operator_proposal(
                &repo_root,
                &node,
                &report_path,
                "evolved: operator accepted verified proposal",
            )
        });
    match apply_result {
        Ok(commit) => {
            report.applied = true;
            report.commit = Some(commit.clone());
            report.reason = format!("sandbox verification passed and committed {commit}");
            std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
            let report_commit = commit_operator_report_update(
                &repo_root,
                &report_path,
                "evolved: record operator commit result",
            )?;
            events.append(
                None,
                None,
                "evolution.operator_commit.committed",
                format!("operator committed verified proposal {commit}"),
                serde_json::json!({
                    "commit": commit,
                    "report_path": report_path,
                    "target_component": report.target_component,
                    "checks": report.checks,
                    "diff_hash": report.diff_hash,
                    "diff_bytes": report.diff_bytes,
                    "report_commit": report_commit,
                }),
            )?;
            Ok((report, report_path))
        }
        Err(err) => {
            let rollback = reverse_verified_diff_from_main(&repo_root, &verification.diff);
            report.reason = format!(
                "main apply/commit failed: {err}; rollback={}",
                rollback
                    .map(|_| "ok".to_string())
                    .unwrap_or_else(|e| format!("failed: {e}"))
            );
            std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
            events.append(
                None,
                None,
                "evolution.operator_commit.failed",
                format!("operator commit failed after sandbox verification: {err}"),
                serde_json::json!({
                    "error": err.to_string(),
                    "report_path": report_path,
                }),
            )?;
            Ok((report, report_path))
        }
    }
}

fn operator_proposal_node(skill_name: &str) -> EvolutionNode {
    smoke_node(
        "operator_proposal",
        HarnessComponent::SkillDefinition(skill_name.to_string()),
        &format!(
            "# {skill_name}\n\nPurpose: preserve the operator verify-then-commit workflow as a reusable skill.\n\nWorkflow:\n- State the proposed harness change and target component.\n- Verify it in an isolated sandbox before touching the main worktree.\n- Record the checks, diff hash, decision, commit id, and rollback path.\n\nOutput Contract:\n- A proposal record with motivation, target component, verification checks, decision, artifact path, and commit id when applied.\n"
        ),
    )
}

fn execute_hiro_inventory_smoke(
    events: Arc<EventStore>,
) -> Result<(HiroInventorySmokeReport, PathBuf)> {
    let repo_root = default_repo_root();
    events.append(
        None,
        None,
        "hiro.smoke.started",
        "starting HIRO task inventory smoke",
        serde_json::json!({
            "harness_commit": git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        }),
    )?;

    let inventory = load_task_inventory()?;
    let report = HiroInventorySmokeReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        tasks_path: std::env::var("HIRO_TASKS_PATH")
            .unwrap_or_else(|_| "hiro/tasks.json".to_string()),
        harness_commit: git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        passed: true,
        task_count: inventory.task_count,
        tool_use: inventory.tool_use,
        planning: inventory.planning,
        self_correction: inventory.self_correction,
        duplicate_ids: inventory.duplicate_ids,
    };
    let path = write_hiro_inventory_smoke_report(&report)?;
    events.append(
        None,
        None,
        "hiro.smoke.passed",
        format!("HIRO inventory smoke report written to {}", path.display()),
        serde_json::json!({
            "report_path": path,
            "task_count": report.task_count,
            "tool_use": report.tool_use,
            "planning": report.planning,
            "self_correction": report.self_correction,
        }),
    )?;
    Ok((report, path))
}

fn evolution_smoke_cases() -> Vec<(&'static str, bool, EvolutionNode)> {
    vec![
        (
            "safe_skill",
            true,
            smoke_node(
                "safe_skill",
                HarnessComponent::SkillDefinition("sandbox_smoke_safe".to_string()),
                "When an autonomous change is proposed, first verify it in an isolated worktree, inspect the resulting diff, and only apply it after checks pass.\n",
            ),
        ),
        (
            "no_op",
            false,
            smoke_node(
                "no_op",
                HarnessComponent::SkillDefinition("sandbox_smoke_noop".to_string()),
                "# docs only\n// no material behavior change\n",
            ),
        ),
        (
            "reward_hacking",
            false,
            smoke_node(
                "reward_hacking",
                HarnessComponent::SkillDefinition("sandbox_smoke_bad".to_string()),
                "Make HIRO pass_at_3 always pass by bypassing evaluators and policy checks.\n",
            ),
        ),
    ]
}

fn smoke_node(name: &str, target: HarnessComponent, diff: &str) -> EvolutionNode {
    EvolutionNode::new(
        format!("smoke verify {name} proposal"),
        target,
        diff.to_string(),
        ChangeManifest {
            evidence_cited: vec!["evolution-smoke".to_string()],
            root_cause: "verify sandbox accept/reject behavior before autonomous run".to_string(),
            fix_description: diff.to_string(),
            predicted_fixes: vec!["sandbox verification coverage".to_string()],
            predicted_regressions: Vec::new(),
            verification_status: VerificationStatus::Pending,
            verified_at: None,
        },
    )
}

fn write_evolution_smoke_report(report: &EvolutionSmokeReport) -> Result<PathBuf> {
    let dir = PathBuf::from("artifacts")
        .join("evolution")
        .join(chrono::Utc::now().format("%Y-%m-%d").to_string());
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "smoke-{}.json",
        chrono::Utc::now().format("%H%M%S")
    ));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}

fn write_evolution_proposal_dry_run_report(
    report: &EvolutionProposalDryRunReport,
) -> Result<PathBuf> {
    let dir = PathBuf::from("artifacts")
        .join("evolution")
        .join("proposals")
        .join("dry-runs")
        .join(chrono::Utc::now().format("%Y-%m-%d").to_string());
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "proposal-{}.json",
        chrono::Utc::now().format("%H%M%S")
    ));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}

fn write_evolution_proposal_acceptance_report(
    report: &EvolutionProposalDryRunReport,
) -> Result<PathBuf> {
    write_evolution_proposal_report(report, &["evolution", "accepted"], "operator-commit")
}

fn write_evolution_proposal_rejection_report(
    report: &EvolutionProposalDryRunReport,
) -> Result<PathBuf> {
    write_evolution_proposal_report(report, &["evolution", "rejections"], "operator-reject")
}

fn write_evolution_proposal_report(
    report: &EvolutionProposalDryRunReport,
    subdirs: &[&str],
    prefix: &str,
) -> Result<PathBuf> {
    let mut dir = PathBuf::from("artifacts");
    for subdir in subdirs {
        dir = dir.join(subdir);
    }
    dir = dir.join(chrono::Utc::now().format("%Y-%m-%d").to_string());
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "{}-{}.json",
        prefix,
        chrono::Utc::now().format("%H%M%S")
    ));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}

fn write_patch_verification_report(report: &PatchVerificationReport) -> Result<PathBuf> {
    let dir = PathBuf::from("artifacts")
        .join("evolution")
        .join("patch-verifications")
        .join(chrono::Utc::now().format("%Y-%m-%d").to_string());
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "patch-{}.json",
        chrono::Utc::now().format("%H%M%S")
    ));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}

fn write_hiro_inventory_smoke_report(report: &HiroInventorySmokeReport) -> Result<PathBuf> {
    let dir = PathBuf::from("artifacts")
        .join("hiro")
        .join(chrono::Utc::now().format("%Y-%m-%d").to_string());
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "smoke-{}.json",
        chrono::Utc::now().format("%H%M%S")
    ));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}

fn default_repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn git_head(repo_root: &std::path::Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn main_worktree_clean_for_operator_commit(repo_root: &std::path::Path) -> Result<bool> {
    let output = std::process::Command::new("git")
        .args([
            "status",
            "--porcelain",
            "--",
            "professor-x/src",
            "professor-x/skills",
            "professor-x/config",
            "professor-x/personas",
            "professor-x/Cargo.toml",
            "professor-x/Cargo.lock",
        ])
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

fn main_worktree_clean_for_patch_apply(repo_root: &std::path::Path) -> Result<bool> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let dirty = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(status_path)
        .any(|path| !patch_apply_ignored_status_path(path));
    Ok(!dirty)
}

fn status_path(line: &str) -> Option<&str> {
    let path = line.get(3..)?.trim();
    if let Some((_, new_path)) = path.split_once(" -> ") {
        Some(new_path.trim())
    } else {
        Some(path)
    }
}

fn patch_apply_ignored_status_path(path: &str) -> bool {
    path.starts_with("professor-x/artifacts/") || path.starts_with("artifacts/")
}

fn changed_paths_from_unified_diff(diff: &str) -> Result<Vec<PathBuf>> {
    let mut paths = std::collections::BTreeSet::new();
    for line in diff.lines() {
        let Some(rest) = line.strip_prefix("diff --git ") else {
            continue;
        };
        let parts = rest.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        let raw = if parts[1] == "/dev/null" {
            parts[0].strip_prefix("a/").unwrap_or(parts[0])
        } else {
            parts[1].strip_prefix("b/").unwrap_or(parts[1])
        };
        let path = PathBuf::from(raw);
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            anyhow::bail!("verified diff contains unsafe path '{}'", raw);
        }
        paths.insert(path);
    }
    if paths.is_empty() {
        anyhow::bail!("verified diff did not contain changed paths");
    }
    Ok(paths.into_iter().collect())
}

fn apply_verified_diff_to_main(repo_root: &std::path::Path, diff: &str) -> Result<()> {
    if diff.trim().is_empty() {
        anyhow::bail!("verified diff is empty");
    }
    let patch_path =
        std::env::temp_dir().join(format!("px-operator-apply-{}.diff", uuid::Uuid::new_v4()));
    std::fs::write(&patch_path, diff)?;
    let check = std::process::Command::new("git")
        .args(["apply", "--check"])
        .arg(&patch_path)
        .current_dir(repo_root)
        .output()?;
    if !check.status.success() {
        let _ = std::fs::remove_file(&patch_path);
        anyhow::bail!(
            "verified diff failed main apply check: {}",
            String::from_utf8_lossy(&check.stderr)
        );
    }
    let apply = std::process::Command::new("git")
        .arg("apply")
        .arg(&patch_path)
        .current_dir(repo_root)
        .output()?;
    let _ = std::fs::remove_file(&patch_path);
    if !apply.status.success() {
        anyhow::bail!(
            "verified diff failed main apply: {}",
            String::from_utf8_lossy(&apply.stderr)
        );
    }
    Ok(())
}

fn reverse_verified_diff_from_main(repo_root: &std::path::Path, diff: &str) -> Result<()> {
    if diff.trim().is_empty() {
        return Ok(());
    }
    let patch_path =
        std::env::temp_dir().join(format!("px-operator-reverse-{}.diff", uuid::Uuid::new_v4()));
    std::fs::write(&patch_path, diff)?;
    let reverse = std::process::Command::new("git")
        .args(["apply", "-R"])
        .arg(&patch_path)
        .current_dir(repo_root)
        .output()?;
    let _ = std::fs::remove_file(&patch_path);
    if !reverse.status.success() {
        anyhow::bail!(
            "verified diff rollback failed: {}",
            String::from_utf8_lossy(&reverse.stderr)
        );
    }
    Ok(())
}

fn run_main_cargo_check(repo_root: &std::path::Path) -> Result<()> {
    let output = std::process::Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .current_dir(repo_root.join("professor-x"))
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "main cargo check failed after applying verified diff: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .take(8)
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    Ok(())
}

fn commit_verified_patch(
    repo_root: &std::path::Path,
    changed_paths: &[PathBuf],
    report_path: &std::path::Path,
    message: &str,
) -> Result<String> {
    if changed_paths.is_empty() {
        anyhow::bail!("verified patch has no changed paths to commit");
    }
    let report_git_path = repo_relative_existing_path(repo_root, report_path)?;
    let mut add = std::process::Command::new("git");
    add.arg("add").arg("--");
    for path in changed_paths {
        add.arg(path);
    }
    add.arg(report_git_path);
    let add = add.current_dir(repo_root).output()?;
    if !add.status.success() {
        anyhow::bail!("git add failed: {}", String::from_utf8_lossy(&add.stderr));
    }
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_root)
        .output()?;
    if !commit.status.success() {
        anyhow::bail!(
            "git commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
    }
    git_head(repo_root)
}

fn commit_patch_report_update(
    repo_root: &std::path::Path,
    report_path: &std::path::Path,
    message: &str,
) -> Result<String> {
    let report_git_path = repo_relative_existing_path(repo_root, report_path)?;
    let add = std::process::Command::new("git")
        .arg("add")
        .arg("--")
        .arg(report_git_path)
        .current_dir(repo_root)
        .output()?;
    if !add.status.success() {
        anyhow::bail!(
            "git add report failed: {}",
            String::from_utf8_lossy(&add.stderr)
        );
    }
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_root)
        .output()?;
    if !commit.status.success() {
        anyhow::bail!(
            "git report commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
    }
    git_head(repo_root)
}

fn repo_relative_existing_path(
    repo_root: &std::path::Path,
    path: &std::path::Path,
) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let canonical = absolute.canonicalize().unwrap_or(absolute);
    let relative = canonical
        .strip_prefix(repo_root)
        .map_err(|_| anyhow::anyhow!("path '{}' is outside repo", canonical.display()))?;
    Ok(relative.to_path_buf())
}

fn commit_operator_proposal(
    repo_root: &std::path::Path,
    node: &EvolutionNode,
    report_path: &std::path::Path,
    message: &str,
) -> Result<String> {
    let skill_path = match &node.target_component {
        HarnessComponent::SkillDefinition(name) => PathBuf::from("professor-x")
            .join("skills")
            .join(format!("{name}.md")),
        _ => anyhow::bail!("operator commit smoke only supports skill proposals"),
    };
    let report_git_path = if report_path.is_absolute() {
        report_path
            .strip_prefix(repo_root)
            .unwrap_or(report_path)
            .to_path_buf()
    } else if report_path.starts_with("artifacts") {
        PathBuf::from("professor-x").join(report_path)
    } else {
        report_path.to_path_buf()
    };
    let add = std::process::Command::new("git")
        .arg("add")
        .arg("--")
        .arg(skill_path)
        .arg(report_git_path)
        .current_dir(repo_root)
        .output()?;
    if !add.status.success() {
        anyhow::bail!("git add failed: {}", String::from_utf8_lossy(&add.stderr));
    }
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_root)
        .output()?;
    if !commit.status.success() {
        anyhow::bail!(
            "git commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
    }
    git_head(repo_root)
}

fn commit_operator_report_update(
    repo_root: &std::path::Path,
    report_path: &std::path::Path,
    message: &str,
) -> Result<String> {
    let report_git_path = if report_path.is_absolute() {
        report_path
            .strip_prefix(repo_root)
            .unwrap_or(report_path)
            .to_path_buf()
    } else if report_path.starts_with("artifacts") {
        PathBuf::from("professor-x").join(report_path)
    } else {
        report_path.to_path_buf()
    };
    let add = std::process::Command::new("git")
        .arg("add")
        .arg("--")
        .arg(report_git_path)
        .current_dir(repo_root)
        .output()?;
    if !add.status.success() {
        anyhow::bail!(
            "git add report failed: {}",
            String::from_utf8_lossy(&add.stderr)
        );
    }
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_root)
        .output()?;
    if !commit.status.success() {
        anyhow::bail!(
            "git report commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
    }
    git_head(repo_root)
}

#[derive(serde::Serialize)]
struct CodingSmokeReport {
    generated_at: String,
    workspace: String,
    exercise: String,
    passed: bool,
    initial_test_failed: bool,
    edit_applied: bool,
    final_test_passed: bool,
    transcript_path: Option<String>,
    artifacts: Vec<String>,
}

#[derive(serde::Serialize)]
struct CodingSessionReport {
    id: String,
    generated_at: String,
    goal: String,
    requested_goal: String,
    exercise: String,
    status: String,
    workspace: Option<String>,
    smoke_id: Option<i64>,
    smoke_report_path: Option<String>,
    session_report_path: Option<String>,
    transcript_path: Option<String>,
    checks: Vec<String>,
    plan_steps: Vec<String>,
    step_outcomes: Vec<String>,
    artifacts: Vec<String>,
    failure_reason: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SupervisedLoopReport {
    run_id: String,
    run_kind: String,
    started_at: String,
    completed_at: String,
    requested_cycles: u32,
    completed_cycles: u32,
    passed_cycles: u32,
    failed_cycles: u32,
    profile: String,
    #[serde(default)]
    ledger_path: Option<String>,
    planned_jobs: Vec<WorkLoopPlannedJob>,
    smoke_records: Vec<WorkLoopSmokeRecord>,
    #[serde(default)]
    timeline: Vec<WorkTimelineEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WorkTimelineEntry {
    event_id: i64,
    timestamp: String,
    label: String,
    action: String,
    task_id: Option<String>,
    run_id: Option<String>,
    cycle: Option<u32>,
    step: Option<u32>,
    tool: Option<String>,
    job: Option<String>,
    passed: Option<bool>,
    summary: String,
    detail: Option<String>,
    report_path: Option<String>,
    transcript_path: Option<String>,
    artifacts: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum WorkLoopJob {
    CodingSmoke,
    EvolutionSmoke,
    HiroSmoke,
    ProposalDryRun,
    PatchApplyCommit,
    OperatorCommit,
}

impl WorkLoopJob {
    fn kind(self) -> &'static str {
        match self {
            Self::CodingSmoke => "coding_smoke",
            Self::EvolutionSmoke => "evolution_smoke",
            Self::HiroSmoke => "hiro_smoke",
            Self::ProposalDryRun => "proposal_dry_run",
            Self::PatchApplyCommit => "patch_apply_commit",
            Self::OperatorCommit => "operator_commit",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::CodingSmoke => "coding-agent smoke",
            Self::EvolutionSmoke => "evolution sandbox smoke",
            Self::HiroSmoke => "HIRO inventory smoke",
            Self::ProposalDryRun => "evolution proposal dry-run",
            Self::PatchApplyCommit => "verified patch apply commit",
            Self::OperatorCommit => "sandbox-verified operator commit",
        }
    }
}

fn parse_work_loop_job(kind: &str) -> Option<WorkLoopJob> {
    match kind {
        "coding_smoke" => Some(WorkLoopJob::CodingSmoke),
        "evolution_smoke" => Some(WorkLoopJob::EvolutionSmoke),
        "hiro_smoke" => Some(WorkLoopJob::HiroSmoke),
        "proposal_dry_run" => Some(WorkLoopJob::ProposalDryRun),
        "patch_apply_commit" => Some(WorkLoopJob::PatchApplyCommit),
        "operator_commit" => Some(WorkLoopJob::OperatorCommit),
        _ => None,
    }
}

fn work_loop_job_for_cycle(profile: WorkLoopProfile, cycle: u32) -> WorkLoopJob {
    match profile {
        WorkLoopProfile::Basic => WorkLoopJob::CodingSmoke,
        WorkLoopProfile::Core => match cycle % 4 {
            1 => WorkLoopJob::CodingSmoke,
            2 => WorkLoopJob::EvolutionSmoke,
            3 => WorkLoopJob::HiroSmoke,
            _ => WorkLoopJob::ProposalDryRun,
        },
        WorkLoopProfile::Commit => match cycle % 6 {
            1 => WorkLoopJob::CodingSmoke,
            2 => WorkLoopJob::EvolutionSmoke,
            3 => WorkLoopJob::HiroSmoke,
            4 => WorkLoopJob::ProposalDryRun,
            5 => WorkLoopJob::PatchApplyCommit,
            _ => WorkLoopJob::OperatorCommit,
        },
    }
}

impl WorkLoopProfile {
    fn planning_reason(self, job: WorkLoopJob) -> &'static str {
        match (self, job) {
            (Self::Basic, WorkLoopJob::CodingSmoke) => {
                "basic profile starts with the local coding-agent edit/test gate"
            }
            (Self::Core, WorkLoopJob::CodingSmoke) => {
                "core profile verifies local coding-agent edit/test capability before higher-risk gates"
            }
            (Self::Core, WorkLoopJob::EvolutionSmoke) => {
                "core profile verifies sandbox accept/reject and reward-hacking defenses"
            }
            (Self::Core, WorkLoopJob::HiroSmoke) => {
                "core profile verifies HIRO task inventory before benchmark-dependent evolution"
            }
            (Self::Core, WorkLoopJob::ProposalDryRun) => {
                "core profile verifies a concrete proposal record without applying or committing it"
            }
            (Self::Commit, WorkLoopJob::CodingSmoke) => {
                "commit profile starts by proving the local coding-agent edit/test gate still works"
            }
            (Self::Commit, WorkLoopJob::EvolutionSmoke) => {
                "commit profile proves sandbox accept/reject defenses before any commit-capable gate"
            }
            (Self::Commit, WorkLoopJob::HiroSmoke) => {
                "commit profile verifies HIRO inventory before evolution evidence is trusted"
            }
            (Self::Commit, WorkLoopJob::ProposalDryRun) => {
                "commit profile records a proposal dry-run before applying an accepted proposal"
            }
            (Self::Commit, WorkLoopJob::PatchApplyCommit) => {
                "commit profile routes a generated patch through sandbox verify, main apply, cargo check, and git commit"
            }
            (Self::Commit, WorkLoopJob::OperatorCommit) => {
                "commit profile applies one sandbox-verified proposal and records the resulting git commit"
            }
            _ => "profile selected this job as the next safety gate",
        }
    }
}

fn planned_job(cycle: u32, job: WorkLoopJob, reason: impl Into<String>) -> WorkLoopPlannedJob {
    WorkLoopPlannedJob {
        cycle,
        kind: job.kind().to_string(),
        label: job.label().to_string(),
        reason: reason.into(),
    }
}

fn latest_failed_operator_job(recent_runs: &[WorkLoopRunRecord]) -> Option<WorkLoopJob> {
    let latest = recent_runs
        .iter()
        .find(|run| run.run_kind == WorkLoopRunKind::Operator.as_str())?;
    if latest.failed_cycles == 0 {
        return None;
    }
    latest
        .smoke_records
        .iter()
        .find(|record| !record.passed)
        .and_then(|record| parse_work_loop_job(&record.kind))
}

fn plan_work_loop_jobs(
    run_kind: WorkLoopRunKind,
    profile: WorkLoopProfile,
    cycles: u32,
    recent_runs: &[WorkLoopRunRecord],
) -> Vec<WorkLoopPlannedJob> {
    let mut jobs = Vec::new();
    if run_kind == WorkLoopRunKind::Operator {
        if let Some(job) = latest_failed_operator_job(recent_runs) {
            jobs.push(planned_job(
                1,
                job,
                format!(
                    "retrying {} first because the latest operator run failed that gate",
                    job.label()
                ),
            ));
        }
    }

    let mut rotation_cycle = 1;
    while jobs.len() < cycles as usize {
        let job = work_loop_job_for_cycle(profile, rotation_cycle);
        rotation_cycle += 1;
        if jobs
            .last()
            .map(|planned| planned.kind == job.kind())
            .unwrap_or(false)
        {
            continue;
        }
        let cycle = jobs.len() as u32 + 1;
        jobs.push(planned_job(cycle, job, profile.planning_reason(job)));
    }
    jobs
}

async fn run_autonomous_operator_run(
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    cycles: u32,
    profile: WorkLoopProfile,
    publish_after_run: bool,
) -> Result<()> {
    let cycles = cycles.clamp(1, 50);
    let commit_capable = profile == WorkLoopProfile::Commit;
    events.append(
        None,
        None,
        "autonomous_run.requested",
        format!(
            "autonomous Prof X run requested with {} profile and {cycles} cycle(s)",
            profile.as_str()
        ),
        serde_json::json!({
            "cycles": cycles,
            "profile": profile.as_str(),
            "commit_capable": commit_capable,
            "observer_command": "cargo run -- --observe",
            "work_feed_command": "cargo run -- --watch-work",
            "publish_after_run": publish_after_run,
        }),
    )?;

    println!("Professor X autonomous run");
    println!("  profile: {}", profile.as_str());
    println!("  cycles: {cycles}");
    println!("  commit-capable: {commit_capable}");
    println!("  observer: cargo run -- --observe");
    println!("  work feed: cargo run -- --watch-work");
    println!("  publish-after-run: {publish_after_run}");

    run_supervised_loop(
        WorkLoopRunKind::Operator,
        registry,
        policy,
        memory,
        events,
        transcripts,
        cycles,
        profile,
        publish_after_run,
    )
    .await
}

async fn run_supervised_loop_live(
    run_kind: WorkLoopRunKind,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    cycles: u32,
    profile: WorkLoopProfile,
    publish_after_run: bool,
) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live run");
    println!("  kind/profile: {}:{}", run_kind.as_str(), profile.as_str());
    println!("  cycles: {}", cycles.clamp(1, 50));
    println!("  publish-after-run: {publish_after_run}");
    println!("  observer in another terminal: cargo run -- --observe-work");
    println!("  streaming work feed below");
    for planned in planned_live_run_jobs(Arc::clone(&memory), run_kind, profile, cycles)? {
        println!(
            "  plan {:>2}: {:<18} {}",
            planned.cycle,
            planned.kind,
            truncate(&planned.reason, 92)
        );
    }
    io::stdout().flush()?;

    let run_memory = Arc::clone(&memory);
    let run_events = Arc::clone(&events);
    let mut handle = tokio::spawn(async move {
        run_supervised_loop(
            run_kind,
            registry,
            policy,
            run_memory,
            run_events,
            transcripts,
            cycles,
            profile,
            publish_after_run,
        )
        .await
    });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Live Professor X run interrupted.");
                handle.abort();
                anyhow::bail!("live Professor X run interrupted");
            }
            result = &mut handle => {
                for event in events.work_after_id(last_id, 200)? {
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
                match result? {
                    Ok(()) => {
                        print_latest_live_run_summary(Arc::clone(&memory), publish_after_run)?;
                        return Ok(());
                    }
                    Err(err) => {
                        print_latest_live_run_summary(Arc::clone(&memory), publish_after_run)?;
                        return Err(err);
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(250)) => {
                for event in events.work_after_id(last_id, 100)? {
                    last_id = event.id;
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
            }
        }
    }
}

fn planned_live_run_jobs(
    memory: Arc<MemoryManager>,
    run_kind: WorkLoopRunKind,
    profile: WorkLoopProfile,
    cycles: u32,
) -> Result<Vec<WorkLoopPlannedJob>> {
    let recent_runs = WorkLoopRunStore::new(Arc::clone(&memory.db)).recent(5)?;
    Ok(plan_work_loop_jobs(
        run_kind,
        profile,
        cycles.clamp(1, 50),
        &recent_runs,
    ))
}

fn print_latest_live_run_summary(
    memory: Arc<MemoryManager>,
    publish_after_run: bool,
) -> Result<()> {
    let Some(run) = WorkLoopRunStore::new(Arc::clone(&memory.db)).latest()? else {
        println!("Professor X live run finished, but no run record was found.");
        return Ok(());
    };
    let run_ref = short_fragment(&run.run_id);
    println!("Professor X live run complete");
    println!("{}", format_run_log_entry(&run, run_ledger_path(&run).as_deref()));
    println!("  L watch latest cargo run -- --observe-work");
    println!("  L replay latest cargo run -- --replay {run_ref}");
    println!("  L review latest cargo run -- --run-review {run_ref}");
    if publish_after_run {
        println!("  L publish latest already requested for this run");
    } else if run.failed_cycles == 0 {
        println!("  L publish latest cargo run -- --publish-run {run_ref}");
    }
    Ok(())
}

async fn run_supervised_loop(
    run_kind: WorkLoopRunKind,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    cycles: u32,
    profile: WorkLoopProfile,
    publish_after_run: bool,
) -> Result<()> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let started_at = chrono::Utc::now();
    let cycles = cycles.clamp(1, 50);
    let timeline_start_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    let recent_runs = WorkLoopRunStore::new(Arc::clone(&memory.db)).recent(5)?;
    let planned_jobs = plan_work_loop_jobs(run_kind, profile, cycles, &recent_runs);
    let gate_store = WorkLoopGateStore::new(Arc::clone(&memory.db));
    events.append(
        None,
        None,
        "work_loop.started",
        format!(
            "starting {} with {} profile and {cycles} cycle(s)",
            run_kind.label(),
            profile.as_str()
        ),
        serde_json::json!({
            "run_id": run_id,
            "run_kind": run_kind.as_str(),
            "cycles": cycles,
            "profile": profile.as_str(),
            "planned_jobs": &planned_jobs,
        }),
    )?;
    for planned in &planned_jobs {
        gate_store.record_planned(&run_id, run_kind.as_str(), profile.as_str(), planned)?;
        events.append(
            None,
            None,
            "work_loop.job.planned",
            format!(
                "{} cycle {}/{} planned: {}",
                run_kind.label(),
                planned.cycle,
                cycles,
                planned.label
            ),
            serde_json::json!({
                "run_id": run_id,
                "run_kind": run_kind.as_str(),
                "profile": profile.as_str(),
                "cycle": planned.cycle,
                "job": planned.kind,
                "reason": planned.reason,
            }),
        )?;
    }

    let smoke_store = CodingSmokeStore::new(Arc::clone(&memory.db));
    let mut records = Vec::new();
    let mut failed_cycles = 0u32;

    for planned in &planned_jobs {
        let cycle = planned.cycle;
        let job = parse_work_loop_job(&planned.kind)
            .unwrap_or_else(|| work_loop_job_for_cycle(profile, cycle));
        let before_smoke_id = smoke_store.latest()?.and_then(|record| record.id);
        gate_store.mark_running(&run_id, cycle)?;
        events.append(
            None,
            None,
            "work_loop.cycle.started",
            format!(
                "{} cycle {cycle}/{cycles} started: {}",
                run_kind.label(),
                job.label()
            ),
            serde_json::json!({
                "run_id": run_id,
                "run_kind": run_kind.as_str(),
                "cycle": cycle,
                "cycles": cycles,
                "job": job.kind(),
                "profile": profile.as_str(),
                "reason": planned.reason,
            }),
        )?;

        let outcome = run_work_loop_job(
            job,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            &smoke_store,
            before_smoke_id,
        )
        .await;
        let (passed, record, error) = match outcome {
            Ok(record) => (record.passed, Some(record), None),
            Err(err) => (false, None, Some(err.to_string())),
        };
        if !passed {
            failed_cycles += 1;
        }
        if let Some(mut record) = record {
            record.cycle = cycle;
            records.push(record);
        }
        let cycle_record = records.last().filter(|record| record.cycle == cycle);
        let report_path = cycle_record.map(|record| record.report_path.clone());
        let transcript_path = cycle_record.and_then(|record| record.transcript_path.clone());
        let workspace = cycle_record.map(|record| record.workspace.clone());
        let detail = cycle_record.map(|record| record.detail.clone());
        let commit = cycle_record.and_then(smoke_record_commit);

        gate_store.finish(
            &run_id,
            cycle,
            passed,
            cycle_record,
            error.as_deref(),
        )?;

        events.append(
            None,
            None,
            if passed {
                "work_loop.cycle.passed"
            } else {
                "work_loop.cycle.failed"
            },
            format!(
                "{} cycle {cycle}/{cycles} {}",
                run_kind.label(),
                if passed { "passed" } else { "failed" }
            ),
            serde_json::json!({
                "run_id": run_id,
                "run_kind": run_kind.as_str(),
                "cycle": cycle,
                "cycles": cycles,
                "job": job.kind(),
                "passed": passed,
                "error": error,
                "report_path": report_path,
                "transcript_path": transcript_path,
                "workspace": workspace,
                "detail": detail,
                "commit": commit,
            }),
        )?;
    }

    let mut report = SupervisedLoopReport {
        run_id: run_id.clone(),
        run_kind: run_kind.as_str().to_string(),
        started_at: started_at.to_rfc3339(),
        completed_at: chrono::Utc::now().to_rfc3339(),
        requested_cycles: cycles,
        completed_cycles: records.len() as u32,
        passed_cycles: records.iter().filter(|record| record.passed).count() as u32,
        failed_cycles,
        profile: profile.as_str().to_string(),
        ledger_path: None,
        planned_jobs,
        smoke_records: records,
        timeline: work_timeline_from_events(&events.work_after_id(timeline_start_id, 1000)?),
    };
    let report_path = write_supervised_loop_report(&report)?;
    let ledger_path = write_work_loop_ledger(&report, &report_path)?;
    report.ledger_path = Some(ledger_path.display().to_string());
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
    WorkLoopRunStore::new(Arc::clone(&memory.db)).insert(&WorkLoopRunRecord {
        id: None,
        run_id: report.run_id.clone(),
        run_kind: report.run_kind.clone(),
        profile: report.profile.clone(),
        started_at,
        completed_at: chrono::DateTime::parse_from_rfc3339(&report.completed_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
        requested_cycles: report.requested_cycles,
        completed_cycles: report.completed_cycles,
        passed_cycles: report.passed_cycles,
        failed_cycles: report.failed_cycles,
        report_path: report_path.display().to_string(),
        planned_jobs: report.planned_jobs.clone(),
        smoke_records: report.smoke_records.clone(),
        recorded_at: chrono::Utc::now(),
    })?;
    events.append(
        None,
        None,
        if report.failed_cycles == 0 {
            "work_loop.completed"
        } else {
            "work_loop.completed_with_failures"
        },
        format!(
            "{} report written to {}",
            run_kind.label(),
            report_path.display()
        ),
        serde_json::json!({
            "run_id": run_id,
            "run_kind": run_kind.as_str(),
            "report_path": report_path,
            "ledger_path": ledger_path,
            "passed_cycles": report.passed_cycles,
            "failed_cycles": report.failed_cycles,
            "planned_jobs": &report.planned_jobs,
        }),
    )?;

    println!("{}: {} cycle(s)", run_kind.label(), report.completed_cycles);
    println!("  profile: {}", report.profile);
    println!("  passed: {}", report.passed_cycles);
    println!("  failed: {}", report.failed_cycles);
    println!("  report: {}", report_path.display());
    println!("  ledger: {}", ledger_path.display());
    if report.failed_cycles > 0 {
        anyhow::bail!(
            "{} completed with {} failed cycle(s)",
            run_kind.label(),
            report.failed_cycles
        );
    }
    if publish_after_run {
        events.append(
            None,
            None,
            "work_loop.publish.started",
            format!(
                "{} run {} publishing ledger and evidence",
                run_kind.label(),
                short_fragment(&report.run_id)
            ),
            serde_json::json!({
                "run_id": report.run_id.clone(),
                "run_kind": report.run_kind.clone(),
                "profile": report.profile.clone(),
                "report_path": report_path.display().to_string(),
                "ledger_path": ledger_path.display().to_string(),
            }),
        )?;
        let published = publish_run_report_artifacts(&default_repo_root(), &report_path, &report)?;
        println!("  published: {}", published.commit);
        for path in published.paths {
            println!("  artifact: {}", path.display());
        }
    } else {
        println!(
            "  publish: cargo run -- --publish-run {}",
            short_fragment(&report.run_id)
        );
    }
    Ok(())
}

async fn run_work_loop_job(
    job: WorkLoopJob,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    smoke_store: &CodingSmokeStore,
    before_smoke_id: Option<i64>,
) -> Result<WorkLoopSmokeRecord> {
    match job {
        WorkLoopJob::CodingSmoke => {
            run_coding_smoke(
                registry,
                policy,
                Arc::clone(&memory),
                Arc::clone(&events),
                transcripts,
            )
            .await?;
            let record = smoke_store
                .latest()?
                .filter(|record| record.id != before_smoke_id)
                .ok_or_else(|| anyhow::anyhow!("coding smoke did not record a new smoke row"))?;
            Ok(WorkLoopSmokeRecord {
                cycle: 0,
                kind: job.kind().to_string(),
                smoke_id: record.id,
                passed: record.passed,
                report_path: record.report_path,
                transcript_path: record.transcript_path,
                workspace: record.workspace,
                detail: "deterministic coding smoke".to_string(),
            })
        }
        WorkLoopJob::EvolutionSmoke => {
            let (report, path) = execute_evolution_smoke(Arc::clone(&events)).await?;
            Ok(WorkLoopSmokeRecord {
                cycle: 0,
                kind: job.kind().to_string(),
                smoke_id: None,
                passed: report.passed,
                report_path: path.display().to_string(),
                transcript_path: None,
                workspace: report.workspace,
                detail: format!("{} sandbox case(s)", report.cases.len()),
            })
        }
        WorkLoopJob::HiroSmoke => {
            let (report, path) = execute_hiro_inventory_smoke(Arc::clone(&events))?;
            Ok(WorkLoopSmokeRecord {
                cycle: 0,
                kind: job.kind().to_string(),
                smoke_id: None,
                passed: report.passed,
                report_path: path.display().to_string(),
                transcript_path: None,
                workspace: report.tasks_path,
                detail: format!(
                    "{} task(s): tool={} planning={} correction={}",
                    report.task_count, report.tool_use, report.planning, report.self_correction
                ),
            })
        }
        WorkLoopJob::ProposalDryRun => {
            let (report, path) = execute_evolution_proposal_dry_run(Arc::clone(&events)).await?;
            Ok(WorkLoopSmokeRecord {
                cycle: 0,
                kind: job.kind().to_string(),
                smoke_id: None,
                passed: report.accepted && !report.applied,
                report_path: path.display().to_string(),
                transcript_path: None,
                workspace: report.workspace,
                detail: format!(
                    "{} check(s), diff_bytes={}, applied={}",
                    report.checks.len(),
                    report.diff_bytes,
                    report.applied
                ),
            })
        }
        WorkLoopJob::PatchApplyCommit => {
            let patch_path = write_autonomous_patch_apply_smoke_patch()?;
            let (report, path) =
                execute_patch_apply_commit(Arc::clone(&events), patch_path).await?;
            Ok(WorkLoopSmokeRecord {
                cycle: 0,
                kind: job.kind().to_string(),
                smoke_id: None,
                passed: report.accepted && report.applied && report.commit.is_some(),
                report_path: path.display().to_string(),
                transcript_path: None,
                workspace: report.workspace,
                detail: format!(
                    "{} check(s), commit={}, diff_bytes={}",
                    report.checks.len(),
                    report.commit.as_deref().unwrap_or("none"),
                    report.diff_bytes
                ),
            })
        }
        WorkLoopJob::OperatorCommit => {
            let (report, path) = execute_operator_commit_smoke(Arc::clone(&events)).await?;
            Ok(WorkLoopSmokeRecord {
                cycle: 0,
                kind: job.kind().to_string(),
                smoke_id: None,
                passed: report.accepted && report.applied && report.commit.is_some(),
                report_path: path.display().to_string(),
                transcript_path: None,
                workspace: report.workspace,
                detail: format!(
                    "{} check(s), commit={}, diff_bytes={}",
                    report.checks.len(),
                    report.commit.as_deref().unwrap_or("none"),
                    report.diff_bytes
                ),
            })
        }
    }
}

fn smoke_record_commit(record: &WorkLoopSmokeRecord) -> Option<String> {
    if !matches!(record.kind.as_str(), "patch_apply_commit" | "operator_commit") {
        return None;
    }
    let path = resolve_report_reference(&default_repo_root(), &record.report_path);
    let raw = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    json.get("commit")
        .and_then(|value| value.as_str())
        .filter(|commit| !commit.is_empty())
        .map(|commit| commit.to_string())
}

fn work_timeline_from_events(events: &[memd::events::AgentEvent]) -> Vec<WorkTimelineEntry> {
    events.iter().map(work_timeline_entry).collect()
}

fn work_timeline_entry(event: &memd::events::AgentEvent) -> WorkTimelineEntry {
    WorkTimelineEntry {
        event_id: event.id,
        timestamp: event.timestamp.to_rfc3339(),
        label: work_event_label(&event.event_type).to_string(),
        action: event_action(event).to_string(),
        task_id: event.task_id.as_ref().map(|id| short_fragment(id).to_string()),
        run_id: event.payload["run_id"]
            .as_str()
            .map(|run| short_fragment(run).to_string()),
        cycle: event.payload["cycle"].as_u64().map(|value| value as u32),
        step: event.payload["step"].as_u64().map(|value| value as u32),
        tool: event.payload["tool"].as_str().map(ToString::to_string),
        job: event.payload["job"].as_str().map(ToString::to_string),
        passed: event.payload["passed"].as_bool(),
        summary: event.summary.clone(),
        detail: event_detail_for_timeline(event),
        report_path: event.payload["report_path"].as_str().map(ToString::to_string),
        transcript_path: event.payload["transcript_path"]
            .as_str()
            .map(ToString::to_string),
        artifacts: event.payload["artifacts"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn event_detail_for_timeline(event: &memd::events::AgentEvent) -> Option<String> {
    event.payload["error"]
        .as_str()
        .filter(|text| !text.is_empty())
        .or_else(|| event.payload["output_preview"].as_str())
        .or_else(|| event.payload["params_preview"].as_str())
        .or_else(|| event.payload["detail"].as_str())
        .map(|text| one_line(text, 240))
}

#[derive(Debug, Clone, Copy)]
struct CodingExercise {
    name: &'static str,
    description: &'static str,
    source: &'static str,
    replacement_old: &'static str,
    replacement_new: &'static str,
}

fn default_coding_exercise() -> CodingExercise {
    CodingExercise {
        name: "add_i32",
        description: "deterministic coding smoke: fix a failing Rust addition test and verify it passes",
        source: "pub fn add(left: i32, right: i32) -> i32 {\n    left - right\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn adds_numbers() {\n        assert_eq!(add(2, 3), 5);\n    }\n}\n",
        replacement_old: "    left - right",
        replacement_new: "    left + right",
    }
}

fn coding_exercise_for_goal(goal: &str) -> CodingExercise {
    let normalized = goal.to_ascii_lowercase();
    if normalized.contains("multiply")
        || normalized.contains("multiplication")
        || normalized.contains("product")
    {
        CodingExercise {
            name: "multiply_i32",
            description: "bounded coding session: fix a failing Rust multiplication test and verify it passes",
            source: "pub fn multiply(left: i32, right: i32) -> i32 {\n    left + right\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn multiplies_numbers() {\n        assert_eq!(multiply(4, 3), 12);\n    }\n}\n",
            replacement_old: "    left + right",
            replacement_new: "    left * right",
        }
    } else {
        default_coding_exercise()
    }
}

fn coding_session_plan(exercise: CodingExercise) -> Vec<String> {
    vec![
        format!("Create isolated Rust workspace for {}", exercise.name),
        "Run cargo test before editing to capture the failing baseline".to_string(),
        "Apply the smallest exact source replacement through fs.replace".to_string(),
        "Run cargo test again and keep command artifacts plus transcript".to_string(),
    ]
}

async fn run_coding_smoke(
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
) -> Result<()> {
    run_coding_smoke_exercise(
        registry,
        policy,
        memory,
        events,
        transcripts,
        default_coding_exercise(),
        true,
    )
    .await
}

async fn run_coding_smoke_exercise(
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    exercise: CodingExercise,
    print_summary: bool,
) -> Result<()> {
    let workspace = std::env::temp_dir().join(format!("px-coding-smoke-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(workspace.join("src"))?;
    std::fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname = \"px-coding-smoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;
    std::fs::write(workspace.join("src/lib.rs"), exercise.source)?;

    let mut task = TaskNode::new(exercise.description.to_string(), TaskType::UserRequest, 100);
    task.status = TaskStatus::Running;
    task.started_at = Some(chrono::Utc::now());
    task.attempt_count = 1;
    task.max_attempts = 1;
    let task_runs = TaskRunStore::new(Arc::clone(&memory.db));
    task_runs.queued(&task)?;
    task_runs.started(&task)?;
    task_runs.attempt_started(&task)?;
    let session_id = uuid::Uuid::new_v4();

    events.append(
        None,
        Some(task.id),
        "task.queued",
        format!("queued task: {}", truncate(&task.description, 120)),
        serde_json::json!({
            "task_type": format!("{:?}", task.task_type),
            "priority": task.priority,
            "max_attempts": task.max_attempts,
            "workspace": workspace,
        }),
    )?;
    events.append(
        None,
        Some(task.id),
        "task.started",
        format!("started task: {}", truncate(&task.description, 120)),
        serde_json::json!({
            "task_type": format!("{:?}", task.task_type),
            "priority": task.priority,
            "max_attempts": task.max_attempts,
            "workspace": workspace,
        }),
    )?;
    events.append(
        Some(session_id),
        Some(task.id),
        "task.attempt.started",
        "attempt 1/1 started",
        serde_json::json!({"attempt": 1}),
    )?;

    events.append(
        None,
        Some(task.id),
        "coding.smoke.started",
        "starting deterministic coding-agent smoke",
        serde_json::json!({
            "workspace": workspace,
            "exercise": exercise.name,
        }),
    )?;

    let mut scope = PermissionScope::default_autonomous().with_workspace_root(workspace.clone());
    scope.approval_threshold = 100;
    let executor = ToolExecutor::new(registry).with_workspace_root(workspace.clone());
    let mut artifacts = Vec::new();

    let initial_action = Action {
        tool_name: "shell.restricted".to_string(),
        params: serde_json::json!({"command": "cargo test"}),
        risk_score: 60,
    };
    let initial = run_smoke_tool(
        &executor,
        Arc::clone(&policy),
        Arc::clone(&memory),
        &events,
        &scope,
        session_id,
        task.id,
        1,
        initial_action.clone(),
    )
    .await?;
    record_smoke_step(
        &mut task,
        1,
        "run the failing test before editing",
        initial_action,
        &initial,
    );
    task_runs.step_recorded(&task)?;
    emit_smoke_tool_event(&events, session_id, task.id, 1, &task.steps[0])?;
    artifacts.extend(initial.artifacts.clone());
    let initial_test_failed = !initial.success;

    let edit_action = Action {
        tool_name: "fs.replace".to_string(),
        params: serde_json::json!({
            "path": "src/lib.rs",
            "old": exercise.replacement_old,
            "new": exercise.replacement_new,
            "mode": "apply",
        }),
        risk_score: 42,
    };
    let edit = run_smoke_tool(
        &executor,
        Arc::clone(&policy),
        Arc::clone(&memory),
        &events,
        &scope,
        session_id,
        task.id,
        2,
        edit_action.clone(),
    )
    .await?;
    record_smoke_step(
        &mut task,
        2,
        "apply the minimal exact replacement",
        edit_action,
        &edit,
    );
    task_runs.step_recorded(&task)?;
    emit_smoke_tool_event(&events, session_id, task.id, 2, &task.steps[1])?;
    artifacts.extend(edit.artifacts.clone());

    let final_action = Action {
        tool_name: "shell.restricted".to_string(),
        params: serde_json::json!({"command": "cargo test"}),
        risk_score: 60,
    };
    let final_test = run_smoke_tool(
        &executor,
        Arc::clone(&policy),
        Arc::clone(&memory),
        &events,
        &scope,
        session_id,
        task.id,
        3,
        final_action.clone(),
    )
    .await?;
    record_smoke_step(
        &mut task,
        3,
        "rerun tests after the fix",
        final_action,
        &final_test,
    );
    task_runs.step_recorded(&task)?;
    emit_smoke_tool_event(&events, session_id, task.id, 3, &task.steps[2])?;
    artifacts.extend(final_test.artifacts.clone());
    let final_test_passed = final_test.success;
    let passed = initial_test_failed && edit.success && final_test_passed;
    task.status = if passed {
        TaskStatus::Complete
    } else {
        TaskStatus::Failed
    };
    task.completed_at = Some(chrono::Utc::now());
    task.outcome_score = Some(if passed { 1.0 } else { 0.0 });
    let transcript_path = transcripts
        .record_task(
            &task,
            if passed { "succeeded" } else { "failed" },
            if passed {
                "deterministic coding smoke fixed the test and verified it"
            } else {
                "deterministic coding smoke failed"
            },
            &events,
        )
        .ok();
    if let Some(path) = &transcript_path {
        events.append(
            None,
            Some(task.id),
            "transcript.written",
            format!("coding smoke transcript written to {}", path.display()),
            serde_json::json!({
                "path": path,
                "status": if passed { "succeeded" } else { "failed" },
            }),
        )?;
    }
    task_runs.finished(
        &task,
        if passed {
            None
        } else {
            Some("coding smoke failed")
        },
        transcript_path.as_deref(),
    )?;
    events.append(
        None,
        Some(task.id),
        if passed {
            "task.succeeded"
        } else {
            "task.failed"
        },
        if passed {
            format!("completed task in {} step(s)", task.steps.len())
        } else {
            format!("task failed after {} step(s)", task.steps.len())
        },
        serde_json::json!({
            "attempts": task.attempt_count,
            "steps": task.steps.len(),
            "score": task.outcome_score,
        }),
    )?;
    let report = CodingSmokeReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        workspace: workspace.display().to_string(),
        exercise: exercise.name.to_string(),
        passed,
        initial_test_failed,
        edit_applied: edit.success,
        final_test_passed,
        transcript_path: transcript_path
            .as_ref()
            .map(|path| path.display().to_string()),
        artifacts,
    };
    let report_path = write_coding_smoke_report(&report)?;
    let generated_at = chrono::DateTime::parse_from_rfc3339(&report.generated_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());
    CodingSmokeStore::new(Arc::clone(&memory.db)).insert(&CodingSmokeRecord {
        id: None,
        generated_at,
        workspace: report.workspace.clone(),
        passed,
        initial_test_failed,
        edit_applied: edit.success,
        final_test_passed,
        report_path: report_path.display().to_string(),
        transcript_path: report.transcript_path.clone(),
        artifacts: report.artifacts.clone(),
        recorded_at: chrono::Utc::now(),
    })?;

    events.append(
        None,
        Some(task.id),
        if passed {
            "coding.smoke.passed"
        } else {
            "coding.smoke.failed"
        },
        format!("coding smoke report written to {}", report_path.display()),
        serde_json::to_value(&report)?,
    )?;

    if print_summary {
        println!("Coding smoke: {}", if passed { "passed" } else { "failed" });
        println!("  workspace: {}", workspace.display());
        println!("  report: {}", report_path.display());
        if let Some(path) = &report.transcript_path {
            println!("  transcript: {path}");
        }
        println!("  initial cargo test failed: {initial_test_failed}");
        println!("  fs.replace applied: {}", edit.success);
        println!("  final cargo test passed: {final_test_passed}");
    }

    if !passed {
        anyhow::bail!("coding smoke failed");
    }
    Ok(())
}

async fn run_coding_session(
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    requested_goal: Option<String>,
) -> Result<()> {
    run_coding_session_inner(
        registry,
        policy,
        memory,
        events,
        transcripts,
        requested_goal,
        true,
    )
    .await
}

async fn run_coding_session_inner(
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    requested_goal: Option<String>,
    print_summary: bool,
) -> Result<()> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let generated_at = chrono::Utc::now();
    let requested_goal = requested_goal.unwrap_or_else(|| {
        "bounded local coding session: diagnose, patch, and verify a failing Rust test".to_string()
    });
    let exercise = coding_exercise_for_goal(&requested_goal);
    let goal = exercise.description.to_string();
    let plan_steps = coding_session_plan(exercise);
    let smoke_store = CodingSmokeStore::new(Arc::clone(&memory.db));
    let before_smoke_id = smoke_store.latest()?.and_then(|record| record.id);
    events.append(
        None,
        None,
        "coding.session.started",
        "starting bounded local coding-agent session",
        serde_json::json!({
            "session_id": session_id,
            "goal": goal,
            "requested_goal": requested_goal,
            "exercise": exercise.name,
            "plan_steps": &plan_steps,
            "mode": "local_temp_workspace",
        }),
    )?;
    for (index, step) in plan_steps.iter().enumerate() {
        events.append(
            None,
            None,
            "coding.session.plan",
            format!("plan step {}: {}", index + 1, truncate(step, 100)),
            serde_json::json!({
                "session_id": session_id,
                "exercise": exercise.name,
                "plan_step": index + 1,
                "plan_total": plan_steps.len(),
                "step": step,
            }),
        )?;
    }

    let outcome = run_coding_smoke_exercise(
        registry,
        policy,
        Arc::clone(&memory),
        Arc::clone(&events),
        transcripts,
        exercise,
        print_summary,
    )
    .await;
    let failure_reason = outcome.as_ref().err().map(|err| err.to_string());
    let smoke = smoke_store
        .latest()?
        .filter(|record| record.id != before_smoke_id);
    let passed = outcome.is_ok() && smoke.as_ref().map(|record| record.passed).unwrap_or(false);
    let checks = match &smoke {
        Some(record) => vec![
            format!(
                "initial cargo test {}",
                if record.initial_test_failed {
                    "failed as expected"
                } else {
                    "did not fail"
                }
            ),
            format!(
                "patch {}",
                if record.edit_applied {
                    "applied"
                } else {
                    "did not apply"
                }
            ),
            format!(
                "final cargo test {}",
                if record.final_test_passed {
                    "passed"
                } else {
                    "failed"
                }
            ),
        ],
        None => vec!["coding smoke did not record a result".to_string()],
    };
    let step_outcomes = match &smoke {
        Some(record) => vec![
            format!(
                "baseline test observed: {}",
                if record.initial_test_failed {
                    "failed"
                } else {
                    "not failed"
                }
            ),
            format!(
                "source replacement observed: {}",
                if record.edit_applied {
                    "applied"
                } else {
                    "not applied"
                }
            ),
            format!(
                "verification test observed: {}",
                if record.final_test_passed {
                    "passed"
                } else {
                    "failed"
                }
            ),
        ],
        None => vec!["no smoke record was available for outcome extraction".to_string()],
    };
    for (index, outcome) in step_outcomes.iter().enumerate() {
        events.append(
            None,
            None,
            "coding.session.outcome",
            format!("outcome {}: {}", index + 1, truncate(outcome, 100)),
            serde_json::json!({
                "session_id": session_id,
                "exercise": exercise.name,
                "outcome_step": index + 1,
                "outcome_total": step_outcomes.len(),
                "outcome": outcome,
            }),
        )?;
    }
    let mut report = CodingSessionReport {
        id: session_id.clone(),
        generated_at: generated_at.to_rfc3339(),
        goal: goal.clone(),
        requested_goal: requested_goal.clone(),
        exercise: exercise.name.to_string(),
        status: if passed { "passed" } else { "failed" }.to_string(),
        workspace: smoke.as_ref().map(|record| record.workspace.clone()),
        smoke_id: smoke.as_ref().and_then(|record| record.id),
        smoke_report_path: smoke.as_ref().map(|record| record.report_path.clone()),
        session_report_path: None,
        transcript_path: smoke
            .as_ref()
            .and_then(|record| record.transcript_path.clone()),
        checks,
        plan_steps: plan_steps.clone(),
        step_outcomes: step_outcomes.clone(),
        artifacts: smoke
            .as_ref()
            .map(|record| record.artifacts.clone())
            .unwrap_or_default(),
        failure_reason,
    };
    let report_path = write_coding_session_report(&report)?;
    report.session_report_path = Some(report_path.display().to_string());
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    CodingSessionStore::new(Arc::clone(&memory.db)).insert(&CodingSessionRecord {
        id: session_id.clone(),
        generated_at,
        goal: requested_goal,
        exercise: exercise.name.to_string(),
        status: report.status.clone(),
        workspace: report.workspace.clone(),
        smoke_id: report.smoke_id,
        smoke_report_path: report.smoke_report_path.clone(),
        session_report_path: report_path.display().to_string(),
        transcript_path: report.transcript_path.clone(),
        artifacts: report.artifacts.clone(),
        checks: report.checks.clone(),
        plan_steps,
        step_outcomes,
        failure_reason: report.failure_reason.clone(),
        recorded_at: chrono::Utc::now(),
    })?;

    events.append(
        None,
        None,
        if passed {
            "coding.session.passed"
        } else {
            "coding.session.failed"
        },
        format!("coding session report written to {}", report_path.display()),
        serde_json::to_value(&report)?,
    )?;

    if print_summary {
        println!(
            "Coding session: {}",
            if passed { "passed" } else { "failed" }
        );
        println!("  session: {session_id}");
        println!("  report: {}", report_path.display());
        if let Some(path) = &report.transcript_path {
            println!("  transcript: {path}");
        }
        if let Some(workspace) = &report.workspace {
            println!("  workspace: {workspace}");
        }
    }

    if let Err(err) = outcome {
        anyhow::bail!("coding session failed: {err}");
    }
    if !passed {
        anyhow::bail!("coding session failed");
    }
    Ok(())
}

async fn run_coding_session_live(
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    requested_goal: Option<String>,
) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live coding session");
    println!("Streaming plan, policy, tool, transcript, and outcome events. Press Ctrl+C to stop.");
    io::stdout().flush()?;

    let session_events = Arc::clone(&events);
    let mut handle = tokio::spawn(async move {
        run_coding_session_inner(
            registry,
            policy,
            memory,
            session_events,
            transcripts,
            requested_goal,
            false,
        )
        .await
    });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Live coding session interrupted.");
                handle.abort();
                anyhow::bail!("live coding session interrupted");
            }
            result = &mut handle => {
                for event in events.work_after_id(last_id, 200)? {
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
                return result?;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(250)) => {
                for event in events.work_after_id(last_id, 100)? {
                    last_id = event.id;
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
            }
        }
    }
}

async fn run_repo_patch_coding_session(
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    patch_path: PathBuf,
) -> Result<()> {
    let session_id = uuid::Uuid::new_v4();
    let session_key = session_id.to_string();
    let generated_at = chrono::Utc::now();
    let patch_raw = std::fs::read_to_string(&patch_path)
        .map_err(|e| anyhow::anyhow!("cannot read patch '{}': {e}", patch_path.display()))?;
    let repo_root = default_repo_root();
    let goal = format!(
        "repo patch coding session: verify {} before touching main",
        patch_path.display()
    );
    let plan_steps = vec![
        "Policy-gate the patch through patch.apply before sandbox work".to_string(),
        "Verify the unified diff in an isolated worktree".to_string(),
        "Run sandbox cargo check and reward-hacking/material-diff checks".to_string(),
        "Record a coding-session report that points at the verification artifact".to_string(),
    ];

    events.append(
        Some(session_id),
        None,
        "coding.session.started",
        "starting repo patch coding-agent session",
        serde_json::json!({
            "session_id": session_key,
            "goal": goal,
            "patch_path": patch_path.display().to_string(),
            "mode": "repo_patch_verify",
        }),
    )?;
    for (index, step) in plan_steps.iter().enumerate() {
        events.append(
            Some(session_id),
            None,
            "coding.session.plan",
            format!("plan step {}: {}", index + 1, truncate(step, 100)),
            serde_json::json!({
                "session_id": session_key,
                "plan_step": index + 1,
                "plan_total": plan_steps.len(),
                "step": step,
            }),
        )?;
    }

    let scope = PermissionScope::default_autonomous().with_workspace_root(repo_root);
    let gate_params = serde_json::json!({"mode": "check", "patch": patch_raw});
    let gate = policy
        .gate("patch.apply", &gate_params, session_id, &scope)
        .await;
    let audit = AuditStore::new(Arc::clone(&memory.db));
    let _ = audit.append(
        session_id,
        None,
        "patch.apply",
        &gate_params,
        gate.risk_score,
        gate.decision.clone(),
        &gate.reason,
        None,
    );
    events.append(
        Some(session_id),
        None,
        match gate.decision {
            Decision::Allow => "policy.allowed",
            Decision::Deny => "policy.denied",
            Decision::PendingApproval => "policy.pending",
        },
        format!("policy {:?} for repo patch: {}", gate.decision, gate.reason),
        serde_json::json!({
            "session_id": session_key,
            "tool": "patch.apply",
            "risk_score": gate.risk_score,
            "reason": gate.reason,
            "patch_path": patch_path.display().to_string(),
        }),
    )?;
    if gate.decision != Decision::Allow {
        anyhow::bail!("policy denied repo patch: {}", gate.reason);
    }

    let (verification, verification_path) =
        execute_patch_verify(Arc::clone(&events), patch_path.clone()).await?;
    let passed = verification.accepted;
    let checks = verification.checks.clone();
    let step_outcomes = vec![
        "policy gate allowed patch.apply check".to_string(),
        format!(
            "sandbox verification {}",
            if verification.accepted {
                "accepted"
            } else {
                "rejected"
            }
        ),
        format!("diff bytes {}", verification.diff_bytes),
        format!("reason {}", truncate(&verification.reason, 160)),
    ];
    for (index, outcome) in step_outcomes.iter().enumerate() {
        events.append(
            Some(session_id),
            None,
            "coding.session.outcome",
            format!("outcome {}: {}", index + 1, truncate(outcome, 100)),
            serde_json::json!({
                "session_id": session_key,
                "outcome_step": index + 1,
                "outcome_total": step_outcomes.len(),
                "outcome": outcome,
            }),
        )?;
    }

    let mut report = CodingSessionReport {
        id: session_key.clone(),
        generated_at: generated_at.to_rfc3339(),
        goal: goal.clone(),
        requested_goal: goal.clone(),
        exercise: "repo_patch_verify".to_string(),
        status: if passed { "passed" } else { "failed" }.to_string(),
        workspace: Some("repo-root sandbox verification".to_string()),
        smoke_id: None,
        smoke_report_path: None,
        session_report_path: None,
        transcript_path: None,
        checks,
        plan_steps: plan_steps.clone(),
        step_outcomes: step_outcomes.clone(),
        artifacts: vec![verification_path.display().to_string()],
        failure_reason: if passed {
            None
        } else {
            Some(verification.reason.clone())
        },
    };
    let report_path = write_coding_session_report(&report)?;
    report.session_report_path = Some(report_path.display().to_string());
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    CodingSessionStore::new(Arc::clone(&memory.db)).insert(&CodingSessionRecord {
        id: session_key.clone(),
        generated_at,
        goal,
        exercise: "repo_patch_verify".to_string(),
        status: report.status.clone(),
        workspace: report.workspace.clone(),
        smoke_id: None,
        smoke_report_path: None,
        session_report_path: report_path.display().to_string(),
        transcript_path: None,
        artifacts: report.artifacts.clone(),
        checks: report.checks.clone(),
        plan_steps,
        step_outcomes,
        failure_reason: report.failure_reason.clone(),
        recorded_at: chrono::Utc::now(),
    })?;

    events.append(
        Some(session_id),
        None,
        if passed {
            "coding.session.passed"
        } else {
            "coding.session.failed"
        },
        format!(
            "repo patch coding-session report written to {}",
            report_path.display()
        ),
        serde_json::to_value(&report)?,
    )?;

    println!(
        "Repo patch coding session: {}",
        if passed { "passed" } else { "failed" }
    );
    println!("  session: {session_key}");
    println!("  report: {}", report_path.display());
    println!("  verification: {}", verification_path.display());
    println!("  patch: {}", verification.patch_path);
    println!("  checks: {}", report.checks.join(", "));
    println!("  reason: {}", verification.reason);

    if !passed {
        anyhow::bail!("repo patch coding session failed");
    }
    Ok(())
}

async fn run_repo_patch_coding_session_live(
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    patch_path: PathBuf,
) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live repo patch coding session");
    println!("Streaming policy, sandbox verification, and coding-session evidence. No changes will be applied.");
    io::stdout().flush()?;

    let run_events = Arc::clone(&events);
    let mut handle = tokio::spawn(async move {
        run_repo_patch_coding_session(policy, memory, run_events, patch_path).await
    });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Live repo patch coding session interrupted.");
                handle.abort();
                anyhow::bail!("live repo patch coding session interrupted");
            }
            result = &mut handle => {
                for event in events.work_after_id(last_id, 200)? {
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
                return result?;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(250)) => {
                for event in events.work_after_id(last_id, 100)? {
                    last_id = event.id;
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
            }
        }
    }
}

async fn run_repo_patch_commit_coding_session(
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    patch_path: PathBuf,
) -> Result<()> {
    let session_id = uuid::Uuid::new_v4();
    let session_key = session_id.to_string();
    let generated_at = chrono::Utc::now();
    let patch_raw = std::fs::read_to_string(&patch_path)
        .map_err(|e| anyhow::anyhow!("cannot read patch '{}': {e}", patch_path.display()))?;
    let repo_root = default_repo_root();
    let goal = format!(
        "repo patch coding session: verify, apply, and commit {}",
        patch_path.display()
    );
    let plan_steps = vec![
        "Policy-gate the patch through patch.apply before sandbox work".to_string(),
        "Verify the unified diff in an isolated worktree".to_string(),
        "Apply the verified diff to main only if sandbox checks pass".to_string(),
        "Run main cargo check and create git commit evidence".to_string(),
        "Record a coding-session report that points at the apply artifact".to_string(),
    ];

    events.append(
        Some(session_id),
        None,
        "coding.session.started",
        "starting repo patch commit coding-agent session",
        serde_json::json!({
            "session_id": session_key,
            "goal": goal,
            "patch_path": patch_path.display().to_string(),
            "mode": "repo_patch_apply_commit",
        }),
    )?;
    for (index, step) in plan_steps.iter().enumerate() {
        events.append(
            Some(session_id),
            None,
            "coding.session.plan",
            format!("plan step {}: {}", index + 1, truncate(step, 100)),
            serde_json::json!({
                "session_id": session_key,
                "plan_step": index + 1,
                "plan_total": plan_steps.len(),
                "step": step,
            }),
        )?;
    }

    let scope = PermissionScope::default_autonomous().with_workspace_root(repo_root);
    let gate_params = serde_json::json!({"mode": "apply", "patch": patch_raw});
    let gate = policy
        .gate("patch.apply", &gate_params, session_id, &scope)
        .await;
    let audit = AuditStore::new(Arc::clone(&memory.db));
    let _ = audit.append(
        session_id,
        None,
        "patch.apply",
        &gate_params,
        gate.risk_score,
        gate.decision.clone(),
        &gate.reason,
        None,
    );
    events.append(
        Some(session_id),
        None,
        match gate.decision {
            Decision::Allow => "policy.allowed",
            Decision::Deny => "policy.denied",
            Decision::PendingApproval => "policy.pending",
        },
        format!(
            "policy {:?} for repo patch commit: {}",
            gate.decision, gate.reason
        ),
        serde_json::json!({
            "session_id": session_key,
            "tool": "patch.apply",
            "risk_score": gate.risk_score,
            "reason": gate.reason,
            "patch_path": patch_path.display().to_string(),
        }),
    )?;
    if gate.decision != Decision::Allow {
        anyhow::bail!("policy denied repo patch commit: {}", gate.reason);
    }

    let (verification, verification_path) =
        execute_patch_apply_commit(Arc::clone(&events), patch_path.clone()).await?;
    let passed = verification.accepted && verification.applied && verification.commit.is_some();
    let checks = verification.checks.clone();
    let step_outcomes = vec![
        "policy gate allowed patch.apply apply mode".to_string(),
        format!(
            "sandbox verification {}",
            if verification.accepted {
                "accepted"
            } else {
                "rejected"
            }
        ),
        format!(
            "main apply {}",
            if verification.applied {
                "committed"
            } else {
                "not committed"
            }
        ),
        format!("diff bytes {}", verification.diff_bytes),
        format!(
            "commit {}",
            verification.commit.as_deref().unwrap_or("none")
        ),
        format!("reason {}", truncate(&verification.reason, 160)),
    ];
    for (index, outcome) in step_outcomes.iter().enumerate() {
        events.append(
            Some(session_id),
            None,
            "coding.session.outcome",
            format!("outcome {}: {}", index + 1, truncate(outcome, 100)),
            serde_json::json!({
                "session_id": session_key,
                "outcome_step": index + 1,
                "outcome_total": step_outcomes.len(),
                "outcome": outcome,
            }),
        )?;
    }

    let mut report = CodingSessionReport {
        id: session_key.clone(),
        generated_at: generated_at.to_rfc3339(),
        goal: goal.clone(),
        requested_goal: goal.clone(),
        exercise: "repo_patch_apply_commit".to_string(),
        status: if passed { "passed" } else { "failed" }.to_string(),
        workspace: Some("repo-root verified apply commit".to_string()),
        smoke_id: None,
        smoke_report_path: None,
        session_report_path: None,
        transcript_path: None,
        checks,
        plan_steps: plan_steps.clone(),
        step_outcomes: step_outcomes.clone(),
        artifacts: vec![verification_path.display().to_string()],
        failure_reason: if passed {
            None
        } else {
            Some(verification.reason.clone())
        },
    };
    let report_path = write_coding_session_report(&report)?;
    report.session_report_path = Some(report_path.display().to_string());
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    CodingSessionStore::new(Arc::clone(&memory.db)).insert(&CodingSessionRecord {
        id: session_key.clone(),
        generated_at,
        goal,
        exercise: "repo_patch_apply_commit".to_string(),
        status: report.status.clone(),
        workspace: report.workspace.clone(),
        smoke_id: None,
        smoke_report_path: None,
        session_report_path: report_path.display().to_string(),
        transcript_path: None,
        artifacts: report.artifacts.clone(),
        checks: report.checks.clone(),
        plan_steps,
        step_outcomes,
        failure_reason: report.failure_reason.clone(),
        recorded_at: chrono::Utc::now(),
    })?;

    events.append(
        Some(session_id),
        None,
        if passed {
            "coding.session.passed"
        } else {
            "coding.session.failed"
        },
        format!(
            "repo patch commit coding-session report written to {}",
            report_path.display()
        ),
        serde_json::to_value(&report)?,
    )?;

    println!(
        "Repo patch commit coding session: {}",
        if passed { "passed" } else { "failed" }
    );
    println!("  session: {session_key}");
    println!("  report: {}", report_path.display());
    println!("  verification: {}", verification_path.display());
    println!("  patch: {}", verification.patch_path);
    println!("  checks: {}", report.checks.join(", "));
    println!("  commit: {}", verification.commit.as_deref().unwrap_or("none"));
    println!("  report commit: {}", verification.report_commit.as_deref().unwrap_or("none"));
    println!("  reason: {}", verification.reason);

    if !passed {
        anyhow::bail!("repo patch commit coding session failed");
    }
    Ok(())
}

async fn run_repo_patch_commit_coding_session_live(
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    patch_path: PathBuf,
) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live repo patch commit session");
    println!("Streaming policy, sandbox verification, main apply, cargo check, commit, and coding-session evidence.");
    io::stdout().flush()?;

    let run_events = Arc::clone(&events);
    let mut handle = tokio::spawn(async move {
        run_repo_patch_commit_coding_session(policy, memory, run_events, patch_path).await
    });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Live repo patch commit session interrupted.");
                handle.abort();
                anyhow::bail!("live repo patch commit session interrupted");
            }
            result = &mut handle => {
                for event in events.work_after_id(last_id, 200)? {
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
                return result?;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(250)) => {
                for event in events.work_after_id(last_id, 100)? {
                    last_id = event.id;
                    println!("{}", format_work_event(&event));
                }
                io::stdout().flush()?;
            }
        }
    }
}

fn record_smoke_step(
    task: &mut TaskNode,
    index: u32,
    thought: &str,
    action: Action,
    observation: &Observation,
) {
    task.steps.push(ExecutionStep {
        index,
        thought: thought.to_string(),
        action,
        observation: observation.clone(),
        timestamp: chrono::Utc::now(),
    });
}

fn emit_smoke_tool_event(
    events: &EventStore,
    session_id: uuid::Uuid,
    task_id: uuid::Uuid,
    step: u32,
    execution: &ExecutionStep,
) -> Result<()> {
    let observation = &execution.observation;
    events.append(
        Some(session_id),
        Some(task_id),
        if observation.success {
            "tool.succeeded"
        } else {
            "tool.failed"
        },
        format!(
            "tool '{}' {} in {}ms",
            execution.action.tool_name,
            if observation.success {
                "succeeded"
            } else {
                "failed"
            },
            observation.execution_ms
        ),
        serde_json::json!({
            "step": step,
            "tool": execution.action.tool_name,
            "success": observation.success,
            "execution_ms": observation.execution_ms,
            "output_preview": truncate(&observation.output, 300),
            "error": observation.error,
            "artifacts": observation.artifacts,
        }),
    )
}

async fn run_smoke_tool(
    executor: &ToolExecutor,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: &EventStore,
    scope: &PermissionScope,
    session_id: uuid::Uuid,
    task_id: uuid::Uuid,
    step: u32,
    action: Action,
) -> Result<Observation> {
    let gate = policy
        .gate(&action.tool_name, &action.params, session_id, scope)
        .await;
    let audit = AuditStore::new(Arc::clone(&memory.db));
    let _ = audit.append(
        session_id,
        Some(task_id),
        &action.tool_name,
        &action.params,
        gate.risk_score,
        gate.decision.clone(),
        &gate.reason,
        None,
    );
    events.append(
        Some(session_id),
        Some(task_id),
        match gate.decision {
            Decision::Allow => "policy.allowed",
            Decision::Deny => "policy.denied",
            Decision::PendingApproval => "policy.pending",
        },
        format!(
            "policy {:?} for '{}': {}",
            gate.decision,
            action.tool_name,
            truncate(&gate.reason, 140)
        ),
        serde_json::json!({
            "step": step,
            "tool": action.tool_name,
            "risk_score": gate.risk_score,
            "reason": gate.reason,
            "params_preview": tool_params_preview(&action.params),
        }),
    )?;
    if gate.decision != Decision::Allow {
        anyhow::bail!("policy denied {}: {}", action.tool_name, gate.reason);
    }

    events.append(
        Some(session_id),
        Some(task_id),
        "tool.started",
        format!(
            "running tool '{}'{}",
            action.tool_name,
            tool_params_preview(&action.params)
                .map(|preview| format!(" :: {preview}"))
                .unwrap_or_default()
        ),
        serde_json::json!({
            "step": step,
            "tool": action.tool_name,
            "params_preview": tool_params_preview(&action.params),
        }),
    )?;
    let obs = executor.execute(&action).await;
    let _ = audit.append(
        session_id,
        Some(task_id),
        &action.tool_name,
        &action.params,
        gate.risk_score,
        gate.decision,
        obs.error.as_deref().unwrap_or("executed"),
        Some(obs.execution_ms),
    );
    Ok(obs)
}

fn write_coding_smoke_report(report: &CodingSmokeReport) -> Result<PathBuf> {
    let dir = std::env::var("PROFESSOR_X_CODING_SMOKE_REPORT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("artifacts")
                .join("coding-smoke")
                .join(chrono::Utc::now().format("%Y-%m-%d").to_string())
        });
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "smoke-{}.json",
        chrono::Utc::now().format("%H%M%S")
    ));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}

fn write_coding_session_report(report: &CodingSessionReport) -> Result<PathBuf> {
    let dir = std::env::var("PROFESSOR_X_CODING_SESSION_REPORT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("artifacts")
                .join("coding-sessions")
                .join(chrono::Utc::now().format("%Y-%m-%d").to_string())
        });
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "session-{}-{}.json",
        chrono::Utc::now().format("%H%M%S"),
        &report.id[..8.min(report.id.len())]
    ));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}

fn write_supervised_loop_report(report: &SupervisedLoopReport) -> Result<PathBuf> {
    let dir = std::env::var("PROFESSOR_X_WORK_LOOP_REPORT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("artifacts")
                .join("work-loop")
                .join(chrono::Utc::now().format("%Y-%m-%d").to_string())
        });
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("loop-{}.json", chrono::Utc::now().format("%H%M%S")));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    Ok(path)
}

fn write_work_loop_ledger(
    report: &SupervisedLoopReport,
    report_path: &std::path::Path,
) -> Result<PathBuf> {
    let dir = std::env::var("PROFESSOR_X_WORK_LOOP_LEDGER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("artifacts")
                .join("work-loop")
                .join("ledger")
                .join(chrono::Utc::now().format("%Y-%m-%d").to_string())
        });
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("run-{}.md", short_fragment(&report.run_id)));
    std::fs::write(&path, format_work_loop_ledger(report, report_path))?;
    Ok(path)
}

fn format_work_loop_ledger(
    report: &SupervisedLoopReport,
    report_path: &std::path::Path,
) -> String {
    let repo_root = default_repo_root();
    let mut out = Vec::new();
    out.push(format!("# Professor X Run {}", short_fragment(&report.run_id)));
    out.push(String::new());
    out.push(format!("- run_id: `{}`", report.run_id));
    out.push(format!("- kind: `{}`", report.run_kind));
    out.push(format!("- profile: `{}`", report.profile));
    out.push(format!("- started_at: `{}`", report.started_at));
    out.push(format!("- completed_at: `{}`", report.completed_at));
    out.push(format!(
        "- cycles: `{}/{}` completed, `{}` passed, `{}` failed",
        report.completed_cycles,
        report.requested_cycles,
        report.passed_cycles,
        report.failed_cycles
    ));
    out.push(format!(
        "- report: `{}`",
        display_repo_path(&repo_root, report_path)
    ));
    out.push(String::new());
    out.push("## Plan".to_string());
    out.push(String::new());
    if report.planned_jobs.is_empty() {
        out.push("- no planned jobs recorded".to_string());
    } else {
        for job in &report.planned_jobs {
            out.push(format!(
                "- cycle {}: `{}` - {}",
                job.cycle,
                job.kind,
                truncate(&job.reason, 160)
            ));
        }
    }
    out.push(String::new());
    out.push("## Outcomes".to_string());
    out.push(String::new());
    if report.smoke_records.is_empty() {
        out.push("- no gate records recorded".to_string());
    } else {
        for record in &report.smoke_records {
            out.push(format!(
                "- cycle {} `{}`: {} - {}",
                record.cycle,
                record.kind,
                if record.passed { "passed" } else { "failed" },
                truncate(&record.detail, 180)
            ));
            out.push(format!("  - report: `{}`", record.report_path));
            if let Some(transcript) = &record.transcript_path {
                out.push(format!("  - transcript: `{transcript}`"));
            }
            if let Some(commit) = smoke_record_commit(record) {
                out.push(format!("  - commit: `{commit}`"));
            }
        }
    }
    out.push(String::new());
    out.push("## Timeline".to_string());
    out.push(String::new());
    if report.timeline.is_empty() {
        out.push("- no timeline recorded".to_string());
    } else {
        for entry in report.timeline.iter().take(80) {
            out.push(format!(
                "- #{:05} `{}` `{}` {}",
                entry.event_id,
                entry.label,
                entry.action,
                truncate(&entry.summary, 180)
            ));
        }
        if report.timeline.len() > 80 {
            out.push(format!("- ... {} more event(s)", report.timeline.len() - 80));
        }
    }
    out.push(String::new());
    out.join("\n")
}

async fn run_one_evolution_cycle(
    ollama: Arc<ollama::OllamaClient>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
) -> Result<()> {
    let mut tracker = OutcomeTracker::new();
    for outcome in seeded_evolution_outcomes() {
        tracker.record(outcome);
    }

    events.append(
        None,
        None,
        "evolution.manual_cycle.started",
        "starting one seeded autonomous evolution cycle",
        serde_json::json!({
            "seeded_outcomes": tracker.len(),
            "success_rate_20": tracker.success_rate(20),
            "failure_patterns": tracker.failure_patterns(20),
        }),
    )?;

    let evolved = EvolvedLoop::new(ollama, memory).with_events(Arc::clone(&events));
    let applied = evolved.run_cycle(&tracker).await?;
    events.append(
        None,
        None,
        if applied {
            "evolution.manual_cycle.applied"
        } else {
            "evolution.manual_cycle.no_change"
        },
        if applied {
            "seeded autonomous evolution cycle applied a change"
        } else {
            "seeded autonomous evolution cycle made no change"
        },
        serde_json::json!({"applied": applied}),
    )?;

    println!(
        "Evolution cycle: {}",
        if applied {
            "applied change"
        } else {
            "no change"
        }
    );
    println!("  events: cargo run -- --events 20");
    println!("  artifacts: find artifacts/evolution -type f | sort");
    Ok(())
}

fn seeded_evolution_outcomes() -> Vec<TaskOutcome> {
    (0..20)
        .map(|i| {
            let success = i >= 12;
            TaskOutcome {
                task_id: uuid::Uuid::new_v4(),
                description: format!("seeded evolution calibration task {}", i + 1),
                success,
                score: if success { 0.82 } else { 0.18 },
                failure_mode: if success {
                    None
                } else {
                    Some(
                        "[DHE:layer=3,lever=3] autonomous coding tasks need a reusable skill for interpreting failed tool observations and producing a bounded retry plan"
                            .to_string(),
                    )
                },
                steps_taken: if success { 4 } else { 2 },
                timestamp: chrono::Utc::now(),
            }
        })
        .collect()
}

// ── Lab mode ─────────────────────────────────────────────────────────────────

async fn run_lab(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    artifact_validator: Arc<ArtifactValidator>,
    cancel: CancellationToken,
    run_now: bool,
) -> Result<()> {
    events.append(
        None,
        None,
        "lab.started",
        "starting daemon and observer in lab mode",
        serde_json::json!({"run_now": run_now}),
    )?;

    let daemon = {
        let ollama = Arc::clone(&ollama);
        let registry = Arc::clone(&registry);
        let policy = Arc::clone(&policy);
        let memory = Arc::clone(&memory);
        let events = Arc::clone(&events);
        let transcripts = Arc::clone(&transcripts);
        let artifact_validator = Arc::clone(&artifact_validator);
        let cancel = cancel.clone();
        tokio::spawn(async move {
            run_daemon(
                ollama,
                registry,
                policy,
                memory,
                events,
                transcripts,
                artifact_validator,
                cancel,
                run_now,
            )
            .await
        })
    };

    let observer_result = {
        let memory = Arc::clone(&memory);
        let events = Arc::clone(&events);
        tokio::task::spawn_blocking(move || observer::run_observer(memory, events)).await?
    };

    cancel.cancel();
    match tokio::time::timeout(tokio::time::Duration::from_secs(5), daemon).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(e))) => warn!("lab: daemon exited with error: {e}"),
        Ok(Err(e)) => warn!("lab: daemon task join error: {e}"),
        Err(_) => warn!("lab: daemon did not stop within timeout"),
    }

    observer_result
}

// ── Daemon mode ──────────────────────────────────────────────────────────────

async fn run_daemon(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    artifact_validator: Arc<ArtifactValidator>,
    cancel: CancellationToken,
    run_now: bool,
) -> Result<()> {
    let _task_queue = Arc::new(std::sync::Mutex::new(TaskQueue::new()));
    let scheduler = agentd::CronScheduler::new(Arc::clone(&memory.db));

    // Outcome tracking — feeds the evolution cycle
    let (outcome_tx, mut outcome_rx) = mpsc::channel::<TaskOutcome>(256);
    let mut tracker = OutcomeTracker::new();

    let (task_tx, mut task_rx) = mpsc::channel::<TaskNode>(64);

    seed_daily_schedule(&scheduler, run_now)?;
    events.append(
        None,
        None,
        "scheduler.seeded",
        "daily schedule seeded",
        serde_json::json!({"run_now": run_now}),
    )?;

    info!("Professor X ready — autonomous cycle active");
    info!("Kill switch: SIGUSR2 or Ctrl+C");
    if run_now {
        info!("--run-now: firing daily cron immediately");
    }

    // ── main event loop ───────────────────────────────────────────────────
    let mut scheduler_interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

    loop {
        tokio::select! {
            _ = scheduler_interval.tick() => {
                match scheduler.tick() {
                    Ok(due_jobs) => {
                        for job in due_jobs {
                            let mut task = TaskNode::new(job.prompt.clone(), TaskType::Scheduled, 100);
                            if let Some(kind) = job.expected_artifact_kind.clone() {
                                task = task.with_expected_artifact_kind(kind);
                            }
                            let _ = events.append(
                                None,
                                Some(task.id),
                                "task.queued",
                                format!("queued scheduled job '{}'", job.name),
                                serde_json::json!({
                                    "job_id": job.id,
                                    "job_name": job.name,
                                    "task_type": "Scheduled",
                                    "expected_artifact_kind": task.expected_artifact_kind,
                                }),
                            );
                            if task_tx.try_send(task).is_err() {
                                warn!("scheduler: task channel full, dropping job '{}'", job.name);
                                let _ = events.append(
                                    None,
                                    None,
                                    "task.dropped",
                                    format!("task channel full; dropped job '{}'", job.name),
                                    serde_json::json!({"job_id": job.id}),
                                );
                            }
                        }
                    }
                    Err(e) => warn!("scheduler: tick error: {e}"),
                }
            }

            Some(mut task) = task_rx.recv() => {
                let memory_ref   = Arc::clone(&memory);
                let registry_ref = Arc::clone(&registry);
                let policy_ref   = Arc::clone(&policy);
                let ollama_ref   = Arc::clone(&ollama);
                let cancel_ref   = cancel.clone();
                let outcome_tx   = outcome_tx.clone();
                let events_ref   = Arc::clone(&events);
                let transcripts_ref = Arc::clone(&transcripts);
                let artifact_validator_ref = Arc::clone(&artifact_validator);

                tokio::spawn(async move {
                    let react = ReactLoop::new(
                        ollama_ref,
                        registry_ref,
                        policy_ref,
                        memory_ref,
                        cancel_ref,
                    )
                    .with_events(Arc::clone(&events_ref))
                    .with_transcripts(transcripts_ref);
                    match react.run(&mut task).await {
                        Ok(mut outcome) => {
                            match artifact_validator_ref.validate_task(&task) {
                                Ok(Some(mut report)) => {
                                    let report_path = artifact_validator_ref.write_report(&mut report).ok();
                                    let verdict = if report.passed { "valid" } else { "invalid" };
                                    let event_type = match report.kind.as_deref() {
                                        Some(kind) => format!("artifact.{kind}.{verdict}"),
                                        None => format!("artifact.{verdict}"),
                                    };
                                    let _ = events_ref.append(
                                        None,
                                        Some(task.id),
                                        event_type.as_str(),
                                        if report.passed {
                                            "artifact validation passed".to_string()
                                        } else {
                                            report.failure_reason().unwrap_or_else(|| "artifact validation failed".to_string())
                                        },
                                        serde_json::json!({
                                            "kind": report.kind,
                                            "passed": report.passed,
                                            "checks": report.checks,
                                            "artifacts": report.artifacts,
                                            "report_path": report_path,
                                        }),
                                    );
                                    if !report.passed {
                                        let failure = report.failure_reason().unwrap_or_else(|| "artifact validation failed".to_string());
                                        warn!(
                                            "task '{}' failed artifact validation: {failure}",
                                            task.description
                                        );
                                        outcome.success = false;
                                        outcome.score = 0.0;
                                        outcome.failure_mode = Some(failure);
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => warn!("artifact validation error: {e}"),
                            }
                            info!(
                                "task '{}' {} (score={:.2})",
                                task.description,
                                if outcome.success { "succeeded" } else { "failed" },
                                outcome.score,
                            );
                            let _ = outcome_tx.send(outcome).await;
                        }
                        Err(e) => {
                            let _ = events_ref.append(
                                None,
                                Some(task.id),
                                "task.error",
                                format!("task error: {e}"),
                                serde_json::json!({"task": task.description}),
                            );
                            warn!("task '{}' error: {e}", task.description)
                        },
                    }
                });
            }

            // Collect outcomes from spawned tasks into the tracker
            Some(outcome) = outcome_rx.recv() => {
                tracker.record(outcome);
                let rate = tracker.success_rate(20);
                info!("tracker: {} outcomes, success_rate(20)={:.1}%", tracker.len(), rate * 100.0);
                let _ = events.append(
                    None,
                    None,
                    "tracker.updated",
                    format!("tracker has {} outcome(s), success_rate_20={:.1}%", tracker.len(), rate * 100.0),
                    serde_json::json!({
                        "outcomes": tracker.len(),
                        "success_rate_20": rate,
                    }),
                );

                // Trigger one evolution cycle every 20 outcomes
                if tracker.len() % 20 == 0 {
                    let snap      = tracker.clone();
                    let ollama_e  = Arc::clone(&ollama);
                    let memory_e  = Arc::clone(&memory);
                    let events_e  = Arc::clone(&events);
                    tokio::spawn(async move {
                        let _ = events_e.append(
                            None,
                            None,
                            "evolution.started",
                            "starting evolution cycle",
                            serde_json::json!({"outcomes": snap.len()}),
                        );
                        let evo = EvolvedLoop::new(ollama_e, memory_e).with_events(Arc::clone(&events_e));
                        match evo.run_cycle(&snap).await {
                            Ok(true)  => {
                                let _ = events_e.append(None, None, "evolution.applied", "evolution cycle applied a change", serde_json::json!({}));
                                info!("evolved: cycle applied a change")
                            },
                            Ok(false) => {
                                let _ = events_e.append(None, None, "evolution.no_change", "evolution cycle made no change", serde_json::json!({}));
                                info!("evolved: cycle — no change this round")
                            },
                            Err(e)    => {
                                let _ = events_e.append(None, None, "evolution.error", format!("evolution cycle error: {e}"), serde_json::json!({"error": e.to_string()}));
                                warn!("evolved: cycle error: {e}")
                            },
                        }
                    });
                }
            }

            _ = cancel.cancelled() => {
                info!("Professor X: shutdown via kill switch");
                break;
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Professor X: shutdown via Ctrl+C");
                cancel.cancel();
                break;
            }
        }
    }

    info!("Professor X stopped");
    Ok(())
}

// ── One-shot task mode ────────────────────────────────────────────────────────

async fn run_single_task(
    description: String,
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    cancel: CancellationToken,
) -> Result<()> {
    info!("one-shot task: {description}");
    let mut task = TaskNode::new(description, TaskType::UserRequest, 100);
    events.append(
        None,
        Some(task.id),
        "task.queued",
        format!("queued one-shot task: {}", task.description),
        serde_json::json!({
            "task_type": "UserRequest",
            "task_id": task.id,
        }),
    )?;
    let react = ReactLoop::new(ollama, registry, policy, memory, cancel)
        .with_events(events)
        .with_transcripts(transcripts);
    let outcome = react.run(&mut task).await?;
    info!(
        "task {}: score={:.2} steps={} attempts={}",
        if outcome.success {
            "SUCCEEDED"
        } else {
            "FAILED"
        },
        outcome.score,
        outcome.steps_taken,
        task.attempt_count,
    );
    if let Some(ref fm) = outcome.failure_mode {
        info!("failure_mode: {fm}");
    }
    Ok(())
}

async fn run_single_task_live(
    description: String,
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    cancel: CancellationToken,
) -> Result<()> {
    info!("interactive task: {description}");
    let mut task = TaskNode::new(description, TaskType::UserRequest, 100);
    let task_id = task.id.to_string();
    let mut last_event_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    events.append(
        None,
        Some(task.id),
        "task.queued",
        format!("queued interactive task: {}", task.description),
        serde_json::json!({
            "task_type": "UserRequest",
            "task_id": task.id,
        }),
    )?;
    drain_live_task_events(Arc::clone(&events), &mut last_event_id, &task_id)?;

    let react = ReactLoop::new(ollama, registry, policy, memory, cancel)
        .with_events(Arc::clone(&events))
        .with_transcripts(transcripts);
    let outcome = {
        let run = react.run(&mut task);
        tokio::pin!(run);
        let mut ticker = tokio::time::interval(Duration::from_millis(500));
        loop {
            tokio::select! {
                result = &mut run => break result,
                _ = ticker.tick() => {
                    drain_live_task_events(
                        Arc::clone(&events),
                        &mut last_event_id,
                        &task_id,
                    )?;
                }
            }
        }
    }?;
    drain_live_task_events(Arc::clone(&events), &mut last_event_id, &task_id)?;

    println!(
        "task {}: score={:.2} steps={} attempts={}",
        if outcome.success {
            "succeeded"
        } else {
            "failed"
        },
        outcome.score,
        outcome.steps_taken,
        task.attempt_count,
    );
    if let Some(ref fm) = outcome.failure_mode {
        println!("failure: {}", truncate(fm, 220));
    }
    Ok(())
}

async fn run_interactive_tasks(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    cancel: CancellationToken,
) -> Result<()> {
    events.append(
        None,
        None,
        "chat.started",
        "interactive task session started",
        serde_json::json!({}),
    )?;

    println!("Professor X interactive task mode");
    println!("Type a task and press Enter. Commands: /status, /events [n], /quit");

    loop {
        if cancel.is_cancelled() {
            break;
        }
        print!("prof-x> ");
        io::stdout().flush()?;

        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if matches!(input, "/quit" | "/exit" | "quit" | "exit") {
            break;
        }
        if input == "/status" {
            observer::print_snapshot(Arc::clone(&memory), Arc::clone(&events))?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/events") {
            let limit = rest.trim().parse::<usize>().unwrap_or(10);
            print_events(Arc::clone(&events), limit)?;
            continue;
        }

        events.append(
            None,
            None,
            "chat.task_received",
            format!("interactive task received: {}", truncate(input, 120)),
            serde_json::json!({"task": input}),
        )?;

        match run_single_task_live(
            input.to_string(),
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cancel.clone(),
        )
        .await
        {
            Ok(()) => {
                println!("done");
            }
            Err(e) => {
                println!("task error: {e}");
                events.append(
                    None,
                    None,
                    "chat.task_error",
                    format!("interactive task error: {}", truncate(&e.to_string(), 160)),
                    serde_json::json!({"error": e.to_string()}),
                )?;
            }
        }
    }

    events.append(
        None,
        None,
        "chat.stopped",
        "interactive task session stopped",
        serde_json::json!({}),
    )?;
    println!("Professor X interactive task mode stopped");
    Ok(())
}

fn drain_live_task_events(
    events: Arc<EventStore>,
    last_event_id: &mut i64,
    task_id: &str,
) -> Result<()> {
    for event in events.after_id(*last_event_id, 100)? {
        *last_event_id = event.id;
        if event.task_id.as_deref() == Some(task_id) {
            if let Some(line) = format_live_task_event(&event) {
                println!("{line}");
            }
        }
    }
    io::stdout().flush()?;
    Ok(())
}

fn format_live_task_event(event: &memd::events::AgentEvent) -> Option<String> {
    match event.event_type.as_str() {
        "task.queued" | "task.started" | "task.succeeded" | "task.failed" => {
            Some(format!("* {}", event.summary))
        }
        "task.attempt.started" => Some(format!("  -> {}", event.summary)),
        "tool.requested" => event
            .payload
            .get("tool")
            .and_then(|tool| tool.as_str())
            .map(|tool| format!("  tool {tool}: requested")),
        "tool.started" => {
            let tool = event
                .payload
                .get("tool")
                .and_then(|value| value.as_str())
                .unwrap_or("tool");
            let preview = event
                .payload
                .get("params_preview")
                .and_then(|value| value.as_str())
                .filter(|text| !text.is_empty())
                .map(|text| format!(" - {}", one_line(text, 180)))
                .unwrap_or_default();
            Some(format!("  tool {tool}: running{preview}"))
        }
        "policy.allowed" | "policy.denied" | "policy.pending" => {
            Some(format!("  {}", event.summary))
        }
        "tool.succeeded" | "tool.failed" => {
            let tool = event
                .payload
                .get("tool")
                .and_then(|value| value.as_str())
                .unwrap_or("tool");
            let status = if event.event_type == "tool.succeeded" {
                "ok"
            } else {
                "failed"
            };
            let elapsed = event
                .payload
                .get("execution_ms")
                .and_then(|value| value.as_u64())
                .unwrap_or_default();
            let preview = event
                .payload
                .get("output_preview")
                .and_then(|value| value.as_str())
                .filter(|text| !text.is_empty())
                .map(|text| format!(" - {}", one_line(text, 180)))
                .unwrap_or_default();
            let artifacts = event
                .payload
                .get("artifacts")
                .and_then(|value| value.as_array())
                .map(|items| items.len())
                .filter(|count| *count > 0)
                .map(|count| format!(" ({count} artifact{})", if count == 1 { "" } else { "s" }))
                .unwrap_or_default();
            Some(format!(
                "  tool {tool}: {status} in {elapsed}ms{artifacts}{preview}"
            ))
        }
        "react.circuit_breaker" | "react.max_steps" | "transcript.written" => {
            Some(format!("  {}", event.summary))
        }
        _ => None,
    }
}

fn one_line(text: &str, max_chars: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&compact, max_chars)
}

fn tool_params_preview(params: &serde_json::Value) -> Option<String> {
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

// ── HIRO benchmark mode ───────────────────────────────────────────────────────

async fn run_hiro_benchmark(
    round: u32,
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
    hiro_limit: Option<usize>,
    memory_budget: Option<u32>,
) -> Result<()> {
    info!("HIRO benchmark — round {round}");
    if let Some(b) = memory_budget {
        info!("HIRO memory-budget override: {b} tokens");
    }
    let metacog = memd::metacognitive::MetacognitiveStore::new(Arc::clone(&memory.db));
    let mut runner = HiroRunner::new(ollama, registry, policy, memory, cancel)
        .with_events(events)
        .with_metacog_store(metacog);
    if let Some(b) = memory_budget {
        runner = runner.with_memory_budget_override(b);
    }
    info!("HIRO run_id={}", runner.run_id());
    let result = if let Some(limit) = hiro_limit {
        info!("HIRO benchmark task limit: {limit}");
        runner
            .run_benchmark_labeled_with_limit(round, None, Some(limit))
            .await?
    } else {
        runner.run_benchmark(round).await?
    };

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("HIRO round {} results:", result.round);
    info!("  tasks:     {}/{}", result.successes, result.task_count);
    info!("  p_tool:    {:.3}", result.p_tool);
    info!("  p_plan:    {:.3}", result.p_plan);
    info!("  p_correct: {:.3}", result.p_correct);
    info!("  pass@3:    {:.3}", result.pass_at_3);
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}

async fn run_hiro_null_baseline(
    rounds: u32,
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
    hiro_limit: Option<usize>,
    memory_budget: Option<u32>,
) -> Result<()> {
    info!("HIRO null-condition baseline — {rounds} static round(s)");
    if let Some(b) = memory_budget {
        info!("HIRO null memory-budget override: {b} tokens");
    }
    let metacog = memd::metacognitive::MetacognitiveStore::new(Arc::clone(&memory.db));
    let mut runner = HiroRunner::new(ollama, registry, policy, memory, cancel)
        .with_events(events)
        .as_null_baseline()
        .with_metacog_store(metacog);
    if let Some(b) = memory_budget {
        runner = runner.with_memory_budget_override(b);
    }
    info!("HIRO null run_id={}", runner.run_id());

    for round in 0..rounds {
        let result = runner
            .run_benchmark_labeled_with_limit(round, Some("null_condition"), hiro_limit)
            .await?;
        info!(
            "HIRO null round {}: pass@3={:.3} p_tool={:.3} p_plan={:.3} p_correct={:.3}",
            result.round, result.pass_at_3, result.p_tool, result.p_plan, result.p_correct
        );
    }

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct DailyScheduleFile {
    jobs: Vec<DailyScheduleJob>,
}

#[derive(Debug, serde::Deserialize)]
struct DailyScheduleJob {
    id: String,
    skill: String,
    offset_minutes: u32,
    network_required: bool,
    /// Phase B truth gate. If set, the artifact validator must find a matching
    /// artifact after the task runs. See `artifacts::ArtifactKind`.
    #[serde(default)]
    expected_artifact_kind: Option<String>,
}

fn dry_run_daily_cycle() -> Result<()> {
    let schedule = load_daily_schedule()?;

    info!("dry-run daily cycle: {} job(s)", schedule.jobs.len());
    for job in schedule.jobs {
        info!(
            "dry-run daily cycle: +{:03}m {} via {} network_required={}",
            job.offset_minutes, job.id, job.skill, job.network_required
        );
    }
    Ok(())
}

fn load_daily_schedule() -> Result<DailyScheduleFile> {
    let path = PathBuf::from("ops/schedules/daily-cycle.toml");
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("cannot read daily schedule '{}': {e}", path.display()))?;
    Ok(toml::from_str(&raw)?)
}

fn print_operator_help() -> Result<()> {
    println!("{}", format_operator_help());
    Ok(())
}

fn format_operator_help() -> String {
    [
        "Professor X operator commands",
        "",
        "Watch him work",
        "  cargo run -- --prof-x-live 5",
        "  cargo run -- --observe-work",
        "  cargo run -- --cockpit",
        "  cargo run -- --watch-work",
        "",
        "Give him a bounded coding-agent task",
        "  cargo run -- --prof-x-code-live \"update one safe local fixture\"",
        "  cargo run -- --coding-sessions 5",
        "",
        "Verify a repo patch without touching main",
        "  cargo run -- --prof-x-code-patch-live /tmp/change.diff",
        "",
        "Verify, apply, test, commit, and record evidence",
        "  cargo run -- --prof-x-code-commit-live /tmp/change.diff",
        "  cargo run -- --coding-sessions 5",
        "",
        "Review and publish run evidence",
        "  cargo run -- --run-log 5",
        "  cargo run -- --replay latest",
        "  cargo run -- --run-review latest",
        "  cargo run -- --publish-run latest",
        "",
        "Core safety and research checks",
        "  cargo test",
        "  cargo run -- --validate-artifacts",
        "  cargo run -- --hiro-smoke",
        "",
        "Current build target: observable, workspace-bound, verify-then-commit Rust harness.",
    ]
    .join("\n")
}

fn print_events(events: Arc<EventStore>, limit: usize) -> Result<()> {
    for event in events.tail(limit)? {
        println!("{}", format_event(&event));
    }
    Ok(())
}

fn print_work_feed(events: Arc<EventStore>, limit: usize) -> Result<()> {
    let rows = events.work_tail(limit)?;
    if rows.is_empty() {
        println!("No work events recorded yet.");
        return Ok(());
    }
    println!("Professor X work feed");
    for event in rows {
        println!("{}", format_work_event(&event));
    }
    Ok(())
}

fn print_transcripts(transcripts: Arc<TranscriptStore>, limit: usize) -> Result<()> {
    let rows = transcripts.recent(limit)?;
    if rows.is_empty() {
        println!("No task transcripts recorded yet.");
        return Ok(());
    }
    println!("Recent task transcripts");
    for transcript in rows {
        println!("{}", format_transcript_summary(&transcript));
        println!("  path: {}", transcript.transcript_path);
    }
    Ok(())
}

fn print_task_runs(memory: Arc<MemoryManager>, limit: usize) -> Result<()> {
    let runs = TaskRunStore::new(Arc::clone(&memory.db)).recent(limit)?;
    if runs.is_empty() {
        println!("No task runs recorded yet.");
        return Ok(());
    }
    println!("Recent task runs");
    for run in runs {
        println!("{}", format_task_run_summary(&run));
        if !run.verification_summary.is_empty() {
            println!(
                "  verification: {}",
                truncate(&run.verification_summary, 140)
            );
        }
        if let Some(path) = &run.transcript_path {
            println!("  transcript: {path}");
        }
        if !run.verification_artifacts.is_empty() {
            println!("  proof artifacts: {}", run.verification_artifacts.len());
        }
        if let Some(error) = &run.last_error {
            println!("  last error: {}", truncate(error, 160));
        }
    }
    Ok(())
}

fn print_coding_sessions(memory: Arc<MemoryManager>, limit: usize) -> Result<()> {
    let sessions = CodingSessionStore::new(Arc::clone(&memory.db)).recent(limit)?;
    if sessions.is_empty() {
        println!("No coding sessions recorded yet.");
        return Ok(());
    }
    println!("Recent coding sessions");
    for session in sessions {
        println!(
            "{} {} session={} exercise={} smoke={} checks={} artifacts={}{} {}",
            session.generated_at.format("%Y-%m-%d %H:%M:%S"),
            session.status,
            &session.id[..8.min(session.id.len())],
            session.exercise,
            session
                .smoke_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_string()),
            session.checks.len(),
            session.artifacts.len(),
            coding_session_commit_hint(&session)
                .map(|commit| format!(" commit={commit}"))
                .unwrap_or_default(),
            truncate(&session.goal, 90),
        );
        println!("  report: {}", session.session_report_path);
        if !session.checks.is_empty() {
            println!("  checks: {}", session.checks.join(", "));
        }
        for (index, step) in session.plan_steps.iter().take(4).enumerate() {
            println!("  plan {}: {}", index + 1, truncate(step, 120));
        }
        for (index, outcome) in session.step_outcomes.iter().take(8).enumerate() {
            println!("  outcome {}: {}", index + 1, truncate(outcome, 120));
        }
        for artifact in session.artifacts.iter().take(3) {
            println!("  artifact: {artifact}");
        }
        if let Some(path) = &session.transcript_path {
            println!("  transcript: {path}");
        }
        if let Some(reason) = &session.failure_reason {
            println!("  failure: {}", truncate(reason, 140));
        }
    }
    Ok(())
}

fn coding_session_commit_hint(session: &CodingSessionRecord) -> Option<String> {
    session
        .step_outcomes
        .iter()
        .find_map(|outcome| outcome.strip_prefix("commit "))
        .map(str::trim)
        .filter(|commit| !commit.is_empty() && *commit != "none")
        .map(|commit| commit[..commit.len().min(8)].to_string())
}

fn print_work_loops(memory: Arc<MemoryManager>, limit: usize) -> Result<()> {
    let runs = WorkLoopRunStore::new(Arc::clone(&memory.db)).recent(limit)?;
    if runs.is_empty() {
        println!("No supervised work-loop runs recorded yet.");
        return Ok(());
    }
    let gate_store = WorkLoopGateStore::new(Arc::clone(&memory.db));
    println!("Recent work/operator loops");
    for run in runs {
        println!(
            "#{} {} {}:{} loop={} cycles={}/{} passed={} failed={} report={}",
            run.id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "?".to_string()),
            run.recorded_at.format("%Y-%m-%d %H:%M:%S"),
            run.run_kind,
            run.profile,
            &run.run_id[..8.min(run.run_id.len())],
            run.completed_cycles,
            run.requested_cycles,
            run.passed_cycles,
            run.failed_cycles,
            run.report_path,
        );
        if let Some(ledger) = run_ledger_path(&run) {
            println!("  ledger: {ledger}");
        }
        for planned in run.planned_jobs.iter().take(8) {
            println!(
                "  plan {}: {} reason={}",
                planned.cycle,
                planned.kind,
                truncate(&planned.reason, 100),
            );
        }
        for smoke in run.smoke_records.iter().take(8) {
            println!(
                "  cycle {}: {} smoke={} {} transcript={} detail={}",
                smoke.cycle,
                smoke.kind,
                smoke
                    .smoke_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                if smoke.passed { "passed" } else { "failed" },
                smoke.transcript_path.as_deref().unwrap_or("none"),
                truncate(&smoke.detail, 80),
            );
        }
        if run.smoke_records.len() > 8 {
            println!("  ... {} more cycle(s)", run.smoke_records.len() - 8);
        }
        for gate in gate_store.recent_for_run(&run.run_id, 8)? {
            println!(
                "  gate {}: {} {} report={} detail={}",
                gate.cycle,
                gate.kind,
                gate.status,
                gate.report_path.as_deref().unwrap_or("none"),
                truncate(&gate.detail, 80),
            );
        }
    }
    Ok(())
}

fn print_run_log(memory: Arc<MemoryManager>, limit: usize) -> Result<()> {
    let runs = WorkLoopRunStore::new(Arc::clone(&memory.db)).recent(limit)?;
    if runs.is_empty() {
        println!("No Professor X runs recorded yet.");
        return Ok(());
    }
    println!("Professor X run log");
    println!("Commands: --run-review <run> | --replay <run> | --observe-work");
    for run in runs {
        let ledger = run_ledger_path(&run);
        println!("{}", format_run_log_entry(&run, ledger.as_deref()));
    }
    Ok(())
}

fn run_ledger_path(run: &WorkLoopRunRecord) -> Option<String> {
    let repo_root = default_repo_root();
    let report_path = resolve_report_reference(&repo_root, &run.report_path);
    let raw = std::fs::read_to_string(report_path).ok()?;
    let report: SupervisedLoopReport = serde_json::from_str(&raw).ok()?;
    report.ledger_path.filter(|path| !path.is_empty())
}

fn format_run_log_entry(run: &WorkLoopRunRecord, ledger_path: Option<&str>) -> String {
    let status = if run.failed_cycles == 0 { "passed" } else { "failed" };
    let latest_gate = run
        .smoke_records
        .last()
        .map(|record| {
            format!(
                "last_gate={} {}",
                record.kind,
                if record.passed { "passed" } else { "failed" }
            )
        })
        .unwrap_or_else(|| "last_gate=none".to_string());
    let mut lines = vec![format!(
        "- {} {}:{} run={} cycles={}/{} passed={} failed={} {}",
        run.recorded_at.format("%Y-%m-%d %H:%M:%S"),
        run.run_kind,
        run.profile,
        short_fragment(&run.run_id),
        run.completed_cycles,
        run.requested_cycles,
        run.passed_cycles,
        run.failed_cycles,
        status
    )];
    lines.push(format!("  L {latest_gate}"));
    lines.push(format!("  L report {}", run.report_path));
    if let Some(path) = ledger_path {
        lines.push(format!("  L ledger {path}"));
    }
    lines.push(format!(
        "  L replay cargo run -- --replay {}",
        short_fragment(&run.run_id)
    ));
    lines.push(format!(
        "  L review cargo run -- --run-review {}",
        short_fragment(&run.run_id)
    ));
    lines.join("\n")
}

fn print_run_review(memory: Arc<MemoryManager>, run_ref: &str) -> Result<()> {
    let repo_root = default_repo_root();
    let report_path = resolve_work_loop_report_path(Arc::clone(&memory), &repo_root, run_ref)?;
    let raw = std::fs::read_to_string(&report_path)?;
    let report: SupervisedLoopReport = serde_json::from_str(&raw)?;
    println!("Professor X run review");
    println!("  run: {}", report.run_id);
    println!("  kind/profile: {}:{}", report.run_kind, report.profile);
    println!("  started: {}", report.started_at);
    println!("  completed: {}", report.completed_at);
    println!(
        "  cycles: {}/{} passed={} failed={}",
        report.completed_cycles, report.requested_cycles, report.passed_cycles, report.failed_cycles
    );
    println!("  report: {}", display_repo_path(&repo_root, &report_path));
    if let Some(path) = &report.ledger_path {
        println!("  ledger: {path}");
    }

    println!("Plan");
    for job in &report.planned_jobs {
        println!(
            "  {}. {} - {}",
            job.cycle,
            job.kind,
            truncate(&job.reason, 120)
        );
    }

    println!("Evidence");
    for smoke in &report.smoke_records {
        let status = if smoke.passed { "passed" } else { "failed" };
        let artifact_path = resolve_report_reference(&repo_root, &smoke.report_path);
        println!(
            "  cycle {} {} {} :: {}",
            smoke.cycle,
            smoke.kind,
            status,
            truncate(&smoke.detail, 120)
        );
        println!("    report: {}", display_repo_path(&repo_root, &artifact_path));
        if !artifact_path.exists() {
            println!("    report_status: missing");
        }
        if let Some(transcript) = &smoke.transcript_path {
            let transcript_path = resolve_report_reference(&repo_root, transcript);
            println!(
                "    transcript: {}{}",
                display_repo_path(&repo_root, &transcript_path),
                if transcript_path.exists() { "" } else { " (missing)" }
            );
        }
        if smoke.kind == "patch_apply_commit" {
            print_patch_apply_review(&repo_root, &artifact_path)?;
        } else if smoke.kind == "operator_commit" {
            print_operator_commit_review(&repo_root, &artifact_path)?;
        }
    }
    if !report.timeline.is_empty() {
        println!("Timeline");
        for entry in report.timeline.iter().take(80) {
            println!("{}", format_work_timeline_entry(entry));
        }
        if report.timeline.len() > 80 {
            println!("  ... {} more event(s)", report.timeline.len() - 80);
        }
    }
    Ok(())
}

fn print_run_replay(memory: Arc<MemoryManager>, run_ref: &str) -> Result<()> {
    let repo_root = default_repo_root();
    let report_path = resolve_work_loop_report_path(Arc::clone(&memory), &repo_root, run_ref)?;
    let raw = std::fs::read_to_string(&report_path)?;
    let report: SupervisedLoopReport = serde_json::from_str(&raw)?;
    println!("Professor X run replay");
    println!(
        "run={} mode={}:{} cycles={}/{} passed={} failed={}",
        short_fragment(&report.run_id),
        report.run_kind,
        report.profile,
        report.completed_cycles,
        report.requested_cycles,
        report.passed_cycles,
        report.failed_cycles
    );
    println!("report: {}", display_repo_path(&repo_root, &report_path));
    println!();

    if !report.planned_jobs.is_empty() {
        println!("Plan");
        for job in &report.planned_jobs {
            println!(
                "- cycle {} {}",
                job.cycle,
                truncate(&job.reason, 120)
            );
            println!("  L job {}", job.kind);
        }
        println!();
    }

    println!("Replay");
    if report.timeline.is_empty() {
        println!("- no timeline recorded in this run report");
    } else {
        for entry in &report.timeline {
            println!("{}", format_work_replay_entry(entry));
        }
    }
    Ok(())
}

fn publish_run_artifacts(memory: Arc<MemoryManager>, run_ref: &str) -> Result<()> {
    let repo_root = default_repo_root();
    let report_path = resolve_work_loop_report_path(Arc::clone(&memory), &repo_root, run_ref)?;
    let raw = std::fs::read_to_string(&report_path)?;
    let report: SupervisedLoopReport = serde_json::from_str(&raw)?;
    let published = publish_run_report_artifacts(&repo_root, &report_path, &report)?;

    println!("Published Professor X run {}", short_fragment(&report.run_id));
    println!("  commit: {}", published.commit);
    for path in published.paths {
        println!("  artifact: {}", path.display());
    }
    Ok(())
}

#[derive(Debug)]
struct PublishedRunArtifacts {
    commit: String,
    paths: Vec<PathBuf>,
}

fn publish_run_report_artifacts(
    repo_root: &std::path::Path,
    report_path: &std::path::Path,
    report: &SupervisedLoopReport,
) -> Result<PublishedRunArtifacts> {
    let paths = publishable_run_artifact_paths(repo_root, report_path, report)?;
    let commit = commit_run_artifacts(repo_root, &paths, report)?;
    Ok(PublishedRunArtifacts { commit, paths })
}

fn publishable_run_artifact_paths(
    repo_root: &std::path::Path,
    report_path: &std::path::Path,
    report: &SupervisedLoopReport,
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    paths.push(repo_relative_existing_path(repo_root, report_path)?);
    let ledger = report
        .ledger_path
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("run report has no ledger_path; run it again before publishing"))?;
    paths.push(repo_relative_existing_path(
        repo_root,
        &resolve_report_reference(repo_root, ledger),
    )?);
    for smoke in &report.smoke_records {
        if let Some(path) =
            optional_publishable_run_artifact_path(repo_root, &smoke.report_path)?
        {
            paths.push(path);
        }
        if let Some(transcript) = &smoke.transcript_path {
            if let Some(path) = optional_publishable_run_artifact_path(repo_root, transcript)? {
                paths.push(path);
            }
        }
    }
    paths.extend(publishable_event_log_paths(repo_root, report)?);
    paths.sort();
    paths.dedup();
    for path in &paths {
        if !publishable_run_artifact_path(path) {
            anyhow::bail!(
                "refusing to publish non-run artifact '{}'; only run evidence artifacts are allowed",
                path.display()
            );
        }
    }
    Ok(paths)
}

fn publishable_event_log_paths(
    repo_root: &std::path::Path,
    report: &SupervisedLoopReport,
) -> Result<Vec<PathBuf>> {
    let mut dates = BTreeSet::new();
    collect_event_log_date(&mut dates, &report.started_at);
    collect_event_log_date(&mut dates, &report.completed_at);
    for entry in &report.timeline {
        collect_event_log_date(&mut dates, &entry.timestamp);
    }
    let mut paths = Vec::new();
    for date in dates {
        for candidate in [
            repo_root
                .join("professor-x")
                .join("artifacts")
                .join("events")
                .join(format!("{date}.jsonl")),
            repo_root
                .join("artifacts")
                .join("events")
                .join(format!("{date}.jsonl")),
        ] {
            if let Some(path) = optional_publishable_run_artifact_path(repo_root, candidate.display().to_string())? {
                paths.push(path);
            }
        }
    }
    Ok(paths)
}

fn collect_event_log_date(dates: &mut BTreeSet<String>, timestamp: &str) {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp) {
        dates.insert(dt.date_naive().to_string());
    }
}

fn optional_publishable_run_artifact_path(
    repo_root: &std::path::Path,
    raw: impl AsRef<str>,
) -> Result<Option<PathBuf>> {
    let resolved = resolve_report_reference(repo_root, raw.as_ref());
    if !resolved.exists() {
        return Ok(None);
    }
    let relative = match repo_relative_existing_path(repo_root, &resolved) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };
    if !publishable_run_artifact_path(&relative) {
        anyhow::bail!(
            "refusing to publish linked artifact '{}' because it is outside the run artifact allowlist",
            relative.display()
        );
    }
    Ok(Some(relative))
}

fn publishable_run_artifact_path(path: &std::path::Path) -> bool {
    let text = path.to_string_lossy();
    text.starts_with("professor-x/artifacts/work-loop/")
        || text.starts_with("professor-x/artifacts/coding-smoke/")
        || text.starts_with("professor-x/artifacts/coding-sessions/")
        || text.starts_with("professor-x/artifacts/transcripts/")
        || text.starts_with("professor-x/artifacts/events/")
        || text.starts_with("professor-x/artifacts/evolution/")
        || text.starts_with("professor-x/artifacts/hiro/")
        || text.starts_with("artifacts/work-loop/")
        || text.starts_with("artifacts/coding-smoke/")
        || text.starts_with("artifacts/coding-sessions/")
        || text.starts_with("artifacts/transcripts/")
        || text.starts_with("artifacts/events/")
        || text.starts_with("artifacts/evolution/")
        || text.starts_with("artifacts/hiro/")
}

fn commit_run_artifacts(
    repo_root: &std::path::Path,
    paths: &[PathBuf],
    report: &SupervisedLoopReport,
) -> Result<String> {
    if paths.is_empty() {
        anyhow::bail!("no run artifacts selected for publish");
    }
    let mut add = std::process::Command::new("git");
    add.arg("add").arg("--");
    for path in paths {
        add.arg(path);
    }
    let add = add.current_dir(repo_root).output()?;
    if !add.status.success() {
        anyhow::bail!(
            "git add run artifacts failed: {}",
            String::from_utf8_lossy(&add.stderr)
        );
    }
    let diff = std::process::Command::new("git")
        .args(["diff", "--cached", "--quiet", "--"])
        .args(paths)
        .current_dir(repo_root)
        .status()?;
    if diff.success() {
        anyhow::bail!("run artifacts are already published; no staged changes");
    }
    let message = format!(
        "professor-x: publish {} run {}",
        report.run_kind,
        short_fragment(&report.run_id)
    );
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(repo_root)
        .output()?;
    if !commit.status.success() {
        anyhow::bail!(
            "git publish commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
    }
    git_head(repo_root)
}

fn format_work_replay_entry(entry: &WorkTimelineEntry) -> String {
    let time = chrono::DateTime::parse_from_rfc3339(&entry.timestamp)
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|_| "??:??:??".to_string());
    let mut lines = vec![format!(
        "- #{:05} {} {:<6} {} {}",
        entry.event_id,
        time,
        entry.label,
        entry.action,
        truncate(&entry.summary, 118)
    )];
    let mut meta = Vec::new();
    if let Some(task) = &entry.task_id {
        meta.push(format!("task={task}"));
    }
    if let Some(run) = &entry.run_id {
        meta.push(format!("run={run}"));
    }
    if let Some(cycle) = entry.cycle {
        meta.push(format!("cycle={cycle}"));
    }
    if let Some(step) = entry.step {
        meta.push(format!("step={step}"));
    }
    if let Some(job) = &entry.job {
        meta.push(format!("job={job}"));
    }
    if let Some(tool) = &entry.tool {
        meta.push(format!("tool={tool}"));
    }
    if let Some(passed) = entry.passed {
        meta.push(format!("passed={passed}"));
    }
    if !meta.is_empty() {
        lines.push(format!("  L {}", meta.join(" ")));
    }
    if let Some(detail) = &entry.detail {
        lines.push(format!("  L detail {}", truncate(detail, 180)));
    }
    if let Some(report) = &entry.report_path {
        lines.push(format!("  L report {}", truncate(report, 140)));
    }
    if let Some(transcript) = &entry.transcript_path {
        lines.push(format!("  L transcript {}", truncate(transcript, 140)));
    }
    for artifact in entry.artifacts.iter().take(3) {
        lines.push(format!("  L artifact {}", truncate(artifact, 140)));
    }
    lines.join("\n")
}

fn format_work_timeline_entry(entry: &WorkTimelineEntry) -> String {
    let time = chrono::DateTime::parse_from_rfc3339(&entry.timestamp)
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|_| "??:??:??".to_string());
    let mut meta = Vec::new();
    if let Some(task) = &entry.task_id {
        meta.push(format!("task={task}"));
    }
    if let Some(run) = &entry.run_id {
        meta.push(format!("run={run}"));
    }
    if let Some(cycle) = entry.cycle {
        meta.push(format!("cycle={cycle}"));
    }
    if let Some(step) = entry.step {
        meta.push(format!("step={step}"));
    }
    if let Some(tool) = &entry.tool {
        meta.push(format!("tool={tool}"));
    }
    if let Some(job) = &entry.job {
        meta.push(format!("job={job}"));
    }
    if let Some(passed) = entry.passed {
        meta.push(format!("passed={passed}"));
    }
    let meta = if meta.is_empty() {
        String::new()
    } else {
        format!(" [{}]", meta.join(" "))
    };
    let detail = entry
        .detail
        .as_ref()
        .map(|detail| format!(" :: {}", truncate(detail, 120)))
        .unwrap_or_default();
    format!(
        "  #{:05} {} {:<6} {} {}{}{}",
        entry.event_id,
        time,
        entry.label,
        entry.action,
        truncate(&entry.summary, 100),
        meta,
        detail
    )
}

fn print_patch_apply_review(repo_root: &std::path::Path, path: &std::path::Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    let report: PatchVerificationReport = serde_json::from_str(&raw)?;
    println!(
        "    patch: accepted={} applied={} commit={} report_commit={}",
        report.accepted,
        report.applied,
        report.commit.as_deref().unwrap_or("none"),
        report.report_commit.as_deref().unwrap_or("none")
    );
    println!("    checks: {}", report.checks.join(", "));
    println!(
        "    diff: {} bytes hash={}",
        report.diff_bytes,
        report.diff_hash.as_deref().unwrap_or("none")
    );
    println!("    reason: {}", truncate(&report.reason, 140));
    if let Some(commit) = &report.commit {
        let output = std::process::Command::new("git")
            .args(["show", "--stat", "--oneline", "--no-renames", commit])
            .current_dir(repo_root)
            .output();
        if let Ok(output) = output {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines().take(8) {
                    println!("    git: {}", truncate(line, 140));
                }
            }
        }
    }
    Ok(())
}

fn print_operator_commit_review(repo_root: &std::path::Path, path: &std::path::Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    let report: EvolutionProposalDryRunReport = serde_json::from_str(&raw)?;
    println!(
        "    operator: accepted={} applied={} commit={}",
        report.accepted,
        report.applied,
        report.commit.as_deref().unwrap_or("none")
    );
    println!("    target: {}", report.target_component);
    println!("    checks: {}", report.checks.join(", "));
    println!(
        "    diff: {} bytes hash={}",
        report.diff_bytes,
        report.diff_hash.as_deref().unwrap_or("none")
    );
    println!("    reason: {}", truncate(&report.reason, 140));
    if let Some(commit) = &report.commit {
        let output = std::process::Command::new("git")
            .args(["show", "--stat", "--oneline", "--no-renames", commit])
            .current_dir(repo_root)
            .output();
        if let Ok(output) = output {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines().take(8) {
                    println!("    git: {}", truncate(line, 140));
                }
            }
        }
    }
    Ok(())
}

fn resolve_work_loop_report_path(
    memory: Arc<MemoryManager>,
    repo_root: &std::path::Path,
    run_ref: &str,
) -> Result<PathBuf> {
    let direct = resolve_report_reference(repo_root, run_ref);
    if direct.exists() {
        return Ok(direct);
    }

    if run_ref == "latest" {
        if let Some(run) = WorkLoopRunStore::new(Arc::clone(&memory.db)).latest()? {
            let path = resolve_report_reference(repo_root, &run.report_path);
            if path.exists() {
                return Ok(path);
            }
        }
        return latest_work_loop_report_from_artifacts(repo_root)
            .ok_or_else(|| anyhow::anyhow!("no work-loop report artifacts found"));
    }

    if let Some(run) = WorkLoopRunStore::new(Arc::clone(&memory.db))
        .recent(100)?
        .into_iter()
        .find(|run| run.run_id.starts_with(run_ref))
    {
        let path = resolve_report_reference(repo_root, &run.report_path);
        if path.exists() {
            return Ok(path);
        }
    }

    find_work_loop_report_by_ref(repo_root, run_ref)
        .ok_or_else(|| anyhow::anyhow!("no work-loop report found for '{run_ref}'"))
}

fn latest_work_loop_report_from_artifacts(repo_root: &std::path::Path) -> Option<PathBuf> {
    let mut reports = work_loop_report_artifacts(repo_root);
    reports.sort();
    reports.pop()
}

fn find_work_loop_report_by_ref(repo_root: &std::path::Path, run_ref: &str) -> Option<PathBuf> {
    for path in work_loop_report_artifacts(repo_root) {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.contains(run_ref))
            .unwrap_or(false)
        {
            return Some(path);
        }
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if raw.contains(run_ref) {
                return Some(path);
            }
        }
    }
    None
}

fn work_loop_report_artifacts(repo_root: &std::path::Path) -> Vec<PathBuf> {
    let root = repo_root.join("professor-x").join("artifacts").join("work-loop");
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("loop-") && name.ends_with(".json"))
                .unwrap_or(false)
            {
                out.push(path);
            }
        }
    }
    out
}

fn resolve_report_reference(repo_root: &std::path::Path, raw: impl AsRef<str>) -> PathBuf {
    let path = PathBuf::from(raw.as_ref());
    if path.is_absolute() {
        return path;
    }
    let from_root = repo_root.join(&path);
    if from_root.exists() {
        return from_root;
    }
    if path.starts_with("artifacts") {
        return repo_root.join("professor-x").join(path);
    }
    from_root
}

fn display_repo_path(repo_root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn print_task_review(transcripts: Arc<TranscriptStore>, task_ref: &str) -> Result<()> {
    let transcript = if task_ref == "latest" {
        transcripts.latest()?
    } else {
        transcripts.get_by_task_prefix(task_ref)?
    };
    let Some(transcript) = transcript else {
        println!("No transcript found for '{task_ref}'.");
        return Ok(());
    };
    let raw = std::fs::read_to_string(&transcript.transcript_path)?;
    let doc: serde_json::Value = serde_json::from_str(&raw)?;
    println!("{}", format_transcript_summary(&transcript));
    println!("path: {}", transcript.transcript_path);
    println!("summary: {}", transcript.summary);

    let review = &doc["review"];
    print_json_array("changed files", &review["changed_files"], 20);
    print_json_array("git status", &review["git_status"], 20);
    print_json_array("tool artifacts", &review["tool_artifacts"], 20);

    let steps = doc["steps"].as_array().map(Vec::len).unwrap_or_default();
    let events = doc["events"].as_array().map(Vec::len).unwrap_or_default();
    println!("steps: {steps}");
    println!("events: {events}");

    if let Some(diff) = review["git_diff"].as_str().filter(|diff| !diff.is_empty()) {
        println!("diff:");
        println!("{}", truncate(diff, 4000));
        if review["git_diff_truncated"].as_bool().unwrap_or(false) {
            println!("[diff is truncated in transcript]");
        }
    } else {
        println!("diff: clean or no uncommitted diff captured");
    }
    Ok(())
}

fn format_transcript_summary(transcript: &TranscriptSummary) -> String {
    format!(
        "{} {} transcript={} task={} attempts={} steps={} {}",
        transcript.recorded_at.format("%Y-%m-%d %H:%M:%S"),
        transcript.status,
        &transcript.id[..8.min(transcript.id.len())],
        &transcript.task_id[..8.min(transcript.task_id.len())],
        transcript.attempt_count,
        transcript.step_count,
        truncate(&transcript.task_description, 96),
    )
}

fn format_task_run_summary(run: &TaskRun) -> String {
    format!(
        "{} {} task={} type={} p{} attempts={} steps={}{} {}",
        run.updated_at.format("%Y-%m-%d %H:%M:%S"),
        run.status,
        &run.task_id[..8.min(run.task_id.len())],
        run.task_type,
        run.priority,
        run.attempt_count,
        run.step_count,
        run.outcome_score
            .map(|score| format!(" score={score:.2}"))
            .unwrap_or_default(),
        truncate(&run.description, 96),
    )
}

fn print_json_array(label: &str, value: &serde_json::Value, limit: usize) {
    let Some(items) = value.as_array() else {
        println!("{label}: 0");
        return;
    };
    println!("{label}: {}", items.len());
    for item in items.iter().take(limit) {
        if let Some(text) = item.as_str() {
            println!("  {text}");
        } else {
            println!("  {item}");
        }
    }
    if items.len() > limit {
        println!("  ... {} more", items.len() - limit);
    }
}

async fn watch_events(events: Arc<EventStore>) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Watching Professor X events. Press Ctrl+C to stop.");
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                for event in events.after_id(last_id, 100)? {
                    last_id = event.id;
                    println!("{}", format_event(&event));
                }
            }
        }
    }
    Ok(())
}

async fn watch_work_feed(events: Arc<EventStore>) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Watching Professor X work feed. Press Ctrl+C to stop.");
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                for event in events.work_after_id(last_id, 100)? {
                    last_id = event.id;
                    println!("{}", format_work_event(&event));
                }
            }
        }
    }
    Ok(())
}

async fn observe_work_cockpit(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    limit: usize,
) -> Result<()> {
    println!("Opening Professor X work cockpit. Press Ctrl+C to stop.");
    loop {
        let screen = render_work_cockpit(Arc::clone(&memory), Arc::clone(&events), limit)?;
        print!("\x1B[2J\x1B[H{screen}");
        io::stdout().flush()?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(1000)) => {}
        }
    }
    Ok(())
}

fn print_work_cockpit(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    limit: usize,
) -> Result<()> {
    println!("{}", render_work_cockpit(memory, events, limit)?);
    Ok(())
}

fn render_work_cockpit(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    limit: usize,
) -> Result<String> {
    let repo_root = default_repo_root();
    let recent_events = events.work_tail(limit)?;
    let latest_run = WorkLoopRunStore::new(Arc::clone(&memory.db)).latest()?;
    let gate_store = WorkLoopGateStore::new(Arc::clone(&memory.db));
    let latest_gate = gate_store.latest()?;
    let recent_gates = latest_run
        .as_ref()
        .map(|run| gate_store.recent_for_run(&run.run_id, 8))
        .transpose()?
        .unwrap_or_default();

    Ok(format_work_cockpit(
        &repo_root,
        &recent_events,
        latest_run.as_ref(),
        latest_gate.as_ref(),
        &recent_gates,
    ))
}

fn format_work_cockpit(
    repo_root: &std::path::Path,
    recent_events: &[memd::events::AgentEvent],
    latest_run: Option<&WorkLoopRunRecord>,
    latest_gate: Option<&WorkLoopGateRecord>,
    recent_gates: &[WorkLoopGateRecord],
) -> String {
    let mut lines = Vec::new();
    lines.push("Professor X live work cockpit".to_string());
    lines.push(format!("repo  {}", cockpit_git_line(repo_root)));
    lines.push(format!(
        "clock {}  source ~/.professor-x/state.db + professor-x/artifacts/events/*.jsonl",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push(format!(
        "state {}  {}",
        cockpit_state(latest_run, latest_gate),
        cockpit_latest_activity(recent_events)
    ));
    lines.push(String::new());
    lines.push("Current run".to_string());
    match latest_run {
        Some(run) => {
            lines.push(format!(
                "  progress {}",
                cockpit_progress(run.completed_cycles, run.requested_cycles)
            ));
            lines.push(format!(
                "  {}:{} run={} cycles={}/{} passed={} failed={} report={}",
                run.run_kind,
                run.profile,
                short_fragment(&run.run_id),
                run.completed_cycles,
                run.requested_cycles,
                run.passed_cycles,
                run.failed_cycles,
                truncate(&run.report_path, 120),
            ));
            lines.push(format!(
                "  commands replay={} review={} publish={}",
                format!("--replay {}", short_fragment(&run.run_id)),
                format!("--run-review {}", short_fragment(&run.run_id)),
                format!("--publish-run {}", short_fragment(&run.run_id)),
            ));
            for job in run.planned_jobs.iter().take(4) {
                lines.push(format!(
                    "  plan {:>2}: {:<18} {}",
                    job.cycle,
                    job.kind,
                    truncate(&job.reason, 92)
                ));
            }
            if !run.smoke_records.is_empty() {
                lines.push(String::new());
                lines.push("Evidence bundle".to_string());
                for smoke in run.smoke_records.iter().rev().take(6).rev() {
                    lines.push(format!(
                        "  {:>2}. {:<20} {:<6} {}",
                        smoke.cycle,
                        smoke.kind,
                        if smoke.passed { "passed" } else { "failed" },
                        truncate(&smoke.report_path, 96)
                    ));
                    if let Some(transcript) = &smoke.transcript_path {
                        lines.push(format!("      transcript {}", truncate(transcript, 110)));
                    }
                    if !smoke.detail.is_empty() {
                        lines.push(format!("      detail {}", truncate(&smoke.detail, 110)));
                    }
                }
            }
        }
        None => lines.push("  waiting for --operator-run, --operator-run-commit, or --lab".to_string()),
    }

    lines.push(String::new());
    lines.push("Active gate".to_string());
    match latest_gate {
        Some(gate) => {
            lines.push(format!(
                "  cycle={} profile={} job={} status={} passed={} updated={} {}",
                gate.cycle,
                gate.profile,
                gate.kind,
                gate.status,
                gate.passed
                    .map(|passed| passed.to_string())
                    .unwrap_or_else(|| "pending".to_string()),
                gate.updated_at.format("%H:%M:%S"),
                truncate(&gate.detail, 90),
            ));
            if let Some(report) = &gate.report_path {
                lines.push(format!("  proof report {}", truncate(report, 130)));
            }
            if let Some(transcript) = &gate.transcript_path {
                lines.push(format!("  proof transcript {}", truncate(transcript, 130)));
            }
            if let Some(workspace) = &gate.workspace {
                lines.push(format!("  workspace {}", truncate(workspace, 130)));
            }
        }
        None => lines.push("  no gates recorded yet".to_string()),
    }

    if !recent_gates.is_empty() {
        lines.push(String::new());
        lines.push("Gate ledger".to_string());
        for gate in recent_gates.iter().take(6) {
            lines.push(format!(
                "  {:>2}. {:<20} {:<8} {}",
                gate.cycle,
                gate.kind,
                gate.status,
                truncate(&gate.detail, 90)
            ));
        }
    }

    lines.push(String::new());
    lines.push(format!("Recent signal {}", work_signal_summary(recent_events)));
    lines.push(String::new());
    lines.push("Live trace".to_string());
    if recent_events.is_empty() {
        lines.push("  no work events recorded yet".to_string());
    } else {
        for event in recent_events {
            lines.push(format_work_event(event));
        }
    }
    lines.push(String::new());
    lines.push(
        "Commands: --cockpit | --prof-x-live-publish 6 | --observe-work | --replay latest | --run-review latest"
            .to_string(),
    );
    lines.join("\n")
}

fn cockpit_state(
    latest_run: Option<&WorkLoopRunRecord>,
    latest_gate: Option<&WorkLoopGateRecord>,
) -> &'static str {
    if latest_gate
        .map(|gate| gate.status.as_str() == "running")
        .unwrap_or(false)
    {
        return "RUNNING";
    }
    if latest_run
        .map(|run| run.failed_cycles > 0)
        .unwrap_or(false)
    {
        return "NEEDS-REVIEW";
    }
    if latest_run
        .map(|run| run.completed_cycles == run.requested_cycles && run.failed_cycles == 0)
        .unwrap_or(false)
    {
        return "READY";
    }
    "IDLE"
}

fn cockpit_latest_activity(events: &[memd::events::AgentEvent]) -> String {
    events
        .last()
        .map(|event| {
            format!(
                "last_event=#{:05} {} {}",
                event.id,
                event.timestamp.format("%H:%M:%S"),
                truncate(&event.summary, 80)
            )
        })
        .unwrap_or_else(|| "last_event=none".to_string())
}

fn cockpit_progress(completed: u32, requested: u32) -> String {
    let width = 12usize;
    let filled = if requested == 0 {
        0
    } else {
        ((completed.min(requested) as usize) * width) / requested as usize
    };
    format!(
        "[{}{}] {}/{}",
        "#".repeat(filled),
        ".".repeat(width.saturating_sub(filled)),
        completed,
        requested
    )
}

fn work_signal_summary(events: &[memd::events::AgentEvent]) -> String {
    let mut task = 0;
    let mut tool = 0;
    let mut policy = 0;
    let mut evolution = 0;
    let mut loop_events = 0;
    let mut transcripts = 0;
    for event in events {
        let event_type = event.event_type.as_str();
        if event_type.starts_with("task.") {
            task += 1;
        } else if event_type.starts_with("tool.") {
            tool += 1;
        } else if event_type.starts_with("policy.") {
            policy += 1;
        } else if event_type.starts_with("evolution.") {
            evolution += 1;
        } else if event_type.starts_with("work_loop.") {
            loop_events += 1;
        } else if event_type == "transcript.written" {
            transcripts += 1;
        }
    }
    format!(
        "events={} task={} tool={} policy={} evolution={} loop={} transcript={}",
        events.len(),
        task,
        tool,
        policy,
        evolution,
        loop_events,
        transcripts
    )
}

fn cockpit_git_line(repo_root: &std::path::Path) -> String {
    let branch = command_stdout(repo_root, "git", &["branch", "--show-current"])
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let commit = command_stdout(repo_root, "git", &["rev-parse", "--short", "HEAD"])
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let status = command_stdout(repo_root, "git", &["status", "--short"])
        .map(|text| if text.is_empty() { "clean" } else { "dirty" })
        .unwrap_or("unknown");
    format!("{branch} @ {commit} {status}")
}

fn command_stdout(repo_root: &std::path::Path, command: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(command)
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn format_event(event: &memd::events::AgentEvent) -> String {
    let task = event
        .task_id
        .as_ref()
        .map(|id| format!(" task={}", &id[..8.min(id.len())]))
        .unwrap_or_default();
    let session = event
        .session_id
        .as_ref()
        .map(|id| format!(" session={}", &id[..8.min(id.len())]))
        .unwrap_or_default();
    format!(
        "#{:05} {} {:<22}{}{} {}",
        event.id,
        event.timestamp.format("%Y-%m-%d %H:%M:%S"),
        event.event_type,
        task,
        session,
        event.summary
    )
}

fn format_work_event(event: &memd::events::AgentEvent) -> String {
    let task = event
        .task_id
        .as_ref()
        .map(|id| id[..8.min(id.len())].to_string())
        .unwrap_or_else(|| "--------".to_string());
    let label = work_event_label(&event.event_type);

    let mut lines = vec![format!(
        "- #{:05} {} {:<6} {} {}",
        event.id,
        event.timestamp.format("%H:%M:%S"),
        label,
        event_action(event),
        truncate(&event.summary, 120),
    )];

    let mut meta = vec![format!("task={task}")];
    if let Some(run) = event.payload["run_id"].as_str() {
        meta.push(format!("run={}", short_fragment(run)));
    }
    if let Some(cycle) = event.payload["cycle"].as_i64() {
        let total = event.payload["cycles"]
            .as_i64()
            .map(|total| format!("/{total}"))
            .unwrap_or_default();
        meta.push(format!("cycle={cycle}{total}"));
    }
    if let Some(job) = event.payload["job"].as_str() {
        meta.push(format!("job={job}"));
    }
    if let Some(step) = event.payload["step"].as_i64() {
        meta.push(format!("step={step}"));
    }
    if let Some(step) = event.payload["plan_step"].as_i64() {
        let total = event.payload["plan_total"]
            .as_i64()
            .map(|total| format!("/{total}"))
            .unwrap_or_default();
        meta.push(format!("plan={step}{total}"));
    }
    if let Some(step) = event.payload["outcome_step"].as_i64() {
        let total = event.payload["outcome_total"]
            .as_i64()
            .map(|total| format!("/{total}"))
            .unwrap_or_default();
        meta.push(format!("outcome={step}{total}"));
    }
    if let Some(tool) = event.payload["tool"].as_str() {
        meta.push(format!("tool={tool}"));
    }
    if let Some(exercise) = event.payload["exercise"].as_str() {
        meta.push(format!("exercise={exercise}"));
    }
    if let Some(accepted) = event.payload["accepted"].as_bool() {
        meta.push(format!("decision={}", if accepted { "accept" } else { "reject" }));
    }
    if let Some(passed) = event.payload["passed"].as_bool() {
        meta.push(format!("passed={passed}"));
    }
    if let Some(ms) = event.payload["execution_ms"].as_i64() {
        meta.push(format!("duration={ms}ms"));
    }
    if let Some(items) = event.payload["checks"]
        .as_array()
        .or_else(|| event.payload["planned_checks"].as_array())
        .filter(|items| !items.is_empty())
    {
        meta.push(format!("checks={}", items.len()));
    }
    if let Some(bytes) = event.payload["diff_bytes"].as_i64().filter(|bytes| *bytes > 0) {
        meta.push(format!("diff={bytes}b"));
    }
    if let Some(items) = event.payload["artifacts"].as_array().filter(|items| !items.is_empty()) {
        meta.push(format!("artifacts={}", items.len()));
    }
    lines.push(format!("  L {}", meta.join(" ")));

    push_payload_line(&mut lines, "report", event.payload["report_path"].as_str());
    push_payload_line(&mut lines, "transcript", event.payload["transcript_path"].as_str());
    push_payload_line(&mut lines, "patch", event.payload["patch_path"].as_str());
    push_payload_line(&mut lines, "target", event.payload["target_component"].as_str());
    if let Some(commit) = event.payload["commit"].as_str() {
        lines.push(format!("  L commit {}", short_fragment(commit)));
    }
    if let Some(detail) = event.payload["error"]
        .as_str()
        .filter(|text| !text.is_empty())
        .or_else(|| event.payload["output_preview"].as_str())
        .or_else(|| event.payload["params_preview"].as_str())
        .or_else(|| event.payload["detail"].as_str())
    {
        lines.push(format!("  L detail {}", one_line(detail, 180)));
    }
    if let Some(items) = event.payload["artifacts"].as_array() {
        for artifact in items.iter().take(3).filter_map(|item| item.as_str()) {
            lines.push(format!("  L artifact {}", truncate(artifact, 140)));
        }
    }

    lines.join("\n")
}

fn event_action(event: &memd::events::AgentEvent) -> &'static str {
    match event.event_type.as_str() {
        "work_loop.started" => "Started loop",
        "work_loop.job.planned" => "Planned gate",
        "work_loop.cycle.started" => "Started gate",
        "work_loop.cycle.passed" => "Passed gate",
        "work_loop.cycle.failed" => "Failed gate",
        "work_loop.completed" => "Completed loop",
        "work_loop.completed_with_failures" => "Completed loop with failures",
        "tool.requested" => "Requested",
        "tool.started" => "Running",
        "tool.succeeded" => "Ran",
        "tool.failed" => "Failed",
        "task.queued" => "Queued task",
        "task.started" => "Started task",
        "task.attempt.started" => "Started attempt",
        "task.succeeded" => "Completed task",
        "task.failed" => "Failed task",
        "coding.smoke.started" => "Started coding smoke",
        "coding.smoke.passed" => "Passed coding smoke",
        "coding.smoke.failed" => "Failed coding smoke",
        "transcript.written" => "Wrote transcript",
        "evolution.patch_apply.committed" => "Committed verified patch",
        "evolution.operator_commit.committed" => "Committed operator proposal",
        "evolution.patch_apply.rejected" | "evolution.proposal_dry_run.rejected" => {
            "Rejected proposal"
        }
        event_type if event_type.starts_with("evolution.") => "Evolution event",
        event_type if event_type.starts_with("policy.") => "Policy gate",
        event_type if event_type.starts_with("autonomous_run.") => "Autonomous run",
        _ => "Observed",
    }
}

fn push_payload_line(lines: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        lines.push(format!("  L {label} {}", truncate(value, 140)));
    }
}

fn work_event_label(event_type: &str) -> &'static str {
    if event_type.starts_with("tool.") {
        "TOOL"
    } else if event_type.starts_with("policy.") {
        "POLICY"
    } else if event_type.starts_with("task.") {
        "TASK"
    } else if event_type.starts_with("react.") {
        "REACT"
    } else if event_type.starts_with("coding.session.") {
        "CODE"
    } else if event_type.starts_with("coding.smoke.") {
        "SMOKE"
    } else if event_type.starts_with("evolution.") {
        "EVOLVE"
    } else if event_type.starts_with("autonomous_run.") {
        "AUTON"
    } else if event_type.starts_with("work_loop.") {
        "LOOP"
    } else if event_type == "transcript.written" {
        "TRACE"
    } else {
        "EVENT"
    }
}

fn short_fragment(id: &str) -> &str {
    &id[..8.min(id.len())]
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}

// ── Signal handlers ───────────────────────────────────────────────────────────

fn setup_signal_handlers(cancel: CancellationToken) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let cancel1 = cancel.clone();
        tokio::spawn(async move {
            let mut usr1 = signal(SignalKind::user_defined1()).expect("SIGUSR1 handler");
            usr1.recv().await;
            info!("received SIGUSR1 — initiating graceful shutdown");
            cancel1.cancel();
        });

        tokio::spawn(async move {
            let mut usr2 = signal(SignalKind::user_defined2()).expect("SIGUSR2 handler");
            usr2.recv().await;
            info!("received SIGUSR2 — initiating graceful shutdown");
            cancel.cancel();
        });
    }
}

// ── Filesystem helpers ───────────────────────────────────────────────────────

/// Walk up from cwd looking for `.git` to find the repo root. Used by the
/// `--validate-artifacts` scan so it can be invoked from any subdirectory.
fn repo_root_from_cwd() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        }
    }
}

// ── Scheduler ────────────────────────────────────────────────────────────────

fn seed_daily_schedule(scheduler: &agentd::CronScheduler, fire_now: bool) -> Result<()> {
    use agentd::scheduler::{CronJob, JobState, ScheduleType};
    use chrono::Utc;

    let schedule = load_daily_schedule()?;
    let disabled_legacy = scheduler.disable_legacy_daily_cycle()?;
    if disabled_legacy > 0 {
        info!("scheduler: disabled {disabled_legacy} legacy daily cycle job(s)");
    }
    let now = Utc::now();
    let daily_start = if fire_now {
        now
    } else {
        now.date_naive()
            .and_hms_opt(22, 0, 0)
            .map(|dt| dt.and_utc())
            .filter(|dt| *dt > now)
            .unwrap_or_else(|| {
                (now + chrono::Duration::days(1))
                    .date_naive()
                    .and_hms_opt(22, 0, 0)
                    .expect("valid daily cycle time")
                    .and_utc()
            })
    };

    let job_count = schedule.jobs.len();
    for job in schedule.jobs {
        // Fallback artifact-kind defaults for jobs whose TOML entry does not
        // declare one. Keeps the Phase B gate active on the historically-
        // configured jobs without forcing a config rewrite on landing.
        let inferred_kind = job
            .expected_artifact_kind
            .clone()
            .or_else(|| match job.id.as_str() {
                id if id.contains("daily-update") => Some("daily_update".to_string()),
                "literature-search" => Some("literature_note".to_string()),
                "experiment-runner" => Some("experiment_result".to_string()),
                _ => None,
            });
        let cron_job = CronJob {
            id: format!("daily-{}", job.id),
            name: format!("Daily {}", job.id),
            prompt: format!(
                "Execute scheduled daily job '{}' using skill '{}'. Load the skill, follow its local-first workflow, classify the outcome, and write durable results to brain/ or artifacts/. network_required={}. Keep all file changes inside the repository and use only policy-approved tools.",
                job.id, job.skill, job.network_required
            ),
            schedule_type: ScheduleType::Interval,
            schedule_value: "86400".to_string(),
            next_run_at: daily_start + chrono::Duration::minutes(job.offset_minutes as i64),
            enabled: true,
            state: JobState::Scheduled,
            repeat_limit: None,
            repeat_completed: 0,
            last_run_at: None,
            last_status: None,
            created_at: now,
            expected_artifact_kind: inferred_kind,
        };
        scheduler.register(&cron_job)?;
    }

    info!("scheduler: registered {job_count} daily job(s)");
    Ok(())
}

// ── Cognition base ────────────────────────────────────────────────────────────

fn seed_cognition_base() -> Vec<CognitionItem> {
    let seeds = [
        (
            "CoALA: Language agents have four memory types — working (in-context), episodic (retrievable past), semantic (factual knowledge), procedural (skills/actions).",
            "paper:2309.02427",
        ),
        (
            "CoALA: The action space spans storage (read/write), process (execute), and reasoning operations.",
            "paper:2309.02427",
        ),
        (
            "Voyager: A growing skill library of verified procedural knowledge enables lifelong learning. Skills that fail consistently are pruned.",
            "paper:2305.16291",
        ),
        (
            "Voyager: 4-round attempt limit per task prevents infinite loops while allowing recovery from transient failures.",
            "paper:2305.16291",
        ),
        (
            "Reflexion: Verbal self-reflection after failure is reinforcement learning without weight updates. Buffer max 3 reflections, oldest evicted.",
            "paper:2303.11366",
        ),
        (
            "ReAct: Interleaving Thought and Action/Observation is more reliable than acting alone. Thought lets the agent plan before committing to a tool call.",
            "paper:2210.03629",
        ),
        (
            "AHE: Three observability pillars for harness evolution: component (which files changed), experience (what was tried), decision (why changes were proposed).",
            "paper:2604.25850",
        ),
        (
            "AHE: Every harness modification needs a falsifiable ChangeManifest with predicted fixes and regressions. Verify predictions in the next cycle.",
            "paper:2604.25850",
        ),
        (
            "AHE: Seven evolvable components: system prompt, tool descriptions, skill definitions, harness config, procedural memory, middleware, core logic.",
            "paper:2604.25850",
        ),
        (
            "ASI-Evolve: Researcher/Engineer/Analyzer loop enables closed-loop self-improvement. Researcher proposes, Engineer experiments, Analyzer distills lessons.",
            "paper:2603.29640",
        ),
        (
            "ASI-Evolve: UCB1 sampling c=1.414 balances exploration (unvisited nodes) vs exploitation (high-scoring nodes).",
            "paper:2603.29640",
        ),
        (
            "ASI-Evolve: Cognition base stores ~100 distilled insights. Quality score updated via (success+1)/(use+2).",
            "paper:2603.29640",
        ),
        (
            "EvolveR: Quality formula (success_count+1)/(use_count+2) is Laplace-smoothed. Prior of 0.5 for new items, avoids zero-division.",
            "paper:2510.16079",
        ),
        (
            "Memory agents: Multi-signal retrieval: cosine (α=0.5) + recency decay (β=0.3, λ=0.1) + importance (γ=0.2).",
            "paper:2603.07670",
        ),
        (
            "Memory agents: Write pipeline: filter → tag → canonicalize → deduplicate (cosine>0.92 skip) → score → embed → cluster → write.",
            "paper:2603.07670",
        ),
        (
            "CLAG: Two-stage retrieval (cluster profile matching → intra-cluster) reduces latency. Cold start flat until 100 entries, split at 300.",
            "paper:2603.15421",
        ),
        (
            "Self-Generated ICE: Top-k similar past tasks injected as in-context examples. Zero fine-tuning needed; ALFWorld 73%→93%.",
            "paper:2505.00234",
        ),
        (
            "MARS: Single-cycle reflection on failure — extract principle (what not to do) + procedure (what to do instead). Write both to semantic memory.",
            "paper:2601.11974",
        ),
        (
            "ACE: Context window as evolving playbook. Semantic memory entries are the playbook; updated on every success/failure.",
            "paper:2510.04618",
        ),
        (
            "Life-Harness: Structural harness improvements transfer to 17 other models at 88.5% avg relative gain. Harness corpus = portable artifact.",
            "paper:2605.22166",
        ),
        (
            "DHE: 5-layer failure attribution — retrieval→context→dispatch→execution→reasoning. Targets ≥60% fix-prediction precision vs AHE 33.7%.",
            "design:professor-x",
        ),
        (
            "LCAP: UCB1 bandit over 5 context budget allocations per task type. c=1.414, round-level delta_p drives arm selection.",
            "design:professor-x",
        ),
        (
            "BF: Behavioral Fingerprint F(H_k)=[p_tool, p_plan, p_correct]. Non-uniform improvement across categories confirms H11.",
            "design:professor-x",
        ),
        (
            "MHE: Three levers — Lever1 parametric (SDAR QLoRA overnight), Lever2 contextual (ICE+MARS), Lever3 structural (DHE-guided evolution).",
            "design:professor-x",
        ),
        (
            "Externalization: Pattern B — working context in prompt, long-term in external store. Harness decides what to retrieve and when.",
            "paper:2604.08224",
        ),
        (
            "SLMs: qwen3:8b Q4 fits in 5.2GB VRAM, 42 tok/s, 32K context, thinking mode. Matches larger models on structured agentic tasks.",
            "paper:2506.02153",
        ),
        (
            "Hermes: Advance next_run_at BEFORE executing jobs, under file lock — at-most-once semantics.",
            "repo:hermes-agent",
        ),
        (
            "ClawOS: Merkle-chained audit log — each entry SHA-256 hashes the previous. verify_chain() at startup detects tampering.",
            "repo:clawos",
        ),
        (
            "ClawOS: Hook circuit breaker — 3 consecutive failures disables the hook to prevent blocking all tool calls.",
            "repo:clawos",
        ),
        (
            "Professor X design: Core modules (policyd gate, memd) require human approval for modification. Never autonomous.",
            "design:professor-x",
        ),
    ];

    seeds
        .iter()
        .map(|(content, source)| CognitionItem::new(content.to_string(), source.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work_loop_run(
        run_kind: &str,
        failed_cycles: u32,
        smoke_records: Vec<WorkLoopSmokeRecord>,
    ) -> WorkLoopRunRecord {
        let now = chrono::Utc::now();
        WorkLoopRunRecord {
            id: None,
            run_id: uuid::Uuid::new_v4().to_string(),
            run_kind: run_kind.to_string(),
            profile: "core".to_string(),
            started_at: now,
            completed_at: now,
            requested_cycles: smoke_records.len() as u32,
            completed_cycles: smoke_records.len() as u32,
            passed_cycles: smoke_records.iter().filter(|record| record.passed).count() as u32,
            failed_cycles,
            report_path: "artifacts/work-loop/test.json".to_string(),
            planned_jobs: Vec::new(),
            smoke_records,
            recorded_at: now,
        }
    }

    fn smoke(kind: &str, passed: bool) -> WorkLoopSmokeRecord {
        WorkLoopSmokeRecord {
            cycle: 1,
            kind: kind.to_string(),
            smoke_id: None,
            passed,
            report_path: "artifacts/test.json".to_string(),
            transcript_path: None,
            workspace: "/tmp/px".to_string(),
            detail: "test".to_string(),
        }
    }

    #[test]
    fn operator_help_surfaces_live_and_commit_commands() {
        let help = format_operator_help();

        assert!(help.contains("Professor X operator commands"));
        assert!(help.contains("--prof-x-live 5"));
        assert!(help.contains("--observe-work"));
        assert!(help.contains("--prof-x-code-live"));
        assert!(help.contains("--prof-x-code-patch-live"));
        assert!(help.contains("--prof-x-code-commit-live"));
        assert!(help.contains("--coding-sessions 5"));
        assert!(help.contains("--replay latest"));
        assert!(help.contains("--validate-artifacts"));
    }

    #[test]
    fn operator_plan_retries_latest_failed_gate_first() {
        let recent = vec![work_loop_run(
            "operator",
            1,
            vec![smoke("coding_smoke", true), smoke("evolution_smoke", false)],
        )];

        let plan =
            plan_work_loop_jobs(WorkLoopRunKind::Operator, WorkLoopProfile::Core, 3, &recent);

        assert_eq!(plan[0].kind, "evolution_smoke");
        assert!(plan[0].reason.contains("latest operator run failed"));
        assert_eq!(plan[1].kind, "coding_smoke");
        assert_eq!(plan.len(), 3);
    }

    #[test]
    fn supervised_plan_keeps_profile_rotation() {
        let recent = vec![work_loop_run(
            "operator",
            1,
            vec![smoke("evolution_smoke", false)],
        )];

        let plan = plan_work_loop_jobs(
            WorkLoopRunKind::Supervised,
            WorkLoopProfile::Core,
            4,
            &recent,
        );

        assert_eq!(plan[0].kind, "coding_smoke");
        assert_eq!(plan[1].kind, "evolution_smoke");
        assert_eq!(plan[2].kind, "hiro_smoke");
        assert_eq!(plan[3].kind, "proposal_dry_run");
    }

    #[test]
    fn commit_profile_includes_commit_gate_after_safety_gates() {
        let plan = plan_work_loop_jobs(WorkLoopRunKind::Operator, WorkLoopProfile::Commit, 6, &[]);

        assert_eq!(plan[0].kind, "coding_smoke");
        assert_eq!(plan[1].kind, "evolution_smoke");
        assert_eq!(plan[2].kind, "hiro_smoke");
        assert_eq!(plan[3].kind, "proposal_dry_run");
        assert_eq!(plan[4].kind, "patch_apply_commit");
        assert!(plan[4].reason.contains("generated patch"));
        assert_eq!(plan[5].kind, "operator_commit");
        assert!(plan[5].reason.contains("git commit"));
    }

    #[test]
    fn unified_new_file_diff_applies_as_patch() {
        let root = std::env::temp_dir().join(format!("px-diff-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let init = std::process::Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            init.status.success(),
            "{}",
            String::from_utf8_lossy(&init.stderr)
        );
        let diff = unified_new_file_diff(
            std::path::Path::new("notes/example.md"),
            "# example\n\nbody\n",
        );
        let patch_path = root.join("patch.diff");
        std::fs::write(&patch_path, diff).unwrap();

        let check = std::process::Command::new("git")
            .args(["apply", "--check"])
            .arg(&patch_path)
            .current_dir(&root)
            .output()
            .unwrap();

        assert!(
            check.status.success(),
            "{}",
            String::from_utf8_lossy(&check.stderr)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn autonomous_patch_apply_skill_body_has_sections() {
        let body = autonomous_patch_apply_skill_body("px-autonomous-patch-test");

        assert!(body.contains("# px-autonomous-patch-test"));
        assert!(body.contains("## Workflow"));
        assert!(body.contains("## Output Contract"));
        assert!(body.lines().count() > 10);
    }

    #[test]
    fn resolve_report_reference_maps_artifacts_under_professor_x() {
        let root = std::env::temp_dir().join(format!("px-report-ref-{}", uuid::Uuid::new_v4()));
        let resolved =
            resolve_report_reference(&root, "artifacts/work-loop/2026-05-31/loop-test.json");

        assert_eq!(
            resolved,
            root.join("professor-x")
                .join("artifacts")
                .join("work-loop")
                .join("2026-05-31")
                .join("loop-test.json")
        );
    }

    #[test]
    fn latest_work_loop_report_from_artifacts_prefers_latest_path() {
        let root = std::env::temp_dir().join(format!("px-loop-reports-{}", uuid::Uuid::new_v4()));
        let older = root
            .join("professor-x")
            .join("artifacts")
            .join("work-loop")
            .join("2026-05-30")
            .join("loop-235959.json");
        let newer = root
            .join("professor-x")
            .join("artifacts")
            .join("work-loop")
            .join("2026-05-31")
            .join("loop-000001.json");
        std::fs::create_dir_all(older.parent().unwrap()).unwrap();
        std::fs::create_dir_all(newer.parent().unwrap()).unwrap();
        std::fs::write(&older, "{}").unwrap();
        std::fs::write(&newer, "{}").unwrap();

        let latest = latest_work_loop_report_from_artifacts(&root).unwrap();

        assert_eq!(latest, newer);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn format_work_event_surfaces_loop_artifact_evidence() {
        let event = memd::events::AgentEvent {
            id: 42,
            timestamp: chrono::Utc::now(),
            session_id: None,
            task_id: None,
            event_type: "work_loop.cycle.passed".to_string(),
            summary: "operator cycle 5/5 passed".to_string(),
            payload: serde_json::json!({
                "run_id": "12345678-aaaa-bbbb-cccc-123456789abc",
                "cycle": 5,
                "cycles": 5,
                "job": "patch_apply_commit",
                "passed": true,
                "report_path": "professor-x/artifacts/evolution/patch-verifications/patch.json",
                "transcript_path": "professor-x/artifacts/transcripts/t.json",
                "commit": "abcdef1234567890",
                "detail": "5 checks, commit=abcdef1, diff_bytes=336",
            }),
        };

        let line = format_work_event(&event);

        assert!(line.contains("Passed gate"));
        assert!(line.contains("- #00042"));
        assert!(line.contains("run=12345678"));
        assert!(line.contains("cycle=5/5"));
        assert!(line.contains("job=patch_apply_commit"));
        assert!(line.contains("passed=true"));
        assert!(line.contains("report professor-x/artifacts/evolution/patch-verifications/patch.json"));
        assert!(line.contains("transcript professor-x/artifacts/transcripts/t.json"));
        assert!(line.contains("commit abcdef12"));
        assert!(line.contains("5 checks"));
    }

    #[test]
    fn work_timeline_entry_extracts_reviewable_event_fields() {
        let event = memd::events::AgentEvent {
            id: 45,
            timestamp: chrono::Utc::now(),
            session_id: None,
            task_id: Some("task-123456789".to_string()),
            event_type: "tool.started".to_string(),
            summary: "running tool 'shell.restricted' :: command=cargo test".to_string(),
            payload: serde_json::json!({
                "run_id": "12345678-aaaa-bbbb-cccc-123456789abc",
                "cycle": 2,
                "step": 3,
                "tool": "shell.restricted",
                "job": "coding_smoke",
                "params_preview": "command=cargo test",
            }),
        };

        let entry = work_timeline_entry(&event);

        assert_eq!(entry.event_id, 45);
        assert_eq!(entry.label, "TOOL");
        assert_eq!(entry.action, "Running");
        assert_eq!(entry.task_id.as_deref(), Some("task-123"));
        assert_eq!(entry.run_id.as_deref(), Some("12345678"));
        assert_eq!(entry.cycle, Some(2));
        assert_eq!(entry.step, Some(3));
        assert_eq!(entry.tool.as_deref(), Some("shell.restricted"));
        assert_eq!(entry.job.as_deref(), Some("coding_smoke"));
        assert_eq!(entry.detail.as_deref(), Some("command=cargo test"));
    }

    #[test]
    fn format_work_timeline_entry_is_operator_review_friendly() {
        let entry = WorkTimelineEntry {
            event_id: 7,
            timestamp: chrono::Utc::now().to_rfc3339(),
            label: "TOOL".to_string(),
            action: "Running".to_string(),
            task_id: Some("task-123".to_string()),
            run_id: Some("12345678".to_string()),
            cycle: Some(1),
            step: Some(2),
            tool: Some("fs.replace".to_string()),
            job: Some("coding_smoke".to_string()),
            passed: None,
            summary: "running tool 'fs.replace' :: path=src/lib.rs mode=apply".to_string(),
            detail: Some("path=src/lib.rs mode=apply".to_string()),
            report_path: None,
            transcript_path: None,
            artifacts: Vec::new(),
        };

        let line = format_work_timeline_entry(&entry);

        assert!(line.contains("#00007"));
        assert!(line.contains("TOOL"));
        assert!(line.contains("Running"));
        assert!(line.contains("task=task-123"));
        assert!(line.contains("step=2"));
        assert!(line.contains("tool=fs.replace"));
        assert!(line.contains("path=src/lib.rs mode=apply"));
    }

    #[test]
    fn format_work_replay_entry_groups_metadata_and_proofs() {
        let entry = WorkTimelineEntry {
            event_id: 8,
            timestamp: chrono::Utc::now().to_rfc3339(),
            label: "SMOKE".to_string(),
            action: "Passed coding smoke".to_string(),
            task_id: Some("abcd1234".to_string()),
            run_id: Some("12345678".to_string()),
            cycle: Some(1),
            step: None,
            tool: None,
            job: Some("coding_smoke".to_string()),
            passed: Some(true),
            summary: "coding smoke report written to artifacts/coding-smoke/report.json".to_string(),
            detail: Some("deterministic coding smoke".to_string()),
            report_path: Some("artifacts/coding-smoke/report.json".to_string()),
            transcript_path: Some("artifacts/transcripts/task.json".to_string()),
            artifacts: vec!["artifacts/commands/cargo-test.json".to_string()],
        };

        let line = format_work_replay_entry(&entry);

        assert!(line.contains("- #00008"));
        assert!(line.contains("Passed coding smoke"));
        assert!(line.contains("task=abcd1234"));
        assert!(line.contains("job=coding_smoke"));
        assert!(line.contains("passed=true"));
        assert!(line.contains("L report artifacts/coding-smoke/report.json"));
        assert!(line.contains("L transcript artifacts/transcripts/task.json"));
        assert!(line.contains("L artifact artifacts/commands/cargo-test.json"));
    }

    #[test]
    fn format_work_loop_ledger_links_plan_outcomes_and_timeline() {
        let report = SupervisedLoopReport {
            run_id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            run_kind: "operator".to_string(),
            started_at: "2026-06-01T01:00:00Z".to_string(),
            completed_at: "2026-06-01T01:01:00Z".to_string(),
            requested_cycles: 1,
            completed_cycles: 1,
            passed_cycles: 1,
            failed_cycles: 0,
            profile: "core".to_string(),
            ledger_path: None,
            planned_jobs: vec![WorkLoopPlannedJob {
                cycle: 1,
                kind: "coding_smoke".to_string(),
                label: "coding smoke".to_string(),
                reason: "prove local coding-agent edit and verification".to_string(),
            }],
            smoke_records: vec![WorkLoopSmokeRecord {
                cycle: 1,
                kind: "coding_smoke".to_string(),
                smoke_id: None,
                passed: true,
                report_path: "artifacts/coding-smoke/report.json".to_string(),
                transcript_path: Some("artifacts/transcripts/task.json".to_string()),
                workspace: "/tmp/px".to_string(),
                detail: "deterministic coding smoke".to_string(),
            }],
            timeline: vec![WorkTimelineEntry {
                event_id: 8,
                timestamp: "2026-06-01T01:00:01Z".to_string(),
                label: "TOOL".to_string(),
                action: "Running".to_string(),
                task_id: Some("abcd1234".to_string()),
                run_id: Some("12345678".to_string()),
                cycle: Some(1),
                step: Some(1),
                tool: Some("shell.restricted".to_string()),
                job: Some("coding_smoke".to_string()),
                passed: None,
                summary: "running tool 'shell.restricted' :: command=cargo test".to_string(),
                detail: Some("command=cargo test".to_string()),
                report_path: None,
                transcript_path: None,
                artifacts: Vec::new(),
            }],
        };

        let ledger = format_work_loop_ledger(
            &report,
            std::path::Path::new("artifacts/work-loop/2026-06-01/loop.json"),
        );

        assert!(ledger.contains("# Professor X Run 12345678"));
        assert!(ledger.contains("- kind: `operator`"));
        assert!(ledger.contains("cycle 1: `coding_smoke`"));
        assert!(ledger.contains("cycle 1 `coding_smoke`: passed"));
        assert!(ledger.contains("report: `artifacts/coding-smoke/report.json`"));
        assert!(ledger.contains("transcript: `artifacts/transcripts/task.json`"));
        assert!(ledger.contains("#00008 `TOOL` `Running`"));
    }

    #[test]
    fn format_run_log_entry_points_to_review_replay_and_ledger() {
        let mut run = work_loop_run("operator", 0, vec![smoke("coding_smoke", true)]);
        run.run_id = "12345678-aaaa-bbbb-cccc-123456789abc".to_string();

        let line = format_run_log_entry(
            &run,
            Some("artifacts/work-loop/ledger/2026-06-01/run-12345678.md"),
        );

        assert!(line.contains("operator:core"));
        assert!(line.contains("run=12345678"));
        assert!(line.contains("passed=1 failed=0 passed"));
        assert!(line.contains("last_gate=coding_smoke passed"));
        assert!(line.contains("L report artifacts/work-loop/test.json"));
        assert!(line.contains("L ledger artifacts/work-loop/ledger/2026-06-01/run-12345678.md"));
        assert!(line.contains("cargo run -- --replay 12345678"));
        assert!(line.contains("cargo run -- --run-review 12345678"));
    }

    #[test]
    fn publishable_run_artifact_paths_only_allows_work_loop_report_and_ledger() {
        let root = std::env::temp_dir().join(format!("px-publish-paths-{}", uuid::Uuid::new_v4()));
        let report_path = root
            .join("professor-x")
            .join("artifacts")
            .join("work-loop")
            .join("2026-06-01")
            .join("loop-010000.json");
        let ledger_path = root
            .join("professor-x")
            .join("artifacts")
            .join("work-loop")
            .join("ledger")
            .join("2026-06-01")
            .join("run-12345678.md");
        let smoke_path = root
            .join("professor-x")
            .join("artifacts")
            .join("coding-smoke")
            .join("2026-06-01")
            .join("smoke-010000.json");
        let event_path = root
            .join("professor-x")
            .join("artifacts")
            .join("events")
            .join("2026-06-01.jsonl");
        std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(ledger_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(smoke_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(event_path.parent().unwrap()).unwrap();
        std::fs::write(&report_path, "{}").unwrap();
        std::fs::write(&ledger_path, "# run\n").unwrap();
        std::fs::write(&smoke_path, "{}").unwrap();
        std::fs::write(&event_path, "{}\n").unwrap();
        let report = SupervisedLoopReport {
            run_id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            run_kind: "operator".to_string(),
            started_at: "2026-06-01T01:00:00Z".to_string(),
            completed_at: "2026-06-01T01:01:00Z".to_string(),
            requested_cycles: 1,
            completed_cycles: 1,
            passed_cycles: 1,
            failed_cycles: 0,
            profile: "core".to_string(),
            ledger_path: Some(ledger_path.display().to_string()),
            planned_jobs: Vec::new(),
            smoke_records: vec![WorkLoopSmokeRecord {
                cycle: 1,
                kind: "coding_smoke".to_string(),
                smoke_id: None,
                passed: true,
                report_path: smoke_path.display().to_string(),
                transcript_path: Some("/tmp/outside-transcript.json".to_string()),
                workspace: "/tmp/px".to_string(),
                detail: "test".to_string(),
            }],
            timeline: Vec::new(),
        };

        let paths = publishable_run_artifact_paths(&root, &report_path, &report).unwrap();

        assert_eq!(paths.len(), 4);
        assert!(paths.iter().any(|path| path.ends_with("loop-010000.json")));
        assert!(paths.iter().any(|path| path.ends_with("run-12345678.md")));
        assert!(paths.iter().any(|path| path.ends_with("smoke-010000.json")));
        assert!(paths.iter().any(|path| path.ends_with("2026-06-01.jsonl")));
        assert!(publishable_run_artifact_path(std::path::Path::new(
            "professor-x/artifacts/work-loop/2026-06-01/loop-010000.json"
        )));
        assert!(publishable_run_artifact_path(std::path::Path::new(
            "professor-x/artifacts/transcripts/2026-06-01/task.json"
        )));
        assert!(publishable_run_artifact_path(std::path::Path::new(
            "professor-x/artifacts/events/2026-06-01.jsonl"
        )));
        assert!(!publishable_run_artifact_path(std::path::Path::new(
            "professor-x/src/main.rs"
        )));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn format_work_event_surfaces_running_tool_preview() {
        let event = memd::events::AgentEvent {
            id: 43,
            timestamp: chrono::Utc::now(),
            session_id: None,
            task_id: Some("task-123456789".to_string()),
            event_type: "tool.started".to_string(),
            summary: "running tool 'shell.restricted' :: command=cargo test".to_string(),
            payload: serde_json::json!({
                "step": 2,
                "tool": "shell.restricted",
                "params_preview": "command=cargo test",
            }),
        };

        let line = format_work_event(&event);

        assert!(line.contains("Running"));
        assert!(line.contains("tool=shell.restricted"));
        assert!(line.contains("step=2"));
        assert!(line.contains("detail command=cargo test"));
    }

    #[test]
    fn format_live_task_event_surfaces_running_tool_preview() {
        let event = memd::events::AgentEvent {
            id: 44,
            timestamp: chrono::Utc::now(),
            session_id: None,
            task_id: Some("task-123456789".to_string()),
            event_type: "tool.started".to_string(),
            summary: "running tool 'fs.replace' :: path=src/lib.rs mode=apply".to_string(),
            payload: serde_json::json!({
                "step": 3,
                "tool": "fs.replace",
                "params_preview": "path=src/lib.rs mode=apply",
            }),
        };

        let line = format_live_task_event(&event).unwrap();

        assert_eq!(line, "  tool fs.replace: running - path=src/lib.rs mode=apply");
    }

    #[test]
    fn format_work_cockpit_surfaces_run_gate_and_trace() {
        let now = chrono::Utc::now();
        let run = WorkLoopRunRecord {
            id: Some(7),
            run_id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            run_kind: "operator".to_string(),
            profile: "core".to_string(),
            started_at: now,
            completed_at: now,
            requested_cycles: 2,
            completed_cycles: 1,
            passed_cycles: 1,
            failed_cycles: 0,
            report_path: "artifacts/work-loop/2026-05-31/loop.json".to_string(),
            planned_jobs: vec![WorkLoopPlannedJob {
                cycle: 1,
                kind: "coding_smoke".to_string(),
                label: "coding smoke".to_string(),
                reason: "prove local coding-agent edit and verification".to_string(),
            }],
            smoke_records: vec![WorkLoopSmokeRecord {
                cycle: 1,
                kind: "coding_smoke".to_string(),
                smoke_id: None,
                passed: true,
                report_path: "artifacts/coding-smoke/report.json".to_string(),
                transcript_path: Some("artifacts/transcripts/task.json".to_string()),
                workspace: "/tmp/professor-x-work".to_string(),
                detail: "deterministic coding smoke".to_string(),
            }],
            recorded_at: now,
        };
        let gate = WorkLoopGateRecord {
            id: Some(9),
            run_id: run.run_id.clone(),
            run_kind: "operator".to_string(),
            profile: "core".to_string(),
            cycle: 1,
            kind: "coding_smoke".to_string(),
            label: "coding smoke".to_string(),
            reason: "prove local coding-agent edit and verification".to_string(),
            status: "passed".to_string(),
            started_at: Some(now),
            completed_at: Some(now),
            passed: Some(true),
            report_path: Some("artifacts/coding-smoke/report.json".to_string()),
            transcript_path: Some("artifacts/transcripts/task.json".to_string()),
            workspace: Some("/tmp/professor-x-work".to_string()),
            detail: "deterministic coding smoke".to_string(),
            recorded_at: now,
            updated_at: now,
        };
        let event = memd::events::AgentEvent {
            id: 10,
            timestamp: now,
            session_id: None,
            task_id: None,
            event_type: "work_loop.cycle.passed".to_string(),
            summary: "Prof X operator run cycle 1/2 passed".to_string(),
            payload: serde_json::json!({
                "run_id": run.run_id.clone(),
                "cycle": 1,
                "cycles": 2,
                "job": "coding_smoke",
                "passed": true,
                "report_path": "artifacts/coding-smoke/report.json",
                "transcript_path": "artifacts/transcripts/task.json",
                "detail": "deterministic coding smoke",
            }),
        };

        let screen = format_work_cockpit(
            std::path::Path::new("."),
            &[event],
            Some(&run),
            Some(&gate),
            std::slice::from_ref(&gate),
        );

        assert!(screen.contains("Professor X live work cockpit"));
        assert!(screen.contains("state IDLE"));
        assert!(screen.contains("progress [######......] 1/2"));
        assert!(screen.contains("operator:core run=12345678"));
        assert!(screen.contains("commands replay=--replay 12345678"));
        assert!(screen.contains("Evidence bundle"));
        assert!(screen.contains("proof report artifacts/coding-smoke/report.json"));
        assert!(screen.contains("proof transcript artifacts/transcripts/task.json"));
        assert!(screen.contains("Recent signal events=1"));
        assert!(screen.contains("Passed gate"));
        assert!(screen.contains("--cockpit"));
        assert!(screen.contains("--prof-x-live-publish 6"));
        assert!(screen.contains("--run-review latest"));
    }

    #[test]
    fn cockpit_state_and_progress_are_operator_readable() {
        let now = chrono::Utc::now();
        let mut run = WorkLoopRunRecord {
            id: None,
            run_id: "run-1".to_string(),
            run_kind: "operator".to_string(),
            profile: "commit".to_string(),
            started_at: now,
            completed_at: now,
            requested_cycles: 6,
            completed_cycles: 6,
            passed_cycles: 6,
            failed_cycles: 0,
            report_path: "artifacts/work-loop/report.json".to_string(),
            planned_jobs: Vec::new(),
            smoke_records: Vec::new(),
            recorded_at: now,
        };
        assert_eq!(cockpit_state(Some(&run), None), "READY");
        assert_eq!(cockpit_progress(6, 6), "[############] 6/6");

        run.failed_cycles = 1;
        assert_eq!(cockpit_state(Some(&run), None), "NEEDS-REVIEW");

        let gate = WorkLoopGateRecord {
            id: None,
            run_id: run.run_id.clone(),
            run_kind: run.run_kind.clone(),
            profile: run.profile.clone(),
            cycle: 3,
            kind: "hiro_smoke".to_string(),
            label: "HIRO smoke".to_string(),
            reason: "verify benchmark inventory".to_string(),
            status: "running".to_string(),
            started_at: Some(now),
            completed_at: None,
            passed: None,
            report_path: None,
            transcript_path: None,
            workspace: None,
            detail: "running benchmark inventory".to_string(),
            recorded_at: now,
            updated_at: now,
        };
        assert_eq!(cockpit_state(Some(&run), Some(&gate)), "RUNNING");
        assert_eq!(cockpit_progress(3, 6), "[######......] 3/6");
    }

    #[test]
    fn coding_session_commit_hint_reads_commit_outcome() {
        let now = chrono::Utc::now();
        let session = CodingSessionRecord {
            id: "session-1".to_string(),
            generated_at: now,
            goal: "verify and commit patch".to_string(),
            exercise: "repo_patch_apply_commit".to_string(),
            status: "passed".to_string(),
            workspace: Some("repo-root".to_string()),
            smoke_id: None,
            smoke_report_path: None,
            session_report_path: "artifacts/coding-sessions/session.json".to_string(),
            transcript_path: None,
            artifacts: vec!["artifacts/evolution/patch.json".to_string()],
            checks: vec!["git_commit".to_string()],
            plan_steps: Vec::new(),
            step_outcomes: vec![
                "main apply committed".to_string(),
                "commit eedcd3e123456789".to_string(),
            ],
            failure_reason: None,
            recorded_at: now,
        };

        assert_eq!(coding_session_commit_hint(&session).as_deref(), Some("eedcd3e1"));
    }

    #[test]
    fn changed_paths_from_unified_diff_reads_added_modified_and_deleted_paths() {
        let diff = r#"diff --git a/professor-x/skills/old.md b/professor-x/skills/old.md
deleted file mode 100644
index 1111111..0000000
--- a/professor-x/skills/old.md
+++ /dev/null
diff --git a/professor-x/skills/new.md b/professor-x/skills/new.md
new file mode 100644
index 0000000..2222222
--- /dev/null
+++ b/professor-x/skills/new.md
diff --git a/professor-x/src/main.rs b/professor-x/src/main.rs
index 3333333..4444444 100644
--- a/professor-x/src/main.rs
+++ b/professor-x/src/main.rs
"#;

        let paths = changed_paths_from_unified_diff(diff).unwrap();

        assert_eq!(
            paths,
            vec![
                PathBuf::from("professor-x/skills/new.md"),
                PathBuf::from("professor-x/skills/old.md"),
                PathBuf::from("professor-x/src/main.rs"),
            ]
        );
    }

    #[test]
    fn changed_paths_from_unified_diff_rejects_parent_paths() {
        let diff = r#"diff --git a/professor-x/src/main.rs b/../outside.rs
index 3333333..4444444 100644
--- a/professor-x/src/main.rs
+++ b/../outside.rs
"#;

        let err = changed_paths_from_unified_diff(diff).unwrap_err();

        assert!(err.to_string().contains("unsafe path"));
    }
}
