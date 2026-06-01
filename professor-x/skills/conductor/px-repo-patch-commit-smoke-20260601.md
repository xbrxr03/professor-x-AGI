---
name: px-repo-patch-commit-smoke-20260601
description: Preserve evidence that repo patch coding sessions can verify, apply, check, and commit a material harness patch.
allowed-tools:
  - patch.apply
  - shell.restricted
---

# Repo Patch Commit Smoke

Use this conductor note when validating the coding-agent bridge from a proposed unified diff to a committed harness change.

Required evidence:
- policy decision for patch.apply
- sandbox verification report
- main cargo check result
- git commit hash for the accepted patch
