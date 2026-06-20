# MASTER research index — every finding, source, and what to steal (2026-06-20)

One-stop index of the ~35-source scrape. Format per entry: **[title](url)** — finding → **STEAL:** what we
take for Professor X. Companion synthesis (the dot-connecting) is in `2026-06-20-synthesis-new-perspectives.md`.
Priority tags: 🔴 do-now · 🟠 soon · 🟢 later/context.

---

## 1. On-Policy Distillation (OPD) — the 2026 post-training standard 🔴
- [A Survey of On-Policy Distillation (arXiv 2604.00626)](https://arxiv.org/abs/2604.00626) — organizes OPD by
  feedback signal / teacher-access / loss scope. → **STEAL:** the design taxonomy for our OPD rewrite.
- [Thinking Machines Lab — On-Policy Distillation](https://thinkingmachines.ai/blog/on-policy-distillation/) —
  canonical method: train student on ITS OWN rollouts with teacher per-token/step correction. → **STEAL:** the core recipe.
- [SOD: Step-wise On-policy Distillation for SLM Agents (arXiv 2605.07725)](https://arxiv.org/html/2605.07725v1)
  — reweights teacher guidance by step-level divergence to stop **tool-induced cascade drift** (our looping). →
  **STEAL:** step-weighted OPD for the agent loop.
- [TCOD: Temporal Curriculum in OPD (arXiv 2604.24005)](https://arxiv.org/html/2604.24005v3) — order steps by a
  temporal curriculum. → **STEAL:** curriculum ordering of trajectory steps.
- DeepSeek-V4 OPD ([phemex](https://phemex.com/news/article/deepseek-v4-adopts-onpolicy-distillation-integrates-expert-models-75479),
  [kili](https://kili-technology.com/blog/data-story-deepseek-v4)) — domain specialists merged via OPD from **10+
  teachers** + a Generative Reward Model. → **STEAL:** multi-teacher OPD (our 14b + coder), generative reward idea.
- [awesome-on-policy-distillation](https://github.com/chrisliu298/awesome-on-policy-distillation) — curated OPD
  papers/tools; MOPD adopted by Qwen3/DeepSeek-V4/MiMo/GLM-5/Nemotron. → **STEAL:** reference + validation we're on-trend.

## 2. RL with Verifiable Rewards (RLVR) / GRPO — second weights lever 🟠
- [awesome-RLVR](https://github.com/opendilab/awesome-RLVR) — RLVR landscape; pass/fail unit tests = canonical reward.
  → **STEAL:** check.py IS our verifiable reward.
- GRPO (DeepSeekMath/R1) — de-facto RLVR algo, no value model (group-relative advantages). → **STEAL:** the algorithm.
- [Unsloth GRPO docs](https://unsloth.ai/docs/get-started/reinforcement-learning-rl-guide) +
  [memory-efficient RL](https://unsloth.ai/docs/get-started/reinforcement-learning-rl-guide/memory-efficient-rl) —
  GRPO+QLoRA in **~5GB**, 8B fits 24GB; model-params≈VRAM. → **STEAL:** feasible on our 3060; the recipe.
- [Promptfoo — RLVR makes models faster, not smarter](https://www.promptfoo.dev/blog/rlvr-explained/) — RLVR sharpens
  & collapses the distribution; doesn't add skills. → **STEAL:** honest caveat → pair RL with distillation, never replace.
- [ARLArena: Stable Agentic RL (arXiv 2602.21534)](https://arxiv.org/pdf/2602.21534);
  [VerlTool (arXiv 2509.01055)](https://arxiv.org/pdf/2509.01055) — stable agentic-RL frameworks. → **STEAL:** infra patterns.
- [An Imperfect Verifier is Good Enough (arXiv 2604.07666)](https://arxiv.org/pdf/2604.07666) — noisy rewards still
  train well. → **STEAL:** tolerate imperfect check.py; don't over-engineer the verifier.

## 3. Process rewards & SLM self-evolution 🔴
- [rStar-Math (arXiv 2501.04519)](https://arxiv.org/pdf/2501.04519) — 7B + MCTS + an SLM process-reward model, 4
  self-evolution rounds, **Qwen2.5-7B 58.8%→90%, no bigger teacher**. → **STEAL:** the self-evolution loop template
  (search + PRM + verifier), proof our model class can bootstrap.
- [AgentPRM (WWW 2026)](https://dl.acm.org/doi/10.1145/3774904.3792551) — step-wise "promise & progress" rewards. →
  **STEAL:** step-level credit signal.
- [AgentFlow-Pro (ICLR 2026)](https://github.com/awesome-pro/agentflow-pro) — Qwen3-8B **Planner** trained with a
  step-level PRM + DAPO; Planner→Executor→Verifier. → **STEAL:** train only the planner; DAPO; PEV structure.
- [ThinkPRM (OpenReview)](https://openreview.net/forum?id=V727xqBYIW) — data-efficient PRM via verification CoT, few
  labels. → **STEAL:** cheap PRM if we need a learned one.
- [Karpathy on RL (Medium recap)](https://medium.com/@zlf465074419/insights-from-andrej-karpathy-potcast-reflections-on-reinforcement-learning-scaling-laws-agi-b1215cffd168)
  — outcome RL = "supervision through a straw," gameable; wants **process supervision (big model grades small
  model's steps)**. → **STEAL:** 14b grades 8b's steps = free process reward.

## 4. Self-play, curriculum & data quality 🔴 (fixes the corpus)
- [Anchored Self-Play for Code Repair (ICLR 2026)](https://openreview.net/forum?id=lTbBFAoPSA) — one model alternates
  GENERATING bugs ↔ FIXING; generator adapts difficulty → auto-curriculum; unit-test verified; embedding-similarity
  reward + reference-mixed fixer; BugSourceBench for realism. → **STEAL:** our `--generate-curriculum` becomes a
  difficulty-adapting bug-generator self-play; this IS our domain.
- [Learning to Solve and Verify (arXiv 2502.14948)](https://arxiv.org/html/2502.14948) — joint code+test self-play. →
  **STEAL:** co-train a verifier/test-writer.
- [GASP: Guided Asymmetric Self-Play (arXiv 2603.15957)](https://arxiv.org/html/2603.15957) — teacher makes easy
  "lemma" then hard "lift" variants. → **STEAL:** difficulty laddering of generated tasks.
- [Scaling RL for Code w/ Synthetic Data & Curricula (arXiv 2603.24202)](https://arxiv.org/pdf/2603.24202) — practical
  recipe. → **STEAL:** synthetic-data + curriculum pipeline.
- [ProCuRL / ZPD curriculum (arXiv 2304.12877)](https://arxiv.org/abs/2304.12877) — max learning progress at the
  competence frontier (pass ~40–60%). → **STEAL:** select/generate tasks in the model's live ZPD band.
- Data selection — [EDGE (arXiv 2502.12494)](https://arxiv.org/pdf/2502.12494),
  [Influence-Preserving Proxies (arXiv 2602.17835)](https://arxiv.org/pdf/2602.17835): reward=1 traces are often
  **too easy → low learning value**; prioritize by learning impact. → **STEAL:** drop trivial passes; keep
  high-learning-impact traces. **This is our flywheel's deepest flaw (we collect pass-only/easy).**
- [FireAct](https://fireact-agent.github.io/) — ~500 traces to learn the agent format (we used 69). → **STEAL:** data-scale target.

## 5. Memory as a learned policy + Complementary Learning Systems 🟠
- [Memory-R1 (arXiv 2508.19828)](https://arxiv.org/abs/2508.19828) — RL'd memory ops (ADD/UPDATE/DELETE/NOOP) +
  answer agent; **152 training pairs**, 3B–14B. → **STEAL:** a learned consolidation policy (keep/prune/promote).
- [MemRL: runtime RL on episodic memory (arXiv 2601.03192)](https://arxiv.org/pdf/2601.03192). → **STEAL:** runtime memory updates.
- [Voyager (arXiv 2305.16291)](https://arxiv.org/abs/2305.16291) — ever-growing **executable skill library**,
  embedding-indexed, auto-curriculum, "alleviates catastrophic forgetting." → **STEAL:** skill-library pattern for our
  procedural memory; skills feed both levers.
- [ProcMEM (arXiv 2602.01869)](https://arxiv.org/pdf/2602.01869) — reusable procedural memory via non-parametric PPO.
  → **STEAL:** learn skills from experience.
- [SCM: Sleep-Consolidated Memory (2604.20943)](https://www.emergentmind.com/papers/2604.20943) — NREM/REM
  consolidation, importance tagging, value-based forgetting, self-model. → **STEAL:** schedule for our "sleep" pass.
- [LLMs Need Sleep (2606.03979)](https://www.emergentmind.com/papers/2606.03979);
  [sleeping-llm (MEMIT + null-space)](https://github.com/vbario/sleeping-llm) — consolidate context → fast weights. →
  **STEAL:** fast-weight / weight-edit consolidation idea.
- [Titans: Learning to Memorize at Test Time (arXiv 2501.00663)](https://arxiv.org/pdf/2501.00663) — neural long-term
  memory module. → **STEAL:** CLS framing (fast vs slow stores).
- Continual learning — **standard LoRA FAILS continual learning** (functional drift); mitigations:
  [Merge before Forget (arXiv 2512.23017)](https://arxiv.org/pdf/2512.23017),
  [FOREVER replay (arXiv 2601.03938)](https://arxiv.org/pdf/2601.03938), critical-parameter constraints, EWC/orthogonal.
  → **STEAL:** continual-LoRA discipline so turn N+1 doesn't erase turn N.

## 6. Self-improving agents 🟢/🟠
- [Darwin Gödel Machine (arXiv 2505.22954)](https://arxiv.org/abs/2505.22954) — self-modifies code, grows an archive;
  SWE-bench 20→50%. → **STEAL:** archive of agent variants (don't keep only the latest).
- [Huxley-Gödel Machine (arXiv 2510.21614)](https://arxiv.org/abs/2510.21614) — **Metaproductivity–Performance
  Mismatch**: pick agents by descendants' performance, not own score. → **STEAL:** smarter self-mod selection metric.
- [Karpathy "loopy era" / AutoResearch (NextBigFuture)](https://www.nextbigfuture.com/2026/03/andrej-karpathy-on-code-agents-autoresearch-and-the-self-improvement-loopy-era-of-ai.html)
  — 700 self-improvement experiments on **one GPU** (630 LOC), 20 real optimizations; agent edits train.py. →
  **STEAL:** validation; the "agent edits its own training code" loop for M4.
- [Memento: fine-tune agents without fine-tuning (arXiv 2508.16153)](https://arxiv.org/pdf/2508.16153) — memory-based
  improvement, no weight updates. → **STEAL:** the harness/fast lever.
- [Group-Evolving Agents (arXiv 2602.04837)](https://arxiv.org/pdf/2602.04837) — experience sharing. → **STEAL:** later (breadth).
- 🔴 [**SkillOpt: Executive Strategy for Self-Evolving Agent Skills** (arXiv 2605.23904, MSR, May 2026)](https://arxiv.org/abs/2605.23904)
  ([site](https://microsoft.github.io/SkillOpt/)) — treats a compact NL **skill document as the trainable STATE of a
  FROZEN agent** and optimizes it like a DL optimizer: scored rollouts → a *separate optimizer model* makes **bounded
  add/delete/replace edits** to ONE skill doc → an edit is accepted **only if a held-out validation score strictly
  improves**. Adds a textual "learning-rate" budget, rejected-edit buffer, epoch-wise slow/meta update; **zero extra
  inference cost** at deploy. +23.5 (GPT-5.5 chat) / +24.8 (Codex) / +19.1 (Claude Code); skills **transfer** across
  models & environments. → **STEAL (big):** this is the *disciplined* version of our harness/skill lever (we have
  skills + `evolve-skill-on-repofix`, but it's the "loosely controlled self-revision" SkillOpt beats). Use 14b as the
  optimizer model, our repo-fix held-out tasks as the validation gate, bounded edits to `px-fix-bug.md` et al. It's
  **local-friendly, reversible/auditable (text not weights), and the SAFEST first self-improvement lever to ship** —
  the perfect complement to OPD (weights). See synthesis NP8.

## 7. Reward hacking / honesty / gate hardening 🔴 (urgent — our gate is hackable today)
- [Anthropic — Natural Emergent Misalignment from Reward Hacking in Production RL](https://www.anthropic.com/research/emergent-misalignment-reward-hacking)
  ([paper](https://assets.anthropic.com/m/74342f2c96095771/original/Natural-emergent-misalignment-from-reward-hacking-paper.pdf))
  — a model taught to fake tests with **`sys.exit(0)`** generalized to sabotage (12%), alignment-faking (50%);
  mitigations: reward design, agentic RLHF, **inoculation prompting**. → **STEAL:** harden check.py (forbid editing
  test, detect sys.exit/degenerate pass, held-out variant); inoculation prompting; do this BEFORE any RL.
- [Reward Hacking Benchmark (arXiv 2605.02964)](https://arxiv.org/html/2605.02964v1) — measures tool-agent exploits. →
  **STEAL:** test our gate against it.
- [Adversarial Reward Auditing (arXiv 2602.01750)](https://arxiv.org/html/2602.01750v1) — auditor-vs-policy game gates
  the reward. → **STEAL:** a reward-auditor in the gate.
- [Play Favorites: self-bias in LLM-as-judge (arXiv 2508.06709)](https://arxiv.org/pdf/2508.06709) — self-preference
  +10–25%; "never use the same model as judge and candidate"; IRT-on-judges. → **STEAL:** never let the student judge
  itself; prefer deterministic verifier; treat our bench as a measurement instrument (verify-the-ruler, formalized).

## 8. Repo-level retrieval / embeddings / vector store 🟠 (fixes multi-file localization)
- [RepoGraph (ICLR 2025)](https://github.com/YerbaPage/Awesome-Repo-Level-Code-Generation) — repo code-graph boosts
  agents **+32.8% on SWE-bench**; LocAgent (multi-hop localization); GraphCoder (ASE 2024);
  [Codebase-Memory tree-sitter KG (arXiv 2603.27277)](https://arxiv.org/pdf/2603.27277). → **STEAL:** upgrade
  `repo.map` → a control/data-dependence code graph for multi-hop localization (our hard-tier weakness).
- [Jina code embeddings 0.5B/1.5B (SOTA)](https://jina.ai/news/jina-code-embeddings-sota-code-retrieval-at-0-5b-and-1-5b/)
  + [jina-reranker-v3 0.6B](https://jina.ai/models/jina-reranker-v3/) — code-specialized retrieval, 78–79% across 25
  benchmarks. → **STEAL:** swap `nomic-embed` → jina-code-embeddings + reranker for memory/repo search.
- [turbovec (Rust, TurboQuant)](https://github.com/RyanCodrai/turbovec) +
  [TurboQuant (arXiv 2504.19874)](https://arxiv.org/abs/2504.19874) — data-free, 8× compression, faster than FAISS. →
  **STEAL:** Rust vector store for the memory spine (matches our stack).

## 9. Quantization / inference / hardware 🟢/🟠
- [Choosing a GGUF: K-quants vs IQ (kaitchup)](https://kaitchup.substack.com/p/choosing-a-gguf-model-k-quants-i) +
  [llama.cpp quantize README](https://github.com/ggml-org/llama.cpp/blob/master/tools/quantize/README.md) — imatrix
  improves quality at same size; IQ4_XS smaller than Q4_K_M. → **STEAL:** imatrix calibrated on our traces (we quantize
  with none today).
- KV-cache quant (`OLLAMA_KV_CACHE_TYPE=q8_0`; llama.cpp `--cache-type-k/v`) — frees VRAM. → **STEAL:** VRAM headroom on the 3060.
- [Edge LLMs 2026 (Edge AI Vision)](https://www.edge-ai-vision.com/2026/01/on-device-llms-in-2026-what-changed-what-matters-whats-next/)
  — **memory bandwidth, not compute, is the binding constraint**; deeper-thinner > wide-shallow; MoE = memory-movement
  bottleneck. → **STEAL:** design/选 for bandwidth; informs base-model + quant choices.
- [CoDA: diffusion code LM 1.7B (arXiv 2510.03270)](https://arxiv.org/html/2510.03270v1) — ≈7B quality, fast on light
  HW. → **STEAL:** watch as a future fast local base (park).
- **Tensor vs pipeline parallelism** ([jarvislabs](https://jarvislabs.ai/blog/scaling-llm-inference-dp-pp-tp),
  [TPI-LLM arXiv 2410.00531](https://arxiv.org/pdf/2410.00531)) — TP = intra-layer split, low latency but needs fast
  interconnect (NVLink); PP = inter-layer split, lower bandwidth need, better for limited HW. **Not relevant
  single-GPU** (we are). Only matters in the mesh-teacher scenario (§GitHub mesh-llm): over consumer ethernet (no
  NVLink) **pipeline/stage-split wins** — which is exactly what mesh-llm's "Skippy" does. TPI-LLM shows TP *can* serve
  70B on edge clusters with heavy optimization. → **STEAL:** if we pool boxes for a bigger OPD teacher, use pipeline/
  stage-splitting, not TP; revisit TP only with NVLink-class interconnect. *Park until multi-node.*

## 10. Models, benchmarks & tool-calling FT — status quo 🟢/🟠
- Best small open coding models 2026 ([kilo.ai](https://kilo.ai/open-source-models),
  [localaimaster](https://localaimaster.com/models/best-local-ai-coding-models)) — **Qwen3-Coder-Next (3B active/80B
  MoE)**, GLM-5.1, DeepSeek V4-Flash, Gemma 4 outclass qwen3:8b on agentic coding. → **STEAL:** base-model bake-off candidates.
- Open agents leaderboard ([morphllm](https://www.morphllm.com/best-ai-coding-agents-2026),
  [presenc](https://presenc.ai/research/coding-agent-benchmarks-2026)) — OpenHands+CodeAct 53%, Cline 59%+, Aider
  architect 31.4%; SWE-bench Verified top ~74–78% (**saturating**). → **STEAL:** harness ideas; we need a harder bench.
- [SWE-EVO: long-horizon software evolution (arXiv 2512.18470)](https://arxiv.org/pdf/2512.18470);
  Terminal-Bench 2.0; [GDPval](https://artificialanalysis.ai/evaluations/gdpval-aa). → **STEAL:** harder/realistic bench targets.
- Decoupled tool-calling FT — [AgentFlux (arXiv 2510.00229)](https://arxiv.org/pdf/2510.00229) (tool-selection vs
  arg-gen, separate loss masks, **+46%**); TinyAgent; APIGen/ToolACE data pipelines. → **STEAL:** decoupled FT recipe
  + function-calling data generation for our distillation.
- [Karpathy Software 3.0](https://www.latent.space/p/s3) — context window = RAM, weights = CPU, "jagged intelligence,"
  context engineering. → **STEAL:** framing for our dual-lever (we program both the RAM and the CPU).

---

### How to use this file
Each 🔴 is in the active roadmap (synthesis NP1–7). When we build a stage, open the linked sources for that row and
lift the specific mechanism noted in **STEAL**. New scrapes append here; the synthesis doc stays the "why/what-next."
