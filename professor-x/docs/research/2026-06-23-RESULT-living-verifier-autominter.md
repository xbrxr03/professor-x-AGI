# RESULT: Living Verifier — automated codebook growth via differential testing (2026-06-23)

Big-swing experiment (Abrar: "go big on the Living Verifier"). The genuinely-uncharted framing:
**self-improvement as joint channel–code co-design** — the agent's policy is a noisy channel (injects
bugs); the verifier is a rateless, open-world error-correcting CODE that grows its codebook to track
the drifting fault distribution. Kernel validated (7/7 families are locating codes after tonight's
codebook-growth). The UNPROVEN pillar = **automated** codebook growth (tonight's interval separators
were HAND-authored). This tests whether minting can be automated. Method: verify-the-ruler,
adversarial-self-review. CPU, no GPU (distillation owns the GPU).

## Frontier scan (new resources; intersection still open)
- Test-suite-as-rateless/fountain-code for LLM agents: **no unified prior work** (unoccupied).
- Open-set fault diagnosis: mature in INDUSTRIAL systems (prototype learning, distance-rejection,
  evidential fusion) — NOT software/program-repair → transplant open + gives methods to fix the
  beachhead's confounded 35% open-set number.
- Active testing / BOED (BayesFLo, GO-CBED/EIG): mature → lift for the which-check-next pillar.
- Self-improvement as channel-code co-design: unoccupied (JSCC only in comms).
=> honest novelty shape holds: mature components transplanted into the rename-invariant behavioral-
syndrome verifier of a LOCAL agent, framed as channel-code co-design — empty intersection.

## The auto-minter (differential testing — family-agnostic, CPU, no hand-specified properties)
When two faults COLLIDE (alias to one syndrome), search random inputs for one whose correct-output
assert yields DISTINCT pass/fail across the colliding buggy impls; that asserted input is a new
codeword that splits the collision. `scripts/benchmarks/repo_fix/living_verifier_autominter.py`.

## Result (the real interval collision: bug04 sign-flip vs bug05 no-merge, both -> 1111100)
- **Collision auto-resolved: 10/10 seeds.**
- Mints **1 check**, found in **mean 3 random probes (min 1)** — rateless/cheap.
- Blindly rediscovered separators equivalent to the hand-authored ones, e.g.
  `assert covered([(1,3)]) == 2` (disjoint input: sign-flip fails -2, no-merge passes 2).
- **red->green preserved by construction** (asserted value = correct output) — never breaks a valid fix.

## Honest scope (adversarial-self-review — what this does and does NOT show)
- DOES: automate the codebook-growth step — a blind differential search auto-mints the discriminating
  check that previously needed a human, cheaply. The "code grows to track the channel" pillar works.
- Does NOT (yet): (1) generalize — one collision (interval), impls hand-extracted; need synthetic
  collisions across all 7 families + a count of auto-resolve rate. (2) live integration — wire
  detect-collision -> mint -> append into the real verifier loop. (3) novelty: differential/metamorphic
  testing is mature (Code-A1, AdverTest); the new part is the ROLE (auto-minter for the rename-invariant
  syndrome code in the channel-code loop on a local agent) — integration-novel, said plainly.
- Open-set (detect a GENUINELY-novel fault, not just a known collision) is still the harder pillar;
  the industrial prototype-learning / distance-rejection methods are the principled next attack.

## Next (CPU first)
1. Generalize the auto-minter: synthesize collisions across all families, measure auto-resolve rate +
   probes-per-resolve (rateless efficiency curve). KILL if differential testing can't separate most.
2. Clean open-set test (fix the beachhead 35% confound) with prototype-learning distance-rejection.
3. Live loop: detect-collision -> auto-mint -> append-check, in the verifier (Rust/harness).
4. Then the channel-code co-design loop: agent improves (channel noise down) while verifier grows
   (codebook tracks drift) -> measure residual decode-error -> 0.
