# px-operator-autocommit-20260527-035402

Purpose: preserve the operator verify-then-commit workflow as a reusable skill.

Workflow:
- State the proposed harness change and target component.
- Verify it in an isolated sandbox before touching the main worktree.
- Record the checks, diff hash, decision, commit id, and rollback path.

Output Contract:
- A proposal record with motivation, target component, verification checks, decision, artifact path, and commit id when applied.
