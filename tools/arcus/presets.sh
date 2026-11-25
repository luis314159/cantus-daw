#!/usr/bin/env bash
# Arcus's presets heard and fitted (round 42, ADR 0157, ADR 0101's level windows): every
# factory preset rendered through the real engine on its role's audition phrase (as the
# preset browser plays it) and as five held notes across its instrument's range, its LUFS
# against its role's target; `fit` moves each preset's `gain` to the target (five passes,
# never past a phrase peak of -1.2 dBFS) and writes it into
# crates/cantus-ui/src/views/inst/arcus_presets.rs. The same tool as Fides's
# (`crates/cantus-ui/examples/fides_listen.rs`, its `arcus` argument).
#
#   bash tools/arcus/presets.sh [out-dir] [fit]
#
# out-dir defaults to $ARCUS_PRESETS or $CANTUS_WORK/listen/arcus-presets (tools/sound-refs). Every
# cargo command goes through tools/build/slot.sh; set SLOT to the slot script's path if this
# tree has none of its own.
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
. "$here/tools/sound-refs/cantus_paths.sh"  # CANTUS_REFS, CANTUS_WORK (tools/sound-refs)
out=${1:-${ARCUS_PRESETS:-$CANTUS_WORK/listen/arcus-presets}}
shift || true
slot=${SLOT:-$here/tools/build/slot.sh}
mkdir -p "$out"
cd "$here"
bash "$slot" cargo run -p cantus-ui --release --example fides_listen -- "$out" "$@" arcus
