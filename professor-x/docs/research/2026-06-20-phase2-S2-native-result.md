# Phase 2 S2 result — native tool-calling works (format-fragility class eliminated) — 2026-06-20

## What shipped (flag-gated, default OFF)
`PROFESSOR_X_NATIVE_TOOLS=1` switches the react loop's step from free-form ReAct text →
`/api/chat` with native `tool_calls`:
- `ollama.rs`: ToolSpec/FunctionSpec (offered tools), ToolCall/ToolCallFunction (parsed calls),
  `tool_calls`/`tool_name` on ChatMessage, `chat_with_tools()`, `ChatResponse::tool_calls()`.
- `react.rs`: `tool_specs()` (repo-fix tool catalog as JSON-schema), `build_native_messages()`
  (clean chat history: system + task + prior steps as real assistant tool_calls + tool results),
  `native_step()` → maps tool_calls to `ParsedStep` so ALL downstream gates/execution are reused.
  Concise `NATIVE_SYSTEM_PROMPT` (the big ReAct SYSTEM_PROMPT suppresses tool_calls).

## The key diagnosis
First native attempts still showed 44 `llm.parse_failed` — the model emitted `Thought:` prose, not
tool_calls. Isolation test (direct `/api/chat` + tools, clean minimal prompt) → stock qwen3:8b
returned a PERFECT tool_call (`fs.read({"path":"lru.py"})`, empty content). So the model/Ollama are
fine; the failure was **our prompt**: reusing `build_step_prompt` fed the model the ReAct text tool
list + `Thought:/Action:/Observation:`-formatted history, which primed text output. Fix =
`build_native_messages()` (no ReAct text; history as structured messages).

## Measured (stock qwen3:8b-q4_K_M, hard_003, isolated by rowid)
| metric | ReAct text path | native path |
|---|---|---|
| llm.parse_failed | 44–46 | **0** |
| policy.denied | up to 12 | **0** |
| duplicate_action (looping) | 24–27 | **3** |
| termination | 300s timeout | **clean finish** |
| workflow | thrash | fs.list→read→hash_read→hash_edit→finish |
| pass@1 | 0 | 0 |

## Honest verdict
Native tool-calling **eliminates the entire parse-failure / markdown / infinite-loop class by
construction** and makes the agent terminate cleanly with a correct-shaped workflow. pass@1 is still
0 on this one task — the edit was wrong / hit a bad path (`os error 2`), a **capability** miss at the
same model ceiling, not a format failure. The format tax that caused 0/22 is gone.

## Next
- **S2 full compare:** run native vs text on the full hard set (K passes) for a real pass@1 delta
  (verify-the-ruler; one task proves plumbing, not capability).
- **S3:** collect teacher traces in native tool-call format + retrain (train==collect==serve in one
  structured format) — the distillation path that should finally lift capability.
- Refinements: richer tool_specs (window_goto/scroll, scratchpad), feed ICE/cognition context into
  the native user message, consider think=off for tool-call reliability.
