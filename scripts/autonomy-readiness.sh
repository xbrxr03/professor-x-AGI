#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE="$ROOT/professor-x"

echo "== Professor X autonomy readiness =="
echo "repo: $ROOT"
echo

echo "== git status =="
git -C "$ROOT" status --short
echo

echo "== cargo check =="
(cd "$CRATE" && cargo check)
echo

echo "== cargo test =="
(cd "$CRATE" && cargo test)
echo

echo "== dry-run daily cycle =="
(cd "$CRATE" && cargo run -- --dry-run-daily)
echo

echo "readiness checks passed"
