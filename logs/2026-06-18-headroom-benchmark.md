# 2026-06-18 — Building a ruler that can actually measure self-improvement

**Goal:** the headline feature is "the agent improves itself," but you **can't show improvement on a
test the agent already aces.** Yesterday's gate rejected-by-ceiling because the 14-task repo-fix set
is saturated (~0.95). So: build a benchmark with real **headroom**.

**Headline:** done. We now have a graded measuring stick where the 8B baseline sits at **0.58** —
mid-band, with room to rise. Plus an honest catch: it's **noisy**, and that changes how we must
measure.

---

## What we built
- **Hard tier** (`hard_001..008`): 8 fixtures built around the difficulty lever that actually breaks
  small agents — **multi-file indirection** (the bug is buried 2-3 files deep behind an imported
  helper, with distractor code; the task says only the *symptom*). All validated red→green.
  → 8B baseline: **0.125 (1/8).** Real headroom; the opposite of the saturated set.
- **Graded ruler** (`tasks_graded.json`): 36 medium (single-file, subtle) + 8 hard = 44 tasks, a
  difficulty gradient. → 8B baseline **K=2 mean 0.580** (runs: 0.477, 0.682).

## Two findings that matter
1. **The hard tier fails on multi-file *localization* — a harness gap, not model capability.**
   The agent burns its step budget without surveying the other files (the bench prompt even says
   "Read the buggy *file*", singular). That's directly on-thesis: *the harness is the lever.* It's
   also the **first experiment**: add a "survey all files first" step + more steps for multi-file
   tasks, and measure the lift.
2. **The ruler is noisy** (verify-the-ruler). Two runs swung **0.205** (21→30 of 44 tasks) — mid-
   difficulty tasks at the model's ability edge flip pass/fail run-to-run. Implication for the gate:
   use **K≥5 passes** and/or grow to ~80 tasks, and **MDE ≥ 0.10**. Large improvements will show;
   small ones will drown in the noise. (Honest ruler > flattering ruler.)

## Decisions added
- **D-008** — A self-improvement product needs a *headroom* benchmark first; you can't demo a curve
  on a saturated test.
- **D-009** — A headroom benchmark is noisy by construction (edge-of-ability tasks flip). Measure
  deltas with enough passes (K≥5) / enough tasks, MDE sized to the observed run-to-run variance.

## Status / next
- ✅ Measuring stick exists: `tasks_graded.json`, baseline pinned at 0.58 (`baseline_graded.txt`).
- ✅ Unified-loop design + first-experiment spec written
  (`docs/research/2026-06-18-unified-loop-design.md`).
- ▶️ **Next (needs build+test+verify, do with a human):** Experiment #1 — multi-file-survey prompt /
  step-budget change → measure on `tasks_graded.json` (K≥5). First real point on the curve.
- Did NOT make autonomous harness code changes overnight (misevolution risk; that's what the gate
  and this discipline exist to prevent).
