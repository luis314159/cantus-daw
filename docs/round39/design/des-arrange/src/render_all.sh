#!/usr/bin/env bash
# des-arrange: build every page and render every PNG with Edge headless (greyscale text, like
# the app's), then the 4x cuts and the private comparisons.
# usage: bash render_all.sh [name-filter]
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
D="$(cd "$HERE/.." && pwd)"
cd "$HERE"
python -I main.py "$1"
while read -r name w h s png; do
  bash "$HERE/render.sh" "$name" "$w" "$h" "$s" "$D/$png" >/dev/null </dev/null
  echo "  $png"
done < "$D/build/pages.txt"
