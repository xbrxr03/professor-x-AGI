# Professor X — Project Atlas (living register of EVERY direction)

Principle (Abrar, 2026-06-21): **no direction is abandoned.** Pivots are *sequencing*, not deletion.
The harness is the primary thread; the invention hunt is the key that unlocks it; every parked thread
stays here for eventual completion (the long arc is "complete all approaches → maybe AGI"). Keep this
file current — add rows, never silently drop them.

Status legend: ACTIVE · NEXT · BUILT · VALIDATED · PARKED · SHELVED(evidence) · FALSIFIED · PROGRAM(research).

---
## 0. NORTH STAR (primary)
- **Best agentic harness for LOCAL LLMs** — frontier-level coding experience (cf. Claude Code, Kimi
  Code, Hermes Agent, OpenClaw) from a small model on a $400 GPU. ACTIVE-PRIMARY. Win = (a) match
  frontier harness features, (b) beat them via verifier-driven self-improvement only we do.
  Refs: MILESTONE.md, memory: local_models_mission_and_ux, milestone_north_star.

## 1. SELF-IMPROVEMENT LEVERS (the MHE portfolio — all stay alive)
- **Lever 1 Parametric / weights** — distillation flywheel. ACTIVE (un-paused 2026-06-21 after the
  corrupt-GGUF find; clean distilled 0.40 > stock 0.30). Resume: re-quant from f16 + K-pass gate.
  ❌ **RESOLVED (2026-06-22, TGC gate): the "0.40 > 0.30" un-pause claim is FALSE.** D-integration ran
  the trust gate on the recipe-fixed `profx-distilled-p3`: held-out renamed-anchor pass@1 (K=3) =
  **0.238 (p3) vs 0.500 (stock qwen3:8b) → −0.262, REJECT** (does not generalize; 26pts worse than
  stock). Earlier `profx-distilled-clean` also lost on the hard set (0.133 vs 0.400). So **distilled
  does NOT beat stock**; the harness stays on qwen3:8b. The flywheel is ACTIVE but its current
  candidates underperform — resume = RECIPE iteration (assistant-only mask + EPOCHS=2 + more frontier
  teacher passes), re-gate. Results: docs/research/2026-06-22-RESULT-D-integration-tgc-gate-p3.md +
  audit F1.
- **Lever 2 Contextual / memory** — ICE / memory-driven recall. PARTIAL. Resume: behavior-keyed
  retrieval (see Inventions §2 failure-signature).
- **Lever 3 Structural / harness** — harness self-evolution, SkillOpt-style skill optimizer. ACTIVE.
- **Lever 4 Representational / verifier-driven quant** — SHELVED (precheck: symmetric collapse, no
  asymmetry; base was NaN-corrupt → confounded). Resume only if a Q3/per-layer probe + serving-fix get cheap.
- **Metacognitive self-model** — PARKED (ties levers together; build after kernel proven).

## 2. INVENTION HUNT (KEY to the goal — never stop)
- **Failure-signature embeddings** — VALIDATED (rename-invariant 0.93 vs text 0.14). Embed by which
  verifier-checks fail. NEXT: behavior-keyed RAG (does it lift pass@1 vs text retrieval?).
- **Diagnostic Verifier Codes** — PROGRAM (grounded in SBFL/syndrome-decoding/locating-arrays/MBD).
  Verifier co-designed as a code over the fault space so failures decode the fix. Kernel: 6/7 families
  already unique-syndrome; 44% checks redundant (rateless headroom).
- **Living Verifier / self-improvement = channel-code co-design** — PROGRAM. Kernel validated; the
  open-world novelty-growth pillar UNPROVEN (open-set 35% OOD, confounded). Frontier bet.
- **TGC (Transfer-Gated Co-Evolution)** — BUILT + RAN (integration-novel). Accept a harness/weight
  update only if it improves verifier on RENAMED held-out anchors; logs the Goodhart gap. Targets SIA's
  named open problem. 2026-06-22/23: gate ran on `profx-distilled-p3` → **REJECT** (held-out 0.238 <
  stock 0.500, gap~0 = worse-everywhere, not overfit) — the gate works (refuses a non-generalizing
  candidate). **UPGRADED to Collateralized-TGC** (2026-06-23): + per-anchor no-drawdown collateral
  (self-test PASS, scalar accepts / collateral rejects a gains-mask-drawdown candidate) — the buildable
  nugget from Codex's CGW. Docs: 2026-06-23-collateralized-tgc-gate.md, 2026-06-22-RESULT-D-integration-*.
- **AACE (Goodhart drift-tripwire)** — DESIGNED. Detect reward-hack/drift in the loop.
- **CGW/CLT (Codex, collateralized cognition / counterfactual-liquidity consciousness)** — PARKED
  (long-arc §5). Genuine-shaped novelty (reverse transplant: finance no-arbitrage math as self-mod
  control law) but untested theory in the frozen lane; its kernel is being validated in miniature by
  Collateralized-TGC above. Docs: 2026-06-23-codex-cgw-novelty-spotcheck.md.
- **VCA (Verifier-Counterfactual Credit Assignment)** — CANDIDATE (delta-debug green diffs for credit).
- **VGTS (verifier-grounded embedding) / Re-Verified RAG** — CANDIDATES, were benchmark-blocked; now
  UNBLOCKED by reuse-families (transfer 0.979).
- **Counterfactual Verifier Head** — INCONCLUSIVE (harness/thinking-channel bug; weak premise since our
  verifier is cheap). Revisit only for the dense-credit angle.
- **Verifier-causal self-lesioning** — SHELVED (corrupt base, no coarse localization, least novel).
- **Compression / Residual gate** — FALSIFIED (templated hacks compress better; premise reversed).

## 3. HARNESS FEATURES (table-stakes vs frontier CLIs)
- **Native tool-calling** (/api/chat, flag-gated) — BUILT. Kills format-fragility class.
- **Parser hardening** (normalize_action) — BUILT.
- **MCP client · sub-agents/mirror critic · repo.map · Tree-of-Thoughts** — BUILT (harness-gaps branch).
- **Edit-lever + anti-thrash cluster** — NEXT (the documented gap that moves p_correct on weak models:
  fuzzy search/replace, validate-before-apply, auto-retry, loop/dup-action breaking).
- **Rust port / codex-portability** — PLANNED (Frankenstein master plan).
- **Tool timeout, reward-hack guard, native baseline pin** — BUILT (Stage 0).

## 4. BENCHMARKS / RULERS (verify-the-ruler discipline)
- **repo-fix** (red→green, stdlib check.py, pass@1, K-pass) — ACTIVE headline ruler.
- **Reuse-families + renamed anchors** — BUILT 2026-06-21 (7 families/34 tasks + 14 anchors; transfer
  0.979 confirmed; all in ZPD band). Unblocks VGTS/RAG/VCA + TGC + the verifier-code program.
- **Self-authored test store / --generate-curriculum** — BUILT (unbounded diversity).
- **M1 real benchmark** (SWE-Gym / R2E-Gym validation) — PLANNED (stepping-stone realism).

## 5. CONSCIOUSNESS / AGI DIRECTION (frozen post-north-star, NOT deleted)
- **7 consciousness seeds** — oscillatory cognition, STDP causal learning, complementary learning
  systems, computational interoception, DMN, narrative self, predictive self-modeling. PARKED.
- **Consciousness measurement / phi instrument** — BUILT; integration measurable, consciousness NOT
  demonstrated (5/7 modules were degenerate, fixed instrument). PARKED.
- **IPE (Identity-Preserving Evolution: Strange Loop, Free Energy, ICS)** — DESIGNED. PARKED.
- **Functional Affect system** — DESIGNED. PARKED.
- **DFA Trifecta (DHE diagnostic, BF fingerprint, LCAP context allocation)** — DESIGNED. PARKED.
- Long arc: these are the "complete all approaches → maybe AGI" threads; resume after the local-harness
  kernel + self-improvement loop are proven.

## 6. PRODUCT / UX
- **profx TUI · --serve · @file launch · ONNX-embed steal (jcode)** — PARTIAL/BUILT. PARKED behind north star.

## 7. INFRA / TOOLING
- **Distillation env gauntlet · llama.cpp convert+quantize · stop-sanity + GPU guards** — BUILT.
- **NaN/stop-sanity validation on the quantize step** — TODO (from the 2026-06-21 corrupt-GGUF find;
  never gate a NaN gguf again).

---
## How to use this atlas
Each row is resumable: it names the status + the resume hook. When picking work, default to the NORTH
STAR + its unblockers (Levers 1/3, edit-lever, behavior-keyed retrieval), keep the INVENTION hunt
running in parallel (it's the moat), and periodically revisit a PARKED thread. Update this file whenever
a status changes — it is the single place that guarantees no direction is lost.
