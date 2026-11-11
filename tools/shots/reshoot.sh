#!/usr/bin/env bash
# Every interface picture on the website and in the README, in one command (round 39, `shots`):
#
#   bash tools/shots/reshoot.sh [units|rest|all] [--no-build]
#
# - `units`: every instrument's and effect's window (shoot_skins; Delay's too since round 39),
#   English and Spanish, Slate Night and Slate Paper, made into
#   web/assets/units/ and docs/images/units/ (+ thumbs/) by web/tools/units_pictures.py; the
#   sizes in web/src/units_data.rs are checked (and rewritten) by tools/shots/check_units.py.
# - `rest`: the program picture (web/assets/program/), the README's pictures (docs/images/*.png)
#   the site's studio/, session/, score/ and ai/ pictures, and the manual's (web/assets/manual/img,
#   docs/manual/img), by tools/shots/place.py, which holds the crops and encodings.
# - `all` (the default): both.
#
# The shots go to $CANTUS_SHOTS (default target/shots/reshoot), one folder per example; the
# pictures are written in place, in the repository. Every example runs headless, on the CPU,
# in the default look (Slate Night; Slate Paper for the light pictures).
#
# Builds go through $CANTUS_SLOT when it is set (round 39: CANTUS_SLOT=/f/projects/daw-r39/slot.sh).
# Needs Python with Pillow; oxipng is used when installed.
set -euo pipefail
cd "$(dirname "$0")/../.."
what=all
build=1
for a in "$@"; do
  case "$a" in
    units|rest|all) what=$a ;;
    --no-build) build=0 ;;
    *) echo "usage: reshoot.sh [units|rest|all] [--no-build]" >&2; exit 2 ;;
  esac
done
out=${CANTUS_SHOTS:-target/shots/reshoot}
bin=target/release/examples
units_ex=(shoot_skins)
rest_ex=(shoot_site shoot_readme shoot_showcase shoot_editor shoot_external shoot shoot_session
  shoot_sync shoot_mpe shoot_ai shoot_mastering shoot_stems shoot_transcribe shoot_rec shoot_picker
  shoot_audio_setup shoot_views shoot_notefx)
case $what in
  units) ex=("${units_ex[@]}") ;;
  rest) ex=("${rest_ex[@]}") ;;
  all) ex=("${units_ex[@]}" "${rest_ex[@]}") ;;
esac

if [ "$build" = 1 ]; then
  flags=()
  for e in "${ex[@]}"; do flags+=(--example "$e"); done
  slot=()
  [ -n "${CANTUS_SLOT:-}" ] && slot=(bash "$CANTUS_SLOT")
  "${slot[@]}" cargo build --release -p cantus-ui "${flags[@]}"
fi

run() { # run <example> <args…>: one example, its output in the log beside the shots
  local e=$1
  shift
  echo "== $e $*"
  "$bin/$e" "$@" >>"$out/$e.log" 2>&1 || { echo "$e failed: $out/$e.log" >&2; tail -20 "$out/$e.log" >&2; exit 1; }
}

mkdir -p "$out"
if [ "$what" != rest ]; then
  rm -rf "$out/skins" "$out/skins-es"
  run shoot_skins "$out/skins"
  run shoot_skins "$out/skins-es" --es
  python web/tools/units_pictures.py "$out/skins"
  python web/tools/units_pictures.py "$out/skins-es" --es
  # A window that changed size: its declared size in web/src/units_data.rs follows.
  python tools/shots/check_units.py --fix
fi
if [ "$what" != units ]; then
  for d in site readme showcase editor external ara session sync mpe ai mastering stems transcribe manual; do
    rm -rf "${out:?}/$d"
    mkdir -p "$out/$d"
  done
  run shoot_site "$out/site" night
  run shoot_site "$out/site" night es
  run shoot_readme "$out/readme" window-night window-paper mixer inspector matrix linked \
    pianoroll rack appearance themes mark packs score
  run shoot_showcase "$out/showcase"
  run shoot_editor "$out/editor"
  run shoot_external "$out/external" -150
  run shoot "$out/ara" 1 ara
  run shoot_session "$out/session"
  run shoot_sync "$out/sync" sync-bar-link-night-en-100
  run shoot_mpe "$out/mpe" mpe-roll-night-en-100
  run shoot_ai "$out/ai" ai-activity-night-en ai-settings-paper-en
  run shoot_mastering "$out/mastering" 1 en
  run shoot_stems "$out/stems" 1 en
  run shoot_transcribe "$out/transcribe"
  # The manual's pictures (web/assets/manual/img, docs/manual/img).
  run shoot "$out/manual" 1 startup
  run shoot "$out/manual" 1 master
  run shoot "$out/manual" 1 samples
  run shoot_rec "$out/manual" rec-window-slate
  run shoot_picker "$out/manual" instruments dark
  run shoot_audio_setup "$out/manual" first-run popover
  run shoot_views "$out/manual" pianoroll-night-welcome-1280x720
  run shoot_notefx "$out/manual" notefx-chain-night-en-100 notefx-roll-night-en-100 notefx-clip-night-en-100
  python tools/shots/place.py "$out"
fi
echo "done: shots in $out"
