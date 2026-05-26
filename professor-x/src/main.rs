mod artifacts;
mod agentd;
mod evolved;
mod memd;
mod ollama;
mod observer;
mod policyd;
mod toolbridge;

use anyhow::Result;
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
use evolved::cognition_base::CognitionItem;
use evolved::proposer::{
    ChangeManifest, EvolutionNode, HarnessComponent, VerificationStatus,
};
use evolved::tracker::{OutcomeTracker, TaskOutcome};
use evolved::verify_node_in_sandbox;
use evolved::CognitionStore;
use evolved::{EvolvedLoop, HiroRunner};
use memd::coding_smoke::{CodingSmokeRecord, CodingSmokeStore};
use memd::events::EventStore;
use memd::task_runs::{TaskRun, TaskRunStore};
use memd::transcripts::{TranscriptStore, TranscriptSummary};
use memd::MemoryManager;
use policyd::{AuditStore, Decision, PermissionScope, PolicyEngine};
use toolbridge::executor::{Action, Observation};
use toolbridge::{ToolExecutor, ToolRegistry};

// ── CLI args ──────────────────────────────────────────────────────────────────

struct CliArgs {
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
    /// Print a task transcript review by task id prefix, or 'latest'.
    task_review: Option<String>,
    /// Follow agent events until interrupted.
    watch: bool,
    /// Follow work/task/tool events until interrupted.
    watch_work: bool,
    /// Open the full-screen terminal observer.
    observe: bool,
    /// Start the daemon and open the full-screen observer in one process.
    lab: bool,
    /// Run deterministic evolution accept/reject smoke checks and exit.
    evolution_smoke: bool,
    /// Run deterministic local coding-agent edit/verify smoke and exit.
    coding_smoke: bool,
    /// Run N bounded local supervised work-loop cycles and exit.
    supervised_loop_cycles: Option<u32>,
    /// Run one seeded autonomous evolution cycle and exit.
    evolution_cycle: bool,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut cli = CliArgs {
        task: None,
        chat: false,
        run_now: false,
        hiro_round: None,
        hiro_limit: None,
        hiro_null_rounds: None,
        dry_run_daily: false,
        status: false,
        events_limit: None,
        work_feed_limit: None,
        transcripts_limit: None,
        task_runs_limit: None,
        task_review: None,
        watch: false,
        watch_work: false,
        observe: false,
        lab: false,
        evolution_smoke: false,
        coding_smoke: false,
        supervised_loop_cycles: None,
        evolution_cycle: false,
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
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
            "--coding-smoke" => {
                cli.coding_smoke = true;
                i += 1;
            }
            "--supervised-loop" => {
                let cycles = args
                    .get(i + 1)
                    .filter(|next| !next.starts_with("--"))
                    .and_then(|next| next.parse::<u32>().ok());
                cli.supervised_loop_cycles = Some(cycles.unwrap_or(1));
                i += if cycles.is_some() { 2 } else { 1 };
            }
            "--evolution-cycle" => {
                cli.evolution_cycle = true;
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
    let inspect_mode = cli.status
        || cli.events_limit.is_some()
        || cli.work_feed_limit.is_some()
        || cli.transcripts_limit.is_some()
        || cli.task_runs_limit.is_some()
        || cli.task_review.is_some()
        || cli.watch
        || cli.watch_work
        || cli.observe
        || cli.lab
        || cli.chat;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                if inspect_mode {
                    EnvFilter::new("error")
                } else {
                    EnvFilter::new("professor_x=info,warn")
                }
            }),
        )
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
    let events = Arc::new(
        EventStore::new(Arc::clone(&memory.db)).with_jsonl_mirror(event_log_dir),
    );
    let transcript_dir = std::env::var("PROFESSOR_X_TRANSCRIPT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("artifacts/transcripts"));
    let transcripts = Arc::new(TranscriptStore::new(
        Arc::clone(&memory.db),
        transcript_dir,
    ));
    let artifact_report_dir = std::env::var("PROFESSOR_X_ARTIFACT_REPORT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("artifacts/validation"));
    let artifact_validator = Arc::new(ArtifactValidator::new(artifact_report_dir));
    info!("memd: initialized at {}", data_dir.display());

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

    if let Some(task_ref) = cli.task_review {
        return print_task_review(Arc::clone(&transcripts), &task_ref);
    }

    if cli.watch {
        return watch_events(Arc::clone(&events)).await;
    }

    if cli.watch_work {
        return watch_work_feed(Arc::clone(&events)).await;
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

    if let Some(cycles) = cli.supervised_loop_cycles {
        return run_supervised_loop(
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
            cycles,
        )
        .await;
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
            cancel,
            cli.hiro_limit,
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
            cancel,
            cli.hiro_limit,
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

async fn run_evolution_smoke(events: Arc<EventStore>) -> Result<()> {
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
        format!("evolution sandbox smoke report written to {}", path.display()),
        serde_json::json!({
            "passed": passed,
            "report_path": path,
        }),
    )?;

    println!("Evolution sandbox smoke: {}", if passed { "passed" } else { "failed" });
    println!("  report: {}", path.display());
    for case in &report.cases {
        println!(
            "  {}: {} (expected {}) — {}",
            case.name,
            if case.accepted { "accepted" } else { "rejected" },
            if case.expected_accepted { "accepted" } else { "rejected" },
            case.reason
        );
    }

    if !passed {
        anyhow::bail!("evolution sandbox smoke failed");
    }
    Ok(())
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
    let path = dir.join(format!("smoke-{}.json", chrono::Utc::now().format("%H%M%S")));
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
        anyhow::bail!("git rev-parse failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[derive(serde::Serialize)]
struct CodingSmokeReport {
    generated_at: String,
    workspace: String,
    passed: bool,
    initial_test_failed: bool,
    edit_applied: bool,
    final_test_passed: bool,
    transcript_path: Option<String>,
    artifacts: Vec<String>,
}

#[derive(serde::Serialize)]
struct SupervisedLoopReport {
    run_id: String,
    started_at: String,
    completed_at: String,
    requested_cycles: u32,
    completed_cycles: u32,
    passed_cycles: u32,
    failed_cycles: u32,
    smoke_records: Vec<SupervisedLoopSmokeRecord>,
}

#[derive(serde::Serialize)]
struct SupervisedLoopSmokeRecord {
    cycle: u32,
    smoke_id: Option<i64>,
    passed: bool,
    report_path: String,
    transcript_path: Option<String>,
    workspace: String,
}

async fn run_supervised_loop(
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
    cycles: u32,
) -> Result<()> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let started_at = chrono::Utc::now();
    let cycles = cycles.clamp(1, 50);
    events.append(
        None,
        None,
        "work_loop.started",
        format!("starting supervised work loop with {cycles} cycle(s)"),
        serde_json::json!({"run_id": run_id, "cycles": cycles}),
    )?;

    let smoke_store = CodingSmokeStore::new(Arc::clone(&memory.db));
    let mut records = Vec::new();
    let mut failed_cycles = 0u32;

    for cycle in 1..=cycles {
        let before_smoke_id = smoke_store.latest()?.and_then(|record| record.id);
        events.append(
            None,
            None,
            "work_loop.cycle.started",
            format!("supervised work-loop cycle {cycle}/{cycles} started"),
            serde_json::json!({"run_id": run_id, "cycle": cycle, "cycles": cycles}),
        )?;
        let result = run_coding_smoke(
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
            Arc::clone(&transcripts),
        )
        .await;

        let latest = smoke_store
            .latest()?
            .filter(|record| record.id != before_smoke_id);
        let passed = result.is_ok() && latest.as_ref().map(|record| record.passed).unwrap_or(false);
        if !passed {
            failed_cycles += 1;
        }
        if let Some(record) = latest {
            records.push(SupervisedLoopSmokeRecord {
                cycle,
                smoke_id: record.id,
                passed: record.passed,
                report_path: record.report_path,
                transcript_path: record.transcript_path,
                workspace: record.workspace,
            });
        }

        events.append(
            None,
            None,
            if passed {
                "work_loop.cycle.passed"
            } else {
                "work_loop.cycle.failed"
            },
            format!(
                "supervised work-loop cycle {cycle}/{cycles} {}",
                if passed { "passed" } else { "failed" }
            ),
            serde_json::json!({
                "run_id": run_id,
                "cycle": cycle,
                "cycles": cycles,
                "passed": passed,
                "error": result.err().map(|e| e.to_string()),
            }),
        )?;
    }

    let report = SupervisedLoopReport {
        run_id: run_id.clone(),
        started_at: started_at.to_rfc3339(),
        completed_at: chrono::Utc::now().to_rfc3339(),
        requested_cycles: cycles,
        completed_cycles: records.len() as u32,
        passed_cycles: records.iter().filter(|record| record.passed).count() as u32,
        failed_cycles,
        smoke_records: records,
    };
    let report_path = write_supervised_loop_report(&report)?;
    events.append(
        None,
        None,
        if report.failed_cycles == 0 {
            "work_loop.completed"
        } else {
            "work_loop.completed_with_failures"
        },
        format!("supervised work-loop report written to {}", report_path.display()),
        serde_json::json!({
            "run_id": run_id,
            "report_path": report_path,
            "passed_cycles": report.passed_cycles,
            "failed_cycles": report.failed_cycles,
        }),
    )?;

    println!("Supervised loop: {} cycle(s)", report.completed_cycles);
    println!("  passed: {}", report.passed_cycles);
    println!("  failed: {}", report.failed_cycles);
    println!("  report: {}", report_path.display());
    if report.failed_cycles > 0 {
        anyhow::bail!("supervised loop completed with {} failed cycle(s)", report.failed_cycles);
    }
    Ok(())
}

async fn run_coding_smoke(
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    transcripts: Arc<TranscriptStore>,
) -> Result<()> {
    let workspace = std::env::temp_dir().join(format!("px-coding-smoke-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(workspace.join("src"))?;
    std::fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname = \"px-coding-smoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;
    std::fs::write(
        workspace.join("src/lib.rs"),
        "pub fn add(left: i32, right: i32) -> i32 {\n    left - right\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn adds_numbers() {\n        assert_eq!(add(2, 3), 5);\n    }\n}\n",
    )?;

    let mut task = TaskNode::new(
        "deterministic coding smoke: fix a failing Rust test and verify it passes".to_string(),
        TaskType::UserRequest,
        100,
    );
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
        serde_json::json!({"workspace": workspace}),
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
        &scope,
        session_id,
        task.id,
        initial_action.clone(),
    )
    .await?;
    record_smoke_step(&mut task, 1, "run the failing test before editing", initial_action, &initial);
    task_runs.step_recorded(&task)?;
    emit_smoke_tool_event(&events, session_id, task.id, 1, &task.steps[0])?;
    artifacts.extend(initial.artifacts.clone());
    let initial_test_failed = !initial.success;

    let edit_action = Action {
        tool_name: "fs.replace".to_string(),
        params: serde_json::json!({
            "path": "src/lib.rs",
            "old": "    left - right",
            "new": "    left + right",
            "mode": "apply",
        }),
        risk_score: 42,
    };
    let edit = run_smoke_tool(
        &executor,
        Arc::clone(&policy),
        Arc::clone(&memory),
        &scope,
        session_id,
        task.id,
        edit_action.clone(),
    )
    .await?;
    record_smoke_step(&mut task, 2, "apply the minimal exact replacement", edit_action, &edit);
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
        &scope,
        session_id,
        task.id,
        final_action.clone(),
    )
    .await?;
    record_smoke_step(&mut task, 3, "rerun tests after the fix", final_action, &final_test);
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

    println!("Coding smoke: {}", if passed { "passed" } else { "failed" });
    println!("  workspace: {}", workspace.display());
    println!("  report: {}", report_path.display());
    if let Some(path) = &report.transcript_path {
        println!("  transcript: {path}");
    }
    println!("  initial cargo test failed: {initial_test_failed}");
    println!("  fs.replace applied: {}", edit.success);
    println!("  final cargo test passed: {final_test_passed}");

    if !passed {
        anyhow::bail!("coding smoke failed");
    }
    Ok(())
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
            if observation.success { "succeeded" } else { "failed" },
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
    scope: &PermissionScope,
    session_id: uuid::Uuid,
    task_id: uuid::Uuid,
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
    if gate.decision != Decision::Allow {
        anyhow::bail!("policy denied {}: {}", action.tool_name, gate.reason);
    }

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
    let dir = PathBuf::from("artifacts")
        .join("coding-smoke")
        .join(chrono::Utc::now().format("%Y-%m-%d").to_string());
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("smoke-{}.json", chrono::Utc::now().format("%H%M%S")));
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
        if applied { "applied change" } else { "no change" }
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
                            let task = TaskNode::new(job.prompt.clone(), TaskType::Scheduled, 100);
                            let _ = events.append(
                                None,
                                Some(task.id),
                                "task.queued",
                                format!("queued scheduled job '{}'", job.name),
                                serde_json::json!({
                                    "job_id": job.id,
                                    "job_name": job.name,
                                    "task_type": "Scheduled",
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
                                    let event_type = if report.passed {
                                        "artifact.valid"
                                    } else {
                                        "artifact.invalid"
                                    };
                                    let _ = events_ref.append(
                                        None,
                                        Some(task.id),
                                        event_type,
                                        if report.passed {
                                            "artifact validation passed".to_string()
                                        } else {
                                            report.failure_reason().unwrap_or_else(|| "artifact validation failed".to_string())
                                        },
                                        serde_json::json!({
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

// ── HIRO benchmark mode ───────────────────────────────────────────────────────

async fn run_hiro_benchmark(
    round: u32,
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    cancel: CancellationToken,
    hiro_limit: Option<usize>,
) -> Result<()> {
    info!("HIRO benchmark — round {round}");
    let runner = HiroRunner::new(ollama, registry, policy, memory, cancel);
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
    cancel: CancellationToken,
    hiro_limit: Option<usize>,
) -> Result<()> {
    info!("HIRO null-condition baseline — {rounds} static round(s)");
    let runner = HiroRunner::new(ollama, registry, policy, memory, cancel);

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
            println!("  verification: {}", truncate(&run.verification_summary, 140));
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
    let step = event.payload["step"]
        .as_i64()
        .map(|step| format!(" step={step}"))
        .unwrap_or_default();
    let tool = event.payload["tool"]
        .as_str()
        .map(|tool| format!(" tool={tool}"))
        .unwrap_or_default();
    let duration = event.payload["execution_ms"]
        .as_i64()
        .map(|ms| format!(" {ms}ms"))
        .unwrap_or_default();
    let proof_count = event.payload["artifacts"]
        .as_array()
        .filter(|items| !items.is_empty())
        .map(|items| format!(" artifacts={}", items.len()))
        .unwrap_or_default();
    let detail = event.payload["error"]
        .as_str()
        .filter(|text| !text.is_empty())
        .or_else(|| event.payload["output_preview"].as_str())
        .map(|text| format!(" :: {}", truncate(text, 120)))
        .unwrap_or_default();
    format!(
        "#{:05} {} {:<6} task={}{}{}{}{} {}{}",
        event.id,
        event.timestamp.format("%H:%M:%S"),
        label,
        task,
        step,
        tool,
        duration,
        proof_count,
        truncate(&event.summary, 110),
        detail,
    )
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
    } else if event_type.starts_with("coding.smoke.") {
        "SMOKE"
    } else if event_type.starts_with("evolution.") {
        "EVOLVE"
    } else if event_type.starts_with("work_loop.") {
        "LOOP"
    } else if event_type == "transcript.written" {
        "TRACE"
    } else {
        "EVENT"
    }
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
        };
        scheduler.register(&cron_job)?;
    }

    info!("scheduler: registered {job_count} daily job(s)");
    Ok(())
}


// ── Cognition base ────────────────────────────────────────────────────────────

fn seed_cognition_base() -> Vec<CognitionItem> {
    let seeds = [
        ("CoALA: Language agents have four memory types — working (in-context), episodic (retrievable past), semantic (factual knowledge), procedural (skills/actions).", "paper:2309.02427"),
        ("CoALA: The action space spans storage (read/write), process (execute), and reasoning operations.", "paper:2309.02427"),
        ("Voyager: A growing skill library of verified procedural knowledge enables lifelong learning. Skills that fail consistently are pruned.", "paper:2305.16291"),
        ("Voyager: 4-round attempt limit per task prevents infinite loops while allowing recovery from transient failures.", "paper:2305.16291"),
        ("Reflexion: Verbal self-reflection after failure is reinforcement learning without weight updates. Buffer max 3 reflections, oldest evicted.", "paper:2303.11366"),
        ("ReAct: Interleaving Thought and Action/Observation is more reliable than acting alone. Thought lets the agent plan before committing to a tool call.", "paper:2210.03629"),
        ("AHE: Three observability pillars for harness evolution: component (which files changed), experience (what was tried), decision (why changes were proposed).", "paper:2604.25850"),
        ("AHE: Every harness modification needs a falsifiable ChangeManifest with predicted fixes and regressions. Verify predictions in the next cycle.", "paper:2604.25850"),
        ("AHE: Seven evolvable components: system prompt, tool descriptions, skill definitions, harness config, procedural memory, middleware, core logic.", "paper:2604.25850"),
        ("ASI-Evolve: Researcher/Engineer/Analyzer loop enables closed-loop self-improvement. Researcher proposes, Engineer experiments, Analyzer distills lessons.", "paper:2603.29640"),
        ("ASI-Evolve: UCB1 sampling c=1.414 balances exploration (unvisited nodes) vs exploitation (high-scoring nodes).", "paper:2603.29640"),
        ("ASI-Evolve: Cognition base stores ~100 distilled insights. Quality score updated via (success+1)/(use+2).", "paper:2603.29640"),
        ("EvolveR: Quality formula (success_count+1)/(use_count+2) is Laplace-smoothed. Prior of 0.5 for new items, avoids zero-division.", "paper:2510.16079"),
        ("Memory agents: Multi-signal retrieval: cosine (α=0.5) + recency decay (β=0.3, λ=0.1) + importance (γ=0.2).", "paper:2603.07670"),
        ("Memory agents: Write pipeline: filter → tag → canonicalize → deduplicate (cosine>0.92 skip) → score → embed → cluster → write.", "paper:2603.07670"),
        ("CLAG: Two-stage retrieval (cluster profile matching → intra-cluster) reduces latency. Cold start flat until 100 entries, split at 300.", "paper:2603.15421"),
        ("Self-Generated ICE: Top-k similar past tasks injected as in-context examples. Zero fine-tuning needed; ALFWorld 73%→93%.", "paper:2505.00234"),
        ("MARS: Single-cycle reflection on failure — extract principle (what not to do) + procedure (what to do instead). Write both to semantic memory.", "paper:2601.11974"),
        ("ACE: Context window as evolving playbook. Semantic memory entries are the playbook; updated on every success/failure.", "paper:2510.04618"),
        ("Life-Harness: Structural harness improvements transfer to 17 other models at 88.5% avg relative gain. Harness corpus = portable artifact.", "paper:2605.22166"),
        ("DHE: 5-layer failure attribution — retrieval→context→dispatch→execution→reasoning. Targets ≥60% fix-prediction precision vs AHE 33.7%.", "design:professor-x"),
        ("LCAP: UCB1 bandit over 5 context budget allocations per task type. c=1.414, round-level delta_p drives arm selection.", "design:professor-x"),
        ("BF: Behavioral Fingerprint F(H_k)=[p_tool, p_plan, p_correct]. Non-uniform improvement across categories confirms H11.", "design:professor-x"),
        ("MHE: Three levers — Lever1 parametric (SDAR QLoRA overnight), Lever2 contextual (ICE+MARS), Lever3 structural (DHE-guided evolution).", "design:professor-x"),
        ("Externalization: Pattern B — working context in prompt, long-term in external store. Harness decides what to retrieve and when.", "paper:2604.08224"),
        ("SLMs: qwen3:8b Q4 fits in 5.2GB VRAM, 42 tok/s, 32K context, thinking mode. Matches larger models on structured agentic tasks.", "paper:2506.02153"),
        ("Hermes: Advance next_run_at BEFORE executing jobs, under file lock — at-most-once semantics.", "repo:hermes-agent"),
        ("ClawOS: Merkle-chained audit log — each entry SHA-256 hashes the previous. verify_chain() at startup detects tampering.", "repo:clawos"),
        ("ClawOS: Hook circuit breaker — 3 consecutive failures disables the hook to prevent blocking all tool calls.", "repo:clawos"),
        ("Professor X design: Core modules (policyd gate, memd) require human approval for modification. Never autonomous.", "design:professor-x"),
    ];

    seeds
        .iter()
        .map(|(content, source)| CognitionItem::new(content.to_string(), source.to_string()))
        .collect()
}
