#!/usr/bin/env bash
# Per-key draw search with chord beats: DRAWS="1 2 3" K=<MIDI key> NM=<note name, e.g. D3> CH=<chord keys, e.g. 43,50,57,62>
#   bash dk.sh — keys_mf on BASE + draw.K=d for each d, then drawq, the chord's beats and the tuning row.
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
for d in $DRAWS; do
  CHORDA_PARTIALS=${PARTIALS:-$C16_PARTIALS} CHORDA_LEVELS=${LEVELS:-$C16_LEVELS} CHORDA_PHRASES=keys_mf "$CHORDA_EXE" lab $VW/c16/keys k$K$d "$(head -1 ${BASE:-$C16_SPEC}),draw.$K=$d" >/dev/null 2>&1
  W=$VW/c16/keys/wav/k$K$d/keys_mf.wav
  echo "== draw.$K=$d"
  $PY -I $T/probes/c11/drawq.py $W $K
  $PY -I $T/probes/c7/chordbeat.py $W $VW/keys/wav/salamander/keys_mf.wav $CH 2>&1 | grep -E "beat cand|^$NM "
  $PY -I $T/probes/c16/tunek.py $W
  rm -f $W
done
