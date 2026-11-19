#!/usr/bin/env bash
# bash ks.sh <tag> "<extra>": keys_mf on BASE (default candidate 16; +extra) -> tuning worst keys, flanger outliers A0-C5, drawq for $DK; deletes render
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
CHORDA_PARTIALS=${PARTIALS:-$C16_PARTIALS} CHORDA_LEVELS=${LEVELS:-$C16_LEVELS} CHORDA_PHRASES=keys_mf "$CHORDA_EXE" lab $VW/c16/keys "$1" "$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}" >/dev/null 2>&1
W=$VW/c16/keys/wav/$1/keys_mf.wav
echo "== $1 ($2)"
$PY -I $T/probes/c16/tunek.py $W
$PY -I $T/probes/c11/flank.py $W - 21 72 | grep outliers
[ -n "${DK:-}" ] && $PY -I $T/probes/c11/drawq.py $W $DK
[ -n "${KEEP:-}" ] || rm -f $W
