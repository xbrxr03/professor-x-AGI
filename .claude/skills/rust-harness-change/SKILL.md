---
name: rust-harness-change
description: "The safe workflow for changing Professor X's Rust harness code (react.rs, executor.rs, hiro.rs, main.rs, proposer/loop_runner). Use when editing the harness/agent loop, tools, evaluation, or evolution code; before committing a Rust change; or when a change touches behavior a test or benchmark covers. Enforces: build → FULL test suite → measure on the trustworthy benchmark → commit on harness-gaps → PR."
allowed-tools: Bash(*), Read, Edit, Write, Grep, Glob
---

# Changing the Rust harness safely

Professor X is one Rust binary (`professor-x/`). A change that compiles can still break a
pinned test or silently regress a benchmark. This is the discipline that keeps the tree honest.

## Step 0 — prerequisites (exit early if unmet)
```bash
cd /home/abrar/professor-x-AGI
git branch --show-current          # must be harness-gaps (NOT main — direct main push is blocked)
git status -s                      # know what's already dirty before you start
```
Build from the crate dir: `cd professor-x` (paths are cwd-relative).

## Workflow
1. **Locate, then edit.** Read the target before editing; match surrounding style. The agent
   loop is `agentd/react.rs` (`run_attempt`); tools `toolbridge/executor.rs`; eval/evolution
   `evolved/hiro.rs`, `evolved/loop_runner.rs`, `evolved/proposer.rs`; CLI/runners `main.rs`.
2. **Build:** `cargo build --bins 2>&1 | grep -E "^error|error\[" | head`. Fix all errors.
3. **FULL test suite — not a filtered subset:** `cargo test --bins 2>&1 | grep -E "test result"`.
   A `cargo test --bins <filter>` ran earlier this session and MISSED a failing executor test —
   always run the full suite before committing.
4. **If you changed behavior a test pins, update the test to the NEW intended contract** — do
   not just make it pass. Verify the new behavior is what you want (see `adversarial-self-review`).
5. **Measure** on the trustworthy benchmark if the change could affect capability:
   `PROFESSOR_X_DATA_DIR=$HOME/.professor-x ./target/debug/professor-x --repo-fix-bench --model qwen3:8b-q4_K_M`
   (see `professor-x-ops`; apply `verify-the-ruler` to the number; ~7 min, ±0.1 variance).
6. **Commit + push** on `harness-gaps`; open/append a PR to main. End commit messages with the
   Co-Authored-By line.

## Key rules
- Never push directly to `main` (policy-blocked); use a branch + PR.
- Don't pile speculative changes on an unvalidated one — measure first.
- Use absolute paths in Bash; a persisted `cd` has broken later commands here before.
- Behavior changes end in a measurement, recorded honestly in `docs/research/`.

## Output
A committed, pushed change with: full test suite green, the measured benchmark delta (with its
honest caveat), and a PR. If the change was neutral/within noise, say so — don't dress it as a win.
