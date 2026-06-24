# RESULT: A2 — on-policy p4 distillation REJECT (made_edit% leading indicator) (2026-06-24)

Phase-A on-policy attempt (PLAN_DISTILLATION_2026-06-23). Trained stock qwen3:8b on 78 of its OWN
verified raw-ReAct passes (anchor-free, on-policy), 2 epochs, assistant-only mask. Clean loss descent
(0.41→0.26→0.20→0.18). Served `profx-distilled-p4` (quantize passed the NaN guard; stop-sanity PASS:
done_reason=stop, has_action=True). Method: verify-the-ruler, diagnose-from-trajectory.

## Result (leading indicator = made_edit%, fam16, raw-ReAct = train==serve)
| model | made_edit% | pass@1 |
|---|---|---|
| stock qwen3:8b (native) | ~98% | — |
| p3 (off-policy) raw-ReAct | 56% | 0.250 |
| **p4 (on-policy, 78ex) raw-ReAct** | **19% (3/16)** | **0.062 (1/16)** |

**REJECT** — p4 is WORSE than p3 on both made_edit% and pass@1. The on-policy hypothesis (train on the
model's own tool-driving to preserve adherence) FAILED at this corpus size. Per the pre-registered rule
(a recipe that doesn't lift made_edit% toward 98% is not worth gating), the full TGC gate is not run.

## Honest diagnosis (why on-policy made it WORSE)
- **Tiny corpus overfit.** 78 examples × 2 epochs → the clean 0.18 train loss was a FALSE COMFORT (a
  good fit to 78 trajectories ≠ a good general agent). Held-out made_edit cratered to 19% (13/16
  no-edit) — the model learned the surface of its own 78 passes and generalizes worse on the loop.
- stop-sanity passed (it CAN emit Action + halt at single-generate), so it's not degenerate at that
  level — but in the multi-step agentic loop it drives to an edit only 19% of the time. Small-corpus
  SFT degraded multi-step tool-use.

## The 4-strike pattern (the real conclusion)
turn-1 degenerate → distilled-clean lost (0.133<0.40) → p3 lost (0.238<0.500 held-out) → p4 lost
(19% made_edit). **Four honest distillation failures.** The common factor: at this benchmark scale
(~64–92 tasks → ~78 unique on-policy trajectories) there is NOT ENOUGH DATA for SFT/on-policy
distillation to beat the base — small-corpus fine-tunes overfit/degrade. This is the **corpus-saturation
wall** PLAN_11_10 flagged, now confirmed four times.

## Strategic implication (honest)
The weights lever (distillation) is **not achievable on the current small benchmark.** Two honest paths:
1. **Grow the benchmark first** (more diverse tasks → a corpus of hundreds-to-thousands of trajectories)
   — the prerequisite EVERY prior invention also needed. Then retry on-policy + teacher-frontier mix.
2. **Shelve the weights lever with evidence** and let the moat be what IS working: the **trust gate
   (Collateralized-TGC)** + the **Living Verifier** (verifier-as-code, auto-minter generalized 6/7).
   These are novel, validated, and don't depend on a model that beats stock.
Recommendation: shelve distillation pending a bigger benchmark; the gate REJECTING 4 bad candidates is
itself the trustworthy-self-improvement demonstration. No fabricated win.

## Process notes
- Caught + excluded 65 held-out-anchor trajectories from the corpus (would have invalidated the gate).
- On-disk trainers hardcode curated.jsonl (the PX_DATA version was in a removed worktree) — placed the
  corpus at the expected path; confirmed it trained on the right 78 examples.
