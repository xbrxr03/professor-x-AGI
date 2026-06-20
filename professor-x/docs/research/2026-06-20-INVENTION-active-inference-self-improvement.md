# INVENTION candidate: Active-Inference Self-Improvement (intrinsic-signal-gated tri-lever consolidation) — 2026-06-20

Honest framing: everything before this (OPD, SkillOpt, GRPO, native tool-calling, "unify the levers")
is *adoption*. SIA (Hexo Labs, arXiv 2605.27276) already does dual-lever (harness OR weights) routed by
an LLM feedback-agent. So unification is NOT novel. This doc proposes something that, after a ~40-source
scan, appears genuinely unclaimed — and is testable, falsifiable, and grounded in code we already have.

## The thesis (what's new)
**Make the agent's own intrinsic prediction-error / expected-free-energy (EFE) the CONTROLLER of its
self-improvement** — not an external verifier outcome (everyone) and not an LLM feedback-agent's
judgment (SIA). Concretely, after each verified solve, the intrinsic signal decides:
1. **WHICH lever** a lesson is written to — by the *signature* of the surprise:
   - surprise about **format/procedure** (the agent knew the fix but mis-executed the tool protocol) → **skill/harness** (SkillOpt),
   - surprise that is **general + recurring knowledge** (a fix pattern that reduces error across many tasks) → **weights** (OPD/GRPO),
   - surprise that is **one-off / contextual** (a fact specific to this repo) → **memory**.
2. **WHAT to consolidate** — only **high-information** (high surprise-reduction) verified trajectories enter
   the training corpus; low-surprise successes are redundant (the model already "knew" it) and are dropped.
   This is active-inference data selection for self-distillation.
3. **WHEN to stop** — when free energy on a held-out set stops decreasing (principled convergence).

Working name: **tri-lever consolidation routed by an intrinsic free-energy signal.**

## Why this is genuinely new (closest prior art + the precise delta)
- **SIA (arXiv 2605.27276):** 2 levers (harness/weights), router = an LLM **feedback-agent reading the
  trajectory and judging** (black-box prompt), on 120B/H100. → **Delta:** we add a **3rd channel (memory)**
  AND replace the LLM-judge router with a **measurable intrinsic signal** (EFE/prediction-error) — principled
  and falsifiable, not vibes; and local.
- **Active inference + LLMs (arXiv 2412.10425; Friston 2023):** free energy used as an **inference-time**
  layer to pick prompts/search actions. → **Delta:** we use it to drive the **learning loop** (which
  permanent update + what to distill), not action selection.
- **Surprise/EFE intrinsic motivation in RL:** used for **exploration** (curiosity) and **action** selection.
  → **Delta:** we use it for **consolidation routing & data selection** in a self-improving *coding* agent.
- **Memory policies (MemTier/AgeMem):** RL'd memory ops, frequency/salience triggers **within memory**. →
  **Delta:** intrinsic-signal-gated **memory→weights compilation** across the boundary (compile high-info
  recurring memories into weights, then free them — true systems consolidation).

No single result in the scan combines: intrinsic-signal router + tri-lever (text/weights/memory) +
self-distillation data selection + local + verifier-gated. That intersection is the invention.

## Why WE can build it (unfair advantage)
We already have the instrument others would have to invent: `src/memd/free_energy.rs` (Free-Energy-Delta),
`src/memd/computational_body.rs` (computational interoception), FED (predicted, actual) pairs flushed in
`react.rs`/`loop_runner.rs`, plus the native-tool-calling structured trajectories (clean per-step signal).
Plus a deterministic verifier (check.py) and a pinned ruler. SIA needs H100s; this targets the 3060.

## The honest risk (and the first experiment is to kill it fast)
Our own consciousness-measurement work found **5/7 seed modules were degenerate** and the φ instrument was
broken before we fixed it. The FED/surprise signal on a quantized 8B may be **too noisy to be a useful
controller**. So the invention is DOA unless the signal is informative. We test that FIRST, cheaply.

## Falsifiable experiment plan (gate each phase on the trustworthy ruler)
- **Phase A — Is the signal real? (cheap, no training)** For the repo-fix trajectories we already collect,
  log per-trajectory free-energy/prediction-error. Test two correlations: (i) does surprise correlate with
  task difficulty (1 − pass-rate)? (ii) does training on **high-surprise** verified traces improve held-out
  pass@1 **more** than training on an equal number of **low-surprise** traces? If neither holds → the
  instrument is degenerate → fix or **abandon the invention** (report honestly). *This is the make-or-break.*
- **Phase B — Is the router real?** Compare 4 consolidation policies on the held-out ruler, equal budget:
  (1) random, (2) outcome-only (all passes — today's flywheel), (3) SIA-style LLM-judge router, (4) our
  intrinsic-signal router. Invention validated only if (4) beats (2) and (3) beyond MDE.
- **Phase C — Tri-lever routing:** measure whether routing by surprise-signature (skill vs weights vs
  memory) beats sending everything to one lever. 

## If it works, the claim
"A local coding agent that improves itself by minimizing its own expected free energy — using an intrinsic
surprise signal (not an external judge) to decide which lesson becomes a weight update, a skill edit, or a
memory, and which experiences are worth learning at all." That is a new *mechanism*, not a recombination —
active inference operationalized as the control law of a self-improving agent, demonstrated on consumer HW.

## Where it slots
This REPLACES the hand-wired routing in the Stage 0–4 plan: Stage 3 (OPD) and Stage 2 (SkillOpt) become the
*actuators*; this intrinsic controller is the *policy* that drives them. Build it after Stage 3 exists (need
a working weights actuator to route to), but run **Phase A now** (it only needs trajectory logging we already
have) to find out if the signal is real before investing.
