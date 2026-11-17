# ADR 0141 — Caelum, the shimmer reverb

**Status:** proposed (fx side round wave A, provisional number — renumbered at merge) ·
**Date:** 2026-11-16 · **Schema:** 64 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0005 (the audio thread), ADR 0018 (null defaults), ADR 0022 (an effect's
sidechain), ADR 0031 §5.1 (at rest), ADR 0071 (effects that sound on insert),
`cantus_dsp::spatium` (its mix law)

## Context

Spatium's rooms, Aula's convolution and the legacy Reverb leave no pitch-shifted feedback,
infinite washes, bloom or freeze (Valhalla Shimmer and SuperMassive, Strymon BigSky,
Eventide Blackhole — function only). The DSP was built first (`docs/round-fx/caelum.md`:
an FDN of 8 or 16 lines with Jot's absorption, RT60 worst band 3.1 %, an energy-exchanging
shimmer that makes no energy, exact rest by a drain, CPU 1.6–4 % of a core).

## Decision

A new effect kind, `EffectKind::Shimmer`, model `cantus_core::fx::caelum`, engine unit
`cantus_engine::fx::caelum`, DSP `cantus_dsp::caelum`.

- **Parameters** (32 keys, every default a null — the mix at 0 over a musical space):
  size, decay, infinite, freeze, bloom, the pre-delay (free or a note value at the song's
  tempo, `psync`), diffusion, high and low damping, modulation, the shimmer (amount, two
  voices, the second's level, the loop's tone), ducking (keyed by the sidechain when one
  is routed), the wet's tone, width, the tank's lines, mix, `out` (applied after the DSP,
  gliding, not at all at 0 dB).
- **The owner's two open questions, each a parameter with today's measured behaviour the
  default:** `ishim` — infinite with the shimmer **fades** as it climbs out through the
  loop's tone (0, the default: −27 dB in ten seconds, −120 dB in a minute) or **sustains**
  (1: once nothing more comes in, the climb settles — the shimmer's amount glides to nothing
  over 1.5 s — and the hold keeps the wash it climbed to: within +0.2 dB for a minute);
  `ilevel` — an infinite tank
  **8 dB under** an 8 s tank (0, the default: it keeps all it is fed) or **as loud** (1).
  Both added to the DSP (`CaelumSettings::{infinite_sustain, infinite_full}`); every
  existing setting's bits unchanged. A preset can choose either.
- **Starting preset** (ADR 0071): *Octave shimmer* — the DSP's `presets::starting`.
- **A pedal** on Tabula from phase B (below).

## Phase B (2026-11-17): the editor, presets, export (design sheet v2 approved)

- **The window as approved**, with the owner's two notes on v2: the astrolabe's octave names
  (+36 … −36) stood alternately left and right of the meridian, close enough to crowd and
  touch — each is now led out from its dot by an engraved line to a column 14 points from the
  meridian (`OCTAVE_NAME_GAP`), the two columns well apart and the alidade between them; the
  frost covered the sky's scale ("+48 st") — the scale's names are drawn after the frost, on a
  plate while the glass is frosted (`caelum_sky::rows`). Shots:
  `compartir/caelum/design/v3-step3/`.
- **The astrolabe answers the pointer** (`caelum_astro::edit`): a press on the plate takes the
  hand whose jewel it is on, else the one whose line passes nearer (`hand_at`) — the alidade
  the first voice (`pa`), the slimmer hand the second (`pb`), heard or not — and dragging
  turns it to the spiral's semitone under the pointer: the angle names the semitone in the
  octave, the distance from the centre the spiral's nearest turn (`semitone_at`). A click
  puts the nearer hand where it falls. One drag, one step of undo (the gesture
  `caelum-voice`); the cards' pills keep each voice's automation and MIDI learn.
- **The decay rack answers the pointer** (`caelum_desk::tubes_edit`): the dotted line is the
  set decay, and a press or drag anywhere on the rack puts it at the pointer's height (the
  log scale 0.3 … 60 s, a tenth of a second under 10 s, a second above); one drag one step
  of undo. **The tubes are not each a control**: each shows what the whole tank achieves at
  its octave (the damping's two shelves shaping the curve), so a per-tube drag would ask for a
  value the shelves cannot give exactly; the damping stays on its card. While infinite the
  rack shows ∞ and takes no drag (the knob still sets the decay the tank returns to).
- **The sill's pushbuttons** as on the sheet (each click one undo step; `fxa_whole`).
- **Presets** nineteen (`presets/effects/shimmer.toml`): the DSP's eleven factory spaces as
  written (a test holds them to `cantus_dsp::caelum::presets::factory()`, the output aside),
  then *Fifth shimmer*, *Twin octaves*, *Infinite drone* (infinite, `ishim` 1: it keeps what
  it climbed to), *Guitar ambience* (ducked by its own playing), *Eighth-note echoes* and
  *Dotted rise* (the pre-delay synced), *Shimmer snare*, *Bus haze*. Roles creative, vocal,
  bus, drums. Fitted by `preset_tool fit`: −3.8 … 0 dB (*Black sky* −3.8; four within 0.3 dB
  as written); **the starting preset moved −0.9 dB** (`out`, in `starting.rs` and the
  editor's demo too). **No preset starts frozen**: a tank frozen from its first sample holds
  silence. *Freeze swell* is infinite and swells in, its note asking for Freeze by hand; the
  two infinite presets build as they are played (an infinite tank keeps what it is fed).
- **Export** `Cantus Caelum` (CLAP and VST3; `desc.rs`, the `caelum` feature, xtask; the VST3
  class id by its rule): the engine's unit through the generic processor, so its ducking
  hears **the sidechain input** when the plug-in's *Sidechain* switch is on the external key
  (CLAP's second audio port, VST3's aux bus) and a **synced pre-delay follows the host's
  tempo** (`set_position` from CLAP's transport and VST3's `ProcessContext`) — both
  bit-identical to the DSP; the tail it reports (`tail_samples`) is infinite while infinite
  or frozen, else 1.5 × the decay plus the pre-delay (at the host's tempo when synced) and
  the bloom. The plug-ins' Direct path names Caelum's DSP with its output glide. Ids
  appended and blessed.
- **AI**: the effect words say what Caelum is for (intervals, infinite washes that build,
  freeze, bloom, ducking by sidechain, synced pre-delay; cathedrals, frozen pads, drones,
  guitars, vocal shimmers); the MCP tools golden regenerated on purpose.
- **A pedal on Tabula** (`pedal_allowed`): a reverb pedal is natural. Its face: decay,
  shimmer, first voice, mix (wide, the rings motif, "C"). A board takes a key when one of its
  pedals does, and passes it to its pedals, so a keyed board's key reaches Caelum's ducking
  exactly as an insert's; **with no key on the board, the ducking hears the pedal's own
  input** (it does not do nothing). The synced pre-delay hears the song through the board.

## Consequences

- A shimmer reverb whose voices and decay are set on the picture itself: the astrolabe and the
  rack are controls, not only displays.
- The two infinite choices (`ishim`, `ilevel`) stay at today's behaviour until the owner
  listens; *Infinite drone* already uses `ishim` 1, so the browser offers both.
- Caelum joins Tabula; `a_board_of_one_is_the_unit` covers it (keyed) with every other pedal.
- The exported tail is honest: a host that stops processing after the tail does not cut an
  infinite or frozen Caelum.

## What the tests hold

`cantus-engine/tests/fx_caelum.rs`: the null to the bit and latency 0; the starting preset
sounds, live = offline, blocks of 1/37/64/512; the ducking hears the sidechain (live =
offline); a synced pre-delay is its note at the tempo (117.2 ms a sixteenth at 128 BPM);
a fork carries on; the two choices reach the DSP. `fx_contract`,
`rt_memory::caelum_never_allocates` (shimmer, infinite and sustained, frozen, a redesign,
keyed and synced). DSP: `infinite_with_the_shimmer_fades_or_sustains_and_its_level_is_a_choice`
and the DSP's own 28.

Phase B: `cantus-ui/tests/caelum_e2e.rs` (through `CantusApp::assemble`): a chord struck once
on its track, Caelum inserted as the picker does (*Octave shimmer*, latency 0); **the octaves
heard** — the tail's energy above the chord's octave 2–3.5 s on is +2.4 dB against −3.3 dB
without the shimmer (+5.7 dB), and it climbs +3.3 dB from 0.6–1.4 s to 2–3.5 s; live in
blocks of 256/333/512/17/1024 equal to the render, to the bit; **infinite holds** +0.10 dB
from 1–2 s to 3–4 s (the same space finite −18.3 dB); **freeze holds**, thrown live a second
in, +0.17 dB from 1.2–2 s to 3–4 s (open −17.6 dB); **ducking keyed by another strip** −26.5
dB while the key plays, and before it plays +19.8 dB over the unkeyed tail (which its own
chord still ducks); an edit (the first voice a fifth) heard and undone to the bit; the
defaults the dry master to the bit, the key still routed;
`the_file_holds_the_dsp_s_factory_spaces_and_the_starting_preset_is_named`.
`caelum_drag.rs` (real pointer events): the alidade dragged from +12 to +19, the slimmer hand
from +7 to −5, each one undo step; a click by the meridian's −12 puts the first voice there;
the rack dragged to 12 s and to 2 s (one undo step each), no drag while infinite.
`fxa_whole.rs`: the sill's buttons; no word cut or clipped (EN/ES, 75–200 %, every colourway).
`cantus-engine/tests/fx_caelum.rs::as_a_pedal_its_ducking_hears_the_board_s_key` (the keyed
board = the keyed insert to the bit, live = offline; unkeyed it ducks on its own input);
`fx_contract::a_board_of_one_is_the_unit`. `cantus-plugins`: `clap.rs`
`caelum_s_ducking_is_keyed_by_the_sidechain_input_when_asked`, `vst3.rs`
`caelum_s_ducking_is_keyed_by_the_vst3_sidechain_bus` and
`caelum_s_synced_pre_delay_follows_the_vst3_hosts_tempo`, `transport.rs`
`caelum_s_synced_pre_delay_follows_the_hosts_tempo_through_clap`, and every generic test
(bit-identical to the DSP at any block size, automation on the grid, state, any sample rate).
