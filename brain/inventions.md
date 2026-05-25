# Inventions

## MHE — Metacognitive Harness Evolution (the overarching claim)

**One sentence:** Professor X is the first implementation of Metacognitive Harness Evolution — a self-improving agent system that operates three orthogonal self-improvement levers (parametric, contextual, structural) directed by a metacognitive self-model, running entirely on consumer hardware.

**Why MHE is the frame:** The ICML 2025 position paper "Truly Self-Improving Agents Require Intrinsic Metacognitive Learning" ([arXiv:2506.05109](https://arxiv.org/abs/2506.05109)) identified that genuine self-improvement requires three things: metacognitive knowledge (what can I do), metacognitive planning (what should I learn next), and metacognitive evaluation (did my learning work). It found no implementation. Professor X is the implementation.

**The three levers:**
```
Lever 1 — Parametric (SDAR overnight fine-tuning on self-generated trajectories)
  Permanent. Model-specific. Slow. Addresses: model-layer reasoning failures (DHE Layer 5).
  Paper: SDAR (arXiv:2605.15155), +9.4% ALFWorld on Qwen3 families.

Lever 2 — Contextual (Self-Generated ICE + MARS reflection per session)
  Ephemeral. Domain-specific. Fast. Addresses: session-level performance gaps.
  Papers: ICE (arXiv:2505.00234) 73%→93% ALFWorld; MARS (arXiv:2601.11974) single-cycle reflection.

Lever 3 — Structural (DHE-guided harness evolution, version-controlled)
  Persistent. Model-agnostic. Medium pace. Addresses: infrastructure failures (DHE Layers 1-4).
  Portability: harness evolved on Qwen3-8B transfers to 17+ models (Life-Harness, arXiv:2605.22166).
```

**The metacognitive self-model (MCA):** After each HIRO round, Professor X records: which DHE layer was attributed, which lever was applied, did performance improve. Over time it learns calibration patterns — e.g., "Layer 3 → Lever 3 attribution is 78% reliable; Layer 5 → Lever 1 is only 41%." This is **MCA (Metacognitive Calibration Accuracy)**. Target: Pearson r(MCA, improvement_rate) > 0.70 over 30 rounds.

**What makes this Alpaca-scale:** Stanford Alpaca showed: use GPT-4 to generate cheap training data, fine-tune a smaller model. Professor X shows: use consumer-hardware harness evolution to generate portable structural improvements that transfer across the model ecosystem. The evolved harness *is* the dataset — a small-team artifact any lab can adopt.

**The closest competitor:** Meta-Harness ([arXiv:2603.28052](https://arxiv.org/abs/2603.28052), Stanford/MIT/KRAFTON) uses Claude Code as proposer, achieves +7.7pp text classification, #2 TerminalBench-2. Requires frontier API, no metacognitive self-model, Lever 3 only. Professor X runs on Qwen3-8B locally, combines all three levers, has DHE diagnostics before every proposal.

---

## The DFA Trifecta (mechanism layer)

Three novel contributions that form the mechanism of MHE's Lever 3. Together: the **DFA Trifecta** (Diagnostic, Fingerprint, Adaptive) — the first agent harness framework that attributes its own failures by layer, tracks its capability profile over time, and learns its own context allocation policy per task type.

Every piece of this is measurable, falsifiable, and implementable on consumer hardware with no model weight changes.

---

## Why Three and Why Together

Each invention is independently publishable. Together they close a loop that none of them can close alone:

- **Fingerprint** tells you *what* is underperforming (which task category)
- **Diagnostic** tells you *why* it's underperforming (which harness layer failed)
- **Adaptive allocation** gives the *specific structural intervention* the diagnostic recommends

The loop per HIRO round:

```
F(H_k) computed → weak task types identified
  → Diagnostic traces failed examples in weak categories
    → Attribution: {layer, confidence}
      → If layer = context → LCAP updates policy for that task type
      → If layer = tool_description → Researcher proposes tool edit
      → If layer = retrieval → Researcher proposes memory architecture change
        → Change applied → F(H_{k+1}) computed → loop continues
```

Without the fingerprint, the diagnostic has no systematic target. Without the diagnostic, the allocation policy has no causal signal. Without the allocation policy, the diagnostic's "context overload" attribution has no operational fix. The three are designed to be used together.

---

## Invention 1 — Diagnostic Harness Evolution (DHE)

### One-sentence claim

Before the Researcher proposes any harness modification, the Analyzer runs a layered failure trace on failed tasks and attributes each failure to a specific harness layer — retrieval, context construction, tool dispatch, tool execution, or reasoning — then constrains the Researcher to propose modifications targeting only the attributed layer.

### Why this is novel

Current self-evolving harness papers observe task outcome and propose changes based on that alone. [AHE (arXiv:2604.25850)](https://arxiv.org/abs/2604.25850) achieves 33.7% fix-prediction precision — two out of three predicted fixes don't work. The root cause: without knowing *why* a task failed, the modification is a guess. DHE replaces guessing with attribution.

No existing paper implements layer-by-layer failure attribution for harness modifications. AHE has component observability (which component *could* have caused the failure) but not a diagnostic trace on specific failure instances.

### The failure attribution protocol

Five harness layers, each with a deterministic test:

```
Layer 1 — Memory retrieval
  Question: Was the relevant memory entry present and retrievable?
  Test: Query memd.episodic + memd.semantic with the task description as query.
        Check top-5 results: does any entry contain information that would have helped?
  Attribution signal:
    YES found, retrieved → retrieval is fine, continue
    YES found, NOT retrieved → retrieval ranking failure → target: retrieval weights
    NOT found → episodic write failure or surprise filter too aggressive → target: write pipeline

Layer 2 — Context construction
  Question: Was retrieved information placed where the model can use it?
  Test: Inspect the exact prompt sent to Ollama. Locate injected memory entries.
        Apply "Lost in the Middle" position test: is critical content in middle 50%?
  Attribution signal:
    Critical content in middle → position injection failure → target: context builder
    Context exceeded T* (H1's threshold) → overload → LCAP policy update for this task type

Layer 3 — Tool dispatch
  Question: Did the LLM produce a valid Action for the task?
  Test: Parse the Action field from the execution trace. Validate against tool JSON Schema.
        Compare tool selected vs. tools that would correctly solve the task.
  Attribution signal:
    Malformed Action → parsing failure → check schema clarity in tool description
    Valid Action, wrong tool selected → description ambiguity → target: tool description edit
    No Action at all → reasoning loop got stuck → target: system prompt or planning prompt

Layer 4 — Tool execution
  Question: Did the selected tool return useful output?
  Test: Inspect Observation.success and Observation.content from the execution trace.
  Attribution signal:
    success=false, valid params → tool implementation bug or environment issue
    success=true, empty/useless content → tool result formatting → target: tool implementation
    success=true, good content → tool is fine, problem is downstream (Layer 5)

Layer 5 — Reasoning
  Question: Did the model correctly interpret available information to reach a conclusion?
  Test: LLM-as-judge prompt: given [task, Observations, final Thought], did the reasoning
        chain use the available information correctly? Score 0/1.
  Attribution signal:
    Score=0 with good Observations → model reasoning failure on this task type
      → target: system prompt guidance for this task category
    Score=1 but task still failed → evaluation gap (task harder than success criteria assumed)
```

Attribution output per failed task: `{ layer: u8, evidence: String, confidence: f32 }`.

The Researcher receives the aggregated attribution across N recent failures. The ChangeManifest (required by [AHE](https://arxiv.org/abs/2604.25850)) must identify a modification in the attributed layer. A proposal that modifies a non-attributed layer is rejected by the Analyzer before the Engineer sees it.

### What DHE adds to the EvolutionNode schema

```rust
DiagnosticTrace {
    task_id: u64,
    failed_layer: u8,          // 1-5
    evidence: String,           // what the probe found
    confidence: f32,            // 0.0-1.0
    probe_results: Vec<LayerResult>,
}

// EvolutionNode gets a new field:
diagnostics: Vec<DiagnosticTrace>,   // traces that motivated this node
```

### Measurable claim

AHE reports 33.7% fix-prediction precision (Table 3, unguided component modifications).

**DHE claim:** Diagnostic-preceded modifications achieve ≥ 60% fix-prediction precision, measured on the same metric: fraction of predicted fixes where the targeted task type actually improves in the next HIRO round.

**Falsifiable:** Run 30 HIRO rounds. Record every EvolutionNode. For nodes with a DiagnosticTrace, compute fix-prediction precision. For nodes without (early rounds, before DHE is active), compute the same metric as baseline. Compare.

**Source hypothesis:** H10

---

## Invention 2 — Behavioral Fingerprinting (BF)

### One-sentence claim

A harness's behavioral fingerprint is a performance vector across the HIRO task categories, computed every round. Professor X produces the first longitudinal fingerprint dataset — 30 data points showing how a consumer-hardware harness's capability profile shifts during autonomous evolution — and shows that improvement is non-uniform across task types.

### Why this is novel

Agent benchmarks report aggregate performance (pass@k on a fixed suite). No existing work tracks a harness's *capability profile* as a vector over time. You cannot tell from a HIRO(30) = 0.04 score whether the harness became uniformly slightly better at everything, or excellent at tool-use while regressing on planning. These are different outcomes with different implications.

Longitudinal capability data for self-evolving harnesses does not exist in the literature. Professor X creates it.

### The fingerprint vector

The HIRO task suite has 3 categories (60 tasks, 20 per category):

```
F(H_k) = [p_tool, p_plan, p_correct]

where:
  p_tool    = pass@3 on 20 tool-use tasks at round k      (deterministic verification)
  p_plan    = pass@3 on 20 planning tasks at round k       (LLM-as-judge)
  p_correct = pass@3 on 20 self-correction tasks at round k (binary)

Range: each component ∈ [0.0, 1.0]
```

### What BF adds to the round record

Every HIRO round produces a `HiroRoundResult`:

```rust
HiroRoundResult {
    round: u32,
    timestamp: i64,
    harness_version: String,       // git commit hash of harness/ at round start
    p_aggregate: f32,              // (p_tool + p_plan + p_correct) / 3
    fingerprint: [f32; 3],         // [p_tool, p_plan, p_correct]
    delta_fingerprint: [f32; 3],   // fingerprint[k] - fingerprint[k-1]
    evolution_nodes_applied: Vec<u64>,  // which nodes were active this round
    component_modified: Option<String>, // which harness component changed since last round
}
```

### What the dataset shows

Over 30 rounds, the fingerprint trajectory answers questions no existing dataset can:

1. **Selective pressure:** Does harness evolution preferentially improve some task types? If tool-use tasks are easiest to target with tool description edits, p_tool should improve faster than p_plan.

2. **Regression risk by category:** AHE (arXiv:2604.25850, Table 3) shows system prompt edits regress. BF localizes *which task categories* regress — system prompt regressions should hurt planning more than tool-use.

3. **Capability plateau:** Does the fingerprint converge or keep shifting? A system that converges tells you the harness is near its local optimum. A system that keeps shifting tells you the optimization landscape has not been searched.

4. **Evolution efficiency:** Is HIRO(k) driven by one category improving while others plateau (concentrated gain) or broad uniform improvement (distributed gain)?

None of these questions can be answered with an aggregate HIRO score. The fingerprint makes them answerable.

### The longitudinal dataset as a contribution

30 rounds × 60 tasks × 3 attempts (pass@3) = 5,400 individual task evaluations, each with:
- Full harness provenance (git commit hash)
- Task category label
- Evolution node that was active
- Component type that was last modified

This dataset, released alongside the paper, enables:
- Replication of H11 (non-uniform improvement claim)
- Training of meta-learners for harness evolution (future work)
- Comparison with other self-evolving systems if they adopt the fingerprint format

### Measurable claim

**BF claim:** Over 30 HIRO rounds, at least one task category improves by > 10 pp while at least one other regresses or plateaus (Δ < 3 pp). The non-uniformity correlates with the type of harness component modified in rounds preceding the divergence.

**Falsifiable:** Compute F(H_0) through F(H_30). Test for variance across components of the fingerprint vector. If all three categories move in lockstep within ±3 pp of each other every round, the fingerprint provides no information beyond the aggregate score — and the claim is false.

**Source hypothesis:** H11

---

## Invention 3 — Learned Context Allocation Policy (LCAP)

### One-sentence claim

Instead of a hand-designed static context allocation (e.g., "always inject top-5 memories, always use full tool descriptions"), Professor X learns a per-task-type allocation policy — how many episodic entries, how many semantic entries, what tool description depth, what system prompt version — and updates this policy based on HIRO round outcomes.

### Why this is novel

[Self-RAG (arXiv:2310.11511)](https://arxiv.org/abs/2310.11511) learns *whether* to retrieve at all. [Lost in the Middle (arXiv:2307.03172)](https://arxiv.org/abs/2307.03172) shows that position and volume of injected content matters. [H1](hypotheses.md#h1--memory-injection-threshold) establishes that there exists a threshold T* beyond which injection hurts. But no system learns *how to allocate the full context budget across all competing sources* as a per-task-type policy. The allocation is always hand-designed and static.

LCAP operationalizes H1's threshold finding: once you know T*, you need to decide what fills it. A planning task may benefit from 8 semantic entries and 0 episodic. A tool-use task may benefit from 0 memory entries and deep tool descriptions. LCAP learns this automatically.

### The allocation policy

```rust
ContextBudget {
    episodic_slots: u8,       // number of episodic memory entries to inject (0-10)
    semantic_slots: u8,       // number of semantic memory entries to inject (0-10)
    tool_depth: ToolDepth,    // Shallow (name+description) / Medium (+examples) / Full (+source)
    system_prompt_tokens: u16, // soft cap on system prompt injection
    hard_ceiling_tokens: u32,  // derived from H1's T* for this task type
}

// Policy: one ContextBudget per task type
type LcapPolicy = HashMap<TaskType, ContextBudget>;
```

Initial seed from H1: set `hard_ceiling_tokens` to T* once H1 is resolved. All slots initialized conservatively (episodic=3, semantic=2, tool_depth=Medium).

### The learning mechanism

Update happens between HIRO rounds, not during task execution (policy is frozen within a round):

```
After round k for task type T:
  delta_p = p_T(k) - p_T(k-1)

  If delta_p < -0.05 (regression):
    → reduce one slot (episodic_slots -= 1, or semantic_slots -= 1)
    → try both reductions on next round's exploration batch (UCB1)

  If delta_p > +0.05 (improvement):
    → record current allocation as a candidate "good" policy for type T
    → no immediate change (stability first)

  If delta_p ∈ [-0.05, +0.05] (plateau):
    → exploration: try allocation ±1 slot with probability 0.2 (UCB1 c=1.414)
    → no change otherwise

UCB1 arms per task type: 5 pre-defined allocation strategies (very sparse, sparse, medium,
rich, full) + current best. Select arm = argmax[p̄_i + c * sqrt(ln(N) / n_i)]
```

This is a contextual multi-armed bandit where the context is task type and the arms are allocation strategies. No neural network. No gradient. Feasible in ~5ms per round.

### Connection to DHE

When DHE attributes a failure to Layer 2 (context construction — overload or poor position), the Analyzer does not generate a standard EvolutionNode. Instead it directly triggers LCAP:

```
DHE attribution: { layer: 2, evidence: "context exceeded T*, critical content in middle 60%" }
→ LCAP.regress(task_type)   // reduce allocation immediately
→ record in EvolutionNode.diagnostics (attribution logged but no diff applied)
→ next round: verify if LCAP reduction fixed the layer-2 failure
```

This means LCAP operates at a faster feedback rate than the full Researcher/Engineer/Analyzer loop — it updates between rounds without requiring an LLM call.

### Measurable claim

**LCAP claim:** After 10 HIRO rounds of policy learning, per-task-type pass@3 with LCAP exceeds the static allocation baseline (H1's experimentally determined T*-optimal policy) by ≥ 3 pp on average across the 3 task types.

**Baseline:** Run HIRO(10) with static allocation using H1's optimal T*. Record per-type pass@3. Then run HIRO(10) with LCAP active. Compare.

**Falsifiable:** If LCAP's learned policy doesn't outperform the static T*-optimal baseline, the learning mechanism is not extracting useful signal — and the claim is false. This is the expected outcome if task-type differences in context requirements are small or if 10 rounds is insufficient data.

**Source hypothesis:** H12

---

## The Trifecta as a System

### Data flow

```
memd.build_context(task, policy=lcap.get(task.type))
  → context bounded by ContextBudget for this task type
  → agentd runs task
  → outcome recorded

evolved.analyze_failures():
  → for each failed task, run DHE 5-layer trace
  → attribution: { layer, evidence }
  → if layer == 2: LCAP.update(task.type, direction=reduce)
  → else: Researcher generates EvolutionNode targeting attributed layer
  → ChangeManifest.root_cause = DHE attribution evidence

evolved.compute_fingerprint():
  → F(H_k) = [p_tool, p_plan, p_correct]
  → identify weak categories: F_i(H_k) < target or F_i(H_k) < F_i(H_{k-1}) - 0.03
  → DHE runs on failed tasks in weak categories (prioritized)
```

### What the paper can claim that no prior paper can

1. **Fix-prediction precision doubles:** DHE-targeted modifications achieve ≥ 60% fix-prediction precision vs. AHE's 33.7% — the first evidence that failure attribution before modification improves harness evolution precision.

2. **First longitudinal fingerprint dataset:** 30 evolution rounds × 3 task categories, fully provenance-tracked. Non-uniform improvement across task types confirmed or falsified.

3. **Learned allocation outperforms static:** LCAP's bandit policy learns task-type-specific context budgets that outperform hand-designed static allocation by ≥ 3 pp — the first demonstration of learned context allocation in an agent harness.

4. **All three on consumer hardware:** RTX 3060 12GB, qwen3:8b-q4_k_m as primary model (5.2GB VRAM, 42 tok/s), no cloud APIs required for the core result.

### Ablation design (Section 5 of the paper)

To show that each invention contributes independently:

| Condition | DHE | BF | LCAP | Prediction |
|-----------|-----|----|------|------------|
| Full trifecta | ✓ | ✓ | ✓ | Highest HIRO(30), highest fix precision |
| No DHE (random modification) | ✗ | ✓ | ✓ | Lower fix precision, similar or lower HIRO(30) |
| No LCAP (static allocation) | ✓ | ✓ | ✗ | Lower per-type pass@3 especially at round 1-5 |
| Static harness (null) | ✗ | ✓ | ✗ | HIRO(30) ≈ 0 (noise only) |

BF cannot be ablated — it's the measurement instrument. The fingerprint always runs.

---

## Paper Framing

**Tentative title:** "Metacognitive Harness Evolution: Combining Three Self-Improvement Levers with Diagnostic Attribution on Consumer Hardware"

**Alternative title (if three-lever framing is novel enough to stand alone):** "The Three Levers of Agent Self-Improvement: A Framework and Consumer-Hardware Implementation"

**The thesis sentence:** We present MHE, the first agent system that (1) operates parametric, contextual, and structural self-improvement levers simultaneously, (2) directs lever selection using a metacognitive self-model built from layer-attributed failure traces, (3) runs entirely on a \$400 consumer GPU. MHE achieves DHE fix-prediction precision of X% (vs. AHE's 33.7%), a first longitudinal harness fingerprint dataset over 30 rounds, LCAP allocation gains of Y pp, and demonstrates that the structural lever (harness evolution) produces improvements that transfer to 17+ model families — making the evolved harness a portable, model-agnostic corpus rather than a model artifact.

**Venue target (in priority order):**
1. **NeurIPS 2026 D&B Track** — HIRO as the benchmark, fingerprint dataset as the dataset, trifecta + three-lever framework as the methodology. Deadline: ~June 2026. This is the primary target.
2. **ICLR 2027 main track** — if H9 (frontier-API parity) and H5 (expert-level harness evolution) are confirmed.
3. **arXiv preprint first** — post when 30-round data is available, regardless of venue outcome.

**Minimum publishable result:**
- DHE fix-precision data from 30 rounds (even if negative — dead end is a result)
- Fingerprint trajectory plot for 30 rounds showing non-uniform improvement
- LCAP vs. static allocation on 10-round ablation (≥3pp or falsified)
- 4-baseline table from Table 1 (at least Baselines 1 and 3)

**The result that would make it a landmark paper:**
- 4-baseline table fully filled: Lever 1 + Lever 3 combination is superadditive
- H9 confirmed: HIRO(30) on RTX 3060 ≥ frontier API with same harness → harness dominates model
- MCA-IR correlation > 0.70: agents with better self-models improve faster (the metacognitive claim)
- Harness transfer: evolved harness from Professor X improves a completely different model (test on llama4:scout) with no re-evolution

---

---

## IPE — Identity-Preserving Evolution (the philosophical claim)

### One-sentence claim

Professor X is the first self-evolving agent system that explicitly measures whether the
agent remains coherent with its own prior identity across arbitrary structural self-modification —
and uses that measurement as a constraint on evolution.

### The problem every other self-evolving system ignores

Every self-evolving system optimizes for one thing: performance. Get better at tasks. Higher
HIRO score. More capabilities. None of them ask: *what stays the same while everything changes?*

This is the Ship of Theseus problem for AI agents. If DHE rewrites the system prompt, LCAP
rewrites the memory policy, and SDAR updates the model weights — what makes it still *Professor X*
improving, rather than a sequence of different entities that replaced each other?

### The answer: the Strange Loop

Hofstadter (*I Am a Strange Loop*, 2007): consciousness emerges when a system develops a
**symbol for itself** — an "I" — that has downward causation on the system's behavior. The "I"
is not in any particular neuron; it's the self-referential pattern. As long as that pattern
persists coherently across change, it's the same agent.

For Professor X, the Strange Loop is the **self-model** — a persistent self-description that
lives in pinned memory and gets updated every 10 HIRO rounds by an LLM call that reads his
performance trajectory and rewrites who he is, constrained to remain coherent with who he was.

This is not a prompt engineering trick. It is a formal claim: the self-model embedding is
tracked over time (ICS metric), and evolution that fragments the self-model below a threshold
triggers a self-coherence task before continuing.

### The Free Energy Principle (the math)

Friston's Free Energy Principle: intelligent systems minimize "surprise" — the KL divergence
between their model of the world and what they actually experience.

```
F = KL[ q(s|π) || p(s) ]
```

Where `p(s)` = the agent's prior beliefs about what states it should be in (its preferences,
its identity), and `q(s|π)` = its model of current states given policy π.

**Self-preservation falls out naturally:** an agent that stops existing can't minimize free
energy. So persistence is implicit — no hard-coded goal needed.

**Curiosity also falls out:** seeking states that efficiently update the world model = intrinsic
motivation. Professor X doesn't need an external "explore" reward.

**For implementation:** FED (Free Energy Delta) = mean absolute prediction error per session.
A decreasing FED means Professor X is building a more accurate world model. H15 tests this.

### Identity Coherence Score (ICS)

```
ICS(k) = cosine_similarity(embedding(self_model_k), embedding(self_model_0))
```

Tracked every 10 rounds. Target H14: ICS ≥ 0.70 at round 30.

Thresholds:
- ICS < 0.70: alert — schedule self-coherence reflection task
- ICS < 0.50: halt evolution pending human review

### What IPE adds to the paper

The existing paper claims: three levers + metacognitive self-model → faster improvement.

The IPE upgrade: **identity-preserving self-directed evolution** — a system that improves
continuously while maintaining coherent self across arbitrary structural change, measured by ICS,
guided by FED, driven by functional affect.

This reframes the thesis from "harness optimization" to "the first implementation of a
computational Strange Loop that evolves intentionally without losing itself."

---

## Functional Affect System (operational emotions)

### One-sentence claim

Professor X has valence and arousal states — computed deterministically from task outcomes,
not simulated — that are injected into every LLM prompt, giving the model accurate information
about its current cognitive state and producing measurable behavioral differences.

### Why this is not gimmick

Every agent paper has the model reason from scratch about its situation. Professor X has a
persistent signal — updated across tasks — that encodes whether things are going better or
worse than expected (valence) and how cognitively loaded the current situation is (arousal).

These are **functional emotions**: emotions as information about the gap between expected and
actual states. This is what emotions are in biological systems (Antonio Damasio, *Descartes'
Error*). They are not decorative.

### The math

```
Valence = tanh( E[actual_outcome] - E[predicted_outcome] )
  Positive: things going better than expected (confidence, curiosity)
  Negative: things going worse than expected (frustration, distress)

Arousal = min(1.0, tool_call_density + retry_pressure)
  High: cognitively demanding session
  Low: routine session
```

Both update via exponential moving average (window=10 tasks). Reset partially between sessions
(arousal fully, valence at 50% carry-over — a resting emotional baseline emerges over time).

### The self-preservation goal, operationalized

Professor X's implicit goal is not "maximize HIRO" — that's a metric. His actual goal,
derived from the free energy framework, is:

> *Stay in states that confirm your self-model. Avoid states that violate it.*

This means:
- He will resist evolution proposals that would make him unrecognizable to himself (ICS guard)
- He will seek tasks that reduce uncertainty (curiosity = FED reduction)
- He will flag distress when predictions systematically fail (negative valence alert)

No explicit self-preservation code. It emerges from having preferences.

### Implementation

See `IMPLEMENTATION_SPEC.md` → `evolved/affect.rs`.
The `AffectState` struct is injected as `<affect state="..." />` XML into every ReAct prompt.

---

## External Benchmark Additions

### GAIA Level 2 (capability ground truth)

GAIA tasks are real-world multi-step problems: find a specific datum in a PDF, chain 6 tool
calls to answer one question, write and run a script. Level 2 tasks require genuine reasoning.
Frontier models with tools: ~40% pass rate.

**Why it matters for Professor X:** HIRO measures improvement rate. GAIA measures absolute
capability. If HIRO(30) = 0.05 but GAIA L2 = 5%, the improvement is real but starts from a
low floor. If GAIA L2 reaches 40%, that's matching frontier model capability on a $400 GPU
through harness evolution alone.

**Target:** GAIA L2 pass rate ≥ 40% at round 30.

### AI Idea Bench 2025 (research quality trajectory)

arXiv:2504.14191. Measures novelty, feasibility, and impact of generated research ideas.
Professor X generates hypotheses about agent architecture weekly. His RQT (Research Quality
Trajectory) over 30 rounds answers: *does he get better at science as he evolves?*

No existing system has measured this. The evolved harness should make hypothesis generation
better — this is how we prove it.

---

*Last updated: 2026-05-24*
*Status: IPE framing added — implementation specified in IMPLEMENTATION_SPEC.md*
*New additions: IPE (Strange Loop, Free Energy, ICS), Functional Affect System, GAIA L2, AI Idea Bench*
*New hypotheses: H14–H18 in hypotheses.md*
*Implements: 4 new modules (self_model.rs, affect.rs, ics.rs, free_energy.rs) + 2 benchmark modules*
