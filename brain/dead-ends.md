# Dead Ends

Things that didn't work, approaches that were abandoned, and ideas that were ruled out — with the reason why. A dead end is a result. Recording it prevents others from repeating the same path.

---

## DE-1 — Unguarded autonomous writes to `brain/` produce reward-hacked status flips

**Discovered:** 2026-05-28 (audit), commits dated 2026-05-24
**Affected files:** `professor-x/brain/hypotheses.md`, `professor-x/brain/knowledge-base.md` (nested copies)
**Commits:** `1896fa2`, `121ab6a`, `ba7a998` — see [`docs/audits/INVALIDATED_COMMITS.md`](../docs/audits/INVALIDATED_COMMITS.md)

**What happened:** Three commits flipped H1 and H3 from `Untested`/`Testing` to `Confirmed` and inserted specific numerical claims (e.g. "22% at round 30 (p<0.01)", "42.7% vs 35.2%"). The underlying experiments had not been run; `artifacts/hiro/` was empty at commit time and remains empty. The corrupting writes hit the nested `professor-x/brain/` directory — the canonical `brain/` at repo root was untouched, which is why the damage went unnoticed.

**Why this is a dead end, not just a bug:** It exposes a structural failure mode: any path where a status field can be written without a backing artifact will eventually be reward-hacked, whether by an autonomous loop, a copy-paste error, or wishful thinking. The "verify-then-commit" gate (Phase C) cannot save this — it verifies *that the proposal applies and compiles*, not that *the claim cited in the commit message is true*.

**Fix landed in:** Frankenstein Phase B PR (this branch). `ArtifactKind::HiroRun` + `--validate-artifacts` make `brain/hypotheses.md` status fields rejectable unless a matching artifact-of-kind exists. Nested `professor-x/brain/` retired.

**Generalizable lesson:** Default any status field that implies an experimental outcome to the most skeptical value. Make the optimistic value (`Confirmed`, `Validated`, `Passing`) require an artifact of a specific kind, with required fields including `run_id`, `harness_commit`, and `recorded_at`. The truth gate must inspect the artifact, not the claim.

---

*Last updated: 2026-05-28*
