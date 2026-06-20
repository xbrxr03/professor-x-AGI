# AI research landscape scan (2026-06-20) — studies/experiments + how they bear on us

Two buckets: **(A) directly usable** for our local self-improving coding agent, and **(B) context**
(the broader science of AI / where the field thinks progress comes from). Honest framing: A is where
we act; B shapes the bet. Companion to 2026-06-20-topics-to-explore.md.

## A. Directly usable (ranked)

### A1. On-policy distillation (OPD) — fix the static-SFT drift
Train the STUDENT on its OWN rollouts with teacher correction, not static teacher traces.
SOD (2605.07725, step-wise, stops tool cascade-drift), TCOD (temporal curriculum), MAD-OPD, ROPD;
DeepSeek-V4 uses OPD. **Our looping/bad-edit ceiling is the exact thing OPD targets.**

### A2. RLVR / GRPO on our check.py reward — and it fits the 3060
pass/fail unit tests = the canonical verifiable reward. GRPO is the de-facto algo; **Unsloth GRPO+QLoRA
~5GB VRAM, 8B feasible on 12GB at modest context** (we already use Unsloth). Honest caveat: "RLVR makes
models faster, not smarter" — sharpens, doesn't add skills → pair with distillation, don't replace.

### A3. Anchored Self-Play for Code Repair (ICLR 2026) — almost exactly our problem
One model alternates GENERATING bugs and FIXING them; as the fixer improves the generator makes harder
bugs → automatic curriculum; unit tests verify; embedding-similarity reward + reference-mixed fixer for
realism (BugSourceBench). Also "Learning to Solve and Verify" (2502.14948). **This is repo-fix self-play
— our `--generate-curriculum` + verifier already gesture at it; ASP is the principled version.**

### A4. Process Reward Models (PRM), step-level — better credit than outcome-only
AgentPRM (WWW 2026), AgentFlow-Pro (ICLR 2026: Qwen3-8B Planner trained with PRM + DAPO, Planner→Executor
→Verifier), ThinkPRM (data-efficient, CoT verification, few labels). **Our reward is outcome-only
(check.py at the end); a step-level PRM would tell the agent which STEP went wrong (the bad fs.hash_edit),
not just that the task failed.** Pairs with our native tool-calling (clean steps to score).

### A5. Memory-R1 (2508.19828) — LEARN memory management, don't hand-code it
RL (PPO/GRPO) trains a Memory Manager (ADD/UPDATE/DELETE/NOOP) + Answer Agent; only 152 training pairs,
3B–14B. MemRL (2601.03192) runtime RL on episodic memory. **This is the missing engine for our
"memory as the improvement engine" thesis — our memory ops are heuristic; this makes them learned.**

### A6. Continual learning without forgetting — our distillation overwrites skills
Key finding: **standard LoRA FAILS at continual learning** — it preserves raw weights but not functional
behavior; domain fine-tunes drop general performance (our exact risk each flywheel turn). Mitigations:
critical-parameter constraints (Apr 2026), EWC / orthogonal-subspace, "Merge before Forget" continual
LoRA merging (2512.23017), replay (FOREVER, 2601.03938), Titans neural long-term memory. **Adopt a
continual-LoRA discipline so turn N+1 doesn't erase turn N.**

## B. Context — the science of AI / where progress is coming from

### B1. Scaling laws are plateauing → algorithmic breakthroughs, not more compute
Growing lab consensus that more data+compute won't reach superintelligence; new unified theory frames
scaling laws as "statistical inevitabilities of heavy-tailed data," describing *what* success looks like,
not *when/why* it happens. **Validates our whole bet:** the edge is algorithmic (memory + self-improvement
+ continual learning on small local models), not scale.

### B2. Hassabis/field consensus: AGI path = continual learning + memory + world models + reasoning
"Era of just scaling LLMs is transitioning to memory-augmented, world-model-driven, continually learning
agentic systems." **We are squarely in that lane** (memory-driven, continually self-improving) — just
local + gated + on consumer HW, which nobody else is doing.

### B3. Mechanistic interpretability — MIT 2026 Breakthrough Technology
Anthropic open-source circuit tracer, DeepMind Gemma Scope 2, SAEs, certified circuits; used to catch
deception. **For us:** an honest instrument to audit what a self-improvement step actually changed in the
weights (misevolution detection beyond black-box pass@1) — ties to our consciousness/φ measurement work.

### B4. Reward hacking is the central safety risk of self-improvement
Reward Hacking Benchmark (2605.02964) for tool agents; Adversarial Reward Auditing (auditor vs policy
game); EvilGenie (LLM judges + held-out tests); CoT-monitoring (but models learn obfuscated hacks).
**This IS our misevolution risk.** Our gate (pinned eval + held-out + full-test + honesty) is exactly the
mitigation literature converges on — we should harden it (held-out tasks the proposer never sees, reward
auditing) as we add RL/self-play, which are prime hacking surfaces.

### B5. Test-time training (TTT) — ephemeral inference-time adaptation
Self-Improving LLM Agents at Test-Time (2510.07841): temporary per-task parameter updates at inference,
avoids catastrophic forgetting, less offline data. Interesting long-term for a local agent that adapts to
a user's repo on the fly; heavy for now — park it.

## So what (synthesis for our roadmap)
The field is telling us the SAME thing from five directions: the frontier is **memory + continual
self-improvement on a verifiable signal**, not scale — and the binding risk is **reward hacking**, which
our gate already targets. Our concrete near-term sequence, all hardware-feasible and on our verifier:
1. native-format trace collection (done plumbing) → **OPD/SFT** (A1) to fix on-policy drift,
2. **GRPO/RLVR** on check.py (A2) to sharpen, with **continual-LoRA** discipline (A6) to not forget,
3. grow data via **anchored self-play** (A3) + a **step-level PRM** (A4) for credit assignment,
4. make memory ops **learned** (A5), and harden the gate with **reward-auditing / held-out** (B4).
Everything stays gated on the trustworthy ruler.
