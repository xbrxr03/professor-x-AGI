# Benchmarks

This directory contains generated benchmark figures and metric tables for the paper and README.

## Directory Structure

```
benchmarks/
├── hiro/              # HIRO round-level results
│   ├── rounds/        # Per-round BF/HIRO summaries
│   ├── attempts/      # Per-task attempt-level results
│   ├── null-baselines/# Static-harness null conditions
│   └── regression/    # Fast regression subsets
├── dhe/               # DHE attribution distribution plots
├── lcap/              # LCAP arm selection frequency over rounds
├── ics/               # Identity coherence score over time
└── bf/                # Behavior factor trajectories
```

## How to Generate

```bash
# Run HIRO benchmark (single round)
cargo run -- --hiro 0 --hiro-limit 1

# Run null baseline (3 rounds, static harness)
PROFESSOR_X_DATA_DIR="$PWD/.px-data-null" cargo run -- --hiro-null 3

# Evolution smoke test
PROFESSOR_X_DATA_DIR=/tmp/px-evolution-smoke cargo run -- --evolution-smoke
```

## Citation

If you use HIRO or IPE-MHE results in your work, please cite:

```bibtex
@article{habib2026ipemhe,
  title={Identity-Preserving Metacognitive Harness Evolution: A Self-Evolving Agent That Knows Itself},
  author={Habib, Abrar},
  journal={arXiv preprint},
  year={2026}
}
```