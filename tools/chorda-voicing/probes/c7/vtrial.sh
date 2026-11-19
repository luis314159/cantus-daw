#!/usr/bin/env bash
# vsweep of base+extra -> vsweep.py table vs the Salamander.  bash vtrial.sh <tag> "<extra>"
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${CHORDA_PARTIALS:-$C16_PARTIALS}
export CHORDA_LEVELS=${CHORDA_LEVELS:-$C16_LEVELS}
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
CHORDA_PHRASES=vsweep "${EXE:-$CHORDA_EXE}" lab $V/c7/vs "$1" "$spec" >/dev/null 2>&1
echo "== $1: $2"
"$PY" -I $T/probes/c7/vsweep.py $V/c7/vs/wav/$1/vsweep.wav $V/c7/vs/wav/salamander/vsweep.wav | tail -12
