#!/usr/bin/env bash
# Arcus's violin and viola for the owner's ears (round 42, arcus-upper): per instrument a solo
# note at pp, mf and ff, a four-bar legato phrase with a CC1 swell, détaché, pizzicato and
# tremolo; and the blind ABX's pairs — a recorded note (Iowa MIS, VSCO 2 CE's solo violin) and
# Arcus on the same key, dynamic and length, both at −23 LUFS, into <out>/ref/ — then
# tools/listen/abx.html copied beside them with its SETS set to those pairs
# (`examples/arcus_upper_listen.rs`).
#
#   bash tools/arcus/upper-listen.sh [out-dir] [refs-dir]
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
slot=${SLOT:-$here/tools/build/slot.sh}
out=${1:-$here/../listen/arcus-upper}
refs=${2:-${ARCUS_UPPER_REFS:-$here/../downloads/arcus-upper/refs}}
cd "$here"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_upper_listen -- "$out" "$refs"
# The ABX page beside the pairs, its SETS the ones just written.
page=${ABX_PAGE:-$here/tools/listen/abx.html}
[ -f "$page" ] || page=$here/../contract/tools/listen/abx.html
python -I - "$page" "$out/sets.js.txt" "$out/abx.html" <<'PYX'
import re, sys
page = open(sys.argv[1], encoding="utf-8").read()
sets = open(sys.argv[2], encoding="utf-8").read().strip()
page = re.sub(r"const SETS = \{.*?\n\};", lambda m: sets, page, count=1, flags=re.S)
page = page.replace("ABX a ciegas — Arcus", "ABX a ciegas — Arcus: violín y viola")
open(sys.argv[3], "w", encoding="utf-8").write(page)
PYX
echo "ABX page: $out/abx.html"
