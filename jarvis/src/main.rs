mod memd;
mod toolbridge;
mod agentd;
mod policyd;
mod evolved;

use anyhow::Result;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("jarvis=info,warn"))
        )
        .init();

    info!("JARVIS starting — single binary, five modules");

    let data_dir = PathBuf::from(
        std::env::var("JARVIS_DATA_DIR")
            .unwrap_or_else(|_| format!("{}/.jarvis", std::env::var("HOME").unwrap_or_default()))
    );

    // Initialize memory manager (opens SQLite, applies schema)
    let memory = memd::MemoryManager::open(&data_dir)?;
    info!("memd: initialized");

    // Initialize tool registry + scan skills/
    let registry = std::sync::Arc::new(std::sync::RwLock::new(
        toolbridge::ToolRegistry::new()
    ));
    let skills_dir = PathBuf::from("skills");
    if skills_dir.exists() {
        let skills = toolbridge::skill_loader::scan_skills_dir(&skills_dir);
        info!("toolbridge: loaded {} skill(s) from skills/", skills.len());
    }

    // Kill switch: CancellationToken propagated to all components
    let cancel = CancellationToken::new();

    // Initialize policyd
    let _policy = policyd::PolicyEngine::new(cancel.clone());
    info!("policyd: initialized (approval_threshold=50, timeout=300s)");

    // Verify Merkle audit chain at startup
    {
        let audit = policyd::AuditStore::new(std::sync::Arc::clone(&memory.db));
        match audit.verify_chain() {
            Ok(true) => info!("policyd: audit chain intact"),
            Ok(false) => {
                tracing::error!("policyd: AUDIT CHAIN TAMPERED — halting");
                std::process::exit(1);
            }
            Err(e) => tracing::warn!("policyd: chain verification error: {e}"),
        }
    }

    // Seed cognition base if empty (~30 items from 15 papers)
    {
        let cognition = evolved::CognitionStore::new(std::sync::Arc::clone(&memory.db));
        cognition.seed_if_empty(seed_cognition_base())?;
        let count = cognition.count()?;
        info!("evolved: cognition base has {count} items");
    }

    info!("JARVIS ready");
    info!("To activate Professor X: inject personas/professor_x.md into pinned memory");
    info!("Kill switch: SIGUSR1 or Ctrl+C");

    // Main event loop — scheduler tick + task dispatch (Week 2)
    tokio::select! {
        _ = cancel.cancelled() => {
            info!("JARVIS shutdown via kill switch");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("JARVIS shutdown via Ctrl+C");
        }
    }

    info!("JARVIS stopped");
    Ok(())
}

fn seed_cognition_base() -> Vec<evolved::cognition_base::CognitionItem> {
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
        ("Externalization: Pattern B — working context in prompt, long-term in external store. Harness decides what to retrieve and when.", "paper:2604.08224"),
        ("Externalization: Self-evolving harnesses are an emerging direction with no existing implementations — confirmed literature gap.", "paper:2604.08224"),
        ("SLMs: qwen2.5-14B Q4 fits in 12GB VRAM and matches larger models on structured agentic tasks.", "paper:2506.02153"),
        ("SLMs: xLAM-2-8B at ~4.5GB Q4 is a strong dedicated tool-calling sub-model.", "paper:2506.02153"),
        ("Hermes: Advance next_run_at BEFORE executing jobs, under file lock — at-most-once semantics.", "repo:hermes-agent"),
        ("Hermes: SQLite WAL mode + busy_timeout=15000 + BEGIN IMMEDIATE + 15-attempt retry with random jitter.", "repo:hermes-agent"),
        ("ClawOS: Merkle-chained audit log — each entry SHA-256 hashes the previous. verify_chain() at startup detects tampering.", "repo:clawos"),
        ("ClawOS: Hook circuit breaker — 3 consecutive failures disables the hook to prevent blocking all tool calls.", "repo:clawos"),
        ("ClawOS: URL validation blocks private IPs, loopback, metadata endpoints (169.254.169.254), embedded credentials.", "repo:clawos"),
        ("JARVIS design: Single binary + tokio channels beats five daemons. No IPC overhead, no fragmented allocators.", "design:jarvis"),
        ("JARVIS design: SQLite + FAISS beats ChromaDB for in-process operation. No Python server dependency.", "design:jarvis"),
        ("JARVIS design: Core Rust modules (policyd gate, memd) must never be autonomously modified. Human approval required.", "design:jarvis"),
        ("JARVIS design: Approval timeout 300s (not 5s). The system runs overnight — 5 seconds is useless for a sleeping user.", "design:jarvis"),
        ("JARVIS design: all-MiniLM-L6-v2 via ONNX on CPU. ~80MB RAM, no VRAM cost, ~5-10ms per embedding.", "design:jarvis"),
    ];

    seeds.iter().map(|(content, source)| {
        evolved::cognition_base::CognitionItem::new(content.to_string(), source.to_string())
    }).collect()
}
