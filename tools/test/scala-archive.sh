#!/usr/bin/env bash
# The Scala scale archive (version 94, 5 401 `.scl` files) through Cantus's Scala reader
# (round 42, ADR 0155): every file parses or is refused for a stated reason, and each one
# read writes back to the same cents within 1e-9; the historical files agree with Cantus's
# temperaments. The archive has no licence, so none of its files is ever committed — this
# tool fetches it from its fixed Wayback Machine copy (the official URL is not versioned:
# version 95 will replace its bytes), checks its SHA-256 and unpacks it outside the
# repository, then runs the `#[ignore]` test that reads it.
#
#   bash tools/test/scala-archive.sh [dir]
#
# dir: where the zip and its files go (default $SCALA_ARCHIVE_DIR, else
# ~/.cantus-tools/scala-archive). Wrap the cargo step in tools/build/slot.sh on a machine
# whose builds share slots (SLOT=1 does it). A corporate proxy may answer with an HTML page
# for a while: the script checks the bytes and says so — wait and run it again; never
# bypass the proxy or TLS. A hand-run tool, never part of CI (ADR 0155): the download retries
# on every error (docs/RELEASING.md L6) and is checked by its SHA-256 (`sha256sum`, or
# `shasum -a 256` on a stock macOS).
set -euo pipefail
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}
dir="${1:-${SCALA_ARCHIVE_DIR:-$HOME/.cantus-tools/scala-archive}}"
url='https://web.archive.org/web/20260404005834id_/https://huygens-fokker.org/docs/scales.zip'
want='d2218f2fa6acd3b11c116e0626bd178af927636e2ed26d0ab7a4c27741b97b9a'
here="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$dir"
zip="$dir/scales-v94.zip"
if [ ! -f "$zip" ] || [ "$(sha256 "$zip")" != "$want" ]; then
  echo "fetching the archive (version 94) from the Wayback Machine…"
  curl -sSL --fail --retry 5 --retry-all-errors --retry-delay 5 -o "$zip.part" "$url"
  if [ "$(head -c 2 "$zip.part")" != "PK" ]; then
    echo "not a zip (a proxy's page?): wait a few minutes and run again" >&2
    rm -f "$zip.part"
    exit 1
  fi
  mv "$zip.part" "$zip"
fi
got="$(sha256 "$zip")"
if [ "$got" != "$want" ]; then
  echo "SHA-256 $got, not version 94's $want" >&2
  exit 1
fi
out="$dir/v94"
if [ ! -d "$out/scl" ]; then
  rm -rf "$out"
  mkdir -p "$out"
  # Bytes as they are (no -a): the reader takes CRLF, LF and a lone CR.
  unzip -q "$zip" -d "$out"
fi
echo "$(find "$out/scl" -name '*.scl' | wc -l) files in $out/scl"
cd "$here"
run=(cargo test -p cantus-core --test tuning_scala_archive -- --ignored --nocapture)
if [ "${SLOT:-0}" = "1" ]; then
  run=(bash "$here/tools/build/slot.sh" "${run[@]}")
fi
CANTUS_SCALA_ARCHIVE="$out/scl" "${run[@]}"
