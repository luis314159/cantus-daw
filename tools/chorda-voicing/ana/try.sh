#!/usr/bin/env bash
# Round 40: render a voicing (phrases, single notes) through the lab and measure it with the
# independent analyst's scripts (copied here, slot 3 pointed at it) and with our metric.
#   bash try.sh <tag> <spec file>
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
tag=$1
spec=$(head -1 "$2")
EXE=${EXE:-$CHORDA_EXE}
CHORDA_PHRASES=middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top,triad "$EXE" lab $V/lab4 "$tag" "$spec" 2>/dev/null
CHORDA_PHRASES=notes_pp,notes_mf,notes_ff "$EXE" lab $V/notes5 "$tag" "$spec" 2>/dev/null
prev=$VW/ana/prev_$tag
mkdir -p "$prev"
"$PY" $T/preview.py "$prev" "1 Chorda 2 (beta.14)=$VW/eng40/wav/chorda2" "2 candidato 1=$VW/lab4/wav/${ANA_SLOT2:-candB}" "3 candidato 2=$VW/lab4/wav/$tag" "4 Salamander=$VW/eng40/wav/salamander" >/dev/null
export ANA_PREV="$VW/ana/prev_$tag" ANA_OUT=$VW/ana/out_$tag ANA_RAW3=$VW/lab4/wav/$tag ANA_NOTES3=$VW/notes5/wav/$tag
mkdir -p "$ANA_OUT"
cd $T/ana
for s in notes_analysis decay_beats fund_decay bursts phrases_spectra octave_diff space phrase_time onset_shape fill; do
  "$PY" $s.py > "$ANA_OUT/$s.log" 2>&1 || echo "$s failed"
done
cd $V
"$PY" $T/metric.py eng40/wav/salamander lab4/wav/$tag | tail -1 > "$ANA_OUT/metric.log"
"$PY" $T/bassobj.py eng40/wav/salamander lab4/wav/$tag | tail -1 >> "$ANA_OUT/metric.log"
"$PY" $T/levels.py eng40/wav/salamander lab4/wav/$tag >> "$ANA_OUT/metric.log"
"$PY" $T/ana/summary.py "$ANA_OUT"
