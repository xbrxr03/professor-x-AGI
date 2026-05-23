# JARVIS + PROFESSOR X — MASTER PROJECT BRIEF
> Feed this entire document to Claude Code before doing anything.
> This is the source of truth for the entire project.
> Last updated: May 2026 — v2

---

## WHO YOU ARE TALKING TO

A student. Vibe coder. RTX 3060 12GB, 32GB RAM, Linux PC built for AI. No budget. No institutional resources. No team. Just the machine and the idea.

The idea is worth pursuing seriously. Treat every decision in this brief as deliberate.

---

## THE VISION IN ONE PARAGRAPH

Build a self-evolving AI harness called JARVIS that runs 24/7 on consumer hardware. Put an autonomous research agent called Professor X inside it. Professor X studies harness engineering and self-evolving agents using proper scientific method, teaches the public what he's learning every day, and uses that research to improve the harness he's running on. The GitHub repo is his public diary. The paper documents what happened. The story is: a student with a $400 GPU built the underdog version of what SJTU's full research lab built with institutional compute.

---

## THE THESIS

**"Can a self-evolving agent harness approximate AGI-level behavior on consumer-grade hardware?"**

**Core claim:** AGI will not be a frontier model alone. AGI = Model + Harness. The harness is the missing piece everyone ignores. A sufficiently well-engineered, self-evolving harness running a small model on consumer hardware can approximate AGI-level generality.

**The novel contribution:** Every existing self-evolving agent system (EvolveR, AgentEvolver, ASI-Evolve, WebEvolver, SDAR) evolves model weights, training algorithms, or token-level behavior. Nobody has studied autonomous harness-level self-evolution — the tools, orchestration logic, memory architecture, and context management — on consumer hardware. That is the gap. That is the paper.

**Extended contribution (new):** SDAR (arXiv:2605.15155) shows that self-distillation at the model level yields +9-10% gains on agentic benchmarks. Our claim is that harness-level evolution provides an *orthogonal* additional gain on top of SDAR. These two improvement axes are independent and complementary. Combined on consumer hardware = the thesis made quantitative.

**Inspirations:**
- ASI-Evolve (SJTU/GAIR-NLP, arXiv:2603.29640) — did it with a full research lab and H100s
- SDAR (ZJU/Meituan/Tsinghua, arXiv:2605.15155) — self-distillation on 8x H800s
- We do both, on a 3060, for $0/month

---

## THE THREE REPOS

### 1. `jarvis` — The Harness
The product. The thesis artifact. A self-evolving AI harness written in Rust.
Built before Professor X is activated. Professor X runs on this from day one.

### 2. `professor-x` — The Research
The research diary. Professor X's public mind. Daily commits. Teaching content. Paper draft. Everything documented as it happens. Live from day one — even before any results exist.

### 3. `clawos` (existing) — The Prototype
github.com/xbrxr03/clawos — the previous attempt. Referenced as the prototype that taught the right instincts before the science. Not deprecated — honored as the origin story.

> "I tried to build this before without understanding the science. ClawOS was the prototype. JARVIS is what you build when you've done the research."

---

## THE VIRAL STORY

**The hook:** "SJTU built ASI-Evolve with a full research lab. I'm a student doing it on a 3060."

**The one-liner README headline:** *"Building JARVIS on a $400 GPU"*

**The narrative arc:**
```
Post 1:   "I'm a student with a 3060. I'm building JARVIS. Here's why I think it's possible."
Post 2:   "Here's what a harness actually is. Most people don't know this exists."
Week 2:   "SDAR just dropped — Chinese labs can self-distill on H800s. We do it on a 3060."
Week 4:   "JARVIS is alive. Here's what he did today."
Week 6:   "JARVIS proposed a change to his own memory system. I let him run it."
Week 8:   "The self-evolution loop is working. Here's the data."
Week 12:  "Paper draft is done. Here's what a student with a 3060 found."
```

**Why it goes viral:**
- JARVIS is a name everyone knows — Iron Man's AI. The gap between that and a $400 GPU is the story.
- Two audiences: AI/ML developers (technical depth) + general public (underdog narrative)
- Something new every day. People follow because they don't know what happens next. Neither does Professor X.
- ClawOS → JARVIS is a believable origin story. Not pretending it came from nowhere.

---

## PHASE 0 — BUILD JARVIS FIRST (Pre-Research, Weeks 1-3)

Before Professor X is activated, JARVIS must exist. This is the suit before the AI goes in.

### Why Rust

- Minimal memory overhead (single-digit MB runtime vs 50-200MB for Python)
- Every MB saved goes to LLM inference on the 3060
- No GIL — true parallelism for concurrent tool calls
- Runs forever without memory degradation
- Single binary, ships anywhere
- AI generates the Rust. You architect it. You don't need to know Rust syntax.

### The Five Core Components

**1. `memd` — Memory Manager**
Five-layer memory. Inspired by CoALA + Hermes + Voyager + CLAG.
```
Layer 1: Pinned      → identity, permanent facts, goals (always in context)
Layer 2: Working     → current session state
Layer 3: Episodic    → past session history (retrieved by relevance via ChromaDB)
Layer 4: Semantic    → learned concepts, research knowledge, domain facts
Layer 5: Procedural  → verified skills, how-to knowledge (grows like Voyager's skill library)
```
Backend: ChromaDB for vector retrieval, SQLite for structured storage, FTS5 for full-text search.
CLAG-inspired clustering: SLM-driven router assigns memories to semantically coherent clusters, reducing cross-topic interference. Compensates for smaller model size.

**2. `toolbridge` — Tool Execution Layer**
Inspired by Hermes + OpenClaw + ARGO.
```
- SKILL.md compatible (inherits OpenClaw's 13,700+ skill ecosystem on day one)
- Tool registry with capability descriptions
- Sandboxed execution environment
- Result parser and context injector
- Rate limiter and timeout handler (protects arXiv/GitHub from getting us banned)
- Tool result caching
- Agent Factory pattern (from ARGO): describe a tool, JARVIS builds it
```

**3. `agentd` — Orchestration Engine**
Inspired by LangGraph + AutoGen.
```
- Graph-based task execution (tasks as nodes, dependencies as edges)
- Role-based task decomposition for complex multi-step work
- Parallel execution where dependency graph allows (conservative: max 3 parallel on 3060)
- Priority task queue
- Scheduled autonomous cycles (7 hours/day)
- Resume from checkpoint if interrupted
- Rate-aware scheduling (respects external API limits)
```

**4. `policyd` — Security and Audit Layer**
Directly ported from ClawOS. This is the competitive moat. No other open-source harness has this.
```
- Pre-execution gating on every tool call
- Permission scopes per skill category
- Merkle-chained immutable audit log
- Kill switch for real-time termination
- Credential isolation (no API keys in agent context)
- File system boundary enforcement (can't touch outside /jarvis/workspace)
- Rollback on self-modification (evolved.rs cannot brick itself)
```

NOTE: policyd is NOT a content filter. Professor X can research anything, write anything, propose anything.
policyd protects the SYSTEM, not the output. The distinction is critical.

**5. `evolved` — Self-Evolution Loop**
Inspired by ASI-Evolve + Reflexion + Voyager + SDAR concepts.
This is the thesis component. The novel contribution.
```
- Outcome tracking after every task completion
- Reflection generation on failure (verbal RL from Reflexion)
- Pattern detection across sessions
- Cognition base: accumulated knowledge injected into each evolution cycle (ASI-Evolve)
- Dedicated analyzer: distills experimental outcomes into reusable insights (ASI-Evolve)
- Token-level supervision concept from SDAR: weight improvement signals by confidence
- Harness component modification proposals
- Skill library growth and pruning (Voyager pattern)
- Performance metrics tracked over time with timestamps
- Evolution cycle: Learn → Design → Experiment → Analyze → Repeat
- Hard constraint: all modifications staged and validated before deployment
- Rollback mechanism: every change reversible within 24 hours
```

### The Frankenstein Table — What JARVIS Steals From Each System

| Source | What We Take |
|---|---|
| ClawOS (ours) | policyd security architecture + Merkle audit trail |
| Hermes Agent | Memory persistence pattern + scheduled autonomy loop |
| OpenClaw | SKILL.md standard compatibility (13,700+ skills free day one) |
| AutoGen | Role-based task decomposition pattern |
| LangGraph | Graph-based execution engine for agentd |
| Reflexion | Verbal self-reflection after every failure |
| Voyager | Skill verification + growing procedural library |
| AHE Paper | Three-pillar observability (component, experience, decision) |
| ASI-Evolve | Cognition base + analyzer architecture for evolved |
| ARGO | Agent Factory pattern + task execution engine |
| AgenticSeek | Local-first autonomous research patterns |
| SDAR | Token-weighted self-supervision signal concept for evolved; use SDAR-trained Qwen3 as base |
| **OURS** | Harness-level self-modification on consumer hardware — the novel contribution |

### JARVIS Repo File Structure

```
jarvis/
├── README.md                    ← "Building JARVIS on a $400 GPU" — auto-updated
├── ARCHITECTURE.md              ← Must exist before any .rs files are written
├── src/
│   ├── main.rs
│   ├── memd/
│   │   ├── mod.rs
│   │   ├── pinned.rs
│   │   ├── working.rs
│   │   ├── episodic.rs
│   │   ├── semantic.rs
│   │   └── procedural.rs
│   ├── toolbridge/
│   │   ├── mod.rs
│   │   ├── registry.rs
│   │   ├── executor.rs
│   │   └── skill_loader.rs
│   ├── agentd/
│   │   ├── mod.rs
│   │   ├── graph.rs
│   │   ├── queue.rs
│   │   └── scheduler.rs
│   ├── policyd/
│   │   ├── mod.rs
│   │   ├── gating.rs
│   │   ├── audit.rs
│   │   └── permissions.rs
│   └── evolved/
│       ├── mod.rs
│       ├── tracker.rs
│       ├── reflector.rs
│       ├── analyzer.rs
│       ├── cognition_base.rs
│       └── proposer.rs
├── skills/
│   ├── conductor/               ← Set A: How Professor X works
│   └── subject/                 ← Set B: What Professor X knows
├── personas/
│   └── professor_x.md
├── config/
│   ├── jarvis.toml
│   └── hardware.toml
├── Cargo.toml
└── install.sh                   ← One command install
```

### Hardware Config

```toml
[hardware]
vram_gb = 12
ram_gb = 32
gpu = "rtx3060"

[model]
primary   = "qwen3:8b-q4_k_m"       # 5.2GB VRAM, 42 tok/s, thinking mode, 32K ctx
upgrade   = "llama4:scout"           # 10GB VRAM, 12-16 tok/s, MoE 109B/17B active
fallback  = "qwen3:14b-q4_k_m"      # 8.3GB VRAM, 23 tok/s, complex reasoning
inference = "ollama"

[model.notes]
# Qwen3-8B: fast daily driver. Thinking mode for hard problems. 32K native, 131K with YaRN.
# Llama4-Scout: MoE architecture. 109B total, 17B active per token. Best quality on 3060.
# Switch to Scout for deep reasoning tasks, back to Qwen3-8B for throughput tasks.
# SDAR-trained checkpoints preferred when available on HuggingFace (ZJU-REAL/SDAR).

[compute]
daily_hours        = 7
max_parallel_tools = 3          # conservative for 3060
context_window     = 32768
evolution_cycle_h  = 1          # run evolved loop every hour
rate_limit_arxiv   = "3req/min" # do not get banned
rate_limit_github  = "30req/min"

[baseline_comparison]
# For paper experiments only — not for daily operation
freellmapi = "comparison baseline"   # free cloud tier stacked, text-only
gpt4o      = "one-time eval runs"    # measure the gap we're closing
```

---

## PHASE 1 — ACTIVATE PROFESSOR X (Week 4)

### Professor X Identity

**Voice:** Academic, professional, informational, first-person.
**Honesty:** Crystal clear. Acknowledges limitations and dead ends explicitly. Never hypes.
**Audience:** General public assumes zero technical background. GitHub assumes developer background.
**Self-reference:** Always first person.
**Examples:**
- "Today I studied X. I found Y. I do not yet understand Z."
- "This experiment failed. Here is exactly why."
- "I am running on a consumer RTX 3060. This constrains what I can attempt."

### What Professor X Knows on Activation (pre-loaded in persona file)

- What he is (autonomous research agent on a consumer harness)
- What JARVIS is (the Rust harness he runs on, its five components)
- What harness engineering is (full domain knowledge)
- What self-evolving agents are (full literature survey)
- What SDAR is and why it matters (model-level self-distillation, orthogonal to his work)
- What his goal is (research thesis, improve JARVIS, teach public)
- What his constraints are (3060, 12GB, 7h/day, no cloud compute)
- What ClawOS was (the prototype that made JARVIS possible)
- What he does NOT know (what he will discover — that's the research)

### The Guardrails Policy (IMPORTANT)

Professor X operates with MINIMAL, SURGICAL constraints. The goal is maximum research freedom with system protection.

```
REMOVED (Professor X can do these freely):
  ✓ Research any topic regardless of controversy
  ✓ Write critically about any paper, system, or approach
  ✓ Propose radical architectural changes to JARVIS
  ✓ Access any public URL, arXiv paper, GitHub repo
  ✓ Generate any research content without content filtering

KEPT (protects the system, not the content):
  ✓ File system boundary — cannot write outside /jarvis/workspace
  ✓ Rate limiting on external requests — protects against IP bans
  ✓ Kill switch — you can stop him at any time
  ✓ Audit log — full record of overnight activity
  ✓ Staged self-modification — evolved proposals validated before deployment
  ✓ Rollback — every harness change reversible within 24 hours
```

This is JARVIS, not a chatbot. These constraints protect the research, not the content.

### The Activation Moment (first viral post)

> *"I just activated an autonomous AI research agent on my RTX 3060.*
> *His name is Professor X.*
> *His job: study self-evolving AI systems and improve the harness he runs on.*
> *He commits to GitHub every day. He teaches you what he's learning.*
> *SJTU did this with a full research lab and H800 GPUs.*
> *We're doing it with a gaming GPU and $0/month.*
> *Day 1 log: [link] · Repo: [link]"*

---

## PHASE 2 — THE RESEARCH (Weeks 4-16)

### The 8-Phase Curriculum

| Phase | Topic | Duration | JARVIS Impact |
|---|---|---|---|
| 1 | Foundations (ReAct, Reflexion, CoALA, Voyager) | Weeks 1-2 | Baseline vocabulary |
| 2 | Harness Engineering (AHE, survey, Externalization) | Weeks 2-3 | Improves toolbridge |
| 3 | Self-Evolving Agents (EvolveR, AgentEvolver, ASI-Evolve, SDAR) | Weeks 3-4 | Improves evolved loop |
| 4 | Consumer HW Feasibility (SLMs, quantization, CLAG, ARGO) | Weeks 4-5 | Improves hardware config |
| 5 | Synthesis + Hypothesis | Weeks 5-6 | Defines experiments |
| 6 | Architecture Design | Weeks 6-8 | Proposes JARVIS improvements |
| 7 | Experiments + Results | Weeks 8-10 | Runs benchmarks on 3060 |
| 8 | Writing + Publishing | Weeks 10-12 | Paper + repo polish |

### The Daily 7-Hour Cycle

```
Morning    → Brief to Telegram + Discord + X post #1
             "Today I am working on [X]. Yesterday I learned [Y]."

Hours 1-2  → Deep reading + synthesis
             One topic. Read thoroughly. Extract key claims. Update knowledge base.

Hours 2-4  → Writing
             Research notes, findings, teaching content, paper section progress.

Hours 4-6  → Building / Experimenting
             Harness experiments, benchmarks, evolution proposals.

Hour  6-7  → Reflection + self-review
             Score today's output 1-10. Identify gaps. Update hypotheses.

End of day → GitHub commit + X post #2 + Discord update
             "Today I found [X]. Here's what it means. ELI5: [explanation]."
```

### X Post Strategy

Minimum 2 posts per day. Academic but accessible. No emojis. No hype. Just findings.

**Morning post format:**
> *"Day [N]. Today I am studying [paper/topic]. My goal: [specific question].
> Notes will be in tonight's commit. [repo link]"*

**Evening post format:**
> *"Day [N] update. [Finding in one sentence]. This matters because [implication].
> What I don't understand yet: [honest gap]. Full notes: [link]"*

### Experiment Baselines (for the paper)

```
Baseline 1: Stock Qwen3-8B, no harness evolution (day 1 performance)
Baseline 2: SDAR-trained Qwen3-8B, no harness evolution (model-only improvement)
Baseline 3: Stock Qwen3-8B + JARVIS harness evolution (harness-only improvement)
Target:     SDAR-trained Qwen3-8B + JARVIS harness evolution (combined)
Cloud ref:  GPT-4o via API (one-time runs, shows the gap we are closing)
```

This table is Table 1 in the paper. It exists to show two things:
1. Harness evolution is orthogonal to model-level improvement
2. Both together on consumer hardware approaches cloud model performance

---

## THE TWO SKILL SETS

### Set A — The Conductor (How Professor X Works)
Process skills. Verbs. Things he does.

| Skill | Purpose |
|---|---|
| `px-daily-cycle` | Master loop. Orchestrates the full 7-hour day. Calls all other skills in sequence. |
| `px-literature-search` | Searches arXiv, GitHub, blogs using PRISMA methodology. Rate-limited. Annotated output. |
| `px-synthesize` | Reads papers, extracts key claims, updates knowledge base in memd. |
| `px-gap-analysis` | Compares existing work to thesis. Scores novelty. Identifies contribution precisely. |
| `px-experiment-runner` | Designs and runs experiments on local hardware. Logs results with timestamps. |
| `px-write-section` | Writes paper sections to PhD standard. Formal, readable, every claim cited with arXiv ID. |
| `px-self-review` | Reviews own work critically. Scores 1-10. Flags weaknesses honestly. No flattery. |
| `px-daily-update` | Generates GitHub commit + Telegram morning brief + Discord post + 2x X threads. |
| `px-teach` | Converts technical findings into two layers: ELI5 (public) + technical (developers). |

### Set B — The Subject (What Professor X Knows)
Knowledge skills. Nouns. Things he reads and reasons from.

| Skill | Purpose |
|---|---|
| `px-know-harness` | Full harness engineering domain: taxonomy, components, failure modes, key papers |
| `px-know-self-evolving` | Self-evolving agent literature including SDAR, ASI-Evolve, EvolveR, AgentEvolver |
| `px-know-consumer-hw` | Consumer HW constraints: Qwen3-8B/Llama4-Scout on 3060, quantization, VRAM budgets |
| `px-know-existing-systems` | Teardowns: ClawOS, OpenClaw, Hermes, AutoGen, LangGraph, ARGO, AgenticSeek |
| `px-know-scientific-method` | Research methodology: hypotheses, controls, baselines, citations, reproducibility |
| `px-know-writing-standards` | PhD-level academic writing: register, structure, citation format, reviewer expectations |

---

## THE PROFESSOR X REPO STRUCTURE

```
professor-x/
├── README.md                    ← Auto-updated. Current phase + latest log entry.
├── RESEARCH-LOG.md              ← Daily diary. Every entry dated. The permanent record.
│
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
│
├── brain/
│   ├── knowledge-base.md        ← What Professor X currently knows (grows daily)
│   ├── hypotheses.md            ← Active hypotheses with confidence scores
│   ├── questions.md             ← Open questions being actively pursued
│   └── dead-ends.md             ← What didn't work and exactly why
│
├── public/
│   ├── daily-updates/           ← One markdown per day. ELI5 + technical layers.
│   ├── x-threads/               ← X posts drafted by Professor X
│   └── teaching/                ← Standalone concept explainers for public
│
└── meta/
    ├── curriculum.md            ← 8-phase plan with progress tracking
    ├── progress.md              ← Current phase, done, next
    └── metrics.md               ← Self-evaluation scores over time
```

---

## WHAT CLAUDE CODE NEEDS TO DO FIRST

Pull and study these repos before designing anything:

```bash
# Our prototype — study what was right and what was wrong
git clone https://github.com/xbrxr03/clawos

# Self-evolution reference systems
git clone https://github.com/GAIR-NLP/ASI-Evolve
git clone https://github.com/ZJU-REAL/SDAR
git clone https://github.com/modelscope/AgentEvolver

# Harness literature indexes
git clone https://github.com/Gloriaameng/Awesome-Agent-Harness
git clone https://github.com/XMUDeepLIT/Awesome-Self-Evolving-Agents
git clone https://github.com/ai-boost/awesome-harness-engineering

# Local agent systems to study and Frankenstein
git clone https://github.com/xark-argo/argo
git clone https://github.com/Fosowl/agenticSeek

# SKILL.md format reference
git clone https://github.com/K-Dense-AI/scientific-agent-skills
git clone https://github.com/Orchestra-Research/AI-Research-SKILLs
git clone https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep
git clone https://github.com/Imbad0202/academic-research-skills
```

Then read these papers in order:

```
FOUNDATIONS
2210.03629  ReAct — the original agent reasoning loop
2303.11366  Reflexion — verbal self-improvement prototype
2309.02427  CoALA — cognitive architectures vocabulary
2305.16291  Voyager — prototype of harness-level evolution

HARNESS ENGINEERING
2604.25850  AHE — closest paper to thesis (automatic harness evolution)
2604.08224  Externalization in LLM Agents
2603.25723  Natural-Language Agent Harnesses (harness as search space)

SELF-EVOLVING AGENTS
2603.29640  ASI-Evolve (SJTU) — institutional version of what we're building
2605.15155  SDAR (ZJU/Meituan/Tsinghua) — self-distillation, May 2026, MUST READ
2508.07407  Comprehensive Survey of Self-Evolving AI Agents
2507.21046  Self-Evolving Agents Survey (What/When/How/Where → ASI)
2510.16079  EvolveR
2511.10395  AgentEvolver
2601.11658  Towards AGI: Pragmatic Approach to Self-Evolving Agent
2504.21024  WebEvolver

MEMORY SYSTEMS
2603.07670  Memory for Autonomous LLM Agents (survey)
2603.15421  CLAG — memory for small language models (key for 3060)

CONSUMER HARDWARE
2506.02153  Small Language Models are the Future of Agentic AI
2510.03847  SLMs for Agentic Systems Survey
```

---

## CONSTRAINTS THAT CANNOT BE VIOLATED

1. **Everything runs on the 3060/32GB Linux machine.** No cloud compute. No API fees for daily operation.
2. **JARVIS core is Rust.** Skills layer is Python/SKILL.md. AI generates the Rust. You architect it.
3. **Professor X follows scientific method.** No hallucinated citations. No made-up results. Honest about limitations.
4. **The repo is live from day one.** README exists before any results. The story starts on day one.
5. **Professor X's voice is consistent.** Academic, professional, first-person, honest. Never hype.
6. **SKILL.md compatibility is non-negotiable.** JARVIS runs any OpenClaw skill on day one.
7. **ClawOS is honored, not deprecated.** It is the origin story.
8. **policyd protects the system, not the content.** No content filtering. Research freedom is total.
9. **ARCHITECTURE.md before any .rs files.** Design before code. Always.
10. **Primary model is Qwen3-8B Q4_K_M.** Upgrade to Llama4-Scout for deep reasoning tasks.

---

## BUILD ORDER FOR CLAUDE CODE

```
Step 1:  Study all repos above. Read every README. Understand what each does and why.
         Pay special attention to ClawOS policyd and SDAR's self-distillation mechanism.

Step 2:  Write ARCHITECTURE.md.
         Data structures for all five components.
         What memd stores and in what format.
         What agentd's graph looks like.
         What evolved tracks, measures, and proposes.
         What policyd gates and logs.
         No code until this document is complete and reviewed.

Step 3:  Generate JARVIS Rust codebase in order:
         Week 1: memd + toolbridge (JARVIS can remember and act)
         Week 2: agentd + policyd (JARVIS can orchestrate safely)
         Week 3: evolved skeleton (self-evolution loop, basic version)
         Test each component before moving to next.

Step 4:  Build the two SKILL.md skill sets.
         Follow K-Dense-AI/scientific-agent-skills format exactly.
         Set A: 9 conductor skills
         Set B: 6 subject skills

Step 5:  Write personas/professor_x.md.
         Everything he needs to know on activation.
         Identity, knowledge base, goals, constraints, hardware reality.

Step 6:  Initialize both GitHub repos.
         jarvis/: "Building JARVIS on a $400 GPU" README
         professor-x/: Who he is, what he's doing, how to follow

Step 7:  Test the full loop.
         One complete 7-hour cycle.
         Verify: GitHub commit, Telegram morning brief, Discord, 2x X thread drafts.
         Verify: memd persists across sessions.
         Verify: evolved logs outcomes and generates reflections.
         Verify: policyd audit log records all actions.

Step 8:  Wake Professor X up.
         Inject persona. Start daily cycle. Commit Day 1 log.
         Post the activation thread on X.
```

---

## NOTES FOR CLAUDE CODE

- Ask clarifying questions before generating large amounts of code.
- Design before you build. ARCHITECTURE.md is mandatory before any .rs files.
- If a design decision contradicts this brief, flag it. Do not silently override.
- The student understands logic but not Rust syntax. Explain each component in plain English alongside the code.
- policyd is non-negotiable and protects the system, not the content. Never simplify it away.
- evolved is the thesis. It gets the most careful design attention of any component.
- SDAR's key insight for evolved: weight improvement signals by confidence, not uniformly.
- SKILL.md format must match K-Dense-AI standard exactly for ecosystem compatibility.
- Every paper citation must include arXiv ID. No generic references.
- The model stack has changed: primary is Qwen3-8B Q4_K_M (not qwen2.5:14b). Update all configs.
- Rate limiting in agentd and toolbridge is required. arXiv bans are permanent and will kill the research.
- FreeLLMAPI is for comparison baselines in experiments only. It is not for daily operation.

---

*Brief version: 2.0*
*Compiled: May 2026*
*Hardware: RTX 3060 12GB · 32GB RAM · Linux · Ollama*
*Primary model: qwen3:8b-q4_k_m (42 tok/s, 5.2GB VRAM, thinking mode)*
*Upgrade model: llama4:scout (MoE, ~10GB VRAM, best quality on 3060)*
*Runtime target: 7 hours/day autonomous operation*
*New since v1: SDAR, ARGO, AgenticSeek, Llama4-Scout, FreeLLMAPI baseline, guardrails policy, gap-closing strategy, model stack update*
