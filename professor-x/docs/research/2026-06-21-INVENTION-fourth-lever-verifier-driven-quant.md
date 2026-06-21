# INVENTION: Verifier-Driven Quantization — the FOURTH self-improvement lever (2026-06-21)

Cross-referenced against the existing portfolio (`brain/inventions.md`): MHE has THREE levers — parametric
(weights), contextual (memory/ICE), structural (harness). **None touches the model's own PRECISION.** This is
a fourth, orthogonal lever, and it is the one moonshot candidate that is (a) genuinely outside the existing
portfolio, (b) unblocked by the benchmark gap, (c) buildable today. Applied skills: px-experiment-runner,
verify-the-ruler, adversarial-self-review, px-know-scientific-method.

## One-sentence claim
Professor X improves itself by **re-quantizing its own weights to maximize *measured verifier pass@1* under
its fixed VRAM budget** — bit-allocation as a self-improvement lever, driven by an executable functional
verifier instead of a proxy (Hessian/KL/activation/gradient sensitivity).

## Why it's novel (adversarial-self-review applied)
- Mixed-precision quant exists (HAQ CVPR'19; TAQ arXiv 2511.06516; NVIDIA AutoQuantize) — **all rank tensors
  by PROXIES** (gradient/activation/output-distribution sensitivity), never by an executable functional
  verifier, and never on a *generative agentic* task.
- "Can Compressed LLMs Truly Act?" (arXiv 2505.19433) *evaluates* agentic capability after quant but does
  NOT use it to *drive* allocation.
- Within Professor X: MHE/DFA/IPE never treat precision as a lever. The "$400 GPU" identity makes precision a
  first-class resource — so optimizing it against the verifier is squarely on-thesis and unclaimed.
- Honest novelty class: **integration-novel** (delta-debugging-style search + existing `--tensor-type` flags +
  the verifier), not a new algorithm. Defensible only if the experiment shows verifier-measured tensor
  sensitivity is real and beats uniform Q4_K_M at equal VRAM.

## Why it fits the paper's frame (extends MHE, doesn't compete with it)
MHE = parametric + contextual + structural. **Fourth lever = precision/representational.** It's the most
consumer-hardware-native lever of all (it directly trades the agent's scarce resource — VRAM — for verified
capability), and it shares MHE's metacognitive shape: measure (verifier) → attribute (which tensors matter)
→ act (re-quantize) → re-measure. It can even be folded into the metacognitive self-model: "I am most
capability-sensitive in my attention tensors, so I keep those high-precision."

## Measurable claim
A verifier-driven per-tensor bit allocation achieves **≥ the pass@1 of uniform Q4_K_M at equal-or-lower VRAM**
(ideally: recovers pass@1 at smaller size, or raises pass@1 at equal size by protecting the sensitive tensors
and aggressively compressing the rest).

## Falsifiable experiment (running now — px-experiment-runner)
PRE-CHECK (decisive, cheap): build baseline Q4_K_M, plus attn→Q2_K and ffn→Q2_K variants; serve each; measure
native repo-fix pass@1. **Success criterion (fixed up-front):** if both demotions land within ~1 task of
baseline → FLAT → tensor sensitivity is not exploitable → KILL. If one craters while another holds → sensitivity
is REAL → proceed to a greedy per-tensor budget search (start all-Q8, demote least-sensitive tensors until VRAM
budget, keep demotions that don't drop pass@1). Artifacts in `distill/quant_probe/`, log `/tmp/quant_probe.log`.

## Honest risks (verify-the-ruler + the agent's flags)
- **Capability-gate confound:** if the (distilled) model can't clear the format gate, quant effects are masked
  — run baseline sanity first; if the model can't act, fix the recipe before believing quant deltas.
- **Overfitting:** split search/held-out fixtures; report held-out pass@1 only (M0 discipline — this project
  has shipped fabricated wins).
- **Coarse MDE:** ~30 fixtures, pass@1 moves in ~0.033 steps; use a strong demotion (Q2) so a real effect
  exceeds noise, and K passes for the final search.
- **Not framed around free-energy** (that module is a stub; and free-energy is already claimed by IPE).

## If validated
This is a clean, new, paper-worthy contribution that EXTENDS the existing three-lever paper to four levers,
is uniquely enabled by our deterministic verifier + consumer-HW constraint, and needs no new benchmark
(unlike VGTS/RAG/VCA). Add to `brain/inventions.md` as "Lever 4 — Representational (verifier-driven quant)."
