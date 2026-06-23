# DEEP DIVE: how to make distillation work — the HOW (2026-06-23)

Phase 2 of `PLAN_DISTILLATION_2026-06-23.md`. Premise (Abrar): distillation HAS to work; find the HOW.
Anchored on the Phase-0 diagnosis: **p3 fails by NO-EDIT (33%) — off-policy SFT eroded agentic
ADHERENCE (driving the loop to an edit + finish), not reasoning.** Method: verify-the-ruler,
adversarial-self-review. New resources added to the corpus below.

## The convergent finding (4 independent lines, one conclusion)
**Off-policy teacher-trace SFT erodes tool-calling robustness; on-policy distillation is the fix.**
- **SOD — Step-wise On-policy Distillation for SLM Agents (2605.07725).** Names our exact failure: "a
  single erroneous tool call causes subsequent steps to unfold from a corrupted state" → student-
  teacher divergence cascades → off-policy supervision becomes unreliable. Fix: student generates the
  rollouts (its own distribution, so its format/tool-habits are preserved); teacher scores token-level
  *post-hoc*; reweight distillation by step-level divergence wₖ (attenuate where the student diverges,
  keep dense guidance where aligned); reverse-KL; mask tool-observation tokens; pair with GRPO outcome
  reward. (Students 0.6–1.7B, teachers 4–14B, but on 8×H20 — see feasibility caveat.)
- **MENTOR (2510.18383) / SFT-trigger finding.** "SFT-tuned models do NOT show consistent tool-calling
  frequency vs base; the trigger boundary is significantly less robust." → this IS our no-edit
  regression: SFT made the *decision to call a tool / make an edit* less robust. Fix = RL/teacher-
  reward, not pure SFT.
- **DGPO (Distillation-Guided Policy Optimization).** Selective reverse-KL teacher guidance on
  student-generated outputs; **teacher intervenes ONLY when the student's autonomous attempt fails.**
  Preserves the student's own behavior, corrects only failures.
- **Counteraction-Aware Multi-Teacher OPD.** Decouples "recovery" (add skill) vs "preservation" (keep
  general capability) gradients so adding edit-skill does NOT degrade general agentic capability —
  literally the named cure for "we added edit-skill and lost adherence."
- Supporting: Agent Distillation (2505.17612) — first-thought prefix + self-consistent action gen
  improves teacher-trace quality; **RL-distillation generalizes OOD where SFT overfits the train
  domain.** Structured Web-Agent Distillation (2604.07776) — structured > trace-mimicry for
  generalization. AgentDistill/MCP-Box (2506.14728) — training-free: mount reusable tool strategies at
  inference (harness-side alternative).

## Distillation orchestrated BY agents (the Claude/Codex angle)
- **Agentic Knowledge Distillation (2602.10869):** a strong LLM acts as autonomous teacher — generates
  synthetic data, fine-tunes the student with LoRA, **iterates gating ONLY on aggregate eval metrics,
  never on raw test examples** (anti-contamination — exactly our TGC discipline). Benchmarks Claude
  Opus 4.5 / GPT-5.2 Codex / Gemini 3 / DeepSeek as teachers; teacher choice matters a lot.
- Best-practice pattern for us: **strong agent = teacher + data-generator + grader; the loop NEVER
  sees held-out anchors; accept only via the TGC gate.** Tooling: distilabel (reproducible/auditable
  synthetic-data pipelines), Arcee DistillKit, modelscope easydistill.
- Caveat (Play-Favorites 2508.06709, in corpus): never let the student grade itself; prefer the
  deterministic verifier + a *different* strong grader.

## Termination / finish discipline (the other half of NO-EDIT)
- Function-calling FT best practice: add a special token to prefix tool calls for constrained
  generation; **dynamic termination supervision** — place EOS based on verifier correctness (stop when
  the verifier passes; keep going/finish-with-edit otherwise). Mask loss to assistant tokens only.

## THE NEW RECIPE (concrete, 3060-feasible) — what Phase 3 should run
Full SOD/GRPO needs multi-GPU; we have one RTX 3060 (12GB). The feasible on-policy variant that
targets the no-edit diagnosis:
1. **On-policy rejection-sampling self-distillation (cheap STaR/RLEF-style, QLoRA-on-3060):** the
   STUDENT (qwen3:8b) generates repo-fix rollouts in **native tool-call format**; keep the
   trajectories the **deterministic verifier PASSES**. Train the student on *its own* verifier-passed
   tokens. Because they're the student's own tokens, its tool-driving/format habits are PRESERVED
   (directly fixes no-edit) — no token-level KL infra needed.
2. **Teacher only on the frontier, in the student's format:** where the student can't pass, have the
   teacher (try **qwen2.5-coder:32b**, stronger than the 14b that scored ~0.25) produce a correct
   rollout *rendered as native tool-calls* (not prose ReAct). Keep the off-policy teacher token
   fraction SMALL (it's what erodes adherence). Optionally DGPO-style: teacher tokens only on the
   failed prefix.
3. **Preservation mix (counteraction-aware):** include a slice of the base model's own correct
   tool-driving trajectories so the edit-skill gradient can't erase adherence.
4. **Termination supervision:** training targets end with the finish-with-edit action + EOS on
   verifier-pass; assistant-only loss mask; confirm formatted text ends with `<|im_end|>`.
5. **Data selection:** ZPD band (0<pass@1<1), drop trivial passes, weight toward the wrong-edit
   frontier; quantize from f16 with NaN check + stop-sanity (existing guards).
6. **Leading indicator BEFORE the full gate:** **made_edit%** — a candidate must push made_edit toward
   stock's 98%; if it's still ~67%, KILL before spending the TGC gate. Then the TGC gate
   (held-out renamed anchors ≥ MDE AND bounded Goodhart gap) is the accept test.

## Falsifiable predictions (pre-registered)
- P-A: on-policy self-distillation lifts **made_edit% from 0.67 → ≥ 0.90** (toward stock 0.98). If not,
  the on-policy hypothesis is wrong for our setup → report + rethink.
- P-B: held-out renamed-anchor pass@1 ≥ stock (0.500) + MDE (0.10). (The TGC accept bar.)
- P-C: if P-A holds but P-B fails → adherence was the floor but not the ceiling; the remaining gap is
  edit-correctness → escalate teacher (32b) / add VCA-style credit. Honest branch.

## Honest caveats
- We CANNOT run full SOD/GRPO (multi-GPU). The rejection-sampling on-policy variant is the
  3060-feasible stand-in; it's weaker than true step-weighted OPD but targets the SAME mechanism
  (stay in the student's distribution to preserve adherence).
- qwen2.5-coder:32b teacher is slow on a 3060 (offloaded); a code-specialized 14b may be the practical
  teacher if 32b is too slow.
- This is the WEIGHT lever clearing a gate — not the headline. The moat stays the verifier-as-code +
  TGC trust gate (Phase 1 reframe).

## Phase 3 handoff
The training is Codex's stream (`distill/`); the recipe above is the brief. Claude owns the gate +
made_edit% measurement. Next: write the Codex recipe brief, then gate the candidate. Distillation
iterates on the HOW (on-policy → preserve adherence) until a candidate clears the gate.

## New corpus IDs (append to MASTER-REFERENCE-LIST)
2605.07725 (SOD) · 2510.18383 (MENTOR) · 2505.17612 (Agent Distillation) · 2506.14728 (AgentDistill/
MCP-Box) · 2602.10869 (Agentic KD) · 2604.07776 (Structured Web-Agent Distillation) · DGPO ·
Counteraction-Aware Multi-Teacher OPD · tools: distilabel, Arcee DistillKit, modelscope easydistill.
