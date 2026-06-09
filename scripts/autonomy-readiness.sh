#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE="$ROOT/professor-x"

echo "== Professor X autonomy readiness =="
echo "repo: $ROOT"
echo

echo "== git status =="
status="$(git -C "$ROOT" status --short)"
if [[ -n "$status" ]]; then
  printf '%s\n' "$status"
  if [[ "${PROFESSOR_X_ALLOW_DIRTY:-0}" != "1" ]]; then
    echo
    echo "working tree is dirty; set PROFESSOR_X_ALLOW_DIRTY=1 to run advisory checks anyway" >&2
    exit 1
  fi
else
  echo "clean"
fi
echo

echo "== whitespace checks =="
git -C "$ROOT" diff --check
git -C "$ROOT" diff --cached --check
echo

echo "== repository structure =="
required_dirs=(
  "$CRATE/harness/config"
  "$CRATE/harness/prompts"
  "$CRATE/harness/tool_descriptions"
  "$CRATE/harness/skills"
  "$CRATE/harness/middleware"
  "$CRATE/harness/policies"
  "$CRATE/tests/policy"
  "$CRATE/tests/hiro"
  "$CRATE/tests/evolution"
  "$CRATE/tests/skills"
  "$CRATE/artifacts/audit/chain-checks"
  "$CRATE/artifacts/evolution/reward-hacking"
  "$CRATE/artifacts/evolution/rollbacks"
  "$CRATE/artifacts/paper/tables"
  "$CRATE/artifacts/paper/figures"
  "$CRATE/artifacts/paper/runs"
)
for dir in "${required_dirs[@]}"; do
  test -d "$dir" || {
    echo "missing required directory: ${dir#$ROOT/}" >&2
    exit 1
  }
done
echo "structure ok"
echo

echo "== cargo check =="
(cd "$CRATE" && cargo check)
echo

echo "== cargo test --bins =="
(cd "$CRATE" && cargo test --bins)
echo

echo "== HIRO inventory smoke =="
(cd "$CRATE" && cargo run -- --hiro-smoke)
echo

echo "== dry-run daily cycle =="
(cd "$CRATE" && cargo run -- --dry-run-daily)
echo

echo "readiness checks passed"
