# ADR 0150 — Fides, the resonator instrument

**Status:** Accepted as a contract (round 41; the owner named it, 2026-11-20); the `fides`
builder fills in the design and the measurements · **Date:** 2026-11-20 · **Builds on:**
ADR 0023 (the instrument family), ADR 0038 (voices across the pool), ADR 0059 (a stored model
version), ADR 0101 (presets as data) · **Schema:** 78

## Context

FL Studio covers plucked and bowed strings and struck bars with several instruments (Sakura,
Plucked!), and Ableton (Collision, Tension), Logic (Sculpture) and AAS (Chromaphone) build
them from an exciter into a resonator. Cantus has the parts of that family scattered: Gravis's
waveguide string (dispersion, frets, pickups), Pulsus's modal percussion voice, Lamina's
hammer, Chorda's soundboard and body responses — and no instrument that plays a marimba, a
kalimba, a harp, a nylon guitar or a bowed cello (BACKLOG §3e item 6, §6l A2). The owner chose
it for round 41 and named it **Fides**: the Latin for a lyre's strings — a stringed
instrument — and for faith.

## Decision (what the contract fixed)

- **`InstrumentKind::Resonator`**, its core module `inst::fides` (`NAME_KEY`
  `instrument.fides`, `ICON` Strings, `READY` false, `MODEL` 1.0 stored with every new
  instance), every dispatch arm across the workspace, a silent engine stub
  (`cantus-engine/src/inst/fides.rs`: no sound, at rest from birth), an editor stub
  (`views/inst/fides.rs`), Gravis's skin sheet as a placeholder, its locale area
  (`locales/areas/fides.*.txt`) and `--editor fides`.
- **Not in `InstrumentKind::ALL`** until it sounds (Tympanum's rule): every kind-wide test
  iterates `ALL`, and a stub would move the null corpus, the bench and the undo fuzz's draws.
- **Schema 78** (an empty step: an older build cannot read the variant's name; the fixture
  `v78.cantus` holds a Fides track).

## The builder's (to fill in)

The design (exciters: pluck, pick, a bow by a friction curve, a mallet, noise, a live input;
resonators: waveguide strings reusing `gravis_string`'s free functions, modal bars, plates,
membranes and tubes — Pulsus's modal tables are private to Pulsus: Fides keeps its own or they
are exposed additively without moving Pulsus's or Tympanum's hashes; a body; coupling; MPE
pressure and slide on bow pressure and position), the parameter table (keys forever once
shipped), the measurements, the presets, the face, and the commit that puts it in `ALL` and
the picker with its end-to-end test. See `docs/round41/briefs.md`.

## Consequences

- Every match on `InstrumentKind` has an arm for it from the contract on; the builder replaces
  stubs, never adds dispatch.
- A song saved with a Fides track does not open in beta.16 or older (schema 78).

## What the tests hold

- `cantus-project`'s `schema_78_keeps_fides_nitor_and_fragor_s_five_pedals`: the fixture's
  Fides track loads with its kind and its gain, and no older fixture holds one.
- Every existing test unchanged (the kind is outside `ALL`): the null corpus, the contract
  twins, the golden hashes.
