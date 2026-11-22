#!/usr/bin/env bash
# The finished files in one unattended run, each heavy step waiting for room on the shared
# machine (gently.sh inside build.sh): optionally some scenes re-rendered at 4K/60 from the
# r41/video-int build, then the cards in both languages, every deliverable of both, and
# tools/video/final/checks.py's report.
#
#   bash tools/video/film/final_chain.sh [scene,scene…]      (e.g. s3-piano,s5-mixer)
#
# Logs and markers in F:\projects\video-work\final\logs\chain.log (each step's start and its
# exit), the checks' full output in checks.log.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
WORK=/f/projects/video-work
L="$WORK/final/logs"
FILM_ROOT="${FILM_ROOT:-/f/projects/daw-r41/video-int}"
mkdir -p "$L"
cd "$ROOT"
say() { echo "$(date +%T) $*" >> "$L/chain.log"; }
step() {
  local name="$1"; shift
  say "start $name"
  "$@" > "$L/chain-$name.log" 2>&1
  say "exit $? $name"
}
winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
if [[ -n "${1:-}" ]]; then
  step "scenes" bash tools/video/film/gently.sh "$L/g-scenes.log" "$FILM_ROOT/target/release/examples/film.exe" \
    --out "$(winpath "$WORK/film4k")" --fps 60 --scale 2 --scene "$1"
fi
for lang in en es; do
  step "cards-$lang" bash tools/video/film/gently.sh "$L/g-cards-$lang.log" "$(command -v bash)" tools/video/final/build.sh cards "$lang"
done
for lang in en es; do
  step "all-$lang" bash tools/video/final/build.sh all "$lang"
done
say "start checks"
py -3.12 -I tools/video/final/checks.py --work "$(winpath "$WORK")" --lang en es > "$L/checks.log" 2>&1
say "exit $? checks"
say "done"
