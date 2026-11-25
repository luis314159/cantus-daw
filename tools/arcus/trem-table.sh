#!/usr/bin/env bash
# Arcus's tremolo pitch table (round 42, pass 5, `docs/round42/arcus-match.md`): every
# instrument's tremolo measured at each key and five dynamics by the study's tracker (YIN,
# the strokes' median pitch), twice — the offset, then what is left with it applied — and
# written to crates/cantus-dsp/src/arcus_trem_pitch_data.rs (`arcus_tests_p5.rs`'s
# `write_tremolo_table`). Re-run it when the string, the bow or the tremolo's stroke changes.
#
#   bash tools/arcus/trem-table.sh
#
# Every cargo command goes through tools/build/slot.sh; set SLOT to the slot script's path if
# this tree has none of its own.
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
slot=${SLOT:-$here/tools/build/slot.sh}
cd "$here"
bash "$slot" cargo test -p cantus-dsp --lib arcus::tests_p5::write_tremolo_table -- --ignored --nocapture --exact
