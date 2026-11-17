# Caelum — shimmer reverb

*Effect kind `Shimmer`. References for function: Valhalla Shimmer and SuperMassive, Strymon
BigSky's Shimmer, Cloud and Bloom, Eventide Blackhole, Soundtoys SuperPlate — read about and
listened to, nothing copied (ADR 0141, provisional number).*

*Caelum* is Latin for the sky. A reverb for spaces no building holds: a tank of 16 (or 8)
modulated delay lines whose loop carries two **pitch-shifted voices**, so a tail climbs an
octave (or a fifth, or any interval from −36 to +36 semitones) each time round; **Infinite**
holds the tail for good, **Freeze** holds what is in the tank and lets nothing in, **Bloom**
swells the sound into the space, **ducking** steps the space aside while the dry plays (or
while another strip does, through the sidechain), and the pre-delay can follow the song's
tempo.

The shimmer **moves energy and makes none**: each pass replaces a share of the lines with the
same share shifted, power-matched and uncorrelated with what it replaces, so the decay you
set is the decay you hear, shimmer or not, even at infinite (measured: 3.01 s without, 2.99
to 3.01 s with). **The null**: the mix starts at 0 — the signal untouched, to the bit. No
latency.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/caelum.rs` (32 keys) |
| DSP | `crates/cantus-dsp/src/caelum*.rs` (the unit, the way in, the tank, the shifter, the parts, the factory spaces); tests `caelum_tests.rs`, `caelum_tests_measure.rs`, `tests/caelum_alloc.rs` |
| Engine glue | `crates/cantus-engine/src/fx/caelum.rs` (settings, the synced pre-delay at the song's tempo, `out` after the DSP, telemetry) |
| Editor | `crates/cantus-ui/src/views/fx/caelum.rs` (the cards), `caelum_display.rs` (the layout, the climb), `caelum_sky.rs` (the sky), `caelum_desk.rs` (the sill, the decay rack), `caelum_astro.rs` (the astrolabe), `caelum_art.rs` (the frame, the metals, the frost) |
| Face | `crates/cantus-ui/src/kit/skin/side_fxa.rs` (`CAELUM`; Night, Dawn and Aurora), `kit/skin/observatory.rs` |
| Strings | `crates/cantus-ui/locales/fx/caelum.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/shimmer.toml` (nineteen); starts on *Octave shimmer* |
| Tests | `crates/cantus-engine/tests/fx_caelum.rs`, `rt_memory`, `fx_contract`; `crates/cantus-ui/tests/caelum_e2e.rs`, `caelum_drag.rs`, `fxa_whole.rs`; the CLAP/VST3 suites (`clap.rs`, `vst3.rs`: the keyed ducking; `transport.rs`, `vst3.rs`: the synced pre-delay) |
| Exported | `Cantus Caelum` (CLAP and VST3), its ducking keyed by the sidechain input, its pre-delay on the host's tempo |

## The window

An observatory at night, dark in every theme, its metal following the colourway (lacquered
brass, rose gold, a bronze whose engraving holds verdigris):

- **The sky** in a framed window: the tail glowing on the horizon over the tank's still
  water; the shimmer's octaves as bands a voice's interval apart, each lit as the energy
  reaches it, so the higher bands appear later as the tail ages; wisps climbing from band to
  band; a dotted line where the light is on average. Infinite writes ∞; frozen frosts the
  glass (the scale's names stay over the frost); the ducking lights a slot in the right rail.
- **The sill**: Infinite and Freeze as lit pushbuttons, and the two choices an infinite tank
  makes (its shimmer fades or sustains as it climbs; its level 8 dB lower or as loud) as
  pairs. A click throws one, one step of undo; the menu offers automation and MIDI learn.
- **The astrolabe**: the voices on a pitch spiral (−36 … +36, every octave on the meridian).
  **Drag the alidade to set the first voice, the slimmer hand to set the second**: the angle
  is the note in the octave, the distance from the centre the octave. A click puts the
  nearer hand where it falls. One drag, one step of undo.
- **The decay by octave**: eight glass tubes, each lit to the decay the tank achieves at its
  octave; the dotted line across them is the set decay — **drag it up or down**. While
  infinite the rack shows ∞ and takes no drag. The damping stays on its card: a tube shows
  what the whole tank does at its octave, not a control of its own.
- **The cards**: Space (size, decay, pre-delay and its sync, diffusion, the tank's lines),
  Damping, Shimmer (amount, the two voices, the second's level), Shimmer tone (the loop's
  low and high cut), Motion (modulation, bloom), Ducking, Output.

## Presets

Nineteen, fitted to the dry's loudness (`preset_tool fit`, −3.8 … 0 dB): the eleven factory
spaces the DSP was measured with (Octave shimmer, Fifth cathedral, Bloom pad, Freeze swell,
Ducked vocal shimmer, Sub-octave cloud, Endless ascent, Glass plate, Fourth rise, Black sky,
Light shimmer — a test holds them to `cantus_dsp::caelum::presets::factory()`), then Fifth
shimmer, Twin octaves, Infinite drone, Guitar ambience, Eighth-note echoes and Dotted rise
(the pre-delay synced), Shimmer snare and Bus haze. **No preset starts frozen**: a tank frozen
from its first sample holds silence. *Freeze swell* is infinite and swells in; throw Freeze
on a chord to hold it. The infinite presets build as you play.

## Exported

`Cantus Caelum` in CLAP and VST3. Its ducking listens to the sidechain input once the
plug-in's own *Sidechain* switch is on *Sidechain input* (`cantus.sidechain`, as on every
exported unit), to its own input otherwise. A synced pre-delay (`psync`) follows the host's
tempo (CLAP's transport, VST3's `ProcessContext`), bit-identical to the DSP at that pre-delay.
The tail it reports is infinite while infinite or frozen, else 1.5 × the decay plus the
pre-delay and the bloom. Its ids are blessed in `crates/cantus-plugins/tests/golden/ids.txt`.

## A pedal on Tabula

Caelum may be a pedal (a reverb pedal is natural): its face shows the decay, the shimmer's
amount, the first voice and the mix; the rest is in its editor, opened from the pedal. A
board takes a key when one of its pedals does, so a keyed board's key reaches Caelum's
ducking exactly as an insert's (`fx_caelum::as_a_pedal_its_ducking_hears_the_board_s_key`);
with no key, the pedal's ducking hears its own input. Its synced pre-delay hears the song
through the board.

## Ours, better

- A shimmer that makes no energy: the decay is the setting's, in every band within 3.1 %.
- Two voices at any interval, a three-head shifter whose drawn offsets keep a tone within
  13 cents and a diffuse tail free of flutter.
- Infinite held five minutes within 0.4 dB, Freeze within 0.15 dB; a governor that keeps an
  infinite tank fed for minutes from running away.
- The voices dragged on an astrolabe, the decay dragged on its tubes; exact rest when the
  tail has died (the tank drained), so a silent Caelum costs nothing.
