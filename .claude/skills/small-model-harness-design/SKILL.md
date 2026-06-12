---
name: small-model-harness-design
description: "Design principles for making a weak local model (qwen3:8b) into a capable agent on Professor X — the project's core thesis. Use when designing/changing the agent loop, tools, prompts, or evolution; when deciding how to fix a capability gap; or when tempted to blame the model. The recurring finding: the gap is almost always the HARNESS, not the 8B's reasoning."
allowed-tools: Read, Grep, Glob
---

# Small-model + great-harness design principles

Thesis: "the harness is the intelligence, not the model." Demonstrated this session — the same
qwen3:8b went 0.50 → ~0.85 on the trustworthy repo-fix benchmark purely from harness fixes. When
the agent fails, suspect the harness first (and `diagnose-from-trajectory` to confirm).

## Principles (each earned from a measured result)

| Principle | Why (the measured lesson) |
|---|---|
| **Minimal instructions win** | A verbose 4-step "edit→test→iterate" prompt DROPPED pass@1 0.50→0.30. Small models follow short prompts better; one clear line beats a procedure. |
| **Break greedy loops with temperature** | At temp 0.3 the 8B re-emits the identical stuck action forever. Escalate temperature on a duplicate-blocked retry to force a different action. |
| **Make tools forgiving of model slips** | The 8B invents line-hashes even when its fix is correct; strict `hash_edit` rejected correct edits. Fall back to line-based apply; let the lint/parse gate (editverify) be the real guard. |
| **The wall is usually downstream of where you look** | A fix that doesn't move the number often WORKED and exposed the next bottleneck (loop → edit-tool-reject → ...). Re-read the trajectory. |
| **Don't ask the 8B to self-engineer blindly** | Blind "improve this prompt" produced WORSE prompts (rejected by the gate). A failure-AWARE proposer shown the real failures does better. The strongest signal is reading the trajectory, not LLM self-proposal. |
| **Grade deterministically** | A qwen3:8b LLM-judge is unstable in both directions (false-neg ↔ false-pos). Use test exit codes (repo-fix); reserve LLM-judging for cases with no deterministic check, and never for headline numbers. |
| **Gate self-improvement on measurement** | Accept a harness/prompt/skill change ONLY if it measurably beats baseline beyond noise (K reps, MDE). The legacy loop and ARIS meta-optimize accept on LLM-approval and drift. |

## The edit stack (what makes editing work on an 8B)
hash-anchored + line-fallback edit · windowed file reads · edit-time lint/parse verification ·
fuzzy apply-patch fallback. Edit failure is a MECHANICAL interface problem, not a reasoning one.

## Rule
Before concluding "the 8B can't do X," prove it from a trajectory. The default hypothesis is a
harness gap with a measurable fix — that is the entire project. See `docs/research/eval-trust.md`
and the Frankenstein harness master plan.
