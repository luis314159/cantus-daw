#!/usr/bin/env bash
# mf notes of base+extra: the treble rows of trial + hf2 rows C6/C7.  bash hft.sh <tag> "<extra>"
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
bash $T/probes/c7/trial.sh "$1" "$2" > $V/c7/t_$1.txt 2>&1
echo "== $1: $2"; grep -E "^C7-C8|^C5-B6" $V/c7/t_$1.txt
awk 'NR>7 {print}' $V/c7/t_$1.txt | awk '$1>=96' | awk '{printf "  %s f1@3s %s\n", $1, $NF}' | tr '\n' ' '; echo
cd $T/ana2 && "$PY" -I hf2.py $VW/c7/notes/wav/$1 mf | tail -2
