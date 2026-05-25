# px-write-section

## Purpose
Draft or revise paper sections from recorded evidence, not from memory or hype.

## Inputs
- `brain/paper_outline.md`
- `brain/knowledge-base.md`
- `brain/inventions.md`
- `professor-x/artifacts/paper/`

## Workflow
1. Select the smallest useful section from the paper outline.
2. Pull only claims that have citations or experiment ids.
3. Mark missing evidence inline as `TODO(evidence): ...`.
4. Keep mechanism claims separate from result claims.
5. Save drafts under `professor-x/artifacts/paper/runs/` until promoted.

## Output Contract
Produce a Markdown draft with `claim`, `evidence`, `method`, `limitations`, and `open_todos`.
