#!/usr/bin/env bash
# Arcus's gate probes (round 42, ADR 0157), printed: the Schelleng diagram of the cello's open
# D at 10 and 20 cm/s, the Guettler diagram of its attacks, the friction curves and the
# torsion compared. The asserted gate is `cargo test -p cantus-dsp --lib arcus::tests_gate`
# (not ignored); these are the measurements behind its numbers.
#
#   bash tools/arcus/gate.sh [probe-name-filter]
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
slot=${SLOT:-$here/tools/build/slot.sh}
cd "$here"
bash "$slot" cargo test -p cantus-dsp --lib "arcus::tests_gate::probe${1:+_$1}" -- --ignored --nocapture
