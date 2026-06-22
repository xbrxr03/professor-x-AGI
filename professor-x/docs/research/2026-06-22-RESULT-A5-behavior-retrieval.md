# RESULT: A5 — behavior-keyed retrieval ON vs OFF (2026-06-22)

Stream A of Phase 1 (Claude). Applied skills: verify-the-ruler, adversarial-self-review,
diagnose-from-trajectory.

## Setup
14 held-out RENAMED anchors (NOT in the signature index), qwen3:8b-q4_K_M, native tools, K=1.
`PROFESSOR_X_BEHAVIOR_RETRIEVAL` OFF vs ON. Index = 34 family tasks (signature + fix hint).

## Result (trustworthy — after fixing the ruler)
| mode | pass@1 | behavior_hits |
|---|---|---|
| OFF | 0.286 (4/14) | 0 |
| ON  | 0.357 (5/14) | **14/14** |

- **Mechanism: VALIDATED.** The hint fired on every anchor; each renamed anchor matched its exact
  origin family task at similarity 1.00 (e.g. fam_csv_anchor_1→fam_csv_01). Rename-invariant behavioral
  retrieval works end-to-end in the live harness.
- **Impact: NOT a confirmed win.** +1 task (0.286→0.357) is within noise (±1/14 = ±0.07; the prior
  run's OFF was itself 0.357). Per the pre-registered gate (ON must beat OFF by ≥MDE), this does NOT
  clearly clear MDE.

## The louder signal + trajectory diagnostic
The model failed **9/14 anchors even with the correct fix handed to it as a hint.** Inspected a failing
case (`fam_csv_anchor_2`): the model used the CORRECT renamed vocabulary (`split_cells`/`pluck`, so NOT
the vocabulary-mismatch I'd hypothesized) but produced a WRONG edit — duplicated lines after a `return`
(dead code) instead of the targeted `lines → lines[1:]` fix. So:
- The failure is **wrong-edit production**, not missing information and not vocabulary confusion.
- This is exactly Codex's dominant bucket (wrong-edit-verified-fail, 60-80%).

## Honest verdict
The behavior-signature REPRESENTATION is validated (rename-invariant retrieval, 14/14 correct matches).
Its retrieval-USE gives **at-best-marginal lift** on a weak local model, because the bottleneck is the
model's ability to PRODUCE a correct edit, not its access to the right fix — handing it the answer barely
helps. Per the gate: keep the representation, **shelve the retrieval-use as marginal**, and say so.

## Strategic implication (drives the agenda)
This + the taxonomy together: ALL harness mechanisms (edit-lever, anti-thrash, verifier-feedback retry)
exist, AND even perfect information barely helps → the real lever is **model CAPABILITY**, i.e.
Phase 3 / Lever 1 (the distillation flywheel, un-paused 2026-06-22). Information-side tuning (abstract
hints, etc.) has low expected value given handing the model the answer didn't move it.

## What stays / what's next
- KEEP: fault_signature module + SignatureIndex (validated, flag-gated OFF, full suite green) — reusable
  for Diagnostic Verifier Codes (Phase 4) and as a contamination-proof eval primitive.
- SHELVE: behavior-keyed retrieval as a pass@1 lever (marginal; revisit if a stronger student makes the
  edit-production reliable enough to exploit hints).
- PIVOT: Phase 3 — drive the distillation flywheel against the wrong-edit-production ceiling.
