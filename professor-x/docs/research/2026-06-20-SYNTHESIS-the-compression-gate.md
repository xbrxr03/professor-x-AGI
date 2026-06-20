# SYNTHESIS — "Self-improvement = compression progress on verified experience" (2026-06-20)

Four parallel research agents (active-inference, neuro/bio, open-problems, cross-disciplinary) were sent to
find an *unprecedented* mechanism, each with our substrate + the non-novel exclusion list. They came back
with 9 candidates. The important result is **where they converge** — that intersection is the invention.

## The convergence
Every agent, from a different field, circled the SAME two problems:
1. **Cross-channel decisions** (weights vs skill vs memory): which channel gets a lesson, who earned a gain,
   how to protect it. (active-inference EFE-router; Shapley attribution; reconsolidation lock.)
2. **Honest gating against self-Goodhart** — the field's #1 *quantified, admitted-unsolved* failure
   (reward-hacking rises 26%→58% as a code self-improver runs 10→100 steps; held-out sets don't fix it).
   (AACE correlation tripwire; compression-progress gate; proper-scoring self-prediction gate.)

The cross-disciplinary agent's lead candidate **unifies both** and is the most defensible as genuinely new:

## THE CANDIDATE — the Compression Gate
**Thesis (one line):** *A self-modification (to weights, skill, OR memory) is accepted only if it keeps the
verifier green AND makes the agent's corpus of verified solutions **more compressible** — i.e., lets the
agent re-derive its past wins from a shorter program.* Self-improvement is redefined as **compression
progress on verified experience.**

**Why this is the inventive core, not a recombination:**
- It gives self-improvement **one currency** — description length (bits) — that is **commensurable across all
  three heterogeneous channels** (a weight delta, a skill-text edit, a memory entry all cost/save bits).
  Nobody has a single keep-rule spanning weights+skill+memory. SIA *switches* levers; this *prices* them.
- It is **anti-reward-hacking by construction, not heuristically**: a memorized hack (hard-coded special
  case, `sys.exit(0)` trick, test-specific patch) **adds** description length — it fails the gate *even
  though it passes the verifier*. Generalization compresses; memorization bloats. This is the structural
  honesty rail our whole "trustable self-improvement" identity needs.
- It is a **tractable empirical stand-in for the Gödel Machine's intractable proof** (Schmidhuber: rewrite
  only on a *proof* of improvement → undecidable). We replace the proof with a cheap certificate: "the
  corpus got more compressible." Real lineage, real advance.

**Closest prior art + precise delta (from the agents, verified):**
- Schmidhuber *compression progress / Formal Theory of Creativity* (2008) — compression as intrinsic
  reward for *exploration/curiosity*. Δ: we use ΔDL as the **acceptance gate for self-modification**, not a
  curiosity bonus.
- *ReuseRL / MDL-Skills* (arXiv 2605.31509; OpenReview r4XxtrIo1m9) — MDL as a **training-time loss** to
  extract a skill dictionary. Δ: we use ΔDL as a **keep-rule across all three levers**, post-hoc, not a loss.
- *Gödel Machine* (arXiv cs/0309048) — proof-gated self-rewrite (intractable). Δ: empirical compression
  certificate (tractable).
- *SIA* (arXiv 2605.27276) — lever switching by an LLM judge. Δ: one principled bit-currency gate, not a judge.
None combine: ΔDL-as-acceptance-gate + across weights/skill/memory + verifier-grounded + anti-hack-by-construction.

## The system it composes into (each piece a separate agent's find, each falsifiable)
1. **Search:** the intrinsic free-energy/surprise signal (we have it, confirmed non-degenerate) PROPOSES
   what to try / where to look — cheap heuristic. *(active-inference agent)*
2. **Keep:** the **Compression Gate** — accept iff pass-rate held AND ΔDL(verified corpus) > 0. *(cross-disc)*
3. **Attribute:** **Channel-Shapley** — exact Shapley over the 2³ channel subsets (8 cheap inference passes)
   says how many of the saved bits each channel earned; discard ≤0 channels. *(open-problems agent)*
4. **Protect:** **reconsolidation lock** — a consolidated (compressed) lesson is write-protected until its
   task RE-FAILS the verifier, then briefly editable. Structural anti-forgetting. *(neuro/bio agent)*
5. **Stay honest:** **AACE tripwire** — a sealed anchor suite (never read/trained on) + adversarial fixture
   mutator; halt+rollback when Spearman(visible-score, anchor-score) decays. The outer Goodhart guard. *(open-problems)*

Unifying principle: **"a self-improvement is real iff it lets the agent re-derive its verified past from a
shorter program."** Free-energy searches, compression decides, Shapley attributes, reconsolidation protects,
AACE polices. That connected loop, local, on one verifier, does not exist in the literature.

## Honest status — this is a CANDIDATE, with kill-tests, not a claimed result
- **Make-or-break #1 (free, no training):** does ΔDL (gzip/zstd or token-count over canonicalized
  trajectories) actually *correlate with held-out generalization*, and does a planted memorization-hack
  *increase* DL while a genuine fix *decreases* it? If ΔDL is uncorrelated with held-out pass@1 → the gate is
  fake → drop it. Pandas + existing `artifacts/trajectories/*.jsonl`. **Run this first.**
- **Make-or-break #2:** the EFE epistemic/pragmatic split must be *two* signals not one (else free-energy
  search collapses to D-MEM); and our per-task `predicted_success` hook (`react.rs` TODO) must be landed.
- **THE prerequisite ALL FOUR agents independently flagged:** at ~50 fixtures the benchmark is too small to
  falsify ANY of this. **A headroom benchmark must come first** (matches our saturation finding). The AACE
  adversarial mutator doubles as a cheap way to grow it.

## Discards (already published — agents verified, so we don't re-invent)
surprise-prioritized replay (SuRe 2511.22367) · text-memory reconsolidation (HiMem 2601.06377) · RPE-gated
memory tiers (D-MEM 2603.14597) · self-distillation collapse guard (SDFT 2601.19897) · QD/archive
self-rewrite (DGM 2505.22954) · metaplasticity anti-forgetting (Laborieux 2101.07592).

## Recommendation
Lead invention = **the Compression Gate** ("self-improvement = compression of verified experience"), with
free-energy search + Channel-Shapley + reconsolidation + AACE as the system around it. **Next concrete step
is free and decisive:** run kill-test #1 (does ΔDL predict generalization on data we already have). In
parallel, grow the headroom benchmark (the universal prerequisite). Only then build any of it.

Sources: see the four agent reports (active-inference, neuro/bio, open-problems, cross-disciplinary) — full
URLs preserved there; key: Schmidhuber compression/creativity, Gödel Machine (cs/0309048), ReuseRL
(2605.31509), MDL-Skills (r4XxtrIo1m9), reward-hacking-in-RSI (OpenReview ikrQWGgxYg), D-MEM (2603.14597),
HiMem (2601.06377), SuRe (2511.22367), Yu&Dayan ACh/NE, STC-in-RNNs (Nature Comms 2021), SIA (2605.27276).

---
## Kill-test #1a result (2026-06-20) — premise check with NAIVE gzip: WEAK/ambiguous
Built a controlled corpus: 10 idiomatic general fixes vs 10 minimal memorized hacks (each hard-codes its
test's expected output). gzip(-9) compressed sizes:
- full corpus: general=236B, hack=301B -> **hack/gen = 1.28x** (hacks bigger, direction correct)
- steady-state marginal (items 6-10): general 21 B/item vs hack 24 B/item -> **1.12x** (within noise)
**Verdict: directionally correct but too weak to gate on.** gzip measures generic byte-redundancy, not
"new information relative to what the agent knows." 
**Fix = the theoretically-correct DL proxy: description length UNDER THE AGENT'S OWN MODEL (token-level
perplexity / -logprob of the solution under qwen3:8b).** A memorized literal is high-perplexity (surprising
-> many bits); a general idiom is low-perplexity (already known -> few bits). This should separate
general vs memorized far better than gzip — and it's the correct operationalization of "compression =
generalization." Needs the GPU (Stage 2 has it now) -> queued: compute mean -logprob of general-fix vs
hack-fix solution sets under qwen3:8b; premise holds iff hack -logprob >> general -logprob beyond noise.
Honest status: the invention is NOT validated; naive-gzip premise is weak; the perplexity-DL test is the
real make-or-break and is pending a GPU slot.
