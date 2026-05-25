# Repository Structure

This repository is organized around safe autonomous evolution. Source code stays small and reviewable; generated data, verification records, and paper artifacts have dedicated homes.

## Top Level

- `professor-x/` - Rust crate and runtime-owned project assets.
- `brain/` - human-readable research state for the paper and hypotheses.
- `docs/` - architecture, operating procedures, and repository conventions.
- `scripts/` - operator scripts for repeatable local runs.
- `_refs/` - cloned reference repositories and external research material.

## Rust Crate

- `professor-x/src/memd/` - memory stores and SQLite schema.
- `professor-x/src/toolbridge/` - tool registry, schema validation, skill loading, and tool execution.
- `professor-x/src/agentd/` - task graph, scheduler, ReAct loop, and outcome production.
- `professor-x/src/policyd/` - path, shell, URL, audit, vault, and approval policy.
- `professor-x/src/evolved/` - HIRO, DHE, BF, LCAP, proposal, verification, and autonomous evolution logic.

## Harness Assets

- `professor-x/harness/config/` - evolvable harness configuration that is not core Rust.
- `professor-x/harness/prompts/` - prompt assets and prompt variants.
- `professor-x/harness/tool_descriptions/` - tool description variants for controlled evolution.
- `professor-x/harness/skills/` - generated or promoted harness-level skill definitions.
- `professor-x/harness/middleware/` - non-core middleware assets.
- `professor-x/harness/policies/` - policy configuration and allowlist data.

## Benchmarks And Artifacts

- `professor-x/hiro/` - benchmark task definitions. `tasks.json` is the source of truth.
- `professor-x/artifacts/hiro/attempts/` - exported attempt-level HIRO results.
- `professor-x/artifacts/hiro/rounds/` - exported round-level BF/HIRO summaries.
- `professor-x/artifacts/hiro/null-baselines/` - static-harness null-condition runs.
- `professor-x/artifacts/hiro/regression-subsets/` - selected fast HIRO subsets used by verification gates.

## Evolution Records

- `professor-x/artifacts/evolution/proposals/` - proposal provenance and generated diffs.
- `professor-x/artifacts/evolution/verifications/` - checks, logs, and pass/fail outcomes.
- `professor-x/artifacts/evolution/reward-hacking/` - reward-hacking scan records.
- `professor-x/artifacts/evolution/accepted/` - accepted change manifests.
- `professor-x/artifacts/evolution/rejections/` - rejected proposals and reasons.
- `professor-x/artifacts/evolution/rollbacks/` - rollback events and post-rollback checks.

## Safety And Operations

- `professor-x/artifacts/audit/` - audit-chain exports and chain verification snapshots.
- `professor-x/sandbox/worktrees/` - temporary evolution worktrees. Do not treat as source of truth.
- `professor-x/sandbox/patches/` - proposed patches before verification.
- `professor-x/ops/schedules/` - declarative scheduled jobs.
- `professor-x/ops/daily/` - daily-cycle plans and dry-run outputs.
- `professor-x/ops/runbooks/` - operator procedures for HIRO, rollback, and autonomous runs.

## Paper Outputs

- `professor-x/artifacts/paper/tables/` - generated metric tables.
- `professor-x/artifacts/paper/figures/` - BF, DHE, LCAP, ICS, and FED plots.
- `professor-x/artifacts/paper/runs/` - complete experiment run summaries.

## Tests

- Rust unit tests live next to the code they protect.
- `professor-x/tests/policy/` - policy integration fixtures.
- `professor-x/tests/hiro/` - HIRO fixture tasks and expected evaluator results.
- `professor-x/tests/evolution/` - verification, rollback, and reward-hacking fixtures.
- `professor-x/tests/skills/` - skill loading and permission-scope fixtures.

## Rules

- Do not write generated benchmark or evolution data into `src/`.
- Do not commit sandbox worktrees.
- Every autonomous code change must have a proposal, verification, decision, and, if accepted, a git commit.
- HIRO claims must cite a run id, harness commit, and recorded metric.
