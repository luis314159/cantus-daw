#!/bin/bash
# Once per machine: rustup and the -dev packages Cantus's Linux build scripts need, for
# tools/wsl/check.sh — without sudo. `apt-get download` fetches the .deb files and `dpkg -x`
# unpacks them into ~/linux-deps/root (nothing is installed system-wide). If the apt index
# is stale (a 404), pin the release's base version, e.g. libasound2-dev=1.2.11-1build2.
#
#   wsl -d Ubuntu-24.04 -- bash -lc 'bash /mnt/f/projects/cantus/tools/wsl/fetch-deps.sh'
set -e
if [ ! -x "$HOME/.cargo/bin/rustup" ]; then
  curl -sSf https://sh.rustup.rs -o /tmp/rustup.sh
  sh /tmp/rustup.sh -y --profile minimal --default-toolchain none
fi
mkdir -p "$HOME/linux-deps/debs" "$HOME/linux-deps/root"
cd "$HOME/linux-deps/debs"
for p in pkgconf pkgconf-bin libpkgconf3 \
         libdbus-1-dev libdbus-1-3 \
         libwayland-dev libwayland-client0 libwayland-cursor0 libwayland-egl1 libwayland-server0 \
         libxkbcommon-dev libxkbcommon0 \
         libasound2-dev=1.2.11-1build2 libasound2t64=1.2.11-1build2 \
         libssl-dev=3.0.13-0ubuntu3 libssl3t64=3.0.13-0ubuntu3 \
         libpipewire-0.3-dev=1.0.5-1 libpipewire-0.3-0t64=1.0.5-1 libspa-0.2-dev=1.0.5-1 \
         libclang1-18 libclang-common-18-dev libllvm18 libz3-4 libedit2 libffi8; do
  if apt-get download "$p" >/dev/null 2>&1; then echo "got $p"; else echo "MISSING $p"; fi
done
for d in *.deb; do dpkg -x "$d" "$HOME/linux-deps/root"; done
# The audio servers, to try Cantus's PipeWire and JACK hosts in WSL (round 30): a PipeWire
# daemon with WirePlumber, and jackd2 — unpacked apart, in ~/linux-deps/rt, never on the
# build's paths (packaging/linux/README.md, "Trying the audio hosts in WSL").
mkdir -p "$HOME/linux-deps/rt-debs" "$HOME/linux-deps/rt"
cd "$HOME/linux-deps/rt-debs"
for p in pipewire=1.0.5-1 pipewire-bin=1.0.5-1 libpipewire-0.3-modules=1.0.5-1 \
         libspa-0.2-modules=1.0.5-1 pipewire-jack=1.0.5-1 libpipewire-0.3-common=1.0.5-1 \
         libpipewire-0.3-0t64=1.0.5-1 wireplumber libwireplumber-0.4-0 liblua5.4-0 \
         jackd2 libjack-jackd2-0 libopus0 libsamplerate0 libwebrtc-audio-processing1; do
  if apt-get download "$p" >/dev/null 2>&1; then echo "got $p"; else echo "MISSING $p"; fi
done
for d in *.deb; do dpkg -x "$d" "$HOME/linux-deps/rt"; done
cd "$HOME/linux-deps/debs"
# OpenSSL keeps opensslconf.h and configuration.h under the multiarch directory, which a
# plain OPENSSL_INCLUDE_DIR does not search: merge them into include/openssl.
cp -n "$HOME"/linux-deps/root/usr/include/x86_64-linux-gnu/openssl/*.h       "$HOME"/linux-deps/root/usr/include/openssl/ 2>/dev/null || true
find "$HOME/linux-deps/root" -name "*.pc" | sed 's|.*/||' | sort | tr '\n' ' '
echo
