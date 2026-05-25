# px-gap-analysis

## Purpose
Find the highest-impact gap between the current harness and the autonomous evolution plan.

## Inputs
- `docs/REPO_STRUCTURE.md`
- `professor-x/ops/runbooks/`
- `brain/hypotheses.md`
- `brain/knowledge-base.md`
- Recent `git diff --stat`

## Workflow
1. Compare current repo state against the next unblocked phase in the plan.
2. Identify missing safety, measurement, skill, or evolution-gate pieces.
3. Rank gaps by whether they block autonomous run, measurement trust, or rollback.
4. Recommend one concrete next implementation target.

## Output Contract
Return `top_gap`, `why_it_blocks`, `evidence`, and `recommended_patch`.
