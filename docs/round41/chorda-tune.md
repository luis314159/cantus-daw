# Round 41 — Chorda, every string of every key in tune (`DSP_VERSION` 11, for the owner's ear)

Branch `r41/chorda-tune` (from master 5b38a884). ADR 0098's fifth amendment.

The owner, 2026-11-20, on beta.15: *"Chorda: some notes sound like some of their strings are
slightly out of tune, and I can tell quite clearly. E4, C#3, D3, C3, B2, E5. Alone, they sound
like one of the strings inside the key is out of tune. I haven't checked the other keys."* He
decided: no comparison against a recorded piano; measure each string of each note and adjust
its tuning; review all 88 keys.

**Where it stands.** Every unison of the 88 keys is now nearly beatless and every string alone
sits on its key's pitch, measured string by string: before, 70 of the 88 keys had a beat the
ear counts in five seconds on partials 1–3 (C#3's third partial beat 5.7 Hz, E4's 5.1, D3's
3.9); after, none does (the most is 0.75 of a swell in five seconds, C6 at pianissimo). The
sound change ships as `DSP_VERSION` 11 (ADR 0098's fifth amendment) — **but not before the
owner's ear**: his listening set and blind ABX are in `F:\projects\daw-r41\chorda-tune-listen\`
(below). Tuning the unisons together takes away part of the slow second stage a detuned
unison rang, so a third of the keys sustain several dB less after the first second (C3, E5,
and most of G4–A6); that is the first thing to listen for. The CPU check is still to run on a
quiet machine (the integrator's call). Everything here is measured on the model; nobody has
listened yet.

## Why round 41's first measurement passed what the owner heard

`docs/round41/chorda-strings.md` measured every loop alone and found everything "within
tolerance". It was right about what it measured and wrong about what to measure against:

- each loop was held to its **designed** partials, and the design scattered each unison's
  strings on purpose — a fan of about ±1–2 cents (`detune_scale`, `uni_skew`), and a
  stiffness scatter (`uni_b`, twice as wide in the bass) that moves the upper partials of
  each string apart. That scatter is enormous: on C#3 the three strings' third partials stand
  at −14.7, +2.5 and +10.0 cents from the curve (−14.7 against *designed* −14.71: "in
  tolerance"), so they beat 5.7 Hz. A real unison's strings are one wire; their inharmonicity
  differs by a percent or so, which moves a third partial by hundredths of a cent;
- the beat between the strings sounding together was never measured — only each string
  alone against its own design.

## How it was measured: each unison's modes (`chorda::modekit`)

- A key's strings, once the hammer has left them, are a linear system. Near each partial a
  unison rings as a few damped sinusoids — one per loop, as the bridge couples them (the
  in-phase mode that moves the bridge and decays fast, the opposed ones that barely move it
  and ring on: Weinreich's two stages). Any linear filter keeps their frequencies and decays.
- So each partial of the key's own bridge force (its voice rendered alone, before the
  sympathetic strings, the board and the room; the action's noises and the phantom partials
  off — none of them moves a string's frequency) is heterodyned to its designed frequency,
  low-passed by a Hann 8 periods long (at least 0.1 s) and decimated to 100 Hz, and fitted
  with damped exponentials by ESPRIT (Roy & Kailath 1989): the signal subspace of its Hankel
  covariance (a complex Hermitian Jacobi eigen-decomposition), the rotation between its
  shifted halves, its eigenvalues the poles; the amplitudes by least squares. Its self-test
  finds two tones 0.12 Hz apart and a third 30 dB down to a thousandth of a hertz.
- **Each string alone** is the same read with the other loops muted on the bridge (round
  41's felt, `Chorda::set_loops_kept`): one mode per partial, its frequency to 0.001 cent.
- **The unison together**: its modes per partial, their power-weighted pitch, and **the beat
  the ear counts** (`most_heard_beat`). A pair of modes swings the partial's level by
  `20·log10((A + m)/(A − m))` (its softer mode `m` against the partial's whole amplitude `A`);
  it is audible while that swing is at least 1 dB (about the least a slow beat is heard by)
  and the partial is within 40 dB of its peak; it is *a beat* once it swells and falls a whole
  time while audible in the first five seconds. Slower than one in five seconds, or heard for
  less than a cycle (a fast first stage over a slow second), it is a change of level.
- Velocities 0.3, 0.6 and 0.9. The frequencies do not depend on the velocity (the system is
  linear once the hammer has gone); which modes are excited, and how loud, does.
- Diagnostics (`chorda_tests_unison.rs`, ignored): `unison_map` (each key's loops alone and
  together on partials 1–3), `unison_modes` (every mode of a key), `unison_table` (one line a
  key at three velocities, and JSON for the page), `unison_levels` (each key's level over
  0–0.5, 0.25–1.25 and 1.25–3.4 s against `DSP_VERSION` 10's), `unison_search` and
  `unison_optimise` (below), `unison_retable` (the table), `tune_listen` (the renders).

## The bounds, from what the ear detects

| What | Bound | Why |
|---|---|---|
| The unison together, partials 1–3 | **no beat the ear counts in the first five seconds**: no audible pair of modes swells and falls a whole time (≥ 1 dB, within 40 dB of the partial's peak) | "One beat per several seconds" is the ear's beatless: a unison whose beat is slower than one in five seconds is heard as a slow change of colour, not a string out of tune. Five seconds is most of a middle-register note's life at mezzo-forte; in the treble the partials die sooner, so the same rule asks less of them — which is how a tuner hears the treble too. |
| Two strings of a unison, each alone, partials 1–3 | within **0.5 cent** of each other | "A fraction of a cent", as a fine technician leaves a unison: 0.5 cent is a beat every 7 s at C4's fundamental and every 2.3 s at its third partial, if the two sounded without their bridge. The horizontal loop (the same wire's other polarisation, 0.62 cent apart by design) is judged with the unison it rings in. |
| Each string alone, its pitch | its partials 1–3 as the ear weighs them within **0.75 cent** of its key's pitch (the curve and its table; a single-string key within the whole key's 1 cent), and the table itself within **0.65 cent** (it was 1.21) | Half a cent of its own and a quarter the table may leave between a string alone and the unison together (the in-phase mode's pull the bridge gives the unison and not the string alone; the board's and the room's share of the whole key's measure). |
| The whole key (round 40's gate, unchanged) | the ear's pitch within 1 cent of the curve to E6, 2.5 above; octaves and fifths as the curve's | `tests_tune_gate`. |

The search aims under the gates (0.75 of a swell, so a strike's variation never reaches one).

## What was out, and why

Measured on `DSP_VERSION` 10 (beta.15/16) with the kit above, every key at three velocities
(`unison_table`; the whole table is in `docs/round41/chorda-tune.html` and
`F:\projects\daw-r41\chorda-tune-data\before2.txt`):

- **The stiffness scatter** (`uni_b` 0.054, twice that at A0, fading out C5–C6) moved each
  string's upper partials by **tens of cents**: on C#3 the three strings' third partials stand
  at −14.7, +2.5 and +10.0 cents from the curve, so the unison's third partial beats 5.7 Hz.
  A real unison's strings are one wire — their inharmonicity differs by about a percent, which
  moves a third partial by hundredths of a cent. This was most of what the owner heard: C#3
  5.7 Hz, E4 5.1, D3 3.9, C3 1.1, B2 0.5 on partial 3.
- **The designed fan** (±1–2 cents, `detune_scale` 1.48, a skew of 0.7) on top: E4's strings
  alone at +0.17, +1.40 and +2.11 cents on their fundamentals, E5's at −1.17, −0.39, +0.74. A
  cent laid on a loop's fraction is a cent on every partial, so a third partial beats three
  times as fast as the fundamental — while the bridge, whose loss is the same on every partial,
  locks two strings only within a fixed number of hertz (Weinreich 1977).
- **The bridge's resonances** pulled the unison's in-phase mode up to 2 cents (the fourth
  amendment's cap) while its opposed modes, which barely move the bridge, stayed: a beat
  between the two stages. On A#1 and C2 a resonance lay right on the second partial and split
  the in-phase mode in two (C2: +4.8 and −7.0 cents, 0.9 Hz) — the phase formula gives no pull
  at a resonance's centre, so the cap never saw it.
- In the top octaves (C6–B7) the fan was already narrowed, but at 2–4 kHz even half a cent is
  1–2 Hz: C6 4.6 Hz, G#6 4.9, D#7 1.8 on partials 2–3.
- **70 of the 88 keys** had a beat the ear counts in five seconds (the bound below); the 18
  without one were most of the single strings of the low bass, a few two- and three-string
  keys of G#1–G2, and seven of the top nine.

## What was changed (`chorda::voicing::unison`, over the technician's pass; every knob neutral at 0 or 1)

| Knob | What it does | Shipped |
|---|---|---|
| `bridge.pull_hz` | the bridge resonances' pull on partials 1–3 held in hertz too — what the ear hears of it is the beat between the in-phase mode it moves and the opposed modes it leaves — and read across a ±2 % band, so a resonance lying on a partial is seen (`KeyStrings::set_bridge_tuned`) | 0.1 Hz (with the old 2-cent cap) |
| `uni_b.<key>` | each key's stiffness scatter scaled | 0.02–0.35 of `uni_b` (was 1): the upper partials still beat slowly on their own schedules, partials 1–3 no longer |
| `uni_tight.<key>` | the share of the `unison` control's first cent taken as the technician's: at the control's default (1) the key's strings are as close as its beat allows; each cent above it detunes as before (the jazz, honky-tonk and old-piano presets keep their character) | median 0.66; 0 where the designed fan was already beatless once the scatter went (most of the bass), 1 on C#3 |
| `uni_pair.<key>` | a three-string unison laid as two strings alike and a third apart, instead of a fan: two strings alike share one mode the bridge never hears, the third makes one slow mode the bridge locks to the strings' pitch — a second stage with no beat in it, where a fan of three rings two slow modes that beat each other | 35 keys (where it kept the level best) |
| `coupling.<key>` | the key's share of the strings' loss through the bridge: a unison tuned closer rings less of the slow stage, so less loss through the bridge lets it sustain | 16 keys, 0.2–0.6 |
| `uni_loss_top` | the loss scatter kept where the other scatters fade (tried; it changes no level here: the strings' own loss is small against the bridge's) | 0 |
| `tune` | the technician's table, re-measured on the tuned unisons (`unison_retable`, three secant rounds, A0–E6 within 0.04 cent); above E6 nought but B6, D#7 (+0.5) and A#7 (−0.5) | ±0.61 cent at most (was ±1.21) |

`unison_optimise` chose each key's values: every way of laying it the pass knows (fan or pair;
the stiffness scatter at 0.02, 0.05, 0.1, 0.2, 0.35; the bridge share at 1, 0.6, 0.35, 0.2),
each as wide as it can be (bisection on `uni_tight`) with fewer than 0.75 swells heard and its
strings alone within half a cent, and of those the one whose level over 0–0.5, 0.25–1.25 and
1.25–3.4 s is closest to `DSP_VERSION` 10's. D6 was then laid as a pair by hand (as a fan its
slow beat swung its level 21.6 dB, past round 40's recorded-grand ceiling of 20). The whole
pass is `tools/chorda-voicing/unison_pass.sh` (the optimiser ~1 h on 12 threads, the table a
few minutes, `ship_unison.py` writes the module).

## Every key, before and after

Fastest audible beat on partials 1–3 over velocities 0.3/0.6/0.9 (and the swells the ear
hears of a beat in five seconds — the bound is under one), the widest two strings alone stand
apart there, and the key's level change at the lab's mezzo-forte. Before: beta.15/16
(`DSP_VERSION` 10). The page `docs/round41/chorda-tune.html` draws the same.

| Key | strings | fastest beat on partials 1–3, before: Hz (swells heard in 5 s) | after | strings alone apart, before → after (Hz) | level change, 0–0.5 / 0.25–1.25 / 1.25–3.4 s (dB) |
|---|---|---|---|---|---|
| A0 (21) | 1 | 0.26 (1.3) | 0.04 (0.18) | 0.00 → 0.00 | -0.14 / +0.24 / +0.89 |
| A#0 (22) | 1 | 0.04 (0.2) | 0.04 (0.20) | 0.00 → 0.00 | +0.01 / +0.02 / +0.06 |
| B0 (23) | 1 | 0.10 (0.5) | 0.03 (0.17) | 0.00 → 0.00 | -0.13 / -0.23 / +0.43 |
| C1 (24) | 1 | 0.28 (1.4) | 0.08 (0.42) | 0.00 → 0.00 | -0.21 / -0.04 / +0.92 |
| C#1 (25) | 1 | 0.27 (1.4) | 0.07 (0.31) | 0.00 → 0.00 | -0.17 / -0.15 / +0.34 |
| D1 (26) | 1 | 0.19 (0.9) | 0.04 (0.19) | 0.00 → 0.00 | -0.19 / -0.20 / +0.30 |
| D#1 (27) | 1 | 0.21 (1.1) | 0.05 (0.27) | 0.00 → 0.00 | -0.14 / -0.03 / +0.72 |
| E1 (28) | 1 | 0.40 (2.0) | 0.08 (0.41) | 0.00 → 0.00 | -0.26 / -0.23 / -0.13 |
| F1 (29) | 1 | 0.08 (0.4) | 0.07 (0.37) | 0.00 → 0.00 | -0.00 / -0.01 / +0.01 |
| F#1 (30) | 1 | 0.13 (0.6) | 0.05 (0.27) | 0.00 → 0.00 | -0.02 / -0.05 / +0.08 |
| G1 (31) | 2 | 0.37 (1.8) | 0.14 (0.17) | 0.38 → 0.02 | +0.16 / +0.10 / -1.52 |
| G#1 (32) | 2 | 0.09 (0.4) | 0.06 (0.32) | 0.01 → 0.01 | -0.01 / -0.04 / -0.03 |
| A1 (33) | 2 | 1.03 (5.2) | 0.07 (0.36) | 1.04 → 0.05 | +0.14 / +0.26 / -1.25 |
| A#1 (34) | 2 | 0.64 (1.5) | 0.06 (0.26) | 0.04 → 0.02 | +0.28 / -0.00 / -0.74 |
| B1 (35) | 2 | 0.17 (0.5) | 0.11 (0.41) | 0.13 → 0.05 | +0.02 / -0.15 / +0.29 |
| C2 (36) | 2 | 0.95 (1.9) | 0.15 (0.75) | 0.41 → 0.05 | +0.35 / +0.07 / -0.55 |
| C#2 (37) | 2 | 0.17 (0.9) | 0.10 (0.51) | 0.21 → 0.06 | +0.01 / -0.35 / +0.09 |
| D2 (38) | 2 | 1.78 (8.9) | 0.14 (0.63) | 1.17 → 0.06 | +0.07 / -0.19 / -2.37 |
| D#2 (39) | 2 | 0.57 (2.9) | 0.10 (0.41) | 0.59 → 0.06 | +0.30 / +0.69 / -0.25 |
| E2 (40) | 2 | 0.35 (1.7) | 0.13 (0.63) | 0.04 → 0.06 | -0.07 / -0.03 / +2.19 |
| F2 (41) | 2 | 0.17 (0.8) | 0.07 (0.25) | 0.19 → 0.03 | +0.02 / +0.18 / +0.02 |
| F#2 (42) | 3 | 0.11 (0.4) | 0.10 (0.36) | 0.07 → 0.07 | -0.09 / -0.06 / -0.47 |
| G2 (43) | 3 | 0.10 (0.4) | 0.16 (0.48) | 0.14 → 0.10 | +0.06 / +0.24 / -0.39 |
| G#2 (44) | 3 | 0.48 (1.1) | 0.09 (0.25) | 0.26 → 0.09 | +0.16 / -0.18 / -2.80 |
| A2 (45) | 3 | 0.34 (1.6) | 0.09 (0.44) | 0.42 → 0.09 | +1.42 / +3.29 / -0.43 |
| A#2 (46) | 3 | 0.85 (4.3) | 0.10 (0.32) | 0.90 → 0.06 | +0.07 / -1.14 / -1.38 |
| B2 (47) | 3 | 0.50 (1.5) | 0.14 (0.39) | 0.52 → 0.11 | +0.43 / +0.70 / -1.84 |
| C3 (48) | 3 | 1.09 (5.5) | 0.10 (0.16) | 1.13 → 0.06 | +1.03 / -0.05 / -7.56 |
| C#3 (49) | 3 | 5.67 (28.5) | 0.17 (0.54) | 5.68 → 0.11 | +2.66 / +2.34 / -1.12 |
| D3 (50) | 3 | 3.91 (19.6) | 0.11 (0.53) | 3.92 → 0.11 | +3.09 / +3.36 / -3.23 |
| D#3 (51) | 3 | 0.83 (2.8) | 0.13 (0.28) | 1.02 → 0.13 | +0.98 / +0.90 / -5.87 |
| E3 (52) | 3 | 0.39 (1.6) | 0.10 (0.53) | 0.41 → 0.13 | +0.95 / +2.87 / -0.16 |
| F3 (53) | 3 | 0.51 (1.4) | 0.14 (0.42) | 0.48 → 0.15 | +1.07 / -1.19 / -2.21 |
| F#3 (54) | 3 | 3.01 (15.1) | 0.12 (0.63) | 3.03 → 0.15 | +1.94 / +2.90 / -0.26 |
| G3 (55) | 3 | 2.93 (14.7) | 0.12 (0.42) | 2.95 → 0.09 | +2.31 / +4.94 / -0.52 |
| G#3 (56) | 3 | 1.68 (8.4) | 0.09 (0.33) | 1.71 → 0.11 | +1.44 / +4.36 / -1.23 |
| A3 (57) | 3 | 2.61 (13.1) | 0.12 (0.55) | 2.64 → 0.19 | +0.20 / +0.48 / -5.18 |
| A#3 (58) | 3 | 1.59 (8.0) | 0.16 (0.48) | 1.62 → 0.20 | +0.58 / -0.04 / -5.29 |
| B3 (59) | 3 | 1.26 (6.3) | 0.12 (0.35) | 1.30 → 0.21 | +0.84 / +1.25 / -5.33 |
| C4 (60) | 3 | 1.27 (6.4) | 0.15 (0.73) | 1.29 → 0.16 | +1.99 / +4.89 / -1.21 |
| C#4 (61) | 3 | 4.45 (22.3) | 0.14 (0.59) | 4.46 → 0.15 | +2.39 / +5.83 / -0.62 |
| D4 (62) | 3 | 0.72 (3.4) | 0.19 (0.70) | 0.81 → 0.25 | +0.24 / +0.72 / -2.70 |
| D#4 (63) | 3 | 1.51 (6.0) | 0.19 (0.35) | 1.52 → 0.26 | +0.60 / +0.71 / -9.63 |
| E4 (64) | 3 | 5.14 (13.7) | 0.15 (0.74) | 5.17 → 0.24 | +0.82 / +0.83 / -4.69 |
| F4 (65) | 3 | 0.89 (3.2) | 0.16 (0.75) | 0.75 → 0.24 | +1.87 / +4.02 / -2.29 |
| F#4 (66) | 3 | 0.83 (2.5) | 0.21 (0.74) | 0.86 → 0.29 | +0.10 / +0.21 / -0.31 |
| G4 (67) | 3 | 2.45 (5.4) | 0.17 (0.74) | 2.73 → 0.28 | +0.84 / +2.11 / -10.73 |
| G#4 (68) | 3 | 3.93 (7.4) | 0.23 (0.20) | 4.04 → 0.36 | +0.78 / +0.88 / -5.70 |
| A4 (69) | 3 | 2.15 (6.1) | 0.11 (0.34) | 2.22 → 0.13 | +0.85 / +1.56 / -3.40 |
| A#4 (70) | 3 | 1.21 (2.5) | 0.19 (0.74) | 1.09 → 0.22 | +0.81 / +2.51 / -6.95 |
| B4 (71) | 3 | 0.71 (2.8) | 0.27 (0.73) | 0.85 → 0.43 | +0.71 / +1.86 / +2.32 |
| C5 (72) | 3 | 2.56 (5.1) | 0.27 (0.74) | 2.68 → 0.42 | +0.95 / +1.13 / -4.16 |
| C#5 (73) | 3 | 1.43 (2.4) | 0.13 (0.16) | 1.64 → 0.47 | +1.45 / +1.42 / -0.93 |
| D5 (74) | 3 | 2.05 (4.1) | 0.27 (0.74) | 2.25 → 0.43 | +0.20 / +0.34 / -3.48 |
| D#5 (75) | 3 | 1.09 (2.3) | 0.22 (0.75) | 1.24 → 0.41 | -0.41 / -2.02 / -1.86 |
| E5 (76) | 3 | 0.94 (2.6) | 0.22 (0.68) | 1.10 → 0.36 | +0.35 / +0.16 / -12.61 |
| F5 (77) | 3 | 2.91 (5.0) | 0.30 (0.74) | 3.05 → 0.51 | +4.06 / +3.10 / -6.62 |
| F#5 (78) | 3 | 1.64 (2.1) | 0.27 (0.74) | 1.44 → 0.45 | +0.21 / -0.93 / -10.44 |
| G5 (79) | 3 | 2.78 (4.6) | 0.33 (0.20) | 3.07 → 0.67 | +0.08 / -5.56 / -1.54 |
| G#5 (80) | 3 | 4.59 (5.7) | 0.42 (0.74) | 4.93 → 0.68 | -0.39 / -7.13 / -11.57 |
| A5 (81) | 3 | 0.95 (2.0) | 0.31 (0.74) | 1.28 → 0.47 | -0.15 / -3.61 / -5.21 |
| A#5 (82) | 3 | 3.65 (3.7) | 0.40 (0.18) | 4.07 → 0.80 | +0.28 / -9.63 / -4.46 |
| B5 (83) | 3 | 4.92 (4.5) | 0.58 (0.60) | 5.26 → 0.86 | +0.22 / -11.11 / +1.55 |
| C6 (84) | 3 | 4.57 (6.9) | 0.28 (0.75) | 4.97 → 0.45 | +0.10 / -13.50 / +0.45 |
| C#6 (85) | 3 | 3.75 (4.1) | 0.38 (0.20) | 4.24 → 0.96 | -0.56 / -5.70 / -4.45 |
| D6 (86) | 3 | 1.02 (3.8) | 0.08 (0.04) | 1.66 → 0.25 | +0.07 / -14.93 / -19.58 |
| D#6 (87) | 3 | 3.44 (3.3) | 0.41 (0.20) | 4.10 → 1.06 | -0.43 / -6.36 / -3.86 |
| E6 (88) | 3 | 3.23 (4.3) | 0.29 (0.10) | 3.96 → 0.81 | +0.32 / -7.86 / -0.23 |
| F6 (89) | 3 | 1.25 (2.6) | 0.36 (0.75) | 2.02 → 0.62 | -0.01 / -9.04 / -7.38 |
| F#6 (90) | 3 | 1.55 (2.4) | 0.42 (0.75) | 2.40 → 0.70 | -0.31 / -6.98 / -5.08 |
| G6 (91) | 3 | 2.04 (2.7) | 0.57 (0.74) | 3.16 → 0.99 | +0.04 / -11.74 / -6.76 |
| G#6 (92) | 3 | 0.71 (1.7) | 0.36 (0.75) | 1.23 → 0.63 | +0.02 / -7.04 / -5.40 |
| A6 (93) | 3 | 2.58 (1.4) | 0.37 (0.09) | 3.26 → 1.03 | -0.61 / -7.40 / +2.25 |
| A#6 (94) | 3 | 1.10 (1.7) | 0.48 (0.75) | 1.73 → 0.78 | -0.28 / -4.88 / -3.90 |
| B6 (95) | 3 | 1.82 (2.9) | 0.41 (0.08) | 2.94 → 1.16 | -0.90 / -3.75 / -5.64 |
| C7 (96) | 3 | 0.65 (1.0) | 0.49 (0.75) | 1.11 → 0.84 | -0.07 / -1.91 / -2.30 |
| C#7 (97) | 3 | 0.79 (1.3) | 0.50 (0.75) | 1.35 → 0.86 | -0.10 / -3.21 / -3.70 |
| D7 (98) | 3 | 0.82 (1.3) | 0.52 (0.75) | 1.39 → 0.88 | -0.13 / -3.33 / -3.51 |
| D#7 (99) | 3 | 0.48 (1.4) | 0.29 (0.75) | 0.82 → 0.49 | -0.16 / -5.76 / -1.99 |
| E7 (100) | 3 | 0.24 (0.0) | 0.24 (0.03) | 0.71 → 0.71 | +0.00 / +0.00 / +0.00 |
| F7 (101) | 3 | 0.37 (0.9) | 0.32 (0.75) | 0.64 → 0.55 | -0.01 / -0.97 / -0.47 |
| F#7 (102) | 3 | 0.39 (0.0) | 0.39 (0.02) | 0.62 → 0.62 | +0.00 / +0.00 / +0.00 |
| G7 (103) | 3 | 0.37 (0.0) | 0.31 (0.02) | 1.08 → 0.92 | -0.11 / -1.15 / -0.40 |
| G#7 (104) | 3 | 0.81 (1.5) | 0.33 (0.03) | 1.39 → 0.97 | -0.07 / +0.50 / -1.02 |
| A7 (105) | 3 | 0.43 (0.8) | 0.43 (0.75) | 0.69 → 0.69 | +0.00 / -0.05 / -0.13 |
| A#7 (106) | 3 | 0.71 (1.3) | 0.28 (0.02) | 1.21 → 0.84 | +0.47 / -1.31 / +1.06 |
| B7 (107) | 3 | 0.54 (0.7) | 0.54 (0.70) | 0.93 → 0.93 | -0.03 / -0.46 / -0.31 |
| C8 (108) | 3 | 0.74 (0.8) | 0.69 (0.75) | 1.24 → 1.16 | -0.00 / -0.40 / -0.01 |

## The six keys the owner named

| Key | before: beat, swells in 5 s | after | strings alone apart | how it is laid now | level change (0–0.5 / 0.25–1.25 / 1.25–3.4 s) |
|---|---|---|---|---|---|
| B2 (47) | 0.50 Hz on p3, 1.5 | 0.14 Hz, 0.39 | 0.52 → 0.11 Hz | pair, scatter ×0.2, 34 % of the fan | +0.4 / +0.7 / −1.8 dB |
| C3 (48) | 1.09 Hz on p3, 5.5 | 0.10 Hz, 0.16 | 1.13 → 0.06 Hz | pair, scatter ×0.1, 34 % | +1.0 / −0.1 / −7.6 dB |
| C#3 (49) | **5.67 Hz** on p3, 28.5 | 0.17 Hz, 0.54 | 5.68 → 0.11 Hz | pair, scatter ×0.02, tuned dead, bridge share ×0.2 | +2.7 / +2.3 / −1.1 dB |
| D3 (50) | **3.91 Hz** on p3, 19.6 | 0.11 Hz, 0.53 | 3.92 → 0.11 Hz | fan, scatter ×0.02, 26 %, bridge share ×0.2 | +3.1 / +3.4 / −3.2 dB |
| E4 (64) | **5.14 Hz** on p3, 13.7 | 0.15 Hz, 0.74 | 5.17 → 0.24 Hz | pair, scatter ×0.02, 32 % | +0.8 / +0.8 / −4.7 dB |
| E5 (76) | 0.94 Hz on p3, 2.6 | 0.22 Hz, 0.68 | 1.10 → 0.36 Hz | fan, scatter ×0.2, 26 % | +0.4 / +0.2 / −12.6 dB |

Others that were out as far: C#4 (61: 4.45 Hz, 22 swells in 5 s), F#3 (54: 3.0 Hz, 15), G3 (55:
2.9 Hz, 15), A3 (57: 2.6 Hz, 13), G#5 (80: 4.6 Hz), B5 (83: 4.9 Hz), C6–D#6 (84–87: 1–4.6 Hz).
The worst left after the fix: C6 at pianissimo (0.75 of a swell, 0.28 Hz on its third partial,
under the bound); the widest strings alone, G2's third partials 0.56 cent apart (the gate
allows 0.6: the search held 0.5 at mezzo-forte and a soft strike's fit reads a few hundredths
further); the string alone furthest from its key's pitch, E2's first, 0.72 cent (bound 0.75).

## What it costs the sound: the second stage

A unison's slow second stage (Weinreich's aftersound) rings only if its strings differ. In
`DSP_VERSION` 10 they differed by beating — the fan and the scatter — and much of what rang
after the first second was those beats. Tuned to within half a cent and no counted beat, a
unison keeps a weaker second stage. The optimiser kept each key's level as close to
`DSP_VERSION` 10's as the bounds allow: the attack (0–0.5 s) moved 0.9 dB rms (C#3, D3, F5:
+2.7 to +4.1 dB, where less loss through the bridge brought the sustain back). But over
1.25–3.4 s the change is 4.5 dB rms, and these keys lose 6 dB or more in a window: C3, D#4,
G4, A#4, E5, F5, F#5, G#5, A#5, B5, C6, D6, D#6, E6, F6–A6 (D6 −14.9 and −19.6, E5 −12.6, C6
−13.5 over 0.25–1.25 s). **This is the question for the owner's ear**: whether the tuned
unisons' shorter bloom in C3 and the fifth and sixth octaves is better or worse than the
beating sustain it replaces. If it is worse, the next step is a model of the second stage that
does not beat — a horizontal polarisation on the three-string keys (a fourth loop: CPU) or a
reactive bridge share per string — not wider unisons.

## Tests

- `chorda::modekit::tests::the_kit_finds_close_modes_to_a_thousandth_of_a_hertz` — the kit
  itself, on tones whose answers are known.
- `chorda::tests_unison` (normal suite, about 20 s on 8 threads):
  `every_unison_is_beatless_on_its_first_three_partials`,
  `the_strings_of_each_unison_alone_agree_within_half_a_cent`,
  `every_string_alone_sounds_its_keys_pitch`, `a_single_loop_rings_one_mode_per_partial`.
- `chorda::tests_tune_gate` (round 40's whole-key gate) holds on the new voicing: the ear's
  pitch within 1 cent A0–E6 (0.04 after the table), 2.5 above (worst D#7, 2.08); octaves and
  fifths; no unison deeper than a recorded grand's.
- The pins (`chorda_tests_voicing3.rs`): `MODEL_2_SHIPPED` re-made (`0x2dca_3cab_2f21_fbbc`);
  `DSP_VERSION` 10's voicing pinned by name (`Voicing::technician`, `0x9561_d8f8_a3ce_3a61`,
  its old shipped hash — every new knob at its neutral value changes no bit of it); candidate
  16, beta.14 and model 1 held;
  `the_shipped_voicing_is_the_technicians_with_the_unison_pass_and_a_new_unit_plays_it`.
- `cantus-ui` `chorda_tune_e2e::a_new_chorda_plays_the_six_keys_the_owner_named_without_a_beat`:
  a Chorda added the way the picker adds one plays E4, C#3, D3, C3, B2 and E5 through the real
  engine's render, each partial 1–3 swinging under 3 dB about its decay, where the same keys
  rendered on `DSP_VERSION` 10 by the DSP swing more on at least five of the six; the table
  check now reads the unison pass's.
- Counts: __COUNTS__
- No CPU cost per sample: the pass adds work only at a strike (the bridge's pull read at nine
  points across a resonance's band, on partials 1–3); the per-sample loop is unchanged.
  **Measured: not yet** — the integrator asked to keep CPU measurements until the machine is
  quiet (`measure_model_2s_sixty_four_pedal_down_voices` and
  `model_2s_sixty_four_pedal_down_voices_cost_at_most_twice_model_1`, best of 5, model 2,
  against master).

## The size of the change (`DSP_VERSION` 10 → 11)

| Render | max \|Δ\| | RMS of Δ | |
|---|---|---|---|
| Every other unit's goldens (`null_tests` `v5`, Pulsus's kits, the scene bakes, the null corpus) | 0 | 0 | none includes model 2 of Chorda: no golden moved (the whole `cantus-dsp` suite passes) |
| Model 1 (`tests_hash`) | 0 | 0 | pinned, held |
| Model 2 at beta.14's, candidate 16's and the technician's voicings | 0 | 0 | pinned, held |
| Model 2 at its default (the pinned 0.5 s phrase) | 0.393 | 0.0802 (−21.9 dBFS) | re-made: the phrase's RMS 0.2015 → 0.2180 (+0.68 dB); new pin `0x2dca_3cab_2f21_fbbc` |

Each key at the lab's mezzo-forte against `DSP_VERSION` 10 (`unison_levels`): 0.93 / 4.17 /
4.47 dB rms over 0–0.5 / 0.25–1.25 / 1.25–3.4 s (the table above, key by key).

Songs saved before play the tuned unisons if they use model 2 (render and freeze caches key on
the version); model 1 is untouched, to the bit. No schema bump: nothing in the file changes.
The `unison` control keeps its meaning above its default: each cent above 1 detunes as before
(the jazz, honky-tonk and old-piano presets keep their spreads, less the first cent).

## For the owner's ear

`F:\projects\daw-r41\chorda-tune-listen\`:

- `pares.html` — every pair side by side, named (not blind): `-before` is beta.16
  (`DSP_VERSION` 10), `-after` the tuned unisons, 24-bit 48 kHz, as rendered.
  - `<key>-held`: the key alone, mezzo-forte 7 s, then piano 5 s, then forte 5 s — the six he
    named (B2, C3, C#3, D3, E4, E5) and five that were out as far (F#3, A3, C#4, C6, D6).
  - `<key>-phrase`: the key, its octave below, a chord holding it, a short line around it
    ending held on it.
  - `named-keys`: the six, one after another.
- `abx/index.html` — the blind test (zip `abx/` to send it): 16 ABX trials over the pairs, then
  "which sounds more in tune?" for each, the reveal and the score (one-sided binomial p) only
  at the end; both versions of every clip at one RMS so a level never tells them apart; the
  answers kept in the browser and offered as a JSON file. Which file is which:
  `abx-key.json` (never in the kit).
- `unisonos.html` — every key's beat before and after (= `docs/round41/chorda-tune.html`).

What to listen for:

1. **The beat**: in C#3, D3, E4 and C#4 held, the wavering "one string out of tune" should be
   gone; in B2, C3 and E5 it was slower: listen for whether it is gone too.
2. **The bloom after the first second**: C3, E5, C6 and D6 ring less after the attack. Is the
   tuned sound purer, or deader?
3. **The phrases**: chords and octaves around each key — any key that now sounds out of tune
   *against its neighbours* (the whole-key gate says no: within a cent of the curve).

The questions: does the "string out of tune" leave the six keys? Is the shorter bloom
acceptable, or should the second stage be modelled before this ships?

## Verified only in emulation

All of it: measurements of the model with the kit above (ESPRIT on the voice's own bridge
force; the whole key through the instrument for the table and round 40's gate). Nobody has
listened. The kit was checked on synthetic tones; on a real piano it has not been run. No
recorded piano was compared (the owner's decision). The CPU check and the Linux check
(`tools/wsl/check.sh`) have not run from this worktree.

## Tools kept

- `crates/cantus-dsp/src/chorda_modekit.rs` — the modal kit (tests only).
- `crates/cantus-dsp/src/chorda_tests_unison.rs` — the gates and every diagnostic above.
- `tools/chorda-voicing/unison_pass.sh` — optimise, re-measure the table, write the module.
- `tools/chorda-voicing/ship_unison.py` — the spec and table → `chorda_voicing_unison.rs`.
- `tools/chorda-voicing/unison_page.py` — the before/after page.
- `tools/chorda-voicing/tune_abx.py` (+ `tune_abx_plain.html`, `tune_abx_kit.html`) — the
  plain pairs and the blind kit from `tune_listen`'s renders.

