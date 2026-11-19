#!/usr/bin/env bash
# Quick (candidate 6): mf single notes of a base spec + extra keys, then the beats and the
# sustain measures.   BASE=<spec file> bash quick6.sh <tag> "<extra spec>"
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
CHORDA_PHRASES=${PHR:-notes_mf} "${EXE:-$CHORDA_EXE}" lab $V/notes5 "$1" "$spec" >/dev/null 2>&1
export ANA2_NOTES=$VW/notes5/wav/$1 ANA2_OUT=$VW/ana2/out_$1 MPLBACKEND=Agg
mkdir -p $V/ana2/out_$1
cd $T/ana2
for s in ${QS:-s24_beat_depth s07_sweep s17_sustain}; do
  "$PY" -I $s.py > $V/ana2/out_$1/$s.log 2>&1 || echo "$s failed"
done
echo "== $1: $2"
"$PY" $T/ana2/summary2.py $V/ana2/out_$1 | grep -E "^1 beat|^1 flanger"
grep -E "^(C2|C3|C4|C5)-" $V/ana2/out_$1/s17_sustain.log | cut -c1-120
