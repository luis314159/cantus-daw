#!/usr/bin/env bash
# des-mixer: build every page and render every deliverable PNG with Edge headless.
# bash docs/round39/design/des-mixer/src/render_all.sh [quick]
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
D="$HERE/.."
cd "$HERE"
python -I main.py
r() { bash "$HERE/render.sh" "$@" >/dev/null; echo "  $5"; }
for th in night paper; do
  r mixer-wide-$th 1280 720 1 "$D/mixer-wide-$th-1x.png"
  r mixer-wide-$th-idle 1280 720 1 "$D/mixer-wide-$th-idle-1x.png"
  r mixer-compact-$th 1280 720 1 "$D/mixer-compact-$th-1x.png"
  r mixer-large-$th 1280 720 1 "$D/mixer-large-$th-1x.png"
  r overflow-wide-$th 1280 720 1 "$D/overflow-wide-$th-1x.png"
  r overflow-es-$th 1280 720 1 "$D/overflow-es-$th-1x.png"
  r strips-$th 1280 800 1 "$D/strips-$th-1x.png"
  r redline-$th 1280 1040 1 "$D/redline-$th-1x.png"
  r states-$th 1280 1180 1 "$D/states-$th-1x.png"
  r inspector-$th 1280 770 1 "$D/inspector-$th-1x.png"
  if [ "$1" != quick ]; then
    r mixer-wide-$th 1280 720 1.5 "$D/mixer-wide-$th-1.5x.png"
    r mixer-compact-$th 1280 720 1.5 "$D/mixer-compact-$th-1.5x.png"
    r mixer-large-$th 1280 720 1.5 "$D/mixer-large-$th-1.5x.png"
    r overflow-es-$th 1280 720 1.5 "$D/overflow-es-$th-1.5x.png"
    r states-$th 1280 1180 1.5 "$D/states-$th-1.5x.png"
    r inspector-$th 1280 770 1.5 "$D/inspector-$th-1.5x.png"
    r strips-$th 1280 800 1.5 "$D/strips-$th-1.5x.png"
  fi
done
if [ "$1" != quick ]; then
  python "$HERE/zoom.py" "$D"
fi
python "$HERE/compare.py" "$D" "$HERE/../../../../../research/ui/round39/des-mixer"
