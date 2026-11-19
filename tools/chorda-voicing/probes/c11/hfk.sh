#!/usr/bin/env bash
# hf_key.60 sweep: HS="0.5 0.65 0.8" bash hfk.sh — keys_mf on BASE + hf_key.60=h, then middle C's drawq row.
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
export CHORDA_PARTIALS=${PARTIALS:-$C16_PARTIALS} CHORDA_LEVELS=${LEVELS:-$C16_LEVELS}
for h in $HS; do
  CHORDA_PHRASES=keys_mf "$CHORDA_EXE" lab $V/c11/ds hk$h "$(head -1 ${BASE:-$C16_SPEC}),hf_key.60=$h" >/dev/null 2>&1
  echo "== hf_key.60=$h"; $PY -I $T/probes/c11/drawq.py $V/c11/ds/wav/hk$h/keys_mf.wav 60
  rm -f $V/c11/ds/wav/hk$h/keys_mf.wav
done
