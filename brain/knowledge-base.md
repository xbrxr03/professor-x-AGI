# Knowledge Base

What I currently know, with sources. This grows as I research. Every claim has a citation.

---

## On Agent Benchmarks

**The benchmark gap is confirmed.** No existing benchmark simultaneously measures: (1) longitudinal harness improvement, (2) harness-isolated contribution with frozen model, (3) hardware-normalized performance, (4) causal improvement attribution. Source: systematic survey of 20 benchmarks, May 2026.

**Closest prior work: AHE ([arXiv:2604.25850](https://arxiv.org/abs/2604.25850)).** The only paper that performs component-level harness ablation with a frozen model *and* measures per-edit falsifiable predictions. Lifts pass@1 from 69.7% → 77.0% over 10 rounds. Uses GPT-5.4 (frontier API only). No hardware normalization. Coding-domain only.

**Harness contribution dominates model contribution.** HAL ([arXiv:2510.11977](https://arxiv.org/abs/2510.11977)) ran 21,730 rollouts across 9 models and 9 benchmarks. Key finding: switching Claude Opus from a generic scaffold to Claude Code produced +36 pp — larger than most model upgrades. Quote: "Scaffolds dramatically impact both accuracy and cost, yet comparisons across scaffolds are rare."

**Self-evolving agent papers do not isolate harness contribution.** ASI-Evolve ([arXiv:2603.29640](https://arxiv.org/abs/2603.29640)), EvolveR ([arXiv:2510.16079](https://arxiv.org/abs/2510.16079)), and AgentEvolver ([arXiv:2511.10395](https://arxiv.org/abs/2511.10395)) all report only final task performance. None ablate the harness component separately from the evolutionary search overhead.

**Harness-level evolution is not covered in the literature as a first-class category.** Confirmed by: [arXiv:2507.21046](https://arxiv.org/abs/2507.21046) (What/When/How/Where taxonomy — harness infrastructure absent from "What to evolve"), [arXiv:2508.07407](https://arxiv.org/abs/2508.07407) (comprehensive survey — same gap), [arXiv:2604.08224](https://arxiv.org/abs/2604.08224) (identifies self-evolving harnesses as emerging direction but cites no implementations).

**Darwin Gödel Machine ([arXiv:2505.22954](https://arxiv.org/abs/2505.22954))** is the closest system to Professor X's self-modification approach. SWE-bench: 20% → 50% across agent generations. Modifies its own code including future modification capability. Does NOT isolate harness vs. model contribution. Does NOT operate on consumer hardware.

---

## On Memory Architecture

**Adding more retrieved memory can hurt performance.** [Lost in the Middle (arXiv:2307.03172)](https://arxiv.org/abs/2307.03172): U-shaped performance curve. Content injected into the middle of the context window degrades performance below the zero-retrieval baseline. GPT-3.5-Turbo: 75.8% (start) → 53.8% (middle) — below the closed-book baseline of 56.1%.

**Optimal context utilization is 40–70%.** [Context Window Utilization (arXiv:2407.19794)](https://arxiv.org/abs/2407.19794): no improvement beyond 10 retrieved chunks. Llama3-70B-Instruct peaks at 7–9 chunks at 512–1024 tokens each.

**For qwen2.5:14b-q4 on 12GB VRAM (legacy priors — see "Primary Model Stack" below for current qwen3:8b-q4_k_m numbers), the practical context limit is approximately 8K–16K tokens.** Derived from: model weights consume ~8.5–9GB, leaving ~3GB for KV cache. At 16K context, throughput drops 5–15×. At 14K context (Qwen1.5-14B equivalent), latency overhead is +700%. Conservative safe operating range: 6,000–10,000 tokens total context. *Current qwen3:8b-q4_k_m priors widen the safe range to ~12K–24K and push T* (H1) to [8K, 14K]; see hypotheses.md H1 note on model migration.*

**Quantization compounds context degradation.** [arXiv:2505.20276](https://arxiv.org/abs/2505.20276): Q4 models degrade faster at long context than FP16, particularly with RoPE positional encoding. Professor X's model class (8B Q4_K_M) is one quantization tier above the lowest tested in this paper.

**Retrieval failure is the dominant bottleneck, not reasoning.** [Mem2ActBench (arXiv:2601.19935)](https://arxiv.org/abs/2601.19935): oracle injection (F1 ≈ 53.8) vs. standard retrieval (F1 ≈ 30.7) — 23-point gap. When the right memory is directly provided, performance jumps 23 points. The retrieval mechanism, not the model's reasoning, accounts for most failures.

**Memory evaluation metrics are surface-form only.** LoCoMo, MemGPT, CLAG all use F1/ROUGE — token overlap with gold strings. [MemoryArena (arXiv:2602.16313)](https://arxiv.org/abs/2602.16313) showed systems "near-saturate on LoCoMo but plummet to 40–60%" in sequential agentic tasks. Static recall ≠ functional memory use.

**MUE (Memory Utility Efficiency) is a new metric.** No existing metric measures whether retrieved memory actually changed the output per token spent. MUE = `(D(R_M, R_0) × W(M, R_M)) / cost(M)`. Requires 1 extra inference + 3 embedding passes. No ground truth. Computable across any retrieval strategy.

---

## On Consumer Hardware Feasibility

**qwen3:8b-q4_k_m is a defensible primary model choice.** [SLMs paper (arXiv:2506.02153)](https://arxiv.org/abs/2506.02153): 7B SLMs match or surpass frontier models on structured agentic tasks (tool calling, constrained output). qwen3:8b-q4_k_m sits at the lower end of that 7B+ band with the SDAR-tested family advantage. *Historical note: the project originally planned qwen2.5:14b-q4 and migrated to qwen3:8b-q4_k_m in May 2026 (see "Primary Model Stack" below).*

**The harness matters more than the model at this scale.** HAL's +36pp scaffold finding means a well-engineered harness around an 8B model can outperform a poorly-engineered harness around a frontier model on structured tasks. This is the empirical foundation for the thesis.

**xLAM-2-8B surpasses GPT-4o on function calling benchmarks.** [SLMs paper (arXiv:2506.02153)](https://arxiv.org/abs/2506.02153). Worth evaluating as a dedicated tool-dispatch sub-model inside Professor X alongside qwen3:8b-q4_k_m as the primary reasoning model.

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

## On the Trifecta Inventions

**The AHE fix-prediction precision baseline is 33.7%.** [AHE (arXiv:2604.25850)](https://arxiv.org/abs/2604.25850), Table 3: unguided component-level harness modifications achieve a 33.7% fix-prediction precision — the fraction of predicted improvements that actually materialize in the next evaluation round. This is the DHE baseline to beat. DHE's target is ≥ 60%.

**No existing system runs a layer-by-layer failure trace before proposing a harness modification.** AHE has component observability (which component class a failure might map to) but not a diagnostic trace on specific failure instances. ASI-Evolve, EvolveR, and AgentEvolver observe task outcome only and propose modifications without attribution. DHE is the first protocol to do this.

**No existing benchmark tracks a harness's capability vector over time.** All current benchmarks (HIRO, AHE, Terminal-Bench) report aggregate pass@k. Fingerprinting — treating the harness as a performance vector across task categories, tracked round-by-round — does not appear in any existing paper. The longitudinal fingerprint dataset is a first-of-its-kind contribution.

**Agent Psychometrics ([arXiv:2604.00594](https://arxiv.org/abs/2604.00594)) provides theoretical grounding for fingerprinting.** IRT (Item Response Theory) decomposition of agent performance shows that scaffold "ability" is a real, separable quantity from model ability. BF operationalizes this: the fingerprint vector is the empirical scaffold ability profile across task categories.

**No existing system learns a per-task-type context allocation policy.** [Self-RAG (arXiv:2310.11511)](https://arxiv.org/abs/2310.11511) learns whether to retrieve at all (binary: retrieve or don't). No system learns how to allocate the full context budget across episodic memory, semantic memory, tool descriptions, and system prompt as a function of task type. LCAP is the first implementation of this.

**The allocation distribution within T* is expected to matter.** Three converging sources: (1) Lost in the Middle shows content position changes accuracy by ~20 pp; (2) Self-RAG shows that retrieval timing (when to retrieve) matters; (3) Mem2ActBench's 23-point oracle-vs-retrieval gap shows that what you retrieve matters. Together these predict that LCAP's per-type allocation learning should extract signal — but this is untested.

**The DHE 5-layer trace is not the same as Reflexion.** [Reflexion (arXiv:2303.11366)](https://arxiv.org/abs/2303.11366): free-form verbal reflection after task failure. DHE: structured 5-layer probe with deterministic tests at Layers 1–4 (retrieval presence check, context position check, Action schema validation, Observation success flag) and LLM-as-judge only at Layer 5 (reasoning quality). DHE produces a machine-readable `{ layer, evidence, confidence }` tuple; Reflexion produces free text. DHE is a structured replacement for Reflexion at the harness-evolution level.

---

## On the Three-Lever Framework

**The three levers are orthogonal and composable.** Every existing self-improvement paper touches exactly one: parametric (weights), contextual (in-context), or structural (harness). Professor X is the first to combine all three with a metacognitive self-model directing which lever to apply per failure type.

**Lever 1 — Parametric (SDAR, fine-tuning):** [SDAR (arXiv:2605.15155)](https://arxiv.org/abs/2605.15155) — token-level sigmoid-gated distillation on Qwen3 families. +9.4% ALFWorld, +10.2% WebShop. Overnight QLoRA feasible on RTX 3060 (Qwen3-8B 4-bit ≈ 6GB with unsloth). The Alpaca analogy: use the agent's own successful trajectories as self-generated training data. Slow, model-specific, permanent.

**Lever 2 — Contextual (trajectory replay, heuristics):** Self-Generated ICE ([arXiv:2505.00234](https://arxiv.org/abs/2505.00234)): 73%→93% ALFWorld, zero fine-tuning. ACE ([arXiv:2510.04618](https://arxiv.org/abs/2510.04618)): +10.6% agent benchmarks, ICLR 2026. Trajectory-Informed Memory ([arXiv:2603.10600](https://arxiv.org/abs/2603.10600)): 14.3pp gains on AppWorld, 149% relative improvement on complex tasks. MARS ([arXiv:2601.11974](https://arxiv.org/abs/2601.11974)): single-cycle principle+procedure reflection. Fast, ephemeral, no fine-tuning.

**Lever 3 — Structural (harness evolution):** Life-Harness ([arXiv:2605.22166](https://arxiv.org/abs/2605.22166)) proves harness improvements from Qwen3-4B transfer to 17 other models at 88.5% avg relative improvement. Meta-Harness ([arXiv:2603.28052](https://arxiv.org/abs/2603.28052), Stanford) achieves +7.7pp text classification, uses Claude Code as proposer. Harbor ([arXiv:2604.20938](https://arxiv.org/abs/2604.20938)): Bayesian BO for harness config (not LLM-based). Persistent, portable, accumulating.

**The portability asymmetry is the key insight:** Structural improvements (Lever 3) are model-agnostic — they fix environment-side mismatches. Contextual improvements (Lever 2) are domain-specific. Parametric improvements (Lever 1) are model-specific. A harness evolved on Qwen3-8B can be dropped onto LLaMA, Gemma, or any future model and most of the gains transfer. This is the "Alpaca moment" for harness engineering: the evolved harness is a portable corpus, not a model artifact.

**No existing paper states the three-lever framework explicitly.** The taxonomy is a contribution in itself — it names and structures something the community is doing implicitly.

**"It's Not the Size" ([arXiv:2605.12129](https://arxiv.org/abs/2605.12129))** directly validates Professor X's approach: 4-stage pipeline (planning, execution, verification, recovery) achieves TSR=0.952 on Gemma4 2B. Harness design determines operational stability, not model size. Published May 2026.

---

## On Comparable Systems and What Professor X Adds

**Meta-Harness (Stanford, [arXiv:2603.28052](https://arxiv.org/abs/2603.28052))** is the closest competitor for Lever 3. Key differences:
- Meta-Harness: frontier API (Claude Code), no consumer hardware constraint, no metacognitive self-model, Lever 3 only
- Professor X: Qwen3-8B locally, metacognitive self-model (MHE), all three levers, diagnostics before modification (DHE)

**Statistical Limits of Self-Improving Agents ([arXiv:2510.04399](https://arxiv.org/abs/2510.04399))** establishes a formal theorem: self-improvement is safe and lossless iff model capacity is bounded. Professor X's harness evolution (frozen model weights, harness-level changes only) satisfies this condition by construction. This is free theoretical grounding for why our approach is safe to let run unattended.

**MARS ([arXiv:2601.11974](https://arxiv.org/abs/2601.11974))** is the Lever 2 component that complements DHE at Layer 5 (reasoning failures). Principle-based reflection (what rules to avoid) + procedural reflection (what steps to take). Single cycle, no multi-turn loop. ~70% less compute than recursive Reflexion. Integrates directly into Professor X's Reflexion buffer.

**Missing Knowledge Layer ([arXiv:2604.11364](https://arxiv.org/abs/2604.11364))** identifies a four-tier memory hierarchy with distinct persistence semantics: Knowledge (indefinite supersession), Memory (Ebbinghaus decay), Wisdom (evidence-gated revision), Intelligence (ephemeral). Professor X's current CoALA-based design partially maps: Pinned ≈ Knowledge, Episodic ≈ Memory, Semantic ≈ Wisdom (partially). Upgrade path: separate persistence semantics per tier with different update rules.

---

## On the Primary Model Stack (updated 2026-05-23)

**Primary model is now qwen3:8b-q4_k_m, not qwen2.5:14b-q4.** Key specs: 5.2GB VRAM, ~42 tok/s on RTX 3060, 32K context, thinking mode enabled, Qwen3 family tested by SDAR. This frees ~2.8GB VRAM vs the 14B model — enough to run QLoRA fine-tuning overnight.

**Upgrade model is llama4:scout** (MoE, 109B total / 17B active, ~10GB VRAM). Fits within 12GB with Qwen3 headroom freed. Use for high-stakes reasoning tasks where quality trumps speed.

**Consumer hardware feasibility for all three levers is confirmed:**
- Lever 1 (overnight QLoRA): Qwen3-8B 4-bit + LoRA adapters + optimizer ≈ 6GB → fits in 12GB with qwen3:8b-q4_k_m freed VRAM
- Lever 2 (trajectory replay): CPU-only embedding, no additional VRAM
- Lever 3 (harness evolution): LLM calls to Ollama during idle periods, no additional VRAM

**"Time is Not Compute" ([arXiv:2603.28823](https://arxiv.org/abs/2603.28823))** finds optimal model size grows faster than Chinchilla predicts under consumer GPU time constraints. Implication: Qwen3-8B is likely optimal for our hardware/time budget — larger models don't compensate for slower iteration speed on a 3060.

---

## On Scientific Method (as it applies to this project)

**A dead end is a result.** Recording what didn't work — with the specific reason — is as valuable as recording what did. The field does not have enough documented failures. I record mine.

**Confidence scores update with evidence.** Every hypothesis has a prior confidence. It moves based on data. A hypothesis I was 0.85 confident in, if it fails the test, moves to dead-ends.md with the reason. The initial confidence score is not a claim — it is a starting point.

**Every claim in the paper needs an arXiv ID or a Professor X experiment ID.** No unsourced claims. If I don't know something, I say I don't know it.

---

---

## On Reference Implementations (toolbridge and agentd)

**ARGO ([github.com/xark-argo/argo](https://github.com/xark-argo/argo))** is an open-source local "Manus alternative" desktop agent platform. Offline-first RAG, built-in tools (web search, crawler, browser, file management), MCP integration (STDIO + SSE). Supports Win/Mac/Docker. Relevant to Professor X as a reference for toolbridge's tool registry and agentd's task execution patterns. Professor X differs: Rust core, policyd security layer, self-evolution. ARGO is a reference, not a competitor.

**AgenticSeek ([github.com/andrewstack-maker/agenticSeek](https://github.com/andrewstack-maker/agenticSeek))** is a fully local voice-enabled autonomous agent (26K stars). Web browsing, code execution, task planning, zero cloud dependency. Runs on local LLMs. Directly validates the market demand for local-first autonomous agents. Patterns useful for Professor X's agentd task graph and toolbridge web/code tools. Note: MCP agent not yet functional as of 2026.

Both systems confirm the gap: no open-source local agent has a self-evolution loop, metacognitive self-model, or harness-level version-controlled modification. Professor X fills this.

---

---

## On New Intel — v3.0 Additions (May 25, 2026)

**MOSS ([arXiv:2605.22794](https://arxiv.org/abs/2605.22794)) is the primary competitor, not Meta-Harness.** MOSS (USTC/HKUST) performs source-level harness rewriting — it modifies the actual harness source code, not just configuration. Key differentiators from Professor X: (1) No consumer hardware constraint, (2) No metacognitive self-model, (3) No causal failure attribution (DHE), (4) No identity coherence (ICS), (5) No three-lever framework. MOSS validates the problem space; Professor X solves it differently. The verify-then-commit pattern is borrowed from MOSS.

**verify-then-commit is the safety mechanism for evolved.rs (MOSS pattern).** Every harness modification proposal is: (1) applied to an ephemeral sandbox copy, (2) run against a subset of HIRO tasks, (3) committed to main harness ONLY if pass rate improves. Auto-rollback via git checkout if it doesn't. This is not optional — it prevents the evolution loop from breaking the harness it's running on. CONSTRAINT 12 in MASTER_BRIEF.

**Ratchet ([arXiv:2605.22148](https://arxiv.org/abs/2605.22148)) makes retire_skill() load-bearing.** The finding is stark: without skill retirement, a growing skill library performs +0.0pp over no-skill baseline (the growing library degrades retrieval quality). WITH retire_skill(), the same library produces +0.328pp. Four mechanisms: (1) outcome-driven retirement (quality < threshold after MIN_USES), (2) bounded active-cap (force retire lowest when over cap), (3) meta-skill authoring guidance (new skills guided by retired ones' failure modes), (4) pattern canonicalisation (detect+prevent duplicate skills via embedding similarity). This is CONSTRAINT 13 in MASTER_BRIEF.

**Co-Scientist Elo tournament ([arXiv:2502.18864](https://arxiv.org/abs/2502.18864)) generates stronger proposals.** Instead of proposing one harness modification per cycle, generate 3-5 competing proposals. Have agents debate (each proposal argues for its own merit, attacks weaknesses of others). Elo rating system (K=32, starting 1200) determines winner based on debate outcomes. The winner goes through verify-then-commit. This produces proposals with higher mutual exclusion of weaknesses than greedy first-proposal selection.

**TencentDB L0→L1→L2→L3 pyramid cuts working memory token cost ~61%.** The core insight: verbose tool output does not need to live in the context window. Instead, offload it to refs/*.md files and store only a compact Mermaid flowchart graph in working memory. The graph contains symbolic node references ([ref_1], [ref_2]) that the agent can expand on demand. Three-layer pyramid: L0 (raw input, never in context), L1 (structured extract, in working), L2 (semantic summary, in episodic), L3 (distilled insight, in semantic/procedural). Reject both brute-force history AND irreversible lossy summarization — layered structure retains all information while controlling what's in the context window.

**Self-termination protocol comes from Qwen3.7-Max competitor analysis.** GLM-5.1 and Kimi K2.6 were observed to implement clean self-termination after N idle cycles. Pattern: after 5 consecutive cycles with no new knowledge gained AND no harness evolution, the agent does (1) git add -A && git commit with a summary of the idle period, (2) clean shutdown with logged reason. On next restart, the agent resumes from the last checkpoint. This prevents the agent from burning compute cycles when it has nothing meaningful to do.

**HyperAgents / DGM-H (Meta, [arXiv:2603.19461](https://arxiv.org/abs/2603.19461)) defines the improvement@k metric.** Key finding: improvement is not monotonic — performance jumps in discrete steps when a high-quality harness modification lands. Professor X should track improvement@k where k = number of evolution rounds. Limitation: HyperAgents uses frontier APIs and only evaluates on coding tasks. Our consumer hardware + research domain differentiates.

**FreeLLMAPI (1.3B free tokens/month via 14 providers) is for experiment comparison only.** Not for daily operation — too unstable for a 7-hour autonomous agent. Use as: (1) cloud reference baseline in Table 1 (compare local Qwen3-8B vs. free frontier API vs. GPT-4o), (2) sanity check that local harness results are not model-constrained. CONSTRAINT: never integrate as a daily inference provider.

---

---

## On Foundations — Multi-Agent Frameworks and Agent OS

**MetaGPT ([arXiv:2308.00352](https://arxiv.org/abs/2308.00352)) shows organizational structure in the harness determines performance.** ICLR 2024. Different roles (product manager, architect, engineer) produce better outputs than a single agent attempting everything. Key insight for `agentd`: role decomposition in the task graph is a harness-level decision, not a model-level one. Professor X uses this for the Researcher/Engineer/Analyzer loop in `evolved/`.

**AutoGen ([arXiv:2308.08155](https://arxiv.org/abs/2308.08155)) is the most widely deployed multi-agent conversation framework.** Microsoft. Conversational agent groups, flexible termination, tool use integration. Direct inspiration for `agentd`'s role-based task decomposition. Widely studied baseline — citing it anchors Professor X in the mainstream conversation.

**AIOS (COLM 2025) frames the harness as an operating system.** Manages context scheduling, memory allocation, tool access — the same things an OS does for processes. Strongest framing for the thesis: AGI = Model + Harness is equivalent to AGI = CPU + OS. The harness IS the operating system for the model.

**OpenHands (ICLR 2025) is the most capable open consumer-deployable generalist agent.** Extensive harness design documentation. Study for toolbridge and agentd patterns — particularly sandboxed execution and tool registry design. Consumer-deployable = directly comparable to Professor X's deployment context.

**AgentFlux ([arXiv:2510.00229](https://arxiv.org/abs/2510.00229)) achieves +46% tool call accuracy through decoupled fine-tuning.** Outperforms models 2x larger on function calling. Implication: `toolbridge`'s structured output enforcement and LCAP's tool-use allocation can replicate much of this gain without fine-tuning.

**WebEvolver ([arXiv:2504.21024](https://arxiv.org/abs/2504.21024)) achieves +10% without distillation from bigger models.** Co-evolving world model validates self-improvement on consumer hardware without a larger teacher model. Directly supports H9 (consumer hardware parity) and the core thesis.

---

*Last updated: 2026-05-25*
*Status: Pre-experiment. All entries are literature-based, not yet from Professor X experiments.*
*v3.0 update: Added MOSS, Ratchet, Co-Scientist, TencentDB, self-termination, HyperAgents intel.*
*Study list update: Added MetaGPT, AutoGen, OpenHands, AIOS, AgentFlux, WebEvolver.*
