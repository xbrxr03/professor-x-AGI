# px-daily-update

## Purpose
Produce a compact daily state update from local repo, brain, HIRO, and evolution records.

## Inputs
- `brain/knowledge-base.md`
- `brain/hypotheses.md`
- `brain/dead-ends.md`
- `professor-x/artifacts/hiro/`
- `professor-x/artifacts/evolution/`
- `git status --short`

## Workflow
1. Inspect local project state and recent changed files.
2. Summarize what changed, what evidence was created, and what remains blocked.
3. Record a dated update in `professor-x/ops/daily/YYYY-MM-DD.md`.
4. If no new evidence exists, write that explicitly and do not invent progress.
5. Do not use network unless the scheduled job explicitly allows it.

## Output Contract
Write a Markdown update with sections: `Status`, `Evidence`, `Risks`, `Next`.
