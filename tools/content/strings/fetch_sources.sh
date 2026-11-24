#!/usr/bin/env bash
# Round 42 (strings-rec, ADR 0158): fetch the recorded strings' upstream sources into a folder
# OUTSIDE the repository, to find their loops and write their manifests (never into git):
#
#   bash tools/content/strings/fetch_sources.sh <dir>            # every source
#   bash tools/content/strings/fetch_sources.sh <dir> bigcat     # one: vsco | bigcat | smolken | meatbass | sneakybass
#
# The Karoryfer / Smolken libraries come as their GitHub release zips; VSCO 2 CE's strings as
# the raw files at the pinned commit the manifest names (440300901d…), plus its SFZ branch's
# mappings (6dd651d5…: the key centres the octave fix reads). Behind the Zscaler proxy a file
# may come back as an HTML page for ~10 minutes: every file is checked by its first bytes
# (PK for a zip, RIFF for a WAV) and fetched again after a wait. Never bypasses the proxy or TLS.
set -u
dir=${1:?usage: fetch_sources.sh <dir> [source]}
only=${2:-all}
mkdir -p "$dir"
fetch() { # url dest magic
  local url=$1 dest=$2 magic=$3 tries=0
  [ -s "$dest" ] && [ "$(head -c ${#magic} "$dest")" = "$magic" ] && return 0
  mkdir -p "$(dirname "$dest")"
  while [ $tries -lt 12 ]; do
    curl -sSL --fail -m 1800 -o "$dest.part" "$url" && [ "$(head -c ${#magic} "$dest.part")" = "$magic" ] && { mv "$dest.part" "$dest"; return 0; }
    tries=$((tries + 1)); echo "retry $tries: $url" >&2; sleep 60
  done
  echo "FAILED: $url" >&2; return 1
}
rel=https://github.com/sfzinstruments
[ "$only" = all ] || [ "$only" = bigcat ] && fetch "$rel/karoryfer-bigcat.cello/releases/download/v1.001/Karoryfer_Bigcat_cello.v1.001.zip" "$dir/bigcat.zip" PK
[ "$only" = all ] || [ "$only" = smolken ] && fetch "$rel/dsmolken.double-bass/releases/download/v1.001/DSmolken.double_bass.v1.001.zip" "$dir/smolken.zip" PK
[ "$only" = all ] || [ "$only" = meatbass ] && fetch "$rel/karoryfer.meatbass/releases/download/v1.001/Karoryfer.Meatbass.v1.001.zip" "$dir/meatbass.zip" PK
[ "$only" = all ] || [ "$only" = sneakybass ] && fetch "$rel/karoryfer.sneakybass/releases/download/v1.000/Sneakybass_v1.000.zip" "$dir/sneakybass.zip" PK
if [ "$only" = all ] || [ "$only" = vsco ]; then
  sha=440300901dfe9275fd84e0b7763af1f8443ae62e
  sfz=6dd651d55dde97fd4028699be9d4481f26917891
  api=https://api.github.com/repos/sgossner/VSCO-2-CE/git/trees
  curl -sS --fail -m 120 -o "$dir/vsco_LICENSE" "https://raw.githubusercontent.com/sgossner/VSCO-2-CE/$sha/LICENSE"
  curl -sS -m 120 -o "$dir/vsco_tree.json" "$api/$sha?recursive=1"
  curl -sS -m 120 -o "$dir/vsco_sfz_tree.json" "$api/$sfz?recursive=1"
  python -I -c "
import json, sys, urllib.parse
for tree, ref, ext in (('vsco_tree.json', '$sha', '.wav'), ('vsco_sfz_tree.json', '$sfz', '.sfz')):
    t = json.load(open(sys.argv[1] + '/' + tree))['tree']
    for e in t:
        p = e['path']
        if e['type'] == 'blob' and p.lower().endswith(ext) and (ext == '.sfz' or (p.startswith('Strings/') and '/Harp/' not in p)):
            url = 'https://raw.githubusercontent.com/sgossner/VSCO-2-CE/' + ref + '/' + urllib.parse.quote(p)
            sys.stdout.write(url + chr(9) + p + chr(10))
" "$dir" | tr -d '\r' > "$dir/vsco_files.txt"
  while IFS=$'\t' read -r url path; do
    case "$path" in
      *.sfz) [ -s "$dir/vsco/$path" ] || { mkdir -p "$(dirname "$dir/vsco/$path")"; curl -sS --fail -m 120 -o "$dir/vsco/$path" "$url" || echo "FAILED: $url" >&2; } ;;
      *) fetch "$url" "$dir/vsco/$path" RIFF ;;
    esac
  done < "$dir/vsco_files.txt"
fi
echo "fetched into $dir"
