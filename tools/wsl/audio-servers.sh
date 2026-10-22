#!/bin/bash
# An audio server in WSL, to open and measure Cantus's PipeWire and JACK hosts there (round
# 30, ADR 0065) — from the packages tools/wsl/fetch-deps.sh unpacked into ~/linux-deps/rt,
# without sudo. WSL has no sound card: PipeWire runs a null sink and a null source (driven
# by its timer, "Dummy-Driver"), JACK its dummy driver; the callbacks and the latency are
# real, the sound goes nowhere.
#
#   source tools/wsl/audio-servers.sh env       # this shell can reach them (clients)
#   bash tools/wsl/audio-servers.sh pipewire    # start PipeWire + WirePlumber
#   bash tools/wsl/audio-servers.sh jack        # start jackd (dummy, 48 kHz, 256 frames)
#   bash tools/wsl/audio-servers.sh stop
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

case "${1:-env}" in
  env) ;;
  pipewire)
    config
    nohup pipewire >"$XDG_RUNTIME_DIR/pipewire.log" 2>&1 &
    sleep 1
    nohup wireplumber >"$XDG_RUNTIME_DIR/wireplumber.log" 2>&1 &
    sleep 2
    pw-cli info 0 | grep -E "version|name" | head -3
    wpctl status 2>/dev/null | sed -n '/Audio/,/Video/p' | head -20
    ;;
  jack)
    nohup jackd --no-realtime -d dummy -r 48000 -p 256 -C 2 -P 2 \
      >"$XDG_RUNTIME_DIR/jackd.log" 2>&1 &
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
    echo "usage: audio-servers.sh env|pipewire|jack|stop"
    exit 1
    ;;
esac
