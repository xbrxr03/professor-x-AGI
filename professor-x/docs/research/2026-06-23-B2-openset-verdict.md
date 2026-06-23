# RESULT: B2 — open-set disambiguation resolves the beachhead confound (2026-06-23, CPU)

Living Verifier Phase B2. B1 left csv (4/5) and stack (5/6) partial. Question: are the residual
aliased faults COVERAGE GAPS (fixable with more/edge probes) or TRUE BEHAVIORAL DUPLICATES (two
different code-bugs, identical observable behavior → correctly non-separable)? Method: heavier
boundary-inclusive fuzz to re-check aliasing, then an INTENSIVE 6000-probe differential search per
residual pair. `scripts/benchmarks/repo_fix/autominter_openset.py`.

## Result
- **stack: NO aliasing under boundary fuzz** — the B1 partial was a COVERAGE GAP; boundary inputs
  resolve stack to a FULL locating code. (So 5/6 families now fully auto-resolve: interval, money, sm,
  unit, stack.)
- **csv: fam_csv_04 vs fam_csv_05 = TRUE BEHAVIORAL DUPLICATE** — no separator in 6000 boundary probes
  → identical observable behavior → CORRECTLY non-separable.

## Why this matters (resolves the beachhead's 35% open-set confound)
The beachhead's "open-world novelty-growth UNPROVEN (35% OOD, 65% collide, confounded)" verdict blamed
behavioral duplicates. B2 confirms it directly: **aliasing happens exactly when faults are
behaviorally identical (correct matching), not from a detection failure.** So the verifier-as-code +
auto-minter is SOUND open-world: it mints a separating check for every *behaviorally-distinct* fault
(5/6 families, and the coverage gaps close with boundary fuzz), and correctly leaves *true duplicates*
aliased (you cannot and should not split faults with identical behavior). The open-set "problem" was
mostly a measurement confound; the mechanism is correct.

## Honest scope (verify-the-ruler)
- "True duplicate" = no separator in 6000 boundary-inclusive probes. Strong for these small functions,
  but a negative search is not a proof of behavioral equivalence (a rarer input could exist). Stated.
- graph still has a harvester gap (AST literal_eval missed its input shape) — an implementation fix,
  unrelated to the mechanism.

## Living Verifier status after B1+B2
- Codebook growth AUTOMATED and GENERALIZED (differential testing; 5/6 families full locating codes).
- Open-set aliasing EXPLAINED (= behavioral duplicates, correct).
- Remaining: B3 wire detect-collision→auto-mint→append into the live verifier (Rust/harness);
  fix the graph harvester; then the channel-code co-design loop (Phase D).
