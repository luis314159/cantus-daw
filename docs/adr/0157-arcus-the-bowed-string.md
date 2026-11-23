# ADR 0157 — Arcus, the bowed string

**Status:** Accepted (round 42; the integrator, with the owner's mandate of 2026-11-22; the
name is the integrator's, the owner may rename it before beta.18) · **Date:** 2026-11-22 ·
**Builds on:** ADR 0023 (the instrument family), ADR 0038 (voices across the pool), ADR 0059
(a model version per instance), ADR 0150 (Fides: its bow, its waveguide, its cello body), ADR
0155 (the project's tuning) · **Schema:** 81

## Context

The owner, 2026-11-20 (BACKLOG §3e, *Strings and winds, modelled and recorded*): an
orchestral family as professional as Chorda and Tympanum, a modelled voice and a recorded
one, the bar SWAM (modelled), BBC SO / Spitfire (recorded) and Logic's Studio Strings (ease).
The research of 2026-11-22 (`research/instruments-next/strings-verified.md`) found: no SWAM
or Audio Modeling patent in the US, EP or WO; every patent the design touches expired; four
that might be in force (Fable Sounds' stepped CC1 layers and virtual key-switches, Yamaha's
legato joint built after the next note-on, Yamaha's recording fed into the loop, a winds-only
reed patent) — each avoided by the design below and listed for a lawyer before release
(`docs/beta-checklist.md`). Measured targets exist for the cello (Schelleng and Guettler
diagrams of its open D, Galluzzo & Woodhouse 2014; Guettler & Askenfelt 1997's attack limits;
vibrato 5.5–6 Hz at ±15–25 cents with the partials' amplitude movement from the body).
Fides's bow measured +3.1 to −1.2 cents of pitch offset from C2 to C6 and its tracker failed.

## Decision

- **Arcus** (`InstrumentKind::BowedString`, Latin for the bow) is its own instrument, on
  Fides's DSP (its bow, waveguide string and body code, reused by public functions; Fides
  stays the general resonator and its golden hashes do not move).
- **Round 42 ships the cello and the double bass**, solo, to the gate below; **the violin and
  the viola** are built in the same engine (string tables, bodies scaled from the cello and fit
  to the CNSM violin admittance, CC BY 4.0) and **offered only if they pass the same gate** —
  otherwise they stay out of the picker with the reason written in `docs/round42/arcus.md`.
- **Techniques:** arco sustain with legato (0 ms: the string keeps ringing, the finger moves)
  and portamento on request; détaché with re-bow and bow direction (alternating by itself, or
  the note's own, ADR 0158); pizzicato (Fides's pluck on the same string and body); tremolo;
  a delayed vibrato of its own whose partials' amplitude movement comes from the body.
  Harmonics, sordino, sul ponticello/tasto and col legno are the next pass.
- **Playing with a keyboard alone never falls silent:** velocity sets bow force and
  acceleration inside the Guettler region; CC1, CC11, aftertouch, breath or MPE pressure move
  the dynamic continuously (bow force and speed along Askenfelt's measured relation); CC2 or
  CC21 the vibrato; MPE slide the contact point.
- **Pitch:** a correction table measured offline per pitch × force × bow position, applied to
  the loop's length, deterministic; the bowed pitch ≤ 1 cent from its target over the grid,
  plucked 0.1 cent.

## The builder's design (round 42, the `arcus` builder)

Files: `cantus-core/src/inst/arcus.rs` (the table), `cantus-dsp/src/arcus*.rs` (the DSP),
`cantus-engine/src/inst/arcus.rs` (the unit), `cantus-ui/src/views/inst/arcus*.rs` and
`kit/skin/arcus.rs` (the editor and its face); the measurements in `docs/round42/arcus.md`.

- **A string in physical units** (`arcus_string.rs`): Fides's topology — two loops meeting at
  the bow, the loss lumped at the bridge, the neck loop solved for the exact period on
  `gravis::string`'s public tuning maths — in velocity waves (m/s), each string with its
  length, tension and so its impedance `Z₀ = T/(2Lf₀)` (`arcus_tables.rs`, the sources cited
  there and their status). No stiffness allpass: orchestral strings sit far under
  Schelleng's 0.1 cent per n².
- **The bow** is McIntyre–Schumacher–Woodhouse's junction solved exactly: with `R = F/(2Z)`
  and `u = v_b − v_h`, stick while `|u| ≤ R·μs`, else the hyperbolic curve's slip is a
  quadratic's larger root (closed form, no iteration; the Friedlander rule for its
  hysteresis). The **thermal option** (one contact temperature heated by the slip's power,
  `μ(θ) = μd + (μs − μd)/(1 + θ)`) is behind the same switch; measured, it keeps Helmholtz
  motion in a narrower band and settles fewer attacks, so the hyperbolic curve is the default.
- **The string's torsion at the contact** (added after measuring): the bow moves the string's
  surface, translation and twist in series, so a sticking bow is not a rigid end; the twist's
  short, heavily damped loops (speed ×5, impedance ×3, Q 5) carry away the ripple between bow
  and bridge. Without it the Helmholtz region spanned 0.3–0.85 of Schelleng's upper force and
  8 of 48 test attacks settled within 50 ms; with it 0.15–1.0 (Schelleng's "1 to 10") and a
  Guettler wedge of good attacks. Its Q is below the literature's tens: said so.
- **The gesture** (`arcus_voice*.rs`): the dynamic `d` sets speed (geometric pp → ff), β
  (closer to the bridge as it grows) and force (0.42 of Schelleng's upper limit, the middle of
  the measured region) together (Askenfelt 1989's direction); the velocity sets the attack in
  the model's own measured terms — the speed gained a period `α = a·P/v` from 0.18 to 1.45,
  the force at 0.65 of Schelleng's limit while it accelerates; a re-bow slows the bow through
  zero on the ringing string and starts the other way; a slur moves the loop's length over the
  finger's 3 ms with the bow going on; staccato stops the bow on the string, spiccato bounces
  it, tremolo reverses it at its rate; the direction alternates or is the note's (`note_art`).
- **Pizzicato** is Fides's pluck exciter on the same string (DC blocker in, as Fides's).
- **The pitch**: the bowed offset measured offline per key × dynamic × contact point
  (`arcus_pitch_data.rs`, `tools/arcus/pitch-table.sh`, two passes) corrects the loop while the
  bow is on; the free and plucked string is exact (the loop's own design).
- **The body** is Fides's (`fides::body::Body`): the cello's, and for the double bass the
  cello's scaled by 1.72 with a radiation corner (two first-order high-passes at 120 Hz) that
  matches the Smolken bass's harmonic balance; vibrato is the loop's length moving, so the AM
  comes from the body.
- **Determinism, rest, the pool** as Fides's; voices on lanes summed in order (ADR 0038).

## Consequences

- One more instrument in the null corpus and the bench; its model version stored (ADR 0059).
- The recorded strings (ADR 0158) are its listening reference: the Karoryfer Bigcat cello and
  the Smolken double bass (both CC0) for the ABX.
- Sections, the stage and its microphones, the hybrid voice and the remaining techniques are
  round 43's (BACKLOG §3e).

## What the tests hold — the gate, before anyone listens

1. Schelleng: over β × force at 10 and 20 cm/s, Helmholtz motion holds between the bounds,
   the shape of Galluzzo 2014's cello D. 2. Attack: the pre-Helmholtz transient ≤ 50 ms (or
   90 ms for multiple flyback) at every default velocity. 3. Pitch as above. 4. Vibrato: the
   rate and depth by default; each harmonic's amplitude movement differs and vanishes with the
   body off. 5. The body passive and within Fritz 2007's thresholds at its signature modes.
6. CPU: one voice ≤ 0.5 % of a core at 48 kHz on a quiet bench; sixteen across the pool.
7. The house invariants: null at defaults, any block size the same bits, offline = real time,
   pool = serial, exact rest, an end-to-end test through `CantusApp::assemble`. 8. Then the
   owner's blind ABX against the recorded cello and bass, a solo note and a four-bar phrase
   with legato and a CC1 swell.
