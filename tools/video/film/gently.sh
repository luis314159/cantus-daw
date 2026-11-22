#!/usr/bin/env bash
# Run a heavy job of ours on the shared machine as the video session's rule asks: start it
# only when RAM has been ≥ 6 GB free and C: ≥ 6.5 GB for a minute, at below-normal priority,
# under watchdog.sh (RAM < 4 GB or C: < 6 GB for 20 s stops it), and try again (up to
# five times) when it was stopped.
#   bash tools/video/film/gently.sh <log> <exe> [args…]
# GENTLY_START_MB (default 6000) and GENTLY_STOP_MB (default 4096, watchdog.sh) set the RAM
# floors: a 4K render wants the defaults; an ffmpeg encode needs far less (final_chain.sh's
# encode steps use 3000 / 1500) — on a machine whose owner's browser holds 17 GB, 6 GB free
# never came for two hours (2026-11-22).
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
log="$1"; exe="$2"; shift 2
winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
args=""
for a in "$@"; do args+="'$a',"; done
for try in 1 2 3 4 5; do
  ok=0
  until [ "$ok" -ge 6 ]; do
    f=$(awk '/^MemFree:/ {print int($2/1024)}' /proc/meminfo)
    c=$(df -k /c | awk 'NR==2 {print int($4/1024)}')
    if [ "$f" -ge "${GENTLY_START_MB:-6000}" ] && [ "$c" -ge 6500 ]; then ok=$((ok + 1)); else ok=0; fi
    sleep 10
  done
  echo "$(date +%T) gently: try $try — $exe $*" >> "$log"
  pid=$(powershell.exe -NoProfile -Command "\$p = Start-Process -FilePath '$(winpath "$exe")' -ArgumentList ${args%,} -WorkingDirectory '$(winpath "$ROOT")' -RedirectStandardOutput '$(winpath "$log").out' -RedirectStandardError '$(winpath "$log").err' -PassThru -WindowStyle Hidden; \$p.PriorityClass = 'BelowNormal'; \$p.Id" | tr -d '\r')
  bash "$ROOT/tools/video/film/watchdog.sh" "$pid" "$log"
  rc=$?
  if [ "$rc" -eq 0 ]; then
    echo "$(date +%T) gently: done" >> "$log"
    exit 0
  fi
done
echo "$(date +%T) gently: stopped five times, giving up" >> "$log"
exit 2
