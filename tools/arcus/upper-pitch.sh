#!/usr/bin/env bash
# Arcus's violin and viola bowed-pitch table (round 42, arcus-upper): the cello's method on the
# upper strings, whose voices run at `upper::factor` × the rate — every key bowed at nine
# dynamics and five contact points, the offset written as the loop's correction, measured again
# with it applied and what is left added — into crates/cantus-dsp/src/arcus_upper_pitch_data.rs
# (`examples/arcus_upper_pitch.rs`). Run it after any change to the string, the bow, the
# bowing map or the factor; `upper_tests_gate` then holds the bowed pitch on a held-out grid.
#
#   bash tools/arcus/upper-pitch.sh
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
slot=${SLOT:-$here/tools/build/slot.sh}
cd "$here"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_upper_pitch -- \
  crates/cantus-dsp/src/arcus_upper_pitch_data.rs
