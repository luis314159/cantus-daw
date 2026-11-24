#!/usr/bin/env bash
# Arcus's violin and viola beside recordings (round 42, arcus-upper): every reference note found
# under the references' folder (the Iowa MIS violin and viola runs, VSCO 2 CE's solo violin) and
# Arcus at the same key, dynamic and technique, measured the same way (the match tool's features);
# each feature's distance Arcus–recording beside recording–recording, into <out>/compare.txt
# (`examples/arcus_upper_compare.rs`). The references live outside the worktree:
# `$ARCUS_UPPER_REFS` (default ../downloads/arcus-upper; VSCO's solo violin is under
# ../downloads/strings-rec/vsco — pass a folder holding both, or link them in).
#
#   bash tools/arcus/upper-compare.sh [refs-dir] [out-dir]
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
slot=${SLOT:-$here/tools/build/slot.sh}
refs=${1:-${ARCUS_UPPER_REFS:-$here/../downloads/arcus-upper}}
out=${2:-$here/../listen/arcus-upper/compare}
cd "$here"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_upper_compare -- "$refs" "$out"
