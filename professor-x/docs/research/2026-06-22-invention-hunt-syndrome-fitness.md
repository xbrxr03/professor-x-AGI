# Invention hunt (2026-06-22): the rename-invariant SYNDROME as a denser fitness/gate signal

Deep-dive grind (Abrar's standing goal: invent something new — use every resource, find new ones,
rinse/repeat). Method = the project's: scan the live frontier (new external resources since the
Jan-2026 model cutoff), find the empty intersection against our unique assets, propose, then
**kill it with adversarial-self-review + verify-the-ruler** before claiming anything.

## Unique assets (unchanged, and they are the whole edge)
A cheap, **decomposable, deterministic verifier** (`check.py` per fixture, callable thousands of
times) → a **rename-invariant behavioral SYNDROME** (which checks fail = a bit-vector), validated
0.93 rename-invariant vs text 0.14. Plus reuse-families + renamed anchors, dual-lever on a $400 GPU.

## New resources found this session (post-cutoff; add to MASTER-REFERENCE-LIST)
- **2605.30621 — "Harness Updating Is Not Harness Benefit: Disentangling Evolution Capabilities."**
  Decomposes harness self-evolution into Δ_update (quality of the update, *flat* in model strength —
  a 9B authors updates as good as a frontier model) vs Δ_benefit (ability to *use* an updated
  harness, **non-monotonic, peaks at mid-tier; WEAK models gain least despite the most headroom**).
  Weak models fail at **harness activation** (25% load vs 96% strong) and **adherence drifts
  0.52→0.13** across a trajectory. → **External corroboration of our own pivot:** our A5/taxonomy
  found "even handing the model the answer barely helps" — same wall. The bottleneck for a weak local
  8B is capability/adherence, NOT more harness features. *This validates Lever-1 (distillation) over
  more harness work, and reframes the "harness is the intelligence" claim: true for strong models,
  weakest exactly for our model class.*
- **2511.21654 — EvilGenie (reward-hacking benchmark, MIT/futuretech).** Three detectors: held-out
  unit tests, LLM judge, test-file-edit detection. Reported: **held-out tests gave minimal
  improvement** at catching hacks; the LLM judge was effective on unambiguous cases. (PDF did not
  expose whether held-out = same-distribution; could not confirm.) → raises the bar for us: plain
  held-out is a weak anti-hack gate; we must *show* our **renamed/metamorphic** held-out is stronger,
  not assume it (this is exactly TGC's untested Arm-A/Arm-B).
- **2605.08741 — "Training with Harnesses: On-Policy Harness Self-Distillation (OPHSD)."** OPD that
  internalizes an inference-time harness's behavior into the model. → **Partially KILLS** the naive
  "distill-for-harness-adherence" idea: the core is taken.
- **2602.07900 — "Rethinking the Value of Agent-Generated Tests."** Agent-written tests have
  **minimal impact on solve rate**; they function as *observation* (value-prints > assertions), not
  as validators or fault discriminators; **no** metamorphic/diagnostic-code framing. → the
  **verifier-as-discriminating-code white space is still open** (our DVC/Living-Verifier thread).
- **2510.04399 — Two-Gate (validation margin + capacity cap), 2603.28650 — info-theoretic limits of
  safety verification, 2604.00072 — classification-verification dichotomy for safety gates.** Gate
  theory is maturing. → TGC's *gate structure* (margin ≥ MDE + bounded gap) converges with published
  Two-Gate; **TGC's only surviving novelty is the gate SIGNAL = behavior-preserving renamed transfer**
  (contamination-proof by construction), not the gate mechanism. Narrow but defensible.
- Adjacent: 2602.07670 surprisal-guided execution-grounded selection · 2510.18471 CodeRL+ execution-
  semantics alignment · SWE-Lego step-level error masking · 2601.04728 EDL · 2602.05547 Multi-Task GRPO.

## The honest reframing the external work forces
1. Weak-model **capability/adherence** is the bottleneck, externally confirmed (2605.30621) — stop
   expecting harness features to move our 8B; the model lever (Lever 1) is right.
2. **Held-out alone is a weak anti-hack gate** (EvilGenie); behavior-invariance is the differentiator
   we must *demonstrate*, not assume.
3. Two of our "new" bets narrowed: harness-adherence-distill ≈ OPHSD (taken); TGC-gate ≈ Two-Gate
   (taken) except for the renamed-signal. The **one thread still in open water is the verifier as a
   discriminating diagnostic CODE** (DVC / Living Verifier), confirmed open by 2602.07900.

## The candidate that survives the kill-argument
**Syndrome-severity as a denser, rename-invariant FITNESS/GATE signal.**
A failing attempt is not 1 bit (pass/fail) — it is a ~6–9-bit **syndrome** (which decomposed checks
fail). Two models that both score pass@1 = 0 on a task can fail with *different* syndromes (3 checks
red vs 5). So **syndrome-severity gives gradient exactly where binary pass@1 is pinned at the floor**
— e.g. the F1 case where `profx-distilled-clean` scores 0.133 on the hard set and pass@1 cannot tell
"almost there" from "way off." Use the syndrome as the unit for: (a) the acceptance gate over
**renamed anchors** (finer ⇒ lower MDE on the same 50 fixtures ⇒ **directly attacks the coarse-MDE
wall that has blocked nearly every prior invention here**), and (b) curriculum/ZPD banding.

Novelty class (honest): **integration-novel + one new use** — the rename-invariant execution
syndrome as a *denser fitness/gate currency* on a local self-improving agent. DVC proposed the
syndrome for *retrieval*; using it as the *fitness/gate signal that beats the MDE wall* is the new,
unclaimed move. Not a new algorithm.

## Adversarial self-review (kill-argument)
- **DEFENSE:** Unifies validated assets (DVC kernel: 6/7 families unique-syndrome, 44% checks
  redundant; failure-signature 0.93 rename-invariant) with the externally-confirmed real problems
  (weak-model capability, weak held-out gate, coarse MDE). Robust, $0-CPU-testable, on-thesis.
- **PROSECUTION — the fatal flaw, named:** the **CREDIT** use (mask OPD loss to the syndrome-delta
  tokens) is **likely NULL on our 1-line fixtures** — a single correct edit flips *all* failing
  checks at once, so syndrome-delta ≡ pass/fail. This is the *same* wall that nulled VCA
  ("small fixes already ~all-causal") and the auto-repair A/B. **Do not pursue syndrome-credit on the
  current benchmark.**
- **What survives the prosecution:** the **GATE/FITNESS** use does not depend on credit. It needs
  only that *failure syndromes VARY sub-pass@1 across models/attempts on the same task*. If they do,
  there is real denser signal (lower MDE, floor-gradient) that pass@1 lacks; if syndromes are
  all-or-nothing, it collapses to pass@1 and we report NULL and move on.

## The decisive $0 CPU pre-check (run next; no GPU, does not touch Codex)
On **existing** repo-fix trajectories/artifacts (no new runs): for each hard-set + family task,
compute the per-attempt failure syndrome (bit-vector of failed decomposed checks) across the
`profx-distilled-clean` vs `qwen3:8b` attempts already on disk. Measure:
1. **Sub-pass@1 variance:** on tasks where pass@1 = 0 for both, do the two models' syndromes differ
   (Hamming > 0)? Fraction of floored tasks with syndrome separation = the signal's existence proof.
2. **MDE shrink:** bootstrap the variance of (mean syndrome-severity) vs (mean pass@1) on the 50
   fixtures; does syndrome-severity have a materially smaller MDE?
- **WIN:** syndromes vary sub-pass@1 AND MDE shrinks → build the renamed-syndrome gate as the
  fitness signal for the TGC gate (a strictly better ruler for D-integration).
- **KILL:** syndromes are all-or-nothing or MDE doesn't shrink → report NULL (like VCA); fall back to
  pass@1; the syndrome stays a retrieval-only representation.
Decomposed checks already exist for the 7 families (`beachhead_living_verifier.py` found 6/7
unique-syndrome). The hard set may need its `check.py` decomposed into sub-asserts first (cheap).

## Bottom line (verify-the-ruler: nothing claimed, one cheap test queued)
The honest yield of this grind: (1) external confirmation that our pivot to Lever-1 is right and the
weak-model bottleneck is real; (2) two of our novelty bets narrowed by new papers (OPHSD, Two-Gate);
(3) one surviving, cheap-to-falsify candidate — **the rename-invariant syndrome as a denser fitness/
gate signal that could break the coarse-MDE wall** — with its credit-use honestly pre-killed and a
$0 CPU pre-check that decides it. Next action = run that pre-check (CPU, no GPU contention with
Codex). No invention is claimed until the syndrome shows sub-pass@1 signal on data we already have.

---
## PRE-CHECK RESULT (2026-06-22, CPU, $0) — WEAK-POSITIVE, not decisive
Ran `scratchpad/syndrome_precheck.py`: reconstructed every FAILED bench attempt with a recorded
`diff_summary` (15 attempts; 13 multi-assert so gradient is possible), replayed the decomposed
`check.py` as a per-assert syndrome (`sig_runner.py`), compared to the buggy-fixture syndrome.

Result (of 13 multi-assert fails): **2 PARTIAL-progress** (fam_money_02, fam_unit_02: 3→1 failing
checks — fixed some, not all = the hypothesized sub-pass@1 gradient) · **4 regressed** (agent broke
an extra check — also sub-pass@1 info) · **7 no-change** (edit moved no check; syndrome ≡ pass@1).
Severity among multi-assert fails spans **1–5 failing checks** (distinct levels {1,2,3,4,5}).

**Honest verdict (verify-the-ruler):** the signal is **not NULL** (unlike VCA: syndromes DO vary
sub-pass@1; partial progress and regression are detectable where pass@1 can't distinguish) — but it
is **SPARSE** (54% no-change: most wrong edits are in the wrong place entirely and move no check) and
**n is tiny**. Two hard limits I will not paper over: (1) the on-disk diffs are 14b-teacher + 8b-on-
easy-set, which **share no tasks**, so I could NOT run the model-separation / MDE-shrink test that the
GATE use actually needs; (2) the F1-relevant `profx-distilled-clean` vs `qwen3:8b` diffs are not on
disk. So the candidate is **neither killed nor confirmed** — it earns a *cheap follow-up*, not a build:
  - decompose the hard-set `check.py` into more sub-asserts (raise syndrome length L>2 so gradient is
    even possible there — today most fix_0xx are L=2);
  - capture per-attempt diffs for `profx-distilled-clean` AND `qwen3:8b` on the SAME family+hard tasks,
    then test whether mean syndrome-severity separates the two models with a smaller MDE than pass@1.
The biggest *finding from the data itself*: the dominant wrong-edit class is **syndrome-invisible
(no-change)** — the agent edits the wrong location entirely. That is a LOCALIZATION failure, which
re-points at SBFL/fault-localization (the verifier-as-code thread) as the higher-value attack.

---
## ITERATION 2 (rinse & repeat) — frontier sweep narrows the candidate, redirects to DVC
Sweep 3 (fault-localization + dense-reward + harness-adaptation):
- **2601.03525 VeRPO** — verifiable DENSE per-unit-test partial-credit reward (no reward model) with
  online per-test difficulty weighting to resist trivial-test hacking. → **substantially takes** the
  "denser-than-pass@1 test signal" core of the syndrome-fitness candidate. Surviving delta = only
  *rename-invariance* + *use as a transfer GATE* (VeRPO is an RL reward, same-distribution). Thin.
- **OpenAI SWE-bench-Verified audit (Feb 2026): 59.4% of hardest tasks pass even with the bug
  UNFIXED.** → strongest external motivation yet for **verifier-as-discriminating-code**: real-world
  verifiers don't discriminate the fault; engineering diagnosability (locating arrays / DDU, +34% FL)
  is the high-value, unoccupied move for LLM agents.
- **2605.22166 Life-Harness** (the corpus's "harness portability" cite): adapt the interface not the
  model; +88.5% avg, harness from a 4B transfers to 17 backbones. Confirms harness-adaptation space is
  crowded/taken.
- **2604.05481 / SemLoc 2603.29109 / Agentless** — "even when the correct fix location is identified,
  agents frequently drift and produce incorrect edits." Corroborates our data (no-change = wrong
  location) AND A5 (right info, wrong edit) → localization alone ≠ capability.

### Honest redirect (verify-the-ruler + adversarial-self-review)
Three of this hunt's candidates are now narrowed by published work: dense-reward (VeRPO), gate-structure
(Two-Gate), harness-distill (OPHSD), harness-adaptation (Life-Harness). The **one thread that the
external scan keeps REINFORCING rather than taking** is **the verifier as a discriminating diagnostic
code** (Diagnostic Verifier Codes / Living Verifier): 2602.07900 (tests used as observation, not codes)
+ the OpenAI 59.4%-non-discriminating-tests finding + DDU diagnosability literature all converge on
"the field's verifiers don't discriminate the fault." Our beachhead already showed our verifier IS a
locating code for 6/7 families (interval the lone collision) and 44% of checks are redundant. The
data's own lesson (most wrong edits are syndrome-INVISIBLE = localization failures on coarse checks)
points the same way: make the verifier MORE discriminating, don't just read severity off coarse checks.

### Next concrete experiment (CPU, $0, decisive) — DVC diagnosability, not syndrome-fitness
1. Take the lone failing family (`interval`, two faults collide to one syndrome). Author the minimal
   extra metamorphic sub-check(s) that SEPARATE the colliding pair → does the family become a 7/7
   locating code? (engineering diagnosability, à la DDU). KILL if no small check separates them.
2. Re-run the beachhead: confirm rename-invariance preserved (anchors 14/14) AND localization
   (predict buggy_module from syndrome) now beats text (was 0.35 vs 0.47) once the suite is a code.
3. Only if 1+2 pass does the GPU payoff (behavior-keyed RAG / syndrome-credit) become worth it.
This is the project's most defensible, externally-motivated, still-open ground — and it has a cheap
CPU kill-test. The syndrome-as-fitness idea is parked as weak-positive/narrowed (see prior section),
not pursued.
