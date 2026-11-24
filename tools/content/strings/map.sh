#!/usr/bin/env bash
# Round 42 (strings-rec, ADR 0158): measure the recorded strings and put their packs in the
# manifest — the octave fix and tuning per zone (pitch measured), the loops found and kept
# only click-free, the zones, keyswitches and mod-wheel layers.
#
#   bash tools/content/strings/fetch_sources.sh <dir>     # once: the sources, outside git
#   bash tools/content/strings/map.sh <dir> [pack]        # measure, then merge into the manifest
#
# Writes <dir>/strings.json (every sample's measurement: root, octave fix, tune, loop, seam
# error, click excess) and rewrites crates/cantus-content/manifest/content.json in place.
# Cargo goes through tools/build/slot.sh (three builds machine-wide).
set -eu
dir=${1:?usage: map.sh <sources dir> [pack]}
here=$(cd "$(dirname "$0")/../../.." && pwd)
slot=${CANTUS_SLOT:-/f/projects/cantus/tools/build/slot.sh}
cd "$here"
bash "$slot" cargo run --release -p cantus-content --example strings_map -- "$dir" "$dir/strings.json" ${2:-}
python -I tools/content/strings/merge.py "$dir" "$dir/strings.json" crates/cantus-content/manifest/content.json
