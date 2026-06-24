# Toward "frontier feel in 12GB": 14B capability + a real-feel benchmark (2026-06-24)

Goal (Abrar): the agent should FEEL like OpenClaw-with-a-frontier-model, but be a local LLM + harness
in 12GB VRAM. This drives the capability leg toward the biggest model that fits 12GB (not a distilled
8B — 4 distillation strikes). Method: verify-the-ruler.

## 14B vs 8B capability validation (hard set, native tools, K=1)
| model | pass@1 | made_edit | fits 12GB? |
|---|---|---|---|
| qwen3:8b-q4_K_M | 0.467 | 28/30 (93%) | yes (5.2GB) |
| **qwen3:14b-q4_K_M** | **0.533** | **29/30 (97%)** | **YES** (9.3GB; ran to completion, no OOM) |

- **14B is better AND fits 12GB.** +0.066 pass@1, made_edit 93%→97% (toward frontier-feel reliability).
- **Validates the path:** use the bigger model that fits, not distill the 8B. Replaces the dead
  distillation-of-8B lever for the capability leg.
- **Honest caveats:** K=1; +0.066 ≈ 1–2 tasks on 30 (within MDE ~0.067) → directional, needs K=3 to be
  beyond-noise. 14B (9.3GB) leaves ~2.7GB for KV cache on 12GB — fine for repo-fix's short contexts;
  LONG real-repo contexts may need q8 KV-cache quant. To verify on real-feel tasks (below).

## Real-feel benchmark (the measurement gap)
Toy 1-function fixtures can't tell us if it "feels frontier." Started a REALISTIC tier
(`tasks_real.json`): multi-file, stateful tasks with behavioral tests, all red→green validated:
- **real_01** — KV store with TTL: off-by-one expiry (item lives one tick past expiry). store.py+clock.py.
- **real_02** — expression evaluator: operator-precedence bug (2+3*4 → 20 not 14). lexer.py+evaluator.py.
- **real_03** — event-sourced ledger: transfer debits source but never credits destination. events.py+ledger.py.
Each requires cross-file reasoning + understanding real logic (vs "add returns a-b"). Next: grow this
tier (10–20 tasks) and bench 14B vs 8B on it (does the bigger model help more on realistic tasks?).

## Path to the goal (the three legs)
1. **Capability** — 14B base (fits 12GB, validated ≥ 8B). NEXT: K=3 confirm + KV-cache config for long ctx.
2. **Harness** — Codex's agentic-perf track + validated edit-lever/native-tools/repo-map.
3. **Trust-gated self-improvement** — Collateralized-TGC (now 34 anchors) + Living Verifier, so it
   improves on real usage. Measured on the real-feel bench.

## Update: 14B context/VRAM fit (the KV-cache caveat)
`ollama ps` during a 16384-context load: `qwen3:14b-q4_K_M  12 GB  9%/91% CPU/GPU`. So:
- 14B (9.3GB weights) **fully fits 12GB at SHORT contexts** (the hard-set bench ran on-GPU, completed).
- At **16k context it partially offloads (~9% to CPU)** → slower (the KV cache + weights exceed 12GB).
- **For "frontier feel in 12GB": use `OLLAMA_KV_CACHE_TYPE=q8_0`** (halves KV-cache VRAM) and/or cap
  context, so 14B stays fully on-GPU at useful context lengths. This is a config lever (Codex's harness
  area) — flag it: the agent should pin a context budget that keeps 14B on-GPU.

## Path to the goal (concrete, integration)
1. **Base:** qwen3:14b-q4_K_M (validated ≥ 8B, fits 12GB short-ctx). Pin context + q8 KV so it stays
   on-GPU. NEXT: K=3 confirm (queued) + 14B-vs-8B on the 9-task real-feel tier (queued).
2. **Harness (Codex):** agentic-perf track + native-tools/edit-lever/repo-map — make 14B *feel* frontier.
3. **Trust (Claude):** Collateralized-TGC (34 anchors) + Living Verifier so it improves on real usage —
   the local differentiator a cloud frontier agent can't offer.
4. **Measure:** the real-feel benchmark (9 validated multi-file tasks) + the real M1 (SWE-Gym-lite) later.
