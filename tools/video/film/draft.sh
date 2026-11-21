#!/usr/bin/env bash
# The launch video's first draft (round 41, step 3): the film's scenes cut together at
# 1920 x 1080, 30 fps, with each scene's own sound, the cards laid over at the cut's times.
#
#   bash tools/video/film/draft.sh [render|cards|cut|all]   (default: cut)
#
#   render  build the film example (release) and render every scene into $W
#   cards   render the cards (tools/video/cards/render.mjs) at scale 1, 30 fps, into $W/cards
#   cut     encode each scene, join them, lay the cards over: $W/draft-1080p30.mp4
#
# The frames, sound and video stay in F:\projects\video-work\film (never in the repository).
# ffmpeg is a tool only (tools/video/film/ffmpeg.txt says which build and its SHA-256).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
W="${FILM_OUT:-/f/projects/video-work/film}"
FF="${FFMPEG:-/f/projects/video-work/tools/ffmpeg/ffmpeg-n8.1.3-34-ga35157abb9-win64-gpl-8.1/bin/ffmpeg.exe}"
FPS=30
what="${1:-cut}"

# The cut: scene, start, end (seconds) — script.md's table, shot 5 to 0:44, shot 6 in thirds.
SCENES=(s1-open s2-window s3-piano s4-drums s5-mixer s6a-record s6b-session s6c-plugin s7-master s8-desk s9-end)

winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }

if [[ "$what" == render || "$what" == all ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
  export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
  export CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
  cd "$ROOT"
  # The hosted plug-in of shot 6c: the repository's test gain, built from source.
  bash tools/build/slot.sh cargo build -j 3 -p cantus-test-clap --target-dir target/test-plugins
  bash tools/build/slot.sh cargo build --release -j 3 -p cantus-ui --example film
  # Below normal priority: the machine is shared.
  powershell.exe -NoProfile -Command "\$p = Start-Process -FilePath '$(winpath "$ROOT/target/release/examples/film.exe")' -ArgumentList '--out','$(winpath "$W")','--fps','$FPS' -WorkingDirectory '$(winpath "$ROOT")' -NoNewWindow -PassThru; \$p.PriorityClass = 'BelowNormal'; \$p.WaitForExit(); exit \$p.ExitCode"
fi

if [[ "$what" == cards || "$what" == all ]]; then
  cd "$ROOT"
  node tools/video/cards/render.mjs "$(winpath "$W/cards")" --lang en --scale 1 --fps $FPS
fi

if [[ "$what" == cut || "$what" == all ]]; then
  mkdir -p "$W/seg"
  # Each scene: its frames and its sound, the same length to the frame.
  for s in "${SCENES[@]}"; do
    [[ -f "$W/$s.wav" ]] || { echo "missing $s (render it first)" >&2; exit 1; }
    "$FF" -hide_banner -loglevel error -y -framerate $FPS -i "$(winpath "$W/$s")/%05d.png" -i "$(winpath "$W/$s.wav")" \
      -c:v libx264 -preset veryfast -crf 14 -pix_fmt yuv420p -r $FPS \
      -c:a pcm_s16le -ar 48000 -shortest "$(winpath "$W/seg/$s.mkv")"
  done
  # Joined, hard cuts (the draft's), a fade in from black and out at the end.
  : > "$W/seg/list.txt"
  for s in "${SCENES[@]}"; do echo "file '$s.mkv'" >> "$W/seg/list.txt"; done
  "$FF" -hide_banner -loglevel error -y -f concat -safe 0 -i "$(winpath "$W/seg/list.txt")" -c copy "$(winpath "$W/seg/joined.mkv")"

  # The cards over the film, each at its time (script.md's Card column).
  C="$W/cards/en"
  declare -a CARD=(title count-instruments count-effects lower-record lower-session lower-plugins free end-b)
  declare -A AT=([title]=0 [count-instruments]=15.5 [count-effects]=33.5 [lower-record]=44.0 [lower-session]=46.667 [lower-plugins]=49.333 [free]=62.5 [end-b]=68.0)
  inputs=(-i "$(winpath "$W/seg/joined.mkv")")
  graph="[0:v]fade=t=in:st=0:d=1.0,fade=t=out:st=79.0:d=1.0[v0];"
  last="v0"; n=1
  if [[ -d "$C" ]]; then
    for c in "${CARD[@]}"; do
      [[ -d "$C/$c" ]] || continue
      inputs+=(-framerate $FPS -i "$(winpath "$C/$c")/%05d.png")
      graph+="[$n:v]format=rgba,setpts=PTS+${AT[$c]}/TB[c$n];[$last][c$n]overlay=eof_action=pass:format=auto[v$n];"
      last="v$n"; n=$((n + 1))
    done
    # Onda at the end card's side: the Stage of s9-end, cut from its frames, in the card's
    # place for it (420 x 560 at 1350, 300).
    if [[ -d "$C/end-b" && -f "$W/s9-end.json" ]]; then
      read -r sx sy sw sh < <(node -e 'const j=require(process.argv[1]);const s=j.stage||[0,0,420,560];const k=j.scale||1;console.log(Math.round(s[0]*k),Math.round(s[1]*k),Math.round(s[2]*k),Math.round(s[3]*k))' "$(winpath "$W/s9-end.json")")
      inputs+=(-framerate $FPS -i "$(winpath "$W/s9-end")/%05d.png")
      graph+="[$n:v]crop=$sw:$sh:$sx:$sy,scale=420:560:force_original_aspect_ratio=decrease,setpts=PTS+68/TB[o$n];[$last][o$n]overlay=x=1350+(420-w)/2:y=300+(560-h)/2:eof_action=pass[v$n];"
      last="v$n"; n=$((n + 1))
    fi
  fi
  graph+="[$last]format=yuv420p[vout];[0:a]afade=t=in:st=0:d=0.3,afade=t=out:st=78.5:d=1.5[aout]"
  "$FF" -hide_banner -loglevel error -y "${inputs[@]}" -filter_complex "$graph" -map "[vout]" -map "[aout]" \
    -c:v libx264 -preset medium -crf 20 -pix_fmt yuv420p -r $FPS -movflags +faststart \
    -c:a aac -b:a 192k -t 80 "$(winpath "$W/draft-1080p30.mp4")"
  echo "$W/draft-1080p30.mp4"
fi
