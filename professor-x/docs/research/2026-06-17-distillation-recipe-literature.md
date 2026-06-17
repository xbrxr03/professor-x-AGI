# Distillation recipe — literature check (2026-06-17)

Triggered by turn-1 REJECT: our distilled qwen3:8b reasons correctly about repo-fix tasks but
won't emit the ReAct action format and never stops (done_reason=length). Diagnosis: training-recipe
problem (no completion-only masking; 1 epoch; ReAct-vs-native-format clash). The literature confirms
this and hands us concrete fixes.

## The direct fix — `assistant_only_loss` (highest priority, ~1 flag)
TRL's `SFTConfig(assistant_only_loss=True)` computes loss **only on assistant responses** (masks
system/user/observation tokens to -100). This is exactly the missing piece — our current
`train_qlora.py` trains on the full conversation text, diluting the signal to produce the assistant
format + EOS.
- **Crucially: "For known model families (e.g. Qwen3), TRL automatically patches the [chat] template
  when assistant_only_loss=True"** — so it Just Works for our base, no manual `{% generation %}`.
- **Caveat (TRL issue #3927):** if assistant tokens fall beyond `max_length`, masks become all-0 →
  0 loss silently. Our trajectories are long (multi-turn) — keep MAX_SEQ large enough OR truncate
  carefully, and assert non-zero loss.
- Refs: [TRL SFT docs](https://huggingface.co/docs/trl/sft_trainer) ·
  [issue #3927](https://github.com/huggingface/trl/issues/3927)

## The better frame — agent/trajectory distillation (if the flag isn't enough)
Recent work (2025-2026) is exactly our problem: distilling a ReAct teacher into a small student.
- **Structured Agent Distillation** — segments trajectories into **reasoning spans vs action spans**
  and aligns each separately (span-level, beyond token-level imitation). Preserves both reasoning
  fidelity AND action-format consistency — directly targets our "reasons but won't act/stop" failure.
  [arxiv 2505.13820](https://arxiv.org/html/2505.13820v4)
- **Distilling LLM Agent into Small Models with Retrieval and Code Tools** — "first-thought prefix"
  to improve teacher trajectories + "self-consistent action generation" at test time. Reports even
  0.5B-3B students matching next-tier CoT-distilled models. Has reference code.
  [paper 2505.17612](https://huggingface.co/papers/2505.17612) ·
  [code: Nardien/agent-distillation](https://github.com/Nardien/agent-distillation)
- **Agent Distillation overview:** [emergentmind](https://www.emergentmind.com/topics/agent-distillation)
- Recipe note from these: reformat ReAct data with **both short and long CoT for an SFT cold-start**,
  then optionally an RL stage on held-out tasks. Our pipeline is SFT-only (cold-start) so far.

## Open decision this informs (from PLAN_11_10.md resume point)
Keep plain ReAct text vs reformat as Qwen3-native tool-calls. Literature leans: **keep ReAct but
mask to assistant-only and segment reasoning/action spans**, rather than fight Qwen3's native format.
The model already reasons; we need to (a) only train it on its own outputs, (b) make it emit the
action format + EOS reliably.

## Concrete next experiment (cheapest → strongest)
1. `assistant_only_loss=True` + EPOCHS=2 in `train_qlora.py`; assert non-zero loss; retrain →
   stop-sanity (must be done_reason=stop) → gate. (Cheapest; likely fixes stopping + format.)
2. If still weak: span-level reasoning/action segmentation (Structured Agent Distillation) and/or
   first-thought-prefix teacher trajectories.
3. Only then consider an RL stage.

Ties to: [[milestone_north_star]], [[distillation_flywheel]], docs/research/eval-trust.md.
