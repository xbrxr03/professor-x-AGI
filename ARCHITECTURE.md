# Professor X — Architecture Document
> Design-before-code. No .rs files until this document is reviewed and approved.
>
> **For the Linux agent:** Every repo and paper linked below should be cloned/fetched before starting implementation. The repos are the direct sources for the data structures and patterns described here.
>
> **Path note:** All source files are now under `professor-x/src/` (renamed from `professor-x/src/`). All IMPLEMENTATION_SPEC paths use `professor-x/` prefix.

---

## Source Material

### Repos to clone

```bash
# Our prototype
git clone https://github.com/xbrxr03/clawos                          # Our prototype — policyd source

# Direct competitors — study DEEPLY before writing evolved.rs
git clone https://github.com/dav-joy-thon/MOSS                       # PRIMARY COMPETITOR: source-level harness rewriting (arXiv:2605.22794)
git clone https://github.com/facebookresearch/Hyperagents             # HyperAgents/DGM-H: improvement@k (arXiv:2603.19461)

# Self-evolution reference systems
git clone https://github.com/GAIR-NLP/ASI-Evolve                     # Self-evolution reference (SJTU, arXiv:2603.29640)
git clone https://github.com/ZJU-REAL/SDAR                           # SDAR: token-weighted distillation (arXiv:2605.15155)
git clone https://github.com/modelscope/AgentEvolver                 # Self-evolving RL reference (arXiv:2511.10395)

# Memory systems
git clone https://github.com/Tencent/TencentDB-Agent-Memory          # L0→L1→L2→L3 pyramid + Mermaid canvas (PRIMARY memd inspiration)

# Local agent systems — study for agentd + toolbridge patterns
git clone https://github.com/xark-argo/argo                          # ARGO: Agent Factory pattern, local Manus alternative
git clone https://github.com/Fosowl/agenticSeek                      # AgenticSeek: local-first, smart agent routing

# SKILL.md format references
git clone https://github.com/K-Dense-AI/scientific-agent-skills      # SKILL.md spec + examples (K-Dense-AI format)
git clone https://github.com/NousResearch/hermes-agent               # Scheduler + memory schema reference
git clone https://github.com/Orchestra-Research/AI-Research-SKILLs  # Additional SKILL.md examples
git clone https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep  # Research automation patterns
git clone https://github.com/Imbad0202/academic-research-skills     # Academic SKILL.md examples

# Literature indexes
git clone https://github.com/Gloriaameng/Awesome-Agent-Harness       # Harness paper index
git clone https://github.com/XMUDeepLIT/Awesome-Self-Evolving-Agents # Self-evolving agent index
git clone https://github.com/ai-boost/awesome-harness-engineering    # Harness engineering index
```

### Papers to fetch (arXiv)

**Tier 1 — Core architecture (read in full before touching any component):**

| ID | Title | Link | What it gives Professor X |
|----|-------|------|----------------------|
| 2604.25850 | Agentic Harness Engineering (AHE) | [arxiv.org/abs/2604.25850](https://arxiv.org/abs/2604.25850) | Harness taxonomy, 3-pillar observability, change manifests |
| 2603.29640 | ASI-Evolve: AI Accelerates AI | [arxiv.org/abs/2603.29640](https://arxiv.org/abs/2603.29640) | Researcher/Engineer/Analyzer loop, cognition base, Node schema |
| 2309.02427 | CoALA: Cognitive Architectures for Language Agents | [arxiv.org/abs/2309.02427](https://arxiv.org/abs/2309.02427) | Memory taxonomy (4 types), action space taxonomy, decision cycle |
| 2305.16291 | Voyager: Open-Ended Embodied Agent | [arxiv.org/abs/2305.16291](https://arxiv.org/abs/2305.16291) | Skill library + verified procedural memory |
| 2303.11366 | Reflexion: Verbal Reinforcement Learning | [arxiv.org/abs/2303.11366](https://arxiv.org/abs/2303.11366) | Self-reflection after failure, bounded memory buffer |
| 2210.03629 | ReAct: Synergizing Reasoning and Acting | [arxiv.org/abs/2210.03629](https://arxiv.org/abs/2210.03629) | Thought/Action/Observation execution trace format |

**Tier 2 — Memory and context (read before implementing memd):**

| ID | Title | Link | What it gives Professor X |
|----|-------|------|----------------------|
| 2603.07670 | Memory for Autonomous LLM Agents | [arxiv.org/abs/2603.07670](https://arxiv.org/abs/2603.07670) | Write-manage-read loop, multi-signal retrieval scoring |
| 2603.15421 | CLAG: Memory for Small Language Models | [arxiv.org/abs/2603.15421](https://arxiv.org/abs/2603.15421) | Two-stage cluster retrieval, 100-entry cold start |
| 2604.08224 | Externalization in LLM Agents | [arxiv.org/abs/2604.08224](https://arxiv.org/abs/2604.08224) | Why harness > model for memory; Pattern B architecture |
| 2510.16079 | EvolveR: Closed-Loop Self-Evolving QA Agent | [arxiv.org/abs/2510.16079](https://arxiv.org/abs/2510.16079) | Principle quality formula `(success+1)/(use+2)`, self-distillation |

**Tier 3 — Self-evolution taxonomy + SLMs (read before implementing evolved):**

| ID | Title | Link | What it gives Professor X |
|----|-------|------|----------------------|
| 2507.21046 | Self-Evolving Agents: What/When/How/Where | [arxiv.org/abs/2507.21046](https://arxiv.org/abs/2507.21046) | Confirms harness-level evolution is a literature gap |
| 2508.07407 | Comprehensive Survey of Self-Evolving AI | [arxiv.org/abs/2508.07407](https://arxiv.org/abs/2508.07407) | Four-component framework; confirms gap |
| 2508.16153 | Memento: Agent Optimization Without Weight Updates | [arxiv.org/abs/2508.16153](https://arxiv.org/abs/2508.16153) | Closest prior work to Professor X's approach — read carefully |
| 2507.19457 | GEPA: Reflective Prompt Evolution Beats RL | [arxiv.org/abs/2507.19457](https://arxiv.org/abs/2507.19457) | Prompt-level evolution as a feasible evolution target |
| 2511.10395 | AgentEvolver | [arxiv.org/abs/2511.10395](https://arxiv.org/abs/2511.10395) | Experience unit format "when to use" + "content" |
| 2506.02153 | Small Language Models are the Future of Agentic AI | [arxiv.org/abs/2506.02153](https://arxiv.org/abs/2506.02153) | Validates SLM + good harness ≥ frontier + bad harness; xLAM-2-8B for tool calling |
| 2510.03847 | SLMs for Agentic Systems Survey | [arxiv.org/abs/2510.03847](https://arxiv.org/abs/2510.03847) | vLLM/SGLang serving; JSON Schema validation patterns |

**Tier 4 — Trifecta inventions (DHE, BF, LCAP) — read before implementing Section 14):**

| ID | Title | Link | What it gives Professor X |
|----|-------|------|----------------------|
| 2310.11511 | Self-RAG: Learning to Retrieve, Generate, and Critique | [arxiv.org/abs/2310.11511](https://arxiv.org/abs/2310.11511) | Adaptive retrieval (learn when to retrieve) — LCAP predecessor |
| 2604.00594 | Agent Psychometrics: IRT for AI Agents | [arxiv.org/abs/2604.00594](https://arxiv.org/abs/2604.00594) | IRT decomposition: scaffold ability is separable from model ability — BF grounding |
| 2601.19935 | Mem2ActBench | [arxiv.org/abs/2601.19935](https://arxiv.org/abs/2601.19935) | Oracle vs. retrieval 23-point gap — confirms allocation matters for LCAP |
| 2506.21605 | MemBench | [arxiv.org/abs/2506.21605](https://arxiv.org/abs/2506.21605) | Store size degrades quality at 100K tokens — confirms BF & LCAP motivation |

**Tier 5 — NEW: v3.0 additions (MOSS, Ratchet, Co-Scientist, TencentDB) — read before implementing evolved.rs:**

| ID | Title | Link | What it gives Professor X |
|----|-------|------|----------------------|
| 2605.22794 | MOSS: Source-Level Harness Rewriting | [arxiv.org/abs/2605.22794](https://arxiv.org/abs/2605.22794) | **PRIMARY COMPETITOR**. verify-then-commit pattern. Health-probe rollback. No consumer HW, no metacognitive self-model — our gap. |
| 2605.22148 | Ratchet: Skill Lifecycle Management | [arxiv.org/abs/2605.22148](https://arxiv.org/abs/2605.22148) | retire_skill() is load-bearing. WITHOUT: +0.0pp. WITH: +0.328pp. Pattern canonicalisation. Bounded active-cap. |
| 2502.18864 | Co-Scientist: Elo-Based Idea Tournament | [arxiv.org/abs/2502.18864](https://arxiv.org/abs/2502.18864) | Generate 3-5 competing proposals per cycle. Agents debate. Elo ranking selects winner. Stronger than greedy first-proposal acceptance. |
| — | TencentDB Agent Memory | [github.com/Tencent/TencentDB-Agent-Memory](https://github.com/Tencent/TencentDB-Agent-Memory) | L0→L1→L2→L3 semantic pyramid. Mermaid task canvas in working memory. Cuts token usage ~61%. Reject brute-force history AND lossy summarization. |
| 2603.19461 | HyperAgents / DGM-H (Meta) | [arxiv.org/abs/2603.19461](https://arxiv.org/abs/2603.19461) | improvement@k metric. Frontier APIs, coding domain only — our consumer HW differentiator. |

**Tier 6 — Three-lever framework + comparative landscape (read before writing the paper):**

| ID | Title | Link | What it gives Professor X |
|----|-------|------|----------------------|
| 2605.15155 | SDAR: Self-Distilled Agentic Reinforcement Learning | [arxiv.org/abs/2605.15155](https://arxiv.org/abs/2605.15155) | Lever 1 (parametric): token-level sigmoid-gated distillation, +9.4% ALFWorld on Qwen3 |
| 2605.22166 | Life-Harness (Adapting the Interface, Not the Model) | [arxiv.org/abs/2605.22166](https://arxiv.org/abs/2605.22166) | Lever 3 portability proof: harness from Qwen3-4B transfers to 17 models, 88.5% avg improvement |
| 2505.00234 | Self-Generated In-Context Examples | [arxiv.org/abs/2505.00234](https://arxiv.org/abs/2505.00234) | Lever 2 (contextual): 73%→93% ALFWorld zero fine-tuning, trajectory replay via ICL |
| 2505.03335 | Absolute Zero | [arxiv.org/abs/2505.03335](https://arxiv.org/abs/2505.03335) | Self-generated curriculum, zero external data, NeurIPS 2025 spotlight |
| 2603.28052 | Meta-Harness (Stanford) | [arxiv.org/abs/2603.28052](https://arxiv.org/abs/2603.28052) | Closest competitor: LLM-based harness optimization using frontier APIs (Claude Code as proposer). Professor X differs: consumer hardware, metacognitive self-model, three levers combined |
| 2604.20938 | Harbor: Automated Harness Optimization | [arxiv.org/abs/2604.20938](https://arxiv.org/abs/2604.20938) | Bayesian optimization (not LLM-based) for harness config search — complementary approach, no metacognition |
| 2603.10600 | Trajectory-Informed Memory Generation | [arxiv.org/abs/2603.10600](https://arxiv.org/abs/2603.10600) | Lever 2 variant: 14.3pp gains (149% relative on complex tasks) from trajectory-derived memory |
| 2601.11974 | MARS: Metacognitive Agent Reflective Self-improvement | [arxiv.org/abs/2601.11974](https://arxiv.org/abs/2601.11974) | Principle + procedural reflection in single cycle — DHE Layer 5 grounding |
| 2605.12129 | It's Not the Size: Harness Design Determines Stability in SLMs | [arxiv.org/abs/2605.12129](https://arxiv.org/abs/2605.12129) | 4-stage pipeline achieves TSR=0.952 on 2-3B models — validates Professor X's SLM+harness thesis |
| 2510.04618 | Agentic Context Engineering (ACE) | [arxiv.org/abs/2510.04618](https://arxiv.org/abs/2510.04618) | ICLR 2026: context as evolving playbook, +10.6% agent benchmarks — Lever 2 predecessor |
| 2510.04399 | Statistical Limits of Self-Improving Agents | [arxiv.org/abs/2510.04399](https://arxiv.org/abs/2510.04399) | Theorem: self-improvement safe iff capacity bounded. Harness evolution (frozen model) satisfies this by construction |
| 2604.11364 | The Missing Knowledge Layer | [arxiv.org/abs/2604.11364](https://arxiv.org/abs/2604.11364) | Four-layer memory (Knowledge/Memory/Wisdom/Intelligence) with distinct persistence semantics — upgrade path for memd |
| 2506.05109 | Truly Self-Improving Agents Require Intrinsic Metacognitive Learning | [arxiv.org/abs/2506.05109](https://arxiv.org/abs/2506.05109) | ICML 2025 position paper with no implementation — Professor X is the implementation |
| 2603.25723 | Natural-Language Agent Harnesses | [arxiv.org/abs/2603.25723](https://arxiv.org/abs/2603.25723) | Harness logic is rarely portable — confirms the gap Professor X fills |

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
14. [Trifecta Inventions — DHE, BF, LCAP](#14-trifecta-inventions--dhe-bf-lcap)
15. [Three-Lever Framework & Experimental Design](#15-three-lever-framework--experimental-design)

---

## 1. Design Principles

Every architectural decision in this document follows from three rules:

**Rule 1 — VRAM is the scarce resource.**
The RTX 3060 has 12GB VRAM. The LLM inference engine (qwen3:8b-q4_k_m) consumes ~5.2GB. Everything else shares the remaining ~6.8GB. Every MB saved in the harness is a MB available for KV cache, which is a larger effective context window, which is a smarter agent.

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

**The policyd module is the exception.** It wraps every outbound tool call as async middleware. Every call that exits Professor X's boundary goes through policyd's gate function before it touches the OS.

### Process topology at runtime

```
professor-x (single PID)
  ├── tokio runtime (async executor)
  ├── memd (memory manager, owns SQLite handles)
  ├── toolbridge (tool registry + executor)
  ├── agentd (task graph + scheduler)
  ├── policyd (gates every toolbridge call)
  └── evolved (background loop, reads from agentd outcomes)
```

### What runs outside the binary

- **Ollama**: Separate process, LLM inference. Professor X talks to Ollama via HTTP (localhost:11434). Primary model: `qwen3:8b-q4_k_m` (5.2GB VRAM, ~42 tok/s, 32K ctx, thinking mode). Upgrade: `llama4:scout` (MoE 109B total / 17B active, ~10GB VRAM). Fallback: `qwen3:14b-q4_k_m`. No Professor X code runs inside Ollama.
- **Embedding model** (`all-MiniLM-L6-v2`): Runs via ONNX runtime using the `ort` crate. In-process, CPU only. No Python dependency, ~80MB RAM, no VRAM cost. Same model used by [ASI-Evolve's](https://github.com/GAIR-NLP/ASI-Evolve) cognition store.

---

## 3. Component Map

```
┌──────────────────────────────────────────────────────────────┐
│  professor-x binary                                                │
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

Taxonomy from [CoALA](https://arxiv.org/abs/2309.02427) (Working / Episodic / Semantic / Procedural), extended with a Pinned layer for Professor X's identity and goals. Write path from [Memory for LLM Agents](https://arxiv.org/abs/2603.07670). Retrieval from [CLAG](https://arxiv.org/abs/2603.15421). Quality scoring from [EvolveR](https://arxiv.org/abs/2510.16079).

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

Risk scoring and validation pipeline from [ClawOS](https://github.com/xbrxr03/clawos) (`policyd/service.py`). Merkle chaining is designed here from scratch — ClawOS claims it in docs but the code does plain SQLite append. Professor X actually implements it.

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
    HarnessConfig,              // professor-x.toml keys
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
~/.professor-x/
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
- **Professor X extensions go in `metadata`:** `metadata.px-version`, `metadata.min-harness-version`, `metadata.requires` (dependency list — not in the spec but needed)

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

Each task uses Professor X's SKILL.md conductor skills (defined in `professor-x/skills/conductor/`).

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

Verification: `verify_chain() → bool` walks all entries in timestamp order, recomputes hashes. Called at Professor X startup. Any mismatch means the log has been tampered with.

**Why this matters for the thesis:** [ClawOS](https://github.com/xbrxr03/clawos) claimed Merkle chaining as a competitive moat but the `policyd/service.py` source shows plain SQLite append with no hashing. Professor X actually builds this. It's part of the paper's contribution: a real tamper-evident audit trail for an autonomous agent on consumer hardware.

### Credential vault

Storage: `~/.professor-x/vault.enc`. Encryption: AES-256-GCM (`aes-gcm` crate). Key: `~/.professor-x/vault.key`, chmod 600. Injection: credentials only reach subprocess via `Command::env(key, value)` at exec boundary. Never appear in LLM prompts, audit logs, or evolution diffs.

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

### What Professor X can evolve

Based on [AHE's 7-component harness taxonomy](https://arxiv.org/abs/2604.25850) and [ClawOS's](https://github.com/xbrxr03/clawos) security precedent:

| Component | Change Type | Autonomy |
|-----------|-------------|----------|
| System prompt | Text edit | Semi-autonomous (Professor X approves) |
| Tool descriptions (YAML) | Text edit | Semi-autonomous |
| Skill definitions (SKILL.md) | Text edit + new files | Semi-autonomous |
| Harness config (professor-x.toml) | Key-value edits | Semi-autonomous |
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

All evolvable components live in `professor-x/harness/` under git:
```
professor-x/harness/
  system_prompt.md
  tool_descriptions/   ← *.yaml
  skills/              ← *.md  (SKILL.md format)
  config/              ← professor-x.toml
  middleware/          ← *.rs  (human-review-only)
```

Every evolution cycle commits. Rollback = `git revert HEAD`. Full history = full evolution record.

This satisfies [AHE Component Observability](https://arxiv.org/abs/2604.25850): "each failure pattern maps to a single component class, every pass-rate change localizable to one file."

### Cognition base initialization

Professor X starts with ~100–150 pre-seeded CognitionItems (matching [ASI-Evolve's](https://github.com/GAIR-NLP/ASI-Evolve) documented scale). Seeded from:
- Key claims extracted from the 15 papers in this document
- Professor X's own design decisions (from this ARCHITECTURE.md)
- Known failure modes from [ClawOS](https://github.com/xbrxr03/clawos) history

### Why this is novel

#### The three-lever landscape (as of May 2026)

Agent self-improvement operates on three orthogonal axes. Every existing system operates on exactly one:

| Lever | What changes | Representative works | Limitation |
|-------|-------------|---------------------|------------|
| **Lever 1 — Parametric** | Model weights (fine-tuning) | SDAR (2605.15155), QLoRA/Alpaca | Slow, expensive, model-bound, loses generality |
| **Lever 2 — Contextual** | In-context content (trajectory replay, heuristics) | Self-Generated ICE (2505.00234), MARS (2601.11974), ACE (2510.04618), Trajectory-Informed Memory (2603.10600) | Ephemeral (lost each session), can't accumulate structural fixes |
| **Lever 3 — Structural** | Harness infrastructure (tool descriptions, memory arch, prompts) | AHE (2604.25850), Meta-Harness (2603.28052), Life-Harness (2605.22166), Harbor (2604.20938) | Each paper covers structural-only; no metacognitive self-model |

**Professor X operates on all three simultaneously, with a metacognitive self-model tracking performance across levers.**

#### What the latest papers do and don't cover

- [Meta-Harness (Stanford, arXiv:2603.28052)](https://arxiv.org/abs/2603.28052): closest competitor for Lever 3. Uses Claude Code (frontier API) as harness proposer. Achieves 7.7pp on text classification, #2 TerminalBench-2. Does NOT run on consumer hardware. Does NOT combine with Levers 1/2. Does NOT have a metacognitive self-model.
- [Life-Harness (arXiv:2605.22166)](https://arxiv.org/abs/2605.22166): proves Lever 3 is portable (88.5% avg improvement across 17 models). Runtime adaptation only — no evolution loop, no diagnostics.
- [SDAR (arXiv:2605.15155)](https://arxiv.org/abs/2605.15155): Lever 1 on Qwen3 families. +9.4% ALFWorld. No harness evolution. Feasible on RTX 3060 overnight (see hardware budget).
- [Harbor (arXiv:2604.20938)](https://arxiv.org/abs/2604.20938): Bayesian optimization for harness config search. Not LLM-based, no metacognition, no diagnostics.
- [arXiv:2510.04399](https://arxiv.org/abs/2510.04399): Statistical limits theorem. Proves self-improvement is safe iff model capacity is bounded. Professor X's harness evolution (frozen model weights) satisfies this by construction.

**The gap:** No existing paper (a) combines all three levers, (b) uses metacognitive self-knowledge to direct which lever to pull at each round, (c) runs entirely on consumer hardware, (d) measures improvement with harness-isolated attribution (HIRO + DHE + BF + LCAP).

Confirmed by independent surveys:
- [arXiv:2507.21046](https://arxiv.org/abs/2507.21046): harness-level evolution absent from What/When/How/Where taxonomy
- [arXiv:2508.07407](https://arxiv.org/abs/2508.07407): four-component framework does not include harness infrastructure as an evolution target
- [arXiv:2604.08224](https://arxiv.org/abs/2604.08224): identifies self-evolving harnesses as an emerging direction but cites no implementations

---

## 10. Hardware Budget

RTX 3060 12GB VRAM. Numbers based on [SLMs paper](https://arxiv.org/abs/2506.02153) benchmarks, Qwen3 technical report, and [ASI-Evolve's](https://github.com/GAIR-NLP/ASI-Evolve) embedding model specs.

| Component | VRAM | RAM | Source |
|-----------|------|-----|--------|
| qwen3:8b-q4_k_m (weights) | 5.2 GB | — | Qwen3 technical report, 4-bit 8B |
| KV cache (32K context) | ~1.5 GB | — | Varies with depth; 32K = Qwen3 native ctx |
| FAISS indices | 0 | ~150 MB | CPU only |
| all-MiniLM-L6-v2 (ONNX) | 0 | ~80 MB | CPU only, same as ASI-Evolve |
| professor-x binary | ~0 | ~15 MB | Rust single binary |
| SQLite | 0 | ~50 MB | WAL mode |
| Ollama runtime | ~100 MB | ~100 MB | Server overhead |
| **Total** | **~6.9 GB** | **~400 MB** | |
| **Headroom** | **~5.1 GB** | **~31.6 GB** | Significant buffer — enables sleep-time LoRA |

**Upgrade path:** `llama4:scout` (MoE, 109B total / 17B active parameters, ~10GB VRAM) fits within headroom budget for higher-stakes reasoning tasks. Switch via config, no code change.

**Sleep-time fine-tuning:** With qwen3:8b-q4_k_m as primary, the 5.1GB VRAM headroom is sufficient for overnight QLoRA fine-tuning (Qwen3-8B 4-bit + LoRA adapters + optimizer states ≈ 6GB with [unsloth](https://github.com/unslothai/unsloth)). Schedule via evolved's background runner when agent is idle. This is Lever 1 (parametric) in the three-lever framework.

Config (`config/hardware.toml`):
```toml
[hardware]
vram_gb = 12
ram_gb = 32
gpu = "rtx3060"

[model]
primary = "qwen3:8b-q4_k_m"
upgrade = "llama4:scout"
fallback = "qwen3:14b-q4_k_m"
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
**Architecture clarifies:** Professor X implements the full [SKILL.md spec](https://github.com/K-Dense-AI/scientific-agent-skills), making any agentskills.io skill installable. "Day one interoperability" = the parser is ready. The 13,700+ skills are installable on demand, not pre-bundled. This is an accurate statement of what we deliver.

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

Week 5 — Trifecta (DHE + BF + LCAP)
  - DiagnosticTrace struct + 5-layer probe functions (see Section 14)
  - DHE integration: Analyzer calls diagnostic before generating EvolutionNode
  - BF integration: HiroRoundResult gets fingerprint: [f32; 3] field
  - LCAP: LcapPolicy struct, ContextBudget per TaskType, bandit update loop
  - LCAP integration: memd.build_context() accepts ContextBudget from LCAP
  - Wire LCAP into DHE: Layer 2 attribution triggers LCAP.regress() directly
  Run H1 experiment first (before LCAP goes live) to establish T*
  Test: one failed task → DHE trace → attribution logged → correct layer identified
  Test: 3 HIRO rounds with BF → fingerprint values differ across task categories
  Test: LCAP policy updates after regression detected in one task type
```

---

## 14. Trifecta Inventions — DHE, BF, LCAP

**Full specification:** [brain/inventions.md](../professor-x-AGI/brain/inventions.md)

Three novel mechanisms layered on top of the evolved component. None of them modify the core ReAct loop, memory architecture, or security model. They are purely additive — the system works without them (baseline Professor X) and is instrumented by them (trifecta Professor X).

**Source papers (Tier 4 — read before Week 5):**
- [Self-RAG (arXiv:2310.11511)](https://arxiv.org/abs/2310.11511) — adaptive retrieval as LCAP predecessor
- [Agent Psychometrics (arXiv:2604.00594)](https://arxiv.org/abs/2604.00594) — IRT decomposition as BF grounding
- [AHE Table 3 (arXiv:2604.25850)](https://arxiv.org/abs/2604.25850) — 33.7% fix-prediction precision as DHE baseline

---

### 14.1 — DHE: Diagnostic Harness Evolution

When a task fails, the Analyzer runs a 5-layer probe before invoking the Researcher:

```rust
// New struct in evolved/diagnostic.rs
DiagnosticTrace {
    task_id: u64,
    failed_layer: u8,       // 1=retrieval, 2=context, 3=dispatch, 4=execution, 5=reasoning
    evidence: String,
    confidence: f32,
    probe_results: Vec<LayerResult>,
}

LayerResult {
    layer: u8,
    test: String,           // what was checked
    passed: bool,
    detail: String,
}
```

**5-layer probe logic:**

```
Layer 1 — Retrieval presence
  memd.query(task.description, top_k=10)
  Pass: any result has cosine_sim > 0.75 to the fact needed to solve the task
  Fail: relevant memory was not retrieved → attribution = retrieval

Layer 2 — Context construction
  Inspect raw prompt sent to Ollama.
  Pass: injected content is in first 25% or last 25% of token positions
  Fail A: critical content is in middle 50% → position failure → attribution = context_builder
  Fail B: total_tokens > T* (LCAP ceiling for this task type) → overload → attribution = context_overload

Layer 3 — Tool dispatch
  Parse Action field from execution trace.
  Pass: Action parses cleanly AND selected tool is appropriate for the task
  Fail: malformed Action OR wrong tool selected → attribution = tool_description

Layer 4 — Tool execution
  Inspect Observation.success.
  Pass: success=true AND content is non-empty AND content is relevant
  Fail: success=false → attribution = tool_implementation

Layer 5 — Reasoning (LLM-as-judge)
  Prompt: "Given [task, Observations, final Thought]: did reasoning correctly use the Observations? Answer yes/no."
  Pass: yes → reasoning was fine, failure is at a different layer (re-examine 1-4)
  Fail: no → attribution = reasoning (system_prompt or planning guidance)
```

**Integration with evolved cycle:**

Modified Phase 4 (Analyze) in Section 9:

```
If task failed:
  1. Run DHE probe → DiagnosticTrace
  2. If failed_layer == 2 (context_overload): LCAP.regress(task_type) — no Researcher call needed
  3. Else: Researcher receives attribution as constraint
     ChangeManifest.root_cause must cite failed_layer and evidence
     Researcher proposal must modify a component in the attributed layer
  4. DiagnosticTrace written to EvolutionNode.diagnostics[]
```

**Measurable output:**

After 30 rounds: `fix_prediction_precision = hits / DHE_preceded_nodes`. Target ≥ 0.60. Baseline (rounds without DHE) ≈ 0.337 (AHE reported figure).

---

### 14.2 — BF: Behavioral Fingerprinting

Every HIRO round computes a 3-component fingerprint in addition to the aggregate score:

```rust
// Extend HiroRoundResult with:
fingerprint: [f32; 3],          // [p_tool_use, p_planning, p_self_correction]
delta_fingerprint: [f32; 3],    // fingerprint[k] - fingerprint[k-1]; [0,0,0] for round 0
component_modified: Option<ComponentClass>,  // what changed since last round
harness_commit: String,         // git commit hash of harness/ at round start

// ComponentClass enum
enum ComponentClass {
    SystemPrompt,
    ToolDescription,
    MemoryArchitecture,
    SkillDefinition,
    ContextPolicy,   // LCAP policy change
    None,            // null round (no modification)
}
```

**Computation:** fingerprint[i] = pass@3 on the 20 tasks in category i. Already computed during HIRO — this is just storage and breakdown, not additional inference.

**Storage:** `hiro_rounds` table in SQLite. One row per round. The 30-round fingerprint trajectory is the longitudinal dataset.

**Automatic analysis at round end:**

```
If max(|delta_fingerprint|) > 0.05:
  Log: "Significant fingerprint shift in round {k}: {category} moved {delta} pp"
  Tag the EvolutionNode that was active this round as "fingerprint-significant"

If any component < 0.50 and delta < 0:
  Prioritize DHE probe on that category's failed tasks next round
  (BF drives DHE targeting — this is the detect → attribute loop)
```

---

### 14.3 — LCAP: Learned Context Allocation Policy

Per-task-type context budget, updated between HIRO rounds via a UCB1 multi-armed bandit.

```rust
// New file: evolved/lcap.rs

ContextBudget {
    episodic_slots: u8,         // 0-10 episodic memory entries
    semantic_slots: u8,         // 0-10 semantic memory entries
    tool_depth: ToolDepth,      // Shallow / Medium / Full
    system_prompt_tokens: u16,  // soft cap on system prompt length
    hard_ceiling_tokens: u32,   // set from H1's T* once resolved
}

// Bandit arms per task type: 5 pre-defined strategies
const ARMS: [ContextBudget; 5] = [
    // Sparse: 1 episodic, 1 semantic, Shallow tools
    // Conservative: 3 episodic, 2 semantic, Medium tools  (initial default)
    // Balanced: 4 episodic, 4 semantic, Medium tools
    // Rich: 6 episodic, 5 semantic, Full tools
    // Memory-heavy: 8 episodic, 7 semantic, Shallow tools
];

LcapPolicy = HashMap<TaskType, ArmState>

ArmState {
    arm_idx: usize,            // current selected arm
    arm_stats: [(f32, u32); 5], // (mean_pass3, visit_count) per arm
    total_rounds: u32,
}
```

**Update rule (called at end of each HIRO round):**

```
For each TaskType T:
  arm = policy[T].arm_idx
  arm_stats[arm].visits += 1
  arm_stats[arm].mean = running_mean(arm_stats[arm].mean, p_T(k))

  // UCB1 selection for next round
  next_arm = argmax_i [ arm_stats[i].mean + 1.414 * sqrt(ln(total_rounds) / arm_stats[i].visits) ]
  policy[T].arm_idx = next_arm
```

**Integration with memd.build_context():**

```rust
// Modified function signature:
memd.build_context(task: &TaskNode, budget: &ContextBudget) -> ContextPrefix

// agentd calls:
let budget = lcap.get(task.task_type);
let context = memd.build_context(task, &budget);
```

**DHE fast-path:**

```
If DHE attribution == context_overload (Layer 2, failed_layer=2):
  lcap.force_reduce(task.task_type)  // reduce both episodic_slots and semantic_slots by 1
  // no EvolutionNode generated, no Researcher call
  // logged as DiagnosticTrace with layer=2 and action="lcap_reduce"
```

**Initial policy (before H1 resolves):**

All task types start on arm 1 (Conservative: 3 episodic, 2 semantic, Medium tools, hard_ceiling_tokens = 6000). Once H1 experiment resolves T*, hard_ceiling_tokens is updated across all arms for all task types.

---

### 14.4 — Trifecta Integration Summary

Three questions answered per HIRO round:

| Question | Answered by | Data produced |
|----------|-------------|---------------|
| What is the harness bad at? | BF (fingerprint delta) | fingerprint[k], delta_fingerprint |
| Why is it bad at it? | DHE (layer attribution) | DiagnosticTrace.failed_layer |
| How should context be adjusted? | LCAP (bandit update) | new arm selection per task type |

The three do not require each other to function. BF runs regardless. DHE runs on any failure. LCAP runs at every round end. But they are designed to feed each other:

```
BF identifies weak category
  → DHE prioritizes that category's failures for diagnostic
    → DHE attribution (layer 2) triggers LCAP fast-path reduce
      → next round, BF measures whether the category recovered
```

This is the primary feedback loop of the trifecta. It operates faster than the full Researcher/Engineer/Analyzer loop (no LLM call needed for layer-2 attribution → LCAP response), which means context-related failures can be corrected in one round rather than waiting for a full evolution cycle.

---

*Architecture version: 0.3*
---

## 15. Three-Lever Framework & Experimental Design

### The framework

Agent self-improvement operates on three orthogonal levers. Professor X combines all three. The framework is the thesis framing — the paper's central contribution beyond the specific mechanisms.

```
Lever 1 — PARAMETRIC (weights)
  What: SDAR fine-tuning on successful trajectories. Overnight QLoRA run.
  When: After 30 HIRO rounds, when agent has accumulated 500+ successful trajectories.
  How: Alpaca-style self-distillation: use agent's own successful episodes as training data.
       Generate with qwen3:8b, fine-tune qwen3:8b on its own good outputs.
       Implementation: unsloth + SDAR objective (sigmoid-gated token distillation).
  Signal: model-level capability improvement (not harness)
  Pace: Slow (1x per batch of episodes, overnight)
  Portability: LOW (adapter is model-specific)

Lever 2 — CONTEXTUAL (trajectory replay)
  What: Self-Generated ICE, ACE-style evolving system prompt, MARS reflection.
  When: Every session, before each task.
  How: memd.episodic retrieves similar past successful trajectories → inject as few-shot examples.
       MARS single-cycle reflection: after failure, generate principle + procedure → inject to working memory.
  Signal: session-level performance boost (ephemeral)
  Pace: Fast (every task)
  Portability: MEDIUM (tied to task domain, not model)

Lever 3 — STRUCTURAL (harness)
  What: DHE-guided harness evolution. Changes tool descriptions, memory architecture, prompts.
  When: Every evolved cycle (default: every hour).
  How: DHE diagnoses → Researcher proposes → Engineer applies → Analyzer verifies.
  Signal: persistent harness improvement (accumulates)
  Pace: Medium (1 cycle/hour)
  Portability: HIGH (harness transfers across model families, Life-Harness proves this)
```

### Why the combination is stronger than any single lever

| Property | Lever 1 alone | Lever 2 alone | Lever 3 alone | All three |
|----------|--------------|--------------|--------------|-----------|
| Persistent gains | ✓ | ✗ | ✓ | ✓ |
| Session-time speed | ✗ | ✓ | ✓ | ✓ |
| Transfers to new model | ✗ | Partial | ✓ | Partial+structure |
| Theoretically bounded (safe) | Depends | ✓ | ✓ | ✓ |
| Consumer hardware feasible | ✓ overnight | ✓ always | ✓ always | ✓ |
| Metacognitive self-model | ✗ | ✗ | ✗ | ✓ (MHE) |

No existing paper combines all columns. Professor X is the first implementation.

### The 4-baseline experimental table

Required for the paper (Table 1). Isolates each lever's contribution.

| Condition | Lever 1 (SDAR) | Lever 2 (ICE) | Lever 3 (evolved) | Prediction |
|-----------|---------------|---------------|-------------------|------------|
| **Baseline 1**: Stock qwen3:8b, no evolution | ✗ | ✗ | ✗ | Lowest absolute performance, HIRO(30) ≈ 0 |
| **Baseline 2**: SDAR qwen3:8b, no evolution | ✓ | ✗ | ✗ | Model-only ceiling — confirms Lever 1 gain |
| **Baseline 3**: Stock qwen3:8b + Professor X evolved | ✗ | ✓ | ✓ | Structural+contextual without parametric |
| **Target**: SDAR qwen3:8b + Professor X evolved | ✓ | ✓ | ✓ | All-lever combination — expected: superadditive |
| **Cloud ref**: GPT-4o one-time runs | — | — | — | Frontier ceiling for calibration (no evolution) |

**The superadditivity claim:** If Lever 1 gains X pp and Lever 3 gains Y pp independently, the combination should gain ≥ X + Y pp — because SDAR-improved model is a better proposer for Lever 3 (it generates better ChangeManifests), and Lever 3 structural improvements make Lever 1 fine-tuning data higher quality (failures are already diagnosed and addressed, so remaining failures are model-layer failures that fine-tuning can fix).

### MHE — Metacognitive Harness Evolution

MHE is the overarching frame that unifies the trifecta with the three levers. The system knows what it is, what it can do, and what's holding it back.

```
MHE loop:
  1. BF computes F(H_k) = [p_tool, p_plan, p_correct]  → "what am I good/bad at?"
  2. DHE traces failed tasks in weak categories           → "why am I failing there?"
  3. Attribution: which lever can fix this layer?
     - Layer 1-2 (retrieval/context) → Lever 2 + LCAP
     - Layer 3-4 (tool dispatch/execution) → Lever 3 (structural)
     - Layer 5 (reasoning) → Lever 1 (parametric, if pattern is pervasive)
  4. Apply targeted lever, record as EvolutionNode with lever_type: {1|2|3}
  5. Next round: verify if attribution was correct (DHE fix-prediction precision)
  6. Metacognitive self-model updated: what attribution patterns are reliable?
```

**The self-model store** lives in `memd.semantic` under the key prefix `mhe::`. Entries:
```rust
MetacognitiveEntry {
    round: u32,
    task_type: TaskType,
    predicted_layer: u8,          // DHE attribution
    predicted_lever: u8,          // 1, 2, or 3
    actual_improvement: f32,      // delta F_i(H_{k+1} - H_k) for this task type
    attribution_correct: bool,    // predicted_layer matched actual fix location
    confidence: f32,              // DHE confidence score at prediction time
}
```

The system can ask itself: "When I attributed a failure to Layer 3 (tool dispatch) and applied a Lever 3 fix, how often did the relevant task type actually improve next round?" This is **MCA (Metacognitive Calibration Accuracy)** — the self-model's reliability metric.

**Target:** Pearson r(MCA, IR) > 0.70 over 30 rounds. Interpretation: agents with more calibrated self-models improve faster.

---

*Compiled: 2026-05-23*
*Status: Pre-implementation. No Rust files written yet.*
*Next action: User reviews this document → switch to Linux machine → begin Week 1.*
*Trifecta (Section 14) + Three-Lever Framework (Section 15) implemented in Week 5 after core system is stable.*
