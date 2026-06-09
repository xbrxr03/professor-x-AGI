# GitHub Actions CI

The intended Rust CI workflow is stored at `docs/ops/github-actions-ci.proposed.yml`.
It should be copied to `.github/workflows/ci.yml` once the operator has a GitHub
token with `workflow` scope.

## Current Blocker

On 2026-06-09, pushing `.github/workflows/ci.yml` was rejected by GitHub:

```text
refusing to allow an OAuth App to create or update workflow `.github/workflows/ci.yml` without `workflow` scope
```

The attempted workflow commit is preserved locally on branch
`preserve/ci-workflow-20260609` in the integration worktree.

## Enablement Steps

1. Refresh GitHub authentication with `workflow` scope.
2. Re-apply or cherry-pick `preserve/ci-workflow-20260609`.
3. Push `main`.
4. Confirm the `CI` workflow runs `cargo check` and `cargo test --bins`.
