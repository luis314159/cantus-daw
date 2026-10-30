# ADR 0078 — Ableton Link

**Status:** accepted (round 33) · **Date:** 2026-10-29 · **Extends:** ADR 0005 (real
time), ADR 0021 §6 (its sketch of tempo sync), ADR 0047 (licences), ADR 0060 (the tempo
map) · **With:** ADR 0079 (MIDI clock and time code, the same seam)

## Context

BACKLOG §0.2 and the owner's round-33 ask: tempo, phase and start/stop shared with every
Link app on the local network (Live, Bitwig, Traktor, phone apps, another Cantus), the
output latency compensated, tested against another Cantus and a Link app if possible.

**The licence.** The Link SDK (github.com/Ableton/link) is dual-licensed GPL-2.0-or-later
or proprietary. GPLv2-or-later code may go into the GPL-3.0-or-later application, never into
the MPL-2.0 engine crates (ADR 0047). ADR 0021 had sketched building the SDK's C wrapper
with a C++ toolchain; that would put CMake and a C++ compiler into every build of every
platform and the asio library into the program.

## Decision

1. **A crate of its own, `crates/cantus-link`, GPL-3.0-or-later**: Link's protocol in Rust,
   wire-compatible with the SDK, following its source file by file. The C++ SDK is not
   built into the app: the protocol is small (a few message types, a median, a few rules)
   and a Rust implementation keeps one toolchain, one build and code we can test without
   sockets. The crate is a workspace member and default member and in `LICENSE`'s list; it
   depends only on `log` and `socket2` (MIT OR Apache-2.0, for `SO_REUSEADDR` and the
   multicast options std does not expose).

2. **The protocol** (which part follows which SDK file):
   - `wire.rs` — the bytes: discovery's `_asdp_v\x01` header, type (alive 1, response 2,
     bye-bye 3), time to live, group 0, node id (`discovery/v1/Messages.hpp`); payload
     entries key/size/value, unknown keys skipped (`discovery/Payload.hpp`); `tmln` (µs per
     beat, beat origin in µbeats, time origin µs), `sess`, `stst` (Link 3's start/stop),
     `mep4` (`link/Timeline.hpp`, `Tempo.hpp`, `Beats.hpp`, `SessionId.hpp`,
     `StartStopState.hpp`, `EndpointV4.hpp`, `PeerState.hpp`); measurement's `_link_v\x01`,
     ping 1 / pong 2, `__ht`, `__gt`, `_pgt` (`link/v1/Messages.hpp`,
     `link/PayloadEntries.hpp`). Big-endian throughout
     (`discovery/NetworkByteStreamSerializable.hpp`).
   - `timeline.rs` — microbeats rounded half away from zero, tempo travelling as whole
     µs per beat, timelines, the ghost transform, phase and phase matching, the tempo change
     that makes a newer beat origin (`Beats.hpp`, `Tempo.hpp`, `Timeline.hpp`,
     `GhostXForm.hpp`, `Phase.hpp`, `ClientSessionTimelines.hpp`, `Link.ipp`).
   - `measure.rs` — the measurement of a session's ghost clock: ping with the host time,
     pong with the ghost time and the ping echoed, two estimates per round trip, the median
     of more than 100 (`Measurement.hpp`, `PingResponder.hpp`, `MeasurementService.hpp`,
     `Median.hpp`). **One deliberate difference**: a ping unanswered for 50 ms is resent at
     most five times *in a row*; the SDK counts five over the whole measurement, so on a
     network losing a few per cent of packets nearly every one of its ~150 round trips'
     measurements fails. Nothing on the wire changes.
   - `node.rs`, `node_rx.rs` — the participant without I/O: alive every 250 ms (time to
     live 5 s, ratio 20), at most every 50 ms on a change, answered by a unicast response,
     peers forgotten after their time to live, bye-bye on leaving
     (`discovery/UdpMessenger.hpp`, `PeerGateway.hpp`); sessions: a node founds its own with
     a ghost clock starting at zero, measures any other session it sees, and joins the
     **older** (the larger ghost time by more than 0.5 s, or the smaller id within it); a
     joined session is measured again every 30 s; alone again, it founds a new one, the beat
     kept; within a session the timeline with the larger beat origin wins, start/stop the
     newer timestamp (`Controller.hpp`, `Sessions.hpp`, `Peers.hpp`).
   - `net.rs`, `link.rs` — the sockets as `platforms/asio/Context.hpp` opens them, per
     interface — the loopback and **every IPv4 address of every interface that is up**
     (`if-addrs`, MIT OR BSD-3-Clause), scanned again every 30 s as the SDK's
     `InterfaceScanner.hpp` does: an interface gone is closed and its peers forgotten, a
     new one opened in a free slot (the others keep their index), and a handle turned on
     with no network yet stays on and takes part from the scan that finds one (the fix
     round: only the interface routing the group was used, on this machine a WSL switch,
     and never looked for again). Per interface: a
     multicast receiver on `0.0.0.0:20808` with `SO_REUSEADDR` (several Link apps on one
     machine), joined on the interface, multicast loop only on the loopback; a sender and a
     measurement socket bound to the interface's address. Each socket has a thread that
     stamps a datagram the moment it arrives; pings are answered on the measurement thread at
     once. A ping's host time is written as it leaves.

3. **The engine sees only an abstract timeline** (`cantus-engine/src/sync.rs`, MPL, ours,
   no Link code — see ADR 0079 §5 for the seam itself): a seqlock of atomics the audio
   thread reads (kind, tempo, a beat at a host time, playing, the quantum, start/stop sync,
   an epoch) and a ring it writes its proposals into (play, stop, a tempo). The
   application's sync thread (`cantus-ui/src/sync_service*.rs`) owns the `Link` handle,
   copies its session into the cell every millisecond and hands the proposals to Link.
   Link, MIDI clock in and time code in are three sources of that seam; one is followed
   at a time.

4. **Following, in the engine** (`processor_sync*.rs`, `transport_follow.rs`):
   - **The clock.** Each block's start (the processor's sample count) and the host time it
     ran at feed an ordinary least-squares line over the last 2 048 blocks
     (`HostTimeFilter`, refitted every 32 blocks: 0.14 µs a block; nothing at all while
     sync is idle). Its slope is the device's real rate against the host clock (tens of
     ppm on real hardware), smoothed over ten seconds more. The engine side cites only
     mathematics (least squares, modular arithmetic for phase), never an SDK file: the
     SDK is GPL and the engine MPL.
   - **Latency.** The block's first engine sample is heard at the line's host time plus the
     device's output latency (`DeviceTelemetry`, the driver's playback minus callback time)
     plus the delay compensation the engine renders ahead by (`pdc.total`). That moment's
     session beat is what our beat is matched with: what Cantus plays reaches the speakers
     when the session's beat does.
   - **Start.** Press play (or the session starts, with start/stop sync): the transport
     waits, stopped, for the next session beat in the phase of the playhead within the
     quantum, then starts **on that sample** of its block (a fractional seek,
     `Transport::seek_f64`, so the start is sample-accurate where the transport is). Alone
     with start/stop sync, Cantus asks the session for beat 0 30 ms ahead, so a press
     starts at once.
   - **Tempo and phase.** The tempo is the session's times the device's rate, plus a
     proportional pull on the phase error (0.3 s, at most 1 %, nothing within 150 µs). A
     retune is at most every half second, for a change of 10 ppm or more.
   - **Nothing jumps.** The tempo map is followed, not rewritten: while following, the
     transport's position is a chain of stretches (`transport_follow.rs`); a retune adds
     one at the playhead, so every sample already played keeps its tick — a note due stays
     due, an audio clip keeps reading from the frame it reached. Full, the chain merges the
     neighbours whose union moves the past least (a fraction of a sample). An error past
     30 ms (the session jumped) is a jump instead; whole quanta of renumbering (a joined
     session) only move our offset.
   - **A project with a tempo track**: Link overrides it while on, as Live does; the map
     comes back (a re-anchor at the playhead) when Link is off.
   - **Null:** a processor never given a seam returns at the first line of the hook; with
     a seam and nothing followed, no sample moves (`tests/sync.rs`).

5. **The interface.** The top bar's **Sync** group (`views/transport_sync.rs`): **LINK**
   (on/off, the number of peers; red when the network could not be opened), **Q** (the
   quantum), **EXT** (the source, its tempo, the lock light); the group is hidden by
   default (as Live's Link toggle) and shows itself while a source is followed. The tempo
   well shows the session's tempo while Link is followed, and dragging it (or the wheel)
   asks the session for a new one — the project's own tempo is not written; a change of the
   project's tempo (an edit, the double-click) is proposed to the session too. Settings ▸ MIDI's **Sync**
   band: the source, the quantum, start/stop sync, the firewall note. The manual's
   *Playing in sync* (EN, ES).

6. **The firewall.** The shipped app enabling Link listens on UDP 20808 on the network, as
   Live does: Windows Defender Firewall asks once to allow Cantus (on private networks) —
   said in the manual and in Settings ▸ MIDI. **The tests never ask**: `Config::local`
   speaks on the loopback alone with a port of the tests' own, every socket bound to
   127.0.0.1 (off Windows, the multicast receiver to the group itself), and a test holds
   it (`net::tests::a_local_handle_binds_the_loopback_only`).

## Consequences

- Cantus plays with Live, Bitwig and every Link app with no C++ in its build, on every
  platform the rest of it builds on.
- Interop with the SDK is held by byte vectors written out from its headers and by the
  protocol's rules followed file by file; **a real Link app was not run here**: building
  the SDK's LinkHut in WSL (g++ and asio, no CMake) was refused by this machine's
  permissions for code from outside, so it is on the owner's list (`docs/beta-checklist.md`:
  Live, a phone app, another PC).
- Every IPv4 interface `if-addrs` lists takes part, scanned every 30 s; IPv6 (`mep6`,
  `ff12::8080`) is not spoken. `if-addrs` does not expose a Linux interface's
  `IFF_UP | IFF_RUNNING`, so an interface that is down still gets sockets (nothing arrives
  on them; an address that vanishes is closed at the next scan). Several interfaces are
  closed together — every listener told, then waited for — so a rescan or leaving waits
  one receive timeout (100 ms), not one per interface (round 2: 523 → 76 ms for five).
- **No interface open** (no network, port 20808 refused everywhere): Link stays on and
  keeps looking every 30 s, and says so — the top bar's LINK lamp turns red and Settings ›
  MIDI reads "no network interface could be opened" (round 2: it read "on, 0 peers").
- **Hostile peers** (the fix round): at most 128 peers and 32 other sessions are kept (a
  flood stops there; a session with no measurable peer is never kept); beats and times
  past 2^53 are refused, a start/stop of our session stamped more than a minute ahead is
  refused (one at the end of time froze start/stop), all arithmetic on a peer's numbers
  saturates or is done in floating point, and the sockets' datagrams wait in a bounded
  queue of 1 024.
- `request_beat_at` counts phase from the session's beat 0 (what the SDK's client timeline
  counts from), not from its beat origin (the fix round: after a tempo change a start alone
  landed 1.7 beats off). With others in the session it changes nothing: the SDK renumbers
  only the asking client's own beats then, which a node whose owner maps phase itself does
  not need, and renumbering the shared session by our quantum would shift a peer on
  another (round 2).
- A measured clock offset past 2^53 µs (a peer's pongs set it) is a failed measurement:
  never a session joined, never our own clock (round 2: an `i64::MAX` ghost time made a
  bogus session the "oldest").
- Link Audio (the SDK's newest, `aep4`/`aep6`) is skipped on the wire, not implemented.

## What the tests hold

- `cantus-link/tests/wire.rs`: an alive message, a bye-bye, a ping and a pong byte for byte
  against vectors written from the SDK's headers (cited); unknown entries skipped, short
  ones refused; tempo as whole µs per beat.
- `cantus-link/tests/sim.rs` (real node code, simulated wire, clocks seconds apart, delays
  0.1–5 ms each way): two peers converge — **0.000 ms** phase error — and keep it through a
  tempo change; three peers join (0.125 ms), one leaves with bye-bye, one falls silent and
  expires, the last founds anew; **10 % packet loss**: 0.125 ms; start/stop reaches every
  peer.
- `cantus-link/tests/udp.rs` (real sockets, loopback only): two handles in one process
  find each other (0.002–0.006 ms) and follow a tempo change; **two Cantus processes**
  converge to a worst **0.0014–0.003 ms** over their last three seconds.
- `cantus-engine/tests/sync_follow.rs`: following a 120 BPM session on a device 50 ppm fast
  with callbacks jittering ±1 ms: worst phase error **0.19 ms** after settling, **0.17 ms**
  after a change to 132 BPM; never a jump once locked; our own play with start/stop off
  waits for the quantum and lands within **0.03 ms** of the phase.
- `cantus-engine/tests/rt_memory.rs`: following Link, a tempo change, a jump, a stop, a
  clock source and none, with clock and time code out — not one allocation.
