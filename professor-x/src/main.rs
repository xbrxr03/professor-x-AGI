mod agentd;
mod evolved;
mod memd;
mod ollama;
mod observer;
mod policyd;
mod toolbridge;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use agentd::react::ReactLoop;
use agentd::{TaskNode, TaskQueue, TaskType};
use evolved::cognition_base::CognitionItem;
use evolved::tracker::{OutcomeTracker, TaskOutcome};
use evolved::CognitionStore;
use evolved::{EvolvedLoop, HiroRunner};
use memd::events::EventStore;
use memd::MemoryManager;
use policyd::{AuditStore, PolicyEngine};
use toolbridge::ToolRegistry;

// ── CLI args ──────────────────────────────────────────────────────────────────

struct CliArgs {
    /// Run a single task immediately and exit.
    task: Option<String>,
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
    /// Follow agent events until interrupted.
    watch: bool,
    /// Open the full-screen terminal observer.
    observe: bool,
    /// Start the daemon and open the full-screen observer in one process.
    lab: bool,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut cli = CliArgs {
        task: None,
        run_now: false,
        hiro_round: None,
        hiro_limit: None,
        hiro_null_rounds: None,
        dry_run_daily: false,
        status: false,
        events_limit: None,
        watch: false,
        observe: false,
        lab: false,
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--task" if i + 1 < args.len() => {
                cli.task = Some(args[i + 1].clone());
                i += 2;
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
            "--watch" => {
                cli.watch = true;
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
    let inspect_mode = cli.status || cli.events_limit.is_some() || cli.watch || cli.observe || cli.lab;
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
    info!("memd: initialized at {}", data_dir.display());

    if cli.status {
        return print_status(Arc::clone(&memory), Arc::clone(&events));
    }

    if let Some(limit) = cli.events_limit {
        return print_events(Arc::clone(&events), limit);
    }

    if cli.watch {
        return watch_events(Arc::clone(&events)).await;
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

    // ── one-shot --task mode ──────────────────────────────────────────────
    if let Some(task_desc) = cli.task {
        return run_single_task(
            task_desc,
            Arc::clone(&ollama),
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&memory),
            Arc::clone(&events),
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
            cancel,
            cli.run_now,
        )
        .await;
    }

    run_daemon(ollama, registry, policy, memory, events, cancel, cli.run_now).await
}

// ── Lab mode ─────────────────────────────────────────────────────────────────

async fn run_lab(
    ollama: Arc<ollama::OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
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
        let cancel = cancel.clone();
        tokio::spawn(async move {
            run_daemon(ollama, registry, policy, memory, events, cancel, run_now).await
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

                tokio::spawn(async move {
                    let react = ReactLoop::new(
                        ollama_ref,
                        registry_ref,
                        policy_ref,
                        memory_ref,
                        cancel_ref,
                    )
                    .with_events(Arc::clone(&events_ref));
                    match react.run(&mut task).await {
                        Ok(mut outcome) => {
                            if let Some(validation_error) = validate_scheduled_artifacts(&task) {
                                warn!(
                                    "task '{}' failed artifact validation: {validation_error}",
                                    task.description
                                );
                                let _ = events_ref.append(
                                    None,
                                    Some(task.id),
                                    "artifact.invalid",
                                    validation_error.clone(),
                                    serde_json::json!({"task": task.description}),
                                );
                                outcome.success = false;
                                outcome.score = 0.0;
                                outcome.failure_mode = Some(validation_error);
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
                        let evo = EvolvedLoop::new(ollama_e, memory_e);
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
    cancel: CancellationToken,
) -> Result<()> {
    info!("one-shot task: {description}");
    events.append(
        None,
        None,
        "task.queued",
        format!("queued one-shot task: {description}"),
        serde_json::json!({"task_type": "UserRequest"}),
    )?;
    let react = ReactLoop::new(ollama, registry, policy, memory, cancel).with_events(events);
    let mut task = TaskNode::new(description, TaskType::UserRequest, 100);
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

fn print_status(memory: Arc<MemoryManager>, events: Arc<EventStore>) -> Result<()> {
    let db = memory.db.lock().unwrap();
    let active_jobs: i64 = db.query_row(
        "SELECT COUNT(*) FROM cron_jobs WHERE enabled = 1 AND state = 'Scheduled'",
        [],
        |row| row.get(0),
    )?;
    let paused_jobs: i64 = db.query_row(
        "SELECT COUNT(*) FROM cron_jobs WHERE enabled = 0 OR state = 'Paused'",
        [],
        |row| row.get(0),
    )?;
    let hiro_rounds: i64 =
        db.query_row("SELECT COUNT(*) FROM hiro_rounds", [], |row| row.get(0))?;
    let audit_entries: i64 =
        db.query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))?;
    drop(db);

    println!("Professor X status");
    println!("  scheduled jobs: {active_jobs} active, {paused_jobs} paused");
    println!("  HIRO rounds: {hiro_rounds}");
    println!("  audit entries: {audit_entries}");
    println!("  recent events:");
    for event in events.tail(8)? {
        println!("  {}", format_event(&event));
    }
    Ok(())
}

fn print_events(events: Arc<EventStore>, limit: usize) -> Result<()> {
    for event in events.tail(limit)? {
        println!("{}", format_event(&event));
    }
    Ok(())
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

fn validate_scheduled_artifacts(task: &TaskNode) -> Option<String> {
    if task.task_type != TaskType::Scheduled {
        return None;
    }

    if std::path::Path::new("professor-x").exists() {
        return Some(
            "scheduled job created nested professor-x/ directory inside crate; refusing outcome"
                .to_string(),
        );
    }

    if let Some(job_id) = scheduled_job_id(&task.description) {
        if job_id.contains("daily-update") {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            let expected = std::path::PathBuf::from("ops/daily").join(format!("{today}.md"));
            if !expected.exists() {
                return Some(format!(
                    "scheduled job '{job_id}' did not write expected daily note {}",
                    expected.display()
                ));
            }
        }
    }

    None
}

fn scheduled_job_id(description: &str) -> Option<String> {
    let marker = "scheduled daily job '";
    let start = description.find(marker)? + marker.len();
    let rest = &description[start..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
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
