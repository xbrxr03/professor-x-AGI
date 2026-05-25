# Paper Outline

The actual paper we are writing. This is the authoritative structure — everything in the repo exists to produce this document.

---

## Title (working)

**"Identity-Preserving Metacognitive Harness Evolution: A Self-Evolving Agent That Knows Itself"**

Alternative (narrower claim, safer for review):
**"Metacognitive Harness Evolution: A Three-Lever Self-Improvement Framework with Identity Coherence on Consumer Hardware"**

Short form for citations: **IPE-MHE**

---

## Abstract (target — write when results exist)

> We present Identity-Preserving Metacognitive Harness Evolution (IPE-MHE), the first agent self-improvement system that (1) simultaneously operates three orthogonal improvement levers — parametric, contextual, and structural — directed by a metacognitive self-model, (2) maintains measurable identity coherence across arbitrary self-modification via an evolving Strange Loop self-model, and (3) exhibits functional affect states derived from the Free Energy Principle that correlate with improvement quality. Running on a consumer RTX 3060 GPU with a quantized 8B model, IPE-MHE achieves: DHE fix-prediction precision of X% vs. AHE's 33.7% baseline; the first longitudinal harness fingerprint dataset across 30 evolution rounds; LCAP allocation outperforming static policy by Y pp; Pearson r(MCA, IR) = Z; Identity Coherence Score ≥ 0.70 after 30 rounds confirming identity preservation; and GAIA Level 2 pass rate of W%, matching frontier-model capability through harness evolution alone on consumer hardware. We release the evolved harness as a portable corpus transferring to 17+ model families and a self-model dataset enabling future study of identity under self-modification.

---

## Section 1 — Introduction (3 pages)

**Story arc:**
1. AGI framing: the dominant view is AGI = a sufficiently large model. The alternative: AGI = Model + Harness. The harness is the missing piece.
2. The $400 GPU problem: frontier-scale compute is unavailable to most researchers. If the harness improvement is the dominant lever, consumer hardware may be sufficient.
3. The three-lever gap: agent self-improvement has been studied in three bodies of work — fine-tuning (parametric), in-context trajectory replay (contextual), and harness evolution (structural) — but no paper combines them or studies their interaction.
4. The metacognitive gap: [arXiv:2506.05109] identified that truly self-improving agents need a metacognitive self-model. No implementation exists.
5. Contributions (bulleted):
   - The three-lever taxonomy: naming and formalizing parametric/contextual/structural as orthogonal axes of agent self-improvement
   - Professor X: first implementation combining all three on consumer hardware
   - HIRO benchmark: first metric for harness-isolated improvement rate
   - DFA Trifecta: DHE + BF + LCAP as the structural improvement mechanism
   - MHE: metacognitive self-model directing lever selection, measured by MCA
   - The harness corpus: evolved structural improvements that transfer across 17+ model families
6. What the paper does NOT claim: we do not claim to achieve AGI. We claim to demonstrate that the harness lever is measurably the dominant contributor to agent performance improvement on consumer hardware.

**Key citations in intro:** HAL (+36pp scaffold swap, arXiv:2510.11977), arXiv:2506.05109 (metacognition gap), arXiv:2604.25850 (AHE 33.7% baseline), arXiv:2605.22166 (Life-Harness portability proof).

---

## Section 2 — Background and Related Work (4 pages)

### 2.1 Harness Engineering
- What a harness is: the infrastructure layer determining context, tools, memory, orchestration
- Why it matters: HAL study shows harness changes produce +36pp — larger than most model upgrades
- The AHE paper as baseline: component observability, change manifests, 33.7% fix precision
- Meta-Harness (Stanford, arXiv:2603.28052): closest Lever-3-only work, frontier API required
- Harbor (arXiv:2604.20938): Bayesian optimization for harness config (non-LLM approach)
- Life-Harness (arXiv:2605.22166): portability of structural improvements across 17 models

### 2.2 Parametric Self-Improvement (Lever 1)
- SDAR (arXiv:2605.15155): token-level sigmoid-gated distillation, +9.4% ALFWorld on Qwen3
- Absolute Zero (arXiv:2505.03335): self-generated curriculum, zero external data
- Connection to our work: SDAR as Lever 1, applied overnight on consumer hardware

### 2.3 Contextual Self-Improvement (Lever 2)
- Self-Generated ICE (arXiv:2505.00234): 73%→93% ALFWorld, zero fine-tuning
- ACE (arXiv:2510.04618): context as evolving playbook, ICLR 2026, +10.6%
- Trajectory-Informed Memory (arXiv:2603.10600): 14.3pp gains, 149% relative improvement
- MARS (arXiv:2601.11974): single-cycle principle+procedure reflection
- Connection to our work: Lever 2 implemented via memd episodic retrieval + MARS reflection layer

### 2.4 Metacognition in AI Systems
- arXiv:2506.05109 (ICML 2025): position paper, three components, no implementation
- HyperAgents (Meta, arXiv:2603.19461): improvement@k, frontier APIs, coding domain only
- Agent Psychometrics (arXiv:2604.00594): IRT decomposition — scaffold ability is separable
- Connection to our work: Professor X is the implementation of arXiv:2506.05109

### 2.5 Consumer Hardware Feasibility
- arXiv:2506.02153: SLMs match frontier models on structured agentic tasks
- arXiv:2605.12129: harness design determines stability in 2-3B models
- arXiv:2603.28823: optimal model size on consumer hardware (Chinchilla adjusted)
- Statistical limits (arXiv:2510.04399): self-improvement is safe iff capacity bounded — harness evolution satisfies by construction

---

## Section 3 — The Three-Lever Framework (2 pages)

**This is a taxonomy contribution. It names something the community is doing implicitly.**

### 3.1 Definition of the three levers
Table: Lever | What changes | Pace | Persistence | Portability | Representative works

### 3.2 Orthogonality
Argument: each lever addresses a distinct failure mode that the other two cannot fix.
- Lever 1 (parametric) fixes model-layer failures: the model's internal representations are systematically wrong for a domain
- Lever 2 (contextual) fixes session-level failures: relevant experience is not being used
- Lever 3 (structural) fixes infrastructure failures: the harness itself is misconfigured

### 3.3 The portability asymmetry
Key insight: Lever 3 improvements are model-agnostic (Life-Harness: 88.5% transfer). Lever 2 improvements are domain-specific (ephemeral). Lever 1 improvements are model-specific (adapter is frozen to model family).
Implication: a harness evolved on any consumer hardware is a portable artifact — more like a dataset than a fine-tuned model. This is the Alpaca analogy: the evolved harness is the corpus, not the checkpoint.

### 3.4 Composability prediction
Hypothesis: combining all three should be superadditive, because:
- SDAR-trained model generates better ChangeManifest proposals (Lever 1 improves Lever 3)
- Lever 3 structural fixes clear infrastructure noise from the failure signal, making Lever 1 training data cleaner
- Lever 2 provides session-time performance that helps DHE get better diagnostic signals
This is the claim we test in Table 1 (4-baseline experimental design).

### 3.5 Identity-Preserving Evolution (IPE) — the fourth axis

Beyond the three levers, IPE addresses what all self-evolving systems ignore: coherence of
self across change. The Strange Loop (Hofstadter): the agent's "I" is a self-referential
pattern. IPE formalizes this as:
- **Self-model**: an evolving self-description in pinned memory, updated every 10 rounds
- **ICS**: Identity Coherence Score — cosine similarity between current and baseline self-model
- **Free Energy Drive**: the agent minimizes prediction error (FED) as its implicit goal,
  producing self-preservation, curiosity, and frustration as emergent functional states
- **Functional Affect**: valence + arousal computed from task outcomes, injected into every
  LLM prompt — not simulated, but derived directly from the gap between predictions and reality

IPE reframes the thesis: not "harness optimization" but "the first computational Strange Loop
that evolves intentionally without losing itself."

---

## Section 4 — Professor X: System Description (3 pages)

### 4.1 Architecture overview
Five modules: memd, toolbridge, agentd, policyd, evolved. Single Rust binary. Tokio async. Hardware: RTX 3060, qwen3:8b-q4_k_m.

### 4.2 Memory architecture (memd)
Five-layer: Pinned → Working → Episodic → Semantic → Procedural. All-MiniLM-L6-v2 embeddings. CLAG-style cluster retrieval. LCAP context budget enforcement. MARS reflection buffer.
**New:** Pinned layer includes the current SelfModelSnapshot. Updated every 10 rounds.

### 4.3 Lever 2 implementation
Session startup: top-k episodic retrieval of similar past tasks injected as ICE. MARS: on failure, single-cycle principle + procedural reflection written to Working, persisted to Semantic.

### 4.4 Lever 3 implementation: evolved module
Researcher/Engineer/Analyzer loop (from ASI-Evolve). All evolvable components version-controlled in harness/ under git. Every evolution cycle = one git commit. Rollback = git revert. DHE, BF, LCAP active from round 10 onward.

### 4.5 Lever 1 implementation: sleep-time fine-tuning
Triggered by scheduler when agent is idle (post-7h daily cycle). Successful trajectories formatted as SDAR training examples. QLoRA run via unsloth on qwen3:8b. LoRA adapter saved; model restored to base + adapter for next session. Budget: ~6GB VRAM with model offloaded.

### 4.6 The metacognitive self-model (MCA)
MetacognitiveEntry store in memd.semantic. After each HIRO round: record predicted_layer, predicted_lever, actual_improvement, attribution_correct. MCA computed as rolling accuracy over last 10 rounds.

### 4.7 The Strange Loop self-model (ICS)
SelfModelSnapshot stored in SQLite + injected into pinned memory. LLM-generated update every 10 rounds from: fingerprint history, MCA, mean affect over last 10 rounds, prior self-description. ICS computed as cosine similarity (embedding) vs. round-0 baseline.

### 4.8 Functional affect system
AffectState (valence, arousal) updated after every task via exponential moving average.
Valence = tanh(actual - predicted). Arousal = tool_density + retry_pressure.
Injected as `<affect state="X" valence="Y" arousal="Z" />` into every ReAct prompt.
FED (Free Energy Delta) = mean |predicted - actual| per session, logged for H15.

---

## Section 5 — HIRO: The Benchmark (2 pages)

**HIRO is a publishable standalone contribution — it defines the measurement space.**

### 5.1 Motivation
No existing benchmark measures harness-isolated improvement rate. All benchmarks report aggregate performance of (model + harness) together. HIRO isolates the harness contribution by freezing the model.

### 5.2 Task suite (60 tasks)
- 20 tool-use tasks: deterministic verification (pass/fail by output matching)
- 20 planning tasks: LLM-as-judge (Sonnet 4.5, 0/1 score with rubric)
- 20 self-correction tasks: binary (agent must detect and fix its own error)
Source of tasks: synthetic + adapted from ALFWorld, AppWorld, ToolEval. Tasks are fixed and public.

### 5.3 Metrics
- **HIRO(N)**: `(P_N - P_0) / N` — mean pass@3 gain per round
- **BF**: `F(H_k) = [p_tool, p_plan, p_correct]` — capability fingerprint at round k
- **DHE Fix Precision**: fraction of predicted improvements that materialize next round
- **MCA**: metacognitive calibration accuracy over last 10 rounds
- **IR**: 5-round rolling improvement rate

### 5.4 Baselines
- Static harness null (noise floor): HIRO(30) ≈ 0 expected
- Human expert (same time budget, informed by same papers): establishes H5 claim
- Frontier API with same harness: GPT-4o endpoint, same Professor X harness, no evolution — establishes H9 claim

### 5.5 Reproducibility
Full task suite, evaluation rubrics, and harness snapshot at round 0 released on GitHub. Any researcher with an RTX 3060 and Ollama can reproduce the benchmark in ~47 min/round.

---

## Section 6 — The DFA Trifecta (4 pages)

### 6.1 DHE — Diagnostic Harness Evolution
- The 5-layer probe (retrieval → context → tool dispatch → tool execution → reasoning)
- Each layer: deterministic test + attribution output `{layer, evidence, confidence}`
- Integration with Researcher: ChangeManifest must target attributed layer
- Claim: ≥60% fix-prediction precision vs AHE's 33.7% (H10)
- Comparison to Reflexion: structured 5-layer vs free-form verbal. DHE produces machine-readable attribution; Reflexion produces free text.

### 6.2 BF — Behavioral Fingerprinting
- F(H_k) = [p_tool, p_plan, p_correct] at every HIRO round
- HiroRoundResult schema with fingerprint field
- The longitudinal dataset: 30 rounds × 60 tasks × 3 attempts = 5,400 evaluations with full harness provenance
- Claim: non-uniform improvement — at least one category diverges >10pp from others (H11)
- Why aggregate reporting is insufficient: two hypothetical systems with identical HIRO(30) scores but opposite fingerprint trajectories have different implications for deployment

### 6.3 LCAP — Learned Context Allocation Policy
- ContextBudget: {episodic_slots, semantic_slots, tool_depth, system_prompt_tokens, hard_ceiling_tokens}
- UCB1 bandit over 5 allocation strategies per task type (c=1.414)
- Update rule: round-level delta_p drives arm selection
- Connection to DHE: Layer 2 attribution (context overload) triggers LCAP directly, bypassing Researcher
- Claim: ≥3pp over static T*-optimal allocation (H12)

---

## Section 7 — Experiments and Results (5 pages)

### 7.1 Table 1 — The 4-Baseline Experiment
```
Condition              | Lever 1 | Lever 2 | Lever 3 | HIRO(30) | Pass@3 overall
Baseline 1: Stock      |    ✗    |    ✗    |    ✗    |    ~0    |      P_0
Baseline 2: SDAR only  |    ✓    |    ✗    |    ✗    |    ~0    |      P_0 + ΔL1
Baseline 3: Struct only |   ✗    |    ✓    |    ✓    |    H3   |      P_0 + ΔL23
Target: Full MHE       |    ✓    |    ✓    |    ✓    |    H_T  |      P_0 + ΔL123
Cloud ref: GPT-4o      |    —    |    —    |    —    |    —    |      P_frontier
```
Superadditivity test: H_T > H3 + ΔL1? (Is combining all three levers better than the sum of parts?)

### 7.2 DHE Fix-Prediction Precision (H10)
- Plot: rounds 1-9 (unguided) vs rounds 10-30 (DHE-guided) fix-prediction precision
- Hypothesis: ≥60% in DHE-guided phase, ≤40% in unguided phase
- Statistical test: Fisher's exact test on fix counts

### 7.3 Behavioral Fingerprint Trajectories (H11)
- Plot: F(H_k) = [p_tool, p_plan, p_correct] across 30 rounds (3-line trajectory chart)
- Divergence test: max(|Δ_i|) > 0.10, Range(Δ_i) > 0.07
- Correlation: component_modified vs delta_fingerprint component (Pearson r)

### 7.4 LCAP vs Static Allocation (H12)
- Plot: per-type pass@3 over 10 rounds, LCAP vs static
- Primary metric: mean delta ≥ 3pp

### 7.5 MCA-IR Correlation (H13)
- Plot: MCA(k) vs IR(k) for k ∈ [10, 30]
- Pearson r > 0.70 claim
- Interpretation: agents with more accurate self-models improve faster

### 7.6 Consumer Hardware Parity (H9)
- Comparison: Professor X + qwen3:8b-q4_k_m vs Professor X + GPT-4o endpoint, same harness
- Metric: |HIRO_local - HIRO_frontier| < 0.03

### 7.7 Identity Coherence Under Evolution (H14)
- Plot: ICS trajectory at rounds 0, 10, 20, 30 (4-point line chart)
- Primary claim: ICS ≥ 0.70 at round 30
- Secondary: ICS delta never exceeds -0.15 in a single 10-round window
- Interpretation: the Strange Loop persists; Professor X is still himself after 30 rounds

### 7.8 Free Energy Reduction (H15)
- Plot: FED per session across all rounds (rolling mean, regression line)
- Statistical test: linear regression slope < 0 (p < 0.10)
- Interpretation: Professor X's world model becomes more accurate over time

### 7.9 GAIA Level 2 Trajectory (H18)
- Plot: GAIA L2 pass rate at rounds 0, 10, 20, 30
- Primary target: ≥ 40% at round 30
- Comparison: same harness + GPT-4o endpoint at round 0 (establishes frontier baseline)
- This is the headline number: frontier-level capability from harness evolution on $400 GPU

### 7.10 Research Quality Trajectory (H17)
- Plot: AI Idea Bench 2025 mean score at weeks 0, 4, 8, 12
- Novelty and feasibility components shown separately
- Interpretation: does the evolved harness improve scientific thinking, not just task execution?

---

## Section 8 — Discussion (2 pages)

### 8.1 What the results mean
- If H9 confirmed: the harness dominates the model for structured agentic tasks. A $400 GPU harness outperforms a frontier API with a weaker harness.
- If H13 confirmed: metacognitive calibration is the missing ingredient in self-evolving systems. Agents that know what they don't know improve faster.
- The portability result: releasing the evolved harness as a corpus, not a model.

### 8.2 Limitations
- HIRO task suite is limited to 60 tasks; results may not generalize to long-horizon tasks
- qwen3:8b-q4_k_m is a single model; different model families may produce different fingerprint trajectories
- The MCA-IR correlation has n=20 rounds; larger longitudinal studies are needed
- Sleep-time LoRA fine-tuning produces adapters that drift over long runs; adapter consolidation strategy not yet designed

### 8.3 The Alpaca analogy
Stanford Alpaca's contribution was not a better model — it was a demonstration that training data generation could be automated cheaply. The contribution transferred across the ecosystem.
MHE's structural contribution is analogous: the evolved harness is not a better model — it is a portable infrastructure improvement that transfers across the model ecosystem. The insight is in the process, not the checkpoint.

---

## Section 9 — Conclusion (1 page)

- Three-lever framework: names and formalizes what the community is doing implicitly
- HIRO: first benchmark for harness-isolated improvement rate
- DFA Trifecta: first structured attribution-before-modification protocol for harness evolution
- MHE: first implementation of metacognitive harness evolution, running on consumer hardware
- The harness corpus: released as portable artifact transferring to 17+ model families
- Open question left for future work: does the MCA-IR correlation hold across different model families and task domains?

---

## Appendix

- A: Full HIRO task suite (60 tasks, evaluation rubrics, judge prompts)
- B: DHE 5-layer probe implementation (pseudocode + Rust structs)
- C: LCAP UCB1 update rules and arm definitions
- D: MetacognitiveEntry schema and MCA computation
- E: Hardware setup and reproducibility instructions (one command install on RTX 3060)

---

## What We Need to Write the Paper

In priority order:

1. **Week 5 onwards:** Implement DHE, BF, LCAP, MetacognitiveEntry — the measurement instruments
2. **Weeks 6–10:** Run 30 HIRO rounds. This generates Sections 7.2–7.5 automatically.
3. **Week 10–11:** Run 4-baseline experiment (Table 1). This requires SDAR Qwen3-8B checkpoint.
4. **Week 11:** Run H9 comparison (swap model endpoint, keep harness frozen).
5. **Week 12:** Write Sections 1, 2, 3, 4, 5 (from this outline + literature already documented).
6. **Week 13:** Submit preprint to arXiv. Begin NeurIPS 2026 D&B submission.

**Minimum viable paper (if time runs out):** Sections 1, 2, 3, 5, 6, 7.2–7.3 only. DHE fix-precision + fingerprint dataset alone is publishable. The rest strengthens the claim.

---

*Created: 2026-05-23*
*Status: Outline only — no results exist yet. Results populate Sections 7–8.*
*This document is updated whenever the paper structure changes. Brain/inventions.md drives the mechanism detail; this document drives the narrative structure.*
