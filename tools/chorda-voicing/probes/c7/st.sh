#!/usr/bin/env bash
# Stereo only: render phrases + notes_mf of base+extra, run the first analysis's space.py and
# phrases_spectra.py, print P3 and its parts.   BASE=… EXE=… bash st.sh <tag> "<extra>"
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${CHORDA_PARTIALS:-$C16_PARTIALS} CHORDA_LEVELS=${CHORDA_LEVELS:-$C16_LEVELS}
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
EXE=${EXE:-$CHORDA_EXE}
CHORDA_PHRASES=middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top,triad "$EXE" lab $V/c7/st "$1" "$spec" >/dev/null 2>&1
CHORDA_PHRASES=notes_pp,notes_mf,notes_ff "$EXE" lab $V/c7/st "$1" "$spec" >/dev/null 2>&1
prev=$VW/c7/st/prev_$1; mkdir -p $prev
"$PY" $T/preview.py "$prev" "1 Chorda 2 (beta.14)=$VW/eng40/wav/chorda2" "2 candidato 1=$VW/lab4/wav/${ANA_SLOT2:-candB}" "3 candidato 2=$VW/c7/st/wav/$1" "4 Salamander=$VW/eng40/wav/salamander" >/dev/null
export ANA_PREV=$prev ANA_OUT=$VW/c7/st/out_$1 ANA_RAW3=$VW/c7/st/wav/$1 ANA_NOTES3=$VW/c7/st/wav/$1
mkdir -p $ANA_OUT; cd $T/ana
for s in space phrases_spectra; do "$PY" $s.py > $ANA_OUT/$s.log 2>&1 || echo "$s failed"; done
cd $V; echo "== $1: $2"
"$PY" -I -c "
import sys; sys.path.insert(0, r'$TW/ana'); import accept, numpy as np
a, p = accept.score(r'$VW/c7/st/out_$1')
print('P3 %.3f' % p['P3 stereo'])
I = accept.rows(r'$VW/c7/st/out_$1', 'space_itd.csv')
print('lag', {s: round(float(np.mean([abs(float(r['lag_ms'])) for r in I if r['src']==s])),3) for s in ('sal','c2')})
S = accept.rows(r'$VW/c7/st/out_$1', 'phrase_summary.csv')
print(' '.join('%s %s/%s' % (r['phrase'], r['corr'], [q['corr'] for q in S if q['phrase']==r['phrase'] and q['source']=='Salamander'][0]) for r in S if r['source']=='cand2'))
"
"$PY" $T/metric.py eng40/wav/salamander c7/st/wav/$1 | tail -1
