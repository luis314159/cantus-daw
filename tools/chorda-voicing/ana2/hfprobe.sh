#!/usr/bin/env bash
# Quick (candidate 6): phrases + mf/ff notes of a base spec + extra keys; the 4-6 kHz sustain,
# round 38's metric, the middle and upper-middle sustain.  BASE=… bash hfprobe.sh <tag> "<extra>"
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
EXE=${EXE:-$CHORDA_EXE}
CHORDA_PHRASES=middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top,triad "$EXE" lab $V/lab4 "$1" "$spec" >/dev/null 2>&1
CHORDA_PHRASES=notes_mf,notes_ff "$EXE" lab $V/notes5 "$1" "$spec" >/dev/null 2>&1
cd $T/ana2
echo "== $1: $2"
"$PY" -I hf2.py $VW/notes5/wav/$1 ff | tail -6 | awk '{print "  hf", $0}'
mkdir -p $V/ana2/out_$1
ANA2_NOTES=$VW/notes5/wav/$1 ANA2_OUT=$VW/ana2/out_$1 MPLBACKEND=Agg "$PY" -I s17_sustain.py > $V/ana2/out_$1/s17_sustain.log 2>&1
grep -E "^(C3|C4|C5|C6)-" $V/ana2/out_$1/s17_sustain.log | cut -c1-96 | sed 's/^/  /'
cd $V
"$PY" -I $T/metric.py eng40/wav/salamander lab4/wav/$1 | tail -1
"$PY" $T/levels.py eng40/wav/salamander lab4/wav/$1 | tail -1
