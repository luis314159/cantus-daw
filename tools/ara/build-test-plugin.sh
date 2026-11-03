#!/usr/bin/env bash
# Builds Celemony's ARATestPlugIn (VST3) from the ARA SDK's tagged release, for the ARA
# hosting tests (ADR 0097). Nothing it fetches or builds is committed: the SDK and the VST3 SDK
# it pulls are test tools, never shipped with Cantus.
#
#   tools/ara/build-test-plugin.sh [work-dir]
#
# work-dir defaults to $CANTUS_ARA_DIR, else ../downloads/ara beside the repository's parent
# (F:\projects\daw-r36\downloads\ara on the owner's machine). The plug-in lands in
# <work-dir>/build/ARATestPlugIn.vst3; the tests find it there through CANTUS_ARA_TEST_PLUGIN
# or that default, and skip (saying why) when it is missing.
#
# Same steps on Windows (Git Bash, the CMake that comes with Visual Studio) and Linux (cmake,
# g++, make), as CI would run them.
set -euo pipefail

ARA_TAG="releases/2.3.0"
ARA_COMMIT="0005c532bb03eb210e7bb490329fc6d7ba1792c7" # ARA_SDK at that tag
here="$(cd "$(dirname "$0")/../.." && pwd)"
work="${1:-${CANTUS_ARA_DIR:-$here/../downloads/ara}}"
mkdir -p "$work"
work="$(cd "$work" && pwd)"

cmake_bin="$(command -v cmake || true)"
if [ -z "$cmake_bin" ]; then
    for c in "/c/Program Files/Microsoft Visual Studio/2022/"*/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe \
             "/c/Program Files (x86)/Microsoft Visual Studio/2022/"*/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe; do
        if [ -x "$c" ]; then cmake_bin="$c"; break; fi
    done
fi
[ -n "$cmake_bin" ] || { echo "cmake not found (install it, or Visual Studio's C++ CMake tools)" >&2; exit 1; }

sdk="$work/ARA_SDK"
if [ ! -d "$sdk/.git" ]; then
    git clone --depth 1 --branch "$ARA_TAG" --recurse-submodules --shallow-submodules \
        https://github.com/Celemony/ARA_SDK.git "$sdk"
fi
got="$(git -C "$sdk" rev-parse HEAD)"
[ "$got" = "$ARA_COMMIT" ] || { echo "ARA_SDK is at $got, expected $ARA_COMMIT ($ARA_TAG)" >&2; exit 1; }

# The VST3 SDK subset the examples need, at the version the ARA SDK pins.
"$cmake_bin" -DVST3_SDK_DIR="$sdk/vst3sdk" -P "$sdk/install_vst3sdk.cmake"

build="$work/build-cmake"
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) gen=(-G "Visual Studio 17 2022" -A x64) ;;
    *) gen=(-DCMAKE_BUILD_TYPE=Release) ;;
esac
"$cmake_bin" -S "$sdk/ARA_Examples" -B "$build" "${gen[@]}" \
    -DARA_VST3_SDK_DIR="$sdk/vst3sdk" -DARA_SETUP_DEBUGGING=OFF
"$cmake_bin" --build "$build" --config Release --target ARATestPlugInVST3 --parallel 4

# Gather the bundle where the tests look for it.
out="$work/build"
rm -rf "$out/ARATestPlugIn.vst3"
mkdir -p "$out"
bundle="$(find "$build" -type d -name 'ARATestPlugIn.vst3' | head -n 1)"
if [ -n "$bundle" ]; then
    cp -r "$bundle" "$out/"
else
    # A build that made a bare binary: wrap it in the standard bundle layout.
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*) bin="$(find "$build" -name 'ARATestPlugIn.vst3' -type f | head -n 1)"; arch=x86_64-win; ext=vst3 ;;
        *) bin="$(find "$build" -name 'ARATestPlugIn.so' | head -n 1)"; arch=x86_64-linux; ext=so ;;
    esac
    [ -n "$bin" ] || { echo "the build made no ARATestPlugIn binary" >&2; exit 1; }
    mkdir -p "$out/ARATestPlugIn.vst3/Contents/$arch"
    cp "$bin" "$out/ARATestPlugIn.vst3/Contents/$arch/ARATestPlugIn.$ext"
fi
echo "ARATestPlugIn: $out/ARATestPlugIn.vst3"
