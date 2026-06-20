# Phase 1 parse-check result — format fix works, failure moved downstream (2026-06-20)

## What ran
- Retrained qwen3:8b on the 69-trace corpus with the Phase-1 fix (strip leading `Thought:` from
  training targets, emit it as masked context so train==serve). Clean train loss 0.391.
- Merged offline → GGUF → q4_K_M → served as `professor-x-distilled`.
- **Pre-gate PASSED**: raw `/api/generate` with a tiny prompt → `done_reason=stop` AND emits `Action:`.
- **End-to-end 1-task check** (`REPO_FIX_TASKS=one_task.json`, hard_003 lru.py, distilled model):
  ran the full harness loop. Killed by a 420s timeout (looped), so no pass@1 number — but the
  event trace (216 events) is decisive.

## Verdict: real progress, not a win
**The proximate `0/22` "could not parse step output" wall is BROKEN.** The distilled model now
drives the harness: parseable `Thought:/Action:` steps, called `fs.list → fs.window_open → fs.read`,
and **attempted a real edit** (`fs.hash_edit`, `new_text="self.maxsize = max(1, self.maxsize)"`,
policy-allowed at step 6). Previously it produced nothing the parser could read. Format unification
did what it was supposed to.

**But it does not SOLVE the task.** The failure moved from *format* to *edit-quality + discipline*:
1. **Markdown contamination.** The model wraps actions in backticks/bold —
   `` **`fs.hash_edit({...})`** ``, `` `fs.list({...})`** `` — so the tool-name extractor reads a
   garbage tool name → `policy.denied: tool '…' not in granted tools` (12 denials this run). This is
   the base model's chat/markdown habit leaking through; 69 traces didn't suppress it on the big
   harness prompt (the real prompt is far larger than the tiny pre-gate prompt — that's why the
   pre-gate passed but the harness run struggled).
2. **Bad edit mechanics.** Used a fake `"hash":"abc"` instead of reading the real line hash via
   `fs.hash_read`, and wrong indentation → `edit verification failed (py_compile): IndentationError`.
3. **Loops then bails.** 27 `react.duplicate_action` (retried the same broken edit), then emitted
   "Task complete" prose with no successful edit → 7 `task.finish_rejected`, then forfeit.

Run tally: 54 tool.requested · 27 duplicate_action · 12 policy.denied · 8 parse_failed (down from
"every step") · 7 finish_rejected · 1 fs.hash_edit reached execution (failed verification).

## Why this matters / what it implies
- **Phase 1's hypothesis is confirmed**: the blocker was a train/serve format mismatch, and fixing it
  lets the distilled model actually operate the loop. Good.
- **The remaining problems are exactly what the plan predicted**: (a) markdown/format noise that
  free-form text parsing is fragile to, and (b) too few traces (69 « ~500 floor) to instill clean
  tool-use discipline. Both point the same way:
  - **Strengthens the case for Phase 2 (native `/api/chat` tool-calling)** — structured `tool_calls`
    eliminate markdown contamination and the tool-name-extraction failure *by construction*.
  - **Strengthens the case for data scaling** — clean edit mechanics (use the real hash, correct
    indentation, don't loop) is learnable from more/better verified traces.

## Recommended next steps (need approval — harness code changes, not doing autonomously overnight)
1. **Cheap parser hardening (harness code):** strip surrounding markdown (`` ` ``, `**`) and a
   leading `Action N:` ordinal from the tool-name field before lookup. This alone would have turned
   ~12 denials into valid calls this run. Small, testable, reversible. *Recommend first — fast ROI.*
2. **Then Phase 2 (native tool-calling):** the durable fix; removes the whole text-parse class.
3. **In parallel (safe, done tonight):** grew the hard fixture set 22→26 for trace diversity.

Gate everything on the trustworthy ruler; accept only measured, beyond-noise gains.
