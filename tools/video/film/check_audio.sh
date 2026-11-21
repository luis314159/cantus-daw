#!/usr/bin/env bash
# The soundtrack and a draft measured by ffmpeg (the cross-check of Cantus's own meter):
# integrated loudness, loudness range and true peak (EBU R128, ebur128 peak=true), and
# loudnorm's own reading.
#   bash tools/video/film/check_audio.sh <file.wav|file.mp4>…
set -euo pipefail
FF="${FFMPEG:-/f/projects/video-work/tools/ffmpeg/ffmpeg-n8.1.3-34-ga35157abb9-win64-gpl-8.1/bin/ffmpeg.exe}"
winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
for f in "$@"; do
  echo "== $f"
  "$FF" -hide_banner -nostats -i "$(winpath "$f")" -af ebur128=peak=true -f null - 2>&1 \
    | sed -n '/Summary:/,$p' | grep -E "I:|LRA:|Peak:" | sed 's/^ */  ebur128 /'
  "$FF" -hide_banner -nostats -i "$(winpath "$f")" -af loudnorm=I=-14:TP=-1:print_format=json -f null - 2>&1 \
    | grep -E '"input_i"|"input_tp"' | sed 's/^ */  loudnorm /'
done
