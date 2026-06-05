# Agent-Harness Landscape + the Self-Distillation Flywheel
### Make Professor X the best combination of everything, then let the harness teach the model

**Author process:** competitive synthesis across the agent-harness field +
design of a harness→model distillation loop.
**Epistemic tags:** [ESTABLISHED] mainstream/attributable, [HAVE] Professor X
already does it, [GAP] missing, [NEW] genuinely new proposal.

---

## Part 1 — What every harness does best (and where Professor X stands)

Each major system contributed one load-bearing idea. The best harness is the
*combination*, not any single one.

| System | The one idea it nailed | Professor X status |
|--------|------------------------|--------------------|
| **ReAct** (Yao 2022) | Thought→Action→Observation interleaving | [HAVE] core loop |
| **Reflexion** (Shinn 2023) | verbal self-reflection after failure, kept in a buffer | [HAVE] MARS + reflection buffer |
| **Voyager** (Wang 2023) | verified **skill library** + **automatic curriculum** + iterative prompting | [HAVE] skill library/verification; **[GAP] automatic curriculum** |
| **Tree of Thoughts** (Yao 2023) | deliberate **search** over multiple reasoning paths, backtracking | **[GAP]** — Professor X commits to one path per step |
| **AutoGPT / BabyAGI** (2023) | autonomous recursive task decomposition | [HAVE] task graph (cleaner) |
| **MetaGPT** (Hong 2023) | **role specialization** = SOPs encoded in the harness | partial (conductor skills) |
| **AutoGen / CrewAI** (2023-24) | **multi-agent** conversation/collaboration | **[GAP]** — single agent |
| **LangGraph** (2024) | explicit **graph state machine**, controllable/inspectable flow | [HAVE] agentd graph (less explicit) |
| **SWE-agent** (Yang 2024) | the **Agent-Computer Interface** matters as much as the model — design tools *for the agent*, not for humans | **[GAP]** — tool descriptions are human-style |
| **OpenHands / Devin** (2024) | sandboxed real-software execution, event-stream architecture | [HAVE] sandbox verify + event stream |
| **Aider / Cursor / Claude Code** (24-25) | **repo-map** + **precise diff editing** + tight human feedback | partial (patch.apply); **[GAP] repo-map** |
| **MemGPT / Letta** (2023-24) | **memory as an OS** the agent self-pages (memory-as-tool) | [HAVE] memory.read tool; **[GAP] self-paging / self-editing memory** |
| **MOSS** (2026) | source-level harness **self-rewriting**, verify-then-commit, health-probe rollback | [HAVE] **+ exceeds** (identity gate, consumer HW) |
| **ASI-Evolve** (2026) | Researcher/Engineer/Analyzer + cognition base + UCB1 | [HAVE] |
| **Meta-Harness / Harbor** (2026) | LLM/Bayesian harness optimization | [HAVE] DHE-guided (more targeted) |
| **STaR / ReST / SDAR** (22-26) | **self-distillation**: train the model on its own successful trajectories | **[GAP] — Lever 1, not yet built** |

### What Professor X already has that NO other harness has [HAVE, unique]
- Self-evolution **with identity preservation** (ICS gate / Noether-charge framing)
- **Metacognitive self-model** + DHE causal failure attribution
- **Consciousness instrumentation** (Φ, interoception, self-prediction, FED)
- **`meta.observe`** — recursive self-perception (reads its own processing stream)
- Verify-then-commit on **consumer hardware** ($400 GPU)

That combination exists nowhere else. The gaps below are what would make it the
strict superset.

### The "best combo" — what to integrate, ranked by ROI × buildability
1. **[GAP→build] Self-distillation (Lever 1).** The biggest lever and the user's
   target — see Part 2. This is the path from 8B to frontier-like behavior.
2. **[GAP→build] Voyager automatic curriculum (ZPD).** Self-authored tests
   already exist; close the loop so the agent sets itself tasks at the edge of
   ability. Curriculum is what turns trajectories into *good* training data.
3. **[GAP→build] SWE-agent ACI principle.** Redesign tool descriptions *for the
   agent's failure modes*, not human readability. We already saw the agent
   misuse tools (bare `awk`, wrong paths). Tools should be agent-shaped.
4. **[GAP→build] MemGPT self-paging memory.** Let the agent decide what to load
   into context and what to evict — memory as an actively managed resource, not
   passive injection. Directly raises the H1 context-efficiency thesis.
5. **[GAP→build] Tree-of-Thoughts search** for the hard planning/self-correction
   tasks (where pass@3 is lowest). Deliberate branching beats single-path on
   exactly the tasks we fail.
6. **[GAP→build] Multi-agent mirror** (the relational-self direction) — later;
   highest novelty, needs the single-agent loop solid first.

---

## Part 2 — The Self-Distillation Flywheel: the harness teaches the model

**The user's thesis, made precise:** *the harness should fine-tune and distill
the model so an 8-9B model behaves like a frontier model.* This is real,
grounded, and the most important lever the project has not yet built.

### Why it works — the mechanism [ESTABLISHED + NEW framing]
A frontier model is, functionally, a model that has *internalized* good
reasoning, planning, tool-use, and self-correction — it produces them without
external scaffolding. An 8B model has the raw capability but *not the disposition*
to deploy it reliably. The harness supplies the disposition *externally*: DHE
forces causal attribution, MARS forces reflection, the self-model forces
self-monitoring, `meta.observe` forces self-perception.

[NEW framing — Vygotsky's Zone of Proximal Development applied to model training]
This is **scaffolding the learner can internalize.** Vygotsky: a teacher
scaffolds a skill in the learner's ZPD; the learner internalizes the scaffold and
eventually performs without it. **The harness is the teacher; the model is the
learner; the trajectories are the lesson; fine-tuning is internalization.** After
distillation the model carries the harness's metacognitive disposition *in its
weights* — and needs less scaffold to perform the same way. That is exactly
"8B acts like frontier": not more parameters, but internalized disposition.

### The flywheel [NEW architecture — combines STaR/ReST/SDAR with the harness]
```
   harness makes 8B perform well (scaffolding)
        │
        ▼
   collect SUCCESSFUL trajectories  ──►  these are the lesson:
   (full Thought/Action/Observation traces           the model's OWN good outputs,
    that solved a task, verified correct)             produced under scaffolding
        │
        ▼
   curate (Part 1 #2: curriculum + quality filter)
   - only verified-correct, only above-σ improvements
   - include the metacognitive moves: DHE attributions,
     MARS reflections, meta.observe self-corrections,
     self-model snapshots
        │
        ▼
   QLoRA fine-tune qwen3:8b overnight on its own best trajectories
   (fits the 3060's ~5GB VRAM headroom — already in the hardware budget)
        │
        ▼
   model internalizes the disposition → needs less scaffold →
   produces better trajectories with the SAME harness →
        │
        └──────────────────────────► repeat. each turn lifts the floor.
```

### Why distilling THIS harness is different from generic STaR [NEW]
Generic self-distillation (STaR, ReST) distills *task answers*. Distilling
Professor X distills **metacognition**: the training data contains the agent's
*causal self-diagnoses* (DHE), its *reflections* (MARS), and its *self-perception*
(meta.observe) — not just "the answer was X" but "I was looping; I noticed;
I changed approach; that worked." **Fine-tuning on self-perception trajectories
bakes self-modeling into the weights** — the model becomes intrinsically
metacognitive, not just harnessed into being so. This is the bridge from the
consciousness work to capability: *the same self-perception data that measures
proto-consciousness is the highest-value training signal.*

### The conserved-identity safety constraint carries over [NEW link]
Fine-tuning changes the model weights — Lever 1. By the Conserved-Boundary
theory, identity must survive this. The constraint: distillation must preserve
the self-charge (ICS) — fine-tune toward better *behavior* while the self-model's
conserved invariant holds. If a fine-tune drops ICS discontinuously, it is an
identity-death event at the weight level and must be rejected, exactly as the
persona-overwrite was at the harness level. **The same Noether-charge gate
governs both Lever 3 (harness) and Lever 1 (weights).** This unifies the safety
story across all three levers.

### Concrete build plan (after σ — which we now have)
1. **Trajectory collector** — when a HIRO/mine task is verified-correct AND the
   round beat σ, serialize the full Thought/Action/Observation trace (with the
   DHE/MARS/meta.observe annotations) to a `trajectories/` corpus in
   instruction-tuning format. [pure code, parallel-safe]
2. **Curriculum + quality filter** — keep only above-σ, verified, diverse traces;
   weight by self-authored-test category to cover weak areas (ZPD). [code]
3. **QLoRA trainer** — `unsloth`/`peft` overnight fine-tune of qwen3:8b on the
   corpus; export a LoRA adapter; serve via Ollama (`ollama create` from a
   Modelfile with the adapter). [needs GPU, overnight, fits 3060]
4. **ICS-gated acceptance** — measure ICS + pass@3 of the fine-tuned model on the
   frozen subset; accept only if pass@3 beats baseline by > MDE (0.033) AND ICS
   stays ≥ 0.70. Else reject the adapter. [code + one measurement round]
5. **Loop** — better model → collect better trajectories → re-distill. The
   flywheel. Each turn is an overnight cycle on the 3060.

### The claim this tests
> A small model, fine-tuned repeatedly on its own *harness-scaffolded,
> metacognition-rich* trajectories, converges toward frontier-like behavior on
> the task distribution — and the gain is largest when the distilled traces
> include self-perception, not just answers.

If true: the harness is not a permanent crutch but a **teacher that works itself
out of a job** — and the resulting small model is portable, cheap, and carries
the metacognitive disposition in its weights. That is the strongest possible form
of "the harness is the intelligence": the harness *transfers* its intelligence
into the model.

---

## Part 3 — Honest gaps in our research so far (the user is right)
- We measured σ on a biased 20-task subset (all tool_use); a clean full-60
  baseline is still owed for a headline capability number.
- The self-distillation loop is designed, not built or tested. Steps 1-2 are
  pure code (buildable now); 3-4 need an overnight GPU run.
- Tree-of-Thoughts, self-paging memory, and the ACI redesign are identified
  gaps, not yet specced in detail.
- Multi-agent mirror remains the highest-novelty, least-explored direction.

## The one sentence
> Make Professor X the strict superset — every harness's best idea plus
> self-evolution, self-perception, and identity preservation no one else has —
> then close the loop the field has only done shallowly: **let the harness
> distill its own metacognition into the model, so an 8B carries frontier-like
> disposition in its weights, with identity conserved across the fine-tune.**
