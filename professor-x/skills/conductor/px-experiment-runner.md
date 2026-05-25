# px-experiment-runner

## Purpose
Run local experiments that produce reproducible evidence for Professor X hypotheses.

## Inputs
- `brain/hypotheses.md`
- `professor-x/hiro/tasks.json`
- `professor-x/artifacts/hiro/`
- `professor-x/artifacts/evolution/`

## Workflow
1. Select one untested or weakly tested hypothesis with a local-only experiment.
2. Define the command, data directory, expected artifact path, and success criteria before running it.
3. Prefer fast checks first: `cargo check`, targeted tests, HIRO subsets, then longer null baselines.
4. Store raw outputs under `professor-x/artifacts/`.
5. Update the hypothesis status only with observed results.

## Output Contract
Write an experiment note containing `hypothesis`, `commands`, `results`, `interpretation`, and `next_test`.
