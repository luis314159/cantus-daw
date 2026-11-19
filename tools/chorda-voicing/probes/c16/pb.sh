#!/usr/bin/env bash
# bash pb.sh <tag> "<extra>": notes_mf,notes_ff render -> P10 (s21) and the first analysis's P1 (decay_beats); deletes render
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
KEEP=1 bash $T/probes/c16/p10.sh "$1" "$2" | grep -E "P10|mean"
mkdir -p $V/c16/out/$1/a/csv
(cd $T/ana && ANA_NOTES3=$VW/c16/notes/wav/$1 ANA_OUT=$VW/c16/out/$1/a MPLBACKEND=Agg "$PY" -I decay_beats.py > $V/c16/out/$1/a/decay_beats.log 2>&1)
"$PY" -I $T/probes/c16/p1.py $VW/c16/out/$1/a
[ -n "${KEEP:-}" ] || rm -f $V/c16/notes/wav/$1/*.wav
