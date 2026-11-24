#!/usr/bin/env bash
# Round 42 (strings-rec, ADR 0158): the recorded strings for the owner's ear, and the ABX
# references for Arcus (the modelled cello and bass).
#
#   bash tools/content/strings/render.sh <sources> <out> [abx.mid]
#
# <sources>: what fetch_sources.sh fetched (each pack is laid out there as an install lays it
# out, its SFZ written by the same build). <out>: WAV 48 kHz 24-bit, peak −1 dBFS, and a
# −23 LUFS copy of each (*-lufs23.wav): `<pack>-<instrument>-line.wav` (a legato pair, a
# mod-wheel swell, détaché notes) for every instrument and preset, and, with the ABX phrase
# (tools/arcus/abx_phrase.mid, on r42/arcus), abx-cello.wav and abx-bass.wav.
set -eu
src=${1:?usage: render.sh <sources> <out> [abx.mid]}
out=${2:?the output folder}
here=$(cd "$(dirname "$0")/../../.." && pwd)
slot=${CANTUS_SLOT:-/f/projects/cantus/tools/build/slot.sh}
cd "$here"
bash "$slot" cargo run --release -p cantus-ui --example strings_listen -- "$src" "$out" ${3:-}
