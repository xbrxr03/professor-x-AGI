# INVENTION: Diagnostic Verifier Codes (Fault-Syndrome Verification) — 2026-06-21

Emerged from the cross-genre research grind (px-interdisciplinary-bridge + px-synthesize +
px-literature-search). Builds on the one surviving invention (failure-signature embeddings, validated
rename-invariant 0.93) by grounding it in — and repairing it with — six mature fields that already
solved "infer the fault from which checks fail."

## The bridge: six fields, one problem, an empty intersection
Our object: a task/attempt embedded by WHICH deterministic verifier-checks fail ("failure signature").
Pre-check 1 proved this is rename-invariant (0.93 vs text 0.14) but does NOT localize the fix (Test B
0.35 < text 0.47) — because I used naive nearest-neighbor. These fields say WHY, and how to fix it:

| Field | Mature mechanism | What it gives us |
|---|---|---|
| Coding theory | **Syndrome decoding**: one-to-one syndrome→error pattern; precomputed table | the THEORETICAL ideal — a signature that *uniquely decodes* the fault. Test B failed because our checks aren't a discriminating code (many faults → same signature) |
| SW eng (SBFL) | **Tarantula/Ochiai/DStar** suspiciousness from pass/fail spectra | the principled fix-localization Test B did naively |
| SW eng | **DDU / test-suite diagnosability metric** (ICSE'17) — optimize a suite FOR diagnosability, +34% FL accuracy | you can *engineer* checks to maximize fault-information, not just coverage |
| Combinatorics | **Locating / Detecting Arrays** — test suites built so pass/fail data UNIQUELY locates the fault, size scales log in #factors | the construction recipe: design checks so the syndrome is invertible |
| AI | **Model-based diagnosis** (Reiter/de Kleer) — minimal hitting sets of conflicts → diagnoses | decode multi-fault syndromes |
| Immunology | **Negative selection** — self/non-self detectors | framing for the renamed-anchor gate (reject updates matching seen surface) |

Confirmed empty intersection: a search for "design test suite as error-correcting code / fault
signature / LLM agent" returned NO prior work; the closest (FaR-Loc 2509.20552) does RAG fault
localization with SEMANTIC embeddings — surface-dependent, not rename/contamination-invariant.

## The invention (one sentence)
**Co-design the agent's verifier as a discriminating code over the fault space** — a locating-array /
error-correcting check battery — so the failure signature is a SYNDROME that (a) uniquely *decodes* to
the fault, (b) is rename/contamination-invariant, (c) is a behavioral embedding for retrieval (match a
new failing task to the past solved case with the same syndrome → inject its fix), and (d) yields dense
EXACT credit (which check flipped = what the edit fixed) for the distillation/credit loop.

## What is genuinely new (adversarial-self-review, honest)
- The COMPONENTS are mature (SBFL, locating arrays, syndrome decoding, DDU, MBD) — so this is NOT a
  new algorithm in the abstract, and I won't claim it is.
- The NEW PRINCIPLE: flip the verifier from a PASSIVE scorer (pass/fail) into an **invertible
  diagnostic instrument deliberately engineered so failures self-decode** — and use that as the
  rename-invariant BEHAVIORAL EMBEDDING + RETRIEVAL + CREDIT substrate for a LOCAL self-improving LLM
  agent. That exact transplant/role is unoccupied (confirmed by search), and it is squarely enabled by
  our one unique asset: a cheap, decomposable, deterministic verifier we can call thousands of times.
- It also REPAIRS the only failure of pre-check 1 (Test B): naive NN can't localize; locating-array
  construction + syndrome decoding is the field-proven way that should.
- Honest novelty class: **novel transplant + new design principle in an empty intersection** — the
  realistic shape of real novelty, defensible because it is grounded in 6 theories rather than vibes.

## Falsifiable experiment (cheap, CPU, decisive — pre-registered)
Redesign the 7 family check-batteries as **locating arrays**: add enough independent metamorphic
sub-checks that each injected fault class produces a UNIQUE syndrome (verify the syndrome→fault map is
one-to-one on the known bug set). Then:
1. **Localization:** syndrome-decoding fix-location accuracy should jump from 0.35 → ≈1.0 (vs text 0.47).
   KILL if it does not beat text after the suite is made diagnosable.
2. **Rename-invariance preserved:** anchor→origin recovery stays ≈0.93 (the new checks are renamed too).
3. **Retrieval utility (the real payoff, needs GPU):** behavior-keyed RAG using syndrome match raises
   pass@1 over text-RAG / no-RAG on held-out RENAMED anchors. KILL if no lift.
All of 1–2 are CPU-only and runnable immediately; 3 needs the GPU (after the current bench).

## Fit / why this beats the other candidates
Grounded in 6 mature theories (defensible), repairs pre-check 1's only miss, builds on a validated
result (0.93), is contamination-proof by construction, and is the substrate that powers retrieval +
exact credit + (with the renamed-anchor gate) the dual-lever loop — all on a $400 GPU. Add to
brain/inventions.md as a candidate once experiment (1)+(2) pass.
