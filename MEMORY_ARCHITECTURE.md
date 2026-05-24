# Professor X — Memory Architecture Research Document
> Status: Pre-implementation research. Ideas to study before Week 3 build.
> This document captures the memory design rethink from the May 2026 session.
> Feed to Professor X on activation — this is part of his research agenda.

---

## The Core Problem

The standard agent memory approach is: retrieve relevant memories → inject into LLM context → call model. Every framework does this. It is fundamentally flawed.

**The context window is a linear, sequential, finite bottleneck.**

Every approach that asks "what should we inject?" is still capped by that bottleneck. You are rearranging furniture inside a room with a fixed ceiling. The right question is:

> **How do we stop needing to inject so much in the first place?**

---

## How LLMs Actually Work (Memory-Relevant)

### Attention is not uniform
Transformers pay disproportionate attention to the beginning and end of context. The middle is diluted. Injecting 30 memory entries means most entries receive ~3% of the model's attention. You are not giving the model more knowledge — you are giving it noise with occasional signal.

**Research to find:** "Lost in the middle" phenomenon (Liu et al., 2023). Needle-in-a-haystack benchmarks. Attention distribution studies for 14B class models.

### KV cache is VRAM
Every token in the prompt becomes a key-value pair cached in VRAM during inference. On the RTX 3060 with ~2.3GB headroom after model weights:
- 8k token context → ~0.5GB KV cache
- 20k token context → ~1.2GB KV cache  
- 32k token context → ~2.0GB KV cache (near limit)

Memory bloat is not an abstract problem. It is a direct compute cost that shrinks the headroom for generation and parallel tool calls.

### Instruction following degrades with context length
Smaller models (14B class) show measurable instruction-following degradation as context length increases. The model attends to memory entries instead of the task instruction. Less context, executed well, outperforms more context, executed poorly.

**Research to find:** Studies on instruction following vs context length for sub-20B models. Anthropic's work on long-context degradation. qwen2.5 technical report context handling.

### Prompt caching is architecture-dependent
Some inference backends (not Ollama default) support KV cache reuse across calls — if the prefix is identical. This has implications: if pinned memory is always the same prefix, it could theoretically be cached across calls. Worth studying for Ollama specifically.

---

## How Human Cognition Works (Memory-Relevant)

### Sparse activation
~86 billion neurons. Only a tiny fraction fire at any moment. The brain does not recall everything when thinking — specific memories activate on cue via associative triggering, then return to latency. Activation is the exception, not the default state.

**Implication:** Most of Professor X's memory should be latent (encoded, searchable) not active (in context). Active memory should be small, sparse, and task-specific.

### Predictive coding
The brain does not react to inputs equally. It constantly predicts what is coming and only updates when there is a prediction error (surprise). You do not consciously process the feeling of your shirt against your skin right now — it is filtered out because it matches expectation. Only surprises reach awareness.

**Implication:** Logging every agent step is wrong. Only surprises should be logged with full fidelity. Expected outcomes should be discarded or compressed.

### The cerebellum pattern
Riding a bike bypasses conscious thought entirely. Procedural memory runs *below* the prefrontal cortex — in the cerebellum and basal ganglia. It is not in working memory while executing. It operates faster, cheaper, and more reliably than conscious deliberation.

**Implication:** Verified skills should not go through the LLM at all. They should execute directly as code. The LLM is only invoked when a skill fails or needs adaptation.

### Working memory: 4 slots, not a list
Human working memory capacity is approximately 4 chunks (Cowan, 2001 — updated from Miller's famous "7±2"). Critically, each slot holds a *chunk* — a compressed, high-level abstraction — not a raw item. "The meeting I had yesterday" occupies one slot, not a 10,000-word transcript of it.

**Implication:** Working memory in Professor X should be 4 high-level chunks, not 20 raw ReAct triples. Before each LLM call, recent steps get compressed into chunks first.

### Forgetting is a feature
Jorge Luis Borges wrote *Funes the Memorious* (1942): a man who could not forget anything. He was cognitively disabled — he could not generalize, could not abstract, could not think efficiently. Every leaf he had ever seen existed as a separate discrete memory. He could not group them into the concept "leaf."

Forgetting enables abstraction. Without forgetting there is no compression. Without compression there is no generalization. Without generalization there is no intelligence — only lookup.

**Implication:** Professor X must actively forget. Not as a failure mode — as a design decision. Episodic memory should compress and eventually discard. Only patterns survive.

### Associative retrieval (not key-value)
Human memories are not stored with primary keys. They are accessed via association — a smell, an emotion, a context. One activated memory spreads activation to related memories through a network. Retrieval is graph traversal, not database lookup.

**Implication:** Memory should be a graph where nodes are memories and edges are associations. Retrieval follows the graph. One relevant memory activates its neighbors. Context injection becomes a subgraph, not a flat list of top-k results.

### Emotional salience tagging
The amygdala tags memories with emotional/importance weight at write time. High-salience events are encoded more strongly and retrieved more reliably. The brain has a write-time importance signal that is not just "how relevant is this to my current query."

**Implication:** Importance scoring at write time (which we have) is correct. But the signal should be richer — not just LLM-assessed importance but also: was this a failure? A surprise? A novel finding? These are the memories worth keeping.

---

## Current Design Failure Modes

### Failure 1: Flat injection
CLAG two-stage retrieval gives "top 5 from relevant clusters" — still a flat list in context. Items compete for attention. No hierarchy. No compression. No relationship between items.

### Failure 2: Logging everything episodically
Every ReAct step gets written to episodic memory. In a 7-hour day with dozens of tasks that is hundreds of entries. After a week: thousands. After a month: tens of thousands. Retrieval from a flat store of 50,000 entries is expensive and increasingly noisy.

### Failure 3: Unbounded working memory
Current `working.rs` has a `VecDeque<String>` capped at 20 raw step strings. 20 full ReAct triples (Thought + Action + Observation) at ~100 tokens each = 2,000 tokens of working memory alone, before any retrieved memory or pinned context.

### Failure 4: Pinned injected always
Professor X's identity, goals, constraints, and persona get injected on every single LLM call. Including mechanical calls like file writes, git commits, and shell commands. The model does not need to know it is an academic researcher to run `git commit`.

### Failure 5: Memory is passive
Current design: memories are stored, retrieved, injected. They don't interact. They don't build on each other. There is no consolidation, no compression over time, no pattern extraction. Memory grows linearly forever with no active management.

### Failure 6: No novelty tracking
We score memory by recency and importance. We don't score by *novelty*. A memory that has been injected 50 times this week is not adding new information anymore — the model has effectively internalized it. But we keep injecting it at full weight.

---

## Proposed Solutions

### Solution 1: The Cerebellum Layer
**Status:** Architecturally ready, needs implementation in `toolbridge`

Verified procedural skills (verification_score > 0.85, times_succeeded > 10) bypass the LLM entirely. `toolbridge` detects that the requested skill is cerebellar and executes the script directly, returning an Observation without an Ollama call.

The LLM is only invoked when:
- The skill fails (error in execution)
- The skill needs to be adapted (parameters outside normal range)
- The skill is not yet verified (new skill, learning phase)

**Savings:** Eliminates LLM calls for all routine mechanical operations. On a day where Professor X runs 30 verified skills, that could be 20+ Ollama calls saved.

**Research needed:** How Voyager determines skill verification threshold. Whether the 0.85 score is the right threshold or needs empirical tuning.

---

### Solution 2: Surprise-Based Logging (Predictive Coding)
**Status:** Not in current design — new addition

Before executing an action, the LLM states its predicted observation in the Thought step (it already does this implicitly in ReAct — make it explicit). After execution, compare prediction vs actual observation using embedding cosine similarity.

```
surprise_score = 1 - cosine(embed(predicted_obs), embed(actual_obs))
```

Storage decision:
- `surprise_score < 0.2` → low surprise → discard or store compressed (one line)
- `surprise_score 0.2–0.6` → medium surprise → store summarized
- `surprise_score > 0.6` → high surprise → store in full detail, high importance

**Effect:** After a week of operation, instead of 5,000 episodic entries you have ~500 — the times something genuinely unexpected happened. Signal-to-noise ratio of episodic memory increases dramatically over time.

**Research needed:** Predictive coding in neuroscience (Karl Friston's free energy principle). Whether explicit prediction extraction from ReAct Thought steps is feasible. Embedding similarity as a surprise metric — calibration.

---

### Solution 3: Memory as a Queried External System
**Status:** Architectural rethink — most radical idea

Instead of injecting memories into the main LLM context, give the LLM a `memory.query(question)` tool and inject almost nothing.

```
Main LLM context (lean):
  [identity core: ~150 tokens — always]
  [current task: ~100 tokens]
  [current step history: 4 chunks, ~200 tokens]
  
  Tool available: memory.query("what do I know about X?")
```

The main model asks questions. A memory subsystem (smaller model, or FTS + extractive summarization) answers with a concise response — not raw entries, the *answer*. The main model gets only what it actively requests.

**Advantages:**
- Main context stays under 1,000 tokens for most calls
- Model only retrieves what it actually needs for the current step (not what the harness predicts it might need)
- Memory system can be upgraded independently of the main model
- The act of querying is itself a signal — what the model asks for tells us what's relevant

**Disadvantages:**
- Adds latency (extra tool call per retrieval)
- Requires the model to know it *needs* to query (may miss relevant memories it doesn't think to ask about)
- More complex architecture

**Hybrid approach:** Inject a small pinned "memory index" — a one-line description of each memory category — so the model knows what it *can* query, without the full content being in context.

**Research needed:** RAG vs in-context injection benchmarks. Tool-augmented memory systems (MemGPT, arXiv:2310.08560 — Packer et al.). Whether qwen2.5:14b reliably uses memory tools without explicit prompting.

---

### Solution 4: Temporal Compression (Sleep Consolidation)
**Status:** Nightly job to add in Week 3

After each daily cycle, a background consolidation pass rewrites episodic memory by age:

```
Today's entries:        full detail (stored as-is)
Yesterday's entries:    LLM summarizes clusters into paragraphs
Last week's entries:    one-line abstracts per session
Last month's entries:   keywords + outcome score only
Older:                  discarded unless flagged as permanently significant
```

This runs during Professor X's off-hours (after the 14:00 daily commit, before the next 06:00 start).

The compression itself uses the LLM — but a cheap, short call. One summary call per session-cluster costs far less than injecting thousands of raw entries across hundreds of future calls.

**Research needed:** What compression ratio is acceptable before information loss becomes significant. Whether LLM-generated summaries introduce hallucination into compressed memories. AutoCompressors paper (arXiv:2305.14788 — Chevalier et al.) — soft prompt compression approach.

---

### Solution 5: Chunked Working Memory (4-Slot Model)
**Status:** Rewrite of `working.rs` — straightforward

Cap working memory at 4 chunks. Before each LLM call, compress recent steps into chunks using a lightweight summarization call or a rule-based compressor.

```
Chunk 1: Goal context    — "Researching AHE paper for harness taxonomy section"
Chunk 2: Progress so far — "Found 3 relevant papers, extracted 7 key claims"
Chunk 3: Last action     — "Fetched full AHE paper, identified 3-pillar model"
Chunk 4: Immediate next  — "Writing synthesis note on component observability"
```

~150 tokens vs 2,000 tokens for 20 raw triples.

The 4-slot model is backed by cognitive science (Cowan 2001). It's not arbitrary — it reflects a real constraint in working memory that produces better thinking by forcing compression and prioritization.

**Research needed:** Cowan (2001) "The magical number 4 in short-term memory." Whether rule-based chunk compression is sufficient or LLM-based compression is needed. Chunk formation strategies.

---

### Solution 6: Differential Pinned Injection
**Status:** Config change to `memd/pinned.rs` — low complexity

Not all pinned memory is needed on every call. Tag each pinned entry with an injection scope:

```
identity_core       → always inject (who Professor X is: ~150 tokens)
research_context    → inject during: Research, Writing, Synthesis tasks
operational_rules   → inject during: Experiment, Tool-use tasks
hardware_constraints → inject during: Evolution proposals only
```

The LLM doesn't need Professor X's full academic identity to write a file. It needs `identity_core` and nothing else. Saves 300-500 tokens per mechanical call.

**Implementation:** Add a `scope` field to `PinnedEntry`. `build_context_prefix()` takes the current `TaskType` and filters accordingly.

---

### Solution 7: Memory Graph (Associative Retrieval)
**Status:** Future architecture — Week 4+, research first

Replace flat episodic/semantic stores with a graph where:
- Nodes are memories (episodic, semantic, cognition items)
- Edges are typed associations: temporal (happened near), causal (led to), topical (about same subject), outcome (preceded success/failure)

Retrieval becomes graph traversal from a seed node. One relevant memory activates its neighbors with decaying weight. The retrieved "context" is a small subgraph, not a ranked list.

**Why this matters:** Current vector similarity retrieval treats every memory as independent. But memories have structure. "The experiment on harness config failed" is related to "harness config entries in the cognition base" is related to "the AHE paper on config as an evolvable component." Graph traversal finds this chain. Vector search may not.

**Research needed:** Knowledge graph memory for agents. GraphRAG (Microsoft, 2024). MemoryBank paper. Whether a graph structure can be maintained efficiently in SQLite (it can — just add an `associations` table). Spreading activation models in cognitive science.

---

### Solution 8: Novelty Decay on Injection Weight
**Status:** Add to retrieval scoring — low complexity

Track how many times each memory entry has been injected this session in a session-scoped counter. Apply a novelty decay to the retrieval score:

```
effective_score = retrieval_score × (1 / (1 + injection_count))
```

A memory injected 0 times: full score.  
A memory injected 5 times this session: score × 0.17.  
A memory injected 10 times: score × 0.09 → effectively deprioritized.

The rationale: if we've injected this memory 10 times and the model keeps producing good outputs, it has effectively internalized it. Continuing to inject it wastes tokens.

**Implementation:** Add `session_injection_count` to working memory's tracking. Update retrieval scoring formula.

---

## The Novel Contribution: Meta-Memory Management

None of the existing self-evolving agent systems study how their memory management strategy performs over time and evolve it.

**The proposed contribution:** Professor X's `evolved` module monitors memory system performance and proposes changes to the memory management strategy itself as harness-level evolution proposals.

Metrics to track:
- Which retrieved memories are actually referenced in model outputs (requires attention tracing or output parsing)
- Rate of prediction errors (surprise scores) — indicates whether current memory is helping the model predict correctly
- KV cache size per call over time — tracks context bloat
- Token cost per unit of task progress — efficiency metric
- How often the model issues `memory.query()` calls vs ignores injected context

Evolution proposals the system could generate:
- "Increase compression threshold for episodic entries older than 3 days — storage cost is high, retrieval quality is not improving"
- "Disable pinned injection of research_context for shell execution tasks — the LLM never references it in those outputs"
- "Lower cerebellum threshold for `px-daily-update` skill — it has 97% success rate with zero LLM adaptations needed"

This is memory management as a first-class evolvable component. The system learns how to remember, not just what to remember.

**Survey confirmation:** None of arXiv:2507.21046, arXiv:2508.07407, or arXiv:2604.08224 describe a system that evolves its own memory architecture. This gap is confirmed.

---

## Papers to Read (Priority Order)

### Tier 1 — Read before touching memory architecture
| Paper | Why |
|---|---|
| MemGPT (arXiv:2310.08560) | Closest existing system to Solution 3 (memory as queried external system). Must understand this before designing the query interface. |
| Lost in the Middle (Liu et al., 2023) | Empirical evidence for attention non-uniformity. Quantifies the middle-context problem. |
| Cowan (2001) "The magical number 4" | Cognitive science basis for 4-slot working memory. The number matters. |
| AutoCompressors (arXiv:2305.14788) | Soft prompt compression — may be relevant for chunk compression without extra LLM calls. |
| GraphRAG (Microsoft, 2024) | Graph-structured retrieval. Most complete implementation of Solution 7. |

### Tier 2 — Read during Week 3 build
| Paper | Why |
|---|---|
| Karl Friston — Free Energy Principle | Theoretical basis for predictive coding (Solution 2). Dense but foundational. |
| Memento (arXiv:2508.16153) | Already in architecture doc — agent optimization without weight updates. Closest to evolved module approach. |
| MemoryBank | Long-term memory management for LLM agents. Compare against our design. |
| Cognitive Architectures survey (2023) | Broader survey — how ACT-R, SOAR handle memory. May reveal patterns we've missed. |

### Tier 3 — Background reading
- Baddeley (2000) — Working memory model (the original cognitive science paper)
- Borges, "Funes the Memorious" (1942) — Read it. Seriously. Short story, 10 pages. Captures the forgetting-as-feature insight better than any academic paper.
- Ebbinghaus forgetting curve — mathematical model of memory decay. Basis for spaced repetition. Relevant to temporal compression.

---

## Open Questions for Professor X's Research

1. What is the right compression ratio for episodic memory? At what age does an entry lose enough marginal value to justify compression overhead?

2. Can prediction errors be reliably extracted from existing ReAct Thought steps, or does the prompt need to explicitly elicit predictions?

3. Is 4 the right number for working memory chunks in LLM agents, or does the cognitive science number not transfer? What does empirical testing on qwen2.5:14b show?

4. What is the minimum viable pinned context for Professor X's identity? Can it be compressed below 100 tokens without losing behavioral coherence?

5. Does `memory.query()` as a tool actually improve task performance on 14B models? MemGPT found it works — but they used GPT-4. Does it hold at smaller scales?

6. Can a graph-based memory structure be maintained efficiently in SQLite without a dedicated graph database? What are the query performance characteristics at 10,000 nodes?

7. What is the right threshold for cerebellar treatment of procedural skills? Is verification_score > 0.85 appropriate, or should it be adaptive?

---

## Proposed Architecture Changes (To Be Decided After Research)

| Change | Complexity | Impact | Priority |
|---|---|---|---|
| Differential pinned injection | Low | Medium — saves 300-500 tokens/call | Week 3 |
| Chunked working memory (4 slots) | Medium | High — saves ~1,800 tokens/call | Week 3 |
| Surprise-based episodic logging | Medium | High — prevents long-term bloat | Week 3 |
| Novelty decay on injection weight | Low | Medium — reduces redundant injection | Week 3 |
| Cerebellum layer in toolbridge | Medium | High — eliminates LLM calls for verified skills | Week 3 |
| Temporal compression (nightly job) | Medium | High — manages long-term growth | Week 4 |
| Memory as queried external system | High | Very High — fundamental bottleneck removed | Week 4+ |
| Memory graph (associative retrieval) | High | High — better retrieval quality | Week 4+ |
| Meta-memory evolution in evolved | Very High | Novel contribution — the thesis | Week 6+ |

---

*Document version: 1.0*
*Session: May 2026*
*Status: Research phase — no implementation until questions above are answered*
*Next action: Professor X reads this on activation and adds it to his research agenda*
