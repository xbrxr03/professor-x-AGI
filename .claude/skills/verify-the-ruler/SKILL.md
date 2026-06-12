---
name: verify-the-ruler
description: "Mechanism-check a metric before trusting it on Professor X. Use when about to report/record/act on a pass-rate, pass@1, pass@3, φ, or any score; when a number jumps or looks too good; before writing a result to docs/research or memory; before calling a milestone done; or when a metric reads a suspicious constant (0.000, always-same). This project has shipped fabricated 'confirmed' metrics — M0 exists to stop that."
allowed-tools: Bash(*), Read, Grep, Glob
---

# Verify the ruler before trusting the measurement

The single most expensive failure mode on Professor X is treating a number as truth without
checking the instrument. This session caught **two mirages** that would have been recorded as
results:

1. **LLM-judge `pass@3 = 0.733`** — inflated by a qwen3:8b judge crediting wrong/hallucinated
   answers (it had swung from too-harsh false-negatives to too-lenient false-positives).
2. **repo-fix `pass@1 = 0/4`** — a broken benchmark: `pytest` wasn't installed, so every test
   errored regardless of the code.

## The discipline

Before reporting / recording / acting on a metric:

1. **Prove the mechanism end-to-end on a known case.** Apply the *correct* answer/fix by hand
   and confirm the metric flips (red→green, fail→pass). If a known-good input doesn't score as
   good, the ruler is broken — fix it before measuring anything else.
2. **Prefer deterministic, ungameable signals.** A test exit code (repo-fix) can't be inflated
   by a lenient judge. An LLM-judge on a weak local model is unstable in BOTH directions — do
   not trust it for headline numbers.
3. **A number that didn't move may mean the fix worked and exposed the next bottleneck.** Read
   the trajectory (see `diagnose-from-trajectory`) before concluding the fix failed.
4. **Hand-label a sample.** For any judge, check agreement with a human on ~15 cases; below
   ~90% it is not trustworthy.
5. **Watch for degenerate metrics** (always 0, always the same) — usually a sampling/wiring bug
   (e.g. `pass/0`), not a real signal.

## Output
State the metric only with its provenance: how it was measured, why the ruler is trusted, and
the honest caveat (variance, n, judge limits). Never write a "confirmed" result you haven't
mechanism-checked. See `docs/research/eval-trust.md`.
