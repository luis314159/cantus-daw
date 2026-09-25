# ADR 0021 — Exchanging work with other DAWs: MIDI files, stems, DAWproject

**Status:** Accepted · **Date:** 2026-09-24 · **Extends:** ADR 0004 (one command bus), ADR 0005 (the audio thread), ADR 0018 (delay compensation)

## Context

The user asked for Cantus to be compatible with the DAWs on the market — Ableton Live, FL
Studio, Logic, Pro Tools, Reaper, Bitwig Studio, Cubase, Studio One. Two things are meant
by that. One is plug-ins: hosting VST3 and CLAP, and shipping Cantus's own effects as
plug-ins; other agents build those this round. The other, this ADR, is **moving a piece of
work** between Cantus and another application, both ways.

What each of those applications reads, which decides what is worth building:

| Carrier | Reads it | Carries |
|---|---|---|
| Standard MIDI File | all eight | notes, tempo map, metre, track names |
| WAV stems | all eight | the sound of each channel, as mixed |
| DAWproject 1.0 (Bitwig/PreSonus, MIT) | Bitwig 5.0.9+, Studio One 6.5+, Cubase 14+ (and Reaper, Live, FL through the open converters ProjectConverter and DawVert) | tracks, mixer, sends, clips, notes, devices, automation, audio |
| AAF / OMF | Pro Tools, Logic, Cubase, Studio One, Nuendo | audio clips and fades only, in seconds |

AAF carries audio clips, which Cantus does not have yet, so it waits for them. The other
three are built here.

## Decisions

### 1. One principle for all three

**What is written is what the engine plays, and what cannot be carried is said.** Exports
start from `Snapshot::compile` or from the offline render, never from a second reading of
the model; imports return a `cantus_project::Report` — locale keys with arguments, since
that crate has no strings — which the interface shows as *Not carried* and *Done*. Nothing
is dropped in silence: a controller change, a plug-in, a pre-fader send turned post-fader
and an audio clip are each named. An import is **one undo step**: a MIDI file is one
`Command::Batch` of `InsertTrack`s (and the tempo, when asked), a DAWproject one
`Command::ReplaceProject`. No new command, no model field, **no schema change**.

### 2. MIDI files (`cantus_project::midi`)

- **The codec is ours** (`midi::smf`, ~400 lines): the format is small and fixed, and owning
  it is what lets the reader be lenient where real files are sloppy (a missing
  end-of-track, running status kept across a meta event, a RIFF `.rmi` wrapper) and strict
  where sloppiness would be a lie (a chunk longer than the file, a data byte with no status
  to run on).
- **Export** writes a **type 1** file for a clip, a track or the song: a conductor track
  (the song's name, the time signature, the tempo), then one track per Cantus track with
  its name. The notes are `Snapshot::compile`'s events paired back into notes by id, so a
  muted note, clip or pad is absent, a looping clip is written as the notes it plays and a
  note past its clip stops where the clip does. A clip is written from its own start.
  Melodic tracks take channels 1–16 skipping 10; a Pulsus kit goes on **channel 10**, each
  pad on its General MIDI key — the shipped kit is already on the GM map; a pad elsewhere
  (off the map, or a kick sitting on the bongo's key) moves to its shape's GM key (36, 38,
  42) when that key is free, and a sample keeps its note, since nobody can guess its sound.
- **Tick conversion** is `round(t · PPQ_file / 960)`, halves away from zero, on both edges of
  a note, and a note is never rounded to nothing (at least one file tick). At 960 or any
  multiple of it nothing moves. At the same tick note-offs come before note-ons, and every
  track ends with exactly one end-of-track, at the song's end (the clip's, for a clip), so
  a clip with silence at its end keeps its length elsewhere. The writer does not use
  running status: it saves bytes and is the one thing an old reader gets wrong.
- **Import** reads types 0, 1 and 2. A type 0 file is **split by channel**; a type 1 track
  that plays on several channels too (`"Piano (ch 2)"`). Tracks are named by the file, or by
  the channel's General MIDI program (`"Fingered Bass"`), or `Drums` / `Channel N` in the
  interface's language. Channel 10 becomes a Pulsus kit: the shipped kit plus a pad, named
  after the GM map, for every key the part plays that the kit lacks (up to Pulsus's 32
  pads). A note struck again before release pairs first-on-first-off, as every sequencer
  does; a note never released ends with its track; a note of no length gets a 64th.
- **The tempo map.** A Cantus project has one tempo. A file with changes is imported one of
  two ways, chosen in the dialog: **keep the bars** (the default — every note stays on its
  beat, the piece plays at the file's first tempo, and the report says how many changes
  were not kept) or **keep the timing** (every note sounds at the second it did, at the
  project's tempo, off the grid if it must — right for a recorded performance). An SMPTE
  file is always read by seconds. The file's tempo and metre are applied to the project
  when asked, which the dialog proposes when the project is empty (a song already there
  would change speed otherwise).
- **Placement**: one clip per imported track, from the bar before its first note to the bar
  after its last, at the drop position or the playhead's bar; each new track takes a free
  mixer strip the way `Project::first_free_strip` does; past the engine's 64 tracks the
  report says how many were left.

### 3. Stems (`cantus_engine::stems`)

- **One offline pass.** The processor carries a `StemTap` while a stems render runs. At the
  two places a signal is mixed — a track into its strip, a strip out of its fader — it
  copies the product it is about to add. It reads and never writes, so the mix is the
  bounce to the bit (a test holds the master stem equal to `render` exactly), and live the
  tap is `None`: one `Option` test per segment. The tap is allocated before the render, for
  its block size. Rendering N times would cost N times as long and would let a
  time-varying effect (Spatium's modulated lines) differ between passes.
- **Sources.** A *strip* stem is what leaves the strip's fader (inserts, EQ, pan, mute
  included — the mix as heard). A *track* stem is the instrument after the track's own
  level and balance, before its strip. *Direct* is the sum of the tracks that feed the
  master with no strip; *Master* is the output.
- **Alignment.** Delay compensation makes every path reach the master `L` samples after the
  timeline. A strip's fader output is `P` late (ADR 0018's arrival plus its own inserts), so
  each stem is trimmed by its own lateness — tracks and Direct by 0, a strip by its `P`
  (`Processor::strip_latency`), the master by `L` — and the render runs `max` lateness
  longer so every file is complete. Sample 0 of every file is the song's first tick.
- **The sum.** With the master at unity and no master processing, **the stems of the strips
  that feed the master plus Direct add up to the master exactly** (tolerance 0, summed in
  the engine's order; any order is within f32 rounding). A strip that feeds a bus is in its
  own stem *and* in the bus's — that is what a bus stem is — so the set that sums to the
  master is the strips whose output is the master.
- **Silent stems are skipped** unless *Write silent stems as well* is ticked: a desk opens
  with sixteen strips, and thirteen empty files are noise. A stem's file is opened at its
  first non-zero sample (the silence before it written then), so a silent one never exists
  on disk; the ones skipped are listed in the status bar's count.
- **Files**: `NN Name.wav`, 32-bit float stereo at the engine's rate — the strip's number and
  the name the desk shows (the track's number and name for tracks; `00` for the master and
  for Direct), with the characters no file system takes replaced. Written from a thread of
  their own with the progress in the status bar; *Stop* removes whatever was opened.

### 4. DAWproject (`cantus_project::dawproject`)

- **The container** is a ZIP we write and read ourselves (~250 lines: stored or deflated
  entries through `flate2`, read through the central directory so a streaming writer's data
  descriptors read the same, CRC-checked, no ZIP64). `roxmltree` reads the XML; writing it
  is a small escaping writer. Output is deterministic: the same project makes the same
  bytes.
- **Export, the mixer.** A Cantus track is FL's channel, feeding a strip; a DAWproject track
  *is* a channel. So a strip fed by one track and nothing else, where that track's own
  level, balance, mute and solo are at rest — the common case — is written **as** that
  track's channel. Otherwise the track's channel carries the track's own settings, marked
  `comment="cantus:channel-rack"`, and names the strip's channel as its destination; the
  strip is a track of its own, `submix` when tracks or strips feed it, `effect` when only
  sends do. A strip nothing feeds is not written. The master is the `master` channel, last.
  Sends are post-fader with their level and switch. Volume is linear, pan normalized.
- **Export, devices.** An instrument or insert is a generic `<Device>` with Cantus's name
  (*Aequalis*, *Comprimo*…), `deviceID="cantus.<kind>"`, `deviceVendor="Cantus"`, its
  parameters listed as `RealParameter`s for anyone reading, and its settings as a JSON state
  file in the archive (`devices/<kind>-<id>.json`), from which Cantus restores it exactly —
  the slot's mix and its sidechain (by the key channel's XML id) included. The strip's own
  three-band EQ is DAWproject's built-in `<Equalizer>` (low shelf, bell, high shelf), which
  Bitwig, Studio One and Cubase map to their own EQs. **When our effects ship as CLAP
  plug-ins** (another agent is building that export this round) the same slot becomes a
  `<ClapPlugin deviceID="…">` with the plug-in's own state file, and every CLAP host can
  load it; the `cantus.<kind>` device stays readable for older files.
- **Export, the timeline**: one lane per track, `timeUnit="beats"`, a `Clip` per clip with
  `time`, `duration`, `playStart` (Cantus's offset), `loopStart`/`loopEnd` for a repeating
  clip, `enable="false"` for a muted one, its name and colour, and its notes (`vel` = v/127).
  A muted note has no place in the format and is reported. Tick → beats is `t/960` written
  as the shortest decimal that reads back to the same double, so positions come back exact.
- **Import, any application's file.** Every channel becomes a mixer strip (volume, pan, mute,
  solo, sends, devices, destination) except the master's (the master) and a
  `cantus:channel-rack` one (a track's own level and balance). Every track with notes
  becomes a Cantus track feeding the strip made from its channel; a group track is a bus
  its children feed. A Cantus device comes back from its state; the built-in equaliser
  becomes the strip's EQ when it fits three bands; every other device and plug-in is
  named in the report, and a track whose instrument could not come plays Firmus — or
  Pulsus when its notes are all on channel 10. A clip in seconds is read at the file's
  tempo. The report names audio clips, automation lanes, markers, launcher slots and
  scenes, note expressions, clip references, tempo and metre automation, pre-fader sends
  made post-fader, and a loop that does not start where its clip plays. The desk is padded
  to sixteen strips, so there is somewhere to route to.
- **Validation.** The official `Project.xsd` and `MetaData.xsd` (MIT, Bitwig) are vendored in
  `crates/cantus-project/tests/fixtures/dawproject` with their licence. No Rust crate
  validates XML Schema, so the test carries a validator of the subset these schemas use
  (sequences, choices, occurrence bounds, extension, attribute types, enumerations, lists,
  `xs:ID`/`xs:IDREF`) that fails loudly on any construct outside it; a test proves it
  refuses seven kinds of breakage, and an ignored test runs the JDK's full
  `javax.xml.validation` over the same files when a JDK is on the PATH. It agreed on the
  day this was written.

### 5. The interface (`views::interop`)

The File menu gains *Import MIDI…*, *Export MIDI…* (the song, the selected track or the
selected clip; the resolution), *Export stems…* (strips or tracks, tail, silent stems,
master mix, folder), *Import DAWproject…* and *Export DAWproject…*. A MIDI file dropped on
the arrangement is imported at the tick under the pointer (snapped as the arrangement
snaps), below the row it was dropped on; a `.dawproject` dropped anywhere opens it. winit
0.30 does not report the pointer during an operating-system drag on every platform, so when
the position is unknown the file lands at the playhead's bar. Progress and errors go in the
status bar; a report window lists what was and was not carried. Strings are in
`locales/areas/interop.*`.

### 6. Tempo sync (written down, not built)

- **MIDI clock out** is 24 clocks per quarter (`F8`), Start/Stop/Continue (`FA`/`FC`/`FB`)
  and Song Position Pointer (`F2`). The audio thread may not make the system call that sends
  a MIDI message (ADR 0005), so it would push each clock with its sample time into a ring
  (as the recorder's capture does), and a MIDI-out thread would send it at the matching
  wall-clock time, one buffer and the output latency ahead. `midir` (already a dependency)
  has output ports; WinMM's millisecond jitter is inside what receivers smooth. The position
  pointer is sent on every seek. Following an *incoming* clock is harder (a tempo estimator
  with hysteresis) and is a separate job.
- **Ableton Link** is open source, GPL-2.0-or-later, C++ with asio. Its C wrapper `abl_link`
  (in the Link repository) is the stable surface for a binding: build it with the `cc` or
  `cmake` crate (a C++ compiler; asio is vendored in Link as a submodule), expose the
  session state, and read it once per block on the audio thread —
  `captureAudioSessionState` is documented as realtime-safe — mapping Link's host time to
  sample time through the output latency, with beat and phase aligned to the quantum.
  GPL-2.0-or-later is compatible with the application's GPL-3.0-or-later but not with the
  engine crate's MIT OR Apache-2.0 (ADR 0001), so Link belongs in a crate of its own that
  the application wires in, as `cantus-host` is, never in `cantus-engine`.

## What the tests hold

- `crates/cantus-project/tests/midi_file.rs`: export → import gives **identical compiled
  events** for 120 notes at odd positions and lengths at PPQ 960, 1920, 3840, 7680 and
  9600; at 96, 120, 192, 240 and 480 notes on the file's grid come back exactly and any
  other lands within half a file tick with none lost; a hand-built type 0 file (bytes in the
  test, running status, note-on velocity 0 as note-off) splits into a *Fingered Bass* and a
  kit with a *Crash cymbal 1* pad; a tempo change is kept by beats (and reported) or by
  seconds (a note 4 s in lands on beat 8 at 120 BPM, beat 6 at 90); the file is well formed
  to the byte (chunk lengths add up to the file, one end-of-track per track at the song's
  end, tempo 528 634 µs for 113.5 BPM, 7/8 as `07 03 0C 08`); muted notes and clips are
  absent; a kick off the GM map is written on 36; an import is one undo step.
- `crates/cantus-engine/tests/stems.rs`: the master-feeding strips plus Direct add up to the
  master with **tolerance 0**, and the master stem equals `render` **to the bit** (also with a
  bus); a **100-** and a **16 884-sample** latent insert move **no** stem by a single sample and
  the sum still holds; stems are identical at blocks 7, 64, 1000 and 4096; track stems do
  not move when their strip's settings do; 13 of 16 strips are skipped as silent and
  written as exact zeros of the full length when asked; a stopped export leaves nothing.
- `crates/cantus-project/tests/dawproject.rs`: the export of a project with every mixer
  feature is **valid against the official XSD** (and an empty project's), the validator
  refuses seven kinds of breakage, a Cantus project **comes back whole** (tracks,
  instruments, clips, notes, strips, inserts with their parameters, bypass and sidechain,
  sends, buses, the return, the EQ, the master; gains within 0.001 dB, everything else
  exact; the compiled events identical; an import report with nothing dropped), a Bitwig
  file comes in with its group, return, EQ and send, and its report names nine kinds of
  loss; a damaged file is refused with a reason.
- `crates/cantus-ui/tests/interop_flows.rs`: a MIDI import and a DAWproject import are each
  one undo step, a drop lands under the pointer on the grid and below its row, stems are
  named `NN Name.wav` in both languages and export off the interface thread, every report
  key has words in both languages (`cantus_project::report::KEYS`, which a test keeps equal
  to the keys the code uses), and the dialogs render headless. The dialogs are drawn in
  both themes and languages by `cargo run -p cantus-ui --example shoot_interop -- <dir>
  1.0 [es]`.

## Consequences

- New: `cantus_project::{midi, dawproject, report}`, `cantus_engine::stems`,
  `Processor::{attach_stem_tap, stem_tap, track_slot, strip_slot, strip_latency}`,
  `views::interop`. Dependencies: `flate2` and `roxmltree` in `cantus-project` (both MIT OR
  Apache-2.0).
- Left for later, each waiting on something else: **AAF/OMF** and **audio in DAWproject**
  (audio clips); **tempo and metre changes** kept losslessly (a tempo track); **automation**
  both ways (automation lanes); **controllers and pitch bend** from MIDI files (a model for
  them); plug-in devices on import (hosting, this round's host agent); the drop position on
  Windows (a winit with drag positions).
