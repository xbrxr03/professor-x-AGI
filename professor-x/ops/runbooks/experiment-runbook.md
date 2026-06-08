# Professor X — Experiment Runbook

How to run the MHE (Metacognitive Harness Evolution) experiment end to end, and
how to read the result. Written 2026-06-04 after the first real evolution run.

## 0. Prerequisites

```bash
# Models (one-time). qwen3:8b is the primary — it fits the 3060's 12GB.
# Do NOT use llama4:scout (109B MoE, ~55GB — will not fit).
ollama pull qwen3:8b-q4_K_M
ollama pull nomic-embed-text        # 768-dim embeddings, CPU-only

cd ~/professor-x-AGI/professor-x
cargo build --release               # one build; all commands use the binary
export PXD=~/.professor-x           # data dir (state.db, embeddings)
```

The binary uses `DEFAULT_MODEL` in `src/ollama.rs`; `config/hardware.toml` is
documentation, not read by the code.

## 1. Watch it work (any time, in a second terminal)

```bash
cd ~/professor-x-AGI/professor-x && ./prof-x-stream.py     # colored live transcript
# raw fallback:
tail -f artifacts/events/$(date +%F).jsonl
```

## 2. The experiment, in order

### 2a. Null baseline → σ (the measurement floor)

A single round cannot establish an evolution effect — run-to-run noise is large
(observed: 0.483 vs 0.417 on the same harness). Run several FROZEN rounds first:

```bash
PROFESSOR_X_DATA_DIR=$PXD ./target/release/professor-x --hiro-null 5
```

~80 min/round. When done:

```bash
PROFESSOR_X_DATA_DIR=$PXD ./target/release/professor-x --consciousness-report
```

Read the top section: mean pass@3, **σ**, and the **minimum detectable effect
(≈1.96σ)** — the threshold an evolution gain must clear to be real. σ is computed
only over rounds sharing one harness commit (a genuinely frozen harness).

### 2b. One evolution cycle (see the loop turn)

```bash
PROFESSOR_X_DATA_DIR=$PXD ./target/release/professor-x --evolve
```

Learns from the latest round's real pass/fail (`hiro_attempts`), generates 3
proposals steered to distinct applyable components (SystemPrompt / SkillDefinition
/ HarnessConfig), Elo-judges them, sandbox-verifies the winner (cargo check +
reward-hacking scan + **identity-preservation gate**), and commits if it passes.
SystemPrompt changes are ADDITIVE (appended, never overwrite the persona).

A committed change shows as a `evolved: …` commit in `git log`.

### 2c. Continuous mining (overnight self-improvement)

```bash
PROFESSOR_X_DATA_DIR=$PXD ./target/release/professor-x --mine        # unbounded
PROFESSOR_X_DATA_DIR=$PXD ./target/release/professor-x --mine 20     # 20 blocks
```

Each block: evolve → measure on a fixed 15-task subset → KEEP if pass@3 beats
best, else `git reset` rollback. Only demonstrably-helpful changes survive.
Blocks logged as `evolve.forever.block` events. Ctrl-C to stop.

> Note: `--mine` uses `git reset --hard` to roll back rejected changes, so the
> local branch diverges from origin during mining. Decide what to push after.

### 2d. The agent-authored benchmark

```bash
PROFESSOR_X_DATA_DIR=$PXD ./target/release/professor-x --run-self-tests 10
```

Runs the tests Professor X wrote for his own diagnosed failure classes, judged
against his own pass criteria. The thesis question: does this self-authored
pass-rate track HIRO pass@3 over rounds?

## 3. Reading the result

`--consciousness-report` answers five questions across rounds:

| Q | Metric | Supported when |
|---|--------|----------------|
| Q1 | phi (total correlation of module activations) | slope > 0 |
| Q2 | interoceptive prediction error | falling |
| Q3 | self-prediction error | falling (watch the blind-spot dimension) |
| Q4 | DMN insights feeding evolution | accumulating + proposals improve |
| Q5 | ICS (identity vs round-0) | ≥ 0.70 |

Reading: **2+ of Q1–Q5 supported across ~30 rounds → the thesis holds.**

## 4. Integrity rules (non-negotiable)

- Never write a "confirmed" status without an artifact + round in `state.db`.
  (The 2026-05-24 incident flipped H1/H3 to "confirmed" with zero rounds run.)
- σ before deltas: a pass@3 change under the MDE is noise, not a result.
- Report confounds. Round 0→1 here ran on slightly different binaries; the clean
  baseline must be N rounds on ONE frozen commit.

## 5. Safety properties (verified, not assumed)

- **Identity-preservation gate** (`apply_node_change_at` → `preservation_guard`):
  a persona/config/skill overwrite that guts the file or drops the "Professor X"
  anchor is refused at mutation time. Verified firing in production (rolled back
  a 719-char persona "replacement", 28% < 60% floor).
- **Additive persona evolution**: identity destruction is structurally
  impossible; the original persona is always retained.
- **Sandbox verify-then-commit**: every change applied in an ephemeral worktree,
  cargo-checked, reward-hacking-scanned before touching main.
- **Core modules** (policyd gate, memd internals) are never autonomously
  mutable — only SystemPrompt / SkillDefinition / HarnessConfig.

## Command reference

| Command | What |
|---------|------|
| `--hiro N` | one HIRO round, recorded as round N |
| `--hiro-null N` | N frozen rounds (baseline / σ) |
| `--evolve` | one evolution cycle from real outcomes |
| `--mine [n]` | continuous evolve→measure→keep/rollback |
| `--run-self-tests [n]` | run the agent-authored benchmark |
| `--consciousness-report` | trajectory, σ, MDE, five questions |
| `./prof-x-stream.py` | live colored transcript |
