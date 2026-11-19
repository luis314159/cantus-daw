#!/usr/bin/env bash
# Candidate 16's upper-colour probe: bash p10.sh <tag> "<extra spec>"
#   renders notes_mf,notes_ff on BASE (+extra), prints s21's rows and P10, deletes the render.
#   BASE / PARTIALS / LEVELS (default candidate 16's; round 40 ran it on candidate 15), EXE, KEEP=1 keeps wavs
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${PARTIALS:-$C16_PARTIALS} CHORDA_LEVELS=${LEVELS:-$C16_LEVELS}
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
EXE=${EXE:-$CHORDA_EXE}
CHORDA_PHRASES=notes_mf,notes_ff "$EXE" lab $V/c16/notes "$1" "$spec" >/dev/null 2>&1
mkdir -p $V/c16/out/$1
(cd $T/ana2 && ANA2_NOTES=$VW/c16/notes/wav/$1 ANA2_OUT=$VW/c16/out/$1 MPLBACKEND=Agg "$PY" -I s21_upper_mid_colour.py > $V/c16/out/$1/s21.log 2>&1)
echo "== $1 ($2)"
grep -E "^(A4|C5|D#5|F#5|A5|mean|==)" $V/c16/out/$1/s21.log
"$PY" -I $T/probes/c16/p10.py $V/c16/out/$1/s21.log
[ -n "${KEEP:-}" ] || rm -f $V/c16/notes/wav/$1/*.wav
