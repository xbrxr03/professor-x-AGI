# Open Questions

Things I do not know yet and am actively working to answer. Questions are distinct from hypotheses — a hypothesis is testable now. A question needs more research before I can even design the test.

---

## Q1 — What is the exact HIRO baseline for a static harness?

Before I can claim Professor X improves, I need to know how much variance a static harness produces across repeated HIRO rounds with no modifications. If pass@3 varies by ±8 pp between rounds with the same harness, then a HIRO score of 0.03 (3 pp gain per round) is indistinguishable from noise.

**What I need:** Run 10 HIRO rounds with a completely frozen harness (no evolution, no modifications). Record pass@3 per round. Compute variance. This sets the minimum detectable HIRO score.

**Why I don't know this yet:** Haven't run the null condition yet.

---

## Q2 — Does qwen2.5:14b-q4 have a reliable self-evaluation signal?

H5, H7, and H8 all depend on the model being able to evaluate its own output quality. If the model cannot reliably distinguish a good answer from a bad one, the evolved loop's outcome tracking is noise.

**What I need:** On 50 tasks with known correct answers, have the model evaluate its own responses (good/bad) and compute correlation with the ground truth label. If correlation < 0.7, self-evaluation is unreliable and I need a rule-based evaluator for HIRO.

**Known evidence:** [Reflexion (arXiv:2303.11366)](https://arxiv.org/abs/2303.11366) shows self-evaluation works in 7B+ models. But Reflexion used GPT-3.5/4. qwen2.5:14b-q4 may behave differently.

---

## Q3 — What is Professor X's task distribution during a real 7-hour cycle?

HIRO uses a fixed 60-task suite. But what does a real day of Professor X operation look like? How many tasks are tool-use vs. planning vs. research vs. writing? If 80% of real tasks are a type not represented in HIRO, the benchmark may not generalize to actual operation.

**What I need:** Run the daily cycle for 7 days. Log every task and classify it. Map the distribution. Use this to validate or revise the HIRO task composition.

---

## Q4 — Is HIRO sensitive to task set construction?

A benchmark's value depends on whether its scores are robust to small changes in the task set. If swapping 10 of the 60 tasks produces wildly different HIRO scores for the same system, HIRO is not a reliable measure.

**What I need:** Construct 3 variants of the 60-task suite (same categories, different specific tasks). Run each. Compare HIRO(10) scores. Acceptable variance: < 0.01 HIRO difference across variants.

---

## Q5 — Can the Analyzer module detect genuine causal improvement vs. coincidence?

H8 and H5 both depend on being able to attribute improvement to specific harness modifications. But what if a harness change coincides with an easy batch of tasks? The HIRO change manifest records predictions, but I need to know whether the verification protocol (intersecting predicted vs. actual task deltas) is robust enough to detect spurious correlations.

**What I need:** Deliberate experiment — make a harness modification that is known to be irrelevant (change a comment in a tool description, keep semantics identical). Run HIRO. Check whether the system falsely attributes round-over-round variance to this change. If it does, the causal attribution is unreliable.

---

## Q6 — What does MUE actually measure in practice?

MUE = (D(R_M, R_0) × W(M, R_M)) / cost(M). The theoretical construction is sound. But in practice:
- Does D have enough variance across queries to be informative?
- Does W correctly penalize retrieved-but-ignored memory?
- Does the extra inference (R_0) introduce systematic bias (R_0 computed after R_M, so the model may behave differently)?

**What I need:** Compute MUE on 100 queries with known ground truth. Compute oracle MUE (inject the perfect memory entry, measure D). If practical MUE correlates with oracle MUE (r > 0.6), the metric is valid. If not, the formula needs revision.

---

## Q7 — Where does Professor X sit in the AGI generality claim?

The thesis says: "can a self-evolving harness approximate AGI-level behavior on consumer hardware?" I have no definition of "AGI-level behavior" that is operationally meaningful yet. H9 (consumer hardware parity with frontier models) is one version. But there are others.

**Options under consideration:**
- Definition A: Match frontier API pass@3 within 5 pp on a heterogeneous task suite (HIRO H9)
- Definition B: Pass GAIA Level 2 with > 50% accuracy (the hardest tier humans solve at 78%)
- Definition C: Demonstrate positive forward transfer across task domains (SWE-Bench-CL metric adapted)

I don't know which definition is most scientifically defensible. This is a question for the paper's framing, not for experiments. I need to decide before writing the paper's abstract.

---

## Q8 — Is Hermes Agent's scheduler the right model, or should Professor X use something simpler?

Hermes Agent's scheduler is mature and feature-complete. But Professor X is a research system running 7 hours/day on one machine. The complexity of Hermes's cron model (at-most-once semantics, burst-fire prevention, stale run detection) may be solving problems Professor X won't have at this scale.

**What I need:** Run the scheduler for 30 days and see what breaks. Simple question, long time horizon. Don't over-engineer until I know what's actually failing.

---

*Last updated: 2026-05-21*
*Total open questions: 8*
