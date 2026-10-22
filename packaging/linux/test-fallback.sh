#!/bin/bash
# Proof that Cantus's AppImage starts on a system without libpipewire (ADR 0065): the
# stand-in in usr/lib/fallback is what the loader takes, and Cantus starts and lists its
# audio drivers — ALSA, and no PipeWire.
#
#   bash packaging/linux/test-fallback.sh <the .AppImage>
#
# On a system that has libpipewire (CI), AppRun is told to use the stand-in anyway
# (CANTUS_FORCE_PIPEWIRE_FALLBACK=1); on one without it (WSL as set up by
# tools/wsl/fetch-deps.sh), it takes it by itself — and the test says which it saw.
set -euo pipefail
image="$(readlink -f "${1:?usage: test-fallback.sh <the .AppImage>}")"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
"$image" --appimage-extract >/dev/null
app="$work/squashfs-root"
if ldconfig -p 2>/dev/null | grep -q libpipewire-0.3.so.0 ||
  [ -e /usr/lib/x86_64-linux-gnu/libpipewire-0.3.so.0 ]; then
  echo "this system has libpipewire: AppRun is told to take the stand-in"
  export CANTUS_FORCE_PIPEWIRE_FALLBACK=1
else
  echo "this system has no libpipewire"
fi
LD_DEBUG=libs "$app/AppRun" --version 2>"$work/ld.txt" | tee "$work/version.txt"
grep -q "^cantus " "$work/version.txt" || {
  echo "FAIL: it did not start"
  exit 1
}
grep "libpipewire-0.3.so.0" "$work/ld.txt" | grep -q "usr/lib/fallback" || {
  echo "FAIL: the loader did not take the stand-in:"
  grep "libpipewire" "$work/ld.txt" | head
  exit 1
}
drivers=$("$app/AppRun" --audio-drivers 2>/dev/null || true)
echo "drivers: $drivers"
echo "$drivers" | grep -q "ALSA" || {
  echo "FAIL: no ALSA"
  exit 1
}
if echo "$drivers" | grep -qi "pipewire"; then
  echo "FAIL: PipeWire offered without its library"
  exit 1
fi
echo "OK: started with the stand-in libpipewire; ALSA offered, PipeWire not"
