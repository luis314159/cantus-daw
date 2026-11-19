#!/usr/bin/env bash
# Quick: render the mf single notes for candidate 5 + extra keys, and the second analyst's
# beat measures on them.   bash beats.sh <tag> "<extra spec>"
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
CHORDA_PHRASES=notes_mf "${EXE:-$CHORDA_EXE}" lab $V/notes5 "$1" "$spec" >/dev/null 2>&1
export ANA2_NOTES=$VW/notes5/wav/$1 ANA2_OUT=$VW/ana2/out_$1 MPLBACKEND=Agg
mkdir -p $V/ana2/out_$1
cd $T/ana2
for s in s24_beat_depth s06_coherence s07_sweep; do
  "$PY" -I $s.py > $V/ana2/out_$1/$s.log 2>&1 || echo "$s failed"
done
"$PY" $T/ana2/summary2.py $V/ana2/out_$1 | grep "^1"
