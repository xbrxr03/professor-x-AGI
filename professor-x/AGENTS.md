# AGENTS.md — Claude × Codex coordination ledger

Two agents work this repo in PARALLEL (Claude = Professor-X dev; Codex). **Read this file before you
start. Check your box and append a log line when you finish a unit of work.** Phase 1 plan:
`.claude/plans/sparkling-sauteeing-marshmallow.md`; full map: `docs/PROJECT_ATLAS.md`.

## Rules
- **Disjoint file ownership — never edit a file another agent owns:**
  - **Claude:** `src/agentd/react.rs`, `src/agentd/fault_signature.rs` (new), trajectory-store index.
  - **Codex:** `scripts/benchmarks/repo_fix/failure_taxonomy.py` (new), `docs/research/*-failure-taxonomy.md`.
  - Shared/coordination files (`AGENTS.md`, `CODEX_TASK.md`): append-only; don't rewrite the other's lines.
- Each agent works in its **own git worktree/branch** off `prereboot-flywheel-prep`.
  - Codex: `git worktree add ../px-codex-measure -b codex/failure-taxonomy prereboot-flywheel-prep`.
- **Integrate Stream B (Codex, no code) before Stream A.** Stream C is SEQUENTIAL after A (shares react.rs).
- Discipline: full `cargo test --bins` green before committing any `src/` change; verify-the-ruler
  (honest before/after, no fabricated wins).

## Task checklist
### Stream A — behavior-keyed retrieval (Claude)
- [ ] A1 new `src/agentd/fault_signature.rs` — per-assert pass/fail bit-vector (port `sig_runner.py`)
- [ ] A2 index solved trajectories by failure-signature
- [ ] A3 wire behavioral retrieval into `retrieve_ice` (flag `PROFESSOR_X_BEHAVIOR_RETRIEVAL`, default OFF)
- [ ] A4 `cargo build --bins` + full `cargo test --bins` green
- [ ] A5 measure native repo-fix pass@1 (K-pass) on families: ON vs OFF vs text-retrieval; honest delta
### Stream B — failure taxonomy (Codex) — see CODEX_TASK.md
- [ ] B1 `failure_taxonomy.py` runs native bench on `qwen3:8b-q4_K_M` + `profx-distilled-clean` over hard set + families
- [ ] B2 bucket failures (duplicate_action / finish_rejected / edit-apply-error / wrong-edit-verified-fail / loop-forfeit / other)
- [ ] B3 write `docs/research/2026-06-21-failure-taxonomy.md` (per-model, per-task-set table)
### Stream C — apply-retry-with-feedback (Claude, AFTER A; ONLY if B shows edit-apply is a top bucket)
- [ ] C1 (conditional) one bounded retry feeding the editverify rejection reason back to the model

## Log (append-only; newest at bottom)
- [2026-06-21] (Claude) created AGENTS.md + CODEX_TASK.md on prereboot-flywheel-prep; starting Stream A (A1).
