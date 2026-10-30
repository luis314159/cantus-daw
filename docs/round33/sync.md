# Round 33 — sync: Ableton Link, MIDI clock and time code (ADR 0078, ADR 0079)

Branch `r33/sync` from the contract e0853ec1. Items 2 and 3 of the round.

## What was built

**Ableton Link, in Rust** — `crates/cantus-link` (new, GPL-3.0-or-later, workspace and
default member, in `LICENSE`'s list; one new dependency, `socket2` 0.5, MIT OR Apache-2.0,
in the notices). Wire-compatible with the SDK, followed file by file (ADR 0078 §2 names
which): discovery over UDP multicast `224.76.78.75:20808`, the ghost clock measured by ping
and pong (the median of > 100 estimates), the timeline and its newer-beat-origin rule,
start/stop (Link 3), the older session winning on merge, peers expiring, bye-bye. A
sans-I/O node (`node.rs`) drives both the real sockets (`net.rs`, `link.rs`) and a simulated
network in the tests. One deliberate difference: five unanswered pings *in a row* fail a
measurement (the SDK counts five in all, which fails nearly always at a few per cent loss).
The C++ SDK is not built in (no CMake, no C++ toolchain in any build).

**The engine's abstract seam** — `cantus-engine/src/sync.rs` (MPL, no Link code): a
seqlock timeline (Link, MIDI clock or time code; tempo, a position at a host time, playing,
quantum, start/stop sync, an epoch) the audio thread reads; a ring of proposals it writes
(play, stop, tempo); a least-squares host-time filter of the sample clock (2 048 blocks);
the out ring and choices; lock state for the top bar. Declared from `transport.rs`.

**Following** — `processor_sync*.rs` (children of the processor) and
`transport_follow.rs`: the block's first engine sample is matched at the moment it is heard
(device output latency + delay compensation); a start lands on the sample (fractional
seek); the tempo is the source's times the device's measured rate, pulled by the phase
error (0.3 s, ≤ 1 %, 150 µs deadband); while following, the position is a chain of
stretches — a retune adds one, so nothing already played moves (notes due stay due, a
playing audio clip keeps its frame); an error past 30 ms or a new epoch jumps. The tempo map
is followed, not rewritten (Link overrides a tempo track while on, as Live does). Our own
play/stop while following becomes a proposal (Link) or waits for the master (clock, MTC).

**MIDI clock and time code** — `sync_clock.rs` (messages, the third-order α-β-γ clock
tracker; Adriaensen's DLL kept for MTC), `sync_mtc.rs` (four rates, drop-frame, quarter and
full frames, the chaser), `processor_sync_out.rs` (clock and MTC stamped on their samples:
Start or SPP+Continue, Stop, SPP on a stopped locate, Stop+SPP+Continue on a running jump,
MTC full frame on start/locate), `sync_out.rs` (the MIDI-out thread: host line + device
latency + per-port offset, high-resolution sleep then yield).

**MIDI in** — `midi_sync.rs`: the ports' system messages go to sync with their arrival
time (midir's stamp mapped to the host clock by the fastest delivery), never to the note
path; per-port flags keep a port's clock/MTC (only the source's port) and can turn its
notes and controllers off. `midi.rs`'s input path changed only by calls into it (mpe's
channel work stays separate). Controller feedback can be kept off an output
(`MidiOutputs::set_blocked`).

**The application** — `sync_service.rs`, `sync_service_run.rs` (a thread, 1 ms while a
source is followed, 20 ms at rest: the source port's clock/MTC to its follower, Link's
session to the cell, the proposals to Link); wired on the boot path (`assemble`, the audio
restart, the MIDI rescan; `tests/sync_boot.rs`). **Top bar**: a **Sync** group
(`views/transport_sync.rs`) — LINK (on/off, peers; red if the network failed), Q (the
quantum; a click steps 1/2/3/4/8), EXT (source tempo, lock light) — in two rows; hidden by
default like Live's Link toggle and shown while a source is followed or when the owner shows
it (`TopBarSettings::shown`, additive). **Settings ▸ MIDI** (`views/settings_midi.rs`):
the inputs' table (Notes, Controls, Clock, MTC), the outputs' table (Clock, MTC, Feedback,
Offset), the Sync band (Follow: Off/Link/Clock/MTC, the port, the quantum, start/stop sync,
the code rate, the start code with validation, a live status line, the firewall note).
Strings in `locales/areas/sync.{en,es}.txt`; the manual's *Playing in sync* (EN, ES) and
the top bar's list; ADRs 0078 and 0079; shots in `docs/round33/sync/`.

## The numbers

| What | Measured |
|---|---|
| Link, two simulated peers (clocks seconds apart, 0.2–2 ms delays) | **0.000 ms** phase error, also after a tempo change |
| Link, three peers | 0.125 ms |
| Link, 10 % packet loss, 0.1–5 ms delays | 0.125 ms |
| Link, two handles over loopback UDP | 0.002–0.006 ms |
| **Link, two Cantus processes over loopback** | worst **0.0014–0.003 ms** (58 common instants) |
| Engine following Link (device 50 ppm fast, callbacks ±1 ms) | worst **0.19 ms**; after a change to 132 BPM 0.17 ms; own play lands within 0.03 ms |
| Clock out schedule, 120 BPM, any block size | **0 samples** of jitter; through a 90→150 BPM ramp ≤ 1 sample |
| MTC out, 4 rates | each quarter frame within 1 sample of its time |
| MIDI clock in, ±1 ms jitter | worst **0.52 ms** |
| MIDI clock in, ±3 ms jitter | worst **1.66 ms** |
| MIDI clock in, 100→140 BPM ramp in 10 s (±1 ms) | worst **1.37 ms** (a 2nd-order DLL: 6.8–31 ms) |
| MTC chase, 25 fps, ±1 ms, an hour's offset | worst **0.27 ms** |
| MIDI-out thread at a port (Windows) | median 1 µs late, 90th 16–23 µs, worst 163–259 µs (fix round: yields only the last 0.5 ms) |
| Host-time filter per block | 8.5 µs → **0.14 µs** (fix round); none while sync is idle |
| Time code chase, one quarter frame lost / after a 150 ms dropout | 10.0 ms → **0.000 ms** / 0.000 ms, same epoch (fix round) |
| Time code chase, one quarter frame 0.6 late and two bunched | 7.72 ms → **0.000 ms** (round 2) |
| Closing five Link interfaces | 523 ms → **76 ms** (round 2) |
| Host-time filter, callbacks up to 2 ms late | line within 150 µs; slope within 30 ppm per 5 s (smoothed 10 s more) |

## Tests

New: `cantus-link` (unit + `tests/{wire,sim,udp}.rs`), `cantus-engine`
`tests/{sync,sync_follow,sync_midi,midi_loopback}.rs` and unit tests in `sync.rs`,
`transport_follow.rs`, `sync_clock.rs`, `midi_sync.rs`, a test in `tests/rt_memory.rs`
(following and clock out allocate nothing), `cantus-ui/tests/sync_boot.rs`. The null
tests: a seam with nothing on moves no sample at blocks 64, 137, 512, and clock/MTC out on
moves none either; the golden hashes are untouched (`DSP_VERSION` stays 5).

Test sockets never ask the firewall: `Config::local` binds 127.0.0.1 only (off Windows the
multicast receiver to the group address), held by a test.

**Runs.** Windows (`RUSTFLAGS=-D warnings`): `cargo test -p cantus-link -p cantus-engine
-p cantus-ui` — 2 883 passed, 0 failed, 41 ignored (four failures of the first run fixed and
their targets rerun: a glyph the fonts lack, the layout's column count, the top bar sweep
showing the new group, the out-thread test under three parallel builds); `cargo clippy
--workspace --all-targets` clean. Linux (WSL, `tools/wsl/check.sh r33/sync clippy` in
`~/ci-r33-sync`, then the three crates' tests headless): clippy and the plug-ins without
editor clean; cantus-link 16 passed (two processes: 0.0000 ms), cantus-engine 915, cantus-ui
1 961, none failed. WSL has no ALSA sequencer, so `midi_loopback.rs` reported that and
passed: the end-to-end loopback is on the owner's list. The clone's `target/` is deleted.

## Fix round (the integrator's review of d83643fa)

Every fix has a test that failed before it, except where said.

1. **A clock silence drops the tracker** (`sync_clock.rs`): any gap of four periods
   re-acquires from the next interval. `a_long_silence_then_start_acquires_again` (2 s
   silence then Start: read 61.7 BPM a beat in; now within 1 %, place within 1 ms) and
   `a_hiccup_does_not_become_the_tempo` (five clocks lost: read 85.8 BPM a sixth of a beat
   on; held and delivered at once: passed before too, kept as a guard).
2. **A failed timeline read keeps the last one** (`processor_sync.rs`, `SyncRun::last_tl`):
   `a_failed_read_keeps_the_last_timeline` (the cell held odd for four blocks while locked:
   it unfollowed; now still following and locked), with `TimelineCell::hold_for_tests`.
3. **The chaser counts quarters elapsed** (`sync_mtc.rs`): one lost quarter frame was
   10.0 ms late for good, now 0.000 ms two seconds on; a 150 ms dropout resumes with the
   same epoch and 0.000 ms — `the_chaser_counts_quarters_not_messages`. A whole code more
   than an eighth of a frame off puts the count back (no jump); more than two frames is a
   relocate.
4. **Clock out's pointer and first clock agree** (`processor_sync_out.rs`): the pointer
   names the next sixteenth and the first clock after Continue is stamped on it; a start
   before tick 0 sends Start. `a_continue_off_the_grid_puts_the_slave_on_the_transports_beat`
   (from ticks 1 000, 3 000, 239, 241 a follower fed the stamps reads the transport's
   beat; it read 2.958 for 3.000) and `a_link_start_inside_a_block_sends_start` (it sent
   the pointer to 0 and Continue).
5. **Link's requested beat counts phase from the session's beat 0** (`node_rx.rs`):
   `a_requested_beat_lands_in_the_sessions_phase` (after a tempo change it landed 1.7
   beats off; also after a new session and for beat 2).
6. **Every up IPv4 interface, scanned again every 30 s** (`net.rs`, `link.rs`; new
   dependency `if-addrs` 0.13, MIT OR BSD-3-Clause, in the notices): a handle with no
   interface yet stays on. `an_interface_found_by_a_later_scan_takes_part` (its scanner
   finds nothing first, the loopback later; before, `enable` returned an error). Tests stay
   loopback-only. ADR 0078 updated.
7. **The host-time filter's cost**: refitted every 32 points (a pass per block twice
   before) — **8.5 µs → 0.14 µs a block** (`the_filter_costs_little_per_block`, bound
   4 µs) — and nothing at all while no source is followed and no clock is sent
   (`an_idle_seam_does_no_work`: the host line was published every block).
8. **The stop key calls off a queued start** (`sync_stop_pressed`, one marked call in
   `processor_msg.rs`'s `Stop` arm): `a_stop_calls_off_a_queued_start` (started at the
   quantum anyway).
9. **The out thread's messages keep their port by id** and it yields only the last 0.5 ms
   (it slept to 1 ms before, then spun): `a_waiting_message_keeps_its_port_when_the_ports_change`
   (A's clock went nowhere after a reorder). The yield cannot be made to fail in a test
   (it is CPU time); measured at the port: median 1 µs, 90th 16–23 µs, worst 163–259 µs
   (0.3 ms was tried: 90th 0.2–0.25 ms, the timer's own lateness).
10. **cantus-link against hostile peers**: caps (128 peers, 32 sessions; a session with no
    measurable peer is not kept, nor one whose peers are gone), saturating maths and
    floating-point measurement, beats and times past 2^53 refused, a start/stop stamped a
    minute ahead refused, peers counted without a heap, a bounded datagram queue.
    `hostile_packets_neither_panic_nor_grow_the_node` (overflowed in `Beats + Beats`
    before).
11. **Licence hygiene**: the engine's phase helper is `next_congruent` (modular
    arithmetic) and the filter's doc cites ordinary least squares, no SDK file (ADR 0078
    too); `take_sync` no longer sits between `midi_sender`'s doc and its function. (No
    test: names and comments.)
12. **The loopback test's skip shows**: without an ALSA sequencer (CI, WSL) it prints
    `SKIPPED …` on stderr and passes — it cannot fail there; the loopback is owner-run
    (`docs/beta-checklist.md`).

**Runs after the fix round.** Windows (`RUSTFLAGS=-D warnings`): cantus-link,
cantus-engine and cantus-ui — 2 896 passed, 0 failed, 41 ignored; workspace clippy clean.
Linux (WSL, `~/ci-r33-sync`): clippy and the plug-ins without editor clean; cantus-link 19
(two processes: 0.0123 ms), cantus-engine 925 (the loopback test printed `SKIPPED`),
cantus-ui 1 961 — none failed; the clone's `target/` deleted again.

## Fix round 2

Each with a test that failed before it (run against the old code, then the fix):

- **D1** `sync_mtc.rs`: the chaser counts by piece number, elapsed time only for whole
  runs of eight; a quarter frame far from its predicted moment is counted, not followed.
  `late_or_bunched_quarter_frames_keep_the_count` (one 0.6 of a quarter late, two
  together): worst **7.72 ms → 0.000 ms**.
- **D2** `sync_clock.rs`: a gap resumes the tracker with its period (no re-acquisition),
  clocks lost while playing are counted by the gap's length, and the held ones arriving
  bunched afterwards are not counted twice. `a_hiccup_in_a_jittery_clock_keeps_tempo_and_place`
  (±1.5 ms jitter, five clocks lost: tempo within 2 % at every clock after — it read 0 BPM
  at the first — and the beat right at once, so the engine never jumps).
- **D3** `node_rx.rs`: a measured intercept past 2^53 µs is a failed measurement;
  saturating maths. `an_absurd_measurement_is_refused` (gt = `i64::MAX`: the bogus session
  was joined).
- **D4** `processor_sync.rs`: the host-time fit starts afresh on the edge from idle.
  `turned_on_again_the_clock_is_fitted_afresh` (an hour off with a 20 ppm skew: the line
  was **72.07 ms** off, now 0.0 µs).
- **D5** `link.rs`: interfaces closed together. `closing_several_interfaces_waits_once`
  (five on the loopback: **523 ms → 76 ms**).
- **D6** `link.rs` (`interfaces_open`), `sync_service_run.rs`, `settings_midi.rs`, strings
  EN/ES: Link on with no interface open turns LINK red and Settings says so while it keeps
  looking. `link_with_no_interface_says_so` (it read "0 other apps") and the later-scan
  test now checks the count (0, then 1). `if-addrs` does not expose Linux's
  `IFF_UP|IFF_RUNNING` (noted in ADR 0078).
- **D7** `node_rx.rs`: with others in the session `request_beat_at` changes nothing.
  `a_request_with_others_leaves_the_session_alone` (the shared timeline moved).

**Runs after round 2.** Windows (`RUSTFLAGS=-D warnings`): cantus-link, cantus-engine,
cantus-ui — 2 903 passed, 0 failed, 41 ignored; workspace clippy clean. Linux (WSL): clippy
and the plug-ins without editor clean; cantus-link 22 (two processes 0.0090 ms),
cantus-engine 928 (loopback `SKIPPED`), cantus-ui 1 962 — none failed; the clone's
`target/` deleted.

## Integrated review (after the merge, ce23e294)

- **#7** Only explicit tempos are proposed to Link: the engine no longer proposes when a
  snapshot's map changes (every session projection is a new map; a scene's tempo reached
  Link at a later install, the song's at the performance's end).
  `only_an_explicit_tempo_is_proposed` (two tempos were proposed by installs) and the
  UI's `a_tempo_edit_is_proposed_to_link` (a tempo command while Link is followed asks the
  session for the song's tempo at the playhead; the tempo well already asked directly).
- **#8** A port whose Notes or Controls are turned off still lets go of what the engine
  holds from it (`route_port`: the release of a held key or pedal passes, nothing else).
  `a_held_key_is_released_after_its_port_is_turned_off` (the note hung). The Controls tip
  says that on an MPE port it carries each note's expression (EN/ES).
- **#9** One connection per output port: `midi::pool::OutPool` hands clock out and
  controller feedback the same connection (Windows' class driver serves one client).
  `clock_and_feedback_share_one_connection` (one open, both streams arrive; it cannot run
  against the old code — the pool is new — which opened twice by construction).
- The manual: the ports' table names the MPE and Grid columns; *the session view while
  following*.

**Runs after the integrated review** (on ce23e294 + these fixes). Windows
(`RUSTFLAGS=-D warnings`): cantus-link, cantus-engine, cantus-ui — 3 000 passed, 0 failed,
43 ignored; workspace clippy clean. Linux (WSL): clippy and the plug-ins without editor
clean; cantus-link 22 (two processes 0.0020 ms), cantus-engine 959 (loopback `SKIPPED`),
cantus-ui 2 027 passed and 1 failed — `widgets::bar::tests::the_fill_runs_from_the_detent_and_says_which_way`,
which reads the global palette (`theme::p()`) while the merged `views/session_tests.rs`
installs themes in the same test binary: a race between two lib tests, neither sync's
(it passed on Windows and in every earlier Linux run). The clone's `target/` deleted.

## Seams (what other parts call, and what they can rely on)

- **`Processor::propose_tempo(bpm, tick)`** (`processor_sync.rs`) — the session
  conductor's way to tell the followed Link session a tempo: call it when a scene's tempo
  takes effect (session's `session_rt`, where the engine applies a scene's tempo at its
  launch), with the tick it takes effect at. It is timed to the host moment that tick is
  heard; it does nothing unless Link is followed. Nothing else proposes a tempo.
- **`Processor::sync_stop_pressed()`** — one marked call in session's `processor_msg.rs`
  `Stop` arm: a start queued for the quantum or the master is called off.
- **`Processor::attach_sync` / `EngineHandle::take_sync`** — the seam's two ends; the app's
  sync service takes the handle on the boot path and after an audio restart.
- **`MidiTap::sync_input()`** — per-port flags (clock/MTC kept for sync; Notes/Controls
  off) and the system messages' ring; `midi::route_port` is the callback's entry
  (mpe's `route` stays as it was, called through it).
- **`midi::pool::OutPool`** — the shared output connections; `MidiOutputs::set_pool`
  (controller feedback, grid LEDs) and `sync::out::MidiOut::start` take the same pool.
- **Settings › MIDI** (`settings_midi.rs`): mpe's zone column and launcher's grid column
  are marked lines there.

## Marked lines in files sync does not own

- `cantus-engine/src/processor.rs`: `#[path = "processor_sync.rs"]` and `mod sync_run;`;
  the field `sync: Option<Box<sync_run::SyncRun>>`; its `sync: None` in `new`;
  `self.sync_block(frames);` after `drain_messages`; `song_position_of`'s
  `!transport.following() &&`.
- `cantus-engine/src/processor_pass.rs`: `self.sync_pass(done, n);` before the advance.
- `cantus-engine/src/processor_msg.rs` (fix round): the `Stop` arm in braces with
  `self.sync_stop_pressed();`.
- `cantus-ui/src/app.rs` (integrated review): `exec` and `exec_merging` note whether a
  command edits the tempo and call `sync_service::tempo_edited` on success (2 lines each).
- `cantus-engine/src/backend.rs`: the field `sync: Option<SyncHandle>`; the pair and
  `attach_sync` in `start` (2 lines) and `manual` (3 lines); `sync: Some(sync)` and
  `sync: None` in the two constructors; `take_sync()` (3 lines).
- `cantus-ui/src/app.rs`: the field `sync` and its default; `sync_service::wire(&mut app)`
  in `assemble`; `engine_replaced(self)` in `restart_audio`; `apply(self)` in `rescan_midi`.
- `cantus-ui/src/settings.rs` (additive, `#[serde(default)]`): `Settings::sync` and its
  default; `TopBarSettings::shown`.
- `cantus-ui/src/views/transport.rs` (the top bar's groups): the module declarations of
  `transport_sync.rs` and `../sync_service.rs`; the two `Group::Sync` arms; `shown_into`'s
  `in_use` and `is_shown` (2 lines).
- `cantus-ui/src/views/transport_tempo.rs` (the tempo group): the gesture handed to
  `take_tempo_gesture` and the tempo shown through `followed_tempo` (2 lines).
- `cantus-ui/src/settings.rs`'s test and `cantus-ui/tests/topbar_layout.rs`: the new
  `TopBarSettings::shown` in their initialisers, the sync group shown for the width test.
- `cantus-ui/src/views/transport_more.rs` (the customise panel): `shown` cleared on reset,
  `is_shown` (2 lines), the shown list kept on a click (4 lines).
- `cantus-ui/src/views/workspace_settings.rs` (sync owns the MIDI page): `Page::midi_outputs`;
  the MIDI page now calls `settings_midi.rs`; `demo_midi` for the shots.
- `cantus-engine/tests/rt_memory.rs`: one appended test (`following_and_clock_out_never_allocate`).
- `docs/beta-checklist.md`: the round-33 sync checks.
- For mpe and launcher: `settings_midi.rs` has `// r33:mpe` and `// r33:launcher` comments
  where the MPE zone column and the controller-grid choice go (heading and cell).

## What needs the owner

- **A real Link app**: Live, a phone app or another PC on the same network (the SDK's
  LinkHut could not be built here — this machine's permissions refused compiling outside
  code). Windows' firewall asks once when the shipped app turns Link on.
- **Real MIDI hardware** for clock and MTC, in and out; **a Windows MIDI loopback**
  (loopMIDI or Windows MIDI Services — none is installed here).
