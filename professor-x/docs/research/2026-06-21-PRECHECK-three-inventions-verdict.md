# Verdict: three invention pre-checks + an accidental flywheel bug-find (2026-06-21)

Ran cheap falsifications for the three genuinely-new candidates. Applied skills: px-experiment-runner,
verify-the-ruler, adversarial-self-review.

## 1. Failure-signature embeddings — SURVIVES (genuinely new, validated)
Embed by WHICH verifier-asserts fail, not by text. Rename-invariance test: a renamed anchor recovers
its exact origin by behavioral signature **13/14 (0.93)** vs text **2/14 (0.14)**, chance ~0.21. Text
similarity is destroyed by renaming; the behavioral signature is invariant. This is a representation
text/code embeddings cannot produce — contamination-proof behavioral matching.
- Fix-localization (predict buggy_module) did NOT beat text (0.35 vs 0.47) → value is behavioral
  RETRIEVAL, not credit assignment. Honest scope.
- NEXT (falsify the application): behavior-keyed RAG — index solved cases by signature; on a new
  failing task retrieve the nearest-signature solved case and inject its fix. Win = signature-retrieval
  raises pass@1 over text-retrieval / no-retrieval on held-out (renamed) anchors.
- Doc: 2026-06-21-PRECHECK-failure-signature-embeddings.md. Status: KEEP.

## 2. Counterfactual Verifier Head — INCONCLUSIVE (broken ruler) + weak premise
Cheap test (does model self-confidence predict verifier pass/fail, AUC) returned **0/34 pass →
AUC uncomputable**. Root cause = harness bug, NOT signal: `/no_think` did not disable qwen3 thinking,
so `message.content` came back EMPTY (output went to the `thinking` field) and empty files were written
(verified: p2-fam_graph_02/graph.py was empty). Same thinking-channel bug that bit the quant probe.
- Also: the premise is WEAK for us — our verifier is already cheap+deterministic, so a learned head
  mostly duplicates a call we can afford; its only real value is dense per-token credit, which overlaps
  existing PRM/credit-assignment work (2604.09459, GiGPO, iStar).
- To test properly: use Ollama `"think": false` + larger num_predict, or capture logprobs from real
  AGENTIC attempts. Status: DEPRIORITIZE (low marginal value + harness cost); revisit only for the
  dense-credit angle.

## 3. Verifier-causal self-lesioning — KILLED (for now)
Not cleanly testable on current artifacts: the only base GGUF we can prune is the distilled model,
which is NaN-corrupted (see below). Prior counter-evidence: the quant probe already showed no coarse
localization (attn/ffn cratered symmetrically). Also the least novel of the three (ablation-for-interp
is well-trodden; only the "task verifier as probe" twist is ours). Proper test needs a clean white-box
ablation harness (CUDA llama-cpp-python with tensor-zeroing hooks) — a tooling project, not a cheap
probe. Status: SHELVE.

## BONUS (the real prize): the distillation flywheel's served model was a CORRUPT BUILD
While trying to prune for #3, llama-quantize found a **NaN in `blk.4.attn_v.weight`** of
`distill/out/gguf/distilled-Q4_K_M.gguf`. Localization:
- f16 distilled weights (`distilled-f16.gguf`) → Q8_0: **CLEAN** (exit 0, no NaN).
- fresh f16 → Q4_K_M: **CLEAN** (exit 0, no NaN).
- therefore the **shipped `distilled-Q4_K_M.gguf` was a bad/stale build** (corrupted after a clean
  train), and every downstream artifact (served distilled model + quant-probe base/attn/ffn) inherited
  the NaN.
**Implications:** (a) the distilled model's TRAINING is fine — the "degenerate distilled model" verdict
was partly read through a corrupt quantization; (b) the fourth-lever quant probe was confounded by a
corrupt base (its verdict is even less interpretable than recorded); (c) trivial fix — re-quantize from
the clean f16 (clean Q4_K_M at /tmp/repro_q4km.gguf, clean Q8_0 at /tmp/nan_test_q8.gguf ready).
**Highest-value next action:** re-quantize cleanly, re-serve, and RE-BENCHMARK the distilled model —
this could un-stick the paused distillation flywheel.

## One-line scorecard
#1 KEEP (new + validated) · #2 inconclusive/deprioritize · #3 shelve · BONUS = flywheel un-blocker.
