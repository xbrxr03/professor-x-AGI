# AGENTS.md — Claude × Codex coordination ledger

Two agents work this repo in PARALLEL (Claude = Professor-X dev; Codex). **Read this file before you
start. Check your box and append a log line when you finish a unit of work.** Phase 1 plan:
`.claude/plans/sparkling-sauteeing-marshmallow.md`; full map: `docs/PROJECT_ATLAS.md`.
**PICKUP BRIEFS (read yours after this file): Claude → `CLAUDE_TASK.md` · Codex → `CODEX_TASK_P3.md`.**
Current phase: `docs/PLAN_PHASE3_2026-06-22.md`.

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
### Stream A — behavior-keyed retrieval (Claude) — MERGED 2026-06-22
- [x] A1 fault_signature.rs · [x] A2 signature_index.json · [x] A3 wired (flag default OFF) ·
      [x] A4 suite green · [x] A5 measured: mechanism VALIDATED (14/14 origin matches) but pass@1 lift
      marginal/within-noise -> retrieval-use SHELVED, representation KEPT (doc: 2026-06-22-RESULT-A5-*).
### Stream B — failure taxonomy (Codex) — MERGED
- [x] B1/B2/B3 done. FINDING: wrong-edit-verified-fail dominates (61-81%); bottleneck = edit CAPABILITY.
### Stream C — verifier-feedback retry — ALREADY BUILT (RLEF loop in react.rs) -> SKIPPED.

## PHASE 3 — distillation flywheel vs the wrong-edit ceiling (see docs/PLAN_PHASE3_2026-06-22.md)
Disjoint: Claude owns `src/` (Rust gate); Codex owns `distill/` (Python training). No shared files.
GPU: Codex owns it during collect+train; Claude's gate MEASUREMENT runs after — never concurrently.
### Stream D — Claude (TGC trust-gate + collection quality)  [gate BUILT, full run = integration]
- [x] D1 `scripts/benchmarks/repo_fix/tgc_gate.py`: pure `decide()` (accept iff held-out anchor delta
      >=MDE AND Goodhart gap bounded) + bench-running mode; `--self-test` PASS (4 cases incl. the
      overfit/Goodhart-reject). Full GPU run = INTEGRATION (gate Codex's profx-distilled-p3 vs stock).
      NOTE: built as a Claude-owned script (orchestration over the existing bench), not a main.rs change.
- [~] D2 NaN guard: `gguf_is_safe()` precondition in the gate (present + non-tiny); full NaN check is in
      Codex's quantize step (E2); stop-sanity at gate-run time.
- [x] D3 collection already captures verified-correct native-tool-call trajectories (exists; verify-only).
### Stream E — Codex (Python: run the pipeline on the failure frontier) — see CODEX_TASK_P3.md
- [ ] E1 collect teacher qwen3:14b verified-correct trajectories on the wrong-edit frontier
- [ ] E2 QLoRA train (assistant-only mask + 2 epochs) -> clean GGUF (NaN-checked, stop-sane) -> serve profx-distilled-p3
- [ ] E3 hand profx-distilled-p3 to Claude's TGC gate (do not self-declare success)

## Log (append-only; newest at bottom)
- [2026-06-21] (Claude) created AGENTS.md + CODEX_TASK.md on prereboot-flywheel-prep; starting Stream A (A1).
- [2026-06-22] (Claude) PHASE 1 CONCLUDED (on branch claude/behavior-keyed-retrieval): Stream A built+measured (behavior retrieval validated as mechanism, pass@1 lift marginal/within-noise), Stream C found already-built. Bottleneck = edit-production CAPABILITY. Scoped PHASE 3 here (Stream D Claude / Stream E Codex) — distillation flywheel + TGC trust-gate. Codex: see CODEX_TASK_P3.md.
- [2026-06-22] (Claude) committed Codex's Stream B (commit 4302a20, pushed). Started Stream D on branch claude/p3-tgc-gate: TGC gate tgc_gate.py built + --self-test PASS (decision logic rejects the train-overfit/Goodhart case). D2 precondition guard in gate, D3 collection already exists. Full GPU gate run = integration after Codex Stream E. GPU left FREE for Codex training.
- [2026-06-22] (Claude) MERGED completed work into prereboot-flywheel-prep: Stream A (claude/behavior-keyed-retrieval), Stream B (codex/failure-taxonomy), Stream D (claude/p3-tgc-gate). Build clean + full suite 370/370 green. Switching to parallel-code (v1.10.0) for worktree orchestration.
- [2026-06-22T05:46:00-04:00] (Codex) starting Stream E on branch `codex/p3-distill`; claimed GPU for qwen3:14b frontier collection + QLoRA train on the 35-task wrong-edit manifest.
- [2026-06-22T16:33:40-04:00] (Codex) paused Stream E on `codex/p3-distill` after prep + initial teacher collection: committed distill frontier/safety helpers, collected 7 verified trajectories so far (`hard_004`, `fam_csv_01`, `fam_csv_02`, `fam_money_03`, `fam_stack_02`, `fam_stack_04`, `fam_unit_04`), latest non-CSV frontier sweep (`repo-fix-202836-47664c90.json`) went 4/16 with `qwen3:14b-q4_K_M`; no active GPU jobs.
- [2026-06-22] (Claude) resolved AGENTS.md merge conflict (unioned the log) + merged PR #23 (Codex Stream E prep) into prereboot-flywheel-prep. Stream E E1 partial (7 verified teacher trajectories so far); E2 training + E3 gate handoff still ahead.
- [2026-06-22] (Claude) HANDOFF for parallel-code: wrote `CLAUDE_TASK.md` (self-contained pickup brief — a fresh Claude task off prereboot-flywheel-prep reads it + AGENTS.md and resumes). CLAUDE PICKUP = D-INTEGRATION: when Codex serves `profx-distilled-p3` and the GPU is free, run `scripts/benchmarks/repo_fix/tgc_gate.py` (baseline qwen3:8b vs candidate profx-distilled-p3, train=tasks_families.json, heldout=anchors) → report accept/reject + Goodhart gap honestly. Do NOT gate while Codex trains.
- [2026-06-22] (Claude) REFERENCE CROSS-CHECK AUDIT on branch `claude/ref-cross-check` (no GPU; read-only on the corpus): read ~48 curated reference docs + cross-checked under verify-the-ruler → `docs/audits/2026-06-22-reference-cross-check.md`. Patched 7 stale spots (Atlas Lever-1 provenance flag, brain/hypotheses+inventions staleness banners, old failure-taxonomy.md SUPERSEDED banner, standard-readiness 0.85→~0.77 + M4-curve-retracted note, dead-ends broken INVALIDATED_COMMITS link, backlog QLoRA-unblocked). **KEY FINDING (F1):** Atlas "clean distilled 0.40 > stock 0.30" (the flywheel un-pause reason) is contradicted by the 06-22 taxonomy (`profx-distilled-clean` hard=0.133 < `qwen3:8b` 0.400) → treat "distilled beats stock" as UNPROVEN until the D-integration TGC gate measures p3. **FOR CODEX:** `docs/research/2026-06-21-failure-taxonomy.md` (yours per ownership) is internally mis-dated — title "2026-06-21" but body "Measured on 2026-06-22"; fix at your convenience.
- [2026-06-22] (Claude) D-INTEGRATION DONE — ran the TGC gate on Codex's `profx-distilled-p3` (GPU free, build-only flywheel had deferred the gate). **VERDICT: REJECT.** Held-out renamed-anchor pass@1 (K=3): baseline qwen3:8b **0.500** vs p3 **0.238** = **−0.262 ≪ MDE**. Candidate does NOT generalize (26pts worse than stock on the contamination-proof set). Train half = NaN (manifest bug: `tasks_families.json` tasks lack the `category` field the binary needs → Goodhart gap unmeasured; verdict stands on held-out). Result: `docs/research/2026-06-22-RESULT-D-integration-tgc-gate-p3.md`. Resolves audit F1 (distilled does NOT beat stock). **Do NOT serve p3 as default** — harness stays on qwen3:8b. FOR CODEX: recipe iteration needed (E2 candidate underperforms); also add `category` to `tasks_families.json` so the gate's train half runs. Gate-script edits (per-bench timeout 1800→7200s, anchors-first progress logging) are on branch `claude/ref-cross-check`.
