#!/usr/bin/env bash
# Watch a drive's free space while a long build or suite runs, and say so before it fills:
# WSL's disk image lives on C: and grows with every target built inside it, and never
# shrinks by itself (2026-11-20: the WSL suite took C: from 26 to 15 GB free). Prints a line
# when the free space moves by `step` GB, and LOW under `low` GB — stop the suite then
# (wsl --shutdown) rather than let Windows's own drive fill.
#
#   bash tools/test/disk-watch.sh [drive, default /c] [low GB, 8] [step GB, 4]
#
# Run it beside the suite (in the background, or as a Monitor's command); it runs until
# stopped. To give the space back afterwards: delete old ~/cantus-ci-* clones' targets inside
# WSL, then `wsl --shutdown` and compact the image (the memory note on this machine's quirks,
# docs/DEVELOPING.md §4).
drive="${1:-/c}"
low="${2:-8}"
step="${3:-4}"
last=""
while true; do
  free=$(df -BG --output=avail "$drive" | tail -1 | tr -dc 0-9)
  band=$((free / step))
  if [ "$band" != "$last" ]; then
    echo "$drive: $free GB free"
    last=$band
  fi
  if [ "$free" -lt "$low" ]; then
    echo "LOW: $drive: $free GB free"
  fi
  sleep 30
done
