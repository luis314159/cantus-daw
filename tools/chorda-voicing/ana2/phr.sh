#!/usr/bin/env bash
# Quick (candidate 6): the eight phrases of a base spec + extra keys, round 38's metric in
# detail and the phrases' balance.   BASE=<spec file> bash phr.sh <tag> "<extra spec>"
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
CHORDA_PHRASES=middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top,triad "${EXE:-$CHORDA_EXE}" lab $V/lab4 "$1" "$spec" >/dev/null 2>&1
cd $V
echo "== $1: $2"
"$PY" -I $T/metric.py eng40/wav/salamander lab4/wav/$1 x | cut -c1-250
"$PY" $T/levels.py eng40/wav/salamander lab4/wav/$1 | tail -2
