# ADR 0139 — Loquax, the vocoder

**Status:** proposed (fx side round wave A, provisional number — renumbered at merge) ·
**Date:** 2026-11-16 · **Schema:** 64 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0022 (an effect's MIDI source and sidechain),
ADR 0031 §5.1 (at rest), ADR 0071 (effects that sound on insert), ADR 0134 (Tabula)

## Context

Cantus had no vocoder; FL Studio's Vocodex is the bar the owner holds us to, with Arturia's
Vocoder V, Waves' OVox and Morphoder and the Roland VP-330, SVC-350 and EMS 2000 as the
character references (read about, nothing copied). The DSP was built first in the DSP lane
(`docs/round-fx/loquax.md`: Butterworth banks of order 4/6/8 within 0.003 dB of the
analytic response, NCM 0.74 at 16 bands, latency 0, CPU 1.6–4.7 % of a core).

## Decision

A new effect kind, `EffectKind::Vocoder`, model `cantus_core::fx::loquax`, engine unit
`cantus_engine::fx::loquax`, DSP `cantus_dsp::loquax`.

- **Parameters** (100 keys in phase A, 101 with `obj` in phase B; every default a null — the mix at 0): mix, carrier source,
  swap, bands (8–64), slope order, range, width, followers, formant and band shift, tilt,
  flatten, sibilance and consonant noise, hold, stereo spread, the raw carrier through, the
  synthesiser (saw/pulse/noise blend, unison, octave, envelope, level), the robot's drone,
  and one level per band, `b0` … `b63` (the display's drawn bars), `out`.
- **Routing — the carrier and the voice.** The strip's own sound is the voice. The carrier
  is the built-in polyphonic synthesiser played by the notes of the track the slot listens
  to (`EffectSlot::midi_from`, `TAKES_NOTES`), and/or the slot's **sidechain**
  (`EffectSlot::sidechain`, `TAKES_SIDECHAIN`): the engine's ordinary key path — the strip
  runner reads the keyed strip's buffer and hands it to `FxUnit::process` as `key`, exactly
  as for a compressor's detector. Nothing new was built for it; `src` chooses the
  sidechain, the synthesiser or both, and `swap` makes the key the voice and the strip the
  carrier. A strip with a note listener never rests, so held keys always reach the unit.
- **Notes** wait in a fixed list of 64 per block (an allocation-free insertion sort keeps
  them in time order) and reach the DSP with their frames.
- **Starting preset** (ADR 0071): *Robot* — the drone on C3, 16 bands, all wet: a voice
  through it is vocoded at once with no notes and no sidechain.
- **Telemetry:** the DSP's own 32 numbers, then the bands' output levels in 32 groups.
- **Not yet a pedal** on Tabula (`pedal_allowed` false until its face joins the board in
  phase B).

## Phase B (2026-11-17): the editor, presets, export (design sheet v2 approved)

- **The window as approved** (`views/fx/loquax*.rs`), with the owner's two adjustments to
  v2:
  - **The display keeps its size.** Where a language's cards need another row (Spanish:
    four, English three — `loquax::card_rows`, held to what the cards' packing measures),
    the designed size grows a row (`loquax::designed_size`, 1340 × 900 in English,
    1340 × 1030 in Spanish); the display, the microphone and the meters are the same size
    in both languages at 75, 100 and 200 %.
  - **Three objects to speak into**, chosen in the ⋯ menu (*Microphone*, three radio rows,
    then the colourways with their swatches — Vetus's pattern): the gooseneck (the default),
    the **talk-box** (its driver's housing on the floor with the on-air lamp, footswitch and
    jack, the satin driver and nozzle, the clear tube carrying the voice's light up to a clip
    on its stand, its open end glowing) and the **desk microphone** (a weighted base with its
    rubber foot and talk bar, the on-air lamp, a chrome stem with a knurled lock, the yoke
    and pivot knobs, a satin capsule with a slotted grille lit by the voice).
    `views/fx/loquax_objects.rs`; the housing and the base wear each colourway's finish
    (brushed aluminium, black lacquer with chrome lips, the road case's steel corners).
- **The object is kept per insert, not per user.** Colourways are the user's (settings,
  ADR 0130); the owner asked for the object *per instance*, so it is a key, `obj` (0 … 2,
  appended after `b63`; 101 keys): saved in the song, one step of undo, in the exported
  plug-in's state. It is **a look** — `fx::loquax::LOOK`, `EffectKind::is_look`: never heard
  (the engine ignores it), and a preset neither carries nor resets it, nor does a value off
  its default make a preset "modified" (the host's `load`, `apply_fx_params`,
  `frame::modified`), so the user's microphone stays as presets change. The exported
  plug-in lists it among its parameters (a host may automate it; it changes no sample).
  Schema 64 stays (provisional, unreleased): an older 64 file simply has no `obj` and reads
  the gooseneck.
- **The band levels are drawn**: a stroke across the bank's screen sets every band it
  crosses to the level at the pointer's height (−24 … +12 dB in half decibels, snapped to
  0 within a decibel), one merged gesture — one step of undo.
- **Presets**: fifteen (`presets/effects/vocoder.toml`), both carriers (the sidechain in
  *Talk-box*, *Sidechain carrier*, *Talking drums*; the notes in *Classic vocoder*, *French
  house robot*, *Choir pad*, *String machine*, *Helium*, *Sixty-four bands*, *Talking bass*;
  both in *Synth and sidechain*; the robot in *Robot*, *Whisper*, *Deep robot*, *Intercom*),
  roles vocal and creative. The browser's fixed source gives a vocoder its carriers
  (`preset_audition::vocoder_carriers`: the source's last melodic line as its notes and its
  key), so every preset sounds in an audition and `preset_tool fit` can level it: −4.9 …
  +1.9 dB; the starting preset *Robot* +0.5 dB with them.
- **Export**: `Cantus Loquax` (CLAP and VST3; `desc.rs`, the `loquax` feature, xtask):
  notes on its note port / event bus play the synthesiser, the sidechain input carries
  when the plug-in's own switch is on *Sidechain input*; its direct DSP in the plug-ins'
  tests; ids blessed (appended). The plug-ins' suite found the DSP panicking at a host's
  1 234.6 Hz (the bank's top above Nyquist): under 3.6 kHz the bank is squeezed under its
  Nyquist; from 3.6 kHz up nothing changes.
- **AI**: the unit's words name `obj` and the presets; the effect words name what it is for;
  the MCP tools golden regenerated on purpose.
- **Not a pedal.** A board has no key (`tabula::TAKES_SIDECHAIN` false) and hears no notes,
  so a Loquax pedal could only drone its robot: Loquax stays off Tabula (`pedal_allowed`)
  until a board routes a key to a pedal.

## Consequences

- A vocoder that needs nothing to start (the robot), then either carrier the user has — a
  track's notes or another track's sound — in Cantus and in another DAW alike.
- A look can now be a key without being a sound: `EffectKind::is_look` is the one place
  that says which keys presets leave alone (Loquax's `obj` only, today).
- The editor's window height depends on the language (only Loquax's, today); a theme with
  a larger text size could still need a row the table does not give, and then the display
  gives up height rather than a card its words.
- The plug-in shows `obj` to hosts as a parameter; harmless, and it travels with the
  plug-in's state as the owner asked.

## What the tests hold

`cantus-engine/tests/fx_loquax.rs`: the null to the bit and latency 0; the starting preset
sounds, live = offline, blocks of 1/37/64/512 at tolerance 0; another track's notes play
the synthesiser (live = offline, blocks of 1 and 64); the sidechain carries and `swap`
trades the roles (live = offline, three block sizes); a fork carries on as the unit does;
**the object touches no sample** (`the_object_is_a_look_and_touches_no_sample`).
`fx_contract` (`split::loquax`, `at_rest::loquax`), `rt_memory::loquax_never_allocates`,
the undo fuzz (its parameters through `SetEffectParam`). The DSP's 22 tests
(`docs/round-fx/loquax.md`). `cantus-core`: the table's keys, `obj` last and a look.
`cantus-ui/tests/loquax_e2e.rs` (through `CantusApp::assemble`): Robot heard with no
notes and no key; the synthesiser silent without notes and heard with a muted keyboard
track's notes; the formant edit heard and undone to the bit; the object changes no sample;
the sidechain carrier silent without a key and heard with the pad's strip; the defaults
the dry master to the bit. `loquax_drag.rs` (real pointer events): a stroke draws the
band levels, one undo step; the ⋯ menu chooses the object (one undo step) and a colourway
(the settings'); the carrier's and the roles' switches thrown by a click, one undo step
each. `fxa_whole.rs`: no word cut or clipped with each object, EN/ES, 75–200 %, every
colourway; `loquax_s_display_keeps_its_size_in_every_language_and_scale`.
`cantus-plugins`: `clap.rs` `loquax_s_carrier_is_the_sidechain_input_when_asked`,
`vst3.rs` `loquax_s_carrier_is_the_vst3_sidechain_bus`, `notes.rs` (Loquax's case through
CLAP in both dialects and VST3, every block size), the suites over every exported kind,
`ids.rs`. The preset suites (`preset_audit`, `preset_render`, `stated_preset_counts`).


