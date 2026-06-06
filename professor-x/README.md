# Professor X

A self-evolving research agent that runs **entirely on your own machine** — one
Rust binary driving a local LLM (via Ollama), with persistent memory, a tool
system, security gating, a live dashboard, and a self-improvement loop. Built on
the thesis that the *harness* — not a frontier API — is the lever, and that it
should run on consumer hardware (developed on an RTX 3060, 12 GB).

> **Status: research prototype, usable for tinkering today.** It has interactive
> chat, a coding session, real tools, memory, and a live dashboard. It is **not**
> yet a frontier-quality daily driver — it runs a local 8B model, so it succeeds
> on simpler tasks reliably and harder ones intermittently. See
> [Daily-driver readiness](#daily-driver-readiness).

## Quickstart

### 1. Prerequisites
- **Rust** (stable): https://rustup.rs
- **Ollama**: https://ollama.com — then pull the model:
  ```bash
  ollama pull qwen3:8b-q4_K_M
  ```
  (Embeddings now run locally in-process via ONNX — no separate embed model needed.)
- A CUDA GPU helps but is not required; the 8B q4 model fits ~6–8 GB VRAM.

### 2. Build
```bash
cd professor-x
cargo build --release
```

### 3. Use it
```bash
# Interactive session (REPL): type a request, watch the agent work
./target/release/professor-x --chat

# One-shot task
./target/release/professor-x --task "read /etc/os-release and summarize it"

# Coding session
./target/release/professor-x --coding-session

# Live dashboard (run in a second terminal while it works)
./px-dashboard.py
```

State (memory, audit, vitals) lives in `~/.professor-x/` by default; override with
`PROFESSOR_X_DATA_DIR`.

## What it can do
- **Interactive REPL** (`--chat`) with slash commands: `/status`, `/cockpit`,
  `/brief`, `/run N`, free-text tasks dispatched to the live agent.
- **Tools**: file read/write/replace, sandboxed shell, web search/fetch, patch
  apply, `repo.map` (ranked codebase map), sub-agents (`agent.delegate`), a mirror
  critic (`agent.critic`), Tree-of-Thoughts search (`tot.search`).
- **MCP**: connect external Model Context Protocol servers via `.mcp.json` (see
  `.mcp.json.example`) — their tools register as `mcp.<server>.<tool>`.
- **Memory**: 5-layer (episodic, semantic, procedural, + research-grade
  consciousness modules) with local ONNX embeddings.
- **Security**: every tool call is policy-gated and written to a Merkle-chained
  audit log.
- **Self-evolution** (`--evolve-forever`): proposes harness changes, verifies them
  in a sandbox, keeps only those that beat the benchmark and preserve identity
  (ICS gate), rolls back the rest.
- **Live dashboard** (`px-dashboard.py`): real-time activity + consciousness vitals.

## What makes it unusual (the research layer)
Professor X carries instrumentation no other harness has: integrated information
(φ), differentiation (LZc), metacognitive sensitivity (meta-d′), identity
coherence (ICS), and a consciousness indicator-property audit. These are rigorous
*candidacy* measures — see `docs/research/`. Run `--consciousness-report` to see
them. (They measure correlates of consciousness; they do not claim subjective
experience — the hard problem is real.)

## Daily-driver readiness
| Aspect | State |
|---|---|
| Interactive UX | ✅ working REPL + coding session + live event stream |
| Tools / memory / security | ✅ solid |
| Onboarding | ✅ this README |
| **Capability** | ⚠️ local 8B (~0.2–0.3 pass@3 on our benchmark) — reliable on simple tasks, intermittent on hard ones |
| Speed | ⚠️ ~minutes/task locally |
| Diff-review-before-apply | ⚠️ planned (see `docs/backlog.md`) |

The path to a true daily driver is **capability** — either the self-distillation
flywheel (fine-tune the local model on its own verified trajectories,
`distill/README.md`) or pluggable stronger models. Tracked in `docs/backlog.md`.

## Docs
- `docs/backlog.md` — prioritized roadmap
- `docs/research/` — the consciousness measurement program, jcode gap analysis
- `distill/README.md` — the self-distillation flywheel
- `personas/professor_x.md` — the agent's identity

## License
See repository.
