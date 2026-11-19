#!/usr/bin/env bash
# pp.sh and pfq.sh on one line: partials 3/8/9/10 and their frequencies.   bash pp2.sh <tag> "<extra spec>"
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
bash $T/probes/c7/pp.sh "$1" "$2" | sed 's/^.*): //' | awk -v t="$1 ($2)" '{printf "%s p3 %s p8 %s p9 %s p10 %s | ", t, $4, $9, $10, $11}'
"$PY" -I $T/probes/c7/pfreq.py $V/c7/keys/wav/$1/keys_mf.wav 48 76 | awk '{print $9, $10, $13, $14, $NF}'
