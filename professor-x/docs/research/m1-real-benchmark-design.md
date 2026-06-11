# M1 — Real Benchmark Design (offline, local-model coding agent)

**Goal (from MILESTONE.md M1):** wire a *small real* coding benchmark runnable fully
offline on the 3060, producing an honest `pass@1` we can trust and improve. This is
the target M2 grinds against and M4 evolves on.

## Why not raw SWE-bench
SWE-bench(-Verified) needs per-issue Docker images, large repos, and frontier-grade
agents; on a 3060 + qwen3:8b it would score ~0 and each task takes many minutes —
useless as a fast feedback loop right now. We need a benchmark with **SWE-bench's
shape** (issue → edit → tests decide) but **lightweight + offline + self-contained**.

## The format: `repo-fix` tasks
Each task is a self-contained mini-repo with a planted bug and a test that encodes the
fix. Success is **objective and deterministic**: the repo's own test goes red→green
after the agent's edit. No LLM-judge needed for these (deterministic ground truth).

Task spec (JSON, mirrors `hiro/tasks.json` style):
```json
{
  "id": "fix_001",
  "category": "repo_fix",
  "setup": "scripts/benchmarks/repo_fix/fix_001/",   // files copied into a /tmp workdir
  "description": "In calc.py, `add` returns a-b instead of a+b. Fix it so tests pass.",
  "verify_cmd": "python -m pytest -q",               // run in the workdir
  "expect_exit": 0                                    // pass iff verify_cmd exits 0
}
```

Runner (new HIRO category or a sibling `--repo-fix-bench`):
1. Copy `setup/` into a fresh `/tmp/px-bench-<id>/` (never mutate the source fixtures).
2. Run `verify_cmd` once to confirm it starts **red** (test fails) — guards against
   trivially-passing tasks.
3. Give the agent the workdir + description; let it edit via the existing edit stack
   (hashedit/window/editverify/apply_patch).
4. Run `verify_cmd` again. **Pass iff exit code == `expect_exit`.**
5. Record pass/fail + the diff the agent produced (audit trail).

This reuses the M0.2 judge plumbing: it's a third evaluator, `repo_fix_test`
(deterministic, exit-code based) alongside `expected`/`success_criteria`.

## Starter set (author 10, balanced difficulty)
Small planted bugs across languages the harness can run offline:
- Python: off-by-one, wrong operator, missing return, bad dict key, exception not caught.
- Rust: wrong comparison, missing `?`, off-by-one in a slice, wrong match arm.
- Shell/text: a config value, a regex.
Each ships a minimal test (`pytest`/`cargo test`/a shell assert). Keep each repo < 5 files.

## Gate (M1 done-when)
- `--repo-fix-bench` runs the 10 tasks offline, copies to /tmp, confirms red→(agent)→
  re-test, and reports an honest `pass@1`.
- Baseline recorded even if near-zero. (Expect low until M2's thrash fix lands —
  that's fine; this is the ruler, M2 is the lifting.)

## Sequencing note
Author the fixtures + runner now (no GPU). First real `pass@1` measurement waits until
M2.1 (thrash→synthesis) is validated on HIRO, so the agent can actually finish a task.
Order: **close M0 → validate M2.1 on HIRO → run this benchmark for the M1 baseline.**
