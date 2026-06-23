# RESULT: B1 — auto-minter generalizes across families (2026-06-23, CPU)

Living Verifier Phase B1. Tests whether the differential-testing auto-minter (interval, 10/10) is a
GENERAL mechanism: can blind differential testing auto-construct a complete locating code (unique
syndrome per fault) for every family? Family-agnostic method: harvest seed inputs + func->module map
from each check.py (AST), typed-random mutation of seeds, MAJORITY-vote reference across task variants
(each task bugs one module → majority is correct; no hand-written correct impl), greedily keep checks
that increase distinct syndromes. `scripts/benchmarks/repo_fix/autominter_generalize.py`.

## Result
| family | faults | checks minted | probes | unique | full locating code? |
|---|---|---|---|---|---|
| interval | 5 | 4 | 34 | 5 | **YES** |
| money | 5 | 4 | 11 | 5 | **YES** |
| sm | 4 | 2 | 12 | 4 | **YES** |
| unit | 5 | 4 | 18 | 5 | **YES** |
| csv | 5 | 3 | 400 | 4 | partial 4/5 |
| stack | 6 | 4 | 400 | 5 | partial 5/6 |
| graph | — | — | — | — | skip (harvester got no seeds) |

**4/6 families auto-resolved to a FULL locating code**, cheaply (11–34 probes for the wins).

## Honest verdict (verify-the-ruler + adversarial-self-review)
- **The mechanism GENERALIZES** — differential testing auto-mints a complete locating code for most
  families with no hand-authored properties, confirming the interval result was not a one-off.
- **The 2 partials are diagnostic, not necessarily failures:** csv (4/5) and stack (5/6) each leave one
  fault pair aliased after 400 probes. Either (a) the typed-random mutation didn't reach a
  distinguishing input (coverage gap — smarter/typed fuzzing would close it), OR (b) the two faults are
  BEHAVIORALLY IDENTICAL (genuine aliases — same observable behavior, different code), in which case
  non-separation is CORRECT (you can't/shouldn't split behaviorally-equal faults). This is the same
  behavioral-duplicate confound the open-set beachhead hit — **must disambiguate (B2)**.
- **graph**: harvester gap (AST literal_eval missed its input shape) — an implementation fix, not a
  mechanism failure.
- **Assumption**: majority-vote reference holds because each task bugs one module; named, not hidden.

## Next
- **B2 (open-set):** disambiguate the csv/stack partials — are the residual pairs coverage gaps or true
  behavioral duplicates? Use prototype-learning distance-rejection (industrial open-set methods) on the
  syndrome space to detect genuinely-novel faults vs aliases. Fix the graph harvester.
- Then **B3**: wire detect-collision → auto-mint → append into the live verifier.
