pub mod affect;
pub mod autonomy_queue;
pub mod causal_traces;
pub mod computational_body;
pub mod narrative;
pub mod phi;
pub mod pci;
pub mod self_authored_tests;
pub mod self_prediction;
pub mod coding_sessions;
pub mod coding_smoke;
pub mod events;
pub mod episodic;
pub mod free_energy;
pub mod ics;
pub mod metacognitive;
pub mod pinned;
pub mod procedural;
pub mod self_model;
pub mod semantic;
pub mod task_runs;
pub mod transcripts;
pub mod working;
pub mod work_loops;

use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::embeddings::EmbeddingStore;
use crate::memd::causal_traces::CausalTraceStore;
use crate::memd::computational_body::ComputationalBodyStore;
use crate::memd::narrative::NarrativeStore;
use crate::memd::phi::PhiStore;
use crate::memd::self_authored_tests::SelfAuthoredTestStore;
use crate::memd::self_prediction::SelfPredictionStore;
use crate::memd::affect::AffectStore;
use crate::memd::episodic::EpisodicStore;
use crate::memd::free_energy::FreeEnergyStore;
use crate::memd::ics::IcsStore;
use crate::memd::pinned::PinnedStore;
use crate::memd::procedural::ProceduralStore;
use crate::memd::self_model::SelfModelStore;
use crate::memd::semantic::SemanticStore;
use crate::memd::working::WorkingMemory;

// SQLite schema — single source of truth.
// Hermes Agent pattern: ALTER TABLE ADD COLUMN for migrations, no migration files.
const SCHEMA_SQL: &str = r#"
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA busy_timeout=15000;

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    model TEXT,
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    tool_call_count INTEGER DEFAULT 0,
    end_reason TEXT,
    parent_session_id TEXT
);

CREATE TABLE IF NOT EXISTS pinned (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    immutable INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS episodic (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    task_id TEXT,
    timestamp TEXT NOT NULL,
    content TEXT NOT NULL,
    keywords TEXT NOT NULL DEFAULT '[]',
    importance REAL NOT NULL DEFAULT 0.5,
    embedding_id INTEGER,
    cluster_id INTEGER
);
CREATE VIRTUAL TABLE IF NOT EXISTS episodic_fts
    USING fts5(content, keywords, content='episodic', content_rowid='rowid');

CREATE TABLE IF NOT EXISTS semantic (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'unknown',
    keywords TEXT NOT NULL DEFAULT '[]',
    quality REAL NOT NULL DEFAULT 0.5,
    use_count INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    embedding_id INTEGER,
    cluster_id INTEGER,
    created_at TEXT NOT NULL,
    last_accessed TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS procedural (
    id TEXT PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    description TEXT NOT NULL,
    skill_body TEXT NOT NULL,
    verified INTEGER NOT NULL DEFAULT 0,
    verification_score REAL NOT NULL DEFAULT 0.0,
    times_used INTEGER NOT NULL DEFAULT 0,
    times_succeeded INTEGER NOT NULL DEFAULT 0,
    embedding_id INTEGER,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cognition (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'unknown',
    keywords TEXT NOT NULL DEFAULT '[]',
    quality REAL NOT NULL DEFAULT 0.5,
    use_count INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    embedding_id INTEGER,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS evolution_nodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    parent_ids TEXT NOT NULL DEFAULT '[]',
    motivation TEXT NOT NULL,
    target_component TEXT NOT NULL,
    diff TEXT NOT NULL DEFAULT '',
    results TEXT NOT NULL DEFAULT '{}',
    analysis TEXT NOT NULL DEFAULT '',
    manifest TEXT NOT NULL DEFAULT '{}',
    score REAL NOT NULL DEFAULT 0.0,
    visit_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'Proposed'
);

CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    prev_hash TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    session_id TEXT NOT NULL,
    task_id TEXT,
    tool TEXT NOT NULL,
    params_hash TEXT NOT NULL,
    risk_score INTEGER NOT NULL,
    decision TEXT NOT NULL,
    reason TEXT NOT NULL,
    execution_ms INTEGER
);

CREATE TABLE IF NOT EXISTS agent_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    session_id TEXT,
    task_id TEXT,
    event_type TEXT NOT NULL,
    summary TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_agent_events_timestamp ON agent_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_agent_events_type ON agent_events(event_type);
CREATE INDEX IF NOT EXISTS idx_agent_events_task ON agent_events(task_id);

CREATE TABLE IF NOT EXISTS task_transcripts (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    session_ids TEXT NOT NULL DEFAULT '[]',
    task_description TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    step_count INTEGER NOT NULL DEFAULT 0,
    transcript_path TEXT NOT NULL,
    summary TEXT NOT NULL,
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_task_transcripts_task ON task_transcripts(task_id);
CREATE INDEX IF NOT EXISTS idx_task_transcripts_recorded ON task_transcripts(recorded_at);

CREATE TABLE IF NOT EXISTS task_runs (
    task_id TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    task_type TEXT NOT NULL,
    status TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    step_count INTEGER NOT NULL DEFAULT 0,
    last_tool TEXT,
    last_summary TEXT NOT NULL DEFAULT '',
    last_output_preview TEXT,
    last_error TEXT,
    last_artifacts TEXT NOT NULL DEFAULT '[]',
    verification_summary TEXT NOT NULL DEFAULT '',
    verification_artifacts TEXT NOT NULL DEFAULT '[]',
    outcome_score REAL,
    failure_mode TEXT,
    transcript_path TEXT,
    queued_at TEXT NOT NULL,
    started_at TEXT,
    updated_at TEXT NOT NULL,
    completed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_task_runs_updated ON task_runs(updated_at);
CREATE INDEX IF NOT EXISTS idx_task_runs_status ON task_runs(status);

CREATE TABLE IF NOT EXISTS autonomy_queue (
    id TEXT PRIMARY KEY,
    goal TEXT NOT NULL,
    kind TEXT NOT NULL,
    profile TEXT NOT NULL,
    cycles INTEGER NOT NULL DEFAULT 1,
    priority INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    result_run_id TEXT,
    result_report_path TEXT,
    failure_reason TEXT,
    queued_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_autonomy_queue_status ON autonomy_queue(status, priority, queued_at);
CREATE INDEX IF NOT EXISTS idx_autonomy_queue_updated ON autonomy_queue(updated_at);

CREATE TABLE IF NOT EXISTS coding_smokes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    generated_at TEXT NOT NULL,
    workspace TEXT NOT NULL,
    passed INTEGER NOT NULL DEFAULT 0,
    initial_test_failed INTEGER NOT NULL DEFAULT 0,
    edit_applied INTEGER NOT NULL DEFAULT 0,
    final_test_passed INTEGER NOT NULL DEFAULT 0,
    report_path TEXT NOT NULL,
    transcript_path TEXT,
    artifacts TEXT NOT NULL DEFAULT '[]',
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_coding_smokes_generated ON coding_smokes(generated_at);

CREATE TABLE IF NOT EXISTS coding_sessions (
    id TEXT PRIMARY KEY,
    generated_at TEXT NOT NULL,
    goal TEXT NOT NULL,
    exercise TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    workspace TEXT,
    smoke_id INTEGER,
    smoke_report_path TEXT,
    session_report_path TEXT NOT NULL,
    transcript_path TEXT,
    artifacts TEXT NOT NULL DEFAULT '[]',
    checks TEXT NOT NULL DEFAULT '[]',
    plan_steps TEXT NOT NULL DEFAULT '[]',
    step_outcomes TEXT NOT NULL DEFAULT '[]',
    failure_reason TEXT,
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_coding_sessions_generated ON coding_sessions(generated_at);
CREATE INDEX IF NOT EXISTS idx_coding_sessions_status ON coding_sessions(status);

CREATE TABLE IF NOT EXISTS work_loop_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL UNIQUE,
    run_kind TEXT NOT NULL DEFAULT 'supervised',
    profile TEXT NOT NULL DEFAULT 'basic',
    started_at TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    requested_cycles INTEGER NOT NULL DEFAULT 0,
    completed_cycles INTEGER NOT NULL DEFAULT 0,
    passed_cycles INTEGER NOT NULL DEFAULT 0,
    failed_cycles INTEGER NOT NULL DEFAULT 0,
    report_path TEXT NOT NULL,
    planned_jobs TEXT NOT NULL DEFAULT '[]',
    smoke_records TEXT NOT NULL DEFAULT '[]',
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_work_loop_runs_recorded ON work_loop_runs(recorded_at);

CREATE TABLE IF NOT EXISTS work_loop_gates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    run_kind TEXT NOT NULL DEFAULT 'supervised',
    profile TEXT NOT NULL DEFAULT 'basic',
    cycle INTEGER NOT NULL,
    kind TEXT NOT NULL,
    label TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'planned',
    started_at TEXT,
    completed_at TEXT,
    passed INTEGER,
    report_path TEXT,
    transcript_path TEXT,
    workspace TEXT,
    detail TEXT NOT NULL DEFAULT '',
    recorded_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(run_id, cycle)
);
CREATE INDEX IF NOT EXISTS idx_work_loop_gates_run ON work_loop_gates(run_id, cycle);
CREATE INDEX IF NOT EXISTS idx_work_loop_gates_updated ON work_loop_gates(updated_at);
CREATE INDEX IF NOT EXISTS idx_work_loop_gates_status ON work_loop_gates(status);

CREATE TABLE IF NOT EXISTS cron_jobs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    schedule_type TEXT NOT NULL,
    schedule_value TEXT NOT NULL,
    next_run_at TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    state TEXT NOT NULL DEFAULT 'Scheduled',
    repeat_limit INTEGER,
    repeat_completed INTEGER NOT NULL DEFAULT 0,
    last_run_at TEXT,
    last_status TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS approval_queue (
    id TEXT PRIMARY KEY,
    tool TEXT NOT NULL,
    params_summary TEXT NOT NULL,
    risk_score INTEGER NOT NULL,
    requested_at TEXT NOT NULL,
    timeout_secs INTEGER NOT NULL DEFAULT 300,
    auto_decision TEXT NOT NULL DEFAULT 'Deny',
    decided_at TEXT,
    decision TEXT
);

-- MHE metacognitive self-model (ARCHITECTURE.md Section 14).
-- Records per-round attribution accuracy for MCA computation.
CREATE TABLE IF NOT EXISTS metacognitive (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    round INTEGER NOT NULL,
    task_type TEXT NOT NULL,
    predicted_layer INTEGER NOT NULL,
    predicted_lever INTEGER NOT NULL,
    actual_improvement REAL NOT NULL DEFAULT 0.0,
    attribution_correct INTEGER NOT NULL DEFAULT 0,
    confidence REAL NOT NULL DEFAULT 0.0,
    recorded_at TEXT NOT NULL
);

-- HIRO benchmark results — one row per round.
CREATE TABLE IF NOT EXISTS hiro_rounds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    round INTEGER NOT NULL UNIQUE,
    p_tool REAL NOT NULL DEFAULT 0.0,
    p_plan REAL NOT NULL DEFAULT 0.0,
    p_correct REAL NOT NULL DEFAULT 0.0,
    pass_at_3 REAL NOT NULL DEFAULT 0.0,
    component_modified TEXT,
    harness_commit TEXT,
    recorded_at TEXT NOT NULL
);

-- HIRO attempt-level results — one row per task attempt.
CREATE TABLE IF NOT EXISTS hiro_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    round INTEGER NOT NULL,
    harness_commit TEXT NOT NULL,
    task_id TEXT NOT NULL,
    category TEXT NOT NULL,
    attempt INTEGER NOT NULL,
    passed INTEGER NOT NULL DEFAULT 0,
    failure_reason TEXT,
    output_hash TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    recorded_at TEXT NOT NULL,
    UNIQUE(round, task_id, attempt)
);

-- LCAP UCB1 arm state — persisted across runs so learning accumulates over rounds.
CREATE TABLE IF NOT EXISTS lcap_arms (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category TEXT NOT NULL,
    arm TEXT NOT NULL,
    pull_count INTEGER NOT NULL DEFAULT 0,
    total_reward REAL NOT NULL DEFAULT 0.0,
    updated_at TEXT NOT NULL,
    UNIQUE(category, arm)
);

-- IPE (Identity-Preserving Evolution) — H14 to H16.
-- self_model: Strange Loop snapshot per round.
CREATE TABLE IF NOT EXISTS self_model (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    round INTEGER NOT NULL,
    text TEXT NOT NULL,
    embedding_id INTEGER,
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_self_model_round ON self_model(round);

-- ics_scores: pairwise cosine similarity between self-model snapshots.
CREATE TABLE IF NOT EXISTS ics_scores (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    round_a INTEGER NOT NULL,
    round_b INTEGER NOT NULL,
    score REAL NOT NULL,
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ics_round_pair ON ics_scores(round_a, round_b);

-- affect_states: per-task (valence, arousal). Mean-valence binning drives H16.
CREATE TABLE IF NOT EXISTS affect_states (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    round INTEGER NOT NULL,
    valence REAL NOT NULL,
    arousal REAL NOT NULL,
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_affect_session ON affect_states(session_id);

-- fed_records: Free Energy Delta per session. H15 trajectory plot input.
CREATE TABLE IF NOT EXISTS fed_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    round INTEGER NOT NULL,
    n_predictions INTEGER NOT NULL,
    mean_abs_error REAL NOT NULL,
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_fed_round ON fed_records(round);

-- self_authored_tests: tests written by the Researcher to catch its own failure classes.
-- The agent-authored benchmark — nobody specified these tasks, the agent discovered them.
CREATE TABLE IF NOT EXISTS self_authored_tests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    origin_round INTEGER NOT NULL,
    origin_layer INTEGER NOT NULL,
    failure_pattern TEXT NOT NULL,
    description TEXT NOT NULL,
    evaluator TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'other',
    times_run INTEGER NOT NULL DEFAULT 0,
    times_passed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    last_run_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_self_authored_last_run ON self_authored_tests(last_run_at);

-- embeddings: dense vector store for semantic retrieval.
-- Keyed by (source_table, source_id). Brute-force cosine at query time.
-- Populated by Ollama nomic-embed-text (768-dim). Falls back to FTS5 if empty.
CREATE TABLE IF NOT EXISTS embeddings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_table TEXT NOT NULL,
    source_id TEXT NOT NULL,
    vector BLOB NOT NULL,
    UNIQUE(source_table, source_id)
);
CREATE INDEX IF NOT EXISTS idx_embeddings_table ON embeddings(source_table);

-- causal_traces (Seed 2 — STDP): action sequences with timing relative to outcome.
-- Learns causal chains, not correlations. Order and timing carry the signal.
CREATE TABLE IF NOT EXISTS causal_traces (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    task_category TEXT NOT NULL,
    actions TEXT NOT NULL,
    outcome INTEGER NOT NULL,
    outcome_score REAL NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_causal_traces_category ON causal_traces(task_category, created_at);

-- computational_vitals (Seed 4 — interoception): the agent's "body" signals.
-- Inference latency, token budget, evolution health. The substrate IS the body.
CREATE TABLE IF NOT EXISTS computational_vitals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    round INTEGER NOT NULL,
    inference_latency_ms REAL NOT NULL,
    token_budget_used REAL NOT NULL,
    memory_pressure REAL NOT NULL,
    evolution_health REAL NOT NULL,
    predicted_latency_ms REAL,
    interoceptive_error REAL,
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_vitals_session ON computational_vitals(session_id);

-- narrative_episodes (Seed 6): the autobiographical self as a story.
-- Each self-model update adds a connected chapter, not a flat description.
CREATE TABLE IF NOT EXISTS narrative_episodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    round INTEGER NOT NULL,
    chapter TEXT NOT NULL,
    inciting_incident TEXT NOT NULL,
    turning_point TEXT NOT NULL,
    lesson TEXT NOT NULL,
    anticipated_arc TEXT NOT NULL,
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_narrative_round ON narrative_episodes(round);

-- self_predictions (Seed 7): the agent predicts its own behaviour before tasks.
-- Per-dimension error tracks developing self-knowledge; persistent error = blind spot.
CREATE TABLE IF NOT EXISTS self_predictions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    round INTEGER NOT NULL,
    task_category TEXT NOT NULL,
    expected_tools TEXT NOT NULL,
    expected_steps INTEGER NOT NULL,
    expected_success REAL NOT NULL,
    expected_failure_mode TEXT,
    tool_err REAL NOT NULL,
    step_err REAL NOT NULL,
    success_err REAL NOT NULL,
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_self_pred_session ON self_predictions(session_id);

-- phi_activations (IIT): which cognitive modules activated per decision.
-- One row per task; activation_index packs the 7 module bits.
CREATE TABLE IF NOT EXISTS phi_activations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    round INTEGER NOT NULL,
    activation_index INTEGER NOT NULL,
    active_count INTEGER NOT NULL,
    recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_phi_activations_round ON phi_activations(round);

-- phi_rounds (IIT): integrated information (total correlation) per round.
-- The trajectory tests whether the system grows more unified as it evolves.
CREATE TABLE IF NOT EXISTS phi_rounds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    round INTEGER NOT NULL UNIQUE,
    phi REAL NOT NULL,
    n_decisions INTEGER NOT NULL,
    mean_active_modules REAL NOT NULL,
    recorded_at TEXT NOT NULL
);
"#;

pub struct MemoryManager {
    pub db: Arc<Mutex<Connection>>,
    pub pinned: PinnedStore,
    pub working: WorkingMemory,
    pub episodic: EpisodicStore,
    pub semantic: SemanticStore,
    pub procedural: ProceduralStore,
    pub self_model: SelfModelStore,
    pub ics: IcsStore,
    pub affect: AffectStore,
    pub free_energy: FreeEnergyStore,
    /// Dense vector store for semantic retrieval (nomic-embed-text, 768-dim).
    /// Populated lazily at task write-time; falls back to FTS5 when empty.
    pub embeddings: EmbeddingStore,
    /// Agent-authored benchmark: tests the Researcher writes to catch its own failure classes.
    pub self_authored_tests: SelfAuthoredTestStore,
    /// Seed 2 (STDP): causal action-sequence traces with timing.
    pub causal_traces: CausalTraceStore,
    /// Seed 4 (interoception): the agent's computational "body" signals.
    pub computational_body: ComputationalBodyStore,
    /// Seed 6 (narrative self): the autobiographical story, chapter by chapter.
    pub narrative: NarrativeStore,
    /// Seed 7 (predictive self-model): the agent's predictions about itself.
    pub self_prediction: SelfPredictionStore,
    /// IIT: integrated information (phi) — module co-activation per decision,
    /// total correlation per round. Tests whether the system grows more unified.
    pub phi: PhiStore,
}

impl MemoryManager {
    pub fn open(data_dir: &PathBuf) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        std::fs::create_dir_all(data_dir.join("embeddings"))?;

        let db_path = data_dir.join("state.db");
        let conn = Connection::open(&db_path)?;

        // Apply schema
        conn.execute_batch(SCHEMA_SQL)?;
        ensure_columns(
            &conn,
            "task_runs",
            &[
                ("last_output_preview", "TEXT"),
                ("last_error", "TEXT"),
                ("last_artifacts", "TEXT NOT NULL DEFAULT '[]'"),
                ("verification_summary", "TEXT NOT NULL DEFAULT ''"),
                (
                    "verification_artifacts",
                    "TEXT NOT NULL DEFAULT '[]'",
                ),
            ],
        )?;
        ensure_columns(
            &conn,
            "coding_smokes",
            &[("transcript_path", "TEXT")],
        )?;
        ensure_columns(
            &conn,
            "coding_sessions",
            &[
                ("exercise", "TEXT NOT NULL DEFAULT ''"),
                ("plan_steps", "TEXT NOT NULL DEFAULT '[]'"),
                ("step_outcomes", "TEXT NOT NULL DEFAULT '[]'"),
            ],
        )?;
        ensure_columns(
            &conn,
            "work_loop_runs",
            &[
                ("run_kind", "TEXT NOT NULL DEFAULT 'supervised'"),
                ("profile", "TEXT NOT NULL DEFAULT 'basic'"),
                ("planned_jobs", "TEXT NOT NULL DEFAULT '[]'"),
            ],
        )?;
        ensure_columns(
            &conn,
            "work_loop_gates",
            &[
                ("run_kind", "TEXT NOT NULL DEFAULT 'supervised'"),
                ("profile", "TEXT NOT NULL DEFAULT 'basic'"),
                ("reason", "TEXT NOT NULL DEFAULT ''"),
                ("detail", "TEXT NOT NULL DEFAULT ''"),
            ],
        )?;
        // Phase B truth gate — declared on a per-cron-job basis. Old rows get
        // NULL via the ALTER TABLE default; the validator treats that as
        // "no expected artifact" (back-compat).
        ensure_columns(
            &conn,
            "cron_jobs",
            &[("expected_artifact_kind", "TEXT")],
        )?;
        info!("memd: database opened at {}", db_path.display());

        let db = Arc::new(Mutex::new(conn));

        Ok(Self {
            pinned: PinnedStore::new(Arc::clone(&db)),
            working: WorkingMemory::new(),
            episodic: EpisodicStore::new(Arc::clone(&db)),
            semantic: SemanticStore::new(Arc::clone(&db)),
            procedural: ProceduralStore::new(Arc::clone(&db)),
            self_model: SelfModelStore::new(Arc::clone(&db)),
            ics: IcsStore::new(Arc::clone(&db)),
            affect: AffectStore::new(Arc::clone(&db)),
            free_energy: FreeEnergyStore::new(Arc::clone(&db)),
            embeddings: EmbeddingStore::new(Arc::clone(&db)),
            self_authored_tests: SelfAuthoredTestStore::new(Arc::clone(&db)),
            causal_traces: CausalTraceStore::new(Arc::clone(&db)),
            computational_body: ComputationalBodyStore::new(Arc::clone(&db)),
            narrative: NarrativeStore::new(Arc::clone(&db)),
            self_prediction: SelfPredictionStore::new(Arc::clone(&db)),
            phi: PhiStore::new(Arc::clone(&db)),
            db,
        })
    }

    /// Build the context prefix injected before every LLM call.
    /// Order: pinned → working summary → reflexion buffer → (retrieved memory injected by caller)
    pub fn build_context_prefix(&self, _session_id: &str) -> Result<String> {
        let mut parts = Vec::new();

        // Layer 1: pinned (always first)
        let pinned_entries = self.pinned.load_all()?;
        if !pinned_entries.is_empty() {
            let pinned_text = pinned_entries
                .iter()
                .map(|e| e.content.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            parts.push(format!("<identity>\n{pinned_text}\n</identity>"));
        }

        // Layer 2: working memory summary
        let working_summary = self.working.summarize();
        if !working_summary.is_empty() {
            parts.push(format!(
                "<working-memory>\n{working_summary}\n</working-memory>"
            ));
        }

        // Layer 3: reflexion buffer (injected by agentd per-task, not here)

        Ok(parts.join("\n\n"))
    }
}

fn ensure_columns(conn: &Connection, table: &str, columns: &[(&str, &str)]) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let existing = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<std::collections::HashSet<_>, _>>()?;
    for (name, definition) in columns {
        if !existing.contains(*name) {
            conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {name} {definition}"),
                [],
            )?;
        }
    }
    Ok(())
}
