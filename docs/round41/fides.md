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
