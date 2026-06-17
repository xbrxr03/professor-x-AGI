#!/usr/bin/env bash
# Live flywheel dashboard — open in its own terminal to watch the GPU crunch:
#   bash distill/watch.sh
# Shows: GPU util/mem/temp/power, the Ollama model loaded, and the current
# flywheel phase + latest training loss from /tmp/distill_flywheel.log.
LOG="${1:-/tmp/distill_flywheel.log}"
strip_ansi() { sed -u 's/\x1b\[[0-9;?]*[a-zA-Z]//g; s/\r//g'; }
while true; do
  clear
  echo "============================ PROFESSOR X — FLYWHEEL WATCH ============================"
  date '+%Y-%m-%d %H:%M:%S'
  echo
  echo "── GPU ──────────────────────────────────────────────────────────────────────────────"
  nvidia-smi --query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw \
             --format=csv,noheader 2>/dev/null \
    | awk -F',' '{printf "  %s | util %s | mem %s/%s | %s | %s\n",$1,$2,$3,$4,$5,$6}'
  echo
  echo "── Ollama (resident model) ───────────────────────────────────────────────────────────"
  ollama ps 2>/dev/null | sed '1d' | awk 'NF{printf "  %s  %s  %s\n",$1,$3" "$4,$5" "$6}' || true
  [ -z "$(ollama ps 2>/dev/null | sed '1d')" ] && echo "  (none loaded)"
  echo
  echo "── Flywheel phase  ($LOG) ────────────────────────────────────────────────────────────"
  if [ -f "$LOG" ]; then
    grep -aE "==.*\[|GPU free before train|loss'?:|epoch|Merging|Converting|quantize|stop-sanity|done_reason|baseline mean|distilled mean|ACCEPT|REJECT|STOP:" "$LOG" \
      | strip_ansi | tail -n 8 | sed 's/^/  /'
  else
    echo "  (no flywheel log yet — start a run with distill/run_after_reboot.sh)"
  fi
  echo "──────────────────────────────────────────────────────────────────────────────────────"
  echo "  refresh 1s · Ctrl-C to exit"
  sleep 1
done
