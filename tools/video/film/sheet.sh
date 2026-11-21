#!/usr/bin/env bash
# A contact sheet of a scene's frames, to look at them quickly:
#   bash tools/video/film/sheet.sh <scene-dir> <out.png> [step=10] [cols=4] [width=480]
# Every <step>th frame that exists (00000.png, then every step), tiled <cols> wide, each
# <width> pixels. No process per frame: on a loaded machine each fork is slow.
set -euo pipefail
dir="$1"; out="$2"; step="${3:-10}"; cols="${4:-4}"; w="${5:-480}"
FF="${FFMPEG:-/f/projects/video-work/tools/ffmpeg/ffmpeg-n8.1.3-34-ga35157abb9-win64-gpl-8.1/bin/ffmpeg.exe}"
winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
wdir="$(winpath "$dir")"
list="$dir.sheet.txt"
: > "$list"
n=0
i=0
while :; do
  printf -v f '%05d.png' "$i"
  [[ -f "$dir/$f" ]] || break
  printf "file '%s\\\\%s'\nduration 1\n" "$wdir" "$f" >> "$list"
  n=$((n + 1))
  i=$((i + step))
done
rows=$(( (n + cols - 1) / cols ))
"$FF" -hide_banner -loglevel error -y -f concat -safe 0 -i "$(winpath "$list")" \
  -vf "scale=$w:-1,tile=${cols}x${rows}:padding=4" -frames:v 1 "$(winpath "$out")"
rm -f "$list"
echo "$out ($n frames, every $step)"
