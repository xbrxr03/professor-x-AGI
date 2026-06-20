# Topics to explore — research scan (2026-06-20)

Scan for directions relevant to our goal (local, memory-driven, dual-lever self-improving coding
agent on an RTX 3060). Ranked by ROI given our constraints. Honest note: most are techniques to
*adopt*; the novel bet remains the unification (memory → harness + weights → one gate, local).

## Tier 1 — directly attacks our current blocker (the weak distilled model)

### 1. On-policy distillation (OPD) — replaces our static SFT
Our flywheel does **off-policy SFT**: train on the teacher's traces, then hope the student behaves.
That's exactly what failed — the student drifts (loops, bad edits) because it never trains on *its own*
mistakes. **OPD has the STUDENT generate rollouts and the teacher score/correct them on-policy.**
- SOD: Step-wise On-policy Distillation for SLM Agents (arXiv 2605.07725) — reweights teacher guidance
  by step-level divergence to stop **tool-induced cascade drift** (our exact failure mode).
- TCOD (2604.24005) temporal curriculum; MAD-OPD (multi-agent-debate teacher); ROPD (rubric-based).
- DeepSeek-V4 adopted OPD as a core ingredient. verl has an OPD implementation.
- **Why us:** we already have teacher (14b), student (8b), and a verifier. OPD is the principled fix
  for the looping/bad-edit ceiling SFT can't cross. **HIGH ROI, medium effort.**

### 2. RLVR via GRPO on our check.py reward — and it FITS our GPU
We already have a **verifiable reward**: repo-fix `check.py` exits 0/1. That's the ideal RLVR signal
(pass/fail unit tests are the canonical example).
- GRPO is the de-facto RLVR algorithm (DeepSeek-R1). **Unsloth GRPO+QLoRA runs in ~5GB VRAM**, 8B in
  24GB at long context; rule of thumb model-params ≈ VRAM for 4-bit → **8B GRPO is plausible on our
  12GB 3060** at modest context. We already use Unsloth.
- Honest caveat (Promptfoo): "RLVR makes models *faster, not smarter*" — it sharpens existing
  capability and collapses the sampling distribution; it won't conjure skills the base lacks. So pair
  it with distillation (which adds the teacher's skills), don't replace.
- **Why us:** a second, verifier-driven weights lever beyond SFT — and the gate already exists.
  **HIGH ROI, medium effort.** Likely sequence: distill (add skill) → GRPO (sharpen on verifier).

## Tier 2 — strengthens the self-improvement loop design

### 3. Huxley-Gödel Machine (HGM) — pick self-mods by *metaproductivity*
Successor to DGM (arXiv 2510.21614). Identifies a **Metaproductivity–Performance Mismatch**: an
agent's *own* benchmark score is a poor predictor of how good its *descendants* will be. HGM scores a
candidate by the aggregated performance of its descendants. **Why us:** our gate currently accepts a
self-mod on its immediate pass@1 delta — HGM says that's myopic. Worth folding into the harness-lever
selection. **MED ROI, low effort (it's a selection-policy idea, not new infra).**

### 4. Procedural-memory learning from experience (ProcMEM, non-parametric PPO; arXiv 2602.01869)
Learn **reusable executable skills** from interaction and reuse them — exactly our procedural-memory
layer, which is currently fragmented/partly-dead. **Why us:** turns our memory from recall into the
improvement engine (the product thesis). **MED ROI, medium effort.**

## Tier 3 — harder benchmarks / breadth (after capability rises)

### 5. SWE-EVO (arXiv 2512.18470) — long-horizon software *evolution* benchmark
Beyond single-bug SWE-bench: multi-step feature evolution over a repo. A harder, more realistic target
once repo-fix is solved — and a better showcase for self-improvement. **Track, don't adopt yet.**

### 6. Group-Evolving Agents (2602.04837) — open-ended self-improvement via experience *sharing*
Multiple agents share experience to improve open-endedly. Interesting long-term, but we're driving one
vertical deep, not breadth. **Park it.**

## Recommendation
The two Tier-1 items are the real unlock and both fit our hardware + existing verifier:
1. **OPD** to fix the distilled model's on-policy drift (the thing SFT couldn't).
2. **GRPO/RLVR** on check.py as a second weights lever (sharpen, after distill adds skills).
Both ride the Phase-2 native tool-calling format we just built (clean trajectories = clean RL/OPD
data). Sequence after the native-vs-text compare lands: native-format trace collection → OPD/SFT →
GRPO → gate. Everything stays on the trustworthy ruler.
