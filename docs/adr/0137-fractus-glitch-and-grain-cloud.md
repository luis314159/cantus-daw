# ADR 0137 — Fractus, the glitch sequencer and grain cloud

**Status:** proposed (fx side round wave A) ·
**Date:** 2026-11-16 · **Schema:** 74 ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0022 (the memory path; an effect's MIDI source), ADR 0031 §5.1 (at rest), ADR 0071
(effects that sound on insert), ADR 0083 (a range render's checkpoint), ADR 0129 (Lapsus)

## Context

Lapsus is a time curve; the owner's list (Effectrix, Turnado, Stutter Edit, Beat Repeat,
Portal, Granulator) asks for a *sequence of events*. The DSP was built first
(`docs/round-fx/fractus.md`: ten modules on ten lanes, a grain cloud, every choice a
function of the song sample and the seed, joins −62…−71 dB of splatter, latency 0).

## Decision

A new effect kind, `EffectKind::Glitch`, model `cantus_core::fx::fractus`, engine unit
`cantus_engine::fx::fractus`, DSP `cantus_dsp::fractus`.

- **Parameters** (52 keys, every default a null — nothing painted, the cloud at 0): the
  grid, the trigger, every module's settings, the cloud's, `out`. Note values are indices
  into the DSP's fourteen; `vslice`, `ttime` and `grate` keep 0 for their own meaning.
- **The lanes are content, not parameters:** `EffectSpec::fractus:
  Option<Box<Pattern>>` — per lane a sorted list of blocks `{at, len, amount, prob}`,
  only the lanes that hold one; `None` when nothing is painted (and not written). Schema 74,
  `migrate_73_to_74` writes nothing; a hand-edited pattern is repaired on load
  (`fx::fractus::repair_project`). **`Command::SetFractusLane {strip, effect, lane,
  blocks}`** replaces one lane (sanitized: sorted, apart, inside the 256 steps); its exact
  inverse is the lane it replaced; `cmd.fractus_lane`; its undo-fuzz arm.
- **The memory, off the audio thread.** The DSP holds 8.6 MB at 48 kHz (34 MB at
  192 kHz). A unit is made without it — a wire, exactly what the DSP plays at the defaults —
  and asks through the memory path (`MemoryNeed::Fractus`, a value small enough for the
  event) once its settings make it more than a wire. The UI thread builds the whole DSP
  (`EffectMemory::Fractus(Box<Fractus>)`) and the unit takes the box (a pointer moved; it
  hands back one it cannot use). `EffectUnit::new` supplies it at once, so a project's
  render and a live play are the same from the first sample; an empty Fractus costs
  nothing. Once taken it stays (freed with the unit, off the audio thread). This follows
  Lapsus's and Arca's path rather than building 8.6 MB at every insert of an empty unit.
- **The pitch jump's style** (the owner's open choice): `pstyle` — the classic four-head
  delay-line warble (0, the default: the DSP report calls it deliberate, the glitch pitch)
  or **clean** (1): two heads, one sounding at a time with a splice of a fifth of the
  window (sin²/cos², summing to one), no comb between taps. Added to the DSP
  (`PitchSettings::clean`); the classic's bits unchanged.
- **Forks** clone the whole DSP (history, capture, cloud, the sequence's place).
- **Starting spec** (ADR 0071, `Start::Spec`): *Last beat stutter* — a repeat on the last
  beat of each bar of sixteenths.
- **Not yet a pedal** on Tabula (its pattern is not routed through a board; phase B: not a
  pedal, below).

## Phase B (2026-11-17): the editor, presets, export (design sheet v2 approved)

- **The window as approved** (`views/fx/fractus*.rs`, 1340 × 720): the chassis with its three
  windows — the lanes, the grain cloud's scope, the keys that play the lanes — over four pages
  of cards (*Sequencer*, *Slicing*, *Colour*, *Grain cloud*), each one row in English and
  Spanish. The pitch jump's default stays the classic warble (the owner's verdict on v1).
- **The lanes are painted by the pointer** (`fractus_paint.rs`): press on a step and drag
  along the lane — a block from the press to the pointer, **as tall as the pointer's height
  in the row** (its amount, 5 % steps; what it covers is its own: `fx::fractus::paint`);
  **Shift and drag**, a stroke for **chance** over the blocks it passes; **right-click**
  erases a block (`fx::fractus::erase`). Each gesture is one step of undo: a new
  `FxHost::set_fractus_lane(lane, blocks, gesture)` — Cantus runs `SetFractusLane` merged
  under the gesture (`touches` counts it as the device's), the plug-in's window posts the
  lanes to its processor. Gestures are numbered across contexts (two strokes never merge).
  The **Frozen** lamp over the scope is a switch (one step). A lane's name and the page keys
  open pages — a view, nothing to undo.
- **Every stepped parameter has its offer.** The ping-pong switch (`wpp`) had no control: it
  is on the throw's card now (a stepped knob; *Joins* and *Seed* moved to an *Events* card on the slicing page to keep every
  page one row — `fxa_whole::fractus_s_pages_are_one_row_each`). `learn_everywhere` names why the first page cannot draw the
  other pages' keys (`fractus_page`) and a test of its own opens each page and finds every
  stepped key there offered (`every_fractus_page_offers_its_stepped_parameters`) — and every
  stepped key on some page.
- **Presets carry their lanes.** A preset is a flat map of numbers (data files, a user's
  saved preset, A/B); a Fractus preset writes its blocks among its values as
  `"{lane}.{at}" = len`, with `.amount` and `.chance` when not whole
  (`fx::fractus::to_flat`, `pattern_of_flat`, `lane_key`, `lane_value_ok`). Loading one — the
  header's list, the browser (`apply_fx_params`), a host's `load`, A/B — sets values and
  lanes in **one** step; a map without blocks clears them (A/B now stores the flat map:
  `views::fx::preset_map`, which also saves a user's preset with its lanes, and Tabula's
  board). The drift mark compares lanes; the audition paints them
  (`preset_audition::lanes_after`) and plays a notes preset with the source's last melodic
  line as its notes, its first key on that line's lowest note (`fractus_keys`).
  **Eighteen presets** (`presets/effects/glitch.toml`): *Last beat stutter* (the starting
  spec, held equal to it), *Stutter build*, *Tape stop ending*, *Tape start*, *Reverse
  build*, *Reversed backbeat*, *Gated sixteenths*, *Trance gate*, *Filter sweep down*,
  *Delay throws*, *Shuffled sixteenths*, *Lo-fi fills*, *Grain pad*, *Cloud to freeze*,
  *Grain scatter*, and played by notes *Notes: the whole kit*, *Notes: stutters*, *Steps and
  keys*; roles drums, bus, vocal, creative. Fitted by `preset_tool fit`: −1.9 … +6.5 dB
  (*Cloud to freeze* +6.5: only the cloud, its long swelling grains); nine within 0.3 dB as
  written, the starting preset among them (no change to its spec).
- **Export** `Cantus Fractus` (CLAP and VST3; `desc.rs`, the `fractus` feature, xtask): the
  lanes are content in the state (`state::save_spec_extras`/`load_fractus`, the project's
  JSON field `fractus`); on the main thread they are posted to the processor as Lapsus's
  banks are — the post made generic (`lapsus_bank::ContentPost<T: Content>`, `BankPost` its
  Lapsus alias, `take_newest<T>`): swapped in at the next block, the old ones handed back to
  be freed on the main thread. A fresh instance starts on the starting stutter (its lanes and
  its values). **The DSP is built at activation** (`Processor::new`, main thread) whatever
  the settings — a host may paint, raise the cloud or send a note at any sample after — as
  the EQ's linear-phase buffers are; 8.6 MB per instance at 48 kHz. The steps follow the
  host's transport (CLAP's and VST3's), bit-identical to the DSP given the same song position;
  notes play the lanes in CLAP (both dialects) and VST3. The plug-ins' Direct path names
  Fractus's DSP with its lanes written as the engine writes them. Ids appended.
- **AI**: `set_effect` takes `lanes` — Fractus's lanes by name, each a list of blocks
  `{at, len, amount, chance}`, `[]` clearing one (`SetFractusLane`, sanitized); the unit's
  words name it and the presets; the MCP tools golden regenerated on purpose.
- **Not a pedal.** `SetFractusLane` addresses an insert, not a pedal — nothing could be
  painted on a board — and a board hears no notes, so the keys and the presets played by them
  would do nothing. Fractus stays off Tabula (`pedal_allowed`) until a lane command takes a
  pedal (as `SetLapsusPattern` does) and a board routes notes.

## Consequences

- A glitch sequencer painted as one: the lanes are the display, the pointer is the pen, and
  the steps, the notes or both play it — in Cantus and in another DAW alike, with the lanes in
  the plug-in's state.
- A preset can now carry content that is not a parameter in its flat map (Tabula's board
  already did, by its own code; Fractus's lanes by data): `preset_map` is the one place that
  says what a preset of a spec holds, and loaders that only knew parameters (A/B, a host's
  `load`, the audition) learnt the lanes.
- `lapsus_bank`'s post is generic: the next unit with content beside its values (a board's
  pedals have their own, `board_post`) posts it the same way.
- 8.6 MB per exported instance, always (48 kHz; 34 MB at 192 kHz) — the price of never
  building on the audio thread in a host that can change anything at any sample. Inside
  Cantus an empty Fractus still costs nothing.
- A user's preset saved before phase B (none shipped) has no lanes: loading it clears them.

## What the tests hold

`cantus-engine/tests/fx_fractus.rs`: the null to the bit, latency 0, no memory asked when
empty; the starting spec sounds, live = offline, blocks of 1/37/64/512; every lane and the
cloud live = offline; painted while playing it asks once and takes its DSP; another
track's notes play the lanes (live = offline); a fork carries on. `fx_contract`,
`rt_memory::fractus_never_allocates` (empty, painted while playing — the DSP taken —
cloud, notes, cleared and painted again). Core: the pattern's sanitizing and its JSON;
`SetFractusLane` in the undo fuzz. Project: `schema_64_keeps_wave_a_s_kinds_and_fractus_s_lanes`
(and the repair). DSP: `the_clean_pitch_jump_holds_its_level_and_the_classic_warbles`,
`a_pitch_jump_lands_on_its_interval` (both styles), and the DSP's own 26.

Phase B: `cantus-ui/tests/fractus_e2e.rs` (through `CantusApp::assemble`): the starting
stutter heard **on its step** — the master the dry riff to the bit until beat 3 + 1/32
(frame 75 000 at 120 BPM), then the repeats; **what plays live equals the offline render**
(the engine the app drives, blocks of 256/333/512/17/1024); a lane painted with
`SetFractusLane` heard on its step and gone by the next beat, undone to the bit; **notes
from a muted keyboard track** play the gate's lane from the note's sample; an edit heard and
undone to the bit; the neutral defaults with nothing painted the dry master to the bit;
`the_starting_stutter_is_its_factory_preset`. `fractus_drag.rs` (real pointer events): a
block painted by a drag, its height the amount, dragged back over its start, one undo step a
stroke; a Shift stroke sets the chance and keeps amount and steps; a right-click erases (an
empty step changes nothing); the page keys and a lane's name open pages and change nothing;
the Frozen lamp and the Style switch, one undo step each. `learn_everywhere`
(`every_fractus_page_offers_its_stepped_parameters`), `fxa_whole`. Core: `paint` and
`erase`; the lanes through a flat map and back. `cantus-plugins/tests/fractus.rs`: a fresh
instance on the stutter, the lanes round-trip in the state (a state without lanes clears
them); lanes and values posted while playing reach the audio thread with no allocation or
free there, the DSP built at activation. `transport.rs`
`fractus_s_lanes_follow_the_hosts_tempo_and_position_through_clap` (90 BPM from beat 2.5,
blocks 512/1/7/4096/5-333-64, the first repeat at frame 20 000 exactly) and `vst3.rs`
`fractus_s_lanes_follow_the_hosts_tempo_through_vst3`; `notes.rs` (Fractus's case: 64, 67,
71 from D4 play the tape stop, the crush and the cloud — CLAP both dialects, VST3, every
block size); the suites over every exported kind (`clap.rs`: the cloud with no transport,
`out` automated); `ids.rs`. The preset suites (`preset_data`, `preset_audit`).
