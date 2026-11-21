#!/usr/bin/env bash
# How far two renders of the Heart differ over a span (draft 3: draft 2's DSP-10 render
# against integration's DSP-11 one, over the intro, where Chorda plays alone): each one's
# RMS, and the RMS of their difference (one minus the other, sample for sample).
#   bash tools/video/film/heart_diff.sh <a.wav> <b.wav> [from=0] [to=16.55]
set -euo pipefail
FF="${FFMPEG:-/f/projects/video-work/tools/ffmpeg/ffmpeg-n8.1.3-34-ga35157abb9-win64-gpl-8.1/bin/ffmpeg.exe}"
winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
a="$1"; b="$2"; from="${3:-0}"; to="${4:-16.55}"
rms() {
  "$FF" -hide_banner -nostats -i "$(winpath "$1")" -af "atrim=$from:$to,astats=measure_overall=RMS_level:measure_perchannel=none" -f null - 2>&1 \
    | grep -m1 "RMS level" | sed 's/.*RMS level dB: //'
}
echo "a: $(rms "$a") dBFS RMS ($a)"
echo "b: $(rms "$b") dBFS RMS ($b)"
diff=$("$FF" -hide_banner -nostats -i "$(winpath "$a")" -i "$(winpath "$b")" -filter_complex \
  "[0:a]atrim=$from:$to,asetpts=PTS-STARTPTS[x];[1:a]atrim=$from:$to,asetpts=PTS-STARTPTS,volume=-1[y];[x][y]amix=inputs=2:normalize=0,astats=measure_overall=RMS_level:measure_perchannel=none" \
  -f null - 2>&1 | grep -m1 "RMS level" | sed 's/.*RMS level dB: //')
echo "a - b: $diff dBFS RMS, from $from s to $to s"
