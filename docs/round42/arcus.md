# Round 42 — `arcus`: Arcus, the bowed string (ADR 0157)

*The `arcus` builder's report. Branch `r42/arcus` from `r42/contract`.*

## What was built

**Arcus** (`InstrumentKind::BowedString`): a cello and a double bass modelled, not sampled — a
string in physical units bowed by a force in newtons at a speed and a distance from the
bridge, met by friction, radiated through Fides's body. The design is ADR 0157's *builder's
design* section; the player's view is `docs/manual/en/arcus.md` (and `es`).

- **DSP** (`cantus-dsp/src/arcus*.rs`, MPL-2.0): the string (`arcus_string.rs`: Fides's
  two-loop waveguide on Gravis's public tuning maths, in m/s, each string's impedance from its
  tension; the McIntyre–Schumacher–Woodhouse junction solved in closed form — the hyperbolic
  curve's slip a quadratic's root; the thermal option; the string's **torsion** at the
  contact), the tables (`arcus_tables.rs`: four instruments' strings, sources and status
  cited), the voice (`arcus_voice*.rs`: the dynamic's bowing map, the attack in measured terms,
  re-bow, slur, portamento, staccato, spiccato, tremolo, pizzicato with **Fides's pluck**,
  the delayed vibrato on the loop's length, MPE, the bow-hair noise layer), the pitch table
  (`arcus_pitch*.rs`), the render on the pool's lanes into **Fides's body and output stage**
  (`fides::body`, reused by its public API; the double bass's body the cello's scaled ×1.72
  with a radiation corner — said plainly: no measured bass body was found).
- **The model** (`cantus-core/src/inst/arcus.rs`): 47 keys (`model` and `poly` set, never
  automated), their choices' names, the sources (macros and controllers).
- **The unit** (`cantus-engine/src/inst/arcus.rs`): settings from the spec, the routes on the
  control grid, CC1/CC11/breath/aftertouch → the dynamic as `dyn.src` says, CC21 (or CC2, or
  aftertouch) → the vibrato, CC65 → portamento, MPE pressure/slide/tuning, `set_tuning` (the
  track's table added to each key; 12-TET adds 0.0 — the same bits), `note_art` (the note's
  articulation and bow), `fork`, voices across the pool.
- **The editor** (`cantus-ui/src/views/inst/arcus*.rs`, `kit/skin/arcus.rs`): Play (the
  string as it is bowed and the **Schelleng diagram** with the measured Helmholtz region and
  the bow's point — the display is the control; the instrument's card with a readout of the
  note, the string, the dynamic's marking and the bow's numbers), Edit (every part as a
  card), Matrix (the frame's); its face — **design sheet v1** for the owner
  (`docs/round42/arcus/sheet/`). 24 factory presets, both locales, the manual.
- **Tools** (`tools/arcus/`): `listen.sh` (the owner's renders), `pitch-table.sh` (the bowed
  pitch table), `compare.sh`/`arcus_compare` (beside a recording), `gate.sh` (the probes).

## Measured (the gate of ADR 0157)

*Measured on a loaded machine (two or three other builds running): no CPU claim.*

1. **Schelleng** (`tests_gate2::the_helmholtz_region_has_schelleng_s_shape`, the cello's open D,
   10 cm/s): Helmholtz motion holds between a lower and an upper force at every β from 0.06
   to 0.22 — 0.51–2.67 N at β 0.06 … 0.08–1.67 N at β 0.22, the ratio of the limits 2–21 (5
   at β 0.13; Schelleng's "1 to 10"). The **lower limit falls as β^−1.5** (Schelleng's β⁻²,
   a little gentler), the **upper only as β^−0.23** — gentler than Schelleng's 1/β: at a long
   β the twist lets a sticking bow hold more than the rigid-contact formula (0.63 of
   Schelleng's F_max at β 0.06, 1.46 at β 0.22). Against Schoonderwaldt 2008: the lower
   limit moves with the bow's speed here (Schelleng's law) where his bowing machine found it
   independent. The region's shape against Galluzzo 2014's cello D: their figures were not
   available (paywalled, NV) — compared in kind only.
2. **Attack** (`…::the_attack_settles_within_guettler_and_askenfelt_s_limits`, every string at
   its open note and a fifth up, velocities 0.15–0.95, the instruments' defaults): the
   pre-Helmholtz time (to six main slips a period apart within 4 %) against Guettler &
   Askenfelt's 50 ms / 90 ms (multiple flyback) — for the cello in ms, for the double bass in
   the violin G's periods those limits are (its E1's period is 24 ms). *Numbers: see below.*
3. **Pitch** (`tests_pitch`): *see below.* Plucked: **−0.0013 cent worst** over the four
   instruments (`tests::a_plucked_note_is_within_a_tenth_of_a_cent`).
4. **Vibrato** (`tests_gate2`): at A3 **6.0 Hz, ±22.1 cents** by default; each harmonic's
   swing through the body 1.6–7.2 dB, all different (spread > 1 dB), and without the body
   0.3–2.7 dB (the loss filter's own slope on the sweep) — the AM is the body's.
5. **Body** (`…::the_bodies_ring_at_their_signature_modes_and_die_away`): the cello's and the
   bass's four signature modes ring where the table puts them over the body's size, within
   1 %; the impulse dies (second half < 10⁻³ of the first); the loop is passive (a plucked
   string's energy falls every four periods). Against Fritz 2007's thresholds: the realised
   modes are the table's to well within them; whether Fides's cello table is within them of a
   real cello is unknown — no measured cello admittance was found (§3.1's gap).
6. **CPU**: *see below.*
7. **The house invariants**: a new instance silent and at rest; every instrument × technique ×
   friction × play mode finite, bounded and sounding; block sizes 1, 37, 64, 512 and ragged
   the same bits, renders repeat; the pool (reversed jobs, 2/3/7 lanes) the serial bits; a
   released note ends in exact silence and the instance is as good as new; the tuning at
   12-TET the same bits, an unmapped key silent; never silent from a keyboard alone (velocity
   1, CC1 at 0, CC11 at 0 all sound). Through the engine (`inst_arcus`, 7): block sizes 1, 37,
   64, 512, 4096 the same bits for six shapes, the extras at their defaults null, CC1 and MPE
   pressure and slide move the bow, every note art and bow reaches it and another note's does
   not, rest as new, a four-note chord's headroom, the pool; `rt_memory::an_arcus_never_allocates`
   (the audio thread and the pool); `arcus_image` (a fork's bytes against `approx_bytes`).

**Beside the references** (`tools/arcus/compare.sh`; the Karoryfer Bigcat cello's `A2_*_d.wav`
— sounding A3 — and the Smolken bass's `arco_d2_*_down.wav`, CC0, in
`F:\\projects\\daw-r42\\downloads\\arcus\\`): the cello's A3 harmonics 2–4 at −14/−34/−33 dB
against the recording's −22/−35/−40, its centroid 230 Hz against 225–235; the bass's D2 centroid
171–184 Hz against 196–300; the noise between harmonics within a few dB of the recordings'
in the lower bands, a little higher in the top one. The recordings' level reaches 90 % of
its sustain in 350–1 000 ms (the players' own onsets); Arcus speaks in 10–50 ms (a keyboard's).

## Verified only in emulation

- **The sound**: nobody has listened. Every number above is a measurement; the owner's ear
  is the judge (the renders below).
- **MPE and the controllers** from tests' expressions, never from a controller.
- **The look**: shot on the CPU renderer, Slate Lapis Night and Paper, 100 % and 200 %.

## Left

- **The owner's ears** on the renders and the blind ABX against the Karoryfer cello and the
  Smolken bass; then the one commit that puts `BowedString` in `InstrumentKind::ALL`.
- **The owner's eyes** on the design sheet (`docs/round42/arcus/sheet/`).
