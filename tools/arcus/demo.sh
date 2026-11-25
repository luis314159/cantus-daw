#!/usr/bin/env bash
# Arcus heard as music (round 42, ADR 0157, ADR 0158): an original Andante cantabile in D major
# for violin, viola, cello and double bass, in which the instruments enter one by one, reach a
# climax and leave again. The song is built in a running app as a user builds it (the picker,
# the preset browser, the commands), saved, exported through the app's export (48 kHz 24-bit,
# -16 LUFS under -1 dBTP) with stems, and checked: a second export and the saved project's
# export are the same to the byte; levels, audibility per section, clicks at note boundaries,
# the hall's tail (crates/cantus-ui/examples/arcus_demo/).
#
#   bash tools/arcus/demo.sh [out-dir]
#
# out-dir defaults to $ARCUS_DEMO or ../listen/arcus-demo beside the worktree; it gets
# arcus-cuarteto.wav, arcus-cuarteto.cantus, stems/ and index.html. Exits 1 when a check fails.
# Every cargo command goes through tools/build/slot.sh; set SLOT to the slot script's path if
# this tree has none of its own.
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
out=${1:-${ARCUS_DEMO:-$here/../listen/arcus-demo}}
slot=${SLOT:-$here/tools/build/slot.sh}
mkdir -p "$out"
cd "$here"
bash "$slot" cargo run -p cantus-ui --example arcus_demo -- "$out"
