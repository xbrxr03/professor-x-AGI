# Master Reference List — every source used in Professor X research (2026-06-21)

One consolidated index of every reference material we have used to date: papers (arXiv), GitHub
repos, external resources, our own research docs, and the project skills/brain. Built by harvesting
`professor-x/docs/research/` + `brain/` and adding the 2026-06-21 deep-dive finds.

---
## A. Papers cited across our research (arXiv, 117 unique IDs)
Grouped by theme where known; the rest listed as a complete index. Resolve any ID at
`https://arxiv.org/abs/<id>`.

**Self-improving / evolving agents (harness and/or weights)**
- 2605.27276 — **SIA: Self-Improving AI with Harness & Weight Updates** (Hexo Labs). The dual-lever
  baseline: only entry that updates BOTH scaffold and weights in one loop. H100 + gpt-oss-120B.
  Names the **Coupled Co-Evolutionary Goodhart Problem** as unsolved; no transfer, no memory.
- 2604.01687 — **EvoSkills: Self-Evolving Agent Skills via Co-Evolutionary Verification** (surrogate
  verifier + opaque oracle; text skills only; explicitly NO unseen-task-variant transfer).
- 2605.23904 — **SkillOpt** (Microsoft): held-out **selection gate** for text skills (accept only if
  selection-split score strictly improves); frozen weights, same-distribution split.
- 2604.08377 — SkillClaw (skills evolve collectively). 2603.28716 — Dynamic Dual-Granularity Skill Bank.
- 2509.26354 — **Your Agent May Misevolve: Emergent Risks in Self-evolving LLM Agents**.
- 2510.04399 — On the Statistical Limits of Self-Improving Agents (Two-Gate policy, VC-rate bounds).
- 2603.25681 — Self-Improvement of LLMs: Technical Overview & Future Outlook.
- 2305.16291 — Voyager (skill library). 2303.11366 — Reflexion. 2308.00352 — MetaGPT. 2308.08155 — AutoGen.

**RL from verifier / execution feedback (code)**
- 2410.02089 — **RLEF: Grounding Code LLMs in Execution Feedback with RL**.
- 2510.22075 — Agentic RL for Real-World Code Repair. 2512.21919 — SWE-RM: execution-free feedback.
- 2509.02547 — The Landscape of Agentic RL for LLMs (survey).
- 2510.10931 — Proof-of-Use: mitigating tool-call hacking in research agents.

**Test-time RL / unsupervised RLVR**
- 2504.16084 — **TTRL: Test-Time Reinforcement Learning** (majority-vote reward, unlabeled).
- 2603.16223 — Dual Consensus (escaping spurious majority in unsupervised RLVR). 2510.07841 — Self-Improving
  LLM Agents at Test-Time. 2502.20379 — Multi-Agent Verification (scaling test-time verifiers).

**Credit assignment**
- 2604.09459 — **From Reasoning to Agentic: Credit Assignment in RL for LLMs** (survey, 47 methods).
- 2602.16165 — HiPER (hierarchical, explicit credit). 2603.08754 — Hindsight Credit Assignment for
  long-horizon agents. 2509.19199 — Agentic RL with Implicit Step Rewards (iStar). GiGPO (NeurIPS25).

**On-policy distillation / small-model training**
- 2604.00626 — A Survey of On-Policy Distillation for LLMs. 2604.13016 — Rethinking OPD.
- 2605.07725 — **SOD: Step-wise On-policy Distillation for Small Language Model Agents**.

**Quantization**
- 2511.06516 — TAQ. 2505.19433 — "Can Compressed LLMs Truly Act?" 2510.16805 — Mixed-Precision Quant
  for LMs (techniques & prospects). 2603.17354 — Beyond Outliers: data-free layer-wise mixed-precision
  (dual-sensitivity). 2604.13440 — A KL Lens on Quantization (forward-only sensitivity). 2504.21553 —
  Spike-Aware Mixed-Precision. 2302.05397 — Practical Mixed-Precision PTQ.

**Memory for agents**
- 2603.07670 — Memory for Autonomous LLM Agents (survey). 2509.24704 — MemGen. 2603.18718 — MemMA.
- 2603.11768 — SSGM (governing evolving memory). 2512.16301 — Adaptation of Agentic AI (post-training,
  memory, skills survey).

**Benchmarks / contamination**
- 2310.06770 — SWE-bench. 2412.21139 — SWE-Gym. 2512.12216 — R2E-Gym. 2602.23866 — SWE-rebench.
- 2403.19114 — EvoEval (anti-contamination transforms). 2406.04244 / 2407.07565 — contamination studies.
- 2603.21454 — Cross-Context Verification (contamination via session isolation). 2502.17259 —
  contamination via watermarking. 2510.09259 — detecting contamination from RL post-training.
- 2406.05397 — metamorphic testing. 1807.03512 — mutation testing. 2405.02481 — Proximal Curriculum (ZPD).

**Reward modeling / overfitting**
- 2507.07981 — Why is Your LM a Poor Implicit Reward Model? (IM-RMs fail on paraphrase → near-zero).
- 2602.09305 — Reward Modeling for RL-based LLM Reasoning. 2403.03185 — Correlated Proxies (reward-hack
  mitigation). 2510.01925 — Reward Models analytical survey.

**Complete ID index (all 117, incl. the above):**
1802.07044 1807.03512 1811.08886 1906.05271 2006.08381 2008.03703 2101.07592 2303.11366 2304.12877
2305.16291 2305.19674 2307.03172 2308.00352 2308.08155 2310.06770 2310.11511 2310.19791 2403.19114
2405.02481 2406.04244 2406.05397 2407.07565 2407.19794 2410.00531 2412.10425 2412.21139 2501.00663
2501.04519 2502.02534 2502.12494 2502.14948 2502.18864 2504.14191 2504.19874 2504.21024 2505.00234
2505.03335 2505.13820 2505.14635 2505.17612 2505.19433 2505.20276 2505.22954 2506.02153 2506.05109
2506.21605 2507.19457 2507.21046 2508.06709 2508.07407 2508.16153 2508.19828 2509.01055 2510.00229
2510.03270 2510.04399 2510.04618 2510.07841 2510.11977 2510.16079 2510.21614 2511.06516 2511.10395
2511.16043 2511.22367 2512.12216 2512.15943 2512.18470 2512.23017 2601.03192 2601.03938 2601.04728
2601.06377 2601.11974 2601.19897 2601.19935 2602.01750 2602.01869 2602.04837 2602.16313 2602.17835
2602.21534 2602.23866 2603.03329 2603.10600 2603.14597 2603.15421 2603.15957 2603.16158 2603.19461
2603.24202 2603.27277 2603.28052 2603.28823 2603.29640 2604.00594 2604.00626 2604.07666 2604.08224
2604.11364 2604.13016 2604.20938 2604.20943 2604.24005 2604.25850 2605.02964 2605.07725 2605.12129
2605.15155 2605.22148 2605.22166 2605.22794 2605.23904 2605.27276 2605.31509 2606.03979

**Added 2026-06-21 (new IDs from this deep-dive, append to corpus):** 2510.22075 2512.21919 2509.02547
2510.10931 2504.16084 2603.16223 2502.20379 2604.09459 2602.16165 2603.08754 2509.19199 2604.00626
2604.13016 2605.07725 2510.16805 2603.17354 2604.13440 2504.21553 2302.05397 2603.07670 2509.24704
2603.18718 2603.11768 2512.16301 2603.21454 2502.17259 2510.09259 2507.07981 2602.09305 2403.03185
2510.01925 2604.01687 2605.23904 2604.08377 2603.28716 2509.26354 2603.25681.

---
## B. GitHub repos referenced
jcode (1jehuang/jcode) · ggml-org/llama.cpp · huggingface/trl · jennyzzt/dgm + lemoz/darwin-godel-machine
+ mmtmn/Darwin-Godel-Machine · MaximeRobeyns/self_improving_coding_agent · BetterForAll/self-improving-agents
· facebookresearch/HyperAgents · metauto-ai/GPTSwarm · MineDojo/Voyager · microsoft/rStar + zhentingqi/rStar
· thunlp/OPD + RUCBM/G-OPD + thinkwee/AwesomeOPD · chrisliu298/awesome-on-policy-distillation +
nick7nlp/Awesome-LLM-On-Policy-Distillation · Nardien/agent-distillation · opendilab/awesome-RLVR ·
mkurman/grpo-llm-evaluator + X-jun-0130/GRPO-LLM · ozyyshr/RepoGraph + YerbaPage/Awesome-Repo-Level-Code-Generation
· Fangkang515/MeshLLM + Mesh-LLM/mesh-llm · HewlettPackard/llmesh · RyanCodrai/turbovec · georust/rstar ·
prime-rl/ttrl · pprp/Awesome-LLM-Quantization · Shichun-Liu/Agent-Memory-Paper-List · NousResearch/hermes-agent
· (others surveyed: agenticSeek, agentflow-pro, agent-comms, cogitum, loom, evolve, argo, metabot, sleeping-llm, Nedster).

## C. External resources
Thinking Machines — On-Policy Distillation (thinkingmachines.ai/blog/on-policy-distillation) · Unsloth RL
guide (unsloth.ai/docs) · HF TRL SFT docs · Anthropic — Natural Emergent Misalignment from Reward Hacking
(assets.anthropic.com / anthropic.com/research) · kaitchup (GGUF k-quants) · Karpathy on code agents &
the self-improvement loop (nextbigfuture 2026-03) · SWE-bench Pro leaderboard (morphllm.com/swe-bench-pro)
· LiveCodeBench (openreview) · ICLR 2026 MemAgents workshop · MarkTechPost — Hexo Labs open-sources SIA ·
artificialanalysis.ai (GDPval) · presenc.ai coding-agent benchmarks 2026 · edge-ai-vision on-device LLMs 2026.

## D. Our own research docs (professor-x/docs/research/, 36)
consciousness-measurement-program · indicator-property-audit · jcode-vs-professor-x-gap-analysis ·
frankenstein-harness-master-plan · react-synthesis-guard-measurement · distillation-recipe-literature ·
memd-keep-prune-map · unified-loop-design · ai-research-landscape · format-unification-plan ·
github-repos-to-steal · IMPLEMENTATION-PLAN · INVENTION-active-inference-self-improvement ·
MASTER-research-index · phase1-parsecheck-result · phase2-native-toolcalling-plan · phase2-S2-native-result ·
quantization-vector-techniques · stage1-result · synthesis-new-perspectives · SYNTHESIS-the-compression-gate ·
topics-to-explore · INVENTION-AACE-goodhart-tripwire · INVENTION-fourth-lever-verifier-driven-quant ·
precheck-results · RESULT-family-transfer · reuse-family-benchmark-recipe · SYNTHESIS-verifier-as-first-class-signal ·
VERDICT-fourth-lever-quant-precheck · eval-trust · failure-taxonomy · m1-real-benchmark-design ·
m4-code-proposer-scoping · m4-frontier-self-improvement-engine · standard-readiness ·
MASTER-REFERENCE-LIST (this doc).

## E. Project skills & brain
Skills (`.claude/skills/`): verify-the-ruler, adversarial-self-review, diagnose-from-trajectory,
professor-x-ops, rust-harness-change, small-model-harness-design, add-repofix-fixture.
Agent method skills (`professor-x/skills/`): px-know-scientific-method, px-experiment-runner,
px-gap-analysis, px-interdisciplinary-bridge, px-deep-research, px-literature-search, px-synthesize.
Portfolio: `brain/inventions.md` (MHE three levers + DFA + IPE + Functional Affect; Lever-4 quant SHELVED).
