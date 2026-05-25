# px-synthesize

## Purpose
Turn raw notes, experiment outputs, and literature findings into concise project knowledge.

## Inputs
- `brain/knowledge-base.md`
- `brain/inventions.md`
- `brain/hypotheses.md`
- `professor-x/artifacts/`

## Workflow
1. Collect only new evidence since the last synthesis note.
2. Separate claims into `confirmed`, `weakened`, `unknown`, and `contradicted`.
3. Update confidence only when there is a cited source or local run artifact.
4. Preserve failed results; do not smooth them into positive claims.
5. Write compact synthesis notes suitable for later paper drafting.

## Output Contract
Produce a dated synthesis note under `professor-x/ops/daily/` and update brain files only when warranted.
