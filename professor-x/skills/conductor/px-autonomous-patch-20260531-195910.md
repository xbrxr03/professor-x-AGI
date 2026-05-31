# px-autonomous-patch-20260531-195910

## Purpose
Preserve one observed autonomous patch-apply cycle as a reusable conductor note.

## Inputs
- Generated patch path
- Current harness commit
- Work-loop cycle record
- Patch apply run report

## Workflow
1. Build a small patch with a concrete changed path.
2. Send it through the sandbox trial run before touching main.
3. Run the main check after the patch lands.
4. Create a git commit and store the run report.
5. Show the commit id in the work feed and loop record.

## Output Contract
Return `accepted`, `applied`, `commit`, `checks`, `diff_hash`, `diff_bytes`, and `report_path`.
