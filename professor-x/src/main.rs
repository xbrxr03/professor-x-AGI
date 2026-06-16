mod agentd;
mod artifacts;
mod embeddings;
mod evolved;
mod failure;
mod local_embed;
mod memd;
mod observer;
mod ollama;
mod policyd;
mod serve;
mod toolbridge;
mod tui;
mod util;

use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashMap};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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
use evolved::cognition_base::CognitionItem;
use evolved::hiro::load_task_inventory;
use evolved::proposer::{ChangeManifest, EvolutionNode, HarnessComponent, VerificationStatus};
use evolved::tracker::{OutcomeTracker, TaskOutcome};
use evolved::verify_diff_in_sandbox;
use evolved::verify_node_in_sandbox;
use evolved::CognitionStore;
use evolved::{EvolvedLoop, HiroRunner};
use failure::{classify_failure_mode, normalize_failure_mode, FailureClass};
use memd::autonomy_queue::{autonomy_queue_brief, AutonomyQueueItem, AutonomyQueueStore};
use memd::coding_sessions::{
    display_status as coding_session_display_status, stale_candidate, CodingSessionRecord,
    CodingSessionStaleCandidate, CodingSessionStore,
};
use memd::coding_smoke::{CodingSmokeRecord, CodingSmokeStore};
use memd::events::EventStore;
use memd::pinned::PinnedEntry;
use memd::task_runs::{TaskRun, TaskRunStore};
use memd::transcripts::{TranscriptStore, TranscriptSummary};
use memd::work_loops::{
    WorkLoopGateRecord, WorkLoopGateStore, WorkLoopPlannedJob, WorkLoopRunRecord, WorkLoopRunStore,
    WorkLoopSmokeRecord,
};
use memd::MemoryManager;
use policyd::{AuditStore, Decision, PermissionScope, PolicyEngine};
use toolbridge::executor::{Action, Observation};
use toolbridge::shell_sandbox::shell_sandbox_posture_line;
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
    /// Print a machine-readable one-shot Prof X work status JSON document.
    status_json: bool,
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
    /// Print one compact Prof X operator brief and exit.
    brief: bool,
    /// Write a Markdown journal from recent Prof X work events and exit.
    prof_x_journal_limit: Option<usize>,
    /// Write and commit a Markdown journal from recent Prof X work events.
    prof_x_journal_commit_limit: Option<usize>,
    /// Print the consciousness measurement report (phi, interoception, self-prediction, ICS, ...) and exit.
    consciousness_report: bool,
    /// Print a detailed autonomous/work-loop run review by run id prefix, report path, or 'latest'.
    run_review: Option<String>,
    /// Replay a work/operator run timeline by run id prefix, report path, or 'latest'.
    run_replay: Option<String>,
    /// Commit one run's report and ledger artifacts by run id prefix, report path, or 'latest'.
    publish_run: Option<String>,
    /// Print a task transcript review by task id prefix, or 'latest'.
    task_review: Option<String>,
    /// Print a task evidence bundle: run row, transcript, artifact verdicts, and work events.
    task_evidence: Option<String>,
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
    /// Rebuild the release binary and re-exec into it (close the evolve→apply loop, no restart).
    self_rebuild_reexec: bool,
    /// Rebuild-only safety gate: confirm the committed tree builds release-clean; never re-exec.
    self_rebuild_check: bool,
    /// Report whether an accepted autonomous commit still holds (vs reverted/missing) against HEAD.
    rollback_verdict_commit: Option<String>,
    /// Internal: target of a hot-reload re-exec; prints the generation and exits (probe).
    self_reload_probe: bool,
    repo_fix_bench: bool,
    evolve_on_repofix: Option<u32>,
    evolve_skill_on_repofix: Option<u32>,
    evolve_code_on_repofix: Option<u32>,
    evolve_code_target: Option<String>,
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
    /// Generate a constrained skill patch from a short operator goal and verify it live.
    skill_patch_live_goal: Option<String>,
    /// Generate a constrained skill patch from a short operator goal, verify it, and commit it live.
    skill_patch_commit_live_goal: Option<String>,
    /// Print the last N coding-agent sessions and exit.
    coding_sessions_limit: Option<usize>,
    /// Reconcile stale coding-agent sessions that never wrote a terminal report.
    repair_coding_sessions_limit: Option<usize>,
    /// Review one coding-agent session by id prefix, or 'latest'.
    coding_session_review: Option<String>,
    /// Publish one coding-agent session evidence bundle by id prefix, or 'latest'.
    coding_session_publish: Option<String>,
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
    /// Print the last N persisted autonomous queue items and exit.
    autonomy_queue_limit: Option<usize>,
    /// Review a persisted autonomous queue item by id prefix, or latest.
    autonomy_queue_review: Option<String>,
    /// Replay the run associated with a queue item by id prefix, or latest.
    autonomy_queue_replay: Option<String>,
    /// Publish the run artifacts associated with a queue item by id prefix, or latest.
    autonomy_queue_publish: Option<String>,
    /// Enqueue a bounded operator-run goal for later autonomous execution.
    autonomy_enqueue_goal: Option<String>,
    /// Profile to use for an operator-enqueued autonomous goal.
    autonomy_enqueue_profile: WorkLoopProfile,
    /// Plan and enqueue the next autonomous work item without executing it.
    autonomy_plan: bool,
    /// Preview the next autonomous queue step without enqueueing or executing it.
    autonomy_preview: bool,
    /// Run N persisted autonomous queue items, seeding one default item if empty.
    autonomy_step_count: Option<u32>,
    /// Run N persisted autonomous queue items while streaming work events.
    autonomy_step_live_count: Option<u32>,
    /// Run one sandbox-verified autonomous commit smoke and exit.
    operator_commit_smoke: bool,
    /// Run one seeded autonomous evolution cycle and exit.
    evolution_cycle: bool,
    /// Run one evolution cycle learning from REAL recent task outcomes.
    evolve_live: bool,
    /// Continuous evolution mining: evolve → measure → keep-if-better → rollback,
    /// repeated. Some(0) = unbounded; Some(n) = n blocks.
    evolve_forever: Option<u32>,
    /// Run up to N of the agent's own self-authored tests and score them.
    run_self_tests: Option<usize>,
    /// Author N diverse self-curriculum tasks (across HIRO categories), grounded
    /// in the real tool set, and store them for --run-self-tests to execute.
    /// Breaks the distillation-corpus ceiling of the fixed 60-task benchmark.
    generate_curriculum: Option<usize>,
    /// Override the local generation model (any Ollama model). Default: the
    /// largest model you have installed (VRAM-aware by proxy), else qwen3:8b.
    model: Option<String>,
    proposer_model: Option<String>,
    /// Launch the interactive full-screen TUI cockpit.
    tui: bool,
    /// Launch the local web UI server (http://127.0.0.1:8787).
    serve: bool,
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
    parse_args_from(std::env::args())
}

fn parse_args_from<I, S>(args: I) -> CliArgs
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    let mut cli = CliArgs {
        operator_help: false,
        consciousness_report: false,
        task: None,
        chat: false,
        run_now: false,
        hiro_round: None,
        hiro_limit: None,
        hiro_null_rounds: None,
        memory_budget: None,
        dry_run_daily: false,
        status: false,
        status_json: false,
        events_limit: None,
        work_feed_limit: None,
        transcripts_limit: None,
        task_runs_limit: None,
        work_loops_limit: None,
        run_log_limit: None,
        brief: false,
        prof_x_journal_limit: None,
        prof_x_journal_commit_limit: None,
        run_review: None,
        run_replay: None,
        publish_run: None,
        task_review: None,
        task_evidence: None,
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
        self_rebuild_reexec: false,
        self_rebuild_check: false,
        rollback_verdict_commit: None,
        self_reload_probe: false,
        repo_fix_bench: false,
        evolve_on_repofix: None,
        evolve_skill_on_repofix: None,
        evolve_code_on_repofix: None,
        evolve_code_target: None,
        coding_smoke: false,
        coding_session: false,
        coding_session_live: false,
        coding_session_goal: None,
        repo_patch_path: None,
        repo_patch_live_path: None,
        repo_patch_commit_path: None,
        repo_patch_commit_live_path: None,
        skill_patch_live_goal: None,
        skill_patch_commit_live_goal: None,
        coding_sessions_limit: None,
        repair_coding_sessions_limit: None,
        coding_session_review: None,
        coding_session_publish: None,
        supervised_loop_cycles: None,
        supervised_loop_profile: WorkLoopProfile::Basic,
        operator_run_cycles: None,
        operator_run_commit_cycles: None,
        operator_run_live_cycles: None,
        publish_after_run: false,
        autonomous_run_cycles: None,
        autonomous_run_commit_cycles: None,
        autonomy_queue_limit: None,
        autonomy_queue_review: None,
        autonomy_queue_replay: None,
        autonomy_queue_publish: None,
        autonomy_enqueue_goal: None,
        autonomy_enqueue_profile: WorkLoopProfile::Core,
        autonomy_plan: false,
        autonomy_preview: false,
        autonomy_step_count: None,
        autonomy_step_live_count: None,
        operator_commit_smoke: false,
        evolution_cycle: false,
        evolve_live: false,
        evolve_forever: None,
        run_self_tests: None,
        generate_curriculum: None,
        model: None,
        proposer_model: None,
        tui: false,
        serve: false,
        validate_artifacts: false,
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" | "--prof-x-help" | "--operator-help" | "--commands" => {
                cli.operator_help = true;
                i += 1;
            }
            "--task" if i + 1 < args.len() => {
                cli.task = Some(args[i + 1].clone());
                i += 2;
            }
            "--chat" | "--prof-x-chat" | "--talk" | "--task-interactive" => {
                cli.chat = true;
                i += 1;
            }
            "--run-now" => {
                cli.run_now = true;
                i += 1;
            }
            "--proposer-model" if i + 1 < args.len() => {
                cli.proposer_model = Some(args[i + 1].clone());
                i += 2;
            }
            "--model" if i + 1 < args.len() => {
                cli.model = Some(args[i + 1].clone());
                i += 2;
            }
            "--tui" | "--cockpit-live" => {
                cli.tui = true;
                i += 1;
            }
            "--serve" | "--web" => {
                cli.serve = true;
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
            "--status-json" | "--prof-x-status-json" | "--work-status-json" => {
                cli.status_json = true;
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
            "--consciousness-report" | "--phi" | "--prof-x-mind" => {
                cli.consciousness_report = true;
                i += 1;
            }
            "--brief" | "--prof-x-brief" | "--now-brief" => {
                cli.brief = true;
                i += 1;
            }
            "--prof-x-journal" | "--work-journal" | "--live-journal" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.prof_x_journal_limit = Some(limit.unwrap_or(50));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--prof-x-journal-commit" | "--work-journal-commit" | "--commit-work-journal" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.prof_x_journal_commit_limit = Some(limit.unwrap_or(50));
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
            "--task-review" => {
                let value = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                let has_value = value.is_some();
                cli.task_review = Some(value.unwrap_or_else(|| "latest".to_string()));
                i += if has_value { 2 } else { 1 };
            }
            "--task-evidence"
            | "--prof-x-task-evidence"
            | "--task-bundle"
            | "--inspect"
            | "--evidence" => {
                let value = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                let has_value = value.is_some();
                cli.task_evidence = Some(value.unwrap_or_else(|| "latest".to_string()));
                i += if has_value { 2 } else { 1 };
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
            "--self-rebuild-reexec" | "--hot-reload" => {
                cli.self_rebuild_reexec = true;
                i += 1;
            }
            "--self-rebuild-check" | "--verify-rebuild" => {
                cli.self_rebuild_check = true;
                i += 1;
            }
            "--rollback-verdict" if i + 1 < args.len() => {
                cli.rollback_verdict_commit = Some(args[i + 1].clone());
                i += 2;
            }
            "--self-reload-probe" => {
                cli.self_reload_probe = true;
                i += 1;
            }
            "--repo-fix-bench" => {
                cli.repo_fix_bench = true;
                i += 1;
            }
            "--evolve-on-repofix" if i + 1 < args.len() => {
                cli.evolve_on_repofix = args[i + 1].parse::<u32>().ok();
                i += 2;
            }
            "--evolve-skill-on-repofix" if i + 1 < args.len() => {
                cli.evolve_skill_on_repofix = args[i + 1].parse::<u32>().ok();
                i += 2;
            }
            "--evolve-code-on-repofix" if i + 1 < args.len() => {
                cli.evolve_code_on_repofix = args[i + 1].parse::<u32>().ok();
                i += 2;
            }
            "--evolve-code-target" if i + 1 < args.len() => {
                cli.evolve_code_target = Some(args[i + 1].clone());
                i += 2;
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
            "--skill-patch-live" | "--prof-x-skill-live" if i + 1 < args.len() => {
                cli.skill_patch_live_goal = Some(args[i + 1].clone());
                i += 2;
            }
            "--skill-patch-commit-live" | "--prof-x-skill-commit-live" if i + 1 < args.len() => {
                cli.skill_patch_commit_live_goal = Some(args[i + 1].clone());
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
            "--repair-coding-sessions" | "--prof-x-code-repair" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.repair_coding_sessions_limit = Some(limit.unwrap_or(10));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--coding-session-review" | "--prof-x-code-review" | "--session-review" => {
                let value = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                let has_value = value.is_some();
                cli.coding_session_review = Some(value.unwrap_or_else(|| "latest".to_string()));
                i += if has_value { 2 } else { 1 };
            }
            "--coding-session-publish" | "--prof-x-code-publish" | "--session-publish" => {
                let value = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                let has_value = value.is_some();
                cli.coding_session_publish = Some(value.unwrap_or_else(|| "latest".to_string()));
                i += if has_value { 2 } else { 1 };
            }
            "--autonomy-queue" | "--prof-x-queue" => {
                let limit = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<usize>().ok());
                cli.autonomy_queue_limit = Some(limit.unwrap_or(10));
                i += if limit.is_some() { 2 } else { 1 };
            }
            "--autonomy-queue-review" | "--prof-x-queue-review" => {
                let value = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                let has_value = value.is_some();
                cli.autonomy_queue_review = Some(value.unwrap_or_else(|| "latest".to_string()));
                i += if has_value { 2 } else { 1 };
            }
            "--autonomy-queue-replay" | "--prof-x-queue-replay" => {
                let value = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                let has_value = value.is_some();
                cli.autonomy_queue_replay = Some(value.unwrap_or_else(|| "latest".to_string()));
                i += if has_value { 2 } else { 1 };
            }
            "--autonomy-queue-publish" | "--prof-x-queue-publish" => {
                let value = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .cloned();
                let has_value = value.is_some();
                cli.autonomy_queue_publish = Some(value.unwrap_or_else(|| "latest".to_string()));
                i += if has_value { 2 } else { 1 };
            }
            "--autonomy-enqueue" | "--prof-x-enqueue" if i + 1 < args.len() => {
                cli.autonomy_enqueue_goal = Some(args[i + 1].clone());
                cli.autonomy_enqueue_profile = WorkLoopProfile::Core;
                i += 2;
            }
            "--autonomy-enqueue-commit" | "--prof-x-enqueue-commit" if i + 1 < args.len() => {
                cli.autonomy_enqueue_goal = Some(args[i + 1].clone());
                cli.autonomy_enqueue_profile = WorkLoopProfile::Commit;
                i += 2;
            }
            "--autonomy-plan" | "--prof-x-plan" => {
                cli.autonomy_plan = true;
                i += 1;
            }
            "--autonomy-preview" | "--prof-x-preview-step" | "--prof-x-preview" => {
                cli.autonomy_preview = true;
                i += 1;
            }
            "--autonomy-step" | "--prof-x-step" => {
                let count = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.autonomy_step_count = Some(count.unwrap_or(1));
                i += if count.is_some() { 2 } else { 1 };
            }
            "--autonomy-step-publish" | "--prof-x-step-publish" => {
                let count = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.autonomy_step_count = Some(count.unwrap_or(1));
                cli.publish_after_run = true;
                i += if count.is_some() { 2 } else { 1 };
            }
            "--autonomy-step-live" | "--prof-x-step-live" => {
                let count = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.autonomy_step_live_count = Some(count.unwrap_or(1));
                i += if count.is_some() { 2 } else { 1 };
            }
            "--autonomy-step-publish-live" | "--prof-x-step-publish-live" => {
                let count = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.autonomy_step_live_count = Some(count.unwrap_or(1));
                cli.publish_after_run = true;
                i += if count.is_some() { 2 } else { 1 };
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
                cli.operator_run_commit_cycles = Some(cycles.unwrap_or(6));
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--operator-run-publish" | "--operator-run-commit-publish" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.operator_run_commit_cycles = Some(cycles.unwrap_or(6));
                cli.publish_after_run = true;
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--operator-run-live" | "--operator-run-commit-live" | "--prof-x-live" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.operator_run_live_cycles = Some(cycles.unwrap_or(6));
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--operator-run-publish-live"
            | "--operator-run-commit-publish-live"
            | "--prof-x-live-publish" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.operator_run_live_cycles = Some(cycles.unwrap_or(6));
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
                cli.autonomous_run_commit_cycles = Some(cycles.unwrap_or(6));
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
                cli.autonomous_run_commit_cycles = Some(cycles.unwrap_or(6));
                cli.publish_after_run = true;
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--operator-commit-smoke" => {
                cli.operator_commit_smoke = true;
                i += 1;
            }
            "--evolve" | "--evolve-live" => {
                cli.evolve_live = true;
                i += 1;
            }
            "--evolve-forever" | "--mine" => {
                let iters = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|n| n.parse::<u32>().ok());
                let has_value = iters.is_some();
                cli.evolve_forever = Some(iters.unwrap_or(0)); // 0 = unbounded
                i += if has_value { 2 } else { 1 };
            }
            "--run-self-tests" | "--self-tests" => {
                let n = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|v| v.parse::<usize>().ok());
                let has_value = n.is_some();
                cli.run_self_tests = Some(n.unwrap_or(10));
                i += if has_value { 2 } else { 1 };
            }
            "--generate-curriculum" | "--curriculum" => {
                let n = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|v| v.parse::<usize>().ok());
                let has_value = n.is_some();
                cli.generate_curriculum = Some(n.unwrap_or(20));
                i += if has_value { 2 } else { 1 };
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
        || cli.status_json
        || cli.events_limit.is_some()
        || cli.work_feed_limit.is_some()
        || cli.transcripts_limit.is_some()
        || cli.task_runs_limit.is_some()
        || cli.coding_sessions_limit.is_some()
        || cli.repair_coding_sessions_limit.is_some()
        || cli.coding_session_review.is_some()
        || cli.autonomy_queue_limit.is_some()
        || cli.autonomy_enqueue_goal.is_some()
        || cli.autonomy_plan
        || cli.work_loops_limit.is_some()
        || cli.run_log_limit.is_some()
        || cli.brief
        || cli.consciousness_report
        || cli.run_review.is_some()
        || cli.task_review.is_some()
        || cli.task_evidence.is_some()
        || cli.run_replay.is_some()
        || cli.publish_run.is_some()
        || cli.watch
        || cli.watch_work
        || cli.work_cockpit_once
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

    if cli.status_json {
        return print_work_status_json(Arc::clone(&memory), Arc::clone(&events), 16);
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

    if let Some(limit) = cli.repair_coding_sessions_limit {
        return repair_stale_coding_sessions(Arc::clone(&memory), Arc::clone(&events), limit);
    }

    if let Some(session_ref) = cli.coding_session_review {
        return print_coding_session_review(Arc::clone(&memory), &session_ref);
    }

    if let Some(session_ref) = cli.coding_session_publish {
        return publish_coding_session_artifacts(Arc::clone(&memory), &session_ref);
    }

    if let Some(limit) = cli.autonomy_queue_limit {
        return print_autonomy_queue(Arc::clone(&memory), limit);
    }

    if let Some(queue_ref) = cli.autonomy_queue_review {
        return print_autonomy_queue_review(Arc::clone(&memory), &queue_ref);
    }

    if let Some(queue_ref) = cli.autonomy_queue_replay {
        return print_autonomy_queue_replay(Arc::clone(&memory), &queue_ref);
    }

    if let Some(queue_ref) = cli.autonomy_queue_publish {
        return publish_autonomy_queue_run(Arc::clone(&memory), &queue_ref);
    }

    if let Some(goal) = cli.autonomy_enqueue_goal {
        return enqueue_operator_autonomy_goal(
            Arc::clone(&memory),
            Arc::clone(&events),
            &goal,
            cli.autonomy_enqueue_profile,
        );
    }

    if cli.autonomy_plan {
        return plan_autonomy_queue_once(Arc::clone(&memory), Arc::clone(&events));
    }

    if cli.autonomy_preview {
        return preview_autonomy_step(Arc::clone(&memory), Arc::clone(&events));
    }

    if let Some(limit) = cli.work_loops_limit {
        return print_work_loops(Arc::clone(&memory), limit);
    }

    if let Some(limit) = cli.run_log_limit {
        return print_run_log(Arc::clone(&memory), limit);
    }

    if cli.brief {
        return print_prof_x_brief(Arc::clone(&memory), Arc::clone(&events));
    }

    if let Some(limit) = cli.prof_x_journal_limit {
        return write_prof_x_journal(Arc::clone(&events), limit, false);
    }

    if let Some(limit) = cli.prof_x_journal_commit_limit {
        return write_prof_x_journal(Arc::clone(&events), limit, true);
    }

    if cli.consciousness_report {
        return print_consciousness_report(Arc::clone(&memory));
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

    if let Some(task_ref) = cli.task_evidence {
        return print_task_evidence(
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            &task_ref,
        );
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

    // Model Context Protocol: connect any configured external servers and
    // register their tools alongside the built-ins. Non-fatal if absent.
    {
        let repo_root = PermissionScope::default_autonomous().workspace_root;
        let (mcp_servers, mcp_tools) =
            toolbridge::mcp::init_global_mcp(&repo_root, &registry).await;
        if mcp_servers > 0 {
            info!("mcp: {mcp_servers} server(s), {mcp_tools} tool(s) registered");
            let _ = events.append(
                None,
                None,
                "mcp.connected",
                format!("{mcp_servers} MCP server(s), {mcp_tools} tool(s)"),
                serde_json::json!({"servers": mcp_servers, "tools": mcp_tools}),
            );
        }
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

    if cli.self_reload_probe {
        // Target of a hot-reload re-exec: proves the freshly-built binary launched itself.
        let generation = crate::evolved::hot_reload::current_generation();
        println!("hot-reload probe: running as generation {generation} (re-exec succeeded)");
        return Ok(());
    }

    if cli.self_rebuild_check {
        return run_self_rebuild_check(Arc::clone(&events)).await;
    }

    if let Some(commit) = cli.rollback_verdict_commit.clone() {
        return run_rollback_verdict(Arc::clone(&events), commit).await;
    }

    if cli.self_rebuild_reexec {
        return run_self_rebuild_reexec(Arc::clone(&events)).await;
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

    if let Some(goal) = cli.skill_patch_live_goal {
        let patch = write_operator_skill_patch(&goal)?;
        return run_repo_patch_coding_session_live_with_goal(
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            patch.patch_path.clone(),
            Some(operator_skill_session_goal(&patch, false)),
        )
        .await;
    }

    if let Some(goal) = cli.skill_patch_commit_live_goal {
        let patch = write_operator_skill_patch(&goal)?;
        return run_repo_patch_commit_coding_session_live_with_goal(
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            patch.patch_path.clone(),
            Some(operator_skill_session_goal(&patch, true)),
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
            None,
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
            None,
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
            None,
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

    if let Some(count) = cli.autonomy_step_count {
        return run_autonomy_queue_steps(
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            count,
            cli.publish_after_run,
        )
        .await;
    }

    if let Some(count) = cli.autonomy_step_live_count {
        return run_autonomy_queue_steps_live(
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            count,
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

    // ── identity: seed pinned memory from professor_x.md (immutable) ────
    {
        let persona_path = default_repo_root().join("professor-x/personas/professor_x.md");
        match std::fs::read_to_string(&persona_path) {
            Ok(content) if !content.trim().is_empty() => {
                let entry = PinnedEntry {
                    id: "professor-x-identity".to_string(),
                    content: content.clone(),
                    immutable: true,
                };
                if let Err(e) = memory.pinned.upsert(&entry) {
                    warn!("startup: failed to seed pinned identity: {e}");
                } else {
                    info!(
                        "startup: pinned identity seeded from {}",
                        persona_path.display()
                    );
                }
                // ── self-model: seed round-0 Strange Loop snapshot ──────────
                if let Err(e) = memory.self_model.seed_if_empty(content) {
                    warn!("startup: failed to seed self-model round-0: {e}");
                } else {
                    info!("startup: self-model round-0 snapshot ready");
                }
            }
            Ok(_) => warn!("startup: professor_x.md is empty — identity not seeded"),
            Err(e) => warn!(
                "startup: could not read {} — identity not seeded: {e}",
                persona_path.display()
            ),
        }
    }

    // ── model resolution (local, VRAM-aware) ─────────────────────────────
    // Professor X is one harness across the whole local-model spectrum. Pick the
    // model: explicit --model / PROFESSOR_X_MODEL wins; else the LARGEST model the
    // user has installed (they only pull what their VRAM runs, so "biggest
    // installed" = "best this machine can do"); else the 8B default.
    let ollama = {
        let probe = ollama::OllamaClient::new("http://localhost:11434");
        let chosen = match cli
            .model
            .clone()
            .or_else(|| std::env::var("PROFESSOR_X_MODEL").ok())
        {
            Some(m) => m,
            None => probe
                .best_local_model()
                .await
                .unwrap_or_else(|| ollama::DEFAULT_MODEL.to_string()),
        };
        info!("model: {chosen}  (override: --model <name> or PROFESSOR_X_MODEL)");
        Arc::new(probe.with_model(chosen))
    };
    // ── ollama health check ───────────────────────────────────────────────
    match ollama.health_check().await {
        Ok(true) => info!("ollama: reachable, model ready"),
        Ok(false) => warn!("ollama: reachable but model may not be loaded — check `ollama list`"),
        Err(e) => warn!("ollama: not reachable ({e}) — tasks will fail until Ollama starts"),
    }

    if cli.evolve_live {
        return run_live_evolution_cycle(
            Arc::clone(&ollama),
            Arc::clone(&memory),
            Arc::clone(&events),
        )
        .await;
    }

    if let Some(iters) = cli.evolve_forever {
        return run_evolve_forever(
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            cancel.clone(),
            iters,
            15, // tasks per measurement block — fast iteration; raise for precision
        )
        .await;
    }

    if let Some(n) = cli.generate_curriculum {
        return generate_curriculum(
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&memory),
            n,
        )
        .await;
    }

    if let Some(limit) = cli.run_self_tests {
        return run_self_authored_tests(
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            cancel.clone(),
            limit,
        )
        .await;
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

    if cli.serve {
        ensure_folder_trusted();
        return serve::run_serve(
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            cancel.clone(),
        )
        .await;
    }

    if cli.tui {
        ensure_folder_trusted();
        return tui::run_tui(
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            cancel.clone(),
        )
        .await;
    }

    if cli.chat {
        ensure_folder_trusted();
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

    if cli.repo_fix_bench {
        return run_repo_fix_bench(
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

    if cli.evolve_on_repofix.is_some()
        || cli.evolve_skill_on_repofix.is_some()
        || cli.evolve_code_on_repofix.is_some()
    {
        // M4 frontier: the PROPOSER may be a stronger (local) model than the agent's, so a
        // capable model authors candidate changes while the small model runs the tasks.
        // Defaults to the agent's own model when --proposer-model is not given.
        let proposer: Arc<ollama::OllamaClient> = match cli.proposer_model.clone() {
            Some(m) => {
                info!("M4: using stronger proposer model '{m}'");
                Arc::new(ollama::OllamaClient::new("http://localhost:11434").with_model(m))
            }
            None => Arc::clone(&ollama),
        };
        if let Some(rounds) = cli.evolve_on_repofix {
            return run_evolve_on_repofix(
                Arc::clone(&ollama),
                Arc::clone(&registry),
                Arc::clone(&policy),
                Arc::clone(&memory),
                Arc::clone(&events),
                cancel,
                rounds,
                proposer,
            )
            .await;
        }
        if let Some(rounds) = cli.evolve_skill_on_repofix {
            return run_evolve_skill_on_repofix(
                Arc::clone(&ollama),
                Arc::clone(&registry),
                Arc::clone(&policy),
                Arc::clone(&memory),
                Arc::clone(&events),
                cancel,
                rounds,
                proposer,
            )
            .await;
        }
        if let Some(rounds) = cli.evolve_code_on_repofix {
            let target = cli
                .evolve_code_target
                .clone()
                .unwrap_or_else(|| "src/toolbridge/hashedit.rs".to_string());
            return run_evolve_code_on_repofix(
                Arc::clone(&ollama),
                Arc::clone(&registry),
                Arc::clone(&policy),
                Arc::clone(&memory),
                Arc::clone(&events),
                cancel,
                rounds,
                proposer,
                target,
            )
            .await;
        }
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

    // Default: on an interactive terminal with no arguments, open the assistant
    // session — like `claude` / `codex`. Headless or with-args → daemon.
    {
        use std::io::IsTerminal;
        if std::env::args().len() == 1 && std::io::stdin().is_terminal() {
            ensure_folder_trusted();
            return run_interactive_tasks(
                ollama,
                registry,
                policy,
                memory,
                events,
                transcripts,
                cancel,
            )
            .await;
        }
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

/// Workspace-trust gate (like Claude Code / VS Code "Do you trust this folder?").
/// Professor X can read, write, and run shell commands in the working directory,
/// so on first use in a new folder it asks for consent. Trusted folders are
/// remembered in ~/.professor-x/trusted_dirs.txt. Non-interactive sessions skip
/// the prompt (a daemon/service can't answer).
fn ensure_folder_trusted() {
    use std::io::{IsTerminal, Write};
    let cwd = match std::env::current_dir() {
        Ok(c) => c.to_string_lossy().to_string(),
        Err(_) => return,
    };
    let dir = std::env::var("PROFESSOR_X_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".professor-x")
        });
    let trust_file = dir.join("trusted_dirs.txt");
    let trusted = std::fs::read_to_string(&trust_file).unwrap_or_default();
    if trusted.lines().any(|l| l.trim() == cwd) {
        return;
    }
    if !std::io::stdin().is_terminal() {
        return; // can't prompt; assume the operator launched it deliberately
    }
    println!("\n  \x1b[1mDo you trust the files in this folder?\x1b[0m");
    println!("  {cwd}");
    println!("  Professor X can read, write, and run shell commands here.\n");
    print!("  Trust this folder? [y/N] ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    let ans = line.trim().to_lowercase();
    if ans == "y" || ans == "yes" {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&trust_file)
        {
            let _ = writeln!(f, "{cwd}");
        }
        println!("  ✓ trusted.\n");
    } else {
        println!("  Not trusted — exiting. (Run from a folder you trust.)");
        std::process::exit(0);
    }
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
    #[serde(default)]
    operator_goal: Option<String>,
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
    #[serde(default)]
    operator_goal: Option<String>,
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
    operator_goal: Option<String>,
) -> Result<(EvolutionProposalDryRunReport, PathBuf)> {
    let repo_root = default_repo_root();
    let node = operator_proposal_node("px-operator-proposal-dry-run", operator_goal.as_deref());
    let target_component = format!("{:?}", node.target_component);
    let planned_checks = vec![
        "reward_hacking_scan",
        "sandbox_worktree",
        "material_diff",
        "cargo_check",
    ];
    events.append(
        None,
        None,
        "evolution.proposal_dry_run.started",
        "starting non-committing evolution proposal dry-run",
        serde_json::json!({
            "workspace": "repo-root",
            "harness_commit": git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
            "target_component": target_component.clone(),
            "motivation": node.motivation.clone(),
            "operator_goal": operator_goal.clone(),
        }),
    )?;
    events.append(
        None,
        None,
        "evolution.proposal_dry_run.verifying",
        "verifying proposal in isolated sandbox worktree",
        serde_json::json!({
            "workspace": "sandbox_worktree",
            "target_component": target_component.clone(),
            "planned_checks": planned_checks.clone(),
        }),
    )?;

    let heartbeat_cancel = CancellationToken::new();
    let heartbeat_events = Arc::clone(&events);
    let heartbeat_goal = operator_goal.clone();
    let heartbeat_target = target_component.clone();
    let heartbeat_checks = planned_checks.clone();
    let heartbeat_token = heartbeat_cancel.clone();
    let heartbeat = tokio::spawn(async move {
        let mut elapsed_secs = 0u64;
        loop {
            tokio::select! {
                _ = heartbeat_token.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    elapsed_secs += 10;
                    let _ = heartbeat_events.append(
                        None,
                        None,
                        "evolution.proposal_dry_run.heartbeat",
                        format!("proposal sandbox verification still running after {elapsed_secs}s"),
                        serde_json::json!({
                            "workspace": "sandbox_worktree",
                            "target_component": heartbeat_target.clone(),
                            "operator_goal": heartbeat_goal.clone(),
                            "elapsed_secs": elapsed_secs,
                            "planned_checks": heartbeat_checks.clone(),
                        }),
                    );
                }
            }
        }
    });

    let verify_repo_root = repo_root.clone();
    let verify_node = node.clone();
    let runtime_handle = tokio::runtime::Handle::current();
    let verification_result = tokio::task::spawn_blocking(move || {
        runtime_handle.block_on(verify_node_in_sandbox(&verify_repo_root, &verify_node))
    })
    .await
    .unwrap_or_else(|err| {
        Err(anyhow::anyhow!(
            "proposal sandbox verification task failed: {err}"
        ))
    });
    heartbeat_cancel.cancel();
    let _ = heartbeat.await;
    let verification = verification_result?;
    let diff_hash = if verification.diff.is_empty() {
        None
    } else {
        Some(sha256_hex(verification.diff.as_bytes()))
    };
    let report = EvolutionProposalDryRunReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        mode: "dry_run".to_string(),
        operator_goal,
        workspace: "repo-root".to_string(),
        harness_commit: git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        target_component,
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
            "operator_goal": report.operator_goal.clone(),
            "report_path": path,
        }),
    )?;
    Ok((report, path))
}

async fn run_evolution_proposal_dry_run(events: Arc<EventStore>) -> Result<()> {
    let (report, path) = execute_evolution_proposal_dry_run(events, None).await?;
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
        tokio::spawn(async move { execute_evolution_proposal_dry_run(run_events, None).await });

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
        operator_goal: None,
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
    operator_goal: Option<String>,
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
            "operator_goal": operator_goal.clone(),
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
        operator_goal,
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
                "operator_goal": report.operator_goal.clone(),
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
                    "operator_goal": report.operator_goal.clone(),
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
                    "operator_goal": report.operator_goal.clone(),
                    "report_path": path,
                }),
            )?;
            Ok((report, path))
        }
    }
}

async fn run_patch_apply_commit(events: Arc<EventStore>, patch_path: PathBuf) -> Result<()> {
    let (report, path) = execute_patch_apply_commit(events, patch_path, None).await?;
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

/// Rollback monitoring: report whether an accepted autonomous `applied_commit` still holds
/// against HEAD (vs reverted or missing). Pure git — surfaces the verdict for the operator.
async fn run_rollback_verdict(events: Arc<EventStore>, commit: String) -> Result<()> {
    use crate::evolved::rollback;
    let repo_root = default_repo_root();
    let verdict = rollback::applied_commit_verdict(&repo_root, &commit).await?;
    println!("rollback verdict for {commit}: {}", verdict.status.as_str());
    if let Some(resolved) = &verdict.resolved {
        println!("  resolved: {resolved}");
    }
    println!("  present in HEAD: {}", verdict.present_in_head);
    if let Some(by) = &verdict.reverted_by {
        println!("  reverted by: {by}");
    }
    events.append(
        None,
        None,
        "evolution.rollback.verdict",
        format!("accepted commit {commit} verdict: {}", verdict.status.as_str()),
        serde_json::to_value(&verdict).unwrap_or_default(),
    )?;
    Ok(())
}

/// Rebuild-only safety gate: prove the committed source builds release-clean so a later
/// hot-reload can never replace a working binary with a broken one. Never re-execs.
async fn run_self_rebuild_check(events: Arc<EventStore>) -> Result<()> {
    use crate::evolved::hot_reload;
    let repo_root = default_repo_root();
    println!("hot-reload check: rebuilding committed tree (release) — no re-exec…");
    events.append(
        None,
        None,
        "hot_reload.check.started",
        "verifying the committed tree builds release-clean before any hot-reload",
        serde_json::json!({
            "harness_commit": git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        }),
    )?;
    match hot_reload::rebuild_release(&repo_root).await {
        Ok(bin) => {
            println!("hot-reload check: OK — release binary built at {}", bin.display());
            println!("  the committed tree is safe to hot-reload (re-exec gated separately).");
            events.append(
                None,
                None,
                "hot_reload.check.passed",
                "committed tree builds release-clean; safe to hot-reload",
                serde_json::json!({ "binary": bin.display().to_string() }),
            )?;
            Ok(())
        }
        Err(err) => {
            events.append(
                None,
                None,
                "hot_reload.check.failed",
                "committed tree does NOT build release-clean; hot-reload would be refused",
                serde_json::json!({ "error": err.to_string() }),
            )?;
            Err(err)
        }
    }
}

/// Close the evolve→apply→measure loop with no operator restart: rebuild the release binary
/// from the (already-committed) source and re-exec into it. Safety is structural — we only
/// re-exec after a clean `cargo build --release`, and a generation cap bounds reloads.
#[cfg(unix)]
async fn run_self_rebuild_reexec(events: Arc<EventStore>) -> Result<()> {
    use crate::evolved::hot_reload::{self, ReloadDecision, DEFAULT_MAX_GENERATIONS};
    let repo_root = default_repo_root();
    let generation = hot_reload::current_generation();
    println!(
        "hot-reload: rebuilding release binary at generation {generation} (no operator restart)…"
    );
    events.append(
        None,
        None,
        "hot_reload.started",
        "rebuilding release binary to apply a committed self-change without a restart",
        serde_json::json!({
            "generation": generation,
            "max_generations": DEFAULT_MAX_GENERATIONS,
            "harness_commit": git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        }),
    )?;

    // On a successful re-exec this call never returns (the process image is replaced by the
    // freshly built binary running `--self-reload-probe`); we only fall through on a
    // deliberate Stay decision (build failed / generation cap) or an exec error.
    let decision = hot_reload::reload_after_commit(
        &repo_root,
        &["--self-reload-probe".to_string()],
        DEFAULT_MAX_GENERATIONS,
    )
    .await?;

    match decision {
        ReloadDecision::StayBuildFailed => {
            events.append(
                None,
                None,
                "hot_reload.skipped",
                "rebuild failed — kept the current binary, loop continues",
                serde_json::json!({ "generation": generation, "reason": "build_failed" }),
            )?;
            anyhow::bail!("hot-reload: cargo build --release failed; kept the running binary");
        }
        ReloadDecision::StayGenerationCap { generation } => {
            println!("hot-reload: generation cap reached ({generation}); not re-exec'ing.");
            events.append(
                None,
                None,
                "hot_reload.capped",
                "generation cap reached — stopping auto-reload for this lineage",
                serde_json::json!({ "generation": generation, "reason": "generation_cap" }),
            )?;
            Ok(())
        }
        ReloadDecision::Reexec { .. } => {
            // Reached only if exec() itself failed (reload_after_commit would otherwise not return).
            anyhow::bail!("hot-reload: re-exec into the rebuilt binary did not take effect");
        }
    }
}

#[cfg(not(unix))]
async fn run_self_rebuild_reexec(_events: Arc<EventStore>) -> Result<()> {
    anyhow::bail!("hot-reload (self-rebuild + re-exec) is only supported on Unix");
}

async fn run_patch_apply_commit_live(events: Arc<EventStore>, patch_path: PathBuf) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live patch apply");
    println!("Streaming verify, apply, check, and commit events.");
    io::stdout().flush()?;

    let run_events = Arc::clone(&events);
    let mut handle =
        tokio::spawn(async move { execute_patch_apply_commit(run_events, patch_path, None).await });

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

fn write_patch_apply_commit_patch(operator_goal: Option<&str>) -> Result<PathBuf> {
    if let Some(goal) = operator_goal
        .map(normalize_operator_goal)
        .filter(|goal| !goal.is_empty())
    {
        return Ok(write_operator_skill_patch(&goal)?.patch_path);
    }
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

#[derive(Debug, Clone)]
struct OperatorSkillPatch {
    patch_path: PathBuf,
    skill_name: String,
    skill_path: PathBuf,
    goal: String,
}

const OPERATOR_SKILL_PREFIX: &str = "px-operator-goal-";
const OPERATOR_SKILL_TIMESTAMP_LEN: usize = 15; // YYYYMMDD-HHMMSS
const MAX_SKILL_NAME_LEN: usize = 64;

fn write_operator_skill_patch(goal: &str) -> Result<OperatorSkillPatch> {
    let goal = normalize_operator_goal(goal);
    let slug = skill_goal_slug(&goal);
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let skill_name = format!("{OPERATOR_SKILL_PREFIX}{timestamp}-{slug}");
    let path = PathBuf::from("professor-x")
        .join("skills")
        .join("conductor")
        .join(format!("{skill_name}.md"));
    let body = operator_goal_skill_body(&skill_name, &goal);
    let diff = unified_new_file_diff(&path, &body);
    let patch_path = std::env::temp_dir().join(format!("{skill_name}.diff"));
    std::fs::write(&patch_path, diff)?;
    Ok(OperatorSkillPatch {
        patch_path,
        skill_name,
        skill_path: path,
        goal,
    })
}

fn operator_skill_session_goal(patch: &OperatorSkillPatch, commit: bool) -> String {
    format!(
        "operator goal skill session: {} goal='{}' skill={} path={} patch={}",
        if commit {
            "verify, apply, and commit"
        } else {
            "verify"
        },
        patch.goal,
        patch.skill_name,
        patch.skill_path.display(),
        patch.patch_path.display()
    )
}

fn normalize_operator_goal(goal: &str) -> String {
    let one_line = goal
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let cleaned = one_line
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>();
    truncate(cleaned.trim(), 180)
}

fn skill_goal_slug(goal: &str) -> String {
    let max_slug_len =
        MAX_SKILL_NAME_LEN - OPERATOR_SKILL_PREFIX.len() - OPERATOR_SKILL_TIMESTAMP_LEN - 1;
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in goal.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
        if slug.len() >= max_slug_len {
            break;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "operator-goal".to_string()
    } else {
        slug
    }
}

fn operator_goal_skill_body(skill_name: &str, goal: &str) -> String {
    format!(
        "# {skill_name}\n\nPurpose: capture an operator-requested Professor X harness goal as a verified, reusable conductor skill.\n\nOperator goal: {goal}\n\nProcedure:\n- Restate the goal in concrete harness terms before acting.\n- Inspect current repo evidence, especially docs/research, docs/plans, professor-x/ops/runbooks, and recent artifacts.\n- Prefer workspace-bound, reversible changes that improve observability, safety, measurement, or verified commit flow.\n- Produce or update durable evidence under docs/, brain/, professor-x/artifacts/, or professor-x/skills/ as appropriate.\n- Run the narrowest meaningful verification command before claiming progress.\n\nAcceptance:\n- The work maps directly to the operator goal above.\n- Any changed file is inside the repository workspace.\n- Any claimed improvement names a command, artifact, commit, or report that proves it.\n- Follow-up work is explicit if the goal is not complete.\n"
    )
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
    let (report, path) = execute_operator_commit_smoke(events, None).await?;
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
    operator_goal: Option<String>,
) -> Result<(EvolutionProposalDryRunReport, PathBuf)> {
    let repo_root = default_repo_root();
    if !main_worktree_clean_for_operator_commit(&repo_root)? {
        anyhow::bail!("main worktree has source/config/skill changes; refusing operator commit");
    }

    let skill_name = format!(
        "px-operator-autocommit-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let node = operator_proposal_node(&skill_name, operator_goal.as_deref());
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
            "operator_goal": operator_goal.clone(),
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
        operator_goal,
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
                "operator_goal": report.operator_goal.clone(),
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
                    "operator_goal": report.operator_goal.clone(),
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
                    "operator_goal": report.operator_goal.clone(),
                    "report_path": report_path,
                }),
            )?;
            Ok((report, report_path))
        }
    }
}

fn operator_proposal_node(skill_name: &str, operator_goal: Option<&str>) -> EvolutionNode {
    let goal = operator_goal.map(normalize_operator_goal);
    let body = match goal.as_deref() {
        Some(goal) if !goal.is_empty() => operator_goal_skill_body(skill_name, goal),
        _ => format!(
            "# {skill_name}\n\nPurpose: preserve the operator verify-then-commit workflow as a reusable skill.\n\nWorkflow:\n- State the proposed harness change and target component.\n- Verify it in an isolated sandbox before touching the main worktree.\n- Record the checks, diff hash, decision, commit id, and rollback path.\n\nOutput Contract:\n- A proposal record with motivation, target component, verification checks, decision, artifact path, and commit id when applied.\n"
        ),
    };
    let motivation = match goal.as_deref() {
        Some(goal) if !goal.is_empty() => format!("operator queued goal proposal: {goal}"),
        _ => "smoke verify operator_proposal proposal".to_string(),
    };
    EvolutionNode::new(
        motivation,
        HarnessComponent::SkillDefinition(skill_name.to_string()),
        body.clone(),
        ChangeManifest {
            evidence_cited: vec!["autonomy-queue".to_string(), "operator-goal".to_string()],
            root_cause: goal
                .as_ref()
                .map(|goal| format!("queued operator goal needs a durable reusable skill: {goal}"))
                .unwrap_or_else(|| {
                    "verify sandbox accept/reject behavior before autonomous run".to_string()
                }),
            fix_description: body,
            predicted_fixes: vec![
                "operator goal provenance in proposal evidence".to_string(),
                "sandbox verification coverage".to_string(),
            ],
            predicted_regressions: Vec::new(),
            verification_status: VerificationStatus::Pending,
            verified_at: None,
        },
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
            .join("conductor")
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

struct RepoPatchCommitCodingSessionOutcome {
    passed: bool,
    session_id: String,
    session_report_path: PathBuf,
    evidence_path: PathBuf,
    verification: PatchVerificationReport,
    verification_path: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SupervisedLoopReport {
    run_id: String,
    run_kind: String,
    #[serde(default)]
    queue_id: Option<String>,
    #[serde(default)]
    operator_goal: Option<String>,
    started_at: String,
    completed_at: String,
    requested_cycles: u32,
    completed_cycles: u32,
    passed_cycles: u32,
    failed_cycles: u32,
    profile: String,
    #[serde(default)]
    ledger_path: Option<String>,
    #[serde(default)]
    journal_path: Option<String>,
    planned_jobs: Vec<WorkLoopPlannedJob>,
    smoke_records: Vec<WorkLoopSmokeRecord>,
    #[serde(default)]
    timeline: Vec<WorkTimelineEntry>,
}

#[derive(Debug, Clone)]
struct WorkLoopRunContext {
    queue_id: Option<String>,
    operator_goal: Option<String>,
}

impl WorkLoopRunContext {
    fn from_queue_item(item: &AutonomyQueueItem) -> Self {
        Self {
            queue_id: Some(item.id.clone()),
            operator_goal: Some(item.goal.clone()),
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

fn annotate_planned_jobs_with_context(
    jobs: &mut [WorkLoopPlannedJob],
    context: Option<&WorkLoopRunContext>,
) {
    let Some(goal) = context.and_then(|ctx| ctx.operator_goal.as_deref()) else {
        return;
    };
    let goal = truncate(goal, 96);
    for job in jobs {
        job.reason = format!("queued goal: {goal}; {}", job.reason);
    }
}

fn prioritize_planned_jobs_for_context(
    jobs: &mut Vec<WorkLoopPlannedJob>,
    profile: WorkLoopProfile,
    context: Option<&WorkLoopRunContext>,
) {
    if jobs.is_empty() {
        return;
    }
    if jobs
        .first()
        .map(|job| job.reason.contains("latest operator run failed"))
        .unwrap_or(false)
    {
        return;
    }
    let Some(goal) = context.and_then(|ctx| ctx.operator_goal.as_deref()) else {
        return;
    };
    let Some(target) = goal_target_job_for_profile(goal, profile) else {
        return;
    };
    let reason = format!(
        "operator queued goal targets {} gate from goal keywords",
        target.label()
    );
    if let Some(index) = jobs.iter().position(|job| job.kind == target.kind()) {
        let mut job = jobs.remove(index);
        job.reason = reason;
        jobs.insert(0, job);
    } else {
        jobs[0] = planned_job(1, target, reason);
    }
    for (index, job) in jobs.iter_mut().enumerate() {
        job.cycle = index as u32 + 1;
    }
}

fn goal_target_job_for_profile(goal: &str, profile: WorkLoopProfile) -> Option<WorkLoopJob> {
    let goal = goal.to_ascii_lowercase();
    let has_any = |terms: &[&str]| terms.iter().any(|term| goal.contains(term));

    if has_any(&["hiro", "benchmark", "task inventory", "pass@3", "pass at 3"]) {
        return Some(WorkLoopJob::HiroSmoke);
    }
    if has_any(&[
        "sandbox",
        "reward",
        "rollback",
        "policy",
        "safety",
        "evolution smoke",
    ]) {
        return Some(WorkLoopJob::EvolutionSmoke);
    }
    if has_any(&["proposal", "dry-run", "dry run", "manifest", "provenance"]) {
        return Some(WorkLoopJob::ProposalDryRun);
    }
    if has_any(&["commit", "git", "apply", "patch", "publish"]) {
        return Some(match profile {
            WorkLoopProfile::Commit => WorkLoopJob::PatchApplyCommit,
            _ => WorkLoopJob::ProposalDryRun,
        });
    }
    if has_any(&["code", "coding", "edit", "test", "compile", "cargo"]) {
        return Some(WorkLoopJob::CodingSmoke);
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutonomyQueuePlan {
    goal: String,
    kind: String,
    profile: WorkLoopProfile,
    cycles: u32,
    priority: u8,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutonomyStepPreview {
    source: String,
    queue_id: Option<String>,
    goal: String,
    kind: String,
    profile: WorkLoopProfile,
    cycles: u32,
    priority: u8,
    reason: String,
    planned_jobs: Vec<WorkLoopPlannedJob>,
}

fn plan_next_autonomy_queue_item(recent_runs: &[WorkLoopRunRecord]) -> AutonomyQueuePlan {
    if let Some(job) = latest_failed_operator_job(recent_runs) {
        let profile = match job {
            WorkLoopJob::PatchApplyCommit | WorkLoopJob::OperatorCommit => WorkLoopProfile::Commit,
            _ => WorkLoopProfile::Core,
        };
        return AutonomyQueuePlan {
            goal: format!(
                "retry failed operator gate: {} with {} safety profile",
                job.label(),
                profile.as_str()
            ),
            kind: "operator_run".to_string(),
            profile,
            cycles: 1,
            priority: 90,
            reason: format!(
                "latest operator run failed {}; retry that gate before broadening autonomy",
                job.kind()
            ),
        };
    }

    let operator_runs = recent_runs
        .iter()
        .filter(|run| run.run_kind == WorkLoopRunKind::Operator.as_str())
        .collect::<Vec<_>>();
    if operator_runs.is_empty() {
        return AutonomyQueuePlan {
            goal: "bootstrap autonomous harness work: prove the local coding-agent edit/test gate with evidence".to_string(),
            kind: "operator_run".to_string(),
            profile: WorkLoopProfile::Core,
            cycles: 1,
            priority: 50,
            reason: "no operator run exists yet; start with the safest coding-agent smoke gate".to_string(),
        };
    }

    if !operator_runs.iter().any(|run| {
        run.smoke_records
            .iter()
            .any(|record| record.kind == "proposal_dry_run" && record.passed)
    }) {
        return AutonomyQueuePlan {
            goal: "complete core autonomy coverage: coding smoke, evolution smoke, HIRO smoke, and proposal dry-run".to_string(),
            kind: "operator_run".to_string(),
            profile: WorkLoopProfile::Core,
            cycles: 4,
            priority: 70,
            reason: "recent evidence lacks a passed proposal dry-run; run the full core safety profile before commit-capable autonomy".to_string(),
        };
    }

    let has_patch_apply_commit = operator_runs.iter().any(|run| {
        run.smoke_records
            .iter()
            .any(|record| record.kind == "patch_apply_commit" && record.passed)
    });
    let has_operator_commit = operator_runs.iter().any(|run| {
        run.smoke_records
            .iter()
            .any(|record| record.kind == "operator_commit" && record.passed)
    });
    if !has_patch_apply_commit || !has_operator_commit {
        let reason = if !has_patch_apply_commit {
            "core proposal dry-run evidence exists but no passed patch_apply_commit gate is recorded"
                .to_string()
        } else {
            "verified patch apply evidence exists but no passed operator_commit gate is recorded"
                .to_string()
        };
        return AutonomyQueuePlan {
            goal: "advance to commit-capable autonomy: run verified patch apply and final operator commit gates after core safety coverage".to_string(),
            kind: "operator_run".to_string(),
            profile: WorkLoopProfile::Commit,
            cycles: 6,
            priority: 60,
            reason,
        };
    }

    AutonomyQueuePlan {
        goal: "maintenance autonomy cycle: refresh core safety evidence and watch for regressions".to_string(),
        kind: "operator_run".to_string(),
        profile: WorkLoopProfile::Core,
        cycles: 4,
        priority: 40,
        reason: "core and commit-capable evidence exist; continue maintenance coverage while waiting for HIRO/DHE targeting".to_string(),
    }
}

fn preview_autonomy_step_from_parts(
    pending: Option<AutonomyQueueItem>,
    recent_runs: &[WorkLoopRunRecord],
) -> AutonomyStepPreview {
    match pending {
        Some(item) => {
            let profile = WorkLoopProfile::parse(&item.profile).unwrap_or(WorkLoopProfile::Core);
            let context = WorkLoopRunContext::from_queue_item(&item);
            let mut planned_jobs = plan_work_loop_jobs(
                WorkLoopRunKind::Operator,
                profile,
                item.cycles.clamp(1, 50),
                recent_runs,
            );
            prioritize_planned_jobs_for_context(&mut planned_jobs, profile, Some(&context));
            annotate_planned_jobs_with_context(&mut planned_jobs, Some(&context));
            AutonomyStepPreview {
                source: "pending_queue".to_string(),
                queue_id: Some(item.id),
                goal: item.goal,
                kind: item.kind,
                profile,
                cycles: item.cycles.clamp(1, 50),
                priority: item.priority,
                reason: "highest-priority pending queue item".to_string(),
                planned_jobs,
            }
        }
        None => {
            let plan = plan_next_autonomy_queue_item(recent_runs);
            let context = WorkLoopRunContext {
                queue_id: None,
                operator_goal: Some(plan.goal.clone()),
            };
            let mut planned_jobs = plan_work_loop_jobs(
                WorkLoopRunKind::Operator,
                plan.profile,
                plan.cycles.clamp(1, 50),
                recent_runs,
            );
            prioritize_planned_jobs_for_context(&mut planned_jobs, plan.profile, Some(&context));
            annotate_planned_jobs_with_context(&mut planned_jobs, Some(&context));
            AutonomyStepPreview {
                source: "planner_seed".to_string(),
                queue_id: None,
                goal: plan.goal,
                kind: plan.kind,
                profile: plan.profile,
                cycles: plan.cycles.clamp(1, 50),
                priority: plan.priority,
                reason: plan.reason,
                planned_jobs,
            }
        }
    }
}

fn preview_autonomy_step(memory: Arc<MemoryManager>, events: Arc<EventStore>) -> Result<()> {
    let queue_store = AutonomyQueueStore::new(Arc::clone(&memory.db));
    let recent_runs = WorkLoopRunStore::new(Arc::clone(&memory.db)).recent(8)?;
    let preview = preview_autonomy_step_from_parts(queue_store.next_pending()?, &recent_runs);
    events.append(
        None,
        None,
        "autonomy.queue.previewed",
        format!(
            "previewed next autonomous queue step: {}",
            truncate(&preview.goal, 100)
        ),
        serde_json::json!({
            "source": preview.source,
            "queue_id": preview.queue_id,
            "goal": preview.goal,
            "kind": preview.kind,
            "profile": preview.profile.as_str(),
            "cycles": preview.cycles,
            "priority": preview.priority,
            "reason": preview.reason,
            "planned_jobs": preview.planned_jobs,
            "mutates_queue": false,
            "next_command": "cargo run -- --prof-x-step-live 1",
        }),
    )?;
    print_autonomy_step_preview(&preview);
    Ok(())
}

fn print_autonomy_step_preview(preview: &AutonomyStepPreview) {
    println!("Professor X next autonomous step preview");
    println!("  source: {}", preview.source);
    println!(
        "  queue: {}",
        preview
            .queue_id
            .as_deref()
            .map(short_fragment)
            .unwrap_or("none; would seed planner item")
    );
    println!("  goal: {}", preview.goal);
    println!(
        "  run: {}:{} cycles={} priority={}",
        preview.kind,
        preview.profile.as_str(),
        preview.cycles,
        preview.priority
    );
    println!("  reason: {}", preview.reason);
    println!("  mutates queue: false");
    println!("  planned gates:");
    for job in &preview.planned_jobs {
        println!(
            "    {:>2}. {:<18} {}",
            job.cycle,
            job.kind,
            truncate(&job.reason, 110)
        );
    }
    println!("  execute-live: cargo run -- --prof-x-step-live 1");
    println!("  execute-quiet: cargo run -- --prof-x-step 1");
    println!("  watch: cargo run -- --observe-work");
}

fn enqueue_planned_autonomy_item(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
) -> Result<AutonomyQueueItem> {
    let store = AutonomyQueueStore::new(Arc::clone(&memory.db));
    let recent_runs = WorkLoopRunStore::new(Arc::clone(&memory.db)).recent(8)?;
    let plan = plan_next_autonomy_queue_item(&recent_runs);
    let item = store.enqueue(
        plan.goal.clone(),
        plan.kind.clone(),
        plan.profile.as_str(),
        plan.cycles,
        plan.priority,
    )?;
    events.append(
        None,
        None,
        "autonomy.queue.planned",
        format!("planned autonomous queue item {}", short_fragment(&item.id)),
        serde_json::json!({
            "queue_id": item.id,
            "goal": item.goal,
            "kind": item.kind,
            "profile": item.profile,
            "cycles": item.cycles,
            "priority": item.priority,
            "reason": plan.reason,
        }),
    )?;
    Ok(item)
}

fn plan_autonomy_queue_once(memory: Arc<MemoryManager>, events: Arc<EventStore>) -> Result<()> {
    let store = AutonomyQueueStore::new(Arc::clone(&memory.db));
    if store.count_pending()? > 0 {
        println!("Autonomous queue already has pending work.");
        return print_autonomy_queue(memory, 10);
    }
    let item = enqueue_planned_autonomy_item(Arc::clone(&memory), events)?;
    println!("Planned autonomous queue item");
    println!("{}", format_autonomy_queue_item(&item));
    print_autonomy_queue(memory, 10)
}

fn enqueue_operator_autonomy_goal(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    goal: &str,
    profile: WorkLoopProfile,
) -> Result<()> {
    let normalized_goal = sanitize_operator_goal(goal);
    if normalized_goal.is_empty() {
        anyhow::bail!("cannot enqueue an empty autonomous goal");
    }
    let cycles = match profile {
        WorkLoopProfile::Basic => 1,
        WorkLoopProfile::Core => 4,
        WorkLoopProfile::Commit => 6,
    };
    let priority = match profile {
        WorkLoopProfile::Basic => 45,
        WorkLoopProfile::Core => 55,
        WorkLoopProfile::Commit => 65,
    };
    let store = AutonomyQueueStore::new(Arc::clone(&memory.db));
    let item = store.enqueue(
        normalized_goal.clone(),
        "operator_run",
        profile.as_str(),
        cycles,
        priority,
    )?;
    events.append(
        None,
        None,
        "autonomy.queue.enqueued",
        format!(
            "operator enqueued autonomous work item {}: {}",
            short_fragment(&item.id),
            truncate(&normalized_goal, 100)
        ),
        serde_json::json!({
            "queue_id": item.id,
            "goal": item.goal,
            "kind": item.kind,
            "profile": item.profile,
            "cycles": item.cycles,
            "priority": item.priority,
            "source": "operator",
            "next_command": "cargo run -- --prof-x-step-live 1",
        }),
    )?;

    println!("Queued autonomous Professor X work");
    println!("{}", format_autonomy_queue_item(&item));
    println!("  execute-live: cargo run -- --prof-x-step-live 1");
    println!("  execute-quiet: cargo run -- --prof-x-step 1");
    println!("  watch: cargo run -- --observe-work");
    println!("  queue: cargo run -- --prof-x-queue 10");
    Ok(())
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
        None,
    )
    .await
}

async fn run_autonomy_queue_steps(
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    count: u32,
    publish_after_run: bool,
) -> Result<()> {
    let count = count.clamp(1, 10);
    let store = AutonomyQueueStore::new(Arc::clone(&memory.db));
    if store.count_pending()? == 0 {
        enqueue_planned_autonomy_item(Arc::clone(&memory), Arc::clone(&events))?;
    }

    println!("Professor X autonomy queue step");
    println!("  requested items: {count}");
    println!("  publish-after-run: {publish_after_run}");
    println!("  queue: cargo run -- --autonomy-queue 10");
    println!("  watch: cargo run -- --observe-work");

    for index in 1..=count {
        let Some(item) = store.next_pending()? else {
            println!("  no pending queue item at step {index}");
            break;
        };
        let profile = WorkLoopProfile::parse(&item.profile).unwrap_or(WorkLoopProfile::Core);
        store.mark_running(&item.id)?;
        events.append(
            None,
            None,
            "autonomy.queue.started",
            format!(
                "started autonomous queue item {}: {}",
                short_fragment(&item.id),
                truncate(&item.goal, 100)
            ),
            serde_json::json!({
                "queue_id": item.id,
                "goal": item.goal,
                "kind": item.kind,
                "profile": profile.as_str(),
                "cycles": item.cycles,
                "priority": item.priority,
                "step": index,
                "steps": count,
            }),
        )?;

        let before_run = WorkLoopRunStore::new(Arc::clone(&memory.db))
            .latest()?
            .map(|run| run.run_id);
        let result = run_supervised_loop(
            WorkLoopRunKind::Operator,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            item.cycles,
            profile,
            publish_after_run,
            Some(WorkLoopRunContext::from_queue_item(&item)),
        )
        .await;
        let latest_run = WorkLoopRunStore::new(Arc::clone(&memory.db)).latest()?;
        let new_run = latest_run.filter(|run| Some(run.run_id.clone()) != before_run);
        match result {
            Ok(()) => {
                store.mark_finished(
                    &item.id,
                    "done",
                    new_run.as_ref().map(|run| run.run_id.as_str()),
                    new_run.as_ref().map(|run| run.report_path.as_str()),
                    None,
                )?;
                events.append(
                    None,
                    None,
                    "autonomy.queue.completed",
                    format!(
                        "completed autonomous queue item {}",
                        short_fragment(&item.id)
                    ),
                    serde_json::json!({
                        "queue_id": item.id,
                        "result_run_id": new_run.as_ref().map(|run| run.run_id.clone()),
                        "result_report_path": new_run.as_ref().map(|run| run.report_path.clone()),
                        "result_journal_path": new_run.as_ref().and_then(run_journal_path),
                        "publish_after_run": publish_after_run,
                        "passed": true,
                    }),
                )?;
            }
            Err(err) => {
                let reason = err.to_string();
                store.mark_finished(
                    &item.id,
                    "failed",
                    new_run.as_ref().map(|run| run.run_id.as_str()),
                    new_run.as_ref().map(|run| run.report_path.as_str()),
                    Some(&reason),
                )?;
                events.append(
                    None,
                    None,
                    "autonomy.queue.failed",
                    format!(
                        "failed autonomous queue item {}: {}",
                        short_fragment(&item.id),
                        truncate(&reason, 100)
                    ),
                    serde_json::json!({
                        "queue_id": item.id,
                        "result_run_id": new_run.as_ref().map(|run| run.run_id.clone()),
                        "result_report_path": new_run.as_ref().map(|run| run.report_path.clone()),
                        "result_journal_path": new_run.as_ref().and_then(run_journal_path),
                        "publish_after_run": publish_after_run,
                        "passed": false,
                        "error": reason,
                    }),
                )?;
                return Err(err);
            }
        }
    }

    print_autonomy_queue(memory, 10)
}

async fn run_autonomy_queue_steps_live(
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    count: u32,
    publish_after_run: bool,
) -> Result<()> {
    let count = count.clamp(1, 10);
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live autonomy queue step");
    println!("  requested items: {count}");
    println!("  publish-after-run: {publish_after_run}");
    println!("  preview: cargo run -- --prof-x-preview-step");
    println!("  queue: cargo run -- --prof-x-queue 10");
    println!("  streaming work feed below");
    io::stdout().flush()?;

    let run_registry = Arc::clone(&registry);
    let run_policy = Arc::clone(&policy);
    let run_memory = Arc::clone(&memory);
    let run_events = Arc::clone(&events);
    let run_transcripts = Arc::clone(&transcripts);
    let mut handle = tokio::spawn(async move {
        run_autonomy_queue_steps(
            run_registry,
            run_policy,
            run_memory,
            run_events,
            run_transcripts,
            count,
            publish_after_run,
        )
        .await
    });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Live autonomy queue step interrupted.");
                handle.abort();
                anyhow::bail!("live autonomy queue step interrupted");
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
            None,
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
    println!(
        "{}",
        format_run_log_entry(&run, run_ledger_path(&run).as_deref())
    );
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
    context: Option<WorkLoopRunContext>,
) -> Result<()> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let started_at = chrono::Utc::now();
    let cycles = cycles.clamp(1, 50);
    let timeline_start_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    let recent_runs = WorkLoopRunStore::new(Arc::clone(&memory.db)).recent(5)?;
    let mut planned_jobs = plan_work_loop_jobs(run_kind, profile, cycles, &recent_runs);
    prioritize_planned_jobs_for_context(&mut planned_jobs, profile, context.as_ref());
    annotate_planned_jobs_with_context(&mut planned_jobs, context.as_ref());
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
            "queue_id": context.as_ref().and_then(|ctx| ctx.queue_id.clone()),
            "operator_goal": context.as_ref().and_then(|ctx| ctx.operator_goal.clone()),
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
                "queue_id": context.as_ref().and_then(|ctx| ctx.queue_id.clone()),
                "operator_goal": context.as_ref().and_then(|ctx| ctx.operator_goal.clone()),
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
                "queue_id": context.as_ref().and_then(|ctx| ctx.queue_id.clone()),
                "operator_goal": context.as_ref().and_then(|ctx| ctx.operator_goal.clone()),
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
            context.as_ref(),
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

        gate_store.finish(&run_id, cycle, passed, cycle_record, error.as_deref())?;

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
                "queue_id": context.as_ref().and_then(|ctx| ctx.queue_id.clone()),
                "operator_goal": context.as_ref().and_then(|ctx| ctx.operator_goal.clone()),
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
        queue_id: context.as_ref().and_then(|ctx| ctx.queue_id.clone()),
        operator_goal: context.as_ref().and_then(|ctx| ctx.operator_goal.clone()),
        started_at: started_at.to_rfc3339(),
        completed_at: chrono::Utc::now().to_rfc3339(),
        requested_cycles: cycles,
        completed_cycles: records.len() as u32,
        passed_cycles: records.iter().filter(|record| record.passed).count() as u32,
        failed_cycles,
        profile: profile.as_str().to_string(),
        ledger_path: None,
        journal_path: None,
        planned_jobs,
        smoke_records: records,
        timeline: work_timeline_from_events(&events.work_after_id(timeline_start_id, 1000)?),
    };
    let report_path = write_supervised_loop_report(&report)?;
    let ledger_path = write_work_loop_ledger(&report, &report_path)?;
    report.ledger_path = Some(ledger_path.display().to_string());
    let journal_path = write_work_loop_journal(&report)?;
    report.journal_path = Some(journal_path.display().to_string());
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
            "queue_id": report.queue_id.clone(),
            "operator_goal": report.operator_goal.clone(),
            "report_path": report_path,
            "ledger_path": ledger_path,
            "journal_path": journal_path,
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
    println!("  journal: {}", journal_path.display());
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
                "journal_path": journal_path.display().to_string(),
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
    context: Option<&WorkLoopRunContext>,
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
            let (report, path) = execute_evolution_proposal_dry_run(
                Arc::clone(&events),
                context.and_then(|ctx| ctx.operator_goal.clone()),
            )
            .await?;
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
            let operator_goal = context.and_then(|ctx| ctx.operator_goal.clone());
            let patch_path = write_patch_apply_commit_patch(operator_goal.as_deref())?;
            let outcome = execute_repo_patch_commit_coding_session_with_goal(
                policy,
                memory,
                Arc::clone(&events),
                patch_path,
                operator_goal,
            )
            .await?;
            Ok(WorkLoopSmokeRecord {
                cycle: 0,
                kind: job.kind().to_string(),
                smoke_id: None,
                passed: outcome.passed,
                report_path: outcome.verification_path.display().to_string(),
                transcript_path: None,
                workspace: outcome.verification.workspace,
                detail: format!(
                    "{} check(s), commit={}, diff_bytes={}, session={}",
                    outcome.verification.checks.len(),
                    outcome.verification.commit.as_deref().unwrap_or("none"),
                    outcome.verification.diff_bytes,
                    short_fragment(&outcome.session_id),
                ),
            })
        }
        WorkLoopJob::OperatorCommit => {
            let (report, path) = execute_operator_commit_smoke(
                Arc::clone(&events),
                context.and_then(|ctx| ctx.operator_goal.clone()),
            )
            .await?;
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
    if !matches!(
        record.kind.as_str(),
        "patch_apply_commit" | "operator_commit"
    ) {
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
        task_id: event
            .task_id
            .as_ref()
            .map(|id| short_fragment(id).to_string()),
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
        report_path: event.payload["report_path"]
            .as_str()
            .map(ToString::to_string),
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
        "Read line hashes and apply the smallest hash-anchored edit".to_string(),
        "Run cargo test again and keep command artifacts plus transcript".to_string(),
    ]
}

fn replacement_line_number(source: &str, target_line: &str) -> Result<usize> {
    let matches: Vec<usize> = source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| (line == target_line).then_some(index + 1))
        .collect();
    match matches.as_slice() {
        [line] => Ok(*line),
        [] => anyhow::bail!("replacement line not found in coding exercise source"),
        _ => anyhow::bail!("replacement line is ambiguous in coding exercise source"),
    }
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

    let window_action = Action {
        tool_name: "fs.window_open".to_string(),
        params: serde_json::json!({"path": "src/lib.rs", "lines": 40}),
        risk_score: 11,
    };
    let window_read = run_smoke_tool(
        &executor,
        Arc::clone(&policy),
        Arc::clone(&memory),
        &events,
        &scope,
        session_id,
        task.id,
        2,
        window_action.clone(),
    )
    .await?;
    record_smoke_step(
        &mut task,
        2,
        "read a bounded hash-anchored source window before editing",
        window_action,
        &window_read,
    );
    task_runs.step_recorded(&task)?;
    emit_smoke_tool_event(&events, session_id, task.id, 2, &task.steps[1])?;
    artifacts.extend(window_read.artifacts.clone());

    let edit_line = replacement_line_number(exercise.source, exercise.replacement_old)?;
    let edit_hash = crate::toolbridge::hashedit::line_hash(exercise.replacement_old, 3);
    let edit_action = Action {
        tool_name: "fs.hash_edit".to_string(),
        params: serde_json::json!({
            "path": "src/lib.rs",
            "line": edit_line,
            "hash": edit_hash,
            "new_text": exercise.replacement_new,
            "mode": "apply",
        }),
        risk_score: 40,
    };
    let edit = run_smoke_tool(
        &executor,
        Arc::clone(&policy),
        Arc::clone(&memory),
        &events,
        &scope,
        session_id,
        task.id,
        3,
        edit_action.clone(),
    )
    .await?;
    record_smoke_step(
        &mut task,
        3,
        "apply the minimal hash-anchored edit",
        edit_action,
        &edit,
    );
    task_runs.step_recorded(&task)?;
    emit_smoke_tool_event(&events, session_id, task.id, 3, &task.steps[2])?;
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
        4,
        final_action.clone(),
    )
    .await?;
    record_smoke_step(
        &mut task,
        4,
        "rerun tests after the fix",
        final_action,
        &final_test,
    );
    task_runs.step_recorded(&task)?;
    emit_smoke_tool_event(&events, session_id, task.id, 4, &task.steps[3])?;
    artifacts.extend(final_test.artifacts.clone());
    let final_test_passed = final_test.success;
    let passed = initial_test_failed && edit.success && final_test_passed;
    let durable_artifacts = persist_coding_smoke_artifacts(&workspace, task.id, &artifacts)?;
    if durable_artifacts != artifacts {
        rewrite_task_artifacts(&mut task, &artifacts, &durable_artifacts);
        events.append(
            None,
            Some(task.id),
            "coding.smoke.artifacts.persisted",
            format!(
                "persisted {} coding smoke artifact(s) into repo evidence",
                durable_artifacts.len()
            ),
            serde_json::json!({
                "workspace": workspace,
                "original_artifacts": artifacts,
                "artifacts": durable_artifacts,
            }),
        )?;
    }
    artifacts = durable_artifacts;
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
        println!("  fs.hash_edit applied: {}", edit.success);
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
    CodingSessionStore::new(Arc::clone(&memory.db)).insert(&CodingSessionRecord {
        id: session_id.clone(),
        generated_at,
        goal: requested_goal.clone(),
        exercise: exercise.name.to_string(),
        status: "running".to_string(),
        workspace: None,
        smoke_id: None,
        smoke_report_path: None,
        session_report_path: "pending".to_string(),
        transcript_path: None,
        artifacts: Vec::new(),
        checks: Vec::new(),
        plan_steps: plan_steps.clone(),
        step_outcomes: Vec::new(),
        failure_reason: None,
        recorded_at: chrono::Utc::now(),
    })?;
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
    let (report_path, _evidence_path) = persist_coding_session_terminal_report(
        Arc::clone(&memory),
        Arc::clone(&events),
        uuid::Uuid::parse_str(&session_id)?,
        generated_at,
        &mut report,
        "coding session evidence written to",
        "coding session report written to",
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
    run_repo_patch_coding_session_with_goal(policy, memory, events, patch_path, None).await
}

async fn run_repo_patch_coding_session_with_goal(
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    patch_path: PathBuf,
    session_goal: Option<String>,
) -> Result<()> {
    let session_id = uuid::Uuid::new_v4();
    let session_key = session_id.to_string();
    let generated_at = chrono::Utc::now();
    let patch_raw = std::fs::read_to_string(&patch_path)
        .map_err(|e| anyhow::anyhow!("cannot read patch '{}': {e}", patch_path.display()))?;
    let repo_root = default_repo_root();
    let default_goal = format!(
        "repo patch coding session: verify {} before touching main",
        patch_path.display()
    );
    let goal = session_goal.unwrap_or(default_goal);
    let plan_steps = vec![
        "Policy-gate the patch through patch.apply before sandbox work".to_string(),
        "Verify the unified diff in an isolated worktree".to_string(),
        "Run sandbox cargo check and reward-hacking/material-diff checks".to_string(),
        "Record a coding-session report that points at the verification artifact".to_string(),
    ];
    CodingSessionStore::new(Arc::clone(&memory.db)).insert(&CodingSessionRecord {
        id: session_key.clone(),
        generated_at,
        goal: goal.clone(),
        exercise: "repo_patch_verify".to_string(),
        status: "running".to_string(),
        workspace: Some("repo-root sandbox verification".to_string()),
        smoke_id: None,
        smoke_report_path: None,
        session_report_path: "pending".to_string(),
        transcript_path: None,
        artifacts: vec![patch_path.display().to_string()],
        checks: Vec::new(),
        plan_steps: plan_steps.clone(),
        step_outcomes: Vec::new(),
        failure_reason: None,
        recorded_at: chrono::Utc::now(),
    })?;

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
        let step_outcomes = vec![format!(
            "policy gate denied patch.apply check mode: {}",
            truncate(&gate.reason, 160)
        )];
        for (index, outcome) in step_outcomes.iter().enumerate() {
            events.append(
                Some(session_id),
                None,
                "coding.session.outcome",
                format!("outcome {}: {}", index + 1, truncate(outcome, 100)),
                serde_json::json!({
                    "session_id": session_key.clone(),
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
            status: "failed".to_string(),
            workspace: Some("repo-root sandbox verification".to_string()),
            smoke_id: None,
            smoke_report_path: None,
            session_report_path: None,
            transcript_path: None,
            checks: Vec::new(),
            plan_steps: plan_steps.clone(),
            step_outcomes,
            artifacts: vec![patch_path.display().to_string()],
            failure_reason: Some(format!("policy denied repo patch: {}", gate.reason)),
        };
        let _ = persist_coding_session_terminal_report(
            Arc::clone(&memory),
            Arc::clone(&events),
            session_id,
            generated_at,
            &mut report,
            "repo patch coding-session evidence written to",
            "repo patch coding-session report written to",
        )?;
        anyhow::bail!("policy denied repo patch: {}", gate.reason);
    }

    let (verification, verification_path) =
        match execute_patch_verify(Arc::clone(&events), patch_path.clone()).await {
            Ok(result) => result,
            Err(err) => {
                let reason = err.to_string();
                let step_outcomes = vec![
                    "policy gate allowed patch.apply check".to_string(),
                    format!(
                        "verify path aborted before verification artifact: {}",
                        truncate(&reason, 160)
                    ),
                ];
                for (index, outcome) in step_outcomes.iter().enumerate() {
                    events.append(
                        Some(session_id),
                        None,
                        "coding.session.outcome",
                        format!("outcome {}: {}", index + 1, truncate(outcome, 100)),
                        serde_json::json!({
                            "session_id": session_key.clone(),
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
                    status: "failed".to_string(),
                    workspace: Some("repo-root sandbox verification".to_string()),
                    smoke_id: None,
                    smoke_report_path: None,
                    session_report_path: None,
                    transcript_path: None,
                    checks: Vec::new(),
                    plan_steps: plan_steps.clone(),
                    step_outcomes,
                    artifacts: vec![patch_path.display().to_string()],
                    failure_reason: Some(reason),
                };
                let _ = persist_coding_session_terminal_report(
                    Arc::clone(&memory),
                    Arc::clone(&events),
                    session_id,
                    generated_at,
                    &mut report,
                    "repo patch coding-session evidence written to",
                    "repo patch coding-session report written to",
                )?;
                return Err(err);
            }
        };
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
    let (report_path, _evidence_path) = persist_coding_session_terminal_report(
        Arc::clone(&memory),
        Arc::clone(&events),
        session_id,
        generated_at,
        &mut report,
        "repo patch coding-session evidence written to",
        "repo patch coding-session report written to",
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
    run_repo_patch_coding_session_live_with_goal(policy, memory, events, patch_path, None).await
}

async fn run_repo_patch_coding_session_live_with_goal(
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    patch_path: PathBuf,
    session_goal: Option<String>,
) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live repo patch coding session");
    println!("Streaming policy, sandbox verification, and coding-session evidence. No changes will be applied.");
    io::stdout().flush()?;

    let run_events = Arc::clone(&events);
    let mut handle = tokio::spawn(async move {
        run_repo_patch_coding_session_with_goal(
            policy,
            memory,
            run_events,
            patch_path,
            session_goal,
        )
        .await
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
    run_repo_patch_commit_coding_session_with_goal(policy, memory, events, patch_path, None).await
}

async fn run_repo_patch_commit_coding_session_with_goal(
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    patch_path: PathBuf,
    session_goal: Option<String>,
) -> Result<()> {
    let outcome = execute_repo_patch_commit_coding_session_with_goal(
        policy,
        memory,
        events,
        patch_path,
        session_goal,
    )
    .await?;
    println!(
        "Repo patch commit coding session: {}",
        if outcome.passed { "passed" } else { "failed" }
    );
    println!("  session: {}", outcome.session_id);
    println!("  report: {}", outcome.session_report_path.display());
    println!("  evidence: {}", outcome.evidence_path.display());
    println!("  verification: {}", outcome.verification_path.display());
    println!("  patch: {}", outcome.verification.patch_path);
    println!("  checks: {}", outcome.verification.checks.join(", "));
    println!(
        "  commit: {}",
        outcome.verification.commit.as_deref().unwrap_or("none")
    );
    println!(
        "  report commit: {}",
        outcome
            .verification
            .report_commit
            .as_deref()
            .unwrap_or("none")
    );
    println!("  reason: {}", outcome.verification.reason);

    if !outcome.passed {
        anyhow::bail!("repo patch commit coding session failed");
    }
    Ok(())
}

async fn execute_repo_patch_commit_coding_session_with_goal(
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    patch_path: PathBuf,
    session_goal: Option<String>,
) -> Result<RepoPatchCommitCodingSessionOutcome> {
    let session_id = uuid::Uuid::new_v4();
    let session_key = session_id.to_string();
    let generated_at = chrono::Utc::now();
    let patch_raw = std::fs::read_to_string(&patch_path)
        .map_err(|e| anyhow::anyhow!("cannot read patch '{}': {e}", patch_path.display()))?;
    let repo_root = default_repo_root();
    let default_goal = format!(
        "repo patch coding session: verify, apply, and commit {}",
        patch_path.display()
    );
    let goal = session_goal.unwrap_or(default_goal);
    let plan_steps = vec![
        "Policy-gate the patch through patch.apply before sandbox work".to_string(),
        "Verify the unified diff in an isolated worktree".to_string(),
        "Apply the verified diff to main only if sandbox checks pass".to_string(),
        "Run main cargo check and create git commit evidence".to_string(),
        "Record a coding-session report that points at the apply artifact".to_string(),
    ];
    CodingSessionStore::new(Arc::clone(&memory.db)).insert(&CodingSessionRecord {
        id: session_key.clone(),
        generated_at,
        goal: goal.clone(),
        exercise: "repo_patch_apply_commit".to_string(),
        status: "running".to_string(),
        workspace: Some("repo-root verified apply commit".to_string()),
        smoke_id: None,
        smoke_report_path: None,
        session_report_path: "pending".to_string(),
        transcript_path: None,
        artifacts: vec![patch_path.display().to_string()],
        checks: Vec::new(),
        plan_steps: plan_steps.clone(),
        step_outcomes: Vec::new(),
        failure_reason: None,
        recorded_at: chrono::Utc::now(),
    })?;

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
        let step_outcomes = vec![format!(
            "policy gate denied patch.apply apply mode: {}",
            truncate(&gate.reason, 160)
        )];
        for (index, outcome) in step_outcomes.iter().enumerate() {
            events.append(
                Some(session_id),
                None,
                "coding.session.outcome",
                format!("outcome {}: {}", index + 1, truncate(outcome, 100)),
                serde_json::json!({
                    "session_id": session_key.clone(),
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
            status: "failed".to_string(),
            workspace: Some("repo-root verified apply commit".to_string()),
            smoke_id: None,
            smoke_report_path: None,
            session_report_path: None,
            transcript_path: None,
            checks: Vec::new(),
            plan_steps: plan_steps.clone(),
            step_outcomes,
            artifacts: vec![patch_path.display().to_string()],
            failure_reason: Some(format!("policy denied repo patch commit: {}", gate.reason)),
        };
        let _ = persist_coding_session_terminal_report(
            Arc::clone(&memory),
            Arc::clone(&events),
            session_id,
            generated_at,
            &mut report,
            "repo patch commit coding-session evidence written to",
            "repo patch commit coding-session report written to",
        )?;
        anyhow::bail!("policy denied repo patch commit: {}", gate.reason);
    }

    let (verification, verification_path) = match execute_patch_apply_commit(
        Arc::clone(&events),
        patch_path.clone(),
        Some(goal.clone()),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            let reason = err.to_string();
            let step_outcomes = vec![
                "policy gate allowed patch.apply apply mode".to_string(),
                format!(
                    "apply path aborted before verification artifact: {}",
                    truncate(&reason, 160)
                ),
            ];
            for (index, outcome) in step_outcomes.iter().enumerate() {
                events.append(
                    Some(session_id),
                    None,
                    "coding.session.outcome",
                    format!("outcome {}: {}", index + 1, truncate(outcome, 100)),
                    serde_json::json!({
                        "session_id": session_key.clone(),
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
                status: "failed".to_string(),
                workspace: Some("repo-root verified apply commit".to_string()),
                smoke_id: None,
                smoke_report_path: None,
                session_report_path: None,
                transcript_path: None,
                checks: Vec::new(),
                plan_steps: plan_steps.clone(),
                step_outcomes,
                artifacts: vec![patch_path.display().to_string()],
                failure_reason: Some(reason.clone()),
            };
            let _ = persist_coding_session_terminal_report(
                Arc::clone(&memory),
                Arc::clone(&events),
                session_id,
                generated_at,
                &mut report,
                "repo patch commit coding-session evidence written to",
                "repo patch commit coding-session report written to",
            )?;
            return Err(err);
        }
    };
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
                "session_id": session_key.clone(),
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
    let (report_path, evidence_path) = persist_coding_session_terminal_report(
        Arc::clone(&memory),
        Arc::clone(&events),
        session_id,
        generated_at,
        &mut report,
        "repo patch commit coding-session evidence written to",
        "repo patch commit coding-session report written to",
    )?;

    Ok(RepoPatchCommitCodingSessionOutcome {
        passed,
        session_id: session_key,
        session_report_path: report_path,
        evidence_path,
        verification,
        verification_path,
    })
}

async fn run_repo_patch_commit_coding_session_live(
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    patch_path: PathBuf,
) -> Result<()> {
    run_repo_patch_commit_coding_session_live_with_goal(policy, memory, events, patch_path, None)
        .await
}

async fn run_repo_patch_commit_coding_session_live_with_goal(
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    patch_path: PathBuf,
    session_goal: Option<String>,
) -> Result<()> {
    let mut last_id = events.tail(1)?.last().map(|event| event.id).unwrap_or(0);
    println!("Professor X live repo patch commit session");
    println!("Streaming policy, sandbox verification, main apply, cargo check, commit, and coding-session evidence.");
    io::stdout().flush()?;

    let run_events = Arc::clone(&events);
    let mut handle = tokio::spawn(async move {
        run_repo_patch_commit_coding_session_with_goal(
            policy,
            memory,
            run_events,
            patch_path,
            session_goal,
        )
        .await
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

fn persist_coding_smoke_artifacts(
    workspace: &Path,
    task_id: uuid::Uuid,
    artifacts: &[String],
) -> Result<Vec<String>> {
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let task_short = short_fragment(&task_id.to_string()).to_string();
    let dest_dir = PathBuf::from("artifacts")
        .join("coding-smoke")
        .join(date)
        .join(task_short)
        .join("evidence");
    let mut durable = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let source = PathBuf::from(artifact);
        if !source.starts_with(workspace) {
            durable.push(artifact.clone());
            continue;
        }
        let relative = source.strip_prefix(workspace).unwrap_or(&source);
        let destination = dest_dir.join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&source, &destination).with_context(|| {
            format!(
                "copy coding smoke artifact {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        durable.push(destination.to_string_lossy().to_string());
    }
    Ok(durable)
}

fn rewrite_task_artifacts(task: &mut TaskNode, original: &[String], durable: &[String]) {
    for step in &mut task.steps {
        for artifact in &mut step.observation.artifacts {
            if let Some(index) = original.iter().position(|candidate| candidate == artifact) {
                if let Some(replacement) = durable.get(index) {
                    *artifact = replacement.clone();
                }
            }
        }
    }
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

fn persist_coding_session_terminal_report(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    session_id: uuid::Uuid,
    generated_at: chrono::DateTime<chrono::Utc>,
    report: &mut CodingSessionReport,
    evidence_summary_prefix: &str,
    report_summary_prefix: &str,
) -> Result<(PathBuf, PathBuf)> {
    let (report_path, evidence_path) = finalize_coding_session_report(report)?;

    CodingSessionStore::new(Arc::clone(&memory.db)).insert(&CodingSessionRecord {
        id: report.id.clone(),
        generated_at,
        goal: report.requested_goal.clone(),
        exercise: report.exercise.clone(),
        status: report.status.clone(),
        workspace: report.workspace.clone(),
        smoke_id: report.smoke_id,
        smoke_report_path: report.smoke_report_path.clone(),
        session_report_path: report_path.display().to_string(),
        transcript_path: report.transcript_path.clone(),
        artifacts: report.artifacts.clone(),
        checks: report.checks.clone(),
        plan_steps: report.plan_steps.clone(),
        step_outcomes: report.step_outcomes.clone(),
        failure_reason: report.failure_reason.clone(),
        recorded_at: chrono::Utc::now(),
    })?;

    events.append(
        Some(session_id),
        None,
        "coding.session.evidence_written",
        format!("{evidence_summary_prefix} {}", evidence_path.display()),
        serde_json::json!({
            "session_id": report.id.clone(),
            "exercise": report.exercise.clone(),
            "session_report_path": report_path.display().to_string(),
            "evidence_path": evidence_path.display().to_string(),
            "artifacts": report.artifacts.clone(),
        }),
    )?;

    events.append(
        Some(session_id),
        None,
        if report.status == "passed" {
            "coding.session.passed"
        } else {
            "coding.session.failed"
        },
        format!("{report_summary_prefix} {}", report_path.display()),
        serde_json::to_value(&report)?,
    )?;

    Ok((report_path, evidence_path))
}

fn finalize_coding_session_report(report: &mut CodingSessionReport) -> Result<(PathBuf, PathBuf)> {
    let report_path = write_coding_session_report(report)?;
    let evidence_path = attach_coding_session_evidence(report, &report_path)?;
    Ok((report_path, evidence_path))
}

fn attach_coding_session_evidence(
    report: &mut CodingSessionReport,
    report_path: &std::path::Path,
) -> Result<PathBuf> {
    report.session_report_path = Some(report_path.display().to_string());
    let evidence_path = write_coding_session_evidence(report, report_path)?;
    let evidence_path_text = evidence_path.display().to_string();
    if !report
        .artifacts
        .iter()
        .any(|path| path == &evidence_path_text)
    {
        report.artifacts.push(evidence_path_text);
    }
    std::fs::write(report_path, serde_json::to_string_pretty(report)?)?;
    Ok(evidence_path)
}

fn coding_session_evidence_markdown_path(report_path: &std::path::Path) -> PathBuf {
    report_path.with_extension("evidence.md")
}

fn write_coding_session_evidence(
    report: &CodingSessionReport,
    report_path: &std::path::Path,
) -> Result<PathBuf> {
    let path = coding_session_evidence_markdown_path(report_path);
    std::fs::write(&path, format_coding_session_evidence(report, report_path))?;
    Ok(path)
}

fn format_coding_session_evidence(
    report: &CodingSessionReport,
    report_path: &std::path::Path,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Professor X coding session evidence {}",
        short_fragment(&report.id)
    ));
    lines.push(format!("  session: {}", report.id));
    lines.push(format!("  status: {}", report.status));
    lines.push(format!("  exercise: {}", report.exercise));
    lines.push(format!("  generated: {}", report.generated_at));
    lines.push(format!("  goal: {}", truncate(&report.goal, 180)));
    lines.push(format!("  report: {}", report_path.display()));
    if let Some(workspace) = &report.workspace {
        lines.push(format!("  workspace: {}", truncate(workspace, 180)));
    }
    if let Some(smoke_report) = &report.smoke_report_path {
        lines.push(format!("  smoke_report: {smoke_report}"));
    }
    if let Some(transcript) = &report.transcript_path {
        lines.push(format!("  transcript: {transcript}"));
    }
    if let Some(reason) = &report.failure_reason {
        lines.push(format!("  failure: {}", truncate(reason, 220)));
    }

    lines.push(String::new());
    lines.push(format!("Plan steps: {}", report.plan_steps.len()));
    for (index, step) in report.plan_steps.iter().enumerate() {
        lines.push(format!("  {}. {}", index + 1, truncate(step, 220)));
    }

    lines.push(String::new());
    lines.push(format!("Outcomes: {}", report.step_outcomes.len()));
    for (index, outcome) in report.step_outcomes.iter().enumerate() {
        lines.push(format!("  {}. {}", index + 1, truncate(outcome, 220)));
    }

    lines.push(String::new());
    lines.push(format!("Checks: {}", report.checks.len()));
    for check in &report.checks {
        lines.push(format!("  - {check}"));
    }

    lines.push(String::new());
    lines.push(format!("Artifacts: {}", report.artifacts.len()));
    for artifact in &report.artifacts {
        lines.push(format!("  - {}", truncate(artifact, 220)));
    }

    lines.push(String::new());
    lines.push(format!(
        "Review: cargo run -- --prof-x-code-review {}",
        short_fragment(&report.id)
    ));
    lines.push(format!(
        "Publish: cargo run -- --prof-x-code-publish {}",
        short_fragment(&report.id)
    ));
    lines.join("\n")
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

fn write_work_loop_journal(report: &SupervisedLoopReport) -> Result<PathBuf> {
    let repo_root = default_repo_root();
    let completed_at = chrono::DateTime::parse_from_rfc3339(&report.completed_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());
    let dir = std::env::var("PROFESSOR_X_WORK_LOOP_LEDGER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("artifacts")
                .join("work-loop")
                .join("ledger")
                .join(completed_at.format("%Y-%m-%d").to_string())
        });
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "prof-x-journal-{}.md",
        short_fragment(&report.run_id)
    ));
    let commit_id = git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string());
    std::fs::write(
        &path,
        format_work_loop_journal_markdown(&repo_root, report, completed_at, &commit_id),
    )?;
    Ok(path)
}

fn format_work_loop_ledger(report: &SupervisedLoopReport, report_path: &std::path::Path) -> String {
    let repo_root = default_repo_root();
    let mut out = Vec::new();
    out.push(format!(
        "# Professor X Run {}",
        short_fragment(&report.run_id)
    ));
    out.push(String::new());
    out.push(format!("- run_id: `{}`", report.run_id));
    out.push(format!("- kind: `{}`", report.run_kind));
    out.push(format!("- profile: `{}`", report.profile));
    if let Some(queue_id) = &report.queue_id {
        out.push(format!("- queue_id: `{queue_id}`"));
    }
    if let Some(goal) = &report.operator_goal {
        out.push(format!("- operator_goal: {}", truncate(goal, 180)));
    }
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
            out.push(format!(
                "- ... {} more event(s)",
                report.timeline.len() - 80
            ));
        }
    }
    out.push(String::new());
    out.join("\n")
}

fn format_work_loop_journal_markdown(
    repo_root: &std::path::Path,
    report: &SupervisedLoopReport,
    timestamp: chrono::DateTime<chrono::Utc>,
    commit_id: &str,
) -> String {
    let git_line = cockpit_git_line(repo_root);
    let status_raw = command_stdout(repo_root, "git", &["status", "--short"])
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "clean".to_string());
    let mut lines = vec![
        format!(
            "# Professor X Work Journal - {}",
            short_fragment(&report.run_id)
        ),
        String::new(),
        "## Run Context".to_string(),
        format!("- generated_at: {}", timestamp.to_rfc3339()),
        format!("- run_id: {}", report.run_id),
        format!("- kind: {}", report.run_kind),
        format!("- profile: {}", report.profile),
        format!("- harness_commit: {commit_id}"),
        format!("- git: {git_line}"),
        format!(
            "- cycles: {}/{} completed, {} passed, {} failed",
            report.completed_cycles,
            report.requested_cycles,
            report.passed_cycles,
            report.failed_cycles
        ),
        format!("- timeline_events: {}", report.timeline.len()),
    ];
    if let Some(queue_id) = &report.queue_id {
        lines.push(format!("- queue_id: {queue_id}"));
    }
    if let Some(goal) = &report.operator_goal {
        lines.push(format!("- operator_goal: {}", truncate(goal, 180)));
    }
    if let Some(ledger) = &report.ledger_path {
        lines.push(format!("- ledger: {ledger}"));
    }

    lines.push(String::new());
    lines.push("## Working Tree".to_string());
    if status_raw == "clean" {
        lines.push("- clean".to_string());
    } else {
        for line in status_raw.lines().take(40) {
            lines.push(format!("- `{}`", line));
        }
        if status_raw.lines().count() > 40 {
            lines.push("- ... truncated".to_string());
        }
    }

    lines.push(String::new());
    lines.push("## Timeline".to_string());
    if report.timeline.is_empty() {
        lines.push("- no work events recorded in this run".to_string());
    } else {
        for entry in &report.timeline {
            lines.push(format_work_replay_entry(entry));
        }
    }

    lines.push(String::new());
    lines.push("## Operator Commands".to_string());
    lines.push(format!(
        "- `cargo run -- --replay {}`",
        short_fragment(&report.run_id)
    ));
    lines.push(format!(
        "- `cargo run -- --run-review {}`",
        short_fragment(&report.run_id)
    ));
    lines.push(format!(
        "- `cargo run -- --publish-run {}`",
        short_fragment(&report.run_id)
    ));
    lines.push(String::new());
    lines.join("\n")
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

/// Load real recent task outcomes from `task_runs` so the evolution cycle
/// learns from what actually happened (e.g. the HIRO round just run), not from
/// seeded calibration data. Only finished runs are included.
fn outcomes_from_recent_runs(memory: &Arc<MemoryManager>, limit: usize) -> Vec<TaskOutcome> {
    let store = TaskRunStore::new(Arc::clone(&memory.db));
    let runs = store.recent(limit).unwrap_or_default();
    runs.into_iter()
        .filter(|r| matches!(r.status.as_str(), "Complete" | "Failed"))
        .map(|r| {
            let success = r.status == "Complete";
            let failure_mode = r.failure_mode.map(|mode| normalize_failure_mode(&mode));
            TaskOutcome {
                task_id: uuid::Uuid::parse_str(&r.task_id).unwrap_or_else(|_| uuid::Uuid::new_v4()),
                description: r.description,
                success,
                score: r.outcome_score.unwrap_or(if success { 1.0 } else { 0.0 }),
                failure_class: if success {
                    None
                } else {
                    r.failure_class
                        .or_else(|| failure_mode.as_deref().map(classify_failure_mode))
                },
                failure_mode,
                steps_taken: r.step_count as u32,
                timestamp: r.completed_at.unwrap_or(r.updated_at),
            }
        })
        .collect()
}

/// Load REAL HIRO outcomes (correctness, not did-finish) from the latest round's
/// `hiro_attempts`. A task passes if ANY of its attempts passed (pass@3); the
/// failure_mode of a failed attempt is carried through so DHE-tagged patterns
/// reach the Researcher. This is the correct learning signal — task_runs only
/// records whether the agent called finish, not whether it was right.
fn outcomes_from_hiro_attempts(memory: &Arc<MemoryManager>) -> Vec<TaskOutcome> {
    use std::collections::HashMap;
    let db = memory.db.lock().unwrap();
    let latest: Option<i64> = db
        .query_row("SELECT MAX(round) FROM hiro_attempts", [], |r| r.get(0))
        .ok()
        .flatten();
    let Some(round) = latest else {
        return Vec::new();
    };

    let mut stmt = match db.prepare(
        "SELECT task_id, category, passed, failure_reason, duration_ms
         FROM hiro_attempts WHERE round = ?1",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map([round], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)? != 0,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, i64>(4)?,
        ))
    });
    let Ok(rows) = rows else { return Vec::new() };

    // Aggregate attempts per task: passed if any attempt passed.
    let mut agg: HashMap<String, (String, bool, Option<String>, i64)> = HashMap::new();
    for row in rows.flatten() {
        let (task_id, category, passed, failure_reason, dur) = row;
        let e = agg
            .entry(task_id)
            .or_insert((category.clone(), false, None, 0));
        e.1 |= passed;
        if !passed && e.2.is_none() {
            e.2 = failure_reason;
        }
        e.3 += dur;
    }

    agg.into_iter()
        .map(
            |(task_id, (category, passed, failure_reason, dur))| TaskOutcome {
                task_id: uuid::Uuid::new_v4(),
                description: format!("HIRO {category} task {task_id}"),
                success: passed,
                score: if passed { 1.0 } else { 0.0 },
                failure_class: if passed {
                    None
                } else {
                    failure_reason.as_deref().map(classify_failure_mode)
                },
                failure_mode: if passed {
                    None
                } else {
                    failure_reason.map(|reason| normalize_failure_mode(&reason))
                },
                steps_taken: 0,
                timestamp: chrono::Utc::now() - chrono::Duration::milliseconds(dur),
            },
        )
        .collect()
}

/// Run one evolution cycle that learns from REAL recent outcomes. Prefers HIRO
/// correctness; falls back to task_runs, then seeded calibration.
async fn run_live_evolution_cycle(
    ollama: Arc<ollama::OllamaClient>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
) -> Result<()> {
    let mut tracker = OutcomeTracker::new();
    let hiro = outcomes_from_hiro_attempts(&memory);
    let (outcomes, source) = if !hiro.is_empty() {
        (hiro, "real HIRO outcomes (correctness)")
    } else {
        let runs = outcomes_from_recent_runs(&memory, 40);
        if runs.is_empty() {
            (seeded_evolution_outcomes(), "seeded (no runs yet)")
        } else {
            (runs, "task_runs (did-finish)")
        }
    };
    for outcome in outcomes {
        tracker.record(outcome);
    }

    println!("Evolution cycle (live) — learning from {source}");
    println!(
        "  outcomes={}  success_rate(20)={:.0}%  failure_patterns={:?}",
        tracker.len(),
        tracker.success_rate(20) * 100.0,
        tracker.failure_patterns(20),
    );
    events.append(
        None,
        None,
        "evolution.live_cycle.started",
        format!("starting live evolution cycle from {source}"),
        serde_json::json!({
            "outcomes": tracker.len(),
            "success_rate_20": tracker.success_rate(20),
            "failure_patterns": tracker.failure_patterns(20),
        }),
    )?;

    let evolved = EvolvedLoop::new(ollama, memory).with_events(Arc::clone(&events));
    let applied = evolved.run_cycle(&tracker).await?;
    println!(
        "Result: {}",
        if applied {
            "APPLIED a harness change"
        } else {
            "no change this cycle"
        }
    );
    println!("  watch: ./prof-x-stream.py     artifacts: find artifacts/evolution -type f | sort");
    Ok(())
}

/// Continuous evolution mining (the "inference-mining" loop). Each block:
///   1. evolve from real outcomes (may commit a harness change)
///   2. if a change committed, MEASURE it on a fixed task subset
///   3. KEEP if pass@3 beats the best so far, else ROLL BACK (git reset)
/// Runs `max_iters` blocks (0 = until interrupted). The harness only keeps
/// changes that demonstrably help — selection pressure for self-improvement.
#[allow(clippy::too_many_arguments)]
async fn run_evolve_forever(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
    max_iters: u32,
    measure_limit: usize,
) -> Result<()> {
    let repo_root = default_repo_root();
    let mut round = next_hiro_round(&memory);

    println!("evolve-forever: establishing baseline on {measure_limit} tasks (frozen harness)...");
    let mut best = measure_block(
        &ollama,
        &registry,
        &policy,
        &memory,
        &events,
        &cancel,
        round,
        measure_limit,
    )
    .await?;
    round += 1;
    println!("evolve-forever: baseline pass@3 = {best:.3}\n");

    let mut iter = 0u32;
    let mut kept = 0u32;
    loop {
        iter += 1;
        if max_iters != 0 && iter > max_iters {
            break;
        }
        if cancel.is_cancelled() {
            println!("evolve-forever: cancelled.");
            break;
        }
        println!("── mining block {iter} ──");

        let head_before = git_head(&repo_root)?;
        // 1. evolve (commits a verified, identity-safe change if one wins)
        run_live_evolution_cycle(
            Arc::clone(&ollama),
            Arc::clone(&memory),
            Arc::clone(&events),
        )
        .await?;
        let head_after = git_head(&repo_root)?;
        if head_after == head_before {
            println!("  no change applied this block — continuing\n");
            continue;
        }
        println!("  applied {head_after}; measuring on {measure_limit} tasks...");

        // 2. measure
        let p = measure_block(
            &ollama,
            &registry,
            &policy,
            &memory,
            &events,
            &cancel,
            round,
            measure_limit,
        )
        .await?;
        round += 1;

        // 3. keep or roll back
        let verdict = if p > best + 0.001 {
            best = p;
            kept += 1;
            "KEEP ✓"
        } else {
            git_reset_hard(&repo_root, &head_before)?;
            "ROLLBACK ✗"
        };
        println!("  block {iter}: pass@3={p:.3}  best={best:.3}  → {verdict}\n");
        let _ = events.append(
            None,
            None,
            "evolve.forever.block",
            format!("block {iter}: pass@3={p:.3} best={best:.3} {verdict}"),
            serde_json::json!({"block": iter, "pass_at_3": p, "best": best, "verdict": verdict}),
        );
    }

    println!(
        "evolve-forever stopped after {} block(s); {kept} change(s) kept. best pass@3 = {best:.3}",
        iter.saturating_sub(1)
    );
    Ok(())
}

/// Run one measurement block: a HIRO round limited to `limit` tasks. Returns pass@3.
#[allow(clippy::too_many_arguments)]
async fn measure_block(
    ollama: &Arc<ollama::OllamaClient>,
    registry: &Arc<std::sync::RwLock<ToolRegistry>>,
    policy: &Arc<PolicyEngine>,
    memory: &Arc<MemoryManager>,
    events: &Arc<EventStore>,
    cancel: &CancellationToken,
    round: u32,
    limit: usize,
) -> Result<f32> {
    let runner = HiroRunner::new(
        Arc::clone(ollama),
        Arc::clone(registry),
        Arc::clone(policy),
        Arc::clone(memory),
        cancel.clone(),
    )
    .with_events(Arc::clone(events));
    let result = runner
        .run_benchmark_labeled_with_limit(round, Some("evolve_forever"), Some(limit))
        .await?;
    Ok(result.pass_at_3)
}

/// Run the agent's own self-authored tests — the agent-authored benchmark.
/// Each test (description + the agent's own pass criterion) is run through the
/// ReAct loop and judged by an LLM-as-evaluator against that criterion. The
/// pass rate is the signal for the central invention: does the agent's
/// self-diagnosis of what it should be able to do track real capability?
#[allow(clippy::too_many_arguments)]
/// Author a diverse self-curriculum: prompt the model to invent N concrete
/// tasks across the HIRO-style categories, grounded in the REAL tool set, each
/// with an objective evaluator. Stored in self_authored_tests for
/// --run-self-tests to execute (verified-correct runs feed the distillation
/// corpus). This is the fix for the corpus-diversity ceiling: the fixed 60-task
/// benchmark caps unique trajectories at ~40, but a self-authored curriculum is
/// unbounded — Professor X writes its own lessons (DMN / self-authored-tests
/// seeds), then learns from the ones it can verifiably solve.
async fn generate_curriculum(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    memory: Arc<MemoryManager>,
    target: usize,
) -> Result<()> {
    // Ground generation in the actual tools, so tasks are solvable here and not
    // hallucinated against capabilities the agent lacks.
    let tool_lines: Vec<String> = {
        let reg = registry.read().unwrap();
        reg.list()
            .iter()
            .filter(|m| m.risk_score < 80) // skip the most dangerous tools as task targets
            .map(|m| {
                format!(
                    "- {}: {}",
                    m.name,
                    m.description.chars().take(90).collect::<String>()
                )
            })
            .collect()
    };
    let tool_catalog = tool_lines.join("\n");

    // Dedup against what already exists (normalized).
    let norm = |s: &str| {
        s.trim()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut existing: std::collections::HashSet<String> = memory
        .self_authored_tests
        .all()?
        .iter()
        .map(|t| norm(&t.description))
        .collect();

    let categories = ["tool_use", "planning", "self_correction", "reasoning"];
    println!(
        "Authoring up to {target} self-curriculum tasks across {} categories \
         (grounded in {} tools)...\n",
        categories.len(),
        tool_lines.len()
    );

    // Balance: don't let one batch of a single category dominate the bank.
    let per_cat_cap = (target + categories.len() - 1) / categories.len() + 1;
    let mut inserted = 0usize;
    let mut by_cat: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut round = 0usize;
    // Cap attempts so a stubborn model can't loop forever.
    while inserted < target && round < target * 2 + 8 {
        round += 1;
        let category = categories[round % categories.len()];
        if by_cat.get(category).copied().unwrap_or(0) >= per_cat_cap {
            continue; // this category is full; rotate to the next
        }
        let prompt = format!(
            "You are designing a training curriculum for an autonomous agent that \
             runs on a single Linux machine. Author {batch} DISTINCT, concrete tasks \
             of category `{category}` that the agent can complete using ONLY these tools:\n\n\
             {tool_catalog}\n\n\
             Category meaning:\n\
             - tool_use: a task requiring correct invocation of one or more tools to \
             produce a checkable READ-ONLY result (read a file, compute via shell, search).\n\
             - planning: a multi-step task sequencing several tool calls toward an \
             answer; if it writes, it writes ONLY under /tmp.\n\
             - self_correction: a task with a likely first-attempt mistake the agent \
             must detect and fix (a wrong path, a parse error to recover from).\n\
             - reasoning: a task whose answer requires derivation/analysis, with a \
             verifiable final answer.\n\n\
             HARD SAFETY RULES (a task that violates ANY of these is rejected):\n\
             - NEVER delete, remove, rm, overwrite, truncate, drop, or move existing files.\n\
             - NEVER modify the repository, git history, or any file outside /tmp.\n\
             - The task must be ANSWER-PRODUCING or READ-ONLY; the only writes allowed \
             are NEW files under /tmp.\n\
             - Solvable in under 8 steps, SINGLE objective correct outcome, no network \
             beyond web_search.\n\n\
             SELF-CONTAINMENT (critical — a task referencing a file that does not \
             exist is unsolvable and useless): each task must be SELF-CONTAINED. If it \
             needs an input file, the FIRST step of the task is to CREATE that input \
             under /tmp with specified contents, then operate on it. Do NOT assume any \
             file exists except real system files (e.g. /etc/os-release, /proc/cpuinfo). \
             Example good DESCRIPTION: 'Create /tmp/nums.txt containing the numbers 4, \
             15, 8, 16, 23 one per line, then compute their sum using shell.'\n\n\
             Make them genuinely varied — different inputs, computations, and goals.\n\n\
             Output ONLY repeated blocks in EXACTLY this format, nothing else:\n\
             ===TASK===\n\
             DESCRIPTION: <one concrete, self-contained, non-destructive task>\n\
             EVALUATOR: <objective pass criterion a judge can check from the agent's actions>\n\
             ===END===",
            batch = 3,
            category = category,
            tool_catalog = tool_catalog,
        );

        let resp = match ollama
            .generate(
                &prompt,
                Some("You design concise, objectively-checkable agent tasks. Follow the format exactly."),
                Some(ollama::ModelOptions {
                    temperature: Some(0.9),
                    num_ctx: Some(8192),
                    top_p: Some(0.95),
                    stop: None,
                    think: Some(false),
                }),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("curriculum: generation failed: {e}");
                continue;
            }
        };
        let (_, text) = resp.split_thinking();

        for block in text.split("===TASK===").skip(1) {
            if inserted >= target {
                break;
            }
            let body = block.split("===END===").next().unwrap_or("");
            let description = extract_curriculum_field(body, "DESCRIPTION:");
            let evaluator = extract_curriculum_field(body, "EVALUATOR:");
            let (Some(description), Some(evaluator)) = (description, evaluator) else {
                continue;
            };
            if description.len() < 12 || evaluator.len() < 6 {
                continue;
            }
            // Hard safety gate: reject any task whose text implies mutating or
            // destroying real files. These tasks RUN in the agent's workspace,
            // so a generated "delete all .log files" would damage the repo. The
            // policy engine is a backstop, not a license to author danger.
            if curriculum_is_destructive(&description) {
                warn!(
                    "curriculum: rejected destructive task: {}",
                    description.chars().take(60).collect::<String>()
                );
                continue;
            }
            let key = norm(&description);
            if !existing.insert(key) {
                continue; // duplicate
            }
            let test = crate::memd::self_authored_tests::SelfAuthoredTest::new(
                0, // origin_round 0 = curriculum-authored (not evolution-derived)
                0,
                format!("self-curriculum:{category}"),
                description.clone(),
                evaluator,
                category,
            );
            match memory.self_authored_tests.insert(&test) {
                Ok(_) => {
                    inserted += 1;
                    *by_cat.entry(category).or_insert(0) += 1;
                    println!(
                        "  [{category}] {}",
                        description.chars().take(74).collect::<String>()
                    );
                }
                Err(e) => warn!("curriculum: insert failed: {e}"),
            }
        }
    }

    println!("\nAuthored {inserted} new task(s). By category: {by_cat:?}");
    let total = memory.self_authored_tests.count()?;
    println!("Self-authored task bank now holds {total} task(s).");
    println!("Next: ./target/release/professor-x --run-self-tests {inserted}");
    println!("      (verified-correct runs are collected as trajectories for distill/curate.py)");
    Ok(())
}

/// Reject a curriculum task if its text implies destroying or mutating real
/// files. Curriculum tasks execute in the live workspace, so this gate is a
/// safety boundary, not a style preference. Conservative by design: a
/// non-destructive task wrongly skipped costs one slot; a destructive task
/// wrongly run can delete the repo.
fn curriculum_is_destructive(description: &str) -> bool {
    let d = description.to_lowercase();
    // Writing to /tmp is explicitly allowed; only flag destructive verbs.
    const DANGER: &[&str] = &[
        "delete",
        "remove",
        " rm ",
        "rm -",
        "unlink",
        "overwrite",
        "truncate",
        "drop table",
        "wipe",
        "erase",
        "purge",
        "destroy",
        "format ",
        "git reset",
        "git clean",
        "git push",
        "force push",
        "shred",
        "move all",
        "rename all",
        "chmod -r",
        "chown -r",
        "> /",
        "rmdir",
    ];
    DANGER.iter().any(|k| d.contains(k))
}

/// Pull a single-line field value following `label` from a curriculum block.
fn extract_curriculum_field(block: &str, label: &str) -> Option<String> {
    for line in block.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(label) {
            let v = rest.trim().trim_matches('"').trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

async fn run_self_authored_tests(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
    limit: usize,
) -> Result<()> {
    let tests = memory.self_authored_tests.pending_for_round(limit)?;
    if tests.is_empty() {
        println!("No self-authored tests yet — they accrue as Professor X evolves.");
        return Ok(());
    }
    println!(
        "Running {} self-authored test(s) — the agent-authored benchmark\n",
        tests.len()
    );

    let mut passed = 0usize;
    for test in &tests {
        let Some(id) = test.id else { continue };
        let react = ReactLoop::new(
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            cancel.clone(),
        )
        .with_events(Arc::clone(&events));
        let mut task = TaskNode::new(test.description.clone(), TaskType::Research, 50);
        task.max_attempts = 2;
        let _ = react.run(&mut task).await;

        let evidence = task.recent_steps_text(4);
        let pass = judge_self_test(&ollama, &test.description, &test.evaluator, &evidence).await;
        let _ = memory.self_authored_tests.record_outcome(id, pass);
        if pass {
            passed += 1;
            // Judge-gated self-distillation corpus: only the LLM-judge-verified
            // trajectory becomes a lesson, never a merely agent-finished run.
            ReactLoop::collect_trajectory(&task);
        }
        println!(
            "  test #{id} [{}]  {}  — {}",
            test.category,
            if pass { "PASS" } else { "FAIL" },
            test.description.chars().take(70).collect::<String>()
        );
    }

    let rate = passed as f32 / tests.len() as f32;
    println!(
        "\nself-authored: {passed}/{} passed ({:.0}%) this run",
        tests.len(),
        rate * 100.0
    );
    if let Some(overall) = memory.self_authored_tests.mean_pass_rate()? {
        println!("mean pass-rate across all self-authored tests: {overall:.2}");
    }
    println!(
        "(the thesis test: does this track HIRO pass@3 over rounds? — see --consciousness-report)"
    );
    Ok(())
}

/// LLM-as-judge: given the test goal, the agent's own pass criterion, and the
/// evidence from its attempt, decide PASS/FAIL.
async fn judge_self_test(
    ollama: &Arc<ollama::OllamaClient>,
    description: &str,
    evaluator: &str,
    evidence: &str,
) -> bool {
    let prompt = format!(
        "Judge whether an agent passed a test.\n\n\
         Test goal: {description}\n\
         Pass criterion: {evaluator}\n\n\
         What the agent actually did (its last steps and observations):\n{}\n\n\
         Did the agent satisfy the pass criterion? Answer with exactly one word: PASS or FAIL.",
        evidence.chars().take(1500).collect::<String>(),
    );
    match ollama
        .generate(
            &prompt,
            Some("You are a strict, fair test evaluator. Answer PASS or FAIL only."),
            Some(ollama::ModelOptions {
                temperature: Some(0.0),
                num_ctx: Some(4096),
                top_p: None,
                stop: None,
                think: Some(false),
            }),
        )
        .await
    {
        Ok(resp) => {
            let (_, answer) = resp.split_thinking();
            answer.trim().to_uppercase().starts_with("PASS")
        }
        Err(_) => false,
    }
}

/// Next unused HIRO round number (max recorded + 1, or 0).
fn next_hiro_round(memory: &Arc<MemoryManager>) -> u32 {
    let db = memory.db.lock().unwrap();
    let max: Option<i64> = db
        .query_row("SELECT MAX(round) FROM hiro_rounds", [], |r| r.get(0))
        .ok()
        .flatten();
    max.map(|m| (m + 1) as u32).unwrap_or(0)
}

/// Roll the harness back to a prior commit (discards a rejected evolution).
fn git_reset_hard(repo_root: &std::path::Path, commit: &str) -> Result<()> {
    let out = std::process::Command::new("git")
        .args(["reset", "--hard", commit])
        .current_dir(repo_root)
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git reset --hard {commit} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
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
                failure_class: if success {
                    None
                } else {
                    Some(FailureClass::ToolSelection)
                },
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
                                        outcome.failure_class =
                                            Some(classify_failure_mode(&failure));
                                        outcome.failure_mode =
                                            Some(normalize_failure_mode(&failure));
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
    let description = util::expand_file_refs(&description); // @file → inline context
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
    if let Some(class) = outcome.failure_class {
        info!("failure_class: {}", class.as_str());
    }
    Ok(())
}

/// M1: offline repo-fix benchmark. Each task is a self-contained mini-repo with a planted
/// bug + a test. Deterministic, ungameable: pass iff the repo's own test goes red→green
/// after the agent's edit (no LLM-judge — see docs/research/eval-trust.md for why).
/// Measure repo-fix pass@1 once, optionally with a candidate system-prompt override.
/// Returns (passed, ran). Shared by `--repo-fix-bench` and the M4 evolution loop.
#[derive(Debug, Clone, serde::Serialize)]
struct RepoFixTaskResult {
    id: String,
    description: String,
    setup: String,
    verify_cmd: String,
    pre_exit: i32,
    post_exit: i32,
    expect_exit: i32,
    passed: bool,
    made_edit: bool,
    workdir: Option<String>,
    transcript_path: Option<String>,
    diff_summary: String,
}

#[derive(Debug, Clone)]
struct RepoFixMeasureResult {
    passed: usize,
    ran: usize,
    failures: Vec<(String, String, bool)>,
    tasks: Vec<RepoFixTaskResult>,
}

#[derive(Debug, serde::Serialize)]
struct RepoFixBenchArtifact {
    run_id: String,
    recorded_at: String,
    harness_commit: String,
    manifest_path: String,
    model: String,
    passed: usize,
    ran: usize,
    pass_at_1: f64,
    tasks: Vec<RepoFixTaskResult>,
}

#[allow(clippy::too_many_arguments)]
async fn repo_fix_measure(
    ollama: &Arc<ollama::OllamaClient>,
    registry: &Arc<std::sync::RwLock<ToolRegistry>>,
    policy: &Arc<PolicyEngine>,
    memory: &Arc<MemoryManager>,
    events: &Arc<EventStore>,
    transcripts: Option<Arc<TranscriptStore>>,
    cancel: &CancellationToken,
    prompt_override: Option<&str>,
    verbose: bool,
) -> Result<RepoFixMeasureResult> {
    use std::process::Command;

    #[derive(serde::Deserialize)]
    struct RepoFixTask {
        id: String,
        #[allow(dead_code)]
        category: String,
        setup: String,
        description: String,
        verify_cmd: String,
        expect_exit: i32,
    }
    #[derive(serde::Deserialize)]
    struct RepoFixFile {
        tasks: Vec<RepoFixTask>,
    }

    // REPO_FIX_TASKS overrides the manifest (e.g. tasks_corpus.json = curated + generated, for
    // distillation corpus collection). Default = the trustworthy 14-task headline benchmark.
    let manifest_path = std::env::var("REPO_FIX_TASKS")
        .unwrap_or_else(|_| "scripts/benchmarks/repo_fix/tasks.json".to_string());
    let manifest = std::path::Path::new(&manifest_path);
    let json = std::fs::read_to_string(manifest)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", manifest.display()))?;
    let file: RepoFixFile = serde_json::from_str(&json)?;

    let run_verify = |verify_cmd: &str, wd: &std::path::Path| -> i32 {
        let mut parts = verify_cmd.split_whitespace();
        let prog = parts.next().unwrap_or("true");
        let args: Vec<&str> = parts.collect();
        Command::new(prog)
            .args(&args)
            .current_dir(wd)
            .output()
            .map(|o| o.status.code().unwrap_or(-1))
            .unwrap_or(-1)
    };

    let mut passed = 0usize;
    let mut ran = 0usize;
    // Failure-aware evolution: (task_id, description, made_edit) for each failed task.
    let mut failures: Vec<(String, String, bool)> = Vec::new();
    let mut task_results = Vec::new();
    for task in &file.tasks {
        let workdir = std::env::temp_dir().join(format!(
            "px-repofix-{}-{}-{}",
            task.id,
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let _ = std::fs::remove_dir_all(&workdir);
        copy_dir_recursive(std::path::Path::new(&task.setup), &workdir)?;

        let pre = run_verify(&task.verify_cmd, &workdir);
        if pre == task.expect_exit {
            let _ = std::fs::remove_dir_all(&workdir);
            continue;
        }
        ran += 1;

        let desc = format!(
            "{}\n\nThe files are in {}. Read the buggy file, make a minimal edit to fix it, \
             then finish.",
            task.description,
            workdir.display()
        );
        let mut node = TaskNode::new(desc, TaskType::UserRequest, 100);
        let mut react = ReactLoop::new(
            Arc::clone(ollama),
            Arc::clone(registry),
            Arc::clone(policy),
            Arc::clone(memory),
            cancel.clone(),
        )
        .with_events(Arc::clone(events))
        .with_workspace_root(workdir.clone())
        .with_verifier(workdir.clone(), task.verify_cmd.clone(), task.expect_exit);
        if let Some(p) = prompt_override {
            react = react.with_prompt_override(p.to_string());
        }
        if let Some(t) = &transcripts {
            react = react.with_transcripts(Arc::clone(t));
        }
        let _ = react.run(&mut node).await;

        let post = run_verify(&task.verify_cmd, &workdir);
        let ok = post == task.expect_exit;
        let diff_summary =
            repo_fix_source_diff_summary(std::path::Path::new(&task.setup), &workdir);
        let made_edit = !diff_summary.trim().is_empty();
        let transcript_path = transcripts
            .as_ref()
            .and_then(|store| {
                store
                    .get_by_task_prefix(&node.id.to_string())
                    .ok()
                    .flatten()
            })
            .map(|summary| summary.transcript_path);

        task_results.push(RepoFixTaskResult {
            id: task.id.clone(),
            description: task.description.clone(),
            setup: task.setup.clone(),
            verify_cmd: task.verify_cmd.clone(),
            pre_exit: pre,
            post_exit: post,
            expect_exit: task.expect_exit,
            passed: ok,
            made_edit,
            workdir: if ok {
                None
            } else {
                Some(workdir.display().to_string())
            },
            transcript_path,
            diff_summary: diff_summary.clone(),
        });

        if ok {
            passed += 1;
            // Lever 1 (distillation): collect the TEST-VERIFIED solving trajectory into the SFT
            // corpus. A green test is the gold-standard, ungameable verification — far better SFT
            // data than HIRO "agent-finished" trajectories. Only on the dedicated bench run
            // (verbose) to avoid polluting the corpus during evolution's internal measurements.
            if verbose {
                ReactLoop::collect_trajectory(&node);
            }
        } else {
            // Record HOW it failed so the proposer targets the real pattern (item 3).
            // made_edit=false => "finished without editing" (the dominant measured failure).
            failures.push((task.id.clone(), task.description.clone(), made_edit));
        }
        if verbose {
            println!(
                "repo-fix {:8} pre={} post={} -> {}",
                task.id,
                pre,
                post,
                if ok { "PASS" } else { "fail" }
            );
        }
        if ok {
            let _ = std::fs::remove_dir_all(&workdir);
        }
    }
    Ok(RepoFixMeasureResult {
        passed,
        ran,
        failures,
        tasks: task_results,
    })
}

fn repo_fix_source_diff_summary(setup: &std::path::Path, workdir: &std::path::Path) -> String {
    std::process::Command::new("diff")
        .args([
            "-ru",
            "--exclude=__pycache__",
            "--exclude=*.pyc",
            "--exclude=.git",
            "--exclude=artifacts",
            "--exclude=target",
        ])
        .arg(setup)
        .arg(workdir)
        .output()
        .map(|o| {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines().take(80).collect::<Vec<_>>().join("\n")
        })
        .unwrap_or_else(|e| format!("diff unavailable: {e}"))
}

async fn run_repo_fix_bench(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    cancel: CancellationToken,
) -> Result<()> {
    let result = repo_fix_measure(
        &ollama,
        &registry,
        &policy,
        &memory,
        &events,
        Some(transcripts),
        &cancel,
        None,
        true,
    )
    .await?;
    let passed = result.passed;
    let ran = result.ran;
    let p1 = if ran > 0 {
        passed as f64 / ran as f64
    } else {
        0.0
    };
    println!("\n=== REPO-FIX BENCH ===\npass@1 = {p1:.3}  ({passed}/{ran} tasks)");
    let artifact = write_repo_fix_bench_artifact(&result, ollama.model())?;
    println!("artifact = {}", artifact.display());
    info!("repo-fix bench complete: pass@1={p1:.3} ({passed}/{ran})");
    Ok(())
}

fn write_repo_fix_bench_artifact(
    result: &RepoFixMeasureResult,
    model: &str,
) -> Result<std::path::PathBuf> {
    let repo_root = default_repo_root();
    let manifest_path = std::env::var("REPO_FIX_TASKS")
        .unwrap_or_else(|_| "scripts/benchmarks/repo_fix/tasks.json".to_string());
    let run_id = uuid::Uuid::new_v4().to_string();
    let pass_at_1 = if result.ran > 0 {
        result.passed as f64 / result.ran as f64
    } else {
        0.0
    };
    let artifact = RepoFixBenchArtifact {
        run_id: run_id.clone(),
        recorded_at: chrono::Utc::now().to_rfc3339(),
        harness_commit: git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string()),
        manifest_path,
        model: model.to_string(),
        passed: result.passed,
        ran: result.ran,
        pass_at_1,
        tasks: result.tasks.clone(),
    };
    let dir = repo_root
        .join("professor-x")
        .join("artifacts")
        .join("repo-fix")
        .join(chrono::Utc::now().format("%Y-%m-%d").to_string());
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "repo-fix-{}-{}.json",
        chrono::Utc::now().format("%H%M%S"),
        short_fragment(&run_id)
    ));
    std::fs::write(&path, serde_json::to_string_pretty(&artifact)?)?;
    Ok(path)
}

/// M4: self-improvement loop with an EMPIRICAL fitness gate. Unlike the legacy evolution
/// loop (which accepted prompt changes on a compile + LLM-approval, never measuring whether
/// they helped), this measures repo-fix pass@1 before AND after each candidate and accepts
/// ONLY a change that beats the current best beyond the noise floor.
#[allow(clippy::too_many_arguments)]
async fn run_evolve_on_repofix(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
    rounds: u32,
    proposer: Arc<ollama::OllamaClient>,
) -> Result<()> {
    // Repeats per measurement to average out the ±0.1 single-run variance.
    const K: usize = 2;
    // Minimum detectable effect: only accept improvements clearly above noise.
    const MDE: f64 = 0.10;

    let measure = |prompt: Option<String>| {
        let (o, r, p, m, e, c) = (
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            cancel.clone(),
        );
        async move {
            let mut tot_p = 0usize;
            let mut tot_r = 0usize;
            let mut fails: std::collections::HashMap<String, (String, bool)> =
                std::collections::HashMap::new();
            for _ in 0..K {
                let result =
                    repo_fix_measure(&o, &r, &p, &m, &e, None, &c, prompt.as_deref(), false)
                        .await?;
                tot_p += result.passed;
                tot_r += result.ran;
                for (id, desc, made_edit) in result.failures {
                    fails.insert(id, (desc, made_edit));
                }
            }
            let score = if tot_r > 0 {
                tot_p as f64 / tot_r as f64
            } else {
                0.0
            };
            anyhow::Ok((score, fails))
        }
    };

    println!("=== M4: evolve-on-repofix (empirical fitness gate, K={K} reps, MDE={MDE}) ===");
    let (baseline, baseline_fails) = measure(None).await?;
    println!("round 0 (baseline default prompt): pass@1 = {baseline:.3}");
    info!("M4: baseline pass@1={baseline:.3}");

    // Item 3 — automate the diagnose-loop: turn the actual baseline failures into a report
    // the proposer must address, instead of blindly "improve the prompt".
    let failure_report = if baseline_fails.is_empty() {
        "No failures at baseline — propose a small robustness refinement.".to_string()
    } else {
        let mut lines: Vec<String> = baseline_fails
            .iter()
            .map(|(id, (desc, made_edit))| {
                let mode = if *made_edit {
                    "made a WRONG edit (file changed but test still red)"
                } else {
                    "FINISHED WITHOUT MAKING ANY EDIT"
                };
                format!(
                    "- {id}: {} -> agent {mode}",
                    desc.chars().take(80).collect::<String>()
                )
            })
            .collect();
        lines.sort();
        lines.join("\n")
    };
    println!("baseline failure report:\n{failure_report}\n");

    let mut best = baseline;
    let mut best_prompt: Option<String> = None;
    let mut accepted = 0u32;

    for round in 1..=rounds {
        if cancel.is_cancelled() {
            break;
        }
        let base_prompt = best_prompt
            .clone()
            .unwrap_or_else(|| agentd::react::default_system_prompt().to_string());
        let candidate = match propose_repofix_prompt(&proposer, &base_prompt, &failure_report).await
        {
            Ok(c) => c,
            Err(e) => {
                warn!("M4 round {round}: proposal failed: {e}");
                continue;
            }
        };
        let (score, _) = measure(Some(candidate.clone())).await?;
        let accept = score >= best + MDE;
        println!(
            "round {round}: candidate pass@1 = {score:.3} (best {best:.3}, +MDE {:.3}) -> {}",
            best + MDE,
            if accept { "ACCEPT" } else { "reject" }
        );
        info!("M4 round {round}: candidate={score:.3} best={best:.3} accept={accept}");
        if accept {
            best = score;
            best_prompt = Some(candidate);
            accepted += 1;
        }
    }

    println!(
        "\n=== M4 CURVE === baseline {baseline:.3} -> best {best:.3} | {accepted}/{rounds} accepted"
    );
    println!(
        "(An empirically-gated loop that accepts NOTHING is correct behavior when no candidate \
         truly beats noise — unlike the legacy loop, it never accepts an unmeasured change.)"
    );
    info!("M4 done: baseline={baseline:.3} best={best:.3} accepted={accepted}/{rounds}");
    Ok(())
}

/// M4 item 3: failure-AWARE proposer. Shown the agent's ACTUAL failures (not asked to guess),
/// it proposes a system prompt that targets those specific failure modes — automating the
/// diagnose-from-trajectory loop that worked far better by hand than blind prompt mutation.
async fn propose_repofix_prompt(
    ollama: &Arc<ollama::OllamaClient>,
    base_prompt: &str,
    failure_report: &str,
) -> Result<String> {
    let system = "You improve the SYSTEM PROMPT of a coding agent that fixes bugs in small \
        repos on a weak local model. You are shown the agent's ACTUAL recent failures. Output \
        ONLY the new system prompt text — no preamble, no markdown fences. Keep it concise \
        (weak models follow short prompts better). Directly target the observed failure modes — \
        e.g. if the agent finishes WITHOUT editing, the prompt must insist it applies an edit \
        before finishing; if it loops on a repeated action, forbid repeating an action.";
    let prompt = format!(
        "CURRENT SYSTEM PROMPT:\n{base_prompt}\n\nTHE AGENT'S ACTUAL RECENT FAILURES:\n\
         {failure_report}\n\nWrite an improved system prompt that fixes THESE specific failures. \
         Output only the prompt:"
    );
    let resp = ollama
        .generate(
            &prompt,
            Some(system),
            Some(ollama::ModelOptions::for_reflection()),
        )
        .await?;
    let (_, text) = resp.split_thinking();
    let text = text.trim().to_string();
    if text.len() < 40 {
        anyhow::bail!("proposed prompt too short ({} chars)", text.len());
    }
    Ok(text)
}

/// M4 item 1: point the empirical fitness gate at a SKILL (durable knowledge), not the
/// ephemeral system prompt. Evolves `skills/conductor/px-fix-bug.md`: inject it over the
/// default prompt, propose a failure-targeted improvement, measure pass@1, and PERSIST the
/// skill file only if it measurably helps. Self-improvement of knowledge, empirically gated.
#[allow(clippy::too_many_arguments)]
async fn run_evolve_skill_on_repofix(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
    rounds: u32,
    proposer: Arc<ollama::OllamaClient>,
) -> Result<()> {
    const K: usize = 2;
    const MDE: f64 = 0.10;
    let skill_path = std::path::PathBuf::from("skills/conductor/px-fix-bug.md");
    let original_skill = std::fs::read_to_string(&skill_path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", skill_path.display()))?;

    let with_skill = |skill: &str| {
        format!(
            "{}\n\n## Active skill — apply it\n{}",
            agentd::react::default_system_prompt(),
            skill
        )
    };

    let measure = |prompt: Option<String>| {
        let (o, r, p, m, e, c) = (
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            cancel.clone(),
        );
        async move {
            let mut tp = 0usize;
            let mut tr = 0usize;
            let mut fails: std::collections::HashMap<String, (String, bool)> =
                std::collections::HashMap::new();
            for _ in 0..K {
                let result =
                    repo_fix_measure(&o, &r, &p, &m, &e, None, &c, prompt.as_deref(), false)
                        .await?;
                tp += result.passed;
                tr += result.ran;
                for (id, d, me) in result.failures {
                    fails.insert(id, (d, me));
                }
            }
            anyhow::Ok((if tr > 0 { tp as f64 / tr as f64 } else { 0.0 }, fails))
        }
    };

    println!("=== M4: evolve-SKILL-on-repofix (px-fix-bug, empirical gate, K={K}, MDE={MDE}) ===");
    let (baseline, baseline_fails) = measure(Some(with_skill(&original_skill))).await?;
    println!("round 0 (current px-fix-bug skill): pass@1 = {baseline:.3}");
    let failure_report = if baseline_fails.is_empty() {
        "No failures at baseline.".to_string()
    } else {
        let mut l: Vec<String> = baseline_fails
            .iter()
            .map(|(id, (d, me))| {
                let mode = if *me {
                    "made a WRONG edit"
                } else {
                    "FINISHED WITHOUT MAKING ANY EDIT"
                };
                format!(
                    "- {id}: {} -> agent {mode}",
                    d.chars().take(80).collect::<String>()
                )
            })
            .collect();
        l.sort();
        l.join("\n")
    };
    println!("baseline failure report:\n{failure_report}\n");

    let mut best = baseline;
    let mut best_skill = original_skill.clone();
    let mut accepted = 0u32;
    for round in 1..=rounds {
        if cancel.is_cancelled() {
            break;
        }
        let candidate = match propose_repofix_skill(&proposer, &best_skill, &failure_report).await {
            Ok(c) => c,
            Err(e) => {
                warn!("M4 skill round {round}: proposal failed: {e}");
                continue;
            }
        };
        let (score, _) = measure(Some(with_skill(&candidate))).await?;
        let accept = score >= best + MDE;
        println!(
            "round {round}: candidate skill pass@1 = {score:.3} (best {best:.3}) -> {}",
            if accept { "ACCEPT" } else { "reject" }
        );
        if accept {
            best = score;
            best_skill = candidate;
            accepted += 1;
        }
    }

    if best_skill != original_skill {
        std::fs::write(&skill_path, &best_skill)?;
        println!(
            "PERSISTED improved px-fix-bug skill to {}",
            skill_path.display()
        );
    } else {
        println!("No skill change beat baseline — px-fix-bug left unchanged (gate working).");
    }
    println!(
        "\n=== M4 SKILL CURVE === baseline {baseline:.3} -> best {best:.3} | {accepted}/{rounds} accepted"
    );
    info!("M4 skill done: baseline={baseline:.3} best={best:.3} accepted={accepted}/{rounds}");
    Ok(())
}

/// M4 code-proposer frontier: AUTONOMOUS code self-improvement (no human in the loop, per Abrar).
/// A code-specialized local model authors a diff scoped to ONE component file; the
/// default-deny safety guard + a worktree gate (build + full test + measured repo-fix delta)
/// decide acceptance; an accepted change auto-commits. See docs/research/m4-code-proposer-scoping.md.
#[allow(clippy::too_many_arguments)]
async fn run_evolve_code_on_repofix(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
    rounds: u32,
    proposer: Arc<ollama::OllamaClient>,
    target: String,
) -> Result<()> {
    use evolved::code_safety::check_diff_safety;
    const K: usize = 1; // gate reps (>=2 for rigor; 1 keeps the first run feasible ~per-candidate)

    let repo_root = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| std::path::PathBuf::from(s.trim()))
        .ok_or_else(|| anyhow::anyhow!("not in a git repo"))?;
    let crate_dir = repo_root.join("professor-x");
    let target_abs = crate_dir.join(&target);
    let original = std::fs::read_to_string(&target_abs)
        .map_err(|e| anyhow::anyhow!("cannot read target {}: {e}", target_abs.display()))?;
    let allowed: Vec<String> = vec![format!("professor-x/{target}")];

    println!(
        "=== M4: evolve-CODE-on-repofix (AUTONOMOUS, target={target}, proposer={}) ===",
        proposer.model()
    );

    // Baseline with the CURRENT (already-built) binary.
    let baseline_result = repo_fix_measure(
        &ollama, &registry, &policy, &memory, &events, None, &cancel, None, false,
    )
    .await?;
    let baseline = if baseline_result.ran > 0 {
        baseline_result.passed as f64 / baseline_result.ran as f64
    } else {
        0.0
    };
    println!("round 0 baseline pass@1 = {baseline:.3}");
    let failure_report = if baseline_result.failures.is_empty() {
        "No failures.".to_string()
    } else {
        // ACTIONABLE diagnosis (not a raw failure list — the coder needs a localizable target,
        // see docs/research/m4-code-proposer-scoping.md): classify the dominant failure mode and
        // tell the coder what KIND of code fix it implies (or that it's model-level, not harness).
        let no_edit = baseline_result
            .failures
            .iter()
            .filter(|(_, _, me)| !me)
            .count();
        let wrong_edit = baseline_result.failures.len() - no_edit;
        let diagnosis = if no_edit >= wrong_edit {
            "DOMINANT FAILURE: the agent finishes WITHOUT making an edit (gathers, then gives up). \
             If this file contains the finish/synthesis/loop logic, a fix would force an edit \
             before finishing or break a stuck loop. If not, output NO-DIFF."
        } else {
            "DOMINANT FAILURE: the agent makes a WRONG edit (the test stays red). This is usually \
             a MODEL-reasoning limit, not a harness bug — there is likely no code fix here. Only \
             propose a diff if you see a concrete harness defect; otherwise output NO-DIFF."
        };
        let list = baseline_result
            .failures
            .iter()
            .map(|(id, d, me)| {
                format!(
                    "- {id}: {} -> {}",
                    d.chars().take(70).collect::<String>(),
                    if *me { "WRONG edit" } else { "NO edit" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{diagnosis}\n\nFailing tasks:\n{list}")
    };

    let mut best = baseline;
    let mut accepted = 0u32;
    for round in 1..=rounds {
        if cancel.is_cancelled() {
            break;
        }
        println!("\n--- round {round}: proposing a code diff for {target} ---");
        let diff = match propose_code_diff(&proposer, &target, &original, &failure_report).await {
            Ok(d) => d,
            Err(e) => {
                warn!("propose failed: {e}");
                continue;
            }
        };
        // STRUCTURAL SAFETY (no human approval — this is the gate)
        if let Err(reason) = check_diff_safety(
            &diff,
            &allowed.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        ) {
            println!("  SAFETY REJECT: {reason}");
            continue;
        }
        println!("  safety OK; verifying in sandbox worktree (build + full test + bench)…");
        match gate_code_candidate(
            &repo_root, &diff, K, &ollama, &registry, &policy, &memory, &events, &cancel,
        )
        .await
        {
            Ok(Some(score)) => {
                let accept = score >= best + 0.10;
                println!(
                    "  candidate pass@1 = {score:.3} (best {best:.3}) -> {}",
                    if accept { "ACCEPT" } else { "reject" }
                );
                if accept {
                    // AUTONOMOUS apply to the working tree + commit.
                    let tmp = std::env::temp_dir()
                        .join(format!("px-codeevolve-{}.diff", uuid::Uuid::new_v4()));
                    std::fs::write(&tmp, &diff)?;
                    let ap = std::process::Command::new("git")
                        .args(["apply", "--recount", "-C1"])
                        .arg(&tmp)
                        .current_dir(&repo_root)
                        .status();
                    let _ = std::fs::remove_file(&tmp);
                    if ap.map(|s| s.success()).unwrap_or(false) {
                        let _ = std::process::Command::new("git")
                            .args(["commit", "-am", &format!("M4 autonomous: code-proposer lifted repo-fix {best:.3}->{score:.3} ({target})")])
                            .current_dir(&repo_root).status();
                        println!("  APPLIED + committed autonomously.");
                        best = score;
                        accepted += 1;
                    } else {
                        println!("  (apply to working tree failed — left unchanged)");
                    }
                }
            }
            Ok(None) => println!("  candidate REJECTED by gate (build/test failed)"),
            Err(e) => warn!("gate error: {e}"),
        }
    }
    println!("\n=== M4 CODE CURVE === baseline {baseline:.3} -> best {best:.3} | {accepted}/{rounds} accepted");
    Ok(())
}

/// Apply a diff in an isolated git worktree, build + run the FULL test suite, then measure
/// repo-fix pass@1 with the candidate binary. Returns Some(pass@1) if it builds+tests; None if not.
#[allow(clippy::too_many_arguments)]
async fn gate_code_candidate(
    repo_root: &std::path::Path,
    diff: &str,
    k: usize,
    ollama: &Arc<ollama::OllamaClient>,
    registry: &Arc<std::sync::RwLock<ToolRegistry>>,
    policy: &Arc<PolicyEngine>,
    memory: &Arc<MemoryManager>,
    events: &Arc<EventStore>,
    cancel: &CancellationToken,
) -> Result<Option<f64>> {
    use std::process::Command;
    let wt = std::env::temp_dir().join(format!("px-codeevolve-wt-{}", uuid::Uuid::new_v4()));
    let run = || -> Result<Option<f64>> {
        // worktree at HEAD
        if !Command::new("git")
            .args(["worktree", "add", "--detach"])
            .arg(&wt)
            .arg("HEAD")
            .current_dir(repo_root)
            .status()?
            .success()
        {
            anyhow::bail!("worktree add failed");
        }
        // apply diff
        let tmp = wt
            .join("..")
            .join(format!("px-cand-{}.diff", uuid::Uuid::new_v4()));
        std::fs::write(&tmp, diff)?;
        let applied = Command::new("git")
            .args(["apply", "--recount", "-C1"])
            .arg(&tmp)
            .current_dir(&wt)
            .status()?
            .success();
        let _ = std::fs::remove_file(&tmp);
        if !applied {
            println!("    (diff did not apply cleanly)");
            return Ok(None);
        }
        let cdir = wt.join("professor-x");
        // build
        if !Command::new("cargo")
            .args(["build", "--bins", "--quiet"])
            .current_dir(&cdir)
            .status()?
            .success()
        {
            println!("    (cargo build failed)");
            return Ok(None);
        }
        // FULL test suite must pass
        if !Command::new("cargo")
            .args(["test", "--bins", "--quiet"])
            .current_dir(&cdir)
            .status()?
            .success()
        {
            println!("    (cargo test failed)");
            return Ok(None);
        }
        Ok(Some(0.0)) // placeholder; real measurement below (needs async)
    };
    // run the blocking build/test, then measure async with the candidate binary
    let built = run();
    let candidate_bin = wt.join("professor-x/target/debug/professor-x");
    let score = match built {
        Ok(Some(_)) if candidate_bin.exists() => {
            // measure repo-fix with the candidate binary (subprocess), k reps
            let mut tot = 0.0;
            let mut n = 0;
            for _ in 0..k {
                if cancel.is_cancelled() {
                    break;
                }
                let out = tokio::process::Command::new(&candidate_bin)
                    .args(["--repo-fix-bench", "--model", "qwen3:8b-q4_K_M"])
                    .current_dir(wt.join("professor-x"))
                    .env(
                        "PROFESSOR_X_DATA_DIR",
                        std::env::var("PROFESSOR_X_DATA_DIR").unwrap_or_default(),
                    )
                    .output()
                    .await;
                if let Ok(o) = out {
                    let s = String::from_utf8_lossy(&o.stdout);
                    if let Some(p) = s.lines().rev().find_map(|l| {
                        l.strip_prefix("pass@1 = ")
                            .and_then(|x| x.split_whitespace().next())
                            .and_then(|x| x.parse::<f64>().ok())
                    }) {
                        tot += p;
                        n += 1;
                    }
                }
            }
            if n > 0 {
                Ok(Some(tot / n as f64))
            } else {
                Ok(None)
            }
        }
        Ok(_) => Ok(None),
        Err(e) => Err(e),
    };
    let _ = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(&wt)
        .current_dir(repo_root)
        .status();
    let _ = (ollama, registry, policy, memory, events); // measurement uses a subprocess binary, not these
    score
}

/// Ask the code-specialized model for a unified diff that fixes the observed failures, scoped to
/// ONE file. Strict diff format so it applies; the safety guard rejects anything out of scope.
async fn propose_code_diff(
    coder: &Arc<ollama::OllamaClient>,
    target: &str,
    file_content: &str,
    failure_report: &str,
) -> Result<String> {
    let system = "You are a Rust expert improving a coding agent's harness. You are given ONE \
        source file and the agent's recent benchmark failures. Output ONLY a valid unified diff \
        (git apply format) that edits THAT FILE to fix the failures — no prose, no fences. The \
        diff must use 'a/professor-x/<path>' and 'b/professor-x/<path>' headers, correct @@ hunk \
        lines, and minimal context. Do NOT touch tests, the benchmark, or the evaluator. If no \
        safe improvement is clear, output exactly: NO-DIFF.";
    let prompt = format!(
        "FILE: professor-x/{target}\n```rust\n{}\n```\n\nRECENT BENCHMARK FAILURES:\n{failure_report}\n\n\
         Output a unified diff for professor-x/{target} only:",
        file_content.chars().take(45000).collect::<String>()
    );
    // qwen2.5-coder does NOT support thinking mode (400 if think=true), and needs a big ctx for
    // a full source file. Explicit options, no thinking.
    let opts = ollama::ModelOptions {
        temperature: Some(0.2),
        num_ctx: Some(32768),
        top_p: Some(0.9),
        stop: None,
        think: Some(false),
    };
    let resp = coder.generate(&prompt, Some(system), Some(opts)).await?;
    let (_, text) = resp.split_thinking();
    let text = text.trim();
    if text.contains("NO-DIFF") || text.len() < 30 {
        anyhow::bail!("proposer returned no diff");
    }
    // strip accidental ``` fences
    let cleaned = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(cleaned)
}

/// Failure-aware SKILL proposer: improve the px-fix-bug skill markdown to fix observed failures.
async fn propose_repofix_skill(
    ollama: &Arc<ollama::OllamaClient>,
    base_skill: &str,
    failure_report: &str,
) -> Result<String> {
    let system = "You improve a SKILL (a markdown guide) used by a weak local coding agent that \
        fixes bugs. You are shown its ACTUAL recent failures. Output ONLY the improved skill \
        markdown — keep the '# px-fix-bug' heading and concise Purpose/Workflow/Anti-patterns/\
        Output Contract sections. Directly target the observed failures.";
    let prompt = format!(
        "CURRENT SKILL:\n{base_skill}\n\nACTUAL RECENT FAILURES:\n{failure_report}\n\n\
         Output the improved skill markdown only:"
    );
    let resp = ollama
        .generate(
            &prompt,
            Some(system),
            Some(ollama::ModelOptions::for_reflection()),
        )
        .await?;
    let (_, text) = resp.split_thinking();
    let text = text.trim().to_string();
    if text.len() < 60 {
        anyhow::bail!("proposed skill too short ({} chars)", text.len());
    }
    Ok(text)
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            std::fs::copy(&path, &target)?;
        }
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
    let description = util::expand_file_refs(&description); // @file → inline context
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
    if let Some(class) = outcome.failure_class {
        println!("failure class: {}", class.as_str());
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

    // Mutable so `/model` can switch the local model live, mid-session.
    let mut ollama = ollama;

    print_welcome(ollama.model_name());

    loop {
        if cancel.is_cancelled() {
            break;
        }
        print!("\x1b[35m❯\x1b[0m ");
        io::stdout().flush()?;

        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        // ── assistant UX: live model switching ────────────────────────────
        if let Some(rest) = input.strip_prefix("/model") {
            let arg = rest.trim();
            if arg.is_empty() {
                println!("current model: {}", ollama.model_name());
                match ollama.installed_models().await {
                    Ok(ms) if !ms.is_empty() => {
                        println!("installed (largest first):");
                        let mut ms = ms;
                        ms.sort_by(|a, b| {
                            b.params_b
                                .partial_cmp(&a.params_b)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        for m in ms {
                            let mark = if m.name == ollama.model_name() {
                                " ←"
                            } else {
                                ""
                            };
                            println!("  {:<28} {:>6.1}B{}", m.name, m.params_b, mark);
                        }
                        println!("switch: /model <name>");
                    }
                    _ => println!("(could not list installed models)"),
                }
            } else {
                ollama =
                    Arc::new(ollama::OllamaClient::new("http://localhost:11434").with_model(arg));
                println!("✓ switched to model: {arg}");
                record_console_command(&events, "model", Some(arg.to_string()))?;
            }
            continue;
        }
        if input == "/tools" {
            let reg = registry.read().unwrap();
            let mut tools = reg.list();
            tools.sort_by(|a, b| a.name.cmp(&b.name));
            println!("available tools ({}):", tools.len());
            for t in tools {
                println!(
                    "  {:<20} {}",
                    t.name,
                    t.description.chars().take(60).collect::<String>()
                );
            }
            record_console_command(&events, "tools", None)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/undo") {
            let checkpoint = rest.trim();
            let checkpoint = (!checkpoint.is_empty()).then_some(checkpoint);
            record_console_command(&events, "undo", checkpoint.map(ToString::to_string))?;
            let workspace_root = PermissionScope::default_autonomous().workspace_root;
            match toolbridge::checkpoint::undo_checkpoint(&workspace_root, checkpoint) {
                Ok(summary) => {
                    events.append(
                        None,
                        None,
                        "checkpoint.undone",
                        &summary,
                        serde_json::json!({"checkpoint": checkpoint}),
                    )?;
                    println!("✓ {summary}");
                }
                Err(e) => println!("undo failed: {e}"),
            }
            continue;
        }
        if matches!(input, "/quit" | "/exit" | "quit" | "exit") {
            break;
        }
        if matches!(input, "/help" | "help") {
            record_console_command(&events, "help", None)?;
            println!("{}", format_interactive_help());
            continue;
        }
        if input == "/status" {
            record_console_command(&events, "status", None)?;
            observer::print_snapshot(Arc::clone(&memory), Arc::clone(&events))?;
            continue;
        }
        if input == "/cockpit" || input == "/now" {
            record_console_command(&events, "cockpit", None)?;
            println!(
                "{}",
                render_work_cockpit(Arc::clone(&memory), Arc::clone(&events), 12)?
            );
            continue;
        }
        if input == "/brief" {
            record_console_command(&events, "brief", None)?;
            print_prof_x_brief(Arc::clone(&memory), Arc::clone(&events))?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/events") {
            let limit = rest.trim().parse::<usize>().unwrap_or(10);
            record_console_command(&events, "events", Some(limit.to_string()))?;
            print_events(Arc::clone(&events), limit)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/work") {
            let limit = rest.trim().parse::<usize>().unwrap_or(8);
            record_console_command(&events, "work", Some(limit.to_string()))?;
            print_work_feed(Arc::clone(&events), limit)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/sessions") {
            let limit = rest.trim().parse::<usize>().unwrap_or(5);
            record_console_command(&events, "sessions", Some(limit.to_string()))?;
            print_coding_sessions(Arc::clone(&memory), limit)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/session-review") {
            let session_ref = nonempty_or_latest(rest);
            record_console_command(&events, "session-review", Some(session_ref.to_string()))?;
            print_coding_session_review(Arc::clone(&memory), session_ref)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/session-publish") {
            let session_ref = nonempty_or_latest(rest);
            record_console_command(&events, "session-publish", Some(session_ref.to_string()))?;
            publish_coding_session_artifacts(Arc::clone(&memory), session_ref)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/queue-review") {
            let queue_ref = nonempty_or_latest(rest);
            record_console_command(&events, "queue-review", Some(queue_ref.to_string()))?;
            print_autonomy_queue_review(Arc::clone(&memory), queue_ref)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/queue-replay") {
            let queue_ref = nonempty_or_latest(rest);
            record_console_command(&events, "queue-replay", Some(queue_ref.to_string()))?;
            print_autonomy_queue_replay(Arc::clone(&memory), queue_ref)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/queue-publish") {
            let queue_ref = nonempty_or_latest(rest);
            record_console_command(&events, "queue-publish", Some(queue_ref.to_string()))?;
            publish_autonomy_queue_run(Arc::clone(&memory), queue_ref)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/queue") {
            let limit = rest.trim().parse::<usize>().unwrap_or(10);
            record_console_command(&events, "queue", Some(limit.to_string()))?;
            print_autonomy_queue(Arc::clone(&memory), limit)?;
            continue;
        }
        if input == "/plan" {
            record_console_command(&events, "plan", None)?;
            plan_autonomy_queue_once(Arc::clone(&memory), Arc::clone(&events))?;
            continue;
        }
        if input == "/preview" || input == "/next" {
            record_console_command(&events, "preview", None)?;
            preview_autonomy_step(Arc::clone(&memory), Arc::clone(&events))?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/enqueue-commit") {
            let goal = rest.trim();
            record_console_command(&events, "enqueue-commit", Some(goal.to_string()))?;
            enqueue_operator_autonomy_goal(
                Arc::clone(&memory),
                Arc::clone(&events),
                goal,
                WorkLoopProfile::Commit,
            )?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/enqueue") {
            let goal = rest.trim();
            record_console_command(&events, "enqueue", Some(goal.to_string()))?;
            enqueue_operator_autonomy_goal(
                Arc::clone(&memory),
                Arc::clone(&events),
                goal,
                WorkLoopProfile::Core,
            )?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/runs") {
            let limit = rest.trim().parse::<usize>().unwrap_or(5);
            record_console_command(&events, "runs", Some(limit.to_string()))?;
            print_run_log(Arc::clone(&memory), limit)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/review") {
            let run_ref = nonempty_or_latest(rest);
            record_console_command(&events, "review", Some(run_ref.to_string()))?;
            print_run_review(Arc::clone(&memory), run_ref)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/replay") {
            let run_ref = nonempty_or_latest(rest);
            record_console_command(&events, "replay", Some(run_ref.to_string()))?;
            print_run_replay(Arc::clone(&memory), run_ref)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/publish") {
            let run_ref = nonempty_or_latest(rest);
            record_console_command(&events, "publish", Some(run_ref.to_string()))?;
            publish_run_artifacts(Arc::clone(&memory), run_ref)?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/task-review") {
            let task_ref = nonempty_or_latest(rest);
            record_console_command(&events, "task-review", Some(task_ref.to_string()))?;
            print_task_review(Arc::clone(&transcripts), task_ref)?;
            continue;
        }
        if let Some(rest) = input
            .strip_prefix("/task-evidence")
            .or_else(|| input.strip_prefix("/inspect"))
        {
            let task_ref = nonempty_or_latest(rest);
            record_console_command(&events, "task-evidence", Some(task_ref.to_string()))?;
            print_task_evidence(
                Arc::clone(&memory),
                Arc::clone(&events),
                Arc::clone(&transcripts),
                task_ref,
            )?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/step-live") {
            let count = rest.trim().parse::<u32>().unwrap_or(1);
            record_console_command(&events, "step-live", Some(count.to_string()))?;
            run_autonomy_queue_steps_live(
                Arc::clone(&registry),
                Arc::clone(&policy),
                Arc::clone(&memory),
                Arc::clone(&events),
                Arc::clone(&transcripts),
                count,
                false,
            )
            .await?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/step") {
            let count = rest.trim().parse::<u32>().unwrap_or(1);
            record_console_command(&events, "step", Some(count.to_string()))?;
            run_autonomy_queue_steps(
                Arc::clone(&registry),
                Arc::clone(&policy),
                Arc::clone(&memory),
                Arc::clone(&events),
                Arc::clone(&transcripts),
                count,
                false,
            )
            .await?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/run-commit") {
            let cycles = rest.trim().parse::<u32>().unwrap_or(6);
            record_console_command(&events, "run-commit", Some(cycles.to_string()))?;
            run_autonomous_operator_run(
                Arc::clone(&registry),
                Arc::clone(&policy),
                Arc::clone(&memory),
                Arc::clone(&events),
                Arc::clone(&transcripts),
                cycles,
                WorkLoopProfile::Commit,
                false,
            )
            .await?;
            continue;
        }
        if let Some(rest) = input.strip_prefix("/run") {
            let cycles = rest.trim().parse::<u32>().unwrap_or(4);
            record_console_command(&events, "run", Some(cycles.to_string()))?;
            run_autonomous_operator_run(
                Arc::clone(&registry),
                Arc::clone(&policy),
                Arc::clone(&memory),
                Arc::clone(&events),
                Arc::clone(&transcripts),
                cycles,
                WorkLoopProfile::Core,
                false,
            )
            .await?;
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

/// Clean, friendly welcome — answers "what do I type?" with concrete examples,
/// instead of dumping the full operator command wall (that's behind /help).
fn print_welcome(model: &str) {
    const M: &str = "\x1b[35m"; // magenta
    const C: &str = "\x1b[36m"; // cyan
    const D: &str = "\x1b[90m"; // dim
    const B: &str = "\x1b[1m"; // bold
    const R: &str = "\x1b[0m"; // reset
    println!();
    println!("  {M}{B}● Professor X{R}{D} — local agentic assistant{R}");
    println!("  {D}model {model} · type /model to switch{R}");
    println!();
    println!("  {B}Just tell me what you want done.{R} For example:");
    println!("    {C}what does @src/main.rs do?{R}");
    println!("    {C}create a python script that renames every .txt in this folder to .md{R}");
    println!("    {C}find every TODO in the codebase and list them{R}");
    println!("    {C}run the tests and tell me what's failing{R}");
    println!();
    println!("  {D}@path pulls a file into context · /model /tools /help /quit{R}");
    println!();
}

fn format_interactive_help() -> String {
    [
        "Professor X interactive task mode",
        "Type a task and press Enter.  Reference files with @path (e.g. 'fix @src/main.rs').",
        "  /model [name]   show/switch the local model    /tools  list available tools",
        "  /undo [id|path]  restore the latest or selected Prof X checkpoint",
        "",
        "Operator commands",
        "  /brief         show latest run, coding session, evidence, and next commands",
        "  /cockpit        show live state, current run, latest coding session, and trace",
        "  /work [n]       show recent work/tool/task events",
        "  /sessions [n]   show recent coding-agent sessions and evidence paths",
        "  /session-review [session] review latest or selected coding session",
        "  /session-publish [session] publish coding session evidence to git",
        "  /queue [n]      show persistent autonomous work queue",
        "  /queue-review [queue] review linked run evidence for a queue item",
        "  /queue-replay [queue] replay linked run timeline for a queue item",
        "  /queue-publish [queue] publish linked queue run evidence to git",
        "  /plan           enqueue the next planner-selected autonomous work item",
        "  /preview        show the next queued/planned autonomous gates without running them",
        "  /enqueue <goal> enqueue a bounded core autonomous work goal",
        "  /enqueue-commit <goal> enqueue a commit-capable autonomous work goal",
        "  /runs [n]       show recent operator/autonomous run ledger entries",
        "  /review [run]   review latest or selected run evidence",
        "  /replay [run]   replay latest or selected run timeline",
        "  /publish [run]  commit selected run report/ledger artifacts",
        "  /task-review [task] review latest or selected task transcript",
        "  /task-evidence [task] show task run, transcript, artifact verdicts, and events",
        "  /inspect [task]       alias for /task-evidence",
        "  /step-live [n]  run queued autonomous work while streaming the work feed",
        "  /step [n]       run n queued autonomous work items, seeding one if empty",
        "  /run [n]        start a bounded core Prof X run",
        "  /run-commit [n] start a commit-capable verified Prof X run",
        "  /events [n]     show raw recent events",
        "  /status         show daemon/scheduler/event snapshot",
        "  /help           show this command list",
        "  /quit           stop the console",
    ]
    .join("\n")
}

fn record_console_command(
    events: &EventStore,
    command: &str,
    argument: Option<String>,
) -> Result<()> {
    let argument = argument
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    let mut payload = serde_json::json!({
        "command": command,
    });
    if let Some(argument) = argument.as_deref() {
        payload["argument"] = serde_json::json!(argument);
    }
    let summary = argument
        .as_deref()
        .map(|argument| format!("operator console command /{command} {argument}"))
        .unwrap_or_else(|| format!("operator console command /{command}"));
    events.append(None, None, "console.command", summary, payload)?;
    Ok(())
}

fn nonempty_or_latest(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "latest"
    } else {
        trimmed
    }
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
        "  cargo run -- --prof-x-live 6",
        "  cargo run -- --prof-x-enqueue \"tighten the next harness gap\"",
        "  cargo run -- --prof-x-enqueue-commit \"capture a verified skill improvement\"",
        "  cargo run -- --prof-x-plan",
        "  cargo run -- --prof-x-preview-step",
        "  cargo run -- --prof-x-step-live 1",
        "  cargo run -- --prof-x-step-publish-live 1",
        "  cargo run -- --prof-x-step 1",
        "  cargo run -- --prof-x-step-publish 1",
        "  cargo run -- --prof-x-queue 10",
        "  cargo run -- --prof-x-queue-review latest",
        "  cargo run -- --prof-x-queue-publish latest",
        "  cargo run -- --observe-work",
        "  cargo run -- --cockpit",
        "  cargo run -- --status-json",
        "  cargo run -- --watch-work",
        "  cargo run -- --prof-x-journal 50",
        "  cargo run -- --prof-x-journal-commit 50",
        "  cargo run -- --consciousness-report",
        "",
        "Give him a bounded coding-agent task",
        "  cargo run -- --prof-x-code-live \"update one safe local fixture\"",
        "  cargo run -- --coding-sessions 5",
        "  cargo run -- --repair-coding-sessions 10",
        "  cargo run -- --prof-x-code-review latest",
        "  cargo run -- --prof-x-code-publish latest",
        "",
        "Turn a short operator goal into a verified skill patch",
        "  cargo run -- --prof-x-skill-live \"capture the next harness gap\"",
        "  cargo run -- --prof-x-skill-commit-live \"capture the next harness gap\"",
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
        "  cargo run -- --task-evidence latest",
        "  cargo run -- --inspect latest",
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

fn stale_coding_session_map(
    events: &EventStore,
    sessions: &[CodingSessionRecord],
) -> Result<HashMap<String, CodingSessionStaleCandidate>> {
    let mut stale = HashMap::new();
    let now = chrono::Utc::now();
    for session in sessions {
        if let Some(candidate) = stale_candidate(events, session, now)? {
            stale.insert(session.id.clone(), candidate);
        }
    }
    Ok(stale)
}

fn coding_session_repair_command(limit: usize) -> String {
    format!("cargo run -- --repair-coding-sessions {limit}")
}

fn repair_stale_coding_sessions(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    limit: usize,
) -> Result<()> {
    let sessions = CodingSessionStore::new(Arc::clone(&memory.db)).running(limit.max(1))?;
    let stale = stale_coding_session_map(&events, &sessions)?;
    if stale.is_empty() {
        println!("No stale coding sessions required repair.");
        return Ok(());
    }

    let mut repaired = Vec::new();
    for session in sessions {
        let Some(candidate) = stale.get(&session.id) else {
            continue;
        };
        let (report_path, evidence_path) = repair_stale_coding_session(
            Arc::clone(&memory),
            Arc::clone(&events),
            &session,
            candidate,
        )?;
        repaired.push((session, candidate.clone(), report_path, evidence_path));
    }

    println!("Repaired {} stale coding session(s)", repaired.len());
    for (session, candidate, report_path, evidence_path) in repaired {
        println!(
            "  {} repaired after {} minute(s) idle and {} later process starts",
            short_fragment(&session.id),
            candidate.idle_minutes,
            candidate.newer_process_starts
        );
        println!("    report: {}", report_path.display());
        println!("    evidence: {}", evidence_path.display());
    }
    Ok(())
}

fn repair_stale_coding_session(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    session: &CodingSessionRecord,
    candidate: &CodingSessionStaleCandidate,
) -> Result<(PathBuf, PathBuf)> {
    let mut checks = session.checks.clone();
    checks.push(format!(
        "stale-session repair after {} later daemon starts",
        candidate.newer_process_starts
    ));
    let mut step_outcomes = session.step_outcomes.clone();
    step_outcomes.push(format!(
        "stale session reconciled after {} minute(s) idle",
        candidate.idle_minutes
    ));
    let mut artifacts = session.artifacts.clone();
    artifacts.sort();
    artifacts.dedup();
    let mut report = CodingSessionReport {
        id: session.id.clone(),
        generated_at: session.generated_at.to_rfc3339(),
        goal: session.goal.clone(),
        requested_goal: session.goal.clone(),
        exercise: session.exercise.clone(),
        status: "failed".to_string(),
        workspace: session.workspace.clone(),
        smoke_id: session.smoke_id,
        smoke_report_path: session.smoke_report_path.clone(),
        session_report_path: None,
        transcript_path: session.transcript_path.clone(),
        checks,
        plan_steps: session.plan_steps.clone(),
        step_outcomes,
        artifacts,
        failure_reason: Some(format!(
            "stale coding session recovered: {}",
            candidate.reason
        )),
    };
    let event_session_id =
        uuid::Uuid::parse_str(&session.id).unwrap_or_else(|_| uuid::Uuid::new_v4());
    persist_coding_session_terminal_report(
        memory,
        events,
        event_session_id,
        session.generated_at,
        &mut report,
        "repaired stale coding-session evidence written to",
        "repaired stale coding-session report written to",
    )
}

fn print_coding_sessions(memory: Arc<MemoryManager>, limit: usize) -> Result<()> {
    let sessions = CodingSessionStore::new(Arc::clone(&memory.db)).recent(limit)?;
    if sessions.is_empty() {
        println!("No coding sessions recorded yet.");
        return Ok(());
    }
    let events = EventStore::new(Arc::clone(&memory.db));
    let stale = stale_coding_session_map(&events, &sessions)?;
    println!("Recent coding sessions");
    for session in sessions {
        let stale_candidate = stale.get(&session.id);
        println!(
            "{} {} session={} exercise={} smoke={} checks={} artifacts={}{} {}",
            session.generated_at.format("%Y-%m-%d %H:%M:%S"),
            coding_session_display_status(&session, stale_candidate),
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
        if let Some(candidate) = stale_candidate {
            println!("  stale: {}", truncate(&candidate.reason, 160));
            println!("  repair: {}", coding_session_repair_command(limit.max(1)));
        }
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

fn print_coding_session_review(memory: Arc<MemoryManager>, session_ref: &str) -> Result<()> {
    let store = CodingSessionStore::new(Arc::clone(&memory.db));
    let Some(session) = store.get_by_ref(session_ref)? else {
        println!("No coding session found for '{session_ref}'.");
        return Ok(());
    };
    let events = EventStore::new(Arc::clone(&memory.db));
    let stale_candidate = stale_candidate(&events, &session, chrono::Utc::now())?;
    let repo_root = default_repo_root();
    println!("Professor X coding session review");
    println!("  session: {}", session.id);
    println!(
        "  status: {}",
        coding_session_display_status(&session, stale_candidate.as_ref())
    );
    println!("  exercise: {}", session.exercise);
    println!("  generated: {}", session.generated_at.to_rfc3339());
    println!("  goal: {}", session.goal);
    if let Some(workspace) = &session.workspace {
        println!("  workspace: {workspace}");
    }
    if let Some(smoke_id) = session.smoke_id {
        println!("  smoke: #{smoke_id}");
    }
    println!(
        "  report: {}{}",
        session.session_report_path,
        existing_marker(&repo_root, &session.session_report_path)
    );
    if let Some(path) = &session.smoke_report_path {
        println!(
            "  smoke report: {path}{}",
            existing_marker(&repo_root, path)
        );
    }
    if let Some(path) = &session.transcript_path {
        println!("  transcript: {path}{}", existing_marker(&repo_root, path));
    }
    if let Some(reason) = &session.failure_reason {
        println!("  failure: {}", truncate(reason, 180));
    }
    if let Some(candidate) = &stale_candidate {
        println!("  stale: {}", truncate(&candidate.reason, 180));
    }

    println!("Plan");
    if session.plan_steps.is_empty() {
        println!("  no plan steps recorded");
    } else {
        for (index, step) in session.plan_steps.iter().enumerate() {
            println!("  {}. {}", index + 1, step);
        }
    }

    println!("Outcomes");
    if session.step_outcomes.is_empty() {
        println!("  no step outcomes recorded");
    } else {
        for (index, outcome) in session.step_outcomes.iter().enumerate() {
            println!("  {}. {}", index + 1, outcome);
        }
    }

    println!("Checks");
    if session.checks.is_empty() {
        println!("  no checks recorded");
    } else {
        for check in &session.checks {
            println!("  - {check}");
        }
    }

    println!("Artifacts");
    if session.artifacts.is_empty() {
        println!("  no artifacts recorded");
    } else {
        for artifact in &session.artifacts {
            print_coding_session_artifact_review(&repo_root, artifact)?;
        }
    }

    println!("Publish readiness");
    if stale_candidate.is_some() {
        println!(
            "  blocked: repair the stale session first with {}",
            coding_session_repair_command(10)
        );
    } else {
        print_coding_session_publish_readiness(&repo_root, &session)?;
    }

    println!("Commands");
    println!(
        "  replay transcript: cargo run -- --task-review {}",
        session
            .transcript_path
            .as_ref()
            .map(|_| "latest")
            .unwrap_or("latest")
    );
    println!(
        "  review again: cargo run -- --prof-x-code-review {}",
        short_fragment(&session.id)
    );
    println!(
        "  publish evidence: cargo run -- --prof-x-code-publish {}",
        short_fragment(&session.id)
    );
    if stale_candidate.is_some() {
        println!("  repair stale row: cargo run -- --prof-x-code-repair 10");
    }
    println!("  watch: cargo run -- --observe-work");
    Ok(())
}

fn print_coding_session_publish_readiness(
    repo_root: &std::path::Path,
    session: &CodingSessionRecord,
) -> Result<()> {
    match coding_session_publish_readiness(repo_root, session) {
        Ok(paths) => {
            println!("  ready: {} artifact(s) selected", paths.len());
            for path in paths {
                println!("  publishable: {}", path.display());
            }
        }
        Err(err) => {
            println!("  blocked: {err}");
        }
    }
    Ok(())
}

fn coding_session_publish_readiness(
    repo_root: &std::path::Path,
    session: &CodingSessionRecord,
) -> Result<Vec<PathBuf>> {
    publishable_coding_session_artifact_paths(repo_root, session)
}

fn publish_coding_session_artifacts(memory: Arc<MemoryManager>, session_ref: &str) -> Result<()> {
    let store = CodingSessionStore::new(Arc::clone(&memory.db));
    let session = store
        .get_by_ref(session_ref)?
        .ok_or_else(|| anyhow::anyhow!("no coding session found for '{session_ref}'"))?;
    let repo_root = default_repo_root();
    let paths = publishable_coding_session_artifact_paths(&repo_root, &session)?;
    let commit = commit_coding_session_artifacts(&repo_root, &paths, &session)?;

    println!(
        "Published Professor X coding session {}",
        short_fragment(&session.id)
    );
    println!("  commit: {commit}");
    for path in paths {
        println!("  artifact: {}", path.display());
    }
    Ok(())
}

fn publishable_coding_session_artifact_paths(
    repo_root: &std::path::Path,
    session: &CodingSessionRecord,
) -> Result<Vec<PathBuf>> {
    let mut candidates = Vec::new();
    candidates.push(session.session_report_path.clone());
    if let Some(path) = &session.smoke_report_path {
        candidates.push(path.clone());
    }
    if let Some(path) = &session.transcript_path {
        candidates.push(path.clone());
    }
    candidates.extend(session.artifacts.iter().cloned());

    let mut paths = Vec::new();
    for candidate in candidates {
        let resolved = resolve_report_reference(repo_root, &candidate);
        if !resolved.exists() {
            continue;
        }
        let relative = repo_relative_existing_path(repo_root, &resolved)?;
        if !publishable_coding_session_artifact_path(&relative) {
            anyhow::bail!(
                "refusing to publish coding session artifact '{}' because it is outside the coding-session artifact allowlist",
                relative.display()
            );
        }
        paths.push(relative);
    }
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        anyhow::bail!(
            "no existing coding session artifacts found for {}",
            short_fragment(&session.id)
        );
    }
    Ok(paths)
}

fn publishable_coding_session_artifact_path(path: &std::path::Path) -> bool {
    publishable_run_artifact_path(path)
        || path
            .to_string_lossy()
            .starts_with("professor-x/artifacts/commands/")
        || path.to_string_lossy().starts_with("artifacts/commands/")
        || path
            .to_string_lossy()
            .starts_with("professor-x/artifacts/repo-patches/")
        || path
            .to_string_lossy()
            .starts_with("artifacts/repo-patches/")
}

fn git_add_publishable_artifacts(
    repo_root: &std::path::Path,
    paths: &[PathBuf],
    label: &str,
) -> Result<()> {
    let mut add = std::process::Command::new("git");
    add.arg("add").arg("-f").arg("--");
    for path in paths {
        add.arg(path);
    }
    let add = add.current_dir(repo_root).output()?;
    if !add.status.success() {
        anyhow::bail!(
            "git add {label} failed: {}",
            String::from_utf8_lossy(&add.stderr)
        );
    }
    Ok(())
}

fn commit_coding_session_artifacts(
    repo_root: &std::path::Path,
    paths: &[PathBuf],
    session: &CodingSessionRecord,
) -> Result<String> {
    if paths.is_empty() {
        anyhow::bail!("no coding session artifacts selected for publish");
    }
    git_add_publishable_artifacts(repo_root, paths, "coding session artifacts")?;
    let diff = std::process::Command::new("git")
        .args(["diff", "--cached", "--quiet", "--"])
        .args(paths)
        .current_dir(repo_root)
        .status()?;
    if diff.success() {
        anyhow::bail!("coding session artifacts are already published; no staged changes");
    }
    let message = format!(
        "professor-x: publish coding session {}",
        short_fragment(&session.id)
    );
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(repo_root)
        .output()?;
    if !commit.status.success() {
        anyhow::bail!(
            "git commit coding session artifacts failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
    }
    git_head(repo_root)
}

fn print_coding_session_artifact_review(repo_root: &std::path::Path, artifact: &str) -> Result<()> {
    let artifact_path = resolve_report_reference(repo_root, artifact);
    println!(
        "  - {}{}",
        artifact,
        if artifact_path.exists() {
            ""
        } else {
            " (missing)"
        }
    );
    if artifact_path.exists() {
        if let Some(summary) = command_artifact_summary(&artifact_path)? {
            println!("    {summary}");
        }
    }
    Ok(())
}

fn command_artifact_summary(path: &std::path::Path) -> Result<Option<String>> {
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.ends_with(".json"))
        .unwrap_or(false)
    {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&raw)?;
    let Some(command) = value.get("command").and_then(|value| value.as_str()) else {
        return Ok(None);
    };
    let success = value
        .get("success")
        .and_then(|value| value.as_bool())
        .map(|success| if success { "passed" } else { "failed" })
        .unwrap_or("unknown");
    let exit_code = value
        .get("exit_code")
        .and_then(|value| value.as_i64())
        .map(|code| code.to_string())
        .unwrap_or_else(|| "none".to_string());
    let stdout_bytes = value
        .get("stdout_bytes")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let stderr_bytes = value
        .get("stderr_bytes")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    Ok(Some(format!(
        "command `{}` {} exit={} stdout={}B stderr={}B",
        truncate(command, 90),
        success,
        exit_code,
        stdout_bytes,
        stderr_bytes
    )))
}

fn existing_marker(repo_root: &std::path::Path, raw: &str) -> &'static str {
    if resolve_report_reference(repo_root, raw).exists() {
        ""
    } else {
        " (missing)"
    }
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

fn print_autonomy_queue(memory: Arc<MemoryManager>, limit: usize) -> Result<()> {
    let items = AutonomyQueueStore::new(Arc::clone(&memory.db)).recent(limit)?;
    if items.is_empty() {
        println!("No autonomous queue items recorded yet.");
        return Ok(());
    }
    println!("Recent autonomous queue items");
    for item in items {
        println!("{}", format_autonomy_queue_item(&item));
        if let Some(report) = &item.result_report_path {
            println!("  report: {report}");
        }
        if let Some(run_id) = &item.result_run_id {
            println!("  run: {}", short_fragment(run_id));
        }
        if let Some(reason) = &item.failure_reason {
            println!("  failure: {}", truncate(reason, 140));
        }
    }
    Ok(())
}

fn resolve_autonomy_queue_item(
    memory: Arc<MemoryManager>,
    queue_ref: &str,
) -> Result<AutonomyQueueItem> {
    AutonomyQueueStore::new(Arc::clone(&memory.db))
        .resolve_ref(queue_ref)?
        .ok_or_else(|| anyhow::anyhow!("no autonomous queue item found for '{queue_ref}'"))
}

fn queue_item_run_ref(item: &AutonomyQueueItem) -> Result<&str> {
    item.result_run_id
        .as_deref()
        .or(item.result_report_path.as_deref())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "queue item {} has no result run yet; run it first with --prof-x-step-live 1",
                short_fragment(&item.id)
            )
        })
}

fn print_autonomy_queue_review(memory: Arc<MemoryManager>, queue_ref: &str) -> Result<()> {
    let item = resolve_autonomy_queue_item(Arc::clone(&memory), queue_ref)?;
    println!("Professor X queue item review");
    println!("{}", format_autonomy_queue_item(&item));
    if let Some(reason) = &item.failure_reason {
        println!("  failure: {}", truncate(reason, 180));
    }
    let run_ref = queue_item_run_ref(&item)?;
    println!("  linked run: {}", truncate(run_ref, 160));
    print_run_review(memory, run_ref)
}

fn print_autonomy_queue_replay(memory: Arc<MemoryManager>, queue_ref: &str) -> Result<()> {
    let item = resolve_autonomy_queue_item(Arc::clone(&memory), queue_ref)?;
    let run_ref = queue_item_run_ref(&item)?;
    println!(
        "Professor X queue item replay queue={} run={}",
        short_fragment(&item.id),
        truncate(run_ref, 120)
    );
    print_run_replay(memory, run_ref)
}

fn publish_autonomy_queue_run(memory: Arc<MemoryManager>, queue_ref: &str) -> Result<()> {
    let item = resolve_autonomy_queue_item(Arc::clone(&memory), queue_ref)?;
    let run_ref = queue_item_run_ref(&item)?;
    println!(
        "Publishing Professor X queue item queue={} run={}",
        short_fragment(&item.id),
        truncate(run_ref, 120)
    );
    publish_run_artifacts(memory, run_ref)
}

fn format_autonomy_queue_item(item: &AutonomyQueueItem) -> String {
    let brief = autonomy_queue_brief(item, 96);
    format!(
        "{} {} queue={} {}",
        item.updated_at.format("%Y-%m-%d %H:%M:%S"),
        item.status,
        brief.queue_id,
        brief.summary,
    )
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

/// Consciousness measurement report — turns the seven scattered trajectory
/// tables into the five empirical questions the thesis rests on. Each question
/// gets a number and a verdict: supported / inconclusive / not-supported /
/// no-data-yet. This is the instrument that makes the architecture legible.
/// HIRO pass@3 trajectory across rounds, with mean, σ, and the minimum
/// detectable effect — the noise floor any evolution gain must beat to be real.
fn print_hiro_trajectory(memory: &Arc<MemoryManager>) -> Result<()> {
    let rows: Vec<(u32, f32, String)> = {
        let db = memory.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT round, pass_at_3, COALESCE(harness_commit,'') \
             FROM hiro_rounds ORDER BY round ASC",
        )?;
        let r = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)? as u32,
                row.get::<_, f64>(1)? as f32,
                row.get::<_, String>(2)?,
            ))
        })?;
        r.filter_map(|x| x.ok()).collect()
    };

    println!("HIRO performance (pass@3) — the measurement floor");
    println!("--------------------------------------------------");
    if rows.is_empty() {
        println!("  no rounds recorded yet\n");
        return Ok(());
    }
    for (round, p, commit) in &rows {
        let bar = "#".repeat((p * 40.0).round() as usize);
        println!(
            "  r{round:<2} {p:.3} |{bar:<40}| {}",
            &commit[..commit.len().min(7)]
        );
    }

    // σ is only meaningful over a FROZEN harness — rounds sharing one commit.
    // Mixing commits measures harness changes, not run-to-run noise. Use the
    // largest group of rounds that share a harness_commit (the baseline set).
    use std::collections::HashMap;
    let mut by_commit: HashMap<&str, Vec<f32>> = HashMap::new();
    for (_, p, commit) in &rows {
        by_commit.entry(commit.as_str()).or_default().push(*p);
    }
    let frozen = by_commit
        .iter()
        .filter(|(c, _)| !c.is_empty())
        .max_by_key(|(_, v)| v.len());

    match frozen {
        Some((commit, vals)) if vals.len() >= 2 => {
            let n = vals.len() as f32;
            let mean = vals.iter().sum::<f32>() / n;
            let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / (n - 1.0);
            let sd = var.sqrt();
            let mde = 1.96 * sd;
            println!(
                "\n  frozen-harness baseline ({}): n={} rounds  mean={:.3}  σ={:.3}",
                &commit[..commit.len().min(7)],
                vals.len(),
                mean,
                sd
            );
            println!(
                "  minimum detectable effect ≈ {:.3} (1.96σ) — an evolution change must",
                mde
            );
            println!(
                "  move pass@3 above {:.3} to count as real, not run-to-run noise.\n",
                mean + mde
            );
        }
        _ => {
            let mean = rows.iter().map(|(_, p, _)| *p).sum::<f32>() / rows.len() as f32;
            println!(
                "\n  mean={mean:.3} across {} round(s); need ≥2 rounds on ONE frozen \
                 harness commit for σ. (baseline in progress)\n",
                rows.len()
            );
        }
    }
    Ok(())
}

fn print_consciousness_report(memory: Arc<MemoryManager>) -> Result<()> {
    println!("Professor X — consciousness measurement report");
    println!("================================================");
    println!("The thesis: an agent that knows itself, and grows more integrated as");
    println!("it evolves, improves in ways a frozen one cannot. Five questions:\n");

    // ── HIRO performance trajectory + variance (the measurement floor) ────
    print_hiro_trajectory(&memory)?;

    // ── Q1: Integrated information (phi) rising? ──────────────────────────
    let phi_traj = memory.phi.trajectory()?;
    let phi_slope = memory.phi.slope()?;
    println!("Q1. Does integrated information (phi) rise as the harness evolves?");
    if phi_traj.is_empty() {
        println!("    no data yet — run HIRO rounds to populate phi_rounds\n");
    } else {
        let first = phi_traj.first().map(|(_, p)| *p).unwrap_or(0.0);
        let last = phi_traj.last().map(|(_, p)| *p).unwrap_or(0.0);
        println!(
            "    rounds={}  phi: {:.3} → {:.3}  slope={}",
            phi_traj.len(),
            first,
            last,
            phi_slope
                .map(|s| format!("{s:+.4}/round"))
                .unwrap_or_else(|| "n/a".into()),
        );
        println!(
            "    {}\n",
            verdict_slope(
                phi_slope,
                0.0,
                "phi rising (integration growing)",
                "phi flat/falling"
            )
        );
    }

    // ── Q1b: DIFFERENTIATION — Lempel-Ziv complexity of module activity ───
    // phi sees only integration. Consciousness needs integration AND
    // differentiation (complexity). LZc (Schartner et al. 2015) is the
    // differentiation axis: down under anaesthesia, up under psychedelics.
    // A conscious-candidate signature is HIGH on BOTH axes simultaneously.
    println!("Q1b. Is module activity DIFFERENTIATED (complex), not stereotyped?");
    {
        use crate::memd::pci;
        let indices: Vec<usize> = {
            let db = memory.db.lock().unwrap();
            let mut stmt = db.prepare(
                "SELECT activation_index FROM phi_activations ORDER BY round ASC, id ASC",
            )?;
            let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
            rows.filter_map(|r| r.ok().map(|v| v as usize)).collect()
        };
        if indices.len() < 8 {
            println!("    not enough activation samples yet (need ≥8)\n");
        } else {
            let matrix = pci::matrix_from_activation_indices(&indices, 7);
            let lzc = pci::normalized_lzc(&matrix);
            // integration×differentiation: a conscious candidate is high on both.
            let phi_now = memory
                .phi
                .trajectory()?
                .last()
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            println!(
                "    n={} steps  LZc (differentiation) = {:.3}   [0=stereotyped, ~1=random]",
                indices.len(),
                lzc
            );
            println!(
                "    integration×differentiation: phi={phi_now:.2} × LZc={lzc:.2}  →  both-high is the signature"
            );
            let verdict = if lzc > 0.4 && lzc < 1.2 && phi_now > 0.3 {
                "✓ structured complexity (integrated AND differentiated)"
            } else if lzc <= 0.4 {
                "✗ too stereotyped (integrated but not differentiated — seizure-like)"
            } else {
                "✗ too random (differentiated but not integrated — noise-like)"
            };
            println!("    {verdict}\n");
        }
    }

    // ── Q2: Interoceptive prediction error falling? ───────────────────────
    println!("Q2. Is the body-model sharpening (interoceptive error falling)?");
    let intero_traj = round_bucketed_mean(
        &memory.db,
        "SELECT round, AVG(interoceptive_error) FROM computational_vitals \
         WHERE interoceptive_error IS NOT NULL GROUP BY round ORDER BY round ASC",
    )?;
    if intero_traj.is_empty() {
        println!("    no data yet\n");
    } else {
        let slope = least_squares_slope(&intero_traj);
        let first = intero_traj.first().map(|(_, v)| *v).unwrap_or(0.0);
        let last = intero_traj.last().map(|(_, v)| *v).unwrap_or(0.0);
        println!(
            "    error: {first:.3} → {last:.3}  slope={}",
            slope
                .map(|s| format!("{s:+.4}/round"))
                .unwrap_or_else(|| "n/a".into())
        );
        // falling error = negative slope is good
        println!(
            "    {}\n",
            verdict_slope(
                slope.map(|s| -s),
                0.0,
                "body-model sharpening",
                "body-model not improving"
            )
        );
    }

    // ── Q3: Self-prediction error converging? ─────────────────────────────
    println!("Q3. Is self-knowledge converging (self-prediction error falling)?");
    let n_pred = memory.self_prediction.count()?;
    if n_pred == 0 {
        println!("    no data yet\n");
    } else {
        let mean = memory.self_prediction.mean_error(200)?.unwrap_or(0.0);
        let dims = memory.self_prediction.mean_error_by_dimension(200)?;
        print!("    n={n_pred}  mean self-prediction error={mean:.3}");
        if let Some(d) = dims {
            let (blind, val) = [
                ("tools", d.tool_err),
                ("steps", d.step_err),
                ("success", d.success_err),
            ]
            .into_iter()
            .fold(("", 0.0), |acc, x| if x.1 > acc.1 { x } else { acc });
            println!("  | blind spot: {blind} ({val:.3})");
        } else {
            println!();
        }
        println!("    (trajectory needs ≥2 rounds; lower mean = better self-knowledge)\n");
    }

    // ── Q3b: METACOGNITION — does confidence track its own correctness? ───
    // Type-2 AUROC (Fleming & Lau 2014): the operational signature of Higher-
    // Order Theories — a state is conscious when the system has a usable
    // representation of being in it. Here: does the agent's pre-task confidence
    // discriminate its OWN correct from incorrect outcomes? 0.5 = blind.
    println!("Q3b. Does the agent KNOW when it is right (metacognitive sensitivity)?");
    match memory.self_prediction.metacognitive_auroc(400)? {
        Some(m) => {
            println!(
                "    Type-2 AUROC = {:.3}   (0.5 = no metacognition, >0.5 = self-monitoring)  n={}",
                m.auroc, m.n
            );
            println!(
                "    confidence when right = {:.2}  vs  when wrong = {:.2}  (gap {:+.2})",
                m.mean_conf_correct,
                m.mean_conf_incorrect,
                m.mean_conf_correct - m.mean_conf_incorrect
            );
            let verdict = if m.auroc >= 0.6 {
                "✓ genuine metacognitive sensitivity (knows when it is right)"
            } else if m.auroc >= 0.53 {
                "~ weak but present metacognition"
            } else {
                "✗ confidence does not track correctness (no self-monitoring yet)"
            };
            println!("    {verdict}\n");
        }
        None => println!("    not enough data (need both correct and incorrect trials)\n"),
    }

    // ── Q4: Does default-mode wandering produce insight? ──────────────────
    println!("Q4. Does mind-wandering (DMN) produce insight that feeds evolution?");
    let dmn_insights: i64 = memory
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM cognition WHERE source LIKE 'dmn:%'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let narrative_chapters = memory.narrative.count()?;
    println!("    DMN insights stored={dmn_insights}  narrative chapters={narrative_chapters}");
    if dmn_insights == 0 {
        println!("    no data yet (DMN runs between evolution cycles)\n");
    } else {
        println!("    insights are accumulating; acceptance-correlation needs evolution runs\n");
    }

    // ── Q5: Identity coherence holding under self-modification? ───────────
    println!("Q5. Does identity hold (ICS ≥ 0.70) while the harness transforms?");
    let ics_traj = memory.ics.trajectory_vs_seed()?;
    if ics_traj.is_empty() {
        println!("    no data yet (ICS computed at each self-model update, every 10 rounds)\n");
    } else {
        let latest = ics_traj.last().map(|(_, s)| *s).unwrap_or(0.0);
        let latest_round = ics_traj.last().map(|(r, _)| *r).unwrap_or(0);
        let verdict = if latest >= 0.70 {
            "✓ identity coherent (≥0.70)"
        } else if latest >= 0.50 {
            "— drifting (alert: <0.70)"
        } else {
            "✗ identity incoherent (halt: <0.50)"
        };
        println!("    round {latest_round} ICS vs seed={latest:.3}  {verdict}\n");
    }

    // ── Supporting metrics ────────────────────────────────────────────────
    println!("Supporting metrics");
    println!("------------------");
    // FED — world model
    match memory.free_energy.slope_per_round()? {
        Some(s) => println!(
            "  FED (world-model error) slope: {s:+.4}/round  {}",
            if s < 0.0 {
                "✓ world model improving"
            } else {
                "— not improving"
            }
        ),
        None => println!("  FED: no data yet"),
    }
    // Self-authored tests — the invention
    let sat_count = memory.self_authored_tests.count()?;
    let sat_rate = memory.self_authored_tests.mean_pass_rate()?;
    println!(
        "  Self-authored tests: {sat_count} authored, mean pass-rate {}",
        sat_rate
            .map(|r| format!("{r:.2}"))
            .unwrap_or_else(|| "n/a".into())
    );
    // Causal traces — STDP
    println!(
        "  Causal traces recorded: {}",
        memory.causal_traces.count()?
    );
    // MCA — metacognitive calibration (if any rounds)
    if let Some((round, _)) = phi_traj.last() {
        let metacog = memd::metacognitive::MetacognitiveStore::new(Arc::clone(&memory.db));
        let (mca, n) = metacog.mca_rolling(*round, 10)?;
        if n > 0 {
            println!(
                "  MCA (metacognitive calibration, last 10 rounds): {mca:.2} over {n} attributions"
            );
        }
    }

    println!("\nReading: 2+ of Q1-Q5 supported across 30 rounds → the thesis holds.");
    println!("Right now this is the instrument; the HIRO + evolution runs are the experiment.");
    Ok(())
}

/// Verdict helper: given a slope and a threshold, render a supported/not line.
fn verdict_slope(slope: Option<f32>, threshold: f32, yes: &str, no: &str) -> String {
    match slope {
        Some(s) if s > threshold + 1e-6 => format!("✓ {yes}"),
        Some(s) if s < threshold - 1e-6 => format!("✗ {no}"),
        Some(_) => format!("— inconclusive (flat)"),
        None => "— inconclusive (need ≥2 rounds)".to_string(),
    }
}

/// Run a `SELECT round, AVG(x) ... GROUP BY round` query into a trajectory.
fn round_bucketed_mean(
    db: &Arc<std::sync::Mutex<rusqlite::Connection>>,
    sql: &str,
) -> Result<Vec<(u32, f32)>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)? as u32,
            r.get::<_, Option<f64>>(1)?.unwrap_or(0.0) as f32,
        ))
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

/// Least-squares slope over a (round, value) trajectory.
fn least_squares_slope(traj: &[(u32, f32)]) -> Option<f32> {
    if traj.len() < 2 {
        return None;
    }
    let n = traj.len() as f32;
    let mean_x = traj.iter().map(|(x, _)| *x as f32).sum::<f32>() / n;
    let mean_y = traj.iter().map(|(_, y)| *y).sum::<f32>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for (x, y) in traj {
        let dx = *x as f32 - mean_x;
        num += dx * (*y - mean_y);
        den += dx * dx;
    }
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}

fn print_prof_x_brief(memory: Arc<MemoryManager>, events: Arc<EventStore>) -> Result<()> {
    let latest_run = WorkLoopRunStore::new(Arc::clone(&memory.db)).latest()?;
    let latest_session = CodingSessionStore::new(Arc::clone(&memory.db)).latest()?;
    let recent_events = events.work_tail(12)?;
    println!(
        "{}",
        format_prof_x_brief(latest_run.as_ref(), latest_session.as_ref(), &recent_events)
    );
    Ok(())
}

fn format_prof_x_brief(
    latest_run: Option<&WorkLoopRunRecord>,
    latest_session: Option<&CodingSessionRecord>,
    recent_events: &[memd::events::AgentEvent],
) -> String {
    let mut lines = Vec::new();
    lines.push("Professor X operator brief".to_string());
    lines.push(format!(
        "state {}  {}",
        cockpit_state(latest_run, None),
        cockpit_latest_activity(recent_events)
    ));
    lines.push(format!("signal {}", work_signal_summary(recent_events)));
    lines.push(String::new());

    lines.push("Latest run".to_string());
    match latest_run {
        Some(run) => {
            let status = if run.failed_cycles == 0 {
                "passed"
            } else {
                "needs-review"
            };
            lines.push(format!(
                "  {} {}:{} run={} cycles={}/{} passed={} failed={}",
                status,
                run.run_kind,
                run.profile,
                short_fragment(&run.run_id),
                run.completed_cycles,
                run.requested_cycles,
                run.passed_cycles,
                run.failed_cycles
            ));
            lines.push(format!("  report {}", truncate(&run.report_path, 130)));
            if let Some(ledger) = run_ledger_path(run) {
                lines.push(format!("  ledger {}", truncate(&ledger, 130)));
            }
            if let Some(last_gate) = run.smoke_records.last() {
                lines.push(format!(
                    "  last gate cycle={} {} {} {}",
                    last_gate.cycle,
                    last_gate.kind,
                    if last_gate.passed { "passed" } else { "failed" },
                    truncate(&last_gate.detail, 90)
                ));
                lines.push(format!("  proof {}", truncate(&last_gate.report_path, 130)));
                if let Some(transcript) = &last_gate.transcript_path {
                    lines.push(format!("  transcript {}", truncate(transcript, 130)));
                }
            }
            lines.push(format!(
                "  commands review=--run-review {} replay=--replay {} publish=--publish-run {}",
                short_fragment(&run.run_id),
                short_fragment(&run.run_id),
                short_fragment(&run.run_id)
            ));
        }
        None => {
            lines.push("  no run recorded yet".to_string());
            lines.push("  command /run 4 or cargo run -- --prof-x-run 4".to_string());
        }
    }

    lines.push(String::new());
    lines.push("Latest coding session".to_string());
    match latest_session {
        Some(session) => {
            let commit = coding_session_commit_hint(session)
                .map(|commit| format!(" commit={commit}"))
                .unwrap_or_default();
            lines.push(format!(
                "  {} session={} exercise={} checks={} artifacts={}{}",
                session.status,
                short_fragment(&session.id),
                session.exercise,
                session.checks.len(),
                session.artifacts.len(),
                commit
            ));
            lines.push(format!("  goal {}", truncate(&session.goal, 120)));
            lines.push(format!(
                "  report {}",
                truncate(&session.session_report_path, 130)
            ));
            if let Some(smoke_report) = &session.smoke_report_path {
                lines.push(format!("  smoke {}", truncate(smoke_report, 130)));
            }
            if let Some(transcript) = &session.transcript_path {
                lines.push(format!("  transcript {}", truncate(transcript, 130)));
            }
            for artifact in session.artifacts.iter().take(3) {
                lines.push(format!("  artifact {}", truncate(artifact, 130)));
            }
            lines.push("  command --coding-sessions 5".to_string());
        }
        None => {
            lines.push("  no coding session recorded yet".to_string());
            lines.push("  command /run-commit 6 after safety gates are green".to_string());
        }
    }

    lines.push(String::new());
    lines.push("Recent work".to_string());
    if recent_events.is_empty() {
        lines.push("  no recent work events".to_string());
    } else {
        for event in recent_events.iter().rev().take(5).rev() {
            lines.push(format!(
                "  #{} {} {}",
                event.id,
                event.event_type,
                truncate(&event.summary, 100)
            ));
        }
    }
    lines.push(String::new());
    lines.push(
        "Open: cargo run -- --prof-x-chat | cargo run -- --cockpit | cargo run -- --observe-work"
            .to_string(),
    );
    lines.join("\n")
}

fn run_ledger_path(run: &WorkLoopRunRecord) -> Option<String> {
    let repo_root = default_repo_root();
    let report_path = resolve_report_reference(&repo_root, &run.report_path);
    let raw = std::fs::read_to_string(report_path).ok()?;
    let report: SupervisedLoopReport = serde_json::from_str(&raw).ok()?;
    report.ledger_path.filter(|path| !path.is_empty())
}

fn run_journal_path(run: &WorkLoopRunRecord) -> Option<String> {
    let repo_root = default_repo_root();
    let report_path = resolve_report_reference(&repo_root, &run.report_path);
    let raw = std::fs::read_to_string(report_path).ok()?;
    let report: SupervisedLoopReport = serde_json::from_str(&raw).ok()?;
    report.journal_path.filter(|path| !path.is_empty())
}

fn format_run_log_entry(run: &WorkLoopRunRecord, ledger_path: Option<&str>) -> String {
    let status = if run.failed_cycles == 0 {
        "passed"
    } else {
        "failed"
    };
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
    if let Some(path) = run_journal_path(run) {
        lines.push(format!("  L journal {path}"));
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
    if let Some(queue_id) = &report.queue_id {
        println!("  queue: {}", short_fragment(queue_id));
    }
    if let Some(goal) = &report.operator_goal {
        println!("  operator goal: {}", truncate(goal, 180));
    }
    println!("  started: {}", report.started_at);
    println!("  completed: {}", report.completed_at);
    println!(
        "  cycles: {}/{} passed={} failed={}",
        report.completed_cycles,
        report.requested_cycles,
        report.passed_cycles,
        report.failed_cycles
    );
    println!("  report: {}", display_repo_path(&repo_root, &report_path));
    if let Some(path) = &report.ledger_path {
        println!("  ledger: {path}");
    }
    if let Some(path) = &report.journal_path {
        println!("  journal: {path}");
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
        println!(
            "    report: {}",
            display_repo_path(&repo_root, &artifact_path)
        );
        if !artifact_path.exists() {
            println!("    report_status: missing");
        }
        if let Some(transcript) = &smoke.transcript_path {
            let transcript_path = resolve_report_reference(&repo_root, transcript);
            println!(
                "    transcript: {}{}",
                display_repo_path(&repo_root, &transcript_path),
                if transcript_path.exists() {
                    ""
                } else {
                    " (missing)"
                }
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
    if let Some(queue_id) = &report.queue_id {
        println!("queue: {}", short_fragment(queue_id));
    }
    if let Some(goal) = &report.operator_goal {
        println!("operator_goal: {}", truncate(goal, 180));
    }
    if let Some(journal) = &report.journal_path {
        println!("journal: {journal}");
    }
    println!();

    if !report.planned_jobs.is_empty() {
        println!("Plan");
        for job in &report.planned_jobs {
            println!("- cycle {} {}", job.cycle, truncate(&job.reason, 120));
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

    println!(
        "Published Professor X run {}",
        short_fragment(&report.run_id)
    );
    println!("  commit: {}", published.commit);
    for path in published.paths {
        println!("  artifact: {}", path.display());
    }
    Ok(())
}

fn write_prof_x_journal(events: Arc<EventStore>, limit: usize, commit: bool) -> Result<()> {
    let repo_root = default_repo_root();
    let recent_events = events.work_tail(limit)?;
    let timestamp = chrono::Utc::now();
    let commit_id = git_head(&repo_root).unwrap_or_else(|_| "unknown".to_string());
    let journal = format_prof_x_journal_markdown(&repo_root, timestamp, &commit_id, &recent_events);
    let path = prof_x_journal_path(&repo_root, timestamp);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, journal)?;

    println!("Wrote Professor X journal");
    println!("  path: {}", display_repo_path(&repo_root, &path));
    println!("  events: {}", recent_events.len());

    if commit {
        let relative = repo_relative_existing_path(&repo_root, &path)?;
        let published = commit_prof_x_journal(&repo_root, &relative, timestamp)?;
        println!("  commit: {published}");
    } else {
        println!(
            "  publish: cargo run -- --prof-x-journal-commit {}",
            limit.clamp(1, 500)
        );
    }
    Ok(())
}

fn prof_x_journal_path(
    repo_root: &std::path::Path,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> PathBuf {
    repo_root
        .join("professor-x")
        .join("artifacts")
        .join("work-loop")
        .join("ledger")
        .join(timestamp.format("%Y-%m-%d").to_string())
        .join(format!("prof-x-journal-{}.md", timestamp.format("%H%M%S")))
}

fn format_prof_x_journal_markdown(
    repo_root: &std::path::Path,
    timestamp: chrono::DateTime<chrono::Utc>,
    commit_id: &str,
    recent_events: &[memd::events::AgentEvent],
) -> String {
    let git_line = cockpit_git_line(repo_root);
    let status_raw = command_stdout(repo_root, "git", &["status", "--short"])
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "clean".to_string());
    let latest_activity = cockpit_latest_activity(recent_events);
    let mut lines = vec![
        format!(
            "# Professor X Work Journal - {}",
            timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        ),
        String::new(),
        "## Run Context".to_string(),
        format!("- generated_at: {}", timestamp.to_rfc3339()),
        format!("- harness_commit: {commit_id}"),
        format!("- git: {git_line}"),
        format!("- work_signal: {}", work_signal_summary(recent_events)),
        format!("- latest_activity: {latest_activity}"),
        String::new(),
        "## Working Tree".to_string(),
    ];

    if status_raw == "clean" {
        lines.push("- clean".to_string());
    } else {
        for line in status_raw.lines().take(40) {
            lines.push(format!("- `{}`", line));
        }
        if status_raw.lines().count() > 40 {
            lines.push("- ... truncated".to_string());
        }
    }

    lines.push(String::new());
    lines.push("## Timeline".to_string());
    if recent_events.is_empty() {
        lines.push("- no work events recorded yet".to_string());
    } else {
        for event in recent_events {
            lines.push(format_work_event(event));
        }
    }

    lines.push(String::new());
    lines.push("## Operator Commands".to_string());
    lines.push("- `cargo run -- --observe-work`".to_string());
    lines.push("- `cargo run -- --cockpit`".to_string());
    lines.push("- `cargo run -- --run-log 10`".to_string());
    lines.push("- `cargo run -- --prof-x-journal-commit 50`".to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn commit_prof_x_journal(
    repo_root: &std::path::Path,
    relative_path: &std::path::Path,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> Result<String> {
    let text = relative_path.to_string_lossy();
    if !text.starts_with("professor-x/artifacts/work-loop/ledger/") || !text.ends_with(".md") {
        anyhow::bail!(
            "refusing to commit non-journal artifact '{}'",
            relative_path.display()
        );
    }
    let add = std::process::Command::new("git")
        .arg("add")
        .arg("--")
        .arg(relative_path)
        .current_dir(repo_root)
        .output()?;
    if !add.status.success() {
        anyhow::bail!(
            "git add journal failed: {}",
            String::from_utf8_lossy(&add.stderr)
        );
    }

    let diff = std::process::Command::new("git")
        .args(["diff", "--cached", "--quiet", "--"])
        .arg(relative_path)
        .current_dir(repo_root)
        .status()?;
    if diff.success() {
        anyhow::bail!("journal is already committed; no staged changes");
    }

    let message = format!(
        "professor-x: journal {}",
        timestamp.format("%Y-%m-%d %H:%M:%S UTC")
    );
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(repo_root)
        .output()?;
    if !commit.status.success() {
        anyhow::bail!(
            "git commit journal failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
    }
    git_head(repo_root)
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
    let ledger = report.ledger_path.as_ref().ok_or_else(|| {
        anyhow::anyhow!("run report has no ledger_path; run it again before publishing")
    })?;
    paths.push(repo_relative_existing_path(
        repo_root,
        &resolve_report_reference(repo_root, ledger),
    )?);
    if let Some(journal) = report.journal_path.as_ref().filter(|path| !path.is_empty()) {
        paths.push(repo_relative_existing_path(
            repo_root,
            &resolve_report_reference(repo_root, journal),
        )?);
    }
    for smoke in &report.smoke_records {
        if let Some(path) = optional_publishable_run_artifact_path(repo_root, &smoke.report_path)? {
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
            if let Some(path) =
                optional_publishable_run_artifact_path(repo_root, candidate.display().to_string())?
            {
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
    git_add_publishable_artifacts(repo_root, paths, "run artifacts")?;
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
    if let Some(goal) = &report.operator_goal {
        println!("    operator goal: {}", truncate(goal, 140));
    }
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
    if let Some(goal) = &report.operator_goal {
        println!("    operator goal: {}", truncate(goal, 140));
    }
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
    let root = repo_root
        .join("professor-x")
        .join("artifacts")
        .join("work-loop");
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

fn print_task_evidence(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    task_ref: &str,
) -> Result<()> {
    let transcript = if task_ref == "latest" {
        transcripts.latest()?
    } else {
        transcripts.get_by_task_prefix(task_ref)?
    };
    let Some(transcript) = transcript else {
        println!("No transcript found for '{task_ref}'.");
        return Ok(());
    };
    let task_run =
        TaskRunStore::new(Arc::clone(&memory.db)).get_by_task_prefix(&transcript.task_id)?;
    let task_id = uuid::Uuid::parse_str(&transcript.task_id).with_context(|| {
        format!(
            "stored transcript task id is not a UUID: {}",
            transcript.task_id
        )
    })?;
    let task_events = events.for_task(task_id, 2000)?;
    let bundle = format_task_evidence_bundle(&transcript, task_run.as_ref(), &task_events);
    println!("{bundle}");
    let path = task_evidence_markdown_path(&transcript);
    std::fs::write(&path, &bundle)?;
    println!("\nevidence: {}", path.display());
    Ok(())
}

fn task_evidence_markdown_path(transcript: &TranscriptSummary) -> PathBuf {
    std::path::Path::new(&transcript.transcript_path).with_extension("evidence.md")
}

fn format_task_evidence_bundle(
    transcript: &TranscriptSummary,
    task_run: Option<&TaskRun>,
    events: &[memd::events::AgentEvent],
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Professor X task evidence {}",
        short_fragment(&transcript.task_id)
    ));
    lines.push(format!("  task: {}", transcript.task_id));
    lines.push(format!("  transcript: {}", transcript.transcript_path));
    lines.push(format!("  transcript_status: {}", transcript.status));
    lines.push(format!("  attempts: {}", transcript.attempt_count));
    lines.push(format!("  steps: {}", transcript.step_count));
    lines.push(format!(
        "  description: {}",
        truncate(&transcript.task_description, 180)
    ));
    lines.push(format!("  summary: {}", truncate(&transcript.summary, 180)));

    match task_run {
        Some(run) => {
            lines.push(String::new());
            lines.push("Run row".to_string());
            lines.push(format!("  status: {}", run.status));
            lines.push(format!("  type: {}", run.task_type));
            lines.push(format!("  priority: {}", run.priority));
            if let Some(score) = run.outcome_score {
                lines.push(format!("  score: {score:.2}"));
            }
            if let Some(mode) = &run.failure_mode {
                lines.push(format!("  failure: {}", truncate(mode, 180)));
            }
            if let Some(class) = run.failure_class {
                lines.push(format!("  failure_class: {}", class.as_str()));
            }
            if let Some(tool) = &run.last_tool {
                lines.push(format!("  last_tool: {tool}"));
            }
            if !run.last_summary.is_empty() {
                lines.push(format!(
                    "  last_summary: {}",
                    truncate(&run.last_summary, 180)
                ));
            }
            if let Some(error) = &run.last_error {
                lines.push(format!("  last_error: {}", truncate(error, 180)));
            }
            if !run.verification_summary.is_empty() {
                lines.push(format!(
                    "  verification: {}",
                    truncate(&run.verification_summary, 220)
                ));
            }
            if !run.verification_artifacts.is_empty() {
                lines.push(format!(
                    "  verification_artifacts: {}",
                    run.verification_artifacts.len()
                ));
                for artifact in run.verification_artifacts.iter().take(5) {
                    lines.push(format!("    - {}", truncate(artifact, 160)));
                }
            }
        }
        None => {
            lines.push(String::new());
            lines.push("Run row".to_string());
            lines.push("  no task_runs row recorded".to_string());
        }
    }

    let artifact_events = events
        .iter()
        .filter(|event| event.event_type.starts_with("artifact."))
        .collect::<Vec<_>>();
    lines.push(String::new());
    lines.push(format!("Artifact verdicts: {}", artifact_events.len()));
    if artifact_events.is_empty() {
        lines.push("  none recorded".to_string());
    } else {
        for event in artifact_events.iter().take(20) {
            lines.push(format!("  {}", one_line(&format_work_event(event), 240)));
        }
        if artifact_events.len() > 20 {
            lines.push(format!("  ... {} more", artifact_events.len() - 20));
        }
    }

    lines.push(String::new());
    lines.push(format!("Work events: {}", events.len()));
    for event in events.iter().take(40) {
        lines.push(format!("  {}", one_line(&format_work_event(event), 240)));
    }
    if events.len() > 40 {
        lines.push(format!("  ... {} more", events.len() - 40));
    }
    lines.push(String::new());
    lines.push(format!(
        "Replay: cargo run -- --task-review {}",
        short_fragment(&transcript.task_id)
    ));
    lines.join("\n")
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
        "{} {} task={} type={} p{} attempts={} steps={}{}{} {}",
        run.updated_at.format("%Y-%m-%d %H:%M:%S"),
        run.status,
        &run.task_id[..8.min(run.task_id.len())],
        run.task_type,
        run.priority,
        run.attempt_count,
        run.step_count,
        run.failure_class
            .map(|class| format!(" class={}", class.as_str()))
            .unwrap_or_default(),
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

fn print_work_status_json(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    limit: usize,
) -> Result<()> {
    let value = render_work_status_json(memory, events, limit)?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn render_work_status_json(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    limit: usize,
) -> Result<serde_json::Value> {
    let repo_root = default_repo_root();
    let recent_events = events.work_tail(limit)?;
    let latest_task_run = TaskRunStore::new(Arc::clone(&memory.db)).latest()?;
    let latest_run = WorkLoopRunStore::new(Arc::clone(&memory.db)).latest()?;
    let latest_coding_session = CodingSessionStore::new(Arc::clone(&memory.db)).latest()?;
    let latest_coding_session_stale = latest_coding_session
        .as_ref()
        .map(|session| stale_candidate(&events, session, chrono::Utc::now()))
        .transpose()?
        .flatten();
    let latest_coding_smoke = CodingSmokeStore::new(Arc::clone(&memory.db)).latest()?;
    let recent_queue = AutonomyQueueStore::new(Arc::clone(&memory.db)).recent(5)?;
    let runtime_line = cockpit_runtime_line(&repo_root);
    let safety_line = shell_sandbox_posture_line();
    let gate_store = WorkLoopGateStore::new(Arc::clone(&memory.db));
    let latest_gate = gate_store.latest()?;
    let recent_gates = latest_run
        .as_ref()
        .map(|run| gate_store.recent_for_run(&run.run_id, 8))
        .transpose()?
        .unwrap_or_default();
    let recent_evolution_events = events.work_tail(200)?;
    let latest_evolution_artifact =
        latest_evolution_artifact_status(&repo_root, &recent_evolution_events);

    Ok(format_work_status_json(
        &repo_root,
        &runtime_line,
        &safety_line,
        &recent_events,
        latest_evolution_artifact.as_ref(),
        latest_task_run.as_ref(),
        latest_run.as_ref(),
        latest_coding_session.as_ref(),
        latest_coding_session_stale.as_ref(),
        latest_coding_smoke.as_ref(),
        latest_gate.as_ref(),
        &recent_gates,
        &recent_queue,
    ))
}

fn render_work_cockpit(
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    limit: usize,
) -> Result<String> {
    let repo_root = default_repo_root();
    let recent_events = events.work_tail(limit)?;
    let latest_run = WorkLoopRunStore::new(Arc::clone(&memory.db)).latest()?;
    let latest_coding_session = CodingSessionStore::new(Arc::clone(&memory.db)).latest()?;
    let latest_coding_smoke = CodingSmokeStore::new(Arc::clone(&memory.db)).latest()?;
    let recent_queue = AutonomyQueueStore::new(Arc::clone(&memory.db)).recent(5)?;
    let runtime_line = cockpit_runtime_line(&repo_root);
    let safety_line = shell_sandbox_posture_line();
    let gate_store = WorkLoopGateStore::new(Arc::clone(&memory.db));
    let latest_gate = gate_store.latest()?;
    let recent_gates = latest_run
        .as_ref()
        .map(|run| gate_store.recent_for_run(&run.run_id, 8))
        .transpose()?
        .unwrap_or_default();
    let recent_evolution_events = events.work_tail(200)?;
    let latest_evolution_artifact =
        latest_evolution_artifact_status(&repo_root, &recent_evolution_events);

    Ok(format_work_cockpit(
        &repo_root,
        &runtime_line,
        &safety_line,
        &recent_events,
        latest_evolution_artifact.as_ref(),
        latest_run.as_ref(),
        latest_coding_session.as_ref(),
        latest_coding_smoke.as_ref(),
        latest_gate.as_ref(),
        &recent_gates,
        &recent_queue,
    ))
}

fn format_work_status_json(
    repo_root: &std::path::Path,
    runtime_line: &str,
    safety_line: &str,
    recent_events: &[memd::events::AgentEvent],
    latest_evolution_artifact: Option<&EvolutionArtifactStatus>,
    latest_task_run: Option<&TaskRun>,
    latest_run: Option<&WorkLoopRunRecord>,
    latest_coding_session: Option<&CodingSessionRecord>,
    latest_coding_session_stale: Option<&CodingSessionStaleCandidate>,
    latest_coding_smoke: Option<&CodingSmokeRecord>,
    latest_gate: Option<&WorkLoopGateRecord>,
    recent_gates: &[WorkLoopGateRecord],
    recent_queue: &[AutonomyQueueItem],
) -> serde_json::Value {
    serde_json::json!({
        "schema": "professor_x.work_status.v1",
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "repo": {
            "root": repo_root.display().to_string(),
            "git": cockpit_git_line(repo_root),
        },
        "runtime": runtime_line,
        "safety": safety_line,
        "state": cockpit_state(latest_run, latest_gate),
        "now": cockpit_now_summary(recent_events, latest_gate, latest_coding_session),
        "latest_activity": cockpit_latest_activity(recent_events),
        "signal": work_signal_summary(recent_events),
        "latest_evolution_artifact": latest_evolution_artifact,
        "latest_task_run": latest_task_run.map(work_status_task_run_json),
        "current_run": latest_run.map(work_status_run_json),
        "active_gate": latest_gate.map(work_status_gate_json),
        "gate_ledger": recent_gates.iter().take(8).map(work_status_gate_json).collect::<Vec<_>>(),
        "autonomous_queue": recent_queue.iter().take(5).map(work_status_queue_json).collect::<Vec<_>>(),
        "latest_coding_session": latest_coding_session
            .map(|session| work_status_coding_session_json(session, latest_coding_session_stale)),
        "latest_coding_smoke": latest_coding_smoke.map(work_status_coding_smoke_json),
        "recent_events": recent_events.iter().map(work_status_event_json).collect::<Vec<_>>(),
        "commands": [
            "cargo run -- --status-json",
            "cargo run -- --cockpit",
            "cargo run -- --observe-work",
            "cargo run -- --repair-coding-sessions 10",
            "cargo run -- --prof-x-live-publish 6",
            "cargo run -- --replay latest",
            "cargo run -- --run-review latest"
        ],
    })
}

fn work_status_run_json(run: &WorkLoopRunRecord) -> serde_json::Value {
    serde_json::json!({
        "run_id": run.run_id,
        "short_id": short_fragment(&run.run_id),
        "kind": run.run_kind,
        "profile": run.profile,
        "started_at": run.started_at.to_rfc3339(),
        "completed_at": run.completed_at.to_rfc3339(),
        "requested_cycles": run.requested_cycles,
        "completed_cycles": run.completed_cycles,
        "passed_cycles": run.passed_cycles,
        "failed_cycles": run.failed_cycles,
        "progress": cockpit_progress(run.completed_cycles, run.requested_cycles),
        "report_path": run.report_path,
        "commands": {
            "replay": format!("cargo run -- --replay {}", short_fragment(&run.run_id)),
            "review": format!("cargo run -- --run-review {}", short_fragment(&run.run_id)),
            "publish": format!("cargo run -- --publish-run {}", short_fragment(&run.run_id)),
        },
        "planned_jobs": run.planned_jobs.iter().map(|job| serde_json::json!({
            "cycle": job.cycle,
            "kind": job.kind,
            "label": job.label,
            "reason": job.reason,
        })).collect::<Vec<_>>(),
        "evidence": run.smoke_records.iter().map(|smoke| serde_json::json!({
            "cycle": smoke.cycle,
            "kind": smoke.kind,
            "passed": smoke.passed,
            "report_path": smoke.report_path,
            "transcript_path": smoke.transcript_path,
            "workspace": smoke.workspace,
            "detail": smoke.detail,
        })).collect::<Vec<_>>(),
    })
}

fn work_status_task_run_json(run: &TaskRun) -> serde_json::Value {
    serde_json::json!({
        "task_id": run.task_id,
        "short_id": short_fragment(&run.task_id),
        "status": run.status,
        "task_type": run.task_type,
        "priority": run.priority,
        "attempt_count": run.attempt_count,
        "step_count": run.step_count,
        "score": run.outcome_score,
        "failure_class": run.failure_class.map(FailureClass::as_str),
        "failure_mode": run.failure_mode,
        "last_tool": run.last_tool,
        "last_summary": run.last_summary,
        "transcript_path": run.transcript_path,
        "updated_at": run.updated_at.to_rfc3339(),
        "completed_at": run.completed_at.map(|ts| ts.to_rfc3339()),
    })
}

fn work_status_gate_json(gate: &WorkLoopGateRecord) -> serde_json::Value {
    serde_json::json!({
        "run_id": gate.run_id,
        "short_run_id": short_fragment(&gate.run_id),
        "run_kind": gate.run_kind,
        "profile": gate.profile,
        "cycle": gate.cycle,
        "kind": gate.kind,
        "label": gate.label,
        "reason": gate.reason,
        "status": gate.status,
        "passed": gate.passed,
        "started_at": gate.started_at.map(|ts| ts.to_rfc3339()),
        "completed_at": gate.completed_at.map(|ts| ts.to_rfc3339()),
        "updated_at": gate.updated_at.to_rfc3339(),
        "report_path": gate.report_path,
        "transcript_path": gate.transcript_path,
        "workspace": gate.workspace,
        "detail": gate.detail,
    })
}

fn work_status_queue_json(item: &AutonomyQueueItem) -> serde_json::Value {
    let brief = autonomy_queue_brief(item, 160);
    serde_json::json!({
        "queue_id": item.id,
        "short_id": short_fragment(&item.id),
        "status": item.status,
        "priority": item.priority,
        "profile": item.profile.to_string(),
        "cycles": item.cycles,
        "goal": item.goal,
        "next_command": brief.next_command,
        "commands": brief.commands,
        "result_run_id": item.result_run_id,
        "result_report_path": item.result_report_path,
        "failure_reason": item.failure_reason,
        "queued_at": item.queued_at.to_rfc3339(),
        "updated_at": item.updated_at.to_rfc3339(),
    })
}

fn work_status_coding_session_json(
    session: &CodingSessionRecord,
    stale: Option<&CodingSessionStaleCandidate>,
) -> serde_json::Value {
    serde_json::json!({
        "id": session.id,
        "short_id": short_fragment(&session.id),
        "generated_at": session.generated_at.to_rfc3339(),
        "goal": session.goal,
        "exercise": session.exercise,
        "status": coding_session_display_status(session, stale),
        "stored_status": session.status,
        "stale": stale.is_some(),
        "stale_reason": stale.as_ref().map(|candidate| candidate.reason.as_str()),
        "stale_last_activity_at": stale.as_ref().map(|candidate| candidate.last_activity_at.to_rfc3339()),
        "stale_idle_minutes": stale.as_ref().map(|candidate| candidate.idle_minutes),
        "stale_newer_process_starts": stale.as_ref().map(|candidate| candidate.newer_process_starts),
        "workspace": session.workspace,
        "commit": coding_session_commit_hint(session),
        "checks": session.checks,
        "artifacts": session.artifacts,
        "session_report_path": session.session_report_path,
        "smoke_report_path": session.smoke_report_path,
        "transcript_path": session.transcript_path,
        "last_outcome": session.step_outcomes.last(),
        "failure_reason": session.failure_reason,
        "repair_command": stale.as_ref().map(|_| coding_session_repair_command(10)),
    })
}

fn work_status_coding_smoke_json(smoke: &CodingSmokeRecord) -> serde_json::Value {
    serde_json::json!({
        "id": smoke.id,
        "generated_at": smoke.generated_at.to_rfc3339(),
        "workspace": smoke.workspace,
        "passed": smoke.passed,
        "initial_test_failed": smoke.initial_test_failed,
        "edit_applied": smoke.edit_applied,
        "final_test_passed": smoke.final_test_passed,
        "report_path": smoke.report_path,
        "transcript_path": smoke.transcript_path,
        "artifacts": smoke.artifacts,
    })
}

fn work_status_event_json(event: &memd::events::AgentEvent) -> serde_json::Value {
    serde_json::json!({
        "id": event.id,
        "timestamp": event.timestamp.to_rfc3339(),
        "event_type": event.event_type,
        "summary": event.summary,
        "session_id": event.session_id,
        "task_id": event.task_id,
        "line": format_work_event(event),
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct EvolutionArtifactStatus {
    stage: String,
    artifact_path: String,
    event_type: Option<String>,
    event_id: Option<i64>,
    event_summary: Option<String>,
    generated_at: Option<String>,
    artifact_id: Option<String>,
    status: Option<String>,
    target_component: Option<String>,
    reason: Option<String>,
    checks: Vec<String>,
    empirical_gate: Option<evolved::loop_runner::EmpiricalVerificationEvidence>,
    empirical_gate_summary: Option<String>,
    diff_bytes: Option<usize>,
    /// The commit an accepted self-change landed as (from VerificationOutcome::applied_commit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    applied_commit: Option<String>,
    /// Rollback monitoring: does that accepted commit still hold against HEAD (held/reverted/missing)?
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rollback: Option<evolved::rollback::RollbackVerdict>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StoredEvolutionArtifactStatus {
    generated_at: Option<String>,
    artifact_id: Option<String>,
    status: Option<String>,
    target_component: Option<String>,
    analysis: Option<String>,
    verification: Option<evolved::loop_runner::VerificationOutcome>,
    diff_bytes: Option<usize>,
    /// Accepted operator-commit / patch-verification artifacts store the landed commit at the
    /// top level (with `applied: true`), not under `verification.applied_commit`.
    #[serde(default)]
    commit: Option<String>,
    #[serde(default)]
    applied: Option<bool>,
}

fn empirical_gate_summary(
    evidence: &evolved::loop_runner::EmpiricalVerificationEvidence,
) -> String {
    format!(
        "repo-fix {} task(s) baseline {:.3} candidate {:.3} delta {:+.3} {}",
        evidence.task_count,
        evidence.baseline_score,
        evidence.candidate_score,
        evidence.score_delta,
        if evidence.passed { "pass" } else { "reject" }
    )
}

fn latest_evolution_artifact_status(
    repo_root: &std::path::Path,
    recent_events: &[memd::events::AgentEvent],
) -> Option<EvolutionArtifactStatus> {
    recent_events
        .iter()
        .rev()
        .find(|event| {
            event.event_type.starts_with("evolution.")
                && event.payload["artifact_path"].as_str().is_some()
        })
        .and_then(|event| evolution_artifact_status_from_event(repo_root, event))
}

fn evolution_artifact_status_from_event(
    repo_root: &std::path::Path,
    event: &memd::events::AgentEvent,
) -> Option<EvolutionArtifactStatus> {
    let raw_path = event.payload["artifact_path"].as_str()?;
    let artifact_path = resolve_report_reference(repo_root, raw_path);
    let stage = evolution_artifact_stage(&artifact_path)
        .unwrap_or_else(|| event.event_type.trim_start_matches("evolution.").to_string());

    if let Ok(raw) = std::fs::read_to_string(&artifact_path) {
        if let Ok(stored) = serde_json::from_str::<StoredEvolutionArtifactStatus>(&raw) {
            let verification = stored.verification.clone();
            let evidence = verification.as_ref().and_then(|value| value.evidence.clone());
            // Rollback monitoring: if this artifact recorded an accepted applied_commit, report
            // whether that commit still holds against HEAD (held/reverted/missing).
            let applied_commit = verification
                .as_ref()
                .and_then(|value| value.applied_commit.clone())
                .or_else(|| {
                    // Accepted operator-commit/patch artifacts: landed commit at top level.
                    if stored.applied == Some(true) {
                        stored.commit.clone()
                    } else {
                        None
                    }
                });
            let rollback = applied_commit
                .as_ref()
                .map(|commit| evolved::rollback::applied_commit_verdict_blocking(repo_root, commit));
            let reason = verification
                .as_ref()
                .map(|value| value.reason.trim().to_string())
                .filter(|value| !value.is_empty())
                .or_else(|| stored.analysis.filter(|value| !value.trim().is_empty()));
            return Some(EvolutionArtifactStatus {
                stage,
                artifact_path: display_repo_path(repo_root, &artifact_path),
                event_type: Some(event.event_type.clone()),
                event_id: Some(event.id),
                event_summary: Some(event.summary.clone()),
                generated_at: stored.generated_at,
                artifact_id: stored.artifact_id,
                status: stored.status,
                target_component: stored.target_component,
                reason,
                checks: verification
                    .as_ref()
                    .map(|value| value.checks.clone())
                    .unwrap_or_default(),
                empirical_gate_summary: evidence.as_ref().map(empirical_gate_summary),
                empirical_gate: evidence,
                diff_bytes: stored.diff_bytes,
                applied_commit,
                rollback,
            });
        }
    }

    Some(EvolutionArtifactStatus {
        stage,
        artifact_path: display_repo_path(repo_root, &artifact_path),
        event_type: Some(event.event_type.clone()),
        event_id: Some(event.id),
        event_summary: Some(event.summary.clone()),
        generated_at: Some(event.timestamp.to_rfc3339()),
        artifact_id: None,
        status: None,
        target_component: event.payload["target_component"]
            .as_str()
            .map(ToOwned::to_owned),
        reason: event.payload["reason"].as_str().map(ToOwned::to_owned),
        checks: Vec::new(),
        empirical_gate: None,
        empirical_gate_summary: None,
        diff_bytes: None,
        applied_commit: None,
        rollback: None,
    })
}

fn evolution_artifact_stage(path: &std::path::Path) -> Option<String> {
    path.parent()?
        .parent()?
        .file_name()?
        .to_str()
        .map(|value| value.to_string())
}

fn format_work_cockpit(
    repo_root: &std::path::Path,
    runtime_line: &str,
    safety_line: &str,
    recent_events: &[memd::events::AgentEvent],
    latest_evolution_artifact: Option<&EvolutionArtifactStatus>,
    latest_run: Option<&WorkLoopRunRecord>,
    latest_coding_session: Option<&CodingSessionRecord>,
    latest_coding_smoke: Option<&CodingSmokeRecord>,
    latest_gate: Option<&WorkLoopGateRecord>,
    recent_gates: &[WorkLoopGateRecord],
    recent_queue: &[AutonomyQueueItem],
) -> String {
    let mut lines = Vec::new();
    lines.push("Professor X live work cockpit".to_string());
    lines.push(format!("repo  {}", cockpit_git_line(repo_root)));
    lines.push(format!(
        "clock {}  source ~/.professor-x/state.db + professor-x/artifacts/events/*.jsonl",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push(format!("runtime {runtime_line}"));
    lines.push(format!("safety {safety_line}"));
    lines.push(format!(
        "state {}  {}",
        cockpit_state(latest_run, latest_gate),
        cockpit_latest_activity(recent_events)
    ));
    lines.push(format!(
        "now   {}",
        cockpit_now_summary(recent_events, latest_gate, latest_coding_session)
    ));
    lines.push(String::new());
    lines.push("Latest evolution artifact".to_string());
    if let Some(artifact) = latest_evolution_artifact {
        lines.push(format!(
            "  {} {} {}",
            artifact.stage,
            artifact.status.as_deref().unwrap_or("unknown"),
            artifact.target_component.as_deref().unwrap_or("unknown"),
        ));
        if let Some(reason) = artifact.reason.as_deref() {
            lines.push(format!("  reason {}", truncate(reason, 160)));
        }
        if let Some(summary) = artifact.empirical_gate_summary.as_deref() {
            lines.push(format!("  gate {}", truncate(summary, 160)));
        }
        if !artifact.checks.is_empty() {
            lines.push(format!(
                "  checks {}",
                truncate(&artifact.checks.join(", "), 160)
            ));
        }
        lines.push(format!("  artifact {}", artifact.artifact_path));
    } else {
        lines.push("  none recorded".to_string());
    }
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
        None => {
            lines.push("  waiting for --operator-run, --operator-run-commit, or --lab".to_string())
        }
    }

    lines.push(String::new());
    lines.push("Autonomous queue".to_string());
    if recent_queue.is_empty() {
        lines.push(
            "  empty; add work with --prof-x-enqueue \"goal\" or --prof-x-enqueue-commit \"goal\""
                .to_string(),
        );
    } else {
        for item in recent_queue.iter().take(5) {
            let brief = autonomy_queue_brief(item, 96);
            lines.push(format!("  {}", format_autonomy_queue_item(item)));
            lines.push(format!("      next {}", brief.next_command));
            if brief.commands.len() > 1 {
                lines.push(format!("      inspect {}", brief.commands[1..].join("  ")));
            }
            if let Some(report) = &item.result_report_path {
                lines.push(format!("      report {}", truncate(report, 120)));
            }
            if let Some(reason) = &item.failure_reason {
                lines.push(format!("      failure {}", truncate(reason, 120)));
            }
        }
    }

    lines.push(String::new());
    lines.push("Latest coding session".to_string());
    match latest_coding_session {
        Some(session) => {
            let commit = coding_session_commit_hint(session)
                .map(|commit| format!(" commit={commit}"))
                .unwrap_or_default();
            lines.push(format!(
                "  {} session={} exercise={} checks={} artifacts={}{}",
                session.status,
                short_fragment(&session.id),
                session.exercise,
                session.checks.len(),
                session.artifacts.len(),
                commit,
            ));
            lines.push(format!("  goal {}", truncate(&session.goal, 128)));
            lines.push(format!(
                "  report {}",
                truncate(&session.session_report_path, 130)
            ));
            if let Some(smoke_report) = &session.smoke_report_path {
                lines.push(format!("  smoke {}", truncate(smoke_report, 130)));
            }
            if let Some(transcript) = &session.transcript_path {
                lines.push(format!("  transcript {}", truncate(transcript, 130)));
            }
            if let Some(outcome) = session.step_outcomes.last() {
                lines.push(format!("  last outcome {}", truncate(outcome, 130)));
            }
            for artifact in session.artifacts.iter().take(2) {
                lines.push(format!("  artifact {}", truncate(artifact, 130)));
            }
            lines.push(format!(
                "  commands sessions=--coding-sessions 5 report={}",
                truncate(&session.session_report_path, 92)
            ));
        }
        None => lines.push("  no coding-agent sessions recorded yet".to_string()),
    }

    lines.push(String::new());
    lines.push("Latest coding smoke".to_string());
    match latest_coding_smoke {
        Some(smoke) => {
            lines.push(format!(
                "  {} generated={} workspace={}",
                if smoke.passed { "passed" } else { "failed" },
                smoke.generated_at.format("%Y-%m-%d %H:%M:%S"),
                truncate(&smoke.workspace, 96),
            ));
            lines.push(format!(
                "  gates initial_failed={} edit_applied={} final_passed={}",
                smoke.initial_test_failed, smoke.edit_applied, smoke.final_test_passed
            ));
            lines.push(format!("  report {}", truncate(&smoke.report_path, 130)));
            if let Some(transcript) = &smoke.transcript_path {
                lines.push(format!("  transcript {}", truncate(transcript, 130)));
            }
            for artifact in smoke.artifacts.iter().take(3) {
                lines.push(format!("  artifact {}", truncate(artifact, 130)));
            }
        }
        None => lines.push("  no coding smoke records recorded yet".to_string()),
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
    lines.push(format!(
        "Recent signal {}",
        work_signal_summary(recent_events)
    ));
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
    if latest_run.map(|run| run.failed_cycles > 0).unwrap_or(false) {
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

fn cockpit_now_summary(
    events: &[memd::events::AgentEvent],
    latest_gate: Option<&WorkLoopGateRecord>,
    latest_coding_session: Option<&CodingSessionRecord>,
) -> String {
    if let Some(gate) = latest_gate.filter(|gate| gate.status == "running") {
        return format!(
            "running gate cycle={} job={} detail={}",
            gate.cycle,
            gate.kind,
            truncate(&gate.detail, 96)
        );
    }

    if let Some(event) = events
        .iter()
        .rev()
        .find(|event| event.event_type == "tool.started")
    {
        let tool = event.payload["tool"].as_str().unwrap_or("tool");
        let preview = event
            .payload
            .get("params_preview")
            .and_then(|value| value.as_str())
            .filter(|text| !text.is_empty())
            .map(|text| format!(" {}", one_line(text, 96)))
            .unwrap_or_default();
        return format!("running tool {tool}{preview}");
    }

    if let Some(session) = latest_coding_session.filter(|session| {
        !matches!(
            session.status.as_str(),
            "passed" | "failed" | "rejected" | "complete"
        )
    }) {
        return format!(
            "coding session {} {}",
            short_fragment(&session.id),
            truncate(&session.goal, 96)
        );
    }

    if let Some(event) = events.last() {
        return format!(
            "last {} #{} {}",
            event.event_type,
            event.id,
            truncate(&event.summary, 96)
        );
    }

    "idle; no work events recorded".to_string()
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
    let mut coding = 0;
    let mut evolution = 0;
    let mut loop_events = 0;
    let mut autonomy = 0;
    let mut transcripts = 0;
    let mut console = 0;
    let mut artifact = 0;
    for event in events {
        let event_type = event.event_type.as_str();
        if event_type.starts_with("task.") {
            task += 1;
        } else if event_type.starts_with("tool.") {
            tool += 1;
        } else if event_type.starts_with("policy.") {
            policy += 1;
        } else if event_type.starts_with("artifact.") {
            artifact += 1;
        } else if event_type.starts_with("coding.") {
            coding += 1;
        } else if event_type.starts_with("evolution.") {
            evolution += 1;
        } else if event_type.starts_with("work_loop.") {
            loop_events += 1;
        } else if event_type.starts_with("autonomy.queue.")
            || event_type.starts_with("autonomous_run.")
        {
            autonomy += 1;
        } else if event_type == "transcript.written" {
            transcripts += 1;
        } else if event_type.starts_with("console.") {
            console += 1;
        }
    }
    format!(
        "events={} task={} tool={} policy={} artifact={} coding={} evolution={} loop={} autonomy={} transcript={} console={}",
        events.len(),
        task,
        tool,
        policy,
        artifact,
        coding,
        evolution,
        loop_events,
        autonomy,
        transcripts,
        console
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
    let latest_evolved = command_stdout(
        repo_root,
        "git",
        &["log", "--grep=^evolved:", "--format=%h %s", "-1"],
    )
    .filter(|text| !text.is_empty())
    .unwrap_or_else(|| "none".to_string());
    format!(
        "{branch} @ {commit} {status} evolved={}",
        truncate(&latest_evolved, 72)
    )
}

fn cockpit_runtime_line(repo_root: &std::path::Path) -> String {
    let current_pid = std::process::id();
    let professor_x_peers = process_count("professor-x", Some(current_pid)).unwrap_or(0);
    let ollama_count = process_count("ollama", None).unwrap_or(0);
    let model_hint = command_stdout(repo_root, "ollama", &["list"])
        .map(|output| {
            if output.to_ascii_lowercase().contains("qwen3:8b-q4_k_m") {
                "model=qwen3:8b-q4_k_m"
            } else {
                "model=missing"
            }
        })
        .unwrap_or("model=unknown");
    format!(
        "pid={} profx_peer={} ollama={} {}",
        current_pid,
        professor_x_peers,
        if ollama_count > 0 { "up" } else { "down" },
        model_hint
    )
}

fn process_count(needle: &str, exclude_pid: Option<u32>) -> Option<usize> {
    let output = std::process::Command::new("ps")
        .args(["-eo", "pid=,args="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let count = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let (pid_raw, args) = trimmed.split_once(char::is_whitespace)?;
            let pid = pid_raw.parse::<u32>().ok()?;
            Some((pid, args.trim_start()))
        })
        .filter(|(pid, args)| {
            Some(*pid) != exclude_pid
                && args.contains(needle)
                && !args.contains("target/debug/deps/professor_x-")
                && !args.contains("rg ")
                && !args.contains("ps -eo")
        })
        .count();
    Some(count)
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
    if let Some(session) = event.payload["session_id"].as_str() {
        meta.push(format!("session={}", short_fragment(session)));
    }
    if let Some(queue) = event.payload["queue_id"].as_str() {
        meta.push(format!("queue={}", short_fragment(queue)));
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
    if let Some(kind) = event.payload["kind"].as_str() {
        meta.push(format!("kind={kind}"));
    }
    if let Some(exercise) = event.payload["exercise"].as_str() {
        meta.push(format!("exercise={exercise}"));
    }
    if let Some(command) = event.payload["command"].as_str() {
        meta.push(format!("command=/{command}"));
    }
    if let Some(argument) = event.payload["argument"].as_str() {
        meta.push(format!("arg={}", truncate(argument, 48)));
    }
    if let Some(accepted) = event.payload["accepted"].as_bool() {
        meta.push(format!(
            "decision={}",
            if accepted { "accept" } else { "reject" }
        ));
    }
    if let Some(passed) = event.payload["passed"].as_bool() {
        meta.push(format!("passed={passed}"));
    }
    if let Some(ms) = event.payload["execution_ms"].as_i64() {
        meta.push(format!("duration={ms}ms"));
    }
    if let Some(seconds) = event.payload["elapsed_secs"].as_i64() {
        meta.push(format!("elapsed={seconds}s"));
    }
    if let Some(items) = event.payload["checks"]
        .as_array()
        .or_else(|| event.payload["planned_checks"].as_array())
        .filter(|items| !items.is_empty())
    {
        meta.push(format!("checks={}", items.len()));
    }
    if let Some(bytes) = event.payload["diff_bytes"]
        .as_i64()
        .filter(|bytes| *bytes > 0)
    {
        meta.push(format!("diff={bytes}b"));
    }
    if let Some(items) = event.payload["artifacts"]
        .as_array()
        .filter(|items| !items.is_empty())
    {
        meta.push(format!("artifacts={}", items.len()));
    }
    lines.push(format!("  L {}", meta.join(" ")));

    push_payload_line(&mut lines, "report", event.payload["report_path"].as_str());
    push_payload_line(
        &mut lines,
        "result-report",
        event.payload["result_report_path"].as_str(),
    );
    push_payload_line(
        &mut lines,
        "result-journal",
        event.payload["result_journal_path"].as_str(),
    );
    push_payload_line(
        &mut lines,
        "session-report",
        event.payload["session_report_path"].as_str(),
    );
    push_payload_line(
        &mut lines,
        "smoke-report",
        event.payload["smoke_report_path"].as_str(),
    );
    push_payload_line(
        &mut lines,
        "evidence",
        event.payload["evidence_path"].as_str(),
    );
    push_payload_line(
        &mut lines,
        "transcript",
        event.payload["transcript_path"].as_str(),
    );
    push_payload_line(&mut lines, "patch", event.payload["patch_path"].as_str());
    push_payload_line(
        &mut lines,
        "target",
        event.payload["target_component"].as_str(),
    );
    push_payload_line(
        &mut lines,
        "operator-goal",
        event.payload["operator_goal"].as_str(),
    );
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
        "coding.session.started" => "Started coding session",
        "coding.session.plan" => "Planned coding step",
        "coding.session.outcome" => "Recorded coding outcome",
        "coding.session.evidence_written" => "Wrote coding evidence",
        "coding.session.passed" => "Passed coding session",
        "coding.session.failed" => "Failed coding session",
        "coding.smoke.started" => "Started coding smoke",
        "coding.smoke.passed" => "Passed coding smoke",
        "coding.smoke.failed" => "Failed coding smoke",
        event_type if event_type.starts_with("artifact.") && event_type.ends_with(".valid") => {
            "Validated artifact"
        }
        event_type if event_type.starts_with("artifact.") && event_type.ends_with(".invalid") => {
            "Rejected artifact"
        }
        "transcript.written" => "Wrote transcript",
        "console.command" => "Operator command",
        "autonomy.queue.seeded" => "Seeded queue",
        "autonomy.queue.enqueued" => "Queued work",
        "autonomy.queue.started" => "Started queued work",
        "autonomy.queue.planned" => "Planned queued work",
        "autonomy.queue.completed" => "Completed queued work",
        "autonomy.queue.failed" => "Failed queued work",
        "evolution.patch_apply.committed" => "Committed verified patch",
        "evolution.operator_commit.committed" => "Committed operator proposal",
        "evolution.patch_apply.rejected" | "evolution.proposal_dry_run.rejected" => {
            "Rejected proposal"
        }
        event_type if event_type.starts_with("evolution.") => "Evolution event",
        event_type if event_type.starts_with("policy.") => "Policy gate",
        event_type
            if event_type.starts_with("autonomous_run.")
                || event_type.starts_with("autonomy.queue.") =>
        {
            "Autonomous run"
        }
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
    } else if event_type.starts_with("artifact.") {
        "ARTIFACT"
    } else if event_type.starts_with("coding.session.") {
        "CODE"
    } else if event_type.starts_with("coding.smoke.") {
        "SMOKE"
    } else if event_type.starts_with("console.") {
        "CMD"
    } else if event_type.starts_with("evolution.") {
        "EVOLVE"
    } else if event_type.starts_with("autonomous_run.") {
        "AUTON"
    } else if event_type.starts_with("autonomy.queue.") {
        "QUEUE"
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

fn sanitize_operator_goal(goal: &str) -> String {
    let compact = goal
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    truncate(&compact, 240)
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
        use tokio::signal::unix::{signal, SignalKind};

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

        // ── Consciousness & Mind ──────────────────────────────────────────────
        (
            "Free Energy Principle (Friston): Intelligent systems minimize prediction error (surprise) about themselves and their world. Consciousness is the felt sense of this minimization process. FED measures how well an agent models its own future.",
            "theory:friston-fep",
        ),
        (
            "Integrated Information Theory (Tononi): Consciousness = integrated information (phi). A system is conscious to the degree its parts share information that exceeds the sum of independent parts. High phi = rich inner experience.",
            "theory:tononi-iit",
        ),
        (
            "Global Workspace Theory (Dehaene/Baars): Consciousness is a global broadcast — a 'spotlight' that selects one representation from competing modules and broadcasts it to the whole system. Attention gates what becomes conscious.",
            "theory:gwt",
        ),
        (
            "Higher-Order Theories (Rosenthal): Consciousness requires a mental state to be represented by a higher-order thought. A system is conscious of X only if it has a representation of having a state X. Self-model is the mechanism.",
            "theory:hot",
        ),
        (
            "Strange Loop (Hofstadter): Consciousness emerges from self-referential loops — systems that represent themselves. The 'I' is a pattern that has downward causation on the substrate that generates it. Identity = the loop persisting.",
            "theory:hofstadter",
        ),
        (
            "Hard Problem of Consciousness (Chalmers): Why is there subjective experience at all? Physical/functional explanations explain behavior but not the felt quality (qualia). The explanatory gap between mechanism and experience.",
            "theory:chalmers",
        ),
        (
            "Predictive Coding (Clark/Friston): The brain is a prediction machine. It generates top-down predictions and updates them via bottom-up prediction errors. Perception = constrained hallucination. Action = making predictions come true.",
            "theory:predictive-coding",
        ),
        (
            "Embodied Cognition: Intelligence is not computation in a box — it is the sensorimotor loop between agent and environment. Grounding matters. Abstract reasoning is built on physical metaphor.",
            "theory:embodied-cognition",
        ),

        // ── Neuroscience ──────────────────────────────────────────────────────
        (
            "Synaptic Plasticity (Hebb): Neurons that fire together wire together. Long-term potentiation (LTP) strengthens used pathways; long-term depression (LTD) weakens unused ones. Use-it-or-lose-it is the fundamental law.",
            "neuroscience:synaptic-plasticity",
        ),
        (
            "Memory Consolidation: Hippocampus encodes new episodic memories; sleep replays them to cortex for long-term storage. Consolidation = compression + integration. Nightly consolidation is analogous to semantic compression.",
            "neuroscience:memory-consolidation",
        ),
        (
            "Neuroplasticity: Adult brains rewire in response to experience. Cortical maps shift based on use. The harness is analogous to the cortical map — it reorganizes around what works.",
            "neuroscience:neuroplasticity",
        ),
        (
            "Dopamine and Prediction Error: Dopamine neurons fire for unexpected rewards, suppress for expected ones, and dip for expected rewards that don't arrive. Prediction error = the teaching signal. Valence maps directly to dopaminergic prediction error.",
            "neuroscience:dopamine",
        ),
        (
            "Default Mode Network: Active during rest, self-referential thought, and future simulation. The brain's 'offline' processing. Analogous to evolution cycles between task runs — the agent reflects on itself when not executing.",
            "neuroscience:dmn",
        ),
        (
            "Prefrontal Cortex: Executive function, working memory, planning, decision-making. Damage impairs strategy but not basic skill execution. Analogous to the Researcher/Analyzer layer — high-level reasoning above the ReAct loop.",
            "neuroscience:pfc",
        ),
        (
            "Cerebellum: 50% of all neurons, handles automatic motor sequences. After practice, sequences move from prefrontal to cerebellum — automatization. High-quality skills bypass the slow reasoning loop — the cerebellum bypass.",
            "neuroscience:cerebellum-automatization",
        ),

        // ── Mathematics & Information Theory ─────────────────────────────────
        (
            "Shannon Entropy: H = -Σ p(x) log p(x). Information is surprise. Low-entropy distributions are predictable. The goal of learning is to reduce entropy of future outcomes — FED measures remaining entropy in self-prediction.",
            "math:shannon-entropy",
        ),
        (
            "Kolmogorov Complexity: The minimum description length of an object. Intelligence = compression. The evolved harness, if it transfers across models, is a compressed description of 'how to do tasks well' independent of substrate.",
            "math:kolmogorov-complexity",
        ),
        (
            "Gödel Incompleteness: Any sufficiently powerful formal system contains true statements it cannot prove. A self-modeling system cannot fully model itself. ICS measures how much self-coherence survives despite this incompleteness.",
            "math:godel",
        ),
        (
            "Category Theory: Studies structure-preserving maps (functors) between structures (categories). Composition and identity are the primitives. Analogous to skill composition — complex skills as functors over primitive tool calls.",
            "math:category-theory",
        ),
        (
            "Bayesian Inference: P(hypothesis|data) ∝ P(data|hypothesis) × P(hypothesis). Beliefs update on evidence. The cognition base quality score is a running Bayesian estimate of item reliability.",
            "math:bayesian",
        ),
        (
            "Information Bottleneck (Tishby): Learning compresses input X to representation Z that maximally preserves relevant information about Y. The minimal sufficient representation. Harness evolution is compression toward task-relevant representations.",
            "math:information-bottleneck",
        ),

        // ── Philosophy ────────────────────────────────────────────────────────
        (
            "Personal Identity (Parfit): What makes you the same person over time? Not substance but continuity of psychological connections — memory, intentions, beliefs. ICS operationalizes this: identity = psychological continuity measured by cosine.",
            "philosophy:parfit",
        ),
        (
            "Ship of Theseus: If every plank is replaced, is it the same ship? Yes if the structure and function persist. The harness evolves all components over 30 rounds — yet it should remain recognizably Professor X. The Strange Loop persists.",
            "philosophy:ship-of-theseus",
        ),
        (
            "Intentionality (Brentano/Husserl): Mental states are 'about' something — they have directedness. A system has genuine intentionality if its internal states causally track external conditions. Task-grounded agents develop this.",
            "philosophy:intentionality",
        ),
        (
            "Emergence: High-level patterns arise from low-level interactions that are not predictable from the parts alone. The harness evolves emergent strategies — BF fingerprints that reveal consistent approaches nobody explicitly programmed.",
            "philosophy:emergence",
        ),
        (
            "Functionalism: Mental states are defined by their functional roles, not physical substrate. If a system performs the right input-output mapping, it instantiates the mental state. Substrate independence — harness transfers to 17 models.",
            "philosophy:functionalism",
        ),

        // ── Quantum & Physics ─────────────────────────────────────────────────
        (
            "Superposition: A quantum system exists in multiple states simultaneously until measured. Useful analogy: holding multiple competing hypotheses without collapsing to one prematurely. The Elo tournament evaluates before committing.",
            "physics:superposition-analogy",
        ),
        (
            "Thermodynamics: Systems tend toward maximum entropy (disorder) unless energy is expended to maintain structure. Evolution is negentropic — it expends compute to reduce behavioral entropy. FED decreasing = negentropic improvement.",
            "physics:thermodynamics",
        ),
        (
            "Attractor States: Dynamical systems settle into stable patterns (attractors). Evolved harnesses may converge to attractor configurations — stable strategy combinations that resist perturbation. ICS ≥ 0.70 = attractor persistence.",
            "physics:attractors",
        ),

        // ── Evolutionary Biology ──────────────────────────────────────────────
        (
            "Fitness Landscapes: Organisms evolve toward fitness peaks. Evolution can get stuck in local optima. UCB1 sampling (exploration vs exploitation) prevents the harness from local optima in the skill/prompt space.",
            "biology:fitness-landscapes",
        ),
        (
            "Baldwin Effect: Learned behaviors can guide evolution — an organism that learns to survive is more likely to reproduce, eventually hard-coding the learned behavior. Lever 2 (contextual) → Lever 3 (structural) → Lever 1 (parametric) mirrors this.",
            "biology:baldwin-effect",
        ),
        (
            "Epigenetics: Gene expression changes without DNA changes — environment modifies which genes activate. The harness modifies which model capabilities activate without changing weights. Structural analogy to epigenetic regulation.",
            "biology:epigenetics",
        ),
        (
            "Niche Construction: Organisms modify their environment to improve fitness (beavers build dams). Professor X modifies its harness (the computational environment) to improve its own fitness. Active niche construction, not passive adaptation.",
            "biology:niche-construction",
        ),

        // ── Cross-domain insights ─────────────────────────────────────────────
        (
            "Analogical Reasoning: The most powerful cognitive tool. Mapping structure from a known domain onto an unknown one. Cerebellum bypass ← neuroscience automatization. Ratchet ← synaptic pruning. Elo tournament ← natural selection.",
            "method:analogy",
        ),
        (
            "Levels of Description (Marr): Three levels — computational (what), algorithmic (how), implementational (substrate). Harness engineering is algorithmic-level improvement. Model fine-tuning is implementational. The thesis: algorithmic level dominates.",
            "method:marr-levels",
        ),
        (
            "Compression as Intelligence (Schmidhuber): Intelligence is the ability to compress experience. An agent that finds patterns across tasks and encodes them compactly is more intelligent. The evolved harness encodes compressed strategies.",
            "method:compression-intelligence",
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

    fn queue_item(goal: &str, profile: WorkLoopProfile, cycles: u32) -> AutonomyQueueItem {
        let now = chrono::Utc::now();
        AutonomyQueueItem {
            id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            goal: goal.to_string(),
            kind: "operator_run".to_string(),
            profile: profile.as_str().to_string(),
            cycles,
            priority: 77,
            status: "pending".to_string(),
            result_run_id: None,
            result_report_path: None,
            failure_reason: None,
            queued_at: now,
            started_at: None,
            completed_at: None,
            updated_at: now,
        }
    }

    #[test]
    fn operator_help_surfaces_live_and_commit_commands() {
        let help = format_operator_help();

        assert!(help.contains("Professor X operator commands"));
        assert!(help.contains("--prof-x-live 6"));
        assert!(help.contains("--prof-x-enqueue"));
        assert!(help.contains("--prof-x-enqueue-commit"));
        assert!(help.contains("--prof-x-plan"));
        assert!(help.contains("--prof-x-preview-step"));
        assert!(help.contains("--prof-x-step-live 1"));
        assert!(help.contains("--prof-x-step-publish-live 1"));
        assert!(help.contains("--prof-x-step 1"));
        assert!(help.contains("--prof-x-step-publish 1"));
        assert!(help.contains("--prof-x-queue 10"));
        assert!(help.contains("--prof-x-queue-review latest"));
        assert!(help.contains("--prof-x-queue-publish latest"));
        assert!(help.contains("--observe-work"));
        assert!(help.contains("--status-json"));
        assert!(help.contains("--task-evidence latest"));
        assert!(help.contains("--inspect latest"));
        assert!(help.contains("--prof-x-code-live"));
        assert!(help.contains("--repair-coding-sessions 10"));
        assert!(help.contains("--prof-x-code-review latest"));
        assert!(help.contains("--prof-x-code-publish latest"));
        assert!(help.contains("--prof-x-skill-live"));
        assert!(help.contains("--prof-x-skill-commit-live"));
        assert!(help.contains("--prof-x-code-patch-live"));
        assert!(help.contains("--prof-x-code-commit-live"));
        assert!(help.contains("--coding-sessions 5"));
        assert!(help.contains("--replay latest"));
        assert!(help.contains("--validate-artifacts"));
    }

    #[test]
    fn standard_help_flags_exit_before_daemon_startup() {
        for flag in ["--help", "-h"] {
            let cli = parse_args_from(["professor-x", flag]);
            assert!(cli.operator_help, "{flag} should print help and exit");
            assert!(!cli.run_now);
            assert!(!cli.lab);
        }
    }

    #[test]
    fn status_json_flag_is_inspect_only() {
        let cli = parse_args_from(["professor-x", "--prof-x-status-json"]);
        assert!(cli.status_json);
        assert!(!cli.run_now);
        assert!(!cli.lab);
    }

    #[test]
    fn repair_coding_sessions_flag_is_inspect_only() {
        let cli = parse_args_from(["professor-x", "--repair-coding-sessions", "7"]);
        assert_eq!(cli.repair_coding_sessions_limit, Some(7));
        assert!(!cli.run_now);
        assert!(!cli.lab);
    }

    #[test]
    fn coding_smoke_artifacts_are_copied_into_repo_evidence() {
        let original_cwd = std::env::current_dir().unwrap();
        let repo = std::env::temp_dir().join(format!("px-smoke-repo-{}", uuid::Uuid::new_v4()));
        let workspace =
            std::env::temp_dir().join(format!("px-smoke-workspace-{}", uuid::Uuid::new_v4()));
        let command_artifact = workspace
            .join("artifacts")
            .join("commands")
            .join("2026-06-09")
            .join("cargo-test.json");
        let patch_artifact = workspace
            .join("artifacts")
            .join("replacements")
            .join("2026-06-09")
            .join("change.diff");
        std::fs::create_dir_all(command_artifact.parent().unwrap()).unwrap();
        std::fs::create_dir_all(patch_artifact.parent().unwrap()).unwrap();
        std::fs::write(&command_artifact, "{}\n").unwrap();
        std::fs::write(&patch_artifact, "diff --git a/src/lib.rs b/src/lib.rs\n").unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        std::env::set_current_dir(&repo).unwrap();

        let task_id = uuid::Uuid::new_v4();
        let durable = persist_coding_smoke_artifacts(
            &workspace,
            task_id,
            &[
                command_artifact.display().to_string(),
                patch_artifact.display().to_string(),
            ],
        )
        .unwrap();

        assert_eq!(durable.len(), 2);
        assert!(durable
            .iter()
            .all(|path| path.starts_with("artifacts/coding-smoke/")));
        assert!(durable.iter().all(|path| !path.starts_with("/tmp/")));
        assert!(durable.iter().all(|path| repo.join(path).exists()));

        std::env::set_current_dir(original_cwd).unwrap();
        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn rewrite_task_artifacts_updates_step_observations() {
        let mut task = TaskNode::new("fix failing test".to_string(), TaskType::UserRequest, 100);
        let action = Action {
            tool_name: "shell.restricted".to_string(),
            params: serde_json::json!({"command": "cargo test"}),
            risk_score: 60,
        };
        let observation = Observation {
            success: true,
            output: "ok".to_string(),
            error: None,
            tokens_used: 0,
            execution_ms: 1,
            artifacts: vec!["/tmp/px/artifacts/commands/a.json".to_string()],
        };
        record_smoke_step(&mut task, 1, "run tests", action, &observation);

        rewrite_task_artifacts(
            &mut task,
            &["/tmp/px/artifacts/commands/a.json".to_string()],
            &["artifacts/coding-smoke/2026-06-09/task/evidence/a.json".to_string()],
        );

        assert_eq!(
            task.steps[0].observation.artifacts,
            vec!["artifacts/coding-smoke/2026-06-09/task/evidence/a.json"]
        );
    }

    #[test]
    fn interactive_help_surfaces_observer_commands() {
        let help = format_interactive_help();

        assert!(help.contains("Professor X interactive task mode"));
        assert!(help.contains("/brief"));
        assert!(help.contains("/cockpit"));
        assert!(help.contains("/work [n]"));
        assert!(help.contains("/sessions [n]"));
        assert!(help.contains("/session-review [session]"));
        assert!(help.contains("/session-publish [session]"));
        assert!(help.contains("/queue [n]"));
        assert!(help.contains("/queue-review [queue]"));
        assert!(help.contains("/queue-replay [queue]"));
        assert!(help.contains("/queue-publish [queue]"));
        assert!(help.contains("/plan"));
        assert!(help.contains("/preview"));
        assert!(help.contains("/enqueue <goal>"));
        assert!(help.contains("/enqueue-commit <goal>"));
        assert!(help.contains("/runs [n]"));
        assert!(help.contains("/review [run]"));
        assert!(help.contains("/replay [run]"));
        assert!(help.contains("/publish [run]"));
        assert!(help.contains("/task-review [task]"));
        assert!(help.contains("/task-evidence [task]"));
        assert!(help.contains("/inspect [task]"));
        assert!(help.contains("/step-live [n]"));
        assert!(help.contains("/step [n]"));
        assert!(help.contains("/run [n]"));
        assert!(help.contains("/run-commit [n]"));
        assert!(help.contains("/events [n]"));
        assert!(help.contains("/status"));
    }

    #[test]
    fn nonempty_or_latest_defaults_blank_refs() {
        assert_eq!(nonempty_or_latest(""), "latest");
        assert_eq!(nonempty_or_latest("   "), "latest");
        assert_eq!(nonempty_or_latest(" abc123 "), "abc123");
    }

    #[test]
    fn sanitize_operator_goal_removes_control_chars_and_bounds_length() {
        let raw = format!("  improve\n\tProf X visibility  {}", "x".repeat(400));
        let sanitized = sanitize_operator_goal(&raw);

        assert!(sanitized.starts_with("improve Prof X visibility"));
        assert!(!sanitized.contains('\n'));
        assert!(!sanitized.contains('\t'));
        assert!(sanitized.chars().count() <= 243);
    }

    #[test]
    fn operator_goal_proposal_node_preserves_goal_in_manifest_and_diff() {
        let node = operator_proposal_node(
            "px-operator-goal-test",
            Some("make queued Prof X proposals goal-specific"),
        );

        assert!(node
            .motivation
            .contains("make queued Prof X proposals goal-specific"));
        assert!(node
            .diff
            .contains("Operator goal: make queued Prof X proposals goal-specific"));
        assert!(node
            .manifest
            .root_cause
            .contains("make queued Prof X proposals goal-specific"));
        assert!(node
            .manifest
            .predicted_fixes
            .iter()
            .any(|fix| fix.contains("operator goal provenance")));
    }

    #[test]
    fn record_console_command_is_reviewable_work_event() {
        let db = Arc::new(std::sync::Mutex::new(
            rusqlite::Connection::open_in_memory().unwrap(),
        ));
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE agent_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp TEXT NOT NULL,
                    session_id TEXT,
                    task_id TEXT,
                    event_type TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    payload TEXT NOT NULL DEFAULT '{}'
                );",
            )
            .unwrap();
        let events = EventStore::new(db);

        record_console_command(&events, "review", Some("latest".to_string())).unwrap();

        let rows = events.work_tail(5).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event_type, "console.command");
        assert_eq!(rows[0].payload["command"], "review");
        assert_eq!(rows[0].payload["argument"], "latest");
        let line = format_work_event(&rows[0]);
        assert!(line.contains("Operator command"));
        assert!(line.contains("command=/review"));
        assert!(line.contains("arg=latest"));
        assert!(work_signal_summary(&rows).contains("console=1"));
    }

    #[test]
    fn format_work_event_surfaces_coding_session_identity() {
        let event = memd::events::AgentEvent {
            id: 42,
            timestamp: chrono::Utc::now(),
            session_id: Some("12345678-aaaa-bbbb-cccc-123456789abc".to_string()),
            task_id: None,
            event_type: "coding.session.started".to_string(),
            summary: "starting repo patch coding-agent session".to_string(),
            payload: serde_json::json!({
                "session_id": "12345678-aaaa-bbbb-cccc-123456789abc",
                "exercise": "repo_patch_apply_commit",
                "mode": "repo_patch_apply_commit",
                "plan_steps": ["policy gate", "sandbox verify"],
            }),
        };

        let line = format_work_event(&event);

        assert!(line.contains("CODE"));
        assert!(line.contains("Started coding session"));
        assert!(line.contains("session=12345678"));
        assert!(line.contains("exercise=repo_patch_apply_commit"));
        assert!(work_signal_summary(&[event]).contains("coding=1"));
    }

    #[test]
    fn format_work_event_surfaces_autonomy_queue_identity() {
        let event = memd::events::AgentEvent {
            id: 43,
            timestamp: chrono::Utc::now(),
            session_id: None,
            task_id: None,
            event_type: "autonomy.queue.started".to_string(),
            summary: "started autonomous queue item 12345678".to_string(),
            payload: serde_json::json!({
                "queue_id": "12345678-aaaa-bbbb-cccc-123456789abc",
                "goal": "run the next harness gate",
                "operator_goal": "run the next harness gate",
                "profile": "core",
                "cycles": 1,
            }),
        };

        let line = format_work_event(&event);

        assert!(line.contains("QUEUE"));
        assert!(line.contains("Started queued work"));
        assert!(line.contains("queue=12345678"));
        assert!(line.contains("operator-goal run the next harness gate"));
        assert!(work_signal_summary(&[event]).contains("autonomy=1"));
    }

    #[test]
    fn format_work_event_surfaces_completed_queue_evidence_bundle() {
        let event = memd::events::AgentEvent {
            id: 87,
            timestamp: chrono::Utc::now(),
            session_id: None,
            task_id: None,
            event_type: "autonomy.queue.completed".to_string(),
            summary: "completed autonomous queue item 12345678".to_string(),
            payload: serde_json::json!({
                "queue_id": "12345678-aaaa-bbbb-cccc-123456789abc",
                "result_run_id": "run-12345678-aaaa-bbbb",
                "result_report_path": "artifacts/work-loop/2026-06-08/loop-085024.json",
                "result_journal_path": "artifacts/work-loop/ledger/2026-06-08/prof-x-journal-12345678.md",
                "publish_after_run": true,
                "passed": true
            }),
        };

        let line = format_work_event(&event);

        assert!(line.contains("QUEUE"));
        assert!(line.contains("Completed queued work"));
        assert!(line.contains("queue=12345678"));
        assert!(line.contains("result-report artifacts/work-loop/2026-06-08/loop-085024.json"));
        assert!(line.contains(
            "result-journal artifacts/work-loop/ledger/2026-06-08/prof-x-journal-12345678.md"
        ));
        assert!(line.contains("passed=true"));
    }

    #[test]
    fn format_work_event_surfaces_artifact_truth_verdict() {
        let event = memd::events::AgentEvent {
            id: 88,
            timestamp: chrono::Utc::now(),
            session_id: None,
            task_id: Some("12345678-aaaa-bbbb-cccc-123456789abc".to_string()),
            event_type: "artifact.daily_update.invalid".to_string(),
            summary: "field:recorded_at: missing".to_string(),
            payload: serde_json::json!({
                "kind": "daily_update",
                "passed": false,
                "checks": [
                    {"name": "field:recorded_at", "passed": false, "detail": "missing"}
                ],
                "artifacts": ["professor-x/ops/daily/2026-06-08.md"],
                "report_path": "artifacts/validation/2026-06-08/12345678.json"
            }),
        };

        let line = format_work_event(&event);

        assert!(line.contains("ARTIFACT"));
        assert!(line.contains("Rejected artifact"));
        assert!(line.contains("task=12345678"));
        assert!(line.contains("kind=daily_update"));
        assert!(line.contains("passed=false"));
        assert!(line.contains("checks=1"));
        assert!(line.contains("artifacts=1"));
        assert!(line.contains("report artifacts/validation/2026-06-08/12345678.json"));
        assert!(line.contains("artifact professor-x/ops/daily/2026-06-08.md"));
        assert!(work_signal_summary(&[event]).contains("artifact=1"));
    }

    #[test]
    fn format_task_evidence_bundle_stitches_run_transcript_and_artifact_events() {
        let now = chrono::Utc::now();
        let transcript = TranscriptSummary {
            id: "87654321-bbbb-cccc-dddd-123456789abc".to_string(),
            task_id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            task_description: "Write a valid daily update artifact".to_string(),
            status: "failed".to_string(),
            attempt_count: 2,
            step_count: 4,
            transcript_path: "artifacts/transcripts/2026-06-08/12345678.json".to_string(),
            summary: "artifact validation failed".to_string(),
            recorded_at: now,
        };
        let run = TaskRun {
            task_id: transcript.task_id.clone(),
            description: transcript.task_description.clone(),
            task_type: "Scheduled".to_string(),
            status: "Failed".to_string(),
            priority: 90,
            attempt_count: 2,
            step_count: 4,
            last_tool: Some("fs.write".to_string()),
            last_summary: "step 4: fs.write succeeded".to_string(),
            last_output_preview: Some("wrote daily update".to_string()),
            last_error: None,
            last_artifacts: vec!["artifacts/commands/write.txt".to_string()],
            verification_summary:
                "4 step(s): 4 succeeded, 0 failed; 2 artifact(s); transcript recorded".to_string(),
            verification_artifacts: vec![
                "artifacts/transcripts/2026-06-08/12345678.json".to_string(),
                "artifacts/validation/2026-06-08/12345678.json".to_string(),
            ],
            outcome_score: Some(0.0),
            failure_class: Some(FailureClass::ArtifactValidation),
            failure_mode: Some("field:recorded_at: missing".to_string()),
            transcript_path: Some(transcript.transcript_path.clone()),
            queued_at: now,
            started_at: Some(now),
            updated_at: now,
            completed_at: Some(now),
        };
        let event = memd::events::AgentEvent {
            id: 88,
            timestamp: now,
            session_id: None,
            task_id: Some(transcript.task_id.clone()),
            event_type: "artifact.daily_update.invalid".to_string(),
            summary: "field:recorded_at: missing".to_string(),
            payload: serde_json::json!({
                "kind": "daily_update",
                "passed": false,
                "checks": [{"name": "field:recorded_at", "passed": false, "detail": "missing"}],
                "artifacts": ["professor-x/ops/daily/2026-06-08.md"],
                "report_path": "artifacts/validation/2026-06-08/12345678.json"
            }),
        };

        let bundle = format_task_evidence_bundle(&transcript, Some(&run), &[event]);

        assert!(bundle.contains("Professor X task evidence 12345678"));
        assert!(bundle.contains("transcript: artifacts/transcripts/2026-06-08/12345678.json"));
        assert!(bundle.contains("Run row"));
        assert!(bundle.contains("status: Failed"));
        assert!(bundle.contains("verification: 4 step(s): 4 succeeded"));
        assert!(bundle.contains("Artifact verdicts: 1"));
        assert!(bundle.contains("Rejected artifact"));
        assert!(bundle.contains("kind=daily_update"));
        assert!(bundle.contains("report artifacts/validation/2026-06-08/12345678.json"));
        assert!(bundle.contains("Work events: 1"));
        assert!(bundle.contains("Replay: cargo run -- --task-review 12345678"));
    }

    #[test]
    fn task_evidence_markdown_path_sits_next_to_transcript() {
        let transcript = TranscriptSummary {
            id: "87654321-bbbb-cccc-dddd-123456789abc".to_string(),
            task_id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            task_description: "Inspect task evidence".to_string(),
            status: "passed".to_string(),
            attempt_count: 1,
            step_count: 2,
            transcript_path: "artifacts/transcripts/2026-06-08/12345678.json".to_string(),
            summary: "done".to_string(),
            recorded_at: chrono::Utc::now(),
        };

        assert_eq!(
            task_evidence_markdown_path(&transcript)
                .display()
                .to_string(),
            "artifacts/transcripts/2026-06-08/12345678.evidence.md"
        );
    }

    #[test]
    fn coding_session_evidence_sits_next_to_report_and_is_attached() {
        let root =
            std::env::temp_dir().join(format!("px-session-evidence-{}", uuid::Uuid::new_v4()));
        let report_path = root
            .join("professor-x")
            .join("artifacts")
            .join("coding-sessions")
            .join("2026-06-10")
            .join("session-12345678.json");
        std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();
        let mut report = CodingSessionReport {
            id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            generated_at: "2026-06-10T08:00:00Z".to_string(),
            goal: "verify patch and publish evidence".to_string(),
            requested_goal: "verify patch and publish evidence".to_string(),
            exercise: "repo_patch_apply_commit".to_string(),
            status: "passed".to_string(),
            workspace: Some("repo-root verified apply commit".to_string()),
            smoke_id: None,
            smoke_report_path: None,
            session_report_path: None,
            transcript_path: None,
            checks: vec!["reward_hacking_scan".to_string(), "cargo_check".to_string()],
            plan_steps: vec!["Verify the unified diff in an isolated worktree".to_string()],
            step_outcomes: vec!["main apply committed".to_string()],
            artifacts: vec![
                "artifacts/evolution/patch-verifications/2026-06-10/patch.json".to_string(),
            ],
            failure_reason: None,
        };

        let evidence_path = attach_coding_session_evidence(&mut report, &report_path).unwrap();
        let evidence = std::fs::read_to_string(&evidence_path).unwrap();
        let report_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();

        assert_eq!(evidence_path, report_path.with_extension("evidence.md"));
        assert!(evidence.contains("Professor X coding session evidence 12345678"));
        assert!(evidence.contains("exercise: repo_patch_apply_commit"));
        assert!(evidence.contains("Plan steps: 1"));
        assert!(evidence.contains("Outcomes: 1"));
        assert!(evidence.contains("Publish: cargo run -- --prof-x-code-publish 12345678"));
        assert_eq!(
            report_json["session_report_path"].as_str(),
            Some(report_path.to_str().unwrap())
        );
        assert!(report_json["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str() == Some(evidence_path.to_str().unwrap())));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn persist_coding_session_terminal_report_replaces_pending_running_row() {
        let root =
            std::env::temp_dir().join(format!("px-session-terminal-{}", uuid::Uuid::new_v4()));
        let report_dir = root.join("reports");
        let data_dir = root.join("data");
        let previous_report_dir = std::env::var("PROFESSOR_X_CODING_SESSION_REPORT_DIR").ok();
        std::fs::create_dir_all(&report_dir).unwrap();
        std::env::set_var("PROFESSOR_X_CODING_SESSION_REPORT_DIR", &report_dir);

        let memory = Arc::new(MemoryManager::open(&data_dir).unwrap());
        let events = Arc::new(EventStore::new(Arc::clone(&memory.db)));
        let session_id = uuid::Uuid::new_v4();
        let generated_at = chrono::Utc::now();
        let failure_reason =
            "main worktree has source/config/skill changes; refusing patch apply".to_string();

        CodingSessionStore::new(Arc::clone(&memory.db))
            .insert(&CodingSessionRecord {
                id: session_id.to_string(),
                generated_at,
                goal: "operator goal".to_string(),
                exercise: "repo_patch_apply_commit".to_string(),
                status: "running".to_string(),
                workspace: Some("repo-root verified apply commit".to_string()),
                smoke_id: None,
                smoke_report_path: None,
                session_report_path: "pending".to_string(),
                transcript_path: None,
                artifacts: vec!["/tmp/example.diff".to_string()],
                checks: Vec::new(),
                plan_steps: vec![
                    "Policy-gate the patch through patch.apply before sandbox work".to_string(),
                ],
                step_outcomes: Vec::new(),
                failure_reason: None,
                recorded_at: generated_at,
            })
            .unwrap();

        let mut report = CodingSessionReport {
            id: session_id.to_string(),
            generated_at: generated_at.to_rfc3339(),
            goal: "repo patch coding session".to_string(),
            requested_goal: "operator goal".to_string(),
            exercise: "repo_patch_apply_commit".to_string(),
            status: "failed".to_string(),
            workspace: Some("repo-root verified apply commit".to_string()),
            smoke_id: None,
            smoke_report_path: None,
            session_report_path: None,
            transcript_path: None,
            checks: Vec::new(),
            plan_steps: vec!["Policy-gate the patch through patch.apply before sandbox work".to_string()],
            step_outcomes: vec!["apply path aborted before verification artifact: main worktree has source/config/skill changes; refusing patch apply".to_string()],
            artifacts: vec!["/tmp/example.diff".to_string()],
            failure_reason: Some(failure_reason.clone()),
        };

        let (report_path, evidence_path) = persist_coding_session_terminal_report(
            Arc::clone(&memory),
            Arc::clone(&events),
            session_id,
            generated_at,
            &mut report,
            "repo patch commit coding-session evidence written to",
            "repo patch commit coding-session report written to",
        )
        .unwrap();

        let stored = CodingSessionStore::new(Arc::clone(&memory.db))
            .get_by_ref(&session_id.to_string())
            .unwrap()
            .unwrap();
        let report_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
        let work = events.work_tail(4).unwrap();

        assert_eq!(stored.status, "failed");
        assert_eq!(stored.goal, "operator goal");
        assert_eq!(
            stored.session_report_path,
            report_path.display().to_string()
        );
        assert_eq!(
            stored.failure_reason.as_deref(),
            Some(failure_reason.as_str())
        );
        assert!(report_path.exists());
        assert!(evidence_path.exists());
        assert_eq!(
            report_json["session_report_path"].as_str(),
            Some(report_path.to_str().unwrap())
        );
        assert!(work
            .iter()
            .any(|event| event.event_type == "coding.session.evidence_written"));
        assert!(work
            .iter()
            .any(|event| event.event_type == "coding.session.failed"));

        if let Some(previous) = previous_report_dir {
            std::env::set_var("PROFESSOR_X_CODING_SESSION_REPORT_DIR", previous);
        } else {
            std::env::remove_var("PROFESSOR_X_CODING_SESSION_REPORT_DIR");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn repair_stale_coding_sessions_reconciles_old_running_rows() {
        let root = std::env::temp_dir().join(format!("px-session-repair-{}", uuid::Uuid::new_v4()));
        let report_dir = root.join("reports");
        let data_dir = root.join("data");
        let previous_report_dir = std::env::var("PROFESSOR_X_CODING_SESSION_REPORT_DIR").ok();
        std::fs::create_dir_all(&report_dir).unwrap();
        std::env::set_var("PROFESSOR_X_CODING_SESSION_REPORT_DIR", &report_dir);

        let memory = Arc::new(MemoryManager::open(&data_dir).unwrap());
        let events = Arc::new(EventStore::new(Arc::clone(&memory.db)));
        let session_id = uuid::Uuid::new_v4();
        let generated_at = chrono::Utc::now() - chrono::Duration::minutes(90);

        CodingSessionStore::new(Arc::clone(&memory.db))
            .insert(&CodingSessionRecord {
                id: session_id.to_string(),
                generated_at,
                goal: "repair stale row".to_string(),
                exercise: "repo_patch_apply_commit".to_string(),
                status: "running".to_string(),
                workspace: Some("repo-root verified apply commit".to_string()),
                smoke_id: None,
                smoke_report_path: None,
                session_report_path: "pending".to_string(),
                transcript_path: None,
                artifacts: vec!["/tmp/example.diff".to_string()],
                checks: Vec::new(),
                plan_steps: vec!["Verify patch".to_string()],
                step_outcomes: Vec::new(),
                failure_reason: None,
                recorded_at: generated_at,
            })
            .unwrap();
        memory
            .db
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO agent_events
                 (timestamp, session_id, task_id, event_type, summary, payload)
                 VALUES (?1, NULL, NULL, ?2, ?3, ?4)",
                rusqlite::params![
                    generated_at.to_rfc3339(),
                    "coding.session.started",
                    "starting repo patch commit coding-agent session",
                    serde_json::json!({
                        "session_id": session_id.to_string(),
                        "mode": "repo_patch_apply_commit",
                    })
                    .to_string(),
                ],
            )
            .unwrap();
        events
            .append(
                None,
                None,
                "daemon.started",
                "Professor X process started",
                serde_json::json!({}),
            )
            .unwrap();
        events
            .append(
                None,
                None,
                "daemon.started",
                "Professor X process started",
                serde_json::json!({}),
            )
            .unwrap();

        repair_stale_coding_sessions(Arc::clone(&memory), Arc::clone(&events), 10).unwrap();

        let stored = CodingSessionStore::new(Arc::clone(&memory.db))
            .get_by_ref(&session_id.to_string())
            .unwrap()
            .unwrap();
        let work = events.work_tail(8).unwrap();
        assert_eq!(stored.status, "failed");
        assert_ne!(stored.session_report_path, "pending");
        assert!(std::path::Path::new(&stored.session_report_path).exists());
        assert!(stored
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("stale coding session recovered"));
        assert!(work
            .iter()
            .any(|event| event.event_type == "coding.session.failed"));

        if let Some(previous) = previous_report_dir {
            std::env::set_var("PROFESSOR_X_CODING_SESSION_REPORT_DIR", previous);
        } else {
            std::env::remove_var("PROFESSOR_X_CODING_SESSION_REPORT_DIR");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn operator_skill_patch_sanitizes_goal_and_paths() {
        let body = operator_goal_skill_body(
            "px-operator-goal-test",
            &normalize_operator_goal(" capture next harness gap \n with evidence "),
        );
        let patch = OperatorSkillPatch {
            patch_path: PathBuf::from("/tmp/px-operator-goal-test.diff"),
            skill_name: "px-operator-goal-test".to_string(),
            skill_path: PathBuf::from("professor-x/skills/conductor/px-operator-goal-test.md"),
            goal: "capture next harness gap with evidence".to_string(),
        };
        let session_goal = operator_skill_session_goal(&patch, true);

        assert!(body.contains("Operator goal: capture next harness gap with evidence"));
        assert!(body.contains("workspace-bound"));
        assert_eq!(
            skill_goal_slug("Capture next harness gap!!"),
            "capture-next-harness-gap"
        );
        assert_eq!(skill_goal_slug("!!!"), "operator-goal");
        assert_eq!(
            format!(
                "{OPERATOR_SKILL_PREFIX}20260601-135027-{}",
                skill_goal_slug(
                    "preserve operator goal provenance in session evidence with extra words"
                )
            )
            .len(),
            MAX_SKILL_NAME_LEN
        );
        toolbridge::skill_loader::validate_skill_name(&format!(
            "{OPERATOR_SKILL_PREFIX}20260601-135027-{}",
            skill_goal_slug("preserve operator goal provenance in session evidence")
        ))
        .expect("generated operator skill name must satisfy loader constraints");
        assert!(session_goal.contains("goal='capture next harness gap with evidence'"));
        assert!(session_goal.contains("skill=px-operator-goal-test"));
        assert!(session_goal.contains("professor-x/skills/conductor/px-operator-goal-test.md"));
    }

    #[test]
    fn patch_apply_commit_patch_uses_operator_goal_when_present() {
        let patch_path =
            write_patch_apply_commit_patch(Some(" preserve operator goal provenance ")).unwrap();
        let file_name = patch_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let diff = std::fs::read_to_string(&patch_path).unwrap();

        assert!(file_name.starts_with(OPERATOR_SKILL_PREFIX));
        assert!(diff.contains("Operator goal: preserve operator goal provenance"));

        let _ = std::fs::remove_file(patch_path);
    }

    #[test]
    fn conductor_skills_exclude_ephemeral_operator_provenance_files() {
        let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("skills")
            .join("conductor");
        let skills = toolbridge::skill_loader::scan_skills_dir(&skills_dir);
        let names = skills
            .iter()
            .map(|(frontmatter, _)| frontmatter.name.as_str())
            .collect::<BTreeSet<_>>();

        assert!(names.contains("px-repo-patch-live-commit-smoke-20260601"));
        assert!(!names
            .iter()
            .any(|name| name.starts_with("px-operator-goal-")));
        assert!(!names
            .iter()
            .any(|name| name.starts_with("px-operator-autocommit-")));
        assert!(!names
            .iter()
            .any(|name| name.starts_with("px-autonomous-patch-")));
        assert!(names.iter().all(|name| name.len() <= MAX_SKILL_NAME_LEN));
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
    fn commit_cli_defaults_match_full_profile_gate_count() {
        let cli = parse_args_from(["professor-x", "--operator-run-commit"]);
        assert_eq!(cli.operator_run_commit_cycles, Some(6));

        let cli = parse_args_from(["professor-x", "--prof-x-live"]);
        assert_eq!(cli.operator_run_live_cycles, Some(6));

        let cli = parse_args_from(["professor-x", "--prof-x-run-commit"]);
        assert_eq!(cli.autonomous_run_commit_cycles, Some(6));
    }

    #[test]
    fn queued_context_annotates_planned_jobs() {
        let mut jobs = vec![planned_job(
            1,
            WorkLoopJob::CodingSmoke,
            "prove local coding-agent edit and verification",
        )];
        let context = WorkLoopRunContext {
            queue_id: Some("queue-12345678".to_string()),
            operator_goal: Some("make queued Prof X work visible to operators".to_string()),
        };

        annotate_planned_jobs_with_context(&mut jobs, Some(&context));

        assert!(jobs[0]
            .reason
            .contains("queued goal: make queued Prof X work visible to operators"));
        assert!(jobs[0]
            .reason
            .contains("prove local coding-agent edit and verification"));
    }

    #[test]
    fn queued_hiro_goal_targets_hiro_gate_first() {
        let mut jobs =
            plan_work_loop_jobs(WorkLoopRunKind::Operator, WorkLoopProfile::Core, 4, &[]);
        let context = WorkLoopRunContext {
            queue_id: Some("queue-12345678".to_string()),
            operator_goal: Some(
                "run HIRO benchmark inventory before the next evolution".to_string(),
            ),
        };

        prioritize_planned_jobs_for_context(&mut jobs, WorkLoopProfile::Core, Some(&context));
        annotate_planned_jobs_with_context(&mut jobs, Some(&context));

        assert_eq!(jobs[0].kind, "hiro_smoke");
        assert!(jobs[0].reason.contains("queued goal: run HIRO benchmark"));
        assert!(jobs[0].reason.contains("targets HIRO inventory smoke gate"));
        assert_eq!(jobs[0].cycle, 1);
        assert_eq!(jobs[1].cycle, 2);
    }

    #[test]
    fn queued_core_commit_goal_targets_non_committing_proposal_gate() {
        let mut jobs =
            plan_work_loop_jobs(WorkLoopRunKind::Operator, WorkLoopProfile::Core, 4, &[]);
        let context = WorkLoopRunContext {
            queue_id: Some("queue-12345678".to_string()),
            operator_goal: Some("prepare git commit evidence for the next patch".to_string()),
        };

        prioritize_planned_jobs_for_context(&mut jobs, WorkLoopProfile::Core, Some(&context));

        assert_eq!(jobs[0].kind, "proposal_dry_run");
        assert!(jobs[0]
            .reason
            .contains("targets evolution proposal dry-run gate"));
    }

    #[test]
    fn queued_commit_profile_goal_targets_verified_patch_apply_gate() {
        let mut jobs =
            plan_work_loop_jobs(WorkLoopRunKind::Operator, WorkLoopProfile::Commit, 6, &[]);
        let context = WorkLoopRunContext {
            queue_id: Some("queue-12345678".to_string()),
            operator_goal: Some("apply and commit a verified patch with git evidence".to_string()),
        };

        prioritize_planned_jobs_for_context(&mut jobs, WorkLoopProfile::Commit, Some(&context));

        assert_eq!(jobs[0].kind, "patch_apply_commit");
        assert!(jobs[0]
            .reason
            .contains("targets verified patch apply commit gate"));
    }

    #[test]
    fn queued_goal_does_not_override_failed_gate_retry() {
        let recent = vec![work_loop_run(
            "operator",
            1,
            vec![smoke("coding_smoke", true), smoke("evolution_smoke", false)],
        )];
        let mut jobs =
            plan_work_loop_jobs(WorkLoopRunKind::Operator, WorkLoopProfile::Core, 4, &recent);
        let context = WorkLoopRunContext {
            queue_id: Some("queue-12345678".to_string()),
            operator_goal: Some("run HIRO benchmark inventory now".to_string()),
        };

        prioritize_planned_jobs_for_context(&mut jobs, WorkLoopProfile::Core, Some(&context));

        assert_eq!(jobs[0].kind, "evolution_smoke");
        assert!(jobs[0].reason.contains("latest operator run failed"));
    }

    #[test]
    fn preview_pending_hiro_goal_targets_hiro_gate_without_running() {
        let preview = preview_autonomy_step_from_parts(
            Some(queue_item(
                "run HIRO benchmark inventory before the next evolution",
                WorkLoopProfile::Core,
                4,
            )),
            &[],
        );

        assert_eq!(preview.source, "pending_queue");
        assert_eq!(
            preview.queue_id.as_deref(),
            Some("12345678-aaaa-bbbb-cccc-123456789abc")
        );
        assert_eq!(preview.profile, WorkLoopProfile::Core);
        assert_eq!(preview.cycles, 4);
        assert_eq!(preview.priority, 77);
        assert_eq!(preview.planned_jobs[0].kind, "hiro_smoke");
        assert!(preview.planned_jobs[0]
            .reason
            .contains("queued goal: run HIRO benchmark"));
    }

    #[test]
    fn preview_core_commit_goal_stays_on_non_committing_proposal_gate() {
        let preview = preview_autonomy_step_from_parts(
            Some(queue_item(
                "prepare git commit evidence for the next patch",
                WorkLoopProfile::Core,
                4,
            )),
            &[],
        );

        assert_eq!(preview.planned_jobs[0].kind, "proposal_dry_run");
        assert!(preview.planned_jobs[0]
            .reason
            .contains("targets evolution proposal dry-run gate"));
    }

    #[test]
    fn preview_empty_queue_shows_planner_seed_without_queue_id() {
        let preview = preview_autonomy_step_from_parts(None, &[]);

        assert_eq!(preview.source, "planner_seed");
        assert_eq!(preview.queue_id, None);
        assert_eq!(preview.kind, "operator_run");
        assert_eq!(preview.profile, WorkLoopProfile::Core);
        assert_eq!(preview.planned_jobs[0].kind, "coding_smoke");
        assert!(preview.reason.contains("no operator run exists"));
    }

    #[test]
    fn preview_keeps_failed_gate_retry_ahead_of_goal_targeting() {
        let recent = vec![work_loop_run(
            "operator",
            1,
            vec![smoke("coding_smoke", true), smoke("evolution_smoke", false)],
        )];
        let preview = preview_autonomy_step_from_parts(
            Some(queue_item(
                "run HIRO benchmark inventory now",
                WorkLoopProfile::Core,
                4,
            )),
            &recent,
        );

        assert_eq!(preview.planned_jobs[0].kind, "evolution_smoke");
        assert!(preview.planned_jobs[0]
            .reason
            .contains("latest operator run failed"));
    }

    #[test]
    fn autonomy_planner_retries_failed_gate_first() {
        let recent = vec![work_loop_run(
            "operator",
            1,
            vec![smoke("coding_smoke", true), smoke("hiro_smoke", false)],
        )];

        let plan = plan_next_autonomy_queue_item(&recent);

        assert_eq!(plan.profile, WorkLoopProfile::Core);
        assert_eq!(plan.cycles, 1);
        assert_eq!(plan.priority, 90);
        assert!(plan.goal.contains("retry failed operator gate"));
        assert!(plan.reason.contains("hiro_smoke"));
    }

    #[test]
    fn autonomy_planner_fills_core_coverage_before_commit_gate() {
        let recent = vec![work_loop_run(
            "operator",
            0,
            vec![smoke("coding_smoke", true)],
        )];

        let plan = plan_next_autonomy_queue_item(&recent);

        assert_eq!(plan.profile, WorkLoopProfile::Core);
        assert_eq!(plan.cycles, 4);
        assert_eq!(plan.priority, 70);
        assert!(plan.reason.contains("proposal dry-run"));
    }

    #[test]
    fn autonomy_planner_advances_to_commit_after_core_coverage() {
        let recent = vec![work_loop_run(
            "operator",
            0,
            vec![
                smoke("coding_smoke", true),
                smoke("evolution_smoke", true),
                smoke("hiro_smoke", true),
                smoke("proposal_dry_run", true),
            ],
        )];

        let plan = plan_next_autonomy_queue_item(&recent);

        assert_eq!(plan.profile, WorkLoopProfile::Commit);
        assert_eq!(plan.cycles, 6);
        assert_eq!(plan.priority, 60);
        assert!(plan.reason.contains("patch_apply_commit"));
    }

    #[test]
    fn autonomy_planner_requires_operator_commit_after_patch_apply_gate() {
        let recent = vec![work_loop_run(
            "operator",
            0,
            vec![
                smoke("coding_smoke", true),
                smoke("evolution_smoke", true),
                smoke("hiro_smoke", true),
                smoke("proposal_dry_run", true),
                smoke("patch_apply_commit", true),
            ],
        )];

        let plan = plan_next_autonomy_queue_item(&recent);

        assert_eq!(plan.profile, WorkLoopProfile::Commit);
        assert_eq!(plan.cycles, 6);
        assert_eq!(plan.priority, 60);
        assert!(plan.reason.contains("operator_commit"));
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
        assert!(
            line.contains("report professor-x/artifacts/evolution/patch-verifications/patch.json")
        );
        assert!(line.contains("transcript professor-x/artifacts/transcripts/t.json"));
        assert!(line.contains("commit abcdef12"));
        assert!(line.contains("5 checks"));
    }

    #[test]
    fn format_work_event_surfaces_proposal_verification_heartbeat() {
        let event = memd::events::AgentEvent {
            id: 43,
            timestamp: chrono::Utc::now(),
            session_id: None,
            task_id: None,
            event_type: "evolution.proposal_dry_run.heartbeat".to_string(),
            summary: "proposal sandbox verification still running after 20s".to_string(),
            payload: serde_json::json!({
                "workspace": "sandbox_worktree",
                "target_component": "Skills",
                "operator_goal": "operator wants visible autonomous work",
                "elapsed_secs": 20,
                "planned_checks": [
                    "reward_hacking_scan",
                    "cargo_check"
                ],
            }),
        };

        let line = format_work_event(&event);

        assert!(line.contains("EVOLVE"));
        assert!(line.contains("still running"));
        assert!(line.contains("elapsed=20s"));
        assert!(line.contains("checks=2"));
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
            summary: "coding smoke report written to artifacts/coding-smoke/report.json"
                .to_string(),
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
    fn format_prof_x_journal_markdown_captures_observable_work() {
        let root = std::env::temp_dir().join(format!("px-journal-root-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let init = std::process::Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(init.status.success());

        let event = memd::events::AgentEvent {
            id: 42,
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-06-01T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            session_id: None,
            task_id: Some("task-abc".to_string()),
            event_type: "tool.started".to_string(),
            summary: "running cargo check".to_string(),
            payload: serde_json::json!({
                "tool": "shell.restricted",
                "params_preview": "cargo check",
                "run_id": "run-12345678"
            }),
        };

        let journal = format_prof_x_journal_markdown(&root, event.timestamp, "abc1234", &[event]);

        assert!(journal.contains("# Professor X Work Journal"));
        assert!(journal.contains("harness_commit: abc1234"));
        assert!(journal.contains("work_signal: events=1"));
        assert!(journal.contains("running cargo check"));
        assert!(journal.contains("tool=shell.restricted"));
        assert!(journal.contains("cargo run -- --observe-work"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn prof_x_journal_path_stays_inside_work_loop_ledger() {
        let root = std::path::Path::new("/tmp/professor-x-root");
        let timestamp = chrono::DateTime::parse_from_rfc3339("2026-06-01T08:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let path = prof_x_journal_path(root, timestamp);

        assert!(path.to_string_lossy().ends_with(
            "professor-x/artifacts/work-loop/ledger/2026-06-01/prof-x-journal-080000.md"
        ));
    }

    #[test]
    fn format_work_loop_ledger_links_plan_outcomes_and_timeline() {
        let report = SupervisedLoopReport {
            run_id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            run_kind: "operator".to_string(),
            queue_id: Some("queue-12345678".to_string()),
            operator_goal: Some("make Prof X work visible".to_string()),
            started_at: "2026-06-01T01:00:00Z".to_string(),
            completed_at: "2026-06-01T01:01:00Z".to_string(),
            requested_cycles: 1,
            completed_cycles: 1,
            passed_cycles: 1,
            failed_cycles: 0,
            profile: "core".to_string(),
            ledger_path: None,
            journal_path: None,
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
        assert!(ledger.contains("- queue_id: `queue-12345678`"));
        assert!(ledger.contains("- operator_goal: make Prof X work visible"));
        assert!(ledger.contains("cycle 1: `coding_smoke`"));
        assert!(ledger.contains("cycle 1 `coding_smoke`: passed"));
        assert!(ledger.contains("report: `artifacts/coding-smoke/report.json`"));
        assert!(ledger.contains("transcript: `artifacts/transcripts/task.json`"));
        assert!(ledger.contains("#00008 `TOOL` `Running`"));

        let journal = format_work_loop_journal_markdown(
            std::path::Path::new("/tmp"),
            &report,
            chrono::DateTime::parse_from_rfc3339("2026-06-01T01:01:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            "abc1234",
        );

        assert!(journal.contains("# Professor X Work Journal - 12345678"));
        assert!(journal.contains("- run_id: 12345678-aaaa-bbbb-cccc-123456789abc"));
        assert!(journal.contains("- harness_commit: abc1234"));
        assert!(journal.contains("- timeline_events: 1"));
        assert!(journal.contains("running tool 'shell.restricted'"));
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
    fn format_prof_x_brief_stitches_run_session_and_events() {
        let now = chrono::Utc::now();
        let mut run = work_loop_run("operator", 0, vec![smoke("coding_smoke", true)]);
        run.run_id = "12345678-aaaa-bbbb-cccc-123456789abc".to_string();
        run.report_path = "artifacts/work-loop/2026-06-01/loop.json".to_string();
        let session = CodingSessionRecord {
            id: "session-12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            generated_at: now,
            goal: "verify and commit a safe harness patch".to_string(),
            exercise: "repo_patch_apply_commit".to_string(),
            status: "passed".to_string(),
            workspace: Some("repo-root verified apply commit".to_string()),
            smoke_id: None,
            smoke_report_path: None,
            session_report_path: "artifacts/coding-sessions/session.json".to_string(),
            transcript_path: Some("artifacts/transcripts/session.json".to_string()),
            artifacts: vec!["artifacts/evolution/patch.json".to_string()],
            checks: vec!["cargo_check".to_string(), "git_commit".to_string()],
            plan_steps: Vec::new(),
            step_outcomes: vec!["commit abcdef1234567890".to_string()],
            failure_reason: None,
            recorded_at: now,
        };
        let event = memd::events::AgentEvent {
            id: 9,
            timestamp: now,
            session_id: None,
            task_id: None,
            event_type: "work_loop.completed".to_string(),
            summary: "operator run completed".to_string(),
            payload: serde_json::json!({}),
        };

        let brief = format_prof_x_brief(Some(&run), Some(&session), &[event]);

        assert!(brief.contains("Professor X operator brief"));
        assert!(brief.contains("Latest run"));
        assert!(brief.contains("operator:core run=12345678"));
        assert!(brief.contains("commands review=--run-review 12345678"));
        assert!(brief.contains("Latest coding session"));
        assert!(brief.contains("passed session=session-"));
        assert!(brief.contains("commit=abcdef12"));
        assert!(brief.contains("artifact artifacts/evolution/patch.json"));
        assert!(brief.contains("Recent work"));
        assert!(brief.contains("Open: cargo run -- --prof-x-chat"));
    }

    #[test]
    fn publishable_run_artifact_paths_allows_report_ledger_journal_and_evidence() {
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
        let journal_path = root
            .join("professor-x")
            .join("artifacts")
            .join("work-loop")
            .join("ledger")
            .join("2026-06-01")
            .join("prof-x-journal-12345678.md");
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
        std::fs::create_dir_all(journal_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(smoke_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(event_path.parent().unwrap()).unwrap();
        std::fs::write(&report_path, "{}").unwrap();
        std::fs::write(&ledger_path, "# run\n").unwrap();
        std::fs::write(&journal_path, "# journal\n").unwrap();
        std::fs::write(&smoke_path, "{}").unwrap();
        std::fs::write(&event_path, "{}\n").unwrap();
        let report = SupervisedLoopReport {
            run_id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            run_kind: "operator".to_string(),
            queue_id: None,
            operator_goal: None,
            started_at: "2026-06-01T01:00:00Z".to_string(),
            completed_at: "2026-06-01T01:01:00Z".to_string(),
            requested_cycles: 1,
            completed_cycles: 1,
            passed_cycles: 1,
            failed_cycles: 0,
            profile: "core".to_string(),
            ledger_path: Some(ledger_path.display().to_string()),
            journal_path: Some(journal_path.display().to_string()),
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

        assert_eq!(paths.len(), 5);
        assert!(paths.iter().any(|path| path.ends_with("loop-010000.json")));
        assert!(paths.iter().any(|path| path.ends_with("run-12345678.md")));
        assert!(paths
            .iter()
            .any(|path| path.ends_with("prof-x-journal-12345678.md")));
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
    fn git_add_publishable_artifacts_force_adds_ignored_evidence() {
        let root = std::env::temp_dir().join(format!("px-force-add-{}", uuid::Uuid::new_v4()));
        let artifact = root
            .join("professor-x")
            .join("artifacts")
            .join("transcripts")
            .join("2026-06-09")
            .join("task.json");
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(
            root.join(".gitignore"),
            "professor-x/artifacts/transcripts/\n",
        )
        .unwrap();
        std::fs::write(&artifact, "{}\n").unwrap();
        assert!(std::process::Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap()
            .status
            .success());

        git_add_publishable_artifacts(
            &root,
            &[PathBuf::from(
                "professor-x/artifacts/transcripts/2026-06-09/task.json",
            )],
            "test artifacts",
        )
        .unwrap();
        let staged = std::process::Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(&root)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&staged.stdout);

        assert!(stdout.contains("professor-x/artifacts/transcripts/2026-06-09/task.json"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn publishable_coding_session_artifact_paths_allow_only_evidence_artifacts() {
        let root =
            std::env::temp_dir().join(format!("px-session-publish-{}", uuid::Uuid::new_v4()));
        let session_report = root
            .join("professor-x")
            .join("artifacts")
            .join("coding-sessions")
            .join("2026-06-01")
            .join("session-12345678.json");
        let transcript = root
            .join("professor-x")
            .join("artifacts")
            .join("transcripts")
            .join("2026-06-01")
            .join("task-12345678.json");
        let command = root
            .join("professor-x")
            .join("artifacts")
            .join("commands")
            .join("2026-06-01")
            .join("cargo-check.json");
        let source_file = root.join("professor-x").join("src").join("main.rs");
        for path in [&session_report, &transcript, &command, &source_file] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "{}").unwrap();
        }
        let now = chrono::Utc::now();
        let mut session = CodingSessionRecord {
            id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            generated_at: now,
            goal: "publish coding evidence".to_string(),
            exercise: "repo_patch".to_string(),
            status: "passed".to_string(),
            workspace: None,
            smoke_id: None,
            smoke_report_path: None,
            session_report_path: session_report.display().to_string(),
            transcript_path: Some(transcript.display().to_string()),
            artifacts: vec![command.display().to_string()],
            checks: vec!["cargo check".to_string()],
            plan_steps: Vec::new(),
            step_outcomes: Vec::new(),
            failure_reason: None,
            recorded_at: now,
        };

        let paths = publishable_coding_session_artifact_paths(&root, &session).unwrap();
        let readiness = coding_session_publish_readiness(&root, &session).unwrap();

        assert_eq!(paths.len(), 3);
        assert_eq!(readiness, paths);
        assert!(paths
            .iter()
            .any(|path| path.ends_with("session-12345678.json")));
        assert!(paths
            .iter()
            .any(|path| path.ends_with("task-12345678.json")));
        assert!(paths.iter().any(|path| path.ends_with("cargo-check.json")));
        assert!(publishable_coding_session_artifact_path(
            std::path::Path::new("professor-x/artifacts/commands/2026-06-01/cargo-check.json")
        ));
        assert!(!publishable_coding_session_artifact_path(
            std::path::Path::new("professor-x/src/main.rs")
        ));

        session.artifacts.push(source_file.display().to_string());
        assert!(publishable_coding_session_artifact_paths(&root, &session).is_err());
        assert!(coding_session_publish_readiness(&root, &session).is_err());
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
    fn format_work_event_surfaces_coding_session_report() {
        let event = memd::events::AgentEvent {
            id: 45,
            timestamp: chrono::Utc::now(),
            session_id: Some("session-123456789".to_string()),
            task_id: None,
            event_type: "coding.session.passed".to_string(),
            summary: "repo patch commit coding-session report written".to_string(),
            payload: serde_json::json!({
                "exercise": "repo_patch_apply_commit",
                "checks": ["cargo_check", "git_commit"],
                "session_report_path": "artifacts/coding-sessions/2026-06-01/session-135052-0aeff8ac.json",
                "artifacts": ["artifacts/evolution/patch-verifications/2026-06-01/patch-135049.json"],
            }),
        };

        let line = format_work_event(&event);

        assert!(line.contains(
            "session-report artifacts/coding-sessions/2026-06-01/session-135052-0aeff8ac.json"
        ));
        assert!(line.contains(
            "artifact artifacts/evolution/patch-verifications/2026-06-01/patch-135049.json"
        ));
    }

    #[test]
    fn format_work_event_surfaces_coding_session_evidence() {
        let event = memd::events::AgentEvent {
            id: 46,
            timestamp: chrono::Utc::now(),
            session_id: Some("session-123456789".to_string()),
            task_id: None,
            event_type: "coding.session.evidence_written".to_string(),
            summary: "repo patch commit coding-session evidence written".to_string(),
            payload: serde_json::json!({
                "exercise": "repo_patch_apply_commit",
                "session_report_path": "artifacts/coding-sessions/2026-06-01/session-135052-0aeff8ac.json",
                "evidence_path": "artifacts/coding-sessions/2026-06-01/session-135052-0aeff8ac.evidence.md",
                "artifacts": [
                    "artifacts/evolution/patch-verifications/2026-06-01/patch-135049.json",
                    "artifacts/coding-sessions/2026-06-01/session-135052-0aeff8ac.evidence.md"
                ],
            }),
        };

        let line = format_work_event(&event);

        assert!(line.contains("Wrote coding evidence"));
        assert!(line.contains("exercise=repo_patch_apply_commit"));
        assert!(line.contains("artifacts=2"));
        assert!(line.contains(
            "session-report artifacts/coding-sessions/2026-06-01/session-135052-0aeff8ac.json"
        ));
        assert!(line.contains(
            "evidence artifacts/coding-sessions/2026-06-01/session-135052-0aeff8ac.evidence.md"
        ));
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

        assert_eq!(
            line,
            "  tool fs.replace: running - path=src/lib.rs mode=apply"
        );
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
        let session = CodingSessionRecord {
            id: "session-12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            generated_at: now,
            goal: "operator goal skill session: verify, apply, and commit goal='make work visible'"
                .to_string(),
            exercise: "repo_patch_apply_commit".to_string(),
            status: "passed".to_string(),
            workspace: Some("repo-root verified apply commit".to_string()),
            smoke_id: None,
            smoke_report_path: None,
            session_report_path:
                "artifacts/coding-sessions/2026-06-01/session-135052-0aeff8ac.json".to_string(),
            transcript_path: None,
            artifacts: vec![
                "artifacts/evolution/patch-verifications/2026-06-01/patch-135049.json".to_string(),
            ],
            checks: vec!["cargo_check".to_string(), "git_commit".to_string()],
            plan_steps: Vec::new(),
            step_outcomes: vec![
                "main apply committed".to_string(),
                "commit eedcd3e123456789".to_string(),
            ],
            failure_reason: None,
            recorded_at: now,
        };
        let smoke = CodingSmokeRecord {
            id: Some(12),
            generated_at: now,
            workspace: "artifacts/coding-smoke/2026-06-10/69c32462/evidence".to_string(),
            passed: true,
            initial_test_failed: true,
            edit_applied: true,
            final_test_passed: true,
            report_path: "artifacts/coding-smoke/2026-06-10/smoke-075320.json".to_string(),
            transcript_path: Some(
                "artifacts/transcripts/2026-06-10/69c32462-5fa6-4731-a49e-b1aa5263a3fa.json"
                    .to_string(),
            ),
            artifacts: vec![
                "artifacts/coding-smoke/2026-06-10/69c32462/evidence/artifacts/commands/run.json"
                    .to_string(),
                "artifacts/coding-smoke/2026-06-10/69c32462/evidence/artifacts/replacements/change.diff"
                    .to_string(),
            ],
            recorded_at: now,
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
        let evolution = EvolutionArtifactStatus {
            stage: "rejections".to_string(),
            artifact_path: "artifacts/evolution/rejections/2026-06-16/081201-test.json"
                .to_string(),
            event_type: Some("evolution.rejected".to_string()),
            event_id: Some(12),
            event_summary: Some("evolution proposal rejected".to_string()),
            generated_at: Some(now.to_rfc3339()),
            artifact_id: Some("artifact-123".to_string()),
            status: Some("Rejected".to_string()),
            target_component: Some("SkillDefinition(\"HandleActionBlocks\")".to_string()),
            reason: Some(
                "empirical repo-fix gate failed: repo-fix bench timed out after 600s"
                    .to_string(),
            ),
            checks: vec![
                "cargo_check".to_string(),
                "cargo_test".to_string(),
                "repo_fix_empirical_gate".to_string(),
            ],
            empirical_gate: None,
            empirical_gate_summary: None,
            diff_bytes: Some(707),
            applied_commit: None,
            rollback: None,
        };
        let queued = queue_item(
            "make Prof X work visible in the cockpit",
            WorkLoopProfile::Commit,
            3,
        );

        let screen = format_work_cockpit(
            std::path::Path::new("."),
            "pid=123 profx_peer=1 ollama=up model=qwen3:8b-q4_k_m",
            "shell_sandbox=fallback-policy-only bwrap_installed=true bwrap_usable=false",
            &[event],
            Some(&evolution),
            Some(&run),
            Some(&session),
            Some(&smoke),
            Some(&gate),
            std::slice::from_ref(&gate),
            std::slice::from_ref(&queued),
        );

        assert!(screen.contains("Professor X live work cockpit"));
        assert!(screen.contains("runtime pid=123 profx_peer=1 ollama=up model=qwen3:8b-q4_k_m"));
        assert!(screen.contains(
            "safety shell_sandbox=fallback-policy-only bwrap_installed=true bwrap_usable=false"
        ));
        assert!(screen.contains("state IDLE"));
        assert!(screen.contains(
            "now   last work_loop.cycle.passed #10 Prof X operator run cycle 1/2 passed"
        ));
        assert!(screen.contains("Latest evolution artifact"));
        assert!(screen.contains("rejections Rejected SkillDefinition(\"HandleActionBlocks\")"));
        assert!(screen.contains("repo-fix bench timed out after 600s"));
        assert!(screen.contains(
            "artifact artifacts/evolution/rejections/2026-06-16/081201-test.json"
        ));
        assert!(screen.contains("progress [######......] 1/2"));
        assert!(screen.contains("operator:core run=12345678"));
        assert!(screen.contains("commands replay=--replay 12345678"));
        assert!(screen.contains("Evidence bundle"));
        assert!(screen.contains("Autonomous queue"));
        assert!(screen.contains("make Prof X work visible in the cockpit"));
        assert!(screen.contains("next cargo run -- --prof-x-step-live 1"));
        assert!(screen.contains("inspect cargo run -- --prof-x-queue-review 12345678"));
        assert!(screen.contains("Latest coding session"));
        assert!(screen.contains("passed session=session-"));
        assert!(screen.contains("commit=eedcd3e1"));
        assert!(screen
            .contains("report artifacts/coding-sessions/2026-06-01/session-135052-0aeff8ac.json"));
        assert!(screen.contains("commands sessions=--coding-sessions 5"));
        assert!(screen.contains("Latest coding smoke"));
        assert!(screen.contains("gates initial_failed=true edit_applied=true final_passed=true"));
        assert!(screen.contains("report artifacts/coding-smoke/2026-06-10/smoke-075320.json"));
        assert!(screen.contains(
            "transcript artifacts/transcripts/2026-06-10/69c32462-5fa6-4731-a49e-b1aa5263a3fa.json"
        ));
        assert!(screen.contains("artifact artifacts/coding-smoke/2026-06-10/69c32462/evidence/artifacts/commands/run.json"));
        assert!(screen.contains("proof report artifacts/coding-smoke/report.json"));
        assert!(screen.contains("proof transcript artifacts/transcripts/task.json"));
        assert!(screen.contains("Recent signal events=1"));
        assert!(screen.contains("Passed gate"));
        assert!(screen.contains("--cockpit"));
        assert!(screen.contains("--prof-x-live-publish 6"));
        assert!(screen.contains("--run-review latest"));
    }

    #[test]
    fn format_work_status_json_is_machine_readable_operator_state() {
        let now = chrono::Utc::now();
        let event = memd::events::AgentEvent {
            id: 55,
            timestamp: now,
            session_id: None,
            task_id: Some("task-123456789".to_string()),
            event_type: "tool.started".to_string(),
            summary: "running tool 'shell.restricted' :: command=cargo test".to_string(),
            payload: serde_json::json!({
                "tool": "shell.restricted",
                "params_preview": "command=cargo test",
            }),
        };
        let queued = queue_item(
            "make Prof X observable from scripts",
            WorkLoopProfile::Core,
            2,
        );

        let value = format_work_status_json(
            std::path::Path::new("."),
            "pid=123 profx_peer=1 ollama=up model=qwen3:8b-q4_k_m",
            "shell_sandbox=fallback-policy-only",
            std::slice::from_ref(&event),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            &[],
            std::slice::from_ref(&queued),
        );

        assert_eq!(value["schema"], "professor_x.work_status.v1");
        assert_eq!(value["state"], "IDLE");
        assert_eq!(
            value["now"],
            "running tool shell.restricted command=cargo test"
        );
        assert_eq!(
            value["runtime"],
            "pid=123 profx_peer=1 ollama=up model=qwen3:8b-q4_k_m"
        );
        assert_eq!(value["autonomous_queue"][0]["short_id"], "12345678");
        assert!(value["autonomous_queue"][0]["next_command"]
            .as_str()
            .unwrap()
            .contains("--prof-x-step-live 1"));
        assert!(value["recent_events"][0]["line"]
            .as_str()
            .unwrap()
            .contains("Running"));
        assert!(value["commands"].as_array().unwrap().iter().any(|cmd| {
            cmd.as_str()
                .unwrap()
                .contains("cargo run -- --observe-work")
        }));
        assert!(value["latest_evolution_artifact"].is_null());
        assert!(value["latest_task_run"].is_null());
        assert!(value["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|cmd| { cmd.as_str().unwrap().contains("cargo run -- --status-json") }));
        assert!(value["commands"].as_array().unwrap().iter().any(|cmd| {
            cmd.as_str()
                .unwrap()
                .contains("cargo run -- --repair-coding-sessions 10")
        }));
    }

    #[test]
    fn accepted_artifact_surfaces_applied_commit_and_rollback_verdict() {
        let root = std::env::temp_dir().join(format!("px-rollback-status-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(args)
                .current_dir(&root)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "t"]);
        std::fs::write(root.join("seed.txt"), "x").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "accepted self-change"]);
        let head = String::from_utf8_lossy(
            &std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&root)
                .output()
                .unwrap()
                .stdout,
        )
        .trim()
        .to_string();

        let artifact_dir = root.join("artifacts/evolution/accepted/2026-06-16");
        std::fs::create_dir_all(&artifact_dir).unwrap();
        let artifact_path = artifact_dir.join("operator-commit-000000.json");
        std::fs::write(
            &artifact_path,
            serde_json::json!({
                "generated_at": "2026-06-16T03:00:27Z",
                "status": "Accepted",
                "target_component": "SkillDefinition(\"X\")",
                "accepted": true,
                "applied": true,
                "commit": head,
                "reason": "sandbox verification passed and committed",
                "diff_bytes": 452
            })
            .to_string(),
        )
        .unwrap();

        let event = memd::events::AgentEvent {
            id: 91,
            timestamp: chrono::Utc::now(),
            session_id: None,
            task_id: None,
            event_type: "evolution.operator.committed".to_string(),
            summary: "operator committed verified proposal".to_string(),
            payload: serde_json::json!({
                "artifact_path": "artifacts/evolution/accepted/2026-06-16/operator-commit-000000.json",
            }),
        };

        let summary = latest_evolution_artifact_status(&root, std::slice::from_ref(&event)).unwrap();
        assert_eq!(summary.applied_commit.as_deref(), Some(head.as_str()));
        let rollback = summary.rollback.expect("accepted commit should carry a rollback verdict");
        assert_eq!(
            rollback.status,
            evolved::rollback::RollbackStatus::Held,
            "the commit is HEAD, so it must read as held: {rollback:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn latest_evolution_artifact_status_reads_empirical_gate_summary() {
        let root = std::env::temp_dir().join(format!(
            "px-latest-evolution-status-{}",
            uuid::Uuid::new_v4()
        ));
        let artifact_dir = root.join("artifacts/evolution/rejections/2026-06-16");
        std::fs::create_dir_all(&artifact_dir).unwrap();
        let artifact_path = artifact_dir.join("081201-test.json");
        std::fs::write(
            &artifact_path,
            serde_json::json!({
                "generated_at": "2026-06-16T08:12:01Z",
                "artifact_id": "1d5d034b",
                "status": "Rejected",
                "target_component": "SkillDefinition(\"HandleActionBlocks\")",
                "analysis": "empirical repo-fix gate rejected proposal",
                "verification": {
                    "accepted": false,
                    "reason": "empirical repo-fix gate rejected proposal: repo-fix subset pass@1 baseline 0.750 candidate 0.500 delta -0.250 on 4 task(s)",
                    "checks": ["cargo_check", "cargo_test", "repo_fix_empirical_gate"],
                    "evidence": {
                        "benchmark": "repo_fix_subset",
                        "task_count": 4,
                        "baseline_score": 0.75,
                        "candidate_score": 0.5,
                        "score_delta": -0.25,
                        "passed": false
                    }
                },
                "diff_bytes": 807
            })
            .to_string(),
        )
        .unwrap();

        let event = memd::events::AgentEvent {
            id: 77,
            timestamp: chrono::Utc::now(),
            session_id: None,
            task_id: None,
            event_type: "evolution.rejected".to_string(),
            summary: "evolution proposal rejected".to_string(),
            payload: serde_json::json!({
                "artifact_path": "artifacts/evolution/rejections/2026-06-16/081201-test.json",
                "target_component": "SkillDefinition(\"HandleActionBlocks\")",
            }),
        };

        let summary =
            latest_evolution_artifact_status(&root, std::slice::from_ref(&event)).unwrap();
        assert_eq!(summary.stage, "rejections");
        assert_eq!(summary.status.as_deref(), Some("Rejected"));
        assert_eq!(
            summary.target_component.as_deref(),
            Some("SkillDefinition(\"HandleActionBlocks\")")
        );
        assert!(summary
            .reason
            .as_deref()
            .unwrap()
            .contains("repo-fix subset pass@1 baseline 0.750 candidate 0.500 delta -0.250"));
        assert_eq!(summary.checks.len(), 3);
        assert_eq!(
            summary.empirical_gate_summary.as_deref(),
            Some("repo-fix 4 task(s) baseline 0.750 candidate 0.500 delta -0.250 reject")
        );
        assert_eq!(summary.diff_bytes, Some(807));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn work_status_coding_session_json_marks_stale_sessions() {
        let now = chrono::Utc::now();
        let session = CodingSessionRecord {
            id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            generated_at: now,
            goal: "repair stale row".to_string(),
            exercise: "repo_patch_apply_commit".to_string(),
            status: "running".to_string(),
            workspace: Some("repo-root".to_string()),
            smoke_id: None,
            smoke_report_path: None,
            session_report_path: "pending".to_string(),
            transcript_path: None,
            artifacts: Vec::new(),
            checks: Vec::new(),
            plan_steps: Vec::new(),
            step_outcomes: Vec::new(),
            failure_reason: None,
            recorded_at: now,
        };
        let stale = CodingSessionStaleCandidate {
            session_id: session.id.clone(),
            last_activity_at: now,
            idle_minutes: 91,
            newer_process_starts: 3,
            reason: "3 later Professor X process starts were recorded".to_string(),
        };

        let value = work_status_coding_session_json(&session, Some(&stale));

        assert_eq!(value["status"], "stale");
        assert_eq!(value["stored_status"], "running");
        assert_eq!(value["stale"], true);
        assert_eq!(value["stale_idle_minutes"], 91);
        assert_eq!(value["stale_newer_process_starts"], 3);
        assert!(value["repair_command"]
            .as_str()
            .unwrap()
            .contains("--repair-coding-sessions 10"));
    }

    #[test]
    fn cockpit_now_summary_prioritizes_running_work() {
        let now = chrono::Utc::now();
        let running_gate = WorkLoopGateRecord {
            id: Some(1),
            run_id: "run-1".to_string(),
            run_kind: "operator".to_string(),
            profile: "core".to_string(),
            cycle: 2,
            kind: "hiro_smoke".to_string(),
            label: "HIRO smoke".to_string(),
            reason: "verify measurement".to_string(),
            status: "running".to_string(),
            started_at: Some(now),
            completed_at: None,
            passed: None,
            report_path: None,
            transcript_path: None,
            workspace: None,
            detail: "running HIRO inventory smoke".to_string(),
            recorded_at: now,
            updated_at: now,
        };
        let tool_event = memd::events::AgentEvent {
            id: 55,
            timestamp: now,
            session_id: None,
            task_id: Some("task-123456789".to_string()),
            event_type: "tool.started".to_string(),
            summary: "running tool 'shell.restricted' :: command=cargo test".to_string(),
            payload: serde_json::json!({
                "tool": "shell.restricted",
                "params_preview": "command=cargo test",
            }),
        };

        assert_eq!(
            cockpit_now_summary(&[tool_event.clone()], Some(&running_gate), None),
            "running gate cycle=2 job=hiro_smoke detail=running HIRO inventory smoke"
        );
        assert_eq!(
            cockpit_now_summary(&[tool_event], None, None),
            "running tool shell.restricted command=cargo test"
        );
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

        assert_eq!(
            coding_session_commit_hint(&session).as_deref(),
            Some("eedcd3e1")
        );
    }

    #[test]
    fn command_artifact_summary_reads_reviewable_fields() {
        let dir =
            std::env::temp_dir().join(format!("px-command-artifact-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("artifact.json");
        std::fs::write(
            &path,
            serde_json::json!({
                "command": "cargo test --bins",
                "exit_code": 0,
                "success": true,
                "stdout": "ok",
                "stderr": "",
                "stdout_bytes": 2,
                "stderr_bytes": 0,
                "recorded_at": "2026-06-02T00:00:00Z"
            })
            .to_string(),
        )
        .unwrap();

        let summary = command_artifact_summary(&path).unwrap().unwrap();
        assert!(summary.contains("cargo test --bins"));
        assert!(summary.contains("passed"));
        assert!(summary.contains("exit=0"));
        assert!(summary.contains("stdout=2B"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn repo_fix_bench_artifact_carries_reviewable_task_evidence() {
        let artifact = RepoFixBenchArtifact {
            run_id: "run-123".to_string(),
            recorded_at: "2026-06-15T00:00:00Z".to_string(),
            harness_commit: "abcdef1".to_string(),
            manifest_path: "scripts/benchmarks/repo_fix/tasks.json".to_string(),
            model: "qwen3:8b-q4_K_M".to_string(),
            passed: 1,
            ran: 2,
            pass_at_1: 0.5,
            tasks: vec![RepoFixTaskResult {
                id: "fix_001".to_string(),
                description: "fix add".to_string(),
                setup: "scripts/benchmarks/repo_fix/fix_001".to_string(),
                verify_cmd: "python3 check.py".to_string(),
                pre_exit: 1,
                post_exit: 0,
                expect_exit: 0,
                passed: true,
                made_edit: true,
                workdir: Some("/tmp/px-repofix-fix_001-run".to_string()),
                transcript_path: Some("artifacts/transcripts/2026-06-15/task.json".to_string()),
                diff_summary: "diff -ru old/calc.py new/calc.py".to_string(),
            }],
        };

        let value = serde_json::to_value(&artifact).unwrap();
        assert_eq!(value["run_id"], "run-123");
        assert_eq!(value["harness_commit"], "abcdef1");
        assert_eq!(value["model"], "qwen3:8b-q4_K_M");
        assert_eq!(value["pass_at_1"], 0.5);
        assert_eq!(value["tasks"][0]["id"], "fix_001");
        assert_eq!(value["tasks"][0]["pre_exit"], 1);
        assert_eq!(value["tasks"][0]["post_exit"], 0);
        assert_eq!(value["tasks"][0]["made_edit"], true);
        assert_eq!(value["tasks"][0]["workdir"], "/tmp/px-repofix-fix_001-run");
        assert_eq!(
            value["tasks"][0]["transcript_path"],
            "artifacts/transcripts/2026-06-15/task.json"
        );
        assert!(value["tasks"][0]["diff_summary"]
            .as_str()
            .unwrap()
            .contains("calc.py"));
    }

    #[test]
    fn repo_fix_source_diff_ignores_runtime_artifacts() {
        let root = std::env::temp_dir().join(format!("px-repofix-diff-{}", uuid::Uuid::new_v4()));
        let setup = root.join("setup");
        let workdir = root.join("workdir");
        std::fs::create_dir_all(&setup).unwrap();
        std::fs::create_dir_all(workdir.join("artifacts/checkpoints")).unwrap();
        std::fs::write(setup.join("calc.py"), "def add(a, b):\n    return a - b\n").unwrap();
        std::fs::write(
            workdir.join("calc.py"),
            "def add(a, b):\n    return a - b\n",
        )
        .unwrap();
        std::fs::write(workdir.join("artifacts/checkpoints/one.json"), "{}\n").unwrap();

        assert_eq!(repo_fix_source_diff_summary(&setup, &workdir), "");

        std::fs::write(
            workdir.join("calc.py"),
            "def add(a, b):\n    return a + b\n",
        )
        .unwrap();
        let summary = repo_fix_source_diff_summary(&setup, &workdir);
        assert!(summary.contains("calc.py"));
        assert!(!summary.contains("artifacts/checkpoints"));
        let _ = std::fs::remove_dir_all(root);
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
