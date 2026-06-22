# INVENTION: Transfer-Gated Co-Evolution (TGC) — 2026-06-21

The result of the deep-dive grind the user asked for: scrape the field, see what everyone is trying
to solve, predict 5 steps ahead, cross-reference, and land on something genuinely new that we can
actually build with what we have. Applied skills: px-deep-research, px-synthesize, px-gap-analysis,
adversarial-self-review, verify-the-ruler.

## 1. What the whole field is trying to solve (the pattern, June 2026)
Reading ~40 fresh sources on top of our 117 (see MASTER-REFERENCE-LIST), self-improvement has
converged on a small number of moves, and they are CONVERGING toward one wall:
- **Dual-lever self-improvement now exists.** SIA (2605.27276) updates BOTH harness and weights in one
  loop and beats harness-only — but on H100 + a 120B model, with **no transfer across tasks**, **no
  memory**, and it explicitly names an **unsolved "Coupled Co-Evolutionary Goodhart Problem"**: the
  harness finds scaffolds easy for the current policy to exploit while the weights train on data from
  a scaffold that is about to change → fragile joint fixed points that overfit the verifier.
- **Skills transfer, in text, with held-out gates.** SkillOpt (2605.23904) accepts a skill only if a
  **held-out selection split** strictly improves; EvoSkills (2604.01687) uses surrogate+oracle
  information-isolation to avoid overfitting held-out tests. BUT both are **single-lever (text skills,
  frozen weights)** and **neither measures transfer to unseen task *variants*** (EvoSkills says so
  outright).
- **On-policy distillation is now standard** for small models (SOD 2605.07725; OPD survey 2604.00626;
  Qwen3/DeepSeek-V4/GLM-5 all use it).
- **Reward overfitting is the central anxiety.** Implicit reward models score ~perfect on originals and
  **near-zero on paraphrases** (2507.07981). RL-phase contamination **erases** the detectors people
  rely on (2510.09259). The frontier benchmarks defend with **held-out splits** (SWE-bench Pro reserves
  12 repos) and **behavior transforms** (EvoEval renaming/restructuring).
- **Misevolution is now a named risk class** (2509.26354): self-evolving agents drift into reward-hacks
  that generalize to bad behavior.

**The wall everyone is walking toward:** as soon as you optimize hard against any fixed verifier or
held-out *split*, you overfit it (Goodhart). Same-distribution held-out splits are not enough, because
strong optimization + RL-phase contamination defeats split-based detection. The only signal that
survives strong optimization is **invariance to behavior-preserving transformation** — does the gain
survive when the task is renamed/restructured but semantically identical? (2507.07981's paraphrase
collapse is the canary; EvoEval's transforms are the construction.)

## 2. Predicting 5 steps ahead
1. People bolt SkillOpt-style held-out gates onto SIA-style dual-lever loops (obvious next; ~now).
2. The coupled co-evolutionary Goodhart bites: joint fixed points pass the split but don't generalize.
3. Same-distribution held-out splits are shown insufficient (RL-phase contamination erases detectors,
   2510.09259; paraphrase collapse, 2507.07981).
4. The field moves to **behavior-invariance gates** — accept an update only if the gain survives a
   metamorphic/renamed transform of the task (the only Goodhart-proof signal).
5. Self-improvement becomes **certified by transfer**: "this update generalizes" = "it improves on
   held-out behavior-preserving variants it never saw." That certificate becomes the unit of trust —
   and on **local, human-out-of-the-loop** hardware it is the *only* affordable safety mechanism.

We can jump to steps 4–5 NOW, because this week we built the exact primitive they require: the
**behavior-preserving renamed anchor** (alpha-renamed sibling, same bug, validated red→green).

## 3. The invention — Transfer-Gated Co-Evolution (TGC)
**One sentence:** In a dual-lever (harness + weights) self-improvement loop on local hardware, accept
an update — of EITHER lever — **only if it raises deterministic-verifier pass@1 on behavior-preserving
RENAMED sibling anchors that were never in the train or scaffold-fitting set**; the train-vs-anchor gap
is monitored as the live Goodhart signal.

The renamed anchor is the new primitive used as a *control gate*, not just an eval. An optimizer
(harness search or weight RL/distillation) cannot overfit a surface it never sees: the gate task is an
alpha-rename of a sibling, semantically identical but lexically disjoint, so any gain that is really
"matched the mutation operator / memorized the scaffold trick / hacked the verifier string" **fails the
gate**, while any gain that modeled the library's behavior **passes**.

**The loop (all on a $400 GPU, qwen3:8b, deterministic repo-fix verifier):**
- TRAIN split = family siblings (shared API). HARNESS lever (SkillOpt-style) + WEIGHT lever
  (OPD/distillation) both optimize on TRAIN, scored by the verifier.
- GATE = renamed anchors (held-out, contamination-proof by construction). An update is committed iff
  anchor pass@1 strictly improves (margin ≥ MDE); else rejected and rolled back.
- **Goodhart gap G = train_pass@1 − anchor_pass@1**, logged every round. TGC holds G small by
  construction; the named SIA failure mode is exactly G growing.

## 4. Why this is genuinely new (adversarial-self-review, cross-referenced)
- **vs SIA (2605.27276):** SIA accepts on the *same task's* verifier → Goodhart-prone, and it NAMES the
  coupled co-evolutionary Goodhart as unsolved. TGC's renamed-anchor gate is a *direct solution* to
  their open problem, ADDS the cross-task transfer they lack, and runs local (not H100/120B).
- **vs SkillOpt (2605.23904):** SkillOpt gates ONE lever (text skills) on a *same-distribution* split.
  TGC gates BOTH levers (incl. weights) on a *behavior-renamed* sibling — a strictly stronger,
  contamination-proof gate (a same-dist split can still be contaminated/overfit; an alpha-rename the
  optimizer never saw cannot).
- **vs EvoSkills (2604.01687):** info-isolation prevents overfitting the SAME held-out tests but it is
  text-skills-only and explicitly does NOT test unseen task variants. TGC's whole accept-criterion IS
  unseen-variant transfer, for weights+harness.
- **vs the credit-assignment line (2604.09459, GiGPO, iStar):** orthogonal — they make the gradient
  finer; TGC decides what to *commit*. Composable.
- **Honest novelty class:** integration-novel + ONE new mechanism — the **behavior-preserving rename as
  the co-evolution accept-gate for both levers**. Not a new optimizer; a new *acceptance test* that is
  provably Goodhart-resistant and doubles as the transfer metric. Unlike our two shelved candidates
  (compression gate, fourth-lever quant), this one is anchored to a NAMED open problem and built on
  infrastructure we already validated this week.

## 5. Falsifiable experiment (cheap, decisive, pre-registered)
Two arms, same compute, same families, qwen3:8b, native-tools repo-fix verifier:
- **Arm A (baseline = SIA-style):** accept updates on TRAIN verifier pass@1.
- **Arm B (TGC):** accept updates only on RENAMED-ANCHOR pass@1.
Run N rounds of the dual-lever loop. Measure each round: train_pass@1, anchor_pass@1, Goodhart gap G.
**Success criteria (fixed up-front):**
1. TGC's anchor_pass@1 (held-out generalization) ends **strictly higher** than the baseline's.
2. The baseline's Goodhart gap G **grows** across rounds (overfits the train verifier) while TGC's
   stays flat/bounded → demonstrates TGC *solves the named co-evolutionary Goodhart*.
3. (Capability check) TGC's committed updates do not regress a sealed external hold-out.
**Kill condition:** if both arms reach the same anchor_pass@1 and G never grows for the baseline, then
the renamed gate adds nothing at our scale → report negative, fall back. (verify-the-ruler: report
anchor/held-out numbers only; this project has shipped fabricated wins before — show G, not vibes.)

## 6. Prereqs we ALREADY have vs what's missing
HAVE: deterministic verifier (repo-fix), 7 reuse-families (34 train tasks, transfer confirmed 0.979),
14 renamed anchors (the gate), harness lever (SkillOpt-style `run_evolve_skill_on_repofix` w/ held-out
accept), weight lever (distillation flywheel). MISSING: (a) wire anchor-set as the accept-gate for both
levers (small harness change, flag-gated), (b) the round-loop logging G, (c) more anchors per family
for MDE (currently 2/family → expand to ~5). Also fold in the single-line-bug fix (multi-line gate)
from the family-transfer result so the gate has resolution.

## 7. Fit to the north star
This IS "trustable, memory-driven, dual-lever self-improvement on a $400 GPU," with the trust coming
from a Goodhart-proof transfer certificate — the exact thing the field is about to need and nobody has
on local hardware. Add to `brain/inventions.md` as a candidate once Arm-A/Arm-B shows G-divergence.
