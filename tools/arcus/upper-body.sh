#!/usr/bin/env bash
# Arcus's violin body (round 42, arcus-upper): the CNSM dataset's bridge admittances (Zenodo
# 10.5281/zenodo.18696786, CC BY 4.0) — one violin's measurements averaged, the acquisition's
# delay taken out — fitted by a sum of band-pass modes, the fit's distance printed beside the
# distance between the dataset's three violins, written into
# crates/cantus-dsp/src/arcus_upper_body_data.rs (`examples/arcus_upper_body.rs`).
#
#   bash tools/arcus/upper-body.sh [admittances-dir] [violin]
#
# The admittances are the `admittances/` folder of the dataset's zip, never in the worktree:
# by default `$ARCUS_UPPER_REFS/cnsm/admittances` (`../downloads/arcus-upper`). Only that part
# of the 654 MB zip is needed — fetch its byte range (the zip's central directory names the
# offsets; `docs/round42/arcus-upper.md` says how).
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
slot=${SLOT:-$here/tools/build/slot.sh}
refs=${ARCUS_UPPER_REFS:-$here/../downloads/arcus-upper}
dir=${1:-$refs/cnsm/admittances}
violin=${2:-Klimke}
cd "$here"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_upper_body -- \
  "$dir" "$violin" crates/cantus-dsp/src/arcus_upper_body_data.rs
