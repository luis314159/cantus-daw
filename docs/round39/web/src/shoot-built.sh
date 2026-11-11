#!/usr/bin/env bash
# Screenshots the BUILT site (web/dist, from `trunk build --release`) with Edge headless:
# desktop 1440 and phone 390, dark and light, English, plus two in Spanish. The phone is drawn
# at 2x and stored at 1x (390 px wide), to keep the repository light.
# Serves dist on 127.0.0.1:8765 (Python), opens it through shot.html, which sets the theme
# and the language and reports the page's height and anything wider than the frame.
# Usage (Git Bash): bash docs/round39/web/src/shoot-built.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../../.." && pwd)"
out="$here/.."
dist="$repo/web/dist"
edge="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
cp "$here/shot.html" "$dist/_shot.html"
python -m http.server 8765 --bind 127.0.0.1 --directory "$(cygpath -w "$dist")" >/dev/null 2>&1 &
srv=$!
trap 'kill $srv 2>/dev/null; rm -f "$dist/_shot.html"' EXIT
sleep 1
shoot() { # name width theme lang
  local name=$1 w=$2 theme=$3 lang=$4 win scale
  if [ "$w" -lt 500 ]; then win=520; scale=2; else win=$w; scale=1; fi
  local url="http://127.0.0.1:8765/_shot.html?w=$w&theme=$theme&lang=$lang"
  local flags=(--headless=new --disable-gpu --hide-scrollbars --virtual-time-budget=15000)
  local dom h wide
  dom=$("$edge" "${flags[@]}" --window-size=$win,900 --dump-dom "$url" 2>/dev/null || true)
  h=$(printf '%s' "$dom" | grep -o '<body[^>]*' | grep -o 'data-h="[0-9]*"' | grep -o '[0-9]*' || true)
  wide=$(printf '%s' "$dom" | grep -o '<body[^>]*' | grep -o 'data-wide="[^"]*"' || true)
  h=${h:-8000}
  local png
  png="$(cygpath -m "$out/built-$name.png")"
  "$edge" "${flags[@]}" --disable-lcd-text --force-device-scale-factor=$scale \
      --window-size=$win,$h --screenshot="$png" "$url" >/dev/null 2>&1
  # Kept as JPEG (quality 82): a whole page is 26 000 px and more, past WebP's 16 383, and
  # as PNG the six came to 39 MB. The PNG Edge wrote is a scratch file.
  python -c "import sys,os; from PIL import Image; p,w,s=sys.argv[1],int(sys.argv[2]),int(sys.argv[3]); im=Image.open(p).convert('RGB'); im=im.crop((0,0,w*s,im.size[1])); im=im.resize((w,im.size[1]//s),Image.LANCZOS) if s>1 else im; im.save(p[:-4]+'.jpg',quality=82,optimize=True,progressive=True); os.remove(p)" "$png" "$w" "$scale"
  echo "${png%.png}.jpg ${w}x$h @${scale}x $wide"
}
shoot desktop-dark 1440 dark en
shoot desktop-light 1440 light en
shoot mobile-dark 390 dark en
shoot mobile-light 390 light en
shoot es-desktop-dark 1440 dark es
shoot es-mobile-light 390 light es
