# Round 19 — the brief every builder reads first

**The ask (the owner, 2026-10-04, in Spanish):** record from an audio interface
*professionally, with every feature of the DAWs that are best at recording* (FL Studio,
Pro Tools, Cubase, Logic, Studio One, Reaper), then polish everything so whole songs can be
made in Cantus; **effects on the master channel** ("very, very important") with mastering
effects — a Maximus-class maximizer, saturation/colour (Blood Overdrive, Fruity
WaveShaper), stereo control (Patcher's mid/side); demo songs, and a startup project that
shows what Cantus can do. The mission the owner states: *the best DAW in the world, made
with excellence* (Colossians 3:23). Build to that bar.

Read, in order: this file, `docs/adr/0025-recording-audio.md`,
`docs/adr/0026-the-master-chain-and-mastering-effects.md`, `docs/STATUS.md` (top and the
Conventions section), ADR 0005 (real-time rules), ADR 0024 (the hand-drawn kit), and then
your own section below.

## The contract (commit on `r19/contract`)

Already in the tree, compiling, for everyone to build on — **keep these signatures**; if one
must change, say so in your report:

- `cantus-core`: `audio.rs` (`AudioSource`, `AudioRegion`, `FadeCurve`, `TrackInput`,
  `MonitorMode`, `RecordSettings`, `TakeMode`, `ClickMode`), `AudioSourceId`,
  `Clip::{audio, lane}`, `Track::{input, armed, takes_open}`, `Project::{audio_pool,
  recording}`, `Master::{effects, fx_enabled}`, `StripId::MASTER`,
  `Project::make_audio_clip / audio_source(_mut)`; `EffectKind::{Saturator, Stereo,
  Maximizer}` with stub parameter models in `fx/{saturator,stereo,maximizer}.rs`; the
  snapshot's `TrackSnapshot::{audio, input, armed}`, `AudioClipSnapshot`,
  `Snapshot::{master_effects, master_fx_enabled, recording}`; the commands
  `InsertAudioSource, RemoveAudioSource, SetAudioSourcePath, SetClipAudio, SetClipLane,
  SetTrackInput, SetTrackArmed, SetTrackTakesOpen, SetRecordSettings` (their `cmd.*` keys
  are in `locales/areas/recording.*`). Note clips on lane ≠ 0 and audio clips no longer
  fire note events.
- `cantus-project`: schema **18**, `migrate_17_to_18` (writes nothing), fixture
  `v18.cantus`.
- `cantus-engine`: `recorder.rs` (the recording API on `EngineHandle`, `RecordRings`,
  `RecordMark`, `FinishedTake`, `LivePeaks`, `RecordLatency`, `measure_loopback`,
  `set_record_start`), `devices.rs` (`drivers`, `input_devices`), `AudioConfig::{driver,
  input_device_name}`, `Processor::attach_input`, `EngineMessage::{AttachRecorder,
  DetachRecorder}`, `EngineEvent::{Record, GarbageRecorder}`, `Meters::{input_count,
  inputs, master_gr}`, `fx/{saturator,stereo,maximizer}.rs` (wires).
- `cantus-ui`: `waveform.rs` (`Peaks`, `paint_peaks`), `AudioSettings::{driver,
  input_device, record_offset_frames}`, empty locale files `locales/areas/{recording,
  audioclips,master,demos}.*` and `locales/fx/{saturator,stereo,maximizer}.*`.

## Rules (all builders)

1. **Your worktree and branch only.** Never touch `C:\projects\daw` (the owner's tree) or
   another builder's worktree; never push; never merge. Make a WIP commit on your branch
   early and then often (`git add -A && git commit -m "..."`), so nothing is lost if your
   session stops.
2. **Write files in pieces under ~300 lines per tool call** (a 600 s stream watchdog kills
   long single writes). Prefer new files; keep edits to shared files (`command.rs`,
   `processor.rs`, `app.rs`, `arrangement*.rs`, `mixer*.rs`, `mod.rs` lists) small and
   local, because every shared hunk is a merge later. A new module is declared with one
   line in its `mod.rs`, or with `#[path]` inside your own file.
3. **Tests first** (the repo's habit): null tests before wiring — a control at its default
   touches no sample, to the bit; block-size independence at tolerance 0; offline render
   equals real time. New commands go into the undo fuzz
   (`crates/cantus-core/tests/undo_invariants.rs`). Run the tests of every crate you
   touched, `cargo clippy -p <crate> --tests` clean, `cargo fmt`.
4. **Strings** go in your area's locale files only, English **and** Spanish (the i18n test
   requires every English key in every locale). The bundled fonts lack many symbol
   glyphs: paint icons (`icons.rs`, the kit) rather than using Unicode symbols.
5. **UI is the hand-drawn kit** (ADR 0024, `crates/cantus-ui/src/kit/`): no stock egui
   widgets in views, no literal colours (tokens), no `FontId::new` — lints enforce it.
   Labels in clear label/value units, nothing that reads as loose words; text that never
   collides (the kit's boxed text elides). FL Studio is the visual reference. **Look at
   what you build**: `cargo run -p cantus-ui --example shoot -- <dir>` renders PNGs through
   the CPU rasteriser; extend the example if your view is not in it, and read the images.
6. The audio thread never allocates, locks, blocks or does I/O (ADR 0005). Anything sized
   is built on the UI thread and sent in; garbage goes back out.
7. Bash quirk on this machine: a heredoc delimited by `EOF` fails to parse — use another
   delimiter (`XEOF`), or write scripts with the Write tool and run them.
8. Build settings: `.cargo/config.toml` (untracked) has `jobs = 4` and `debug = 0`; nine
   builders share the machine. Don't run `cargo test --workspace` more often than you
   need; test your crates.
9. **Finish with a report**: what is done, the tests (counts, names of the important ones),
   every file you touched outside your own area, what is left, and anything another
   builder or the integrator must know. Update your part of a section in
   `docs/round19/<your-name>.md` (create it) — the integrator folds these into STATUS.

## Who owns what

| Builder | Branch | Owns |
|---|---|---|
| engine-rec | `r19/engine-rec` | `recorder.rs`, processor's input/monitor/record/audio-clip playback, `samples.rs` audio loading, render/stems with audio, `snapshot.rs::compile_audio`, click modes |
| devices | `r19/devices` | `devices.rs`, `backend.rs` stream setup (input stream, drift, latencies, ASIO feature), `measure_loopback`, the settings dialog's audio page |
| ui-rec | `r19/ui-rec` | recording workflow in the UI: arm/monitor/input controls (track header, mixer strip, inspector), transport record options (count-in, punch, takes, click), the pass → clips/takes, take lanes and comping, MIDI recording modes |
| ui-audio | `r19/ui-audio` | `waveform.rs`, audio clips in the arrangement (waveforms, fades, gain, context menu), import (drag & drop, menu), audio files on save (collect into the project folder), audio clip inspector |
| master | `r19/master` | the master's insert chain: commands addressing `StripId::MASTER`, processor's master section, the master strip's slots and device windows, latency into ADR 0022's output latency |
| fervor | `r19/fervor` | `EffectKind::Saturator` end to end: model, DSP, unit, editor, presets, CLAP/VST3 |
| latitudo | `r19/latitudo` | `EffectKind::Stereo` end to end |
| culmen | `r19/culmen` | `EffectKind::Maximizer` end to end |
| demos | `r19/demos` | demo songs, the startup project, the demo picker |

Shared-file etiquette between pairs that meet: engine-rec and master both edit
`processor.rs` (engine-rec: the block start and the track loop; master: the section after
the strips, where the master bus is summed). ui-rec and ui-audio both touch the
arrangement (ui-rec: track header controls and take lanes; ui-audio: the clip body). The
three effect builders each add one line to the shared registries (editor dispatch,
plug-in export list).
