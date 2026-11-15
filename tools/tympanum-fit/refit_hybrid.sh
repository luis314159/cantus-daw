#!/usr/bin/env bash
# The hybrid's residual, fitted again in one command (round 40, drums-hyb; ADR 0124's third
# amendment) — whenever model 5's tables change, the residual is stale (the engine logs it).
#
#   bash refit_hybrid.sh <kit: jazz|studio> <content root> <work dir>
#
# jazz fits against Virtuosity Drums (pack:virtuosity-drums/kit, CC0), studio against DRSKit
# (pack:drskit-tympanum/kit, CC BY 4.0); both packs must be installed under <content root>.
# Writes <work>/residual-<kit>.crs, its report (how much of each recording the model already
# explains), the zip to upload (<work>/pack/) with its manifest entry (written into
# crates/cantus-content/manifest/content.json: its size and SHA-256), and puts the residual
# into <content root>/tympanum-hybrid-<kit>/ so the kit plays it at once on this machine.
set -euo pipefail
KIT=$1
CONTENT=$2
WORK=$3
case $KIT in
  jazz) REC=pack:virtuosity-drums/kit ;;
  studio) REC=pack:drskit-tympanum/kit ;;
  *) echo "kit: jazz or studio" >&2; exit 2 ;;
esac
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
P=${PYTHON:-/f/projects/daw-r40/downloads/chorda/venv/Scripts/python.exe}
# `SLOT=` (empty) when already inside a build slot.
SLOT=${SLOT-bash /f/projects/daw-r40/slot.sh}
mkdir -p "$WORK"
cd "$ROOT"
run() { $SLOT cargo run -q --release -p cantus-ui --example tympanum_hybrid -- "$@"; }
run takes "$WORK" "$CONTENT" "$REC"
run takes "$WORK" "$CONTENT" "model:$KIT"
run probe "$WORK" "$CONTENT" "$KIT"
REC_WAV="$WORK/$(echo "$REC" | tr ':/' '--')-takes.wav"
cd "$HERE"
$P -I residual_fit.py "$REC_WAV" "$WORK/model-$KIT-takes.wav" "$WORK/probe-$KIT.wav" \
  "$WORK/schedule.txt" "$ROOT/crates/cantus-dsp/src/pulsus_measured.bin" \
  "$WORK/residual-$KIT.crs" "$WORK/report-$KIT.txt"
$P -I residual_zip.py "$KIT" "$WORK/residual-$KIT.crs" "$WORK/report-$KIT.txt" "$WORK/pack" > /dev/null
$P -I residual_manifest.py "$ROOT/crates/cantus-content/manifest/content.json"   "$WORK/pack/tympanum-hybrid-$KIT.manifest.json"
mkdir -p "$CONTENT/tympanum-hybrid-$KIT"
cp "$WORK/residual-$KIT.crs" "$CONTENT/tympanum-hybrid-$KIT/residual.crs"
echo "refit done: $WORK/report-$KIT.txt, $WORK/pack/"
