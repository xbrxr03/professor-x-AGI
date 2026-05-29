# Phase A + B Reality Audit

**Date:** 2026-05-28
**Branch:** `audit/phase-ab-hiro-paper`
**Auditor:** operator (via Claude session)
**Source of truth:** [`professor-x/ops/runbooks/eleven-out-of-ten-harness.md`](../../professor-x/ops/runbooks/eleven-out-of-ten-harness.md)

## TL;DR

- **Phase A — Codex-Grade Tool Loop:** ~70% shipped. Strong transcript layer, patch tool, interactive CLI, cancellation. Missing: pause/resume, formal failure classification, command-output truncation policy.
- **Phase B — Artifact Truth Layer:** ~15% shipped. `ArtifactValidator` exists but only runs on `TaskType::Scheduled` and only checks two hardcoded conditions. None of the seven artifact schemas named in the roadmap are defined.
- **Headline:** the operator role and verify-then-commit gate (Phase C work) are landing autonomous commits while Phase B — the layer that's supposed to prevent fake artifacts from being credited — is essentially unbuilt. `brain/experiment_results.md` containing fake temperature data (creativity 8.2 vs 6.7, p=0.003) sits in-tree unflagged. Phase A acceptance is also undertested in the repo (transcripts dir is empty in-tree, runtime data lives in `PROFESSOR_X_DATA_DIR`).

## Phase A — Codex-Grade Tool Loop

Roadmap acceptance: *"A user can give a repo task, watch each step in `--lab`, inspect the generated diff, and see why the task passed or failed."*

| Item | Status | Evidence |
|---|---|---|
| Structured task transcripts (thought/action/observation, file diffs, command outputs, exit codes, duration, artifact links) | ✅ | [`src/memd/transcripts.rs`](../../professor-x/src/memd/transcripts.rs) — `TaskTranscript`/`TranscriptStep` schemas cover all of these; persisted to SQLite + JSON under `artifacts/transcripts/YYYY-MM-DD/{task_id}.json`. Git diff snapshot included with 32k-char truncation. Unit test `transcript_contains_review_bundle` passes. |
| First-class patch application and diff review helpers | ✅ | Reviewable patch tool (commit `6fa7311`); `patch.apply` with `mode=check`/`mode=apply` documented in [`autonomous-run.md`](../../professor-x/ops/runbooks/autonomous-run.md). Patches persisted under `artifacts/patches/`. |
| Command output truncation with full artifact capture | ⚠️ partial | Git-diff truncation present (32k chars). Need to verify command-output truncation policy in `toolbridge::executor` — commit `4e1c295` says "Capture shell command output artifacts" but no policy file documents the limits. |
| `--chat` / `--task-interactive` for conversational tasking | ✅ | `main.rs:188` handles both flags. |
| Task cancellation | ✅ | `CancellationToken` plumbed through React loop, HIRO runner, scheduler. SIGUSR2 graceful shutdown documented. |
| Pause/resume | ❌ | No CLI surface. The only `pause` reference is the ClawOS circuit breaker (`react.rs:9, 305, 511`) — 3 consecutive tool failures pauses the loop internally, not user-facing. |
| Visible failure classification | ❌ | No `FailureClass` type, no `classify_failure` function. Failures are surfaced as strings in transcript `summary` and event payloads; no taxonomy. This blocks Phase D's "every failure produces a DHE trace" coupling. |

### Phase A gaps to close

1. Add command-output truncation policy + tests (or document the existing one if it's hidden in `executor.rs`).
2. Add a `FailureClass` enum aligned with the DHE 5-layer taxonomy so Phase D can attach attributions without re-parsing strings.
3. Decide whether pause/resume is in scope for "Codex-grade." If yes, add `--task-pause`/`--task-resume`; if no, strike it from the roadmap.
4. Confirm whether the in-tree `artifacts/transcripts/` is supposed to receive any data or whether all transcripts go to `PROFESSOR_X_DATA_DIR`. Currently only `.gitkeep` lives there, which contradicts the runbook's `find artifacts/transcripts -type f | sort` inspection step.

## Phase B — Artifact Truth Layer

Roadmap acceptance: *"Bad artifacts like nested `professor-x/professor-x/...` or fake dated notes fail automatically and are visible in `--lab`."*

### Current state of [`src/artifacts.rs`](../../professor-x/src/artifacts.rs)

```rust
pub fn validate_task(&self, task: &TaskNode) -> Result<Option<ArtifactValidationReport>> {
    if task.task_type != TaskType::Scheduled {
        return Ok(None);   // ← every non-scheduled task is silently exempt
    }
    // Check 1: no `professor-x` subdir at CWD (vacuous when run from inner crate dir)
    // Check 2: for daily-update jobs, ops/daily/{today}.md exists
}
```

That is the entire artifact-truth layer.

| Roadmap item | Status | Notes |
|---|---|---|
| Schemas: daily updates | ⚠️ existence-only | Checks the file exists; does not parse, validate dates, sections, or citations. |
| Schemas: literature notes | ❌ | Not defined. |
| Schemas: experiments | ❌ | `brain/experiment_results.md` contains fake temperature comparison (`Creativity 8.2 vs 6.7, p=0.003`) with no run id, no harness commit, no method section. Nothing in the codebase flags it. |
| Schemas: HIRO runs | ❌ | No schema. No HIRO artifacts have ever been written to the repo (see HIRO audit). |
| Schemas: proposals / rejections | ⚠️ implicit | Evolution proposal artifacts have a de facto schema in `evolved::proposer`, but it's not validated by `ArtifactValidator`. Rejections directory is empty. |
| Schemas: paper drafts | ❌ | Not defined. |
| Validators: dates, paths, source citations, run ids, commit ids, required fields | ❌ | None implemented. |
| Mark tasks failed on missing/stale/unsupported artifacts | ⚠️ partial | Only the daily-note check can fail a task today; everything else short-circuits to `Ok(None)`. |
| Surface artifact links in observer selected-event payloads | ✅ | `transcript.written`, `artifact.valid`, `artifact.invalid` events are emitted per runbook. |

### Phase B gaps to close

1. Define `ArtifactSchema` enum: `DailyUpdate`, `LiteratureNote`, `ExperimentResult`, `HiroRun`, `EvolutionProposal`, `EvolutionRejection`, `PaperSection`. One Rust type per kind.
2. Each schema declares required fields: `date`, `run_id`, `harness_commit`, `source_citations`, `method`, `metric_values`, `path_root`.
3. Validator runs on **every** task that claims to produce an artifact, not only `TaskType::Scheduled`. Tighten the dispatcher: tasks declare expected artifact type, validator picks the matching schema.
4. Add a one-shot CLI `--validate-artifacts` that scans `brain/` and `artifacts/` against the schemas and prints a report. This would have caught `experiment_results.md` on day one.
5. Wire the validator into the daily-cycle: every job's expected artifact type is declared in `ops/schedules/daily-cycle.toml`; missing/invalid artifact = task failed, regardless of LLM "success" output.

## Phase A + B verdict against 11/10 acceptance criteria

| Criterion | Status |
|---|---|
| Observable | ✅ event stream + transcripts + observer panels |
| Safe | ✅ workspace bind, audit, kill switch |
| Useful | ❌ daily loop has not yet produced research artifacts that survive Phase B validation (because Phase B doesn't validate them) |
| Measurable | ❌ no HIRO data recorded (separate audit) |
| Codex-grade coding | ⚠️ patch loop present; failure classification absent |
| Verify-then-commit | ✅ Phase C working end-to-end on smoke proposals |
| Diagnostic | ⚠️ DHE module exists; not yet attached to every failure |
| Adaptive | ❌ no round-level evidence yet |
| Skillful | ⚠️ 15 skills loaded; quality tracking unimplemented |
| Scientific | ❌ no run ids on any commit-credited change |
| Self-evolving | ⚠️ gate works; lacking real evidence to feed it |

## Recommended sequencing

1. **Stop new operator commits** until Phase B has at least the `ExperimentResult` and `HiroRun` schemas + validators. Rationale: every operator commit landed today is uncreditable per Section 10 of the roadmap (Scientific: "research claims require artifacts, metrics, run ids, and falsifiable hypotheses").
2. Land `ArtifactSchema` enum + per-kind validators (Phase B item 1–3 above) on a branch off main.
3. Backfill: validate every existing artifact in `brain/` and `artifacts/evolution/accepted/`. Quarantine failures in `brain/_quarantine/`.
4. Re-open the operator role once `--validate-artifacts` returns clean and the daily cycle emits validated artifacts only.

## Open questions for the operator

1. Is `PROFESSOR_X_DATA_DIR` the only persistence target for transcripts, or should the repo `artifacts/transcripts/` also receive a curated subset (e.g., accepted-proposal transcripts)?
2. Should `ArtifactValidator` run on `TaskType::UserRequest` and `TaskType::Operator` as well? Current `Scheduled`-only scope misses every operator commit.
3. Is the `professor-x/professor-x/` nesting the intended layout, or a known wart? The validator's `no_nested_professor_x_dir` check only catches one specific bad output pattern; the structural nesting itself is unflagged.
