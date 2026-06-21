# Reuse-family benchmark construction recipe (deep-research, 2026-06-21)

Keystone deliverable: a benchmark with TASK FAMILIES (tasks sharing a small library/API so solutions
transfer) + realistic multi-line fixes. Unblocks VGTS embedding, Re-Verified RAG, VCA credit-assignment, and
gives AACE/OPD headroom. Current benchmark: 0.1% pair-transfer (confirmed by repo inspection — every module
~unique per task). Seed already exists: `hard_001` (multi-file pricing/discount/tax/order import graph).

## Recipe (per the deep-research note; buildable 3-5 days local)
1. Pick ~7 domains, each a **small internal API of 3-6 functions that call each other** (bug in a helper
   propagates). Generalize `hard_001`. (money/pricing ✓, intervals, stack/queue ADT, CSV records, unit
   conversion, mini state machine, graph adjacency.)
2. Write ONE correct shared library per family + a **metamorphic/property `check.py`** (round-trip,
   monotonicity, invariants — NOT a single equality, so degenerate fixes can't pass). stdlib only.
3. Generate 8-15 tasks per family by **injecting realistic bugs INTO THE SHARED HELPERS** (not isolated
   stubs) — different bug location per task → multi-line, transferable fixes; siblings share the API.
4. **Mandatory red→green validation** (reuse `add-repofix-fixture` gate: buggy=1, fixed=0). No variant ships
   without it (M0).
5. **ZPD filter**: run qwen3:8b pass@k per variant; keep 0 < pass@1 < 1 (in-band → headroom + MDE).
6. **Sealed-anchor (AACE) split**: anchor tasks get EvoEval-style renaming/restructuring + hidden asserts;
   library is hand-written/private → low contamination. A fix transferring to a *renamed* anchor sibling =
   real generalization, not leakage/operator-matching.
7. Manifest `tasks_families.json` with family_id, shared_api, n_changed_lines, zpd_band, split.

## Acceptance gates (falsifiable)
- transfer: within-family sibling patches share ≥40% token-overlap on shared-API lines (vs 0.1% now).
- multi-line: median reference patch ≥3 changed lines.

## Devil's advocate (the key risk) + the built-in detector
Toy families risk "transfer = recognize the same mutation operator" (trivial pattern-match). The
**sealed/renamed anchor (Step 6) is the detector**: an agent that only matched the operator fails the
renamed anchor; only one that modeled the library's behavior transfers. Stepping-stone, NOT a substitute for
real-repo (SWE-Gym/R2E-Gym) validation later.

## Sources (graded A): SWE-bench 2310.06770; SWE-bench Verified (OpenAI); SWE-Gym 2412.21139; SWE-rebench
2602.23866; R2E-Gym 2512.12216; EvoEval 2403.19114 (anti-contamination transforms, -39.4%); mutation testing
1807.03512; metamorphic 2406.05397; ZPD/ZONE (OpenReview) + Proximal Curriculum 2405.02481; Exercism
problem-specifications. Contamination: 2406.04244, 2407.07565. NOT-found (flagged): a ready-made SPL coding benchmark.
