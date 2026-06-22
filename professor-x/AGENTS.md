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

## PHASE 3 — distillation flywheel vs the wrong-edit ceiling (see docs/PLAN_PHASE3_2026-06-22.md)
Disjoint: Claude owns `src/` (Rust gate); Codex owns `distill/` (Python training). No shared files.
GPU: Codex owns it during collect+train; Claude's gate MEASUREMENT runs after — never concurrently.
### Stream D — Claude (Rust: TGC trust-gate + collection quality)
- [ ] D1 TGC gate: accept distilled iff held-out renamed-anchor pass@1 +≥MDE (K-pass); log Goodhart gap
- [ ] D2 NaN/stop-sanity guard rejects a bad gguf before gating (test)
- [ ] D3 collection captures teacher verified-correct EDIT trajectories (native format) for the frontier
### Stream E — Codex (Python: run the pipeline on the failure frontier) — see CODEX_TASK_P3.md
- [ ] E1 collect teacher qwen3:14b verified-correct trajectories on the wrong-edit frontier
- [ ] E2 QLoRA train (assistant-only mask + 2 epochs) -> clean GGUF (NaN-checked, stop-sane) -> serve profx-distilled-p3
- [ ] E3 hand profx-distilled-p3 to Claude's TGC gate (do not self-declare success)

## Log (append-only; newest at bottom)
- [2026-06-21] (Claude) created AGENTS.md + CODEX_TASK.md on prereboot-flywheel-prep; starting Stream A (A1).
- [2026-06-22] (Claude) PHASE 1 CONCLUDED (on branch claude/behavior-keyed-retrieval): Stream A built+measured (behavior retrieval validated as mechanism, pass@1 lift marginal/within-noise), Stream C found already-built. Bottleneck = edit-production CAPABILITY. Scoped PHASE 3 here (Stream D Claude / Stream E Codex) — distillation flywheel + TGC trust-gate. Codex: see CODEX_TASK_P3.md.
- [2026-06-22T05:46:00-04:00] (Codex) starting Stream E on branch `codex/p3-distill`; claimed GPU for qwen3:14b frontier collection + QLoRA train on the 35-task wrong-edit manifest.
- [2026-06-22T16:33:40-04:00] (Codex) paused Stream E on `codex/p3-distill` after prep + initial teacher collection: committed distill frontier/safety helpers, collected 7 verified trajectories so far (`hard_004`, `fam_csv_01`, `fam_csv_02`, `fam_money_03`, `fam_stack_02`, `fam_stack_04`, `fam_unit_04`), latest non-CSV frontier sweep (`repo-fix-202836-47664c90.json`) went 4/16 with `qwen3:14b-q4_K_M`; no active GPU jobs.
