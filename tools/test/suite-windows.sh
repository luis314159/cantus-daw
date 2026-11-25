#!/usr/bin/env bash
# The whole workspace suite on Windows, --no-fail-fast, the way a round's close and a release
# candidate need it (docs/RELEASING.md L2, DEVELOPING §4): every crate but cantus-ui in one
# run, then cantus-ui in batches (tools/test/ui-batches.sh — its ~370 binaries do not fit on a
# small disk at once). No debug info, no incremental cache (CLAUDE.md: a full-debug target
# reached 100 GB). Prints the disk before and after, and the totals of both halves.
#
#   bash tools/test/suite-windows.sh [out-dir, default target/suite-win] [ui batch size, 20]
#
# Run it alone: never two suites, or a suite and a CPU measurement, at once on one machine
# (the timing tests would fail for the load). Run from the repository's root (or a worktree's).
# The suite of 2026-11-20 (7b287c39) took 93 minutes: 36 for the rest, 57 for cantus-ui.
set -u
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
out="${1:-target/suite-win}"
size="${2:-20}"
mkdir -p "$out"
git log --oneline -1 | tee "$out/head.txt"
df -h . | tail -1
echo "== the workspace without cantus-ui: $(date)"
cargo test --workspace --exclude cantus-ui --no-fail-fast > "$out/rest.log" 2>&1
rest=$?
echo "rest exit $rest, $(date)"
echo "== cantus-ui in batches of $size: $(date)"
bash tools/test/ui-batches.sh "$size" "$out/ui.txt" > "$out/ui-run.log" 2>&1
ui=$?
echo "ui exit $ui, $(date)"
df -h . | tail -1
# The sound lab's light tests (round 42, ADR 0160): loudness against EBU Tech 3341/3342 and
# BS.1770-4, the features on synthetic notes, the band method, the blind kit's guarantees —
# when CANTUS_PY names a Python with tools/soundlab/requirements.txt.
lab=0
if [ -n "${CANTUS_PY:-}" ]; then
  echo "== the sound lab (light): $(date)"
  "$CANTUS_PY" -I tools/soundlab/tests/run.py --light > "$out/soundlab.log" 2>&1
  lab=$?
  echo "sound lab exit $lab: $(tail -1 "$out/soundlab.log")"
fi
sum() {
  grep -E '^test result' "$1" | awk -v n="$2" '{ p += $4; f += $6; g += $8; b++ }
    END { print n ": binaries", b, "passed", p, "failed", f, "ignored", g }'
}
sum "$out/rest.log" "rest"
sum "$out/ui.txt" "cantus-ui"
cat "$out/rest.log" "$out/ui.txt" | grep -E '^test result' | awk '{ p += $4; f += $6; g += $8 }
  END { print "TOTAL passed", p, "failed", f, "ignored", g }'
# A test executable named *setup* or *update* is refused by Windows's installer detection
# ("requires elevation", os error 740) and never runs: say so, they are run renamed.
grep -h "os error 740" "$out/rest.log" "$out/ui-run.log" | head -5
grep -h -B2 -E "FAILED|panicked|^error" "$out/rest.log" "$out/ui.txt" | head -60
[ "$rest" = 0 ] && [ "$ui" = 0 ] && [ "$lab" = 0 ]
