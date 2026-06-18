# 2026-06-17 — The distillation gauntlet

**Goal:** run one turn of the distillation flywheel end-to-end after the reboot — teach the local
8B (qwen3:8b) from a stronger teacher's verified solutions, then let the ungameable repo-fix gate
decide if it actually improved.

**Headline:** the *training* was easy and worked first try. Everything else — packaging the model
so it can even run in the benchmark — was a multi-hour gauntlet. **The bug was never the ML. It was
the plumbing.** First real gate verdict: **REJECT-by-ceiling** (the test is too easy to measure a
gain, not evidence the model is bad).

---

## Problems → fixes (in the order they bit us)

| # | Symptom | Root cause | Fix | Status |
|---|---------|-----------|-----|--------|
| 1 | `pip: command not found` | System Python 3.12 is PEP-668 externally-managed, no pip/venv | Bootstrap `virtualenv` (no sudo); isolated `distill/.venv` | ✅ |
| 2 | Trainer crashed *after* 2h of collection | `rich` pulled in transitively by unsloth_zoo but not declared | Add `rich` to deps | ✅ |
| 3 | `fp16 on a bf16 model` error | Hardcoded `fp16=True` on Ampere (bf16) | Auto-pick precision via `is_bfloat16_supported()` | ✅ |
| 4 | Triton can't JIT CUDA kernels (`Python.h` missing) | No `python3-dev` (apt needs sudo) | Extract dev-header `.deb`s into `distill/.pydev`, expose via `CPATH` (no sudo) | ✅ |
| 5 | GGUF export hangs on `sudo apt install cmake` prompt | Unsloth builds llama.cpp itself; Ollama can't import Qwen3 safetensors | Build a libcurl-free `llama-quantize` + use `convert_hf_to_gguf.py` (no sudo) | ✅ |
| 6 | QLoRA OOM mid-train | A stray **parallel** flywheel run loaded the 10GB teacher onto the 12GB card | `flock` single-instance lock + free-GPU-before-train guard | ✅ |
| 7 | Merge hung 20+ min (CLOSE-WAIT) | Unsloth re-downloads the fp16 base mid-merge; connection dropped | Merge **offline** from the cached base (`merge_fp16.py`) | ✅ |
| 8 | **Model loops forever, never stops** (a single gate pass ran ~12h before we caught it) | The serving **Modelfile was bare** — no chat template, no `PARAMETER stop <|im_end|>` | Clone Ollama's official qwen3 Modelfile (template + stop tokens) | ✅ |
| 9 | Model stops in `/api/chat` but **loops in the benchmark** | **Train/serve format mismatch**: trained in Qwen3 chat template, but the bench drives it via raw `/api/generate` ReAct text → out-of-distribution | Train in the **raw ReAct format** the bench actually uses (Option A) | ✅ |
| 10 | Gate result = **REJECT** (0.976 → 0.953) | The 14-task benchmark is **saturated** — baseline already ~14/14, no headroom (can't beat 0.976 + MDE > 1.0) | Grow/​harden the benchmark (50 validated tasks built; re-pin + re-gate) | ⏳ next |

## The two real lessons
1. **The hard part of small-model AI is the harness, not the model.** Nine of ten problems above
   were environment/packaging/serving — exactly the project's thesis, learned the hard way.
2. **You can't measure an improvement against a saturated ruler.** Baseline scoring ~100% means a
   "reject" tells you nothing about the model — only that the test is too easy. Build the honest,
   *hard* ruler first. (See [`DECISIONS.md`](DECISIONS.md).)

## What's working now
- End-to-end pipeline: collect → curate → QLoRA (raw format) → offline merge → convert → quantize →
  serve (official template) → **pre-gate check** (catches non-halting models in seconds) → gate.
- A functional distilled qwen3:8b that halts and emits proper ReAct in the bench's own format.

## Numbers
- Baseline qwen3:8b, K=3 on 14 tasks: `0.929 1.000 1.000` → **mean 0.976**
- Distilled (Option A), K=3: `0.929 0.929 1.000` → **mean 0.953**  (Δ −0.024, within one-task noise)
- Verdict: **REJECT** (no gain > MDE 0.05) — ceiling/noise artifact, not a bad model.

## Next
Re-pin the baseline on the **50-task** benchmark (already built + validated red→green) where the
baseline is *not* at the ceiling, then re-run the gate for a verdict that can actually move.
