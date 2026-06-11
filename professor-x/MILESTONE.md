# MILESTONE — the north star (read this before NEXT_STEPS.md)

**Decided 2026-06-10.** Stop optimizing for breadth (features, instruments, UX,
consciousness) on top of a core agent that scores ~0 on its own trivial benchmark.
Drive ONE vertical to a real, externally-credible, try-able result.

## The ladder: build #2, which becomes #1
1. **#2 first — a daily-driver local coding agent people actually install and use.**
   Runs *fully offline* on a consumer GPU (RTX 3060, 8B). Point it at a repo, it does
   real work. The "wanna try" hook: every competitor needs a frontier API; this doesn't.
2. **#2 becomes #1 — the agent measurably improves *itself* at the tasks people use it
   for.** A reproducible pass-rate curve over autonomous evolution rounds, on a $400 GPU,
   no human in the loop. THIS is the novel, defensible, publishable/viral artifact —
   and it only lands because it's improving a thing that actually works.
3. **Later — consciousness as the deep-research layer** attached to a working,
   self-improving agent. It's the paper's "why," never the demo.

## What "worth showing" means, concretely
- A number on a benchmark strangers recognize (not the toy HIRO-null set).
- One command for a stranger to try it, offline, on their own repo.
- A GIF of it fixing a real task with no network.
- A self-improvement curve that's honest and reproducible.

Until we have the first two, nothing else is worth building.

---

## Honest starting truth (2026-06-10)
Last real re-measure (Phase 0.5.3): `pass@3=0.333 p_tool=0.333 p_plan=0.000 p_correct=0.000`
— on trivial read/shell tasks. Two red flags: `p_correct=0` *after* the answer-gate fix,
and `p_plan` that never fires. **We don't have a working agent AND we don't trust the
scoreboard.** Both get fixed before we build anything else.

---

## Phases (highest ROI first; each gate is a number, not a vibe)

### M0 — Trust the scoreboard *(prerequisite, do first)*
Find out why `p_correct=0` and `p_plan=0` are degenerate: is the judge broken, or are
answers genuinely absent/wrong? Fix the eval so a score *means* something. No new
features until the number is honest (this project has shipped fabricated metrics before
— integrity gate is non-optional).
- **Gate:** on a hand-labeled set of ~15 trajectories, the metric agrees with human
  judgment ≥ 90%. `p_plan` either fires meaningfully or is removed.

### M1 — Wire a real benchmark
Adopt a *small real* coding benchmark runnable offline on the 3060: a curated
mini-SWE-bench / SWE-bench-Verified-Lite subset, or a handful of real GitHub-issue
tasks on a sample repo. Reference: `_refs/harnesses/SWE-agent`, augment-swebench-agent.
- **Gate:** produces an honest `pass@1`; baseline recorded even if it's near-zero.

### M2 — Make the core loop actually finish (the capability grind)
Use the edit stack already built (hashedit/window/editverify/apply_patch) + the failure
taxonomy. Fix the remaining real blockers (finish-gating ✓; thrash/forfeit control;
tool/backend stability). Relentless re-measure.
- **Gate A:** toy HIRO-null → `pass@3 ≥ 0.8` with a *meaningful* `p_correct`.
- **Gate B:** real benchmark → first non-zero, then climbing run-over-run.
- **Gate C:** a stranger's "fix this bug in this small repo" task completes end-to-end,
  verified by its own tests, fully offline.

### M3 — The try-it product (milestone #2)
One-command install; `profx <repo>` works offline on an 8B; live TUI; @file; diffs;
checkpoints/undo. README leads with the M1 number + a GIF of an offline real-task fix.
- **Gate:** a person who is not Abrar installs it and completes a real task with no help.
  Dogfood: Prof X is used to build its own next feature.

### M4 — The self-improvement curve (#2 becomes #1)
Run the evolution loop ON the real benchmark; show pass-rate rising over N autonomous
rounds, identity/ICS gate keeping it safe. Reference: AutoHarness (small model + synth
harness > bigger model) for the citation that this result class is real.
- **Gate:** reproducible curve, round-0 vs round-N pass-rate above noise (> MDE), honest
  log, no fabrication. The shareable artifact.

### M5 — Consciousness layer *(deferred, not cancelled)*
Attach the measurement program to the working, self-improving agent. The paper's depth.

---

## What we CUT / FREEZE now (do not let any agent add these)
- No new harness features beyond the edit stack + reliability work.
- Freeze UX/TUI/web polish except the minimum M3 demo needs.
- Defer all consciousness instruments until M4 is real.
- No multi-provider / frontier-API reach (local-first thesis).
- No heavy multi-agent swarm (survey's "strong single-agent baseline").

## Relationship to NEXT_STEPS.md
`NEXT_STEPS.md` is the granular task order; it had us finish the Phase-1 edit stack.
That stack is good infra but the taxonomy showed it was NOT the bottleneck. Re-aim the
granular plan at **M0 → M1 → M2** next, not the remaining Phase-1/3 polish.
