#!/usr/bin/env bash
# Draw search: for each draw d in $DRAWS, render keys_mf with draw.<k>=d for every key in $KEYS
# (on top of BASE, default candidate 16; round 40: candidate 12), measure those keys (drawq.py), delete the render.
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${PARTIALS:-$C16_PARTIALS} CHORDA_LEVELS=${LEVELS:-$C16_LEVELS}
base="$(head -1 ${BASE:-$C16_SPEC})"
for d in $DRAWS; do
  extra=""; for k in ${KEYS//,/ }; do extra="$extra,draw.$k=$d"; done
  CHORDA_PHRASES=keys_mf "${EXE:-$CHORDA_EXE}" lab $V/c11/ds ds$d "$base$extra" >/dev/null 2>&1
  sz=$(stat -c %s $V/c11/ds/wav/ds$d/keys_mf.wav)
  echo "== draw $d (wav $sz)"
  $PY -I $T/probes/c11/drawq.py $V/c11/ds/wav/ds$d/keys_mf.wav $KEYS
  rm -f $V/c11/ds/wav/ds$d/keys_mf.wav
done
