# px-know-harness

## Purpose
Reason about the Professor X harness as the evolvable operating system around the model.

## Knowledge
- `memd` owns persistent memory and experiment/evolution tables.
- `toolbridge` owns schemas, skill loading, and approved tool execution.
- `agentd` owns task scheduling and ReAct execution.
- `policyd` owns workspace boundaries, command policy, vault/audit protection, and kill-switch behavior.
- `evolved` owns HIRO, DHE, BF, LCAP, proposals, verification, and rollback.
- Generated data belongs in `artifacts/`; source changes belong in `src/`, `harness/`, `skills/`, `ops/`, or `brain/` depending on ownership.

## Use When
Use this skill before proposing file changes, diagnostics, policy edits, or evolution targets.
