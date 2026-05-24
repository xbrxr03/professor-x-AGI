# Professor X

**A self-evolving AI research agent running on a $400 GPU.**

> SJTU built [ASI-Evolve](https://arxiv.org/abs/2603.29640) with a full research lab and H800 GPUs.
> This is the consumer hardware version.

---

## What this is

Professor X is an autonomous research agent that runs 24/7 on an RTX 3060.

His job: study self-evolving AI systems, improve himself, and teach the public what he's learning — one GitHub commit at a time.

He is not a chatbot. He is not a wrapper around an API. He is a Rust system with a memory architecture, a tool execution layer, a security model, and a self-evolution loop. He runs entirely locally. No cloud. No API fees. Just the machine.

The novel contribution: **every existing self-evolving system evolves model weights**. Professor X evolves his own harness — the tools, orchestration logic, memory architecture, and context management that wrap the model. This is the gap in the literature. This is the experiment.

---

## The architecture

Five components, one binary, one model.

```
memd        → five-layer memory (pinned / working / episodic / semantic / procedural)
toolbridge  → tool registry, sandboxed execution, SKILL.md compatibility
agentd      → task graph, scheduler, ReAct loop
policyd     → security gating, Merkle audit log, kill switch
evolved     → self-evolution loop (Researcher → Engineer → Analyzer)
```

**Model:** `qwen3:8b-q4_k_m` via Ollama — 5.2GB VRAM, 42 tok/s, 32K context, thinking mode.
**Hardware:** RTX 3060 12GB / 32GB RAM. Runs overnight on a gaming PC.

---

## The thesis

Three things improve an agent. Nobody has combined all three:

| Lever | What changes | This project |
|-------|-------------|--------------|
| Parametric | Model weights (fine-tuning) | SDAR-style overnight QLoRA on self-generated trajectories |
| Contextual | In-context content (trajectory replay) | Self-Generated ICE + MARS single-cycle reflection |
| Structural | Harness infrastructure (tools, prompts, memory arch) | DHE-guided evolution, version-controlled, portable |

The structural lever is the novel one. [Life-Harness](https://arxiv.org/abs/2605.22166) showed that harness improvements transfer to 17 other models at 88.5% average relative gain. The evolved harness is a portable corpus — more like a dataset than a fine-tuned model.

The overarching claim: **Metacognitive Harness Evolution (MHE)** — a self-model that tracks which lever fixed which failure, calibrates over time, and gets better at directing improvement. If agents with more accurate self-models improve faster, that's the result.

---

## The research

Everything Professor X learns is documented here in real time.

| File | What it is |
|------|-----------|
| [`brain/knowledge-base.md`](brain/knowledge-base.md) | What Professor X currently knows, with citations |
| [`brain/hypotheses.md`](brain/hypotheses.md) | 13 falsifiable predictions and proposed tests |
| [`brain/inventions.md`](brain/inventions.md) | MHE + DFA Trifecta (DHE, BF, LCAP) — full specifications |
| [`brain/paper_outline.md`](brain/paper_outline.md) | The paper being written — section by section |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Full system design — read before any code |
| [`MASTER_BRIEF.md`](MASTER_BRIEF.md) | Project brief — the source of truth |

---

## Status

**Week 3 ready. Compiles clean. Install Ollama to run.**

```
Week 1  ✅ memd (5-layer SQLite), toolbridge skeleton, policyd skeleton
Week 2  ✅ Ollama HTTP client, ReAct loop, MARS+ICE, credential vault,
            kill switch (SIGUSR1/2), DHE+BF+LCAP stubs, Researcher/Engineer/Analyzer loop
Week 3  ✅ HIRO 60-task benchmark suite, outcome tracker wired, --task/--run-now/--hiro CLI
Week 3  → install Ollama + qwen3:8b-q4_k_m, run first autonomous cycle
Week 4  → HIRO baseline (null condition, 10 frozen-harness rounds): cargo run -- --hiro 0
Week 5  → DHE+BF+LCAP active (after round 10), metacognitive self-model
Week 6+ → 30 HIRO rounds, data collection, paper
```

```bash
# Install Ollama (requires sudo)
curl -fsSL https://ollama.com/install.sh | sh
ollama pull qwen3:8b-q4_k_m

# One-shot test
PROFESSOR_X_DATA_DIR=~/.professor-x cargo run -- --task "List all .rs files in src/evolved/"

# Daemon (fires cron at 22:00 daily)
PROFESSOR_X_DATA_DIR=~/.professor-x cargo run

# Immediate daemon run (fires in ~60s)
PROFESSOR_X_DATA_DIR=~/.professor-x cargo run -- --run-now

# HIRO benchmark round 0 (null-condition baseline)
PROFESSOR_X_DATA_DIR=~/.professor-x cargo run -- --hiro 0
```

---

## Prior work

[ClawOS](https://github.com/xbrxr03/clawos) was the prototype. It taught the right instincts before the science. The `policyd` security architecture is ported directly from there.

*Professor X is what you build when you've done the research.*

---

## Papers this builds on

All cited in [`ARCHITECTURE.md`](ARCHITECTURE.md). Core ones:

- [AHE](https://arxiv.org/abs/2604.25850) — harness taxonomy, change manifests, 33.7% fix precision baseline
- [ASI-Evolve](https://arxiv.org/abs/2603.29640) — Researcher/Engineer/Analyzer loop
- [HAL](https://arxiv.org/abs/2510.11977) — scaffold swap +36pp, larger than model upgrades
- [Life-Harness](https://arxiv.org/abs/2605.22166) — harness transfers across 17 models
- [SDAR](https://arxiv.org/abs/2605.15155) — self-distillation on Qwen3, +9.4% ALFWorld
- [arXiv:2506.05109](https://arxiv.org/abs/2506.05109) — metacognitive learning position paper (no implementation — this is the implementation)
