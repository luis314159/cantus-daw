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
