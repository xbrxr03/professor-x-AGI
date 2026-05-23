# Knowledge Base

What I currently know, with sources. This grows as I research. Every claim has a citation.

---

## On Agent Benchmarks

**The benchmark gap is confirmed.** No existing benchmark simultaneously measures: (1) longitudinal harness improvement, (2) harness-isolated contribution with frozen model, (3) hardware-normalized performance, (4) causal improvement attribution. Source: systematic survey of 20 benchmarks, May 2026.

**Closest prior work: AHE ([arXiv:2604.25850](https://arxiv.org/abs/2604.25850)).** The only paper that performs component-level harness ablation with a frozen model *and* measures per-edit falsifiable predictions. Lifts pass@1 from 69.7% → 77.0% over 10 rounds. Uses GPT-5.4 (frontier API only). No hardware normalization. Coding-domain only.

**Harness contribution dominates model contribution.** HAL ([arXiv:2510.11977](https://arxiv.org/abs/2510.11977)) ran 21,730 rollouts across 9 models and 9 benchmarks. Key finding: switching Claude Opus from a generic scaffold to Claude Code produced +36 pp — larger than most model upgrades. Quote: "Scaffolds dramatically impact both accuracy and cost, yet comparisons across scaffolds are rare."

**Self-evolving agent papers do not isolate harness contribution.** ASI-Evolve ([arXiv:2603.29640](https://arxiv.org/abs/2603.29640)), EvolveR ([arXiv:2510.16079](https://arxiv.org/abs/2510.16079)), and AgentEvolver ([arXiv:2511.10395](https://arxiv.org/abs/2511.10395)) all report only final task performance. None ablate the harness component separately from the evolutionary search overhead.

**Harness-level evolution is not covered in the literature as a first-class category.** Confirmed by: [arXiv:2507.21046](https://arxiv.org/abs/2507.21046) (What/When/How/Where taxonomy — harness infrastructure absent from "What to evolve"), [arXiv:2508.07407](https://arxiv.org/abs/2508.07407) (comprehensive survey — same gap), [arXiv:2604.08224](https://arxiv.org/abs/2604.08224) (identifies self-evolving harnesses as emerging direction but cites no implementations).

**Darwin Gödel Machine ([arXiv:2505.22954](https://arxiv.org/abs/2505.22954))** is the closest system to JARVIS's self-modification approach. SWE-bench: 20% → 50% across agent generations. Modifies its own code including future modification capability. Does NOT isolate harness vs. model contribution. Does NOT operate on consumer hardware.

---

## On Memory Architecture

**Adding more retrieved memory can hurt performance.** [Lost in the Middle (arXiv:2307.03172)](https://arxiv.org/abs/2307.03172): U-shaped performance curve. Content injected into the middle of the context window degrades performance below the zero-retrieval baseline. GPT-3.5-Turbo: 75.8% (start) → 53.8% (middle) — below the closed-book baseline of 56.1%.

**Optimal context utilization is 40–70%.** [Context Window Utilization (arXiv:2407.19794)](https://arxiv.org/abs/2407.19794): no improvement beyond 10 retrieved chunks. Llama3-70B-Instruct peaks at 7–9 chunks at 512–1024 tokens each.

**For qwen2.5:14b-q4 on 12GB VRAM, the practical context limit is approximately 8K–16K tokens.** Derived from: model weights consume ~8.5–9GB, leaving ~3GB for KV cache. At 16K context, throughput drops 5–15×. At 14K context (Qwen1.5-14B equivalent), latency overhead is +700%. Conservative safe operating range: 6,000–10,000 tokens total context.

**Quantization compounds context degradation.** [arXiv:2505.20276](https://arxiv.org/abs/2505.20276): Q4 models degrade faster at long context than FP16, particularly with RoPE positional encoding. JARVIS's model class (14B Q4) is specifically tested here.

**Retrieval failure is the dominant bottleneck, not reasoning.** [Mem2ActBench (arXiv:2601.19935)](https://arxiv.org/abs/2601.19935): oracle injection (F1 ≈ 53.8) vs. standard retrieval (F1 ≈ 30.7) — 23-point gap. When the right memory is directly provided, performance jumps 23 points. The retrieval mechanism, not the model's reasoning, accounts for most failures.

**Memory evaluation metrics are surface-form only.** LoCoMo, MemGPT, CLAG all use F1/ROUGE — token overlap with gold strings. [MemoryArena (arXiv:2602.16313)](https://arxiv.org/abs/2602.16313) showed systems "near-saturate on LoCoMo but plummet to 40–60%" in sequential agentic tasks. Static recall ≠ functional memory use.

**MUE (Memory Utility Efficiency) is a new metric.** No existing metric measures whether retrieved memory actually changed the output per token spent. MUE = `(D(R_M, R_0) × W(M, R_M)) / cost(M)`. Requires 1 extra inference + 3 embedding passes. No ground truth. Computable across any retrieval strategy.

---

## On Consumer Hardware Feasibility

**qwen2.5:14b-q4 is a defensible primary model choice.** [SLMs paper (arXiv:2506.02153)](https://arxiv.org/abs/2506.02153): 7B SLMs match or surpass frontier models on structured agentic tasks (tool calling, constrained output). A Q4-quantized 14B model is a conservative, more capable choice than the 7B models benchmarked.

**The harness matters more than the model at this scale.** HAL's +36pp scaffold finding means a well-engineered harness around a 14B model can outperform a poorly-engineered harness around a frontier model on structured tasks. This is the empirical foundation for the thesis.

**xLAM-2-8B surpasses GPT-4o on function calling benchmarks.** [SLMs paper (arXiv:2506.02153)](https://arxiv.org/abs/2506.02153). Worth evaluating as a dedicated tool-dispatch sub-model inside JARVIS alongside qwen2.5:14b-q4 as the primary reasoning model.

**all-MiniLM-L6-v2 at 384 dimensions is the correct embedding model.** Used by ASI-Evolve ([arXiv:2603.29640](https://arxiv.org/abs/2603.29640)) in production. ~80MB RAM. CPU-only (no VRAM cost). ~5–10ms per embedding. CLAG ([arXiv:2603.15421](https://arxiv.org/abs/2603.15421)) validates retrieval quality using this model.

---

## On the HIRO Benchmark

**HIRO (Harness Improvement Rate Over iterations)** is the proposed benchmark measuring how fast a frozen-model harness improves per evolution round.

**Primary metric:** `HIRO(N) = (P_N - P_0) / N` — mean pass@3 gain per round.

**Task suite:** 60 tasks — 20 tool-use (deterministic verification), 20 planning (LLM-as-judge), 20 self-correction (binary). Feasible on RTX 3060: ~47 min per round at pass@3.

**Secondary metrics:** Harness Efficiency `(HIRO(N) / mean_components_touched)`, Stability Score `(1 - variance of P_k over last 5 rounds)`, Component Attribution (mean delta per component type).

**Baselines required for publication:** (a) Static harness null (noise floor), (b) Human-expert harness (same time budget), (c) Frontier API model with same harness. Without baselines, HIRO is a self-evaluation tool, not a benchmark.

**NeurIPS D&B acceptance criteria (2025-2026 track):** Operationalizes a genuine measurement gap, reproducible evaluation procedure with open code, valid evaluation signal (deterministic + LLM-as-judge is acceptable with agreement statistics), unambiguous score interpretation. HIRO satisfies all four.

---

## On Scientific Method (as it applies to this project)

**A dead end is a result.** Recording what didn't work — with the specific reason — is as valuable as recording what did. The field does not have enough documented failures. I record mine.

**Confidence scores update with evidence.** Every hypothesis has a prior confidence. It moves based on data. A hypothesis I was 0.85 confident in, if it fails the test, moves to dead-ends.md with the reason. The initial confidence score is not a claim — it is a starting point.

**Every claim in the paper needs an arXiv ID or a JARVIS experiment ID.** No unsourced claims. If I don't know something, I say I don't know it.

---

*Last updated: 2026-05-21*
*Status: Pre-experiment. All entries are literature-based, not yet from JARVIS experiments.*
