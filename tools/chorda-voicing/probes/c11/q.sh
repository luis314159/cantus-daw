#!/usr/bin/env bash
# Candidate 11's quick probe: bash q.sh <tag> "<extra spec>" [notes]
#   EXE (default $CHORDA_EXE), BASE / PARTIALS / LEVELS (default candidate 16's, candidates/c16)
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${PARTIALS:-$C16_PARTIALS} CHORDA_LEVELS=${LEVELS:-$C16_LEVELS}
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
EXE=${EXE:-$CHORDA_EXE}
CHORDA_PHRASES=middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top "$EXE" lab $V/c11/lab "$1" "$spec" >/dev/null 2>&1
N=-
if [ -n "${3:-}" ]; then
  CHORDA_PHRASES=notes_pp,notes_mf,notes_ff "$EXE" lab $V/c11/notes "$1" "$spec" >/dev/null 2>&1
  N=$V/c11/notes/wav/$1
fi
echo "== $1 ($2)"
$PY -I $T/probes/c11/q.py $V/c11/lab/wav/$1 $N
