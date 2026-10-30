# ADR 0079 — MIDI clock and time code, in and out

**Status:** accepted (round 33) · **Date:** 2026-10-29 · **Extends:** ADR 0005 (real
time), ADR 0021 §6 (clock out sketched), ADR 0060 (the tempo map), ADR 0062 (MIDI in, the
controller tap, feedback out) · **With:** ADR 0078 (Link, the same seam)

## Context

The owner's round-33 ask, item 3: Cantus as master — MIDI clock, song position, start/stop/
continue, time code at its four rates — and as slave — following another device's clock
with its jitter filtered, chasing time code — per port, latency compensated, tested with a
loopback. Before this round the input callback dropped every system message
(`Ignore::All`, `parse`), midir's timestamp was thrown away, and MIDI out existed only for
controller feedback on the interface thread.

## Decision

1. **Clock out** (`processor_sync_out.rs`). In each pass of a playing transport, one `F8`
   on every 24th of a beat the transport crosses, **at the sample it crosses it**
   (`Transport::samples_until`: the arithmetic notes are due by — through the tempo map's
   points and ramps, or a followed tempo). Starting at the song's start sends Start
   (`FA`); anywhere else the song position pointer (`F2`, sixteenths) then Continue
   (`FB`); stopping sends Stop (`FC`); a locate while stopped sends the pointer; a jump
   while running (a loop's wrap, a seek) sends Stop, the pointer, Continue — the pointer
   never while the receiver runs. The pointer names the **next** sixteenth and the first
   clock after Continue is stamped on it — a receiver counts from the pointer (the fix
   round: it named the sixteenth before and clocked from the next 24th, 1/12 beat out); a
   start before tick 0 (a count-in, a Link start inside a block) sends Start, its first
   clock on tick 0. Every stamp is the sample **heard**: the engine's sample plus the delay
   compensation it renders ahead by.

2. **Time code out.** Quarter frames (`F1 0nnn dddd`, eight pieces over two frames, the
   rate in the last) at the song's time through the project's tempo map plus the **start
   code** (`hh:mm:ss:ff`, an hour common), each on its quarter of a frame; a full frame
   (`F0 7F 7F 01 01 hh mm ss ff F7`) on every start and locate. 24, 25, 29.97 drop-frame
   and 30: drop-frame skips labels 0 and 1 at each minute but the tenth (17 982 frames in
   ten minutes) — `sync_mtc.rs`, from the MIDI Time Code recommended practice (RP-004,
   RP-008).

3. **MIDI out from the engine** (`sync_out.rs`). The audio thread pushes each message,
   stamped, into a lock-free ring (1 024; a full ring counts a drop, never waits). A thread
   of its own — never the audio thread, never under MMCSS — maps the stamp to host time
   through the audio thread's line (ADR 0078 §4: the processor's sample count against the
   host clock, fitted over 2 048 blocks), adds the device's output latency and the port's
   **offset** (Settings ▸ MIDI, ±100 ms), sleeps on the system's high-resolution timer
   (Rust's `thread::sleep`: `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION` on Windows,
   `nanosleep` elsewhere) until 0.5 ms before, yields to the moment and writes (0.3 ms was
   tried: the timer's own lateness then showed, 90th percentile 0.2–0.25 ms). A waiting
   message names its port by a stable id, so choosing the ports again in another order
   sends it where it was meant. The
   ports it opens are its own (by name, midir); controller feedback keeps its own
   `MidiOutputs`, and a port's **Feedback** column can keep feedback off it.

4. **Input** (`midi_sync.rs`). The ports' callbacks now ignore only active sensing; any
   system message goes to `SyncInput::route` and never reaches the note path. A port's
   clock or time code is kept (a lock-free per-port flag; a ring to the sync thread) only
   when that port is the chosen source; otherwise dropped as before. **Arrival time**:
   midir's stamp (the driver's own on ALSA and CoreMIDI, WinMM's millisecond on Windows)
   mapped to the host clock by the smallest *host − stamp* seen on the port, relaxed 2 µs a
   message — the fastest delivery sets the line, so a callback woken late does not move the
   time a clock is filtered at. The same flags turn a port's **notes** and **controllers**
   off (Settings ▸ MIDI's table); mpe's MIDI-channel work stays in its own functions of
   `midi.rs`.

5. **The seam** (`cantus-engine/src/sync.rs`, MPL, shared with Link): `Timeline` — kind
   (none, Link, clock, time code), tempo, a position (beats; seconds for time code) at a
   host time, the time code's rate against ours, playing, the quantum, start/stop sync, an
   epoch counting discontinuities — in a seqlock of atomics (a reader that keeps meeting a
   write gives up after eight tries and keeps what it had: the audio thread never spins);
   the proposals ring back; the host line; the out ring; the out choices (which messages,
   the rate, the start code); the follower's lock state, error and tempo for the top bar.
   One source at a time, chosen in Settings ▸ MIDI or the top bar.

6. **Following a clock** (`sync_clock.rs`). The arrival times go through a
   **third-order tracking loop** — the fading-memory α-β-γ filter (E. Brookner,
   *Tracking and Kalman Filtering Made Easy*, Wiley 1998, ch. 1): the next clock's time,
   the period and the period's change per clock, moved by `α = 1 − θ³`,
   `β = 1.5(1 − θ)²(1 + θ)`, `2γ = (1 − θ)³` times each clock's error, θ = 0.75 for the
   first beat (a lock within a beat), then 0.90. A second-order delay-locked loop (F.
   Adriaensen, *Using a DLL to filter time*, LAC 2005 — kept, `Dll`, for the time code
   chaser) was measured first: it lagged a 100→140 BPM ramp by **31 ms** at 0.6 Hz and
   **6.8 ms** at 1.5 Hz; the third term follows a ramp. θ was swept: 0.90 gives the best
   ramp (1.3 ms) for the same jitter; 0.95–0.98 ring longer after the acquisition. Start
   plays from 0 at the next clock, Continue from the pointer, Stop stops; a pointer while
   running is a locate; a gap of four periods **resumes the loop with its period**, the
   phase starting again at the next clock (the first fix round dropped it and re-acquired
   from one jittered interval — a tenth off for a while, after every locate; before that a
   2 s silence read 32 BPM, a 100 ms hiccup 25). Clocks lost in a gap while the master plays
   are counted by the time they took; those that then arrive bunched (a driver held them)
   are already counted. 400 ms without a clock while playing is a stop. Our own play waits for the master, and the stop key calls off a start still
   waiting.

7. **Chasing time code** (`sync_mtc.rs`). A full frame locates (stopped); eight pieces in
   order give a whole code (piece 7 is 1¾ frames past it); every quarter frame advances the
   place by **piece number** — `(piece − last piece) mod 8`, elapsed time only telling
   whole runs of eight apart (not one a message: a lost quarter frame left the count 10 ms
   late for good; not by elapsed time: a quarter frame 6 ms late counted two) — and feeds
   the second-order loop (1 Hz) whose period is the master's speed against ours, unless it
   came more than a quarter of a quarter from its predicted moment (a late or bunched one:
   counted, not followed; four in a row restart the loop); a dropout up to 250 ms freewheels and
   resumes with no jump; a whole code more than an eighth of a frame off the count puts the
   count back on it, more than two frames off is a relocate (the epoch). The song time
   less the start code goes through the project's tempo map to ticks; the tempo is the map's
   times the master's speed.

8. **Following, in the engine** — the same code as Link's (ADR 0078 §4): the start on the
   sample, the device's rate, the pull on the phase, the stretches that move nothing
   played, a jump past 30 ms or at a new epoch.

## Consequences

- Cantus drives drum machines, synths' arpeggiators, video and other DAWs, and follows
  them, with one build on every platform (midir).
- WinMM stamps input in whole milliseconds: on Windows the follower's floor is that
  quantisation (the tests' ±1 ms covers it). Windows has **no MIDI loopback without a
  driver** (loopMIDI or Windows MIDI Services): none is installed on the development
  machine, so the end-to-end loopback runs on Linux (ALSA's virtual ports) and the owner
  checks a Windows loopback or real hardware (`docs/beta-checklist.md`).
- A tempo map's ramp under time code chase is followed block by block (the map's tempo
  where the playhead is, the pull taking the rest).

## What the tests hold

- `cantus-engine/tests/sync.rs`: **null** — a seam with nothing on renders every sample of
  the demo project to the bit at blocks 64, 137, 512; clock and time code out touch no
  sample. **Clock out**: Start on sample 0 then a clock every 1 000 samples at 120 BPM,
  **0 samples** of deviation, the same stamps at blocks 1, 64, 137, 512, 1 024; through a
  90→150 BPM ramp each clock within **one sample** (0.999) of the map's own time; a loop's
  wrap sends Stop, the pointer, Continue and the next clock on the wrap's sample. **Time
  code out** at all four rates reads back as the start code plus the song's time, each
  quarter frame within a sample of its quarter.
- `cantus-engine/tests/sync_follow.rs` (device 50 ppm fast, callbacks ±1 ms): MIDI clock
  in with **±1 ms** jitter → worst **0.52 ms**; **±3 ms** → **1.66 ms**; a **100→140 BPM
  ramp** over 10 s (±1 ms) → **1.37 ms**; time code at 25 fps (±1 ms) with an hour's offset
  → **0.27 ms**.
- `cantus-engine/tests/sync_midi.rs`: every frame of two hours round-trips at each rate;
  drop-frame's labels (00:00:59;29 → 00:01:00;02, 00:09:59;29 → 00:10:00;00, an hour is
  107 892 frames); full and quarter frames carry the rate; a broken run reads nothing; the
  chaser freewheels 150 ms and stops after 300; **the out thread at a port** (200 clocks,
  latency and a 2 ms offset added): median lateness **1 µs**, 90th percentile **2 µs**,
  worst **273–520 µs**, none early — once **65 ms** for one message while three builds
  loaded the machine, so the test holds the median and the 90th percentile.
- `cantus-engine/src/midi_sync.rs`: a port gives only what it is taken for; notes and
  controllers turned off; a late callback keeps its stamp's time.
- `cantus-engine/tests/rt_memory.rs`: following and clock/time code out allocate nothing.
- `cantus-engine/tests/midi_loopback.rs` (Linux): clock out through an ALSA virtual port
  into a MIDI input, end to end, 240 clocks against their schedule. The development
  machine's WSL has no ALSA sequencer, so it reported that and passed: the end-to-end
  loopback is on the owner's list with a Windows loopback driver.
