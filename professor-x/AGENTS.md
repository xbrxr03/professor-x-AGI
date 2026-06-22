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
- [x] A1 new `src/agentd/fault_signature.rs` — per-assert pass/fail bit-vector (port `sig_runner.py`)
- [x] A2 index solved trajectories by failure-signature (`build_signature_index.py` -> `signature_index.json`; Rust `SignatureIndex`)
- [x] A3 wire behavioral retrieval into the loop (flag `PROFESSOR_X_BEHAVIOR_RETRIEVAL`, default OFF; appended post-binding)
- [x] A4 `cargo build --bins` + full `cargo test --bins` green (370/370)
- [~] A5 measuring: first run was a BROKEN RULER (emit_event writes to DB not stdout -> hits=0 was a
      measurement artifact; the 0.357->0.429 was within noise + unverifiable). Fixed: stderr HIT line +
      counter. CONFIRMED firing (each anchor matches its origin at sim 1.00, hint injected). Trustworthy
      ON-vs-OFF re-run in progress (qwen3:8b, 14 held-out anchors).
### Stream B — failure taxonomy (Codex) — see CODEX_TASK.md  [COMPLETE 2026-06-22]
- [x] B1 `failure_taxonomy.py` ran native bench on both models over hard set + families
- [x] B2 bucketed failures
- [x] B3 wrote `docs/research/2026-06-21-failure-taxonomy.md` (in codex worktree; uncommitted — Codex to commit)
- FINDING: dominant failure = **wrong-edit-verified-fail** (distilled 61.5%, qwen3:8b 81.4%); edit-apply
  small (~9-15%); loop/forfeit = 0. Bottleneck = wrong FIX CHOICE, not edit mechanics or thrash.
### Stream C — verifier-feedback retry — ALREADY BUILT → SKIP (do not rebuild)
- [x] C-investigated: `with_verifier` + finish-handling (react.rs ~1545-1623) ALREADY runs the verifier
      on finish, REJECTS it on failure, feeds back the FULL check.py output via
      `verifier_failed_observation` ("make one targeted edit, then finish only after the verifier
      passes"), and attempts `try_python_verifier_repair`. RLEF loop exists.
- CONCLUSION: all 3 Phase-1 mechanisms (edit-lever, anti-thrash, verifier-feedback retry) already exist,
  yet wrong-edit-verified-fail is 60-80% → the bottleneck is a CAPABILITY ceiling (model can't choose
  the right edit even WITH the failing test fed back + retries), NOT a missing feature. Phase 1's only
  genuinely-additive lever is **Stream A (behavior-keyed retrieval = better info, not another retry)**.
- Possible A×C synergy (future): on a verifier-rejected finish, ALSO inject the behavioral hint.

## Log (append-only; newest at bottom)
- [2026-06-21] (Claude) created AGENTS.md + CODEX_TASK.md on prereboot-flywheel-prep; starting Stream A (A1).
- [2026-06-21] (Claude) A1 DONE on branch claude/behavior-keyed-retrieval: src/agentd/fault_signature.rs (fault_signature/hamming/similarity), build clean, 4 unit tests green. Next: A2 (index solved trajectories by signature) + A3 (wire into retrieve_ice).
- [2026-06-22] (Claude) A2+A3+A4 DONE: build_signature_index.py -> signature_index.json (34 entries/7 families); Rust SignatureIndex (load+nearest+self-exclusion); behavioral retrieval wired post-binding, flag PROFESSOR_X_BEHAVIOR_RETRIEVAL (default OFF). Full suite 370/370 green. A5 measurement DEFERRED to avoid GPU contention with Codex Stream B — will run ON vs OFF on the held-out anchors once the GPU frees.
- [2026-06-22] (Claude) Stream B confirmed COMPLETE by Codex; integrated finding: wrong-edit-verified-fail dominates (61-81%) => Stream A (inject correct behaviorally-matching fix) is well-targeted; Stream C reframed to RLEF-style verifier-feedback retry. GPU free -> A5 RUNNING (release binary rebuilt with behavior retrieval; ON vs OFF on 14 held-out anchors, qwen3:8b, with behavior_hit counting).
