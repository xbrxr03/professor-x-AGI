---
name: jupyter-notebook
description: Use when a research idea, hypothesis, or measurement needs an exploratory notebook. Trigger for ablations, metric prototyping, small experiments, plots, sweeps, or reproducible interactive analysis.
allowed-tools:
  - Bash(*)
  - Read
  - Grep
  - Glob
---

# Jupyter Notebook

## Overview

Use this skill when a theory or measurement idea needs a notebook rather than a
loose script. The notebook is for structured exploration, not for hiding
unreproducible state.

## When To Use

Use this skill when:

- prototyping a metric or measurement
- running a small ablation or sweep
- comparing baselines interactively
- plotting or inspecting experimental outputs
- turning rough experimental notes into a reproducible artifact

## Workflow

1. Lock the experiment question before creating cells.
   Write:
   - hypothesis
   - baseline
   - variables to change
   - metrics to record
   - what result would weaken the idea

2. Make the notebook top-to-bottom runnable.
   Early cells should set imports, paths, seeds, and config.
   Do not rely on hidden prior state.

3. Keep cells small and single-purpose.
   Prefer:
   - setup
   - load data
   - compute metric
   - plot or summarize
   - interpretation

4. Record results near the computation.
   Use short markdown cells or compact dictionaries/tables instead of giant
   outputs.

5. End with an explicit verdict.
   State whether the notebook:
   - supports the idea
   - weakens the idea
   - is inconclusive

## Quality Rules

- A notebook is not evidence unless someone can rerun it.
- Prefer one focused notebook per question rather than one giant lab dump.
- Keep outputs tidy and interpretable.
- If execution is not possible in the current environment, say so explicitly.

## Output Contract

Return or produce:

- `hypothesis`
- `baseline`
- `metrics`
- `notebook_path`
- `result_summary`
- `next_experiment`
