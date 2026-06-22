# 2026-06-21 Failure Taxonomy

Measured on **2026-06-22** from the `codex/failure-taxonomy` worktree using `/tmp/px-codex-target/release/professor-x`.

Command recipe:
```bash
PROFESSOR_X_NATIVE_TOOLS=1 PROFESSOR_X_DATA_DIR=$HOME/.professor-x \
REPO_FIX_TASKS=<manifest> ./target/release/professor-x --repo-fix-bench --model <model>
```

Manifests:
- `hard` -> `scripts/benchmarks/repo_fix/tasks_hard_full.json`
- `family:csv` -> `scripts/benchmarks/repo_fix/tasks_family_csv.json`
- `family:graph` -> `scripts/benchmarks/repo_fix/tasks_family_graph.json`
- `family:interval` -> `scripts/benchmarks/repo_fix/tasks_family_interval.json`
- `family:money` -> `scripts/benchmarks/repo_fix/tasks_family_money.json`
- `family:sm` -> `scripts/benchmarks/repo_fix/tasks_family_sm.json`
- `family:stack` -> `scripts/benchmarks/repo_fix/tasks_family_stack.json`
- `family:unit` -> `scripts/benchmarks/repo_fix/tasks_family_unit.json`

## Results

| model | task set | tasks | pass@1 | passed/ran | duplicate_action | finish_rejected | edit-apply-error | wrong-edit-verified-fail | loop/forfeit | other |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| profx-distilled-clean | hard | 30 | 0.133 | 4/30 | 1 | 2 | 3 | 20 | 0 | 0 |
| profx-distilled-clean | family:csv | 5 | 0.400 | 2/5 | 0 | 0 | 1 | 2 | 0 | 0 |
| profx-distilled-clean | family:graph | 4 | 0.500 | 2/4 | 0 | 1 | 0 | 1 | 0 | 0 |
| profx-distilled-clean | family:interval | 5 | 0.000 | 0/5 | 1 | 4 | 0 | 0 | 0 | 0 |
| profx-distilled-clean | family:money | 5 | 0.200 | 1/5 | 0 | 1 | 0 | 3 | 0 | 0 |
| profx-distilled-clean | family:sm | 4 | 0.000 | 0/4 | 0 | 1 | 1 | 2 | 0 | 0 |
| profx-distilled-clean | family:stack | 6 | 0.333 | 2/6 | 0 | 0 | 2 | 2 | 0 | 0 |
| profx-distilled-clean | family:unit | 5 | 0.200 | 1/5 | 1 | 0 | 1 | 2 | 0 | 0 |
| qwen3:8b-q4_K_M | hard | 30 | 0.400 | 12/30 | 3 | 0 | 1 | 14 | 0 | 0 |
| qwen3:8b-q4_K_M | family:csv | 5 | 0.000 | 0/5 | 0 | 0 | 0 | 5 | 0 | 0 |
| qwen3:8b-q4_K_M | family:graph | 4 | 0.500 | 2/4 | 0 | 0 | 1 | 1 | 0 | 0 |
| qwen3:8b-q4_K_M | family:interval | 5 | 0.400 | 2/5 | 0 | 0 | 0 | 3 | 0 | 0 |
| qwen3:8b-q4_K_M | family:money | 5 | 0.200 | 1/5 | 0 | 1 | 0 | 3 | 0 | 0 |
| qwen3:8b-q4_K_M | family:sm | 4 | 0.500 | 2/4 | 0 | 0 | 2 | 0 | 0 | 0 |
| qwen3:8b-q4_K_M | family:stack | 6 | 0.333 | 2/6 | 0 | 0 | 0 | 4 | 0 | 0 |
| qwen3:8b-q4_K_M | family:unit | 5 | 0.000 | 0/5 | 0 | 0 | 0 | 5 | 0 | 0 |

## Provenance

- 7 row(s) were ingested from existing native bench artifacts via `--reuse-existing-root` instead of rerun in this invocation: `qwen3:8b-q4_K_M/family:csv`, `qwen3:8b-q4_K_M/family:graph`, `qwen3:8b-q4_K_M/family:interval`, `qwen3:8b-q4_K_M/family:money`, `qwen3:8b-q4_K_M/family:sm`, `qwen3:8b-q4_K_M/family:stack`, `qwen3:8b-q4_K_M/family:unit`.

## Actionable Read

- profx-distilled-clean: `wrong-edit-verified-fail` dominates at 32/52 (61.5%), while `edit-apply-error` is smaller at 8/52 (15.4%). That makes verifier-informed retry directionally useful, but not sufficient on its own.
- profx-distilled-clean: `finish_rejected` clusters most in `family:interval` (4 case(s)), so finish-gating regressions are family-specific rather than the main global failure mode.
- qwen3:8b-q4_K_M: `wrong-edit-verified-fail` dominates at 35/43 (81.4%), while `edit-apply-error` is smaller at 4/43 (9.3%). That makes verifier-informed retry directionally useful, but not sufficient on its own.
- qwen3:8b-q4_K_M: `finish_rejected` clusters most in `family:money` (1 case(s)), so finish-gating regressions are family-specific rather than the main global failure mode.
- Cross-model outlier: `family:interval` splits sharply by model. `profx-distilled-clean` hit `finish_rejected` 4/5 times at 0.000 pass@1, while `qwen3:8b-q4_K_M` reached 0.400 pass@1 with failures entirely in `wrong-edit-verified-fail` (3).
- High-ROI qwen families are `family:csv` and `family:unit`: both were 0.000 pass@1 and every failure landed in `wrong-edit-verified-fail`, which points at bad patch choice rather than tool execution trouble.
- The harness is not mainly losing to endless thrash anymore: `loop/forfeit` and `other` were both zero across the measured matrix.

## Representative Failures

- profx-distilled-clean: representative `wrong-edit-verified-fail` tasks include `hard/hard_001`, `hard/hard_002`, `hard/hard_003`, `hard/hard_004`, `hard/hard_005`.
- profx-distilled-clean: representative `finish_rejected` tasks include `hard/hard_011`, `hard/hard_014`, `family:graph/fam_graph_01`.
- profx-distilled-clean: representative `edit-apply-error` tasks include `hard/hard_012`, `hard/hard_023`, `hard/hard_029`.
- qwen3:8b-q4_K_M: representative `wrong-edit-verified-fail` tasks include `hard/hard_003`, `hard/hard_004`, `hard/hard_006`, `hard/hard_009`, `hard/hard_011`.
- qwen3:8b-q4_K_M: representative `finish_rejected` tasks include `family:money/fam_money_04`.
- qwen3:8b-q4_K_M: representative `edit-apply-error` tasks include `hard/hard_001`, `family:graph/fam_graph_01`, `family:sm/fam_sm_02`.

## Honest Read

- profx-distilled-clean failed 52 task(s); the dominant bucket was `wrong-edit-verified-fail` at 32/52 (61.5%), which is the first place to attack if Stream C is going to buy real pass@1.
- qwen3:8b-q4_K_M failed 43 task(s); the dominant bucket was `wrong-edit-verified-fail` at 35/43 (81.4%), which is the first place to attack if Stream C is going to buy real pass@1.