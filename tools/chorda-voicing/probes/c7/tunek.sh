#!/usr/bin/env bash
# keys_mf of BASE+extra (keys lo..hi only measured) -> tune rows.   bash tunek.sh <tag> "<extra>" "k1 k2 ..."
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${CHORDA_PARTIALS:-$C16_PARTIALS}
export CHORDA_LEVELS=${CHORDA_LEVELS:-$C16_LEVELS}
spec="$(head -1 ${BASE:-$C16_SPEC})${2:+,$2}"
[ -n "${EMPTY:-}" ] && spec="$2"
CHORDA_PHRASES=keys_mf "${EXE:-$CHORDA_EXE}" lab $V/c7/keys "$1" "$spec" >/dev/null 2>&1
echo "== $1: $2"
for k in $3; do "$PY" -I $T/probes/c7/tune.py $V/c7/keys/wav/$1/keys_mf.wav $V/keys/wav/salamander/keys_mf.wav $k $k 2>/dev/null | sed -n 2p; done
