#!/bin/bash
# The system packages Cantus's Linux build needs, on Ubuntu 22.04 or 24.04 (CI, or a
# developer's machine with sudo) — ADR 0065, docs/DEVELOPING.md §1.
#
#   bash packaging/linux/deps.sh            # apt packages, and PipeWire headers if needed
#
# PipeWire: cpal's PipeWire host (bindgen over PipeWire's headers, so clang too) needs the
# headers of PipeWire 0.3.49 or later. Ubuntu 22.04 has 0.3.48: there the headers of 1.0.5
# (Ubuntu 24.04's packages, pinned by SHA-256) are unpacked into target/pipewire-headers and
# pkg-config is pointed at them, while the program still links against the system's own
# libpipewire-0.3.so.0 — so a symbol 0.3.48 lacks fails the link, and the AppImage built on
# 22.04 starts on 22.04 (cpal reads `pw_buffer.requested` only from a 0.3.49+ library,
# third_party/README.md). The PKG_CONFIG_PATH to use is written to $GITHUB_ENV in CI and
# printed otherwise.
set -euo pipefail
cd "$(dirname "$0")/../.."

sudo apt-get update -q
sudo apt-get install -y -q \
  pkg-config clang libclang-dev \
  libasound2-dev libdbus-1-dev libxkbcommon-dev libwayland-dev libgtk-3-dev \
  libpipewire-0.3-dev libspa-0.2-dev \
  squashfs-tools xvfb xauth

have=$(pkg-config --modversion libpipewire-0.3)
newer() { [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -1)" = "$2" ]; }
if newer "$have" "0.3.49"; then
  echo "PipeWire $have: its own headers serve"
  exit 0
fi

echo "PipeWire $have: unpacking the headers of 1.0.5"
dir="$PWD/target/pipewire-headers"
rm -rf "$dir"
mkdir -p "$dir/debs" "$dir/root" "$dir/lib" "$dir/pc"
pool=http://archive.ubuntu.com/ubuntu/pool/main/p/pipewire
fetch() { # name sha256
  curl -fsSL --retry 3 -o "$dir/debs/$1" "$pool/$1"
  echo "$2  $dir/debs/$1" | sha256sum -c -
  dpkg -x "$dir/debs/$1" "$dir/root"
}
fetch libpipewire-0.3-dev_1.0.5-1_amd64.deb c0b34ed6a34391a94c0711966dbea9b6b5a98758d88813824c05d6662e0b60c9
fetch libspa-0.2-dev_1.0.5-1_amd64.deb 6c99d162e23e13cacf8bc3edc312c60545281078398e52e34dc5ebb4209649dc
system_lib=$(pkg-config --variable=libdir libpipewire-0.3)/libpipewire-0.3.so.0
[ -e "$system_lib" ] || { echo "no $system_lib"; exit 1; }
ln -sf "$system_lib" "$dir/lib/libpipewire-0.3.so"
cat > "$dir/pc/libpipewire-0.3.pc" <<PC
includedir=$dir/root/usr/include
libdir=$dir/lib

Name: libpipewire
Description: PipeWire 1.0.5's headers over the system's library ($have)
Version: 1.0.5
Requires: libspa-0.2
Libs: -L\${libdir} -lpipewire-0.3
Cflags: -I\${includedir}/pipewire-0.3 -D_REENTRANT
PC
cat > "$dir/pc/libspa-0.2.pc" <<PC
includedir=$dir/root/usr/include

Name: libspa
Description: Simple Plugin API (PipeWire 1.0.5's headers)
Version: 0.2
Cflags: -I\${includedir}/spa-0.2 -D_REENTRANT
PC
line="PKG_CONFIG_PATH=$dir/pc${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
if [ -n "${GITHUB_ENV:-}" ]; then
  echo "$line" >> "$GITHUB_ENV"
fi
echo "export $line"
