# TurboQuant/turbovec + quantization techniques we could utilize (2026-06-20)

Scope: local SLM coding agent on an RTX 3060 (12 GB), Rust harness + embeddings/memory store,
distillation pipeline that emits a q4_K_M GGUF via llama.cpp. What's worth adopting, ranked by ROI
for *our* constraints.

## TurboQuant / turbovec (what Abrar asked about)
- **TurboQuant** (Google Research, arXiv:2504.19874, ICLR 2026): a *data-free / data-oblivious*
  online vector quantizer. Randomly rotates vectors → coordinates become ~independent Beta-distributed
  → apply an optimal scalar quantizer per coordinate. Near-optimal distortion at every bit-width
  (≈2.7× the Shannon limit), no codebook training, unbiased inner-product estimates. Strong for KV-cache
  quant and nearest-neighbor search.
- **turbovec** (github.com/RyanCodrai/turbovec): a **Rust** vector index with Python bindings built on
  TurboQuant. 8× memory shrink (e.g. 31 GB → 4 GB for 10M vecs; 6144 B → 384 B per vec at 4-bit),
  **faster than FAISS** IndexPQFastScan by 12–20% on ARM, zero training, add vectors any time.
- **Fit for us:** directly relevant to the *memory-driven self-improvement* identity. Our episodic/
  semantic/embeddings store grows unbounded as the agent accumulates verified work; a data-free Rust
  index that 8×-compresses it (and matches our Rust stack) is the natural backbone. MED effort
  (swap/augment the current embeddings index). Strategic, not urgent.

## Highest-ROI for the distillation pipeline RIGHT NOW
1. **imatrix-guided quantization of the distilled GGUF** *(LOW effort, in our existing pipeline)*.
   We currently run `llama-quantize` to plain **q4_K_M with NO importance matrix**. An imatrix computed
   from a calibration set (use our own repo-fix traces / ReAct transcripts) measurably improves quality
   at the *same* size — it tells the quantizer which weights matter. This is a near-free quality bump
   that could help the distilled model's shaky edit mechanics (bad hashes / indentation). Add an
   `llama-imatrix` pass before `llama-quantize --imatrix` in run_after_reboot.sh.
2. **KV-cache quantization** *(LOW effort)*. llama.cpp `--cache-type-k/--cache-type-v` (q8_0 keys /
   q4 values is the common compromise); Ollama exposes `OLLAMA_KV_CACHE_TYPE=q8_0` (+ flash-attn).
   Frees VRAM on the 3060 → run the 8B with more context, or keep the 14B teacher resident alongside
   the gate without CPU offload. Slight long-context quality cost; fine for our short repo-fix prompts.

## Worth knowing, lower priority
3. **IQ quants (IQ4_XS)** *(situational)*: ~4.46 bpw vs q4_K_M's 4.89 (≈4.17 vs 4.58 GiB for 8B),
   near-identical perplexity *with a good imatrix*. BUT I-quants decompress slower; K-quants give higher
   tok/s on CUDA. Only switch if we need the extra ~0.4 GB; otherwise q4_K_M+imatrix is the sweet spot.
4. **Speculative decoding / MTP** *(MED effort, mixed ROI on 12 GB)*: a small draft model
   (e.g. qwen3:0.6b/1.7b) drafts tokens the 8B verifies → 2–3× throughput in principle. Catch: on
   ≤16 GB the draft/MTP heads eat the KV-cache budget and shrink context. Attractive at 24 GB+, marginal
   on a 3060. Defer until throughput (not capability) is the bottleneck.

## Takeaways
- The agent's *capability* blocker is the training recipe + data scale + native tool-calling (Phase 2),
  not inference speed — so quant tricks are supporting moves, not the main lever.
- Two cheap wins are in-pipeline today: **imatrix quant** (quality) and **KV-cache quant** (VRAM).
- **turbovec/TurboQuant** is the strategic one for the memory spine (Rust, data-free, 8× — fits the
  product identity), to schedule when we harden the memory store.

Sources: arXiv:2504.19874 (TurboQuant); github.com/RyanCodrai/turbovec; research.google TurboQuant blog;
kaitchup "Choosing a GGUF Model" + TurboQuant KV-cache post; llama.cpp quantize README;
runaihome Q4/Q5/Q6/Q8 quality-loss 2026; Medium/Practical-LLM-Systems MTP speculative-decoding test.
