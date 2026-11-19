#!/usr/bin/env bash
# Middle C's partials 8-9 at 0 / 1.5 s and the C3-D#5 partial profile vs the Salamander.
#   bash p89.sh <tag> "<extra spec>"   (BASE / PARTIALS / LEVELS / EXE as the other probes)
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${PARTIALS:-$C16_PARTIALS} CHORDA_LEVELS=${LEVELS:-$C16_LEVELS}
CHORDA_PHRASES=keys_mf "${EXE:-$CHORDA_EXE}" lab $V/c7/keys "$1" "$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}" >/dev/null 2>&1
echo "== $1 ($2)"; $PY -I $T/probes/c7/p89.py $V/c7/keys/wav/$1/keys_mf.wav
$PY -I $T/probes/c7/pprof.py 48,51,54,57,60,63,66,69,72,75 $V/c7/keys/wav/$1 $V/keys/wav/salamander | tail -1
