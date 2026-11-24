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
  bow is on, and a closed loop trims what is left: its **ear** (`arcus::ear`) is a heterodyne
  of the voice's own output at the target (a phasor that follows the vibrato and the glides),
  summed over triangle-weighted windows of whole periods (at least four and 20 ms), the phase
  of one window against the last the fundamental's offset — the quantity the gate measures.
  The trim moves by 0.6 of each window's offset, at most 2 cents a move, within ±15. (The first
  loop timed the main slips; on the lowest strings at pp far over the fingerboard the motion
  settles for a second with the slips' period steady and the fundamental's phase drifting, and
  four held-out points stayed 1.0–1.4 cents off.) The free and plucked string is exact (the
  loop's own design).
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

## Amendment (round 42, the owner's ask): model 2, matched to recordings

**Context.** The owner: "do what we did with Chorda — download high-quality recordings and do
a spectral analysis so our base sound gets better." **Decision.** `MODEL` 2.0 (stored with every
new Arcus; model 1 kept for anything made with it): `arcus_tables::Match` per instrument — a
correction of twelve peaking sections after the body (`arcus::eq`, 63 Hz – 10 kHz), the
hair's noise, the vibrato's depth and rate and the pluck's ring — fitted by
`tools/arcus/match.sh fit` into the spread of about 1 060 recorded notes (the Iowa MIS's
anechoic cello and bass, TinySOL, VSCO 2 CE, Karoryfer, Smolken; licences and sources in
`docs/round42/arcus-match.md`) on two thirds of the notes, judged on the third held out.
Also, for both models: the registers balanced (`balance_db_st`, measured: the cello fell 0.47
dB a semitone, the bass 0.16) and the output trims at 16 dB. **Consequences.** The cello's
held-out distances halve (harmonics 8.7 → 4.1 dB, spectrum 8.2 → 3.8, centroid 7.1 → 1.3
semitones); the vibrato's depth falls inside the recordings' (±19 → ±15.5 cents the cello,
±21 → ±10 the bass); the bass's correction is left flat (fitted, it moved the held-out notes
away). The pizzicato's brightness, the attack and the bass's own body are left open, said in
`arcus-match.md`. **What the tests hold.** Every earlier test at model 2's defaults (the
pitch gate, the attacks, the vibrato's rate and depth within their bounds, rest, block sizes,
the pool); `match.sh` prints the distances.

## Amendment (round 42, pass 2): the reference study's recommendations

**Context.** The study (`F:\projects\daw-r42\study\bowed-realism.md`, its database of 7 cellos
and 5 basses). **Decision.** The defaults change in place (Arcus never shipped; `model` 2):
measured bodies (`BodyKind::MeasuredCello` and the bass's own `MeasuredBass`, A0 70 Hz; Fides's
list unchanged) with the match's correction fitted again over them; the players' dynamic (pp →
ff force ×4.5, β ×0.5, speed ×1.8) with the pitch table measured again; a sustain's level swell
(20 → 10 dB under over 0.25 → 0.05 s) over the clean start, détaché, staccato and spiccato
keeping their speech; the bow noise raised to the bands; a 30 ms finger change with a 3 dB
dip; a pluck's snap; the vibrato by dynamic and the study's rates and depths; the bass's trim
6 dB. Not done: per-partial pizzicato decays and stiffness, the 2025 thermal friction, bow
motion tied to the vibrato. **The measure.** `tools/arcus/study_compare.py` judges the held-out
notes by the study's own estimator against the band of real instruments
(`docs/round42/arcus-match.md`, pass 2). The gates hold: the pitch 168 of 168 within a cent,
the attacks 0 of 80 over Guettler's limits.
## Amendment (round 42, the `arcus-upper` builder): the violin and the viola

<!-- r42:arcus-upper -->

**Context.** At 48 kHz a violin's upper notes leave the bow–bridge segment (`β·P` samples)
too short for the junction's lines and for the motion between bow and bridge: the voice's
`min_beta` forced β above the player's from E5 up (0.14 at E6, 0.27 at E7, against ~0.06 at
ff), and above E6 the motion broke (−30 to −50 cents, the energy between the harmonics).

**Decision.** The violin's voices run **at 4× the rate**, the viola's at 2× — the string, the
bow, the pluck and the pitch loop, their control steps as often in time as the cello's — and
come back through a linear-phase half-band cascade (23 + 75 taps, flat to 20 kHz, ≥ 90 dB
down from 28 kHz; 21 samples of latency); the body, the match and the output stage stay at
the rate (`arcus::upper`). Chosen by measurement (`upper_tests_rate::probe_rate`) over 1× and
2× and over a locally oversampled bridge segment (whose rate changes inside the loop would put
an interpolator's delay and ripple round the loop every period). The cello and the double bass
run the code they ran, at 1×: their output is the same to the bit (a test holds it). On the
upper strings the bridge's corner rounding is at least 6 × the note's fundamental and the bow
lightens and slows high on a string (half an octave of force and a fifth of speed at two
octaves up), as players bow there. Their strings are Percival's measured lengths and decays
and Rossing's tension ranges; the violin's body is **fitted to the CNSM dataset's measured
bridge admittance** (CC BY 4.0) by positive modes; the viola's is the violin's at 1.12 × its
size (the study's measured A0s, 250 against 281 Hz; no measured viola admittance exists in the
open). Their dynamic is mapped the players' way (the study's database: force ×5, β ×0.4, speed
×3 pp → ff) and their noise, vibrato and pluck matched by the study's own estimator. The body radiates each mode above 700 Hz with its own sign and strength and carries a cloud of 60 modes the fit does not resolve (900 Hz – 9 kHz): the measured admittance alone moves a vibrato's partials ±0.6 dB, the recorded violins ±1.5 – 2.2; with them ±1.4 (violin) / ±1.5 (viola).

**Consequences.** A violin voice costs about 1.5 × a cello voice at 1×, a viola voice 1.15 ×
(measured beside each other on a loaded machine; a quiet bench must say how much); its memory is the cello's (the lines already hold its period at 4×). The
instruments stay out of the picker until the owner's listening verdict, as the cello's.
