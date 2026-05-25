pub mod pinned;
pub mod working;
pub mod episodic;
pub mod semantic;
pub mod procedural;

use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::memd::pinned::PinnedStore;
use crate::memd::working::WorkingMemory;
use crate::memd::episodic::EpisodicStore;
use crate::memd::semantic::SemanticStore;
use crate::memd::procedural::ProceduralStore;

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
"#;

pub struct MemoryManager {
    pub db: Arc<Mutex<Connection>>,
    pub pinned: PinnedStore,
    pub working: WorkingMemory,
    pub episodic: EpisodicStore,
    pub semantic: SemanticStore,
    pub procedural: ProceduralStore,
}

impl MemoryManager {
    pub fn open(data_dir: &PathBuf) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        std::fs::create_dir_all(data_dir.join("embeddings"))?;

        let db_path = data_dir.join("state.db");
        let conn = Connection::open(&db_path)?;

        // Apply schema
        conn.execute_batch(SCHEMA_SQL)?;
        info!("memd: database opened at {}", db_path.display());

        let db = Arc::new(Mutex::new(conn));

        Ok(Self {
            pinned: PinnedStore::new(Arc::clone(&db)),
            working: WorkingMemory::new(),
            episodic: EpisodicStore::new(Arc::clone(&db)),
            semantic: SemanticStore::new(Arc::clone(&db)),
            procedural: ProceduralStore::new(Arc::clone(&db)),
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
            parts.push(format!("<working-memory>\n{working_summary}\n</working-memory>"));
        }

        // Layer 3: reflexion buffer (injected by agentd per-task, not here)

        Ok(parts.join("\n\n"))
    }
}
