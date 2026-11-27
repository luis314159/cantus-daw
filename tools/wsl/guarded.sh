#!/usr/bin/env bash
# Run tools/wsl/check.sh <ref> <mode> in WSL while the drives it grows on stay above a floor;
# under it, WSL is shut down and the run reported as stopped.
#   bash tools/wsl/guarded.sh <ref> clippy|test [floor-GB, default 4]
# Why: Windows' paging file lives on C:, and a WSL build's memory can push it until C: is
# full (2026-11-21). The WSL disk (ext4.vhdx) grows 30-40 GB during a full test build and
# never shrinks; on the owner's PC it moved from C: to F:\wsl\Ubuntu-24.04 (2026-11-26), so
# both drives are watched (CANTUS_WSL_DRIVES, default "c f"). The log goes to
# $CANTUS_LOG_DIR (default /tmp).
ref=$1; mode=$2; floor=${3:-4}
drives=${CANTUS_WSL_DRIVES:-c f}
free_gb() { df -BG "/$1" | tail -1 | awk '{gsub("G","",$4); print $4}'; }
report() { for d in $drives; do printf '%s: %s GB free  ' "${d^^}" "$(free_gb "$d")"; done; echo; }
echo "at start: $(report)"
wsl -d Ubuntu-24.04 -- bash -lc "bash /mnt/f/projects/cantus/tools/wsl/check.sh $ref $mode" > "${CANTUS_LOG_DIR:-/tmp}/wsl-$mode.log" 2>&1 &
pid=$!
while kill -0 $pid 2>/dev/null; do
  for d in $drives; do
    if [ "$(free_gb "$d")" -lt "$floor" ]; then
      echo "STOPPED: ${d^^}: under $floor GB"
      wsl --shutdown
      exit 2
    fi
  done
  sleep 20
done
wait $pid; rc=$?
echo "at end: $(report)check exit $rc"
tail -25 "${CANTUS_LOG_DIR:-/tmp}/wsl-$mode.log"
