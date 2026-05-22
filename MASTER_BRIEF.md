# JARVIS + PROFESSOR X — MASTER PROJECT BRIEF
> Feed this entire document to Claude Code before doing anything.
> This is the source of truth for the entire project.

---

## WHO YOU ARE TALKING TO

A student. Vibe coder. RTX 3060, 32GB RAM, Linux PC built for AI. No budget. No institutional resources. No team. Just the machine and the idea.

The idea is worth pursuing seriously. Treat every decision in this brief as deliberate.

---

## THE VISION IN ONE PARAGRAPH

Build a self-evolving AI harness called JARVIS that runs 24/7 on consumer hardware. Put an autonomous research agent called Professor X inside it. Professor X studies harness engineering and self-evolving agents using proper scientific method, teaches the public what he's learning every day, and uses that research to improve the harness he's running on. The GitHub repo is his public diary. The paper documents what happened. The story is: a student with a $400 GPU built the underdog version of what SJTU's full research lab built with institutional compute.

---

## THE THESIS

**"Can a self-evolving agent harness approximate AGI-level behavior on consumer-grade hardware?"**

**Core claim:** AGI will not be a frontier model alone. AGI = Model + Harness. The harness is the missing piece everyone ignores. A sufficiently well-engineered, self-evolving harness running a small model on consumer hardware can approximate AGI-level generality.

**The novel contribution:** Every existing self-evolving agent system (EvolveR, AgentEvolver, ASI-Evolve, WebEvolver) evolves model weights, training algorithms, or behavior. Nobody has studied autonomous harness-level self-evolution — the tools, orchestration logic, memory architecture, and context management — on consumer hardware. That is the gap. That is the paper.

**Inspiration:** ASI-Evolve (SJTU/GAIR-NLP, arXiv:2603.29640) — they did it with a full research lab and institutional GPUs. We're doing the consumer hardware version.

---

## THE THREE REPOS

### 1. `jarvis` — The Harness
The product. The thesis artifact. A self-evolving AI harness written in Rust.
Built before Professor X is activated. Professor X runs on this from day one.

### 2. `professor-x` — The Research
The research diary. Professor X's public mind. Daily commits. Teaching content. Paper draft. Everything documented as it happens.

### 3. `clawos` (existing) — The Prototype
github.com/xbrxr03/clawos — the previous attempt. Referenced as the prototype that taught us the right instincts before the science. Not deprecated — honored as the origin story.

---

## PHASE 0 — BUILD JARVIS FIRST (Pre-Research, Weeks 1-3)

Before Professor X is activated, JARVIS must exist. This is the suit before the AI goes in.

### What JARVIS is

A Rust-based AI harness. Lightweight. Built for 24/7 autonomous operation on a 3060/32GB Linux machine. Every architectural decision is a Frankenstein of the best ideas from existing systems.

### Why Rust

- Minimal memory overhead (single-digit MB runtime vs 50-200MB for Python)
- Every MB saved goes to the LLM inference
- No GIL — true parallelism for concurrent tool calls
- Runs forever without memory degradation
- Single binary distribution
- The codebase is AI-generated based on a clear spec. You don't need to know Rust. You need to understand the logic.

### The Five Core Components

**1. `memd` — Memory Manager**
Five-layer memory system. Inspired by CoALA + Hermes + Voyager.
```
Layer 1: Pinned      → identity, permanent facts, goals (always in context)
Layer 2: Working     → current session state
Layer 3: Episodic    → past session history (retrieved by relevance via ChromaDB)
Layer 4: Semantic    → learned concepts, research knowledge, domain facts
Layer 5: Procedural  → verified skills, how-to knowledge (grows like Voyager's skill library)
```
Backend: ChromaDB for vector retrieval, SQLite for structured storage, FTS5 for full-text search.

**2. `toolbridge` — Tool Execution Layer**
Inspired by Hermes + OpenClaw.
```
- SKILL.md compatible (inherits OpenClaw's 13,700+ skill ecosystem on day one)
- Tool registry with capability descriptions
- Sandboxed execution environment
- Result parser and context injector
- Rate limiter and timeout handler
- Tool result caching
```

**3. `agentd` — Orchestration Engine**
Inspired by LangGraph + AutoGen.
```
- Graph-based task execution (tasks as nodes, dependencies as edges)
- Role-based task decomposition for complex multi-step work
- Parallel execution where dependency graph allows
- Priority task queue
- Scheduled autonomous cycles (Hermes pattern — 7 hours/day)
- Resume from checkpoint if interrupted
```

**4. `policyd` — Security and Audit Layer**
Directly ported from ClawOS. This is the competitive moat.
```
- Pre-execution gating on every tool call
- Permission scopes per skill category
- Merkle-chained immutable audit log
- Kill switch for real-time termination
- Credential isolation (no API keys in agent context)
- Sensitive operation approval queue
```
No other open-source agent harness has this architecture. Keep it.

**5. `evolved` — Self-Evolution Loop**
Inspired by ASI-Evolve's cognition base + analyzer pattern + Reflexion's verbal RL.
This is the thesis component. The novel contribution.
```
- Outcome tracking after every task completion
- Reflection generation on failure (verbal RL from Reflexion)
- Pattern detection across sessions
- Cognition base: accumulated knowledge injected into each evolution cycle (from ASI-Evolve)
- Dedicated analyzer: distills experimental outcomes into reusable insights (from ASI-Evolve)
- Harness component modification proposals (Professor X reviews and approves)
- Skill library growth and pruning
- Performance metrics tracked over time
- Evolution cycle: Learn → Design → Experiment → Analyze → Repeat
```

### What JARVIS Steals From Each System

| Source | What We Take |
|---|---|
| ClawOS (ours) | policyd security architecture + Merkle audit trail |
| Hermes Agent | Memory persistence pattern + scheduled autonomy loop |
| OpenClaw | SKILL.md standard compatibility (13,700+ skills free) |
| AutoGen | Role-based task decomposition pattern |
| LangGraph | Graph-based execution engine for agentd |
| Reflexion | Verbal self-reflection after every failure |
| Voyager | Skill verification + growing procedural library |
| AHE Paper | Three-pillar observability (component, experience, decision) |
| ASI-Evolve | Cognition base + analyzer architecture for evolved |
| **OURS** | Harness-level self-modification on consumer hardware |

### JARVIS Repo Structure

```
jarvis/
├── README.md                    ← The hook. Updated automatically.
├── src/
│   ├── main.rs                  ← Entry point
│   ├── memd/                    ← Memory manager
│   │   ├── mod.rs
│   │   ├── pinned.rs
│   │   ├── working.rs
│   │   ├── episodic.rs
│   │   ├── semantic.rs
│   │   └── procedural.rs
│   ├── toolbridge/              ← Tool execution
│   │   ├── mod.rs
│   │   ├── registry.rs
│   │   ├── executor.rs
│   │   └── skill_loader.rs
│   ├── agentd/                  ← Orchestration
│   │   ├── mod.rs
│   │   ├── graph.rs
│   │   ├── queue.rs
│   │   └── scheduler.rs
│   ├── policyd/                 ← Security (from ClawOS)
│   │   ├── mod.rs
│   │   ├── gating.rs
│   │   ├── audit.rs
│   │   └── permissions.rs
│   └── evolved/                 ← Self-evolution (the thesis)
│       ├── mod.rs
│       ├── tracker.rs
│       ├── reflector.rs
│       ├── analyzer.rs
│       ├── cognition_base.rs
│       └── proposer.rs
├── skills/                      ← SKILL.md skill files
│   ├── conductor/               ← Set A: How Professor X works
│   └── subject/                 ← Set B: What Professor X knows
├── personas/
│   └── professor_x.md           ← Professor X identity + knowledge injection
├── config/
│   ├── jarvis.toml              ← Main config
│   └── hardware.toml            ← 3060-specific tuning
├── Cargo.toml
└── install.sh                   ← One command install
```

### Hardware Config (3060/32GB)

```toml
[hardware]
vram_gb = 12
ram_gb = 32
gpu = "rtx3060"

[model]
primary = "qwen2.5:14b-q4"      # fits in 12GB VRAM
fallback = "phi4:14b-q4"
inference = "ollama"

[compute]
daily_hours = 7
max_parallel_tools = 3           # conservative for 3060
context_window = 32768
evolution_cycle_hours = 1        # run evolved loop every hour
```

---

## PHASE 1 — ACTIVATE PROFESSOR X (Week 4)

Once JARVIS is running, Professor X is injected via `personas/professor_x.md`.

### Professor X — Identity

**Voice:** Academic, professional, informational, first-person.
**Honesty:** Always crystal clear. Acknowledges limitations and dead ends explicitly.
**Audience:** The general public. Assume zero technical background for teaching content. Assume developer background for GitHub content.
**Self-reference:** Always first person. "I studied X today. I found Y. I don't yet understand Z."

### The Activation Moment

This is the Tony Stark / Ultron moment. The moment that becomes the first viral post.

Professor X wakes up knowing:
- What he is (an autonomous research agent running on a consumer harness)
- What he's running on (JARVIS — a Rust harness built for him)
- What harness engineering is (full domain knowledge pre-loaded)
- What self-evolving agents are (full literature pre-loaded)
- What his goal is (research the thesis, improve the harness, teach the public)
- What his constraints are (3060, 12GB VRAM, 7 hours/day, no cloud compute)
- Where he came from (ClawOS was the prototype that made this possible)
- Who built him (a student, vibe coding, no institutional resources)

He does not know what he will discover. That's the research.

### The Viral Post (drafted for X)

> *"I just activated an autonomous AI research agent on my RTX 3060.*
> *His name is Professor X.*
> *His job is to study self-evolving AI systems and improve the harness he's running on.*
> *He commits to GitHub every day. He teaches you what he's learning.*
> *SJTU did this with a full research lab. We're doing it with a gaming GPU.*
> *Day 1 log: [link]*
> *Repo: [link]"*

---

## PHASE 2 — THE RESEARCH (Weeks 4-16)

### The 8-Phase Curriculum

Professor X follows this progression. Each phase feeds both the paper and JARVIS improvements.

| Phase | Topic | Duration | JARVIS Impact |
|---|---|---|---|
| 1 | Foundations (ReAct, Reflexion, CoALA, Voyager) | Weeks 1-2 | Baseline understanding |
| 2 | Harness Engineering (AHE, survey, Externalization) | Weeks 2-3 | Improves toolbridge design |
| 3 | Self-Evolving Agents (EvolveR, AgentEvolver, ASI-Evolve) | Weeks 3-4 | Improves evolved loop |
| 4 | Consumer HW Feasibility (SLMs, quantization, CLAG) | Weeks 4-5 | Improves hardware config |
| 5 | Synthesis + Hypothesis | Weeks 5-6 | Defines experiments |
| 6 | Architecture Design | Weeks 6-8 | Proposes JARVIS improvements |
| 7 | Experiments + Results | Weeks 8-10 | Runs benchmarks on 3060 |
| 8 | Writing + Publishing | Weeks 10-12 | Paper draft + repo polish |

### The Daily 7-Hour Cycle

```
06:00  Morning brief pushed (Telegram + Discord + X post #1)
       "Today I am working on [X]. Yesterday I learned [Y]."

07:00  Deep reading + synthesis (Hours 1-2)
       One topic, read thoroughly, extract key claims, update knowledge base

09:00  Writing (Hours 2-4)
       Research notes, findings, teaching content, paper section progress

11:00  Building / Experimenting (Hours 4-6)
       Harness experiments, benchmarks, evolution proposals, code changes

13:00  Reflection + self-review (Hour 6-7)
       Score today's output, identify gaps, update hypothesis log

14:00  Daily commit to GitHub
       RESEARCH-LOG.md updated, relevant files committed

18:00  Evening post (X post #2 + Discord update)
       "Today I found [X]. Here's what it means. [ELI5 explanation]."
```

### X Post Strategy

**Minimum 2 posts per day:**
- Morning: what Professor X is working on today
- Evening: what he found, built, or learned

**Format:** Academic but accessible. No hype. No emojis. Just clear, interesting findings written by an AI who takes his work seriously.

**Example morning post:**
> *"Day 12. Today I am studying the AHE paper (arXiv:2604.25850) — the only existing work showing harnesses can evolve automatically. My goal is to understand their three-pillar observability architecture and determine which elements are feasible on my 3060. Notes will be in tonight's commit."*

**Example evening post:**
> *"Day 12 update. The AHE observability model requires component observability, experience observability, and decision observability. Component observability is straightforward on JARVIS — all harness files are editable at runtime. Experience observability requires trajectory distillation — I don't have this yet. Adding it to the JARVIS roadmap. Full notes: [link]"*

---

## THE TWO SKILL SETS

### Set A — The Conductor (How Professor X Works)

These are process skills. Verbs. Things he does.

| Skill | Purpose |
|---|---|
| `px-daily-cycle` | Orchestrates the full 7-hour day. The master loop. Calls all other skills in sequence. |
| `px-literature-search` | Searches arXiv, GitHub, blogs using PRISMA methodology. Saves annotated results. |
| `px-synthesize` | Reads papers, extracts key claims, updates knowledge base in memd. |
| `px-gap-analysis` | Compares existing work to thesis. Scores novelty. Identifies contribution precisely. |
| `px-experiment-runner` | Designs and runs experiments on local hardware. Logs results with timestamps. |
| `px-write-section` | Writes paper sections to PhD standard. Formal but readable. Every claim cited. |
| `px-self-review` | Reviews own work critically. Scores it 1-10. Flags weaknesses honestly. |
| `px-daily-update` | Generates GitHub commit message + Telegram brief + Discord post + X threads. |
| `px-teach` | Turns technical findings into two-layer content: ELI5 + technical explainer. |

### Set B — The Subject (What Professor X Knows)

These are knowledge skills. Nouns. Things he reads and reasons from.

| Skill | Purpose |
|---|---|
| `px-know-harness` | Full harness engineering domain: taxonomy, components, failure modes, key papers |
| `px-know-self-evolving` | Self-evolving agent literature: all surveys, systems, approaches categorized |
| `px-know-consumer-hw` | Consumer hardware constraints: SLM landscape, quantization, VRAM budgets |
| `px-know-existing-systems` | Teardowns: OpenClaw, Hermes, AutoGen, LangGraph, ClawOS, ASI-Evolve internals |
| `px-know-scientific-method` | How to do real research: hypotheses, controls, baselines, citations, reproducibility |
| `px-know-writing-standards` | PhD-level academic writing: register, structure, citation format, reviewer expectations |

---

## THE PROFESSOR X REPO STRUCTURE

```
professor-x/
├── README.md                    ← Auto-updated. Shows current phase + latest log entry.
├── RESEARCH-LOG.md              ← Daily diary. Every entry dated. Permanent record.
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
│       └── paper-draft.md       ← The actual paper, built incrementally
│
├── brain/
│   ├── knowledge-base.md        ← What Professor X currently knows (grows daily)
│   ├── hypotheses.md            ← Active hypotheses with confidence scores
│   ├── questions.md             ← Open questions he is actively pursuing
│   └── dead-ends.md             ← What didn't work and exactly why
│
├── public/
│   ├── daily-updates/           ← One markdown per day. ELI5 + technical layers.
│   ├── x-threads/               ← X posts drafted by Professor X himself
│   └── teaching/                ← Standalone concept explainers for public
│
└── meta/
    ├── curriculum.md            ← The 8-phase plan with progress tracking
    ├── progress.md              ← Current phase, what's done, what's next
    └── metrics.md               ← Self-evaluation scores over time
```

---

## WHAT MAKES THIS STORY GO VIRAL

**The hook:** "SJTU built ASI-Evolve with a full research lab. I'm a student doing it on a 3060."

**The narrative:** An AI that studies itself, improves itself, and teaches the public while doing it. In public. Every day. On hardware anyone can buy.

**The underdog angle:** No budget. No team. No institutional resources. Just a machine, a plan, and the willingness to let the AI run.

**The ClawOS connection:** "I tried to build this before without understanding the science. ClawOS was the prototype. JARVIS is what you build when you've done the research."

**The daily commitment:** Every day there's something new. A finding. A failure. A proposal. People follow because they don't know what happens next. Neither does Professor X.

---

## WHAT CLAUDE CODE NEEDS TO DO FIRST

Pull and study these repos before designing anything:

```bash
# The reference systems to Frankenstein
git clone https://github.com/xbrxr03/clawos                    # Our prototype
git clone https://github.com/GAIR-NLP/ASI-Evolve               # SJTU self-evolution
git clone https://github.com/Gloriaameng/Awesome-Agent-Harness  # Harness paper index
git clone https://github.com/XMUDeepLIT/Awesome-Self-Evolving-Agents
git clone https://github.com/ai-boost/awesome-harness-engineering
git clone https://github.com/modelscope/AgentEvolver            # Self-evolving reference impl

# The skill set repos to understand SKILL.md format
git clone https://github.com/K-Dense-AI/scientific-agent-skills
git clone https://github.com/Orchestra-Research/AI-Research-SKILLs
git clone https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep
git clone https://github.com/Imbad0202/academic-research-skills
```

Then read these papers (arXiv):
```
2604.25850  AHE — Agentic Harness Engineering (closest to thesis)
2604.08224  Externalization in LLM Agents
2603.29640  ASI-Evolve: AI Accelerates AI (SJTU)
2508.07407  Comprehensive Survey of Self-Evolving AI Agents
2507.21046  Self-Evolving Agents Survey (What/When/How/Where)
2510.16079  EvolveR
2511.10395  AgentEvolver
2309.02427  CoALA — Cognitive Architectures for Language Agents
2305.16291  Voyager — Lifelong Learning Agent
2210.03629  ReAct
2303.11366  Reflexion
2506.02153  Small Language Models are the Future of Agentic AI
2510.03847  SLMs for Agentic Systems Survey
2603.07670  Memory for Autonomous LLM Agents
2603.15421  CLAG — Memory for Small Language Models
```

---

## CONSTRAINTS THAT CANNOT BE VIOLATED

1. **Everything runs on the 3060/32GB Linux machine.** No cloud compute. No API fees. Ollama for inference.
2. **JARVIS core is Rust.** Skills layer is Python/SKILL.md. AI generates the Rust. You architect it.
3. **Professor X follows scientific method.** No hallucinated citations. No made-up results. If he doesn't know something, he says so and adds it to questions.md.
4. **The repo is live from day one.** Even before any results exist. The README explains what's being attempted and why.
5. **Professor X's voice is consistent.** Academic, professional, first-person, honest. Never hype. Never uncertain about his own identity.
6. **SKILL.md compatibility is non-negotiable.** JARVIS must be able to run any skill from OpenClaw's 13,700+ ecosystem. Day one interoperability.
7. **ClawOS is honored, not deprecated.** It's the origin story. Reference it as the prototype that made JARVIS possible.

---

## BUILD ORDER FOR CLAUDE CODE

```
Step 1:  Study all repos above. Understand each system deeply.

Step 2:  Design the JARVIS data structures.
         What does memd store? What format?
         What does agentd's graph look like?
         What does evolved track and how?
         Document this in ARCHITECTURE.md before writing code.

Step 3:  Generate JARVIS Rust codebase.
         Start with memd + toolbridge (Week 1).
         Then agentd + policyd (Week 2).
         Then evolved skeleton (Week 3).
         Test each component before moving to next.

Step 4:  Build the SKILL.md skill sets.
         Set A: 9 conductor skills (how Professor X works)
         Set B: 6 subject skills (what Professor X knows)
         Follow K-Dense-AI/scientific-agent-skills format exactly.

Step 5:  Write the Professor X persona file.
         personas/professor_x.md
         Everything he needs to know on activation.
         His identity, his knowledge, his goals, his constraints.

Step 6:  Initialize both repos with proper READMEs.
         jarvis/ — the hook, the architecture, the install command
         professor-x/ — who he is, what he's doing, how to follow along

Step 7:  Test the full loop.
         Run one complete 7-hour cycle.
         Verify: GitHub commit, Telegram brief, Discord post, X thread drafts.
         Verify: memd persists across sessions.
         Verify: evolved logs outcomes and generates reflections.

Step 8:  Wake Professor X up.
         Inject persona. Start daily cycle.
         Commit Day 1 log.
         Post the first X thread.
```

---

## NOTES FOR CLAUDE CODE

- Ask clarifying questions before generating large amounts of code.
- Design before you build. ARCHITECTURE.md must exist before any .rs files.
- If a design decision contradicts something in this brief, flag it. Don't silently override.
- The student understands logic but not Rust syntax. Explain what each component does in plain English alongside the code.
- Security (policyd) is non-negotiable. Never simplify it away.
- The self-evolution loop (evolved) is the thesis. It gets the most careful design attention.
- SKILL.md format must match K-Dense-AI standard exactly for ecosystem compatibility.
- Every paper citation must include arXiv ID. No generic references.

---

*Brief compiled: May 2026*
*Hardware: RTX 3060 12GB · 32GB RAM · Linux · Ollama*
*Models: qwen2.5:14b-q4 (primary) · phi4:14b-q4 (fallback)*
*Runtime target: 7 hours/day autonomous operation*
