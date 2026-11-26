#!/usr/bin/env bash
# Round 42 (chorda-c17): a quick trial over candidate 17 — the top phrase and the single notes of
# `base=unison` + <base spec file> + <extra>, then quick17.py.   bash qtrial.sh <tag> <spec file> "<extra>"
set -eu
. "$(dirname "${BASH_SOURCE[0]}")/../../env.sh"
tag=$1; base=$(cat "$2"); extra=${3:-}; H=$(dirname "${BASH_SOURCE[0]}")
spec="base=unison,$base${extra:+,$extra}"
CHORDA_PHRASES=top,middle_c,fast_run "$CHORDA_EXE" lab "$VW/c17q" "$tag" "$spec" >/dev/null 2>&1 &
CHORDA_PHRASES=notes_pp,notes_mf,notes_ff "$CHORDA_EXE" lab "$VW/c17q/n" "$tag" "$spec" >/dev/null 2>&1
wait
echo "== $tag: $extra"
"$PY" -I "$H/quick17.py" "$VW/c17q/wav/$tag" "$VW/c17q/n/wav/$tag" 2>/dev/null | grep -v "^ *sal\|^ *bk\|^ *top45"
