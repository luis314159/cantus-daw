#!/usr/bin/env bash
# Renders the direction mock: two directions x desktop 1440 / phone 390 x dark / light.
# Edge headless; a first pass reads the page's height (body[data-h]), the second shoots it.
# The phone is laid out in a 390-px frame (phone.html): Edge will not open a narrower window
# than about 500 px, so the shot is cropped to the frame afterwards (Pillow).
# Usage (Git Bash): bash docs/round39/web/src/render.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
out="$here/.."
edge="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
cat "$here/part1.html" "$here/part2.html" > "$here/direction.html"
base="file:///$(cygpath -m "$here")"
for dir in a b; do
  for theme in dark light; do
    for dev in desktop mobile; do
      q="?dir=$dir&theme=$theme"
      if [ "$dev" = desktop ]; then w=1440; scale=1; url="$base/direction.html$q"
      else w=520; scale=2; url="$base/phone.html$q"; fi
      flags=(--headless=new --disable-gpu --hide-scrollbars --allow-file-access-from-files --virtual-time-budget=5000)
      h=$("$edge" "${flags[@]}" --window-size=$w,900 --dump-dom "$url" 2>/dev/null \
          | grep -o '<body[^>]*data-h="[0-9]*"' | grep -o 'data-h="[0-9]*"' | grep -o '[0-9]*' || true)
      h=${h:-6000}
      png="$(cygpath -m "$out/direction-$dir-$dev-$theme.png")"
      "$edge" "${flags[@]}" --disable-lcd-text --force-device-scale-factor=$scale \
          --window-size=$w,$h --screenshot="$png" "$url" >/dev/null 2>&1
      if [ "$dev" = mobile ]; then
        python -c "import sys; from PIL import Image; p=sys.argv[1]; im=Image.open(p); im.crop((0,0,780,im.size[1])).save(p)" "$png"
      fi
      echo "$png ${w}x$h @${scale}x"
    done
  done
done
