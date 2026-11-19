#!/usr/bin/env bash
# Quick (candidate 6, third builder): the phrases of a base spec + extra keys, analyst 1's
# space script only (P4: the treble's early and late energy after a staccato; P3's phrase
# correlations).   BASE=<spec file> bash spaceprobe.sh <tag> "<extra spec>"
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
tag=$1
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
CHORDA_PHRASES=middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top,triad "$CHORDA_EXE" lab $V/lab4 "$tag" "$spec" >/dev/null 2>&1
prev=$VW/ana/prev_$tag
mkdir -p "$prev"
"$PY" $T/preview.py "$prev" "1 Chorda 2 (beta.14)=$VW/eng40/wav/chorda2" "2 candidato 1=$VW/lab4/wav/${ANA_SLOT2:-candB}" "3 candidato 2=$VW/lab4/wav/$tag" "4 Salamander=$VW/eng40/wav/salamander" >/dev/null
export ANA_PREV="$prev" ANA_OUT=$VW/ana/out_$tag ANA_RAW3=$VW/lab4/wav/$tag ANA_NOTES3=$VW/notes5/wav/$tag
mkdir -p "$V/ana/out_$tag"
cd $T/ana
"$PY" space.py > "$V/ana/out_$tag/space.log" 2>&1 || echo "space failed"
"$PY" phrases_spectra.py > "$V/ana/out_$tag/phrases_spectra.log" 2>&1 || echo "phrases failed"
"$PY" -I -c "
import csv
E={(r['phrase'],r['source']):r for r in csv.DictReader(open(r'$VW/ana/out_$tag/csv/space_early.csv'))}
for ph in ('top','fast_run'):
    a,b=E[(ph,'cand2')],E[(ph,'Salamander')]
    print('$tag', ph, ' '.join(f'{k[:5]} {float(a[k]):+.1f}/{float(b[k]):+.1f}' for k in ('early_5_80ms_vs_direct_db','late_80_195ms_vs_direct_db','side_vs_mid_5_80ms_db')))
try:
    S=list(csv.DictReader(open(r'$VW/ana/out_$tag/csv/phrase_summary.csv')))
    print('$tag corr', ' '.join(f\"{r['phrase']}={float(r['corr']):.2f}\" for r in S if r['source']=='cand2'))
except Exception as e: print('no summary', e)
"
rm -rf $V/lab4/wav/$tag $V/ana/prev_$tag
