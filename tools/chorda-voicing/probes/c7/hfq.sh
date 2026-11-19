#!/usr/bin/env bash
# mf (and ff) notes of base+extra -> hf2 bass rows + beats (s24) + sustain (s17).  bash hfq.sh <tag> "<extra>"
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
DYNS="${DYNS:-mf}" bash $T/probes/c7/trial.sh "$1" "$2" > /dev/null 2>&1
cd $T/ana2
echo "== $1: $2"
"$PY" -I hf2.py $VW/c7/notes/wav/$1 mf | tail -6 | awk '{print "  hf", $0}'
mkdir -p $V/c7/out_$1
export ANA2_NOTES=$VW/c7/notes/wav/$1 ANA2_OUT=$VW/c7/out_$1 MPLBACKEND=Agg
for s in s24_beat_depth s07_sweep s17_sustain; do "$PY" -I $s.py > $V/c7/out_$1/$s.log 2>&1; done
"$PY" $T/ana2/summary2.py $V/c7/out_$1 | grep -E "^1 beat|^1 flanger"
grep -E "^(A0|C2|C3|C4)-" $V/c7/out_$1/s17_sustain.log | cut -c1-100
