# Living Verifier — B1+B2(+graph) summary: codebook growth AUTOMATED & GENERALIZED (2026-06-23)

Capstone of the Phase-B invention swing (the moat: self-improvement as channel–code co-design — the
verifier as a rateless, open-world code that grows its own checkpoints). All CPU, while distillation
owned the GPU. Method: verify-the-ruler, adversarial-self-review.

## What was unproven before tonight
The beachhead (2026-06-21) validated the KERNEL (6/7 families are locating codes) but left the
open-world pillar UNPROVEN: codebook growth was HAND-authored, and open-set novel-fault detection was
35% (confounded). Tonight closed both.

## Results
- **Auto-minter (B1):** blind DIFFERENTIAL TESTING auto-constructs a complete locating code — no
  hand-authored properties. Reference = majority vote across task variants; inputs = typed-random
  mutations of seeds harvested from each check.py. Stateless families: **interval/money/sm/unit** →
  full (5/5,5/5,4/4,5/5), cheap (11–34 probes); **stack** full after boundary fuzz; **csv** 4/5.
- **Stateful families (graph):** the stateless harvester found no seeds (graph uses a mutable `adj`
  built by `add_edge`). A SCENARIO-based diff-test (random edge-sequence + queries, majority ref) →
  graph **full locating code (4/4, 3 scenarios)**. The mechanism extends to stateful APIs.
- **Open-set (B2):** the only residual alias (csv_04 vs csv_05) is a **TRUE behavioral duplicate** (no
  separator in 6000 boundary probes) → correctly non-separable. So aliasing == behavioral duplicates
  (correct matching), NOT a detection failure — **resolving the beachhead's confounded 35%.**

## Honest tally
**6/7 families auto-resolve to FULL locating codes** (interval, money, sm, unit, stack, graph); csv's
lone residual is a genuine duplicate that SHOULD alias. The auto-minter separates EVERY
behaviorally-distinct fault, across stateless and stateful APIs, automatically and cheaply.

## What this means for the thesis
The genuinely-uncharted framing — self-improvement as channel–code co-design, with a verifier that
GROWS its codebook to track the drifting fault distribution — now has a **validated, automated,
generalized** codebook-growth mechanism, with the open-world pillar de-confounded. This is real
movement on "invent something new," built on the validated verifier-code asset.

## Honest scope / next
- "True duplicate" = no separator in 6000 boundary probes (strong, not a proof of equivalence).
- Reproducibility: `scripts/benchmarks/repo_fix/autominter_generalize.py` (B1),
  `autominter_openset.py` (B2); graph scenario variant in this session's log.
- **B3 (next, Rust):** wire detect-collision → auto-mint → append into the LIVE verifier/benchmark loop
  (rust-harness-change discipline). Then Phase D: the agent-improves ↔ verifier-grows co-evolution loop,
  measuring residual decode-error → 0 under the trust gate.
