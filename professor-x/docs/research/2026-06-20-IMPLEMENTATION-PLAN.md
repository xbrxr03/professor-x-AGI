# Implementation plan — self-improving local coding agent (2026-06-20)

Grounded in today's measured compare + the ~35-source research scrape (see MASTER index, synthesis NP1–8,
github-repos doc). Every stage: **flag-gated, default-off, full `cargo test --bins` green, measured on the
trustworthy ruler, committed**. Branch `prereboot-flywheel-prep`.

## Measured findings that shape this plan (honest)
- **Native tool-calling works & is the better-instrumented path:** clean run = **verifier pass@1 0.433 (13/30)**,
  all 30 terminated, NATIVE_EXIT=0, 5 parse-fails (vs 44 pre-fix), **no hang**. Real headroom → good ruler.
- **The ruler was lying:** native logged **30/30 agent-"succeeded" but only 13/30 verifier-passed** — agent-finished
  overcounts real pass by **~2.3×**. Our finish-gate accepts self-declared completion, not check.py. *Any capability
  claim must use the verifier.*
- **Text path is NOT a trustworthy number:** it **hung 25 min** on a tool call with no timeout (one task froze the
  whole run) and never printed a verifier pass@1; its ~0.85 was agent-finished (inflated by the same ~2.3×). So
  "native 0.43 vs text 0.85" is an illusion — corrected for the finish-gate, they're likely **comparable**.
- **Why native scored "lower" anyway:** `build_native_messages` sends a STRIPPED prompt (system+task+tool history) —
  it DROPS the scaffolding the text path injects (skills/`px-fix-bug`, repair hints, repo context, ICE examples). So
  native traded scaffolding for robustness. The fix is to port scaffolding INTO native, not to choose between them.
- **Saturation:** the hard set has good headroom under native (0.43); near-ceiling under the (unverified) text path →
  we still need harder fixtures (NP4) once capability rises.

**Decision:** native tool-calling is the default eval + collection path going forward (clean, terminates, structured
trajectories). Text path stays as fallback only.

---

## Stage 0 — Trustworthy ruler + safety (PREREQUISITE, do first) 🔴
Nothing downstream is believable until the ruler and gate are honest and un-hangable.
- **0a. Tool-execution hard timeout** (the hang): wrap every tool call in the executor with a per-tool timeout →
  return a failure observation instead of freezing the loop. *Reliability win for real users too.* [`toolbridge/executor.rs`]
- **0b. Verifier-truth finish-gate**: a task counts as solved ONLY if the verifier (`check.py`) passes — never on the
  agent's self-declared finish. Make the bench's pass@1 the single source of truth and have the finish-gate require a
  verified mutation. [`agentd/react.rs` finish-gate, bench]
- **0c. Reward-hack hardening (NP3)**: sandbox the fix so it can't edit `check.py`/tests; reject degenerate passes
  (`sys.exit(0)`, no real diff, test mtime changed); held-out variant check. Refs: Anthropic emergent-misalignment,
  Reward-Hacking-Bench, Adversarial Reward Auditing (master index §7).
- **0d. Pin the ruler**: native verifier pass@1 over K=5 on the 30-task hard set → `baseline_native.txt` with variance
  + MDE. Apply verify-the-ruler.
- **Exit criteria:** no hangs; gate rejects sys.exit/degenerate fixtures (add 2 red-team fixtures that SHOULD fail);
  pinned native baseline with K=5 variance. **Effort: ~1 day.**

## Stage 1 — Native parity: port harness scaffolding into native messages 🔴
Close the scaffolding gap so native ≥ text on capability while keeping robustness.
- Inject into `build_native_messages`: the `px-fix-bug` skill, repair hints, `repo.map`/repo context, ICE examples
  (the things the text `SYSTEM_PROMPT`/`build_step_prompt` already assemble). [`agentd/react.rs`]
- Re-measure native pass@1 vs the Stage-0 baseline.
- **Exit:** native pass@1 up beyond baseline+MDE; still 0 hangs, terminates 30/30. **Effort: ~1–2 days.**

## Stage 2 — NP8 SkillOpt skill optimizer (first self-improvement lever; safest) 🔴
The harness lever done right — reversible, auditable, local, zero-deploy-cost.
- Rebuild `evolve-skill-on-repofix` as a SkillOpt-style text optimizer: 14b = optimizer; bounded add/delete/replace
  edits to ONE skill doc; **accept only on strict held-out improvement**; textual learning-rate budget + rejected-edit
  buffer. Refs: SkillOpt (microsoft.github.io/SkillOpt), arXiv 2605.23904.
- **Exit:** a committed skill edit that beats the held-out native ruler beyond MDE; full audit trail; trivially
  revertible. **This is the first demonstrable, trustable self-improvement. Effort: ~2–3 days.**

## Stage 3 — NP1 On-Policy Distillation flywheel (weights lever; replaces static SFT) 🔴
Our static SFT caused the drift; OPD is the 2026 standard fix.
- Collect native-format STUDENT rollouts on repo-fix → 14b per-step correction/scoring → train student on its OWN
  trajectories (assistant-only masking already in `train_qlora.py`). Multi-teacher = 14b + coder.
- Refs: Thinking Machines OPD, thunlp/OPD recipe (arXiv 2604.13016), mkurman/grpo-llm-evaluator, RUCBM/G-OPD.
- **Exit:** distilled native pass@1 beats baseline beyond MDE AND ICS ≥ 0.70 (identity gate). Continual-LoRA
  discipline (merge-before-forget) so it doesn't erase Stage-2 gains. **Effort: ~3–5 days.**

## Stage 4 — NP2 process rewards + NP5 GRPO (sharpen; second weights lever) 🟠
- Derive a step reward from native trajectories (which edit flipped red→green via counterfactual replay) and/or 14b
  step-grader; GRPO on `check.py` reward (Unsloth GRPO+QLoRA ~5GB). Honest: RLVR sharpens, doesn't add skills → run
  AFTER OPD, behind Stage-0 hardening. Refs: microsoft/rStar, AgentFlow-Pro, mkurman. **Effort: ~3–5 days.**

## Stage 5 — NP4 ZPD harder fixtures + anchored self-play (data) 🟠 [parallel, no-GPU]
Hard set saturates the text harness → need ZPD-band difficulty.
- Anchored-self-play bug-generator targeting the model's live pass-band (40–60%); grow distinct multi-file fixtures.
  Refs: Anchored Self-Play for Code Repair (ICLR 2026), ProCuRL/ZPD, data-selection (drop trivial passes).

## Stage 6 — NP7 repo-graph + code embeddings 🟠 [parallel, no-GPU]
- `repo.map` → tree-sitter control/data-dependence code graph for multi-hop localization (RepoGraph +32.8%).
- Swap `nomic-embed` → jina-code-embeddings (+reranker); turbovec (Rust, 8×) for the store.

## Stage 7 — NP6 CLS framing + learned consolidation (integrate) 🟢
- Frame/schedule the dual-lever as a Complementary Learning System (fast harness/memory + slow weights + sleep
  replay → skill library + OPD corpus). Learned keep/prune/promote policy (Memory-R1). Continual-LoRA throughout.

---

## Sequencing & rationale
0 (ruler+safety) → 1 (native parity) → 2 (SkillOpt: prove trustable self-improvement cheaply) → 3 (OPD: the weights
headline) → 4 (GRPO sharpen). 5 & 6 run in parallel (no GPU) feeding 0/1/3. 7 integrates.
**Why this order:** can't trust any gain without Stage 0; SkillOpt before OPD because it's the safest, fastest, fully
reversible proof of the headline feature; OPD before GRPO because RLVR sharpens but doesn't add skills.

## The north-star loop (what stages 2–4 compose into)
work → memory of *verifier-passed* work → (a) SkillOpt edits the harness + (b) OPD/GRPO update the weights → ONE
reward-hack-hardened held-out gate (accept only measured, beyond-MDE, identity-preserving gains) → repeat. Local, on a
$400 GPU, in public. That unification is the moat (every technique exists elsewhere; nobody runs it unified+local+gated).
