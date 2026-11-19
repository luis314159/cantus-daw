#!/usr/bin/env bash
# Quick: mf+ff single notes for candidate 5 + extra keys, then the second analyst's per-channel
# partial balance for A4 ... D#6.   bash colour.sh <tag> "<extra spec>"
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
CHORDA_PHRASES=notes_mf,notes_ff "${EXE:-$CHORDA_EXE}" lab $V/notes5 "$1" "$spec" >/dev/null 2>&1
export ANA2_NOTES=$VW/notes5/wav/$1 ANA2_OUT=$VW/ana2/out_$1 MPLBACKEND=Agg
mkdir -p $V/ana2/out_$1
cd $T/ana2
"$PY" -I s21_upper_mid_colour.py 57,60,63,66,69,72,75,78,81,84,87 > $V/ana2/out_$1/s21_upper_mid_colour.log 2>&1
grep -E "^(==|mean|C4|A4|C5|D#5|F#5|A5|C6)" $V/ana2/out_$1/s21_upper_mid_colour.log
