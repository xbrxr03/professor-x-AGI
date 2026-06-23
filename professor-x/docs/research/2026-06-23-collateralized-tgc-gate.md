# Collateralized TGC — the buildable nugget from Codex's CGW (2026-06-23)

Extraction from Codex's `cognitive-prime-brokerage.md` (Collateralized Global Workspace / Cognitive
Prime Brokerage). Most of that work is the FROZEN consciousness/AGI long-arc (CLT/AFGW — parked in
PROJECT_ATLAS §5). This doc pulls out the ONE piece that lands inside the north star and is testable
now: **generalize the TGC gate from a scalar accept into a collateralized self-modification gate.**

## The convergence (why this is worth taking)
Codex independently re-derived our moat: in CGW terms, **TGC = out-of-sample PnL validation, ICS =
identity ledger, the held-out gate = collateral on a self-modification.** Two agents arriving
separately at "self-modification must be collateralized by held-out evidence" is corroboration that
the trust-gate is the defensible direction. The CGW lens then says our current gate is *under-priced*:
it only checks aggregate held-out pass@1, which can hide drawdown.

## The gap in the current gate (concrete)
`tgc_gate.py` accepts iff `mean(held-out pass@1)` rises by ≥ MDE AND the train−heldout gap ≤ bound. A
**scalar mean hides per-task drawdown**: a candidate that gains +3 anchors and silently breaks −3
others nets ~0 and looks neutral — but it has traded capability we already had for new capability, a
hidden regression. The field's exact worry (a self-mod that games the aggregate). Today's p3 reject
was uniform-worse so the scalar sufficed, but a *near-miss* candidate could pass the scalar while
regressing tasks — and we'd never see it.

## Collateralized TGC (the upgrade) — accept a self-mod only if it clears ALL of:
1. **Realized PnL (TGC, kept):** held-out renamed-anchor mean pass@1 delta ≥ MDE.
2. **No-drawdown / collateral (NEW):** per-task, the candidate must not REGRESS any held-out anchor it
   previously passed beyond a tolerance (e.g. ≤ 1 anchor flip, or net-regression = 0). Wins can't be
   paid for by hidden losses. (Cheap: per-anchor pass/fail before/after is already in bench artifacts.)
3. **No-arbitrage / consistency (NEW, cheap with our asset):** the improvement must be *behaviorally
   consistent* — using the DVC syndrome, does the candidate fix the SAME check-classes on train and
   held-out, or different ones? Train gains that decode to different syndromes than held-out gains =
   surface-matching, not generalization. (We have decomposable syndromes; this is the rename-invariant
   "internal consistency" check CGW asks for, made concrete.)
4. **Identity exposure (exists):** ICS gate for harness self-mods (already in the harness).

## Why this is north-star-aligned (not the frozen lane)
It's strictly a **better self-improvement governance gate** for the local coding agent — the headline
moat (verifier-as-code + Goodhart-proof gate). It does NOT require the CGW/CLT machine-consciousness
program; it just prices our existing self-mod acceptance with 2 cheap extra factors we can already
measure. It also sharpens the M3.2 TGC claim (the Goodhart-gap demonstration) with per-task evidence.

## Falsifiable test (cheap, decisive)
On the bench artifacts we already have (p3 + future candidates): does the **per-task / syndrome view
ever DISAGREE with the scalar gate** — i.e., catch a candidate the mean would pass, or flag drawdown a
mean hides? 
- WIN: there exist candidates where collateralized-TGC rejects but scalar-TGC accepts (or vice versa)
  → the extra factors carry real signal → fold them into `tgc_gate.py`.
- KILL: at our 14-anchor scale the per-task and scalar verdicts ALWAYS agree → it's overhead → keep the
  scalar gate, shelve the extra factors until the anchor set is larger. (Honest: with only 14 anchors,
  per-task resolution is coarse; this may land KILL until we grow anchors — say so if it does.)

## Implementation (small, Claude-owned Stream-D)
`decide()` in `tgc_gate.py` already has the numbers; add: (a) accept a per-anchor pass/fail vector for
base & candidate, (b) compute `regressions = #anchors base-passed & cand-failed`, reject if
`regressions > tol`; (c) optional syndrome-consistency when the decomposed checks are wired. ~30 lines,
unit-testable (extend the existing `--self-test` with a "gains-mask-drawdown" case the scalar passes
but collateral rejects).

## Status — BUILT (2026-06-23), self-test PASS
Wired into `tgc_gate.py`: `decide()` now takes optional aligned per-anchor pass vectors + `--drawdown-tol`
and adds factor (3) no-drawdown collateral; `bench_vec()` reads per-task `passed` from the repo-fix
artifacts so the live gate computes collateral; backward-compatible (no vectors → scalar gate).
`--self-test` PASS (8 cases) — incl. the decisive pair: a gains-mask-drawdown candidate that the
**scalar gate ACCEPTS but the collateral gate REJECTS** (2 per-anchor regressions > tol 0). Factor (2)
syndrome no-arbitrage left as a documented future hook (needs decomposed anchor checks wired).
**Honest caveat still stands:** at 14 anchors per-task resolution is coarse; the real test is whether
collateral ever flips a *near-miss* candidate's verdict in practice — re-evaluate when anchors grow.
The rest of Codex's CGW/CLT stays PARKED in the Atlas long-arc. Novelty spot-check:
2026-06-23-codex-cgw-novelty-spotcheck.md.
