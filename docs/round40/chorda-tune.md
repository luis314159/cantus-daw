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

The table lists every key that was more than 2 cents off by the ear, had its fundamental more
than 2.5 cents off, or beat deeper than 20 dB, plus A4 and B4. It gives the ear's pitch and
the fundamental against the curve (cents, at velocity 0.6), and the deepest beat of the
three strongest partials over velocities 0.35, 0.6 and 0.85 (dB). The "after" columns are
the shipped voicing. The causes were found by switching each part off in turn (below).

| Key | pitch | p1 | beat | after: pitch | p1 | beat | cause |
|---|---|---|---|---|---|---|---|
| A0 (21) | +6.6 | -1.1 | 3.0 | +0.0 | -1.6 | 4.0 | stiffness scatter (one string: nothing to centre it against) |
| A#0 (22) | +3.4 | -1.1 | 0.9 | +0.0 | -1.1 | 0.2 | stiffness scatter |
| B0 (23) | +17.9 | -0.4 | 4.3 | +0.0 | -0.9 | 1.9 | stiffness scatter (`uni_b_bass` 2) |
| D1 (26) | +2.1 | -0.7 | 2.5 | +0.0 | -0.9 | 4.0 |  |
| D#1 (27) | +3.1 | -0.8 | 2.4 | +0.0 | -1.2 | 4.6 |  |
| E1 (28) | +9.1 | -1.0 | 0.4 | +0.0 | -1.2 | 6.9 | stiffness scatter |
| F#1 (30) | +0.2 | -3.1 | 1.5 | +0.0 | -1.9 | 2.3 | bridge pull on the fundamental |
| G1 (31) | +1.3 | +2.7 | 4.9 | -0.0 | +2.0 | 2.3 |  |
| G#1 (32) | -0.5 | -4.3 | 3.0 | -0.0 | -1.0 | 2.0 | bridge pull on the fundamental |
| A1 (33) | +5.3 | +0.7 | 3.8 | +0.0 | +0.5 | 4.6 | stiffness scatter |
| A#1 (34) | -9.5 | -2.2 | 9.4 | +0.0 | -1.7 | 8.6 | stiffness scatter |
| C#2 (37) | +9.4 | +2.3 | 7.4 | -0.0 | +1.7 | 4.2 | stiffness scatter |
| D2 (38) | +8.8 | -1.4 | 5.6 | +0.0 | -1.7 | 4.5 | stiffness scatter |
| D#2 (39) | +2.8 | -1.7 | 4.3 | -0.0 | -1.2 | 2.3 |  |
| F2 (41) | -0.7 | -3.4 | 19.6 | +0.0 | -1.9 | 3.5 | bridge pull (in-phase against opposed modes) |
| G2 (43) | -0.3 | -4.7 | 19.3 | -0.1 | -1.6 | 6.8 | bridge pull |
| A2 (45) | +2.7 | +1.8 | 10.0 | -0.0 | +0.7 | 4.9 | stiffness scatter |
| A#2 (46) | -13.3 | -2.8 | 12.6 | +0.1 | +0.5 | 10.4 | stiffness scatter; its draw's bent ladder (redrawn) |
| B2 (47) | -2.5 | -3.0 | 22.6 | +0.0 | -1.7 | 17.4 | bridge pull, stiffness scatter |
| C3 (48) | +5.4 | -2.0 | 20.2 | +0.0 | -2.7 | 13.8 | stiffness scatter, bridge pull |
| C#3 (49) | -2.7 | -2.2 | 12.6 | -0.0 | -2.1 | 13.1 | bridge pull |
| D#3 (51) | +1.4 | -2.4 | 21.7 | +0.0 | +1.2 | 12.5 | the unison's draw (redrawn) |
| E3 (52) | -1.3 | -1.7 | 21.3 | +0.0 | -1.2 | 16.1 | the draw |
| F#3 (54) | +5.1 | +1.6 | 21.1 | -0.1 | +1.0 | 12.8 | stiffness scatter; redrawn |
| G3 (55) | +4.3 | -1.1 | 19.2 | +0.0 | -1.5 | 15.4 | stiffness scatter |
| G#3 (56) | -1.3 | -3.8 | 14.8 | +0.0 | -0.6 | 14.3 | bridge pull; redrawn |
| A3 (57) | -2.8 | -2.2 | 15.7 | -0.0 | -0.2 | 16.0 | stiffness scatter |
| A#3 (58) | -2.7 | -1.2 | 23.8 | +0.0 | -0.7 | 18.3 | the draw (redrawn) |
| C#4 (61) | -4.2 | -0.7 | 22.2 | -0.0 | -1.4 | 9.5 | stiffness scatter; redrawn |
| D#4 (63) | +0.2 | +0.8 | 21.0 | +0.1 | -0.2 | 19.4 | the draw (redrawn) |
| F4 (65) | -1.2 | +1.1 | 23.6 | -0.0 | +0.3 | 19.0 | fraction near its clamp; redrawn |
| F#4 (66) | -0.6 | -0.7 | 23.3 | +0.0 | -0.1 | 19.2 | the draw (now under the ceiling) |
| A4 (69) | -1.8 | -1.2 | 16.6 | +0.0 | -0.6 | 18.9 | skewed detune (uncentred) |
| B4 (71) | -1.8 | -2.2 | 17.0 | +0.0 | -0.7 | 14.3 | **fraction clamped** (+0.95: a cent of room upward) |
| E5 (76) | -0.6 | -0.5 | 24.9 | +0.2 | +0.3 | 18.4 | fraction near its clamp; redrawn |
| E6 (88) | -0.0 | -0.0 | 21.2 | -0.1 | -0.1 | 16.5 | redrawn |
| D#7 (99) | -1.2 | -1.2 | 21.1 | -1.3 | -1.3 | 22.1 | the board's tail (top octave, no table) |
| E7 (100) | -1.1 | -1.1 | 25.2 | -1.1 | -1.1 | 20.2 | redrawn |
| F7 (101) | +1.1 | +1.1 | 24.6 | +0.2 | +0.2 | 20.8 | redrawn |
| G#7 (104) | -0.7 | -0.7 | 21.6 | -0.7 | -0.7 | 21.6 | the board's tail |
| A#7 (106) | +0.3 | +0.3 | 28.4 | +0.5 | +0.5 | 19.7 | redrawn |
| B7 (107) | +0.2 | +0.2 | 20.1 | +0.2 | +0.2 | 19.3 |  |

The whole keyboard:

| | candidate 16 | shipped |
|---|---|---|
| The ear's pitch at velocity 0.6, worst key | **17.9 c** (B0) | 0.2 c A0–E6; 1.3 c above (B6) |
| The same at any of the three velocities | 18.3 c | 0.2 c A0–E6; 2.2 c above (D#7 at 0.35) |
| The ear's pitch, rms over the 88 keys | 3.6 c | 0.39 c |
| Keys more than 1 c off (more than 2 c), at 0.6 | 50 (21) | 6 (0), all above E6 |
| Worst key against its neighbours' mean | 16.2 c (B0) | 1.25 c (F6, just past the table's edge) |
| Fundamental, worst | 4.7 c (G2) | 2.7 c (C3: a 0.2 Hz beat in C2–C3) |
| Octaves 2:1 and fifths 3:2 (partials ≤ 5 kHz) off the curve: rms / worst | 2.55 / 9.8 c | 1.08 / 2.9 c |
| Deepest unison beat over three velocities (keys over 20 dB) | 28.4 dB (18 keys) | 22.1 dB (5, all C#7–G#7, under the top octave's 22 but D#7 at pianissimo, 22.1) |
| Median of the keys' deepest beats | 15.8 dB | 14.2 dB |
| The Salamander Grand, measured with the same kit | octaves and fifths 5.8 c rms, 34 c worst; deepest beats 7.5 (A0–A#1), 16.3 (B1–A#2), 12–28 (middle), 16–20 (treble) dB | |

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
     more than 2 c. Most keys keep them whole; about fifteen are scaled to 0.35–0.9
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
4. **Sixteen keys redrawn** (`draw.<key>`, the candidate-13 mechanism): C1, Bb2, D#3, F#3,
   G#3, A#3, C#4, D#4, F4, E5, F#5, E6, B6, E7, F7 and A#7.
   - Each takes a draw from the 32 the search tried (`tuning_draws`) by three rules.
     - Its deepest beat is under its register's ceiling at all three velocities, with 1 dB
       to spare. F4's draw 8 has only 0.3 dB to spare at velocity 0.35.
     - Its ladder is straight: partials 1–6 within 2 c of its pitch.
     - Its level is candidate 16's, measured on the lab's keys at mezzo-forte: within 1 dB
       over 0–0.5 s, 2 dB over 0.25–1.25 s and 3 dB over 1.25–3.4 s. Among the draws that
       pass, it takes the one whose median beat is nearest the key's old one.
   - F4 (+3.4 dB over 0.25–1.25 s), F7 (+4.2, then −6.0) and no other key had no draw that
     kept the level. Their old draws' deep beats were troughs that took the level down.
   - An earlier pass chose by the beat alone. F5 then stood 6 dB over its neighbours, so that
     pass was redone with the level rule. F5 and D#7 now keep candidate 16's draws.
   - Bb2's draw 3 had left its ladder 5.3 c bent (its 4:2 octave beat 1.6 Hz). Its draw 24 is
     straight to 1.0 c.
5. **`Voicing::tune`, the technician's last pass, on the keyboard** (`Tuning::tune`, in the
   designer's key; the sympathetic strings move with it).
   - Each key A0–E6 moves its whole ladder by what it still needed: −1.2…+1.1 c.
   - The table was re-measured for the redrawn keys.
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
  - `MODEL_2_SHIPPED` (new: `0x9561_d8f8_a3ce_3a61`).
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

## Checks (Windows unless said, at the branch's head)

- `cargo test -p cantus-dsp`: 1 301 passed, 0 failed, 57 ignored.
  - In an earlier full run, the CPU test failed once while other builders loaded the machine
    (see Left).
- `cargo test -p cantus-engine`: 973 passed, 0 failed, 17 ignored.
  - Pulsus's three golden tests were first run with `CANTUS_GOLDEN_DUMP=1`, which the dump
    takes for a folder, so they failed. Run again with a folder, they passed: their hashes
    held.
- `cantus-ui`, every test binary that plays Chorda (`chorda_*`, `inst_chorda*`, with the new
  `chorda_tune_e2e`): 35 passed, 0 failed, 8 ignored.
  - Through the app, Bb2 sounds +0.10 c, B2 +0.01, C3 −0.22 and B0 −0.00 against the curve.
- `cargo clippy -p cantus-dsp -p cantus-engine -p cantus-ui --all-targets -- -D warnings`:
  clean.
- WSL (Ubuntu 24.04, `tools/wsl/check.sh r40/chorda-tune clippy`, then
  `cargo fmt --all -- --check`): clippy exit 0, plug-ins without the editor exit 0,
  formatting OK.
- Not run: the whole workspace suite. The touched crates and every Chorda test above were run
  instead; the machine's three build slots were shared with other builders all evening.
- The web excerpts: `render_unit_sounds -- web chorda` (the example now takes card names after
  the folder and leaves the list and the dry sources alone). Only Chorda's four MP3s changed.

## The size of the change

`DSP_VERSION` 9 → 10, measured with `CANTUS_GOLDEN_DUMP`:

| Render | max \|Δ\| | RMS of Δ | |
|---|---|---|---|
| Every golden hash (`null_tests` `v5`, Pulsus's kits, the scene bakes, the null corpus) | 0 | 0 | none includes model 2 of Chorda |
| Model 1 (`tests_hash`, an old song through the app) | 0 | 0 | pinned, held |
| Model 2 at beta.14's voicing and at candidate 16 | 0 | 0 | pinned, held |
| Model 2 at its default (the pinned 0.5 s phrase) | 0.743 | 0.141 (−17.0 dBFS) | new pin |

The default phrase's difference is near the signal's own size, because the strings' phases
drift apart within it once their tuning moves. Its RMS goes from 0.187 to 0.202 (+0.64 dB).

Level, measured on the lab's keys at mezzo-forte (`keys_mf`, velocity 75, before against
after):

| Window | change, rms over the keys | keys moved more than 2 dB | each key against its neighbours, rms |
|---|---|---|---|
| 0–0.5 s | 0.27 dB | none | 0.77 → 0.83 dB |
| 0.25–1.25 s | 0.86 dB | F4 +3.4, F7 +4.2, C5 +2.1 | 2.38 → 2.16 dB |
| 1.25–3.4 s | 1.42 dB | A1 +5.7, B4 +6.1, F7 −6.0, E2 −3.4, D#3 +2.6, F#5 +2.3, C1 +2.2, F#3 −2.1 | 3.03 → 3.42 dB |

Where the later sustain moved, the key's tuning or its beat was the fault:

- B4's clamped string had held the unison apart.
- F4's and F7's old deep beats were troughs.
- A1's stiffness scatter was re-held as one set.
- E2 (a pair of strings and a horizontal loop) moved by its centring alone: its strings now
  stand differently against the loop.

## For the owner's ear

`F:\projects\daw-r40\compartir\chorda-tune\` holds the following, each rendered `-before`
(candidate 16, master 2a79ded2's sound, pinned) and `-after` (the shipped voicing) on a model-2
unit at the mezzo-forte velocity, 24-bit:

- `run-A1-C4`: a chromatic run up and down.
- `octaves-chords-Bb2-C3`: the octaves Bb1–Bb2 … C3–C4, the fifths Bb2–F3, B2–F#3 and C3–G3,
  and the Bb, B and C chords open and close.
- `worst-keys-held`: fourteen keys, 3.5 s each.
- `sweep-A0-C8`: all 88 keys.

`index.html` plays them side by side, and `notes.txt` (in Spanish) lists what was out of tune
and by how much, before and after. The renders come from the diagnostic `tune_renders`
(`CHORDA_RENDER_OUT`, `CHORDA_RENDER_VOICING=before|after`).

## Left

- **The owner's ear.** These are measures. His listening approval of the retuned voicing
  stays open, as does his ABX of candidate 16 against the Salamander (BACKLOG).
- **The top octave (F6–C8) has no table.** Its keys sit within 1.3 c at mezzo-forte and
  2.2 c at pianissimo (D#7). The board's high band and the room take part of a decaying
  partial's measured pitch there, and moving a key moved its beat by up to 9 dB, so a table
  did not converge. Making the measure (or the board's tail) robust there would let it reach
  a cent.
- **The fundamentals.** They sit up to 2.7 c off their own ladders (C3 −2.7, C#3 −2.1, G1
  +2.0): the bridge's allowed 2 c pull plus the scatter's residue. A tuner hears the ladder,
  and in C2–C3 that is a 0.2 Hz beat. A smaller cap (1 c) took it to 1.8 c but scaled most
  bass keys' resonances by half, so the c16 colour of their decay would have changed.
- **The stiffness scatter's upper partials.** Partials 3–6 of a key are still a cent or two
  apart from one another. That is why the 4:2 octaves are allowed 4.5 c. Centring each
  partial, not the scatter's mean, would need each string's partials solved at the strike.
- **The CPU test** `model_2s_sixty_four_pedal_down_voices_fit_a_quarter_of_a_core` failed once
  in the full parallel suite while other builders loaded the machine (64.3 % against its
  60 %). Alone it passed at 29.4 % and 54.3 %. The pass adds work only at a strike (the
  bridge's pulls at eight partials, once a note); the per-sample loop did not change.
- **The voicing lab's gate** (`tuning.py`, gate 2) is a second view. It flags D#7 at +6.7
  ragged ("informative" above E6), where the suite's kit measures −1.3. Its 1 s FFT start
  point lands on the board's tail there.
- **master's tune map** (`chorda_tests_tunemap.rs`, cantus-03's) still plays model 2 on model
  1's keyboard. It is ignored and untouched here, and `tests_tuning::tuning_map` replaces it;
  it can be removed when merged.
