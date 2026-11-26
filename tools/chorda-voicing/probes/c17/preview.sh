#!/usr/bin/env bash
# Round 42 (chorda-c17): the owner's named previews — the seven phrases from candidate 16 as it
# sounds today (the shipped voicing: candidate 16 with the technician's and the unison passes),
# candidate 17 and the Salamander, one loudness per phrase (preview.py), 16-bit WAV copies and
# escuchar.html (listen_page.py, notes in Spanish from <notes.json>).
#   bash preview.sh <candidate tag in eng40/wav> <out dir> <notes.json>
set -eu
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
tag=$1; out=$2; notes=$3
E=$VW/eng40/wav
"$PY" -I "$T/preview.py" "$out" "1 candidato 16=$E/shipped" "2 candidato 17=$E/$tag" "3 Salamander=$E/salamander"
"$PY" -I "$T/listen_page.py" "$out" "$notes"
