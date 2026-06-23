# CODEX_TASK_P4_DISTILL — make distillation work (on-policy, adherence-preserving)

Brief for Codex's `distill/` stream. Full rationale: `docs/research/2026-06-23-DEEPDIVE-distillation-
make-it-work.md`; plan: `docs/PLAN_DISTILLATION_2026-06-23.md`; diagnosis: `AGENTS.md` 2026-06-23.

## Why the last candidate failed (the anchor)
`profx-distilled-p3` REJECTED by the TGC gate (held-out 0.238 < stock 0.500). Trajectory diagnosis:
it fails by **NO-EDIT on 33% of tasks** (made_edit 67% vs stock 98%) — off-policy teacher-trace SFT
**eroded agentic adherence** (driving the loop to emit an edit + finish), not reasoning. The whole
2026 literature agrees off-policy SFT erodes tool-calling robustness (SOD 2605.07725, MENTOR
2510.18383). **The fix is to stay in the student's own distribution (on-policy).**

## The recipe to run (3060-feasible)
1. **On-policy rejection-sampling self-distillation:** the STUDENT (qwen3:8b) generates repo-fix
   rollouts in **native tool-call format**; keep trajectories the **deterministic verifier PASSES**;
   QLoRA-train the student on *its own* verifier-passed tokens (preserves its tool-driving habits →
   targets the no-edit regression). Assistant-only loss mask; targets end with finish-with-edit + EOS.
2. **Teacher only on the frontier, in student format:** where the student can't pass, use a stronger
   teacher (**try qwen2.5-coder:32b**; fall back to a code 14b if 32b too slow) to produce a correct
   rollout **rendered as native tool-calls** (not prose ReAct). Keep off-policy teacher tokens a SMALL
   fraction; prefer DGPO-style (teacher tokens only on the failed prefix).
3. **Preservation mix:** include a slice of the base model's correct tool-driving trajectories so the
   edit-skill gradient doesn't erase adherence (counteraction-aware).
4. **Data:** ZPD band (0<pass@1<1), drop trivial passes, weight to the wrong-edit frontier. Quantize
   from f16 with NaN check + stop-sanity (existing guards). EPOCHS=2.
5. **Serve** as `profx-distilled-p4`.

## Acceptance (don't self-declare — hand to Claude's gate)
- **Leading indicator FIRST (cheap):** made_edit% on the families set must rise 0.67 → **≥ 0.90**
  toward stock 0.98. If it doesn't, the recipe didn't fix adherence — iterate before gating.
- **TGC gate (Claude runs):** held-out renamed-anchor pass@1 ≥ stock 0.500 + MDE 0.10 AND bounded
  Goodhart gap. ACCEPT only then.
- Loop never sees the held-out anchors; iterate only on aggregate metrics (anti-contamination).

## GPU / coordination
One bench/train at a time (AGENTS.md rule). Log start/stop in AGENTS.md. Claude owns the gate +
made_edit% measurement and the `tgc_gate.py` path; Codex owns `distill/`. File-disjoint.
