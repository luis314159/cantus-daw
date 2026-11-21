#!/usr/bin/env bash
# A contact sheet of a scene's frames, to look at them quickly:
#   bash tools/video/film/sheet.sh <scene-dir> <out.png> [step=10] [cols=4] [width=480]
# Every <step>th frame that exists, tiled <cols> wide, each <width> pixels, its number on it.
set -euo pipefail
dir="$1"; out="$2"; step="${3:-10}"; cols="${4:-4}"; w="${5:-480}"
FF="${FFMPEG:-/f/projects/video-work/tools/ffmpeg/ffmpeg-n8.1.3-34-ga35157abb9-win64-gpl-8.1/bin/ffmpeg.exe}"
winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
list="$(mktemp)"
n=0
for f in $(ls "$dir"/*.png | sort); do
  i=$((10#$(basename "$f" .png)))
  (( i % step == 0 )) || continue
  echo "file '$(winpath "$f")'" >> "$list"
  echo "duration 1" >> "$list"
  n=$((n + 1))
done
rows=$(( (n + cols - 1) / cols ))
"$FF" -hide_banner -loglevel error -y -f concat -safe 0 -i "$(winpath "$list")" \
  -vf "scale=$w:-1,drawtext=text='%{frame_num}':start_number=0:x=6:y=6:fontsize=18:fontcolor=yellow:box=1:boxcolor=black@0.6,tile=${cols}x${rows}:padding=4" \
  -frames:v 1 "$(winpath "$out")"
rm -f "$list"
echo "$out ($n frames, every $step)"
