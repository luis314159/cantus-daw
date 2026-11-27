#!/usr/bin/env bash
# r42:arcus-hands, pass 6 — the left hand at ~400 % from the app's own camera (hand_cam.py,
# half 8, --scales=2) for both violinists in first (B4: A string, 2 semitones), third (E5: 7)
# and high position (A5: 12), Photographic and Drawn on Night and Photographic on Paper; then
# per figure and look a sheet with a before folder beside (rows first/third/high), and the
# photographs (study only: keep the sheets outside the repository).
#
#   bash tools/arcus-figures/hand_shots_400.sh <shoot_arcus.exe> <dump.json> <out dir> [<before dir>] [<refs dir>]
#
# CANTUS_FIGURE_DIR, when set, is the figures (and left_hand.bin) the shots are taken with.
set -eu
exe=$1; dump=$2; out=$3; before=${4:-}; refs=${5:-}
PY=${PY:-python}
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out"
cd "$here/../../crates/cantus-ui"
for who in woman man; do
  for pos in first:violin-play:2:--note=2,71 third:violin-play:7: high:violin-high:12:; do
    IFS=: read -r name state semis note <<< "$pos"
    cam=$("$PY" "$here/hand_cam.py" "$dump" "$who" violin 2 "$semis" 8)
    d="$out/$who-$name"
    mkdir -p "$d"
    "$exe" "$d" --hold="$who" --only="$state+$who" --cam="$cam" --scales=2 $note > "$d/shoot.log" 2>&1
  done
done
[ -n "$before" ] || exit 0
# The picture's own area of the 200 % editor shot.
box=@16,122,1198,980
for who in woman man; do
  for look in photo-night drawn-night photo-paper; do
    args=()
    for name in first third high; do
      st=violin-play; [ "$name" = high ] && st=violin-high
      f="arcus-$st-$who-$look-en-200.png"
      args+=("$before/$who-$name/$f$box=before ~400% $name, $look" "$out/$who-$name/$f$box=after ~400% $name, $look")
      if [ -n "$refs" ]; then
        case "$name" in first) r=crop-08.jpg ;; third) r=crop-02.jpg ;; high) r=crop-02.jpg ;; esac
        [ "$who" = man ] && r=crop-11.jpg
        args+=("$refs/$r=photograph $r (study)")
      fi
      "$PY" "$here/nup.py" "$out/row-$who-$look-$name.png" 600 "${args[@]}"
      args=()
    done
    "$PY" - "$out/$who-left-hand-400-$look.png" "$out/row-$who-$look-"{first,third,high}.png <<'XEOF'
import sys
from PIL import Image
ims = [Image.open(p) for p in sys.argv[2:]]
w = max(i.width for i in ims)
out = Image.new("RGB", (w, sum(i.height for i in ims)), (20, 20, 24))
y = 0
for i in ims:
    out.paste(i, (0, y))
    y += i.height
out.save(sys.argv[1])
XEOF
  done
done
