#!/usr/bin/env bash
# Chorda's unison pass, end to end (round 41, `docs/round41/chorda-tune.md`): from a built
# cantus-dsp test binary, optimise every key's unison, re-measure the technician's table on it,
# and write the shipped module. Run from the repository's root:
#
#   bash tools/build/slot.sh cargo test -p cantus-dsp --lib --no-run   # the binary
#   bash tools/chorda-voicing/unison_pass.sh <test exe> <work dir> [<global spec>]
#
# <global spec> is laid before every key's (default `bridge.pull_hz=0.1`). The optimiser takes
# about an hour on 12 threads (CHORDA_THREADS); the table a few minutes. Writes into <work dir>:
# optimise.log, unison.spec, retable.log, table.spec; and crates/cantus-dsp/src/
# chorda_voicing_unison.rs (then `cargo fmt -p cantus-dsp`, rebuild, run the gates).
set -eu
exe=$1
work=$2
global=${3:-bridge.pull_hz=0.1}
mkdir -p "$work"
export CHORDA_THREADS=${CHORDA_THREADS:-12}
# The optimiser reads the shipped voicing with `CHORDA_TUNE_SPEC` over it: start from the
# technician's pass with only the global spec (every key's own values reset).
reset=$(python -I -c "print(','.join(f'uni_tight.{k}=0,uni_b.{k}=1,uni_pair.{k}=0,coupling.{k}=1' for k in range(21,109)))")
CHORDA_TUNE_SPEC="$global,$reset" "$exe" unison_optimise --ignored --nocapture > "$work/optimise.log" 2>&1
keys=$(grep '^SPEC ' "$work/optimise.log" | sed 's/^SPEC //')
[ -n "$keys" ] || { echo "the optimiser printed no spec (see $work/optimise.log)" >&2; exit 1; }
echo "$global,$keys" > "$work/unison.spec"
CHORDA_TUNE_SPEC="$global,$reset,$keys" "$exe" unison_retable --ignored --nocapture > "$work/retable.log" 2>&1
grep '^SPEC ' "$work/retable.log" | sed 's/^SPEC //' > "$work/table.spec"
[ -s "$work/table.spec" ] || { echo "the table printed nothing (see $work/retable.log)" >&2; exit 1; }
python -I tools/chorda-voicing/ship_unison.py "$work/unison.spec" "$work/table.spec"
grep -E '^round|^final' "$work/retable.log"
