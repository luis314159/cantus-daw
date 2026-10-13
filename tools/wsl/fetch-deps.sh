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
         libssl-dev=3.0.13-0ubuntu3 libssl3t64=3.0.13-0ubuntu3; do
  if apt-get download "$p" >/dev/null 2>&1; then echo "got $p"; else echo "MISSING $p"; fi
done
for d in *.deb; do dpkg -x "$d" "$HOME/linux-deps/root"; done
# OpenSSL keeps opensslconf.h and configuration.h under the multiarch directory, which a
# plain OPENSSL_INCLUDE_DIR does not search: merge them into include/openssl.
cp -n "$HOME"/linux-deps/root/usr/include/x86_64-linux-gnu/openssl/*.h       "$HOME"/linux-deps/root/usr/include/openssl/ 2>/dev/null || true
find "$HOME/linux-deps/root" -name "*.pc" | sed 's|.*/||' | sort | tr '\n' ' '
echo
