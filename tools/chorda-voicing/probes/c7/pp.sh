#!/usr/bin/env bash
# Partial profile (levels re p1) C3-D#5 vs the Salamander on keys_mf.   bash pp.sh <tag> "<extra spec>"
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${CHORDA_PARTIALS:-$C16_PARTIALS}
export CHORDA_LEVELS=${CHORDA_LEVELS:-$C16_LEVELS}
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
CHORDA_PHRASES=keys_mf "${EXE:-$CHORDA_EXE}" lab $V/c7/keys "$1" "$spec" >/dev/null 2>&1
echo "$1 ($2): $("$PY" -I $T/probes/c7/pprof.py 48,51,54,57,60,63,66,69,72,75 $V/c7/keys/wav/$1 $V/keys/wav/salamander | tail -1)"
