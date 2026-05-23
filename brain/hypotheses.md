# Hypotheses

Active hypotheses under investigation. Every entry is a falsifiable prediction with a proposed test.
Confidence scores are priors — they update as experiments run. If I don't know something, I say so here rather than in the paper.

Format: Statement → Evidence → Test → Success Criteria → Status

---

## H1 — Memory Injection Threshold

**Statement:** For qwen2.5:14b-q4 on RTX 3060 12GB, there exists a context token threshold T* in [6,000 – 10,000] tokens above which injecting additional retrieved memory *hurts* task performance relative to injecting nothing.

**Why this matters:** If confirmed, every agent memory architecture paper that recommends "retrieve more to improve performance" is wrong for quantized consumer hardware. The implication is that memory compression and selectivity are not nice-to-haves — they are required.

**Evidence (prior to testing):**
- [Lost in the Middle (arXiv:2307.03172)](https://arxiv.org/abs/2307.03172): U-shaped performance curve in 13B models. Middle-injected content can drop accuracy *below* the zero-retrieval baseline (56.1%).
- [Context Window Utilization (arXiv:2407.19794)](https://arxiv.org/abs/2407.19794): Optimal RAG utilization is 40–70% of context window. No improvement beyond 10 chunks.
- [Quantization × Context (arXiv:2505.20276)](https://arxiv.org/abs/2505.20276): Q4 quantization compounds context degradation faster than FP16. JARVIS's specific model class (14B Q4) degrades at lower context lengths than the base papers tested.
- VRAM analysis: At Q4_K_M on 12GB, 14B model weights consume ~8.5–9GB, leaving ~3GB for KV cache. At 16K context, throughput drops 5–15×. Conservative practical limit: 8K–16K tokens before throughput collapse.

**Proposed test:**
Run 30 fixed tasks (from the HIRO task suite) across 8 context injection levels: 0, 500, 1000, 2000, 4000, 6000, 10000, 16000 tokens of retrieved memory. Measure pass@3 at each level. Plot curve. Identify inflection point T*.

**Success criteria:** Statistically significant (p < 0.05) performance degradation when context exceeds T* versus optimal T*. T* falls within the predicted [6000, 10000] window.

**Confidence:** 0.85

**Status:** Untested. This is experiment 1 — it should run before any other memory experiment because its result constrains the safe operating range for all other hypotheses.

---

## H2 — Cerebellum Bypass

**Statement:** For procedural skills with verification score > 0.85, bypassing the LLM entirely and executing the skill directly reduces task latency by > 50% and VRAM consumption by > 15% with no measurable degradation in task success rate.

**Why this matters:** Every LLM call for a task the harness already knows how to do is wasted compute. If verified skills can execute without the LLM, that compute goes to the KV cache, which makes every *non-routine* task smarter.

**Evidence (prior to testing):**
- [MEMORY_ARCHITECTURE.md](../MEMORY_ARCHITECTURE.md): proposal from early harness analysis.
- [Voyager (arXiv:2305.16291)](https://arxiv.org/abs/2305.16291): verified skills with critic validation. Critic catches failures before commitment.
- Standard engineering argument: a verified deterministic function does not need language model routing.

**Proposed test:**
Create 10 verified skills (verification score > 0.85). Construct 100 tasks that invoke these skills. Run two conditions: (a) LLM routes to and invokes skill normally, (b) router bypasses LLM and executes skill directly. Measure mean latency, VRAM delta, and pass@1.

**Success criteria:** Latency reduction > 50%, VRAM reduction > 15%, pass@1 difference < 3 percentage points (not statistically significant).

**Confidence:** 0.70

**Status:** Untested.

---

## H3 — Memory-as-Tool vs. Passive Injection

**Statement:** Giving the LLM a `memory.query(query: str) -> str` tool call produces higher Memory Utility Efficiency (MUE) than passively injecting top-k retrieved results, because the model retrieves only when it needs to.

**Why this matters:** Passive injection assumes the harness knows what the agent needs before it starts reasoning. That's often wrong. A query tool lets the agent decide what it needs mid-reasoning, reducing wasted context and retrieval noise.

**Evidence (prior to testing):**
- [Self-RAG (arXiv:2310.11511)](https://arxiv.org/abs/2310.11511): adaptive retrieval outperforms always-retrieve.
- [Mem2ActBench (arXiv:2601.19935)](https://arxiv.org/abs/2601.19935): oracle injection (F1 ≈ 53.8) vs. standard retrieval (F1 ≈ 30.7) — 23-point gap. Retrieval is the dominant bottleneck, not reasoning. This suggests the *what* and *when* of retrieval matter as much as the retrieval model itself.
- MUE analysis: passive injection often results in near-zero output divergence (the memory was injected but did not change the response), meaning MUE ≈ 0 per wasted token.

**Proposed test:**
Run 60 HIRO tasks under two strategies: (a) passive injection of top-5 episodic/semantic results at context-build time, (b) `memory.query()` available as a tool call, no passive injection. Compute MES (mean MUE across queries) for each. Measure pass@3.

**Success criteria:** Strategy (b) MES > strategy (a) MES by at least 0.1. Pass@3 not degraded significantly.

**Confidence:** 0.65

**Status:** Untested.

---

## H4 — Surprise-Based Episodic Logging

**Statement:** Filtering episodic writes to store only observations with semantic divergence > 0.3 from existing entries reduces episodic store size by > 50% while maintaining retrieval Evidence F1 within 5 percentage points of full logging.

**Why this matters:** Most of what an agent does on a typical day is routine. Logging routine observations pollutes the episodic store with redundant signal, degrading retrieval precision and increasing cluster sizes unnecessarily.

**Evidence (prior to testing):**
- [MEMORY_ARCHITECTURE.md](../MEMORY_ARCHITECTURE.md): proposal from early harness analysis.
- Information-theoretic argument: a memory that stores only new information has higher per-entry signal-to-noise than one that stores everything.
- [CLAG (arXiv:2603.15421)](https://arxiv.org/abs/2603.15421): cluster contamination degrades retrieval in SLMs specifically.

**Proposed test:**
Run JARVIS for 7 days with full logging (baseline). Then replay the same task sequence with the surprise filter (cosine distance threshold 0.3 — only store if most similar existing entry has distance > 0.3). Compare: episodic store entry count, Evidence F1 on 30 fixed test queries, task pass@3.

**Success criteria:** Entry count reduction > 50%. Evidence F1 drop < 5 pp. Pass@3 within 3 pp of baseline.

**Confidence:** 0.60

**Status:** Untested.

---

## H5 — Autonomous Harness Evolution Matches Human Engineering

**Statement:** JARVIS's autonomous harness evolution over 30 days will produce a HIRO(30) score within 0.015 of a baseline where a human expert manually improves the harness for equivalent calendar time.

**Why this matters:** This is the thesis claim. If autonomous evolution can match human engineering — on consumer hardware, with a small model — that is a genuine result. The comparison is honest: same time budget, same starting harness, same task distribution. We are not claiming the agent is smarter than the human. We are claiming the agent is *comparably effective* without the human.

**Evidence (prior to testing):**
- [HAL (arXiv:2510.11977)](https://arxiv.org/abs/2510.11977): Scaffold change from generic to Claude Code lifted accuracy from 42% → 78% — 36 points from harness engineering alone, larger than most model upgrades. Human engineers produce this. JARVIS aims to produce the same autonomously.
- [AHE (arXiv:2604.25850)](https://arxiv.org/abs/2604.25850): 10 automated rounds lifted pass@1 from 69.7% → 77.0% on Terminal-Bench 2. This is machine-produced harness improvement. The question is whether it matches what a human would do in the same time window.
- No paper has run this comparison. This is genuinely unknown.

**Proposed test:**
Run HIRO(30) with three conditions: (a) JARVIS autonomous evolution, (b) human expert improving harness 30 minutes per day for 30 days (equivalent effort estimate), (c) static harness (no evolution, null hypothesis). Compare HIRO(30) scores.

**Success criteria:** |HIRO_JARVIS - HIRO_human| < 0.015. HIRO_JARVIS >> HIRO_static (p < 0.05).

**Confidence:** 0.45 — genuinely uncertain. I think JARVIS will be competitive. I do not know if it will match a skilled human. That is the experiment.

**Status:** This is the core experiment. It runs after H1–H4 are resolved, because the harness under test should already incorporate whatever memory architecture best practices H1–H4 establish.

---

## H6 — Temporal Compression Preserves Retrieval Quality

**Statement:** Nightly semantic compression of episodic entries older than 7 days — replacing raw entries with cluster summary representations — maintains retrieval Evidence F1 within 10 percentage points of the uncompressed store while reducing storage by > 70%.

**Why this matters:** A 24/7 agent accumulates entries continuously. Without compression, retrieval degrades as the store grows (more noise, higher cluster sizes, slower search). Compression should happen while the agent sleeps — it's the equivalent of memory consolidation during sleep in human cognition.

**Evidence (prior to testing):**
- [MEMORY_ARCHITECTURE.md](../MEMORY_ARCHITECTURE.md): proposal.
- [CLAG (arXiv:2603.15421)](https://arxiv.org/abs/2603.15421): cluster profiles efficiently represent cluster content.
- [EvolveR quality decay formula (arXiv:2510.16079)](https://arxiv.org/abs/2510.16079): `(success+1)/(use+2)` — old unused entries should decay in influence regardless.
- [MemBench (arXiv:2506.21605)](https://arxiv.org/abs/2506.21605): accuracy degrades at 100K vs. 10K tokens — demonstrates that store size hurts quality.

**Proposed test:**
Run JARVIS for 14 days with full logging. After day 7, apply nightly compression (K-Means cluster → profile replacement for entries older than 7 days). Measure Evidence F1 on 30 fixed test queries at days 7, 10, 14 (before compression, mid-compression, fully compressed). Compare entry count.

**Success criteria:** Evidence F1 drop < 10 pp at day 14 vs. day 7. Entry count for pre-compression window reduced by > 70%.

**Confidence:** 0.65

**Status:** Untested.

---

## H7 — Self-Distilled Principles Outperform Manual Prompting

**Statement:** For recurring task types, strategic principles distilled by qwen2.5:14b-q4 from its own failure trajectories will outperform hand-written system prompt guidance on the same tasks by at least 5 percentage points in pass@3.

**Why this matters:** If true, it means the agent should *write its own instructions* rather than have a human write them. This has direct implications for how people should operate agents: stop hand-crafting system prompts; let the agent distill its own experience.

**Evidence (prior to testing):**
- [EvolveR (arXiv:2510.16079)](https://arxiv.org/abs/2510.16079): "cognitive alignment" — at 3B+ parameter scale, self-distilled principles outperform teacher-distilled ones. qwen2.5:14b-q4 is well above this threshold.
- [GEPA (arXiv:2507.19457)](https://arxiv.org/abs/2507.19457): reflective prompt evolution outperforms RL for agent improvement.
- Reflexion (arXiv:2303.11366): verbal self-reflection improves next-attempt performance in bounded trials.

**Proposed test:**
Select 20 recurring task types. For each: (a) write a 3-5 sentence system prompt guideline by hand, (b) run 10 failed attempts with no guidance, distill 3-5 principles using the EvolveR method `s(p) = (success+1)/(use+2)`, use those principles in lieu of the hand-written guidance. Measure pass@3 for each condition.

**Success criteria:** Self-distilled condition pass@3 exceeds hand-written condition by > 5 pp.

**Confidence:** 0.70

**Status:** Untested.

---

## H8 — Component Attribution: Tool Descriptions Outperform System Prompts

**Statement:** Modifications to tool descriptions and memory architecture produce higher per-round HIRO improvement than system prompt modifications, and system prompt modifications have the highest regression risk.

**Why this matters:** If confirmed, harness engineers (and evolved's Researcher module) should prioritize tool-level and memory-level changes over system prompt changes. This is a prescriptive finding that changes how people should evolve agent harnesses.

**Evidence (prior to testing):**
- [AHE Table 3 (arXiv:2604.25850)](https://arxiv.org/abs/2604.25850): component ablation with fixed model shows: long-term memory +5.6 pp, tools +3.3 pp, middleware +2.2 pp, system prompt **-2.3 pp** (regression). System prompt evolution was the only component that degraded performance on average.
- [Meta-Harness (arXiv:2603.28052)](https://arxiv.org/abs/2603.28052): 6× performance gap from harness change alone — primarily from tool and context engineering, not system prompt.

**Proposed test:**
This is measured automatically during HIRO. After 10+ rounds, compute the mean next-round pass@1 delta for rounds where each component type was modified:
- Mean delta | system_prompt_modified
- Mean delta | tool_description_modified
- Mean delta | memory_architecture_modified
- Mean delta | skill_definition_modified

**Success criteria:** Mean delta for tool and memory modifications is higher than mean delta for system prompt modifications. System prompt rounds show highest variance (most regressions).

**Confidence:** 0.70

**Status:** Will be measured automatically during HIRO. No separate experiment needed.

---

## H9 — Consumer Hardware HIRO Parity with Frontier APIs

**Statement:** JARVIS running qwen2.5:14b-q4 on RTX 3060 12GB will achieve a HIRO(20) score within 0.03 of the same harness running against a frontier API (GPT-4o or Claude Sonnet), demonstrating that the harness, not the model, dominates HIRO.

**Why this matters:** This is the consumer hardware claim. If confirmed, it means the hardware gap between a $400 GPU and a frontier API subscription is closeable through harness engineering. That is a direct challenge to the assumption that better AI requires more compute.

**Evidence (prior to testing):**
- [HAL (arXiv:2510.11977)](https://arxiv.org/abs/2510.11977): scaffold accounted for a 36-point swing — larger than the difference between most model tiers. If the harness is the dominant factor, then JARVIS's harness quality matters more than the model gap.
- [SLMs paper (arXiv:2506.02153)](https://arxiv.org/abs/2506.02153): 7B SLMs match frontier on structured agentic tasks with guided decoding and schema enforcement.
- [Agent Psychometrics (arXiv:2604.00594)](https://arxiv.org/abs/2604.00594): IRT decomposition suggests scaffold "ability" is a real, separable quantity from model ability.

**Proposed test:**
Run HIRO(20) twice: (a) JARVIS on RTX 3060 with qwen2.5:14b-q4, (b) same JARVIS harness with model endpoint swapped to Claude Sonnet or GPT-4o API. Same task set, same evolution budget, same starting harness. Compare HIRO(20) scores.

**Success criteria:** |HIRO_local - HIRO_frontier| < 0.03. If the harness dominates, the scores should be close.

**Confidence:** 0.40 — speculative. The model still matters. This is the bold hypothesis.

**Status:** Untested. Runs last — requires H5 to be resolved first.

---

## Hypothesis Dependency Graph

```
H1 (context threshold)
  └── constrains safe memory budget for all other hypotheses

H2 (cerebellum bypass)    ← independent, run early
H3 (memory-as-tool)       ← depends on H1 for safe context budget
H4 (surprise logging)     ← independent, run early
H6 (temporal compression) ← run after 7+ days of operation

H7 (self-distilled principles) ← run after 10+ task failures accumulated
H8 (component attribution)     ← measured automatically during HIRO

H5 (autonomous vs human)  ← run after H1–H4 resolved
H9 (consumer hardware parity) ← run after H5
```

---

## What Makes a Hypothesis Dead

A hypothesis is moved to [dead-ends.md](dead-ends.md) if:
- The test ran and the result was not statistically significant in either direction
- The test could not be designed in a way that isolates the variable
- The result was confounded and the confound cannot be controlled

I record dead ends with the same detail as confirmed hypotheses. A dead end is a result.

---

*Last updated: 2026-05-21*
*Status: All hypotheses untested — pre-experiment phase*
