#!/usr/bin/env bash
# Partial frequencies (cents vs the reference) of keys lo..hi on keys_mf.
#   bash pfq.sh <tag> "<extra spec>" [lo 48] [hi 76]
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${CHORDA_PARTIALS:-$C16_PARTIALS}
export CHORDA_LEVELS=${CHORDA_LEVELS:-$C16_LEVELS}
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
CHORDA_PHRASES=keys_mf "${EXE:-$CHORDA_EXE}" lab $V/c7/keys "$1" "$spec" >/dev/null 2>&1
echo "$1 ($2): $("$PY" -I $T/probes/c7/pfreq.py $V/c7/keys/wav/$1/keys_mf.wav ${3:-48} ${4:-76})"
