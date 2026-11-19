#!/usr/bin/env bash
# Round 40, candidate 6: render a voicing (phrases + single notes) and measure it with BOTH
# analysts' scripts: the second analyst's (copied here, pointed at the render) for the ranked
# items, and the first one's accept.py through ana/try.sh.
#   bash try2.sh <tag> <spec file>      (CHORDA_EXE, CHORDA_PARTIALS, CHORDA_LEVELS from env)
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
tag=$1
bash $T/ana/try.sh "$tag" "$2" > $V/ana/out_$tag.txt 2>&1
# A preview set where "3 candidato 5" is this try (the analyst's labels).
P=$VW/ana2/prev_$tag
mkdir -p "$P"
"$PY" $T/preview.py "$P" "3 candidato 5=$VW/lab4/wav/$tag" "4 Salamander=$VW/eng40/wav/salamander" >/dev/null
export ANA2_PREV=$P ANA2_OUT=$VW/ana2/out_$tag ANA2_RAW=$VW/lab4/wav/$tag ANA2_NOTES=$VW/notes5/wav/$tag MPLBACKEND=Agg
mkdir -p $V/ana2/out_$tag
cd $T/ana2
for s in ${ANA2_SCRIPTS:-s02_notes s06_coherence s07_sweep s24_beat_depth s21_upper_mid_colour s18_rise_summary s09_dynamics s17_sustain s19_tonal_sustain s20_top_gaps}; do
  "$PY" -I $s.py > $V/ana2/out_$tag/$s.log 2>&1 || echo "$s failed"
done
"$PY" $T/ana2/accept2.py $V/ana/out_$tag $V/ana2/out_$tag | tail -4
tail -3 $V/ana/out_$tag/metric.log
"$PY" $T/ana2/summary2.py $V/ana2/out_$tag
