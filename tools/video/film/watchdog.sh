#!/usr/bin/env bash
# Stop a render or build of ours when the shared machine runs short (round 41, the video
# session's rule): free RAM under 4 GB or C: under 6 GB.
#   bash tools/video/film/watchdog.sh <pid> [log]
# Polls every 15 s while <pid> runs; kills it (and says so) when either limit is crossed.
pid="$1"; log="${2:-/dev/stderr}"
while tasklist //FI "PID eq $pid" 2>/dev/null | grep -q "$pid"; do
  free_kb=$(awk '/^MemFree:/ {print $2}' /proc/meminfo)
  c_kb=$(df -k /c | awk 'NR==2 {print $4}')
  if [ "${free_kb:-0}" -lt 4194304 ] || [ "${c_kb:-0}" -lt 6291456 ]; then
    echo "$(date +%T) watchdog: RAM free $((free_kb / 1024)) MB, C: free $((c_kb / 1024)) MB — stopping $pid" >> "$log"
    taskkill //PID "$pid" //T //F > /dev/null 2>&1
    exit 2
  fi
  sleep 15
done
exit 0
