# Round 42 — `chorda-c17`: Chorda's next voicing pass (candidate 17)

**Status: candidate 17 ships as the default (`DSP_VERSION` 16, schema 84) — awaiting the owner's
ear and his blind test.** Branch `r42/chorda-c17` from `r42/integration` a17fd209e, merged with
it at 79f17cbbb (arcus-expr: `DSP_VERSION` 15, schema 83). The owner's blind ABX of candidate
16 (2026-11-25, `chorda-abx16-verdict.md`) set the aims. ADR 0098's sixth amendment.

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


## 4. The bass

Measured on the held-out bass groups of the sound lab and the bass phrase: the partials' colour
lies in the grands' spread; what stands out is the clean space between partials (as everywhere,
fixed by the knock), the inharmonicity of A0–G♯1 (B 2.8–3.1·10⁻⁴ against the grands'
0.7–2.0·10⁻⁴: round 40's `keyboard.bass_b` 1.3 followed the Salamander's tuning stretch, and the
technician's and unison passes are built on it — left, see Open), the attack's peak later
(61 against ≤ 56 ms) and the centroid a little low. The knock helps the first and the last.

## 6. Colours a user can reach

`colour` (Output card, beside *Tilt*; −1 … 1, 0 Chorda's own): the Salamander's middle phrases
minus candidate 17's, octave by octave and median over middle C, the soft chords, the run and the
ballad — `SHAPE` = +2.9 / −2.6 / +0.6 / +3.2 / +2.6 dB at 250 / 500 / 1 k / 2 k / 4 kHz: the
recorded grand has less 500 Hz and more 250 Hz and presence. At 1, the middle phrases' octaves
stand on the Salamander's (by construction; the end-to-end test holds the knob to the shape on a
held middle C through the engine). The preset *Recorded Grand Colour* (`colour` 1, the concert
grand's other values); *Concert Grand* keeps Chorda's own. `hardness` (the felt) and `persp`
(the microphones) remain the other two ways to colour it, as the owner listed.

## What ships (`chorda::voicing::c17`, over the unison pass)

`partial3.59/60/61`, `partial6.60` (middle C's nasal partials); `key_return=1`; the knock:
`knock.noise=300.42` (×31.6 at C8), `tail=0.63` (τ 0.16 s), `tau=2.1 ms`, band 90 Hz – 1.66 kHz,
`noise_vel=-0.375`, `knock_treble=0.0258`, `t60=1.2`, `knock_vel=0.8`; `damper_t60=1.6`;
`landing.hi=500`; the top keys' cells `partial2.90/91/95/97/99/100`, `partial3.97`.

## Before → after (shipped `DSP_VERSION` 15 → candidate 17; Salamander / bitKlavier)

| measure | c16 (shipped) | c17 | grands |
|---|---|---|---|
| sound lab, noise between partials (median, band widths out; per-band RMS out) | 1.17; 12.5 dB | 0.22; 5.0 dB | 0 |
| sound lab, harmonics in band; centroid; attack peak (widths out) | 72 %; 0.063; 0.136 | 76 %; 0.016; 0.067 | — |
| middle C's third partial, dB vs the Salamander, 0–0.08 / 0.08–0.3 / 0.3–0.8 / 0.8–1.6 s | +8.6 / +13.1 / +25.2 / +13.8 | −0.7 / +1.9 / +14.1 / +2.7 | bitKlavier −12.9 / +2.2 / +9.9 / −6.9 |
| the run's in-note 125 / 250 Hz vs the Salamander | −16.2 / −11.5 | −7.4 / −6.7 | bitKlavier +6.8 / +5.6 |
| top keys' knock (0 = inside the grands' span), fit keys / held C6–C7 | 10.3 / 9.7 | 5.4 / 6.7 | 0 |
| noise between partials 0.32–0.82 s, middle / treble (dB re note) | −39.0 / −35.0 | −23.9 / −16.3 | −27.4 / −21.3; −20.2 / −14.1 |
| pedal tail at 0.5 / 1 s | −35.5 / −61.8 | −31.7 / −50.0 | −30.5 / −45.1; −16.3 / −30.4 |
| top keys' release (sub-fundamental, dB re note) | none (−80 … −97) | −21 … −34 | −32 … −45; −12 … −22 |
| spectral flatness 4–12 kHz, middle C / top phrase | −39.8 / −30.8 | −32.1 / −29.2 (c17d) | −25.9 / −24.1; −16.2 / −21.1 |

## The method, and what the analysis before the blind test changed

1. Signature (`sig.py`, sound lab) → the clean space between partials.
2. The knock model (Rust, neutral at its default) and its fit (`ksearch.py`: `knockfit.py`'s
   top-key envelope + the middle's between-partials, 8/16 kHz one-sided against hiss, held-out
   C6–C7 and the bass), four searches (k1–k4): k1 found hiss (no HF guard, one-pole band), so the
   band went to 24 dB/oct and the score gained the guard.
3. Middle C's cells, the top keys' cells (`keys_mf` every key against both grands).
4. The owner asked for a full difference analysis (`report17.py`:
   `F:\projects\cantus-work\listen\chorda-c17nalisis\`, Spanish `resumen.html`): it found the
   knock's 70 Hz ring (its modes at the fit's bound, ×4), the late case ring short, the pedal tail
   short, the run's 60 Hz, the bass release click; v3 and v4 fixed what was physical.

## Listening

Named previews (not blind), each with `escuchar.html` in Spanish and 16-bit WAVs:
`F:\projects\cantus-work\listen\chorda-c17\` (v1, c17c: had hiss), `v2\` (c17d), `v3\` (c17e),
`v4\` (c17g = what ships but for D♯7/A♯6, in no phrase). The owner on v2: "Va sonando muy bien".
**Blind kit 17** (built 2026-11-26 at the coordinator's word, the owner having asked for a new
ABX after the analysis): `F:\projects\cantus-work\listen\chorda-abx17\` (zip
`F:\projects\cantus-work\listen\ABX-Chorda17.zip`, 24.3 MB, no key inside), the key beside it in
`chorda-abx17-key\`. Kit 5's design (`abx5-50f5d1b224`): the seven phrases, ABX 16 trials against
the Salamander, preferences on both pairs, two catch questions (ballad, top). "Chorda 3" is
candidate 17 **as the app ships it** — a new Chorda's own spec through the engine's offline render
(`chorda_abx --shipped`; the test `the_kits_shipped_chorda_is_the_apps` holds it to the
instrument picker's Chorda bit for bit on middle C); "Chorda 2" is candidate 16 as it shipped in
`DSP_VERSION` 11–15 (`--previous base=unison`, through the voicing lab: its strikes' variation is
seeded by the phrase's note order where the engine seeds by the clip note's id — the same piano
drawn anew, as round 40 found) — so the kit also says whether 17 beats 16. The browser check:
190 checks pass in Edge; Chrome would not launch on this machine while in use, Firefox is not
installed (4 launch failures, none of the page's).

## Kit and level checks

- `preset_render`: every Chorda preset inside its ±2 LU window at candidate 17 (+0.7 … +1.6);
  *Recorded Grand Colour* read +2.09 at the concert grand's gain, trimmed 4.8 → 4.1 dB.
- `cantus-engine --test listen`: 14 passed (334 s under three parallel builds — the run killed
  earlier had not hung, only crawled).

## Open

- The top phrase's 4/7/12 kHz peaks (+10…+20 dB over both grands; c16 had them; the fix would cost
  the top's dynamics he liked) and its 300–500 Hz (+12; not the knock — every action noise off
  moves it < 1 dB; c16 had it).
- The bass releases' 4–8 kHz click (+9 against ≈0): half the felt, half the strings being damped.
- A♯6's third partial and D♯7's second still stand over both grands (the unison gate).
- A0–G♯1's inharmonicity over both grands (a keyboard change: the tuning passes rest on it).
- The soft chords' air above 8 kHz (−10…−15; the owner prefers Chorda there).
- Not verified by ear yet: everything. The blind kit after his named listen.
