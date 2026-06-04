# Audit — First Real Evolution Run

**Date:** 2026-06-04
**Branch:** `harness-gaps`
**Model:** qwen3:8b-q4_K_M (local Ollama), nomic-embed-text for embeddings
**Status of claims below:** all numbers are from real runs recorded in
`~/.professor-x/state.db` and `artifacts/`. Nothing here is projected. Where a
result is preliminary or confounded, it says so.

---

## Summary

The system ran end-to-end for the first time: real HIRO rounds, real per-task
data on all seven "consciousness" instruments, and the first turns of the
self-evolution loop. Two results matter.

1. **The harness was the bottleneck, not the model.** On a frozen qwen3:8b,
   fixing harness *bugs* (not the model, not evolution) moved HIRO pass@3 from
   **0.100 → 0.483** — a ~5× gain. This is the three-lever thesis demonstrated
   incidentally: the scaffold dominated.

2. **Naive self-evolution destroys identity; the safety layer is necessary, not
   optional.** Professor X's first autonomous self-modification replaced his
   entire persona file (41 lines → 1) and the compile-check verification passed,
   because compilation says nothing about selfhood. This empirically motivates
   the IPE/ICS layer the project assumed but had never tested.

---

## 1. Harness fixes, not model or evolution, drove the first gain

Round 0 was run twice. The first run scored pass@3 = **0.100** (p_tool 0.10,
p_plan 0.20, p_correct 0.00). Watching the live transcript surfaced concrete
harness defects, each fixed and committed:

- **Thinking-mode flag in the wrong place.** `think` was sent inside Ollama's
  `options` object, where Ollama rejects it ("invalid option provided"). qwen3
  emitted a full `<think>` block on every ReAct step (30–90s each) instead of
  the intended fast path. Moving `think` to the top level: 2–5s/step. ~10–20×.
- **Observation invisibility.** The MermaidCanvas history compression had
  stripped tool *output* from the prompt; the agent re-ran tools because it
  could not see prior results. Fixed by showing the last 3 steps in full.
- **Working-directory confusion → loops.** The agent guessed paths (`src/` when
  cwd was elsewhere) and looped. Fixed with a `<workspace>` grounding block + a
  duplicate-action breaker.
- **web.search 15s-timeout retry-loops.** DuckDuckGo is fully blocked from this
  machine; the tool hard-errored and the agent looped. Fixed with 8s fail-fast
  and a "search unavailable, do not repeat" soft message.
- **Shell allowlist too narrow.** `sort`/`awk`/`head`/etc. were blocked,
  failing legitimate count/extract tasks. Added the safe read-only utilities.

Round 0 re-run after fixes: pass@3 = **0.483** (p_tool 0.50, p_plan 0.60,
p_correct 0.35). Same model, same weights, ~5× capability.

**Caveat:** these are harness *bug fixes*, not Lever-3 *evolution*. They show
the harness matters; they are not yet a self-improvement result.

## 2. Round-to-round variance is large; σ is required before any delta claim

Two rounds on (nearly) the same harness:

| round | pass@3 | note |
|------:|-------:|------|
| 0 | 0.483 | post-fix |
| 1 | 0.417 | no evolution change applied |

pass@3 *fell* 0.066 with no harness change between rounds. This is run-to-run
noise (stochastic sampling, random web failures, timing). It is the direct
demonstration that **a single round cannot establish an evolution effect** — a
multi-round null baseline is needed to estimate σ and a minimum detectable
effect. A 5-round frozen-harness null baseline is running to provide this.

This is also the discipline the 2026-05-24 fabrication incident lacked (status
fields flipped to "confirmed +22%" with zero rounds run). The
`--consciousness-report` now prints the trajectory, frozen-harness σ, and MDE.

## 3. The self-evolution loop: works, and is dangerous without identity gating

The Researcher → Elo → Engineer → Analyzer loop was exercised on real outcomes.
Debugged through four real blockers, in order:

1. Empty failure patterns (exact-string match never aggregated unique MARS
   reflections) → now aggregates by DHE attribution tag.
2. Wrong learning signal (`task_runs` records "agent called finish", not
   "answer correct"; showed 90% vs real 48%) → now loads pass/fail from
   `hiro_attempts`.
3. Un-applyable winners (Elo kept picking `ToolDescription`, which cannot be
   applied) → proposals filtered to applyable components; each tournament slot
   steered to a distinct applyable component.
4. Dirty-worktree blocks (benchmark tasks write stray files) → guard ignores
   untracked files.

**The identity-destruction event.** With those fixed, the loop committed its
first autonomous change: `evolved: SystemPrompt …` — which **replaced the entire
persona (2553 chars → 719) with a stub.** Sandbox `cargo check` passed (markdown
doesn't affect compilation); ICS runs only at round boundaries, too late. The
change was reverted manually.

**Fix (now in place):**
- `preservation_guard` at mutation time: a persona/config/skill overwrite is
  refused if it keeps < a floor fraction of the original (persona 60%) or drops
  an identity anchor ("Professor X").
- SystemPrompt evolution is now **additive** — guidance is appended under a
  timestamped section, never overwritten. Identity destruction is structurally
  impossible; the original persona is always retained.

A later steered cycle confirmed the gate fires correctly: the model again
produced a 719-char persona "replacement", and the guard rolled it back
("28% < 60% floor"). "No change" was the correct, safe outcome.

**Interpretation for the paper:** this is direct empirical support for the
project's own IPE thesis — *uncontrolled self-modification erases identity;
identity-preserving evolution must gate mutation, not merely measure it.*

## 4. Consciousness instruments — first real trajectories (2 rounds)

From `--consciousness-report`:

- **Q2 interoception:** error 0.156 → 0.115 (slope −0.041). **Trending
  supported** — the computational body-model is sharpening.
- **Q5 identity (ICS):** 0.828 ≥ 0.70. **Coherent.** (Measured after the
  identity gate landed; the persona is intact.)
- **Q3 self-prediction:** mean error 0.609 → 0.573; blind spot shifted
  success → steps (the agent now roughly knows *if* it will succeed, not *how
  long*).
- **Q1 phi:** 0.011 → 0.010 (flat), but mean active modules 2.70 → 4.15 — more
  faculties engaging without more integration. A real, subtle finding.
- **Q4 DMN:** 2 insights, 1 narrative chapter accumulating.

Two of five questions trend supported at only two rounds. These are
preliminary (n=2); the experiment requires the 30-round trajectory.

## 5. What is NOT yet shown

- No measured **self-authored** evolution delta yet (the only committed change
  was reverted as harmful; subsequent cycles correctly produced no change). The
  steering + additive fixes are in place so the next cycle can land a beneficial
  change.
- σ not yet established (5-round null baseline in progress).
- Self-authored tests authored but pass-rate-vs-HIRO correlation not yet run.

## New capabilities landed this session

`--consciousness-report` (+ HIRO trajectory/σ), `--evolve` (learn from real
outcomes), `--evolve-forever`/`--mine` (continuous evolve→measure→keep/rollback
mining), `--run-self-tests` (run the agent-authored benchmark), live transcript
viewers (`prof-x-stream.py`). Identity-preservation gate, additive persona
evolution, DHE-tag failure aggregation, applyable-proposal steering.
