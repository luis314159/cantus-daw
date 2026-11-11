#!/usr/bin/env bash
# des-views (Slate): build every page and render every deliverable PNG with Edge headless.
# Run from anywhere: bash docs/round39/design/des-views/src/render_all.sh
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
D="$HERE/.."
PRIV="$HERE/../../../../../research/ui/round39/des-views"
mkdir -p "$PRIV"
cd "$HERE"
python -I sheets.py
r() { bash "$HERE/render.sh" "$@" >/dev/null; echo "  $5"; }
cut() { python "$HERE/crop.py" "$1" "$2" $3 $4 $5 $6 ${7:-4} >/dev/null; echo "  $2"; }
for th in night paper; do
  for s in 1 1.5; do
    sx=$([ "$s" = "1" ] && echo 1x || echo 1.5x)
    r pianoroll-$th 1280 720 $s "$D/pianoroll-$th-$sx.png"
    r rack-$th 1280 720 $s "$D/rack-browser-$th-$sx.png"
    r browser-$th 1280 720 $s "$D/browser-pages-$th-$sx.png"
    r clips-$th 1280 1350 $s "$D/clips-$th-$sx.png"
    r keys-$th 1280 560 $s "$D/keyboard-$th-$sx.png"
  done
  # 4x cuts of the 1:1 renders, nearest-neighbour: the real pixels
  P="$D/pianoroll-$th-1x.png"
  cut "$P" "$D/pianoroll-$th-4x-keys.png" 0 74 300 200
  cut "$P" "$D/pianoroll-$th-4x-notes.png" 440 230 300 180
  cut "$P" "$D/pianoroll-$th-4x-muted-ghosts.png" 960 160 300 200
  cut "$P" "$D/pianoroll-$th-4x-velocity.png" 0 586 320 112
  R="$D/rack-browser-$th-1x.png"
  cut "$R" "$D/rack-$th-4x-row.png" 290 96 300 180
  cut "$R" "$D/rack-$th-4x-steps.png" 580 96 320 180
  cut "$R" "$D/rack-$th-4x-rolls.png" 580 290 320 150
  cut "$R" "$D/browser-$th-4x-tree.png" 0 140 288 200
  cut "$R" "$D/browser-$th-4x-preview.png" 0 578 288 122
  B="$D/browser-pages-$th-1x.png"
  cut "$B" "$D/browser-$th-4x-search.png" 12 70 288 250
  cut "$B" "$D/browser-$th-4x-covers.png" 620 112 330 210
  C="$D/clips-$th-1x.png"
  cut "$C" "$D/clips-$th-4x-far.png" 176 84 300 270
  cut "$C" "$D/clips-$th-4x-default.png" 176 380 300 270
  cut "$C" "$D/clips-$th-4x-close.png" 176 972 300 268
  cut "$C" "$D/clips-$th-2x-states.png" 176 1268 1000 78 2
  K="$D/keyboard-$th-1x.png"
  cut "$K" "$D/keyboard-$th-4x-default.png" 290 100 160 300
  cut "$K" "$D/keyboard-$th-4x-compact.png" 16 100 160 200
  cut "$D/keyboard-$th-1.5x.png" "$D/keyboard-$th-1.5x-3x-default.png" 435 150 240 450 3
done
[ -f "$HERE/compare.py" ] && python "$HERE/compare.py" "$D" "$PRIV" || true
