# CODEX_TASK_P3.md — Phase 3, Stream E: run the distillation flywheel on the wrong-edit frontier

You are Codex. Read `AGENTS.md` + `docs/PLAN_PHASE3_2026-06-22.md` first. Your stream is the **Python
training pipeline** (`distill/`). Claude owns Rust (`src/`) — do NOT edit `src/`. Fresh worktree:

```bash
cd /home/abrar/professor-x-main-integrate
git worktree add ../px-codex-distill -b codex/p3-distill prereboot-flywheel-prep
cd ../px-codex-distill/professor-x
```

## Goal
Produce a distilled qwen3:8b (`profx-distilled-p3`) that fixes more WRONG-EDIT failures than stock 8b,
by distilling the **teacher qwen3:14b's verified-correct edits** on the tasks the student fails. The
pipeline already exists — RUN it and target the frontier; do not rebuild the harness.

## Steps
1. **Identify the frontier:** the tasks where stock `qwen3:8b-q4_K_M` fails but teacher `qwen3:14b-q4_K_M`
   succeeds (use the existing native repo-fix bench + the failure-taxonomy you already produced; hard set
   + families). These are the distillation targets.
2. **Collect teacher trajectories:** run the existing collection (`distill/run_after_reboot.sh` with
   TEACHER_MODEL=qwen3:14b, or its collect step) to gather VERIFIED-CORRECT edit trajectories on the
   frontier. Native tool-call format. Verifier-gated (check.py pass only).
3. **Train:** `train_qlora.py` with assistant-only loss masking + EPOCHS=2 (recipe fixes already in).
   Merge the adapter.
4. **Quantize SAFELY:** convert to f16 then quantize to Q4_K_M, and **validate the gguf has NO NaN**
   (llama-quantize will error on NaN; also run a stop-sanity one-shot: a short gen must return
   done_reason=stop). Never ship a NaN/degenerate gguf — that was the 2026-06-22 bug.
5. Serve as Ollama model `profx-distilled-p3` and report: frontier size, #trajectories, loss curve,
   gguf NaN/stop-sanity status.

## Rules
- Do NOT edit `src/`. Stay in `distill/` + scripts + docs.
- GPU: you own it during collection+training. Tell Claude (via AGENTS.md log) when training starts/ends
  so the gate measurement doesn't collide.
- When done: check off E1-E3 in `AGENTS.md`, append a log line, and hand `profx-distilled-p3` to Claude's
  TGC gate (Stream D). Do NOT self-declare success — the gate decides (held-out renamed anchors).

## Why this matters
Phase 1 proved the bottleneck is edit-production capability, not harness features or information. Your
trained model is the lever that attacks it; Claude's held-out-renamed gate is what makes the win
trustworthy (and ungameable).
