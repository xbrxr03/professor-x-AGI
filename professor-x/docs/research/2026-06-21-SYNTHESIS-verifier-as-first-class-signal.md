# SYNTHESIS — "The deterministic verifier as a first-class signal across the stack" (2026-06-21)

Four parallel agents hunted genuinely-new mechanisms in quantization, embeddings, RAG, and LM/training.
They independently converged on the SAME meta-idea — which is the real, defensible research program:

## The unifying thesis (the genuinely-new direction)
Almost no LLM system has a **deterministic, executable verifier in the loop** (ground-truth pass/fail).
We do (`check.py` per fixture). The white space is using that verifier **not just to SCORE outcomes, but
as a first-class signal that drives every layer**: quantization, retrieval, embeddings, and per-token credit
assignment. Each agent found that the *novel, unclaimed* mechanism in its domain is exactly "replace the
usual proxy (Hessian/cosine/likelihood/gradient-estimate) with the executable verifier." Same move, four
layers. That pervasive exploitation of an in-loop verifier is the program; the four candidates are instances.

## The four candidates (honest novelty class + buildability)

### 1. VCA — Verifier-Counterfactual Credit Assignment  ★ LEAD (only PRIMITIVE-novel one)
Delta-debug (DDMIN) the agent's OWN green diff — re-run `check.py` on hunk subsets to find the minimal
**necessary-and-sufficient** hunk set — then mask SFT loss to ONLY those causal tokens. The verifier becomes
a **reference-free causal oracle for dense per-token training credit.**
- Closest prior art: **EGCA** (arXiv 2603.16158) — but EGCA needs a **gold reference solution** + **8×A100
  GRPO**; VCA is **reference-free** (ablate own hunks vs verifier) + **plain SFT loss-masking on a 3060**.
  Delta-debugging (Zeller'99) as a *loss-mask front-end* is unpublished. → **primitive-novel on the signal.**
- Buildable NOW (DDMIN loop in Python shelling `check.py` + masked example into `distill/`). No RL, no cluster.
- **$0 pre-check (decisive):** on existing green trajectories, measure causal-hunk *mask sparsity*. If causal
  tokens ≈ the whole diff → VCA ≡ plain assistant-loss SFT → KILL for free. If a clear minority → real signal.
- Risk: sparsity pre-check may fail (small fixes already ~all-causal → NULL, like the auto-repair A/B).

### 2. Verifier-Driven Per-Tensor Quantization  ★ MOST SHOVEL-READY (integration-novel)
Choose each tensor's bit-width to **maximize measured `check.py` pass@1** under a VRAM budget (not weight
MSE/perplexity). Closest: HAQ/TAQ/NVIDIA-AutoQuant all use **proxies**; none uses an executable functional
verifier on a generative agentic task. → unclaimed.
- Buildable **TODAY, zero C++ changes**: our `llama-quantize` already has `--tensor-type-file` + `--prune-layers`.
- **Cheap decisive test:** demote coarse tensor groups (attn/MLP/embed/head) one at a time, measure pass@1 —
  flat ⇒ kill; some crater ⇒ sensitivity real ⇒ greedy budget search. ~6 builds + 6 benches, <1 day.
- Risk: overfit (use held-out split), coarse MDE at 88 fixtures, recipe-gate could mask effects. NOT to be
  framed around free-energy (that module is a stub).

### 3. VGTS — Verifier-Grounded Transfer-Survival Embedding (integration-novel; benchmark-blocked)
Learn a distance that predicts "will this retrieved solution still PASS the new task's verifier after
adaptation," trained on **binary verifier transfer outcomes**. Delta vs REPLUG: no gold answer, deterministic
binary label. Cheap (MLP head on frozen nomic). **Blocked:** needs solution-reuse *families* in the benchmark
(50 fixtures likely ~0 cross-task transfer). $0 pre-check: measure transfer-positive-rate first.

### 4. Re-Verified Retrieval (RAG) (integration-novel; transfer-risk)
Re-run a retrieved memory's OWN verifier (+ a mutated variant) against the current task BEFORE injecting →
context is correctness-checked at recall, never stale/hallucinated. Novel: execution-grounding moved UPSTREAM
into retrieval (not post-generation like AgentForge). **$0 pre-check:** how often does the similarity-top
memory's verifier discriminate on the target — if ~0, dead. Risk: a stored `check.py` won't import against a
different task (the "mutated variant" is where the research risk lives; may collapse to "rewrote a generic test").

## Honest ranking → what to actually do
- **Two are buildable NOW with $0 pre-checks and NO benchmark-reuse dependency: VCA (#1) and Quant (#2).**
- **VCA is the strongest novelty** (the only *primitive-novel* candidate — reference-free verifier-causal
  per-token credit has no published equivalent; EGCA, the nearest, needs the two things our asset removes).
- **Quant is the most shovel-ready** (zero new code, exploits existing flags, same-day falsification).
- VGTS (#3) and Re-Verified RAG (#4) are real but **gated on the headroom benchmark / cross-task transfer** —
  the same wall every candidate keeps hitting → reinforces benchmark-building as the keystone.

## Immediate next step (nearly free, decisive, run all three pre-checks before any training/GPU)
1. **VCA sparsity pre-check** — DDMIN existing green trajectories; is the causal-hunk mask a real minority?
2. **Quant group-sensitivity** — demote 4 coarse tensor groups, measure pass@1; is sensitivity real?
3. **Transfer-positive-rate** — does any solved fixture's solution re-pass another's verifier? (gates VGTS+RAG)
Each is cheap and tells us which candidate survives before investing. **VCA + Quant are the two to pursue;
the verifier-as-first-class-signal thesis is the genuinely-new program tying them together.**

Sources: per the four agent reports (EGCA 2603.16158, SIA 2605.27276, TAQ 2511.06516, HAQ 1811.08886, REPLUG,
RAGFix, AgentForge, delta-debugging Zeller'99, SuRe 2511.22367 — full URLs in the agent transcripts).
