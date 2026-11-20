# Round 41: Chorda, every string of every key in tune

Branch `r41/chorda-strings` (from master a1f04c40). BACKLOG §3e, *"Every string of every key
in tune, each measured alone inside the instrument"*.

The owner wrote on 2026-11-19: *"en Chorda deberíamos validar afinación de cada cuerda,
cuerdas de una nota para ver que suenen afinadas también."*

**The result.** Every loop of every key sounds within its tolerance. That covers 254 loops,
each struck at three velocities, both as the instrument plays it and with the action's
noises off. The worst string alone is E4's first string at velocity 0.9: +1.35 cents from
its designed pitch, which is 40 % of its tolerance. Almost all of that is the technician's
table (+1.03 cents on E4), which every string of the key carries. Nothing was retuned, and
no sound moved: `DSP_VERSION` is unchanged and every golden hash still passes.

Everything here was measured on the model. **Nobody has listened yet.** The strings-alone
renders are not in the listening set.

The owner's page is `docs/round41/chorda-strings.html`. It is self-contained and works in
both light and dark mode. It shows every key, every string and its error in cents, with the
tolerance drawn as a band.

## What was built

- **The technician's felt.** This is a per-loop "keep" factor in `KeyStrings`
  (`chorda_string.rs`). Each loop's hammer speed and its reflection at the bridge are
  multiplied by it.
  - At 1, the default, every product is exact in IEEE 754, so the instrument is unchanged
    to the bit.
  - At 0 the loop is muted. Its hammer never meets it and it returns nothing from the
    bridge. It still loads the bridge, because the `n` in `v_b = 2 YZ Σa / (1 + n YZ)` is
    unchanged. So the string left sounding shows the pull it really has inside the
    instrument.
  - It is reached through `Chorda::set_loops_kept(key, [f64; 3])`, which is
    `#[doc(hidden)]`, meant for tests and the voicing lab, and not part of the instrument's
    controls.
- **What each loop was laid with.** `Chorda::loops_laid(key)` returns a `LaidLoops`. For
  each loop it holds:
  - its designed detune in cents;
  - its stiffness scatter as applied (`eps`, the sections' pole angles × `1 + eps`);
  - its fraction as the voice designed it;
  - its fraction now, after the bridge's tuning has moved it.

  These are recorded in `lay` and in `set_string_stiffness_set`. Recording them changes no
  sample.
- **A prediction.** `KeyStrings::solo_phase` is test-only. It gives the phase the bridge
  adds to one loop sounding alone:
  - a vertical string meets `−(1 + (n − 2)Y)/(1 + nY)`;
  - the unison moving together meets `−(1 − nY)/(1 + nY)`;
  - so a string alone gets about `1/n` of the unison's pull;
  - the horizontal loop's side of the bridge is a resistance and adds no phase.

  Each loop's partials are then solved from its laid loop's phase plus that bridge phase.
- **The measurement.** It lives in `chorda_tests_strings.rs` and covers:
  - every key A0–C8 and every loop: the single strings of 21–30, both strings of 31–41, all
    three strings from 42, and model 2's horizontal-polarisation loop on keys 21–41;
  - velocities 0.3, 0.6 and 0.9;
  - 1.25 s renders through the whole model-2 unit: hammer, bridge and its resonances,
    phantom partials, sympathetic strings, board, body and room;
  - measurement with `tunekit` against the loop's **designed partials**. Those are the curve
    (`tests_tuning::curve_of`, the keyboard without the technician's table), moved by the
    loop's designed detune. Partials 2 and up are also moved by the loop's designed
    stiffness scatter: its laid loop's resonances against the same loop unscattered. The
    hold keeps the fundamental where the detune put it.
  - Everything runs twice: once as played, and once with `ChordaSettings::noise = 0`, which
    turns off the action's noises (see "The thud" below). Both runs take about 20 s on 16
    threads in the normal suite.
- **Diagnostics** (ignored tests):
  - `strings_map` prints every loop. With `CHORDA_STRINGS_JSON=<path>` it also writes the
    JSON the page is made from, including each key whole at the three velocities.
  - `strings_partials` prints each partial of the keys in `CHORDA_TUNE_KEYS`, broken into
    designed, rigid laid loop, predicted with the bridge, and measured.
  - `tests_tuning::unit` now also reads `CHORDA_TUNE_PHANTOM` and `CHORDA_TUNE_NOISE`, and
    `unit_with` sets the noises.
- **The page.** `tools/chorda-voicing/strings_page.py <json> <html>` writes the page.

Regenerate the page with:

```
CHORDA_STRINGS_JSON=strings.json cargo test --release -p cantus-dsp --lib strings_map -- --ignored --nocapture
python tools/chorda-voicing/strings_page.py strings.json docs/round41/chorda-strings.html
```

These runs used the test profile, where `cantus-dsp` is built at opt-level 3, the same
optimisation as `--release`, and one fewer workspace build on a full disk.

## Tolerances and why

| What | Tolerance | Why |
|---|---|---|
| A single-string key's string, alone (A0–F#1) | **1 cent** for the ear's pitch; 3 cents for the fundamental | The whole key's gate, unchanged. A single string has no unison to hide in: its table is its own and its bridge is tuned for it alone. |
| Any loop alone, ear's pitch | gate + allowance: 1 cent to E6, 2.5 above, plus the allowance | The allowance is what the design itself puts between a loop alone and its key. **The table**: measured on the key whole, laid on every loop alike. **The bridge's tuning**: each string is corrected for the unison's in-phase pull, but a string alone meets only `1/n` of that pull, so `(1 − 1/n)` of the 2-cent cap remains. The horizontal loop gets only the table. |
| Measured against predicted | 0.5 cent to E6; 2.5 above | What no formula here models (the hammer's contact, the phantom partials, the board's colouring of a decaying tone, the kit's own error) comes to a few tenths. Above E6 the board's high band's tail is a larger share of the measure, so the whole key's tolerance applies. Noises off. |
| Partials 1–3 below 5 kHz, within 30 dB of the loop's strongest | 3 cents + allowance | The whole key's limit for its fundamental. Above 5 kHz the loop design promises nothing (`CHECK_HZ`), and the whole-key gate skips those partials too. Noises off. |
| A unison's strings against each other | 1 cent for fundamentals; 1.5 for partials 2–3 | They share their bridge, table and correction, so only their designed difference should remain. A cent is what a technician holds a unison to by its beats. The detune and the correction are laid on each string's fraction, whose delay is not flat, so the upper partials move up to about a cent more or less than the fundamental. Noises off. |
| The horizontal loop against its string | gate + `(1 − 1/n)` × cap | The string alone carries the bridge's remainder; the loop, on a resistive side of the bridge, does not. |

## Results by register

Each row gives the worst error from the designed pitch (ear's pitch, cents) over all three
velocities as played. The quiet, predicted and table figures are for the same loop. The last
column is the loop that came closest to its own tolerance.

| Register | Loops | Worst | Tolerance there | Closest to its limit |
|---|---|---|---|---|
| A0–F#1, single strings | 10 × 3 | C#1 (25) at 0.3: −0.30 (quiet −0.30, predicted −0.26, table −0.27) | ±1.00 | 30 % |
| A0–F#1, horizontal loops | 10 × 3 | D1 (26) at 0.3: −0.52 (predicted −0.56, table −0.43) | ±1.43 | 36 % |
| G1–F2, strings | 22 × 3 | D2 (38) s2 at 0.3: −0.57 (predicted −0.62, table −0.67) | ±2.67 | 24 % (B1 s2) |
| G1–F2, horizontal loops | 11 × 3 | D2 (38) at 0.9: −1.01 (quiet −0.94, predicted −0.91, table −0.67) | ±1.67 | **71 %** (B1 at 0.9: +0.83 as played, +0.17 quiet; the thud) |
| F#2–B3 | 54 × 3 | C3 (48) s3 at 0.9: −1.13 (predicted −1.14, table −1.21) | ±3.54 | 38 % (D3 s1) |
| C4–E6 | 87 × 3 | E4 (64) s1 at 0.9: +1.35 (predicted +1.36, table +1.03) | ±3.36 | 40 % |
| F6–C8 | 60 × 3 | C8 (108) s1 at 0.3: +1.22 (predicted 0.00, table 0) | ±3.83 | 32 % |

The gate's own lines (`cargo test -p cantus-dsp --lib tests_strings -- --test-threads=1`):

```
worst single string: key 25 at 0.3 0.30 cents
worst loop alone: key 64 s1 at 0.9 1.35 cents
worst off its prediction to E6: key 86 s3 0.30 cents
worst first partial: key 30 s1 partial 1 2.21 cents
worst unison offsets off design (cents, key), partials 1-3: [(0.71, 31), (0.69, 86), (1.12, 86)]
worst horizontal loop against its string: key 33 0.57 cents
test result: ok. 9 passed; 0 failed; 2 ignored; ... finished in 19.61s
```

## What the measurement found

1. **No string is out of tune, and the model does what its design says.**
   - Below E6, each loop alone lands within 0.30 cents of the prediction. The prediction
     uses only its laid loop and the bridge phase, so the pitch of a string alone is
     explained by numbers the code sets.
   - In the top octave the residual reaches 1.22 cents (C8), inside the whole key's 2.5.
2. **Most of a unison string's error is the technician's table.**
   - The table (`voicing::tune::TUNE`, ±1.21 cents) was measured on whole keys. It moves
     every string so that the unison sounding together lands on the curve.
   - A string alone carries the table without the unison's own shift: C3's strings sit near
     −1.0 against a table of −1.21, and E4's near +1.1 against +1.03.
   - So on C3 and E4 the unison sounding together sits a cent away from each of its strings
     alone. In a real grand that is Weinreich's coupled unison. Here it is what the model
     does, and it is within tolerance.
3. **The thud and the bass fundamentals.**
   - The key's thud is the same whichever loop sounds. Its modes at 72 and 185 Hz ring on
     through the board and the room.
   - Struck alone at velocity 0.9, the weak fundamentals of G#1–C2 (32–36) measure up to
     +63 cents. These fundamentals sit 17–24 dB under their strongest partial, under the
     55 Hz high-pass. The horizontal loop, struck at 7 % of the blow, is worst.
   - The cause was found by switching each part off in turn:
     - phantom partials off: nothing changes;
     - sympathetic strings off: about a fifth less;
     - body off: about three fifths less;
     - **noises off: gone**, within 1.9 cents.
   - The ear's pitch hardly moves (B1's horizontal loop +0.83 as played against +0.17
     quiet).
   - Whole keys G1–C2 measure +2.45 cents at most on their fundamentals. Their strings are struck
     together, so the thud is smaller against them.
   - This is the thud's sound, not a string's tuning. The partial gates therefore judge the
     noises-off run, and the ear's pitch is judged in both runs.
   - Whether the thud's 72 Hz mode rings on too long under the bass fundamentals is a
     question for the voicing lab and the owner's ear (`knock`, `KEY_T60`). It is not a
     tuning fix.
4. **A detune on the fraction is not uniform in cents.**
   - The unison's fundamentals carry their designed offsets within 0.71 cents.
   - Partials 2–3 drift up to 1.12 cents from their designed offsets (D6, 86), because a
     first-order allpass's delay is not flat when its fraction sits far from zero.
   - The ear's spread of a unison is therefore not always its designed spread. Over
     F#2–B3 it averages 1.29 cents against 1.53 designed; C#3 (49) has 0.91 against 2.08.
     Its fundamentals keep their designed 2.08 exactly, and its third partials fan the
     other way.
   - Within tolerance. A possible lab item, if the owner hears C#3's unison as too pure: lay
     each string's detune as a delay at the ear's weighted partial rather than at the
     fundamental. That would need a `DSP_VERSION` bump.
5. **The whole key at other velocities** (outside this item, seen in the page's data).
   - Round 40's gate holds whole keys only at velocity 0.6.
   - D#7 (99) whole measures −2.81 cents at 0.3 and −2.02 at 0.9; its strings alone are
     within −0.30. That is just past the whole key's 2.5 above E6.
   - A beating unison's power-weighted pitch moves with where its beat falls in the window.
     D#7 beats 13–18 dB deep.
   - Worth a look if the owner hears D#7 or B6–A7 flat when played softly. Not retuned.

## Single strings to F#1, three from F#2

`tables::strings` has laid one string on A0–F#1 (21–30), two on G1–F2 (31–41) and three
from F#2 (42) since round 28 (087190cf). Its doc comment said "three from D2" from the same
commit. No version ever laid three strings on D2–F2, and every voicing, gate and recording
was made on 42. So the comment was corrected, not the keys. A test now pins the boundaries
(`one_string_to_f_sharp_1_two_to_f2_three_from_f_sharp_2`).

Real grands place this break differently by maker. Whether 42 is the Salamander's Yamaha
C5's was not checked here, and moving it would change the sound.

## What the tests hold (all in the normal suite, about 20 s)

- `every_loop_kept_changes_not_a_bit`: a phrase over keys of one, two and three strings
  renders the same bits with the hook untouched or set to `[1; 3]` on every key.
  `chorda_tests_voicing3.rs`'s three model-2 hashes and model 1's hash pass unchanged, so
  the hook's code moved nothing.
- `a_muted_loop_holds_nothing_and_the_kept_one_sounds`: a muted loop's lines hold exactly
  zero, the kept loop sounds, and every loop stays laid.
- `every_single_string_alone_is_held_to_the_whole_keys_gate`
- `every_loop_alone_sounds_its_designed_pitch`
- `every_loop_alone_is_where_its_loop_and_its_bridge_put_it`
- `every_loops_first_partials_lie_on_its_design`
- `the_offsets_inside_each_unison_are_as_designed`
- `the_horizontal_loop_stands_its_designed_offset_from_its_string`
- `one_string_to_f_sharp_1_two_to_f2_three_from_f_sharp_2`

All 124 chorda tests pass. `cargo clippy -p cantus-dsp --all-targets -- -D warnings` is
clean.

## Verified only in emulation

All of it. These are measurements of the model's output with the same kit as round 40's
gate. No string alone has been listened to. The kit's own accuracy on a real piano was set
in round 40 against the Salamander (`tuning_wav`). It was not re-checked here, and the
Salamander has no strings-alone recordings to compare against.

The Linux check (`tools/wsl/check.sh`) was not run from this worktree. The integrator runs
it.
