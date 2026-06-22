# CODEX_TASK.md — Stream B: failure-taxonomy measurement

You are Codex, working in parallel with Claude. Read `AGENTS.md` first. Your stream is **measurement
only — do NOT edit anything under `src/`** (Claude owns the Rust). Work in your own worktree:

```bash
cd /home/abrar/professor-x-main-integrate
git worktree add ../px-codex-measure -b codex/failure-taxonomy prereboot-flywheel-prep
cd ../px-codex-measure/professor-x
```

## Task
Write `scripts/benchmarks/repo_fix/failure_taxonomy.py` that runs the native repo-fix benchmark and
reports WHERE the local models fail, then write the result table to
`docs/research/2026-06-21-failure-taxonomy.md`.

- Models: `qwen3:8b-q4_K_M` and `profx-distilled-clean`.
- Task sets: the hard set, then each `scripts/benchmarks/repo_fix/tasks_family_*.json`.
- Run each via the release binary:
  `PROFESSOR_X_NATIVE_TOOLS=1 PROFESSOR_X_DATA_DIR=$HOME/.professor-x REPO_FIX_TASKS=<tasks.json> \
   ./target/release/professor-x --repo-fix-bench --model <model>`
- Parse the emitted events / run artifacts (under `artifacts/repo-fix/<date>/…json`) and bucket each
  failing task by: `duplicate_action`, `finish_rejected`, `edit-apply-error`, `wrong-edit-verified-fail`,
  `loop/forfeit`, `other`.
- Output `docs/research/2026-06-21-failure-taxonomy.md`: a per-model, per-task-set table of counts +
  pass@1, plus a one-paragraph honest read of the dominant failure bucket.

## Rules
- Do NOT modify `src/`. Only add the script + the doc.
- Don't rewrite other agents' lines in `AGENTS.md` — append only.
- When done: in `AGENTS.md`, check boxes B1–B3 and append a log line
  `- [<ISO-time>] (Codex) <what you produced + dominant failure bucket>`.

## Why this matters
Your histogram decides whether Stream C (apply-retry-with-feedback) is worth building — i.e., whether
edit-apply / wrong-edit is still a leading failure given the harness's existing guards.
