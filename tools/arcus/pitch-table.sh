#!/usr/bin/env bash
# Arcus's bowed-pitch table (round 42, ADR 0157): every key of each instrument bowed at five
# dynamics and three contact points, its fundamental measured, the offset written as the
# loop's correction, then measured again with it applied and what is left added — into
# crates/cantus-dsp/src/arcus_pitch_data.rs (`examples/arcus_pitch_table.rs`). Run it after
# any change to the string, the bow or the bowing map; `tests_gate` then holds the bowed
# pitch within a cent over the grid and between its points.
#
#   bash tools/arcus/pitch-table.sh
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
slot=${SLOT:-$here/tools/build/slot.sh}
cd "$here"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_pitch_table -- \
  crates/cantus-dsp/src/arcus_pitch_data.rs
