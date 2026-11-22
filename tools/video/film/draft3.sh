#!/usr/bin/env bash
# Draft 3 (the owner, 2026-11-21), from a checkout of r41/video that has the frames and the
# cards already rendered (draft.sh render, draft.sh cards): the soundtrack rendered by
# integration's engine (DSP_VERSION 11, round 41's Chorda) in the r41/video-int worktree,
# the end card's platform line re-rendered, the cut, the checks. Each heavy step through
# gently.sh (RAM and C: to spare, below-normal priority, the watchdog).
#
#   bash tools/video/film/draft3.sh [en|es]          (default: en)
#
# First time on a machine: bash tools/video/film/int_worktree.sh make, then build.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
lang="${1:-en}"
W=/f/projects/video-work/film
S=/f/projects/video-work/soundtrack3
INT="${VIDEO_INT:-/f/projects/daw-r41/video-int}"
winpath() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
mkdir -p "$S" "$W/logs"
cd "$ROOT"
# The soundtrack (heart.wav rendered once, by DSP 11); 10.5 dB asked so every line is clear
# of the 10 dB the owner asked.
bash tools/video/film/gently.sh "$S/g-$lang.log" "$INT/target/release/examples/soundtrack.exe" \
  --lang "$lang" --margin 10.5 --out "$(winpath "$S")"
grep -m1 DSP_VERSION "$S/g-$lang.log.out"
# The end card with its platform line (cards.js: --os preview is the default).
bash tools/video/film/gently.sh "$W/logs/g-endb-$lang.log" "$(command -v node)" \
  tools/video/cards/render.mjs "$(winpath "$W/cards")" --lang "$lang" --scale 1 --fps 30 --card end-b
# The cut under the new soundtrack.
SOUND_OUT="$S" DRAFT=draft3 bash tools/video/film/gently.sh "$W/logs/g-cut3-$lang.log" \
  "$(command -v bash)" tools/video/film/draft.sh cut "$lang"
out="$W/draft3-$lang-1080p30.mp4"
# The checks.
bash tools/video/film/check_audio.sh "$S/soundtrack-$lang.wav" "$out"
bash tools/video/film/cut_frames.sh "$out" "$W/checks-draft3-$lang.png"
bash tools/video/film/heart_diff.sh /f/projects/video-work/soundtrack/heart.wav "$S/heart.wav" 0 16.55
