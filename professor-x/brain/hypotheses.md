# Professor X — Research Hypotheses

Hypotheses are numbered H1–H20 per ARCHITECTURE.md Section 12.
Status: Untested | Testing | Confirmed | Rejected

---

## MHE Lever Effectiveness

**H1** — MHE collectively raises HIRO pass@3 above the null-condition baseline by round 30.
Status: Untested | Priority: Critical

**H2** — Lever 2 (ICE+MARS contextual) is responsible for the majority of gains in rounds 1–10.
Status: Untested | Priority: High

**H3** — Lever 3 (DHE structural evolution) produces lasting improvements not achievable by Lever 2 alone.
Status: Untested | Priority: High

**H4** — Lever 1 (SDAR QLoRA) provides additive gain only when layers 1–4 are already saturated.
Status: Untested | Priority: Medium

---

## Behavioral Fingerprint

**H5** — BF non-uniform improvement (gap >= 0.15 between categories) appears within 5 rounds of a harness change.
Status: Untested | Priority: High

**H6** — p_tool is the easiest category to improve; p_correct is the hardest.
Status: Untested | Priority: Medium

---

## ICE and MARS

**H7** — ICE injection (top-3 similar past tasks) improves pass@3 by >=5pp on tool_use tasks vs no-ICE baseline.
Status: Untested | Priority: High

**H8** — MARS reflection reduces repeat failures on the same task class within 10 rounds.
Status: Untested | Priority: High

**H9** — ICE + MARS together outperform either technique alone by >=3pp on self_correction tasks.
Status: Untested | Priority: Medium

---

## DHE Attribution

**H10** — DHE achieves >=60% fix-prediction precision vs AHE baseline of 33.7%.
Status: Untested | Priority: Critical (paper claim)

**H11** — Layer 4 (tool execution) failures are the dominant failure mode in rounds 0–5.
Status: Untested | Priority: High

**H12** — DHE Layer 5 (reasoning loop) failures increase after round 10 as tool issues are resolved.
Status: Untested | Priority: Medium

---

## LCAP (round 10+)

**H13** — LCAP UCB1 bandit converges to non-Balanced arm within 5 rounds of activation.
Status: Untested | Priority: Medium

**H14** — LCAP provides >=3pp gain over static Balanced allocation by round 20.
Status: Untested | Priority: High (paper claim)

---

## Reflexion and Voyager

**H15** — Reflexion buffer (max 3) reduces per-task step count by >=20% on second attempt vs first.
Status: Untested | Priority: Medium

**H16** — 4-attempt Voyager limit is rarely reached; >90% of successful tasks complete within 2 attempts.
Status: Untested | Priority: Low

---

## System-level

**H17** — HIRO score HIRO(30) = (P_30 - P_0)/30 >= 0.01 (>=1pp/round average improvement).
Status: Untested | Priority: Critical (paper claim)

**H18** — The harness evolution artifact (evolved components after 30 rounds) transfers >=70% relative gain to a fresh qwen3:8b instance.
Status: Untested | Priority: High (transferability claim)

**H19** — Merkle-chained audit log verification adds <5ms overhead per startup.
Status: Untested | Priority: Low

**H20** — Evolution cycles that fail (Analyzer rejects) still provide signal: rejected nodes have better-than-random next proposals.
Status: Untested | Priority: Medium
