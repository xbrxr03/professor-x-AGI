#!/usr/bin/env bash
# Polished live dashboard for the distillation flywheel.
#   bash distill/dash.sh
# Uses the training venv's Python (has `rich`). Falls back to the plain watch.sh if rich is missing.
cd "$(dirname "$0")/.."
PY="distill/.venv/bin/python"
if [ -x "$PY" ] && "$PY" -c "import rich" 2>/dev/null; then
  exec "$PY" distill/dash.py "$@"
else
  echo "(rich/venv not found — falling back to distill/watch.sh)"
  exec bash distill/watch.sh "$@"
fi
