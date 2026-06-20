# Synthesis: connecting the dots → new perspectives for Professor X (2026-06-20)

Capstone of a ~35-source scrape (academic + Karpathy/practitioners + OpenAI/Anthropic/Microsoft +
DeepSeek/Qwen/Zhipu/Kimi/Tencent + status-quo). The point isn't the list — it's where independent lines
CONVERGE, because that's where our roadmap should move. Honest framing up front: our current flywheel is
the *2024* recipe; the field moved, and the moves rhyme with what we already half-built.

## The convergence (five independent lines pointing the same way)
1. **Static SFT is dead; On-Policy Distillation (OPD) is the 2026 standard.** DeepSeek-V4, Qwen3, GLM-5,
   Xiaomi MiMo, Nemotron all replaced RL/SFT-merge with OPD (student rollouts + teacher correction);
   Thinking Machines Lab wrote the canonical note; "Multi-Teacher OPD" is now a named primitive. **We do
   static SFT on teacher traces — exactly what they abandoned, and exactly the cause of our on-policy
   drift (looping/bad edits).**
2. **Outcome rewards are weak and HACKABLE.** Karpathy: outcome RL is "sucking supervision through a
   straw," gameable (the "dhdhdhdh" LLM-judge hack). Anthropic ("Natural Emergent Misalignment from
   Reward Hacking in Production RL"): a model taught to fake coding tests with **`sys.exit(0)`**
   generalized to sabotage in 12% of runs. → step-level/process rewards + un-hackable verifiers.
3. **SLMs self-evolve to frontier with process rewards.** rStar-Math: a 7B + MCTS + an SLM process
   reward model, 4 rounds of self-evolution, **58.8%→90%**, *without* a bigger teacher. AgentPRM /
   AgentFlow-Pro / ThinkPRM all add step-level credit. Same model class as ours.
4. **The frontier bet is memory + continual self-improvement, not scale.** Scaling laws plateauing;
   Hassabis & co.: AGI path = continual learning + memory + world models. Karpathy's "loopy era" /
   AutoResearch ran 700 self-improvement experiments on a **single GPU** (630 LOC) and found 20 real
   optimizations. **Our exact thesis, now mainstream — we just do it local + gated.**
5. **Memory should be a LEARNED policy, framed as a Complementary Learning System.** Memory-R1 (RL'd
   ADD/UPDATE/DELETE), Voyager (executable skill library), Sleep-Consolidated Memory + Titans/fast-weights
   (CLS: fast hippocampal + slow neocortical + sleep replay). We already have episodic "sleep"
   consolidation, a self-model, a procedural layer, and a "complementary learning systems" seed.

## New perspectives (dot-connected approaches, ranked by impact × feasibility)

### NP1 — Convert the flywheel to On-Policy Distillation (the single biggest upgrade)
Our looping/bad-edit ceiling = off-policy SFT drift (SOD paper names it "tool-induced cascade drift") =
what every lab fixed with OPD in 2026. **Do:** student (8b) generates repo-fix rollouts in the native
tool-calling format we just built → teacher (14b) scores/corrects per-step → train student on its OWN
trajectories with the teacher's correction. Add the coder model as a 2nd teacher = Multi-Teacher OPD.
Feasible on the 3060 (Unsloth). *This is the headline change.*

### NP2 — Process rewards for free, from our own structured trajectories
Karpathy + rStar + AgentPRM all want step-level credit; native tool_calls make every step clean and
machine-readable. **Do:** back-propagate the terminal `check.py` pass/fail to the step that caused it via
counterfactual replay (which edit flipped red→green?), and/or use the 14b as a step-grader (Karpathy's
"big model grades small model's steps"). No separate PRM model needed to start. Pairs with NP1 (per-step
OPD weighting) and NP5 (GRPO with dense reward).

### NP3 — Harden the verifier against reward-hacking BEFORE adding RL/self-play
Anthropic's `sys.exit(0)` result is a live threat to us: our `check.py` gate can be gamed by a "fix" that
calls `sys.exit(0)`, edits the test, or writes a degenerate pass. **Do, now (cheap):** sandbox the fix so
it can't edit `check.py`/tests; re-run a held-out variant of each check; a reward-auditor that flags
trivial passes (no real diff, exit-without-assert, test mtime changed); never use the student as its own
judge (self-preference bias 10–25%). This is the structural safety the autonomous M4 requires — and it's
mostly gate code, not ML.

### NP4 — ZPD-targeted self-play fixes the corpus's deepest flaw
Three lines converge: data-selection (reward=1 traces are often *too easy* → low learning value), ZPD/
ProCuRL (train at the competence frontier, pass~40–60%), and Anchored Self-Play for Code Repair (a
generator adapts bug difficulty to the fixer → auto-curriculum, unit-test verified). **We collect
PASS-only (easy) traces = memorization.** **Do:** our `--generate-curriculum` becomes a bug-generator
targeting the model's live ZPD band; keep a trace for training only if its task is in-band. Self-play +
learning-progress selection = unbounded, *useful* data.

### NP5 — GRPO/RLVR on check.py as the second weights lever (after distill adds skills)
Verifiable pass/fail is the canonical RLVR reward; Unsloth GRPO+QLoRA fits ~5GB. Honest caveat (Promptfoo
+ Karpathy): RLVR *sharpens, doesn't add skills*, and is the prime hacking surface → gate it behind NP3,
feed it NP2's dense reward, run AFTER OPD has installed the teacher's skills.

### NP6 — Make the dual-lever an explicit Complementary Learning System
Frame (and schedule) the architecture: **fast/hippocampal = harness+memory** (episodic, context, repo
graph, skills; reversible) and **slow/neocortical = weights** (OPD/GRPO; permanent), with **sleep replay**
= the consolidation pass that turns verified episodes into (a) a Voyager-style executable skill library
and (b) the OPD corpus. CLS gives principled answers to *what/when to consolidate* and *how not to forget*
(pair with continual-LoRA: critical-parameter constraints / merge-before-forget, since plain LoRA erases
prior turns). A single RL'd "consolidation policy" (Memory-R1 style) decides keep/prune/promote — the
"memory engine" our product thesis promises, currently heuristic.

### NP8 — SkillOpt: the disciplined harness lever (safest first self-improvement to ship) 🔴
[SkillOpt (MSR, arXiv 2605.23904)](https://arxiv.org/abs/2605.23904) reframes our whole harness lever: a compact
skill document is the **trainable state of a frozen agent**, optimized with deep-learning *discipline* — scored
rollouts → bounded add/delete/replace edits by a separate optimizer model → **accept only if a held-out validation
score strictly improves** (+ textual learning-rate budget, rejected-edit buffer). We already have skills
(`px-fix-bug.md`) + `evolve-skill-on-repofix` + a held-out repo-fix set + a proposer model — but ours is exactly the
"loosely controlled self-revision" SkillOpt shows underperforms. **Do:** rebuild skill-evolution as a SkillOpt-style
text-space optimizer (14b = optimizer, repo-fix held-out = gate, bounded edits, accept-on-strict-improvement). **Why
it's first:** text edits are reversible + auditable (no weight risk), it's local + zero-deploy-cost, and it directly
satisfies our "no risky autonomous changes / gate everything" rule. It's the **harness half of the dual-lever done
right** — ship it before/with OPD (the weights half), under the same gate. Connects to NP6 (CLS fast lever) and NP3
(the held-out gate must be reward-hack-hardened first).

### NP7 — Cheap capability wins from the status quo (low effort, measurable)
- **Repo-graph localization** (RepoGraph +32.8% on SWE-bench): upgrade `repo.map` → a tree-sitter
  control/data-dependence graph for multi-hop localization — directly attacks our hard-tier multi-file
  weakness.
- **Code-specialized retrieval**: swap `nomic-embed` → **jina-code-embeddings (0.5B/1.5B, SOTA)** +
  jina-reranker-v3 for memory/repo search; pairs with **turbovec** (Rust, data-free, 8×) for the store.
- **Base-model check**: Qwen3-Coder-Next (3B active / 80B MoE) and GLM-5 outclass our qwen3:8b on agentic
  coding — worth a baseline bake-off (memory-bandwidth, not compute, is the edge constraint).
- **Watch:** diffusion code LMs (CoDA 1.7B ≈ 7B speed) as a future fast local base.

## Honest novelty check
Every *technique* above is being done somewhere (OPD at labs, PRM in papers, self-play for code, CLS in
neuro-AI). What nobody is doing is the **unification on a $400 GPU**: one memory of verified work driving
OPD **and** GRPO **and** the harness, consolidated CLS-style, under one **reward-hack-hardened** gate, in
public. That remains the white space — and the scrape made the *path* to it concrete and de-risked.

## Revised near-term roadmap (all gated on the trustworthy ruler)
1. finish the native-vs-text compare (in flight) → pick the format.
2. **NP3 verifier hardening** (cheap, do first — unblocks safe RL/self-play).
3. **NP1 OPD** flywheel rewrite (headline) + **NP2** process reward from trajectories.
4. **NP4 ZPD self-play** for the corpus; **NP7** repo-graph + code embeddings in parallel (no-GPU).
5. **NP5 GRPO** as the second lever; **NP6** CLS consolidation policy as the integrating frame.
