#!/usr/bin/env bash
# Live scrolling transcript of Professor X working — like a coding CLI agent.
# Streams thought / action / observation as each step happens.
# Usage:  ./watch-prof-x.sh
set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)/artifacts/events"
FILE="$DIR/$(date +%F).jsonl"

# Wait for today's event file to exist.
while [ ! -f "$FILE" ]; do
  echo "waiting for $FILE ..."; sleep 2
  FILE="$DIR/$(date +%F).jsonl"
done

echo "── Professor X live transcript ── $FILE ──"
tail -n 40 -f "$FILE" | jq -rj --unbuffered '
  (.timestamp[11:19]) as $t |
  if   .event_type=="task.started"   then "\n[1;36m━━━ TASK ━━━ \(.summary)[0m\n"
  elif .event_type=="llm.response"   then "[2m\($t)[0m 💭 \((.payload.preview // "") | gsub("\n";" ") | .[0:200])\n"
  elif .event_type=="tool.requested" then "[33m  → \(.summary)[0m\n"
  elif .event_type=="tool.started"   then "[34m  ⚙ \(.summary)[0m\n"
  elif .event_type=="tool.succeeded" then "[32m  ✓ \(.summary)[0m  [2m\((.payload.output_preview // "") | gsub("\n";" ") | .[0:120])[0m\n"
  elif .event_type=="tool.failed"    then "[31m  ✗ \(.summary)[0m\n"
  elif (.event_type|startswith("policy.")) then "[2m  · \(.summary)[0m\n"
  elif .event_type=="react.circuit_breaker" then "[31m  ‖ \(.summary)[0m\n"
  elif .event_type=="task.succeeded" then "[1;32m  ■ \(.summary)[0m\n"
  elif .event_type=="task.failed"    then "[1;31m  ■ \(.summary)[0m\n"
  elif (.event_type|startswith("hiro.")) then "[1;35m\(.summary)[0m\n"
  else empty end
'
