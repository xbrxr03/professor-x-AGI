# px-verify-patch

## Purpose
Turn a proposed harness patch into a reviewable git commit only after an isolated trial run succeeds.

## Inputs
- Unified diff patch path
- Current harness commit
- `professor-x/artifacts/evolution/patch-verifications/`
- Event stream from `evolution.patch_apply.*`

## Workflow
1. Confirm the main worktree has no source, config, skill, or unmanaged artifact changes.
2. Scan the patch text before any main worktree mutation.
3. Create an isolated sandbox worktree from `HEAD`.
4. Apply the patch in the sandbox and reject empty or no-op diffs.
5. Run `cargo check` in the sandbox.
6. Apply the verified diff to main only after the sandbox passes.
7. Run `cargo check` in main, then create a git commit with the changed paths and run report.
8. If main apply, check, or commit fails, reverse the diff and record the failed report.

## Output Contract
Record `accepted`, `applied`, `checks`, `diff_hash`, `commit`, `report_commit`, `reason`, and `report_path`.

## Operating Notes
Routine patch apply should prefer small, reversible changes with clear changed paths and a concrete run report.
