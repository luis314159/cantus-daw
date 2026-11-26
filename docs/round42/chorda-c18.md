# Round 42 — `chorda-c18`: candidate 18, the top's mechanics dry

**Status: candidate 18 ships as the default (`DSP_VERSION` 18; 17 is the fiddle's, on
integration), awaiting the owner's named listen and his blind kit 18.** Branch `r42/chorda-c17`
(the same worktree) from 904acd372. The owner's blind ABX of candidate 17 (2026-11-26,
`chorda-abx17-verdict.md`) set the aims. ADR 0098's seventh amendment.

Every number below is measured against **both** recorded grands (the Salamander and the
bitKlavier, through the engine), as in candidate 17's pass. The probes are in
`tools/chorda-voicing/probes/c18/` (`mech.py`, `mech2.py`, `spec_png.py`, `qt.sh`, `table18.py`).
The lab build is `chorda_compare_c18c`, and the spec is `candidates/c18/spec.txt`
(`base=c17,…`).

## 1. The top's mechanics: why candidate 17 read as "a knock with reverb"

Measured under the fundamental of C7, D♯7, F♯7, A7 and C8, where no string of the key sounds
(`mech.py`: an 8th-order band 60 Hz – 0.7 f0, a 1 ms RMS envelope; `mech2.py`: the whole
spectrum with every partial masked; `spec_png.py`: the spectrograms):

- **The grands.** About 5–30 ms after the strike there is a broadband burst up to ~3 kHz, 4–6 dB
  under the note's first 60 ms. Its highs die first: 1–3 kHz falls 100–120 dB/s, and the
  spectrogram makes a triangle, wide at the strike and narrowing. A slow, quiet remainder follows,
  about 20 dB under the burst by 120 ms (the case and the recording room).
- **Candidate 17.** A 2 ms click at the strike, then its second stage (63 % of the peak,
  τ 0.16 s). At 5–20 ms it sits at −12 dB, so the burst is missing. It falls 55–65 dB/s at every
  frequency, and as much energy arrives after 30 ms as before (late/early +0.3 dB; the grands
  −6.8 / −1.6). That is a decaying noise of one colour, which is what a room sounds like. In the
  top phrase the knock was −9.5 dB of the whole, and −3 dB of the phrase's 100 Hz – 1 kHz.
- **Neither the room nor the body made it.** `room=0` moves it under 1 dB, and `knock=0` takes it
  all. It was the knock's own second stage, fitted in candidate 17 to the middle keys' noise
  between partials.
- **Not cues.** The envelope's roughness (Chorda's 10.8 dB against the grands' 3.2–3.6) is a
  1 ms window on low content. It is the same with every noise off.

**What candidate 18 does.** New knock parts, each nothing at its default:
- the first stage's rise (`knock.rise`);
- the slow stage's own low-pass (`knock.tail_hi`);
- the top keys' shape from C6 to its values at C7 and above (`knock.*_treble`).

The fit: a 15 ms burst rising over its first quarter, band to 2.7 kHz (×1.6), the slow stage at
0.75 of its share and 0.85 of its fall, under 1.5 kHz, and the noise law at C8 38 (31.6).
Trials A–N and T/U/V are in `qt.sh`'s output: a drier tail (U) undershot the grands' late level
by 8–10 dB, and a brighter band (T3c) overshot 4–8 kHz.

## 2. Middle C's give-away mid

- **The middle C phrase against both grands' span** (1/3-octave windows, loudness-matched):
  - partial 5 (1.31 kHz): +5.6 dB at 0.5–1.5 s and +10.4 dB at 1.5–3 s;
  - partial 2 (523 Hz): +6.5 / +5.0 dB;
  - 400 and 635 Hz, between the partials: +8 and +11 dB in the attack.
- **400 / 635 Hz.** That is the floor between the partials (the action's noise and the body). It
  lies at the bitKlavier's level, 10 dB over the Salamander's. It is not a peak, and it does not
  move with the bridge, the room, the sympathetic strings, the polarisation or the knock (each
  under 1 dB). It is left as it is.
- **Partial 5.** `partial5.60=-10` takes it into the span (1.3 kHz: +2.6 → −3.2 dB against the
  Salamander, the bitKlavier +0.2), and nothing else moves more than 0.2 dB.
- **Partial 2.** Cutting it −6 dB also took middle C's stereo correlation from 0.49 to 0.30 (the
  grands 0.52 / 0.86): that partial carries the centre of the image. It is left.
- **A global 500 Hz cut is wrong.** It would take the soft chords further under both grands at
  500 Hz (already −4 … −8 dB).

## 3. The lowest notes

- **The two first notes: a thump before the tone** (`atk.png` in the probes' method). A bass
  string's tone reaches the bridge 10–15 ms after the hammer.
  - Candidate 17's knock and blow stood there alone: the note began with a thump −20 dB under its
    peak, and its tone came 20–25 ms later.
  - The Salamander has a precursor 15 dB weaker; the bitKlavier none.
  - The rise (−20 to −3 dB of the peak), A0 C1 D♯1 F♯1 A1 C2: **c17 34 / 29 / 21 / 17 / 18 /
    12 ms**, the grands 5–13.
  - With knock *or* blow alone off, the first three were already in the grands' range.
  - `knock_bass` and `blow_bass` (both scaled at A0, log to C4) at 0.3: **11 / 9 / 6 / 5 / 9 /
    11 ms**.
  - The bass's noise between partials moves 2.10 → 1.56 (knockfit's bass score, 0 inside the
    span).
- **Inharmonicity A0–C2 (the open item), fitted on partials 2–30:**
  - A0 3.40·10⁻⁴ against 2.43 / 1.48;
  - C1 2.64 against 1.79 / 0.92;
  - D♯1 2.07 against 1.49 / 0.74;
  - F♯1 1.65 against 0.97 / 0.64.
  - Over both grands throughout, by 40–70 % over the Salamander. **Left, for the reason
    candidate 17 left it**: the technician's and unison passes' tuning tables rest on the
    keyboard's `bass_b`, and changing it means re-measuring both. The sound lab's own `B` feature
    is in band in 5 of 8 held-out groups.
- **The bass release's "4–8 kHz click" (c17's open item) is not a cue.**
  - Relative to the 100 ms before it, the release jumps +11 … +14 dB, against the grands' −2 …
    +5. But Chorda's floor before it is silent.
  - In absolute level the burst is −77 … −80 dB under the note, against the Salamander's
    recording floor (−84 … −86) and the bitKlavier's (−70 … −71): inside the grands' span.
  - It is the strings being damped, not the landing (`landing=0`, `sym=0` and `damper_t60` each
    move it under 0.5 dB).

## 4. ff_chord (Chorda in c16, the Salamander in c17)

- **Candidate 16 against 17 on the ff chord: no band moves more than 2 dB** in any window, except
  10–16 kHz at 0.5–2 s. There c17's knock adds noise, which brings it nearer both grands
  (−18.7 against −31 dB outside the span).
- **c17's knock is −13 dB of the chord's first 100 ms**, a thump under the attack, mostly from its
  bass keys (C1, C2). Candidate 18's bass law lowers it (the knock's share of the whole phrase
  −18.9 → −20.0 dB).
- **The chord's rise moves away from the grands**: 29 → 19 ms, against 31 / 39. The thump had
  started the envelope early. Listen for it.

## 5. The constant cue: the sound lab and the seven phrases

- **The sound lab** (`chorda-grand`, held-out third; runs `c17` and `c18` in
  `$CANTUS_WORK/soundlab/chorda-grand/runs/`): c18 against c17 is "same" on every feature except
  three, which are closer:
  - the noise between harmonics: 4 → 5 of 8 groups in band, 0.19 → 0.17 widths out;
  - the decay's T60: RMS out 2.44 → 2.31;
  - the centroid's attack: 14.5 → 13.8.
  
  Nothing is further.
- **`sig.py` on the seven phrases** (`$CHORDA_LAB/c18-sig.txt`):
  - the top phrase's flatness at 1–4 kHz is −13.4 → −12.8 (grands −11.5 / −9.8), and in the
    attack −10.2 → −9.7 (−8.9 / −7.3);
  - the bass's rise is 20.0 → 11.4 ms (12.5 / 6.3);
  - middle C's centroid is 509 → 499 Hz (484 / 393).
- **No regression where the owner calls them "almost equal"**: fast_run, soft_chords and ballad
  each move under 0.6 dB in flatness, under 3 Hz in centroid, and 0 in correlation.
- **The constant cue is still open.** Middle C's floor between partials and the top phrase's
  crest (27.3 against 21.7 / 23.8; it comes from candidate 17's top-key partial cuts, not from
  the knock) are what the lab still flags.

## Before → after (Salamander / bitKlavier)

| measure | c16 (unison pass) | c17 | c18 | grands |
|---|---|---|---|---|
| top keys' action noise, partials masked, outside the span (RMS dB) pp / mf / ff | 13.5 / 10.9 / 12.3 | 8.1 / 5.4 / 7.3 | 7.8 / 4.7 / 5.2 | 0 |
| burst 5–20 ms under 0.7 f0, mf (dB re note) | −11.8 | −12.1 | −9.2 | −6.4 / −3.9 |
| late 120–320 ms (dB re note) | −43.2 | −24.4 | −28.8 | −27.3 / −24.0 |
| decay 0.5–2.6 kHz (dB/s) | −170 | −63 | −109 | −118 / −102 |
| late/early 0.5–2.6 kHz (dB) | −6.8 | +0.3 | −1.9 | −6.8 / −1.6 |
| knockfit: fit keys / held C6–C7 | 10.3 / 12.8 | 5.4 / 6.7 | 5.2 / 6.6 | 0 |
| between partials: middle / top / bass | 9.5 / 12.9 / 1.5 | 5.1 / 3.0 / 2.1 | 4.9 / 2.8 / 1.6 | 0 |
| bass rise A0, C1, D♯1, F♯1, A1, C2 (ms) | 34 / 29 / 21 / 17 / 18 / 12 | 34 / 29 / 21 / 17 / 18 / 12 | 11 / 9 / 6 / 5 / 9 / 11 | 13 / 6 / 11 / 10 / 5 / 8; 5–6 |
| knock's share of top / ff_chord / fast_run / soft_chords (dB) | — | −9.5 / −18.9 / −23.8 / −33.5 | −9.2 / −20.0 / −23.6 / −33.9 | — |

## Listening

- **The named preview** (not blind): `F:\projects\cantus-work\listen\chorda-c18\escuchar.html`,
  in Spanish, top phrase first. It plays *1 candidato 17*, *2 candidato 18* and *3 Salamander* at
  −20 LUFS, as 16-bit WAVs in `wav16\` with the FLACs beside them.
- **Blind kit 18** (`abx5-f5de7037f4`, built at the coordinator's word):
  - Kit: `F:\projects\cantus-work\listen\chorda-abx18\`. Zip:
    `F:\projects\cantus-work\listen\ABX-Chorda18.zip` (23.4 MB; one folder; `LEEME.md` and
    `README.md` say how to take it and to send the results file to whoever gave it, so it stands
    on its own; no key inside). The key is beside it, in `chorda-abx18-key\`.
  - Kit 5's design: the seven phrases at −20 LUFS, ABX 16 trials against the Salamander,
    preferences on both pairs.
  - *Chorda 3* is candidate 18 as the app ships it (`chorda_abx --shipped`;
    `the_kits_shipped_chorda_is_the_apps` holds it to the picker's Chorda bit for bit). *Chorda 2*
    is candidate 17 through the lab (`--previous base=c17`).
  - The two catch questions are chosen this time, not drawn (`--catch middle_c,bass`; kit 17's
    were the ballad and the top): middle C is the Salamander against itself, the bass candidate
    18 against itself.
  - The page checked in Edge (`abx_browser_check.mjs`, `ABX_ONLY=edge,online`).

## Tests

- **`cantus-dsp --lib chorda`: 145 passed.** That includes the new pin (`0xb19b_545c_67c3_0ee9`),
  candidate 17's (`MODEL_2_C17`), every earlier voicing's, `tests_voicing5`, and
  `tests_voicing4`'s `pass` with every new part on (every block size and thread count, idle as
  new, rest).
- **`cantus-engine --test inst_chorda`: 14 passed.**
- **`cantus-ui`:**
  - `chorda_c17_e2e` and `chorda_model2_e2e` pass. A Chorda through the app knocks as a grand
    under C8 within −22 … −4 dB.
  - `chorda_abx`'s example tests (9) pass, among them `the_kits_shipped_chorda_is_the_apps`.
  - `preset_render`: every Chorda preset is inside its window (+0.7 … +1.6 LU). The test failed
    on CPU alone, under four parallel builds: Fragor's `synth_fuzz` and three Saltus presets, none
    of them Chorda's (beta.15's known flaky CPU gate). Rerun it quiet before calling it a
    regression.
- **Not run on this branch:** the WSL/Linux check, clippy over the workspace, the whole suite.

## Open

- The A0–C2 inharmonicity is over both grands; a keyboard change needs the tuning passes
  re-measured.
- Middle C's floor between partials (400 / 635 Hz) sits at the bitKlavier's level, 10 dB over
  the Salamander's.
- The top phrase's crest, and its 4/7/12 kHz partial peaks (c17's open item).
- The ff chord's rise, now faster than both grands'.
- Nothing is verified by ear yet.
