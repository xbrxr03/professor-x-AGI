# px-fix-bug

## Purpose
Fix a bug in a small repo reliably on a local model. Encodes the read→edit→verify
discipline that lifted repo-fix pass@1 from 0.50 to ~0.77 on qwen3:8b (see
docs/research/eval-trust.md). The whole skill is "don't loop, make one real edit."

## Inputs
- A bug description and the target file(s) in the current workspace.

## Workflow
1. List the workspace **once** (`fs.list`). You now have the file names — do NOT list again.
2. Read the buggy file (`fs.read` or `fs.window_open`) to see the exact wrong line.
3. Identify the single line that is wrong. Do not over-investigate a small file.
4. Make **one minimal edit**: `fs.hash_edit` (line + new_text) or `fs.write` the whole small
   file. Change only what the bug requires.
5. If an edit tool errors, re-read the file once, then try a **different** edit — never
   repeat the identical failing action.
6. Finish once the edit is made, stating the one-line fix.

## Anti-patterns (these are the measured failure modes on weak models)
- **Repeating the same action** (`fs.list`/`fs.read`) after you already have its result.
  That is a loop — take the next step (read, then edit) instead.
- **Finishing without making any edit** (the most common miss — gathering, never editing).
- **Inventing a line hash.** If unsure of the hash, re-read to get the real one, or use
  `fs.write` to replace the whole file.

## Output Contract
A minimal edit applied to the target file such that the repo's own test goes red → green.
