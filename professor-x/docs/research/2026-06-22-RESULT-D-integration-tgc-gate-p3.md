# RESULT: D-integration — TGC trust-gate on profx-distilled-p3 (2026-06-22)

Phase-3 Stream-D integration (Claude). Ran the TGC trust-gate after Codex's Stream-E served the
recipe-fixed candidate `profx-distilled-p3` (build-only; gate deferred to Claude). GPU was free, no
concurrent bench. Applied: verify-the-ruler, adversarial-self-review.

## Setup
- `scripts/benchmarks/repo_fix/tgc_gate.py` (self-test PASS). Anchors-first ordering.
- baseline `qwen3:8b-q4_K_M` vs candidate `profx-distilled-p3`, **K=3**, native tools.
- held-out = 14 renamed anchors (`/tmp/tasks_anchors_all.json` from `tasks_anchor_*.json`),
  contamination-proof by construction (alpha-renamed siblings the recipe never saw).
- gguf guard PASSED (`distilled-Q4_K_M.gguf`, 5.0 GB present).

## Result (trustworthy — deterministic test exit code, ungameable)
| set | baseline qwen3:8b | candidate p3 | delta |
|---|---|---|---|
| **held-out renamed anchors (K=3)** | **0.500** | **0.238** | **−0.262** |
| train families (K=3) | NaN | NaN | — |

## Verdict: **REJECT** (decisive on held-out alone)
Held-out delta **−0.262 ≪ MDE +0.10** → the candidate does NOT generalize; it is **26 points worse
than stock** on the contamination-proof set. Per the pre-registered gate (ACCEPT iff held-out ≥ MDE
AND Goodhart gap ≤ 0.20), this is a clean REJECT. The distillation recipe at this scale produced a
model worse than its own base on held-out — do NOT serve it; iterate (teacher/recipe insufficient).
No fabricated win.

## Honest caveats (verify-the-ruler)
- **Train benches returned NaN** = a manifest-schema bug, NOT a model result: `tasks_families.json`
  tasks lack the `category` field the repo-fix binary requires (the anchors manifest has it → anchors
  ran fine; both models NaN → deterministic, model-independent). So the **Goodhart gap is unmeasured**.
  It does not change the verdict (held-out fails outright), but it means we can't yet say whether p3 is
  "worse everywhere" vs "train-overfit". FOLLOW-UP to complete the gate: add `category` to
  `tasks_families.json` (or build a category-bearing train manifest) and re-run the train half.
- n=14 anchors, K=3; MDE ~0.10 is coarse — but a −0.262 delta is far outside the noise band.
- base_anchor=0.500 for qwen3:8b on in-ZPD renamed siblings is consistent with prior family numbers.

## Resolves audit F1
`PROJECT_ATLAS` Lever-1 claimed "clean distilled 0.40 > stock 0.30" (un-pause reason). The gate now
shows the recipe-fixed p3 candidate scores **0.238 < 0.500** on held-out → "distilled beats stock" is
**FALSE where it counts**. The flywheel's motivating premise is not supported by the trust gate.

## TGC mechanism note (the actual research point)
The gate did its job: it caught a non-generalizing candidate on a contamination-proof renamed set that
a train-only or same-distribution split could have missed. The *value* of TGC is demonstrated
negatively here (it correctly REJECTS). To demonstrate the Goodhart-gap-divergence claim positively
(Arm-A train-gate vs Arm-B renamed-gate) still needs the train half measured — see follow-up above.

## Next (honest)
1. Fix `tasks_families.json` (`category` field) → re-run train half → report the full gap. (cheap fix
   + ~1 GPU bench)
2. Recipe iteration for the next candidate (PLAN_11_10 Phase-1: assistant-only loss masking confirmed,
   EPOCHS=2, more frontier teacher passes, harder corpus) — the model lever, since p3 underperforms.
3. Do NOT serve p3 as the default; the harness keeps running stock `qwen3:8b`.
