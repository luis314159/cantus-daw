#!/usr/bin/env bash
# Round 40, candidate 6 (second builder): one full evaluation of a voicing — both analysts'
# measures (try2.sh), the release scripts, the sub-bass, every key's evenness, accept3.
#   CHORDA_EXE=<lab exe> CHORDA_PARTIALS=<csv> CHORDA_LEVELS=<csv> bash run6.sh <tag> <spec file>
# NO_KEYS=1 skips the every-key render (and so keylev, the tuning gate and P18/P20).
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
tag=$1
export ANA2_SCRIPTS="${ANA2_SCRIPTS:-s02_notes s06_coherence s07_sweep s24_beat_depth s21_upper_mid_colour s18_rise_summary s09_dynamics s17_sustain s19_tonal_sustain s20_top_gaps s13b_release_abs s13c_release_vs_attack s08_attack}"
O=$V/ana2/out_$tag
if [ -z "${NO_KEYS:-}" ]; then
  ( CHORDA_PHRASES=keys_mf "$CHORDA_EXE" lab $V/keys "$tag" "$(head -1 "$2")" >/dev/null 2>&1 ) &
fi
bash $T/ana2/try2.sh "$tag" "$2" > $V/ana2/out_$tag.txt 2>&1
(cd $T/ana2 && ANA2_NOTES=$VW/notes5/wav/$tag ANA2_OUT=$VW/ana2/out_$tag MPLBACKEND=Agg "$PY" -I s21_upper_mid_colour.py 21,24,27,30 > $O/s21_bass.log 2>&1)
wait
if [ -z "${NO_KEYS:-}" ]; then
  "$PY" -I $T/ana2/keylev.py $VW/keys/wav/$tag/keys_mf.wav > $O/keylev.log 2>&1
  "$PY" -I $T/ana2/tuning.py $VW/keys/wav/$tag/keys_mf.wav $O/tuning.log > /dev/null 2>&1
fi
cat $V/ana2/out_$tag.txt
cat $O/keylev.log 2>/dev/null
echo "-- tuning"; cat $O/tuning.log 2>/dev/null
echo "-- release (s13c)"; grep "release noise" $O/s13c_release_vs_attack.log
echo "-- sub-bass (s21 bass, mf)"; sed -n 2,5p $O/s21_bass.log
echo "-- knock pp (s08)"; grep "'pp'" $O/s08_attack.log | tail -5
"$PY" $T/ana2/accept3.py $V/ana/out_$tag $O $O/keylev.log
