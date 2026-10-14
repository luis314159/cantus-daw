# ADR 0023 — Native instruments: one contract, owned by their files

**Status:** Accepted · **Date:** 2026-09-24 · **Extends:** ADR 0005 (the audio thread), ADR 0017 (the plug-in family), ADR 0018 (the plug-in contract) · **Related:** ADR 0019 (hosting, the effects session)

## Context

The user asked for the best instruments on the market — synths, basses, drums, samplers,
keys — researched in depth, including how they look, and then built to equal and surpass
them, with Cantus following the plug-in standards. Nine research reports are in
`research/instruments/` (screenshots local and git-ignored). Their shared verdicts:

- The bar is set by Serum 2, Pigments 7, Diva, Omnisphere 3, Kontakt 8, Battery 4/XO,
  Pianoteq 9, MODO BASS 2 and FM8, and by the stock bundles of Live 12, Logic 12 and FL
  2026 (14–16 of 19 categories each).
- The leaders win on sound and interaction but share structural gaps a host-native
  instrument does not have: their own undo, modulation that cannot leave the plug-in,
  renders that are not repeatable (unseeded drift, quality modes that differ offline),
  knob boundaries at 1.0–2.2:1 contrast, and no idea of the song (its key, its kick).
- Instruments need what effects did not: notes with ids, per-note expression, a sustain
  pedal, voices, and far more parameters (a modulation matrix, zones).

Until now a track held Firmus or Pulsus, both preallocated in every track slot. That does
not scale to seven more instruments, some of them large (a sampler, a piano).

## Decisions

**A native instrument is `InstrumentSpec::Native(NativeSpec)`**: a kind and its state as
numbers by key (`key → f32`, sparse — only what moved is stored) and texts by key (paths,
names). Numbers are what the command bus, undo, the AI bridge, automation and a CLAP/VST3
export already understand. Three commands edit it — `SetInstrumentValue`,
`ClearInstrumentValue`, `SetInstrumentText` — with exact inverses (setting a key never
stored has "clear it" as its inverse, as ADR 0017 did for sparse effects). Firmus and
Pulsus keep their typed forms, so every existing project renders bit-identically.

**An instrument is owned by its files.** Each kind has one module per layer:
`cantus_core::inst::<kind>` (the parameter model: `NAME_KEY`, `ICON`, `READY`, `SOURCES`,
`params()`, `param(key)`, `is_text_key`, `is_asset_key`), a DSP module in `cantus-dsp`
named for the product (`unda`, `calor`, `modulus`, `exemplar`, `gravis`, `lamina`,
`organum`), an `InstrumentUnit` in `cantus_engine::inst::<kind>`, an editor in
`cantus_ui::views::inst::<kind>` and a locale pair in `locales/inst/`. `InstrumentKind`
dispatches to them. **Every kind's dispatch line exists from the first day** (a silent
scaffold behind each), so builders working in parallel never edit the same line; further
modules of an instrument are declared with `#[path]` inside its own files.

| Kind | Product | What |
|---|---|---|
| `Wavetable` | **Unda** | the wavetable / hybrid flagship |
| `Analog` | **Calor** | analog modelling polysynth; owns the shared `cantus_dsp::va` filters |
| `Fm` | **Modulus** | FM with any-to-any operators, DX7 import |
| `Sampler` | **Exemplar** | multisample sampler; SFZ, Decent Sampler, SF2 import |
| `Bass` | **Gravis** | 808, synth bass, modelled electric bass |
| `ElectricPiano` | **Lamina** | modelled tine and reed pianos |
| `Organ` | **Organum** | tonewheel organ and rotary speaker |

Drums stay **Pulsus**, grown in place (Pulsus II) rather than replaced, because the channel
rack, the piano roll's lanes and every existing kit are built on it.

**`InstrumentUnit` is the engine's instrument contract**, the twin of `FxUnit`:
`configure(spec, assets)` on the audio thread without allocating; `note_on(NoteOn { key,
velocity: 0…1, id, channel })`, `note_off(key, id, release velocity)`, `choke`,
`expression(id, NoteExpression, value)`, `controller(Controller, value)`; `process(l, r)`
adding into zeroed buffers and returning whether it wrote anything; `all_notes_off`,
`reset`, `active_voices`, `latency`, `telemetry`. Notes arrive between segments split at
event times, so a note-on is sample-accurate without an offset. **Every note has an id**
(the clip note's, or one per key for live input), so two notes on one key are two notes;
the ids and the seven note expressions are the intersection of CLAP and VST3 and match
`cantus_host::NoteEvent`, so a hosted instrument can sit behind the same trait.

**Units are delivered, not preallocated.** The backend builds a unit off the audio thread
when a track needs one it has not been sent (`inst::units_for`), sends
`EngineMessage::AddInstrument` before the snapshot, and the processor swaps it in; the one
it replaces, and any nobody claimed, go back as `EngineEvent::GarbageInstrument` to be
dropped on the UI thread. A unit whose `(track, kind)` is unchanged stays, so voices ring
across snapshots. Each unit carries a serial, so a unit handed back late is never mistaken
for the one the engine holds. The offline renderer sends the same units, so live and
bounced audio are one code path.

**Assets are decoded before the snapshot is sent**: a text key the kind marks as an asset
(`is_asset_key`) is a file path; `SampleCache::attach` decodes it into
`TrackSnapshot::assets`, and `configure` receives `(key, audio)` pairs. A file that cannot
be read is silence, never an error on the audio thread.

**One modulation model for every instrument** (`cantus_core::inst::modulation`,
`research/instruments/family.md` §6.6): eight macros (`macro1…8`, named by
`macro{n}.name`) and 32 connection slots (`mod{n}.src/.dst/.amt/.via/.curve/.on`) stored in
the spec like any value — so a connection is project data with undo and presets. Sources
are a shared vocabulary of tags (`env1…4`, `lfo1…8`, `vel`, `relvel`, `key`, `rnd`,
`mpe.*`, `macro1…8`, `mw`, `at`, `pb`, `expr`, `sus`), each marked per-voice or global; a
kind lists the ones it produces. The key tables are constants, so `configure` reads them
without formatting a string. `cantus_dsp::modmatrix` evaluates resolved routes;
`cantus_dsp::voices` allocates voices (poly with stealing, mono/legato with a note stack
and priority, the sustain pedal) the same way in every instrument.

**The frame is shared** (`views::inst`, `family.md` §6.3): one window per open editor with
a header (the track, presets, A/B kept by the host, the view switch **Play · Edit ·
Matrix**), a modulation strip of draggable sources, a macro bar, a playable keyboard and a
footer with voices in use; knobs show the connections reaching them as rings and accept a
dropped source. The inspector shows the instrument's picture, an *Open editor* button and
four macros. Telemetry slots 0–2 are the frame's (voices, polyphony, stolen); an
instrument's own start at 8.

**The knob passes 3:1.** Its body's edge at rest is `text_dim` (4.8–5.7:1 against the
panel in the three themes), not `grid_bar` (1.7–2.0:1): `family.md` V1, before instruments
multiply knobs by ten.

**House rules every instrument keeps** (the contract tests hold the first three for every
kind at once, `crates/cantus-engine/tests/inst_contract.rs`):

1. Finite output, **identical at every block size**, and identical live and offline — no
   quality mode that differs when bouncing, no drift or randomness that is not seeded
   from the note (its id, key and start), never from a global counter or the clock.
2. **Headroom**: an eight-note chord at velocity 127 stays below 0 dBFS.
3. **Exact silence** once the notes are over (denormals flushed, tails gated), so an idle
   track is skipped and `process` returning `false` is true.
4. A control at its default changes no sample; a modulation route at depth 0 is not
   evaluated.
5. Every instrument has `gain` (output, dB, −48…+12, default 0): the fixture relies on it.
6. Values in the units a musician reads (Hz, ms, dB, cents, semitones), never an unlabelled
   0…1 where a unit exists.

## What the tests hold

- `cantus-core`: every kind describes itself consistently (ranges, no key listed twice, the
  modulation keys accepted by every kind, asset keys are text keys); setting clamps and
  rounds, clearing restores the default; a fresh spec serialises to its kind alone; the
  modulation tables are consistent and a connection is two texts and a depth; the undo fuzz
  walks the three new commands and native instruments arriving and leaving on any track.
- `cantus-dsp`: curves keep their ends and sign; routes add scaled by depth and via; the
  allocator fills then steals the oldest released voice, tells two notes on one key apart
  by id, glides and returns in legato, keeps the lowest note in low priority, and holds
  releases while the pedal is down.
- `cantus-engine`: the three family rules above for every kind; a delivered unit plays a
  note at exactly sample 24 000 for beat 1 at 120 BPM; units go back when the kind changes,
  when the track returns to Firmus, and when nobody claims them, and stay when only a value
  changes; the probed track reports telemetry; two live notes are told apart and a live
  sustain pedal arrives as a controller.
- `cantus-ui`: every kind opens in the frame in all three views; A/B, a preset, connecting
  and removing modulation are one undo step each and come back exactly.
- `cantus-project`: schema 16, fixture `v16.cantus` with a native instrument, a named
  macro and a connection.

## Consequences

- New: `cantus_core::inst` (with `modulation`), `NativeSpec`, `InstrumentKind`,
  `InstrumentSpec::Native`, `Command::{SetInstrumentValue, ClearInstrumentValue,
  SetInstrumentText}`, `TrackSnapshot::assets`; `cantus_dsp::{modmatrix, voices}` and a
  scaffold module per instrument; `cantus_engine::inst` (`InstrumentUnit`, `NativeUnit`,
  `NoteOn`, `NoteExpression`, `Controller`, `make`, `units_for`),
  `EngineMessage::{AddInstrument, SetInstrumentProbe}`, `EngineEvent::GarbageInstrument`,
  `Meters::inst`, `EngineHandle::set_instrument_probe`; `views::inst` and a scaffold editor
  per instrument; `locales/areas/instruments` and `locales/inst/`; schema 16 (14 and 15 went to the host and the engine work of the same round).
- Live MIDI now reaches native instruments as controllers (CC, pitch bend); Firmus and
  Pulsus ignore them, as before.
- A scaffold kind is offered in the instrument picker only in debug builds until its
  module sets `READY`.
- Not yet: instrument latency is reported but not compensated (a track still leaves at
  zero in ADR 0018's plan); per-pad outputs for Pulsus; the host-wide preset library with
  audition-with-clip; per-voice modulation dots (the telemetry carries what an instrument
  chooses); the hosted-instrument adapter over `cantus-host`, which waits for ADR 0019's
  host to land.

## At integration (2026-09-25)

The seven builders kept to their files, with these shared changes, each argued in its
instrument's document:

- **`inst::prepare_assets`** (Unda): `units_for` — the one place the backend and the
  offline renderer both pass before a snapshot — first builds whatever an instrument's
  assets need off the audio thread (an imported table's mip levels) into a registry that
  never evicts; `configure` only looks it up. Any kind may use it.
- **`SampleCache::attach`** (Exemplar): non-WAV files go to its own decoders (AIFF parser,
  claxon for FLAC, lewton for Ogg; WAV is byte-identical to before), and a sampler track
  also gets each zone's mip levels as extra assets `z{n}.sample~k`, from a never-evicting
  cache.
- **Pulsus II** (schema 17): pads are addressed by a stable `PadId` in every command;
  `NoteEvent::ordinal` lets round robin survive a seek; `fire()` hands a kit the note's
  tick, id and ordinal; plain CCs reach Pulsus; `to_pad` moved into
  `cantus_engine::pulsus_kit`.
- The frame's footer prints a sampler's memory and zone count; the screenshot tool calls
  `demo_feed` after `demo_setup`.

The frame itself changed after the builders' screenshots: white keys are outlined (they
vanished on a light panel), the modulation strip packs its sources into rows, and a macro
knob carries the macro's name with an `M1…M8` handle above it.

## Amendment, 2026-10-14 (round 26, sampler): zones a controller starts, and streaming

- **A fifth trigger, `Controller`** (`z{n}.trig` = 5, with `z{n}.cc`, `z{n}.cclo`,
  `z{n}.cchi`; schema 24, whose migration writes nothing but makes an older build refuse a
  project whose pedal noises it would play on the keys). A zone starts when its controller
  crosses into its range from outside — on the next sample, owned by no note, at its root
  and full velocity, with a round-robin count and a random number per trigger (forgotten at
  a stop, which `at_rest` therefore includes). A looping zone is held while the value stays
  in range; a zone that does not loop plays to its end (as SFZ players play pedal noises);
  classes choke it like any layer. The SFZ importer maps `on_loccN`/`on_hiccN` (−1: no
  trigger) and writes them back; the Salamander's four pedal-noise files are in its pack
  again. Units hear every controller by its MIDI number (`Sustain` is 64).
- **Streaming from disk** is ADR 0050: `InstrumentUnit` gained `set_offline` (an offline
  render's voices wait for disk) and `take_late_reads` (counted into `EngineStats`), both
  with defaults; an Exemplar registers its layers' board with the stream reader when it is
  made, and `configure` finds a streamed sample through a lock-free registry.
