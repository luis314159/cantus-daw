# Round 33 — Ableton's core: the session view, Link, MIDI clock and MTC, MPE

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians
3:23). Tests before code, nothing half done, look at what you build (`shoot` examples).
**The owner's rule:** every gap a feature leaves is closed in the same round. Whatever you
would write under "Left" comes back to you as work; only list as open what truly needs the
owner (their ears, their hardware, a paid account, a decision).

**Run lean** (as rounds 30–32): read only what your brief needs, prefer `grep` and line
ranges to whole files, keep reports short and exact.

The ask (the owner, 2026-10-28, `docs/next-session.md`):

0. What round 32 left: nothing — the last `macos.yml` run (public 809644f8) was green with
   nothing retried, no issues are open on `cantus-daw`, no owner notes were given, the Mac
   session has not happened (beta.8 stays Windows + Linux).
1. **The session view** (Ableton Live and Bitwig as the reference, FL's shape where it
   helps): a grid of clips per track and scenes; clips and scenes launched quantised to the
   bar (or a chosen fraction); follow actions (next, previous, random, other, with chances
   and repeats); recording into a slot (audio and MIDI, with count-in and loop); legato and
   launch modes (trigger, gate, toggle, repeat); stop per track and stop all; **recording
   what is launched into the arrangement** (the played session becomes arrangement, one undo
   step) and taking arrangement clips into the session. Switching between arrangement and
   session at once; both share tracks, mixer and automation (the session's per clip).
   Keyboard, MIDI learn and controllers (a Launchpad/APC-style grid if it fits with
   quality). All in the kit, both themes. Null tests: a project without a session sounds
   the same to the bit; launching is sample-accurate; offline equals real time.
2. **Ableton Link** — tempo, phase and start/stop shared on the local network, output
   latency compensated. The licence first (below), tested against another Cantus instance
   and, if possible, a Link test app.
3. **MIDI clock and MTC, in and out**: Cantus as master (clock, SPP, start/stop/continue,
   MTC at its four frame rates) and as slave (following another device's clock with its
   jitter filtered, chasing MTC), per port, latency compensated; tested with a MIDI
   loopback.
4. **MPE end to end**: recording and editing MPE in the piano roll (bend, pressure and
   timbre per note), native instruments that understand it (Unda, Calor, Modulus at least)
   and hosted ones (CLAP note expressions, VST3 note expression), zone configuration, MPE
   MIDI files in and out. Then MIDI 2.0 if it fits (otherwise measured and written down).
5. The owner's frictions and the beta's users: none at the start; the integrator hands out
   whatever arrives during the round.

Read, in order: this file (the rules and **your** section); `CLAUDE.md`; `docs/STATUS.md`'s
header; the ADRs your brief names.

## The builders

All start from the contract commit on `r33/integration`. **Never more than three compile
at once** at first (a 31 GB machine); a fourth only with the integrator's free-RAM check.

| Builder | Branch / worktree | Delivers | ADR | Schema | Other numbers |
|---|---|---|---|---|---|
| **session** | `r33/session`, `F:\projects\daw-r33\session` | item 1's model, commands, engine, recording | 0076 | 46 | — |
| **launcher** | `r33/launcher`, `F:\projects\daw-r33\launcher` | item 1's view, keyboard, MIDI learn, controller grids | 0077 | — | keymap 14 |
| **sync** | `r33/sync`, `F:\projects\daw-r33\sync` | items 2 and 3 | 0078, 0079 | — | — |
| **mpe** | `r33/mpe`, `F:\projects\daw-r33\mpe` | item 4 | 0080 | 47 | sandbox `PROTOCOL` 7 |

`session`, `sync` and `mpe` start now. **`launcher` starts when session's first delivery
(the model, the commands and the engine's message API) is merged into `r33/integration`**;
it branches from that merge.

The locale areas are declared: `session` (session and launcher share it, each in a block
headed by its name), `sync`, `mpe` — `crates/cantus-ui/locales/areas/` and `i18n.rs`.

## Who owns what (one owner per hot file)

| File(s) | Owner | Others |
|---|---|---|
| `cantus-core/src/session*.rs` (new), `Project.session`, the session `Command`s | session | launcher calls them |
| `cantus-engine/src/snapshot.rs`, `processor.rs`, `processor_msg.rs`, `processor_snap.rs`, `processor_rec.rs`, `graph/track.rs`, `messages.rs` | session | marked lines (below) |
| `cantus-ui/src/recording.rs`, `record_pass.rs`, `record_session.rs`, `views/transport/count_in*` | session | mpe: marked lines |
| `cantus-ui/src/views/session*.rs` (new), `Pane::Session`, `keymap.rs` (keymap 14) | launcher | others name a binding they want in their report |
| `cantus-core/src/controller*.rs`, `cantus-ui/src/controllers*.rs`, `views/controllers.rs` | launcher | — |
| `crates/cantus-link` (new crate), root `Cargo.toml` members, `LICENSE`'s crate list | sync | — |
| `cantus-engine/src/transport.rs`, `midi.rs`, `cantus-engine/src/sync*.rs` (new) | sync | mpe: marked lines in `midi.rs` |
| `views/workspace_settings.rs`'s MIDI page, `views/transport_layout.rs` and the top bar's groups | sync | launcher, mpe: marked lines |
| `cantus-core/src/model.rs`'s `Note`, `clip_lanes*.rs`, `cantus-engine/src/inst/**`, `cantus-dsp`'s instruments | mpe | — |
| `cantus-host/**`, `cantus-sandbox/**` (the wire, `PROTOCOL` 7) | mpe | — |
| `cantus-project/src/midi/**`, `views/piano_roll*.rs` | mpe | — |
| `cantus-project/src/format.rs` (`SCHEMA_VERSION`, migrations), `fixtures/` | session (46), mpe (47) | the integrator renumbers if one lands first |
| `cantus-ui/src/settings.rs` | anyone, **additive fields only**, `#[serde(default)]` | — |
| `crates/cantus-core/tests/undo_invariants.rs` | anyone, **in your own `srng` stream and helper fn** (round 32's pattern, `round32_score_command`); never change `below(49)` | — |

**Marked lines.** When you must call your code from a file another builder owns, put your
code in a new file of your own (declare it with `#[path]` inside one of your files where you
can, so shared `lib.rs`/`mod.rs` stay untouched) and add the fewest lines that call it,
each marked `// r33:<your name>`. List every marked line in your report. Never reformat or
move code in a file you do not own.

## Rules (the usual ones, short)

- Work only in your worktree. Commit WIP often (`r33/<name>`); a cut session resumes.
  Write files in pieces under ~300 lines per tool call.
- Machine: `export PATH="$HOME/.cargo/bin:$PATH"` (cargo is not on PATH); your untracked
  `.cargo/config.toml` sets `jobs = 3`, no debug info and incremental off (never `git add`
  it); `TMP`/`TEMP` = `F:\projects\daw-r33\tmp`. Test executables named `*setup*`/
  `*update*` need elevation on this machine: run them copied to a neutral name. F: and C:
  are nearly full: no extra target folders, no release builds unless your brief asks.
- Windows: `cargo clippy --workspace --all-targets` and the tests of every crate you touch,
  with `RUSTFLAGS=-D warnings`; `cargo fmt -p <crate>` for each crate you touched before
  each commit (`cargo fmt --all` fails here with *os error 206*). The Bash tool is Git
  Bash: a heredoc delimited by `EOF` fails to parse — write files with the Write tool.
  Linux, before you report: write a small script file and run `wsl -d Ubuntu-24.04 --
  bash /mnt/f/…/script.sh` (inline `bash -lc '…'` loses `$VAR`s), the script calling
  `CANTUS_CI_DIR=$HOME/ci-r33-<name> bash /mnt/f/projects/daw-r33/<name>/tools/wsl/check.sh
  r33/<name> clippy` (and `test` for the crates you touched) — your own clone, never
  `~/cantus-ci`; **delete that clone's `target/` when you finish** (the WSL image lives on
  C:). Code behind `#[cfg(windows)]` leaves Linux-only dead-code warnings. UI tests that
  open windows must pass headless (`env -u DISPLAY -u WAYLAND_DISPLAY`).
- **Real time (ADR 0005):** the audio thread never allocates, locks, blocks or does I/O
  (`rt_memory.rs` counts it — add your paths to it); `cantus-engine` is
  `forbid(unsafe_code)`; OS/unsafe code only in `cantus-rt`. Never let a spinning thread run
  inside MMCSS. Network and MIDI-out threads are ordinary threads, never the audio thread.
- **Null tests first**, before wiring: a feature at its default touches no sample, to the
  bit (the golden hashes do not move; `DSP_VERSION` stays 5 unless you change a sound on
  purpose, then one commit with the bump and the measured size — `CANTUS_GOLDEN_DUMP`);
  block-size independence at tolerance 0; offline render equals real time; parallel
  (`CANTUS_WORKERS`) equals serial. Deterministic maths: `cantus_dsp::math::*`.
- **Start-up wiring goes in `boot`'s stages / `CantusApp::assemble`**, never only in
  `CantusApp::new` (the shipped app never calls it), with a test that fails if it leaves
  that path. Reviewers ask "is this reachable from `cantus_ui::run`?".
- UI: the hand-drawn kit (ADR 0024) — no stock egui widgets in views, no literal colours,
  no `FontId::new`. FL Studio is the visual reference, with Ableton's and Bitwig's ideas
  where they are better; clear label/value units, nothing that overlaps, panels resizable,
  every overflowing area scrollable, both themes (Slate and Paper), 75–200 %, keyboard focus
  and the screen reader (AccessKit) as elsewhere, the frame budget held (the budget tests).
  **Look at it**: shoot what you build and put the best in `docs/round33/<name>/`.
- A model change is a schema bump (`SCHEMA_VERSION`, a migration even if it writes nothing,
  `cargo run -p cantus-project --example write_fixture` for `fixtures/vN.cantus`; every old
  fixture keeps loading); every `Command` has an exact inverse, an undo-fuzz arm and a
  `cmd.*` string (both locales; the history panel names it); a new default binding is a
  keymap bump with its migration.
- **One build works with every driver** (WASAPI and ASIO; ALSA/JACK/Pulse on Linux,
  CoreAudio on macOS): no feature-gated editions. Code for macOS compiles on the macOS
  runner (`macos.yml`); mark what only a real Mac can confirm for `docs/mac-session.md`.
- Licences (ADR 0047): `cantus-core/dsp/engine/project/rt` are MPL-2.0 — **never copy GPL
  code into them**; the rest is GPL-3.0-or-later. A new dependency needs an approved licence
  and a notice (`cargo run -p xtask -- notices`; a test fails otherwise).
- Strings in both locales (your area file); the manual (`docs/manual/`, EN and ES) for
  every feature a user touches; your ADR; `docs/round33/<name>.md`: what was done, how it
  was tested, the numbers, the marked lines, and **only** what needs the owner.
- Never touch `master`, never push, never stage files outside your worktree.

## session — the model, the commands, the engine, recording (ADR 0076, schema 46)

Read BACKLOG §0.2 *Clip launching / session view*; ADR 0004 (commands), 0005 (real time),
0031 (the worker pool); `cantus-core/src/model.rs` (`Clip` is a window onto content;
`Track::clips` *is* the arrangement), `command.rs` (how an arm returns its inverse,
`edit_shared`), `cantus-engine/src/snapshot.rs` (`Snapshot::compile`, `compile_events`,
`TrackSnapshot`), `transport.rs` (sample-accurate: `samples_until`), `graph/track.rs`
(`run_track` splits the pass at every event's sample), `processor_snap.rs`
(`install_snapshot`, `reseek_events_from`), `processor_msg.rs`, `messages.rs`, `render.rs`;
the precedents for compiling something other than the arrangement
(`views::transport::pattern::project_to_play`, `count_in::project_to_compile`); recording
(`cantus-ui/src/recording.rs`, `record_pass.rs`, `record_session.rs`); DAWproject's import,
which today drops scenes (`cantus-project/src/dawproject/read.rs:285`).

**First delivery — the model, the commands and the message API** (commit it as its own
step, run clippy and the core/project tests, and tell the integrator: the launcher builder
starts from it). Make it complete enough that the view never needs a model change:

- `cantus-core/src/session.rs` (and siblings): the grid — scenes (name, colour, an optional
  tempo and metre that launching the scene sets, a scene follow action), a column of slots
  per track (same tracks, same mixer as the arrangement), a slot empty or holding a clip
  (audio, MIDI or — if it fits — a pattern), each session clip with its launch settings:
  launch mode (trigger, gate, toggle, repeat), its quantisation (global, none, 8 bars … 1/32,
  triplet values), legato, loop on/off and its loop region and start marker, follow actions
  (two actions A/B — stop, play again, previous, next, first, last, any, other, jump to a
  slot — with a chance A:B, after a time in bars/beats or a count of plays, linked to the
  clip's length, a repeat count), its own envelopes (per-clip automation of mixer and device
  parameters — "the session's automation, per clip"), velocity amount, groove, warp as an
  arrangement clip has. The project's global launch quantisation, record quantisation, a
  seed for follow-action chances (so a performance replays and renders the same), and per
  track: which side plays (its session clip or the arrangement — Ableton's *Back to
  Arrangement*), its stop button.
- Every edit a `Command` with an exact inverse, `cmd.*` strings, undo-fuzz arms in your own
  `srng` stream: create/delete/move/duplicate a slot's clip, copy arrangement clips into
  slots and back, insert/delete/duplicate/move/rename/recolour scenes, every launch setting,
  every follow-action field, the global settings, *capture scene* (Ableton's "capture and
  insert scene"), consolidate a column, and **commit a performance to the arrangement as one
  undoable step** (a `Batch` whose inverse restores everything).
- Schema 46: `Project.session` (absent in old files: no session), the migration (writes
  nothing), `fixtures/v46.cantus` with a session in it, every older fixture loading. The
  DAWproject import and export of scenes and clip slots (the format has them).
- The engine's **message API** the view will call, declared and documented even where the
  processor does not act on it yet: launch a slot, launch a scene, release (for gate mode),
  stop a track, stop all, back to arrangement, record into a slot; and the events the view
  reads back without polling the model — per slot playing / queued / recording /
  stopping, the playing position within each playing clip, a scene's state — sized for the
  audio thread (no allocation; a fixed ring).

**Then the engine:**

- **Null tests first.** A project with no session — and a project whose session is never
  launched — renders bit-identical to before (the golden hashes do not move); rt_memory:
  launching, stopping and following actions allocate nothing on the audio thread.
- **Launching** at the quantisation boundary, **sample-accurate**: the first sample of a
  launched clip's first note (or audio frame) lands on the boundary's sample, at any block
  size (tolerance 0) and with the worker pool (parallel = serial). A clip replaces the
  track's arrangement playback until *back to arrangement*; stopping a clip stops at the
  boundary (notes off cleanly, audio with the click-free fade the engine uses); legato takes
  over the playing clip's position; gate releases on note-off/key-up; toggle; repeat
  retriggers at the quantisation while held. Launching a scene launches its row and
  applies its tempo/metre. Follow actions run on the audio thread, deterministically from
  the seed (offline = real time). Per-clip envelopes play while the clip plays and hand
  back to the track's own automation when it stops. Audio session clips with warp, gain,
  fades and loop as arrangement clips have; tempo changes keep them in time.
- **Recording into a slot**: MIDI and audio, with the count-in, the record quantisation,
  overdub on a looping MIDI clip, the new clip's length set by the bar when recording stops
  (or a fixed length chosen), launched right away as Ableton does; the take kept like any
  recording (the recording paths and their recovery).
- **Recording the session into the arrangement**: with *arrangement record* on, every
  launch, stop and follow action while playing is written into the arrangement as clips
  (exactly where they were heard), with their per-clip envelopes as automation, as one undo
  step; **the recorded arrangement renders identical to the live performance** (a test
  compares the two renders, bit for bit, with follow actions and legato in it).
- **Offline equals real time**: a scripted performance (launches at given samples) rendered
  through `render.rs` and through the real-time processor at two block sizes gives the same
  bits. Freeze, bounce and stems of a track playing a session clip behave as they do for the
  arrangement (or the menu says why not).
- ADR 0076 (the model, why the session shares tracks and mixer, the launch timing and how
  sample accuracy is held, follow actions' determinism, recording into the arrangement), the
  manual's engine-side pages (with launcher, who writes the user's pages),
  `docs/round33/session.md`.

## launcher — the session view, keyboard, MIDI learn, controller grids (ADR 0077, keymap 14)

Starts from session's first delivery merged into `r33/integration`. Read session's
`docs/round33/session.md` and the model it delivered; ADR 0024 (the kit) and 0072 (palette,
history); `views/workspace.rs` (`Pane`, `ZONES`), `views/arrangement*.rs` (how clips are
drawn and dragged — reuse its painters for clip faces), `views/piano_roll*.rs` (opened, not
changed: a session clip edits in the piano roll like any clip), `keymap.rs` (keymap 13,
`NEW_IN_V…`), `views/transport_menus.rs`, the controllers (`cantus-core/src/controller*.rs`,
`cantus-ui/src/controllers*.rs`, `controllers_feedback.rs` — MIDI out for LEDs exists),
ADR 0060 (MIDI learn).

- **The Session pane**: tracks as columns (the arrangement's tracks, same colours, names,
  arm, mute/solo/stop), scenes as rows with their launch buttons, names, tempo/metre tags;
  each slot showing its clip (colour, name, a mini view of its notes or waveform, its
  playing progress ring/bar, queued blinking at the quantisation, recording red), empty
  slots with a stop or record button per track's state; a master column with stop-all and
  the scene launchers; the global quantisation in the pane's header. Scrolls both ways;
  columns follow the mixer's widths where sensible; resizable; both themes, 75–200 %.
  FL's look (the kit), Ableton's and Bitwig's ideas where they are better (Bitwig's clip
  launcher inside the arrangement as a side panel is a good one if it fits with quality).
- **Switching** arrangement ⇄ session instantly (Tab as in Ableton, a top-bar view button,
  the palette), the selection kept; per-track *back to arrangement*; a shared mixer.
- **Editing in the grid**: create a MIDI clip in an empty slot (double-click, FL/Ableton
  gesture), drag clips between slots and between the session and the arrangement (both
  ways), duplicate, delete, rename by the name (round 27's gestures), recolour, copy/paste,
  capture scene, insert/duplicate/delete scenes, drag files from the browser into a slot;
  every edit through session's `Command`s, one undo step each.
- **A session clip's panel** (the clip view): launch mode, quantisation, legato, loop and
  start, follow actions (A/B, chance, after, repeats) drawn clearly — label/value units,
  nothing loose — and the per-clip envelopes editor (pick a parameter, draw its envelope over
  the clip's loop; reuse the automation editor's painters).
- **Keyboard**: arrows move a slot cursor; Enter launches the slot, Shift+Enter its scene;
  keys for stop track / stop all / record into slot / arrangement record; every one an
  `Action` in the palette, rebindable (keymap 14, its migration).
- **MIDI learn**: slots, scenes, stop buttons, stop all, the scene cursor and the global
  quantisation learnable like any control (the controller map's targets), with feedback.
- **Controller grids** (if it fits with quality — aim for it): Launchpad (Mini MK3, X, Pro
  MK3 in their programmer modes) and Akai APC (mini mk2, APC40 mkII) as grids that mirror
  the session — pads launch, LEDs show colours and states (playing, queued blinking,
  recording), scene buttons, stop, the grid moved with the arrows (the "session ring" box
  drawn on screen). Each device's protocol from its published programmer's reference
  (cite it in the ADR); detected by port name, chosen in Settings ▸ MIDI (a marked line in
  sync's page) or in the controllers pane. Tests with recorded byte sequences (no hardware
  needed); the owner tries real ones (`docs/beta-checklist.md`).
- Shots of every state in both themes in `docs/round33/launcher/`; the manual's user pages
  (the session view, launching, follow actions, recording, controllers; EN and ES); ADR
  0077; `docs/round33/launcher.md`.

## sync — Ableton Link (ADR 0078), MIDI clock and MTC (ADR 0079)

Read ADR 0021 (its sketches: MIDI clock out at 24 ppq, FA/FB/FC; Link through GPL code),
0047 (licences), 0005; `cantus-engine/src/transport.rs` (sample-accurate, `TempoMap`),
`midi.rs` (midir; inputs merged into one ring; `ignore(Ignore::All)` and `parse()` drop
system realtime today; outputs exist only for controller feedback on the UI thread),
`processor.rs`'s `song_position_of`, `backend.rs` (the output latency the device reports),
`views/transport*.rs` (the top bar), `views/workspace_settings.rs`'s MIDI page (a rescan
button and a list of names today), `settings.rs`.

**The licence, decided at the contract:** the Link SDK is *dual licensed under GPLv2+ and a
proprietary licence* (github.com/Ableton/link). GPLv2-or-later code may go into our
GPL-3.0-or-later application, never into the MPL-2.0 engine crates. So:

- **A new crate `crates/cantus-link`, GPL-3.0-or-later** (workspace member and default
  member, `LICENSE`'s crate list): the Link protocol in Rust — discovery over UDP multicast
  (224.76.78.75:20808), the peer/session messages, the ping/pong measurement of the session
  clock offset, the timeline (tempo, beat origin, time origin), start/stop state (Link 3),
  session merging — following the SDK's source and its documented concepts
  (ableton.github.io/link); say in the ADR which parts follow which SDK files. Building the
  C++ SDK into the app instead is rejected unless you find the Rust protocol infeasible
  (it would add CMake and a C++ toolchain to every build; say why if you change course).
- **The engine sees only an abstract timeline**: `cantus-engine/src/sync.rs` (MPL, ours,
  no Link code) — a lock-free shared cell the audio thread reads (tempo, a beat at a host
  time, playing, quantum) and a ring it writes its own proposals into (a tempo change, a
  start/stop, a beat request), with no knowledge of who is on the other side. Link, MIDI
  clock in and MTC in are three sources of that one seam.
- **Following**: with Link on, the transport follows the session's tempo and phase —
  quantised start at the next quantum boundary (bars by default, the quantum chosen), tempo
  changes from either side, start/stop sync (a setting), **the output latency compensated**
  (the beat Cantus plays reaches the speakers when the session's beat does), without clicks
  and without moving any clip (the tempo map is followed, not rewritten; a project with a
  tempo track: say what Link does, as Ableton does — Link overrides while on). Sample
  accuracy where the transport already has it; null test: Link off touches no sample.
- **Tests**: protocol messages encoded and decoded against byte vectors you derive from
  the SDK (cite the file); two in-process peers and **two Cantus processes** on loopback
  converging tempo and phase (measured error in the ADR, ≤ 1 ms after settling), joining and
  leaving, three peers, a network that drops packets; against a real Link app if you can
  build one of the SDK's examples (LinkHut) on this machine or in WSL — if not, say so and
  put it on the owner's list (any Link app on a phone or another PC).
- **MIDI out from the engine**: a MIDI-out thread (not the audio thread) fed by a ring of
  sample-stamped messages from the audio thread, sending each when its audio reaches the
  output (the device's output latency plus a per-port offset the user can set) — the high
  resolution timer the engine already uses for its own threads. Output ports listed and
  enabled per port in Settings ▸ MIDI; the controllers' feedback keeps working.
- **Master**: MIDI clock (24 ppq, F8) sample-accurate to the transport, SPP (F2) on locate,
  start/stop/continue (FA/FC/FB) with the right SPP-then-continue order, following tempo
  changes and the tempo track; **MTC** quarter-frames and full-frame on locate at 24, 25,
  29.97 drop-frame and 30 fps, with a project start offset (an hour offset is common).
- **Slave**: MIDI clock in — a filter (a delay-locked loop / second-order PLL; cite what you
  chose) giving a steady tempo and phase from a jittery clock, SPP to locate, start/stop/
  continue; **MTC chase** — locate on full-frame and on quarter-frames, then follow with
  the filter, freewheel through short dropouts, stop when the code stops. One sync source at
  a time (none, Link, MIDI clock on port X, MTC on port Y), shown in the top bar.
- **Input ports**: the system realtime and MTC messages reach the sync code with their
  arrival time (midir's timestamp — today it is dropped), never through the audio thread's
  note path; everything else keeps its route. (mpe also edits `midi.rs`'s input path for
  MIDI channels: keep your changes in your own functions so both merge.)
- **Tests with a loopback**: the clock's bytes and their sample offsets checked offline
  (jitter ≤ 0.1 ms in the rendered schedule; measured at the port on Windows if you can);
  the follower fed recorded jittery clocks (±1 ms, ±3 ms, a tempo ramp) with its error
  measured; MTC encode/decode at each rate, drop-frame rules; a real loopback on Linux
  (ALSA virtual ports through midir, in WSL) end to end; on Windows a loopback needs a
  driver (loopMIDI or Windows MIDI Services) — if none is installed, say so for the owner.
- **The interface**: a LINK button in the top bar (on/off, the number of peers, the
  quantum) and the sync source shown (EXT, its tempo, a lock light); Settings ▸ MIDI
  rebuilt as a per-port table (in: notes, controllers, clock, MTC; out: clock, MTC,
  feedback; per-port latency offset), with mpe's zone column and launcher's grid choice as
  marked lines; the kit; both themes; the manual (Link, clock, MTC; EN/ES).
- ADR 0078 (Link: the licence decision, the protocol, the seam, latency, the tests), ADR
  0079 (clock and MTC), `docs/round33/sync.md`. Mark what only the owner can check
  (another device, a real hardware loopback, a phone app) for `docs/beta-checklist.md`.

## mpe — MPE end to end (ADR 0080, schema 47, sandbox protocol 7)

Read the MPE specification (MIDI Association, *MIDI Polyphonic Expression* 1.1: zones,
the MCM RPN 6, master and member channels, per-note pitch bend with its range, channel
pressure and CC74 as timbre per note); ADR 0028 (note expressions travel to plug-ins but
nothing makes them per note — its line 167), 0068 (release velocity), 0023 (the
instruments' `mpe.*` modulation sources); `cantus-core/src/model.rs` (`Note`, which derives
`Eq`), `clip_lanes*.rs` (channel-wide lanes), `cantus-engine/src/inst/mod.rs` (`NoteOn`
already carries a `channel` that is always 0; `NoteExpression`; `InstrumentUnit::expression`
is called only by live poly pressure), `snapshot.rs`'s `compile_events` and `graph/track.rs`'s
`run_track` (marked lines only — session owns them: put your expression compile in a file
of your own), `processor_msg.rs`'s `midi()` (it ignores the channel nibble), the instruments
that already implement `expression` (Unda `wavetable.rs`, Calor `analog.rs`, Modulus `fm.rs`,
Gravis `bass.rs`; their DSP in `cantus-dsp`), `cantus-host` (CLAP `clap/events.rs` already
sends `CLAP_EVENT_NOTE_EXPRESSION`; VST3 `vst3/audio_thread.rs` sends
`NoteExpressionValueEvent`; `inst_unit.rs` queues every event at offset 0), the sandbox's
`WireNote` and `PROTOCOL = 6`, `cantus-project/src/midi/**` (import groups by channel, so a
channel becomes a track; poly pressure is dropped), `views/piano_roll*.rs`.

- **The model (schema 47)**: per-note expression on `Note` — pitch bend (semitones, a
  curve over the note's length, its end value carried to the release), pressure (0–1) and
  timbre/slide (0–1) as curves, kept compact (absent on ordinary notes: `#[serde(default,
  skip_serializing_if)]`; a note without expression serialises exactly as before); the
  commands to set, edit, scale, smooth, thin and clear them (exact inverses, undo-fuzz arms
  in your own stream, `cmd.*`); `Note`'s `Eq` either kept (fixed-point values) or replaced
  with care for every user. Splitting, gluing, trimming, quantising, transposing, copying,
  linked clips, the score's reading and every note operation keep the expression where it
  belongs (tests for each). Migration and `fixtures/v47.cantus`.
- **Null tests first**: a project without expression renders bit-identical (the golden
  hashes do not move); an instrument given no expression renders as before; MPE off on an
  input port changes nothing on that port.
- **Recording MPE**: an input port marked MPE (Settings ▸ MIDI: zones lower/upper, member
  channel count, the bend range per zone — 48 semitones by default for members, 2 for the
  master; detected from the MCM RPN 6 when the controller sends it), each member channel's
  bend/pressure/CC74 bound to the note it carries, recorded into that note's curves; the
  master channel's messages as the track's ordinary lanes. Live playing goes the same way
  (the note's id and channel to the instrument). Tested with recorded byte streams from a
  Seaboard/LinnStrument/Osmose-style controller (from the spec's examples; no hardware
  needed); the owner plays a real one if they have it.
- **Playback, sample-accurate**: sequenced expression events at their sample offset (not
  block start) to native instruments by note id; to hosted plug-ins as CLAP note
  expressions and VST3 note expression (query `INoteExpressionController` for what the
  plug-in supports; the tuning value's mapping checked against the VST3 SDK), with the
  event offsets the host adapters now flatten to 0 delivered at their offsets; and **as MPE
  MIDI** (one member channel per note, the MCM, the bend range) for plug-ins that take MIDI
  only — a per-plug-in choice, defaulting by what the plug-in declares. Sandbox wire
  `PROTOCOL` 7 if `WireNote` changes.
- **Native instruments**: Unda, Calor, Modulus (and Gravis, which already listens)
  respond to per-note bend (smooth, no zipper, glide-safe), pressure and timbre through
  their `mpe.*` modulation sources with sensible defaults — check by rendering, not only by
  reading; then any other instrument of ours where it is cheap and musical (Exemplar's
  pitch, Lamina's and Chorda's pressure?) — say which and why.
- **The piano roll**: per-note expression shown on the notes (a bend line through the
  note, pressure as thickness or shade, timbre as a colour, each switchable) and edited in a
  lane under the roll for the selected notes (draw, smooth, scale, flatten; bend snapped to
  semitones on a modifier), the kit, both themes, the frame budget with a dense MPE clip.
- **MIDI files**: MPE import (member channels recombined into one track with per-note
  curves, the zone from the MCM or guessed, poly pressure kept) and export (an MPE track as
  a zone with its MCM and RPNs, notes rotated over member channels), round-trip tests;
  DAWproject's note expressions if the format carries them.
- **MIDI 2.0**: measure what it would take (midir has no UMP; Windows MIDI Services, ALSA's
  UMP, CoreMIDI's; CLAP/VST3's MIDI 2 events; per-note controllers map onto what you built)
  and build the part that fits with quality — at least the model's per-note values at
  MIDI 2.0's resolution and the translation layer, if not the transport. Write the rest down
  in the ADR and BACKLOG with sizes.
- ADR 0080, the manual (MPE: zones, recording, editing, instruments, plug-ins, files; EN/ES),
  shots in `docs/round33/mpe/`, `docs/round33/mpe.md`.
