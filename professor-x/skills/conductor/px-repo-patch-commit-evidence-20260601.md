---
name: px-repo-patch-commit-evidence-20260601
description: Confirm that commit-capable repo patch coding sessions report every gate from sandbox verification through git commit.
allowed-tools:
  - patch.apply
  - shell.restricted
---

# Repo Patch Commit Evidence

Use this note as a regression target for the apply-and-commit coding-session path.

The session report should include these proof points:
- sandbox reward-hacking scan
- sandbox material diff and cargo check
- main apply check
- main cargo check
- accepted git commit hash
