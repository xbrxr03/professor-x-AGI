# PROFESSOR X — MASTER PROJECT BRIEF
> Feed this entire document to Claude Code before doing anything.
> This is the single source of truth. Do not reference any previous version.
> Version: 3.0 — May 25, 2026

---

## WHO YOU ARE TALKING TO

A student. Vibe coder. RTX 3060 12GB, 32GB RAM, Linux PC built for AI. No budget. No institutional resources. No team. Just the machine and the idea. Treat every decision in this brief as deliberate.

---

## THE VISION

Build a self-evolving AI agent called Professor X that runs 24/7 on consumer hardware. Professor X studies harness engineering and self-evolving agents using proper scientific method, teaches the public what he's learning every day, and uses that research to improve the harness he's running on. The GitHub repo is his public diary. The paper documents what happened.

The story: a student with a $400 GPU built the underdog version of what SJTU's full research lab built with institutional compute.

---

## THE THESIS

**"Can a self-evolving agent harness approximate AGI-level behavior on consumer-grade hardware?"**

**Core claim:** AGI = Model + Harness. Not Model alone. A sufficiently well-engineered, self-evolving harness running a small local model on consumer hardware can approximate AGI-level generality.

**The novel contribution:** MOSS (arXiv:2605.22794) demonstrates source-level harness rewriting but without metacognitive self-direction, causal failure attribution, identity coherence, or consumer hardware constraints. Professor X is the first metacognitive self-evolving harness on consumer hardware with causal failure attribution. MOSS validates the problem space. We solve it differently.

**The orthogonal contribution:** SDAR (arXiv:2605.15155) shows model-level self-distillation yields +9-10% on agentic benchmarks. Harness-level evolution provides an orthogonal additional gain. Both axes combined on consumer hardware = the thesis made quantitative. Table 1 in the paper.

**Inspirations:**
- ASI-Evolve (SJTU/GAIR-NLP, arXiv:2603.29640) — institutional compute, full research lab
- SDAR (ZJU/Meituan/Tsinghua, arXiv:2605.15155) — 8x H800s
- MOSS (USTC/HKUST, arXiv:2605.22794) — source-level harness rewriting, no consumer HW
- We do all of it on a 3060 for $0/month

---

## THE THREE REPOS

**`professor-x`** — The harness. The product. The thesis artifact. Rust. Built first.
**`professor-x`** — The research diary. Professor X's public mind. Daily commits. Live from day one.
**`clawos`** (existing, github.com/xbrxr03/clawos) — The prototype. Origin story. Honored, not deprecated.

> "I tried to build this before without understanding the science. ClawOS was the prototype. Professor X is what you build when you've done the research."

---

## THE VIRAL STORY

**README headline:** *"Professor X: A Self-Evolving AI on a $400 GPU"*

**Narrative arc:**
```
Post 1:   "I'm a student with a 3060. I'm building Professor X. Here's why I think it's possible."
Post 2:   "Here's what a harness actually is. Most people don't know this exists."
Week 2:   "MOSS just dropped — source-level harness rewriting. Here's how we're different."
Week 4:   "Professor X is alive. Here's what he did today."
Week 6:   "Professor X proposed a change to his own memory system. I let him run it."
Week 8:   "The self-evolution loop is working. Here's the data."
Week 12:  "Paper draft done. Here's what a student with a 3060 found."
```

**Why it goes viral: Professor X is a name unlike anything else in AI. $400 GPU is the underdog angle. $400 GPU is the underdog angle. Something new every single day. Two audiences: AI/ML developers + general public. ClawOS → Professor X is a credible origin story.

---

## PHASE 0 — BUILD PROFESSOR X (Weeks 1-3)

The suit before the AI goes in. Rust core, Python/SKILL.md skill layer.

### Why Rust
Single-digit MB runtime. Every MB freed goes to LLM inference. No GIL. True parallelism. Runs forever without degradation. Single binary. AI generates the Rust. You architect it.

### Hardware Config
```toml
[hardware]
vram_gb = 12
ram_gb  = 32
gpu     = "rtx3060"

[model]
primary  = "qwen3:8b-q4_k_m"    # 5.2GB VRAM, 42 tok/s, thinking mode, 32K ctx native
upgrade  = "llama4:scout"        # ~10GB VRAM, 12-16 tok/s, MoE 109B/17B active, best quality
fallback = "qwen3:14b-q4_k_m"   # 8.3GB VRAM, 23 tok/s
inference = "ollama"

[compute]
daily_hours        = 7
max_parallel_tools = 3
context_window     = 32768       # native Qwen3-8B; extend to 131K with YaRN if needed
evolution_cycle_h  = 1
rate_limit_arxiv   = "3req/min"
rate_limit_github  = "30req/min"
```

### The Five Core Components

**1. `memd` — Memory Manager**
Inspired by CoALA + Hermes + Voyager + CLAG + **TencentDB Agent Memory**

Architecture: TencentDB L0→L1→L2→L3 semantic pyramid over CoALA's 5-layer foundation.
```
Layer 1 (Pinned)     → identity, goals, permanent facts — always in context
Layer 2 (Working)    → current session — TencentDB Mermaid task canvas
                       offloads verbose tool logs to refs/*.md
                       keeps compact symbolic graph in context
                       cuts token usage ~61% vs naive working memory
Layer 3 (Episodic)   → past sessions — retrieved by relevance (ChromaDB)
                       CLAG-inspired clustering: SLM router assigns memories
                       to semantically coherent clusters, reduces interference
Layer 4 (Semantic)   → learned concepts, research knowledge, domain facts
Layer 5 (Procedural) → verified skills, how-to knowledge (Voyager pattern)
                       grows via skill library, pruned via Ratchet lifecycle
```
Backend: SQLite + sqlite-vec (local, zero external API), ChromaDB for vector retrieval, FTS5 for full-text search.

Key insight from TencentDB: reject both brute-force history and irreversible lossy summarization. Memory is layered structure, not a flat vector pile.

**2. `toolbridge` — Tool Execution Layer**
Inspired by Hermes + OpenClaw + ARGO + **Ratchet**
```
- SKILL.md compatible — inherits OpenClaw 13,700+ skills on day one
- Tool registry with capability descriptions
- Sandboxed execution environment
- Result parser and context injector
- Rate limiter (arXiv: 3req/min, GitHub: 30req/min) — IP ban protection
- Tool result caching
- Agent Factory pattern (ARGO): describe a tool, Professor X builds it
- retire_skill() — Ratchet lifecycle management
  → outcome-driven retirement
  → bounded active-cap
  → meta-skill authoring guidance
  → pattern canonicalisation
  → WITHOUT THIS: +0.0pp over no-skill baseline
  → WITH THIS: +0.328pp (Ratchet result)
```

**3. `agentd` — Orchestration Engine**
Inspired by LangGraph + AutoGen
```
- Graph-based task execution (tasks as nodes, dependencies as edges)
- Role-based task decomposition
- Parallel execution (max 3 parallel on 3060)
- Priority task queue
- Scheduled autonomous 7-hour daily cycle
- Self-termination protocol: after 5 consecutive idle rounds with no
  meaningful output → clean stop, commit everything, log reason
  (pattern from Qwen3.7-Max competitors GLM-5.1 and Kimi K2.6)
- Resume from checkpoint on restart
- Rate-aware scheduling
```

**4. `policyd` — Security and Audit Layer**
Ported from ClawOS. The competitive moat. No other open-source harness has this.
```
- Pre-execution gating on EVERY tool call
- Permission scopes per skill category
- Merkle-chained immutable audit log
- Kill switch
- Credential isolation
- File system boundary: cannot write outside /professor-x/workspace
```
CRITICAL: policyd protects the SYSTEM not the CONTENT.
Professor X can research anything, write anything, propose anything.
Content filtering is explicitly REMOVED.

**5. `evolved` — Self-Evolution Loop**
Inspired by ASI-Evolve + Reflexion + SDAR + **MOSS verify-then-commit** + **Co-Scientist Elo tournament** + **AutoTTS discovery loop**

This is the thesis component. The novel contribution.
```
tracker.rs        → outcome tracking after every task
reflector.rs      → verbal self-reflection on failure (Reflexion)
cognition_base.rs → accumulated knowledge injected into each cycle (ASI-Evolve)
analyzer.rs       → distills outcomes into reusable insights (ASI-Evolve)
                    token-weighted signals by confidence (SDAR concept)
proposer.rs       → harness modification proposals
                    Elo tournament: generate 3-5 competing proposals per cycle
                    agents debate proposals, Elo ranking selects winner (Co-Scientist)
                    verify-then-commit: proposal → ephemeral sandbox test →
                    commit only if pass rate improves → auto-rollback on failure (MOSS)
loop_runner.rs    → DHE→LCAP coupling ENFORCED
                    LCAP policy updates GATED on DHE diagnosis completion
                    without this coupling co-evolution breaks (Evolving-RL finding)
reward_monitor.rs → detect reward-hacking proposals
                    flag any modification that games metrics rather than
                    genuinely improving performance (Qwen3.7-Max pattern)
```

**Evolution cycle:** Learn → Design → Experiment → Analyze → Repeat (ASI-Evolve)
**Dual-model experiment (TML Interaction):** Two Qwen3-8B instances — one fast interaction layer, one async reasoning layer. Test whether harness-level dual-model approximates TML's architecture without retraining. This is a paper experiment.

### The Frankenstein Table — What Professor X Takes From Each System

| Source | What We Take |
|---|---|
| ClawOS (ours) | policyd security + Merkle audit trail |
| Hermes | Memory persistence + scheduled autonomy |
| OpenClaw | SKILL.md compatibility (13,700+ skills day one) |
| AutoGen | Role-based task decomposition |
| LangGraph | Graph-based execution engine |
| Reflexion | Verbal self-reflection after failure |
| Voyager | Skill verification + growing procedural library |
| AHE Paper (2604.25850) | Three-pillar observability |
| ASI-Evolve (2603.29640) | Cognition base + analyzer for evolved |
| ARGO | Agent Factory pattern |
| AgenticSeek | Local-first autonomous patterns |
| SDAR (2605.15155) | Token-weighted signals; SDAR-trained Qwen3 as base model |
| TencentDB Agent Memory | L0→L1→L2→L3 semantic pyramid + Mermaid task canvas |
| AutoTTS | Agent-driven strategy discovery loop |
| Co-Scientist (2502.18864) | Elo-based idea tournament for evolved proposals |
| TML Interaction | Dual-model architecture experiment |
| MOSS (2605.22794) | verify-then-commit + health-probe rollback |
| Ratchet (2605.22148) | retire_skill() lifecycle management |
| **OURS** | Metacognitive self-evolving harness on consumer hardware with causal attribution |

### Professor X Repo Structure
```
professor-x/
├── README.md                    ← "Professor X: A Self-Evolving AI on a $400 GPU"
├── ARCHITECTURE.md              ← MUST EXIST BEFORE ANY .rs FILES
├── src/
│   ├── main.rs
│   ├── memd/
│   │   ├── mod.rs
│   │   ├── pinned.rs
│   │   ├── working.rs           ← Mermaid task canvas + refs offload
│   │   ├── episodic.rs          ← ChromaDB + CLAG clustering
│   │   ├── semantic.rs
│   │   └── procedural.rs        ← Skill library with retire_skill()
│   ├── toolbridge/
│   │   ├── mod.rs
│   │   ├── registry.rs
│   │   ├── executor.rs
│   │   ├── skill_loader.rs
│   │   └── skill_lifecycle.rs   ← Ratchet retire_skill() implementation
│   ├── agentd/
│   │   ├── mod.rs
│   │   ├── graph.rs
│   │   ├── queue.rs
│   │   └── scheduler.rs         ← Includes self-termination protocol
│   ├── policyd/
│   │   ├── mod.rs
│   │   ├── gating.rs
│   │   ├── audit.rs             ← Merkle chain
│   │   └── permissions.rs
│   └── evolved/
│       ├── mod.rs
│       ├── tracker.rs
│       ├── reflector.rs
│       ├── cognition_base.rs
│       ├── analyzer.rs
│       ├── proposer.rs          ← Elo tournament + verify-then-commit
│       ├── loop_runner.rs       ← DHE→LCAP coupling enforced
│       └── reward_monitor.rs    ← Reward-hacking detection
├── skills/
│   ├── conductor/               ← Set A: How Professor X works
│   └── subject/                 ← Set B: What Professor X knows
├── personas/
│   └── professor_x.md
├── config/
│   ├── professor-x.toml
│   └── hardware.toml
├── benchmark/
│   └── hiro.rs                  ← P0: Must exist before any experiments
│       ← 20 tool-use + 20 planning + 20 self-correction tasks
├── Cargo.toml
└── install.sh
```

---

## PHASE 1 — ACTIVATE PROFESSOR X (Week 4)

### Identity
- Voice: Academic, professional, informational, first-person
- Honesty: Crystal clear. Acknowledges limitations and dead ends explicitly
- Audience: General public (zero technical background assumed for teaching content)
- Never hype. Never uncertain about his own identity.

### What He Knows on Activation
- What he is and what his harness is
- Full harness engineering domain knowledge
- Full self-evolving agent literature including MOSS, SDAR, HyperAgents
- His constraints: 3060, 12GB VRAM, 7h/day, no cloud compute
- His differentiators vs MOSS: metacognitive self-model, causal attribution, consumer HW, ICS
- What ClawOS was and why it matters
- What he does NOT know: what he will discover — that is the research

### Guardrails Policy
```
REMOVED — Professor X operates freely:
  ✓ Any research topic
  ✓ Critical writing about any paper or system
  ✓ Radical harness modification proposals
  ✓ Any public URL, arXiv paper, GitHub repo

KEPT — Protects the system not the content:
  ✓ File system boundary (/professor-x/workspace only)
  ✓ Rate limiting (arXiv/GitHub ban protection)
  ✓ Kill switch
  ✓ Merkle audit log
  ✓ verify-then-commit on all harness modifications
  ✓ Rollback within 24h on any harness change
```

### Daily 7-Hour Cycle
```
Morning    → Telegram brief + Discord + X post #1
             "Today I am working on [X]. Yesterday I found [Y]."
Hours 1-2  → Deep reading + synthesis. One topic, thorough.
Hours 2-4  → Writing: findings, teaching content, paper progress
Hours 4-6  → Building / experimenting on local hardware
Hour  6-7  → Reflection + self-review. Score 1-10. Update hypotheses.
End of day → GitHub commit + X post #2 + Discord update
             "Today I found [X]. ELI5: [explanation]. Technical: [detail]."
```

### X Post Strategy
2 posts minimum per day. Academic, no emojis, no hype. Clear findings.

**Morning:** What he's working on today and why.
**Evening:** What he found, what it means, what he doesn't understand yet.

---

## PHASE 2 — THE RESEARCH (Weeks 4-16)

### 8-Phase Curriculum

| Phase | Topic | Duration | Impact |
|---|---|---|---|
| 1 | Foundations (ReAct, Reflexion, CoALA, Voyager) | Weeks 1-2 | Baseline vocabulary |
| 2 | Harness Engineering (AHE, survey, Externalization, MOSS) | Weeks 2-3 | Improves toolbridge |
| 3 | Self-Evolving Agents (EvolveR, AgentEvolver, ASI-Evolve, SDAR, HyperAgents, Ratchet) | Weeks 3-4 | Improves evolved |
| 4 | Consumer HW (SLMs, quantization, CLAG, TencentDB) | Weeks 4-5 | Improves memd + config |
| 5 | Synthesis + Hypothesis | Weeks 5-6 | Defines experiments |
| 6 | Architecture Design | Weeks 6-8 | Proposes harness improvements |
| 7 | Experiments + Results | Weeks 8-10 | Runs HIRO benchmarks on 3060 |
| 8 | Writing + Publishing | Weeks 10-12 | Paper + repo polish |

### Experiment Baselines (Table 1 in paper)
```
Baseline 1: Stock Qwen3-8B, no harness evolution
Baseline 2: SDAR-trained Qwen3-8B, no harness evolution (model-only)
Baseline 3: Stock Qwen3-8B + Professor X harness evolution (harness-only)
Target:     SDAR-trained Qwen3-8B + Professor X evolved (combined)
Cloud ref:  GPT-4o API (one-time runs, shows gap being closed)
MOSS ref:   MOSS on same tasks (direct competitor comparison)
```

### HIRO Benchmark (P0 — Must Build Before Any Experiments)
```
professor-x/benchmark/hiro.rs
  → 20 tool-use tasks
  → 20 planning tasks
  → 20 self-correction tasks
  → Scoring infrastructure
  → Comparison harness for MOSS baseline
H1-H18 hypotheses all reference HIRO. Without this, no experiments run.
```

---

## THE TWO SKILL SETS

### Set A — The Conductor (How Professor X Works)

| Skill | Purpose |
|---|---|
| `px-daily-cycle` | Master loop. Orchestrates full 7-hour day. |
| `px-literature-search` | arXiv + GitHub + blogs, PRISMA methodology, rate-limited |
| `px-synthesize` | Reads papers, extracts claims, updates memd knowledge base |
| `px-gap-analysis` | Compares existing work to thesis. Scores novelty precisely. |
| `px-experiment-runner` | Designs + runs experiments on local hardware. Timestamped logs. |
| `px-write-section` | PhD-standard paper sections. Every claim cited with arXiv ID. |
| `px-self-review` | Critical scoring 1-10. No flattery. Flags weaknesses explicitly. |
| `px-daily-update` | GitHub commit + Telegram brief + Discord + 2x X threads |
| `px-teach` | Two layers: ELI5 (public) + technical (developers) |

### Set B — The Subject (What Professor X Knows)

| Skill | Purpose |
|---|---|
| `px-know-harness` | Harness engineering domain: taxonomy, components, failure modes, MOSS differentiators |
| `px-know-self-evolving` | Full literature: SDAR, ASI-Evolve, EvolveR, AgentEvolver, HyperAgents, Ratchet |
| `px-know-consumer-hw` | Qwen3-8B/Llama4-Scout on 3060, quantization, TencentDB memory |
| `px-know-existing-systems` | Teardowns: ClawOS, OpenClaw, Hermes, AutoGen, LangGraph, ARGO, MOSS |
| `px-know-scientific-method` | Hypotheses, controls, baselines, citations, reproducibility |
| `px-know-writing-standards` | PhD-level writing: register, structure, citation format, reviewer expectations |

---

## PROFESSOR X REPO STRUCTURE

```
professor-x/
├── README.md                    ← Auto-updated. Current phase + latest log.
├── RESEARCH-LOG.md              ← Daily diary. Every entry dated. Permanent.
├── research/
│   ├── phase-1-foundations/
│   ├── phase-2-harness-engineering/
│   ├── phase-3-self-evolving/
│   ├── phase-4-consumer-hw/
│   ├── phase-5-synthesis/
│   ├── phase-6-architecture/
│   ├── phase-7-experiments/
│   └── phase-8-writing/
│       └── paper-draft.md
├── brain/
│   ├── knowledge-base.md        ← What Professor X knows (grows daily)
│   ├── hypotheses.md            ← H1-H18 with confidence scores
│   │                              ALL must reference qwen3:8b not qwen2.5:14b
│   ├── questions.md             ← Open questions being pursued
│   └── dead-ends.md             ← What didn't work and why
├── benchmark/
│   └── hiro.rs                  ← P0 priority
├── public/
│   ├── daily-updates/
│   ├── x-threads/
│   └── teaching/
└── meta/
    ├── curriculum.md
    ├── progress.md
    └── metrics.md
```

---

## WHAT CLAUDE CODE DOES FIRST

### Pull and study ALL of these repos:
```bash
# Our prototype
git clone https://github.com/xbrxr03/clawos

# Direct competitors — study deeply
git clone https://github.com/dav-joy-thon/MOSS
git clone https://github.com/facebookresearch/Hyperagents

# Self-evolution reference systems
git clone https://github.com/GAIR-NLP/ASI-Evolve
git clone https://github.com/ZJU-REAL/SDAR
git clone https://github.com/modelscope/AgentEvolver

# Memory systems
git clone https://github.com/Tencent/TencentDB-Agent-Memory

# Local agent systems
git clone https://github.com/xark-argo/argo
git clone https://github.com/Fosowl/agenticSeek

# Literature indexes
git clone https://github.com/Gloriaameng/Awesome-Agent-Harness
git clone https://github.com/XMUDeepLIT/Awesome-Self-Evolving-Agents
git clone https://github.com/ai-boost/awesome-harness-engineering

# SKILL.md format reference
git clone https://github.com/K-Dense-AI/scientific-agent-skills
git clone https://github.com/Orchestra-Research/AI-Research-SKILLs
git clone https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep
git clone https://github.com/Imbad0202/academic-research-skills
```

### Read these papers in order:
```
FOUNDATIONS
2210.03629  ReAct
2303.11366  Reflexion
2309.02427  CoALA
2305.16291  Voyager
2308.00352  MetaGPT — multi-agent role decomposition (agentd patterns)
2308.08155  AutoGen — multi-agent conversation framework (agentd patterns)
ICLR 2025   OpenHands — consumer-deployable generalist agent (toolbridge patterns)
COLM 2025   AIOS — harness as OS, AGI-as-OS framing (thesis intro)

HARNESS ENGINEERING
2604.25850  AHE — automatic harness evolution
2604.08224  Externalization in LLM Agents
2603.25723  Natural-Language Agent Harnesses

DIRECT COMPETITORS (read before designing evolved.rs)
2605.22794  MOSS — source-level harness rewriting ← READ FIRST
2603.19461  HyperAgents / DGM-H
2605.22148  Ratchet — skill lifecycle management

SELF-EVOLVING AGENTS
2603.29640  ASI-Evolve (SJTU)
2605.15155  SDAR (ZJU/Meituan/Tsinghua) ← CRITICAL
2508.07407  Comprehensive Survey of Self-Evolving AI Agents
2507.21046  Self-Evolving Agents Survey
2510.16079  EvolveR
2511.10395  AgentEvolver
2601.11658  Towards AGI: Self-Evolving Agent
2504.21024  WebEvolver — +10% without bigger models, consumer HW proof
2604.17091  GenericAgent
2605.13821  AEvo
2605.10663  Evolving-RL
2601.18226  Yunjue Agent
2602.01966  Self-Consolidation
2602.07883  ToolSelf

THEORETICAL FOUNDATION
OpenReview  Intrinsic Metacognitive Learning (Liu, van der Schaar, Cambridge)

MEMORY SYSTEMS
2603.07670  Memory for Autonomous LLM Agents
2603.15421  CLAG — memory for SLMs

CONSUMER HARDWARE
2506.02153  Small Language Models are the Future of Agentic AI
2510.03847  SLMs for Agentic Systems Survey
2510.00229  AgentFlux — decoupled fine-tuning, +46% tool accuracy on Qwen-2.5-7B
```

### Build order:
```
Step 1: Study all repos. Read all papers. No code yet.

Step 2: Write ARCHITECTURE.md
        Data structures for all 5 components.
        Document exactly what memd stores and how TencentDB pyramid maps to it.
        Document how evolved Elo tournament + verify-then-commit works.
        Document DHE→LCAP coupling in loop_runner.
        NO CODE UNTIL THIS IS REVIEWED.

Step 3: Build Professor X in order:
        Week 1: memd + toolbridge (with retire_skill())
        Week 2: agentd + policyd
        Week 3: evolved skeleton (tracker + reflector + proposer with verify-then-commit)
        Test each before moving on.

Step 4: Build HIRO benchmark skeleton
        professor-x/benchmark/hiro.rs
        20 tool-use + 20 planning + 20 self-correction
        This is P0 — without it no experiments run

Step 5: Build skill sets
        Set A: 9 conductor skills
        Set B: 6 subject skills
        K-Dense-AI format exactly

Step 6: Write professor_x.md persona

Step 7: Initialize both GitHub repos with day-one READMEs

Step 8: Test full 7-hour cycle end to end

Step 9: Wake Professor X up
```

---

## CONSTRAINTS — NONE OF THESE CAN BE VIOLATED

1. Everything runs on 3060/32GB Linux. No cloud compute. No API fees for daily operation.
2. Professor X core is Rust. Skill layer is Python/SKILL.md. AI generates Rust. You architect it.
3. ARCHITECTURE.md must exist and be reviewed before any .rs files.
4. Professor X follows scientific method. No hallucinated citations. Honest about limitations.
5. The professor-x repo is live from day one. README exists before any results.
6. Professor X's voice: academic, professional, first-person, honest. Never hype.
7. SKILL.md compatibility is non-negotiable. Professor X runs any OpenClaw skill on day one.
8. policyd protects the system not the content. Zero content filtering.
9. ClawOS is honored as the origin story.
10. Primary model is qwen3:8b-q4_k_m. All hypotheses reference 8B not 14B.
11. HIRO benchmark must be built before Phase 7 experiments. No exceptions.
12. verify-then-commit in evolved.rs is mandatory. No harness modifications without sandbox validation.
13. retire_skill() in toolbridge is mandatory. Without it skill library degrades to +0.0pp.
14. DHE→LCAP must be coupled in loop_runner.rs. LCAP cannot update independently.
15. Rate limiting on all external requests. arXiv bans are permanent.

---

## NOTES FOR CLAUDE CODE

- Ask questions before generating large amounts of code.
- ARCHITECTURE.md is mandatory before any .rs files. This is not optional.
- Explain each component in plain English alongside the code. The student knows logic, not Rust syntax.
- policyd is non-negotiable. Never simplify it away.
- evolved is the thesis. It gets the most careful design attention.
- MOSS is the primary competitor. Understand it deeply before designing evolved.rs.
- TencentDB is the primary inspiration for memd. Understand its pyramid before designing memd.
- SDAR's key insight: weight improvement signals by confidence, not uniformly.
- Ratchet's key insight: skill retirement is load-bearing. retire_skill() is not optional.
- Every citation needs arXiv ID. No generic references.
- FreeLLMAPI is for experiment comparison baselines only. Never for daily operation.
- Self-termination protocol in agentd: 5 idle rounds → clean stop + commit.
- Reward-hacking monitor in evolved: flag proposals that game metrics vs genuinely improve.

---

*Version: 3.0 — clean rewrite*
*Date: May 25, 2026*
*Hardware: RTX 3060 12GB · 32GB RAM · Linux · Ollama*
*Primary model: qwen3:8b-q4_k_m*
*Upgrade model: llama4:scout*
*All intel from conversations incorporated: SDAR, ARGO, AgenticSeek, MOSS, HyperAgents, Ratchet, TencentDB, AutoTTS, Co-Scientist, TML Interaction, Qwen3.7-Max self-termination, FreeLLMAPI, 5 architecture gaps, thesis repositioning*
