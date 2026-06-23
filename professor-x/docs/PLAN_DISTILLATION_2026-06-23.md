# PLAN: Make distillation work — phased (2026-06-23)

Premise (Abrar): **distillation HAS to work; the question is HOW.** Every phase ends in a measured,
honestly-reported gate (verify-the-ruler). The anchor for the whole plan is the Phase-0 diagnosis.

## Phase 0 — DIAGNOSE the p3 reject (DONE, 2026-06-23)
TGC gate REJECTED `profx-distilled-p3` (held-out 0.238 vs stock 0.500; train ~0.26 vs 0.333 = worse
everywhere). **Trajectory diagnosis (the why):** p3 fails by **NO-EDIT**, not bad reasoning —
- stock qwen3:8b: made_edit **47/48 (98%)**, no-edit **1**; failures are wrong-edit (30).
- p3: made_edit **32/48 (67%)**, **no-edit 16 (33%)**; wrong-edit 20.
=> Distillation **eroded AGENTIC ADHERENCE** (driving the loop to emit a valid edit + finish), while
reasoning is comparable. Residual of the original p3 "reasons right but won't operate the harness"
disease, surviving the recipe fix. Matches 2605.30621 (adherence is the weak-model bottleneck).
**Implication: the recipe must PRESERVE/STRENGTHEN tool-driving + finish-with-edit behavior, not just
transfer reasoning.** This is the lever Phase 2 must crack.

## Phase 1 — Record + reframe the headline (CPU, NOW)
- Record the diagnosis (result doc) + update AGENTS.md ledger + PROJECT_ATLAS Lever-1.
- **Reframe the moat:** the genuinely-novel + *working* thread is the **verifier-as-discriminating-code
  + Goodhart-proof TGC gate** (it caught 3 bad distilled candidates honestly; tonight's DVC work made
  all 7 families locating codes). Headline = "trustable local self-improvement that refuses
  ungeneralizing changes." Distillation is the **capability lever that must clear that gate** — not the
  headline itself. (Keep-all-directions: distillation is sequenced, not demoted out.)

## Phase 2 — Distillation deep dive (RESEARCH, no GPU) — build confidence + new leads
Find the HOW. Topics, anchored on the Phase-0 finding (preserve agentic adherence):
1. **Fundamentals + 2026 SOTA:** on-policy distillation (OPD), step-wise/process variants, what the
   labs actually ship (Qwen3/DeepSeek/GLM), off-policy SFT failure modes.
2. **Agent / tool-use distillation (the core):** how to distill agentic behavior WITHOUT eroding
   tool-call format + finish discipline — on-policy rollouts, format/structure preservation, native
   tool-call training, adherence/“activation” as an explicit objective.
3. **Distillation ORCHESTRATED BY agents (Claude/Codex):** using strong agents as teachers / data
   generators / graders / pipeline drivers; best practices, pitfalls, reproducible recipes.
4. **Recipe levers:** teacher selection (qwen2.5-coder:32b as a stronger teacher), data selection
   (ZPD/hard-frontier, adherence-positive verified traces, drop trivial), loss masking, epochs,
   format=train=serve, NaN/stop-sanity.
- Output: a synthesis doc with a **concrete new recipe + falsifiable predictions** (what should lift
  made_edit% and held-out pass@1, and how we'll know).

## Phase 3 — New distillation attempt, TGC-gated (GPU) — after Phase 2 confidence + GPU free
- Apply the Phase-2 recipe (expected shape: on-policy + adherence-preserving native-tool-call traces +
  stronger teacher). Re-quantize from f16 (NaN-checked), stop-sanity, serve.
- **Gate:** the same TGC gate (held-out renamed anchors ≥ MDE AND bounded Goodhart gap). Also track
  **made_edit%** as a leading indicator (the diagnosed bottleneck) — a recipe that doesn't lift
  made_edit% toward stock's 98% is not worth gating.
- ACCEPT → record as the first trustworthy weight-lever win. REJECT → diagnose again, iterate
  (teacher/data/recipe). Distillation has to work; we keep iterating the HOW until a candidate clears
  the gate.

## Execution order
Phase 1 (now) → Phase 2 (now, parallel with the GPU finishing the train half) → Phase 3 (after GPU
free + Phase-2 recipe). Chronological, each gated, nothing claimed without a measurement.
