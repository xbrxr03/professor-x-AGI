# RESULT: DVC codebook-growth — the verifier becomes a complete locating code (2026-06-23)

Overnight CPU experiment (no GPU; the TGC train-half owned the GPU). The redirected invention-hunt
thread from 2026-06-22 (verifier-as-discriminating-code — the one direction the frontier scan kept
*reinforcing* rather than taking; motivated by OpenAI's Feb-2026 finding that 59.4% of SWE-bench-
Verified hard tasks have tests that pass even with the bug unfixed). Applied: verify-the-ruler,
adversarial-self-review. All work on COPIES — the canonical fixtures were untouched (a gate was
benching them).

## The collision (the one family that wasn't a locating code)
Beachhead (2026-06-21): 6/7 families had unique per-fault syndromes; `interval` was the lone
collision. Pinned it here: with the 7-assert check, **fam_interval_04 and fam_interval_05 both produce
syndrome `1111100`** — both inject a *different* bug into `schedule.py::covered`, but both fail only
the two `covered` asserts and pass the rest:
- correct: `covered(ivs) = sum(e - s for s,e in merge_all(ivs))`
- **interval_04:** `sum(s - e …)` — sign flipped → returns the negative of correct
- **interval_05:** `sum(e - s for s,e in ivs)` — dropped `merge_all` → double-counts overlaps
They collide only because the checks test *equality to the right answer* (both merely "fail"),
discarding the distinguishing behavior — exactly the OpenAI "non-discriminating tests" failure mode.

## The fix: mint two METAMORPHIC sub-checks (codebook growth)
Added two relations that hold for ALL correct inputs (so they are rename-invariant and not overfit to
these specific bugs), each catching one fault class:
```python
assert covered([(1,5),(2,3)]) >= 0                 # non-negativity  -> catches the sign flip (04)
assert covered([(1,5),(2,3)]) == covered([(1,5)])  # overlap-idempotence -> catches the missing merge (05)
```

## Result (deterministic, on copies)
| task | bug | 7-bit syndrome (before) | 9-bit syndrome (after) |
|---|---|---|---|
| fam_interval_01 | intervals.py | 1011111 | 101111111 |
| fam_interval_02 | intervals.py | 1111001 | 111100110 |
| fam_interval_03 | intervals.py | 1100000 | 110000010 |
| **fam_interval_04** | schedule.py (sign) | **1111100** | **111110001** |
| **fam_interval_05** | schedule.py (no-merge) | **1111100** | **111110010** |

- **Unique syndromes: 5/5** (was 4/5). 04 and 05 now separate on the two new bits → **interval is a
  locating code**. With the beachhead's other 6/7, **all 7 families are now discriminating codes**.
- **red→green preserved:** the augmented check on the CORRECT `covered` passes all 9 (syndrome
  `111111111`, exit 0) — the new checks don't break valid solutions, they only add discrimination.

## Honest scope (adversarial-self-review — what this does and does NOT show)
- DOES show: the Living-Verifier "codebook growth" step is real and constructible on real data — a
  collision (novel-to-the-code fault pair) is separable by minting metamorphic relations, and the
  result is a complete locating code with red→green intact. This concretely advances the previously
  **UNPROVEN open-world pillar** for this case.
- Does NOT show (yet): (1) **automation** — I authored the two checks by hand after reading the two
  bugs; the Living Verifier's claim is an *adversarial auto-minter*. (2) **payoff** — that the now-
  complete locating code lifts fix-LOCALIZATION (precheck1 Test B was 0.35 sig vs 0.47 text with naive
  NN) or pass@1. Diagnosability is necessary, not yet shown sufficient. So: kernel extended, value
  still pending.

## Next (highest-ROI, CPU first)
1. Engineer metamorphic discrimination across all families (most are already unique) and re-run
   precheck1 Test B — does syndrome-NN fix-localization now BEAT text once the suite is a code? (the
   test that previously failed at 0.35 < 0.47; KILL the DVC value claim if it still loses.)
2. Only if (1) wins: GPU payoff — behavior-keyed RAG using the locating-code syndrome → does it lift
   pass@1 on held-out renamed anchors.
3. Auto-minting (the adversarial check-author) is the harder research step; defer until (1)+(2) show
   the value is real.

Nothing merged into the canonical fixtures — these metamorphic checks are PROPOSED additions (would
change the ruler), to apply only via review and the add-repofix-fixture red→green discipline, never
while a gate is running.
