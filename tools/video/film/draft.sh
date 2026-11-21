#!/usr/bin/env bash
# The launch video's drafts (round 41, step 3): the film's scenes cut together at
# 1920 x 1080, 30 fps, the cards laid over at the cut's times, under the soundtrack mixed in
# Cantus (examples/soundtrack.rs: *With All Your Heart* with one crossfade, the narration,
# the music ducked under it, -14 LUFS / -1 dBTP).
#
#   bash tools/video/film/draft.sh [render|cards|sound|cut|all] [en|es]   (default: cut en)
#
#   render  build the film example (release) and render every scene into $W
#   cards   render the cards in the language (tools/video/cards/render.mjs, scale 1, 30 fps)
#   sound   build and run the soundtrack example for the language (into $S)
#   cut     encode each scene (only those rendered since), join them, lay the cards over,
#           put the soundtrack under: $W/draft2-<lang>-1080p30.mp4
#
# Generated files stay in F:\projects\video-work (never in the repository). Every heavy
# job runs at below-normal priority; ffmpeg on four threads. ffmpeg is a tool only
# (tools/video/film/ffmpeg.txt says which build and its SHA-256).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
W="${FILM_OUT:-/f/projects/video-work/film}"
S="${SOUND_OUT:-/f/projects/video-work/soundtrack}"
FF="${FFMPEG:-/f/projects/video-work/tools/ffmpeg/ffmpeg-n8.1.3-34-ga35157abb9-win64-gpl-8.1/bin/ffmpeg.exe}"
FPS=30
what="${1:-cut}"
lang="${2:-en}"

# The cut, in order — script.md's table, shot 5 to 0:44, shot 6 in thirds.
SCENES=(s1-open s2-window s3-piano s4-drums s5-mixer s6a-record s6b-session s6c-plugin s7-master s8-desk s9-end)

winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }

# Run a program at below-normal priority and wait for it.
low() {
  local exe="$1"; shift
  local args=""
  for a in "$@"; do args+="'$a',"; done
  powershell.exe -NoProfile -Command "\$p = Start-Process -FilePath '$(winpath "$exe")' -ArgumentList ${args%,} -WorkingDirectory '$(winpath "$ROOT")' -NoNewWindow -PassThru; \$p.PriorityClass = 'BelowNormal'; \$p.WaitForExit(); exit \$p.ExitCode"
}

cargo_env() {
  export PATH="$HOME/.cargo/bin:$PATH"
  export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
  export CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
  cd "$ROOT"
}

if [[ "$what" == render || "$what" == all ]]; then
  cargo_env
  # The hosted plug-in of shot 6c: the repository's test gain, built from source.
  bash tools/build/slot.sh cargo build -j 2 -p cantus-test-clap --target-dir target/test-plugins
  bash tools/build/slot.sh cargo build --release -j 2 -p cantus-ui --example film
  low "$ROOT/target/release/examples/film.exe" --out "$(winpath "$W")" --fps $FPS
fi

if [[ "$what" == cards || "$what" == all ]]; then
  cd "$ROOT"
  node tools/video/cards/render.mjs "$(winpath "$W/cards")" --lang "$lang" --scale 1 --fps $FPS
fi

if [[ "$what" == sound || "$what" == all ]]; then
  cargo_env
  bash tools/build/slot.sh cargo build --release -j 2 -p cantus-ui --example soundtrack
  low "$ROOT/target/release/examples/soundtrack.exe" --lang "$lang" --out "$(winpath "$S")"
fi

if [[ "$what" == cut || "$what" == all ]]; then
  mkdir -p "$W/seg"
  # Each scene's picture (the sound is the soundtrack's): encoded again only when its frames
  # are newer than its segment.
  for s in "${SCENES[@]}"; do
    [[ -d "$W/$s" ]] || { echo "missing $s (render it first)" >&2; exit 1; }
    seg="$W/seg/$s.mkv"
    if [[ ! -f "$seg" || "$W/$s" -nt "$seg" || "$W/$s.json" -nt "$seg" ]]; then
      "$FF" -hide_banner -loglevel error -y -framerate $FPS -i "$(winpath "$W/$s")/%05d.png" \
        -threads 4 -c:v libx264 -preset veryfast -crf 14 -pix_fmt yuv420p -r $FPS -an "$(winpath "$seg")"
    fi
  done
  : > "$W/seg/list.txt"
  for s in "${SCENES[@]}"; do echo "file '$s.mkv'" >> "$W/seg/list.txt"; done
  "$FF" -hide_banner -loglevel error -y -f concat -safe 0 -i "$(winpath "$W/seg/list.txt")" -c copy "$(winpath "$W/seg/joined.mkv")"

  # The soundtrack for the lossy encode (-14 LUFS under -2 dBTP; the master WAV is -1 dBTP).
  sound="$S/soundtrack-$lang-web.wav"
  [[ -f "$sound" ]] || { echo "missing $sound (draft.sh sound $lang)" >&2; exit 1; }
  # The cards over the film, each at its time (script.md's Card column); end card B (A or
  # B waits on the owner). No dancer on the end card (the owner, 2026-11-21).
  C="$W/cards/$lang"
  declare -a CARD=(title count-instruments count-effects lower-record lower-session lower-plugins free end-b)
  declare -A AT=([title]=0 [count-instruments]=15.5 [count-effects]=33.5 [lower-record]=44.0 [lower-session]=46.667 [lower-plugins]=49.333 [free]=62.5 [end-b]=68.0)
  inputs=(-i "$(winpath "$W/seg/joined.mkv")" -i "$(winpath "$sound")")
  graph="[0:v]fade=t=in:st=0:d=1.0,fade=t=out:st=79.0:d=1.0[v0];"
  last="v0"; n=2
  for c in "${CARD[@]}"; do
    [[ -d "$C/$c" ]] || { echo "missing card $C/$c (draft.sh cards $lang)" >&2; exit 1; }
    inputs+=(-framerate $FPS -i "$(winpath "$C/$c")/%05d.png")
    graph+="[$n:v]format=rgba,setpts=PTS+${AT[$c]}/TB[c$n];[$last][c$n]overlay=eof_action=pass:format=auto[v$n];"
    last="v$n"; n=$((n + 1))
  done
  # The soundtrack as mixed and mastered in Cantus; only the last second faded with the
  # picture's fade to black.
  graph+="[$last]format=yuv420p[vout];[1:a]afade=t=out:st=79.0:d=1.0[aout]"
  out="$W/${DRAFT:-draft2}-$lang-1080p30.mp4"
  "$FF" -hide_banner -loglevel error -y "${inputs[@]}" -filter_complex "$graph" -map "[vout]" -map "[aout]" \
    -threads 4 -c:v libx264 -preset medium -crf 20 -pix_fmt yuv420p -r $FPS -movflags +faststart \
    -c:a aac -b:a 256k -ar 48000 -t 80 "$(winpath "$out")"
  echo "$out"
fi
