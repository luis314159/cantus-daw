#!/usr/bin/env bash
# The lab's reference renders, made once per lab (and again after a change to the engine's
# render path or to the phrases): the Salamander Grand through the engine (the picker's
# install, Exemplar), beta.14's Chorda through the lab, and the first analysis's slot 2.
#   bash references.sh <downloads dir>
# <downloads dir> receives the Salamander (CC BY 3.0, Alexander Holm; ~1 GB, fetched by
# Cantus's own pack code into <downloads>/content — `chorda_compare install`).
# Writes into the lab (CHORDA_LAB): eng40/wav/{salamander,chorda2}, notes5/wav/{salamander,b14},
# keys/wav/salamander, c7/vs/wav/salamander, lab4/wav/candB.
set -eu
. "$(dirname "${BASH_SOURCE[0]}")/env.sh"
D=${1:?usage: references.sh <downloads dir>}
PH=middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top,triad
NOTES=notes_pp,notes_mf,notes_ff
"$CHORDA_EXE" install "$D"
CHORDA_PHRASES=$PH "$CHORDA_EXE" render "$D" "$VW/eng40" salamander
CHORDA_PHRASES=$NOTES "$CHORDA_EXE" render "$D" "$VW/notes5" salamander
CHORDA_PHRASES=keys_mf "$CHORDA_EXE" render "$D" "$VW/keys" salamander
CHORDA_PHRASES=vsweep "$CHORDA_EXE" render "$D" "$VW/c7/vs" salamander
# beta.14 (what shipped until DSP_VERSION 7): the lab's empty spec, without the tables. The
# round-40 lab rendered `chorda2` through the engine while beta.14 was its default; since
# candidate 16 shipped, the engine's chorda2 is c16 — the lab's "" is beta.14.
unset CHORDA_PARTIALS CHORDA_LEVELS
CHORDA_PHRASES=$PH "$CHORDA_EXE" lab "$VW/eng40" chorda2 ""
CHORDA_PHRASES=$NOTES "$CHORDA_EXE" lab "$VW/notes5" b14 ""
# The first analysis compares four sources; its slot 2 was candidate 1 (tag candB).
CHORDA_PHRASES=$PH "$CHORDA_EXE" lab "$VW/lab4" candB "$(head -1 "$T/candidates/candB/spec.txt")"
echo "references done in $VW"
