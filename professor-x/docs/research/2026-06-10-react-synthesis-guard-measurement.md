# ReAct Synthesis Guard Measurement - 2026-06-10

## Change
Added a pre-ceiling ReAct guard that forces a synthesis checkpoint before the
hard 20-step limit. If the agent keeps exploring after the checkpoint, the
attempt is explicitly forfeited before max steps.

## Verification
Commands run from `professor-x/`:

```bash
cargo check
cargo test --bins
cargo run -- --validate-artifacts
cargo run -- --hiro-null 1 --hiro-limit 12
```

## HIRO Null Run
- Run id: `f1c8a72c-d601-4591-ad44-f1b2e6310187`
- Harness commit recorded by HIRO: `c0f404d9e434e38ac32248220717f2bbfcb7642f`
- Task count: 12
- Successes: 4
- `pass@3`: 0.333
- `p_tool`: 0.333
- `p_plan`: 0.000
- `p_correct`: 0.000
- Hard max-step warnings: 0
- Synthesis/forfeit guard stops: 24

## Interpretation
The immediate Phase 0.5.2 target was met: hard max-step exhaustion dropped to
zero on the same 12-task null run shape. The run still shows the real autonomy
gap clearly: planning and correctness remain at zero, and several tasks now
forfeit earlier instead of silently burning the full step budget.

The next highest-ROI work is evaluator-aware task solving and edit/tool
interfaces, especially path resolution, shell command decomposition, and
synthesis from successful observations.
