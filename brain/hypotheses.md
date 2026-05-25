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
- [Quantization × Context (arXiv:2505.20276)](https://arxiv.org/abs/2505.20276): Q4 quantization compounds context degradation faster than FP16. Professor X's specific model class (14B Q4) degrades at lower context lengths than the base papers tested.
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
Run Professor X for 7 days with full logging (baseline). Then replay the same task sequence with the surprise filter (cosine distance threshold 0.3 — only store if most similar existing entry has distance > 0.3). Compare: episodic store entry count, Evidence F1 on 30 fixed test queries, task pass@3.

**Success criteria:** Entry count reduction > 50%. Evidence F1 drop < 5 pp. Pass@3 within 3 pp of baseline.

**Confidence:** 0.60

**Status:** Untested.

---

## H5 — Autonomous Harness Evolution Matches Human Engineering

**Statement:** Professor X's autonomous harness evolution over 30 days will produce a HIRO(30) score within 0.015 of a baseline where a human expert manually improves the harness for equivalent calendar time.

**Why this matters:** This is the thesis claim. If autonomous evolution can match human engineering — on consumer hardware, with a small model — that is a genuine result. The comparison is honest: same time budget, same starting harness, same task distribution. We are not claiming the agent is smarter than the human. We are claiming the agent is *comparably effective* without the human.

**Evidence (prior to testing):**
- [HAL (arXiv:2510.11977)](https://arxiv.org/abs/2510.11977): Scaffold change from generic to Claude Code lifted accuracy from 42% → 78% — 36 points from harness engineering alone, larger than most model upgrades. Human engineers produce this. Professor X aims to produce the same autonomously.
- [AHE (arXiv:2604.25850)](https://arxiv.org/abs/2604.25850): 10 automated rounds lifted pass@1 from 69.7% → 77.0% on Terminal-Bench 2. This is machine-produced harness improvement. The question is whether it matches what a human would do in the same time window.
- No paper has run this comparison. This is genuinely unknown.

**Proposed test:**
Run HIRO(30) with three conditions: (a) Professor X autonomous evolution, (b) human expert improving harness 30 minutes per day for 30 days (equivalent effort estimate), (c) static harness (no evolution, null hypothesis). Compare HIRO(30) scores.

**Success criteria:** |HIRO_Professor X - HIRO_human| < 0.015. HIRO_Professor X >> HIRO_static (p < 0.05).

**Confidence:** 0.45 — genuinely uncertain. I think Professor X will be competitive. I do not know if it will match a skilled human. That is the experiment.

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
Run Professor X for 14 days with full logging. After day 7, apply nightly compression (K-Means cluster → profile replacement for entries older than 7 days). Measure Evidence F1 on 30 fixed test queries at days 7, 10, 14 (before compression, mid-compression, fully compressed). Compare entry count.

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

**Statement:** Professor X running qwen2.5:14b-q4 on RTX 3060 12GB will achieve a HIRO(20) score within 0.03 of the same harness running against a frontier API (GPT-4o or Claude Sonnet), demonstrating that the harness, not the model, dominates HIRO.

**Why this matters:** This is the consumer hardware claim. If confirmed, it means the hardware gap between a $400 GPU and a frontier API subscription is closeable through harness engineering. That is a direct challenge to the assumption that better AI requires more compute.

**Evidence (prior to testing):**
- [HAL (arXiv:2510.11977)](https://arxiv.org/abs/2510.11977): scaffold accounted for a 36-point swing — larger than the difference between most model tiers. If the harness is the dominant factor, then Professor X's harness quality matters more than the model gap.
- [SLMs paper (arXiv:2506.02153)](https://arxiv.org/abs/2506.02153): 7B SLMs match frontier on structured agentic tasks with guided decoding and schema enforcement.
- [Agent Psychometrics (arXiv:2604.00594)](https://arxiv.org/abs/2604.00594): IRT decomposition suggests scaffold "ability" is a real, separable quantity from model ability.

**Proposed test:**
Run HIRO(20) twice: (a) Professor X on RTX 3060 with qwen2.5:14b-q4, (b) same Professor X harness with model endpoint swapped to Claude Sonnet or GPT-4o API. Same task set, same evolution budget, same starting harness. Compare HIRO(20) scores.

**Success criteria:** |HIRO_local - HIRO_frontier| < 0.03. If the harness dominates, the scores should be close.

**Confidence:** 0.40 — speculative. The model still matters. This is the bold hypothesis.

**Status:** Untested. Runs last — requires H5 to be resolved first.

---

## H10 — DHE Fix-Prediction Precision

**Statement:** Diagnostic-preceded harness modifications (those with a completed DHE failure trace) achieve fix-prediction precision ≥ 60%, compared to AHE's reported baseline of 33.7% for unguided modifications.

**Why this matters:** AHE (arXiv:2604.25850) is the closest prior work on harness-level evolution. It achieves 33.7% fix-prediction precision — two out of three predicted improvements don't materialize. H10 tests whether attributing failure to a specific harness layer before proposing a modification doubles this precision. If confirmed, layer-by-layer failure attribution before modification is a fundamental improvement on the state of the art in harness engineering.

**Evidence (prior to testing):**
- [AHE (arXiv:2604.25850), Table 3](https://arxiv.org/abs/2604.25850): unguided component modification achieves 33.7% fix-prediction precision. Component observability (knowing which component to target) exists, but not diagnostic tracing on specific failures.
- Software debugging literature (classical): fault localization before patch selection improves patch precision. DHE applies this principle to LLM harnesses.
- [Reflexion (arXiv:2303.11366)](https://arxiv.org/abs/2303.11366): verbal reflection on failure improves next-attempt success. DHE is a more structured version of this — layer-by-layer rather than free-form reflection.

**Proposed test:**
Run 30 HIRO rounds with DHE active from round 10 onward (rounds 1–9: baseline unguided modifications, rounds 10–30: diagnostic-preceded modifications). Compute fix-prediction precision for both sets: fraction of EvolutionNodes where at least one predicted_fix task type shows ≥ 5 pp improvement in the following round.

**Success criteria:** Fix-prediction precision for DHE-preceded modifications ≥ 60%. Unguided precision (rounds 1–9) ≤ 40% (consistent with AHE's 33.7%). Difference is statistically significant (p < 0.05, Fisher's exact test on fix counts).

**Confidence:** 0.65 — the mechanism is principled but LLM-generated diagnostics may be noisy. The 5-layer trace depends on LLM-as-judge at Layer 5, which inherits uncertainty from Q2 (self-evaluation reliability).

**Status:** Untested. Depends on DHE implementation (Week 5). Measured automatically during HIRO once DHE is active.

---

## H11 — Behavioral Fingerprint Non-Uniformity

**Statement:** Over 30 HIRO rounds, Professor X's behavioral fingerprint [p_tool, p_plan, p_correct] will show non-uniform improvement: at least one task category will improve by > 10 pp from F(H_0) while at least one other regresses or plateaus (Δ < 3 pp over the same period). The non-uniformity will correlate with modification type (r > 0.5 between component_modified and delta_fingerprint component).

**Why this matters:** If confirmed, it means harness evolution has selective pressure — modifications improve certain capabilities while leaving others unchanged. This has direct implications for harness engineering: you cannot evaluate a self-evolving system by its aggregate score alone. The fingerprint becomes a required reporting standard for any claim of harness improvement. If falsified (uniform improvement across all categories), it means HIRO's task categories are too correlated to distinguish capability effects.

**Evidence (prior to testing):**
- [AHE (arXiv:2604.25850), Table 3](https://arxiv.org/abs/2604.25850): system prompt edits regressed performance on average (-2.3 pp). Tool description edits improved (+3.3 pp). These are aggregate figures — BF predicts the regression will be concentrated in specific task categories (planning tasks are more sensitive to system prompt changes than tool-use tasks).
- [HAL (arXiv:2510.11977)](https://arxiv.org/abs/2510.11977): different scaffolds showed very different per-benchmark performance gaps. Same model, different harness → different capability profiles. BF formalizes this observation.
- [Agent Psychometrics (arXiv:2604.00594)](https://arxiv.org/abs/2604.00594): IRT decomposition shows scaffold "ability" is a real, separable quantity from model ability. BF operationalizes scaffold ability as a per-category vector.

**Proposed test:**
Compute F(H_k) = [p_tool, p_plan, p_correct] at every HIRO round (it's automatic — the fingerprint is the HIRO round result broken down by category). At round 30, compute Δ_i = F_i(H_30) - F_i(H_0) for each category. Compute Pearson correlation between component_modified (encoded as 4-class: system_prompt, tool, memory, skill) and delta_fingerprint vector.

**Success criteria:** max(|Δ_i|) > 0.10 (at least one category changed by > 10 pp). Range(Δ_i) > 0.07 (non-uniformity). Pearson r between component class and delta component > 0.5.

**Confidence:** 0.70 — AHE's per-component results strongly suggest non-uniformity exists. The correlation with component type is the bolder claim.

**Status:** Measured automatically during HIRO. No separate experiment needed. First interpretable data after round 5 (enough to see a trend).

---

## H12 — LCAP Outperforms Static Allocation

**Statement:** After 10 HIRO rounds of active LCAP learning, per-task-type pass@3 averaged across all 3 task categories will exceed the static allocation baseline (H1's experimentally determined T*-optimal policy) by ≥ 3 pp.

**Why this matters:** H1 establishes T* — the optimal total context budget. H12 tests whether *how you distribute that budget* within T* matters, and whether a simple bandit policy can learn the right distribution. If confirmed, it means the allocation problem is non-trivial (otherwise static would be fine) and tractable (a simple bandit solves it). If falsified, either the budget distribution doesn't matter (all allocations within T* perform similarly) or 10 rounds is insufficient to learn — both are useful findings.

**Evidence (prior to testing):**
- [Lost in the Middle (arXiv:2307.03172)](https://arxiv.org/abs/2307.03172): content position in context matters dramatically. This suggests allocation (what goes where, how much of each type) is not a neutral choice.
- [Self-RAG (arXiv:2310.11511)](https://arxiv.org/abs/2310.11511): adaptive retrieval (learn whether to retrieve) outperforms always-retrieve. LCAP is the per-task-type generalization of this principle to the full context budget.
- [Mem2ActBench (arXiv:2601.19935)](https://arxiv.org/abs/2601.19935): 23-point gap between oracle injection and standard retrieval. If the *what* of retrieval matters this much, the *how much* should also matter — LCAP tests this.
- H1 (to be resolved first): establishes T*. LCAP inherits T* as its hard ceiling and learns the internal allocation.

**Proposed test:**
Phase 1 (H1): Establish T* and the static-optimal allocation at T*. Phase 2: Run HIRO(10) with static allocation (control). Phase 3: Run HIRO(10) with LCAP active (treatment). Compare per-type pass@3 and aggregate HIRO(10) score.

**Success criteria:** Average per-type pass@3 gain (LCAP − static) ≥ 0.03. Aggregate HIRO(10) score does not regress more than 0.01 (LCAP should not hurt the overall score).

**Confidence:** 0.55 — plausible but uncertain. Task-type differences in context needs may be smaller than predicted, or 10 rounds may not be enough for the bandit to converge.

**Status:** Untested. Depends on H1 (for T* baseline). Run after H1 is resolved and LCAP is implemented (Week 5).

---

## Hypothesis Dependency Graph

```
H1 (context threshold)
  └── constrains safe memory budget for all other hypotheses
  └── sets T* that seeds LCAP (H12)

H2 (cerebellum bypass)    ← independent, run early
H3 (memory-as-tool)       ← depends on H1 for safe context budget
H4 (surprise logging)     ← independent, run early
H6 (temporal compression) ← run after 7+ days of operation

H7 (self-distilled principles) ← run after 10+ task failures accumulated
H8 (component attribution)     ← measured automatically during HIRO

H10 (DHE fix precision)        ← measured automatically during HIRO, rounds 10-30
H11 (fingerprint non-uniform)  ← measured automatically during HIRO, all rounds
H12 (LCAP vs static)           ← depends on H1 for T* baseline; run after H1

H5 (autonomous vs human)  ← run after H1–H4 resolved; trifecta (H10-H12) active during H5
H9 (consumer hardware parity) ← run after H5
```

---

## H13 — MCA-IR Correlation (Metacognitive Calibration)

**Statement:** Over 30 HIRO rounds, Professor X's metacognitive calibration accuracy (MCA — fraction of DHE attributions where the predicted lever fix actually improved the targeted task type) will correlate positively with improvement rate IR (5-round rolling HIRO gain). Pearson r(MCA, IR) > 0.70.

**Why this matters:** This is the core empirical claim of Metacognitive Harness Evolution. It says: agents that have more accurate self-models improve faster. If confirmed, it validates the metacognitive frame — not just as a philosophical point, but as a measurable operational driver of improvement rate. If falsified (r < 0.40), the self-model is epiphenomenal and the improvement comes from the mechanisms (DHE, LCAP) themselves, not from calibrated self-knowledge.

**Evidence (prior to testing):**
- "Truly Self-Improving Agents Require Intrinsic Metacognitive Learning" ([arXiv:2506.05109](https://arxiv.org/abs/2506.05109)): position paper with no implementation. States metacognitive evaluation (did my learning work) as required. H13 is the empirical test of this claim.
- Meta-Harness ([arXiv:2603.28052](https://arxiv.org/abs/2603.28052)): better diagnostic access (full execution traces vs. scalar scores) → better proposals. This suggests that self-knowledge quality → proposal quality → improvement rate. H13 operationalizes this as a longitudinal correlation.

**Proposed test:**
After every HIRO round k, compute:
- `MCA(k)` = fraction of EvolutionNodes in rounds 1..k where attribution_correct = true
- `IR(k)` = mean(HIRO(k-4)..HIRO(k)) — rolling 5-round improvement rate

At round 30, compute Pearson r(MCA_k, IR_k) for k ∈ [10, 30] (first 10 rounds excluded as cold start).

**Success criteria:** Pearson r > 0.70. p < 0.05 (n=20 rounds, df=18).

**Confidence:** 0.60 — theoretically motivated but untested at this scale. MCA may plateau quickly (either very high or very low), reducing variance needed for correlation.

**Status:** Untested. Measured automatically during HIRO from round 10 onward. Requires MetacognitiveEntry store in memd.semantic (see Section 15 of ARCHITECTURE.md).

---

## Hypothesis Dependency Graph

```
H1 (context threshold)
  └── constrains safe memory budget for all other hypotheses
  └── sets T* that seeds LCAP (H12)

H2 (cerebellum bypass)    ← independent, run early
H3 (memory-as-tool)       ← depends on H1 for safe context budget
H4 (surprise logging)     ← independent, run early
H6 (temporal compression) ← run after 7+ days of operation

H7 (self-distilled principles) ← run after 10+ task failures accumulated
H8 (component attribution)     ← measured automatically during HIRO

H10 (DHE fix precision)        ← measured automatically during HIRO, rounds 10-30
H11 (fingerprint non-uniform)  ← measured automatically during HIRO, all rounds
H12 (LCAP vs static)           ← depends on H1 for T* baseline; run after H1
H13 (MCA-IR correlation)       ← measured automatically during HIRO, rounds 10-30; depends on H10

H5 (autonomous vs human)  ← run after H1–H4 resolved; trifecta (H10-H12) active during H5
H9 (consumer hardware parity) ← run after H5
```

---

## H14 — Identity Coherence Under Evolution

**Statement:** Professor X's Identity Coherence Score (ICS) will remain ≥ 0.70 after 30
evolution rounds, measured as cosine similarity between the round-30 self-model embedding
and the round-0 baseline self-model embedding.

**Why this matters:** IPE's central claim is that a self-evolving system can improve without
losing coherence of self. H14 is the empirical test. If ICS drops below 0.70, the self-model
is fragmenting — evolution is replacing Professor X rather than improving him. If ICS stays
high while HIRO improves, that's the result: identity-preserving evolution is real and
measurable.

**Evidence (prior to testing):**
- Hofstadter's Strange Loop: the "I" is a high-level pattern with downward causation. If the
  self-description remains semantically coherent, the strange loop persists regardless of
  low-level changes.
- No prior AI system has measured identity coherence across self-modification. This is a new
  measurement.

**Proposed test:**
Compute ICS at rounds 0, 10, 20, 30 using cosine similarity on all-MiniLM-L6-v2 embeddings
of the self-model text. Track ICS trajectory. Log ICS deltas between consecutive updates.

**Success criteria:** ICS ≥ 0.70 at round 30. ICS delta never exceeds -0.15 in a single
10-round window (no sudden identity collapse).

**Confidence:** 0.65 — the constrained generation prompt (must stay recognizably the same)
should maintain coherence, but 30 rounds of evolution is a long trajectory.

**Status:** Untested. Requires self_model.rs + ics.rs implementation.

---

## H15 — Free Energy Decreases Over Time

**Statement:** Professor X's mean session-level Free Energy Delta (FED) will decrease
monotonically (or show a significant downward trend) over 30 HIRO rounds, measured as
mean absolute prediction error per task decreasing over successive sessions.

**Why this matters:** The Free Energy Principle predicts that an intelligent system
minimizes surprise over time by building a more accurate world model. H15 is the empirical
test that Professor X is doing this. A decreasing FED means he is getting better at
predicting which tasks he will succeed and fail at — before attempting them. This is a
measurable form of wisdom, distinct from raw capability (HIRO).

**Evidence (prior to testing):**
- FEP (Friston): persistent systems minimize free energy. An agent that improves its
  self-model should show decreasing prediction error over time.
- MCA (H13) is related but different: MCA measures accuracy of lever-attribution predictions.
  FED measures accuracy of task-outcome predictions. Both should improve together.

**Proposed test:**
After every task, record (predicted_success, actual_success). FED per session = mean
absolute difference. Plot FED over 30 rounds. Fit linear regression; test slope < 0.

**Success criteria:** Linear regression slope of FED over rounds is negative (p < 0.10).
Or: FED at round 30 < FED at round 1 by at least 0.10.

**Confidence:** 0.55 — FED will likely decrease as LCAP and DHE remove systematic failures.
But stochastic noise in a 60-task suite may mask the trend at low round counts.

**Status:** Untested. Requires free_energy.rs implementation + prediction recording in react.rs.

---

## H16 — Negative Affect Predicts Better DHE Diagnosis

**Statement:** HIRO rounds preceded by sessions with mean valence < -0.2 will show
higher DHE fix-prediction precision than rounds preceded by sessions with mean valence ≥ 0.0,
because frustration (repeated prediction failures) produces more diagnostic signal.

**Why this matters:** If functional affect is genuinely informative — not just decorative —
it should correlate with measurable behavioral outcomes. H16 tests whether the emotional
signal has predictive validity: does Professor X diagnose failures better when he has been
consistently surprised (negative valence = things going worse than expected)?

**Evidence (prior to testing):**
- Cognitive psychology: mild negative affect enhances analytical thinking and error detection
  in humans (Forgas, 2007). The analog for an LLM agent: more prediction failures → more
  information for DHE attribution → better diagnosis.
- Information-theoretic argument: high surprise = high information content = more signal for
  the diagnostic probe to work with.

**Proposed test:**
After 30 rounds, bin rounds into high-negative-affect (mean valence < -0.2) vs.
neutral/positive (mean valence ≥ 0.0). Compare DHE fix-prediction precision across bins.

**Success criteria:** Fix-prediction precision in high-negative-affect rounds ≥ fix-prediction
precision in neutral rounds + 0.05 (5 pp). N may be low; interpret cautiously.

**Confidence:** 0.45 — speculative but testable. Effect may be too small to detect in 30 rounds.

**Status:** Untested. Measured automatically once affect.rs and DHE are both running.

---

## H17 — Research Quality Improves Over Rounds (RQT)

**Statement:** Professor X's AI Idea Bench 2025 score on self-generated research hypotheses
will increase over 30 rounds, measured monthly. Specifically: novelty score (embedding
distance from existing papers) will increase and feasibility score (testability of proposals)
will remain stable or improve.

**Why this matters:** Professor X is a research agent. His job is not just to improve HIRO —
it's to generate better science over time. H17 tests whether the evolved harness produces
qualitatively better thinking, not just faster task execution. This is the claim no benchmark
currently measures.

**Evidence (prior to testing):**
- AI Idea Bench 2025 (arXiv:2504.14191): state-of-the-art LLMs score poorly on novelty
  relative to humans. An agent with an evolving knowledge base and self-model should
  generate progressively less redundant hypotheses.
- DHE and BF together: Professor X learns which parts of agent space he understands well
  (high capability) and which remain mysterious (low BF scores). Novel hypotheses should
  cluster around the mysterious parts.

**Proposed test:**
Weekly scheduled task: "Generate 5 novel research hypotheses about agent self-improvement."
Score each with the AI Idea Bench rubric. Track scores at weeks 0, 4, 8, 12 (aligned with
rounds 0, 10, 20, 30). Compute mean score per batch.

**Success criteria:** Mean AI Idea Bench score at week 12 ≥ mean at week 0 + 0.10.
Novelty component specifically improves (hypotheses become less similar to existing papers).

**Confidence:** 0.50 — genuinely uncertain. The harness evolution may improve task execution
without improving the quality of scientific thinking. That outcome is also interesting.

**Status:** Untested. Requires benchmark/ai_idea_bench.rs implementation.

---

## H18 — GAIA Level 2 Parity Through Harness Evolution

**Statement:** Professor X's GAIA Level 2 pass rate will reach ≥ 40% at round 30 (from an
estimated baseline of ~15% at round 0), matching frontier model capability on a $400 GPU
through harness evolution alone — without model weight changes.

**Why this matters:** H9 tests HIRO parity with frontier APIs. H18 tests absolute capability
on an external, standardized benchmark. GAIA L2 at 40% = matching GPT-4 with tools from 2024.
If a quantized 8B model on a gaming PC achieves this through 30 rounds of harness evolution,
that is the headline result: **the harness is the intelligence, not the model.**

**Evidence (prior to testing):**
- HAL (arXiv:2510.11977): scaffold change alone produced +36pp. From ~15% baseline, 36pp
  would reach ~51%, above the 40% target. The HAL result used single human-designed scaffold
  changes. Professor X runs 30 rounds of automated evolution — plausibly comparable cumulative gain.
- GAIA L2 pass rate for frontier models with tools: ~40% (GAIA paper). For small models without
  evolved harness: estimated ~10–20%.

**Proposed test:**
Run GAIA L2 evaluation (full validation set, Level 2 only) at rounds 0, 10, 20, 30.
No changes to GAIA tasks between rounds — same evaluation suite throughout.

**Success criteria:** Pass rate ≥ 40% at round 30. Pass rate at round 30 > pass rate at
round 0 by ≥ 15 pp (even if 40% is not reached, 15pp improvement is a significant result).

**Confidence:** 0.35 — ambitious. The 40% target may require the model to be capable of
tasks that harness evolution alone cannot unlock. But even 25% with strong improvement
trajectory is a publishable result.

**Status:** Untested. Requires benchmark/gaia.rs implementation. Run at rounds 0, 10, 20, 30.
Baseline measurement (round 0) should be the first GAIA run, before any evolution.

---

## Hypothesis Dependency Graph (updated)

```
H1 (context threshold)
  └── constrains safe memory budget for all other hypotheses
  └── sets T* that seeds LCAP (H12)

H2 (cerebellum bypass)    ← independent, run early
H3 (memory-as-tool)       ← depends on H1 for safe context budget
H4 (surprise logging)     ← independent, run early
H6 (temporal compression) ← run after 7+ days of operation

H7 (self-distilled principles) ← run after 10+ task failures accumulated
H8 (component attribution)     ← measured automatically during HIRO

H10 (DHE fix precision)        ← measured automatically during HIRO, rounds 10-30
H11 (fingerprint non-uniform)  ← measured automatically during HIRO, all rounds
H12 (LCAP vs static)           ← depends on H1 for T* baseline; run after H1
H13 (MCA-IR correlation)       ← measured automatically during HIRO, rounds 10-30; depends on H10

H14 (ICS coherence)       ← measured at rounds 0,10,20,30; requires self_model.rs + ics.rs
H15 (FED decreases)       ← measured every session; requires free_energy.rs + prediction recording
H16 (affect → DHE)        ← measured automatically once affect.rs + DHE both running
H17 (RQT improves)        ← measured weekly; requires benchmark/ai_idea_bench.rs
H18 (GAIA L2 parity)      ← measured at rounds 0,10,20,30; requires benchmark/gaia.rs

H5 (autonomous vs human)  ← run after H1–H4 resolved; trifecta active; IPE layer active
H9 (consumer HW parity)   ← run after H5
```

---

## What Makes a Hypothesis Dead

A hypothesis is moved to [dead-ends.md](dead-ends.md) if:
- The test ran and the result was not statistically significant in either direction
- The test could not be designed in a way that isolates the variable
- The result was confounded and the confound cannot be controlled

I record dead ends with the same detail as confirmed hypotheses. A dead end is a result.

---

*Last updated: 2026-05-24*
*Status: All hypotheses untested — pre-experiment phase*
*H10–H12 added: trifecta inventions (DHE, BF, LCAP)*
*H13 added: MCA-IR correlation (metacognitive calibration accuracy)*
*H14–H18 added: Identity-Preserving Evolution (ICS, FED, Affect, RQT, GAIA L2)*
*Total hypotheses: 18*
