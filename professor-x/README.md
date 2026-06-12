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

## What's proven (and how it's measured)

The thesis — *"the harness, not the model, is the lever"* — is demonstrated on an
**ungameable** benchmark: `repo-fix`, where a planted bug must go **red → green** by the
agent's edit (judged by the repo's own test exit code, which no lenient LLM-judge can inflate).

> On `repo-fix`, the **same** `qwen3:8b` went **pass@1 0.50 → ~0.85** — *purely from harness
> improvements; the model never changed.* The fixes weren't guesses: each came from reading a
> real failure trajectory (a greedy decode-loop; a tool rejecting correct edits). That is the
> whole thesis, on a number that can't be faked.

Reproduce it yourself (≈7 min, on a local 8B):
```bash
cd professor-x && cargo build --release
PROFESSOR_X_DATA_DIR=$HOME/.professor-x ./target/release/professor-x \
    --repo-fix-bench --model qwen3:8b-q4_K_M
```

What you'll see (actual run, qwen3:8b) — each task starts **red** (`pre=1`) and the agent must
make it **green** (`post=0`):
```text
repo-fix fix_001  pre=1 post=0 -> PASS     # returned a-b instead of a+b
repo-fix fix_002  pre=1 post=0 -> PASS     # off-by-one xs[len(xs)]
repo-fix fix_003  pre=1 post=0 -> PASS     # missing return
repo-fix fix_006  pre=1 post=0 -> PASS     # filtered odds instead of evens
repo-fix fix_007  pre=1 post=0 -> PASS     # wrong recursion base case
repo-fix fix_008  pre=1 post=0 -> PASS     # and/or boolean bug
repo-fix fix_010  pre=1 post=0 -> PASS     # accumulator overwrote instead of +=
...
pass@1 = 0.714  (10/14 tasks)              # 14 tasks incl. harder multi-file/edge-case/find-the-bug
```
On the easier 10-task subset the agent runs ~0.7–0.9 (≈0.85 mean); on the full 14-task set
(with the harder fixtures) ~0.71. The range holds as the benchmark gets harder — it's not
trivial-task overfitting.

**Self-improvement with an empirical gate.** Professor X can try to improve its own harness and
keep a change *only if it measurably beats baseline beyond noise* — unlike tools that accept
changes on an LLM's say-so and drift:
```bash
./target/release/professor-x --evolve-on-repofix 2 --model qwen3:8b-q4_K_M
```

**Integrity first.** Every headline number is mechanism-checked before it's believed — this
repo has caught and discarded two "mirages" (an inflated LLM-judge score; a benchmark that
scored 0 only because a test runner was missing). See
[`docs/research/eval-trust.md`](docs/research/eval-trust.md).

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
| **Capability** | ⚠️ local 8B — **~0.85 pass@1 on the deterministic `repo-fix` bench** (trivial single-file bug-fixes); reliable on simple coding tasks, intermittent on harder/multi-file ones. (The old `~0.2 pass@3` HIRO figure used an untrustworthy LLM-judge — see `docs/research/eval-trust.md`.) |
| Speed | ⚠️ ~minutes/task locally |
| Diff-review-before-apply | ⚠️ planned (see `docs/backlog.md`) |

The path to a true daily driver is **capability**, and this repo shows the primary lever is the
**harness** — trajectory-diagnosed harness fixes lifted `repo-fix` 0.50 → ~0.85 on a fixed 8B.
Next: harder/multi-file fixtures, then either the self-distillation flywheel (fine-tune the local
model on its own verified trajectories, `distill/README.md`) or pluggable stronger models. Tracked
in `docs/backlog.md`.

## Docs
- `docs/backlog.md` — prioritized roadmap
- `docs/research/` — the consciousness measurement program, jcode gap analysis
- `distill/README.md` — the self-distillation flywheel
- `personas/professor_x.md` — the agent's identity

## License
See repository.
