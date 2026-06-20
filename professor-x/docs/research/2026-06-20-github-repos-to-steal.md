# GitHub repos to steal from (2026-06-20) — meshLLM + related

Scraped via `gh search`. Each: **[repo](url)** — what it is → **STEAL:** what we lift. Tags: 🔴 direct/high
· 🟠 useful · 🟢 reference/context. Pairs with the MASTER research index (papers).

## meshLLM (what was asked)
- 🟠 **[Mesh-LLM/mesh-llm](https://github.com/Mesh-LLM/mesh-llm)** (by Michael Neale, Goose co-creator; updated
  today) — pools GPUs across macOS/Linux into ONE OpenAI-compatible endpoint (`:9337/v1`); runs local, routes to
  peers, or "Skippy" stage-splits models too big for one box; MoE experts distributed per-node; mesh gossip for
  agent coordination; built on a llama.cpp fork. → **STEAL:** the escape hatch from our single-GPU teacher limit —
  pool 2–3 consumer boxes to run a *bigger* teacher (32B coder) for OPD trace collection, while the student/gate
  stays on the 3060. Also a clean OpenAI-compatible-mesh design if we ever distribute. *Not core (our thesis is
  single $400 GPU) but directly relaxes the teacher-size constraint.*
- 🟢 **[Fangkang515/MeshLLM](https://github.com/Fangkang515/MeshLLM)** — ICCV paper, 3D-mesh generation. **Irrelevant**
  (different "mesh"); noted so we don't confuse them.
- 🟢 Others in the "mesh" namespace: [HewlettPackard/llmesh](https://github.com/HewlettPackard/llmesh) (agentic tool
  mesh / plugin orchestration), [ExaDev/agent-comms](https://github.com/ExaDev/agent-comms) (cross-harness agent
  comms mesh), [StarryCod/cogitum](https://github.com/StarryCod/cogitum) (sovereign agentic CLI: TUI + multi-provider
  + skills + MCP + autonomous mode — a Professor-X sibling worth a look). → **STEAL:** multi-agent comms patterns later.

## Self-improving coding agents (our M4 — highest relevance) 🔴
- 🔴 **[MaximeRobeyns/self_improving_coding_agent](https://github.com/MaximeRobeyns/self_improving_coding_agent)**
  (SICA) — the canonical clean loop: (1) eval current agent on benchmark, (2) store results in an **archive**, (3)
  agent edits its OWN codebase to improve, (4) repeat. Runs in a Docker sandbox. → **STEAL:** the exact M4
  harness-lever loop structure + archive-of-versions + sandbox isolation. Our closest reference.
- 🔴 **[BetterForAll/self-improving-agents](https://github.com/BetterForAll/self-improving-agents)** — "four levels of
  self-improving code agents, from the simplest loop to a full adversarial arena with self-modifying agents; each
  level adds one key idea." → **STEAL:** a staged maturity ladder for our self-improvement loop (don't jump to the
  arena; add one idea per level, gated).
- 🔴 **[unrealumanga/Nedster](https://github.com/unrealumanga/Nedster)** — local-first coding agent (Ollama + Qwen,
  8GB VRAM) that hit OUR EXACT failure modes and solved them: "tool amnesia, hallucinated XML tags, infinite loops."
  Fixes: a fortified single-pass parser that catches broken XML/JSON/markdown (= our parser hardening / native
  tool-calling), "amnesia correction" (intercept a refusal → inject correction → force retry), **TurboQuant 4-bit KV
  cache for 256K ctx on 8GB** (validates our quant finding!), built-in ChromaDB RAG, iteration budgets + continuity
  watchdogs to prevent hangs. → **STEAL:** the robustness tricks (amnesia-correction, watchdogs) and confirmation
  that TurboQuant-KV + local RAG work on consumer HW. *Harness-only — no weight self-improvement, so our dual-lever
  is still the differentiator.*
- 🟠 **[jennyzzt/dgm](https://github.com/jennyzzt/dgm)** (official Darwin Gödel Machine) ·
  **[mmtmn/Darwin-Godel-Machine](https://github.com/mmtmn/Darwin-Godel-Machine)** (DGM running **locally via Ollama**)
  · **[lemoz/darwin-godel-machine](https://github.com/lemoz/darwin-godel-machine)** (multi-LLM, sandboxed,
  population evolution + benchmarking) · **[tylergibbs1/evolve](https://github.com/tylergibbs1/evolve)** (DGM-H,
  TS). → **STEAL:** the local-Ollama DGM port (mmtmn) is our stack; lemoz's sandboxing + population + benchmarking
  harness is a ready blueprint for safe self-modification.
- 🟢 **[facebookresearch/HyperAgents](https://github.com/facebookresearch/HyperAgents)** (self-referential
  self-improving, Meta) · **[metauto-ai/GPTSwarm](https://github.com/metauto-ai/GPTSwarm)** (self-improving via
  RL/prompt optimization) · **[xvirobotics/metabot](https://github.com/xvirobotics/metabot)** (local self-evolving
  agent org, shared memory, agent factory, comms bus). → **STEAL:** selection/optimization ideas; metabot for
  local-orchestration patterns.

## On-Policy Distillation (NP1 — the headline rewrite) 🔴
- 🔴 **[thunlp/OPD](https://github.com/thunlp/OPD)** — "Rethinking On-Policy Distillation: Phenomenology, Mechanism,
  and Recipe" ([arXiv 2604.13016](https://arxiv.org/abs/2604.13016)), Tsinghua NLP. → **STEAL:** the concrete OPD
  **recipe** (when/how to mix on-policy rollouts + teacher correction) — the reference implementation for our rewrite.
- 🟠 **[RUCBM/G-OPD](https://github.com/RUCBM/G-OPD)** — Generalized OPD with **reward extrapolation** ("learn beyond
  teacher"). → **STEAL:** how to push the student *past* the 14b teacher, not just match it.
- 🟢 Curated lists: [chrisliu298/awesome-on-policy-distillation](https://github.com/chrisliu298/awesome-on-policy-distillation),
  [nick7nlp/Awesome-LLM-On-Policy-Distillation](https://github.com/nick7nlp/Awesome-LLM-On-Policy-Distillation),
  [thinkwee/AwesomeOPD](https://github.com/thinkwee/AwesomeOPD). → **STEAL:** stay current.

## GRPO / RLVR (NP5 — second lever) 🟠
- 🔴 **[mkurman/grpo-llm-evaluator](https://github.com/mkurman/grpo-llm-evaluator)** — fine-tunes a STUDENT with
  GRPO using TEACHER-provided evaluations. → **STEAL:** our exact 14b-evaluates-8b setup, in code.
- 🟠 **[X-jun-0130/GRPO-LLM](https://github.com/X-jun-0130/GRPO-LLM)** — GRPO + Verl + GenerativeRM. → **STEAL:**
  generative-reward-model wiring (matches DeepSeek-V4's design).
- Infra: **[volcengine/verl]** and **[VerlTool]** (agentic RL w/ tool use) — production agentic-RL frameworks. →
  **STEAL:** trajectory/credit-assignment plumbing if we outgrow Unsloth GRPO.

## Process rewards / self-evolution (NP2) 🔴
- 🔴 **[microsoft/rStar](https://github.com/microsoft/rStar)** + **[zhentingqi/rStar](https://github.com/zhentingqi/rStar)**
  — official rStar-Math (SLM + MCTS + process reward, self-evolution 58.8→90, no bigger teacher). → **STEAL:** the
  search + PRM + verifier self-evolution loop for our 8B.
- 🟠 **[awesome-pro/agentflow-pro](https://github.com/awesome-pro/agentflow-pro)** — Qwen3-8B Planner + step-level PRM
  + DAPO (Planner→Executor→Verifier). → **STEAL:** train only the planner with a step PRM; DAPO optimizer.

## Repo-graph localization (NP7 — multi-file weakness) 🟠
- 🟠 **[ozyyshr/RepoGraph](https://github.com/ozyyshr/RepoGraph)** — official; repo code-graph, +32.8% on SWE-bench.
  → **STEAL:** upgrade our `repo.map` → a control/data-dependence code graph for multi-hop localization.

## Memory / skills (NP6) 🟠
- 🟠 **[Voyager](https://github.com/MineDojo/Voyager)** — executable skill library, embedding-indexed, auto-curriculum.
  → **STEAL:** skill-library pattern for our procedural memory.
- 🟢 **[Saurav-Kalaskar/loom](https://github.com/Saurav-Kalaskar/loom)** — Claude-Code skill: Reflexion self-learning +
  Voyager skill library + research, pure shell+python. → **STEAL:** lightweight skill-library + reflexion wiring.
- 🟢 **[georust/rstar](https://github.com/georust/rstar)** — R*-tree spatial index in **Rust**. → **STEAL:** a Rust
  ANN/index option for our vector store (alongside turbovec).

## Takeaways
1. **SICA + BetterForAll + lemoz/DGM** are ready blueprints for our M4 self-improvement loop (archive + sandbox +
   staged levels) — don't reinvent it.
2. **Nedster** proves our consumer-GPU robustness stack (TurboQuant-KV, watchdogs, parser fortification) works, and
   independently arrived at native-tool-calling-style fixes — but it's harness-only; **our weights lever + gate is
   the moat.**
3. **thunlp/OPD + mkurman/grpo-llm-evaluator + microsoft/rStar** are the three reference implementations for the
   weights levers (OPD, teacher-GRPO, self-evolution) — clone, read, lift the recipe.
4. **mesh-llm** is the pressure-release valve for a bigger OPD teacher without buying hardware.
