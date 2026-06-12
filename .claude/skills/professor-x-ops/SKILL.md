---
name: professor-x-ops
description: "Operational interface for running and measuring Professor X: the benchmarks, the self-improvement loops, the model/data-dir conventions, and the git workflow. Use when running --repo-fix-bench / --hiro / --evolve-*, interpreting their output, or committing work on this project."
---

# Professor X — operational cheatsheet

Single Rust binary at `professor-x/`. Build: `cargo build --bins`. Run from `professor-x/`
(paths like `scripts/benchmarks/repo_fix/tasks.json` and `config/hardware.toml` are cwd-relative).

## Model & data dir
- Primary model is **`qwen3:8b-q4_K_M`** (local, Ollama). Pass `--model qwen3:8b-q4_K_M` to use the
  exact installed tag. llama4:scout is deprecated. Thesis: small model + great harness.
- Prefix runs with `PROFESSOR_X_DATA_DIR=$HOME/.professor-x` (state.db, artifacts).
- Ollama must be up (`curl -s localhost:11434/api/tags`). It occasionally hiccups mid-run.

## Benchmarks (the trustworthy scoreboard is deterministic)
- **`--repo-fix-bench`** — the TRUSTWORTHY metric: copy fixture → confirm test red → agent edits in
  a /tmp workdir → re-run test → pass iff green. Ungameable. Fixtures: `scripts/benchmarks/repo_fix/`
  (stdlib `check.py`, NOT pytest). Baseline ~0.85 pass@1 on qwen3:8b. Each run ~7 min (±0.1 variance).
- **`--hiro-null N --hiro-limit M`** — HIRO benchmark; uses a stratified sample + a hybrid judge
  (deterministic `expected` + LLM-judge). The LLM-judge is UNRELIABLE — prefer repo-fix for headline numbers.
- **`--hiro-smoke`** — validate the 60-task HIRO file deserializes.

## Self-improvement (M4 — empirical fitness gate)
- **`--evolve-on-repofix N`** — evolve the system prompt, gated on measured repo-fix pass@1
  (K=2 reps, MDE=0.10). Accepts ONLY a candidate that beats baseline beyond noise. A failure-aware
  proposer is shown the actual failures and targets them.
- **`--evolve-skill-on-repofix N`** — same gate, but evolves the `skills/conductor/px-fix-bug.md`
  skill and PERSISTS it only if it measurably helps.
- A gate that accepts nothing when nothing beats noise is CORRECT (unlike the legacy loop /
  ARIS meta-optimize, which never measure).

## Conventions
- Long runs: launch in the background, capture to `/tmp/*.log`, grep for `pass@1` / `CURVE`.
- Branch `harness-gaps`; commit + push freely; PRs to main (direct main push is policy-blocked).
- Record honest results in `docs/research/eval-trust.md`. Apply `verify-the-ruler` and
  `adversarial-self-review` before recording or committing.
