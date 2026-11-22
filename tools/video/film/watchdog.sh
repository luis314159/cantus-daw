#!/usr/bin/env bash
# Stop a render or build of ours when the shared machine runs short (round 41, the video
# session's rule): free RAM under 4 GB or C: under 6 GB.
#   bash tools/video/film/watchdog.sh <pid> [log]
# Polls every 10 s while <pid> runs; kills it (and says so) when either limit is crossed at
# three readings in a row (20 s): the other builders' moments (a test's burst of temporary
# files, a link) pass in seconds and are not a shortage.
pid="$1"; log="${2:-/dev/stderr}"
low=0
while tasklist //FI "PID eq $pid" 2>/dev/null | grep -q "$pid"; do
  free_kb=$(awk '/^MemFree:/ {print $2}' /proc/meminfo)
  c_kb=$(df -k /c | awk 'NR==2 {print $4}')
  if [ "${free_kb:-0}" -lt $(( ${GENTLY_STOP_MB:-4096} * 1024 )) ] || [ "${c_kb:-0}" -lt 6291456 ]; then
    low=$((low + 1))
    echo "$(date +%T) watchdog: RAM free $((free_kb / 1024)) MB, C: free $((c_kb / 1024)) MB (low $low/3)" >> "$log"
    if [ "$low" -ge 3 ]; then
      echo "$(date +%T) watchdog: stopping $pid" >> "$log"
      taskkill //PID "$pid" //T //F > /dev/null 2>&1
      exit 2
    fi
  else
    low=0
  fi
  sleep 10
done
exit 0
