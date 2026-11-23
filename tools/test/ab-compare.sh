#!/usr/bin/env bash
# A/B a change to DSP code: the same test binary built twice — once with the files under test
# as they are at a base commit, once as they are in the working tree — then one ignored test
# run alternately on both, each run's dump compared byte for byte and each run's printed cost
# shown side by side. Proves "the same bits, less CPU" for an optimisation (beta.16's Fragor
# solver, 2026-11-20); everything but the named files (the tests, the harness) is the working
# tree's in both builds, so a test added with the change runs on the old code too.
#
#   bash tools/test/ab-compare.sh --crate cantus-dsp --test fragor::tests::cpu_synth_fuzz_board \
#        --files crates/cantus-dsp/src/fragor_dk.rs [--base HEAD] [--runs 2] [--dump-var CANTUS_FRAGOR_DUMP]
#
# The test writes its output's bytes to the file named by $dump-var (when set) and prints its
# cost; `grep` keeps the lines with "%" or "ns". Run it on a quiet machine for the costs (no
# suite, no other measurement); the bit comparison holds under any load. Builds go through
# tools/build/slot.sh; a release build of the crate's lib tests (`--lib`), or `--bin-test
# <name>` for an integration test. The working tree's files are put back however it ends.
set -euo pipefail
crate="" test="" files="" base="HEAD" runs=2 dumpvar="CANTUS_AB_DUMP" kind="--lib"
while [ $# -gt 0 ]; do
  case "$1" in
    --crate) crate="$2"; shift 2 ;;
    --test) test="$2"; shift 2 ;;
    --files) files="$2"; shift 2 ;;
    --base) base="$2"; shift 2 ;;
    --runs) runs="$2"; shift 2 ;;
    --dump-var) dumpvar="$2"; shift 2 ;;
    --bin-test) kind="--test $2"; shift 2 ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done
[ -n "$crate" ] && [ -n "$test" ] && [ -n "$files" ] || { sed -n 2,17p "$0"; exit 2; }
export PATH="$HOME/.cargo/bin:$PATH" CARGO_INCREMENTAL=0
root="$(git rev-parse --show-toplevel)"
cd "$root"
slot="$root/tools/build/slot.sh"
work="$root/target/ab-compare"
mkdir -p "$work/keep"
for f in $files; do
  mkdir -p "$work/keep/$(dirname "$f")"
  cp "$f" "$work/keep/$f"
done
restore() { for f in $files; do cp "$work/keep/$f" "$f"; done; }
trap restore EXIT
# Builds the test binary into $exe, cargo's messages kept in $work/<which>.json and .log. The
# tests must compile against both versions of the files: a test that calls a function only
# the new code has fails the base build, and its error is shown.
build() {
  local which="$1"
  # shellcheck disable=SC2086
  bash "$slot" cargo test --release -p "$crate" $kind --no-run -j "${JOBS:-6}" \
    --message-format=json > "$work/$which.json" 2> "$work/$which.log" || true
  # -a: cargo's JSON can carry a localized linker's bytes (a Spanish Windows's "Creando
  # biblioteca"), and grep would answer "Binary file matches" instead of the path.
  exe=$(grep -ao '"executable":"[^"]*"' "$work/$which.json" | tail -1 \
    | sed 's/"executable":"//; s/"$//; s#\\\\#/#g')
  if [ -z "$exe" ]; then
    echo "the $which build failed:" >&2
    grep -ao '"rendered":"[^"]*' "$work/$which.json" | head -5 | sed 's/"rendered":"//' >&2
    tail -5 "$work/$which.log" >&2
    exit 1
  fi
}
for f in $files; do git show "$base:$f" > "$f"; done
build base
cp "$exe" "$work/base.exe"
restore
build new
cp "$exe" "$work/new.exe"
echo "base: $base's $files; new: the working tree's"
dir="$root/crates/$crate"
same=1
for i in $(seq 1 "$runs"); do
  for v in base new; do
    ( cd "$dir" && env "$dumpvar=$work/$v.bin" "$work/$v.exe" "$test" --ignored --nocapture \
        --test-threads 1 2>&1 | grep -E "%|ns" | sed "s/^/$v: /" ) || true
  done
  if ! cmp -s "$work/base.bin" "$work/new.bin"; then same=0; fi
done
if [ "$same" = 1 ]; then echo "BIT-IDENTICAL ($runs runs)"; else echo "THE OUTPUTS DIFFER"; exit 1; fi
