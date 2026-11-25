#!/usr/bin/env bash
# Arcus's violin and viola beside recordings (round 42, arcus-upper): every reference note found
# under the references' folder (the Iowa MIS violin and viola runs, VSCO 2 CE's solo violin) and
# Arcus at the same key, dynamic and technique, measured the same way (the match tool's features);
# each feature's distance Arcus–recording beside recording–recording, into <out>/compare.txt
# (`examples/arcus_upper_compare.rs`). The references live outside the worktree:
# `$ARCUS_UPPER_REFS` (default $CANTUS_REFS/arcus-upper, whose refs/ holds VSCO 2 CE's solo
# violin beside the Iowa runs; tools/sound-refs fetches both).
#
#   bash tools/arcus/upper-compare.sh [refs-dir] [out-dir]
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
. "$here/tools/sound-refs/cantus_paths.sh"  # CANTUS_REFS, CANTUS_WORK (tools/sound-refs)
slot=${SLOT:-$here/tools/build/slot.sh}
refs=${1:-${ARCUS_UPPER_REFS:-$CANTUS_REFS/arcus-upper}}
out=${2:-$CANTUS_WORK/listen/arcus-upper/compare}
cd "$here"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_upper_compare -- "$refs" "$out"
