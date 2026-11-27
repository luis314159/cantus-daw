#!/usr/bin/env bash
# Round 42 (chorda-presets): every character preset and its reference pianos through the real
# engine (chorda_compare render: the preset's values on a new Chorda, the recorded pianos as
# the picker installs them), the listening phrases and the single notes, into
# $CHORDA_LAB/presets/wav/<source>/. The presets' values are read from the editor's source
# (presets.py), so what is measured is what ships.
#   CHORDA_EXE=<chorda_compare built from this tree> bash render.sh [<preset or ref> …]
set -eu
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
D=${CHORDA_DOWNLOADS:-$CANTUS_REFS/piano}
OUT=$VW/presets
PH=${PH:-soft_chords,middle_c,bass,fast_run,ballad,notes_pp,notes_mf,notes_ff}
mapfile -t SRC < <("$PY" -I -X utf8 "$T/probes/presets/presets.py" sources "$@")
for s in "${SRC[@]}"; do
  CHORDA_PHRASES=$PH "$CHORDA_EXE" render "$D" "$OUT" "$s" &
  # Two at once: the machine is shared.
  while [ "$(jobs -rp | wc -l)" -ge 2 ]; do wait -n; done
done
wait
echo "presets rendered in $OUT"
