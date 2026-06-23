# Codex Restart Handoff

Date: 2026-06-23

## Why this note exists

This note preserves the state of the current Codex research session so a fresh
Codex instance does not need chat memory to recover the thread.

## Current research direction

The current target is not "another agent" or a generic orchestration layer.

The strongest surviving theory candidates explored in this session are:

- `CLT`: Counterfactual Liquidity Theory
- `CGW`: Collateralized Global Workspace
- `DCT`: Diachronic Constraint Theory

Current view:

- `DCT` looks like the deepest candidate so far.
- `CLT` remains useful as a state-level criterion.
- `CGW` remains useful as an engineering / governance layer.

The strongest shift during this session was away from "market metaphor alone"
toward a temporally extended self-constraint theory:

> consciousness-like organization may depend on a slow, history-bearing,
> causally active constraint that binds fast cognition into one temporally
> extended subject.

## Research notes created or extended

- `docs/research/2026-06-23-cognitive-prime-brokerage.md`
- `docs/research/2026-06-23-cognition-consciousness-discipline-map.md`
- `docs/research/2026-06-23-diachronic-constraint-theory.md`

## Repo-tracked shared skill pack

The canonical versions of the cross-agent research skills now belong in this
repo:

- `professor-x/skills/runtime/invention-research/`
- `professor-x/skills/runtime/pdf/`
- `professor-x/skills/runtime/jupyter-notebook/`

These are the versions Professor X should treat as source of truth.

Claude-compatible wrappers also exist in:

- `.claude/skills/invention-research/`
- `.claude/skills/pdf/`
- `.claude/skills/jupyter-notebook/`

For Codex, install or refresh the repo-tracked skills into `~/.codex/skills`
with:

- `python3 professor-x/scripts/install_repo_skills.py --force`

That keeps Codex aligned with the repo copy instead of a drifting home-directory
copy.

## Best next move after restart

After restart, explicitly invoke the custom skill and continue from the current
theory stack rather than reopening broad ideation.

Recommended continuation:

1. Use `$invention-research`.
2. Re-read the three research notes above.
3. Pressure-test `DCT` against the nearest overlaps in synergy, causal
   emergence, active inference, and temporal-self literature.
4. Either kill `DCT` honestly or sharpen it into a stronger surviving claim.

## Short prompt for the next Codex session

Use `$invention-research`. Read:

- `docs/research/2026-06-23-codex-restart-handoff.md`
- `docs/research/2026-06-23-diachronic-constraint-theory.md`
- `docs/research/2026-06-23-cognition-consciousness-discipline-map.md`
- `docs/research/2026-06-23-cognitive-prime-brokerage.md`

Continue the cross-disciplinary invention search. Pressure-test `DCT` first,
then either kill it or refine it.
