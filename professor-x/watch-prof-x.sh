#!/usr/bin/env bash
# Live scrolling transcript of Professor X working — like a coding CLI agent.
# Streams thought / action / observation as each step happens.
# Usage:  ./watch-prof-x.sh
set -uo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)/artifacts/events"
FILE="$DIR/$(date +%F).jsonl"

while [ ! -f "$FILE" ]; do
  echo "waiting for events file: $FILE ..."
  sleep 2
  FILE="$DIR/$(date +%F).jsonl"
done

echo "=================================================================="
echo " Professor X — live transcript"
echo " $FILE"
echo " (Ctrl-C to stop; this does NOT stop the run)"
echo "=================================================================="

tail -n 30 -f "$FILE" | jq -rj --unbuffered '
  (.timestamp[11:19]) as $t |
  if   .event_type=="task.started"        then "\n========== TASK ==========\n\(.summary)\n"
  elif .event_type=="llm.response"        then "\($t)  THINK  \((.payload.preview // "") | gsub("\n";" ") | .[0:160])\n"
  elif .event_type=="tool.requested"      then "        WANTS  \(.summary)\n"
  elif .event_type=="tool.started"        then "        RUN    \(.summary)\n"
  elif .event_type=="tool.succeeded"      then "        OK     \((.payload.output_preview // "") | gsub("\n";" ") | .[0:100])\n"
  elif .event_type=="tool.failed"         then "        FAIL   \(.summary)\n"
  elif .event_type=="react.duplicate_action" then "        BLOCK  duplicate action stopped\n"
  elif .event_type=="react.circuit_breaker"  then "        BREAK  circuit breaker tripped\n"
  elif (.event_type|startswith("policy.deny")) then "        DENY   \(.summary)\n"
  elif .event_type=="task.succeeded"      then ">>>>> TASK SUCCEEDED\n"
  elif .event_type=="task.failed"         then ">>>>> TASK FAILED\n"
  elif (.event_type|startswith("hiro."))  then "### \(.summary)\n"
  else empty end
'
