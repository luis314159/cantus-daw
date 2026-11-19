#!/usr/bin/env bash
# bash q16.sh <tag> "<extra>" : phrases on BASE (default candidate 16; +extra) -> round 38 metric + presence; deletes the render
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${PARTIALS:-$C16_PARTIALS} CHORDA_LEVELS=${LEVELS:-$C16_LEVELS}
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
EXE=${EXE:-$CHORDA_EXE}
CHORDA_PHRASES=middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top "$EXE" lab $V/c16/lab "$1" "$spec" >/dev/null 2>&1
echo "== $1 ($2)"
$PY -I $T/probes/c11/q.py $V/c16/lab/wav/$1 - 2>&1 | head -12
[ -n "${KEEP:-}" ] || rm -f $V/c16/lab/wav/$1/*.wav
