# Chorda candidate 5 vs the Salamander Grand — the second independent analysis (round 40)

> Kept with its scripts (this folder, `ana2/`; `../README.md`). The paths below are the round's:
> the lab is `CHORDA_LAB` now (`voicing/…` → `<lab>/…`, `compartir/…` → `CHORDA_SHARE`), the scripts
> are here. The figures and CSVs are not kept: the scripts make them again into `ANA2_OUT`.

Round 40, 2026-11-12. Second, independent analyst; audio only, no model code read. Numbers from
`scripts/` (venv `downloads/chorda/venv`, `python -I`); tables in `csv/`, figures in `figures/`.
Saved by the integrator from the analyst's hand-back (the tool refused the agent's file write).

## 0. Material and caveats

- Phrases: `compartir/chorda-preview-5/<phrase> - {3 candidato 5, 4 Salamander}.flac` (−20 LUFS);
  raw renders `voicing/lab4/wav/c5`, single notes `voicing/notes5/wav/{c5,salamander}/notes_{pp,mf,ff}.wav`
  (A0–C8 every minor third, 6 s apart, held 3.5 s, vel 30/75/115; layout `chorda_kit/phrases.rs`).
- Measures: LTAS 1/3 & 1/12 oct, spectrograms; inharmonic partial tracking + complex demodulation;
  beat depth and **coherence** (§2.1); attack rise, attack spectrum, knock/tone; sustain on string
  tone only (noise beside partials subtracted); release; stereo per band, ILD/ITD per key; tuning, B;
  velocity curves, evenness; pedal spectra; artefacts; listen-proxies (Zwicker loudness, DIN
  sharpness, roughness/fluctuation proxies — relative only).
- Caveat 1: the reference is a recording — rumble and hum under every note (§4) inflate broadband
  late levels; sustain/tails re-measured on string tone only.
- Caveat 2: never compare partial balances on the mono sum — the A-B pair puts some partials in
  anti-phase (Salamander C5 fundamental −10 dB in (L+R)/2). Partial tables add L and R power (`s21*`).

## 1. Ranked differences (NEW / CONFIRMS / DISAGREES vs the first report)

### 1. Beats move as one block — flanger/phaser (A1–A4) — CONFIRMS + NEW cause
- Chorda's partials of a note dip together (C4, A3, A4) or on t ∝ 1/n — one notch sweeping down
  (A1, D#2, D#3, F#3, C5). Salamander: each partial on its own schedule (shimmer, not swirl).
- Heard: `bass` A1 9–11.5 s (sweeps 0.3–3 kHz); `middle_c` partials ≥5 dip together ~0.65 s, swell by 1.8 s; `ballad`.
- Beat depth (median 8 strongest partials, L+R power): A1 18.7 vs 1.9 dB; D#2 19.9 vs 3.1; F#2 15.3 vs 3.4;
  D#3 17.0 vs 5.0; F#3 17.5 vs 4.3; A3 13.0 vs 4.9; C4–A4 15.5 vs 9–11; equal from C5 up.
- Flanger test (partials 4–16 on t·n; 1 = one notch): 0.55–0.72 vs −0.03…0.22 (A1, D#2, D#3, F#3, C5).
  Simultaneous-dip score A3 0.37, C4 0.40, A4 0.63 vs 0.05–0.07. Fluctuation proxy middle_c 6.3 vs 3.3,
  ballad 8.0 vs 5.3; roughness proxy middle_c 1.18 vs 0.70.
- Figures: beats_A1.png, beats_C4.png, beats_summary.png, spec_bass.png, spec_middle_c.png, psychoacoustic.png. Confidence high.
- Needs: each partial its own beat rate (beat frequency not ∝ n — no single cents detune per note);
  depth ≤ 5–6 dB in A1–A3, ≤ 10–12 dB in C4–A4; never > ~2 of the 8 strongest partials dipping at once;
  flanger score < 0.2 and simultaneous-dip < 0.1 on every key. A0–F#1 the opposite: no beating
  (0.1–0.2 vs 0.4–2.3 dB) — add a little slow movement. `out_c5`'s middle median (13.9 vs 11.7) hides it.

### 2. NEW — melody register C5–A5 octave-heavy: thin, glassy, too bright
- Partial 2 re fundamental at mf: C5 +2.1 vs −14.6; D#5 −4.4 vs −22.1; F#5 −4.6 vs −19.0; A5 −0.4 vs −19.9 dB.
  Partials 3–4 +14–16 dB too strong at mf, ~18 at ff. Sustain centroid (0.3–1 s, mf) 1360–1520 vs 650–940 Hz.
  `ff_chord` sharpness 0.84 vs 0.73; 2.5–5 kHz +5.0 dB, 5–10 kHz +4.2. Present since beta.14.
- A4 the other way (partial 2 10 dB weaker) → timbre break A4/C5.
- Figures: upper_mid_partials.png, dynamics_evenness.png, psychoacoustic.png; scripts s21_upper_mid_colour.py, s21c_check.py. High.
- Needs: C5–A5 partials 2–4 −12…−18 dB re fundamental (p2 ≈ −15…−20, p3 ≈ −25 at mf); blend into A4
  (p2 +8 dB) and C6 (already right). On C4 the first report's "partials 3–8 +8…+14" is likely a mono-sum
  artefact — per channel C4 p4–7 are 7–10 dB weaker (low confidence): DISAGREES.

### 3. Attack switches on 2–3× too fast from C3 up — CONFIRMS
- Rise −20 → −3 dB of the 1-ms envelope peak (median, mf; pp/ff same): C3–B3 4.0 vs 7.4 ms; C4–B4 3.0 vs 8.1;
  C5–B5 3.6 vs 8.3; C6–B6 2.1 vs 4.3; C7–C8 0.7 vs 4.4; A0–B1 reversed 17.6 vs 13.2.
- Figures: attack_waveforms_mf.png, csv/notes.csv `rise_20_3_ms`, s18_rise_summary.py. High (A-B smear explains ≤ 0.5–1.5 ms).
- Needs: ~6–8 ms in C3–B5, ~4–5 ms in C6–C8. Do NOT copy the 7–23 ms lead-in (sample latency).

### 4. Middle C and tenor too loud after the strike, and swell back — DISAGREES with "no plateau"
- `middle_c` re peak: 1 s −16.4 vs −20.9; 2 s −19.6 vs −27.2; 2.9 s −22.5 vs −29.6 dB; +1 dB swell 1.3–1.9 s.
  C3–B3 decay after 1 s −3.1 vs −6.3 dB/s; at 3.3 s −24.6 vs −28.3. middle_c 6.9 vs 4.8 sone at equal LUFS.
- Figures: phrase_contours.png, sustain_mf.png, middle_c_tail.png. High.
- Needs: C3–B4 after-sound (1–3 s) 5–7 dB lower re attack, ~−5…−6 dB/s, no swell (the swell is item 1).

### 5. Soft treble is a pitchless thud — CONFIRMS
- Knock (< 0.6·f0) re string tone, first 80 ms, pp: C7 +5.1 vs −6.6; D#7 +5.6 vs −2.9; F#7 +20.1 vs +4.1;
  A7 +15.7 vs +3.0; C8 +24.7 vs +9.3 dB. mf F#7 +9.2 vs −2.3. Attack centroid pp C7–C8 511 vs 1705 Hz;
  2–10 kHz share −21.7 vs −3.0 dB. pp D#6–F#7 4–9 dB too quiet.
- Figures: treble_knock.png, csv/treble_knock.csv. High.
- Needs: pp C7–C8 string tone +10–15 dB re knock (knock/tone ≤ 0 dB), knock darker and shorter.
  Leave mf/ff C6–F#6 (Chorda has less knock there: −22…−28 vs −11…−14).

### 6. Holes in the treble: D#6, F#7, A7 — NEW as named keys
- D#6 −7 dB under neighbours at mf (−6 pp, −5 ff) while the Salamander's D#6 is +3 → ~10 dB relative.
  F#7 −9 dB at mf; A7 −8 dB at ff. Elsewhere evenness 1.8–2.1 vs 1.9–2.9 dB rms.
- Figure: dynamics_evenness.png; s09_dynamics.py. High (only every third key sampled — check D6, E6, F6, F7, G7).
- Needs: each onto its neighbours' line at all three dynamics.

### 7. pp still too loud in tenor/middle — CONFIRMS
- pp→mf step: C3–B4 11.0 vs 14.4 dB (A3 11.0 vs 16.7; F#4 11.6 vs 16.0; A4 11.2 vs 14.8); A0–B2 10.5 vs 12.1.
  mf→ff matches (8.6–8.8 vs 8.8–9.3); treble matches. pp brightness fixed.
- Needs: vel 30 ~3–5 dB softer in C3–B4, ~2 dB in the bass; mf/ff unchanged.

### 8. Upper middle "plinks": D#5–A5 lose tone too fast in the first 0.5 s — partly DISAGREES with first report's size
- String tone re peak at 0.65 s: D#5 −16.7 vs −10.4; F#5 −19.7 vs −13.4; A5 −18.9 vs −9.8 dB; A5 at 0.4 s
  −12.5 vs −3.2. Slopes match after. At 3.25 s: C6 −52.5 vs −47.7; D#6 −40.7 vs −35.7; A6 −50.2 vs −42.5.
- Figures: treble_tonal_sustain.png, s19_tonal_sustain.py. Medium.
- Needs: D#5–A5 first 0.5 s lose 6–9 dB less (~−15 dB/s not ~−28); C6–A6 keep 3–8 dB more tone at 3 s.
  The remaining broadband treble gap (C6–B6 −64 vs −48 at 3.3 s) is the reference's rumble.

### 9. Key-up is a click + thud, not a damper sigh — CONFIRMS (new shape)
- Chorda: short broadband click + 60–120 Hz thump, 35–40 ms. Salamander: diffuse 100–700 Hz breath, 110–150 ms.
  Release noise peak re the note just before key-up: F#5 +10.4 vs −5.7; C6 +34.3 vs +5.2; D#6 +34.6 vs +5.0 dB.
  Re the note's attack comparable (−30…−41 vs −35…−47): shape and band are the problem.
- Figures: release_spectrograms.png, spec_middle_c.png (3.0 s), s13*. Medium-high.
- Needs: no instantaneous click; noise over ~100–150 ms centred 200–700 Hz, its 60–120 Hz part 15–20 dB
  lower; in C6–D#6 scaled to the note's current level (never > ~5 dB over it).

### 10. Bass strike lacks its bright hammer clang — CONFIRMS
- 2–10 kHz share in first 30 ms: A0–B1 mf −36.4 vs −27.5, ff −25.6 vs −18.1; C3–B3 mf −31.5 vs −24.5 dB.
  C2 attack 10–15 dB weak in 1–4 kHz re sustain. `bass` phrase: 1.25–5 kHz −3…−5 dB, 5–10 kHz +5 dB.
- Figures: attack_spectra_mf.png, ltas_bass.png. Medium (mono sum understates the reference).
- Needs: ~7–9 dB more 1–8 kHz in the first 20–50 ms of bass notes, gone in 50–100 ms; less above 5 kHz.

### 11. NEW — too much sub-bass under the lowest notes
- Fundamental re partial 2: A0 −4.5 vs −16.6; D#1 −8.7 vs −38.4; F#1 −14.2 vs −25.8 dB. `bass` 25–63 Hz
  thirds +6…+15 dB; `ff_chord` 31 Hz +14.6 dB.
- Figures: partial_profiles_mf.png, ltas_bass.png, csv/ltas_third.csv. High.
- Needs: 25–60 Hz of A0–F#1 10–15 dB lower, keep partials 2–6.

### 12. A C7 line keeps whistling after the soft trill — CONFIRMS
- `top` 5.9–7.5 s, string band (> 1.5 kHz) re phrase peak: 6.6 s −39.6 vs −54.6; 7.0 s −40.2 vs −64.8;
  7.4 s −46.1 vs −77.1 dB. Needs: repeated soft strikes on an undamped treble string must not pile up;
  fade to ~−65 dB within 1.1 s. Figures: phrase_contours.png, s20_top_gaps.py.

### 13. The middle a little narrow — low priority
- L/R correlation 1 kHz octave: middle_c 0.54 vs −0.21; soft_chords 0.44 vs 0.15; fast_run 0.40 vs 0.20;
  ballad 0.35 vs −0.12. Side−mid −3…−5 vs −1.7…+1.8 dB. Needs: 0.5–2 kHz decorrelated to ~0–0.2.

### 14. Tails below −40 dB — not a piano difference — DISAGREES with first report's P4.2 / "sterility"
- Tails match to −40 dB; below, the Salamander's hang is 60 % < 200 Hz rumble, 14 % on C4's partials.
  Nothing to copy; if wanted, a quiet decaying reverb tail.

## 2. Supporting detail

- §2.1 coherence test: per partial L+R envelope demodulated, quadratic trend removed; (a) mean pairwise
  correlation of residuals = simultaneous dips; (b) residuals resampled on τ = t·n then correlated =
  1/n notch (fixed-cents detune = flanger). A real piano ≈ 0 on both. Scripts s06_coherence, s07_sweep, s24_beat_depth.
- §2.2 listen-proxies (c5 / Salamander): middle_c loudness 6.86/4.83 sone, roughness 1.18/0.70,
  fluctuation 6.26/3.29; ballad fluctuation 7.98/5.34; ff_chord sharpness 0.84/0.73.
- §2.3 matches now (do not touch): tuning and B (3.4 cents rms over 30 keys; B ratio 0.98); bass formant
  (630 Hz +1.3 dB; A0 +22 dB at 596 vs +24 at 602 Hz); stereo panning/ITD/treble reflections; bass dampers
  and pedal tails; phrase balance, crest factor, LTAS of ballad/fast_run/ff_chord within ±3 dB below 2.5 kHz;
  no artefacts worth naming.

## 4. The reference's recording artefacts — keep out of the target

50 Hz hum +21 dB over neighbours (100 Hz +14, 60 Hz +11); rumble/room noise under every note (−10 dB re
note at C4, −3.6 at C7; Chorda −36…−65); hiss > 16 kHz; 7–23 ms sample lead-in (median 12); mono
incompatibility (C4/C5 fundamentals cancel ~10 dB in L+R); release-sample blips (ballad 16.6 s, ff_chord 7.1 s).

## 5. Listening order

1. `bass` 9–11.5 s (A1) and `middle_c` 0.5–2 s: swirl (1), loud after-sound (4).
2. `ballad` melody and upper `fast_run`: octave-heavy C5–A5 (2), hard attack (3).
3. `top` 4.4–7.5 s: soft-trill thud (5), whistling C7 (12); a D#6/F#7 scale for the holes (6).
4. `soft_chords` for pp (7); `middle_c` at 3.0 s for the key-up click (9).

## 6. Resumen (español)

El candidato 5 corrigió afinación e inarmonicidad, panorámica, apagadores del grave, colas con pedal y el
formante del grave. Lo que más se oye aún: (1) batidos en bloque, como *flanger* (La1–La4, 13–20 dB contra
2–5); (2) Do5–La5 con octava y duodécima tan fuertes como la fundamental (14–20 dB de más): fino y vidrioso;
(3) ataque 2–3× más rápido (3 ms contra 8); (4) Do central y tenor 7 dB demasiado fuertes a los 2–3 s y
vuelven a crecer; (5) agudos *pp* como golpe sordo y teclas hundidas (Re#6 −7, Fa#7 −9, La7 −8 dB). Después:
*pp* del medio 3–5 dB fuerte, el "plinc" de Re#5–La5, el clic grave al soltar, el ataque del grave sin brillo,
exceso de subgrave en La0–Fa#1, silbido del Do7 tras el trino. La cola de sala del Salamander es ruido y
zumbido: no se copia.
