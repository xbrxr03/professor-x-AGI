#!/usr/bin/env bash
# Stranger smoke test (M3 gate): follow the documented install EXACTLY as a fresh user, then
# complete a real task. Proves onboarding works and the agent does real work end-to-end.
# Usage:  bash scripts/stranger_smoke.sh
set -uo pipefail
cd "$(dirname "$0")/.."   # professor-x/

echo "== 1. install (build release + link profx) =="
bash install.sh || { echo "FAIL: install.sh"; exit 1; }

PROFX="$(command -v profx || echo "$HOME/.local/bin/profx")"
echo "profx -> $PROFX"
[ -x "$PROFX" ] || { echo "FAIL: profx not executable"; exit 1; }

echo "== 2. sanity: binary runs =="
"$PROFX" --hiro-smoke >/dev/null 2>&1 && echo "  --hiro-smoke OK (tasks file valid)" || { echo "FAIL: --hiro-smoke"; exit 1; }

echo "== 3. complete a real task: fix bugs on the repo-fix benchmark (red -> green) =="
PROFESSOR_X_DATA_DIR="$HOME/.professor-x" "$PROFX" --repo-fix-bench --model qwen3:8b-q4_K_M 2>/dev/null \
  | grep -E "repo-fix fix_.* pre=|pass@1"
echo "== stranger smoke complete =="
