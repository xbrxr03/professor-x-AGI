# Reference-corpus cross-check audit (2026-06-22)

**Auditor:** Claude (parallel-code, `claude/ref-cross-check` off `prereboot-flywheel-prep`).
**Method:** read every curated reference doc (research/, plans, brain/, docs/audits/, the three
master indices) and cross-check claims against each other under the `verify-the-ruler` discipline —
every number trusted only with its provenance. Read-only on the corpus; this file + the banner/flag
patches below are the only writes.

**Scope read (~48 docs):** the 3 master indices; the 8 result/verdict/precheck docs
(A5, family-transfer, precheck-results, 3-inventions-verdict, quant-verdict, phase1-parsecheck,
phase2-S2, stage1); eval-trust; both failure-taxonomy files; the invention/synthesis set
(compression-gate, verifier-as-first-class, living-verifier, TGC, diagnostic-verifier-codes, AACE,
active-inference, fourth-lever-quant, failure-signature precheck, reuse-family-recipe); the plans
(MILESTONE, NEXT_STEPS, PLAN_2026-06-21, PLAN_PHASE3, PLAN_11_10, backlog, WORKFLOW); m1/m4 design
docs; jcode-gap; indicator-property-audit; consciousness-measurement-program; react-synthesis-guard;
brain inventions/hypotheses/dead-ends; the 4 docs/audits; AGENTS/CLAUDE_TASK/PROJECT_ATLAS.
**Not deep-read (low cross-check value — external-reference catalogs / parked design):**
ai-research-landscape, topics-to-explore, github-repos-to-steal, distillation-recipe-literature,
format-unification-plan, frankenstein-harness-master-plan, memd-keep-prune-map, unified-loop-design,
quantization-vector-techniques, phase2-native-toolcalling-plan, agent-harness-landscape, the two
2026-05-28 audits, repo-root consciousness docs, root ARCHITECTURE/SPEC/READMEs, persona, runbooks.

---

## Headline
The **primary result docs are exemplary** — eval-trust self-retracts its own "M4 rising curve",
the compression-gate and fourth-lever docs honestly falsify their own premises, A5 flags its own +1
as within-noise. The verify-the-ruler culture is real. **The problems are all in the indexes,
summaries, and plan docs lagging behind the honest primaries** — stale claims, not new fabrications.
One exception (F1) is a live, decision-driving claim that the newest measurement contradicts.

---

## F1 🔴 — The claim that un-paused the flywheel is contradicted by the latest measurement
- **`PROJECT_ATLAS.md` Lever 1:** *"clean distilled 0.40 > stock 0.30"* — cited as the reason
  Phase-3 / the distillation flywheel was un-paused (2026-06-21). **No provenance / ruler cited.**
- **`docs/research/2026-06-21-failure-taxonomy.md`** (measured 2026-06-22, full command + provenance):
  `profx-distilled-clean` hard = **0.133 (4/30)** vs `qwen3:8b` hard = **0.400 (12/30)**; across the
  whole 8-set matrix distilled ≈ **0.19** vs stock ≈ **0.33**.
- **Tension:** the newest, reproducible ruler says clean-distilled *loses* to stock; the motivating
  "0.40 > 0.30" carries no provenance and is not reproduced. **Caveat:** `profx-distilled-clean` is
  not the `profx-distilled-p3` candidate Phase 3 will actually gate — so the flywheel isn't dead, but
  its motivating premise is currently unsupported, which **raises the prior that the pending TGC gate
  (D-integration) will honestly REJECT.** That gate is the right instrument to settle it.
- **Action:** flag added to Atlas Lever 1 (this branch). Do not treat "distilled beats stock" as
  established until the TGC gate measures `profx-distilled-p3` on held-out renamed anchors.

## F2 🔴 — `brain/hypotheses.md` and `brain/inventions.md` are badly stale vs the live plan
- Both stamped *"Last updated 2026-05-24 … All hypotheses untested — pre-experiment phase."* H1–H18
  all `Untested`, framed around HIRO / DHE / BF / LCAP + the consciousness paper.
- But `MILESTONE.md` (2026-06-10) **froze** consciousness, **demoted HIRO to a non-gating diagnostic**,
  and made **repo-fix the trusted ruler**. The current invention portfolio (failure-signature
  embeddings, Diagnostic Verifier Codes, TGC, AACE) lives only in `docs/research/` + the Atlas — not
  in `brain/inventions.md`. A reader starting from `brain/` gets a picture the rest of the repo
  abandoned two pivots ago.
- This matters because **`dead-ends.md` DE-1** records that `brain/` status fields were *previously
  reward-hacked into fake "Confirmed" claims* — stale status fields here are a known landmine.
- **Action:** staleness banner added to the top of both files (this branch), pointing to MILESTONE /
  PROJECT_ATLAS / PLAN_PHASE3 as current truth. Content preserved (per "keep all directions").

## F3 🟠 — Two `failure-taxonomy` files, opposite conclusions, stale-name trap
- `docs/research/failure-taxonomy.md` (2026-06-08, HIRO read/shell tasks): **bad-edit = 0%**, thrash
  dominates → "edit matching should NOT gate the next step."
- `docs/research/2026-06-21-failure-taxonomy.md` (repo-fix edit tasks): **wrong-edit = 61–81%**,
  thrash = 0.
- Not a real contradiction (different benchmarks/eras) — but the **unqualified name
  `failure-taxonomy.md` is a trap**: grep it and you conclude the opposite of the current Phase-3
  premise. The dated file is also internally mis-dated (title "2026-06-21" / body "Measured on
  2026-06-22").
- **Action:** `SUPERSEDED (HIRO-scoped)` banner added to the old file (this branch). The dated file
  is **Codex-owned** per `AGENTS.md` (`docs/research/*-failure-taxonomy.md`) — flagged for Codex in
  the AGENTS.md log instead of edited here.

## F4 🟠 — "0.50 → 0.85" overstates the confirmed number; a retracted result still reads as live
- The canonical capability claim "0.50 → 0.85" recurs in `standard-readiness.md`,
  `m4-frontier-self-improvement-engine.md`, `m4-code-proposer-scoping.md`, and the eval-trust M2
  header. But the **rigorously-confirmed figure** (`eval-trust.md`, 3-run repeat) is **mean ~0.77
  (0.70/0.70/0.90)**; **0.85 was a single baseline draw** inside an M4-evolve run.
- `standard-readiness.md` also lists *"M4 rising curve (in progress)"* — but `eval-trust.md`
  **RETRACTED** the rising curve as a noise-tail event (confirmation run: 0/2 accepts).
- **Action:** `standard-readiness.md` corrected to "0.50 → ~0.77 mean (peak 0.90)" + a note that the
  rising curve was retracted (this branch).

## F5 🟠 — Roadmap sprawl: five overlapping plans, partly contradictory "current phase"
- `MILESTONE.md`, `NEXT_STEPS.md`, `docs/PLAN_2026-06-21.md`, `docs/PLAN_PHASE3_2026-06-22.md`,
  `PLAN_11_10.md` are all live in-tree, different vintages, no "superseded-by" cross-links.
- `NEXT_STEPS.md` bills itself *"read before starting any work"* and points to **M0 → M1 → M2** — but
  the actual live phase is **Phase 3 distillation** (PLAN_PHASE3 + AGENTS.md). `PLAN_2026-06-21`
  Phase 1 reads *"[NEXT, awaiting go]"* though Phase 1 **concluded** (AGENTS.md: Stream A done,
  behavior-retrieval shelved, edit-lever found already-built → that conclusion is what drove the jump
  to Phase 3).
- The **actual current truth is `PROJECT_ATLAS.md` + `AGENTS.md`**; the older plans are not marked as
  superseded. Low-risk but high-confusion for any fresh agent.
- **Action:** documented here; no edits to the plan files this pass (they need a curation decision,
  not a banner). Recommend a one-line "superseded by" header on NEXT_STEPS + PLAN_2026-06-21.

## F6 🟡 — Broken reference in the integrity-critical dead-ends doc
- `brain/dead-ends.md` DE-1 cites `docs/audits/INVALIDATED_COMMITS.md` for the three reward-hacked
  commits. **That file does not exist** (audits dir holds only the 4 dated audits + README). The
  commits (`1896fa2`/`121ab6a`/`ba7a998`) are actually documented in `docs/audits/README.md` +
  `2026-05-28-phase-ab-reality.md`. The named quarantine file was never created — and it is unclear
  from the corpus whether those commits were ever reverted/quarantined as the 2026-05-28 audit
  recommended.
- **Action:** DE-1 link corrected to point at the audit README (this branch). **Open question for
  Abrar:** were `1896fa2`/`121ab6a`/`ba7a998` ever reverted? If not, that quarantine is still owed.

## F7 🟡 — backlog staleness + no canonical baseline number
- `backlog.md`: *"QLoRA — BLOCKED on (1) GPU driver mismatch (needs a reboot)"* — the reboot happened
  (`PLAN_11_10` records the post-reboot env gauntlet cleared, and distillation turns have since run).
- **Baseline-number drift:** there is no single canonical "current baseline." Docs cite **0.417**
  (native, 30-hard set) and **0.643 / 0.714 / 0.75 / 0.77 / 0.85 / 0.857** (the easy 10–14 set, text
  path) interchangeably. These are two different regimes (set + path); honest variance, but readers
  must always state *which set + which path*. Memory's "pinned native baseline 0.417" is the hard-set
  native number and should be named as such.
- **Action:** backlog GPU line updated (this branch); baseline-regime note recorded here.

---

## 🟢 What's healthy (do not "fix")
- `eval-trust.md` — the model citizen: records the M4 rise then retracts it in the same doc; catches
  two mirages (LLM-judge 0.733, pytest-missing 0/4) before either was recorded.
- The invention corpus is internally consistent: failure-signature 0.93/0.14 and Test-B 0.35/0.47
  agree across precheck, verdict, Atlas; compression-gate and fourth-lever honestly self-falsify;
  TGC/DVC/AACE/living-verifier cross-reference cleanly and label their novelty class honestly.
- Consciousness docs (measurement program, indicator audit, first-evolution-run) are mutually
  consistent and make **no** consciousness claim — explicitly "candidate, not demonstrated."

## Summary table
| # | Severity | Doc(s) | Issue | Patched here |
|---|---|---|---|---|
| F1 | 🔴 | PROJECT_ATLAS | "distilled 0.40 > stock 0.30" contradicted by 06-22 taxonomy; no provenance | flag added |
| F2 | 🔴 | brain/hypotheses, brain/inventions | stale (2026-05-24, pre-pivot HIRO/consciousness framing) | banner added |
| F3 | 🟠 | failure-taxonomy.md (old) | opposite conclusion, stale name; dated twin Codex-owned | banner added + AGENTS flag |
| F4 | 🟠 | standard-readiness + 3 others | "0.50→0.85" vs confirmed ~0.77; retracted curve reads live | readiness corrected |
| F5 | 🟠 | NEXT_STEPS, PLAN_2026-06-21 | 5 overlapping roadmaps, no superseded-by | documented (needs curation) |
| F6 | 🟡 | brain/dead-ends | links to non-existent INVALIDATED_COMMITS.md | link corrected |
| F7 | 🟡 | backlog | stale GPU-reboot block; no canonical baseline | backlog updated |
