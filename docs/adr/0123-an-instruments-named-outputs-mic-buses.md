# ADR 0123 — An instrument's named outputs: microphones, perspectives and noises as strips

**Status:** Proposed for round 40 phase 3 (contract; builders `tymp-engine`, `tymp-ui`,
`chorda-src`) · **Date:** 2026-11-12 · **Schema:** 66 · **Generalises:** ADR 0100 (Pulsus
III: buses, taps, *Split to strips*, a strip's own pan law) · **Builds on:** ADR 0004
(commands), ADR 0023 (native instruments: numbers by key), ADR 0031 (the graph), ADR 0050
(streaming), ADR 0057 (modulators) · **Research:** `F:\projects\daw-r40\research-p3\`
(`summary.md`, `formats.md` §1.6, `competitors.md` §7) · **Design:**
`F:\projects\daw-r40\design-p3\` (the drummer's *Mixer*: families, **Out** cells, the three
actions)

## Context

The owner asked (2026-11-12) for an acoustic drummer whose every microphone can be its own
mixer strip, and for Chorda's mixer with outputs to strips (perspectives; for a recorded
pack its own mics; noises on faders of their own). ADR 0100 already carries a *pad* to a
strip of its own as a **bus**, played by the engine as a **tap track**, summed back to the
kit to the bit. It is Pulsus-only: `DrumPad::output` is the model, and `feeds_through_bus`,
`orphans` and `output_strips` read pads. A native instrument (Chorda, the new drummer,
Exemplar's multi-mic instruments next) has nowhere to say "this output plays there", and
its internal mixer has no keys.

Every leader makes multi-outs tedious (competitors.md §7): Superior Drummer's 32 outputs
need the DAW's channels made and named by hand; Addictive Drums and BFD route by menus;
Kontakt's output section is a second mixer that does not know the DAW's. The bar is the
summary's first point: *every mic a named, coloured Cantus strip in one click; the split
sums to the internal mix to the bit*.

## Decision

### 1. A named output, and where it plays

`cantus_core::inst_out`:

- `OutputId(u16)` — stable for the instrument's **definition** (a kit's mic, a pack's
  perspective, the model's *Hammers*); `n` of its `o{n}.` keys; at most `MAX_OUTPUTS` = 32.
  For a Pulsus kit's view it is the pad's id.
- `OutputFamily` — `Other, Kick, Snare, HiHat, Toms, Cymbals, Overheads, Rooms,
  Perspective, Noise, Pad`: the mixer's family heads and *Pieces to strips*' grouping.
- `OutputSlot { id, name, family, strip }` — one output that plays **somewhere other than
  Main**. `NativeSpec::outputs: Vec<OutputSlot>` holds them sorted by id; an output it does
  not list plays on **Main** (the instrument's own strip). The name and family are copied
  from the definition so a song whose pack is missing still names its strips, and so a kit
  change carries routes over by name, then family (as `padout::carry_outputs` does by pitch).
- `RouteView { Main, Own(strip), Chosen(strip) }` — what the **Out** cell says: a strip of
  kind `StripKind::Pad` (now documented as *any* output's own strip; the serialized name
  stays) is *Own*, any other *Chosen*.
- `InstrumentSpec::outputs()` shows both models (Pulsus pads, native slots) as
  `OutputView`s; `InstrumentSpec::feeds_strip` answers for both, and
  `Track::feeds_through_bus` now asks it — so `tracks_on_strip`, `orphans`, `kit_of_strip`,
  freeze, solo-follows, stems and the record mode see a native output's strip exactly as
  they see a pad's (ADR 0100 §5, §9).

**What is project state and what is not.** The route (`outputs`) and every number of the
internal mixer are the project's: a song is a mix. What an output *is* — which outputs an
instrument has, their names, channels, positions — is the instrument's definition (Cantus's
tables for modelled instruments, the pack for recorded ones), never written into the song.

### 2. The internal mixer: ordinary numeric keys

`o{n}.level` (dB, −96…+12, 0), `o{n}.pan` (balance, 0), `o{n}.width` (0 mono…1, 1),
`o{n}.invert`, `o{n}.mute`, `o{n}.solo` (0/1, 0), `o{n}.delay` (ms, 0…50, 0), `o{n}.on`
(1: loaded). `InstrumentKind::param` accepts them for every kind that `has_outputs()`
(Chorda, Tympanum; Exemplar next). So automation, modulators, MIDI learn, presets ("Close
jazz", "Hall", "Player's ears"), undo, the agents and a CLAP/VST3 export reach them with no
new path. `o{n}.on` (and a bleed cell's `on`, ADR 0124) loads and unloads samples off the
audio thread: **set, never automated** (`InstrumentKind::automatable`).

**Null at every default**: 0 dB is a gain of exactly 1, a centred balance passes, full
width passes, no delay is no line. `cantus_dsp::outmix::OutMix::is_unity` makes "touches no
sample" literal.

### 3. Buses and taps, as ADR 0100

Bus 0 is the track's own output; bus `k ≥ 1` is the `k`-th distinct strip the slots name,
in slot order (`inst_out::strips_of`, `bus_of`, allocation-free). `InstrumentSpec::
output_strips()` returns them for a native instrument, so `padout::append_taps` gives each a
tap track and the engine calls the unit's `InstrumentUnit::process_buses` (bus `k` handed
to `out(k, offset, l, r)`). Nothing about delay compensation, sends, VCAs, freeze, stems or
the record mode is new.

**To the bit.** The unit renders each output on its own lane (starting at +0.0) and adds
the lanes **in output-id order** into bus 0 when unsplit; split, each lane goes to its bus
and the instrument's strip gathers the own strips in that order (they are placed so, with
the 0 dB law, ADR 0100 §3). Same lanes, same order, same bits — a split Tympanum (every mic
its own strip) sums to the unsplit Tympanum to the bit, offline at 512 and 333 frames and
live on the pool.

### 4. One-click actions, not a mode

`inst_out::plan(project, track, OutputPlan, make_strip)`: **All to Main** (ADR 0100's
*Merge back*: slots cleared, own strips removed when nothing else uses them), **Pieces to
strips** (`ByFamily`: one strip per family), **Each to its own strip** (`EachOwn`). One
batch, one undo step; the result said in words ("3 strips in the mixer · 12 mics on Main").
A cell changed afterwards is not a fourth state to explain. `inst_out::set_route` builds the
single cell's command for any instrument (`SetPadOutput` for a pad, `SetInstrumentOutput`
for a native output). Stereo pairs (overheads, rooms) are one output (`FoundMic::channels`
= 2), so one strip.

### 5. Finding the microphones of an imported instrument

`MicRule` — in order, the first that applies (formats.md §1.6), logged by the importer:
`Outputs` (SFZ `output=n`, Decent Sampler `AUX_STEREO_OUTPUT_n`), `LevelCc` (an ARIA
`<master>` or `<group>` with its own `amplitude_onccN`/`volume_onccN`, named by
`label_ccN`; Decent Sampler's tag bound to `AMP_VOLUME`), `Folders` (paths or tokens that
differ only by a mic word), `Channels` (a DrumGizmo kit), `Single`. A `FoundMic` carries its
name, family (`family_of_name`), source CC (its fader now, not a CC lane), starting level
(`set_ccN`), polarity (a negative `amplitude`) and channels.

### 6. Rejected

- **A per-instrument output model in each kind** (Chorda's own, Tympanum's own): three
  copies of routing, orphans, freeze and stems rules — ADR 0100 §2 rejected the same for
  buses.
- **Storing every output, Main ones included**: a song would name what it never routed,
  and a pack's new mic would be missing from old songs; absent-is-Main keeps old songs and
  new outputs right.
- **A second internal-mixer data structure** (a `Vec<MicMix>` in the spec): it would need
  its own automation targets, its own presets, its own undo. Keys are what every native
  number already is.
- **Pulsus moved onto `NativeSpec::outputs`**: a schema migration of every kit for no
  gain; `OutputView` shows both.

## Consequences

- Schema **66**: `NativeSpec::outputs` (default empty) and `Command::SetInstrumentOutput`;
  the migration writes nothing; every older song renders to the bit.
- A strip made for any output is `StripKind::Pad`; *every* rule ADR 0100 gives such strips
  (goes with its instrument, never handed to a new track, *Merge back* removes it) holds.
- `tymp-engine` generalises the remaining Pulsus-only paths (`padout::split_with`,
  `merge`, `heard_on`, `carry_outputs`, `with_kit_strips_following`, the template and
  DAWproject notes) to named outputs, or states why one stays Pulsus-only.

## What the tests hold

- `inst_out` unit tests: keys parse, every default is null; buses count distinct strips in
  slot order.
- The undo fuzz arm `a_named_output_undoes_and_redoes_among_the_others` (contract), grown
  by `tymp-engine` with the plans.
- To come (builders): a Tympanum and a recorded Chorda split *Each to its own strip* sum to
  the unsplit to the bit at 512 and 333 frames and live; a default `o{n}` touches no sample;
  `rt_memory.rs` with every output split; an e2e through `CantusApp::assemble`: a kit plays
  into its mic strips and the split sums to the unsplit.

## Amendment — round 40 phase 3: families on the desk, sub-groups, added strips (schema 70)

The owner's decisions of 2026-11-12, with ADR 0016's amendment (a group is a band):

- **Families on the desk.** An instrument's own strip and the strips its outputs play on sit
  under a family band in the track's colour: a solid bar, its name, the instrument and its
  count (*Drums · Tympanum · 3 mics*). Folded (the view's choice), the outputs' strips are one
  strip with their meters side by side; the instrument's strip stays (they sum into it). On
  the desk a member keeps its short name; elsewhere it is *Drums · Snare Top*
  (`mixer_group::full_name`).
- **Sub-groups by output family.** *Each mic to its own strip* gathers the new strips into a
  user group per `OutputFamily` (Kick, Snare, Hi-hat, Toms, Cymbals, Rooms…; a family of one
  strip takes none), inside the instrument's band, in the same undo step as the split; *All
  to Main* and *Pieces to strips* take them away in theirs.
- **A strip added to a family** (schema 70): `MixerStrip::attached` names the instrument
  whose band a strip was added to — a strip fed only by the family's strips (its bus, a
  parallel compression). It is drawn at the band's end, marked *added*; it folds with the
  family. While its routing stops coming only from the family it leaves the band, and the
  status line says why; restored, it is back. `Command::SetStripAttached { id, track }` with
  its exact inverse (undo fuzz arm `a_strip_attached_to_a_family_undoes_and_redoes_among_the_others`);
  the strip menu offers *Add to Drums's band* / *Take out of …*. Never heard. Migration 70
  writes nothing; fixture `v70.cantus` (the Tympanum's *Kit Bus* added).

## Amendment — round 40, fix-e: an output's delay line glides, and exists when it is set or automated

The review of 40a27bb7 found `o{n}.delay` (§2) unusable under automation: each change of
the whole-sample delay **cleared** the line (an automated 0 → 20 ms ramp was silence for
the whole ramp; dragging the knob clicked), and a line existed only for a delay *set*
non-zero, so a delay automated up from 0 did nothing.

- **The rule for a line** (shared by Exemplar and Tympanum): an output has a delay line
  when its `o{n}.delay` is set non-zero **or a lane drives it** (a modulator drives a lane,
  so it counts) — `sampler_outs::delay_mask(spec) | sampler_outs::automated_delays(
  snapshot.automation, track)`, read without allocating by `UnitKind::of_track` (backend,
  `units_for`, the processor's swap). The unit is built with exactly those lines off the
  audio thread (`UnitKind::Exemplar { delays }`, new `UnitKind::Drums { room, delays }`
  replacing `DrumRoom(u8)`) and swapped in with the snapshot. No delay and no lane: no line
  — the unit renders to the bit what it did before. Tympanum's `Outputs` is sized by the
  same mask (it held 32 lines of 50 ms, ~615 KB a unit at 48 kHz, and its reset wrote them
  all on the audio thread).
- **A line never forgets** (`cantus_dsp::tympanum::OutDelay`): it behaves as a line fed every
  sample. A change while it holds sound **glides** the read head (an exponential approach of
  `DELAY_GLIDE_MS` = 30 ms in integer 1/65 536-sample steps, at most half a sample per
  sample, so the head never runs backwards: a tape's pitch bend), reading between samples
  while it moves; arrived, it reads whole samples, so **a static delay gives exactly the
  samples it gave before**. While everything the head can reach is silence the delay is
  taken at once. A span its owner does not run (nothing in, nothing to give) is
  `OutDelay::skip`, the same as feeding zeros; a line "rings" until it is all silence at its
  delay, so a unit at rest skips nothing it would later play. Every decision is per sample
  (the run of zeros written) or on the automation's grid: block-size independent, offline
  equals real time.
- **A line at 0 passes to the bit**, so a lane flat at 0 changes no sample of a Tympanum. An
  Exemplar with a line renders output by output from the start (`by_output`), so a ramp up
  from 0 never switches its render path mid-play.
- **What the tests hold** (fix-e): `tympanum_delay.rs` — an automated ramp and a knob's jump
  glide with no silent gap and no step above 0.03 on a 0.5 tone, at chunks 16, 7 and 1; a
  static delay shifts by whole samples to the bit (0, 2.5, 50 ms); skipping silence equals
  feeding it, at any cut; `tympanum_fixe_tests.rs` — a unit owns lines only for its mask and
  a line at 0 renders the bits of no line; `tests/inst_out_lanes.rs` — a lane on an output's
  delay gives the unit its line (`UnitKind::of_track`) and an automated ramp through the
  real engine has no gap, at 512 and 333 frames to the bit.
