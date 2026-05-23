mod memd;
mod ollama;
mod toolbridge;
mod agentd;
mod policyd;
mod evolved;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use agentd::{TaskNode, TaskQueue, TaskType};
use agentd::react::ReactLoop;
use evolved::CognitionStore;
use evolved::cognition_base::CognitionItem;
use memd::MemoryManager;
use policyd::{AuditStore, PolicyEngine};
use toolbridge::ToolRegistry;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("professor_x=info,warn"))
        )
        .init();

    info!("Professor X starting — single binary, five modules");

    let data_dir = PathBuf::from(
        std::env::var("PROFESSOR_X_DATA_DIR")
            .or_else(|_| std::env::var("JARVIS_DATA_DIR"))
            .unwrap_or_else(|_| format!("{}/.professor-x", std::env::var("HOME").unwrap_or_default()))
    );

    // ── memd ──────────────────────────────────────────────────────────────
    let memory = Arc::new(MemoryManager::open(&data_dir)?);
    info!("memd: initialized at {}", data_dir.display());

    // ── tool registry ─────────────────────────────────────────────────────
    let registry = Arc::new(std::sync::RwLock::new(ToolRegistry::new()));
    let skills_dir = PathBuf::from("skills");
    if skills_dir.exists() {
        let skills = toolbridge::skill_loader::scan_skills_dir(&skills_dir);
        info!("toolbridge: loaded {} skill(s) from skills/", skills.len());
    }

    // ── kill switch ───────────────────────────────────────────────────────
    let cancel = CancellationToken::new();
    setup_signal_handlers(cancel.clone());

    // ── policyd ───────────────────────────────────────────────────────────
    let policy = Arc::new(PolicyEngine::new(cancel.clone()));
    info!("policyd: initialized (approval_threshold=50, timeout=300s)");

    {
        let audit = AuditStore::new(Arc::clone(&memory.db));
        match audit.verify_chain() {
            Ok(true)  => info!("policyd: audit chain intact"),
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
        Ok(true)  => info!("ollama: reachable, model qwen3:8b-q4_k_m ready"),
        Ok(false) => warn!("ollama: reachable but model may not be loaded"),
        Err(e)    => warn!("ollama: not reachable ({e}) — tasks will fail until Ollama starts"),
    }

    // ── agentd: task queue + scheduler ───────────────────────────────────
    let _task_queue = Arc::new(std::sync::Mutex::new(TaskQueue::new()));
    let scheduler  = agentd::CronScheduler::new(Arc::clone(&memory.db));

    // ── task dispatch channel ─────────────────────────────────────────────
    let (task_tx, mut task_rx) = mpsc::channel::<TaskNode>(64);

    // Seed the daily autonomous cycle (runs every 7 hours)
    seed_daily_schedule(&scheduler)?;

    info!("Professor X ready — autonomous cycle active");
    info!("Kill switch: SIGUSR2 or Ctrl+C");

    // ── main event loop ───────────────────────────────────────────────────
    let mut scheduler_interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

    loop {
        tokio::select! {
            // Scheduler tick every 60 seconds
            _ = scheduler_interval.tick() => {
                match scheduler.tick() {
                    Ok(due_jobs) => {
                        for job in due_jobs {
                            let task = TaskNode::new(job.prompt.clone(), TaskType::Scheduled, 100);
                            if task_tx.try_send(task).is_err() {
                                warn!("scheduler: task channel full, dropping job '{}'", job.name);
                            }
                        }
                    }
                    Err(e) => warn!("scheduler: tick error: {e}"),
                }
            }

            // Execute incoming tasks via ReAct loop
            Some(mut task) = task_rx.recv() => {
                let memory_ref  = Arc::clone(&memory);
                let registry_ref = Arc::clone(&registry);
                let policy_ref  = Arc::clone(&policy);
                let ollama_ref  = Arc::clone(&ollama);
                let cancel_ref  = cancel.clone();

                tokio::spawn(async move {
                    let react = ReactLoop::new(
                        ollama_ref,
                        registry_ref,
                        policy_ref,
                        memory_ref,
                        cancel_ref,
                    );
                    match react.run(&mut task).await {
                        Ok(outcome) => info!(
                            "task '{}' {} (score={:.2})",
                            task.description,
                            if outcome.success { "succeeded" } else { "failed" },
                            outcome.score,
                        ),
                        Err(e) => warn!("task '{}' error: {e}", task.description),
                    }
                });
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

fn seed_daily_schedule(scheduler: &agentd::CronScheduler) -> Result<()> {
    use agentd::scheduler::{CronJob, JobState, ScheduleType};
    use chrono::Utc;

    let cycle_job = CronJob {
        id: "daily-autonomous-cycle".to_string(),
        name: "Daily research cycle".to_string(),
        prompt: "Run the daily autonomous research cycle: \
                 (1) Review brain/hypotheses.md and select the highest-priority untested hypothesis. \
                 (2) Design and run the experiment. \
                 (3) Record results in brain/hypotheses.md. \
                 (4) If results are significant, update brain/knowledge-base.md. \
                 (5) Commit all changes to git with a descriptive message.".to_string(),
        schedule_type: ScheduleType::Cron,
        schedule_value: "0 22 * * *".to_string(), // 10 PM daily
        next_run_at: Utc::now() + chrono::Duration::minutes(1),
        enabled: false, // disabled until user activates
        state: JobState::Scheduled,
        repeat_limit: None,
        repeat_completed: 0,
        last_run_at: None,
        last_status: None,
        created_at: Utc::now(),
    };

    scheduler.register(&cycle_job)?;
    info!("scheduler: daily cycle job registered (disabled until activated)");
    Ok(())
}

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

    seeds.iter().map(|(content, source)| {
        CognitionItem::new(content.to_string(), source.to_string())
    }).collect()
}
