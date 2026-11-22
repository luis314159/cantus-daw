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

## The builder's design (round 41, the `fides` builder)

**One voice** is an exciter, one or two resonators and their coupling; the instance sums its
voices (each into its own lane on the pool's threads, the lanes in voice order — ADR 0038)
into a body and an output stage. Files: `cantus-core/src/inst/fides.rs` (the table),
`cantus-dsp/src/fides*.rs` (the DSP), `cantus-engine/src/inst/fides.rs` (the unit),
`cantus-ui/src/views/inst/fides*.rs` and `kit/skin/fides.rs` (the editor and its face).

- **Exciters** (`fides_exciter.rs`): *pluck* and *pick* — a raised-cosine pulse of the contact's
  length through a leak whose corner is half the fundamental (a released displacement: the
  partials fall as 1/n from the first, at every pitch), the pick's a quarter as long with its
  scrape; *mallet* — Lamina's power-law spring pulse (`lamina_voice::HammerShape`, the
  Hunt–Crossley contact, p = 2.5), its contact `v^(−(p−1)/(p+1))` shorter the faster it lands at
  *Velocity tone* ½; *bow* — a velocity envelope the resonator meets through Smith's friction
  curve as the STK gives it (MIT): `min(1, (|(Δv + 0.001)·(5 − 4·pressure)| + 0.75)^−4)`, the
  stick and slip inside the resonator's loop; *noise* — band-passed noise or sparse grains
  through an ADSR, also a layer under any exciter; *input* — a track's audio: **another
  strip's post-fader output, chosen as an insert's key is (a sidechain; ADR 0154)**. Every pulse has the
  same area in samples at every rate. Strike and sustained parts leave the exciter apart, so
  each resonator takes them at its own gains.
- **The waveguide** (`fides_string.rs`, string, tube, pipe) is **two loops meeting at the
  exciter**: the bridge loop (`β·P`, the loss filter and the stiffness allpasses lumped there)
  and the neck loop (the rest), the string's velocity at the junction the sum of the two waves
  arriving, what the exciter injects leaving in both. Built on `gravis::string`'s public tuning
  maths (`lagrange`, `lagrange_delay`, `loss_design`, `loss_delay`, `ap_delay`,
  `dispersion_fit`); nothing of Gravis's changed. The Lagrange reads, the loss and the
  allpasses' phase delays at f0 are computed and the neck loop solved for the rest (three
  fixed-point steps), so the fundamental is exact at every control step; a closed tube's loop
  inverts and is half the period. A tube's and a pipe's open end radiates through a first-order
  high-pass at 0.7·f0. The strike is scaled by √(P·P_C4)/2, so a pulse is as loud (RMS) low as
  high.
- **Modal resonators** (`fides_modal.rs`, `fides_tables.rs`): each partial a complex one-pole
  `y ← r·e^{jω}·y + g·x` (exact in frequency — its angle is set, not a cosine's rounding),
  eight to a chunk summed lane by lane in a fixed order. Fides keeps its own tables, cited:
  the free–free beam's βL (Fletcher & Rossing §2.13) and its mode shapes, the cantilever's,
  Bessel zeros (Abramowitz & Stegun 9.5) and `J_m` by its series for membranes, a marimba bar's
  1 : 3.99 : 9.2 (Fletcher & Rossing §19.2), a kettledrum's preferred modes, a simply supported
  plate's `m² + (n/a)²`, a tuned bell's partials (Rossing ch. 9), a wine glass's ring modes, a
  steel-pan note's 1 : 2 : 3. **Pulsus's and Tympanum's files are untouched**: no golden hash of
  theirs moved. A bow on a modal object meets the shapes-weighted velocity of its modes.
- **Decay and material**: a partial's T60 is the fundamental's × `(f/f0)^(−p)`, `p =
  3·(1 − material)²` (wood 3 … glass and metal 0), × `2^(−decay_key·octaves from C4)`; after
  the key is let go, the shorter of that and *Release*.
- **Coupling**: *Parallel* (the exciter into A and B) or *A into B* (A's output drives B, scaled
  by B's `√(1 − r²)` — a marimba's bar into its tube).
- **The body** (`fides_body.rs`): a bank of modes (complex one-poles) per body — a classical
  guitar's air and top-plate modes (Fletcher & Rossing §9.4), a violin's signature modes and
  its bridge hill (Bissinger 2008), a cello's (the violin's scaled), a concert harp's soundbox
  and soundboard (Le Carrou 2005), a box, a shell — with a cloud of modes above them, two
  microphones; continuous in `size` and `ring` with nothing to rebuild; its power to white noise
  1, a tone right on a mode held to +10 dB over the dry (a sharp mode would sing a wolf).
  Chosen over Chorda's convolved responses: no response built off the audio thread, a body
  modulated in size at no cost, a fraction of the memory.
- **Determinism and rest**: every voice's control step on a grid of 32 samples from the
  stream's start (and once at the note's start), the waveguide's lengths gliding between steps;
  every random number a hash of the seed, the key and the note's id. A voice ends when its
  exciter is done and its resonators are still (a waveguide whose every wave passed the
  junction below −160 dB; modes below −180 dB), their state zeroed; the body and the cuts zero
  theirs the same way: `at_rest` is exact.
- **MPE**: a note's pressure presses the bow (`mpe.press`; with Noise or Input it raises the
  level), its slide moves where it is played (`mpe.slide`, ±0.2 of the length at full), its
  tuning bends it. The modulation's voice sources: the exciter's envelope, the vibrato's LFO,
  velocity, key, a seeded random value, MPE's bend, pressure and slide; routes to keys the
  voices move run per voice; to any other key (a body's size, a cut) the unit runs them from the
  macros and the controllers on the same grid.
- **The model version** `model` = 1 is stored with every new instance and is set, never
  automated (`SET_KEYS`, with `poly`).

**The parameter table** (keys forever once shipped) is `cantus_core::inst::fides::PARAMS`, its
units and ranges beside each key; the manual (`docs/manual/en/fides.md`) says what each does.

**Amended by ADR 0154 (round 41):** the *Input* exciter's audio is a sidechain from a strip
(`Track::sidechain`, schema 79), run before the track by the plan and compensated as the
track's own latency. **Inharmonicity** (round 41, `fides::string::stiffness_fit`): the fitted
partial is where a stiff string with `B = 4·10⁻⁴·inharm²` puts it at every setting — the loss
filter's own stretch counted, so 0 is harmonic and a negative setting draws the partials in —
where Gravis's fit, which only adds stretch past the loss's, did nothing under a threshold
that rose with the loss; above it, the same coefficient to the bit.

## Consequences

- Every match on `InstrumentKind` has an arm for it from the contract on; the builder replaces
  stubs, never adds dispatch.
- A song saved with a Fides track does not open in beta.16 or older (schema 78).

## What the tests hold

- `cantus-dsp` `fides::tests`: pitch within 0.1 cent at every bend for the struck and plucked
  resonators (worst measured 0.0014 cent, E1–A6); a bowed string measured (a few cents, the
  stick and slip's); renders the same at every block size and repeated, and the pool's reversed
  jobs the serial bits; every exciter on every resonator finite, bounded and sounding.
- `cantus-engine` `inst_fides`: the same through the engine's render; the extras at their
  defaults change no sample; MPE's pressure and slide reach the bow; exact silence, rest and as
  good as new; the eight-note chord's headroom; 32 heavy voices inside real time.
- `cantus-core` `undo_invariants::fides_edits_undo_and_redo_among_the_others`.
- `cantus-ui` `fides_pick_e2e` (runs with the commit that puts Fides in `ALL`).

- `cantus-project`'s `schema_78_keeps_fides_nitor_and_fragor_s_five_pedals`: the fixture's
  Fides track loads with its kind and its gain, and no older fixture holds one.
- Every existing test unchanged (the kind is outside `ALL`): the null corpus, the contract
  twins, the golden hashes.
