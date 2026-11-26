#!/usr/bin/env bash
# Round 42 (chorda-c17): one trial — the single notes (pp/mf/ff) of `base=shipped` + <extra spec>
# through the lab, then the action's knock under the top keys' fundamentals and the noise between
# the middle's partials against both recorded grands. Renders kept in <lab>/c17/<tag>.
#   bash trial.sh <tag> "<extra spec>" [phrases: also render the listening phrases]
set -eu
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
tag=$1; extra=${2:-}; H=$(dirname "${BASH_SOURCE[0]}")
spec="base=shipped${extra:+,$extra}"
out=$VW/c17/$tag
CHORDA_PHRASES=notes_pp,notes_mf,notes_ff "$CHORDA_EXE" lab "$out" n "$spec" 2>/dev/null
if [ -n "${3:-}" ]; then CHORDA_PHRASES=middle_c,soft_chords,fast_run,ballad,ff_chord,bass,top,triad "$CHORDA_EXE" lab "$out" p "$spec" 2>/dev/null; fi
N=$VW/notes5/wav
for d in mf ff pp; do
  echo "== knock $d (top keys 99-108): dB re note"
  $PY -I "$H/knockenv.py" $d 99,102,105,108 sal=$N/salamander bk=$N/bitklavier ch=$out/wav/n | grep -v "^ *60-180\|^ *180-500" 
done
echo "== between the partials, C3-C5 mf"
$PY -I "$H/between.py" $N/salamander bitkl=$N/bitklavier "$tag=$out/wav/n" --keys 48-72 2>/dev/null | grep -v floor
