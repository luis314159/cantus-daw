#!/usr/bin/env bash
# Arcus measured against recorded cellos and double basses, and matched to them (round 42,
# docs/round42/arcus-match.md): every reference note under the references' folder (the Iowa
# MIS's anechoic runs, TinySOL, VSCO 2 CE's solo bass, Karoryfer's cello, Smolken's bass —
# downloaded file by file, never into the worktree) and Arcus at the same instrument,
# technique, key and dynamic, measured the same way (harmonics 1-40, the third-octave
# spectrum, the centroid, the noise between harmonics, the attack, the vibrato, a pluck's
# decays and thump); the distances to the references' spread on the held-out third, before
# (model 1) and after (model 2), into <out>/distances.csv with a plot per held-out mf note.
# `fit` fits model 2's match on the other two thirds and writes it into
# crates/cantus-dsp/src/arcus_tables.rs (crates/cantus-dsp/examples/arcus_match/).
#
#   bash tools/arcus/match.sh [refs-dir] [out-dir] [fit]
#
# refs-dir defaults to $ARCUS_REFS or $CANTUS_REFS/arcus (tools/sound-refs); out-dir to
# docs/round42/arcus-match. Every cargo command goes through tools/build/slot.sh.
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
. "$here/tools/sound-refs/cantus_paths.sh"  # CANTUS_REFS, CANTUS_WORK (tools/sound-refs)
refs=${1:-${ARCUS_REFS:-$CANTUS_REFS/arcus}}
out=${2:-$here/docs/round42/arcus-match}
shift 2 || true
slot=${SLOT:-$here/tools/build/slot.sh}
cd "$here"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_match -- "$refs" "$out" "$@"
