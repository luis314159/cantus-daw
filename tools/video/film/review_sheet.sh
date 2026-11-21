#!/usr/bin/env bash
# A contact sheet of chosen frames of a finished cut, to look at with one image: the frames a
# reviewer named (the defaults are one or two per scene of the 30 fps cut, the end card last).
#
#   bash tools/video/film/review_sheet.sh <cut.mp4> <sheet.png> [frame numbers...]
#
# FFMPEG_BIN names ffmpeg's folder (tools/video/film/ffmpeg.txt has the pinned build).
set -eu
cut=${1:?cut}; sheet=${2:?sheet}; shift 2
frames=${*:-60 700 1360 1500 1600 1700 1900 2100 2300}
FF=${FFMPEG_BIN:-/f/projects/video-work/tools/ffmpeg/ffmpeg-n8.1.3-34-ga35157abb9-win64-gpl-8.1/bin}
sel=$(for f in $frames; do printf 'eq(n\\,%s)+' "$f"; done)
n=$(echo $frames | wc -w)
cols=3; rows=$(( (n + cols - 1) / cols ))
"$FF/ffmpeg" -v error -y -i "$cut" -vf "select='${sel%+}',scale=640:-1,tile=${cols}x${rows}" -frames:v 1 "$sheet"
echo "$sheet"
