# The effects side round, wave A — `caelum`: Caelum, the shimmer reverb (DSP phase)

Branch `fx/caelum` from `fx/integration` d3b51a09. **DSP only**: no `EffectKind`, no core
parameters, no engine unit, no panel yet. The name is provisional — one constant,
`cantus_dsp::caelum::NAME`. References for *function* (inspired, never copied): Valhalla
Shimmer and SuperMassive, Strymon BigSky's Shimmer/Cloud/Bloom, Eventide Blackhole,
Soundtoys SuperPlate. What Caelum adds to Cantus's reverbs (Spatium's rooms, Aula's
convolution, the legacy Reverb): pitch-shifted feedback, infinite washes, bloom, freeze,
modulated diffusion and ducking.

## Done

| Layer | Files (owned) | Shared files |
|---|---|---|
| DSP | `cantus-dsp/src/caelum.rs` (the unit, its settings, meter, rest), `caelum_input.rs` (pre-delay, bloom, diffusers, freeze gate), `caelum_tank.rs` (the FDN, absorption, infinite, freeze, hold, governor, the shimmer's exchange), `caelum_shift.rs` (the shifter), `caelum_parts.rs` (sine table, filters, all-pass), `caelum_presets.rs` (the starting preset and eleven factory spaces); tests `caelum_tests.rs`, `caelum_tests_measure.rs`, `cantus-dsp/tests/caelum_alloc.rs`; example `cantus-dsp/examples/caelum_sounds.rs` | `cantus-dsp/src/lib.rs`: one line, `pub mod caelum; // fx-a:caelum` |
| Docs | this file | — |

### The DSP API

```rust
use cantus_dsp::caelum::{Caelum, CaelumSettings, CaelumMeter, Lines, presets, sync_ms};

let mut c = Caelum::new(sample_rate);        // allocates here only
c.set(&CaelumSettings { ..presets::starting() }); // every snapshot; redesigns only on change
c.process(&mut left, &mut right, key);       // key: Option<(&[f32], &[f32])> — the ducking's
c.meter();                                   // CaelumMeter, refreshed every block
c.latency();                                 // 0
c.at_rest(); c.rest_blocker(); c.skip(n); c.reset();
c.design();                                  // the tank's TankDesign (achieved_t60(hz), …)
sync_ms(beats, bpm);                         // a synced pre-delay, ms
```

`CaelumSettings` (real units): `size` 0…1 (lines of 15 → 320 ms on average), `decay_s`
0.3…60, `infinite`, `predelay_ms` 0…1000, `diffusion` 0…1, `high_hz` + `high_decay`
(the highs' T60 as a fraction of the middle's), `low_hz` + `low_decay` (a multiple),
`mod_depth` 0…1 (to 4 ms) and `mod_rate_hz`, `shimmer` 0…1, `pitch_a` and `pitch_b`
(semitones, ±36), `pitch_b_level` 0…1, `shimmer_low_hz` / `shimmer_high_hz` (the tone in
the loop; ≤ 20 and ≥ 20 000 are off), `bloom_ms` 0…4000, `freeze`, `duck_db` (0 off),
`duck_thr_db`, `duck_att_ms`, `duck_rel_ms`, `low_cut_hz` / `high_cut_hz` (the wet's tone),
`width` 0…1.5, `lines` (`Eight`, `Sixteen`), `mix` (Spatium's law: dry at unity to 50 %).
**`Default` is the null** (mix 0); `presets::starting()` is what an insert should start
with (ADR 0071), `presets::factory()` the eleven spaces with names and one-line
descriptions.

`CaelumMeter`: wet peaks, input peak, ducking's reduction (dB), freeze 0…1, bloom 0…1, the
tank's energy (dB), the shimmer's share of it, the governor's trim (dB), pre-delay, mean
line length and modulation depth now (ms), keyed, and the achieved decay at eight octaves.

### How it works (each file's header says it in full)

```text
       ┌──────────────────────────── dry ────────────────────────────────────────┐
 in ─┬─ pre-delay ─ bloom taps ─ diffusers ─ freeze gate ─ tank ─ wet tone ─ width ─ duck ─ mix ─ out
     │                                          (8 or 16 lines, ↺ shimmer)       │
     └─ (or the sidechain key) ─ ducking detector ───────────────────────────────┘
```

- **The tank**: 8 or 16 delay lines (distinct primes spread over 1.2 octaves around the
  size's mean, ±4 % jitter), each read through an **all-pass interpolator** whose length a
  slow sine modulates (lossless, so the chorus costs the decay nothing), then **two
  Schroeder all-passes in the line** (3–12 ms and 1–4 ms, gain up to 0.7 with Diffusion —
  what makes a vast tank dense), then **Jot's absorption**: a middle gain and two
  first-order shelves (high damping, low damping), each line losing `60·L/(fs·T)` dB a pass
  in each band's own T, `L` its whole loop. The matrix is `P·H·D` (signs, Walsh–Hadamard,
  a permutation): orthogonal and not an involution. **Early taps** read each line a second
  time part of the way along into two more output rows, so a vast tank speaks before its
  shortest line has gone round.
- **The shimmer** (four channels: voice A on two orthonormal rows of the matrix, voice B on
  two more): each channel's component `s` of the lines is replaced by `√(1−a)·s + √a·z`,
  `z` the shifted `s` through the loop's tone. The shifter's **power normaliser** makes
  `z`'s power `s`'s, and `z` is uncorrelated with `s`, so **the exchange moves energy and
  makes none** — at any amount, at infinite decay. `z` also owes the decay of the time it
  spent in the shifter (its mean delay), so a shimmered tail decays at the set rate.
- **The shifter** (`caelum_shift.rs`, chosen over PSOLA and the phase vocoder — a tail has
  no pitch to lock grains to, and the vocoder costs four times the tank): three delay-line
  heads a third of a turn apart through Hann windows (their sum is exactly 1 — a coherent
  tone keeps its amplitude — and their squares' sum exactly ½ — a diffuse tail keeps a
  constant power, so no flutter either way), 120 ms grains, each grain started at its own
  offset (0–12 ms, drawn from the grain's number) so a pure tone's phase turns randomly at
  the hand-overs instead of steadily (a steady turn detunes it, up to 40 cents), and the
  normaliser (½…2, 150 ms). The phase is an integer count: a skipped shifter's heads are a
  processed one's to the bit.
- **Infinite** holds the middle (gain 1) and keeps the shelves as at 60 s, so damping still
  darkens; once nothing more comes in, a **hold** keeps the energy where it was (±0.5 % a
  pass). **Freeze** cross-fades each line to its raw output ×0.99999, mutes the shimmer,
  shuts the tank's door (the gate is after the pre-delay and the bloom, so a frozen tank
  takes nothing and what is on its way waits), and holds the level 150 ms after closing.
  **The governor**: above a wet of −6 dBFS RMS the loop gain comes down by the square root
  of the excess — never on a normal tail; what makes an infinite tank fed for minutes
  plateau.
- **Bloom**: 12 taps a side over the bloom time after the pre-delay, sparse first and dense
  at the end (`T·((k+1)/K)^0.6`), rising (`u^1.2`), normalised to one tap's energy, the
  sides' patterns apart; cross-faded with the plain input (equal power).
- **Diffusers**: four all-passes a side (20 ms in all), always running (a diffusion of 0 is
  four delays, so the onset never jumps), the two longest modulated (±0.6 ms).
- **Ducking**: a peak detector (50 ms fall) on the input or the key; the reduction it asks
  (the level over the threshold, up to the range) reached at the attack's speed and let go
  at the release's — the release is how fast the wet comes back.
- **The wet's level** follows `(T / 2 s)^¼` of the input's (a quarter of the dB its energy
  would; Spatium's half is too much for a minute-long wash), capped at 30 s; an infinite
  tank is set 8 dB under an 8 s one (it keeps all it is fed).
- **Exact rest**: once the input has been silent for as long as the way in reaches and
  the tank has written nothing above −120 dB for as long as its longest line, the tank, the
  diffusers and the shifters are **drained** (written +0.0 for one more line's length, the
  small states cleared) and the unit is at rest — a new unit, skipped. Infinite and frozen
  tanks never drain while they hold; an empty one is at rest.

## Measured (48 kHz; `cargo test -p cantus-dsp --lib caelum -- --nocapture`)

- **RT60 per octave against the design** (125 Hz – 8 kHz, four responses averaged, against
  `expected_band_t60` of the design's achieved curve), four tanks — small 1.5 s; large 4 s
  dark (4 kHz ×0.4, 200 Hz ×1.5); vast 10 s bright; 8 lines 3 s: **worst band 3.1 %**
  (ISO 3382's audible difference is 5 %). The design itself: every line's loss within 3 %
  of `60·L/(fs·T)` at both ends and 6 % in the middle (far from both corners).
- **Infinite, five minutes**: fed 4 s and left, within **+0.4 dB** of its first ten seconds
  after the input for the whole five minutes (a slow drift up of ~0.1 dB a minute —
  the hold keeps the lines' energy while the damping takes the highs; bound 1 dB).
  **Infinite + full shimmer** (both voices, octave and fifth): never above its first ten
  seconds; it climbs out through the loop's tone and fades (−27 dB in 10 s). **Fed noise
  for five minutes without a pause**: plateaus at −3.2 dB (L + R; the governor's ceiling
  −3.0), spread 0.08 dB over the last two minutes; with the shimmer, at −16 dB.
- **Freeze, five minutes**: within **−0.15…+0.06 dB** of the level where it froze; notes
  played into a frozen tank change its wet by exactly 0.
- **The shimmer makes no energy**: with the absorption flat and the loop's tone open, the
  broadband RT60 is 3.01 s without the shimmer, 3.01 / 3.00 / 2.99 s with it at 0.5, 1 and
  1 with both voices (before the shifter's delay was charged its decay: up to +16 %).
- **The shimmer's artefacts in the loop** (a 440 Hz tone, the tail 1 s later, power within
  ±30 cents of the partials the voices make against the rest, 60 Hz – 12 kHz): no shimmer
  96.6 % (the modulation's own spread); octave **95.2 %** (artefacts −13.0 dB under the
  partials, 50 % of the tail above 600 Hz — it climbs); octave and fifth 95.1 % (−12.9 dB);
  octave down **82.6 %** (−6.8 dB: a shift down hands over half as often and its sidebands
  land wider). The shifter alone on pure tones (220 Hz – 1 kHz, ±5, 7, 12 semitones): the
  centre within **13 cents** worst (was 40 before the drawn offsets), 54 % – 99 % of the
  power within 25 cents (the worst an octave down at 220 Hz). Noise through it: power
  within 0.1 dB, 20 ms windows within 0.65 dB (no flutter).
- **Echo density** (Abel & Huang ≥ 0.9, 20 ms windows, from the impulse): small 20 ms,
  large 120 ms, **vast 260 ms** (lines of 210–480 ms), vast without diffusion 980 ms;
  8 lines large 120 ms.
- **Bloom**: the wet's envelope after a 60 ms burst peaks at 120 ms with no bloom, and at
  640, 1 600 and 3 040 ms with 500, 1 500 and 3 000 ms; 100 ms in it is −2.7 dB under its
  peak without bloom, −64 dB / below −170 dB with it.
- **Ducking** 12 dB: −12.0 dB while the dry plays at −10 dBFS, −3.8 dB 0.4–0.6 s after it
  stops, −0.4 dB 1.2–1.8 s after (release 350 ms).
- **The wet's level** on noise against `(T/2)^¼`: within **0.10 dB** (sizes 0.3–0.9, 1–16 s).
- **Latency 0**: the dry at unity on the first sample; at a pre-delay of 0 the wet starts
  637 samples (13 ms) in — the earliest of the early taps.
- **Rest**: at rest after 6.0 s (Glass Plate), 7.2 s (Ducked Vocal), 10.1 s (8 lines),
  12.9 s (the starting preset), 18.0 s (Bloom Pad) of silence; the last sound before the
  drain −130 to −158 dBFS; the four twins (processing zeros, skipped, reset, new) agree to
  the bit on music afterwards.
- **No allocation after `new`**: a counting allocator over every factory preset and three
  more (frozen, 8 lines with the longest pre-delay and bloom, the defaults), blocks of 1, 37,
  64, 512, 1024, the key, `meter`, `at_rest`, `skip`, `reset`: 0.
- **CPU** (one core, 48 kHz, a minute of program in blocks of 256, best of three, dev
  profile with `cantus-dsp` at opt-level 3; one other builder's `rustc` running at the start,
  none at the end): **16 lines 3.10 %** of a core without the shimmer, 3.59 % with one
  voice, 3.98 % with two, 3.84 % with bloom and shimmer; **8 lines 1.59 %**, 2.01 % with
  the shimmer. Spatium (Room) in the same run 1.76 %.
- **The factory spaces**: all eleven stable on six seconds of loud program (the dry alone
  peaks at −4.4 dBFS; with each space −3.5 to +2.9 dBFS — to be fitted to level with the
  presets' file); each makes sound, each with a mix above 0.
- **The suite**: `cargo test -p cantus-dsp --no-fail-fast` — 948 passed in the library
  (Caelum's 28 among them, one ignored: the CPU measure), the integration tests and
  `caelum_alloc` green; `cargo clippy -p cantus-dsp --all-targets -- -D warnings` clean;
  `cargo fmt -p cantus-dsp` applied.

### Tests and what each holds

`caelum_tests.rs`: `mix_zero_is_a_null_to_the_bit` (the defaults, a heavy setting at mix 0,
infinite and frozen at mix 0 — the output is the input, bit for bit);
`the_output_does_not_depend_on_the_block_size` (1/37/64/512 at tolerance 0, four settings,
one keyed, a change of every kind mid-way — size, decay, shimmer, freeze, the line count);
`the_latency_is_zero_and_the_dry_is_not_delayed`; `a_synced_pre_delay_is_its_note_at_the_tempo`;
`it_comes_to_rest_exactly_and_its_twins_agree` (six settings); `a_new_unit_is_at_rest_even_infinite_or_frozen`;
`infinite_never_comes_to_rest_while_it_holds`; `freeze_holds_and_lets_nothing_in`;
`every_factory_space_is_stable_and_sounds`. `caelum_tests_measure.rs`: the measurements
above (`the_decay_follows_the_setting_in_every_band`, `infinite_does_not_grow_over_five_minutes`,
`freeze_holds_its_level_for_five_minutes`, `the_shimmer_moves_energy_and_makes_none`,
`the_shimmer_moves_a_tail_by_its_interval`, `the_tail_turns_dense_quickly`,
`bloom_is_a_slow_attack`, `ducking_steps_aside_and_comes_back`,
`the_wet_level_follows_the_decay`, `cpu_at_eight_and_sixteen_lines` — `#[ignore]`, run
alone). In the parts: the sine table, the phase increment, the all-pass's losslessness, the
matrices orthogonal and not involutions, the lengths distinct primes, every line's band
rates, infinite never gaining, the shifter's interval on tones, its power on noise, its skip.
`tests/caelum_alloc.rs`: no allocation.

## Shown to the owner

- `C:\projects\daw-fx\compartir\caelum\sound\` — the guitar (Carbonthief, CC0) through the
  octave shimmer, the fifth cathedral, the bloom pad and the freeze swell; a synthetic sung
  line through the ducked space, with and without ducking; a synthetic pad through the
  octave shimmer, the endless ascent and the sub-octave cloud; dry twins; README and the
  render log.

## Verified only in emulation / needs the owner

- Everything is DSP-only and measured; **the sound is for the owner's ears**: the shimmer's
  amount at the starting preset (it climbs clearly in tails, gently under playing), the
  octave down's roughness on low notes, the infinite level, the bloom's shape.

## What the later phases need

1. **Core**: an `EffectKind` (a new `ALL` index; the contract's), `PARAMS` mapping every
   `CaelumSettings` field (the decay knob's top position as Infinite, or a separate switch;
   the line count as a quality choice; the pre-delay's sync — note values to `sync_ms` with
   the tempo from `SongPosition`), the `out` key, schema bump and fixture, commands with
   undo and the parameter menu, the undo fuzz arm, the starting preset (`presets::starting`)
   in `starting.rs`.
2. **Engine unit** (`cantus-engine/src/fx/caelum.rs`): `FxUnit` over `Caelum` — `latency` 0,
   `at_rest`/`skip`/`reset` straight through, the sidechain for ducking
   (`takes_sidechain`), the meter to the editor's telemetry, tempo sync of the pre-delay;
   `tests/fx_caelum.rs` (the null), its rows in `fx_contract.rs` and `must_rest` (it rests
   at its defaults; heavy at infinite or frozen does not), `rt_memory.rs`, the bench kernel.
3. **A panel with its own personality** — fine strokes, a dark display in both themes,
   nothing abbreviated: the display is its centre — e.g. the tank's energy over time as a
   rising sky (the shimmer's octaves as layers climbing, bloom as the swell's curve, the
   freeze as a held horizon), the achieved decay per octave drawn and dragged (high and low
   damping as two handles), the voices' intervals on a small staff or ladder; the design
   sheet first (Slate and Paper, rest and playing, beside the references), the owner's
   approval before the full editor; English and Spanish at 75–200 %.
4. **Presets**: the eleven factory spaces as the file (`presets::factory()` is their
   source; a test should compare the file with it), ten or more, named, fitted to level.
5. **Export**: the CLAP/VST3 descriptor, bit-identical to the engine; golden ids appended.
6. **Docs**: an ADR (the design decisions above: the energy-exchanging shimmer, the
   three-head shifter with drawn offsets, the drain for exact rest, the hold and governor),
   `docs/plugins/caelum.md`, the manual EN/ES, the AI tools' description, the site's card.
7. **End to end**: insert it from the picker on a playing track through
   `CantusApp::assemble`, move a control, hear it in the master.

## Design sheet v2 (the owner's verdict on v1: "more elaborate")

`C:\projects\daw-fx\compartir\caelum\design\v2\`. The display became an observatory at night,
drawn in vectors from `kit::skin::observatory` (a new palette; no texture tile, no layer-atlas
work, so `kit_alloc` is untouched):

- `views/fx/caelum_art.rs` — the window's frame (four mitred rails brushed along their
  length, a raised bead, slotted screws, the chamfer to the glass), a metal per colourway
  (lacquered brass for Night, rose gold for Dawn, a bronze whose engraving holds verdigris
  for Aurora), engraved lines, the glass, the sill's plate, the lit pushbuttons in their
  bezels, the pilot lamps, the ∞ and crystal glyphs, the frost.
- `views/fx/caelum_sky.rs` — the sky: depth (darker toward the zenith, the horizon glowing in
  the tail's light), the Milky Way, three magnitudes of stars brightening with the tank and
  twinkling while it plays; the tail on the horizon over the tank's still water (mirrored,
  rippled, glints under the bright moments); the shimmer's octaves as bands a voice's
  interval apart (both voices), each its interval named in the gutter; wisps climbing from
  band to band where the energy passes; a dotted line where the light is on average, rising
  as the tail ages; the frame's rails engraved (a tick a second along the foot, a tick at
  each band); infinite writes ∞; frozen frosts the glass; the ducking lights a slot in the
  right rail and writes its figure.
- `views/fx/caelum_display.rs` — `View::climb`: how far the light has climbed, a model of the
  tank's exchange fed by its measured energy (what is new — the energy's rise beyond the
  decay the tank owes, while there is input and the tank is open — enters at the horizon;
  each step a share of every band climbs to the next at the shimmer's amount, the voices
  sharing it by the second's level; nothing climbs while frozen; an infinite tank that lets
  the shimmer fade loses what has climbed). The telemetry's shimmer share is what the
  exchange moves in a block (a few per cent, noisy), not how far the light has climbed, so it
  does not set the bands. Unit-tested (the light climbs as the tail ages; nothing climbs
  without shimmer or frozen; the bands are dark without energy).
- `views/fx/caelum_astro.rs` — the voices on an astrolabe: the limb's twelve semitones, the
  plate's pitch spiral (−36 … +36, every octave on the meridian), the rete's filigree, the
  alidade on the first voice, a hand on the second, each voice a jewel.
- `views/fx/caelum_desk.rs` — the sill: Endless's four settings as lit pushbuttons (Infinite
  with its ∞ lamp, Freeze with its crystal lamp, the two infinite choices as pairs), each a
  click (its menu offers automation and MIDI learn, the help line its meaning), and the input
  and ducking pilot lamps; the decay by octave as eight glass tubes in a rack, each lit to its
  achieved decay, its figure over its light, the set decay dotted across.
- `caelum.rs` — the cards in two rows (`fxa_cells::pack`): Endless left the cards for the
  sill; **Pre-delay sync** and the **tank's lines** (8 or 16), which had no control, joined
  Space. The demo of the frozen state freezes the tail a second after the chord; the ducked
  state's demo stops while the chord still sounds.
- Shared, additive: `Brush::ribbon` (a soft band of light, its colour and width free at every
  point). `fxa_whole` probes the frozen and ducked state with both voices, and clicks the
  sill's buttons through the app's editor (each click one undo step).
- Endless's defaults stay as they were (fades; 8 dB lower) until the owner listens.

## Left (this phase)

- Nothing in the DSP phase; the owner's listening, and the phases above.

## Phase B (3), to the finish (2026-11-17): v2 approved with two notes

- **The owner's v2 notes**: the astrolabe's octave names are led out from their dots by an
  engraved line to two columns 14 points either side of the meridian (they had crowded and
  touched); the sky's scale names are drawn after the frost, on a plate while frosted (the
  frost had hidden "+48 st"). Shots, every Caelum state (start at rest and playing, infinite,
  frozen, ducked; Slate EN and Paper ES; 75/150/200 %; Dawn and Aurora):
  `C:\projects\daw-fx\compartir\caelum\design\v3-step3\` (19 PNGs, no word cut).
- **The pointer**: the astrolabe's alidade sets the first voice and the slimmer hand the
  second (angle = the note in the octave, distance = the octave on the spiral; a press on a
  jewel takes that hand, else the nearer line; a click puts the nearer hand there); the decay
  rack's dotted line follows a drag (not while infinite). Each drag one undo step. The tubes
  stay a meter of the achieved decay (the damping's shelves shape them; a per-tube control
  would promise what the shelves cannot do). The sill's buttons as before (`fxa_whole`).
- **Presets** nineteen (`shimmer.toml`), the DSP's eleven first and held to it by a test;
  fitted −3.8 … 0 dB, the starting *Octave shimmer* −0.9 dB (in `starting.rs` and the demo);
  no preset starts frozen (*Freeze swell* asks for Freeze by hand); *Endless ascent* and
  *Infinite drone* build as they are played. Site card: EFFECTS 37, presets 19.
- **Export** `Cantus Caelum`: the Direct path (the DSP and its output glide), keyed ducking
  through CLAP's sidechain port and VST3's aux bus, the synced pre-delay on the host's tempo
  (CLAP transport, VST3 `ProcessContext`), its tail (infinite while infinite or frozen), ids
  appended and blessed.
- **AI** words; the MCP golden regenerated. **Docs**: ADR 0141 phase B, `docs/plugins/caelum.md`,
  the manual EN/ES.
- **Tabula**: Caelum is a pedal (decay, shimmer, first voice, mix on its face). A board takes
  a key when a pedal does and hands it to the pedal, so a keyed board ducks Caelum as an
  insert's key does (bit-exact); with no key the ducking hears the pedal's own input.

### Measured (the e2e, through `CantusApp::assemble`)

The octaves above the chord's own octave 2–3.5 s on: +2.4 dB with the shimmer, −3.3 dB
without (+5.7 dB), climbing +3.3 dB from early to late; infinite +0.10 dB over two seconds
(finite −18.3 dB); frozen live +0.17 dB (open −17.6 dB); keyed ducking −26.5 dB while the key
plays; live = offline to the bit; an edit undone to the bit; the defaults the dry master.

### Shared files touched (marked `fxa:caelum`)

Core: `starting.rs` (the fitted `out`), `fx/tabula.rs` (`pedal_allowed`). UI:
`views/fx/tabula_faces.rs` (Caelum's face), `examples/preset_tool.rs` (the header), tests
`preset_audit.rs`. AI: `tools_mixer.rs`; golden `mcp-tools-list.json`. Plug-ins:
`Cargo.toml`, `build.rs`, `src/desc.rs`, `src/instance.rs` (`tail_samples`), tests
`common/mod.rs`, `clap.rs`, `vst3.rs`, `transport.rs`, `golden/ids.txt`. xtask: `main.rs`.
Web: `units_data.rs`.

### Suites (Windows, 2026-11-17, one crate at a time, clippy `-D warnings --all-targets` on each)

cantus-core 808, cantus-dsp 1 050, cantus-engine 1 282 (+1, the board keyed),
cantus-project 193, cantus-plugins 89 (+4: keyed ducking CLAP and VST3, the synced pre-delay
CLAP and VST3), cantus-ai 51, cantus-ui 2 720 (the whole suite once; two reds fixed and their
targets re-run green: `clipfx::every_starting_preset_is_its_units_factory_preset` — Robot's
starting spec now names its object, `obj` 0, as its factory preset does; `preset_render`,
Arca's three leads CPU +0.22 measured while wave B compiled, green on its re-run with every
Caelum preset within its window and 0.055–0.063 of the budget's units). Clippy clean on all
seven; the MCP tools golden regenerated (`CANTUS_TOOLS_DUMP=1`, the Caelum words only). Not
run: the Linux check (WSL), the whole workspace at once, the plug-ins without default
features, `preset_render` in a quiet release build.

### Verified only in emulation

- No shimmer was heard live on a monitor here; the e2e renders, the engine driven block by
  block and the DSP's measures are the evidence. The plug-in in our own CLAP/VST3 harness
  only, not in a third-party DAW.
- The presets' sound is not yet heard by the owner; the two infinite defaults (Fades, 8 dB
  lower) still wait for his ears.

### Left

- Owner-only: the v3-step3 shots, listening to the nineteen presets and the infinite choices.
- At the integration: the site's pictures (`web/assets/units/caelum-*.webp`, big 1800 × 1209,
  thumb 640 × 430) and preset excerpts; the README's unit grid; renumbering ADR 0141.

## Excellence (2026-11-17): Infinite reworked — the owner's decision

Branch `fx/excel` from `fx/integration` 0e6b2722. The owner decided before listening: Infinite
must not change the level when engaged, must layer new playing under a soft ceiling, must
hold its tone for minutes, and its fade must be one continuous control. ADR 0141's amendment
has the design; this is the record.

### What changed

| Layer | Files |
|---|---|
| DSP | `caelum.rs` (`CaelumSettings::sustain` for `infinite_sustain`/`infinite_full`; the wet's level the set decay's, infinite or not), `caelum_tank.rs` (the output's absorption `TankDesign::out_*`; the loop's own low shelf `ls2`; the engage's lift from the write history `wbins`; the early taps' `early_k`; the soft ceiling `p_in`/`a_flux`/`a_loss`/`ceil_e`/`leak`; the settle `held` — climb, shelves, modulation; the settling's make-up `drop`), `caelum_presets.rs` (*Endless ascent* at Sustain 60 %), tests `caelum_tests_infinite.rs` (new), `caelum_tests.rs`, `caelum_tests_measure.rs` |
| Core | `fx/caelum.rs`: `isus` (Sustain, 0–100 %, default 100 %) for `ishim`/`ilevel`; `RETIRED`, `repair_spec`, `repair_project` |
| Project | `format.rs`: the load repairs a Caelum holding the retired keys (no schema bump: 64 is unreleased); the v64 fixture opens as Sustain 0 % |
| Engine | `fx/caelum.rs` (`sustain` from `isus`); tests `fx_caelum.rs`, `rt_memory.rs` |
| UI | `caelum_desk.rs` (the sill: Infinite, Freeze, the **Sustain gauge** — a glass tube lit to the value, a brass rider, quarter marks, dimmed while Infinite is off; click/drag one undo step, double click 100 %, the menu's automation/modulation/MIDI learn), `caelum_display.rs` (the sky's climb model follows the sustain), `caelum.rs` (`sustain_gauge`), locales EN/ES (`fx.caelum.isus` *Sustain* / *Sostenido*, the help lines), `agent_units.rs`, `presets/effects/shimmer.toml`, `examples/shoot_fxa.rs` (a `sustain` state), tests `fxa_whole.rs`, `caelum_drag.rs`, `caelum_e2e.rs` |
| Plug-ins | `tests/golden/ids.txt`: `caelum ishim`/`ilevel` removed (never released), `caelum isus` appended and blessed |
| Docs | ADR 0141 (amendment), this file, `docs/plugins/caelum.md`, the manual EN/ES, `web/src/units_data.rs` |

### Before and after (48 kHz)

| | Before (`0e6b2722`, defaults *Fades*, *8 dB lower*) | After |
|---|---|---|
| Engaged on a dying tail, held against the level at the switch | −5.2 … −11.2 dB (the 400 ms after: −0.1 … −9.4) | −0.30 … +0.36 dB (vast at 1.5 s +1.52) |
| Engaged while playing, each second against the finite twin | −4.1 … −5.2 dB the first second | −0.18 … +0.15 dB |
| Held five minutes after the input (3 s windows) | *Sustains* −0.43 … +3.37 dB; *Fades* gone | +0.02 … +0.21 (plain), −0.00 … +0.11 dB (both voices, full) |
| Held five minutes while playing (10 s windows) | — | +0.04 … +0.15 dB |
| Each octave 125 Hz … 8 kHz over five minutes | *Sustains*: 8 kHz −54 dB, 125 Hz +11 dB | within 0.49 dB (ten minutes 0.50) |
| Sustain 0 % (the old *Fades*) | −27.3 dB at 10 s, −62.2 at 30 s | −28.7 dB at 10 s, −104 at 30 s |
| Noise for five minutes, against the same tank finite | +4.7 dB (−3.29 against −8.00; the governor's) | −7.95 against −7.98 (plain), −10.00 against −10.04 (both voices) |

CPU (one core, a minute of program in blocks of 256, best of three; the machine shared with
other builders, Spatium's reference moving 2.49–2.74 % between runs): before 16 lines one voice
4.65 %, no shimmer 3.50 %, 8 lines no shimmer 1.82 %, infinite one voice 4.13 %; after 4.66 %,
4.30 %, 2.19 %, infinite 5.27–6.69 % (two runs). The finite path adds a few operations a sample
(the ceiling's measures kept ready, the write history); infinite adds a second absorption and
the ceiling — roughly a quarter more than finite. To be measured again on a quiet machine.

### Choices

- **`ilevel` retired**: the level is continuous at the engage and the soft ceiling bounds a
  wash built by playing — the old "8 dB lower" answered a problem the amendment removes. A
  user wanting the wash quieter turns the mix down.
- **Sustain's curve**: `held = 1 − (1 − s)²`, so the top of the control reaches far (90 % fades
  0.3 dB a minute) and the bottom is the old *Fades*.
- **The held wash's modulation is stilled** at Sustain 100 % (as it settles): with the chorus
  running for minutes, a held tone's energy smeared into its neighbours (+1 … +3 dB in the
  quiet octaves over five minutes at a quarter of the depth).
- **The ceiling is measured**, not computed from the design: the shimmer's climb and the
  shelves lose as much as the middle does on bright material, so a formula from the middle's
  gain alone sat 1–2 dB high; the measure (the finite loop's loss on what the lines hold, the
  input's power, 3 s averages) puts the plateau within 0.05 dB of the finite tank's.
- **Tonal material**: two held tones beat against their own modes in a tank that loses
  nothing, so an octave holding them wavers by about a decibel from one stretch to the next,
  and the ceiling, riding its measure's peaks, sits +0.3 … +1.1 dB over the finite tail when
  the playing is a steady chord (noise: +0.04 … +0.15). Measured and bounded (1.5 dB), not
  hidden.

- **"Something comes in"**, for Infinite, is sound that matters against the wash: input a
  hundred times under what would keep a finite tail at the wash's level (a synthesiser's
  release, a track's noise floor) counts as nothing, so the wash settles and holds.

### Carried through

- **Presets** re-fitted (`preset_tool fit shimmer`): *Freeze swell* −0.8 → −1.8 dB, *Endless
  ascent* −1.4 → −2.0 (and its Sustain 60 %), *Infinite drone* −1.9 → −2.5 (its `ishim` gone:
  Sustain 100 % is the default); the other sixteen within their window, untouched.
- **The e2e** (`caelum_e2e.rs`, through `CantusApp::assemble`): Infinite thrown live on the
  chord's dying tail with the shimmer on — the 400 ms after the switch **+0.44 dB** against the
  tail's level at it, each second after within **0.43 dB** of the first (finite −18.5 dB); from
  the start with both voices full, Sustain 0 % −1.84 dB from 2–3 s to 3–4 s against 100 %'s
  +0.69 dB; then twenty seconds of silence before Freeze is thrown on a fresh tail (frozen
  −0.06 dB). `caelum_drag.rs`: the Sustain rider dragged from 100 % to 50 %, one undo step.
  `fxa_whole.rs`: the sill's two buttons, the gauge clicked to 25 % and undone; the sill
  states shot without a word cut.
- **Shots** (`shoot_fxa`, a `sustain` state added): `C:\projects\daw-fx\compartir\caelum\design\v4-sustain\`
  — 21 PNGs (every Caelum state, Slate EN and Paper ES, 75–200 %, Dawn and Aurora); no word
  cut. The gauge: "SUSTAIN"/"SOSTENIDO" and the figure engraved over a glass tube lit to the
  value, the brass rider, quarter marks; dimmed while Infinite is off.
- **Sounds** (`C:\projects\daw-fx\compartir\caelum\sound\`, rendered to a temporary folder
  first): re-rendered `04-guitar-freeze-swell` and `07-pad-endless-ascent` (both infinite);
  new `09-pad-infinite-thrown-sustain-100` and `10-pad-infinite-thrown-sustain-40` (Infinite
  thrown on the Octave shimmer's tail: nothing jumps; held with its tone, or climbing away).
  The finite ones are bit-identical to before (01 and 06 compared byte for byte) and left as
  they were. README and render log updated.
- **`DSP_VERSION`** unchanged: no golden holds Caelum, and the finite path is bit-exact (the
  output's absorption is the loop's when finite; the early taps' scale is 1).

### Open

- **A short chord into an infinite tank at the climb's fullest** (both voices at 100 %), Sustain
  100 %: the held wash creeps up about 0.4 dB a second over its first seconds (the e2e
  measures +0.41 then +0.69 dB) before holding; with noise (the DSP's five-minute tests) it
  holds within 0.11 dB from 1.5 s on. The hold's reference is taken from an energy smoothed
  over 100 ms, which lags a tail still moving when the sound stops; two fixes tried here (the
  reference from the write history; a make-up only below the reference) each broke another
  case and were taken back. Bounded in the e2e at 1 dB.

### Verified only in emulation

- Nothing heard on a monitor here: the DSP's measures, the engine block by block and the e2e
  render are the evidence. The owner's ears are the test still to come (the re-rendered
  sounds above).
