#!/usr/bin/env bash
# The whole cantus-ui suite on a disk that cannot hold its ~370 test executables at once
# (about 25 GB with debug info off): the library's tests, then the integration tests in
# batches, each batch built and run with --no-fail-fast and its executables deleted after.
# Round 41 ran it this way on F: (docs/STATUS.md). Run from the repository's root:
#
#   bash tools/test/ui-batches.sh [batch size, default 20] [output file]
#
# It stops before a batch when the target's drive has under 6 GB free. Prints the totals
# and every failing line at the end.
set -u
size="${1:-20}"
out="${2:-target/ui-batches.txt}"
target="${CARGO_TARGET_DIR:-target}"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
mkdir -p "$(dirname "$out")"
: > "$out"
git log --oneline -1
keep='^test result|panicked|^error|FAILED|^     Running'
cargo test -p cantus-ui --lib --no-fail-fast 2>&1 | grep -E "$keep" >> "$out"
mapfile -t stems < <(ls crates/cantus-ui/tests/*.rs | xargs -n1 basename | sed 's/\.rs$//')
for ((i = 0; i < ${#stems[@]}; i += size)); do
  free=$(df -BG --output=avail "$target" | tail -1 | tr -dc 0-9)
  if [ "$free" -lt 6 ]; then
    echo "only ${free} GB free before batch $i: stopping"
    break
  fi
  args=()
  for s in "${stems[@]:i:size}"; do args+=(--test "$s"); done
  cargo test -p cantus-ui "${args[@]}" --no-fail-fast 2>&1 | grep -E "$keep" >> "$out"
  for s in "${stems[@]:i:size}"; do
    rm -f "$target"/debug/deps/"$s"-????????????????{,.exe,.pdb}
  done
  echo "batch $i done, $(df -h "$target" | tail -1 | awk '{print $4}') free"
done
awk '/^test result/ { p += $4; f += $6; g += $8; n++ }
  END { print "binaries", n, "passed", p, "failed", f, "ignored", g }' "$out"
grep -B2 -E "FAILED|panicked|^error" "$out" | head -60
