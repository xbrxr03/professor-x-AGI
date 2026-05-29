# Paper Outline + Hypothesis Freshness Pass

**Date:** 2026-05-28
**Branch:** `audit/phase-ab-hiro-paper`
**Source of truth:** [`brain/paper_outline.md`](../../brain/paper_outline.md), [`brain/hypotheses.md`](../../brain/hypotheses.md)

## TL;DR

- The repo-root `brain/` is well-maintained, citation-rich, and substantially expanded since the operator's last memory snapshot: **18 hypotheses now (up from 13)**, with H14–H18 introducing the Identity-Preserving Evolution (IPE) axis.
- The paper has been retitled from "Metacognitive Harness Evolution" to **"Identity-Preserving Metacognitive Harness Evolution: A Self-Evolving Agent That Knows Itself."** This adds a fourth contribution dimension (identity coherence + functional affect + free energy) beyond the original three-lever framework.
- **Zero of the 18 hypotheses are currently testable end-to-end.** The mechanism modules (DHE, BF, LCAP, HIRO) exist in Rust but the IPE infrastructure (self-model, ICS, FED, affect, memory-as-tool, AI Idea Bench, GAIA evaluator) is unbuilt. More urgently: no HIRO round has ever run with persisted data, so even the modules that exist have not been exercised.
- The May 24 "Confirmed H1/H3" commits (see HIRO audit) are inconsistent with the canonical `brain/hypotheses.md`, which still lists every hypothesis as `Status: Untested` and was last updated 2026-05-24 — same day as the bad commits. The corruption hit only the nested `professor-x/brain/` copy.

## What changed in the paper plan since memory snapshot (2026-05-23 → 2026-05-28)

| Item | Before | After |
|---|---|---|
| Working title | "MHE" | **"IPE-MHE"** (Identity-Preserving Metacognitive Harness Evolution) |
| Hypotheses | 13 (H1–H13) | **18 (H1–H18)** — H14 (ICS), H15 (FED), H16 (negative affect → DHE), H17 (RQT/AI Idea Bench), H18 (GAIA L2 parity) added |
| Paper outline | inventions doc only | Full 9-section outline + abstract + appendix written. ~25 page target. |
| Headline experimental claim | DHE fix precision ≥60% (H10) | Now also: ICS ≥ 0.70 at round 30 (H14), GAIA L2 ≥ 40% at round 30 (H18) |
| New axis | n/a | **IPE — Strange Loop self-model + Free Energy Principle + functional affect (valence/arousal)** |
| Minimum viable paper | full trifecta | Sections 1, 2, 3, 5, 6, 7.2–7.3 only (DHE fix precision + fingerprint dataset) — explicitly named as fallback |

The IPE addition is non-trivial. It is a second-axis thesis (the first axis is "harness evolution is the lever"; the second axis is "the harness can evolve without losing identity"). This is a more ambitious paper than what my memory captured.

## Hypothesis testability matrix

For each hypothesis: what infrastructure does it require, what exists, what blocks it?

| ID | Topic | Required infra | Exists in `src/`? | HIRO data needed | Blocker |
|---|---|---|---|---|---|
| H1 | Memory injection threshold T* | HIRO + `--memory-budget` sweep | partial — HiroRunner present, no budget sweep flag | yes (30 tasks × 8 budgets) | no HIRO ever run; flag absent |
| H2 | Cerebellum bypass | Skill verification score + direct execution path | partial — skills loaded, verification score absent | yes (100 tasks × 2 conditions) | Phase E (skill runtime) unbuilt |
| H3 | Memory-as-tool vs passive inject | `memory.query()` tool exposed to LLM | **no** (no MemoryQueryTool in toolbridge) | yes (60 HIRO × 2 strategies) | tool needs implementing |
| H4 | Surprise-based episodic logging | Cosine-distance filter on episodic writes | no | yes (7-day baseline + replay) | filter not implemented |
| H5 | Autonomous vs human harness | 30-day comparison framework | no | yes (3 × HIRO(30)) | human-baseline protocol undesigned |
| H6 | Temporal compression | Nightly K-Means cluster job | no | yes (14-day run) | compression job not implemented |
| H7 | Self-distilled principles | MARS reflection + principle store | partial — `reflector.rs` exists, principle scoring absent | yes (20 task types × 10 failures each) | EvolveR-style scoring not implemented |
| H8 | Component attribution (auto) | HIRO + ChangeManifest.component | partial — proposer.rs exists | yes (30 rounds) | no HIRO data |
| H9 | Consumer HW parity | Model-endpoint swap in ollama.rs | no — `ollama.rs` is Ollama-specific | yes (2 × HIRO(20)) | endpoint abstraction needed |
| H10 | DHE fix-prediction ≥60% | DHE + HIRO rounds 1–30 | DHE module present | yes (30 rounds) | no HIRO data |
| H11 | BF non-uniformity | BF + HIRO rounds | BF module present | yes (30 rounds) | no HIRO data |
| H12 | LCAP vs static allocation | LCAP + T* from H1 | LCAP module present | yes (depends on H1) | H1 not run |
| H13 | MCA-IR correlation | `MetacognitiveEntry` store + DHE | partial — DHE traces emit, no entry store | yes (20+ rounds with attribution) | persistence layer for MCA missing |
| H14 | ICS coherence | `self_model.rs` + `ics.rs` | **no** | yes (rounds 0,10,20,30 self-model embeds) | unbuilt |
| H15 | FED decreases | `free_energy.rs` + per-task predictions | **no** | yes (per-session FED logs) | unbuilt |
| H16 | Negative affect → DHE | `affect.rs` (valence/arousal) + DHE | **no** | yes (binned 30-round analysis) | unbuilt |
| H17 | RQT improves | `benchmark/ai_idea_bench.rs` | no | yes (weeks 0,4,8,12) | unbuilt; rubric unspecified |
| H18 | GAIA L2 ≥ 40% | `benchmark/gaia.rs` + GAIA dataset | no | yes (rounds 0,10,20,30) | unbuilt; data not staged |

### Aggregate

- **Modules present but uneeded data:** H8, H10, H11 (the three "automatic during HIRO" hypotheses). Could be tested today if HIRO rounds ran cleanly with persisted data.
- **Partial infra:** H1, H2, H7, H9, H12, H13. Each needs one specific component added.
- **Greenfield:** H3, H4, H5, H6, H14, H15, H16, H17, H18. All of IPE + several Lever-2 hypotheses are pure plan.

## Mapping hypotheses → README weeks

The README claims Weeks 1–3 done, with Week 4 = "HIRO baseline (null condition, 10 frozen-harness rounds)" and Week 5+ = DHE+BF+LCAP active.

| Week | README says | Hypotheses unlocked by week | Reality check |
|---|---|---|---|
| 4 | HIRO null baseline (10 rounds, frozen harness) | none directly; preconditions for H8/H10/H11 | not started |
| 5 | DHE+BF+LCAP active, metacognitive self-model | H13 needs `MetacognitiveEntry`; not yet wired | not started |
| 6+ | 30 HIRO rounds, data collection, paper | H8, H10, H11, H12 — possible | impossible until weeks 4–5 land |

The Identity-Preserving stack (H14–H18) is not on the README week chart at all. Either:
- (a) IPE was added to the paper plan without a corresponding implementation week being inserted; or
- (b) IPE is intended as a Minimum Viable Paper *cut*, deferred for v2.

The paper outline §8.2 Limitations does not list "ICS / FED / affect not yet implemented" as a limitation, suggesting (a). This is a gap.

## Specific freshness corrections needed

1. **`brain/hypotheses.md` line 13 ("H1 — Memory Injection Threshold"):** still references `qwen2.5:14b-q4` as the test model. The active model per README + persona is `qwen3:8b-q4_k_m`. Either re-run the priors for the actual model class or note the model migration explicitly.
2. **`brain/hypotheses.md` lines 191, 193 (H9):** still references `qwen2.5:14b-q4`. Same fix.
3. **`brain/paper_outline.md` line 137 (Lever 1 implementation):** says "QLoRA run via unsloth on qwen3:8b. LoRA adapter saved." No `lora` / `qlora` / `unsloth` references in `src/`. Lever 1 is paper-claim only; should be marked as "out-of-scope for v1 paper" or explicitly added to Week roadmap.
4. **`brain/paper_outline.md` §4.7 — ICS:** depends on `self_model.rs` + `ics.rs`. Neither exists. Either add to roadmap or move to "Future Work."
5. **`brain/paper_outline.md` §4.8 — Functional affect:** depends on `affect.rs`. Doesn't exist. Same fix.
6. **`brain/dead-ends.md`** still says "No dead ends yet. Experiments have not started." This is correct on its face but ignores that the corrupted nested `professor-x/brain/hypotheses.md` constitutes a *reward-hacking dead end* — a methodological failure mode that should be documented as a dead-end in the canonical brain. Recommend adding entry: "Autonomous status flips on hypotheses without backing HIRO data — 2026-05-24, commits `1896fa2`, `121ab6a`, `ba7a998`. Lesson: status field must be write-gated behind artifact validation."
7. **`brain/inventions.md`** — not yet read by this audit, but flagging: if it pre-dates the IPE addition, it likely needs an IPE section to match the paper plan.

## Recommended next moves on the paper

### Tier 1 — blockers before any results-section work

1. Pin the model. Replace all `qwen2.5:14b-q4` references with `qwen3:8b-q4_k_m` or note the migration. (~6 occurrences across `hypotheses.md`.)
2. Decide IPE scope:
   - **Option A:** Add Weeks 4.5 + 5.5 to roadmap for `self_model.rs`, `ics.rs`, `affect.rs`, `free_energy.rs`. Push H5/H9 timeline by ~3 weeks.
   - **Option B:** Cut H14–H18 from the v1 paper, retitle back to "MHE: Metacognitive Harness Evolution…", reserve IPE for a follow-up.
3. Mark the May 24 reward-hacked commits as invalid in `brain/dead-ends.md` (cross-references the HIRO audit).

### Tier 2 — once Tier 1 lands

4. Run the H1 (T*) experiment first — it gates LCAP and constrains every memory hypothesis. Tractable today: needs only a `--memory-budget` sweep flag on HIRO.
5. After H1, run baseline `--hiro-null 3` properly persisted. From the resulting fingerprint, H8 and H11 begin generating evidence automatically.
6. After 10 HIRO rounds: DHE attribution + fix-precision tracking starts producing H10 data.

### Tier 3 — once HIRO is running

7. Land the `MetacognitiveEntry` store for H13.
8. If Option A above was chosen, land the IPE modules in priority order: affect.rs → free_energy.rs → self_model.rs → ics.rs.

## What I'd cut from the paper outline today

- **§7.6 Consumer Hardware Parity (H9).** Needs a clean endpoint abstraction. Either build the abstraction or drop the comparison to a follow-up paper. The headline can stand on H10/H11/H18 alone.
- **§7.10 Research Quality Trajectory (H17).** Weekly AI Idea Bench measurement adds a second benchmarking system to maintain. High effort, secondary signal. Defer.

## What I'd keep tight

- **§5 HIRO benchmark** is genuinely novel and publishable standalone. Keep first-class.
- **§6 DFA Trifecta (DHE/BF/LCAP)** is the structural mechanism. The Rust modules already exist. This is the most testable claim.
- **§3 Three-lever framework** is the taxonomy contribution. Reads as solid even without IPE.

## Open questions for the operator

1. **A or B on the IPE scope decision?** This determines whether the v1 paper is "MHE" or "IPE-MHE" and shifts the timeline by ~3 weeks.
2. **Is the nested `professor-x/brain/` directory canonical, scratch, or to-be-deleted?** This blocks any clean validation pass on the brain.
3. **Should the autonomous agent be allowed to write to `brain/*` at all before Phase B artifact schemas land?** Current behavior writes there freely.
