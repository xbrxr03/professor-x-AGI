# Plan: fix the train/serve format mismatch (the distillation blocker) — 2026-06-20

**Status:** PLAN — awaiting Abrar's approval to implement (per his instruction).

## Why (the null that motivates this)
Across ~4 distillation cycles, the recipe got *correct* (assistant-only masking, alpha=2r,
weight_decay → clean train loss 0.38) but the gated model scored **0/22** — the harness **"could not
parse step output"** and forfeited every task. Root cause: a **train/serve FORMAT mismatch** that's
structural in the harness, exposed differently by each fine-tune:
- `collect_trajectory` stores assistant turns that START with `Thought:`.
- The bench serves via raw `/api/generate` with a prompt that ENDS in `Thought:` and expects the
  model to continue *without* the label (layout-B). The stock model tolerates this; a fine-tuned
  model over-learns the label / verbose thoughts → the free-form text parser extracts no Action.

This is the 3rd facet of the same class (chat-vs-raw → EOS/template → label/parser). Recipe tweaks
can't fix a structural format inconsistency.

## What the research says (and it's decisive)
- **SLMs are the right bet** ([NVIDIA 2506.02153](https://arxiv.org/abs/2506.02153)): <10B models are
  "good enough" and 10-30× cheaper for repetitive agent tasks (tool-calling, structured steps) —
  validates the whole local-SLM-agent thesis. Use a big model only for plan/decide; SLMs for errands.
- **Native tool-calling beats free-form ReAct text parsing** ([ToolBench fine-tune 77%](https://arxiv.org/html/2512.15943);
  [LangChain tool-calling vs ReAct](https://medium.com/@dzianisv/...)): structured tool_calls "avoid
  errors in parsing free-form text for tool arguments" — *literally our 0/22 bug*. Production SLM
  agents fine-tune on the chat template's native tool_calls, not text ReAct.
- **Format consistency is essential**: successful fine-tunes use ONE standardized template for
  collect = train = serve (system/user/assistant with tool_calls JSON).
- **Data scale**: [FireAct](https://fireact-agent.github.io/) — ~500 samples to learn the agent
  format, 1000 better. We used 69. Way under the floor.

## The plan (staged)

### Phase 1 — Unblock: make collect == serve in the EXISTING ReAct text format (~half day)
Pick ONE canonical text format and use it identically in collection, training, and inference.
- **Decision:** stop pre-filling `Thought:` in the inference prompt; let the model emit its full
  `Thought:/Action:/Action Input:` turn exactly as `collect_trajectory` records it (layout-A only).
  (Alternative: strip the leading `Thought:` from collected turns. Either way — ONE format.)
- Files: `build_step_prompt` (drop trailing `Thought:`), the parser in `react.rs` (require layout-A),
  keep `collect_trajectory` as the canonical format.
- Re-collect teacher traces (now format-consistent), scale to ~500+, re-train (corrected recipe),
  re-gate on the 22 hard tasks. **This validates distillation CAN work once formats match.**
- Add a stronger pre-gate: assert the produced Action names a REAL tool (not just contains "Action:").

### Phase 2 — Durable fix: move the agent loop to NATIVE tool-calling via `/api/chat` (~1-2 days)
- Switch the agent from raw `/api/generate` + text parsing to **`/api/chat` with the model's native
  `tool_calls`** (Ollama supports qwen3 tool-calling). Tools declared via the tools schema, not text.
- Collect traces as chat messages with structured `tool_calls`; train with `assistant_only_loss` on
  the chat template; serve the same way. **Eliminates free-form parsing entirely → the whole class of
  format bugs is gone by construction**, and it's the production-proven SLM-agent path.
- Bigger change (agent loop + executor + collection + training all move to chat/tool-calls), so it's
  Phase 2 after Phase 1 proves the thesis cheaply.

### Data (parallel, no-GPU)
Grow the hard fixture set toward enough DISTINCT problems that ~500-1000 verified teacher traces are
diverse, not 8 problems × many passes.

## Honest expectation
Phase 1 should flip the 0/22 (the parse bug is the proximate cause). Phase 2 removes the fragility for
good. Neither is a recipe tweak — both are harness/format unification, which is the actual blocker.
Gate every step on the trustworthy ruler; accept only measured, beyond-noise gains.
