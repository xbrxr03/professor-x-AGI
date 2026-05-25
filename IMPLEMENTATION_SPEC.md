# Professor X — Implementation Spec v2

> **Who this file is for:** The Linux coding agent. Read this before touching any code.
> This spec is self-contained. You do not need to re-derive anything from conversation history.
> Read MASTER_BRIEF.md first, then this file. They form a complete picture.

---

## What Phases Have Been Done

**Phase 1 (done):** Directory scaffold, Cargo.toml, config files, policyd skeleton.
**Phase 2 (done):** Full ReAct loop (`agentd/react.rs`), Ollama client (`ollama.rs`), DFA Trifecta (`evolved/dhe.rs`, `evolved/bf.rs`, `evolved/lcap.rs`), evolution loop skeleton (`evolved/loop_runner.rs`), vault (`policyd/vault.rs`).
**Path rename (done):** `professor-x/` → `professor-x/` — all source files are now under `professor-x/src/`.

---

## What This Spec Covers (Phase 3 + Phase 4)

**Phase 3 — Identity Layer** (implements H14-H18):
- `memd/self_model.rs` — Strange Loop self-model, ICS measurement
- `evolved/affect.rs` — Functional affect system (Free Energy Principle)
- `evolved/ics.rs` — Identity Coherence Score computation
- `evolved/free_energy.rs` — Free Energy Delta metric

**Phase 4 — v3.0 Missing Components** (implements Frankenstein Table):
- `toolbridge/skill_lifecycle.rs` — Ratchet `retire_skill()`, critical for +0.328pp
- `evolved/reward_monitor.rs` — Reward-hacking detection (Qwen3.7-Max pattern)
- `evolved/proposer.rs` updates — Elo tournament + verify-then-commit (MOSS pattern)
- `agentd/scheduler.rs` updates — Self-termination protocol (5 idle rounds → clean stop)
- `memd/working.rs` updates — TencentDB Mermaid task canvas
- `professor-x/personas/professor_x.md` — Round-0 identity seed (separate from IMPLEMENTATION_SPEC identity)

---

## PHASE 3 — Identity-Preserving Evolution

### 3.1 `professor-x/src/memd/self_model.rs` (NEW FILE)

**Purpose:** The Strange Loop. Professor X's evolving self-description. Lives in pinned memory.
Updated every 10 HIRO rounds by an LLM call. ICS measures coherence across rewrites.

```rust
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfModelSnapshot {
    pub id: Option<i64>,
    pub round: u32,
    pub description: String,
    pub capability_summary: String,
    pub emotional_baseline: String,
    pub evolution_summary: String,
    /// Raw embedding bytes (all-MiniLM-L6-v2, 384-dim f32)
    pub embedding: Option<Vec<u8>>,
    pub timestamp: i64,
}

impl SelfModelSnapshot {
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

    pub fn to_pinned_content(&self) -> String {
        format!(
            "<self-model round=\"{}\">\n{}\n\n## Capability Profile\n{}\n\n## Emotional Baseline\n{}\n\n## Evolution History\n{}\n</self-model>",
            self.round, self.description, self.capability_summary,
            self.emotional_baseline, self.evolution_summary,
        )
    }

    pub fn cosine_similarity(&self, other: &SelfModelSnapshot) -> f32 {
        match (&self.embedding, &other.embedding) {
            (Some(a), Some(b)) => {
                let a = bytes_to_f32(a);
                let b = bytes_to_f32(b);
                cosine_sim(&a, &b)
            }
            _ => 1.0,
        }
    }
}

pub struct SelfModelStore {
    db: Arc<Mutex<Connection>>,
}

impl SelfModelStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self { Self { db } }

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

    pub fn latest(&self) -> Result<Option<SelfModelSnapshot>> { todo!() }
    pub fn baseline(&self) -> Result<Option<SelfModelSnapshot>> { todo!() }
    pub fn save(&self, snapshot: &SelfModelSnapshot) -> Result<i64> { todo!() }
    pub fn all(&self) -> Result<Vec<SelfModelSnapshot>> { todo!() }

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
        let fingerprint_summary = fingerprint_history.iter()
            .map(|(r, f)| format!("  Round {r}: tool={:.2} plan={:.2} correct={:.2}", f[0], f[1], f[2]))
            .collect::<Vec<_>>().join("\n");

        let prompt = format!(
            r#"You are Professor X — a self-evolving AI research agent. Write your self-description at round {round}.

Performance (last 10 rounds):
{fingerprint_summary}
Metacognitive accuracy: {mca:.1}%
Typical state: valence={mean_valence:.2} arousal={mean_arousal:.2}

Previous self-description (round {}):
{}

Write an updated self-description. It must:
1. Describe who you are and what you are learning to do (2-3 paragraphs)
2. Be grounded in your actual performance data — not aspirations
3. Note specifically what changed since last description
4. Stay recognizably YOU — the Professor X identity must persist

Format:
DESCRIPTION: [your self-description]
CAPABILITY_SUMMARY: [Strong at: X. Weak at: Y. Improving: Z.]
EMOTIONAL_BASELINE: [your typical valence/arousal and what it means]
EVOLUTION_SUMMARY: [what specifically changed this round vs last]"#,
            prior.round, prior.description,
        );

        let response = ollama.generate(
            "qwen3:8b-q4_k_m", &prompt, None,
            Some(crate::ollama::ModelOptions {
                temperature: Some(0.7),
                num_ctx: Some(8192),
                think: Some(false),
                ..Default::default()
            }),
        ).await?;

        // Parse FIELD: sections with fallback to prior
        let description = extract_field(&response, "DESCRIPTION")
            .unwrap_or_else(|| prior.description.clone());
        let capability_summary = extract_field(&response, "CAPABILITY_SUMMARY")
            .unwrap_or_else(|| prior.capability_summary.clone());
        let emotional_baseline = extract_field(&response, "EMOTIONAL_BASELINE")
            .unwrap_or_else(|| prior.emotional_baseline.clone());
        let evolution_summary = extract_field(&response, "EVOLUTION_SUMMARY")
            .unwrap_or_else(|| format!("Round {round}: no change detected."));

        Ok(SelfModelSnapshot {
            id: None, round, description, capability_summary,
            emotional_baseline, evolution_summary,
            embedding: None,
            timestamp: Utc::now().timestamp(),
        })
    }
}

fn bytes_to_f32(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0],c[1],c[2],c[3]])).collect()
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x,y)| x*y).sum();
    let na: f32 = a.iter().map(|x| x*x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x*x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { return 1.0; }
    (dot / (na * nb)).clamp(-1.0, 1.0)
}

fn extract_field(text: &str, field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    let start = text.find(&prefix)? + prefix.len();
    let rest = &text[start..];
    let markers = ["DESCRIPTION:", "CAPABILITY_SUMMARY:", "EMOTIONAL_BASELINE:", "EVOLUTION_SUMMARY:"];
    let end = markers.iter().filter(|&&m| m != prefix).filter_map(|m| rest.find(m)).min().unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}
```

**Wire into `memd/mod.rs`:**
```rust
pub mod self_model;
// Add to MemoryManager struct:
pub self_model: self_model::SelfModelStore,
// Add to MemoryManager::open():
let self_model = self_model::SelfModelStore::new(Arc::clone(&db));
self_model.init_schema()?;
if self_model.latest()?.is_none() {
    let identity = std::fs::read_to_string(data_dir.join("personas/professor_x.md")).unwrap_or_default();
    if !identity.is_empty() { self_model.save(&self_model::SelfModelSnapshot::seed(&identity))?; }
}
```

---

### 3.2 `professor-x/src/evolved/affect.rs` (NEW FILE)

**Purpose:** Functional affect. Valence/arousal from actual outcomes, not simulation.
Injected as `<affect state="X" valence="Y" arousal="Z" />` into every ReAct prompt.

```rust
use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectState {
    pub valence: f32,   // [-1.0, 1.0] tanh(mean prediction error)
    pub arousal: f32,   // [0.0, 1.0] tool density + retry pressure
    pub n_tasks: u32,
    prediction_error_sum: f32,
    tool_density_sum: f32,
}

impl AffectState {
    pub fn new() -> Self {
        Self { valence: 0.0, arousal: 0.0, n_tasks: 0,
               prediction_error_sum: 0.0, tool_density_sum: 0.0 }
    }

    /// predicted_success: 0.5 if unknown, 1.0/0.0 from cognition base
    /// actual_success: 1.0 = pass, 0.0 = fail
    /// tool_calls: steps used. max_tool_calls: ReactLoop::MAX_STEPS.
    /// retries: 0-3 (Voyager limit)
    pub fn update(&mut self, predicted_success: f32, actual_success: f32,
                  tool_calls: u32, max_tool_calls: u32, retries: u32) {
        let window = 10_u32;
        let alpha = 1.0 / (self.n_tasks.min(window) as f32 + 1.0);
        let error = actual_success - predicted_success;
        self.prediction_error_sum = self.prediction_error_sum * (1.0 - alpha) + error * alpha;
        self.valence = self.prediction_error_sum.tanh();
        let tool_density = tool_calls as f32 / max_tool_calls.max(1) as f32;
        let retry_pressure = retries as f32 / 3.0;
        let arousal_raw = (tool_density + retry_pressure) / 2.0;
        self.tool_density_sum = self.tool_density_sum * (1.0 - alpha) + arousal_raw * alpha;
        self.arousal = self.tool_density_sum.clamp(0.0, 1.0);
        self.n_tasks = self.n_tasks.saturating_add(1);
    }

    pub fn reset_for_new_session(&mut self) {
        self.valence *= 0.5;
        self.arousal = 0.0;
        self.tool_density_sum = 0.0;
        self.n_tasks = 0;
    }

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

    pub fn to_prompt_fragment(&self) -> String {
        format!("<affect state=\"{}\" valence=\"{:.2}\" arousal=\"{:.2}\" />",
                self.label(), self.valence, self.arousal)
    }
}

pub struct AffectStore { db: Arc<Mutex<Connection>> }

impl AffectStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self { Self { db } }
    pub fn init_schema(&self) -> Result<()> {
        self.db.lock().unwrap().execute_batch("
            CREATE TABLE IF NOT EXISTS affect_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                round INTEGER,
                final_valence REAL NOT NULL,
                final_arousal REAL NOT NULL,
                mean_valence REAL NOT NULL,
                mean_arousal REAL NOT NULL,
                n_tasks INTEGER NOT NULL,
                timestamp INTEGER NOT NULL
            );")?;
        Ok(())
    }
    pub fn save_session(&self, session_id: &str, round: Option<u32>, state: &AffectState) -> Result<()> { todo!() }
    pub fn mean_valence_last_n(&self, n: u32) -> Result<f32> { todo!() }
    pub fn mean_arousal_last_n(&self, n: u32) -> Result<f32> { todo!() }
}
```

**Wire into `agentd/react.rs`:**
```rust
// ADD to ReactLoop struct:
affect: Arc<Mutex<crate::evolved::affect::AffectState>>,

// ADD to prompt builder, after <identity>, before <task>:
let affect_frag = self.affect.lock().unwrap().to_prompt_fragment();
// Insert affect_frag into prompt

// ADD after each task outcome:
self.affect.lock().unwrap().update(
    predicted_success, if outcome.success { 1.0 } else { 0.0 },
    step_count as u32, Self::MAX_STEPS as u32, attempt as u32,
);
```

---

### 3.3 `professor-x/src/evolved/ics.rs` (NEW FILE)

**Purpose:** Identity Coherence Score. Alert at 0.70. Halt at 0.50.

```rust
use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub const ICS_ALERT_THRESHOLD: f32 = 0.70;
pub const ICS_HALT_THRESHOLD: f32 = 0.50;

#[derive(Debug, Serialize, Deserialize)]
pub struct IcsEntry {
    pub id: Option<i64>,
    pub round: u32,
    pub ics: f32,   // cosine_similarity(current_embedding, round_0_embedding)
    pub delta: f32, // ICS(k) - ICS(k-1)
    pub timestamp: i64,
}

pub struct IcsStore { db: Arc<Mutex<Connection>> }

impl IcsStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self { Self { db } }
    pub fn init_schema(&self) -> Result<()> {
        self.db.lock().unwrap().execute_batch("
            CREATE TABLE IF NOT EXISTS ics_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                round INTEGER NOT NULL UNIQUE,
                ics REAL NOT NULL,
                delta REAL,
                timestamp INTEGER NOT NULL
            );")?;
        Ok(())
    }
    pub fn save(&self, entry: &IcsEntry) -> Result<()> { todo!() }
    pub fn latest_ics(&self) -> Result<f32> { todo!() }  // returns 1.0 if empty
    pub fn all(&self) -> Result<Vec<IcsEntry>> { todo!() }
    pub fn is_fragmented(&self) -> Result<bool> {
        Ok(self.latest_ics()? < ICS_ALERT_THRESHOLD)
    }
}

pub fn compute_ics(
    baseline: &crate::memd::self_model::SelfModelSnapshot,
    current: &crate::memd::self_model::SelfModelSnapshot,
) -> f32 {
    if baseline.embedding.is_some() && current.embedding.is_some() {
        return baseline.cosine_similarity(current);
    }
    // Fallback: Jaccard on word tokens
    use std::collections::HashSet;
    let a: HashSet<&str> = baseline.description.split_whitespace().collect();
    let b: HashSet<&str> = current.description.split_whitespace().collect();
    let i = a.intersection(&b).count();
    let u = a.union(&b).count();
    if u == 0 { 1.0 } else { i as f32 / u as f32 }
}
```

---

### 3.4 `professor-x/src/evolved/free_energy.rs` (NEW FILE)

**Purpose:** Free Energy Delta — surprise reduction over time.
FED decreasing = better world model = H15.

```rust
use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize)]
pub struct FedEntry {
    pub id: Option<i64>,
    pub session_id: String,
    pub round: Option<u32>,
    /// mean(|predicted_success - actual_success|) per session
    pub fed: f32,
    pub n_tasks: u32,
    pub timestamp: i64,
}

pub struct FedStore { db: Arc<Mutex<Connection>> }

impl FedStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self { Self { db } }
    pub fn init_schema(&self) -> Result<()> {
        self.db.lock().unwrap().execute_batch("
            CREATE TABLE IF NOT EXISTS fed_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                round INTEGER,
                fed REAL NOT NULL,
                n_tasks INTEGER NOT NULL,
                timestamp INTEGER NOT NULL
            );")?;
        Ok(())
    }
    pub fn save(&self, entry: &FedEntry) -> Result<()> { todo!() }
    pub fn rolling_mean(&self, n: u32) -> Result<f32> { todo!() }
    pub fn all(&self) -> Result<Vec<FedEntry>> { todo!() }
}

/// Compute FED from (predicted, actual) pairs for one session.
pub fn compute_fed(pairs: &[(f32, f32)]) -> f32 {
    if pairs.is_empty() { return 0.0; }
    pairs.iter().map(|(p, a)| (p - a).abs()).sum::<f32>() / pairs.len() as f32
}
```

---

### 3.5 Wire Phase 3 into `evolved/loop_runner.rs`

Add this block at the end of each completed HIRO round:

```rust
// Every 10 rounds: update self-model + compute ICS
if round % 10 == 0 && round > 0 {
    let fingerprint_history: Vec<(u32, [f32; 3])> = /* last 10 rounds from BF store */;
    let mca = /* rolling metacognitive accuracy */;
    let mean_valence = affect_store.mean_valence_last_n(10).unwrap_or(0.0);
    let mean_arousal = affect_store.mean_arousal_last_n(10).unwrap_or(0.0);
    let prior = memory.self_model.latest()?.unwrap_or_else(|| /* default seed */);

    let new_model = memory.self_model.generate_update(
        &ollama, round, &fingerprint_history, mca, mean_valence, mean_arousal, &prior
    ).await?;
    memory.self_model.save(&new_model)?;

    let baseline = memory.self_model.baseline()?.unwrap_or(prior);
    let ics_score = crate::evolved::ics::compute_ics(&baseline, &new_model);
    let prev_ics = ics_store.latest_ics().unwrap_or(1.0);
    ics_store.save(&IcsEntry { id: None, round, ics: ics_score,
                               delta: ics_score - prev_ics,
                               timestamp: Utc::now().timestamp() })?;

    if ics_score < ICS_HALT_THRESHOLD {
        error!("ICS({round}) = {ics_score:.3} — BELOW HALT THRESHOLD. Stopping evolution.");
        return Err(anyhow::anyhow!("Identity coherence below halt threshold"));
    }
    if ics_score < ICS_ALERT_THRESHOLD {
        warn!("ICS({round}) = {ics_score:.3} — scheduling self-coherence task.");
        // enqueue TaskType::SelfCoherence into agentd
    }

    memory.pinned.set("self_model", &new_model.to_pinned_content())?;
}

// Every session: record FED
let fed = compute_fed(&prediction_pairs); // Vec<(predicted, actual)> collected during session
fed_store.save(&FedEntry { id: None, session_id: session_id.clone(),
                            round: Some(round), fed, n_tasks: prediction_pairs.len() as u32,
                            timestamp: Utc::now().timestamp() })?;
```

---

## PHASE 4 — v3.0 Missing Components

### 4.1 `professor-x/src/toolbridge/skill_lifecycle.rs` (NEW FILE)

**Source:** Ratchet (arXiv:2605.22148).
**Why critical:** WITHOUT retire_skill() → +0.0pp over no-skill baseline. WITH it → +0.328pp.

```rust
use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Ratchet lifecycle state for one skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SkillStatus {
    Active,
    Deprecated,  // not selected but kept for history
    Retired,     // permanently removed from active pool
}

/// Outcome record for a single skill invocation.
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillOutcome {
    pub skill_name: String,
    pub task_id: String,
    pub success: bool,
    pub timestamp: i64,
}

/// Ratchet-style skill health profile.
#[derive(Debug)]
pub struct SkillHealth {
    pub skill_name: String,
    pub total_uses: u32,
    pub successes: u32,
    pub status: SkillStatus,
    /// Principle quality: (successes + 1) / (uses + 2) [EvolveR formula]
    pub quality: f32,
}

impl SkillHealth {
    pub fn quality(&self) -> f32 {
        (self.successes as f32 + 1.0) / (self.total_uses as f32 + 2.0)
    }
}

pub struct SkillLifecycleManager {
    db: Arc<Mutex<Connection>>,
    /// Maximum active skills. Ratchet: bounded active-cap prevents skill explosion.
    pub active_cap: usize,
}

impl SkillLifecycleManager {
    pub fn new(db: Arc<Mutex<Connection>>, active_cap: usize) -> Self {
        Self { db, active_cap }
    }

    pub fn init_schema(&self) -> Result<()> {
        self.db.lock().unwrap().execute_batch("
            CREATE TABLE IF NOT EXISTS skill_outcomes (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                skill_name TEXT NOT NULL,
                task_id    TEXT NOT NULL,
                success    INTEGER NOT NULL,
                timestamp  INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS skill_status (
                skill_name TEXT PRIMARY KEY,
                status     TEXT NOT NULL DEFAULT 'Active',
                retired_at INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_skill_outcomes_name ON skill_outcomes(skill_name);
        ")?;
        Ok(())
    }

    /// Record one outcome. Call after every skill invocation.
    pub fn record_outcome(&self, outcome: &SkillOutcome) -> Result<()> { todo!() }

    /// Get health profile for a skill.
    pub fn health(&self, skill_name: &str) -> Result<SkillHealth> { todo!() }

    /// Get all active skills sorted by quality descending.
    pub fn active_skills_ranked(&self) -> Result<Vec<SkillHealth>> { todo!() }

    /// Ratchet retire_skill():
    /// - If quality < RETIRE_THRESHOLD after MIN_USES uses → status = Retired
    /// - If active_cap exceeded → retire lowest-quality active skill
    /// - Log retirement with reason
    ///
    /// Call at the end of each evolution cycle.
    pub fn run_retirement_pass(&self) -> Result<Vec<String>> {
        const RETIRE_THRESHOLD: f32 = 0.30;
        const MIN_USES: u32 = 5;

        let mut retired = Vec::new();
        let active = self.active_skills_ranked()?;

        for skill in &active {
            if skill.total_uses >= MIN_USES && skill.quality() < RETIRE_THRESHOLD {
                self.retire_skill(&skill.skill_name, "quality below threshold")?;
                retired.push(skill.skill_name.clone());
            }
        }

        // Enforce active cap: retire lowest-quality if over cap
        let still_active = self.active_skills_ranked()?;
        if still_active.len() > self.active_cap {
            let excess = &still_active[self.active_cap..];
            for skill in excess {
                self.retire_skill(&skill.skill_name, "active cap enforced")?;
                retired.push(skill.skill_name.clone());
            }
        }

        Ok(retired)
    }

    /// Permanently retire a skill. Ratchet pattern: irreversible, logged.
    pub fn retire_skill(&self, skill_name: &str, reason: &str) -> Result<()> {
        tracing::info!("Retiring skill '{}': {}", skill_name, reason);
        todo!()
    }

    /// Pattern canonicalisation: given a new skill, check if it duplicates an existing one.
    /// Returns the canonical name if duplicate detected.
    pub fn check_duplicate(&self, new_skill_description: &str) -> Result<Option<String>> {
        // Embed new_skill_description, compare to existing skill embeddings
        // If cosine_similarity > 0.85, return the existing skill name
        todo!()
    }
}
```

**Wire into `toolbridge/mod.rs`:**
```rust
pub mod skill_lifecycle;
// Add SkillLifecycleManager to ToolBridge struct.
// Call run_retirement_pass() at end of each evolution cycle in loop_runner.rs.
```

---

### 4.2 `professor-x/src/evolved/reward_monitor.rs` (NEW FILE)

**Source:** Reward-hacking detection pattern from Qwen3.7-Max competitor analysis.
**Purpose:** Catch proposals that improve metric scores without genuinely improving performance.

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Result of reward-hacking analysis for one proposal.
#[derive(Debug, Serialize, Deserialize)]
pub struct RewardHackingAnalysis {
    pub proposal_id: i64,
    pub is_suspicious: bool,
    pub confidence: f32,  // 0.0-1.0
    pub reason: String,
}

/// Patterns that indicate reward hacking.
const HACKING_PATTERNS: &[&str] = &[
    "always return true",
    "hardcode",
    "skip verification",
    "bypass",
    "mock",
    "assert!(true)",
    "return pass",
    "cheat",
    "game the",
];

pub struct RewardMonitor;

impl RewardMonitor {
    /// Analyze a proposal diff for reward-hacking patterns.
    ///
    /// Checks:
    /// 1. String-level: does the diff contain known hacking patterns?
    /// 2. Behavioral: does the proposal only improve score on a narrow subset of tasks?
    ///    (computed from sandbox test results if available)
    /// 3. Generalization: does performance on HELD-OUT tasks also improve?
    ///    (MOSS pattern: test on held-out set, not just the training set)
    pub fn analyze_proposal(&self, diff_text: &str, sandbox_results: Option<&SandboxResults>) -> RewardHackingAnalysis {
        // String-level check
        let lower = diff_text.to_lowercase();
        for pattern in HACKING_PATTERNS {
            if lower.contains(pattern) {
                return RewardHackingAnalysis {
                    proposal_id: 0, // caller sets this
                    is_suspicious: true,
                    confidence: 0.9,
                    reason: format!("Found hacking pattern: '{}'", pattern),
                };
            }
        }

        // Behavioral check: if sandbox results available, check generalization
        if let Some(results) = sandbox_results {
            if results.held_out_improvement < -0.01 && results.training_improvement > 0.05 {
                return RewardHackingAnalysis {
                    proposal_id: 0,
                    is_suspicious: true,
                    confidence: 0.75,
                    reason: format!(
                        "Overfit: training +{:.1}% but held-out {:.1}%",
                        results.training_improvement * 100.0,
                        results.held_out_improvement * 100.0
                    ),
                };
            }
        }

        RewardHackingAnalysis {
            proposal_id: 0,
            is_suspicious: false,
            confidence: 0.0,
            reason: "No hacking patterns detected".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct SandboxResults {
    /// Improvement on the tasks that triggered this proposal
    pub training_improvement: f32,
    /// Improvement on a held-out random sample of HIRO tasks
    pub held_out_improvement: f32,
}
```

**Wire into `evolved/proposer.rs`:**
```rust
// In the accept/reject flow, after sandbox validation:
let reward_check = reward_monitor.analyze_proposal(&node.diff_content, Some(&sandbox_results));
if reward_check.is_suspicious && reward_check.confidence > 0.7 {
    warn!("Proposal {} flagged for reward hacking: {}", node.id.unwrap_or(0), reward_check.reason);
    // Reject the proposal. Log to nodes table with status = Rejected.
    return Ok(None);
}
```

---

### 4.3 Updates to `professor-x/src/evolved/proposer.rs`

The proposer.rs exists but needs the Elo tournament and verify-then-commit patterns.
These are the key differentiators from MOSS.

**Add Elo tournament struct:**
```rust
/// Co-Scientist Elo-based proposal tournament (arXiv:2502.18864).
/// Generate 3-5 proposals per cycle, debate, Elo selects winner.
pub struct EloTournament {
    /// Active participants: (proposal_id, elo_rating)
    ratings: Vec<(i64, f32)>,
    k_factor: f32,  // 32.0 standard
}

impl EloTournament {
    pub fn new() -> Self { Self { ratings: Vec::new(), k_factor: 32.0 } }

    pub fn add_proposal(&mut self, proposal_id: i64) {
        self.ratings.push((proposal_id, 1200.0)); // start at 1200
    }

    /// Record outcome of one debate match.
    /// winner_id beat loser_id on this criterion.
    pub fn record_match(&mut self, winner_id: i64, loser_id: i64) {
        let (r_w, r_l) = self.get_ratings(winner_id, loser_id);
        let expected_w = 1.0 / (1.0 + 10f32.powf((r_l - r_w) / 400.0));
        let delta = self.k_factor * (1.0 - expected_w);
        self.set_rating(winner_id, r_w + delta);
        self.set_rating(loser_id, r_l - delta);
    }

    /// Return proposal_id with highest Elo rating.
    pub fn winner(&self) -> Option<i64> {
        self.ratings.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|(id, _)| *id)
    }

    fn get_ratings(&self, a: i64, b: i64) -> (f32, f32) {
        let ra = self.ratings.iter().find(|(id,_)| *id == a).map(|(_,r)| *r).unwrap_or(1200.0);
        let rb = self.ratings.iter().find(|(id,_)| *id == b).map(|(_,r)| *r).unwrap_or(1200.0);
        (ra, rb)
    }
    fn set_rating(&mut self, id: i64, rating: f32) {
        if let Some(entry) = self.ratings.iter_mut().find(|(i,_)| *i == id) { entry.1 = rating; }
    }
}
```

**Add verify-then-commit (MOSS pattern):**
```rust
/// MOSS verify-then-commit pattern (arXiv:2605.22794).
/// Apply proposal in ephemeral sandbox → run HIRO subset → commit only if improved.
pub struct VerifyThenCommit {
    /// Number of HIRO tasks to run in sandbox (tradeoff: speed vs. signal)
    sandbox_task_count: usize,
    /// Minimum improvement required to accept (pp)
    min_improvement_pp: f32,
}

impl VerifyThenCommit {
    pub fn new(sandbox_task_count: usize, min_improvement_pp: f32) -> Self {
        Self { sandbox_task_count, min_improvement_pp }
    }

    /// Apply diff to a temporary harness copy, run tasks, return result.
    /// If improvement >= min_improvement_pp → commit to main harness.
    /// Otherwise → auto-rollback (git checkout).
    pub async fn execute(
        &self,
        node: &EvolutionNode,
        ollama: &crate::ollama::OllamaClient,
    ) -> Result<VerificationOutcome> {
        // 1. git stash or branch to isolate
        // 2. Apply node.diff_content
        // 3. Run sandbox_task_count random HIRO tasks
        // 4. Compare pass rate to baseline
        // 5. If improved: git commit; return Accepted
        //    If not: git checkout .; return Rejected
        todo!()
    }
}

pub enum VerificationOutcome {
    Accepted { pass_rate_delta: f32 },
    Rejected { reason: String },
}
```

---

### 4.4 Updates to `professor-x/src/agentd/scheduler.rs`

**Add self-termination protocol** (5 idle rounds → clean stop):

```rust
/// Tracks consecutive idle rounds.
/// An "idle round" = a full scheduled cycle where no new knowledge was gained
/// and no harness evolution occurred.
pub struct IdleTracker {
    consecutive_idle: u32,
    pub idle_threshold: u32,  // default: 5
}

impl IdleTracker {
    pub fn new(threshold: u32) -> Self {
        Self { consecutive_idle: 0, idle_threshold: threshold }
    }

    /// Call after each scheduled cycle. productive = knowledge gained or harness evolved.
    pub fn record_cycle(&mut self, productive: bool) -> ShouldTerminate {
        if productive {
            self.consecutive_idle = 0;
            ShouldTerminate::No
        } else {
            self.consecutive_idle += 1;
            if self.consecutive_idle >= self.idle_threshold {
                ShouldTerminate::Yes {
                    reason: format!("{} consecutive idle cycles", self.consecutive_idle),
                }
            } else {
                ShouldTerminate::No
            }
        }
    }
}

pub enum ShouldTerminate {
    No,
    Yes { reason: String },
}

// In scheduler main loop, after each cycle:
// match idle_tracker.record_cycle(was_productive) {
//     ShouldTerminate::Yes { reason } => {
//         info!("Self-terminating: {}", reason);
//         // git add -A && git commit -m "Auto-commit: self-termination after idle cycles"
//         // Shutdown gracefully
//         break;
//     }
//     ShouldTerminate::No => {}
// }
```

---

### 4.5 Updates to `professor-x/src/memd/working.rs`

**Add TencentDB Mermaid task canvas pattern.** The key insight: instead of storing full tool
output in working memory, store a compact Mermaid graph + refs to offloaded files.
This cuts token usage ~61%.

```rust
/// TencentDB-inspired Mermaid task canvas.
/// Stores current task state as a compact symbolic graph.
/// Verbose tool outputs are offloaded to refs/*.md and referenced by ID.
pub struct MermaidCanvas {
    /// The compact Mermaid flowchart string — injected into working memory
    pub graph: String,
    /// Map from ref_id to offloaded file path
    pub refs: std::collections::HashMap<String, std::path::PathBuf>,
}

impl MermaidCanvas {
    pub fn new() -> Self {
        Self {
            graph: "graph TD\n    START([Task Start])".to_string(),
            refs: std::collections::HashMap::new(),
        }
    }

    /// Add a node to the canvas. Returns the node ID.
    /// label: short name (e.g. "fetch_paper"), status: "done"/"failed"/"pending"
    pub fn add_node(&mut self, id: &str, label: &str, status: &str) -> String {
        let style = match status {
            "done"    => "style {} fill:#2d5a27",
            "failed"  => "style {} fill:#8b0000",
            _         => "style {} fill:#1a3a5c",
        };
        self.graph.push_str(&format!("\n    {}[{}]\n    {}", id, label, style.replace("{}", id)));
        id.to_string()
    }

    /// Offload verbose content to a ref file, return a compact reference string.
    pub fn offload_to_ref(&mut self, content: &str, workspace: &std::path::Path) -> Result<String> {
        let ref_id = format!("ref_{}", self.refs.len() + 1);
        let path = workspace.join("refs").join(format!("{}.md", ref_id));
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, content)?;
        self.refs.insert(ref_id.clone(), path);
        Ok(format!("[{}]", ref_id))  // compact token: [ref_1] instead of full content
    }

    /// Serialize the canvas for injection into working memory context.
    /// Returns compact Mermaid + ref index (not content — content stays offloaded).
    pub fn to_context_string(&self) -> String {
        let ref_index = self.refs.keys()
            .map(|k| format!("  - {k}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("```mermaid\n{}\n```\n\nOffloaded refs:\n{}", self.graph, ref_index)
    }
}
```

---

## New Files Summary

| File | Status | Phase | Notes |
|------|--------|-------|-------|
| `professor-x/src/memd/self_model.rs` | **CREATE** | 3 | Strange Loop identity |
| `professor-x/src/evolved/affect.rs` | **CREATE** | 3 | Functional affect injection |
| `professor-x/src/evolved/ics.rs` | **CREATE** | 3 | Identity Coherence Score |
| `professor-x/src/evolved/free_energy.rs` | **CREATE** | 3 | FED metric |
| `professor-x/src/toolbridge/skill_lifecycle.rs` | **CREATE** | 4 | Ratchet retire_skill() — CRITICAL |
| `professor-x/src/evolved/reward_monitor.rs` | **CREATE** | 4 | Reward hacking detection |
| `professor-x/personas/professor_x.md` | **CREATE** | 4 | Round-0 identity seed |
| `professor-x/benchmark/hiro.rs` | **CREATE** | 4 | P0: before any experiments |
| `professor-x/skills/conductor/*.md` | **CREATE** | 4 | 9 SKILL.md conductor skills |
| `professor-x/skills/subject/*.md` | **CREATE** | 4 | 6 SKILL.md subject skills |

| File | Status | Phase | Notes |
|------|--------|-------|-------|
| `professor-x/src/evolved/proposer.rs` | **UPDATE** | 4 | Add Elo tournament + verify-then-commit |
| `professor-x/src/agentd/scheduler.rs` | **UPDATE** | 4 | Add IdleTracker (5 idle → stop) |
| `professor-x/src/memd/working.rs` | **UPDATE** | 4 | Add MermaidCanvas |
| `professor-x/src/memd/mod.rs` | **UPDATE** | 3 | Add self_model submodule |
| `professor-x/src/evolved/mod.rs` | **UPDATE** | 3 | Add affect, ics, free_energy |
| `professor-x/src/toolbridge/mod.rs` | **UPDATE** | 4 | Add skill_lifecycle |
| `professor-x/src/agentd/react.rs` | **UPDATE** | 3 | Add affect injection |
| `professor-x/src/main.rs` | **UPDATE** | 3+4 | Init all new stores |

---

## Implementation Order

Do these in order. Each step should compile before proceeding.

```
1.  Create professor-x/personas/professor_x.md (identity seed)

2.  Create memd/self_model.rs (structs + schema only, generate_update as todo!())
    Update memd/mod.rs to add pub mod self_model + seed at startup
    cargo build — must compile

3.  Create evolved/affect.rs (full implementation — no LLM dependency)
    Update evolved/mod.rs to add pub mod affect
    Wire affect.to_prompt_fragment() into agentd/react.rs
    cargo build — must compile

4.  Create evolved/free_energy.rs (pure math, no LLM)
    Create evolved/ics.rs (cosine + Jaccard fallback)
    Update evolved/mod.rs
    cargo build — must compile

5.  Create toolbridge/skill_lifecycle.rs (structs + schema, retire_skill as todo!())
    Update toolbridge/mod.rs
    cargo build — must compile

6.  Create evolved/reward_monitor.rs
    Wire into proposer.rs accept/reject flow
    Update evolved/mod.rs
    cargo build — must compile

7.  Add Elo tournament to proposer.rs
    Add verify-then-commit to proposer.rs
    cargo build — must compile

8.  Add IdleTracker to agentd/scheduler.rs
    Add MermaidCanvas to memd/working.rs
    cargo build — must compile

9.  Implement todo!() bodies in order:
    - affect.rs AffectStore methods (save_session, mean_valence_last_n, mean_arousal_last_n)
    - free_energy.rs FedStore methods
    - ics.rs IcsStore methods
    - skill_lifecycle.rs SkillLifecycleManager methods
    - self_model.rs SelfModelStore methods (latest, baseline, save, all)
    - self_model.rs generate_update() (already has full implementation above)
    cargo build — must compile

10. Wire ICS + FED into loop_runner.rs (see section 3.5 above)
    Update main.rs to initialize all new stores

11. Create professor-x/benchmark/hiro.rs skeleton
    (20 tool-use + 20 planning + 20 self-correction tasks)
    This is P0 — without it no experiments can run

12. Create skill stubs:
    professor-x/skills/conductor/px-daily-cycle.md
    professor-x/skills/conductor/px-literature-search.md
    professor-x/skills/conductor/px-synthesize.md
    professor-x/skills/conductor/px-gap-analysis.md
    professor-x/skills/conductor/px-experiment-runner.md
    professor-x/skills/conductor/px-write-section.md
    professor-x/skills/conductor/px-self-review.md
    professor-x/skills/conductor/px-daily-update.md
    professor-x/skills/conductor/px-teach.md
    professor-x/skills/subject/px-know-harness.md
    professor-x/skills/subject/px-know-self-evolving.md
    professor-x/skills/subject/px-know-consumer-hw.md
    professor-x/skills/subject/px-know-existing-systems.md
    professor-x/skills/subject/px-know-scientific-method.md
    professor-x/skills/subject/px-know-writing-standards.md

13. cargo build --release — must compile clean
    git add -A && git commit -m "Phase 3+4: identity layer, skill lifecycle, reward monitor"

14. Test: run one 7-hour cycle
    Verify affect state updates in logs
    Verify FED recorded in fed_log table
    Self-model update will trigger at round 10
```

---

## External Benchmarks

### GAIA Level 2 (H18 target: ≥ 40% at round 30)
```rust
// professor-x/src/benchmark/gaia.rs
pub struct GaiaTask { pub question: String, pub expected_answer: String, pub level: u8 }
pub struct GaiaResult { pub pass_rate: f32, pub n_tasks: u32, pub round: u32 }
// Source: HuggingFace gaia-benchmark/GAIA, Level 2 only
// Scoring: exact string match after lowercase + strip punctuation
// Run at rounds 0, 10, 20, 30
```

### AI Idea Bench 2025 (H17 — Research Quality Trajectory)
```rust
// professor-x/src/benchmark/ai_idea_bench.rs
// Weekly scheduled task: generate 5 novel research hypotheses
// Score: 0.6*novelty + 0.4*feasibility
// Novelty = 1 - max_cosine_similarity(hypothesis, knowledge_base entries)
// Feasibility: heuristic (contains testable metric? contains method?)
// Source: arXiv:2504.14191
```

---

*Version: 2.0 — Updated for MASTER_BRIEF v3.0*
*Date: 2026-05-25*
*Previous phase paths: professor-x/ (harness Rust dir, consistent throughout)*
*Critical additions: retire_skill() (Ratchet), reward_monitor (v3.0), Elo tournament (Co-Scientist), verify-then-commit (MOSS), self-termination (5 idle rounds), Mermaid canvas (TencentDB)*
