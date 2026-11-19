#!/usr/bin/env bash
# Candidate 7: render the single notes (pp, mf, ff) of the base spec + extra keys, probe the treble.
#   BASE=<spec> EXE=<exe> bash trial.sh <tag> "<extra spec>"
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${CHORDA_PARTIALS:-$C16_PARTIALS}
export CHORDA_LEVELS=${CHORDA_LEVELS:-$C16_LEVELS}
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
for d in ${DYNS:-pp mf ff}; do
  CHORDA_PHRASES=notes_$d "${EXE:-$CHORDA_EXE}" lab $V/c7/notes "$1" "$spec" >/dev/null 2>&1 &
done
wait
echo "== $1: $2"
"$PY" -I $T/probes/c7/tprobe2.py $V/c7/notes/wav/$1 $V/notes5/wav/salamander
