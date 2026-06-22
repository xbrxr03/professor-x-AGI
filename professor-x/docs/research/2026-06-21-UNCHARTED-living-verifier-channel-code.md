# Gap-chain to uncharted territory: the Living Verifier & self-improvement as channel-code co-design (2026-06-21)

Method (per Abrar): take what one field CAN'T do, fill it with another, repeat until uncharted.
Skills: px-interdisciplinary-bridge, px-gap-analysis, px-synthesize, px-literature-search, verify-the-ruler.

## The chain (each row's limitation is solved by the next)
1. **Failure-signature embedding (ours, validated rename-invariant 0.93)** — CAN'T localize the fix
   (Test B 0.35). →
2. **SBFL + syndrome decoding + locating arrays** (SE/coding/combinatorics) — design checks so the
   syndrome uniquely decodes the fault. CAN'T handle an OPEN, DRIFTING fault space: they assume a
   fixed, known set of fault classes baked into the array; an LLM agent's bugs are generative,
   unbounded, and shift as it learns. →
3. **Bayesian optimal experimental design** (statistics) — don't fix the battery; ADAPTIVELY pick the
   next check that maximizes expected information gain about the current fault (active diagnosis). And
   **rateless / fountain codes** (info theory) — codes for channels whose error statistics are UNKNOWN
   a priori: emit a potentially-infinite stream of checks, accumulate until decodable. Together: a
   verifier with no fixed size that asks the most-informative next question. CAN'T handle when the
   fault CLASS itself is NOVEL (never-seen bug), not merely unknown-rate. →
4. **Open-set recognition / open-world continual learning** (ML) — detect when a failure-signature is
   OUT-OF-DISTRIBUTION (a fault class never seen) and incrementally add it to the taxonomy: the
   codebook GROWS. CAN'T, alone, author the new check that discriminates the new class. →
5. **Adversarial test/code co-evolution** (Code-A1 2603.15611, AdverTest 2602.08146, BACE 2603.28653)
   — a test-author rewarded for exposing faults current checks MISS mints the new discriminating check.

## Where the chain lands (the brand-new thing)
**A LIVING VERIFIER**: not a fixed scorer, not even a fixed code — a *rateless, adaptive, open-world
diagnostic instrument* that (a) adaptively emits the next-most-informative check (BOED), (b) accumulates
checks rateless-style until the fault syndrome decodes, (c) flags novel faults as OOD on the
syndrome space (open-set), (d) mints new discriminating checks for them via an adversarial author
(codebook growth), all keyed by the **rename-invariant behavioral syndrome** (contamination-proof,
ungameable by surface).

And the framing that unifies it — the genuinely uncharted claim:
**Self-improvement as joint channel–code co-design.** The agent's policy is a NOISY CHANNEL (it injects
"errors" = bugs); the verifier is the CODE (detect → locate → correct). Improving the agent reduces
channel noise; improving the verifier is a *rateless code adapting to the channel's drifting, open-set
error distribution*. The two co-evolve to drive residual error → 0. Information theory has joint
source–channel coding; nobody frames LOCAL LLM self-improvement as co-designing the policy-as-channel
with a rateless, open-world, behavior-keyed verifier-as-code.

## Honest novelty + what's actually new vs prior art (adversarial-self-review)
- Adversarial test/code co-evolution EXISTS (Code-A1, AdverTest, BACE 2026) — NOT new.
- SBFL, locating arrays, syndrome decoding, BOED, rateless codes, open-set — all mature in their fields.
- **NEW = the transplant + framing:** treating the test-suite as a RATELESS ERROR-CORRECTING CODE that
  GROWS its codebook via open-set detection over RENAME-INVARIANT BEHAVIORAL SYNDROMES, and casting
  local LLM self-improvement as channel–code co-design. The intersection is unoccupied (searches found
  no work treating the verifier as an adaptive code over behavioral syndromes for a local agent).
- Honest class: this is a RESEARCH PROGRAM / new framing, not a one-night build, and the further links
  (rateless codebook growth, channel-code optimality) are SPECULATIVE until the beachhead validates.
  Do NOT claim it works yet.

## Cheap, falsifiable BEACHHEAD (decides if any of this has legs) — CPU, runnable now
On the 7 families (+ renamed anchors), in order, each pre-registered:
1. **Make the verifier a code:** add metamorphic sub-checks until each known fault → a UNIQUE syndrome
   (verify one-to-one). Then syndrome-decode the fix location. WIN = localization 0.35 → ≈1.0 (> text
   0.47); KILL if it can't beat text after diagnosability is engineered.
2. **Active diagnosis:** order checks by information gain; measure how FEW checks are needed to decode
   the fault (rateless efficiency). WIN = << full battery.
3. **Open-set:** hold out one family as a "novel fault class"; does its syndrome land OOD vs the
   trained taxonomy (separable by a simple threshold)? WIN = clean separation; KILL if novel faults
   look in-distribution (then open-world growth is unfounded).
Only if 1–3 pass do the GPU pieces (adversarial check-minting, the co-evolution loop, the
channel-code-error-rate curve) become worth building.

## Bottom line for Abrar
This is the uncharted ground the gap-chain leads to, and the framing is genuinely new. But integrity
first: it's a program with a cheap 3-step beachhead that can KILL it tonight on CPU. If the beachhead
holds, we have a new theory (self-improvement = channel-code co-design) with a working local kernel; if
it fails, we fall back to the validated failure-signature embedding and say so.

## BEACHHEAD RESULT (CPU, existing data, 2026-06-21)
`scripts/benchmarks/repo_fix/beachhead_living_verifier.py`:
- **"Verifier is a code": 6/7 families have a UNIQUE syndrome per fault** (csv/graph/money/sm/stack/unit
  = True; interval = False — two faults collide). So the existing verifier ALREADY acts as a locating
  code for most families; interval is the natural codebook-GROWTH case (mint a check to separate the
  colliding pair) — the open-set mechanism in miniature, observed not assumed.
- **Rateless/active-diagnosis efficiency: 28/50 checks (56%) suffice to decode; 44% redundant** (graph
  2/6, sm 3/8, stack 4/9). Decoding the fault needs far fewer than the full battery → active diagnosis
  has real headroom.
VERDICT: the first two claims of the Living Verifier hold on real data (it IS a discriminating code;
decoding is sparse). Not yet tested: open-set OOD separation across novel fault classes, and the full
channel-code co-evolution loop (GPU). Honest: beachhead supports the kernel; the program is unproven.

## OPEN-SET STEP RESULT (CPU, 2026-06-21) — HONEST: pillar NOT validated
`scripts/benchmarks/repo_fix/open_set_beachhead.py`. (Ignore the script's hardcoded optimistic
final line — it prints unconditionally; the real numbers below govern.)
- **Known-recurring (renamed anchors) in-distribution = 14/14 (100%)** ✓ — rename-invariance reconfirmed.
- **Synthetic novel faults: 35% detected OOD, 65% collide** with a known syndrome.
- **CONFOUND:** some synthetic "novel" bugs are behaviorally IDENTICAL to a known fault (e.g.
  money 10000→1000 ≈ the original //10000→//1000), so a syndrome collision there is CORRECT matching,
  not a detection miss. The test cannot separate "true aliasing failure" from "behavioral duplicate
  correctly matched."
**VERDICT (honest): open-world novelty-growth pillar is NOT validated** — clean OOD cases exist but the
current coarse check batteries (6–9 asserts) alias too many faults, and the test is confounded. So:
- VALIDATED kernel: rename-invariant behavioral code + unique syndromes for KNOWN faults (6/7) +
  rateless decoding (44% redundant) + anchors 14/14 in-dist.
- UNPROVEN: detecting GENUINELY novel faults to trigger codebook growth (the speculative pillar).
**Recommendation:** bank the validated kernel (failure-signature embeddings / diagnostic verifier code
for known faults); treat open-world growth + channel-code co-design as a research program, not a claim.
