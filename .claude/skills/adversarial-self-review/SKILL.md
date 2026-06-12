---
name: adversarial-self-review
description: "Two-thread adversarial review of your OWN work before you commit it on Professor X: steelman it, then attack it. Use before any commit or PR, before recording a result in docs/memory, before declaring a milestone done, or when you're about to claim a 'win' or improvement. Adapted from ARIS's kill-argument; the same pattern lives in Prof X's agent.critic."
allowed-tools: Bash(*), Read, Grep, Glob
---

# Adversarial self-review (kill-argument, applied to your own work)

Balanced self-review under-commits — it lists weaknesses without deciding which is fatal. An
adversarial pass MUST commit: find the one flaw that, if a sharp reviewer saw it, sinks the
change. This is how this project avoids fabricated results and silent regressions.

## The two threads

**DEFENSE (steelman):** In 2-3 sentences, make the strongest good-faith case the change is
correct, complete, and an improvement. Cite the specific evidence (test output, measured delta,
diff).

**PROSECUTION (attack):** Now attack that case. Find the single strongest reason it is wrong,
incomplete, unmeasured, a variance artifact, or breaks something else. Specifically check:
- Did I **measure** the claim, or am I asserting it? (See `verify-the-ruler`.) Is the "win"
  inside the noise band? Is it ONE run of a stochastic metric?
- Did I read the **actual trajectory/failure**, or guess? (See `diagnose-from-trajectory`.)
- Does a test pin the OLD behavior I just changed? Did I run the FULL suite (`cargo test --bins`),
  not a filtered subset?
- Am I **overselling** (the user notices and pushes back) or claiming "done" what isn't?
- Am I piling speculative code on an unvalidated change instead of measuring first?

## Verdict
State the fatal flaw if one exists and fix it before committing; otherwise commit with the honest
caveat named. Report outcomes faithfully — if a result is unverified, say so; if a fix was
neutral/within noise, say that, don't dress it as a win.
