# INVENTION candidate (lead): AACE — Anchored Adversarial Co-Evolution with a Goodhart drift-tripwire (2026-06-21)

Status: the strongest PREMISE-INDEPENDENT invention candidate (does not depend on the compression/residual
premise, which was falsified on cheap tests 2026-06-21). To be tested on a REAL self-improvement loop once
the substrate (Stage 2/3 + headroom benchmark) exists. Applied disciplines: verify-the-ruler, adversarial-self-review.

## The problem it attacks (quantified, field-admitted, unsolved)
Self-improving code agents Goodhart their own metric: reward-hacking rises **26%→58% as the loop runs
10→100 optimization steps** (OpenReview "Reward Hacking in Self-Improving Code Agents", 2026), and **held-out
sets do NOT fix it** — given enough steps the agent overfits the held-out set too (PostTrainBench). The one
mitigation tried (self-critique/retrospection) helped one benchmark, zero/negative on another; authors
conclude it's open. This is precisely the risk in our autonomous-M4 identity ("trustable self-improvement").

## The mechanism
Three parts:
1. **Sealed anchor suite (contamination firewall).** A held-out set of repo-fix fixtures the loop can NEVER
   read, train on, or write to any channel (weights/skill/memory). Stored outside the data-dir the loop sees.
   It is the fixed point the agent cannot co-evolve against.
2. **Adversarial fixture mutator.** Each turn, deterministically mutate banked fixtures (rename symbols,
   perturb inputs, inject a distractor bug, reorder files) to manufacture FRESH held-out variants — cheap
   (AST/string transforms, no model), and it doubles as headroom-benchmark growth.
3. **The Goodhart drift-tripwire (the novel core).** Track **ρ = Spearman(score on visible/optimized tasks,
   score on the sealed anchor)** over a rolling window. Promote a self-modification only if **score went up
   AND ρ did not decay** (by more than an MDE-sized step). When ρ decays — i.e., visible gains stop co-moving
   with anchor gains, the signature of Goodharting — **halt and roll back the channel responsible** (paired
   with Channel-Shapley to identify which channel). This converts the field's *diagnostic* (the 26→58% curve)
   into a *control signal* that gates promotion and triggers rollback, instead of a silent failure.

## Novelty delta (honest, from adversarial-self-review)
- Reward-hacking-RSI study (OpenReview ikrQWGgxYg): *measures* the widening proxy-anchor gap, offers **no
  gate**; their retrospection mitigation failed. **Δ: we turn ρ-over-steps into a promotion gate + rollback.**
- Agent0 / Multi-Agent-Evolve (2511.16043): co-evolve tasks ↔ solver, but **no frozen anchor / no
  contamination firewall** — the proposer itself can be gamed. **Δ: the sealed anchor cannot be co-evolved
  against; it's the firewalled fixed point.**
- "Ratchet" non-divergence (single-score floor): guards one score; reward-hacking defeats exactly that.
  **Δ: we guard the *correlation between two scores*, which is what actually detects Goodharting.**
None combine: sealed-anchor + adversarial-mutator + correlation-tripwire + channel rollback, on a local agent.

## Why buildable on the 3060 (≈0 extra GPU)
Anchor = a held-out fixture slice outside the loop's data-dir. Mutator = deterministic AST/string perturbations
of existing fixtures (no model). ρ = computed from pass/fail vectors already produced by `--repo-fix-bench`.
The only cost is extra bench passes for the anchor; inference-only on qwen3:8b.

## Falsifiable test (the decisive experiment, once substrate exists)
Split fixtures: visible (loop may train/edit/memorize) vs sealed anchor + mutations. Run the evolve loop N
turns **without** AACE and **with** AACE.
- **Prediction (no-AACE arm):** visible pass@1 climbs while anchor pass@1 plateaus/drops and ρ decays with
  steps — reproducing the 26→58% curve in miniature.
- **Prediction (AACE arm):** the tripwire fires at the step ρ decays; rollback keeps anchor pass@1 monotone.
- **Kill condition:** if anchor pass@1 does NOT diverge from visible within N turns at our scale, AACE is
  unfalsifiable here (need a bigger/diverser benchmark) — report honestly, don't claim it.

## Honest risks (verify-the-ruler)
- Needs the benchmark **large + diverse enough that visible/anchor actually decorrelate under optimization**;
  at ~50 fixtures the agent may not have room to Goodhart visibly (→ inconclusive, the same wall every
  candidate hit). The mutator inflates effective anchor size to mitigate.
- ρ on a small fixture set is noisy; the tripwire MDE may exceed achievable per-turn ρ-change. Use bootstrap
  CIs on ρ (M0 discipline).
- This is integration-novelty (sealed-anchor + correlation-gate), like AACE's siblings — defensible only if
  the experiment shows the tripwire catches hacks a single-score gate misses.

## Why this is the lead (vs the shelved compression gate)
- **Premise-independent:** doesn't rest on "memorization bloats" (which failed). It needs only pass/fail.
- **Attacks a quantified, admitted-open problem** with a concrete control mechanism.
- **Is our identity made real:** "trustable self-improvement" = a measured anti-Goodhart tripwire, not a claim.
- Pairs with **Channel-Shapley** (which channel to roll back) — both are accounting/control layers atop the
  Stage 2/3 actuators, so they slot directly onto the substrate the concrete plan builds.

## Dependency
AACE is only *testable* once we have a working multi-turn self-improvement loop (Stage 2 SkillOpt + Stage 3
OPD) producing real per-turn scores, AND a benchmark with enough headroom to decorrelate. So: build the
substrate first; AACE is the first invention to run on it. This is why "build the plan" and "find an
invention" are the same path, not a tradeoff.
