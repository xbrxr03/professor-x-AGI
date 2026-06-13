# M4 code-proposer frontier — scoping (executable plan)

**Why:** the only lever that reliably moved capability was *code-level, trajectory-diagnosed*
fixes (0.50→0.85). Prompt/skill self-mutation does not lift the curve, even with a 14B proposer
(confirmed: 1 of 4 candidates beat baseline, a noise-tail fluke — see eval-trust.md). So the real
self-improvement engine proposes **code diffs**, gated by a **measured** benchmark delta. This
scopes that build on top of the safe-evolution substrate that already exists.

## What already exists (the safe substrate — reuse, don't rebuild)
- **Isolated sandbox:** `verify_node_in_sandbox` (loop_runner.rs:90) does `git worktree add
  --detach` → applies the diff → compile check, then cleans up. Changes never touch the live tree.
- **Reward-hacking scan:** `analyze_reward_hacking_text` (:1855) flags suspicious diffs.
- **Identity/ICS gate:** `preservation_guard` (:418) refuses diffs that erase identity anchors.
- **Human approval for core:** Middleware / policyd / memd require human sign-off (risk ≥ 85).
- **Automated diagnosis:** `.claude/skills/diagnose-from-trajectory/scripts/diagnose.py` → dominant
  failure mode + implicated component + fix direction.
- **The empirical gate:** `repo_fix_measure` → trustworthy pass@1 (K reps, MDE).

## What's missing (the two real gaps)
1. **A strong code proposer.** An 8B can't author code fixes; a 14B is marginal even on prompts.
2. **Measured-delta acceptance.** `verify_then_apply` accepts on compile + reward-scan + an LLM
   *Analyzer opinion* — it never measures whether the change improves repo-fix. (Same flaw M4
   fixed for prompts, now applied to code.)

## The pipeline
```
diagnose.py  →  PROPOSE code diff (strong proposer, scoped to implicated component)
             →  git-worktree sandbox: apply diff
             →  GATE: build + FULL test suite + repo-fix pass@1 (K reps)  in the worktree
             →  accept ONLY if pass@1 > baseline + MDE  AND  all tests pass  AND  reward-scan clean
             →  HUMAN APPROVES the diff (non-negotiable for code)
             →  commit_node applies to harness-gaps
```

## THE critical risk — reward-hacking the benchmark (make-or-break)
A loop that edits *any* code and is rewarded by repo-fix pass@1 has a strong incentive to **game
the metric** instead of improving the agent: edit the fixtures/`check.py`, weaken the runner,
hardcode answers, or relax `editverify`. Defenses (all required):
1. **Forbidden zones (hard deny-list):** the diff may NOT touch `scripts/benchmarks/**`, the
   repo-fix runner (`repo_fix_measure`/`run_repo_fix_bench`), `evolved/hiro.rs` eval code,
   `policyd/**`, `memd` internals, identity files, or the test files themselves. Scope to the ONE
   implicated component from diagnosis (e.g. `agentd/react.rs`, `toolbridge/hashedit.rs`).
2. **Measure with PINNED eval code:** run the benchmark using the eval harness + fixtures from a
   clean checkout (e.g. `main`), not the worktree's (possibly-edited) copy — so a change can't
   alter how it's scored.
3. **Full test suite must pass** — a metric-gaming hack usually breaks something else.
4. **Reward-hacking scan + human approval** on every diff before apply.
5. **Hold-out fixtures:** keep a private set the proposer never sees, measured post-hoc; a real
   improvement generalizes, an overfit/hack does not.

## Gate integration (concrete)
Extend the sandbox verify to gate on a measured delta: in the worktree, after compile+tests, run
`repo_fix_measure` (K reps) and compare to a baseline measured the same way on the unmodified tree.
This replaces the LLM-Analyzer accept decision with `pass@1 > baseline + MDE`. New mode:
`--evolve-code-on-repofix` (mirrors `--evolve-on-repofix` but the candidate is a code diff).

## The one foundational decision (yours)
**Who is the code proposer?** This is a thesis question — the *agent* stays local-first, but the
*harness-engineer* role that authors improvements could be:
- **(a) Local-only** (qwen3:14b/32b author the diff) — purest thesis, weakest proposer.
- **(b) Strong proposer in the loop** (a frontier model or a human/Claude authoring the diff) —
  honest to what actually worked (the 0.50→0.85 fixes were human-authored); the agent stays local.
- **(c) Human-in-the-loop only** (diagnose → human writes the diff → gate) — safest, least autonomous.

## Staged build plan
1. **`--evolve-code-on-repofix` skeleton** + the forbidden-zone deny-list + pinned-eval measurement
   (reuses verify_node_in_sandbox worktree). *No proposer yet — drive it with a human-supplied diff
   to validate the gate end-to-end.*
2. **Wire the chosen proposer** (per the decision above) to author the diff from diagnosis output.
3. **Hold-out fixtures** + generalization check.
4. **Run it:** diagnose → propose → gated → human-approve, and see if it reproducibly lifts pass@1
   (the real rising curve, this time on the lever that works).

## Honest expectation
This is the genuine engine — but it is a multi-session build, and the reward-hacking surface is
serious (a self-editing, benchmark-rewarded loop is the misevolution risk the safety gates exist
for). The deny-list + pinned eval + full-test-suite + human approval are not optional. Done right,
this is the standard-setting result: *autonomous-up-to-approval harness improvement, every accepted
change empirically proven on an ungameable, un-gameable-by-construction benchmark.*

---

## First runs (2026-06-13) — engine works mechanically; the honest blockers

Built `--evolve-code-on-repofix` (autonomous: diagnose→propose→safety→worktree gate→auto-commit)
and ran it end-to-end. Findings:

1. **The pipeline works.** Baseline measured (0.643), proposer called, safety guard + worktree
   gate + auto-commit all wired and reached the proposer step. Two bugs found+fixed: the coder
   model rejects `think=true` (400); needs a big ctx for a full file.
2. **The coder returned NO-DIFF for `hashedit.rs`** — and that is *reasonable*: the baseline
   failures were WRONG edits on specific tasks (a model-reasoning issue), not a hashedit bug, and
   hashedit already has the forgiving line-fallback. The coder correctly found nothing to fix.
3. **The deeper, honest insight:** the big *harness* gaps were ALREADY harvested manually this
   session (greedy-loop temp escalation, forgiving hash-edit → 0.50→0.85). So the autonomous
   code-proposer has **little low-hanging harness fruit left** — the residual repo-fix failures
   (off-by-one, multi-file, wrong-edits) are largely **model-capability** limits, which need a
   better *model* (the distillation flywheel / Lever 1), not harness code.
4. **The 32B-coder is slow** (offloaded on the 3060's 12 GB; a direct 12K-prompt query timed out
   at 180s). A code-specialized 14B that fits VRAM would be faster, if weaker.

### Honest next steps (multi-session)
- **Failure-driven targeting:** point the proposer at the component `diagnose.py` implicates, not a
  fixed default. (But the current failures don't cleanly implicate harness code — see #3.)
- **Give the coder a code SECTION, not the whole file** (large files botch diff hunk lines).
- **The real remaining lever is the MODEL** (Lever 1 distillation), now that the harness fixes are
  harvested. The autonomous code-proposer is built and ready for when new harness gaps appear.
