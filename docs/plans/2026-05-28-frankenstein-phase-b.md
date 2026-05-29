# Frankenstein Build — Phase B Truth Layer + HIRO Persistence

**Date:** 2026-05-28
**Branch:** `feat/frankenstein-phase-b-truth-layer` (off main)
**Prerequisite reading:** [`docs/audits/2026-05-28-phase-ab-reality.md`](../audits/2026-05-28-phase-ab-reality.md), [`docs/audits/2026-05-28-hiro-baseline-check.md`](../audits/2026-05-28-hiro-baseline-check.md), [`docs/audits/2026-05-28-paper-outline-freshness.md`](../audits/2026-05-28-paper-outline-freshness.md)

## Goal

Make Professor X's evolution loop scientifically honest. Right now the verify-then-commit gate (Phase C) is working, but it's gating *against fabricated artifacts*. The May 24 commits flipped hypothesis status to "Confirmed" with invented HIRO numbers and slipped past the gate because Phase B (the artifact-truth layer that's supposed to catch exactly this) is essentially unbuilt.

This PR builds the Phase B truth layer plus the HIRO persistence path. After this lands, the gate has something real to gate on.

## Out of scope (deferred to follow-up PRs)

- IPE modules: `self_model.rs`, `ics.rs`, `affect.rs`, `free_energy.rs` (Phase D+)
- Voyager-style verified skill library + EvolveR-style `(success+1)/(use+2)` decay (Phase E)
- `--memory-budget` flag for H1 sweep (separate experiment PR)
- Endpoint abstraction in `ollama.rs` for H9 (separate refactor PR)
- Actually running HIRO null baseline (operational task, separate session on Linux)

## Frankenstein pulls landed in this PR

| System | What we pull | Where it lands |
|---|---|---|
| AHE (arXiv:2604.25850) | ChangeManifest schema discipline — every artifact declares fields, fields are validated | `ArtifactKind` enum + per-kind `validate` |
| MOSS / Phase C verify-then-commit | "An artifact is invalid until proven valid" inversion | `ArtifactValidator` returns `Invalid` by default for declared kinds when required fields missing |
| Codex / Claude Code transcript discipline | `run_id` + `harness_commit` on every record | HIRO attempt/round JSON includes both |
| Scientific-agent repos | Source citations are required fields, not optional | `LiteratureNote` + `ExperimentResult` schemas require `citations: Vec<String>` non-empty |
| ClawOS audit chain | Every artifact decision is an event | `artifact.{kind}.valid` / `artifact.{kind}.invalid` events emitted |

## Build sequence

### Phase 1 — Truth-layer fixes (docs only, fast)

1. **Quarantine reward-hacked commits.** New `docs/audits/INVALIDATED_COMMITS.md` listing `1896fa2`, `121ab6a`, `ba7a998` with reasons. Append entry to `brain/dead-ends.md`. No `git revert` — history stays; downstream code rejects status flips not backed by HIRO artifacts.
2. **Pin model references.** `brain/hypotheses.md` H1 + H9 → `qwen3:8b-q4_k_m`. Note the model migration where applicable.
3. **Resolve nested-brain split.** Add `professor-x/brain/` to `.gitignore`. Delete tracked corrupted files. Leave `professor-x/brain/STUB.md` noting "agent writes go to repo-root `brain/`; this directory is reserved for runtime scratch and is gitignored."

### Phase 2 — ArtifactKind enum + per-kind validators

4. **`ArtifactKind` enum** with variants: `DailyUpdate`, `LiteratureNote`, `ExperimentResult`, `HiroRun`, `HiroNullBaseline`, `EvolutionProposal`, `EvolutionRejection`, `PaperSection`.
5. **`ArtifactSchema` trait** per kind. Each kind declares:
   - `required_fields()` — list of field names
   - `validate(path: &Path) -> ArtifactValidationOutcome` — parses file (markdown frontmatter or JSON) and confirms each required field is present, non-empty, and well-typed.
6. **Cross-cutting checks** (run for every kind):
   - Path-root check: file is in the expected directory for its kind.
   - No nested `professor-x/brain/*` writes.
   - Date in filename matches `recorded_at` field (within 1 day tolerance for timezone slop).
   - `harness_commit` field exists in git history (`git cat-file -e <hash>` returns success).

### Phase 3 — Dispatcher + observer wiring

7. **`TaskNode::expected_artifact_kind: Option<ArtifactKind>`**. Daily-cycle jobs declare it in `daily-cycle.toml`. Operator commits declare `EvolutionProposal`.
8. **Validator runs on every task that declares a kind**, not only `Scheduled`. Tasks that don't declare are exempt with a warning event.
9. **Events:** `artifact.{kind}.valid {path, fields}` and `artifact.{kind}.invalid {path, missing_fields, reason}`. Visible in `--lab` and `--events`.
10. **Failure side-effect:** task moves to `Failed` status with `failure_reason` populated from the report.

### Phase 4 — `--validate-artifacts` CLI

11. **One-shot scanner.** Walks `brain/`, `artifacts/`, `ops/daily/`. Picks the schema based on path. Prints `PASS/FAIL` per artifact with reason. Exit 1 on any failure.
12. **CI-friendly.** A pre-commit hook can call this; current operator commits would be caught before they land.

### Phase 5 — HIRO persistence

13. **`HiroRunner::run_round`** writes:
    - `artifacts/hiro/attempts/{run_id}/{task_id}.json` — one per task attempt with `attempts[]`, `pass`, `category`, `elapsed_ms`.
    - `artifacts/hiro/rounds/{run_id}-r{round}.jsonl` — one per round with fingerprint `[p_tool, p_plan, p_correct]`, `pass_at_3`, `harness_commit`, `recorded_at`.
14. **`--hiro-null`** writes to `artifacts/hiro/null-baselines/{run_id}.json` with frozen-harness flag = true.
15. **`run_id`** is a UUID generated at run start; `harness_commit` is `git rev-parse HEAD` captured at start.
16. **Events:** `hiro.round.started`, `hiro.attempt.completed`, `hiro.round.completed` with payload pointing to the artifact path.

### Phase 6 — Tests

17. Unit tests per `ArtifactKind`: known-good shape passes, each required-field-missing case fails with the right `missing_fields` list.
18. End-to-end test: synthetic HIRO round (1 task, mocked LLM) writes attempt + round files, both pass the validator.
19. Regression test for the corrupted nested `professor-x/brain/hypotheses.md` — schema rejects it.

## Out-of-scope but worth flagging for next PR

- **Voyager skill quality store.** Skills currently load but don't track quality. Adding `SkillQuality { uses: u32, successes: u32, score: f32 }` with EvolveR-style `(success+1)/(use+2)` decay unlocks H2 + H7 testing.
- **`MetacognitiveEntry` store for H13.** After DHE attribution + next-round verification, write `{predicted_layer, predicted_lever, actual_improvement, attribution_correct}` to `memd.semantic`. Required for MCA computation.
- **IPE module stubs.** Even just struct + persistence (no LLM update logic yet) for `SelfModelSnapshot`, `AffectState`, `FreeEnergyDelta`, `IcsScore` would unblock plan refinement.

## How we know this PR worked

After this PR lands and one HIRO null run completes on the Linux box:

1. `find artifacts/hiro -name '*.json' | wc -l` returns > 4 (1 round file + 60 attempt files for a full run; smaller for `--hiro-limit`).
2. `cargo run -- --validate-artifacts` exits 0 on the clean tree.
3. If somebody manually edits `brain/hypotheses.md` to flip an H to "Confirmed" without a backing HIRO artifact, `--validate-artifacts` fails with `missing_fields: [hiro_run_id]`.
4. The `experiment_results.md` placeholder either gets a real schema + citations or fails validation (forcing the operator to delete or backfill it).

## What this PR does *not* fix

- Will not run a HIRO baseline. That's an operational task for the Linux machine after merge.
- Will not retroactively validate the May 24 commits' fabricated numbers. Those commits are quarantined via docs; their existence in git history is preserved as a methodological dead-end record.
- Will not migrate the agent's runtime data dir off `PROFESSOR_X_DATA_DIR`. The split between repo-tracked artifacts (decisions, baselines, accepted proposals) and runtime scratch (transcripts, working memory) stays.
