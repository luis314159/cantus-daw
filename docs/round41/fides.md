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

See *Tests* below for the counts.

## Measured

(Filled at the round's close.)

## Shared files touched

(Filled at the round's close.)

## Verified only in emulation

(Filled at the round's close.)

## What the owner should listen to and look at

(Filled at the round's close.)

## Left

(Filled at the round's close.)
