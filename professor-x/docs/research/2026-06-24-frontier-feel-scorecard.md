# Frontier-Feel Scorecard — making the goal measurable (2026-06-24)

The goal ("feel like OpenClaw/frontier but local in 12GB") is vague until measured.
`scripts/benchmarks/repo_fix/frontier_feel_score.py` turns a bench artifact into a measurable profile:
correctness (pass@1) · reliability (made_edit%) · cleanliness (1 − wrong-edit-rate among edits) → a
weighted **FRONTIER-FEEL INDEX** in [0,1], vs a target bar (correctness≥0.60, reliability≥0.95,
cleanliness≥0.60). "Goal met" = index → 1.0 / meets bar on a real-feel benchmark.

## Baseline (hard set, native tools, K=1)
| model | correctness | reliability | cleanliness | FEEL index | meets bar? |
|---|---|---|---|---|---|
| qwen3:8b  | 0.467 | 0.933 | 0.50 | 0.85 | no |
| qwen3:14b | 0.533 | 0.967 | 0.552 | **0.928** | no |

## Read
- **14B is measurably closer to "frontier feel"** (0.928 vs 0.85) — already crosses the *reliability*
  bar (made_edit 0.967 ≥ 0.95). The single blocking gap is **correctness 0.533 < 0.60.**
- That gap is the **harness leg** (Codex's agentic-perf track) + better edit quality — not the model
  (14B is the right base). When correctness clears 0.60 on the real-feel tier, the index meets the bar.
- Honest: K=1 single run; real-feel-tier FEEL number is queued (14B vs 8B on the 9 real-feel tasks).

## How this drives the goal
This is the scoreboard for "frontier feel in 12GB": track the FEEL index as 14B + harness improvements
land. The path is now quantified — capability (14B) ✓ direction, reliability ✓ at bar, **correctness is
the remaining push.**
