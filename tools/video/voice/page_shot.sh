#!/usr/bin/env bash
# A screenshot of a listening page (ab.html, ab-qwen.html, ab-eleven.html), to check its layout
# before the owner opens it, and that every audio link resolves to a file.
#
#   bash tools/video/voice/page_shot.sh <page.html> <shot.png> [height]
set -eu
page=${1:?page}; shot=${2:?shot}; height=${3:-3000}
dir=$(cd "$(dirname "$page")" && pwd)
missing=0
grep -o 'src="[^"]*"' "$page" | sed 's/src="//;s/"$//' | while read -r p; do
  [ -f "$dir/$p" ] || { echo "MISSING $p"; missing=1; }
done
echo "$(grep -c '<audio' "$page") players"
CHROME=${CHROME_PATH:-"/c/Program Files/Google/Chrome/Application/chrome.exe"}
win=$(cygpath -w "$dir/$(basename "$page")" 2>/dev/null || echo "$dir/$(basename "$page")")
out=$(cygpath -w "$shot" 2>/dev/null || echo "$shot")
"$CHROME" --headless=new --disable-gpu --hide-scrollbars --window-size=900,"$height" \
  --screenshot="$out" "file:///${win//\\//}" 2>/dev/null
echo "$shot"
