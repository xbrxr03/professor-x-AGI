# Phase 2 plan — native tool-calling via /api/chat (the durable fix) — 2026-06-20

## Why now (evidence)
Across 4+ distillation cycles the recurring wall has been *free-form ReAct text parsing*: EOS/template,
label-doubling, and now (2026-06-20) **markdown contamination** of the Action field. Phase 1 fixed the
proximate parse bug and the parser hardening (`normalize_action`, commit 2c4006e) drove markdown
`policy.denied` 12→0 — but the distilled model still loops and emits malformed edits. The harness loop
already escalates anti-loop nudges hard (react.rs ~1769); the model ignores them. **Text parsing is a
structural fragility, not a bug to keep patching.** Native `tool_calls` remove the entire class by
construction and are the production-proven path for SLM agents (ToolBench, LangChain tool-calling).

## What changes (file-level)
The executor already dispatches by `action.tool_name` string (`toolbridge/executor.rs:199`) — that is
reusable unchanged. The work is at the LLM boundary.

1. **`src/ollama.rs`** — add tool-calling to the chat path (it already has `chat()` @389 and
   `/api/chat` @566):
   - Extend `ChatRequest` with `tools: Option<Vec<ToolSpec>>` (serde, omit when None).
   - Add `ToolSpec`/`FunctionSpec` structs = OpenAI-style `{"type":"function","function":{name,
     description, parameters(JSON-schema)}}` (Ollama's qwen3 tool format).
   - Extend `ChatMessage` to carry `tool_calls` (assistant) and a `tool` role with `tool_name`.
   - Parse `message.tool_calls[].function.{name,arguments}` from the response.

2. **`src/agentd/react.rs`** — add a native-tool-calling step path behind a flag:
   - New `tool_specs()` builder: emit the same tool catalog already in SYSTEM_PROMPT (fs.read,
     fs.hash_edit, patch.apply, finish, fail, …) as `ToolSpec` JSON-schema entries. Single source of
     truth — generate both the prompt text and the specs from one table.
   - New `run_step_native()`: send running transcript as chat messages + tools; read `tool_calls`;
     map each to `Action{tool_name, params}` (no text parsing); execute via the existing executor;
     append the assistant `tool_calls` message + a `tool`-role observation message. Reuse ALL existing
     guards (duplicate-action, finish-gate/edit-required, synthesis, policy gate).
   - Gate it on `PROFESSOR_X_NATIVE_TOOLS=1` (env) so the text path stays default until proven.

3. **Collection (`collect_trajectory`)** — when native mode is on, record assistant turns as
   `tool_calls` (structured) + `tool` observations, so train==collect==serve in ONE format.

4. **Training (`distill/train_qlola.py`)** — add a chat-template builder that renders the qwen3 tool
   schema + `tool_calls` with assistant-only loss (already have masking). Pick ONE format end-to-end.

## Staged rollout (low-risk, each step measured)
- **S1 (read-only proof):** implement ollama.rs tools + `tool_specs()` + `run_step_native()` for a
  SUBSET of tools (fs.read, fs.list, fs.window_open, finish). Run hard_003 with the **stock** qwen3:8b
  under `PROFESSOR_X_NATIVE_TOOLS=1`. Success = clean tool_calls, no parse failures, model reads then
  finishes. Proves the plumbing on a known-good model before touching training.
- **S2 (full tools + edits):** add fs.hash_edit/fs.write/patch.apply etc. Re-run a few hard tasks with
  stock 8b; confirm edits apply and the gate runs. Compare pass@1 vs the text path (verify-the-ruler).
- **S3 (collect + train native):** collect teacher traces in native format, retrain, gate. Only here do
  we touch the training format.
- Keep the text path as fallback until S2 beats it beyond noise.

## Risks / mitigations
- Ollama qwen3 tool-calling quirks (arguments as string vs object) → normalize in the parse step.
- Some "tools" are harness-internal control (finish/fail/synthesize) → keep them as tools in the schema
  so the model selects them structurally instead of via prose.
- Bigger surface → land behind the env flag, full `cargo test --bins`, measure each stage, never rip out
  the text path until S2 is green.

## Expected payoff
Eliminates markdown/label/parse/edit-format fragility for good; makes collect==train==serve trivially
consistent; aligns with the SLM-agent production standard. This is the lever, not another patch.
