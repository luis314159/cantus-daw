#!/usr/bin/env bash
# The launch video's finished files (round 41, the plan's step 6), from the 3840 x 2160,
# 60 fps picture and the soundtrack mixed in Cantus (draft 3's, approved by the owner on
# 2026-11-21: "Los videos me gustan").
#
#   bash tools/video/final/build.sh <step> [en|es]        DRY=1 prints the commands only
#
#   render         the film's scenes at 3840 x 2160, 60 fps ($F/<scene>/), both languages share them
#   cards <lang>   the cards at scale 2, 60 fps ($F/cards/<lang>/)
#   captions       the SRTs and chapters of both languages (final/captions.py)
#   master <lang>  cantus-<lang>-master.mkv: 2160p60 FFV1 (lossless) + 48 kHz 24-bit PCM, chapters
#   youtube <lang> cantus-<lang>-2160p.mp4: H.264 High, CRF 16, 60 fps, AAC 320 k, chapters
#   site <lang>    cantus-<lang>-1080p.mp4 and .webm (H.264 / VP9, two-pass, at most 12 MB each),
#                  captions burned in, chapters; cantus-poster.jpg
#   loop           cantus-loop.mp4 and .webm: 18 s of the mixer (0:31-0:49), muted, no captions
#   vertical <lang> cantus-<lang>-vertical.mp4: 1080 x 1920, the first 30 s, captions burned in
#   all <lang>     every step for the language, in order (render and cards first)
#
# Heavy steps (render, master, youtube, site, vertical) run through film/gently.sh, at
# below-normal priority, only while the shared machine has RAM and C: to spare. Never while a
# suite's timing tests or a CPU measurement run (the integrator's word).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
WORK="${WORK:-/f/projects/video-work}"
F="$WORK/film4k"                    # the 4K picture: scenes and cards
S="$WORK/soundtrack3"               # draft 3's soundtrack (DSP 11, the carve)
O="$WORK/final"                     # the finished files, the captions, the chapters
FF="${FFMPEG:-$WORK/tools/ffmpeg/ffmpeg-n8.1.3-34-ga35157abb9-win64-gpl-8.1/bin/ffmpeg.exe}"
PY="${PY:-py -3.12}"
step="${1:?step}"; lang="${2:-en}"
SCENES=(s1-open s2-window s3-piano s4-drums s5-mixer s6a-record s6b-session s6c-plugin s7-master s8-desk s9-end)
# The cards and their times in the cut (as draft.sh; end card B, the owner's choice).
CARD=(title count-instruments count-effects lower-record lower-session lower-plugins free end-b)
declare -A AT=([title]=0 [count-instruments]=15.5 [count-effects]=33.5 [lower-record]=44.0 [lower-session]=46.667 [lower-plugins]=49.333 [free]=62.5 [end-b]=68.0)
POSTER_T=3.5        # the title card drawn, the song playing: the same in both languages
LOOP_FROM=31.0; LOOP_S=18.0
VERT_S=30.0
SITE_MB=12

winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
run() { if [[ -n "${DRY:-}" ]]; then printf '%q ' "$@"; echo; else "$@"; fi; }
# A heavy job through gently.sh. Its command goes in a script file first: gently.sh hands its
# arguments to PowerShell in single quotes, which ffmpeg's filter strings carry themselves.
heavy() {
  local log="$1"; shift
  local cmd="$log.cmd.sh"
  { echo '#!/usr/bin/env bash'; printf '%q ' "$@"; echo; } > "$cmd"
  run bash "$ROOT/tools/video/film/gently.sh" "$log" "$(command -v bash)" "$(winpath "$cmd")"
}
calc() { awk "BEGIN { print $1 }"; }
mkdir -p "$O" "$O/logs"
cd "$ROOT"

# The site's captions burned in: Barlow Semi Condensed (the site's type), white on a
# translucent slate box, low in the frame (looked at on a 1080p frame, 2026-11-21). Sizes are
# libass's script units (288 to the frame's height): 13 is about 49 px at 1080.
subs_filter() {
  local srt; srt="$(winpath "$O/cantus-$1.srt")"; srt="${srt//\\//}"; srt="${srt/:/\\:}"
  local fonts; fonts="$(winpath "$ROOT/web/assets/fonts")"; fonts="${fonts//\\//}"; fonts="${fonts/:/\\:}"
  echo "subtitles='$srt':fontsdir='$fonts':force_style='FontName=Barlow Semi Condensed,FontSize=$2,PrimaryColour=&H00F1F4F7,OutlineColour=&H40181D22,BorderStyle=3,Outline=5,Shadow=0,MarginV=$3'"
}

case "$step" in
render)
  # The release's app (integration + r41/video): built and run in the r41/video-int worktree
  # (film/int_worktree.sh), in its own target dir — never another worktree's.
  FILM_ROOT="${FILM_ROOT:-/f/projects/daw-r41/video-int}"
  export PATH="$HOME/.cargo/bin:$PATH" CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
  (cd "$FILM_ROOT" && run bash tools/build/slot.sh cargo build -j 2 -p cantus-test-clap --target-dir target/test-plugins)
  (cd "$FILM_ROOT" && run bash tools/build/slot.sh cargo build --release -j 2 -p cantus-ui --example film)
  heavy "$O/logs/render.log" "$FILM_ROOT/target/release/examples/film.exe" --out "$(winpath "$F")" --fps 60 --scale 2
  ;;
cards)
  run node tools/video/cards/render.mjs "$(winpath "$F/cards")" --lang "$lang" --scale 2 --fps 60 --os preview
  ;;
captions)
  run $PY -I tools/video/final/captions.py --chosen "$WORK/voice/chosen" --out "$O"
  ;;
master)
  # Each scene losslessly, then the cards over them at their times, the master WAV under.
  mkdir -p "$F/seg"
  for s in "${SCENES[@]}"; do
    heavy "$O/logs/seg-$s.log" "$FF" -hide_banner -loglevel error -y -framerate 60 -i "$(winpath "$F/$s")/%05d.png" \
      -c:v ffv1 -level 3 -slices 16 -pix_fmt yuv444p -an "$(winpath "$F/seg/$s.mkv")"
  done
  : > "$F/seg/list.txt"; for s in "${SCENES[@]}"; do echo "file '$s.mkv'" >> "$F/seg/list.txt"; done
  inputs=(-f concat -safe 0 -i "$(winpath "$F/seg/list.txt")" -i "$(winpath "$S/soundtrack-$lang.wav")" -i "$(winpath "$O/chapters-$lang.ffmeta")")
  graph="[0:v]fade=t=in:st=0:d=1.0,fade=t=out:st=79.0:d=1.0[v0];"; last=v0; n=3
  for c in "${CARD[@]}"; do
    inputs+=(-framerate 60 -i "$(winpath "$F/cards/$lang/$c")/%05d.png")
    graph+="[$n:v]format=rgba,setpts=PTS+${AT[$c]}/TB[c$n];[$last][c$n]overlay=eof_action=pass:format=auto[v$n];"
    last="v$n"; n=$((n + 1))
  done
  graph+="[$last]format=yuv444p[vout];[1:a]afade=t=out:st=79.0:d=1.0[aout]"
  heavy "$O/logs/master-$lang.log" "$FF" -hide_banner -loglevel error -y "${inputs[@]}" -filter_complex "$graph" \
    -map "[vout]" -map "[aout]" -map_chapters 2 -c:v ffv1 -level 3 -slices 16 -c:a pcm_s24le -ar 48000 -t 80 \
    "$(winpath "$O/cantus-$lang-master.mkv")"
  ;;
youtube)
  # The picture from the master; the sound from the -2 dBTP export (AAC raises peaks).
  heavy "$O/logs/youtube-$lang.log" "$FF" -hide_banner -loglevel error -y -i "$(winpath "$O/cantus-$lang-master.mkv")" \
    -i "$(winpath "$S/soundtrack-$lang-web.wav")" -map 0:v -map 1:a -map_chapters 0 \
    -c:v libx264 -preset slow -crf 16 -profile:v high -pix_fmt yuv420p -r 60 -movflags +faststart \
    -c:a aac -b:a 320k -ar 48000 -af afade=t=out:st=79.0:d=1.0 -t 80 "$(winpath "$O/cantus-$lang-2160p.mp4")"
  ;;
site)
  # Two passes to a size: 12 MB over 80 s, the audio's share taken first.
  kbps=$(( SITE_MB * 8 * 1000 / 80 - 140 ))
  vf="scale=1920:1080:flags=lanczos,fps=30,$(subs_filter "$lang" 13 22)"
  src=(-i "$(winpath "$O/cantus-$lang-master.mkv")" -i "$(winpath "$S/soundtrack-$lang-web.wav")")
  heavy "$O/logs/site1-$lang.log" "$FF" -hide_banner -loglevel error -y "${src[@]}" -map 0:v -vf "$vf" \
    -c:v libx264 -preset slow -b:v ${kbps}k -pass 1 -passlogfile "$(winpath "$O/x264-$lang")" -an -f mp4 NUL
  heavy "$O/logs/site2-$lang.log" "$FF" -hide_banner -loglevel error -y "${src[@]}" -map 0:v -map 1:a -map_chapters 0 -vf "$vf" \
    -c:v libx264 -preset slow -b:v ${kbps}k -pass 2 -passlogfile "$(winpath "$O/x264-$lang")" -pix_fmt yuv420p -movflags +faststart \
    -c:a aac -b:a 128k -af afade=t=out:st=79.0:d=1.0 -t 80 "$(winpath "$O/cantus-$lang-1080p.mp4")"
  heavy "$O/logs/vp9a-$lang.log" "$FF" -hide_banner -loglevel error -y "${src[@]}" -map 0:v -vf "$vf" \
    -c:v libvpx-vp9 -b:v ${kbps}k -row-mt 1 -pass 1 -passlogfile "$(winpath "$O/vp9-$lang")" -an -f webm NUL
  heavy "$O/logs/vp9b-$lang.log" "$FF" -hide_banner -loglevel error -y "${src[@]}" -map 0:v -map 1:a -vf "$vf" \
    -c:v libvpx-vp9 -b:v ${kbps}k -row-mt 1 -pass 2 -passlogfile "$(winpath "$O/vp9-$lang")" \
    -c:a libopus -b:a 112k -af afade=t=out:st=79.0:d=1.0 -t 80 "$(winpath "$O/cantus-$lang-1080p.webm")"
  run "$FF" -hide_banner -loglevel error -y -ss $POSTER_T -i "$(winpath "$O/cantus-$lang-master.mkv")" -frames:v 1 \
    -vf scale=1920:1080:flags=lanczos -q:v 3 "$(winpath "$O/cantus-poster.jpg")"
  ;;
loop)
  # The mixer at work (the 42-effects card, the first lower thirds): no voice, no captions.
  vf="scale=1920:1080:flags=lanczos,fps=30,fade=t=in:st=0:d=0.4,fade=t=out:st=$(calc "$LOOP_S - 0.4"):d=0.4"
  heavy "$O/logs/loop.log" "$FF" -hide_banner -loglevel error -y -ss $LOOP_FROM -t $LOOP_S -i "$(winpath "$O/cantus-en-master.mkv")" \
    -vf "$vf" -an -c:v libx264 -preset slow -crf 23 -pix_fmt yuv420p -movflags +faststart "$(winpath "$O/cantus-loop.mp4")"
  heavy "$O/logs/loop-webm.log" "$FF" -hide_banner -loglevel error -y -ss $LOOP_FROM -t $LOOP_S -i "$(winpath "$O/cantus-en-master.mkv")" \
    -vf "$vf" -an -c:v libvpx-vp9 -crf 36 -b:v 0 -row-mt 1 "$(winpath "$O/cantus-loop.webm")"
  ;;
vertical)
  # 1080 x 1920: the frame's middle three quarters across the middle (the UI large enough to
  # read on a phone), over the whole frame blurred and darkened; the captions under it; the first 30 s (the title, lines 1-3), the sound faded at the end.
  vf="[0:v]split[a][b];[a]scale=-2:1920,crop=1080:1920,boxblur=24:2,eq=brightness=-0.18[bg];"
  vf+="[b]crop=iw*3/4:ih,scale=1080:-2:flags=lanczos[fg];[bg][fg]overlay=0:(H-h)/2-100,fps=30,"
  vf+="$(subs_filter "$lang" 8 70),fade=t=out:st=$(calc "$VERT_S - 0.8"):d=0.8[v]"
  heavy "$O/logs/vertical-$lang.log" "$FF" -hide_banner -loglevel error -y -t $VERT_S -i "$(winpath "$O/cantus-$lang-master.mkv")" \
    -t $VERT_S -i "$(winpath "$S/soundtrack-$lang-web.wav")" -filter_complex "$vf" -map "[v]" -map 1:a \
    -c:v libx264 -preset slow -crf 20 -pix_fmt yuv420p -movflags +faststart \
    -c:a aac -b:a 192k -af "afade=t=out:st=$(calc "$VERT_S - 1.5"):d=1.5" "$(winpath "$O/cantus-$lang-vertical.mp4")"
  ;;
all)
  for s in captions master youtube site vertical; do DRY="${DRY:-}" bash "$0" "$s" "$lang"; done
  [[ "$lang" == en ]] && DRY="${DRY:-}" bash "$0" loop
  ;;
*) echo "unknown step: $step" >&2; exit 2 ;;
esac
