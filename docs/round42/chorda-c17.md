# Round 42 — `chorda-c17`: Chorda's next voicing pass (candidate 17)

**Status: in progress.** Branch `r42/chorda-c17` from `r42/integration` a17fd209e. The
owner's blind ABX of candidate 16 (2026-11-25, `chorda-abx16-verdict.md`) set the aims.

## The lab this pass measures with

- **Two recorded grands, not one.** Every measure stands Chorda against the Salamander Grand
  (a Yamaha C5) *and* the bitKlavier Grand (Princeton; CC BY 4.0, stage lid ORTF), both played
  through the engine as the picker installs them (`chorda_compare render … bitklavier=pack:…`,
  new in this pass). Where the two grands agree and Chorda does not, the difference is
  Chorda's own; where they disagree, it is a piano's choice, not a target. Scores count a
  cell only outside the grands' span (their mean ± half their difference, at least 2 dB).
- **The shipped voicing as the base.** `base=shipped` in a lab spec reads it over what a new
  Chorda plays (candidate 16 with the technician's and the unison passes), not over beta.14.
- **The sound lab's subject** `tools/soundlab/subjects/chorda-grand`: 180 reference notes
  (both grands, A0–C8 every minor third, the layer each grand's SFZ plays at velocity 30 / 75 /
  115), the held-out third by a seeded hash; the candidate's notes split from the lab's
  single-note renders.
- Probes in `tools/chorda-voicing/probes/c17/` (below).

## 5. The timbre's constant cue (why 16/16)

Measured first, because it explains the others.

- **Chorda is too clean between its partials.** The sound lab's `noise` feature (the energy
  more than 0.2 f0 from every partial, 0.05–0.55 s after the strike) puts the shipped voicing
  **outside the two grands' band in 8 of 8 held-out groups, every register and dynamic,
  always on the clean side**: its harmonic-to-noise ratio 25–40 dB where the grands' band is
  −8 … 30 dB (e.g. the middle at mf 37.5 against 21–30; the top at mf 27.2 against −8 … 6).
  No other feature is out of band so consistently (harmonics 72 % in band, centroid 85 %).
- **What lies between the partials, and when** (`between.py`, C3–C5 at mf, dB re the note's
  first 100 ms): the grands carry a broadband bed — +4 … +9 dB at 62–250 Hz and −8 … −21 dB at
  1–4 kHz in the first 100 ms, falling slowly over a second; the shipped Chorda's is 10–20 dB
  lower in 0.02–0.32 s and gone 20–35 dB sooner after. Its own tail is the strings alone.
- **On the phrases** (`sig.py`, all seven at −20 LUFS): the spectral flatness between 4 and
  12 kHz is the lowest of the three sources on every phrase (5–14 dB under the more tonal
  grand), and in the attack's 1–4 kHz on five of seven (the top phrase −14.4 dB against −8.9 /
  −7.3). The partials' own colour, by frequency per register (`colour.py`), lies inside the
  grands' spread within ±5 dB: the cue is not the partials' balance.
- **Its cause in the model**: the action's knock (round 40's thud) was a low, short click — two
  modes at 72 and 185 Hz and a noise burst so small (`KEY_NOISE`) that ×100 of it moved the
  top keys' output by −44 dB — and nothing else in the model makes sound between the partials
  once the strings have taken the blow (with the action's noises off, `noise=0`, the top
  keys' sub-fundamental band barely moves). A grand's knock rings through its case, keyframe
  and board for a quarter of a second, and its middle keys carry it under their partials.

## 3. The top octave's action

Under a top key's fundamental nothing of its own strings sounds, so what is there is the
action (`knockfit.py`: A7–C8, short Blackman-Harris frames so the fundamental cannot leak in,
dB re the note's first 60 ms):

- the grands: the 0.5–2.8 kHz band −5 … −10 dB from 5 to 20 ms, −12 … −25 at 40–80 ms,
  −28 … −38 at 160–320 ms; the 60–180 Hz band rising to −14 … −16 dB at 20–80 ms (the key
  bottoming and the case);
- the shipped Chorda: the low band +10 … +19 dB too strong in the first 10 ms and 13–25 dB too
  weak after 40 ms; the mid band 6–13 dB too weak at 5–20 ms and too short;
- the release (`action.py`): the grands' top keys come up 50–60 dB under their note in the
  sub-fundamental band; Chorda's undamped keys make no sound at all on release (−80 … −97).

## 1. Middle C's mids

- **Cause found: middle C's third partial (785 Hz).** Every key at mf (`keys_mf`, 0.05–1.05 s,
  each key against its partials' total): the shipped Chorda's C4 holds its third partial
  +10.0 dB over the Salamander's and +13.4 over the bitKlavier's, C♯4 +8.2 / +12.0, B3
  +3.5 / +6.5, while its neighbours A3, D4, D♯4 lie at −2 … −11. In the middle C phrase itself
  (`parts.py`): +8 / +21 dB in the attack, +25 / +15 at 0.3–0.8 s. 785 Hz is the nasal region,
  and the grands both keep C4's third partial 20–30 dB under its second. Neither a softer or
  heavier hammer, nor the bridge's share, the board's level, the register EQ at 1 kHz or the
  partial table (zeroed C4–B5 since candidate 14) moves it more than 3 dB: it is the key's own
  excitation. Candidate 16 alone (before the technician's and unison passes) had it too (+21
  at 0.3–0.8 s).

## 2. Fast runs

- The run note by note (`run.py`, each note's 25–110 ms, octaves against its own 125 Hz–1 kHz):
  the shipped Chorda **−16 dB at 125 Hz and −11.5 at 250 Hz** against the Salamander (−22 /
  −17 against the bitKlavier): under the run's fundamentals (C4–C6) the grands carry the
  case's knock, Chorda nothing — "no mids". Its 2–8 kHz is not over the grands'; the metallic
  impression is the clean, low-less balance and the empty space between the partials.

## Open

(being written)
