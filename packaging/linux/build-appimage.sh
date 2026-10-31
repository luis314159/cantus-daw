#!/bin/bash
# Cantus's AppImage (ADR 0065): one file that runs on any x86-64 Linux with glibc 2.35 or
# later (Ubuntu 22.04, Debian 12, Fedora 36, and newer), built on Ubuntu 22.04.
#
#   bash packaging/linux/build-appimage.sh <version> <out-dir>
#
# From the repository root, after `cargo build --release` (and packaging/linux/deps.sh).
# Writes <out-dir>/Cantus-<version>-linux-x86_64.AppImage and prints its SHA-256.
#
# What is in it (as the Windows installer carries): the launcher as `cantus`, the program
# twice — `cantus-x86-64-v3` (AVX2) and `cantus-x86-64-v2` — of which the launcher starts
# the highest this processor runs (ADR 0031 §5.3), the plug-in scanner and the plug-in
# sandbox, Cantus's effects as CLAP and VST3 plug-ins (usr/share/cantus/plugins, for other
# hosts), the licence and the notices, the .desktop file, the icons, the AppStream
# metadata and the `.cantus` file type.
#
# libpipewire: the program links it (cpal's PipeWire host). A system without it still
# starts Cantus: AppRun then puts usr/lib/fallback/pipewire first, where a stub of the same
# name answers every call with "nothing" — and the PipeWire host is not offered, since it
# is offered only when PipeWire's socket exists (packaging/linux/README.md).
# libxkbcommon-x11 (and libxcb-xkb): winit opens it at run time for an X11 window and
# cannot do without it; a desktop without it (rare: a bare Wayland one) gets the copies in
# usr/lib/fallback/xkb, taken from the build machine.
#
# CANTUS_APPIMAGE_QUICK=1 packs target/release's program alone (no levels, no plug-ins):
# for trying the packaging itself.
set -euo pipefail

version="${1:?usage: build-appimage.sh <version> <out-dir>}"
out="${2:?usage: build-appimage.sh <version> <out-dir>}"
root="$(pwd)"
[ -f "$root/Cargo.toml" ] && [ -d "$root/packaging/linux" ] || {
  echo "run from the repository root"
  exit 1
}
here="$root/packaging/linux"
cargo="${CARGO:-cargo}"
triple=x86_64-unknown-linux-gnu
tgt="${CARGO_TARGET_DIR:-$root/target}"
work="$tgt/appimage"
app="$work/Cantus.AppDir"
quick="${CANTUS_APPIMAGE_QUICK:-0}"

# The AppImage runtime (type 2, static: no libfuse2 needed), pinned.
runtime_url=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
runtime_sha=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

for tool in mksquashfs gcc nm strip curl sha256sum; do
  command -v "$tool" >/dev/null || {
    echo "missing $tool (packaging/linux/deps.sh installs it)"
    exit 1
  }
done

rm -rf "$work"
mkdir -p "$app/usr/bin" "$app/usr/lib/fallback/pipewire" "$app/usr/lib/fallback/xkb" \
  "$app/usr/share/doc/cantus" "$out"

# --- the programs ---------------------------------------------------------------------
release="$tgt/release"
# cantus-mcp: the MCP stdio bridge (round 34, ADR 0081), started as `Cantus.AppImage
# --mcp-bridge` (AppRun). r34:stems — cantus-ml, the ML helper (ADR 0085), beside the others.
for bin in cantus cantus-launch cantus-scan cantus-sandbox cantus-mcp cantus-ml; do
  if [ ! -x "$release/$bin" ]; then
    "$cargo" build --release -p cantus -p cantus-launch -p cantus-host -p cantus-sandbox -p cantus-mcp -p cantus-ml --bins
    break
  fi
done
install -m755 "$release/cantus-scan" "$release/cantus-sandbox" "$release/cantus-mcp" "$release/cantus-ml" "$app/usr/bin/"
if [ "$quick" = 1 ]; then
  install -m755 "$release/cantus" "$app/usr/bin/cantus"
else
  install -m755 "$release/cantus-launch" "$app/usr/bin/cantus"
  for level in x86-64-v2 x86-64-v3; do
    CARGO_TARGET_DIR="$tgt/cpu-$level" RUSTFLAGS="${RUSTFLAGS:-} -C target-cpu=$level" \
      "$cargo" build --release --target "$triple" -p cantus
    exe="$tgt/cpu-$level/$triple/release/cantus"
    # The program must say the level it was built for (as `xtask package` checks).
    "$exe" --version | tee /dev/stderr | grep -q "($level)" || {
      echo "$exe does not say $level: refusing to package it"
      exit 1
    }
    install -m755 "$exe" "$app/usr/bin/cantus-$level"
  done
fi

# Debug information off (the release profile keeps it for profiling); the symbols stay, so
# a crash's backtrace still names functions.
strip --strip-debug "$app"/usr/bin/*

# --- Cantus's effects as plug-ins for other hosts ------------------------------------
if [ "$quick" != 1 ]; then
  "$cargo" run --release -p xtask -- bundle
  mkdir -p "$app/usr/share/cantus/plugins/clap" "$app/usr/share/cantus/plugins/vst3"
  cp -a "$tgt"/bundled/*.clap "$app/usr/share/cantus/plugins/clap/"
  cp -a "$tgt"/bundled/*.vst3 "$app/usr/share/cantus/plugins/vst3/"
fi

# --- the desktop: .desktop, icons, AppStream, MIME ------------------------------------
id=com.cantusdaw.Cantus
icons=$(ls -dt "$release"/build/cantus-*/out/hicolor 2>/dev/null | head -1)
[ -n "$icons" ] || {
  echo "no icons in $release/build/cantus-*/out/hicolor (cantus-app's build.rs)"
  exit 1
}
mkdir -p "$app/usr/share/icons" "$app/usr/share/applications" "$app/usr/share/metainfo" \
  "$app/usr/share/mime/packages"
cp -a "$icons" "$app/usr/share/icons/"
install -m644 "$here/$id.desktop" "$app/usr/share/applications/"
install -m644 "$here/$id.desktop" "$app/"
sed "s/@VERSION@/$version/; s/@DATE@/$(date -u +%Y-%m-%d)/" "$here/$id.metainfo.xml" \
  >"$app/usr/share/metainfo/$id.metainfo.xml"
install -m644 "$here/$id.mime.xml" "$app/usr/share/mime/packages/$id.xml"
cp "$icons/256x256/apps/$id.png" "$app/$id.png"
ln -s "$id.png" "$app/.DirIcon"
install -m644 "$root/LICENSE" "$app/usr/share/doc/cantus/LICENSE"
install -m644 "$root/THIRD-PARTY-NOTICES.md" "$app/usr/share/doc/cantus/THIRD-PARTY-NOTICES.md"

# --- the libpipewire fallback (see the head) ------------------------------------------
pwlib=""
for d in $(pkg-config --variable=libdir libpipewire-0.3 2>/dev/null) \
  /usr/lib/x86_64-linux-gnu /usr/lib64 /usr/lib; do
  if [ -e "$d/libpipewire-0.3.so.0" ]; then
    pwlib="$d/libpipewire-0.3.so.0"
    break
  fi
done
[ -n "$pwlib" ] || {
  echo "no libpipewire-0.3.so.0 to read its symbols from"
  exit 1
}
bash "$here/stub/make-stub.sh" "$pwlib" "$app/usr/lib/fallback/pipewire/libpipewire-0.3.so.0" \
  "$app"/usr/bin/cantus*

# --- libxkbcommon-x11's fallback (see the head) ----------------------------------------
for lib in libxkbcommon-x11.so.0 libxcb-xkb.so.1; do
  found=""
  for d in ${CANTUS_EXTRA_LIBDIR:-} /usr/lib/x86_64-linux-gnu /lib/x86_64-linux-gnu /usr/lib64 /usr/lib; do
    if [ -e "$d/$lib" ]; then
      found="$d/$lib"
      break
    fi
  done
  [ -n "$found" ] || {
    echo "no $lib on this machine to carry (packaging/linux/deps.sh installs it)"
    exit 1
  }
  cp -L "$found" "$app/usr/lib/fallback/xkb/$lib"
done

install -m755 "$here/AppRun" "$app/AppRun"

# --- the image: runtime + squashfs ----------------------------------------------------
runtime="$tgt/appimage-runtime-x86_64"
if ! echo "$runtime_sha  $runtime" | sha256sum -c - >/dev/null 2>&1; then
  curl -fsSL --retry 3 -o "$runtime" "$runtime_url"
  echo "$runtime_sha  $runtime" | sha256sum -c - || {
    echo "the AppImage runtime is not the pinned one (a proxy's page?): $(head -c 64 "$runtime")"
    exit 1
  }
fi
image="$out/Cantus-$version-linux-x86_64.AppImage"
mksquashfs "$app" "$work/app.squashfs" -root-owned -noappend -comp zstd -quiet
cat "$runtime" "$work/app.squashfs" >"$image"
chmod 755 "$image"

# --- it starts --------------------------------------------------------------------------
APPIMAGE_EXTRACT_AND_RUN=1 "$image" --version
APPIMAGE_EXTRACT_AND_RUN=1 "$image" --audio-drivers || true
sha256sum "$image"
ls -l "$image"
