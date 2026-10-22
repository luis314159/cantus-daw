#!/bin/bash
# An audio server in WSL, to open and measure Cantus's PipeWire and JACK hosts there (round
# 30, ADR 0065) — from the packages tools/wsl/fetch-deps.sh unpacked into ~/linux-deps/rt,
# without sudo. WSL has no sound card: PipeWire runs a null sink and a null source (driven
# by its timer, "Dummy-Driver"), JACK its dummy driver; the callbacks and the latency are
# real, the sound goes nowhere.
#
#   bash tools/wsl/audio-servers.sh pipewire    # start PipeWire + WirePlumber
#   bash tools/wsl/audio-servers.sh jack        # start jackd (dummy, 48 kHz, 256 frames)
#   bash tools/wsl/audio-servers.sh run <cmd>   # <cmd> in ~/cantus-ci (check.sh's build
#                                               # environment), able to reach them
#   bash tools/wsl/audio-servers.sh stop
#
# For example, after `check.sh <branch> test` checked the branch out:
#   bash tools/wsl/audio-servers.sh run env CANTUS_TEST_DRIVER=PipeWire CANTUS_TEST_BUFFER=128 \
#     cargo test -p cantus-engine --test devices -- --ignored live_streams --nocapture
rt="$HOME/linux-deps/rt"
root="$HOME/linux-deps/root"
lib="$rt/usr/lib/x86_64-linux-gnu"
export XDG_RUNTIME_DIR="${CANTUS_AUDIO_RUNTIME:-/tmp/cantus-audio}"
mkdir -p -m 700 "$XDG_RUNTIME_DIR"
export LD_LIBRARY_PATH="$lib:$root/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export SPA_PLUGIN_DIR="$lib/spa-0.2"
export PIPEWIRE_MODULE_DIR="$lib/pipewire-0.3"
export PIPEWIRE_CONFIG_DIR="$XDG_RUNTIME_DIR/pipewire"
export WIREPLUMBER_MODULE_DIR="$lib/wireplumber-0.4"
export WIREPLUMBER_DATA_DIR="$rt/usr/share/wireplumber"
export WIREPLUMBER_CONFIG_DIR="$rt/usr/share/wireplumber"
export JACK_DRIVER_DIR="$lib/jack"
export PATH="$rt/usr/bin:$PATH"

config() {
  mkdir -p "$PIPEWIRE_CONFIG_DIR/pipewire.conf.d"
  cp -f "$rt"/usr/share/pipewire/*.conf "$PIPEWIRE_CONFIG_DIR/"
  cat >"$PIPEWIRE_CONFIG_DIR/pipewire.conf.d/cantus-null.conf" <<'CONF'
context.properties = {
    default.clock.rate = 48000
    default.clock.quantum = 256
    default.clock.min-quantum = 32
}
context.objects = [
    { factory = adapter
      args = {
          factory.name = support.null-audio-sink
          node.name = "cantus-null-sink"
          node.description = "Null sink"
          media.class = "Audio/Sink"
          audio.position = [ FL FR ]
          object.linger = true
          monitor.channel-volumes = true
      }
    }
    { factory = adapter
      args = {
          factory.name = support.null-audio-sink
          node.name = "cantus-null-source"
          node.description = "Null source"
          media.class = "Audio/Source/Virtual"
          audio.position = [ FL FR ]
          object.linger = true
      }
    }
]
CONF
}

case "${1:-}" in
  run)
    shift
    # check.sh's build environment (the -dev packages in ~/linux-deps/root).
    export PATH="$HOME/.cargo/bin:$root/usr/bin:$PATH"
    export PKG_CONFIG_PATH="$root/usr/lib/x86_64-linux-gnu/pkgconfig:$root/usr/share/pkgconfig"
    export PKG_CONFIG_SYSROOT_DIR="$root"
    export OPENSSL_DIR="$root/usr" OPENSSL_LIB_DIR="$root/usr/lib/x86_64-linux-gnu"
    export OPENSSL_INCLUDE_DIR="$root/usr/include"
    export LIBCLANG_PATH="$root/usr/lib/llvm-18/lib"
    export BINDGEN_EXTRA_CLANG_ARGS="-I$root/usr/lib/llvm-18/lib/clang/18/include"
    export RUSTFLAGS="-D warnings" CARGO_INCREMENTAL=0
    export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
    cd "${CANTUS_CI_DIR:-$HOME/cantus-ci}" || exit 1
    exec "$@"
    ;;
  pipewire)
    config
    # Their own sessions: WirePlumber quits on the SIGHUP of a closing shell, nohup or not.
    setsid -f pipewire >"$XDG_RUNTIME_DIR/pipewire.log" 2>&1 </dev/null
    sleep 1
    setsid -f wireplumber >"$XDG_RUNTIME_DIR/wireplumber.log" 2>&1 </dev/null
    sleep 2
    pw-cli info 0 | grep -E "version|name" | head -3
    wpctl status 2>/dev/null | sed -n '/Audio/,/Video/p' | head -20
    ;;
  jack)
    setsid -f jackd --no-realtime -d dummy -r 48000 -p "${CANTUS_JACK_PERIOD:-256}" -C 2 -P 2 \
      >"$XDG_RUNTIME_DIR/jackd.log" 2>&1 </dev/null
    sleep 2
    tail -3 "$XDG_RUNTIME_DIR/jackd.log"
    ;;
  stop)
    pkill -f "$rt/usr/bin/wireplumber" 2>/dev/null
    pkill -f "$rt/usr/bin/pipewire" 2>/dev/null
    pkill -f "$rt/usr/bin/jackd" 2>/dev/null
    pkill -x wireplumber 2>/dev/null
    pkill -x pipewire 2>/dev/null
    pkill -x jackd 2>/dev/null
    true
    ;;
  *)
    echo "usage: audio-servers.sh pipewire|jack|run <command>|stop"
    exit 1
    ;;
esac
