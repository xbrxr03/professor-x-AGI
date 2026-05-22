# JARVIS — Architecture Document
> Design-before-code. No .rs files until this document is reviewed and approved.
>
> **For the Linux agent:** Every repo and paper linked below should be cloned/fetched before starting implementation. The repos are the direct sources for the data structures and patterns described here.

---

## Source Material

### Repos to clone

```bash
git clone https://github.com/xbrxr03/clawos                          # Our prototype — policyd source
git clone https://github.com/GAIR-NLP/ASI-Evolve                     # Self-evolution reference (SJTU)
git clone https://github.com/modelscope/AgentEvolver                 # Self-evolving RL reference
git clone https://github.com/K-Dense-AI/scientific-agent-skills      # SKILL.md spec + examples
git clone https://github.com/NousResearch/hermes-agent               # Scheduler + memory schema reference
git clone https://github.com/Gloriaameng/Awesome-Agent-Harness       # Harness paper index
git clone https://github.com/XMUDeepLIT/Awesome-Self-Evolving-Agents # Self-evolving agent index
git clone https://github.com/ai-boost/awesome-harness-engineering    # Harness engineering index
git clone https://github.com/Orchestra-Research/AI-Research-SKILLs  # Additional SKILL.md examples
git clone https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep  # Research automation patterns
git clone https://github.com/Imbad0202/academic-research-skills     # Academic SKILL.md examples
```

### Papers to fetch (arXiv)

**Tier 1 — Core architecture (read in full before touching any component):**

| ID | Title | Link | What it gives JARVIS |
|----|-------|------|----------------------|
| 2604.25850 | Agentic Harness Engineering (AHE) | [arxiv.org/abs/2604.25850](https://arxiv.org/abs/2604.25850) | Harness taxonomy, 3-pillar observability, change manifests |
| 2603.29640 | ASI-Evolve: AI Accelerates AI | [arxiv.org/abs/2603.29640](https://arxiv.org/abs/2603.29640) | Researcher/Engineer/Analyzer loop, cognition base, Node schema |
| 2309.02427 | CoALA: Cognitive Architectures for Language Agents | [arxiv.org/abs/2309.02427](https://arxiv.org/abs/2309.02427) | Memory taxonomy (4 types), action space taxonomy, decision cycle |
| 2305.16291 | Voyager: Open-Ended Embodied Agent | [arxiv.org/abs/2305.16291](https://arxiv.org/abs/2305.16291) | Skill library + verified procedural memory |
| 2303.11366 | Reflexion: Verbal Reinforcement Learning | [arxiv.org/abs/2303.11366](https://arxiv.org/abs/2303.11366) | Self-reflection after failure, bounded memory buffer |
| 2210.03629 | ReAct: Synergizing Reasoning and Acting | [arxiv.org/abs/2210.03629](https://arxiv.org/abs/2210.03629) | Thought/Action/Observation execution trace format |

**Tier 2 — Memory and context (read before implementing memd):**

| ID | Title | Link | What it gives JARVIS |
|----|-------|------|----------------------|
| 2603.07670 | Memory for Autonomous LLM Agents | [arxiv.org/abs/2603.07670](https://arxiv.org/abs/2603.07670) | Write-manage-read loop, multi-signal retrieval scoring |
| 2603.15421 | CLAG: Memory for Small Language Models | [arxiv.org/abs/2603.15421](https://arxiv.org/abs/2603.15421) | Two-stage cluster retrieval, 100-entry cold start |
| 2604.08224 | Externalization in LLM Agents | [arxiv.org/abs/2604.08224](https://arxiv.org/abs/2604.08224) | Why harness > model for memory; Pattern B architecture |
| 2510.16079 | EvolveR: Closed-Loop Self-Evolving QA Agent | [arxiv.org/abs/2510.16079](https://arxiv.org/abs/2510.16079) | Principle quality formula `(success+1)/(use+2)`, self-distillation |

**Tier 3 — Self-evolution taxonomy + SLMs (read before implementing evolved):**

| ID | Title | Link | What it gives JARVIS |
|----|-------|------|----------------------|
| 2507.21046 | Self-Evolving Agents: What/When/How/Where | [arxiv.org/abs/2507.21046](https://arxiv.org/abs/2507.21046) | Confirms harness-level evolution is a literature gap |
| 2508.07407 | Comprehensive Survey of Self-Evolving AI | [arxiv.org/abs/2508.07407](https://arxiv.org/abs/2508.07407) | Four-component framework; confirms gap |
| 2508.16153 | Memento: Agent Optimization Without Weight Updates | [arxiv.org/abs/2508.16153](https://arxiv.org/abs/2508.16153) | Closest prior work to JARVIS's approach — read carefully |
| 2507.19457 | GEPA: Reflective Prompt Evolution Beats RL | [arxiv.org/abs/2507.19457](https://arxiv.org/abs/2507.19457) | Prompt-level evolution as a feasible evolution target |
| 2511.10395 | AgentEvolver | [arxiv.org/abs/2511.10395](https://arxiv.org/abs/2511.10395) | Experience unit format "when to use" + "content" |
| 2506.02153 | Small Language Models are the Future of Agentic AI | [arxiv.org/abs/2506.02153](https://arxiv.org/abs/2506.02153) | Validates qwen2.5:14b-q4; xLAM-2-8B for tool calling |
| 2510.03847 | SLMs for Agentic Systems Survey | [arxiv.org/abs/2510.03847](https://arxiv.org/abs/2510.03847) | vLLM/SGLang serving; JSON Schema validation patterns |

---

## Contents

1. [Design Principles](#1-design-principles)
2. [Deployment Model](#2-deployment-model)
3. [Component Map](#3-component-map)
4. [Data Schemas](#4-data-schemas)
5. [memd — Memory Manager](#5-memd--memory-manager)
6. [toolbridge — Tool Execution Layer](#6-toolbridge--tool-execution-layer)
7. [agentd — Orchestration Engine](#7-agentd--orchestration-engine)
8. [policyd — Security and Audit](#8-policyd--security-and-audit)
9. [evolved — Self-Evolution Loop](#9-evolved--self-evolution-loop)
10. [Hardware Budget](#10-hardware-budget)
11. [Inter-Component Data Flow](#11-inter-component-data-flow)
12. [Design Flags](#12-design-flags)
13. [Build Order](#13-build-order)

---

## 1. Design Principles

Every architectural decision in this document follows from three rules:

**Rule 1 — VRAM is the scarce resource.**
The RTX 3060 has 12GB VRAM. The LLM inference engine (qwen2.5:14b-q4) consumes ~8GB. Everything else shares the remaining 4GB. Every MB saved in the harness is a MB available for KV cache, which is a larger effective context window, which is a smarter agent.

**Rule 2 — Lean and stable beats capable and fragile.**
The system runs 24/7 unattended. A component that crashes once a week is worse than a simpler component that never crashes. Rust is chosen because it gives a single-digit MB runtime and no garbage collector pauses. Every architectural decision that adds reliability beats one that adds capability.

**Rule 3 — The harness is the thesis.**
`evolved` is not a feature. It is the research contribution. Every other component exists to support it. When there is a design tradeoff between `evolved` correctness and anything else, `evolved` wins.

---

## 2. Deployment Model

### One binary, five modules — not five daemons

The brief names components as `memd`, `agentd`, etc. These names describe **modules inside a single Rust binary**, not separate processes.

**Why this matters for the 3060:**
- Separate processes = separate allocators = fragmented memory = wasted RAM
- IPC (sockets, shared memory) between processes adds latency and crash surface
- A single binary is one `cargo build` and one thing to monitor

**Communication model:** Rust `async` channels via tokio.
- `mpsc` (multi-producer, single-consumer): task queues, evolution proposals, approval requests
- `broadcast`: kill switch propagation (one sender, all components listen)
- Direct function calls where the call is synchronous and internal

**The policyd module is the exception.** It wraps every outbound tool call as async middleware. Every call that exits JARVIS's boundary goes through policyd's gate function before it touches the OS.

### Process topology at runtime

```
jarvis (single PID)
  ├── tokio runtime (async executor)
  ├── memd (memory manager, owns SQLite handles)
  ├── toolbridge (tool registry + executor)
  ├── agentd (task graph + scheduler)
  ├── policyd (gates every toolbridge call)
  └── evolved (background loop, reads from agentd outcomes)
```

### What runs outside the binary

- **Ollama**: Separate process, LLM inference. JARVIS talks to Ollama via HTTP (localhost:11434). Primary model: `qwen2.5:14b-q4`. Fallback: `phi4:14b-q4`. No JARVIS code runs inside Ollama.
- **Embedding model** (`all-MiniLM-L6-v2`): Runs via ONNX runtime using the `ort` crate. In-process, CPU only. No Python dependency, ~80MB RAM, no VRAM cost. Same model used by [ASI-Evolve's](https://github.com/GAIR-NLP/ASI-Evolve) cognition store.

---

## 3. Component Map

```
┌──────────────────────────────────────────────────────────────┐
│  JARVIS binary                                                │
│                                                              │
│  ┌─────────┐   context     ┌─────────┐  tasks    ┌───────┐  │
│  │  memd   │◄──injection───│ agentd  │──queue───►│ queue │  │
│  │         │               │         │            └───────┘  │
│  │ pinned  │   retrieval   │ graph   │  outcomes  ┌───────┐  │
│  │ working │◄──────────────│ sched   │──────────►│evolved│  │
│  │ episodic│               │ reflect │            │       │  │
│  │ semantic│               └────┬────┘            │ cogntn│  │
│  │procdural│                    │                 │ nodDB │  │
│  └─────────┘               tool calls             └───────┘  │
│                                 │                             │
│                          ┌──────▼──────┐                     │
│                          │  policyd    │  ← every call gates │
│                          │  (gate)     │    here first        │
│                          └──────┬──────┘                     │
│                                 │                             │
│                          ┌──────▼──────┐                     │
│                          │ toolbridge  │                     │
│                          │ (executor)  │                     │
│                          └──────┬──────┘                     │
└─────────────────────────────────┼────────────────────────────┘
                                  │
              ┌───────────────────┼───────────────────┐
              │                   │                   │
         OS (shell)          Ollama API          File system
         HTTP/TCP          (localhost)           /workspace
```

---

## 4. Data Schemas

These are the canonical data structures. Every component owns some of these. The Rust types will be derived from these schemas. Field names and types are derived directly from the source repos — see links.

### 4.1 Memory types

Taxonomy from [CoALA](https://arxiv.org/abs/2309.02427) (Working / Episodic / Semantic / Procedural), extended with a Pinned layer for JARVIS's identity and goals. Write path from [Memory for LLM Agents](https://arxiv.org/abs/2603.07670). Retrieval from [CLAG](https://arxiv.org/abs/2603.15421). Quality scoring from [EvolveR](https://arxiv.org/abs/2510.16079).

```rust
// Layer 1 — Pinned: identity, goals, permanent facts. Always injected into context.
PinnedMemory {
    entries: Vec<PinnedEntry>,
}
PinnedEntry {
    id: String,          // human-readable key: "identity", "goal-1", etc.
    content: String,
    immutable: bool,     // if true, evolved cannot modify this entry
}

// Layer 2 — Working: current session state. In-process only, not persisted.
WorkingMemory {
    session_id: Uuid,
    context_budget: u32,                    // tokens remaining this session
    active_task_id: Option<Uuid>,
    recent_steps: VecDeque<ExecutionStep>,  // last N Thought/Action/Obs triples
    injected_reflections: Vec<String>,      // Reflexion buffer, max 3 entries
}

// Layer 3 — Episodic: past sessions. SQLite + FAISS.
// Schema derived from Hermes Agent sessions/messages tables (NousResearch/hermes-agent)
// Retrieval scoring from arxiv.org/abs/2603.07670
EpisodicEntry {
    id: Uuid,
    session_id: Uuid,
    task_id: Option<Uuid>,
    timestamp: DateTime<Utc>,
    content: String,
    keywords: Vec<String>,       // SLM-generated at write time
    importance: f32,             // 0.0–1.0, self-assessed
    recency_decay: f32,          // recomputed at read: exp(-λ * days_ago)
    embedding: Vec<f32>,         // 384-dim, all-MiniLM-L6-v2
    cluster_id: Option<u32>,     // CLAG cluster assignment (arxiv.org/abs/2603.15421)
}

// Layer 4 — Semantic: learned concepts, stable knowledge.
// Quality formula from EvolveR (arxiv.org/abs/2510.16079): (success+1)/(use+2)
SemanticEntry {
    id: Uuid,
    content: String,
    source: String,              // "research", "reflection", "skill-distillation"
    keywords: Vec<String>,
    embedding: Vec<f32>,         // 384-dim
    cluster_id: Option<u32>,
    quality: f32,                // (success_count+1) / (use_count+2)
    use_count: u32,
    success_count: u32,
    created_at: DateTime<Utc>,
    last_accessed: DateTime<Utc>,
}

// Layer 5 — Procedural: verified skills. Schema from Voyager (arxiv.org/abs/2305.16291).
// Stored as SKILL.md-compatible bodies indexed by embedding (K-Dense-AI/scientific-agent-skills)
ProceduralEntry {
    id: Uuid,
    name: String,
    description: String,
    skill_body: String,          // full SKILL.md body or code block
    embedding: Vec<f32>,         // 384-dim, indexed on description
    verified: bool,
    verification_score: f32,
    times_used: u32,
    times_succeeded: u32,
    created_at: DateTime<Utc>,
    source_task_id: Option<Uuid>,
}
```

### 4.2 Execution types (agentd)

Execution trace format from [ReAct](https://arxiv.org/abs/2210.03629). Reflection buffer from [Reflexion](https://arxiv.org/abs/2303.11366). Task node structure informed by [CoALA action space taxonomy](https://arxiv.org/abs/2309.02427). Scheduler schema from [Hermes Agent](https://github.com/NousResearch/hermes-agent) (`~/.hermes/cron/jobs.json`).

```rust
// Atomic execution unit — ReAct Thought/Action/Observation triple
// Ref: arxiv.org/abs/2210.03629, Algorithm 1, format: Thought N: / Action N: / Observation N:
ExecutionStep {
    index: u32,                  // 1-indexed per task
    thought: String,             // internal reasoning before action
    action: Action,
    observation: Observation,
    timestamp: DateTime<Utc>,
}

Action {
    tool_name: String,
    params: serde_json::Value,   // JSON Schema validated before dispatch
    risk_score: u8,              // 0–100, looked up from policyd risk table
}

Observation {
    success: bool,
    output: String,
    error: Option<String>,
    tokens_used: u32,
    execution_ms: u64,
}

// Task node in the agentd execution DAG
TaskNode {
    id: Uuid,
    description: String,
    task_type: TaskType,         // Research | Skill | Evolution | Scheduled | UserRequest
    status: TaskStatus,          // Pending | Running | Complete | Failed | Blocked | Cancelled
    priority: u8,                // 0–255
    parent_ids: Vec<Uuid>,       // dependency edges (wait for these to complete first)
    child_ids: Vec<Uuid>,
    steps: Vec<ExecutionStep>,
    reflections: VecDeque<String>,  // Reflexion buffer — max 3, oldest evicted
    attempt_count: u8,
    max_attempts: u8,            // default 4 (from Voyager 4-round timeout pattern)
    scheduled_at: Option<DateTime<Utc>>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    outcome_score: Option<f32>,  // 0.0–1.0
}

// Scheduled job record — schema from Hermes Agent (NousResearch/hermes-agent)
// Source: ~/.hermes/cron/jobs.json in the hermes-agent repo
CronJob {
    id: String,
    name: String,
    prompt: String,
    schedule: ScheduleSpec,      // Once | Interval | Cron
    next_run_at: DateTime<Utc>,
    enabled: bool,
    state: JobState,             // Scheduled | Paused | Completed | Error
    repeat_limit: Option<u32>,
    repeat_completed: u32,
    last_run_at: Option<DateTime<Utc>>,
    last_status: Option<String>,
    created_at: DateTime<Utc>,
}
```

### 4.3 Security types (policyd)

Risk scoring and validation pipeline from [ClawOS](https://github.com/xbrxr03/clawos) (`policyd/service.py`). Merkle chaining is designed here from scratch — ClawOS claims it in docs but the code does plain SQLite append. JARVIS actually implements it.

```rust
// Every tool call produces one of these, win or lose
// prev_hash field = SHA-256 Merkle chain (not in ClawOS — added here)
AuditEntry {
    id: Uuid,
    prev_hash: [u8; 32],         // SHA-256 of serialized previous entry
    timestamp: DateTime<Utc>,
    session_id: Uuid,
    task_id: Option<Uuid>,
    tool: String,
    params_hash: [u8; 32],       // SHA-256 of params (params NOT stored in log)
    risk_score: u8,
    decision: Decision,
    reason: String,
    execution_ms: Option<u64>,
}

Decision { Allow, Deny, PendingApproval }

// Per-session or per-skill permission scope
// granted_tools wire-connected to SKILL.md `allowed-tools` field at skill load time
PermissionScope {
    granted_tools: Vec<String>,
    blocked_paths: Vec<String>,
    allowed_url_schemes: Vec<String>,   // default: ["http", "https"]
    blocked_url_patterns: Vec<String>,  // private IPs, metadata endpoints
    max_risk_score: u8,
    approval_threshold: u8,     // risk >= this → queued for approval (default 50)
    credential_names: Vec<String>,
}

// Tool risk scores — ported from ClawOS policyd/service.py, extended
// fs.read        = 10   (auto-allow)
// http.get       = 15   (auto-allow)
// http.post      = 30   (auto-allow with logging)
// fs.write       = 45   (auto-allow with logging)
// shell.run      = 60   (requires approval or explicit grant)
// fs.delete      = 70   (requires approval)
// net.outbound   = 75   (requires approval)
// shell.elevated = 90   (requires approval, hard limit)
// harness.modify = 85   (requires approval + evolution node)
ApprovalRequest {
    id: Uuid,
    tool: String,
    params_summary: String,      // truncated human-readable, NOT the actual params
    risk_score: u8,
    requested_at: DateTime<Utc>,
    timeout_secs: u64,           // configurable, default 300s (5 min — not ClawOS's 5s)
    auto_decision: Decision,     // what happens on timeout: Deny (default)
}
```

### 4.4 Evolution types (evolved)

Node schema from [ASI-Evolve](https://github.com/GAIR-NLP/ASI-Evolve) (`utils/structures.py`, `Node` dataclass). Change manifest from [AHE](https://arxiv.org/abs/2604.25850) (Algorithm 1, Decision Observability pillar). Quality scoring from [EvolveR](https://arxiv.org/abs/2510.16079). UCB1 sampling from ASI-Evolve config (`ucb1_c: 1.414`). Diff format from ASI-Evolve Researcher (`<<<< SEARCH / ==== / >>>> REPLACE`).

```rust
// One candidate in the evolution history
// Directly mirrors ASI-Evolve's Node dataclass (utils/structures.py)
EvolutionNode {
    id: u64,                             // sequential int (ASI-Evolve uses int IDs)
    created_at: DateTime<Utc>,
    parent_ids: Vec<u64>,                // multi-parent supported (crossover)
    motivation: String,                  // LLM rationale — required field in ASI-Evolve
    target_component: HarnessComponent,
    diff: String,                        // <<<< SEARCH / ==== / >>>> REPLACE format
    results: serde_json::Value,          // evaluation metrics
    analysis: String,                    // Analyzer's distilled lesson
    manifest: ChangeManifest,            // AHE falsifiable contract
    score: f32,
    visit_count: u32,
    status: NodeStatus,
}

NodeStatus { Proposed, Testing, Accepted, Rejected, RolledBack }

// AHE change manifest — every evolution proposal must include this
// Source: AHE paper (arxiv.org/abs/2604.25850), Section 3.3 Decision Observability
ChangeManifest {
    evidence_cited: Vec<String>,         // refs to episodic/semantic entry IDs
    root_cause: String,
    fix_description: String,
    predicted_fixes: Vec<String>,        // task types / IDs expected to improve
    predicted_regressions: Vec<String>,  // what might break
    verification_status: VerificationStatus,
    verified_at: Option<DateTime<Utc>>,
}

VerificationStatus { Pending, Confirmed, Rejected }

// What the harness can evolve — AHE 7-component taxonomy
// Source: AHE paper Table 1 (arxiv.org/abs/2604.25850)
HarnessComponent {
    SystemPrompt,
    ToolDescription(String),    // tool name
    SkillDefinition(String),    // skill name
    HarnessConfig,              // jarvis.toml keys
    ProceduralMemory,           // skill library add/prune
    Middleware,                 // agentd hooks (human-approval only)
}

// Cognition item — mirrors ASI-Evolve CognitionItem dataclass (utils/structures.py)
// FAISS index: all-MiniLM-L6-v2, 384-dim, inner-product similarity
CognitionItem {
    id: Uuid,
    content: String,
    source: String,                  // "paper", "experiment", "reflection"
    keywords: Vec<String>,
    quality: f32,                    // (success_count+1)/(use_count+2) — EvolveR formula
    use_count: u32,
    success_count: u32,
    embedding: Vec<f32>,             // 384-dim
    created_at: DateTime<Utc>,
}
```

---

## 5. memd — Memory Manager

**Source papers:** [CoALA](https://arxiv.org/abs/2309.02427) · [Memory for LLM Agents](https://arxiv.org/abs/2603.07670) · [CLAG](https://arxiv.org/abs/2603.15421) · [EvolveR](https://arxiv.org/abs/2510.16079) · [Externalization](https://arxiv.org/abs/2604.08224)
**Source repos:** [Hermes Agent](https://github.com/NousResearch/hermes-agent) (SQLite schema, WAL mode, write semantics) · [ASI-Evolve](https://github.com/GAIR-NLP/ASI-Evolve) (cognition store schema)

### Storage layout

```
~/.jarvis/
  state.db          ← SQLite: all structured data (WAL mode)
  embeddings/
    episodic.faiss  ← FAISS flat index, 384-dim
    semantic.faiss
    procedural.faiss
    clusters.json   ← CLAG cluster profiles
  vault.enc         ← credential vault (AES-256-GCM)
  vault.key         ← key file (chmod 600, never in context)
```

### SQLite schema

Schema management strategy from [Hermes Agent](https://github.com/NousResearch/hermes-agent): `SCHEMA_SQL` is the Rust source-of-truth. At startup, memd runs `ALTER TABLE ADD COLUMN` for any missing columns. No migration files. WAL mode (falls back to DELETE journal on NFS/SMB). Write: `BEGIN IMMEDIATE`, 15-attempt retry, 20–150ms random jitter.

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    started_at TEXT,
    ended_at TEXT,
    model TEXT,
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    tool_call_count INTEGER DEFAULT 0,
    end_reason TEXT,
    parent_session_id TEXT          -- lineage for compression-split sessions
);

CREATE TABLE episodic (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    task_id TEXT,
    timestamp TEXT,
    content TEXT,
    keywords TEXT,                  -- JSON array, SLM-generated
    importance REAL,
    embedding_id INTEGER,           -- row ID in episodic.faiss
    cluster_id INTEGER
);
CREATE VIRTUAL TABLE episodic_fts USING fts5(content, keywords, content='episodic');

CREATE TABLE semantic (
    id TEXT PRIMARY KEY,
    content TEXT,
    source TEXT,
    keywords TEXT,
    quality REAL,
    use_count INTEGER DEFAULT 0,
    success_count INTEGER DEFAULT 0,
    embedding_id INTEGER,
    cluster_id INTEGER,
    created_at TEXT,
    last_accessed TEXT
);

CREATE TABLE procedural (
    id TEXT PRIMARY KEY,
    name TEXT UNIQUE,
    description TEXT,
    skill_body TEXT,
    verified INTEGER,
    verification_score REAL,
    times_used INTEGER DEFAULT 0,
    times_succeeded INTEGER DEFAULT 0,
    embedding_id INTEGER,
    created_at TEXT
);

CREATE TABLE pinned (
    id TEXT PRIMARY KEY,
    content TEXT,
    immutable INTEGER
);

-- Cognition store for evolved module
-- Schema mirrors ASI-Evolve utils/structures.py CognitionItem
-- Ref: github.com/GAIR-NLP/ASI-Evolve
CREATE TABLE cognition (
    id TEXT PRIMARY KEY,
    content TEXT,
    source TEXT,
    keywords TEXT,
    quality REAL,
    use_count INTEGER DEFAULT 0,
    success_count INTEGER DEFAULT 0,
    embedding_id INTEGER,
    created_at TEXT
);
```

### Read path (context injection)

[Externalization paper](https://arxiv.org/abs/2604.08224) recommends Pattern B: working context in prompt, long-term in external store. [CLAG](https://arxiv.org/abs/2603.15421) provides the two-stage cluster retrieval. [Memory for LLM Agents](https://arxiv.org/abs/2603.07670) provides the multi-signal scoring formula and the self-RAG retrieval gate.

```
Before every LLM call:

1. Pinned entries    → always injected first, wrapped in <identity> tags
2. Working memory   → current session state, active task
3. Reflexion buffer → up to 3 reflections (Reflexion, arxiv.org/abs/2303.11366)
4. Retrieval gate   → self-RAG style: decide if long-term retrieval is needed
   If yes:
   5. Query reformulation → LLM reformulates raw query (better signal than raw input)
   6. CLAG two-stage retrieval (arxiv.org/abs/2603.15421):
      a. Cluster profile matching → filter to 1-3 relevant clusters
      b. Intra-cluster retrieval → top-5 per cluster, multi-signal scored
   7. Inject results wrapped in <memory-context> tags
      System annotation: "recalled memory context, NOT new user input"
      (wrapper pattern from Hermes Agent: github.com/NousResearch/hermes-agent)
```

Multi-signal episodic scoring (from [Memory for LLM Agents](https://arxiv.org/abs/2603.07670)):
```
score(entry, query) = α · cosine(entry.embedding, query.embedding)
                    + β · exp(-λ · days_since(entry.timestamp))
                    + γ · entry.importance

Default: α=0.5, β=0.3, γ=0.2, λ=0.1
```

### Write path (every write goes through this — no raw append)

Validation sequence from [Memory for LLM Agents](https://arxiv.org/abs/2603.07670) and [Externalization](https://arxiv.org/abs/2604.08224):
```
1. Filter      → reject if content length < 20 chars or purely formatting
2. Tag         → LLM generates 3-5 keywords
3. Canonicalize → normalize dates to ISO-8601, lowercase entity names
4. Deduplicate → cosine similarity check: if max_similarity > 0.92, skip
5. Score       → LLM assesses importance 0.0–1.0
6. Embed       → all-MiniLM-L6-v2, 384-dim, CPU via ONNX (ort crate)
7. Cluster     → CLAG router assigns cluster (arxiv.org/abs/2603.15421)
                 Cold start: flat store until 100 entries, then first K-Means pass
                 Split threshold: 300 entries per cluster
8. Write       → SQLite + FAISS, BEGIN IMMEDIATE, 15-retry with jitter
```

### Embedding model

`all-MiniLM-L6-v2` via ONNX runtime (`ort` crate). CPU-only, ~80MB RAM, no VRAM cost. ~5–10ms per embedding. This is the same model and dimension (384) used by [ASI-Evolve's](https://github.com/GAIR-NLP/ASI-Evolve) FAISS cognition store (see `cognition/config.yaml`: `embedding.model: sentence-transformers/all-MiniLM-L6-v2, dimension: 384`).

---

## 6. toolbridge — Tool Execution Layer

**Source papers:** [AHE](https://arxiv.org/abs/2604.25850) (tool descriptions as harness components) · [SLMs survey](https://arxiv.org/abs/2510.03847) (JSON Schema validation, type-safe registries) · [SLMs future](https://arxiv.org/abs/2506.02153) (xLAM-2-8B for tool calling)
**Source repos:** [K-Dense-AI/scientific-agent-skills](https://github.com/K-Dense-AI/scientific-agent-skills) (SKILL.md spec) · [ClawOS](https://github.com/xbrxr03/clawos) (risk scoring, URL validation)

### Tool manifest

```rust
ToolManifest {
    name: String,                    // matches SKILL.md `name` field exactly
    description: String,             // used by LLM for tool selection
    input_schema: serde_json::Value, // JSON Schema, validated before every dispatch
    risk_score: u8,                  // from policyd risk table
    timeout_ms: u64,
    cache_ttl_ms: Option<u64>,
    allowed_tools: Option<Vec<String>>,  // from SKILL.md `allowed-tools` field
    compatibility: Option<String>,       // from SKILL.md `compatibility` field
    skill_path: Option<PathBuf>,
}
```

### SKILL.md compatibility

The [K-Dense-AI/scientific-agent-skills](https://github.com/K-Dense-AI/scientific-agent-skills) repo is the canonical SKILL.md spec. Clone it and read the README before implementing the parser. Key validation rules:

- **Name regex:** `^[a-z0-9]([a-z0-9-]*[a-z0-9])?$` — max 64 chars, no consecutive hyphens, must match directory name exactly
- **Required fields:** `name`, `description`
- **Optional fields:** `license`, `compatibility`, `metadata` (open `Dict[str, str]`), `allowed-tools`
- **JARVIS extensions go in `metadata`:** `metadata.jarvis-version`, `metadata.min-harness-version`, `metadata.requires` (dependency list — not in the spec but needed)

**3-tier progressive disclosure** (discovered from [K-Dense-AI/scientific-agent-skills](https://github.com/K-Dense-AI/scientific-agent-skills) README):
```
Tier 1 — Startup (~100 tokens per skill):
  Parse name + description from all skills/ SKILL.md files
  Register in ToolRegistry
  Check compatibility field against current environment
  Wire allowed-tools → policyd PermissionScope.granted_tools

Tier 2 — On activation (<5000 tokens):
  Load full SKILL.md body when LLM selects this skill
  Parse script references, template variables

Tier 3 — On demand:
  Load scripts/, references/, assets/ only when referenced in body
```

### Tool invocation sequence

Every tool call: `agentd → toolbridge → policyd.gate() → toolbridge.execute() → Observation`

```
1. agentd emits Action { tool_name, params }
2. toolbridge validates params against JSON Schema → reject if invalid
3. policyd.gate(tool, params, session_scope) → Decision
   - Deny: return Observation { success: false, error: "policy denied" }
   - PendingApproval: block on approval channel (timeout → auto-Deny at 300s)
   - Allow: continue
4. toolbridge executes
5. Return Observation to agentd
6. policyd writes AuditEntry (always, including denials)
```

**URL validation** (ported from [ClawOS](https://github.com/xbrxr03/clawos) `policyd/service.py`): scheme must be http/https, no private IP ranges (10.x, 172.16-31.x, 192.168.x, 127.x), no cloud metadata endpoints (169.254.169.254, metadata.google.internal), no embedded credentials.

**Sandboxing — Phase 1:** Path restrictions + URL blocklist + operation risk scoring (ClawOS model). **Phase 2:** seccomp/landlock Linux syscall filtering via `seccompiler` or `landlock` crates.

---

## 7. agentd — Orchestration Engine

**Source papers:** [ReAct](https://arxiv.org/abs/2210.03629) (Thought/Action/Observation loop) · [Reflexion](https://arxiv.org/abs/2303.11366) (reflection after failure) · [CoALA](https://arxiv.org/abs/2309.02427) (propose-evaluate-select cycle) · [Voyager](https://arxiv.org/abs/2305.16291) (4-round timeout)
**Source repos:** [Hermes Agent](https://github.com/NousResearch/hermes-agent) (cron scheduler, `advance_next_run()` crash safety pattern)

### Execution loop

Format follows [ReAct](https://arxiv.org/abs/2210.03629) exactly — numbered `Thought N:` / `Action N:` / `Observation N:` triples:

```
while not done and attempts < max_attempts (default 4):
  1. Build prompt:
     [pinned context]
     [working memory]
     [reflections if any]
     [retrieved memory if retrieval gate says yes]
     [task description]
     [previous steps this attempt]
  2. Call Ollama → get next Thought + Action
  3. Dispatch Action → toolbridge → Observation
  4. Append ExecutionStep(thought, action, observation)
  5. Evaluate completion (rule-based where possible, LLM for open-ended)
  6. If complete: record outcome → evolved
  7. If failed:
     - Reflexion module generates reflection (arxiv.org/abs/2303.11366)
     - Append to reflections buffer (max 3, oldest evicted)
     - Increment attempt_count → loop
  8. If attempts >= max: mark Failed → evolved
```

### Reflexion module

Source: [Reflexion paper](https://arxiv.org/abs/2303.11366), Algorithm 1. The Self-Reflector module generates verbal feedback after failure. Prompt skeleton:

```
You attempted the following task and failed.
Task: {description}
Your steps:
{numbered Thought/Action/Observation list}
Previous reflections: {reflections buffer or "none"}

In 2-4 sentences: what went wrong, and what will you do differently next attempt?
```

Output appended to `task.reflections` (VecDeque, max 3). Injected into context prefix for next attempt.

### Scheduler

Based on [Hermes Agent](https://github.com/NousResearch/hermes-agent) cron scheduler. Key invariant: **`advance_next_run()` is called before execution, under file lock.** Crash safety = at-most-once semantics.

```
tick() every 60 seconds:
  1. Acquire file lock on cron/jobs.db
  2. SELECT jobs WHERE next_run_at <= now()
  3. For each due job:
     a. advance_next_run() — write new next_run_at FIRST (crash safe)
     b. Release lock
     c. Submit TaskNode to queue
  4. Stale detection: if job missed by > 1 period, fast-forward (no burst-fire)
```

Schedule types (from Hermes Agent `jobs.json` schema): one-shot datetime/duration, `"every Xm/Xh"` interval, cron string `"0 9 * * *"`.

### Professor X daily cycle

The 7-hour cycle is a set of cron jobs registered at startup — not special-cased in code:

```
06:00 → task: "Generate morning brief (Telegram + Discord + X post #1)"
07:00 → task: "Deep reading and synthesis — call px-synthesize skill"
09:00 → task: "Writing session — call px-write-section skill"
11:00 → task: "Building / experimenting — call px-experiment-runner skill"
13:00 → task: "Reflection and self-review — call px-self-review skill"
14:00 → task: "Daily GitHub commit — call px-daily-update skill"
18:00 → task: "Evening post (X + Discord) — call px-teach skill"
```

Each task uses Professor X's SKILL.md conductor skills (defined in `jarvis/skills/conductor/`).

---

## 8. policyd — Security and Audit

**Source repos:** [ClawOS](https://github.com/xbrxr03/clawos) `policyd/service.py` — risk scoring, URL blocklist, approval queue, hook circuit-breaker, prompt injection scanner. All ported and extended. Merkle chaining is implemented from scratch (ClawOS does not implement it despite the claim).

### Gate function

```
gate(tool_name, params, session_scope) → Decision:

1. granted_tools check → Deny("not granted") if tool absent
2. blocked_paths check → Deny("blocked path") if params reference blocked paths
3. Workspace boundary → Path.resolve() must stay inside /workspace
4. URL safety (HTTP tools):
   - Scheme: http/https only
   - No private IPs: 10.x, 172.16-31.x, 192.168.x, 127.x, ::1
   - No metadata endpoints: 169.254.169.254, metadata.google.internal
   - No embedded credentials
5. Prompt injection scan — severity 0–10; >= 8 → Deny("injection detected")
   (severity scoring from ClawOS policyd/service.py)
6. GPU guard — defer non-essential calls if VRAM usage critical
7. Risk score routing:
   risk < approval_threshold → Allow
   risk >= approval_threshold → PendingApproval (notify via Telegram/Discord)
   timeout (300s default) → auto-Deny
8. Hook pre-execution — circuit-breaker disables hook after 3 consecutive failures
   (circuit-breaker from ClawOS HookRegistry)
9. Write AuditEntry to SQLite (always)
```

### Merkle-chained audit log

Every AuditEntry includes `prev_hash: [u8; 32]`:

```
entry_bytes = serialize(entry_without_prev_hash field)
entry.prev_hash = SHA256(bytes_of_previous_entry)
Genesis: prev_hash = [0u8; 32]
```

Verification: `verify_chain() → bool` walks all entries in timestamp order, recomputes hashes. Called at JARVIS startup. Any mismatch means the log has been tampered with.

**Why this matters for the thesis:** [ClawOS](https://github.com/xbrxr03/clawos) claimed Merkle chaining as a competitive moat but the `policyd/service.py` source shows plain SQLite append with no hashing. JARVIS actually builds this. It's part of the paper's contribution: a real tamper-evident audit trail for an autonomous agent on consumer hardware.

### Credential vault

Storage: `~/.jarvis/vault.enc`. Encryption: AES-256-GCM (`aes-gcm` crate). Key: `~/.jarvis/vault.key`, chmod 600. Injection: credentials only reach subprocess via `Command::env(key, value)` at exec boundary. Never appear in LLM prompts, audit logs, or evolution diffs.

### Kill switch

```rust
let cancel = CancellationToken::new();
// Each component receives cancel.child_token()
// Triggers:
//   SIGUSR1 signal → cancel.cancel()
//   POST /kill API → cancel.cancel()
//   Critical violation in policyd → cancel.cancel()
```

---

## 9. evolved — Self-Evolution Loop

**This is the thesis component.**

**Source papers:** [ASI-Evolve](https://arxiv.org/abs/2603.29640) (Researcher/Engineer/Analyzer loop, Node + CognitionItem schemas, UCB1) · [AHE](https://arxiv.org/abs/2604.25850) (change manifests, falsifiable contracts, component observability) · [EvolveR](https://arxiv.org/abs/2510.16079) (quality scoring) · [Reflexion](https://arxiv.org/abs/2303.11366) (verbal reflection) · [Memento](https://arxiv.org/abs/2508.16153) (agent-level optimization without weight updates — closest prior work) · [GEPA](https://arxiv.org/abs/2507.19457) (prompt evolution as RL alternative)
**Source repos:** [ASI-Evolve](https://github.com/GAIR-NLP/ASI-Evolve) (`utils/structures.py`, `pipeline/`, `config.yaml`)

### What JARVIS can evolve

Based on [AHE's 7-component harness taxonomy](https://arxiv.org/abs/2604.25850) and [ClawOS's](https://github.com/xbrxr03/clawos) security precedent:

| Component | Change Type | Autonomy |
|-----------|-------------|----------|
| System prompt | Text edit | Semi-autonomous (Professor X approves) |
| Tool descriptions (YAML) | Text edit | Semi-autonomous |
| Skill definitions (SKILL.md) | Text edit + new files | Semi-autonomous |
| Harness config (jarvis.toml) | Key-value edits | Semi-autonomous |
| Procedural memory | Add/prune skills | Autonomous (low risk) |
| Middleware/hooks | Code edit | Human approval required |
| Core Rust modules (policyd, memd) | Code edit | Human approval required — never autonomous |

### Evolution cycle

Runs every `evolution_cycle_hours` (default: 1). Triggered by scheduler.

**Phase 1 — Learn (Researcher)**

Directly mirrors [ASI-Evolve's](https://github.com/GAIR-NLP/ASI-Evolve) pipeline step 1–2:
```
1. Query cognition store: top-k=5 most relevant CognitionItems
   (semantic FAISS search on current task domain + recent failures)
2. Sample 3 EvolutionNodes from node DB via UCB1 (c=1.414, from ASI-Evolve config.yaml)
   UCB1_score = normalized(score) + 1.414 * sqrt(ln(N_total) / visit_count)
   Unvisited nodes → infinite priority
3. Collect recent outcomes from agentd (last N completed/failed tasks)
4. Identify failure patterns: what task types are failing? what's the mode?
```

**Phase 2 — Design (Researcher continued)**

LLM prompt: cognition items + sampled nodes + failure patterns → EvolutionNode proposal including ChangeManifest (required by [AHE Decision Observability](https://arxiv.org/abs/2604.25850)).

Diff format from [ASI-Evolve](https://github.com/GAIR-NLP/ASI-Evolve) (`pipeline/researcher.py`, `diff_pattern` config key):
```
<<<<<<< SEARCH
[exact text to replace]
=======
[replacement text]
>>>>>>> REPLACE
```

**Phase 3 — Experiment (Engineer)**

From [ASI-Evolve Engineer module](https://github.com/GAIR-NLP/ASI-Evolve) — timeout 1800s, structured results:
```
1. Apply diff to target component file (git-tracked harness/ directory)
2. Run verification pass:
   - Skill changes: execute on a test task from recent history
   - Prompt changes: run 3 representative queries, score outputs
   - Config changes: restart affected component, verify health check
3. Record results in EvolutionNode.results
4. Catastrophic failure → immediate git revert
```

**Phase 4 — Analyze (Analyzer)**

From [ASI-Evolve Analyzer module](https://github.com/GAIR-NLP/ASI-Evolve) (`pipeline/analyzer.py`):
```
Input: EvolutionNode + full experimental outputs
Output:
  1. analysis: distilled lesson → written to EvolutionNode.analysis
  2. new CognitionItem: lesson → written to cognition store
  3. ChangeManifest.verification_status updated
  4. Node score updated

Quality update (EvolveR formula, arxiv.org/abs/2510.16079):
  quality = (success_count + 1) / (use_count + 2)
```

**Phase 5 — Verify (start of next cycle, from [AHE](https://arxiv.org/abs/2604.25850))**

```
At start of each cycle, verify previous cycle's manifest:
  - Intersect predicted_fixes with actual improved tasks
  - Intersect predicted_regressions with actual degraded tasks
  - Predictions wrong: mark node RolledBack, git revert
  - Update CognitionItem quality scores
```

### Version control for harness files

All evolvable components live in `jarvis/harness/` under git:
```
jarvis/harness/
  system_prompt.md
  tool_descriptions/   ← *.yaml
  skills/              ← *.md  (SKILL.md format)
  config/              ← jarvis.toml
  middleware/          ← *.rs  (human-review-only)
```

Every evolution cycle commits. Rollback = `git revert HEAD`. Full history = full evolution record.

This satisfies [AHE Component Observability](https://arxiv.org/abs/2604.25850): "each failure pattern maps to a single component class, every pass-rate change localizable to one file."

### Cognition base initialization

Professor X starts with ~100–150 pre-seeded CognitionItems (matching [ASI-Evolve's](https://github.com/GAIR-NLP/ASI-Evolve) documented scale). Seeded from:
- Key claims extracted from the 15 papers in this document
- JARVIS's own design decisions (from this ARCHITECTURE.md)
- Known failure modes from [ClawOS](https://github.com/xbrxr03/clawos) history

### Why this is novel

All existing self-evolving systems target:
- [EvolveR](https://arxiv.org/abs/2510.16079) → model weights + principle repository
- [AgentEvolver](https://arxiv.org/abs/2511.10395) → model weights (RL)
- [ASI-Evolve](https://arxiv.org/abs/2603.29640) → programs being researched (not the harness)
- [GEPA](https://arxiv.org/abs/2507.19457) → prompt templates
- [Memento](https://arxiv.org/abs/2508.16153) → agent behavior without weight updates (closest)

**None treat the harness infrastructure itself — tool descriptions, middleware, memory architecture, security scopes — as the primary unit of evolution, tracked under version control, with falsifiable change manifests, running on consumer hardware.**

Confirmed by three independent surveys:
- [arXiv:2507.21046](https://arxiv.org/abs/2507.21046): harness-level evolution absent from What/When/How/Where taxonomy
- [arXiv:2508.07407](https://arxiv.org/abs/2508.07407): four-component framework does not include harness infrastructure as an evolution target
- [arXiv:2604.08224](https://arxiv.org/abs/2604.08224): identifies self-evolving harnesses as an emerging direction but cites no existing implementations

---

## 10. Hardware Budget

RTX 3060 12GB VRAM. Numbers based on [SLMs paper](https://arxiv.org/abs/2506.02153) benchmarks and [ASI-Evolve's](https://github.com/GAIR-NLP/ASI-Evolve) embedding model specs.

| Component | VRAM | RAM | Source |
|-----------|------|-----|--------|
| qwen2.5:14b-q4 (weights) | 7.8 GB | — | ~8GB for 14B Q4, standard llama.cpp |
| KV cache (32k context) | 1.8 GB | — | Varies with depth |
| FAISS indices | 0 | ~150 MB | CPU only |
| all-MiniLM-L6-v2 (ONNX) | 0 | ~80 MB | CPU only, same as ASI-Evolve |
| JARVIS binary | ~0 | ~15 MB | Rust single binary |
| SQLite | 0 | ~50 MB | WAL mode |
| Ollama runtime | ~100 MB | ~100 MB | Server overhead |
| **Total** | **~9.7 GB** | **~400 MB** | |
| **Headroom** | **~2.3 GB** | **~31.6 GB** | Buffer for KV spikes |

Config (`config/hardware.toml`):
```toml
[hardware]
vram_gb = 12
ram_gb = 32
gpu = "rtx3060"

[model]
primary = "qwen2.5:14b-q4"
fallback = "phi4:14b-q4"
inference = "ollama"

[compute]
daily_hours = 7
max_parallel_tools = 3
context_window = 32768
evolution_cycle_hours = 1
```

Fallback path: if qwen2.5:14b-q4 proves too tight, [SLMs paper](https://arxiv.org/abs/2506.02153) shows 7B models match 14B on structured agentic tasks. xLAM-2-8B (~4.5GB Q4) is worth testing as a dedicated tool-calling sub-model.

---

## 11. Inter-Component Data Flow

**Flow A — Normal task execution:**
```
agentd.run_task(task)
  → memd.build_context() → context_prefix (pinned + working + retrieved)
  → Ollama.complete(prompt) → (Thought, Action)
  → policyd.gate(action) → Allow/Deny/Pending
  → toolbridge.execute(action) → Observation
  → memd.write(episodic_entry) [write pipeline: filter→tag→dedupe→score→embed→cluster→write]
  → loop until done or max_attempts
  → evolved.record_outcome(task_id, score)
```

**Flow B — Evolution cycle:**
```
scheduler.tick() → evolved.run_cycle()
  → memd.query_cognition(domain) → top-5 CognitionItems
  → evolved.node_db.sample_ucb1(n=3) → parent EvolutionNodes
  → agentd.get_recent_outcomes() → failure patterns
  → Ollama.complete(researcher_prompt) → EvolutionNode + ChangeManifest
  → evolved.experiment(node) → verification results
  → Ollama.complete(analyzer_prompt) → analysis + new CognitionItem
  → memd.write_cognition(item)
  → git.commit(harness/changes)
  → evolved.node_db.write(node)
```

**Flow C — High-risk evolution (requires approval):**
```
evolved.propose_change(node) where risk_score >= 85
  → policyd.approval_queue.push(ApprovalRequest)
  → notify user (Telegram + Discord)
  → user responds Allow/Deny (300s timeout → auto-Deny)
  → if Allow: git.apply_diff(node.diff), git.commit()
  → evolved.node_db.update(node.status)
```

---

## 12. Design Flags

Things that diverge from the brief or need a decision before writing code.

### Flag 1 — One binary, not five daemons

**Brief implies:** Five separate daemon processes.
**Architecture:** Five modules in one binary, tokio async channels.
**Why:** ~100MB RAM saved from eliminated IPC and separate runtimes. One process to monitor. Faster inter-component calls.
**Impact:** Naming stays the same. Deployment is simpler.

### Flag 2 — ChromaDB → SQLite + FAISS

**Brief says:** ChromaDB for vector retrieval.
**Architecture:** SQLite (structured) + FAISS (vectors) + FTS5 (full-text search).
**Why:** ChromaDB requires a Python server process with its own memory footprint. FAISS runs in-process. SQLite gives structured queries and proven WAL semantics. This is exactly what [ASI-Evolve](https://github.com/GAIR-NLP/ASI-Evolve) and [Hermes Agent](https://github.com/NousResearch/hermes-agent) use in production.
**Impact:** Same functionality, less operational complexity.

### Flag 3 — Merkle chaining is real this time

**[ClawOS](https://github.com/xbrxr03/clawos):** Claims Merkle chaining in documentation. `policyd/service.py` shows plain SQLite append — no `prev_hash`, no SHA-256, no chain.
**Architecture:** SHA-256 chaining on every AuditEntry. `prev_hash` is a required non-nullable field. `verify_chain()` runs at startup.
**Why:** The audit trail is cited as a competitive moat. If it's not real, the claim is false and the paper is wrong. We actually build this.

### Flag 4 — Approval timeout: 300s not 5s

**[ClawOS](https://github.com/xbrxr03/clawos):** 5-second auto-deny.
**Architecture:** 300-second default (5 min), configurable. Telegram/Discord notification at request time.
**Why:** The system runs overnight autonomously. 5 seconds is useless for a human who is asleep.

### Flag 5 — "13,700+ skills on day one" means parser, not bundled library

**Brief:** "Inherits OpenClaw's 13,700+ skill ecosystem on day one."
**Architecture clarifies:** JARVIS implements the full [SKILL.md spec](https://github.com/K-Dense-AI/scientific-agent-skills), making any agentskills.io skill installable. "Day one interoperability" = the parser is ready. The 13,700+ skills are installable on demand, not pre-bundled. This is an accurate statement of what we deliver.

### Flag 6 — Core Rust never autonomously evolves

**Brief implies:** evolved can propose modifications to harness components.
**Architecture adds:** Modifications to core Rust modules (memd internals, policyd gate logic, audit chain structure) require human approval via the approval queue, regardless of what evolved proposes. Skill files, config, and system prompts can evolve more autonomously.
**Why:** The self-evolving agent must not autonomously rewrite the code that governs its own permissions. This is the safety boundary [ClawOS](https://github.com/xbrxr03/clawos) identified but didn't formalize.

### Flag 7 — Embedding model is CPU-only

**Decision:** all-MiniLM-L6-v2 runs on CPU via ONNX. No VRAM cost.
**Why:** VRAM budget is tight. Embedding happens between LLM calls (not during), so CPU latency (~5–10ms) doesn't affect response time. The same model runs on CPU in [ASI-Evolve's](https://github.com/GAIR-NLP/ASI-Evolve) production setup.

---

## 13. Build Order

```
Week 1 — memd + toolbridge (foundation, no LLM calls yet)
  - memd: SQLite schema init, FAISS init, WAL mode, write pipeline
  - memd: pinned/working/episodic read + write paths
  - toolbridge: ToolManifest, SKILL.md parser (Tier 1 frontmatter only)
  - toolbridge: JSON Schema validation for tool params
  Test: write 10 episodic entries, retrieve by query, verify multi-signal scores
  Test: load a SKILL.md from K-Dense-AI/scientific-agent-skills, validate name, extract description

Week 2 — agentd + policyd
  - policyd: gate function (all 9 checks), risk score table, AuditEntry + Merkle chain
  - policyd: credential vault (AES-256-GCM), approval queue (300s timeout)
  - agentd: TaskNode struct, execution loop (ReAct format), Reflexion module
  - scheduler: CronJob struct, tick(), advance_next_run() crash safety (from Hermes Agent)
  Test: run one task through full Thought/Action/Observation loop
  Test: trigger denied tool call → verify AuditEntry written + chain valid
  Test: kill switch propagation cancels all running tasks

Week 3 — evolved skeleton (observation mode, no modifications yet)
  - EvolutionNode + ChangeManifest + CognitionItem structs
  - Cognition store: seed from paper summaries (~100 items)
  - UCB1 node sampling (c=1.414, from ASI-Evolve config.yaml)
  - Evolution cycle trigger (scheduler integration, every 1 hour)
  - Researcher prompt: generates EvolutionNode proposals, writes to review queue
  - Does NOT apply any diffs yet — proposals only
  Test: run one evolution cycle, verify CognitionItem written, node added with manifest

Week 4 — Professor X activation
  - Load personas/professor_x.md into pinned memory layer
  - Wire daily cycle skills into scheduler (7 cron jobs)
  - Enable evolved to apply diffs to harness/ with git commit + verification
  - Run one complete 7-hour cycle
  Verify: GitHub commit, Telegram/Discord messages, memd persists across restart
  Verify: evolved logged outcomes + generated at least one EvolutionNode
  Verify: AuditEntry chain valid after full day of operation
```

---

*Architecture version: 0.2*
*Compiled: 2026-05-21*
*Status: Pre-implementation. No Rust files written yet.*
*Next action: User reviews this document → switch to Linux machine → begin Week 1.*
