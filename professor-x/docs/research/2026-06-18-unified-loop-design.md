# Unified self-improvement loop — design + first experiment (2026-06-18)

Now that we have a **headroom benchmark** (`tasks_graded.json`: baseline pass@1 ≈ **0.477**,
36 medium + 8 hard), here is the spine to build, and the first improvement to run on it.

## The loop (one path, both levers, one gate)
```
        run tasks on tasks_graded.json
                 │ (verified outcomes)
                 ▼
   MEMORY OF VERIFIED WORK  ── task_runs, causal_traces, episodic→semantic, transcripts, events
                 │
        ┌────────┴─────────┐
        ▼                  ▼
  (A) HARNESS lever     (B) WEIGHTS lever
  diagnose failures →   curate verified hard-task
  propose code/prompt    trajectories → QLoRA
  diff (procedural mem)  (distill, raw format)
        └────────┬─────────┘
                 ▼
   GATE: pass@1 on tasks_graded (K-pass mean, > baseline + MDE)
         + full test suite + reward-hack scan
                 ▼
   IDENTITY CHECK: self_model + ics + behavioral fingerprint within bounds
                 ▼
   keep iff proven & identity-preserved  →  new baseline  →  repeat
```

## What already exists per stage (from the memd audit + research docs)
- **Memory of work:** `task_runs`, `causal_traces` (wired in react.rs), `episodic`/`semantic` sleep
  consolidation (loop_runner), `transcripts`, `events`. ✅
- **Harness lever:** `--evolve-on-repofix` / `--evolve-code-on-repofix` (diagnose→propose→sandbox→
  gate→commit) already built; `diagnose-from-trajectory` skill classifies failures. ✅ (was "tapped
  out" on the *easy* set — untested on the new headroom set.)
- **Weights lever:** the distillation pipeline (raw-format training, offline merge, official-template
  serve, pre-gate check) now works end-to-end. ✅
- **Gate:** `repo_fix_measure` K-pass mean + MDE; the ICS-gate script. ✅
- **Identity:** `self_model`, `ics`, `evolved/bf.rs` behavioral fingerprint. ✅

→ The pieces exist; the build is **wiring them into one loop pointed at `tasks_graded.json`**, plus
wiring `coding_sessions` and pruning the deferred consciousness modules out of the path.

## First experiment (highest ROI, validates the thesis on the NEW ruler)
**Hypothesis:** the hard tier fails ~8/8 because of **multi-file bug localization** — the bench
prompt says "Read the buggy *file*" (singular) and the agent burns its step budget without surveying
the other files (the bug is in an imported helper 2-3 files deep). This is a **harness** gap, not a
model-capability gap → the project's core thesis, directly testable.

**The change (harness lever, needs build+test+measure → do with a human in the loop):**
1. In the repo-fix prompt / SYSTEM_PROMPT, add a multi-file survey step: *"The bug may be in an
   imported helper. First list ALL source files and read the ones on the failing call path before
   editing."*
2. Optionally raise the step budget for tasks with >1 source file.
3. Measure on `tasks_graded.json` (K=2). Accept iff > 0.477 + MDE.

**Why this first:** it's a prompt/config change (low risk, fast turn), it targets the exact observed
failure mode, and a lift here is the **first real point on the self-improvement curve** — on a
benchmark with headroom, the thing today's whole effort was missing. If it works, it's also the
cleanest reel: "watched it learn to read across files; hard-task score went from 0 to N."

## Sequencing
1. ✅ Headroom benchmark (done) — pin baseline (confirming K=2).
2. **First experiment above** (harness, multi-file survey) — fastest curve point.
3. Wire the two levers + gate into one loop pointed at `tasks_graded.json`.
4. Prune deferred modules; wire `coding_sessions`.
5. Distillation lever on the *hard* trajectories (genuine capability gap now exists).
6. Ship (M3) → self-improve on real user tasks.

Refs: docs/research/m4-frontier-self-improvement-engine.md, m4-code-proposer-scoping.md,
2026-06-18-memd-keep-prune-map.md; memory product_direction_self_improving_memory.
