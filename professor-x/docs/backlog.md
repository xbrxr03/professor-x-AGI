# Professor X — Backlog

Prioritized engineering backlog. Sources noted where an item came from a specific
analysis. Status: ☐ open · ◐ in progress · ✓ done.

## From jcode gap analysis (2026-06-06-jcode-vs-professor-x-gap-analysis.md)
- ✓ **Local ONNX embeddings** (DONE, commit 9496ac0). Replace the Ollama
  `nomic-embed-text` dependency with in-process ONNX/`fastembed` vector inference.
  Removes a network/process dependency and speeds every embed (retrieve_ice,
  binding, cognition, case-based confidence). jcode runs vector inference locally
  with no external service.
- ◐ **Persistent server + hot-reload** (HIGH, larger build). Mirror jcode's
  SelfDev/hot_exec: a persistent server so the evolution loop can apply a verified
  change *live* instead of requiring an operator restart. Closes the
  evolve→apply→measure loop without manual intervention.
  **Landed (primitive):** `src/evolved/hot_reload.rs` + `--self-rebuild-reexec`
  (alias `--hot-reload`). After a verified self-change is committed, it rebuilds
  `cargo build --release` and `exec`s into the new binary (`--self-reload-probe`
  confirms the relaunch). Structural safety: re-exec ONLY on a clean build (a broken
  self-edit can't replace a working binary), a generation cap (`PROFESSOR_X_RELOAD_GENERATION`,
  default 8) bounds reload storms, and the running binary is moved aside before relink to
  avoid `ETXTBSY`. 4 unit tests on the decision policy; full suite 337 green.
  **Remaining:** wire it as the operator loop's final cycle after `operator_commit` so the
  live loop self-applies; the live re-exec itself is a self-modifying action gated behind
  explicit operator authorization.
- ☐ **Swarm file-conflict handling** (MEDIUM). jcode's swarm-core gives agents
  shared-repo access with conflict avoidance. Prof X's sub-agents (`agent.delegate`)
  have no scope arbitration — add scope-locks so parallel sub-agents can't clobber.
- ☐ **Browser automation tool** (LOW). jcode has it; Prof X does not. Only if a
  use case demands it.

## Distillation flywheel (the untested headline thesis)
- ◐ **Fill the corpus** — BLOCKED by capability: self-authored curriculum tasks
  mostly FAIL the judge (success ~0.2), so judge-gated collection barely grows
  (stuck ~35 unique). Options: easier/graded curriculum, more volume, or accept a
  smaller corpus. The real ceiling is the agent's task success rate.
- ☐ **QLoRA fine-tune** — BLOCKED on (1) GPU driver mismatch (needs a reboot;
  Ollama tolerates it, PyTorch/CUDA won't) and (2) deps install
  (unsloth/peft/bitsandbytes/trl). After both: run `distill/train_qlora.py`,
  serve, ICS-gate (accept only if pass@3 beats baseline by >MDE AND ICS ≥ 0.70).

## Consciousness measurement program (2026-06-05/06 docs)
- ☐ **meta-d′ resolution** — the one MEASURED deficit (AUROC ~0.48). Calibration
  fixed overconfidence not resolution; a case-based-dominant tweak BACKFIRED
  (reverted). Real fix likely needs per-trial uncertainty from token logprobs
  (does Ollama expose them?), not retrieval-based signals.
- ☐ **Attention schema (AST-1)** — the clear MISSING consciousness indicator from
  the Butlin audit. Build a model of the agent's own attention/context-selection
  it can query and control.
- ☐ **Full per-step perturbational PCI** — today's was a task-level coupling
  on/off contrast (passed, n=36). The gold-standard version needs per-step module
  sampling + a direct perturbation pulse.
- ☐ **φ-rises-as-it-runs** — currently stable (homeostatic fix), not rising. May
  need the models to sharpen over a long evolution run, or a better integration
  measure than total correlation (which saturates).

## MISSION: the best harness for AGENTIC work with LOCAL models
The genre everyone underserves — Claude Code/openclaw etc. assume frontier models
and fall apart on local ones. Professor X's harness is tuned for small-model
failure modes. Make it a daily sensei/assistant for every local-AI user, scaling
from 8B (laptop) to 70B (workstation) on the SAME harness, with best-in-class UX.
- ✓ Model flexibility — `--model` / auto-pick biggest installed (11073c7)
- ✓ Local ONNX embeddings (9496ac0) · ✓ README (6805b2c) · ✓ dashboard (b9090dc)

### UX/UI roadmap — "implement every notable feature of every harness"
Assistant-grade interactive experience (the REPL today is operator-grade):
- ◐ **In-session `/model`** — show current + installed, switch live (leverage the
  new model flexibility) [building now]
- ◐ **`/tools`** — list available tools (built-in + MCP + skills) [building now]
- ◐ **Change visibility** — fs.write/fs.replace now report `created X` / `edited X —
  Δ +N -M lines` with a changed-line preview, so every edit is visible in the feed.
  (Interactive confirm-BEFORE-apply still TODO — needs an agent↔UI approval channel.)
- ☐ **Streaming + rich rendering** — markdown, syntax highlight, spinner/progress
  while the agent works (jcode-grade feel). HIGH.
- ✓ **`@file` references** — type @path to inline a file into the agent's context
  (chat / --task / TUI / web). util::expand_file_refs, 3 tests.
- ☐ **`/memory`** — view/edit what the agent remembers (CLAUDE.md-style).
- ☐ **Plan mode + todo display** (Claude Code) — show the agent's plan/checklist live.
- ☐ **Session resume / history** — `/resume`, up-arrow command history.
- ✓ **`/undo`** — revert the last applied change using git-backed path checkpoints.
- ✓ **Shell OS sandbox posture** — `shell.restricted` now prefers Bubblewrap isolation
  when the host permits user namespaces and records `sandbox=bubblewrap` or
  `sandbox=fallback-policy-only` in command observations/artifacts.
- ☐ **`/add` / `/drop` files** (Aider) — manage the working set.
- ☐ **Slash-command autocomplete + a clean banner/help split** (assistant vs operator).
- ✓ **Full ratatui TUI** (`profx --tui`) — interactive: type tasks, watch the agent
  work live, consciousness-vitals pane. (diff pane still TODO)

### Web UI
- ✓ **`profx --serve`** — local web UI (axum) at http://127.0.0.1:8787: chat,
  live activity, consciousness vitals. API (`/api/events`, `/api/vitals`,
  `/api/task`) is the contract for an OpenUI-generated frontend.

## Consolidation
- ✓ **PR #12** (`harness-gaps` → `main`) — merged into `main`; added
  `NEXT_STEPS.md`, the 2026-06-08 trajectory corpus, and the Frankenstein harness
  master plan.
