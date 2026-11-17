# The effects side round, wave A — `loquax`: Loquax, the vocoder (DSP phase)

Branch `fx/loquax` from `fx/integration` d3b51a09. Name provisional: `cantus_dsp::loquax::NAME`.
No `EffectKind` yet — this phase is the DSP alone (`crates/cantus-dsp/src/loquax*.rs`, one
`mod` line in `lib.rs` marked `// fx-a:loquax`), its measured tests and offline sounds. FL
Studio's Vocodex is the bar; Arturia Vocoder V, Waves OVox/Morphoder, the Roland VP-330 and
SVC-350 and the EMS Vocoder 2000 the character references — read about, nothing copied.

## Done

| File | What |
|---|---|
| `loquax.rs` | the unit: `LoquaxSettings`, `Source`, `Loquax::{new, set, reset, process, latency, at_rest, rest_blocker, skip, bands, telemetry}`, `BandView`, telemetry `t::*` |
| `loquax_run.rs` | the sample loop and the banks' tuning (cached by layout; retuned every 64 samples while a glide moves) |
| `loquax_bank.rs` | Butterworth band-pass design (order 4/6/8, centre exact, edges by Newton), the band layout with a 50 Hz floor, the structure-of-arrays bank of linear TPT state-variable sections, a two-pole SVF |
| `loquax_carrier.rs` | the polyphonic carrier: 16 voices + the robot's drone, saw/pulse/noise, 1–7 unison copies, polyBLEP, attack/release, held keys kept while disengaged |
| `loquax_voicing.rs` | the voiced/unvoiced detector, the sibilance high-pass (4th order) and the gate |
| `loquax_measure.rs` | the speech-like test phrase; NCM (Goldsworthy and Greenberg 2004, STI octave weights); spectral-envelope correlation; Goertzel response |
| `loquax_tests.rs`, `loquax_tests2.rs` | 22 tests (below) |
| `examples/loquax_sounds.rs` | the listening examples and the CPU measure |

## The DSP API (what the engine unit will call)

```rust
let mut q = Loquax::new(sample_rate);
q.set(&settings);                      // allocation-free, any time between blocks
q.process(&mut l, &mut r, side, notes); // side: Option<(&[f32], &[f32])>, notes: &[NoteEvent]
q.latency();                           // 0
q.at_rest(); q.rest_blocker(); q.skip(frames); q.reset();
q.telemetry(&mut [f32; 64]);           // the family's header + t::*
q.bands(&mut BandView);                // per band: centres, modulator/carrier/output dB
```

`NoteEvent { frame, kind: On { key, velocity } | Off { key } | AllOff }`, frames within the
block in order. Notes are always taken (held keys tracked at mix 0); a held key keeps the
unit from rest, so the engine must deliver note events to a resting unit (or wake it).

## Measured

- **Band response**: every band of 8/16/32/64-band banks, orders 4/6/8, at 44.1/48/96 kHz,
  is the analytic Butterworth band-pass within **0.0027 dB** down to −60 dB; centres 0 dB
  (±0.02); edges −3.01 dB within **0.0056 dB**. An octave above the edge: −32.3 / −48.5 /
  −64.7 dB (orders 4/6/8). Summed power of the bank at width 1: ripple 0.73 dB (16 bands),
  0.69 dB (32).
- **Intelligibility proxies** on the speech-like phrase (chord C3 G3 C4 E4): NCM 8 bands
  0.50, 16 → 0.74, 32 → 0.72, 64 → 0.66 (saturates at 16–32, as the vocoded-speech
  literature reports; at 64 a chord's sparse partials leave narrow bands to noise); a
  carrier shaped by noise instead of the voice: 0.00. Consonant help: the 8 kHz octave's
  envelope correlation 0.91 → 0.95. On the real voice (Gettysburg excerpt): 16-band robot
  NCM 0.72, 32 bands 0.71 (0.69 without the consonant help), formant ±5–6 st 0.50–0.54,
  choir 0.57; spectral-envelope r 0.95–0.98.
- **Formant shift**: centroid 466 / 731 / 1022 Hz at −7 / 0 / +7 st, pitch unchanged
  (110 Hz within 1 Hz). Level: the vocoded phrase −1.5 dB from the voice (flatten 1).
- **CPU** (release, one core, 48 kHz, speech + four-note chord with unison 3 — measured
  while another builder compiled on the machine): order 4 — 1.6 / 2.2 / 4.2 % of real time
  at 16 / 32 / 64 bands; order 6 — 2.0 / 2.8 / 4.7 %; order 8 — 2.1 / 2.9 / 4.5 %. Worst
  case, formant and range automated every block: 4.6 % (32 bands) / 8.5 % (64).
- **Latency** 0. **Rest**: 3.3 s into a silence from the busiest settings.

## Tests and what each holds

- `the_defaults_touch_no_sample` / `at_mix_zero_every_other_control_moved_is_still_a_null` —
  the null to the bit (−0.0 kept), with notes and a sidechain.
- `every_band_is_the_butterworth_band_pass_it_was_designed_as`, `the_slopes_are_their_order`,
  `the_bank_sums_flat_at_width_one`, `a_new_band_count_relayouts_…`.
- `block_size_does_not_change_a_sample` — 1/37/64/512 at tolerance 0, glides and retuning
  running, notes on, off and all-off mid-block, sidechain and synth both.
- `it_comes_to_rest_and_its_rest_is_a_new_unit` — the exact rest twin; `a_held_key_…`.
- `a_value_that_is_not_a_number_is_silence`.
- `the_output_follows_the_voice_and_more_bands_carry_more`, `the_consonants_come_through`,
  `the_detector_tells_a_vowel_from_a_sibilant`, `a_formant_shift_moves_the_voice_and_not_the_notes`,
  `a_band_shift_moves_a_band_to_its_neighbour`, `hold_keeps_the_vowel_after_the_voice_stops`,
  `the_robot_drones_its_note_while_the_voice_speaks`, `a_sidechain_carries_and_swap_trades_the_roles`,
  `the_vocoded_voice_is_about_as_loud_as_the_voice`, `spread_sends_alternate_bands_left_and_right`,
  `the_synthesiser_steals_the_oldest_voice_and_a_zero_velocity_releases`.

## Sounds

`C:\projects\daw-fx\compartir\loquax\sound\` — FLAC, README with sources, measures and what to
listen for. Voice: *Gettysburg Address* read by Britton Rea, Wikimedia Commons, public-domain
dedication by its author (not in the repository). Sung voice: our synthetic singer.

## Verified only in emulation / needs the owner

- The sound, by ear. No allocation on the audio path is by construction (stack arrays,
  no `Vec` in `process`/`set`); `rt_memory.rs` proves it once the engine unit exists.

## Left (later phases)

Core params (`EffectKind`, PARAMS keys incl. the 64 band levels), the engine unit
(`TAKES_SIDECHAIN` and `TAKES_NOTES`, note delivery to a resting unit), the null and contract
tests in `cantus-engine`, `rt_memory`, the panel, presets, export, docs, e2e — see the report.

## Design sheet v2 (the owner's verdict on v1: "more elaborate")

`C:\projects\daw-fx\compartir\loquax\design\v2\`. The display became a vocoder's deck of
objects, drawn in vectors from `kit::skin::consoles` (a new palette; no texture tile, no
layer-atlas work, so `kit_alloc` is untouched):

- `views/fx/loquax_art.rs` — the gooseneck microphone (chrome socket and knurled nut, the
  ribbed neck along a cubic, the satin head with its on-air lamp, the woven grille over the
  foam, its glow and the voice's arcs following the voice's level), the machined bezel (a
  finish per colourway: brushed aluminium, black lacquer with chrome lips, the road case's
  steel corners and rivets; screws), glass, pilot lamps, the needle meter.
- `views/fx/loquax_display.rs` — the microphone, the bank's phosphor screen in its bezel
  (scanlines, the lit cells' glow, level lines; the frequency marks kept inside the glass),
  the meter bridge (the voice's needle and the carrier's, dBFS −40 … 0, a needle's
  ballistics), the carrier's source, lamps for a consonant, the hold and the robot.
- `fxa_cells::pack`/`breaks` (additive) — the cards in the fewest rows that fit, the breaks
  evenly filled (Spanish no longer leaves Followers alone on a row); `breaks` unit-tested.
- The *wide* demo presses Hold mid-vowel (held from the first sample its bands kept silence).

## Phase B (3), to the finish (2026-11-17): v2 approved with two adjustments

- **The display keeps its size**: Spanish's cards take four rows, English's three
  (`loquax::card_rows`, held by `fxa_whole` to what the packing measures); the window grows
  a row instead (`loquax::designed_size`), the display the same size in both at 75–200 %.
- **Three objects**, chosen in the ⋯ menu (*Microphone*, then the colourways): the
  gooseneck (default), the talk-box (`loquax_objects.rs`: the driver's housing in the
  colourway's finish with the on-air lamp, footswitch and jack; the driver and nozzle; the
  clear tube carrying the voice's light to its clip on the stand; the glowing open end), the
  desk microphone (the weighted base in the finish, rubber foot, talk bar and on-air lamp;
  the chrome stem and lock; the yoke and pivot knobs; the slotted capsule lit by the voice).
  Kept **per insert** as the key `obj` — a look (`EffectKind::is_look`): never heard, one
  step of undo, presets leave it alone. Shots: `C:\projects\daw-fx\compartir\loquax\design\v3-objects\`.
- **Band levels drawn** on the bank by a stroke (one undo step).
- **Presets** fifteen, both carriers, fitted by `preset_tool fit` (−4.9 … +1.9 dB; Robot
  +0.5 dB with them); the audition gives a vocoder the source's line as notes and key.
- **Export** `Cantus Loquax`: notes (CLAP note port, VST3 event bus), the sidechain input;
  ids appended. **AI** words; the MCP golden regenerated. **Site** card (EFFECTS 35).
- **Docs**: ADR 0139 phase B, `docs/plugins/loquax.md`, the manual EN/ES.
- **Tabula**: Loquax stays off the board (no key, no notes on a board: only the robot).
- **DSP** (`loquax.rs`, one guard): a host's sample rate under 3.6 kHz (clap-validator's
  1 234.6 Hz) panicked in `clamp` (the bank's top above its Nyquist); the bank is now
  squeezed under it. Every rate from 3.6 kHz takes the old branch, bit for bit.

### Shared files touched (marked `fxa:loquax`)

Core: `effect.rs` (`is_look`), `starting.rs`, `fx/tabula.rs` (comment). UI:
`views/fx/frame.rs` (designed size; `Chrome::more_button`/`menu_rows` for the pointer tests;
`modified` skips looks), `views/fx/host.rs` and `views/fx/mod.rs` (presets skip looks),
`views/fx/controls.rs` (`segmented_rect`, a test hook), `kit/skin/consoles.rs` (the
objects' palette), `preset_audition.rs` (`vocoder_carriers`), `agent_units.rs`,
`examples/shoot_fxa.rs`, tests `fxa_whole.rs`, `preset_audit.rs`. AI: `tools_mixer.rs`.
Plug-ins: `Cargo.toml`, `build.rs`, `src/desc.rs`, `src/text.rs` (Loquax's locales, and
Fractus's and Caelum's so `pane_words` names every kind's choices), `src/state.rs` (the
`fractus` field the wave-A contract left out: the crate did not compile), tests `common/mod.rs`,
`clap.rs`, `vst3.rs`, `notes.rs`, `golden/ids.txt`. xtask: `main.rs`. Web: `units_data.rs`.

### Suites (Windows, 2026-11-17, one crate at a time, clippy `-D warnings --all-targets` on each)

cantus-core 806, cantus-dsp 1 050, cantus-engine 1 281, cantus-project 193, cantus-plugins 81,
cantus-ai 50, cantus-ui 2 704 + `learn_everywhere`'s one failure that is Fractus's (its stepped
keys drawn without their offer — the Fractus builder's; Loquax's `obj` excused as a look).
Clippy clean on all seven. Not run: the Linux check (WSL), the whole workspace at once, the
plug-ins without default features, `preset_render` in release.

### Verified only in emulation

- No voice was vocoded live on a monitor here; the e2e renders and the DSP's measures are
  the evidence. The exported plug-in in our own CLAP/VST3 harness only (notes and the
  sidechain bus), not in a third-party DAW.
- The objects judged by their shots; the owner has not seen them yet.

### Left

- Owner-only: the three objects' shots (`v3-objects`), listening to the fifteen presets.
- At the integration: the site's pictures (`web/assets/units/loquax-*.webp`) and preset
  excerpts; the README's unit grid (Loquax is not in it yet); renumbering ADR 0139.

## Follow-up (2026-11-17, the owner's answers)

- **(a) The talk-box redrawn as fine as the gooseneck and the desk microphone**
  (`loquax_objects.rs`): the tube translucent — the vinyl barely tinted, its inner wall seen
  through it as two fainter lines inside the edges, a bright highlight on the lit side broken
  where the tube turns from the light and the far wall's faint reflection, the voice's light
  a thin thread down the bore (it was a thick green curve); a chrome ferrule and a worm-drive
  hose clamp (its band's perforations, its housing and slotted screw) where the tube slides
  over the nozzle; a chrome mouthpiece at the open end, its lip, the vinyl's wall and the dark
  bore with the voice deep in it; the housing's face a punched grille in a recessed panel, a
  knurled level knob (36 ridges, chrome cap, pointer), the jack on its hex nut, the on-air
  lamp (the footswitch gave its place to the knob). Shots, the talk-box in every colourway
  (Aluminium, Ink, Road case), Slate EN and Paper ES, replaced one by one in
  `C:\projects\daw-fx\compartir\loquax\design\v3-objects\` (the gooseneck's and desk
  microphone's untouched).
- **(b) Presets set the object.** Each factory preset names the object it suggests (`obj` in
  `vocoder.toml`, written in the file's header): the **talk-box** for *Talk-box*, *French
  house robot*, *Talking drums*, *Talking bass* (the classic talk-box sounds: a guitar, a
  synth, a bass or drums shaped by a mouth); the **desk microphone** for *Whisper*, *Deep
  robot*, *Sixty-four bands* and *Radio* (spoken, broadcast and intercom voices); the
  **gooseneck** for the rest — *Robot*, the starting preset, so a fresh insert keeps the
  default look, and the keyboard vocoders (*Classic vocoder*, choirs, strings), whose
  instruments carried a gooseneck. Loading a preset sets `obj` in the same step (the host's
  `load`, `apply_fx_params`); a map that does not name it leaves it. The object stays a look:
  never heard, never a drift from a preset (`frame::modified`), and **A/B leaves it** (Cantus's
  `toggle_ab` and the plug-in editor's). A user's saved preset always writes the object it had
  (`save_user_preset`), and loading it sets it. `EffectKind::is_look`'s comment and ADR 0139
  say so; `loquax_e2e::presets_set_the_object_and_neither_the_drift_mark_nor_a_b_sees_it` holds
  it (no earlier test asserted that presets keep the user's object).
