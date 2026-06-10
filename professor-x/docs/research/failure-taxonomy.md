# Failure Taxonomy — 2026-06-08 HIRO Null Runs

Source data:
- `artifacts/trajectories/2026-06-08/trajectories.jsonl`
- `/tmp/ab_off.log`
- `/tmp/ab_on.log`

Purpose: satisfy `NEXT_STEPS.md` Phase 0.1 and rank the actual blocker before
building more harness features.

## Summary

The top failure is not bad edit matching. The measured tasks are mostly
read/search/shell tasks; there are no file-edit tasks in this sample. The dominant
wall is that the agent either loops until `MAX_STEPS=20` or declares `finish {}`
without producing the requested answer.

Immediate highest-ROI build order:

1. Require answer-bearing `finish` actions and reject empty `finish {}`.
2. Add repeated-failure/max-step controls that force synthesis or forfeit earlier.
3. Re-measure HIRO null baseline.
4. Then continue to hash-anchored edit tools, once benchmark failures include edit
   tasks or the next run shows edit failures.

## Counts

| Failure class | Evidence count | Share | Notes |
| --- | ---: | ---: | --- |
| Wrong/no final answer | 20 / 20 stored trajectories | 100% of stored trajectories | Every trajectory ends with bare `Action: finish` and `Action Input: {}`. HIRO reports `p_correct=0.000` even when tool-use passed. |
| Max-step / thrash | 40 max-step events | 100% of logged failed attempts | `/tmp/ab_off.log`: 20 events across 8 unique tasks. `/tmp/ab_on.log`: 20 events across 7 unique tasks. |
| Tool/backend instability | 17 warning events | Secondary | Ollama connection retries and parse failures contributed to long attempts, especially in `ab_off`. |
| Bad edit-match | 0 events | 0% | No benchmark task in these two null logs required editing. |
| Policy fallback/denial | 0 events | 0% | No policy denial appeared in the sampled failures. |
| Judge too strict | Not supported | Unknown | `p_correct=0.000` is explained by missing final answers; no evidence here that the evaluator rejected good final answers. |
| Ran out of context | Not directly supported | Unknown | The visible symptom is repeated max-step behavior, not explicit context overflow. |

## Failed Task Clusters

`ab_off` max-step clusters:

- 3x list `.rs` files and count by subdirectory.
- 3x read `Cargo.toml`, grep dependency usage, report missing deps.
- 3x search semantic and episodic memory for `MARS reflection`.
- 3x read `config/hardware.toml`, fetch Ollama tags, verify primary model.
- 3x compare `free -h` and `/proc/meminfo`.
- 2x timestamp write/sleep/read comparison.
- 2x compare `df -h /` and `df -k /`.
- 1x compare `uname -a` and `/proc/version`.

`ab_on` max-step clusters:

- 3x read `Cargo.toml`, grep dependency usage, report missing deps.
- 3x timestamp write/sleep/read comparison.
- 3x compare `df -h /` and `df -k /`.
- 3x read `config/hardware.toml`, fetch Ollama tags, verify primary model.
- 3x list `src/evolved/*.rs` and count lines.
- 3x compare `free -h` and `/proc/meminfo`.
- 2x list `.rs` files and count by subdirectory.

## Decision

The plan premise that bad edit matching should gate the next step is false for
this dataset. Per `NEXT_STEPS.md` rule 7, the execution order should shift:

- Do answer-gated termination first.
- Then implement loop/repeated-failure detection and earlier strategy change.
- Re-measure.
- Resume hash-anchored edits after the read/shell loop can produce answer-bearing
  completions.

This does not remove the edit stack from the roadmap; it prevents us from
optimizing an unobserved failure class before fixing the measured `p_correct=0`
cause.
