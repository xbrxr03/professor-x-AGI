# Pre-check results — invention candidates (applying px-experiment-runner + verify-the-ruler) 2026-06-21

## Pre-check 3 — reuse-family / transfer potential (gates VGTS embedding + Re-Verified RAG)
- **hypothesis:** fixtures share enough structure that one solved solution could transfer to another task.
- **command:** structural analysis of all 84 repo-fix fixtures — shared module names + function names; count
  ordered pairs where transfer is even STRUCTURALLY possible (shared module AND function).
- **result:** **10 / 6972 ordered pairs = 0.1%.** 10 module names and 15 function names recur across fixtures,
  but almost no pair shares both. Deterministic (no model) -> trustworthy (verify-the-ruler).
- **interpretation:** **NULL — VGTS and Re-Verified RAG are NOT testable on this benchmark.** With ~0 reuse
  families, a retrieved solution cannot transfer/re-verify on a different task; both candidates have no signal
  here. (This is a *result*, per px-know-scientific-method -> recorded, not a failure.)
- **next_test:** only viable after building a benchmark WITH reuse families (tasks sharing modules/APIs so
  solutions genuinely transfer).

## Pre-check 1 — VCA mask-sparsity (gates Verifier-Counterfactual Credit Assignment)
- **status:** BLOCKED on data — 0 stored agent green-diff trajectories. Needs a fresh native agent run to
  capture a real green diff, then DDMIN it. NOTE (verify-the-ruler caution): our fixtures have MINIMAL 1-line
  fixes by design; if the agent also edits minimally, causal-mask sparsity ~0 -> VCA NULL here too. VCA's value
  needs realistic LARGER solutions -> also benefits from a better benchmark. Probe pending (1-fixture run).

## Pre-check 2 — quant group-sensitivity (gates Verifier-Driven Quantization)
- **status:** the ONLY candidate NOT blocked by the benchmark gap (needs only the verifier, which we have).
  Experiment: demote coarse tensor groups, measure pass@1. Pending GPU run.

## NEW OUTCOME (px-gap-analysis): the benchmark is the keystone, quantified
- **top_gap:** the benchmark has **no reuse families and minimal (1-line) solutions** -> 0.1% transfer.
- **why it blocks:** it blocks measurement-trust for VGTS, Re-Verified RAG, AND likely VCA (no realistic
  over-edited diffs), AND the headroom for AACE/OPD MDE. Three of four invention candidates die here.
- **the one unblocked candidate:** **Verifier-Driven Quantization** — needs only the deterministic verifier.
- **recommended_patch:** (a) pursue Quant now (unblocked); (b) build a **reuse-family benchmark** (tasks sharing
  a small shared library/API so solutions transfer + multi-line realistic solutions) as the keystone that
  unblocks VGTS/RAG/VCA and gives AACE/OPD real headroom.
