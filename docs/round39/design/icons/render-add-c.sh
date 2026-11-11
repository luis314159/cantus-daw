#!/bin/sh
# Render set 6 additions C in both themes, 1:1 and 4x (nearest neighbour).
#   sh render-add-c.sh                      -> set6-add-c-{night,paper}-{1x,4x}.png
#   sh render-add-c.sh "&fam=2" out/prefix  -> one family, elsewhere
cd "$(dirname "$0")"
for th in night paper; do
  python render.py sheet-add-c.html "t=$th$1" "${2:-set6-add-c}-$th-1x.png" --x4 "${2:-set6-add-c}-$th-4x.png"
done
