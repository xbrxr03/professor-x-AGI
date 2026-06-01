# PX Repo Patch Live Commit Smoke

Purpose: verifies that Professor X can stream a repo patch session through main apply, cargo check, and git commit.

Procedure:
- Gate patch.apply before touching the main worktree.
- Verify the patch in an isolated sandbox worktree.
- Apply only after sandbox checks pass.
- Record the accepted git commit in the coding-session report.
