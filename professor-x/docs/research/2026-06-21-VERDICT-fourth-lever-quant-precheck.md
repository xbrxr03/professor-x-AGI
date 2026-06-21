# VERDICT: Verifier-Driven Quantization (fourth lever) — cheap pre-check (2026-06-21)

Decision: **NOT SUPPORTED by the cheap pre-check → SHELVED** (not a clean kill, not a win).
Applied skills: px-experiment-runner, verify-the-ruler, adversarial-self-review.

## What was tested
Pre-registered (in `2026-06-21-INVENTION-fourth-lever-verifier-driven-quant.md`):
> if attn-demote AND ffn-demote both land within ~1 task of baseline → FLAT → KILL.
> if one craters while another holds → sensitivity REAL → proceed to per-tensor budget search.

Three Ollama models from the same qwen3 GGUF, native-tools repo-fix, 10-task fixed subset
(hard_001–010):
- `profx-quant-base`   — uniform Q4_K_M (5.0 GB)
- `profx-quant-attnq2` — attention tensors → Q2_K (4.65 GB)
- `profx-quant-ffnq2`  — FFN tensors → Q2_K (3.52 GB)

## Results (the trustworthy, agent-loop signal)
| variant | pass@1 | observation |
|---|---|---|
| base | **0.200 (2/10)** | clean run, ~8.5 min |
| attn→Q2 | — (no number) | emits unparseable garbage, then crashes the Ollama runner; reproduced on an isolated fresh-server run (conn_errors=3) |
| ffn→Q2 | — (no number) | same signature: garbage output → runner crash; reproduced (conn_errors=3) |

## Honest reading (adversarial-self-review)
- Pre-registered rule had two branches (both-flat→kill, one-holds→real). **Neither fired.** The
  actual outcome is a third case the rule didn't enumerate: **symmetric catastrophic collapse** —
  both demotions break the model identically while base works.
- What this DOES establish: precision is **not** a free/flat dimension (Q2 is fatal). The "precision
  matters" half of the premise holds.
- What this does NOT establish (and is what the lever actually needs): an **exploitable asymmetry**
  between tensor classes. Both attn-Q2 and ffn-Q2 collapse → no "compress the tolerant one, protect
  the sensitive one" signal at this granularity. The lever's value proposition is unproven.

## Confounds I will not paper over (verify-the-ruler)
1. **Q2 is the bluntest setting.** "Both break" can mean Q2 is too aggressive to *reveal* an
   asymmetry that exists at Q3_K or per-layer granularity. A complete kill would test a milder
   demotion. So this is "not supported," not "disproven."
2. **Serving fragility is a real cost.** The custom per-tensor requant GGUFs reproducibly crash the
   Ollama runner (the same runner-crash class that killed Stage 2). Even if asymmetry exists, the
   requant→serve path is operationally fragile on our actual stack — that raises the lever's cost
   independent of the science.
3. **Coarse baseline.** base = 0.200 (2/10) → resolution is 0.10/task; weak baselines mask mild
   effects. The earlier 30-task run was discarded outright as a **broken ruler** (every variant hit
   the 1500s cap mid-run → empty pass@1; no conclusion drawn from it).

## Recommendation
- **Shelve the fourth lever** (alongside the compression gate). Revisit ONLY if both become cheap:
  (a) a Q3_K / per-layer asymmetry probe, AND (b) a fix for the requant→Ollama crash.
- **Do not sink more compute now.** The probe was meant to be cheap-and-decisive; it consumed hours
  and repeated infra crashes. Marginal ROI on a Q3 round is uncertain and the serving path is fragile.
- **Pivot to Track A** — the 7-family reuse benchmark is built (34 validated tasks) and is the
  keystone that unblocks VGTS/RAG/VCA. Next: qwen3:8b pass@k across families (ZPD filter) + measure
  within-family transfer against the recipe's falsifiable gate.

## One-line status for the portfolio
Lever 4 (representational / verifier-driven quant): **candidate, not supported by cheap probe;
shelved pending a cheaper asymmetry probe + a serving fix.**
