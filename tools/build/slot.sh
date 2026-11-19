#!/usr/bin/env bash
# Build slots (round 40): at most three cargo builds at once on one machine (a 31 GB machine
# ran out of memory with four), and one WSL check at a time (the WSL image is one disk).
# Wrap every cargo command, in every worktree, in it:
#
#   bash tools/build/slot.sh cargo test -p cantus-ai
#   bash tools/build/slot.sh --wsl wsl -d Ubuntu-24.04 -- bash /mnt/f/<clone>/tools/wsl/check.sh <branch> test
#   bash tools/build/slot.sh --status          # who holds which slot
#
# A slot is a directory (mkdir is atomic) under CANTUS_SLOTS_DIR (default /tmp/cantus-build-slots:
# one place per machine, whichever worktree runs it — set it to the same folder everywhere if
# your shells disagree on /tmp). A slot whose holder is gone, or older than three hours, is taken
# back. Waiting is silent; long waits are normal — run long commands in the background.
#
#   CANTUS_SLOTS=3      build slots (one more for SLOT_PRIORITY=1: a builder whose
#                       build-and-measure cycles keep losing the race; keep its builds at -j 2)
#   CANTUS_SLOTS_DIR    where the slots live
set -u
dir=${CANTUS_SLOTS_DIR:-/tmp/cantus-build-slots}
mkdir -p "$dir"
n=${CANTUS_SLOTS:-3}
[ "${SLOT_PRIORITY:-0}" = "1" ] && n=$((n + 1))
prefix=build
if [ "${1:-}" = "--status" ]; then
  for l in "$dir"/*/; do
    [ -d "$l" ] || continue
    read -r pid t rest < "$l/owner" 2>/dev/null || { echo "$(basename "$l"): (no owner yet)"; continue; }
    echo "$(basename "$l"): pid $pid, $(( ($(date +%s) - t) / 60 )) min, $rest"
  done
  exit 0
fi
if [ "${1:-}" = "--wsl" ]; then
  n=1
  prefix=wsl
  shift
fi
[ $# -gt 0 ] || { echo "usage: slot.sh [--wsl] <command...> | --status" >&2; exit 2; }
while :; do
  for i in $(seq 1 "$n"); do
    l="$dir/$prefix$i"
    if mkdir "$l" 2>/dev/null; then
      echo "$$ $(date +%s) $(pwd) $*" > "$l/owner"
      trap 'rm -rf "$l"' EXIT INT TERM
      "$@"
      rc=$?
      rm -rf "$l"
      trap - EXIT INT TERM
      exit $rc
    fi
    read -r pid t _ < "$l/owner" 2>/dev/null || { pid=""; t=""; }
    now=$(date +%s)
    if [ -n "$t" ] && { [ $((now - t)) -gt 10800 ] || ! kill -0 "$pid" 2>/dev/null; }; then
      rm -rf "$l"
    fi
  done
  sleep 15
done
