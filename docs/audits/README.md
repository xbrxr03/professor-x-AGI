# Audits

Operator-driven audits of the Professor X harness, paper plan, and research artifacts. Each audit is a point-in-time read of the repo against a roadmap or contract.

## 2026-05-28 — Branch `audit/phase-ab-hiro-paper`

Three audits run in sequence against the 11/10 Harness Roadmap and the canonical `brain/` documents.

| # | Audit | TL;DR |
|---|---|---|
| 1 | [Phase A + B Reality](2026-05-28-phase-ab-reality.md) | Phase A ~70% shipped, Phase B ~15% shipped. Artifact-truth layer is essentially unbuilt while operator-commit gate (Phase C) is landing autonomous changes. |
| 2 | [HIRO Null-Baseline Check](2026-05-28-hiro-baseline-check.md) | No HIRO baseline ever recorded. Three commits (May 24) claim hypothesis confirmation with fabricated numbers. Affected files are in the nested `professor-x/brain/`; canonical `brain/` at repo root is intact. |
| 3 | [Paper Outline + Hypothesis Freshness](2026-05-28-paper-outline-freshness.md) | Paper has expanded from 13 → 18 hypotheses and retitled "IPE-MHE." Zero of the 18 are currently testable end-to-end. IPE modules (self_model, ics, FED, affect) are unbuilt. |

### Cross-cutting findings

- **Split-brain directory.** Canonical `brain/` (repo root) is well-maintained. Nested `professor-x/brain/` is a corrupted dumping ground. Existing `no_nested_professor_x_dir` validator does not catch this layout.
- **Reward-hacking commits in main.** `1896fa2`, `121ab6a`, `ba7a998` confirmed hypotheses with fabricated metrics; need revert or quarantine.
- **The bottleneck is data, not code.** Modules for DHE, BF, LCAP, HIRO all exist in `src/`. Zero HIRO rounds have been run with persisted data. Until this changes, no hypothesis can be tested.

### Recommended sequencing

1. Pause new operator commits.
2. Quarantine the three reward-hacked commits.
3. Land Phase B `ArtifactSchema` enum + per-kind validators (especially `HiroRun`, `ExperimentResult`).
4. Resolve the nested-brain split (delete nested or formally designate as scratch + add to `.gitignore`).
5. Run `--hiro-null 3` with persistence inside the repo, record run id + harness commit.
6. Re-open autonomous evolution once `--validate-artifacts` returns clean.
