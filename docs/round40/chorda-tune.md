# Round 40 — Chorda's technician's pass (`DSP_VERSION` 10)

Branch `r40/chorda-tune` (from master 2a79ded2). ADR 0098's fourth amendment.

The owner, 2026-11-18: *"En Chorda hay notas desafinadas, detecté Bb2, B2, C3 por lo menos."*
Then: *"Checa si no hay más notas desafinadas también, esas son las que noté mucho."*
Cantus calls middle C C4, so those keys are MIDI 46, 47 and 48. All 88 keys were audited.

## How it was measured

- **The kit** (`crates/cantus-dsp/src/chorda_tunekit.rs`, started by the stopped
  fx/chorda-tune agent and reused here).
  - Each partial is heterodyned to its designed frequency (a Hann `4·fs/f0` long, both
    channels).
  - Its frequency is the power-weighted mean instantaneous frequency over 0.1–1.1 s, which
    is the pitch a beating pair gives the ear.
  - A key's pitch is its partials 1–8 below 5 kHz, weighed by their level and by the ear's
    dominance region (a log-normal bump at 700 Hz, 1.5 octaves wide).
  - A partial's beat is the 3–97 % swing of its level about a quadratic decay over
    0.15–3.3 s. That is the lab's `s24_beat_depth.py`, so its Salamander numbers compare.
- **The reference** is the curve the keyboard is designed to: `key.partial(n)` of model 2's
  own keyboard (`Voicing::default().tuning(..)`) without the technician's table. It is the
  smooth stretch the design draws.
- **A correction.** master's `chorda_tests_tunemap.rs` (cantus-03's map) played model 2 on
  model 1's keyboard (`Tuning::default()`), so its numbers are not model 2's. The diagnostics
  here (`chorda_tests_tuning.rs`, all ignored) play each voicing on its own keyboard:
  - `tuning_map`: all 88 keys at three velocities.
  - `tuning_partials`: each partial of a key.
  - `tuning_draws`: a key at every draw.
  - `tuning_wav`: a recorded keys file measured with the same kit (the Salamander).
  - `tuning_fractions`, `tuning_bridge_scale`, `tuning_coupling`.
  - `tune_renders`: the listening set.
  - `CHORDA_TUNE_SPEC` sets a spec over the shipped voicing, and `CHORDA_TUNE_KEYS` picks the
    keys.
- **Finding each cause.** Each part was switched off in turn (`uni_b=0`, `bridge.n=0`,
  `uni_skew=0`, the weights, `polar`, `uni_loss`), then the room and the board (the
  diagnostic's `CHORDA_TUNE_BODY=0`). One key's strings were also played alone at four
  couplings (`tuning_coupling`, exact at every one).

## The outliers in candidate 16, and their causes

The table gives the ear's pitch and the fundamental against the curve (cents), and the
deepest beat of the three strongest partials over three velocities (dB). The "after" columns
are the shipped voicing.

| Key | pitch | p1 | beat | after: pitch | p1 | beat | cause |
|---|---|---|---|---|---|---|---|
| A0 (21) | +6.6 | −1.1 | 3.0 | +0.0 | −1.6 | 4.0 | stiffness scatter (one string: nothing to centre against) |
| B0 (23) | **+17.9** | −0.4 | 4.3 | +0.0 | −0.9 | 1.9 | stiffness scatter (`uni_b_bass` 2) |
| C1 (24) | −0.0 | +1.8 | 18.1 | −0.0 | −0.0 | 4.6 | bridge resonance on the fundamental (and draw 4) |
| E1 (28) | +9.1 | −1.0 | 0.4 | +0.0 | −1.2 | 6.9 | stiffness scatter |
| G#1 (32) | −0.5 | −4.3 | 3.0 | −0.0 | −1.0 | 2.0 | bridge pull on the fundamental |
| A1 (33) | +5.3 | +0.7 | 3.8 | +0.0 | +0.5 | 4.6 | stiffness scatter |
| A#1 (34) | −9.5 | −2.2 | 9.4 | +0.0 | −1.7 | 8.6 | stiffness scatter |
| C#2 (37) | +9.4 | +2.3 | 7.4 | −0.0 | +1.7 | 4.2 | stiffness scatter |
| D2 (38) | +8.8 | −1.4 | 5.6 | +0.0 | −1.7 | 4.5 | stiffness scatter |
| F2 (41) | −0.7 | −3.4 | 19.6 | +0.0 | −1.9 | 3.5 | bridge pull (in-phase against opposed modes) |
| G2 (43) | −0.3 | −4.7 | 19.3 | −0.1 | −1.6 | 6.8 | bridge pull |
| **Bb2 (46)** | **−13.3** | −2.8 | 12.6 | +0.0 | −0.9 | 9.7 | stiffness scatter (and draw 3's bent ladder) |
| **B2 (47)** | −2.5 | −3.0 | **22.6** | +0.0 | −1.7 | 17.4 | bridge pull, stiffness scatter |
| **C3 (48)** | **+5.4** | −2.0 | **20.2** | +0.0 | −2.7 | 13.8 | stiffness scatter, bridge pull |
| D#3 (51) | +1.4 | −2.4 | 21.7 | +0.0 | +1.0 | 17.6 | the unison's draw (redrawn) |
| F#3 (54) | +5.1 | +1.6 | 21.1 | +0.1 | −2.0 | 11.8 | stiffness scatter; the draw |
| G3 (55) | +4.3 | −1.1 | 19.2 | +0.0 | −1.5 | 15.4 | stiffness scatter |
| A#3 (58) | −2.7 | −1.2 | 23.8 | +0.0 | −0.7 | 18.3 | the draw |
| C#4 (61) | −4.2 | −0.7 | 22.2 | −0.5 | −0.2 | 13.5 | stiffness scatter; the draw |
| F4 (65) | −1.2 | +1.1 | 23.6 | −0.0 | −0.8 | 18.7 | fraction near its clamp; the draw |
| B4 (71) | −1.8 | −2.2 | 16.7 | +0.0 | −0.7 | 14.2 | **fraction clamped** (+0.95: a cent of room upward) |
| E5 (76) | −0.6 | −0.5 | 24.9 | −0.0 | −0.0 | 12.4 | the draw; fraction near its clamp |
| E7 (100) | −1.1 | −1.1 | 25.2 | −1.1 | −1.1 | 19.3 | the draw |
| A#7 (106) | +0.3 | +0.3 | **28.4** | +0.5 | +0.5 | 19.7 | the draw |

The whole keyboard, at velocity 0.6, and the beat over three velocities:

| | candidate 16 | shipped |
|---|---|---|
| The ear's pitch, worst key | **17.9 c** (B0) | 2.2 c (D#7; 1.0 c A0–E6) |
| The ear's pitch, rms over 88 keys | 3.6 c | 0.45 c |
| Keys more than 1 c off (more than 2 c) | 51 (23) | 7 (2), all above E6 |
| Worst key against its neighbours' mean | 16.2 c | 0.9 c |
| Fundamental, worst | 4.7 c | 2.7 c (C3) |
| Octaves 2:1 and fifths 3:2 ≤ 5 kHz off the curve: rms / worst | 2.55 / 9.8 c | 1.17 / 2.7 c |
| Deepest unison beat (keys over 20 dB) | 28.4 dB (18) | 21.6 dB (2: C#7, G#7, under the top octave's 22) |
| Median of the keys' deepest beats | 15.8 dB | 14.0 dB |
| For comparison, the Salamander Grand (same kit) | octaves/fifths 5.8 / 34 c; deepest beats 7.5 (A0–A#1), 16.3 (B1–A#2), 12–28 (middle), 16–20 (treble) | |

What was not a cause:

- The loop designs. Every key's design is within 0.5 c, and its single strings are exact.
- The fractional delay, except at the clamp (B4).
- The polarisation loop, the loss scatter and the sympathetic strings. Switched off, nothing
  moved.

What moved the treble's last cent or two is the board and the room. With both off, the keys
from G6 up sit at 0.0 c. With them on, the board's high band (a feedback delay network whose
decay shortens with frequency) and the room's tail take a share of a fast-decaying partial's
measured pitch. The share grows with the bridge coupling, which is what decays a treble
unison's first stage.

## The fixes

Each fix is a voicing knob, neutral at 0. `Voicing::beta14()` and `Voicing::candidate16()`
both keep their bits.

1. **`uni_center` 1: the unison centred on its key.**
   - The strings' detunes (the fan and the skew) and their stiffness scatter each lose their
     mean.
   - The mean is weighed by the strings' shares of the blow, squared (their power).
   - The scatter is held as one set: its fallbacks (the sign, a half, a quarter, an eighth)
     scale every string's together.
   - A key of one string keeps its design (`chorda_voice.rs` `centre`, `unison_weights`;
     `KeyStrings::set_string_stiffness_set`).
2. **`bridge.tune` 1, `bridge.pull` 2: the strings tuned on their bridge.**
   - For each of the key's first eight partials (up to 5 kHz), the reflection the strings
     meet when they move together, `−(1 − nY)/(1 + nY)`, is evaluated from the bridge's
     admittance (`YZ` and its resonances). Its phase over the loop's group delay is that
     partial's pull (`KeyStrings::pull_cents`).
   - The key's resonances are scaled down until none pulls one of the first three partials
     more than 2 c. Most keys keep them whole; a dozen are scaled to 0.35–0.75
     (`tuning_bridge_scale`).
   - The strings' fractions then give back the pull the ear hears: the partials' pulls,
     weighed as the ear weighs them (`ear_weight`).
   - It is recomputed whenever `YZ` moves.
3. **`keyboard.frac_room` 8: room in every loop.**
   - `design::design_with_room` re-lays a design whose fraction sits within 8 c of its clamp.
     It moves a sample between the delay and the fraction and refines the design, keeping it
     only if its partials are as good.
   - Six keys moved: F4, B4, E5, F5, A5 and C#7.
   - The value is in `Tuning`, so it is part of the designer's `DesignKey`.
4. **Eighteen keys redrawn** (`draw.<key>`): C1, Bb2, D#3, F#3, G#3, A#3, C#4, D#4, F4, E5,
   F5, F#5, E6, B6, D#7, E7, F7 and A#7.
   - Each takes the draw whose deepest beat is under its register's ceiling (at all three
     velocities, 1 dB to spare) and whose median beat is nearest its old one. Their character
     is kept and their wobble gone.
   - Bb2's draw 3 had left its ladder 5.3 c bent (its 4:2 octave beat 1.6 Hz). Its draw 2 is
     straight to 0.9 c and needs no table.
5. **`Voicing::tune`, the technician's last pass, on the keyboard** (`Tuning::tune`, in the
   designer's key; the sympathetic strings move with it).
   - Each key A0–E6 moves its whole ladder by what it still needed: −1.2…+1.1 c.
   - It was measured with the kit and iterated with secant steps, three rounds, to under
     0.1 c.
   - A per-voice fraction could not carry it: a fraction near −0.95 moves the fundamental
     twice as far as the upper partials.
   - Above E6 there is no table. A key's own sympathetic string rings with it undamped, and
     its measured pitch moves with the board's tail. Iterated there, the offsets did not
     converge, and the unisons' beats moved by up to 9 dB.

## The tolerances held, and why

- **The ear's pitch.**
  - Within 1 c of the curve A0–E6. That is what a technician holds a unison and its octaves
    to by counting beats: a cent at C4 is one beat every 6 s against a pure octave.
  - Within 2.5 c above E6. There a cent is 2–4 Hz, the ear's discrimination is 0.3–0.5 %
    (5–9 c), and a tuner sets the top by ear.
- **The fundamental.** Within 3 c. That is the bridge's allowed pull of 2 c, plus about a cent
  the scatter leaves. The deepest is C3's at −2.7 c. In C2–C3 that is a 0.2 Hz beat,
  slower than one beat in five seconds.
- **The neighbours.** Each key within 1 c of its neighbours' mean A0–E6, 2.5 c above. The
  curve's own bend over two semitones is under a hundredth of a cent.
- **Octaves and fifths.**
  - The coincident partials of every 2:1, 4:2 and 3:2 between keys (partials up to 5 kHz)
    beat within 0.33 Hz of the curve's own, or within 3 c (3.5 c above E6).
  - The 4:2 is allowed 4.5 c or 0.5 Hz: its upper partial is weaker, and it is where the
    strings' stiffness scatter lives.
- **Beat depth ceilings** (deepest of the three strongest partials). Each register's ceiling
  is the Salamander's own worst, measured with the same kit, plus a little room:
  - 10 dB for A0–A#1;
  - 16 dB for B1–A#2;
  - 20 dB for B2–B6;
  - 22 dB for C7–C8.

## Tests

- `chorda::tests_tune_gate` (in the normal suite, 5 s on 16 threads):
  `every_key_sounds_on_the_curve`, `neighbouring_keys_lie_on_one_smooth_curve`,
  `octaves_and_fifths_beat_as_the_curve_draws_them` and
  `no_unison_beats_deeper_than_a_recorded_grands`.
- The pins (`chorda_tests_voicing3.rs`):
  - `MODEL_2_BETA14` (held).
  - `MODEL_2_C16` (candidate 16, the old shipped pin, held).
  - `MODEL_2_SHIPPED` (new: `0x4965_b670_bc68_7c27`).
  - `the_shipped_voicing_is_candidate_16_with_the_technicians_pass_and_a_new_unit_plays_it`.
  - Model 1's pin is held as well.
- `cantus-ui` `chorda_tune_e2e`: a Chorda added the way the picker adds one plays Bb2, B2, C3
  and B0 through the real engine's render, on the curve within 1.5 c. It also checks that
  the app's keyboard carries the table.
- `cantus-engine` `inst::chorda`: the designer's key tells `frac_room` and the table apart.
- The voicing lab's gate (`voicing\ana2\tuning.py`) now has a gate 2 over all 88 keys: the
  ear's pitch, its raggedness against the neighbours, and every octave and fifth against a
  smooth fit of the candidate's own partials. Its first gate (each fundamental within 10 c
  of the Salamander's) is kept. The old file is `tuning_pre_r40tune.py`.

## The size of the change

GOLDEN

## For the owner's ear

RENDERS

## Left

LEFT
