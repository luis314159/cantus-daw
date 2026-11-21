#!/usr/bin/env bash
# Run tools/wsl/check.sh <ref> <mode> in WSL while C: (where the WSL disk grows) stays above
# a floor; under it, WSL is shut down and the run reported as stopped.
#   bash tools/wsl/guarded.sh <ref> clippy|test [floor-GB, default 4]
# Why: the WSL disk (ext4.vhdx) and Windows' paging file both live on C:, and a WSL
# build's memory can push the paging file until C: is full (2026-11-21). The log goes to
# $CANTUS_LOG_DIR (default /tmp).
ref=$1; mode=$2; floor=${3:-4}
free_gb() { df -BG /c | tail -1 | awk '{gsub("G","",$4); print $4}'; }
echo "C: free at start: $(free_gb) GB"
wsl -d Ubuntu-24.04 -- bash -lc "bash /mnt/f/projects/cantus/tools/wsl/check.sh $ref $mode" > "${CANTUS_LOG_DIR:-/tmp}/wsl-$mode.log" 2>&1 &
pid=$!
while kill -0 $pid 2>/dev/null; do
  if [ "$(free_gb)" -lt "$floor" ]; then
    echo "STOPPED: C: under $floor GB"
    wsl --shutdown
    exit 2
  fi
  sleep 20
done
wait $pid; rc=$?
echo "C: free at end: $(free_gb) GB; check exit $rc"
tail -25 "${CANTUS_LOG_DIR:-/tmp}/wsl-$mode.log"
