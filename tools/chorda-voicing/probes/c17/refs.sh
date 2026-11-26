#!/usr/bin/env bash
# Round 42 (chorda-c17): the references this pass measures against, once per lab — the
# Salamander (references.sh) and a second recorded grand, the bitKlavier Grand (Princeton;
# CC BY 4.0), through the engine as the picker installs it (stage lid ORTF), the listening
# phrases and the single notes; and the shipped voicing (`base=shipped`: candidate 16 with the
# technician's and the unison passes, what a new Chorda plays) through the lab.
#   bash refs.sh            (CHORDA_EXE: a lab build with the `base=` and `pack:` sources)
set -eu
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
D=${CHORDA_DOWNLOADS:-$CANTUS_REFS/piano}
PH=middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top,triad
NOTES=notes_pp,notes_mf,notes_ff
BK=bitklavier=pack:bitklavier-grand-stage-lid-ortf:grand
CHORDA_PHRASES=$PH "$CHORDA_EXE" render "$D" "$VW/eng40" salamander "$BK" &
CHORDA_PHRASES=$NOTES "$CHORDA_EXE" render "$D" "$VW/notes5" salamander "$BK" &
CHORDA_PHRASES=keys_mf "$CHORDA_EXE" render "$D" "$VW/keys" salamander &
wait
unset CHORDA_PARTIALS CHORDA_LEVELS
CHORDA_PHRASES=$PH "$CHORDA_EXE" lab "$VW/eng40" shipped "base=shipped"
CHORDA_PHRASES=$NOTES "$CHORDA_EXE" lab "$VW/notes5" shipped "base=shipped"
echo "c17 references done in $VW"
