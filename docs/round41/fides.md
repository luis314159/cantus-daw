# Round 41 — `fides`: Fides, the resonator (ADR 0150)

*The `fides` builder's report. Branch `r41/fides` from `r41/contract`.*

## What was built

**Fides** (`InstrumentKind::Resonator`): an exciter into one or two resonators, coupled, through
a body — the class of Collision, Tension, Sculpture and Chromaphone. The design is ADR 0150;
the player's view is `docs/manual/en/fides.md` (and `es`).

- **Six exciters**: pluck, pick, bow (a friction curve: stick and slip), mallet (Lamina's
  power-law spring pulse), noise (continuous or grains, with its envelope; also a layer under
  any other), live input (the DSP and the unit's `process_input`; the processor's routing is
  the integrator's — see *Left*).
- **Twelve resonators**: a waveguide string, a closed tube and an open pipe (two loops meeting
  at the exciter, Gravis's public tuning maths); a bar, a marimba bar, a tine, a plate, a
  membrane, a drumhead, a bell, a glass, a steel-pan note (banks of up to 48 modes, Fides's own
  cited tables). Two of them, in parallel or A into B.
- **Seven bodies** (none, guitar, violin, cello, harp, box, shell): banks of measured low modes
  and a cloud above, two microphones, continuous in size and ring.
- **MPE**: pressure on the bow's pressure, slide on where it is played, tuning per note.
- **25 factory presets** (the floor 24): the eight named — marimba, vibraphone, kalimba, steel
  pan, harp, nylon guitar, bowed cello, struck glass — and glass harmonica, xylophone,
  glockenspiel, celesta, music box, bell, timpani, frame drum, hammered dulcimer, banjo,
  pizzicato, bowed strings pad, bowed vibraphone, pan flute, rain on glass, gong, crystal pluck.
- **The editor**: Play (the instrument drawn as a plan, the display the control — drag where
  it is played — the partials lit by what sounds; the exciter, resonator, body and output
  cards), Edit (every part as a card), Matrix (the frame's); its own face
  (`kit/skin/fides.rs`: rosewood, gold, a plan's paper).

## How it is tested

- **DSP** (`cantus-dsp/src/fides_tests.rs`, 7 + 2 ignored probes): a new instance silent and at
  rest; every exciter on every resonator (B on and off, serial and parallel, every body,
  vibrato, tremolo, cuts) finite and bounded and sounding; renders repeat to the bit and are the
  same at blocks of 1, 37, 64, 512 and ragged splits; the voices on a pool that runs its jobs
  backwards are the serial bits; the pitch measure finds a sine to 0.001 cent; **pitch within
  0.1 cent at every bend** (string, stiff string, tube, pipe, marimba, tine, pan; E1 to A6; bends
  of 0, ±2, +0.37 st and an MPE tuning of +1.61 st); a bowed string's pitch measured.
- **Engine** (`cantus-engine/tests/inst_fides.rs`, 7): six shapes (the default, a bowed string
  with vibrato, a mallet on a marimba bar into its tube, noise into a tube with cuts and
  tremolo, two voices stolen on a plate and a bell, a macro on the body's size and velocity on
  the decay) through the engine's render at blocks of 1, 37, 64, 512, 4096 and twice, to the
  bit; the extras at their defaults change no sample (B off whatever B says, the cuts at their
  ends, tremolo and vibrato at depth 0, a route at depth 0); MPE pressure presses the bow and
  slide moves it, and another note's expression touches nothing; every shape dies to exact
  silence, comes to rest, and is then as good as new (a fresh unit skipped as far renders the
  next note's bits); an eight-note chord at velocity 127 keeps its headroom (the default −1.3
  dBFS, bowed −5.2); pool = serial through the unit; 32 heavy voices well inside real time.
- **Model** (`cantus-core/src/inst/fides.rs`, 2): the index table follows the list, A's and
  B's keys match, the choices' counts; the extras default to neutral.
- **Undo** (`cantus-core/tests/undo_invariants.rs`, `fides_edits_undo_and_redo_among_the_others`):
  a Fides inserted, every key set and cleared, slot texts set, among the other commands; undo
  all and redo all restore the song.
- **End to end** (`cantus-ui/tests/fides_pick_e2e.rs`, written, runs once Fides is in
  `InstrumentKind::ALL`): listed in both languages on the Keys shelf; chosen through
  `CantusApp::assemble` and `apply_pick`; its model stored; a phrase plays through the real
  engine wiring and is heard; the marimba preset applied as the browser applies it plays.

## Measured

- **Pitch**: struck and plucked, worst 0.0014 cent over the set above (the measure's own floor
  at 41 Hz is 0.001). **Bowed**: the stick and slip move it a few cents from the loop, as a real
  bow does — at the default pressure +3.1 (C2), +2.5 (C3), +1.2 (C4), +0.2 (C5), −1.2 cents (C6);
  softer, C2 −9.0. A correcting "ear" (a period tracker moving the loop) was tried and removed:
  it held C4–C6 within 0.5–0.7 cent but put C2 13 cents out. Bowed tuning within 0.1 cent is
  open (*Left*).
- **CPU** (a debug-optimised build on a loaded machine, three other builds running — not a
  claim): 32 voices of a 48-partial bowed plate with a bell in parallel and the noise layer,
  21.7–37.1 % of real time on one thread (`thirty_two_heavy_voices…`). The bench's
  `inst.fides.v32` is measured when Fides is in `ALL` (the integrator, quiet).
- **Levels**: every exciter on every resonator at C4, velocity 100, brought to about −12 dBFS
  peak by per-kind trims (`examples/fides_sounds -- levels`); the presets at their roles'
  targets (`examples/fides_listen -- fit`): 21 of 25 within 0.2 LU; Timpani −4.3, Frame drum
  −2.7 and Glass harmonica −2.7 LU under, held there by their crest (the fit stops at a phrase
  peak of −1.2 dBFS).

## Shared files touched (each line marked `r41:fides`)

- `crates/cantus-dsp/src/lib.rs` — `pub mod fides;` (the contract declared none for the DSP).
- `crates/cantus-core/src/inst/mod.rs` — `automatable`'s arm for Resonator (`fides::SET_KEYS`:
  `model`, `poly`).
- `crates/cantus-ui/src/kit/skin/instruments.rs` — Fides's sheet (`#[path] mod fides`, `ALL`
  12 sheets, `of()` → `FIDES` instead of Gravis's).
- `crates/cantus-core/tests/undo_invariants.rs` — one test at the end
  (`fides_edits_undo_and_redo_among_the_others`).
- `docs/beta-checklist.md` (a *Ronda 41 — Fides* section), `docs/BACKLOG-excellence.md`
  (appended: what Fides's first round leaves).
- Nothing of Gravis's, Chorda's, Lamina's, Pulsus's or Tympanum's files changed: Fides calls
  `gravis::string`'s public tuning functions and `lamina::voice::HammerShape` as they are, so no
  golden hash of theirs can move; `DSP_VERSION` stays 10.

## Verified only in emulation

- **The sound**: every preset was judged by measurement only — levels (LUFS against the role's
  target), peaks, partial amplitudes, decays, pitch. Nobody has listened. The owner's ear is
  the judge (the files are in `F:\projects\daw-r41\fides-listen\`).
- **MPE**: pressure, slide and tuning tested with expressions written by the tests, never from
  a controller.
- **The look**: the Play view shot on the CPU renderer in Slate Lapis Night and Paper
  (`docs/round41/fides/play-*.png`); not seen on a real screen or at other scales.

## What the owner should listen to and look at

- `F:\projects\daw-r41\fides-listen\NN-<preset>.wav` — each preset on the phrase the preset
  browser auditions it on; `NN-<preset>-notes.wav` — C2, C3, C4, C5, C6 held 1.5 s each. The
  eight named first: **01 Marimba, 02 Vibraphone, 03 Kalimba, 04 Steel pan, 05 Harp, 06 Nylon
  guitar, 07 Bowed cello, 08 Struck glass**. For each: is it *convincing in solo*? What is
  wrong — the attack, the decay, the lows, the highs, the body?
- `docs/round41/fides/play-dark.png` and `play-light.png` — the Play view (the Marimba preset):
  is the picture of the instrument (the bar, its tube under it, the mallet, the partials) the
  face you want, or should a design sheet come first?

## Left

- **`Resonator` in `InstrumentKind::ALL` and the picker** — one commit, after the owner's ear
  on the eight presets (the brief's order): `READY`, `ALL`, the picker row (Keys + Other
  instruments), the agents' `INSTRUMENTS` row and the MCP golden, `inst_contract`'s
  `per_kind!` and count, the learn/offers tests' explanations if any, the null corpus's new
  item and the bench's `inst.fides.v32` measured, `fides_pick_e2e` run. It needs the UI suite,
  which this machine could not hold this session (memory).
- **The live input**: the DSP and `Unit::process_input` are done; the processor does not yet
  hand an instrument a track's audio (the integrator's `processor*.rs`). Until it does, the
  *Input* exciter is silent.
- **A bowed string's tuning** within 0.1 cent (today a few cents, as the physics gives;
  *Measured*).
- The web card (`web/src/units_data.rs`) is the integrator's.

## Review fixes (integration)

*Branch `r41/fix-fides` from `r41/integration` 4f1b74b9e: the integrated review's six findings
on Fides. Each test below was written first and failed on the bug.*

1. **A slow position never moved a modal object.** `Res::step` placed the strike and
   microphone weights again only when the position moved more than 0.002 *since the last
   step*, and the last step's position was stored every step — so an MPE slide (about
   0.0016 a step) or the Position macro (about 0.001 a CC step) never moved a bowed bar,
   plate, glass or pan. It now compares with where the weights were **last placed**
   (`Res::placed`). Test: `a_slow_position_moves_a_bowed_modal_object` (a 0.25 ramp over
   two seconds, 0.00008 a step, on a bowed marimba bar, plate and glass).
2. **The noise generator was frozen at the note's start.** Its band was set once and the
   layer's envelope started only when `noise.level > 0` at the start, so a route or a knob
   on `noise.color`/`noise.width` did nothing mid-note (the blown presets' Macro 1,
   *Colour*), and a route to `noise.level` on a patch whose level is 0 stayed silent.
   `Exciter::follow` now runs on every control step after the first: the band's
   coefficients follow the step (the state runs on — a direct-form filter retuned every 32
   samples, as the tone's low-pass is; no allocation). **The envelope decided:** a layer
   that was not playing comes in when its level rises above 0 while the key is held, on
   its own envelope from the attack (as a layer that starts with the note would); once it
   plays, its level only scales it; after the key is let go nothing starts. The first
   step's modulated level counts too (a velocity route that raises it from the start).
   Test: `the_noise_band_and_level_follow_the_note` (the band moved up an order of
   magnitude in brightness with no sample past twice the steady peak where it moved; a
   level raised mid-note heard from its attack; nothing after the key is up; through the
   instrument, Macro 1 on the colour and on the level of a level-0 patch both heard).
3. **A stage out of use froze its state.** `Body::process` returned before its stillness
   check with no body or a mix of 0, and `Output::process` with both cuts off, so `still`
   stayed false, `Fides::at_rest()` never came back, and the frozen state would ring when
   the stage returned. A body out of use now lets go of its state (and is still); a cut
   switched off clears its filters in `Output::set`, and with both off the stage is still.
   Test: `a_stage_taken_out_mid_ring_comes_to_rest` (body none, mix 0, both cuts off, the
   low cut alone off — each mid-ring: the instrument comes to rest, and the stage put back
   sounds nothing from nothing).
4. **B switched on mid-note was rendered without being started** (another note's kind and
   state, against its own comment). A voice now keeps `b_struck`: B sounds only when it was
   on at the note's start and has stayed on. Switched on mid-note it **waits for the next
   note** and A keeps its level meanwhile; switched off mid-note it is cleared, so it
   cannot come back frozen. Test: `resonator_b_switched_on_mid_note_waits_for_the_next_note`
   (until the next note, the render is the B-off render to the bit, both for on-mid-note
   and for on, off, on again; the next note has B).
5. **A checkpoint's image counted Fides at the contract's 2 KB.** A fork of the real unit,
   counted by the allocator (`cantus-engine/tests/fides_image.rs`): **1 347 KB at 48 kHz**,
   1 265 at 44.1, 2 360 at 96 — the 36 voices' waveguide lines (two a voice, sized for
   20 Hz) grow with the rate; `approx_bytes` scales every unit by the rate's ratio, which
   holds Fides from above. The other figures came from the working-set probe
   (`listen::tests::what_each_unit_adds_to_an_image`), which this loaded machine could not
   read (its deltas went negative: a freed image's pages reused); counted the same way
   beside their figures, a small unit's working set runs 300–700 KB over its own bytes and
   a large one's is level (Chorda 5 621 counted, 5 150 figure), so Fides takes **1 400 KB**
   and the test holds it within 10 %. The probe now measures kinds not yet in `ALL` too.
   *For the integrator:* Tympanum's fork counts **7 171 KB** against its stub's 2.
6. **`fides_pick_e2e`'s picker test needs Fides in `ALL`.** Marked
   `#[ignore = "runs once Fides is in ALL — the owner's listening verdict (docs/round41/fides.md)"]`
   (only `the_picker_lists_fides_in_both_languages_on_the_keys_shelf`);
   `choosing_fides_adds_a_track_that_plays_a_phrase_and_a_preset` passes without it, and
   `inst_faces` and `inst_shell` only name Fides in an exhaustive match (they iterate `ALL` or
   their own lists, neither with Fides). The probe below lives in the same file.

Also, for `clippy --all-targets -D warnings` (Rust 1.97): five lints in `fides_body.rs` and
`fides_modal.rs` (range loops to iterators, a collapsed `if` — the same arithmetic, the
same bits) and `inst_fides.rs`'s `f32::log10` → `cantus_dsp::math::log10`.

**What changed in sound.** The factory presets at their defaults, each on its audition
phrase and on its five notes through the engine
(`fides_pick_e2e::probe_the_factory_presets_bits`, 50 renders, bits before and after):
**all 50 the same bits** (the render after every fix, the lint refactors included). The fixes change only what was broken — a position moving slowly on a modal
object, a noise colour or width moved mid-note, a noise level raised from 0, a stage taken
out of use and brought back, B switched mid-note.

**Verified only in emulation:** an MPE controller's slide and a hardware knob on the
noise's colour — the tests write the expressions and the macros.
