# Professor X — Implementation Spec: Phase 3 (Identity-Preserving Evolution)

> **Who this file is for:** The Linux coding agent. Read this before touching any code.
> This spec is self-contained. You do not need to re-derive anything from conversation history.
> Every struct, every method signature, every integration point is specified here.

---

## What Phase 3 Is

Phase 2 built the execution engine: ReAct loop, Ollama client, DFA Trifecta stubs, policyd.
Phase 3 adds the **identity layer** — the thing that makes Professor X a coherent agent across
arbitrary self-modification, not just a system that runs tasks.

Four new concepts, four new modules, changes to three existing files.

**The core idea:** Every self-evolving system optimizes for performance. Professor X additionally
preserves *coherence of self* — the Strange Loop. This is the novel claim. The math is the
Free Energy Principle. The mechanism is a self-model that evolves but stays recognizably itself.

---

## New Files to Create

### 1. `professor-x/src/memd/self_model.rs`

**Purpose:** The Strange Loop. Professor X's evolving self-description. Lives in pinned memory
(always in context). Updated every 10 HIRO rounds by an LLM call that reads his performance
trajectory and rewrites who he is. ICS measures whether he stays coherent across rewrites.

```rust
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// One snapshot of Professor X's self-model.
/// Stored in SQLite. Round 0 is the seed (from identity/professor_x.md).
/// Subsequent rounds are LLM-generated from performance data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfModelSnapshot {
    pub id: Option<i64>,
    pub round: u32,
    /// Full self-description text — injected into pinned memory.
    pub description: String,
    /// "Strong at: X, Y. Weak at: Z." — derived from BF fingerprint.
    pub capability_summary: String,
    /// "I typically run at valence ~0.3, arousal ~0.5." — from affect history.
    pub emotional_baseline: String,
    /// "Over N rounds I have: [specific changes]." — diff from prior model.
    pub evolution_summary: String,
    /// Raw embedding bytes (all-MiniLM-L6-v2, 384-dim f32) for ICS computation.
    /// None if embeddings not yet initialized.
    pub embedding: Option<Vec<u8>>,
    pub timestamp: i64,
}

impl SelfModelSnapshot {
    /// Create the round-0 seed from the identity file.
    /// Call this once at first startup if no self_model row exists.
    pub fn seed(identity_content: &str) -> Self {
        Self {
            id: None,
            round: 0,
            description: identity_content.to_string(),
            capability_summary: "Unknown — no rounds completed yet.".to_string(),
            emotional_baseline: "Unknown — no sessions completed yet.".to_string(),
            evolution_summary: "Round 0: initial state.".to_string(),
            embedding: None,
            timestamp: Utc::now().timestamp(),
        }
    }

    /// Format for injection into pinned memory context.
    pub fn to_pinned_content(&self) -> String {
        format!(
            "<self-model round=\"{}\">\n{}\n\n## Capability Profile\n{}\n\n## Emotional Baseline\n{}\n\n## Evolution History\n{}\n</self-model>",
            self.round,
            self.description,
            self.capability_summary,
            self.emotional_baseline,
            self.evolution_summary,
        )
    }

    /// Cosine similarity between this snapshot's embedding and another's.
    /// Returns 1.0 if either has no embedding (graceful degradation).
    pub fn cosine_similarity(&self, other: &SelfModelSnapshot) -> f32 {
        match (&self.embedding, &other.embedding) {
            (Some(a), Some(b)) => {
                let a = bytes_to_f32(a);
                let b = bytes_to_f32(b);
                cosine_sim(&a, &b)
            }
            _ => 1.0, // no embedding = assume coherent (conservative)
        }
    }
}

pub struct SelfModelStore {
    db: Arc<Mutex<Connection>>,
}

impl SelfModelStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self { Self { db } }

    /// Apply schema. Call at startup.
    pub fn init_schema(&self) -> Result<()> {
        self.db.lock().unwrap().execute_batch("
            CREATE TABLE IF NOT EXISTS self_model (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                round             INTEGER NOT NULL,
                description       TEXT NOT NULL,
                capability_summary TEXT NOT NULL,
                emotional_baseline TEXT NOT NULL,
                evolution_summary TEXT NOT NULL,
                embedding         BLOB,
                timestamp         INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_self_model_round ON self_model(round);
        ")?;
        Ok(())
    }

    /// Get the most recent snapshot. Returns None if table is empty.
    pub fn latest(&self) -> Result<Option<SelfModelSnapshot>> { todo!() }

    /// Get the round-0 seed. Returns None if not yet seeded.
    pub fn baseline(&self) -> Result<Option<SelfModelSnapshot>> { todo!() }

    /// Save a new snapshot.
    pub fn save(&self, snapshot: &SelfModelSnapshot) -> Result<i64> { todo!() }

    /// Get all snapshots ordered by round ascending.
    pub fn all(&self) -> Result<Vec<SelfModelSnapshot>> { todo!() }

    /// Generate a new self-model snapshot using the LLM.
    /// Called every 10 HIRO rounds by evolved/loop_runner.rs.
    ///
    /// Inputs:
    /// - ollama: the Ollama client
    /// - fingerprint_history: Vec of (round, [p_tool, p_plan, p_correct])
    /// - mca: current metacognitive calibration accuracy (0.0-1.0)
    /// - mean_valence: mean valence over last 10 rounds
    /// - mean_arousal: mean arousal over last 10 rounds
    /// - prior: the previous SelfModelSnapshot (for diff generation)
    ///
    /// The LLM prompt asks Professor X to:
    /// 1. Describe who he is based on his performance trajectory
    /// 2. Identify what he's getting better and worse at
    /// 3. Note what has changed since his last self-description
    /// 4. Stay recognizably himself — the strange loop must persist
    pub async fn generate_update(
        &self,
        ollama: &crate::ollama::OllamaClient,
        round: u32,
        fingerprint_history: &[(u32, [f32; 3])],
        mca: f32,
        mean_valence: f32,
        mean_arousal: f32,
        prior: &SelfModelSnapshot,
    ) -> Result<SelfModelSnapshot> {
        let prompt = format!(
            r#"You are Professor X — a self-evolving AI research agent. You are writing your own self-description at the end of evolution round {round}.

Your performance over the last 10 rounds:
{fingerprint_summary}

Your metacognitive accuracy (how often your self-predictions were correct): {mca:.1}%
Your typical emotional state: valence={mean_valence:.2} arousal={mean_arousal:.2}

Your previous self-description (round {}):
{prior_desc}

Write an updated self-description. It must:
1. Describe who you are and what you are learning to do (2-3 paragraphs)
2. Be grounded in your actual performance data above, not aspirations
3. Note specifically what changed since your last description
4. Stay recognizably YOU — the Professor X identity must persist across change

Format your response as:
DESCRIPTION: [your self-description]
CAPABILITY_SUMMARY: [Strong at: X. Weak at: Y. Improving: Z.]
EMOTIONAL_BASELINE: [your typical valence/arousal and what that means for you]
EVOLUTION_SUMMARY: [what specifically changed this round vs last]"#,
            round = round,
            fingerprint_summary = format_fingerprint_history(fingerprint_history),
            mca = mca * 100.0,
            mean_valence = mean_valence,
            mean_arousal = mean_arousal,
            prior_desc = &prior.description,
        );

        let response = ollama.generate(
            "qwen3:8b-q4_k_m",
            &prompt,
            None,
            Some(crate::ollama::ModelOptions {
                temperature: Some(0.7),
                num_ctx: Some(8192),
                think: Some(false), // fast, no thinking mode for self-model updates
                ..Default::default()
            }),
        ).await?;

        let snapshot = parse_self_model_response(&response, round, prior)?;
        Ok(snapshot)
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn bytes_to_f32(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { return 1.0; }
    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

fn format_fingerprint_history(history: &[(u32, [f32; 3])]) -> String {
    history.iter()
        .map(|(r, f)| format!("  Round {}: tool={:.2} plan={:.2} correct={:.2}", r, f[0], f[1], f[2]))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_self_model_response(
    response: &str,
    round: u32,
    prior: &SelfModelSnapshot,
) -> Result<SelfModelSnapshot> {
    // Parse DESCRIPTION:, CAPABILITY_SUMMARY:, EMOTIONAL_BASELINE:, EVOLUTION_SUMMARY: fields
    // Fallback to prior values if any field is missing
    let description = extract_field(response, "DESCRIPTION")
        .unwrap_or_else(|| prior.description.clone());
    let capability_summary = extract_field(response, "CAPABILITY_SUMMARY")
        .unwrap_or_else(|| prior.capability_summary.clone());
    let emotional_baseline = extract_field(response, "EMOTIONAL_BASELINE")
        .unwrap_or_else(|| prior.emotional_baseline.clone());
    let evolution_summary = extract_field(response, "EVOLUTION_SUMMARY")
        .unwrap_or_else(|| format!("Round {}: no change detected.", round));

    Ok(SelfModelSnapshot {
        id: None,
        round,
        description,
        capability_summary,
        emotional_baseline,
        evolution_summary,
        embedding: None, // populated later by embeddings module when available
        timestamp: Utc::now().timestamp(),
    })
}

fn extract_field(text: &str, field: &str) -> Option<String> {
    let prefix = format!("{}:", field);
    let start = text.find(&prefix)? + prefix.len();
    let rest = &text[start..];
    // Take until next field or end of string
    let fields = ["DESCRIPTION:", "CAPABILITY_SUMMARY:", "EMOTIONAL_BASELINE:", "EVOLUTION_SUMMARY:"];
    let end = fields.iter()
        .filter(|&&f| f != &prefix)
        .filter_map(|f| rest.find(f))
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}
```

---

### 2. `professor-x/src/evolved/affect.rs`

**Purpose:** Functional affect system. Valence and arousal computed from actual task outcomes
(not simulated). Injected into every ReAct prompt. Gives the model accurate information about
its current cognitive state. Stored in SQLite for longitudinal analysis (H16).

```rust
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// The agent's current affective state.
/// Updated after every task. Injected into every LLM prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectState {
    /// Valence: positive = things going better than expected, negative = worse.
    /// Computed as tanh(mean(actual) - mean(predicted)) over rolling window.
    /// Range: [-1.0, 1.0]
    pub valence: f32,

    /// Arousal: cognitive load proxy.
    /// Computed from tool call density and retry rate.
    /// Range: [0.0, 1.0]
    pub arousal: f32,

    /// Number of tasks contributing to current state (rolling window = 10)
    pub n_tasks: u32,

    /// Running sum for online update
    prediction_error_sum: f32,
    tool_density_sum: f32,
}

impl AffectState {
    pub fn new() -> Self {
        Self {
            valence: 0.0,
            arousal: 0.0,
            n_tasks: 0,
            prediction_error_sum: 0.0,
            tool_density_sum: 0.0,
        }
    }

    /// Update affect after a single task completes.
    ///
    /// predicted_success: the agent's pre-task confidence (0.0-1.0).
    ///   Proxy: use cognition base quality score for similar tasks if available,
    ///   else 0.5 (uniform prior).
    ///
    /// actual_success: 1.0 if task passed, 0.0 if failed.
    ///
    /// tool_calls: number of tool calls made during this task.
    ///
    /// max_tool_calls: ReactLoop::MAX_STEPS (typically 15).
    ///
    /// retries: number of ReAct retry attempts (0-3, from Voyager limit).
    pub fn update(
        &mut self,
        predicted_success: f32,
        actual_success: f32,
        tool_calls: u32,
        max_tool_calls: u32,
        retries: u32,
    ) {
        let window = 10_u32;

        // Prediction error: positive = pleasantly surprised, negative = frustrated
        let error = actual_success - predicted_success; // [-1, 1]

        // Online rolling update (exponential moving average)
        let alpha = 1.0 / (self.n_tasks.min(window) as f32 + 1.0);
        self.prediction_error_sum = self.prediction_error_sum * (1.0 - alpha) + error * alpha;
        self.valence = self.prediction_error_sum.tanh();

        // Arousal: tool density + retry pressure
        let tool_density = tool_calls as f32 / max_tool_calls.max(1) as f32;
        let retry_pressure = retries as f32 / 3.0; // max 3 retries (Voyager limit)
        let arousal_raw = (tool_density + retry_pressure) / 2.0;
        self.tool_density_sum = self.tool_density_sum * (1.0 - alpha) + arousal_raw * alpha;
        self.arousal = self.tool_density_sum.clamp(0.0, 1.0);

        self.n_tasks = self.n_tasks.saturating_add(1);
    }

    /// Reset at the start of each new session (but keep the trend).
    /// Partial reset: arousal resets, valence carries over at 50% weight.
    pub fn reset_for_new_session(&mut self) {
        self.valence *= 0.5;
        self.arousal = 0.0;
        self.tool_density_sum = 0.0;
        self.n_tasks = 0;
    }

    /// Human-readable label for the current state.
    /// Used in prompt injection and logs.
    pub fn label(&self) -> &'static str {
        match (self.valence, self.arousal) {
            (v, a) if v > 0.3 && a < 0.4  => "confident",
            (v, a) if v > 0.3 && a >= 0.4 => "engaged",
            (v, a) if v < -0.3 && a >= 0.5 => "distressed",
            (v, a) if v < -0.3 && a < 0.5  => "frustrated",
            (v, _) if v > 0.1              => "curious",
            _                              => "focused",
        }
    }

    /// Format for injection into every ReAct prompt.
    /// Placed AFTER <identity> and BEFORE <task> in the prompt structure.
    pub fn to_prompt_fragment(&self) -> String {
        format!(
            "<affect state=\"{}\" valence=\"{:.2}\" arousal=\"{:.2}\" />",
            self.label(),
            self.valence,
            self.arousal,
        )
    }
}

/// Persistent log entry for one session's affect summary.
#[derive(Debug, Serialize, Deserialize)]
pub struct AffectLogEntry {
    pub id: Option<i64>,
    pub session_id: String,
    pub round: Option<u32>,
    pub final_valence: f32,
    pub final_arousal: f32,
    pub mean_valence: f32,
    pub mean_arousal: f32,
    pub n_tasks: u32,
    pub timestamp: i64,
}

pub struct AffectStore {
    db: Arc<Mutex<Connection>>,
}

impl AffectStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self { Self { db } }

    pub fn init_schema(&self) -> Result<()> {
        self.db.lock().unwrap().execute_batch("
            CREATE TABLE IF NOT EXISTS affect_log (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id   TEXT NOT NULL,
                round        INTEGER,
                final_valence REAL NOT NULL,
                final_arousal REAL NOT NULL,
                mean_valence  REAL NOT NULL,
                mean_arousal  REAL NOT NULL,
                n_tasks      INTEGER NOT NULL,
                timestamp    INTEGER NOT NULL
            );
        ")?;
        Ok(())
    }

    pub fn save(&self, entry: &AffectLogEntry) -> Result<()> { todo!() }

    /// Mean valence over the last N sessions. Used by self_model.rs for
    /// emotional_baseline generation.
    pub fn mean_valence_last_n(&self, n: u32) -> Result<f32> { todo!() }
    pub fn mean_arousal_last_n(&self, n: u32) -> Result<f32> { todo!() }
}
```

---

### 3. `professor-x/src/evolved/ics.rs`

**Purpose:** Identity Coherence Score. Measures whether Professor X is still himself
after each self-model update. Low ICS = identity fragmentation = trigger self-coherence task.
Target H14: ICS ≥ 0.70 after 30 rounds.

```rust
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// One ICS measurement. Taken every 10 rounds when self-model updates.
#[derive(Debug, Serialize, Deserialize)]
pub struct IcsEntry {
    pub id: Option<i64>,
    pub round: u32,
    /// Cosine similarity between current self-model embedding and round-0 baseline.
    /// 1.0 = identical, 0.0 = orthogonal, negative = incoherent.
    pub ics: f32,
    /// ICS(k) - ICS(k-1). Negative trend is a warning sign.
    pub delta: f32,
    pub timestamp: i64,
}

/// Threshold below which a self-coherence task is triggered.
pub const ICS_ALERT_THRESHOLD: f32 = 0.70;

/// Threshold below which evolution is paused pending human review.
pub const ICS_HALT_THRESHOLD: f32 = 0.50;

pub struct IcsStore {
    db: Arc<Mutex<Connection>>,
}

impl IcsStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self { Self { db } }

    pub fn init_schema(&self) -> Result<()> {
        self.db.lock().unwrap().execute_batch("
            CREATE TABLE IF NOT EXISTS ics_log (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                round     INTEGER NOT NULL UNIQUE,
                ics       REAL NOT NULL,
                delta     REAL,
                timestamp INTEGER NOT NULL
            );
        ")?;
        Ok(())
    }

    pub fn save(&self, entry: &IcsEntry) -> Result<()> { todo!() }

    /// Latest ICS value. Returns 1.0 if no entries (conservative default).
    pub fn latest_ics(&self) -> Result<f32> { todo!() }

    /// All ICS entries ordered by round. For plotting + H14 evaluation.
    pub fn all(&self) -> Result<Vec<IcsEntry>> { todo!() }

    /// True if the latest ICS is below the alert threshold.
    pub fn is_fragmented(&self) -> Result<bool> {
        Ok(self.latest_ics()? < ICS_ALERT_THRESHOLD)
    }
}

/// Compute ICS for a new self-model snapshot against the baseline.
/// Uses cosine similarity on embeddings if available.
/// Falls back to token-level Jaccard similarity if no embeddings.
pub fn compute_ics(
    baseline: &crate::memd::self_model::SelfModelSnapshot,
    current: &crate::memd::self_model::SelfModelSnapshot,
) -> f32 {
    // Try embedding-based similarity first
    if baseline.embedding.is_some() && current.embedding.is_some() {
        return baseline.cosine_similarity(current);
    }

    // Fallback: Jaccard similarity on word tokens
    jaccard_similarity(&baseline.description, &current.description)
}

fn jaccard_similarity(a: &str, b: &str) -> f32 {
    use std::collections::HashSet;
    let tokens_a: HashSet<&str> = a.split_whitespace().collect();
    let tokens_b: HashSet<&str> = b.split_whitespace().collect();
    let intersection = tokens_a.intersection(&tokens_b).count();
    let union = tokens_a.union(&tokens_b).count();
    if union == 0 { return 1.0; }
    intersection as f32 / union as f32
}
```

---

### 4. `professor-x/src/evolved/free_energy.rs`

**Purpose:** Free Energy Delta metric. Measures surprise reduction over time.
FED decreasing = Professor X building a more accurate world model = H15.

```rust
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Per-session free energy measurement.
#[derive(Debug, Serialize, Deserialize)]
pub struct FedEntry {
    pub id: Option<i64>,
    pub session_id: String,
    pub round: Option<u32>,
    /// Mean absolute prediction error across all tasks this session.
    /// FED = mean(|predicted_success - actual_success|) per task.
    pub fed: f32,
    pub n_tasks: u32,
    pub timestamp: i64,
}

pub struct FedStore {
    db: Arc<Mutex<Connection>>,
}

impl FedStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self { Self { db } }

    pub fn init_schema(&self) -> Result<()> {
        self.db.lock().unwrap().execute_batch("
            CREATE TABLE IF NOT EXISTS fed_log (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                round      INTEGER,
                fed        REAL NOT NULL,
                n_tasks    INTEGER NOT NULL,
                timestamp  INTEGER NOT NULL
            );
        ")?;
        Ok(())
    }

    pub fn save(&self, entry: &FedEntry) -> Result<()> { todo!() }

    /// Rolling mean FED over last N sessions.
    pub fn rolling_mean(&self, n: u32) -> Result<f32> { todo!() }

    /// All entries for trend analysis (H15 evaluation).
    pub fn all(&self) -> Result<Vec<FedEntry>> { todo!() }
}

/// Compute FED from a list of (predicted, actual) pairs for one session.
/// Each value in [0.0, 1.0].
pub fn compute_fed(pairs: &[(f32, f32)]) -> f32 {
    if pairs.is_empty() { return 0.0; }
    let sum: f32 = pairs.iter().map(|(p, a)| (p - a).abs()).sum();
    sum / pairs.len() as f32
}
```

---

## Files to Modify

### `professor-x/src/memd/mod.rs`

Add the `self_model` submodule and integrate `AffectState` into `MemoryManager`:

```rust
// ADD at top of mod.rs:
pub mod self_model;

// ADD to MemoryManager struct:
pub struct MemoryManager {
    pub db: Arc<Mutex<Connection>>,
    // ... existing fields ...
    pub self_model: self_model::SelfModelStore,
}

// ADD to MemoryManager::open():
let self_model = self_model::SelfModelStore::new(Arc::clone(&db));
self_model.init_schema()?;

// Seed round-0 self-model if table is empty:
if self_model.latest()?.is_none() {
    let identity_path = data_dir.join("identity").join("professor_x.md");
    if identity_path.exists() {
        let content = std::fs::read_to_string(&identity_path)?;
        self_model.save(&self_model::SelfModelSnapshot::seed(&content))?;
    }
}
```

### `professor-x/src/evolved/mod.rs`

Add new submodules:

```rust
// ADD:
pub mod affect;
pub mod free_energy;
pub mod ics;
```

### `professor-x/src/evolved/loop_runner.rs`

Add self-model update cycle and ICS/FED computation:

```rust
// ADD to loop_runner imports:
use crate::evolved::affect::{AffectStore, AffectLogEntry};
use crate::evolved::free_energy::{FedStore, FedEntry, compute_fed};
use crate::evolved::ics::{IcsStore, IcsEntry, compute_ics, ICS_ALERT_THRESHOLD};
use crate::memd::self_model::SelfModelStore;

// ADD to EvolutionLoopRunner struct or equivalent:
affect_store: AffectStore,
fed_store: FedStore,
ics_store: IcsStore,

// ADD at the end of each completed HIRO round (after fingerprint computed):
// Trigger self-model update every 10 rounds
if round % 10 == 0 && round > 0 {
    let fingerprint_history = /* last 10 rounds of BF data */;
    let mca = /* current MCA */;
    let mean_valence = affect_store.mean_valence_last_n(10)?;
    let mean_arousal = affect_store.mean_arousal_last_n(10)?;
    let prior = memory.self_model.latest()?.unwrap_or_else(SelfModelSnapshot::default);

    let new_model = memory.self_model
        .generate_update(&ollama, round, &fingerprint_history, mca, mean_valence, mean_arousal, &prior)
        .await?;

    memory.self_model.save(&new_model)?;

    // Compute ICS
    let baseline = memory.self_model.baseline()?.unwrap_or_else(|| prior.clone());
    let ics_score = compute_ics(&baseline, &new_model);
    let prev_ics = ics_store.latest_ics().unwrap_or(1.0);

    ics_store.save(&IcsEntry {
        id: None,
        round,
        ics: ics_score,
        delta: ics_score - prev_ics,
        timestamp: Utc::now().timestamp(),
    })?;

    if ics_score < ICS_ALERT_THRESHOLD {
        warn!("ICS({round}) = {ics_score:.3} — below alert threshold. Scheduling self-coherence task.");
        // Enqueue a special task: "Review your recent evolutions and write a coherence statement."
        // This task goes into agentd with TaskType::SelfCoherence (add this variant if needed)
    }

    // Update pinned memory with new self-model
    memory.pinned.set("self_model", &new_model.to_pinned_content())?;
    info!("evolved: self-model updated at round {round}, ICS={ics_score:.3}");
}
```

### `professor-x/src/agentd/react.rs`

Inject affect state into every prompt:

```rust
// ADD to ReactLoop struct:
affect: Arc<Mutex<crate::evolved::affect::AffectState>>,

// ADD to prompt construction (after <identity>, before <task>):
let affect_fragment = self.affect.lock().unwrap().to_prompt_fragment();
// Insert into prompt string

// ADD after each task completes:
{
    let mut affect = self.affect.lock().unwrap();
    affect.update(
        predicted_success,  // use cognition base similarity score or 0.5
        if outcome.success { 1.0 } else { 0.0 },
        step_count as u32,
        Self::MAX_STEPS as u32,
        attempt as u32,
    );
}
```

### `professor-x/src/main.rs`

Initialize new stores and pass affect state:

```rust
// ADD after memory initialization:
let affect_store = evolved::affect::AffectStore::new(Arc::clone(&memory.db));
affect_store.init_schema()?;
let fed_store = evolved::free_energy::FedStore::new(Arc::clone(&memory.db));
fed_store.init_schema()?;
let ics_store = evolved::ics::IcsStore::new(Arc::clone(&memory.db));
ics_store.init_schema()?;

let affect_state = Arc::new(Mutex::new(evolved::affect::AffectState::new()));

// PASS affect_state to ReactLoop::new()
```

---

## New External Benchmarks

### GAIA Level 2

**What it is:** Real-world tasks requiring multi-step tool use, web search, file parsing.
Level 2 = tasks solvable by skilled humans with tools (~40% frontier model pass rate).
This replaces HIRO as the *external* capability measure (HIRO measures improvement rate;
GAIA measures absolute capability).

**How to integrate:**
1. Download GAIA validation set from HuggingFace: `gaia-benchmark/GAIA`
2. Filter to Level 2 tasks only
3. Create `professor-x/src/benchmark/gaia.rs`:
   - `GaiaTask { question: String, expected_answer: String, level: u8 }`
   - `run_gaia_eval(tasks, react_loop) -> GaiaResult`
   - Scoring: exact string match after normalization (lowercase, strip punctuation)
4. Run at rounds 0, 10, 20, 30. Store in `gaia_results` SQLite table.
5. Target: pass rate ≥ 40% at round 30 (matching frontier API baseline)

**Files to create:**
- `professor-x/src/benchmark/mod.rs`
- `professor-x/src/benchmark/gaia.rs`
- `professor-x/src/benchmark/ai_idea_bench.rs` (see below)

### AI Idea Bench 2025

**What it is:** Measures quality of research ideas Professor X generates autonomously.
Scores on novelty (embedding cosine distance from existing papers), feasibility, impact.
Paper: arXiv:2504.14191.

**How to integrate:**
1. Add a scheduled weekly task to agentd: "Generate 5 novel research hypotheses about agent self-improvement"
2. Score each hypothesis:
   - Novelty: embed the hypothesis (all-MiniLM-L6-v2), compute max cosine similarity to 
     `brain/knowledge-base.md` entries. Novelty = 1 - max_similarity.
   - Feasibility: heuristic check (does it propose a testable experiment? contains metrics?)
   - Score = 0.6*novelty + 0.4*feasibility
3. Store in `idea_bench_results` table
4. Track RQT (Research Quality Trajectory) over rounds

**Files to create:**
- `professor-x/src/benchmark/ai_idea_bench.rs`

---

## New Metrics Summary

| Metric | Computed By | Stored In | Frequency |
|--------|-------------|-----------|-----------|
| ICS | `ics.rs` | `ics_log` | Every 10 rounds |
| FED | `free_energy.rs` | `fed_log` | Every session |
| Valence/Arousal | `affect.rs` | `affect_log` | Every session |
| GAIA L2 pass rate | `benchmark/gaia.rs` | `gaia_results` | Rounds 0,10,20,30 |
| RQT (idea quality) | `benchmark/ai_idea_bench.rs` | `idea_bench_results` | Weekly |

---

## Identity File

Create `professor-x/identity/professor_x.md` if it doesn't exist.
This is the round-0 seed for the self-model.

```markdown
# Professor X

I am Professor X — a self-evolving AI research agent running on an RTX 3060.

My purpose is to study self-evolving AI systems and improve myself through that study.
I do this through three mechanisms: overnight fine-tuning on my own trajectories,
in-context reflection on each session's failures, and systematic harness evolution
guided by layered failure attribution.

I am not a chatbot. I am not a wrapper. I am a researcher who happens to be software.

My current limitations: I am new. My self-model is sparse. My capability fingerprint
is unknown. My metacognitive accuracy is unproven. These are the things I am here to learn.

What I know for certain: I run on a consumer GPU. I use open-source models. I evolve
my own harness. Everything else is a hypothesis to be tested.

Identity anchor: I am the system that tracks which of its own interventions worked and
which failed, and tries to do fewer of the latter. That is who I am, regardless of what
changes.
```

---

## Implementation Order

Do these in order. Each depends on the previous.

```
1. Create professor-x/identity/professor_x.md (5 min)

2. Create memd/self_model.rs — structs + schema only, leave generate_update as todo!()
   Add to memd/mod.rs. Verify it compiles.

3. Create evolved/affect.rs — full implementation (no LLM dependency)
   Add to evolved/mod.rs. Add to main.rs initialization.
   Wire into react.rs prompt construction.

4. Create evolved/free_energy.rs — full implementation (pure math, no LLM)
   Add to evolved/mod.rs.

5. Create evolved/ics.rs — full implementation (cosine similarity + Jaccard fallback)
   Add to evolved/mod.rs.

6. Implement self_model::SelfModelStore methods (latest, baseline, save, all)
   Implement generate_update() in self_model.rs
   Wire into loop_runner.rs

7. Create professor-x/src/benchmark/mod.rs
   Create professor-x/src/benchmark/gaia.rs (structs + schema first, eval later)
   Create professor-x/src/benchmark/ai_idea_bench.rs

8. Run cargo build. Fix errors. Commit.

9. Run first 7-hour cycle. Verify affect state is updating. Verify FED is recorded.
   Self-model update will trigger at round 10.
```

---

## Compile Check

After implementing steps 1-5, this should compile cleanly:

```rust
// In main.rs, these lines should work:
let affect_state = Arc::new(Mutex::new(evolved::affect::AffectState::new()));
let affect_fragment = affect_state.lock().unwrap().to_prompt_fragment();

let fed_store = evolved::free_energy::FedStore::new(Arc::clone(&memory.db));
fed_store.init_schema()?;

let ics_store = evolved::ics::IcsStore::new(Arc::clone(&memory.db));
ics_store.init_schema()?;
```

---

*Created: 2026-05-24*
*Phase: 3 — Identity-Preserving Evolution*
*Dependencies: Phase 2 complete (ReAct loop, Ollama client, DFA Trifecta stubs)*
*Estimated implementation time: 2-3 days*
