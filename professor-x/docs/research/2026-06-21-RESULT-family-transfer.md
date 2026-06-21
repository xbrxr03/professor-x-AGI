# RESULT: reuse-family transfer measurement (2026-06-21)

Reproducible: `python3 scripts/benchmarks/repo_fix/measure_transfer.py`. Applied skills:
verify-the-ruler (caught + fixed a confounded sub-metric), adversarial-self-review.

## What "transfer" means here and why it matters
The keystone benchmark gap was that every old task was its own island (no shared solution
surface), so a fix/retrieval/credit-signal learned on one task could not transfer to another —
which blocks VGTS (embedding), Re-Verified RAG, and VCA (credit assignment). The 7 reuse-families
inject bugs into a SHARED library, so siblings share the code a correct fix must reason over.
This measures whether that actually holds.

## Method
Each family task bugs exactly one module (`buggy_module`), so the CORRECT content of every module
is reconstructable from a sibling whose bug is elsewhere — no external files needed. Per task we
compute: whole-context token set, shared-API-line token set (full-library lines that use a
shared_api symbol), and the reference patch token set (buggy-vs-correct line diff). Overlap =
mean pairwise Jaccard over sibling pairs; cross-family pairs are the control; the old hard-set
(hard_*) is the "before" anchor.

## Results
| metric | within-family | cross-family (control) | separation |
|---|---|---|---|
| context overlap | **0.979** | 0.163 | 6.0× |
| shared-API-line overlap | **0.968** | 0.122 | **7.9×** |
| patch-line overlap | 0.11–0.44 (varies) | — | (intentionally low) |

Before/after (context overlap, same ruler): **old hard-set 0.111 → families 0.979 (9×).**

Per-family api-line overlap: csv 1.00, sm 1.00, stack 1.00, unit 1.00, graph 0.97, money 0.92,
interval 0.89. All ≥ the recipe gate (≥0.40), all far above the 0.122 cross-family control.

## Honest notes (verify-the-ruler)
- **A first version of the api-line metric was confounded** and discarded: measuring tokens on the
  sparse *patch* lines let generic Python tokens (`return`, `self`, `for`) dominate → within 0.403
  vs cross 0.427 (0.9×, NOT discriminating). Fixed by measuring shared-API lines over the full
  library body → 0.968 vs 0.122 (7.9×). Reported the fix, not the broken number.
- **The "0.1%" in earlier notes was a looser/different ruler.** Measured consistently as
  context token-Jaccard, the old set is **0.111**, not 0.001. The corrected, apples-to-apples
  before/after is 0.111 → 0.979 (9×). Recording the real number.
- **Low patch-line overlap is a feature, not a miss.** Bugs sit at varied locations, so exact
  fixes differ (0.11–0.44). This is what prevents "transfer = memorize the same patch"; the shared
  *context* (0.98) is what transfers, the exact edit does not. The sealed/renamed-anchor split
  (recipe Step 6) remains the complementary guard against operator-matching.

## Verdict
Transfer property **CONFIRMED**: siblings share ~98% of their solution surface (vs 16% across
families and 11% on the old set), while the exact fix varies. The families are real stepping-stones
— the structure VGTS/RAG/VCA require now exists in the benchmark.

## Next (the other half of the gate): ZPD filter
pass@k (qwen3:8b-q4_K_M, native tools, K=3) is running per family (log /tmp/family_passk.log).
ZPD keep-band: 0 < pass@1 < 1 (in-band = real headroom + measurable MDE). Families that come back
all-0 (too hard) or all-1 (too easy) get re-tuned. Result appended when the run finishes.
