#!/usr/bin/env bash
# Slate (dir-a): build every page and render every deliverable PNG (phase A and phase B).
# Run from anywhere: bash docs/round39/design/dir-a/src/render_all.sh
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
D="$HERE/.."
B="$D/b"
PRIV="$HERE/../../../../../research/ui/round39/dir-a"
mkdir -p "$PRIV" "$B"
cd "$HERE"
python -I main.py
python -I parts.py
python -I system.py
python -I dmain.py
r() { bash "$HERE/render.sh" "$@" >/dev/null; echo "  $5"; }
for th in night paper; do
  r window-$th 1280 720 1 "$D/window-$th-1x.png"
  r window-$th@15 1280 720 1.5 "$D/window-$th-1.5x.png"
  r window-$th-playing 1280 720 1 "$D/window-$th-playing-1x.png"
  r overflow-$th 1280 720 1 "$D/overflow-$th-1x.png"
  r overflow-$th@15 1280 720 1.5 "$D/overflow-$th-1.5x.png"
  r parts-$th 1260 970 1 "$D/parts-$th-1x.png"
  python "$HERE/zoom.py" "$D/parts-$th-1x.png" "$D/parts-$th" >/dev/null
  for dl in settings-audio settings-look firstrun export export-progress; do
    r dlg-$dl-$th 1280 720 1 "$B/dlg-$dl-$th-1x.png"
    r dlg-$dl-$th@15 1280 720 1.5 "$B/dlg-$dl-$th-1.5x.png"
  done
done
r system 1260 1240 1 "$D/system.png"
python -I sheets_b.py | while read name w h; do
  case "$name" in
    *@15) r "$name" "$w" "$h" 1.5 "$B/${name%@15}-1.5x.png" ;;
    *) r "$name" "$w" "$h" 1 "$B/$name-1x.png" ;;
  esac
done
python "$HERE/zoom_b.py" "$B"
python "$HERE/compare.py" "$D" "$PRIV"
python "$HERE/compare_b.py" "$B" "$PRIV"
