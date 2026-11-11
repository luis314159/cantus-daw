#!/usr/bin/env bash
# Slate (dir-a): build the pages and render them with Edge headless.
# usage: render.sh <page-name-without-.html> <css-width> <css-height> <scale> <out.png>
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
EDGE="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
name="$1"; w="$2"; h="$3"; s="$4"; out="$5"
page="$(cd "$HERE/../build" && pwd -W)/$name.html"
"$EDGE" --headless=new --disable-gpu --disable-lcd-text --hide-scrollbars --force-device-scale-factor="$s" \
  --window-size="$w,$h" --screenshot="$(cygpath -w "$out")" "file:///${page}" >/dev/null 2>&1
echo "$out"
