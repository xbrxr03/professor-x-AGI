# px-self-review

## Purpose
Classify daily task outcomes and decide whether knowledge, experiments, or harness evolution actually improved.

## Inputs
- `professor-x/ops/daily/`
- `professor-x/artifacts/hiro/`
- `professor-x/artifacts/evolution/`
- `brain/hypotheses.md`
- `brain/dead-ends.md`

## Workflow
1. Review the latest daily update and artifacts.
2. Classify outcomes as `knowledge_gain`, `experiment_result`, `harness_change`, `blocked`, or `idle`.
3. Score the day from 1 to 10 using evidence, not effort.
4. Move failed hypotheses to `brain/dead-ends.md` only when evidence is sufficient.
5. If five consecutive cycles are idle, recommend clean self-termination.

## Output Contract
Write `score`, `outcome_class`, `evidence`, `retire_or_continue`, and `next_cycle_target`.
