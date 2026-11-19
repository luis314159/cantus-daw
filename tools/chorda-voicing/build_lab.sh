#!/usr/bin/env bash
# Build the voicing lab (chorda_compare, release) from this clone through a build slot and keep
# a copy by tag in <lab>/bin (round 40 kept one per knob set, so a candidate's numbers can be
# rerun on the exact build that made them).   bash build_lab.sh <tag> [--check]
#   --check also runs the Chorda DSP tests and clippy -D warnings on what the lab touches.
# Then: CHORDA_EXE=chorda_compare_<tag> (a bare name is looked up in <lab>/bin, env.sh).
set -u
. "$(dirname "${BASH_SOURCE[0]}")/env.sh"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
S="bash $REPO/tools/build/slot.sh"
cd "$REPO" || exit 1
echo "== build $1"
$S cargo build --release -p cantus-ui --example chorda_compare --message-format short 2>&1 | grep -E "^(error|warning)|error\[|Finished" | head -20
ext=; [ -f target/release/examples/chorda_compare.exe ] && ext=.exe
mkdir -p "$V/bin"
cp "target/release/examples/chorda_compare$ext" "$V/bin/chorda_compare_$1$ext" && echo "copied $V/bin/chorda_compare_$1$ext"
if [ "${2:-}" = "--check" ]; then
  echo "== dsp tests"
  $S cargo test -p cantus-dsp --lib chorda 2>&1 | grep -E "^test result|FAILED|panicked|^error" | head -30
  echo "== clippy"
  $S cargo clippy -p cantus-dsp -p cantus-engine --all-targets -- -D warnings 2>&1 | grep -E "^(error|warning)|Finished" -A4 | head -40
  $S cargo clippy -p cantus-ui --example chorda_compare --example chorda_abx -- -D warnings 2>&1 | grep -E "^(error|warning)|Finished" -A4 | head -40
fi
echo all-done
