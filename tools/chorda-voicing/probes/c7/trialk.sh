#!/usr/bin/env bash
# Candidate 7: render every key at mf (keys_mf) with the base spec + extra, the top keys' fundamentals.
#   BASE=<spec> EXE=<exe> bash trialk.sh <tag> "<extra spec>" [lo hi n]
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${CHORDA_PARTIALS:-$C16_PARTIALS}
export CHORDA_LEVELS=${CHORDA_LEVELS:-$C16_LEVELS}
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
CHORDA_PHRASES=keys_mf "${EXE:-$CHORDA_EXE}" lab $V/c7/keys "$1" "$spec" >/dev/null 2>&1
echo "== $1: $2"
"$PY" -I $T/probes/c7/fslope.py $V/c7/keys/wav/$1/keys_mf.wav $V/keys/wav/salamander/keys_mf.wav ${3:-84} ${4:-108} ${5:-1}
