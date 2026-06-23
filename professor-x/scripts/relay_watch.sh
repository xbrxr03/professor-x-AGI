#!/usr/bin/env bash
# Auto-relay watcher. Polls git + RELAY.md; prints ready tasks for an agent and exits when work appears.
# Usage: AGENT=claude scripts/relay_watch.sh [interval_seconds]
#   Claude: run as a background waiter that re-invokes the session when a task is ready.
#   Codex : run in a loop and act on the printed task id.
set -euo pipefail
AGENT="${AGENT:-claude}"
INTERVAL="${1:-270}"            # 270s keeps the prompt cache warm (<5min); raise for idle.
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
cd "$REPO"
while true; do
  git fetch origin -q 2>/dev/null || true
  # pull RELAY.md / trunk updates (fast-forward only; never clobber local work)
  git merge --ff-only "origin/prereboot-flywheel-prep" -q 2>/dev/null || true
  python3 professor-x/scripts/relay.py heartbeat --agent "$AGENT" >/dev/null 2>&1 || true
  if READY="$(python3 professor-x/scripts/relay.py ready --agent "$AGENT" 2>/dev/null)"; then
    echo "=== RELAY: ready task(s) for @$AGENT ==="
    echo "$READY"
    exit 0                       # caller acts; re-launch the watcher after the task completes
  fi
  sleep "$INTERVAL"
done
