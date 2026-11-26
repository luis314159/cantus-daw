#!/usr/bin/env bash
# Round 42 (chorda-c18): one quick trial of the top keys' mechanics — the single notes of
# candidate 17 (`candidates/c17/spec.txt`, over the unison pass) + "<extra>", then mech2.py's
# distance outside the two grands' span on the top keys (C7 ... C8) at <dyn> (default mf).
#   bash qt.sh <tag> "<extra spec>" [dyn] [keys]     renders kept in <lab>/c18/t/<tag>
set -eu
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
tag=$1; extra=${2:-}; dyn=${3:-mf}; keys=${4:-96,99,102,105,108}; H=$(dirname "${BASH_SOURCE[0]}")
spec="$(cat "$T/candidates/c17/spec.txt")${extra:+,$extra}"
out=$VW/c18/t/$tag
[ -f "$out/wav/n/notes_$dyn.wav" ] || CHORDA_PHRASES=notes_pp,notes_mf,notes_ff "$CHORDA_EXE" lab "$out" n "$spec" >/dev/null 2>&1
N=$VW/notes5/wav
"$PY" -I "$H/mech2.py" sal=$N/salamander bk=$N/bitklavier "$tag=$out/wav/n" --dyn "$dyn" --keys "$keys" 2>/dev/null \
  | sed -n '/distance outside/,$p'
