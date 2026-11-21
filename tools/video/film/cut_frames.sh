#!/usr/bin/env bash
# Both sides of every cut of a draft, and its end card, as one sheet to look at:
#   bash tools/video/film/cut_frames.sh <draft.mp4> <sheet.png>
# The frames 50 ms before and after each cut of the film (script.md's table), plus 1:14
# (the end card), each 480 pixels wide, four to a row.
set -euo pipefail
FF="${FFMPEG:-/f/projects/video-work/tools/ffmpeg/ffmpeg-n8.1.3-34-ga35157abb9-win64-gpl-8.1/bin/ffmpeg.exe}"
winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
in="$1"; out="$2"
dir="$(mktemp -d)"
TS=(5.95 6.05 13.95 14.05 23.95 24.05 30.95 31.05 43.95 44.05 46.6 46.7 49.3 49.4 51.95 52.05 59.95 60.05 67.95 68.05 74 79)
inputs=()
for t in "${TS[@]}"; do
  "$FF" -hide_banner -loglevel error -y -ss "$t" -i "$(winpath "$in")" -frames:v 1 -vf scale=480:-1 "$(winpath "$dir/$t.png")"
  inputs+=(-i "$(winpath "$dir/$t.png")")
done
"$FF" -hide_banner -loglevel error -y "${inputs[@]}" -filter_complex \
  "[0][1][2][3]hstack=4[a];[4][5][6][7]hstack=4[b];[8][9][10][11]hstack=4[c];[12][13][14][15]hstack=4[d];[16][17][18][19]hstack=4[e];[20][21]hstack=2,pad=1920:270[f];[a][b][c][d][e][f]vstack=6" \
  "$(winpath "$out")"
rm -rf "$dir"
echo "$out"
