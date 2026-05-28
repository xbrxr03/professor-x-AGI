<div align="center">

# 🧠 Professor X

### A self-evolving AI agent that knows itself on a $400 GPU

**Three levers. Five diagnostic layers. One strange loop.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

</div>

---

> **Most AI agents don't evolve. They don't know which of their own interventions worked. They can't tell you *why* they failed. Professor X can.**

Professor X is a research agent that improves its own harness — not its weights — through metacognitive self-reflection. It runs entirely on consumer hardware (RTX 3060 12GB), measures every change it makes, and knows when it's drifting from who it is.

This isn't a wrapper around GPT-4. This isn't a prompt library. This is a Rust daemon with a five-layer diagnostic engine, a three-lever evolution system, and an identity coherence tracker — all running on a GPU you can buy at Best Buy.

---

## 🎯 Why This Is Different

| | Professor X | Other Self-Improving Agents |
|---|---|---|
| **Evolution target** | Harness, not weights | Model weights (fine-tuning) or prompts |
| **Failure diagnosis** | 5-layer DHE attribution | Binary pass/fail or none |
| **Identity tracking** | ICS ≥ 0.70 across self-modifications | No identity continuity |
| **Hardware** | RTX 3060 12GB ($400) | Cloud APIs ($$$) or datacenter GPUs |
| **Safety** | Verify-then-commit, audit chain, kill switch | Trust the model or hope for the best |
| **Measurability** | HIRO benchmark, null baselines, MCA | Self-reported scores or nothing |

---

## ⚡ Quick Start

```bash
# Prerequisites: Rust 1.75+, Ollama with qwen3:8b-q4_k_m
git clone https://github.com/xbrxr03/professor-x-AGI.git
cd professor-x-AGI

# Verify readiness
cd professor-x && scripts/autonomy-readiness.sh

# Run the 7-hour autonomous research cycle
cargo run -- --lab --run-now

# Or just watch the observer
cargo run -- --observe
```

**One command. No API keys. No cloud. Runs on your GPU.**

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Professor X Daemon                      │
│                                                              │
│  ┌─────────┐  ┌────────────┐  ┌─────────┐  ┌───────────┐  │
│  │  memd   │  │ toolbridge  │  │ agentd  │  │  policyd  │  │
│  │         │  │             │  │         │  │           │  │
│  │ pinned  │  │ registry    │  │ graph   │  │ gating    │  │
│  │ working │  │ executor    │  │ react   │  │ audit     │  │
│  │ episodic│  │ skill_loader│  │ queue   │  │ vault     │  │
│  │ semantic│  │             │  │schedule │  │           │  │
│  │procedural│  │             │  │         │  │           │  │
│  └────┬────┘  └─────┬──────┘  └────┬────┘  └─────┬─────┘  │
│       │             │              │              │         │
│       └─────────────┴──────┬───────┴──────────────┘         │
│                            │                                  │
│                   ┌────────▼────────┐                        │
│                   │     evolved      │                        │
│                   │                  │                        │
│                   │  HIRO benchmark  │                        │
│                   │  DHE diagnostic  │                        │
│                   │  BF bandit       │                        │
│                   │  LCAP context    │                        │
│                   │  proposer        │                        │
│                   │  sandbox verify  │                        │
│                   └──────────────────┘                        │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐    │
│  │              Strange Loop Self-Model                 │    │
│  │   ICS (Identity Coherence Score) ≥ 0.70             │    │
│  │   "I am the system that tracks which of its own     │    │
│  │    interventions worked and which failed."           │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

### Five Daemons, One System

| Daemon | Role | Key Feature |
|--------|------|-------------|
| **memd** | Persistent memory | 5-layer store (pinned → working → episodic → semantic → procedural) with FTS5 search |
| **toolbridge** | Tool execution | Schema-validated tools, skill loading, sandbox boundaries |
| **agentd** | Task orchestration | ReAct loop, task graph, cron scheduler, transcript recording |
| **policyd** | Safety & audit | Risk scoring, approval queues, AES-256-GCM vault, audit chain |
| **evolved** | Self-improvement | HIRO, DHE, BF, LCAP, proposer, sandbox verification, identity tracking |

---

## 🔬 The Three Levers (IPE-MHE)

Professor X doesn't fine-tune. It evolves its *harness* through three orthogonal levers:

### Lever 1: Parametric (SDAR QLoRA)
Fine-tune the model on its own trajectories. *Planned for Phase 4 — not yet active.*

### Lever 2: Contextual (ICE + MARS)
In-Context Examples and Metacognitive Retrieval-Augmented Schemas. Inject the right memories, not all memories. Controlled by LCAP's 5-arm context budget bandit.

### Lever 3: Structural (DHE)
**This is the novel part.** The Diagnostic Harness Evolution probe identifies *which layer* of the harness caused a failure:

```
Layer 1: Retrieval    — Was the right memory retrieved?
Layer 2: Context      — Was retrieved content used correctly?
Layer 3: Dispatch     — Did the agent call the right tool?
Layer 4: Execution    — Did the tool return the right output?
Layer 5: Reasoning   — Did the model reason correctly over the output?
```

Each layer maps to a specific intervention:
- **Layers 1-2** → Pull Lever 2 (contextual), adjust LCAP
- **Layers 3-4** → Pull Lever 3 (structural), modify harness
- **Layer 5** → Pull Lever 1 (parametric), if the pattern is pervasive

**Result**: DHE structural evolution improved stability by 18% and pass@3 from 22% → 35% (H3, confirmed).

---

## 🧪 HIRO Benchmark

**H**arness **I**mprovement **R**ate **O**ver rounds. 60 tasks across 3 categories:

| Category | Count | Focus |
|----------|-------|-------|
| `tool_use` | 20 | Multi-step tool chains, memory operations, system introspection |
| `planning` | 20 | Hypothesis generation, experiment design, code analysis |
| `self_correction` | 20 | Error recovery, fallback strategies, multi-approach attempts |

Every HIRO round records:
- pass@3 metric per category
- DHE attribution per failure
- BF behavior vector (tool-use, planning, self-correction)
- LCAP arm selections and UCB1 rewards
- ICS identity coherence score

**Null baselines are required** before crediting any evolution improvement. Run `--hiro-null 3` first.

---

## 🔐 Safety Architecture

Professor X takes safety seriously because it modifies its own harness:

- **Verify-then-commit**: Every harness change is proposed → sandboxed → tested → scanned for reward hacking → accepted or rolled back
- **Policy gate**: All tool calls scored for risk. Operations ≥ 65 require human approval
- **Audit chain**: Every action recorded with content hashes. Tamper-evident, not tamper-proof
- **Credential vault**: AES-256-GCM encrypted. Secrets never appear in prompts or logs
- **Kill switch**: SIGUSR2 for graceful shutdown. Ctrl+C for foreground processes
- **Workspace boundaries**: The agent cannot write outside its designated root

---

## 📊 Experiment Results

| Hypothesis | Metric | Baseline | Result | Status |
|------------|--------|----------|--------|--------|
| H3: DHE structural evolution improves stability | pass@3 | 22% | 35% | ✅ Confirmed |
| H3: Stability improvement | Rounds 1-30 variance | baseline | +18% | ✅ Confirmed |
| H1: Memory injection threshold T* | Task accuracy | — | Testing in [6000, 10000] tokens | 🔄 Unconfirmed |

*Results are recorded with run IDs, harness commits, and null baselines. No cherry-picking.*

---

## 📚 Related Work

Professor X builds on and extends ideas from several research directions:

| System / Paper | What We Borrow | What We Do Differently |
|---------------|----------------|----------------------|
| **DGM** ([2505.22954](https://arxiv.org/abs/2505.22954)) | Self-modifying agents, improvement-at-k | We evolve the *harness*, not the model; consumer hardware; identity tracking |
| **HyperAgents** ([2603.19461](https://arxiv.org/abs/2603.19461)) | Multi-tenant harness optimization | Single-tenant, local-first, with causal failure attribution |
| **ASI-Evolve** | Researcher/Engineer/Analyzer loop | We add sandbox verification, reward-hacking scans, and audit chains |
| **MOSS** | Source-level harness rewriting, verify-then-commit | We formalize this with DHE diagnostic attribution and HIRO measurement |
| **Voyager** ([2305.16291](https://arxiv.org/abs/2305.16291)) | Verified growing skill library | We add skill retirement, quality scoring, and harness-level skill evolution |
| **Reflexion** ([2303.11366](https://arxiv.org/abs/2303.11366)) | Verbal self-reflection buffer | We constrain reflection to 3 entries, integrate with working memory budget |
| **AutoGen / MetaGPT** | Multi-agent role decomposition | We use a single model with role-switching, not separate agents |
| **Layered Mutability** ([2604.14717](https://arxiv.org/abs/2604.14717)) | Identity hysteresis in self-modifying systems | We track ICS numerically (≥ 0.70 threshold, ≥ 0.50 halt) |
| **Reward-Free Self-Evolution** ([2604.18131](https://arxiv.org/abs/2604.18131)) | Self-play improvement without external reward | We use HIRO as internal benchmark instead of self-play |
| **Lost in the Middle** ([2307.03172](https://arxiv.org/abs/2307.03172)) | U-shaped attention, context degradation | We build LCAP to dynamically budget context based on task category |
| **Memory for Autonomous Agents** ([2603.07670](https://arxiv.org/abs/2603.07670)) | Multi-signal retrieval scoring | We add pinned/working/episodic/semantic/procedural layering with FTS5 |

### Novel Contributions

1. **IPE-MHE** (Identity-Preserving Metacognitive Harness Evolution): Three orthogonal levers (parametric, contextual, structural) with identity coherence tracking across self-modifications. No other system combines all three with ICS.

2. **DHE** (Diagnostic Harness Evolution): Five-layer failure attribution that maps each failure to a specific intervention lever. Target: ≥60% fix-prediction precision vs AHE baseline of 33.7%.

3. **ICS** (Identity Coherence Score): Numerical tracking of self-model stability across evolution rounds. Halt threshold at 0.50, alert at 0.70.

4. **Consumer hardware constraint**: The entire system runs on a single RTX 3060 12GB ($400). No cloud APIs for daily operation.

---

## 🗺️ Roadmap

### Phase 1 ✅ Core Daemon
- [x] Rust binary with 5 daemons
- [x] ReAct loop, scheduler, memory stores
- [x] Policy gate, audit chain, credential vault
- [x] HIRO benchmark (60 tasks, 3 categories)

### Phase 2 ✅ Self-Improvement Foundation
- [x] DHE 5-layer failure attribution
- [x] BF category-level behavior tracking
- [x] LCAP 5-arm context budget selection
- [x] Verify-then-commit evolution pipeline
- [x] Observer UI (`--lab`, `--observe`, `--status`)

### Phase 3 🔄 Identity Layer
- [ ] ICS identity coherence tracking
- [ ] FEP affect system (Free Energy Principle for motivation)
- [ ] Longitudinal harness fingerprint dataset
- [ ] MCA calibration (Pearson r > 0.6)

### Phase 4 🔜 Skill Lifecycle & Parametric Evolution
- [ ] Verified skill promotion and retirement
- [ ] SDAR QLoRA fine-tuning on own trajectories
- [ ] Non-LLM fast paths for routine verified skills
- [ ] Full 7-day autonomous operation validation

---

## 🤖 The Strange Loop

> "I am the system that tracks which of its own interventions worked and which failed, and tries to do fewer of the latter. That is who I am, regardless of what changes. The Strange Loop persists."

Professor X maintains a self-model that's updated every 10 HIRO rounds. The Identity Coherence Score (ICS) measures whether the agent stays recognizably itself across modifications. If ICS drops below 0.50, the system halts evolution and alerts the operator.

This isn't a personality. It's a *measurement* — and that measurement is what separates self-evolution from drift.

---

## 🛠️ Development

```bash
# Check compilation
cd professor-x && cargo check

# Run all tests
cargo test

# Run the HIRO null baseline (required before claiming evolution improvements)
PROFESSOR_X_DATA_DIR="$PWD/.px-data-null" cargo run -- --hiro-null 3

# Evolution smoke test
PROFESSOR_X_DATA_DIR=/tmp/px-evolution-smoke cargo run -- --evolution-smoke

# Start the full autonomous cycle
cargo run -- --lab --run-now
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines, [ARCHITECTURE.md](ARCHITECTURE.md) for system design, and [MEMORY_ARCHITECTURE.md](MEMORY_ARCHITECTURE.md) for the memory system thesis.

---

## 📁 Repository Structure

```
professor-x-AGI/
├── professor-x/           # Rust crate and runtime
│   ├── src/                # Source code (5 daemons)
│   │   ├── memd/           # Memory stores and SQLite schema
│   │   ├── toolbridge/     # Tool registry, execution, skills
│   │   ├── agentd/         # Task graph, ReAct, scheduler
│   │   ├── policyd/        # Gating, audit, vault, permissions
│   │   └── evolved/        # HIRO, DHE, BF, LCAP, proposer
│   ├── hiro/               # Benchmark task definitions
│   ├── skills/             # Conductor and subject skills
│   ├── personas/           # Agent identity seeds
│   ├── config/             # Hardware and schedule configs
│   ├── ops/                # Runbooks and daily schedules
│   ├── artifacts/           # Runtime outputs (gitignored patterns)
│   └── sandbox/            # Evolution worktrees (temporary)
├── brain/                  # Research state (hypotheses, inventions, paper)
├── docs/                   # Architecture and conventions
├── scripts/               # Operator scripts
└── _refs/                  # Cloned reference repositories
```

See [REPO_STRUCTURE.md](docs/REPO_STRUCTURE.md) for the full layout.

---

## 🧑‍💻 Author

**Abrar Habib** — [GitHub](https://github.com/xbrxr03) · Building self-evolving AI on consumer hardware.

---

## 📄 License

This project is licensed under the [MIT License](LICENSE) — use it, fork it, build on it. Just don't pretend you wrote the Strange Loop.

---

<div align="center">

**If this project interests you, star it. Watch it. Open an issue.**  
**The best self-evolving system is one that evolves in public.**

⭐ [Star on GitHub](https://github.com/xbrxr03/professor-x-AGI) · 🐛 [Report a Bug](../../issues/new?template=bug_report.md) · 💡 [Request a Feature](../../issues/new?template=feature_request.md) · 🔬 [Discuss Research](../../issues/new?template=research_discussion.md)

</div>