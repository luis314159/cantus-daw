# ADR 0028 — Hosted CLAP/VST3 instruments on tracks

**Status:** Accepted · **Date:** 2026-10-05 · **Extends:** ADR 0019 (hosting standard
plug-ins), ADR 0023 (native instruments), ADR 0018 and 0022 (delay compensation) ·
**Related:** ADR 0029 (the device picker)

## Context

Round 17 hosted CLAP and VST3 *effects* (ADR 0019) and gave native instruments one
contract, `InstrumentUnit` (ADR 0023). STATUS left the obvious next step: *"Hosted
CLAP/VST3 instruments on tracks: an adapter from `cantus_host::AudioThread` to
`InstrumentUnit`, on `cantus-host` — never a second host."* Round 20's owner asked for a
gallery of every instrument, native and hosted (ADR 0029), and for free instruments that
"confirm compatibility at once". A track has to be able to play Surge XT or Dexed as
naturally as Unda: notes to the sample, its controllers, the song's tempo, its latency
compensated, its sound saved with the project and bounced exactly as it was heard, its own
window one click away, and a project that opens where the plug-in is missing.

## Decisions

### 1. The model: `InstrumentSpec::External(ExternalPlugin)` — schema 19

The record an insert slot already keeps for a hosted effect (format and id to find it,
name and vendor to show when it is missing, its opaque state as base64, its parameter
values by its own ids as `f64`) is reused unchanged as a track's instrument. One record,
one set of rules: values are what undo (and later automation) work on; the state is what
the plug-in says when the project is saved. Two commands, the instrument twins of
`SetPluginParam` / `SetPluginState`, keyed by track: `SetInstrumentPluginParam { track,
id, value: Option<f64> }` and `SetInstrumentPluginState { track, state, params }`, with
exact inverses and refused on a track whose instrument is not hosted; both are in the undo
fuzz, and `SetInstrument` with an `External` spec too. Schema **19** (no migration writes
anything; the number is what makes an older build refuse a hosted instrument instead of
dropping it). Fixture `v19.cantus` holds one installed nowhere.

### 2. The engine learns no format: `UnitKind` and five default hooks

A unit on its way to a track (`NativeUnit`) is now labelled with a `UnitKind`:
`Native(InstrumentKind)` or `Hosted(key)`, where the key is FNV-1a over the plug-in's
format and id (`cantus_engine::hosted_key`, computed without allocating, so the audio
thread matches a snapshot's `External` spec against the unit it holds). The processor
treats a hosted unit exactly as a native one (`SlotInstrument::Native`); a track switching
from plug-in A to plug-in B swaps units because the keys differ; a unit not delivered (a
missing plug-in) leaves the track silent.

`InstrumentUnit` gained five default methods, so every native unit is unchanged:
`configure_hosted(&ExternalPlugin)` (each snapshot's values), `set_position(&SongPosition)`
(before every segment — the round-17 item "the song's tempo and position for every unit"
is thereby done for natives too, should they want it), `param_events` /
`clear_param_events` (the plug-in window's gestures, pushed as
`EngineEvent::InstParam { track, event }` and collected by `EngineHandle::take_inst_params`)
and `stop` (called on the audio thread before any instrument unit is handed back, so a
CLAP plug-in's `stop_processing` happens where the standard says).

Delivery mirrors effects: `EngineHandle::{add_instrument, replace_instrument,
knows_instrument}`; `EngineMessage::ReplaceInstrument` swaps a unit in for the one running
for the same track and kind (a restart after a latency change) and re-plans compensation.
`render_with_instruments` is `render_with_units` plus hosted instrument units.

### 3. Instrument latency is compensated

A track used to leave at zero. Now a track leaves at its instrument's latency: what it
feeds arrives no earlier, and every other path waits (`Processor::compensate` seeds each
strip's arrival with the latency of the tracks feeding it; the track edge is delayed by
the difference). This applies to native instruments as well, which ADR 0023 said reported
latency "but not compensated". A synth with 100 samples of lookahead and an inverted twin
without it cancel to exact silence.

### 4. The adapter: `cantus_host::HostedInstrument`

An `InstrumentUnit` around a backend's `AudioThread`, on `cantus-host` (GPL side), so the
engine stays format-free:

- **Notes, sample-accurate.** The processor splits a block at note times and calls
  `process` per segment, so a note given to the unit belongs to the first sample of the
  next call: it is queued (fixed capacity 512) and sent at offset 0. Correct at any block
  size and identical across them for a deterministic plug-in (the test instrument is, to
  the bit, at 7…4096).
- **Note ids** are the engine's, folded to 31 bits (CLAP's and VST3's width; never −1), so
  note-offs and per-note expressions find one note when two share a key. The unit
  remembers what sounds (256), so `all_notes_off` (stop, seek, loop wrap) releases exactly
  those notes and the sustain pedal: no hanging notes when the song jumps.
- **Controllers** (pitch bend, mod wheel, sustain, expression, channel pressure, any CC)
  become a new contract event, `NoteKind::Midi { status, data1, data2 }`. CLAP sends it as
  `CLAP_EVENT_MIDI` when the note port speaks MIDI; a port that speaks only CLAP gets pitch
  bend as a channel-wide tuning expression; a port that speaks only MIDI gets its notes as
  MIDI too (`NoteDialects`, read from `note-ports`). VST3 has no CC events: the controller's
  `IMidiMapping` is read once at activation (16 channels × 130 controllers) and a message
  becomes a parameter change on the parameter it names (and is echoed to the controller,
  as the host's own changes are). Those parameters are inputs, not settings — JUCE
  plug-ins expose about 2 000 "MIDI CC" ones — so the VST3 backend now reports them hidden
  and read-only: the generic editor does not list them and a project does not store them.
- **Values and gestures** share the insert's machinery, extracted from `HostedUnit` into
  `unit::ParamSync`: snapshot values diffed without allocation, the no-echo rule, a
  gesture in the plug-in's window as one undo step.
- **Latency** is read on the main thread when the unit is made; a change arrives as a
  restart, and the new unit carries the new value into compensation (`Silent` stands in
  while the old half comes back to be deactivated).
- `process` writes over the zeroed buffers (that is the contract's "added") and returns
  whether any sample is non-zero, so an idle hosted track is still skipped downstream.

### 5. The session: instruments by track

`PluginHost` keeps instruments by `TrackId` beside the inserts by `EffectId`
(`session_inst.rs`, `session_inst_idle.rs`, children of `session`): `make_instrument`
(load once, read its parameters and state, return the spec for `SetInstrument`),
`sync` (load what is new — state, then values flushed, then a restart if the state asked
for one, before the first block — deliver, unload what left with its state **parked by
track and plug-in** so an undo brings the sound back, load a state the project changed),
`idle` (requests, restarts, the window's gestures as commands, a preset chosen inside the
plug-in as one `SetInstrumentPluginState`), `project_for_save`, `render_instruments` (fresh
instances with the live state for a bounce, bit-identical to playing), and the plug-in's
window (`open_instrument_window`, `close_instrument_window`) on the same native window
code as effects. Notices: `InstrumentLoadFailed`, `InstrumentRestartFailed`,
`InstrumentWindowClosed`.

**A missing plug-in** opens: no unit is delivered and the track plays silence; the host
reports `InstrumentLoadFailed` (the status line says the name and why, in both
languages); the inspector shows *Missing: silent* with the reason; saving writes the record
back byte for byte.

### 6. The application

`CantusApp::set_hosted_instrument(track, &PluginInfo) -> bool` is the one entry point:
it loads the plug-in and replaces the track's instrument in one command (one undo step;
the clips stay). The picker's `apply_pick` calls it for `PickChoice::Hosted` when the
plug-in is an instrument, for both `ReplaceInstrument` and `AddInstrument`
(`CantusApp::hosted_instruments()` lists the scanned instruments for the gallery).
`open_inst_editor` on a hosted track opens the plug-in's own window instead of the native
frame, so every place that opens an instrument editor — the inspector, the track, the
device chrome — opens the plug-in. The inspector's instrument section shows a hosted
instrument's card: name, maker, format, status, latency, parameter count and the window
switch. Export bounces hosted instruments through `render_instruments`.

## What the tests hold

`crates/cantus-host/tests/hosted_instruments.rs`, against `cantus-test-clap-synth` (built
from source; a sine per note, gain, latency with restart, mod wheel, pitch bend, a
transport readout, a performed gesture, state with a tag):

- the scan says *instrument*, 0 in / 2 out; a chord on beat 2 starts on sample 24 001
  (sample 24 000 is the sine's zero) with exact silence before it;
- blocks of 7, 64, 256, 1000 and 4096 give identical samples; an offline render with fresh
  instances (blocks of 333) equals playing, to the bit;
- a project value reaches the plug-in (−20 dB is a tenth) and undo takes it back;
- state round-trips through save → open into a fresh host, with the same samples;
- a missing instrument opens, plays exact silence, says so, and keeps its record;
- a latency of 100 samples (through a restart) cancels an inverted twin exactly, and the
  latent path alone starts 100 samples later;
- a gesture in its window is one undo step and does not echo;
- tempo and play state reach it; a stop releases every note to exact silence; switching
  to Firmus and undoing brings its sound back; its window opens and closes (Windows).

Real instruments: `tests/real_instruments.rs`, ignored, driven by `CANTUS_PLUGINS`; results
in `docs/round20/hosted-inst.md`.

## Consequences

- New: `InstrumentSpec::External`, `Command::{SetInstrumentPluginParam,
  SetInstrumentPluginState}`, schema 19; `cantus_engine::{UnitKind, hosted_key,
  render_with_instruments}`, `NativeUnit::hosted`, `EngineMessage::ReplaceInstrument`,
  `EngineEvent::InstParam`, five `InstrumentUnit` defaults; `cantus_host::{HostedInstrument,
  HostedInst, inst_unit::Silent}`, `NoteKind::Midi`, `unit::ParamSync`; the test synth.
- Changed for everyone: instrument latency now counts in delay compensation; stems now
  receive native instrument units (they were silent in stems before — found here).
- Not built: hosted instruments in **stems** (hosted inserts are wires there too; both
  need `render_stems` to take units); multi-output instruments (only the main stereo out
  is heard); MPE (note expressions travel, but nothing in Cantus produces them per note
  yet); CLAP `render` (offline mode) and `voice-info`; a plug-in's own preset list in the
  gallery; crash isolation for a running plug-in (unchanged from ADR 0019).
