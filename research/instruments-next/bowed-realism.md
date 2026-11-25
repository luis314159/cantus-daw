# What makes a bowed string sound real: a study for Arcus (cello, double bass, viola, violin)

*Round 42, 2026-11-24. The owner asked: "download more sounds to compare, and study how to
generate or describe the sound mathematically so we have something excellent and close to real
sounds"; then, "con estudios y comparación de viola y violín". This study delivers **data and
knowledge**, not engine code. The `arcus` builder fits its model to it.*

| Part | What | Where |
|---|---|---|
| 1 | the references, each with its licence | `F:\projects\daw-r42\downloads\study\` |
| 2 | the measured reference database | `F:\projects\daw-r42\study\refdb\` (JSON, plots, `README.md`) |
| — | the scripts | `F:\projects\daw-r42\study\tools\` (the integrator moves them to `tools/arcus/study/`) |
| 3 | the mathematics, ranked | this file |
| — | the ABX listening set | `F:\projects\daw-r42\downloads\study\abx-refs\` |

The method follows Chorda's against the Salamander (`docs/round40/chorda.md`): matched loudness,
measured windows, a band of real instruments to land in, and the owner's ear last. One thing is
new: every feature is taken **across several independent instruments**, so Arcus is fitted to
the family rather than to one cello.

## 0. The short version

**What was gathered.**

- 13 recording sources: 7 independent cellos, 5 basses, 8 violins and 4 violas, about 5 100
  analysed notes and 485 phrases. Of these, CC0 or CC BY (shippable): Karoryfer, Smolken,
  VSCO, TinySOL, Good-sounds, HSD and CNSM. The rest are measurement only.
- Measured data, all CC BY 4.0:
  - three violins' bridge admittances (CNSM);
  - friction at the bow on cello strings (Lampis);
  - bridge forces over bowing grids (Lampis);
  - torsion of a bowed cello string (Mores);
  - cellists' bow kinematics (Pàmies-Vilà).
- No open cello or bass admittance exists that we could find.

**What the database says, in one line per family** (medians across instruments, §2):

| Family | A0 | A0 ÷ lowest open string | Hill (after detrending) | Response 10 dB down at | Bow noise re harmonics (≥ 5·f0) | Vibrato |
|---|---|---|---|---|---|---|
| Cello | **105 Hz** (94–132) | 1.61 | 1.9 kHz (0.9–2.4), +6 dB | 3.0 kHz | −35 dB (−37…−30) | 5.7 Hz ±9 c |
| Bass | **70 Hz** (62–74) | 1.70 | 1.3 kHz (0.35–1.6), +6 dB | 2.8 kHz | −33 dB (−38…−25) | 5.4 Hz ±7 c |
| Viola | **250 Hz** (223–250) | **1.91** | 2.2 kHz (2.0–2.8), +8 dB | 3.8 kHz | −32 dB (−37…−32) | 5.8 Hz ±16 c |
| Violin | **281 Hz** | 1.43 | 2.6 kHz (2.1–4.0), +7 dB | 6.0 kHz | −32 dB (−37…−27) | 5.3–6.0 Hz ±8–18 c |

Across all four families:

- **Dynamics.** pp → ff raises the level 13–18 dB and the centroid only **0.1–0.35 octave**.
- **Attack.** Real players reach their level in 0.1–0.3 s (soft) and 0.03–0.08 s (loud).
- **Legato.** A finger change takes **~35 ms** (p10–p90 10–105 ms) with a **3 dB** dip. A
  détaché bow change dips **7–11 dB** and is noisier.
- **Vibrato AM.** Each partial swings **±1.5–2.2 dB** (the fundamental ±1.0–1.3). About half
  the partials swing against the pitch.

**The ranking.** Each item is ranked by perceptual impact × feasibility (§3 gives the details).
The **effect** column is what it should change; the **check** column is the Part 2 measure that
shows it worked.

| # | Do this | Effect | Check (inside the band of real instruments) |
|---|---|---|---|
| 1 | **Measured bodies per family.** The bass gets its own (A0 70 Hz, peaks ~100–120 and ~170–240 Hz, hill ~1.3 kHz), no longer "the cello ×1.72". The cello's hill and upper slope come from the data. The violin is fitted to CNSM's three measured admittances; the viola is sized, not tuned (§3.1, §3.6, §3.13). Move model 2's after-body EQ into the body. | The largest timbre gain, and the vibrato's AM becomes right, because the partials sweep real modes | `body` curve; A0 and hill marks; per-partial vibrato AM |
| 2 | **Re-map the dynamic and the attack the players' way.** pp → ff: force ×4–5, bow–bridge distance ×0.4–0.5, speed about constant. Then a **level and force swell of 0.1–0.3 s** at soft velocities (0.03–0.08 s at loud) on top of a clean Helmholtz start (§3.4). | Phrases breathe; ff is louder without turning harsh; notes no longer "switch on" | centroid slope 0.02–0.07 oct/step; level slope 2.7–3.5 dB/step; `t_rise` per dynamic |
| 3 | **Bow noise as measured.** Shape it by the HNR curve (cello 26 → 4 dB from 0.5 to 8 kHz), pulse it with the slip (Chafe), raise it **3–6 dB in soft playing**, and inject it at the junction (§3.2) | The "bow on string" cue, the clearest realism cue in A/B tests of models | `hnr_band_db` per dynamic; `noise_burst` at the attack |
| 4 | **Legato and bow changes from the phrases.** A slur takes 20–40 ms of finger glide (now 3 ms) with a 2–4 dB dip. A re-bow dips 5–11 dB with a 30–80 ms aperiodic burst (§2.10) | Real phrasing; legato no longer sounds like a pitch switch | `phrases.jsonl` transitions: `trans_ms`, `dip_db`, `ap_max` |
| 5 | **The upper strings at 2–4× rate.** Run the violin's string loop at 192 kHz (×4) and the viola's at 96 kHz (×2) with a fixed polyphase decimator, so the bow–bridge segment keeps ≥ 5 samples. That segment is d·f_s/(L·f_open): **2.2 samples** on the violin E at 48 kHz (§3.13) | Lets the violin and viola pass the same gate as the cello | the Schelleng and Guettler gate on violin E/A and viola A |
| 6 | **A real pizzicato.** Per-partial T60 as measured (cello 4–5 s at 100 Hz → 0.8 s at 3 kHz; violin 1.3–1.9 → 0.4 s), a finger **snap** (+12–22 dB of 1–6 kHz in the first 25 ms), finger damping on stopped notes, stiffness B (§3.9, §3.10) | Closes the 2 kHz centroid gap `arcus-match.md` left open | `pizz_t60_by_freq`, `pizz_thump_excess_db`, `pizz_B` |
| 7 | **Thermal friction, rate-and-state** (Woodhouse & Galluzzo 2025), behind the switch: a θ with conduction, f = A·h(v)·Y(θ) (§3.3). The measured cello G rosin is not a velocity curve: 0.24 median error in μ, with a hysteresis loop in the thermal direction | Attacks and the "grain" of light bowing closer to the bowing-machine data | `attack.t_steady_s`, `noise_burst`; the Guettler gate |
| 8 | **Vibrato judged by its AM, plus correlated bow motion and per-cycle variation** (§3.5). Depth by register and dynamic (cello ±8 → ±13 c from soft to loud; viola wider). Bass rate ~5 Hz (Mick 2024: 5.17 Hz ±19 c) | The warmth that Mellody & Wakefield found lives in the AM | `vibrato.am_db[1..20]`, the AM phases, rate and extent per register |

Further down the ranking: sympathetic open strings (§3.11), finger damping and frequency-
dependent string losses (§3.9), the finite-width bow (§3.7), torsion's Q (§3.8, a playability
parameter), directivity and the microphone (§3.12), and jitter (§3.14).
## 1. The references (Part 1)

Everything sits under `F:\projects\daw-r42\downloads\study\`, one folder per source, each with a
`SOURCE.md`: the URL, the licence quoted, the date, and the verdict. Four folders point to
copies the earlier builders made, rather than duplicating them. **No audio is in any
repository.**

### 1.1 Recordings

**Use** is one of: **ship** (CC0, or CC BY with credit); **measure** (measurement and listening
only); **measure, lawyer to ship** (a non-CC "without restrictions" wording).

| Folder | Instruments (independent ones counted) | What | Format | Licence | Use |
|---|---|---|---|---|---|
| `iowa-cello-bass`, `iowa-violin-viola` | 1 cello, 1 bass, 1 violin, 1 viola | arco and pizz, pp/mf/ff, every string, chromatic runs; **anechoic**, no vibrato | AIFF 16/44.1 | "may be downloaded and used for any projects, without restrictions" | measure, lawyer to ship |
| `tinysol-cello-bass`, `tinysol-violin-viola` | 1 of each family (IRCAM) | ordinario, pp/mf/ff (Vc, Cb also p, f), every string | WAV 16/44.1 | CC BY 4.0 | ship (credit) |
| `karoryfer-bigcat-cello` | 1 cello (K. Borowiak) | sustains p/mp/mf/f, up and down bow, no vibrato; pizz | WAV 24/44.1 | CC0 | ship |
| `smolken-bass` | 1 bass (1958 Rubner, fifths tuning) | arco pp–ff, up and down; pizz | WAV 32f/44.1 | CC0 | ship |
| `vsco-2-ce`, `vsco-solo-violin` | 1 bass, 1 violin (solo); cello, viola and violin **sections** | SusVib/SusNV/Pizz (bass); ArcoVib p/f, Pizz (violin) | WAV 44.1 | CC0 | ship |
| `good-sounds`, `good-sounds-violin` | 2 cellists, 2 violinists (MTG-UPF) | single notes p/mf/f, tasto/ponticello, attack faults, vibrato | WAV 24/48 | CC BY 4.0 | ship (credit) |
| `hsd-string-quartet-k465` | 2 violins, 1 viola, 1 cello (each recorded alone, close MKH 800) | Mozart K465: a 141 s excerpt and phrases, legato and vibrato. **The only freely licensed solo viola playing found** | WAV 24/48 | CC BY 4.0 | ship (credit) |
| `philharmonia`, `philharmonia-violin-viola` | 1 per family (several players) | arco at 0.25–1.5 s, pp–ff, molto-vibrato, non-vibrato; **phrases** (legato, détaché, martelé, portato, tremolo…); pizz | MP3 (lossy) | "must not be sold or made available as is" | measure |
| `phenicx-anechoic` | 1 per family (Aalto takes, denoised) | Mozart, Beethoven, Bruckner and Mahler parts, **anechoic**, with note annotations | WAV 16/44.1 | audio "academic research"; annotations CC BY-NC-SA | measure |
| `aalto-anechoic-orchestra` | the same players, every desk | the same parts | MP3 192k | "free for academic research" | measure |
| `freesound-cc0-previews` | 4 players (cello improvisations, bass pizz) | lossy previews; the originals need a login | MP3 | CC0 per file | measure (ship the originals only) |
| `cnsm-violin` | 3 violins (Klimke 2012, Levaggi 2012, Stoppani 2018) | 97 scale recordings by several players, plus the admittances below | WAV 16/48 | CC BY 4.0 | ship (credit) |
| `abx-refs` | — | the loudness-matched listening references (§1.3) | WAV 24/48 | per source | — |

**Independent instruments per family:**

| Family | Count | Which |
|---|---|---|
| Cello | **7** | Iowa, IRCAM, Karoryfer, Good-sounds ×2, Philharmonia, HSD |
| Bass | **5** | Iowa, IRCAM, Smolken, VSCO, Philharmonia |
| Violin | **8** | Iowa, IRCAM, VSCO, Good-sounds ×2, Philharmonia, HSD ×2 |
| Viola | **4** | Iowa, IRCAM, Philharmonia, HSD |

PHENICX/Aalto and the CNSM violins add more, used for phrases and the body check. The
Freesound previews are not analysed.

### 1.2 Measured data

| Folder | What | Licence |
|---|---|---|
| `cnsm-violin/admittances` | **21 bridge admittances of three violins**: hammer on the G side, vibrometer on the E side, before and after six months of playing. `f,H_real,H_imag,…`, 0–25.6 kHz, Δf 1.56 Hz (two at 0.39 Hz) | CC BY 4.0 (Zenodo 18696786; arXiv 2605.18305) |
| `zenodo-lampis-friction-force` | the friction force reconstructed at the bow and the string velocity on cello G strings (Helicore, Dominant) on a robotic monochord; β 0.028–0.20, F_b 0.25–5.9 N, v_b 5–20 cm/s; 109 recordings | CC BY 4.0 (Zenodo 15879128) |
| `zenodo-lampis-bridge-force-static-dynamic` | 168 four-channel 125 kHz bridge-force recordings and `features.csv`: β, v_b, F_b, f0, spectral centroid | CC BY 4.0 |
| `zenodo-lampis-bridge-force-attack` | 3 948 bridge-force attack recordings over Guettler grids, four string types | CC BY 4.0 |
| `zenodo-mores-torsion-*` | transverse displacement and torsion of a bowed cello G string (Chromcor, 121 N), 48 kHz, bowed 4–16 cm from the bridge; optical marker tracks | CC BY 4.0 |
| `zenodo-pamies-vila-*` | a cellist's and a robot's bow kinematics (motion capture 240 fps, 5 bow markers, nut and bridge) with audio; two cellists' contemporary-bowing sessions (ordinario legato and détaché, tasto, ponticello, col legno) | CC BY 4.0 |
| `string-tension-references` | 109 cello string rows (Aitchison chart, lb at 700 mm → N) | range check only |
| `papers/` | the open PDFs read for §3 (Woodhouse's euphonics.org copies, Mores 2019, Percival 2013, Desvages 2018, Askenfelt 1982, Zhang 2016, Schoonderwaldt 2008, Guettler's notes, Woodhouse & Galluzzo 2025) | their own |

**Searched for, not found:**

- an open **cello or double-bass admittance** or radiation dataset (Zenodo, DataCite with
  Figshare and Mendeley; Cambridge's Apollo answered 503 throughout);
- URMP's licence: the set sits behind a form;
- a trustworthy public-domain solo cello or bass recording on Internet Archive or Musopen.

### 1.3 The ABX listening references (for the owner)

`F:\projects\daw-r42\downloads\study\abx-refs\<family>\`, with `README.md` and `index.json`.
Every file is mono 24-bit 48 kHz at **−23 LUFS** (BS.1770-4, checked to 0.01 LU on the
standard's sine). The families:

| Family | Single notes | Phrases |
|---|---|---|
| Cello | A3 at pp/mf/ff (IRCAM), A3 at p/mf/f (Karoryfer) | Philharmonia legato, PHENICX anechoic Mozart, HSD K465 |
| Bass | D2 at pp/mf/ff (IRCAM and Smolken) | Philharmonia legato, PHENICX anechoic Mozart |
| Viola | D4 at pp/mf/ff (IRCAM) | Philharmonia legato, PHENICX anechoic Mozart, **HSD K465 (CC BY)** |
| Violin | A4 at pp/mf/ff (IRCAM), A4 at p/f (VSCO) | Philharmonia legato, PHENICX anechoic Mozart, HSD K465 |

The dynamics are loudness-matched, so only their timbre differs. Render Arcus's notes at
−23 LUFS to compare.
## 2. The measured reference database (Part 2)

`F:\projects\daw-r42\study\refdb\`. The definitions, how distributions are taken, and the
extractor's validation are in its `README.md`. Each number below is the median across
instruments, with the range across instruments in brackets.

### 2.1 Coverage

| Family | Arco notes (instruments) | Pizz notes | Phrase files |
|---|---|---|---|
| Cello | 1 079: Iowa 170, IRCAM 291, Karoryfer 122, Good-sounds 32 + 67, Philharmonia 397 | 191 | 76 (Philharmonia, PHENICX, HSD) |
| Bass | 1 009: Iowa 141, IRCAM 309, Smolken 139, VSCO 54, Philharmonia 366 | 195 | 78 |
| Viola | 937: Iowa 271, IRCAM 309, Philharmonia 357 (+ HSD in phrases) | 256 | 63 |
| Violin | 1 090: Iowa 291, IRCAM 284, Good-sounds 73 + 68, VSCO 30, Philharmonia 344 | 329 | 268 |

VSCO's sections are measured apart (`section_apart`). The extractor was first validated on
synthetic notes (`validation.json`, the table in the README):

| Feature | Error |
|---|---|
| f0 | ≤ 0.6 cent |
| harmonics | ≤ 0.4 dB |
| vibrato extent | ±0.1 cent at 98–880 Hz; +1.1 cent at 41 Hz |
| noise | +0.4 … +1.1 dB at 98–880 Hz; **+3.9 dB at 41 Hz with vibrato** |
| pizzicato B | < 0.01 % |
| pizzicato T60 | median 0.4 % |

### 2.2 The body (harmonic-peak method)

`body.py`. Every arco note's harmonics, a_n + 20·log10(n) (Helmholtz's 1/n removed), are fitted
as c_note + H(f) on a 1/12-octave grid by alternating medians. Plots:
`plots/<family>_body.png` and `plots/families_body.png`.

| Family | A0 | A0 ÷ lowest open string | Main wood peaks | Hill (detrended), height | Falls 10 dB (re 200–4000 Hz) by | Trend wood → hill |
|---|---|---|---|---|---|---|
| Cello | 105 Hz (94–132) | 1.61 | ~200–210, 300–420, ~560 Hz | 1.9 kHz (0.9–2.4), +6.3 dB | 3.0 kHz | −2 … −7 dB/oct |
| Bass | 70 Hz (62–74) | 1.70 | 100–118 (T1), 170–240, ~400–420 Hz | 1.3 kHz (0.35–1.6), +6.2 dB | 2.8 kHz | −4 … −8 dB/oct |
| Viola | 250 Hz (223–250) | **1.91** | ~350, 420–470, 710–790 Hz | 2.2 kHz (2.0–2.8), +8.0 dB | 3.8 kHz | −4 … −6 dB/oct |
| Violin | 281 Hz (3 of 3 that reach it) | 1.43 | ~445, 500–560, ~890 Hz | 2.6 kHz (2.1–4.0), +7.3 dB | 6.0 kHz | −0.4 … −5 dB/oct |

- **The cello** agrees with Zhang 2016's measured A0 ≈ 97 Hz and its 200/209/281 Hz modes, and
  with Fides's 103 Hz. The hill sits higher than Fides's 1 kHz.
- **The bass** agrees with Askenfelt's A0 ≈ 65 Hz and T1 ≈ 110 Hz. Its peaks are not the
  cello's divided by 1.72.
- **The viola's** A0 is 1.9× its lowest string: the size compromise (§3.13). Its response
  above 2.5 kHz falls a half-octave earlier than the violin's.

**The method checked against measured admittances** (`cnsm.py`, `cnsm_validation.json`,
`plots/cnsm_violins.png`). The CNSM set has three violins, each with hammer admittances
(21 measurements) and 828–1 424 scale notes:

- **The low modes are recovered within one grid step**: A0 265 Hz on the admittance against
  281–297 Hz from the recordings (A0 is an air mode, and radiates more than it moves the
  bridge); B1 530 against 561 Hz.
- The estimate falls **3.6–4.3 dB/octave faster** than the admittance. That is source roll-off
  × radiation × microphone. After removing that tilt, the 1/3-octave curves agree at
  **r = 0.73–0.83**.
- The detrended hill's height agrees within 1.5 dB (+5.0…6.3 against +4.8…6.8). Its frequency
  is within −0.17…+0.6 octave.
- **So:** use the estimator for the modes' positions, the hill's height and the band. Compare
  a model **through the same estimator**, never against an admittance directly.

### 2.3 Bow noise

From the residual of the tracked harmonic model, corrected for the noise the projection
absorbs (`noise_bands`, `plots/<family>_ltas_noise.png`).

Harmonic-to-noise ratio per third octave, all dynamics (dB):

| Family | 0.5 kHz | 1 kHz | 2 kHz | 4 kHz | 8 kHz | Total noise re harmonics |
|---|---|---|---|---|---|---|
| Cello | 26 (20–40) | 21 (17–28) | 20 (15–24) | 13 (9–16) | 4 (3–10) | −34.7 (−36.9…−29.6) |
| Bass | 20 (11–25) | 16 (9–24) | 11 (6–22) | 4 (0–16) | −4 (−4…6) | −32.9 (−38.3…−25.4) |
| Viola | — | 20 (16–28) | 21 (16–23) | 14 (10–14) | 6 (2–8) | −32.2 (−37.1…−31.7) |
| Violin | — | 32 (14–40) | 20 (15–28) | 15 (14–21) | 10 (8–17) | −32.0 (−37.2…−27.4) |

- **Soft against loud.** At 4 kHz the HNR is 3–6 dB lower in soft playing: cello 8.6 → 11.6 dB,
  bass 1.8 → 5.5, viola 10.7 → 15.3, violin 10.3 → 16.5. Light bowing is relatively noisier.
- **Per dynamic step**, the total noise moves only −0.2…+0.8 dB re the harmonics, while the
  level rises 2.7–3.5 dB a step. The noise grows with the bowing, about in step with the tone.
- **Caveats.**
  - Iowa and PHENICX are anechoic; the others are rooms.
  - Philharmonia is MP3: its noise is an upper bound. Iowa's `res_over_floor_db` gives the
    margin over the recording floor.
  - The −5…−25 dB of the bass's 4–8 kHz partly carries the rooms. `arcus-match.md` reached the
    same conclusion.

### 2.4 Harmonics, centroid and dynamics

- **Harmonics.** Partials 1–40 per family × octave × dynamic, dB re the strongest
  (`harmonics_db_re_max`, `plots/<family>_harmonics.png`). Examples, all dynamics:

  | Note range | Partials 1–5 (dB re strongest) |
  |---|---|
  | Cello C3–B3 | 0, −7.7, −10.8, −15.9, −17.0 |
  | Bass E2–D♯3 | 0, −8.8, −15.4, −19.6, −23.3 |
  | Viola C4–B4 | 0, −6.4, −11.9, −13.1, −12.0 |
  | Violin G3–F♯4 | −0.7, −4.2, −8.3, −12.7, −13.7 |

  In the lowest octave of the cello and the viola, the 2nd partial is strongest: the
  fundamental lies below A0.
- **The centroid** of the harmonics, in harmonic numbers: cello 2.4 (1.7–3.2), bass 1.5
  (1.4–3.0), viola 2.1 (1.8–2.1), violin 2.3 (1.7–2.6).
- **Within a note**, pp → ff per dynamic step:

  | Family | Centroid (oct/step) | Level (dB/step) |
  |---|---|---|
  | Cello | +0.046 (−0.11…+0.11) | +2.7 |
  | Bass | +0.041 (0…+0.07) | +3.0 |
  | Viola | +0.024 (−0.01…+0.05) | +2.7 |
  | Violin | **+0.069 (0.057–0.099)** | +3.5 |

  Over pp → ff that is +0.1 to +0.35 octave of centroid and 13–18 dB. The sources' levels are
  not calibrated, so read the level per step within one instrument.
- **The source, without a body** (Lampis's robotic cello A string, `bowing.json`, R² 0.93): the
  bridge-force centroid rises **+0.11 octave per doubling of force**, falls **−0.23 per
  doubling of bow speed**, and rises +0.32 per doubling of β (β 0.10–0.20 there). "A slower bow
  sounds brighter" (Guettler) holds. Force alone explains little in that narrow force range
  (0.04–0.57 N).
- **Mode-locking.** Arco partials sit within **0.07–0.33 cent** of n·f0 (`partial_dev_cents`),
  while the same strings plucked have B = 1e-5 … 7e-4. A bowed model needs no stiffness in its
  steady tone.

### 2.5 The attack

| Family | Rise to steady − 3 dB | Spectral shape settled (`t_steady`) | Noise burst, first 50 ms (bias +5…9) | Build order ρ |
|---|---|---|---|---|
| Cello | 0.23 s; soft 0.21, **loud 0.03** | 0.35 s (loud 0.19) | +24 dB | −0.23 |
| Bass | 0.11 s; loud 0.08 | 0.28 s | +20 dB | −0.19 |
| Viola | 0.09 s; loud 0.07 | 0.31 s | +24 dB | −0.21 |
| Violin | 0.31 s; soft 0.44, **loud 0.06** | 0.51 s | +25 dB | +0.05 |

- Real players' notes **swell**: 0.1–0.45 s at soft and medium dynamics, 0.03–0.08 s at loud.
- In the lower three families the upper partials reach their level slightly *before* the
  fundamental (ρ < 0).
- The attack carries ~10–17 dB more noise than the steady note, after the method's bias.
- Arcus starts within 5–25 ms (`arcus-match.md`). That is correct for the Helmholtz motion's
  start and too abrupt for the level (§3.4).

### 2.6 Vibrato

On notes with vibrato (Iowa, Karoryfer, Smolken and Good-sounds play without it):

| Family | Rate | Extent | Extent soft → loud |
|---|---|---|---|
| Cello | 5.7 Hz (5.2–6.2) | ±9.2 cents (6.2–12.5) | ±7.9 → ±12.6 |
| Bass | 5.4 Hz | ±6.7 cents | |
| Viola | 5.8 Hz | ±16 cents (10–22) | ±11.5 → ±19 |
| Violin | 5.3 Hz | ±8.1 cents (4–16, sample libraries) | |

In **repertoire** (`phrases.jsonl`):

| Recording | Rate | Extent |
|---|---|---|
| PHENICX violin (anechoic) | 6.0 Hz | ±18 cents |
| HSD violin | 5.4 Hz | ±9 cents |
| PHENICX viola | 5.5 Hz | ±13 cents |
| HSD viola | 5.1 Hz | ±8 cents |
| PHENICX bass | 4.6 Hz | ±11 cents (few notes) |

**The AM per partial** (`vibrato_am_db_per_partial`): the fundamental moves **±1.0–1.3 dB**,
partials 2–12 **±1.5–2.2 dB**, in every family. 45–50 % of partials swing against the pitch
(Fletcher & Sanders' opposite phases).

Arcus measured "1.5–7.5 dB" per harmonic at ±22 cents. Compare through `vibrato_stats` (a
±dB sinusoid) at the references' extent before concluding.

### 2.6b The vibrato cycle by cycle (round 42, pass 5)

Measured by `tools/arcus/pass5/vibrato.py` on every single-note arco file of the catalogue
(the sections and the non-vibrato takes left out; 3 102 notes, 1 976 with a vibrato of 8 cents
or more), and on Philharmonia's legato and crescendo phrases. The study's tracker (YIN over
two periods, hop 2.5 ms), the drift taken out over two vibrato periods, the cycles cut at
alternate extrema. **Extents are peak to trough** (the ± figures of §2.6 are the
sinusoid-equivalent half: about a third under half of these, the cycles' median of their
larger half). Medians, [p25, p75]; the full table, by register and by long, short, slurred and
crescendo notes, is `bowed-refdb/vibrato.json`.

| Family | Rate soft / medium / loud (Hz) | Extent soft / medium / loud (cents p-p) | Extent by octave 0 → 3 | Onset (s) soft / loud | Rise (s) | Rate jitter | Extent jitter |
|---|---|---|---|---|---|---|---|
| Cello (551) | 5.0 / 5.1 / 5.5 | 19 / 26 / 32 | 17 → 27 → 29 → 34 | 0.26 / 0.20 | 0.18 [0, 0.47] | 0.114 [0.076, 0.17] | 0.123 [0.092, 0.167] |
| Bass (403) | 4.6 / 5.0 / 5.35 | 15 / 16.5 / 21 | 14 → 15 → 20 → 21 | 0.44 / 0.18 | 0.24 [0, 0.99] | 0.23 [0.15, 0.33] | 0.27 [0.20, 0.38] |
| Viola (510) | 5.5 / 5.5 / 5.7 | 27 / 29 / 44 | 30 → 32 → 32 → 41 | 0.36 / 0.20 | 0.33 [0, 0.51] | 0.131 [0.079, 0.22] | 0.127 [0.091, 0.172] |
| Violin (512) | 4.9 / 5.1 / 5.5 | 19.5 / 22 / 38 | 25 → 24 → 25 → 38 | 0.41 / 0.21 | 0 [0, 0.55] | 0.152 [0.098, 0.23] | 0.144 [0.110, 0.204] |

- **Rate** rises with the dynamic (+0.2 to +0.7 Hz soft → loud) and a little with the register
  (about +0.1 Hz an octave on the cello, less on the others); within a long note it barely
  drifts (−0.04 Hz/s median, every family).
- **Extent** follows the dynamic strongly — loud 1.4 × (bass) to 1.9 × (violin) soft — and
  the register (wider high), as MacLeod 2008 and Mick 2024 report (§3.5). *Within* a note,
  cycle by cycle, it does not follow the level: Pearson's r of extent against level is −0.08
  (cello), −0.01 (viola), −0.11 (violin), −0.57 (bass; the level falls as the vibrato grows
  in after the attack); in Philharmonia's crescendo phrases −0.3. Players set the width by the
  dynamic they play at, not by the swell under the bow.
- **Onset**: a note starts with little or no plain part (the ±3-cent plain start is 0–0.05 s
  at the median) and reaches a quarter of its extent at 0.2–0.25 s, later when soft (0.26 –
  0.44 s) than loud (0.18 – 0.21 s); full 0.2–0.3 s after that. Short notes (under 1 s) vibrate
  faster and wider (5.7–6.0 Hz, 38–48 cents: they are played with intent), slurred ones (the
  cello's 13 phrase notes) at 5.8 Hz and 31 cents with the vibrato carried through the slur.
- **Shape**: nearly sinusoidal — the rising half takes 50–52 % of a cycle (asymmetry +0.02
  cello, −0.01 violin, 0.0 viola and bass), the second harmonic of the pitch track 20 dB under
  its fundamental (13 dB on the bass, whose tracks are the noisiest).
- **Centre**: not measurable this way — the plain start's pitch is the attack's own glide
  (the "centre" medians of 14–60 cents are that glide, not a vibrato's offset); at soft
  dynamics, where the start is long enough, the cello's vibrato sits 1.3 cents under it and the
  violin's 1.6 over: centred, as Brown & Vaughn's listeners heard (the perceived pitch at the
  mean of the swing).
- **Jitter**: cycle-to-cycle, the period changes by 11–23 % of itself and the extent by 12–27 %
  (the mean |change|) — no vibrato is metronomic. The tracker's own noise is in these figures
  (the bass's most: its tracks are the longest-windowed); the model is measured by the same
  estimator, so it cancels.
- **The timbre flicker** (`am_db`, §2.6, the same notes): the partials 1–10 swing ±2.3 dB
  (cello), ±2.0 (bass), ±2.3 (viola, violin) at the vibrato's rate, half of them against the
  pitch.

**The literature beside it.** Rates of 5.5 Hz for violinists and cellists alike (Geringer &
Allen 2004, abstract as cited by later work), 5.26 Hz for the TinySOL cellos and 6.48 Hz in
commercial concerto recordings (Hu et al. 2025, arXiv 2512.18162, its own YIN measure; their
depths are half-widths), a professional soloist's 6.6 Hz and 63 cents (a four-violinist pilot
study, University of Illinois), 0.2–0.35 semitone peak to peak in general (Meyer 1992, cited by
Hu et al.); the double bass 5.17 Hz (Mick 2024). The sample libraries and studio takes measured
here sit at the slower and narrower end of that — the performers of the concerto recordings
vibrate faster. Prame's method (1994, 1997, for singers) is the one used: rate and extent per
cycle, between alternate extrema.

**Arcus (pass 5, measured by the same estimator, long notes over three registers, three
dynamics and four seeds):** cello 5.19 Hz, 25.6 cents, onset 0.17 s, jitter 0.143 / 0.137;
bass 4.76, 15.8, 0.30 s, 0.241 / 0.261; viola 5.41, 24.2, 0.12 s, 0.145 / 0.134; violin 5.13,
23.3, 0.19 s, 0.199 / 0.119 — each inside the family's p25–p75 (`arcus_tests_vib.rs`). Its
flicker comes from the body as the string's length moves (verified: ±0.5–1.5 dB, half the
partials against the pitch), at about half the players' depth — the measured bodies are
smoother than real ones (§3.1); said so, not added by hand.

### 2.7 Pizzicato

| Family | T60 of the fundamental | T60 with frequency | B (by octave up the string) |
|---|---|---|---|
| Cello | 3.0 s (2.8–3.1) | 4–5 s at 60–125 Hz, 2.5 s at 250 Hz, 1.1 s at 1 kHz, 0.8 s at 2.5–3 kHz | 1e-5 → 2e-4 |
| Bass | 2.7 s (2.3–3.2) | ~3 s at 80–200 Hz, 2.1 s at 400 Hz, 1.6 s at 1 kHz, 1.2 s at 2 kHz | ~2e-4 |
| Viola | 1.0 s | 1.3 s at 200–300 Hz, 0.95 s at 500 Hz, 0.7 s at 1 kHz, 0.5 s at 2 kHz | 1.5e-4 → 1.6e-3 |
| Violin | 0.93 s | 1.3–1.9 s at 200–300 Hz, 0.8 s at 500 Hz, 0.66 s at 1 kHz, 0.4 s at 3–4 kHz | 1.8e-4 → 6.8e-4 |

B grows up the fingerboard as the string shortens, B ∝ 1/L².

**The thump/snap** (`pizz_thump_excess_db`): the first 25 ms over 60–160 ms.

| Family | Excess | Peak |
|---|---|---|
| Cello | +8 … +22 dB above 200 Hz | ~3 kHz |
| Bass | +4 … +11 dB | 1.6 kHz |
| Violin | +8 … +23 dB | 3–6 kHz |
| Viola | −8 … +8 dB | 3 kHz |

That broadband snap is most of why the recordings' plucks measure a 2 kHz centroid.

### 2.8 The release

The decay after the bow leaves: cello 39 dB/s, bass 29, viola 26, violin 28, with ranges of
2–110 dB/s across sources. Some players lift the bow, others stop on the string; the rooms add
tails; some sample files are trimmed. This is the weakest feature: use it only as a sanity band,
and Iowa (anechoic, natural lifts) as the primary.

### 2.9 Friction (Lampis, cello G strings)

`friction.json`, `plots/friction.png`:

- The best velocity-only (hyperbolic) curve is μ_d 0.59, μ_s 0.90, z₀ 0.2 m/s, with a median
  error of **0.24** in μ.
- The slip residual is +0.077 while accelerating and +0.034 while decelerating: the
  thermal-model loop direction.
- The peak sticking μ is 0.82 / 1.17 / 1.62 (p10/50/90).

See §3.3.

### 2.10 Phrases: legato and bow changes

`phrases.jsonl`, p10 / median / p90:

| Phrase type | Transition (ms) | Level dip (dB) | Aperiodicity peak |
|---|---|---|---|
| **Legato** (Philharmonia, all four families) | 5–10 / **35** / 65–105 | 0–1 / **3.0–3.6** / 7–9 | 0.28–0.42 |
| **Détaché** | 5–7 / 35–80 / 65–185 | 0.4–3 / **6.8–11.5** / 10–22 | **0.47–0.74** |
| **Martelé** | — | 8.6–10.3 | — |
| **Repertoire, close (HSD)** | violin 20, viola 35, cello 38 | 2.0, 5.3, 2.8 | — |

Re-bows inside a held pitch dip 4.7–5.6 dB.

The transition time includes the pitch tracker's window: 11–40 ms, the floor of the p10.
Real legato finger changes are therefore about **20–35 ms**. Arcus's slur moves the loop's
length in 3 ms.
## 3. The mechanisms, one by one

Each entry gives:

- **What it is**, and the equation or algorithm in a few lines.
- **Cost** in a real-time, deterministic voice. "Cheap" means under 5 % of a voice's
  0.48 % of a core; "moderate" means up to ×2; "dear" means more.
- **Arcus today**, from `docs/round42/arcus.md`, `arcus-match.md` and ADR 0157.
- **The recommendation.**
- **The measurement** from this database that shows it worked.

Sources are marked **V** (read on 2026-11-23/24: the paper's PDF, or its abstract where so
stated) or **NV** (from memory or a snippet, or a page refused us). The PDFs read are in
`F:\projects\daw-r42\downloads\study\papers\`.

### 3.1 The body: a measured response, not a scaled one

**What it is.** The bridge sees the body as an admittance Y(ω) = v/F. The listener hears the
radiated pressure p(ω) = R(ω, mic)·F(ω), where the transfer R is "radiativity". Both are a sum of
modes:

    Y(ω) = Σ_k  jω·A_k / (ω_k² − ω² + jω·ω_k/Q_k)

Below about 1 kHz a few "signature" modes stand out (A0, CBR, B1−, B1+; Bissinger 2008, V
abstract). Above that the modes overlap and only their envelope matters: the bridge hill
(§3.6) and a statistical cloud. Maestre, Scavone & Smith (2017, doi:10.1109/taslp.2017.2689241,
V abstract) fit a *measured* admittance with one bank of parallel second-order sections, made
passive by semidefinite programming. The same bank serves the bridge reflection and any number
of microphones.

Fritz et al. (2007, doi:10.1121/1.2799475, V abstract) give the tolerances that matter:

- musicians detect a single mode or band moved by **3–6 dB in level** or **1.5–20 % in
  frequency**;
- trained listeners detect less.

**Cost.** Fides's body is 40 complex one-pole modes (`BODY_MODES`). Fitting a measured response
changes the coefficients, not the cost. Maestre's method uses 30–60 sections to 6–10 kHz.

**Arcus today.**

- **The cello** is Fides's cello: low modes 103, 152, 172, 202, 262, 318 and 390 Hz, a cloud of
  33 modes from 450 Hz to 5 kHz, and a 6 dB "hill" at 1 kHz. It is the violin's recipe scaled
  to the cello's size, cited (`fides_body.rs`).
- **The bass** is the cello's body ×1.72 with two 120 Hz high-passes ("no measured bass body
  was found, said plainly").
- Model 2 then puts twelve peaking sections after the body, fitted to the recordings'
  third-octave spread (`arcus-match.md`).

**What the database adds.**

- Each family's **harmonic-peak response** (§2.2, `refdb/<family>.json → body`): A0, the
  wood-mode peaks and the bridge hill per instrument, with the spread across instruments.
- **Validation against measured admittances.** For three violins the CNSM set (CC BY 4.0) has
  both hammer admittances and recordings, so the estimator was checked on them (§2.2,
  `refdb/cnsm_validation.json`).
- **Zhang, Woodhouse & Stoppani 2016** (doi:10.1121/1.4964609, **V**, PDF) measured three cellos:
  - **A0 ≈ 97 Hz**, the wolf mode at 173 Hz, then 200, 209 and 281 Hz;
  - the bridge's first in-situ resonance, the cello's "bridge hill", at **≈ 1.2, 1.5 and
    1.35 kHz** on the three instruments;
  - the bridge acts as a rigid body to at least 1 kHz.

  Fides's 1 kHz hill sits at the low edge of that range.
- **Askenfelt 1982** (STL-QPSR 23(4), **V**, PDF) on the double bass:
  - **A0 ≈ 65 Hz** and **T1 ≈ 110 Hz**, which with C3/C4 makes a peak near 100 Hz;
  - the best bass has evenly spaced peaks over 150–400 Hz;
  - two broad bridge humps above 400 Hz;
  - C1 lies near the lowest open string and A0 near the second, the same pattern in the
    violin, cello and bass;
  - the player's hold changes the admittance around 60 Hz strongly.

**Recommendation.**

1. **Give the bass its own body.** Take the low modes from Askenfelt's pattern and the five
   measured basses (§2.2):
   - A0 at **70 Hz** (62–74);
   - a T1/C3/C4 cluster at **100–118 Hz**;
   - peaks at 170–240 and ~400 Hz;
   - a hill at **~1.3 kHz** (+6 dB after detrending);
   - 10 dB down by 2.8 kHz.

   Give the cloud the measured −4…−8 dB/oct trend. This replaces "the cello ×1.72", which puts A0 at 103/1.72 = 60 Hz but
   carries the cello's modal pattern down with it.
2. **Move the cello's hill.** Zhang 2016's bridge resonances lie at 1.2–1.5 kHz; the six
   cellos' detrended hill is 1.9 kHz (0.9–2.4), with the response 10 dB down by 3.0 kHz.
   Fides's hill is at 1.0 kHz. Place it at ~1.4–1.9 kHz and give its cloud the measured upper
   slope. Most of what model 2's twelve-band correction
   now does after the body then moves into the body, where vibrato's AM comes from.
   **This matters.** A correction after the body is fixed in frequency. Inside the body, the
   modes are what the partials sweep across during vibrato.
3. **For the violin, fit the CNSM admittances** (Klimke, Levaggi, Stoppani, CC BY 4.0) with
   Maestre's method offline, then scale the radiation tilt measured in §2.2. For the viola, see
   §3.13.

**The measurement.** Run Arcus's notes (every key, three dynamics) through `extract.py` →
`aggregate.py` and compare its `body` curve with the family's band. The pass is:

- A0 and the hill within the instruments' [min, max];
- the 1/3-octave body curve within the band at ≥ 80 % of the bins from 60 Hz to 6 kHz;
- the per-partial vibrato AM (§3.5) inside the band.

### 3.2 Bow noise: the second sound of a bowed string

**What it is.** A bowed note is a Helmholtz sawtooth *plus* a broadband noise that the ear
hears as "bow", "rosin", "hair" — the clearest cue that a bow is moving.

Its causes:

- **Differential slipping.** During the nominal stick, some hairs slip while others hold. MSW's
  "Aperiodicity in bowed-string motion" (doi:10.25144/23274, V abstract) names it "the most
  important musically" of three aperiodicity sources. Mores 2019 confirms it on a cello string
  (doi:10.1371/journal.pone.0211217, **V** PDF).
- **Flyback jitter**: the Helmholtz corner's round trip varies slightly.
- **Torsion and rosin.**

Chafe (1990, doi:10.1109/icassp.1990.116166, **V** abstract) showed that the noise must be
**pulse-modulated synchronously with the period** to fuse with the tone. Unmodulated noise is
heard as a separate hiss.

**Equation (a model that keeps Arcus deterministic).** Seeded noise n[k], shaped by a filter
H_n(f) and gated by a periodic window w(φ) that is open around the slip:

    y_noise = g(F, v_b, β) · H_n{ n[k] } · w(φ(k))

Here:

- φ is the string's phase (Arcus knows the slip instants exactly);
- w is the slip's share of the period, smoothed;
- the gain g follows the bowing. Noise grows when the force is low or the bow is far from the
  bridge (a weaker grip, more partial slip).

Physically, the noise can be **injected as a velocity at the junction during slip**, so the
string and body filter it, rather than added at the output.

**Cost:** cheap. One seeded generator (xorshift) and one or two biquads per voice.

**Arcus today.** "The hair's noise" is a separate layer, matched in level to the inter-harmonic
floor of harmonics 1–10 (`arcus-match.md`: "already inside the spread"). Whether it is
period-synchronous, and its spectrum, are not stated.

**What the database adds** (§2.3):

- the noise's **spectrum** per family, as harmonic-to-noise ratio per third octave;
- its level **against the dynamic** (`dynamics_slopes_per_step.noise_db_re_harm`);
- the attack's noise burst.

All of these use the residual of a tracked harmonic model, validated to ±1 dB above 5·f0.

**Recommendation.**

- Shape the noise by the measured HNR(f) curve, not a flat level.
- Gate it with the slip window: Chafe's pulsing; on a cello it is audible as "grain".
- Scale it with the dynamic as measured.
- Inject it at the junction so the body colours it.

**The measurement.** Arcus's `noise.hnr_band_db` and `res_valid_db_re_harm` per dynamic class
inside the family's band; the attack's `noise_burst_db_over_steady` inside the band.

### 3.3 Friction: hyperbolic, thermal, and "rate-and-state"

**What it is.** The junction's force law.

**MSW hyperbolic** (Schoonderwaldt et al. 2008, doi:10.3813/AAA.918070, **V** PDF):

    μ(Δv) = μ_d + K / (|Δv| + z₀),   with z₀ = 20 cm/s, K = 8 cm/s, μ_d = 0.4, so μ_s = 0.8.

Their limits, measured on a violin:

    F_max = 2·Z₀·(v_B/β + z₀)/Δμ;   F_min ∝ Z₀²/(R·Δμ·β²)

F_min was found **independent of bow speed**, against Schelleng's assumption.

**Desvages & Bilbao's exponential fit** (doi:10.3390/app6050135, CC BY):

    μ = 0.4·e^(−|Δv|/0.01) + 0.45·e^(−|Δv|/0.1) + 0.35

**Thermal** (Woodhouse 2003, Acta Acust. 89, 355; **V**, PDF from euphonics.org). Friction is set
by the rosin's contact temperature θ, a single averaged value. Its balance:

    μ·F·|Δv| = (conduction into string and bow: a Green's-function convolution of θ)
             + (advection of cold rosin) + ρc·V_contact·dθ/dt

Calibration: contact radius 250 µm at 1 N; active layer 1 µm; cello Dominant D 147 Hz,
Z₀ = 0.55 N·s/m. θ cycles 17–31 °C over ambient, and μ never exceeds 0.77. Helmholtz motion
starts "more benignly" than with the curve.

**Rate-and-state** (Woodhouse & Galluzzo 2025, Tribol. Lett. 73:128,
doi:10.1007/s11249-025-02062-4, CC BY, **V** PDF):

    f = A·h(v)·Y(T),   K = A/F_b,   K·Y(T₀) = 1

- the steady fit is μ_steady = −0.2·sgn(v)(|v| + 0.011)^−0.4;
- the jump curve is h(v) = −0.5·sgn(v)(|v| + 0.27)^−0.4;
- T₀ = 26.2 °C over ambient.

On a cello D (146.8 Hz) bowing machine, the friction-curve model is "uniformly poor", both
thermal models are better, and the enhanced one best matches early transients.

Galluzzo, Woodhouse & Mansour (2017, doi:10.3813/aaa.919136, V abstract) found the
contact-temperature model "significantly better" against machine transients.

**The measured rosin in this study.** Lampis 2025 (Zenodo 15879128, CC BY 4.0) gives the friction
force reconstructed at the bow on two cello G strings (Helicore, Dominant), β 0.028–0.20,
F_b 0.25–5.9 N, v_b 5–20 cm/s. The pooled slip samples (`refdb/friction.json`,
`plots/friction.png`):

- the best hyperbolic curve is μ_d 0.59, μ_s 0.90, z₀ 0.2 m/s, and it leaves a **median
  error of 0.24 in μ**: a velocity-only curve explains the reconstructed friction poorly;
- the residual is higher while the slip **accelerates** (+0.077) than while it decelerates
  (+0.034). That is a loop traversed the way the thermal models predict: hot rosin grips less
  late in the slip;
- the peak μ while sticking (p10/p50/p90): 0.82 / 1.17 / 1.62.

Caveats: the units are inferred (the record has no description), and the reconstruction itself
is noisy.

**Cost.**

- Hyperbolic: a closed form, already in Arcus.
- Thermal: one state θ per voice and a few multiply-adds, provided the conduction convolution is
  replaced by a one- or two-pole thermal model (the 2003 paper's own simplification is a sum of
  exponentials, NV). Deterministic with a fixed iteration count: moderate.

**Arcus today.** Hyperbolic by default. A thermal option, μ(θ) = μ_d + (μ_s − μ_d)/(1 + θ),
"keeps Helmholtz motion in a narrower band and settles fewer attacks". That is not
Woodhouse's law: a plain hyperbola in θ, with no conduction or advection terms.

**Recommendation.** Rebuild the thermal option on Woodhouse & Galluzzo 2025's rate-and-state
form:

- θ heated by μF|Δv| and relaxed by a two-time-constant conduction;
- f = A·h(Δv)·Y(θ);
- calibrated on the cello D numbers above.

Test it, *before any listening*, on two things where the papers agree it wins:

- **the attack's pre-Helmholtz transient** (Galluzzo's machine data: the distribution of
  periods-to-Helmholtz over a Guettler grid);
- **pitch flattening**, which grows with force.

Keep the hyperbolic default until the thermal law passes the same Schelleng and Guettler gate.

**The measurement.** Arcus's `attack.t_steady_s` and `noise_burst_db_over_steady` inside the
family's band at every dynamic. With thermal friction the steady sound changes little; the
attack is where it shows.

### 3.4 The player's gesture: force, speed and position as functions of time

**What it is.** Of everything a player controls, **bow force dominates the spectral centroid**:
"by far the most dominant parameter determining the spectral centroid". β and bow speed act
mostly by giving access to high or low force regions (Schoonderwaldt 2009, Acta Acust. 95, 901,
doi:10.3813/aaa.918221, **V** abstract).

Askenfelt's violin numbers (as quoted by Guettler 2003, CASJ 4(7), **V**):

| Stroke | Player 1: speed, β-distance, force | Player 2: speed, β-distance, force |
|---|---|---|
| piano | 29 cm/s, 42 mm, 0.5 N | 21 cm/s, 42 mm, 0.4 N |
| forte | 21 cm/s, 18 mm, 2.3 N | 24 cm/s, 20 mm, 1.7 N |

- That is ~9 dB of dynamic range.
- Players move position and force more than speed. **The bow speed barely changes, or falls,
  from p to f.**
- Raising the force from 0.2 to 0.5 N lifts harmonics 5–17 by more than 15 dB.
- Gravity changes the force by ~3 N between frog and tip.

Maestre 2010 (doi:10.1109/tasl.2010.2040783, V abstract) models force, speed and β per note as
cubic Bézier segments, with Gaussian-mixture statistics per note class.

**Arcus today.** The dynamic sets speed (**geometric pp → ff**), β (closer as it grows) and force
(0.42 of Schelleng's upper limit) together. The velocity sets the attack's acceleration.

**What the database adds.**

- `dynamics_slopes_per_step.log2_centroid_oct`: how far the centroid rises per dynamic step in
  real instruments (§2.4).
- The attack times: the players "swell". `arcus-match.md` measured 90 ms (bass) and 280 ms
  (cello) for a 10 → 90 % rise; §2.5 gives `t_rise`/`t_steady` per family.

**Recommendation.**

1. **Re-map the dynamic** as players do. From pp to ff:
   - **force ×4–5**;
   - **β-distance ×0.4–0.5** (towards the bridge);
   - **speed about constant**, within ±30 %.

   Arcus now drives speed geometrically, which raises the level mostly through speed and leaves
   the centroid's rise to β alone. Measure the result against the centroid slope of §2.4.
2. **Separate the attack's start from its swell.** Keep Arcus's clean Helmholtz start (the
   Guettler gate). Then shape a **bow-force and speed envelope over 50–300 ms**, set by
   velocity, so the level and brightness swell as in the references. A slow swell is not a slow
   Helmholtz onset: Guettler's limits concern the *periodicity*, not the level.
3. Later: **gesture contours by note class**, in Maestre's method (Bézier segments of force,
   speed and β per note, chosen by context: détaché, legato, the start and end of a phrase).
   No public database exists. The CNSM mocap (CC BY 4.0, violin) and Pàmies-Vilà's cello
   kinematics (Zenodo, CC BY 4.0; downloaded, §1) are the only open bowing data found, and they
   give bow speed and position, not force.

**The measurement.**

- The centroid's rise per dynamic step, and the level's, inside the band.
- `attack.t_rise_3db_s` inside the band at mf and ff.
- `t_steady` must not grow past Guettler's limits. The gate stays.
### 3.5 Vibrato: frequency modulation heard as amplitude modulation

**What it is.** The finger rolls, so the string's length, and therefore f0, swings at 5–6 Hz.
Every partial sweeps across the body's resonances, and its amplitude swings by several dB, each
partial differently, some in opposite phase (Fletcher & Sanders 1967,
doi:10.1121/1.1910516, V abstract).

Mellody & Wakefield (2000, doi:10.1121/1.428326, V abstract) measured violins at **5.9 Hz,
±15.2 cents**. **Removing the AM changed the sound markedly; removing the FM changed little.**
Further measurements:

- violinists and cellists alike: **~5.5 Hz**; the cello's width is smaller than the violin's
  (Geringer & Allen 2004, V abstract; values NV);
- **double bass: 5.17 Hz, ±19 cents**, faster and wider higher up (Mick 2024,
  doi:10.1177/19484992241238989, **V** abstract);
- violin and viola play +0.32 Hz faster and ~26 cents wider high up, and ~4 cents wider in
  forte (MacLeod 2008, V abstract).

The bow also moves with the vibrato (Hu et al. 2025, arXiv 2512.18162, V abstract: the
acoustic depth rises towards the bridge).

**Equation.**

    f0(t) = f̄0 · 2^(E·s(t)·sin(2π·r·t + φ)/1200)

- r is the rate and E the extent;
- s(t) is a delayed onset, 0.2–0.5 s;
- small seeded random changes to r and E per cycle, a few % (NV for the exact statistics,
  §5).

The partials' AM then comes from the body, provided the body is the measured one (§3.1).

**Cost:** nil (the loop length moves).

**Arcus today.**

- The loop length moves, so the AM is the body's. That is correct: the measured AM is
  1.5–7.5 dB through the body against 0.2–1.5 dB without it.
- Model 2 set the depth to ±15.5 cents (cello) and ±9.9 (bass), and the rates to the
  recordings'.

**What the database adds** (§2.6):

- rate and extent per family from the solo notes that have vibrato, and from real phrases
  (PHENICX, HSD, Philharmonia);
- **each partial's AM depth** (±dB at the rate, partials 1–20).

That last item is the measure Mellody & Wakefield found perceptually decisive. Arcus has never
been compared on it.

**Recommendation.**

- Keep the FM-only vibrato.
- Judge it by the per-partial AM distribution. If the AM is too small or too uniform, the body
  is too smooth (§3.1), not the vibrato.
- Add a slight **bow modulation correlated with the vibrato**: force or speed ±5–10 % at the
  rate, in phase with the pitch. This needs a measurement to confirm the sign (Hu 2025 only
  says the depth grows towards the bridge).
- Add per-cycle variation (seeded).
- Settle the bass's rate. The recordings that do have vibrato (§2.6) show ~5 Hz for the bass;
  model 2's "7.5 Hz from few notes" should be checked against Mick's 5.17 Hz.

**The measurement.**

- `vibrato.am_db[n]` for n = 1–20, at mf on every string, inside the band;
- the AM phases spread, not all equal: the share of partials in opposite phase to the FM
  inside the band;
- the rate and extent inside the band per register.

### 3.6 The bridge: its hill and its filtering

**What it is.** The bridge has in-plane resonances. The lowest, with the body's averaged
admittance, makes a broad rise: the "bridge hill". Woodhouse 2005 (Acta Acust. 91, 155, **V**
PDF) puts it **near 2.5 kHz** in many excellent violins, about 3 kHz with the feet clamped.
His model body has modes 200 Hz apart, modal mass 0.1 kg, Q 50, and a 1.5 g bridge. Durup &
Jansson 2005 (V abstract) give 2–3 kHz.

For the **cello** the first in-situ bridge resonance is at **1.2–1.5 kHz** (Zhang 2016, V). Above
it the bridge filters: the response falls. Gough (2016, Acoustics Today) reports bridge
resonances near 3 and 6 kHz and a transitional cluster at 800–1500 Hz.

**Equation.** One way to model it is a two-degree-of-freedom bridge (a mass on a rotational
spring) in series with the body's modal admittance:

    Y_bridge(ω) = Y_body(ω) / (1 − ω²/ω_b² + jω/(ω_b Q_b) + jω·m_b·Y_body(ω))

with ω_b the clamped bridge resonance. Woodhouse shows this yields the hill. A cheaper route
is a broad resonant section (Q ≈ 1–2) on the cloud of modes, which is what Fides's `hill` is.

**Cost:** nil, as a body parameter.

**Arcus today.** A 6 dB hill at 1 kHz, 0.8 octave wide (cello). For the bass, the same scaled by
1.72 (~580 Hz).

**Recommendation.**

- Set the hill's frequency and height from the measured families (§2.2), detrended:

  | Family | Hill | Height |
  |---|---|---|
  | Cello | ~1.4–1.9 kHz | +6 dB |
  | Bass | ~1.3 kHz | +6 dB |
  | Viola | ~2.2 kHz | +8 dB |
  | Violin | ~2.5 kHz | +7 dB (Woodhouse's 2.5 kHz; the CNSM admittances 2.1–2.5 kHz) |
- Set the fall above the hill from the measured "above hill" level. That slope is what makes a
  bowed string "nasal" or "open".

**The measurement.** `body.marks.bridge_hill_hz`, `bridge_hill_db_re_wood` and
`above_hill_db_re_wood` inside the band.

### 3.7 The finite-width bow and the compliant hair

**What it is.** The hair ribbon is ~10 mm wide (about 1/32 of a violin string; β is typically
1/8–1/32). It is compliant, so the hair's velocity differs from the stick's. Pitteroff &
Woodhouse 1998 (Acustica 84, Parts I–III; titles **V**, numbers NV: the PDFs are scans) modelled
the ribbon by finite differences.

The findings:

- **Partial slips at the ribbon's edges** add spikes to the bridge force.
- The width suppresses S-motion and other "higher types" of motion.
- The **Schelleng limits barely move**: the minimum force is slightly lower (euphonics §9.7,
  **V**; cello D, 6 "hairs" over 10 mm).
- A narrower effective ribbon (the bow tilted) **raises partials above n = 20 by 3–6 dB**
  (Schoonderwaldt, Guettler & Askenfelt 2003, SMAC, **V** PDF).

**Equation.** Two (or N) contact points β₁, β₂ a ribbon-width apart, each its own MSW junction,
coupled through the short string segment between them. Hair compliance is a spring k_h in
series with each contact: v_hair = v_bow − (1/k_h)·dF/dt.

**Cost.** Two junctions and one extra short delay line: ×1.5–2 on the junction. Moderate.

**Arcus today.** One point contact. The bridge's corner rounding while bowing (3 kHz or 6·f0 in
Fides model 2) stands in for the width's high-frequency loss.

**Recommendation.** Medium priority. The audible part, the top harmonics' level and the
attack's spikes, is mostly covered by a width-dependent high-frequency roll-off. Make that
roll-off a function of the **bow tilt and dynamic**: narrower and brighter in forte near the
bridge. Build the two-point junction only if the high-harmonic measurement (`h_db_re_max`,
n = 20–40) stays outside the band after §3.1 and §3.4.

**The measurement.** The harmonics 20–40 at ff inside the band; the attack's noise burst.

### 3.8 Torsion

**What it is.** A bowed string also twists: the bow drags its surface. The torsional waves are
fast and heavily damped.

Mores 2019 (**V**, PDF; Zenodo data downloaded, CC BY 4.0) measured a Chromcor cello G string
(98 Hz, 680 mm, 6.15 g/m, 121 N, Z₀ = 0.93 kg/s):

- torsional fundamental **543 Hz** (so c_t/c ≈ 5.5);
- torsional Q **about 10× lower** than transverse;
- every release preceded by a reverse torsional motion.

Woodhouse 2003 gives, for a Dominant cello D: torsional impedance 1.8 N·s/m against
Z₀ 0.55 (**ratio ≈ 3.3**), wave speed 1060 m/s, **Q ≈ 45**.

Schoonderwaldt 2008 gives ZR/Z₀ ≈ 2 (1.4–2.4) for gut or synthetic D strings and ≈ 3.5 for a
steel violin E. Serafin, Smith & Woodhouse 1999 found that dropping torsion "does not degrade
playability". Woodhouse & Galluzzo 2004 (**V**) suggest its damping may suppress Friedlander's
instability.

**Arcus today.** Torsion at the contact (speed ×5, impedance ×3, **Q 5**: "below the literature's
tens, said so"). Measured, it widened the Helmholtz region to Schelleng's 1:10 and doubled the
good attacks.

**Recommendation.**

- Keep it.
- Set the speed per string from the measured ratios: ×5.5 on the cello G steel string.
- Set the impedance ratio to 2 for synthetic cores and 3.3–3.5 for steel.
- Move Q towards the literature (10–45) **only** if the Schelleng and Guettler gate still
  passes. A low Q is a defensible stand-in for the hair's compliance (§3.7), which also
  damps the stick, and the gate is the arbiter. Say so in the ADR.

**The measurement.** No new sound measurement. This is a playability parameter, judged by the
existing gate.

### 3.9 The string's losses, its stiffness, and the finger

**What it is.** A string loses energy by internal friction, bending and air, all frequency
dependent. Woodhouse's loss model (euphonics §5.4, **V**):

    η_bend ≈ 2·α·n²·η_E;    η_air ≈ (d+0.2)·(8√2·ρ_a/(ρ·d))·√(η_a/(2π f_n))

Percival (2013, Glasgow thesis, **V** PDF) measured **three cellos on the instrument**:

- inharmonicity B (×10⁻⁵): A 5.7–6.3, D 12.8–24.9, G 20–23.4, C 53.5–62.6;
- decay r_n = B1 + B2·(n−1)² s⁻¹; cello C: B1 0.4–2.3, B2 0.09–0.18; cello A: B1 2.4–4.1;
- **a fingered note decays clearly faster than the open string**. His best fit is a finger
  damping R_f = 30 model units. It is the only finger-loss measurement found.

Desvages 2018 (Edinburgh thesis, **V**) gives full cello and bass string tables:

| Instrument | Length | String | Radius (mm) | Tension (N) | Mass (g/m) |
|---|---|---|---|---|---|
| Cello | 69 cm | A | 0.375 | 153 | — |
| | | D | 0.44 | 102.6 | — |
| | | G | 0.605 | 112.7 | — |
| | | C | 0.72 | 172.7 | — |
| Bass (steel core) | 106 cm | G | 0.518 | 285.5 | 6.62 |
| | | D | 0.699 | 291.5 | 12.04 |
| | | A | 0.95 | 301.4 | 22.16 |
| | | E | 1.286 | 310.7 | 40.72 |

The bass strings have E = 200 GPa. Guettler's "typical string properties" table (**V**) gives
ranges: cello C 14.3–17.0 g/m and 117–138 N; bass E 37–44 g/m and 285–337 N.

A **bowed** string is mode-locked: Helmholtz motion forces exact harmonics, so its inharmonicity
does not show in the steady tone. The database confirms this: `partial_dev_cents` is near 0 on
the references' arco notes (§2.4). Stiffness shows in the **pizzicato** (B, §2.7) and in the
attack: Schelleng's 0.1 cent·n² tolerance (1973) applies to playability.

**Cost:** nil to cheap. The loss filter's shape and one allpass section for B, plucked only.

**Arcus today.**

- Loss lumped at the bridge, one decay per string.
- No stiffness allpass ("orchestral strings sit far under Schelleng's 0.1 cent per n²").
- The tables (`arcus_tables.rs`): cello tensions 147, 132, 128 and 142 N; bass 290, 290, 280
  and 260 N.
- **No finger loss.**

**Recommendation.**

1. **Finger damping** on stopped notes. A one-pole loss at the nut end, a small R_f, makes a
   stopped note's pluck and release decay faster than the open string, as Percival measured.
   It is one of the cues that tells an open string from a stopped one, along with sympathetic
   resonance (§3.11).
2. The loss filter's frequency dependence from r_n = B1 + B2(n−1)², per string (Percival's
   ranges).
3. **Stiffness for pizzicato only**, B per string (cello A ≈ 6×10⁻⁵ … C ≈ 6×10⁻⁴). An open
   string's plucked partials then spread as measured (§2.7). For the bowed string, leave it out.

**The measurement.**

- Pizzicato `B` inside the band per register.
- `t60_s` per partial frequency inside the band.
- The open-against-stopped pizzicato T60 ratio (Iowa has both on each string).

### 3.10 Pizzicato: the pluck, the thump and the snap

**What it is.** A plucked string decays freely: each partial has its own T60, the body's
radiation sets the early sound, and the finger leaving the string makes a broadband "snap".
A hard pluck also slaps the fingerboard.

**Equation.** A free waveguide (or modal) string with per-partial decay T60(f) from §2.7 and
stiffness B. The excitation is a finger of finite width (a raised cosine over 1–2 cm at the
pluck point) released with a small velocity. The snap is a short (2–5 ms) burst, high-passed
above 1–2 kHz.

**Arcus today.** Fides's pluck on the same string and body. Model 2 shortened the ring
(fundamental T60 ×0.81 cello, ×0.67 bass). Left open: the brightness (centroid ~250–300 Hz
against the recordings' ~2 kHz), the noise (16–31 dB under the recordings'), and the thump
(the cello too much under 0.7·f0, the bass too little).

**What the database adds** (§2.7):

- T60 per partial frequency;
- B;
- the **thump excess**: the first 25 ms over 60–160 ms per third octave. This separates the
  attack's broadband content from the ring.

**Recommendation.** A pluck model with:

- a finite finger width (bright or dark by the plucking position and width);
- a **snap** burst whose level is fitted to the thump-excess band above 1 kHz;
- a fingerboard slap at ff;
- the per-partial T60 from §2.7;
- finger damping (§3.9).

Most of the recordings' ~2 kHz centroid is the first 50 ms: the thump excess shows how much.

**The measurement.** `pizz_thump_excess_db` and `pizz_t60_by_freq` inside the band. The
pizzicato's LTAS inside the band (the lossy and roomy sources discounted).

### 3.11 Sympathetic resonance of the open strings

**What it is.** The open strings are coupled through the bridge. A stopped note whose partials
coincide with an open string's partials makes that string ring. The note then gains a longer,
two-stage decay, beats, and a slight "glow". It is strongest on the notes of the open strings'
pitches and at the octave or fifth above them.

Fletcher & Sanders 1967 (V abstract) heard open strings ringing at the vibrato rate or twice
it. Woodhouse's book (euphonics §7.3, **V**) treats "multiple strings and double decays". **No
measurement of its level on a bowed instrument was found** (§5).

**Equation.** Three idle string loops (the other strings, open), each fed by the bridge
velocity through a small coupling admittance and radiating through the body. That is Chorda's
duplex/sympathetic pattern: a 2-port bridge junction (J. O. Smith's coupled strings).

**Cost.** Three extra free loops per voice, or one shared set per instrument (a mono
instrument needs only one). Moderate.

**Arcus today:** none.

**Recommendation.** Medium priority, and cheap if the open strings are shared per instrument
(Arcus is monophonic per instrument). It adds the ring after a release on notes at the open
strings' pitches, and the "glow" of an open-string unison. Measure it first: the Iowa runs give
the release decay of notes at and between the open strings' pitches.

**The measurement.** `release.decay_db_s` for notes on the open strings' pitches against their
neighbours (references and Arcus).

### 3.12 Directivity, the microphone and the room

**What it is.**

- Above ~1 kHz the violin's radiation pattern changes "drastically from one semitone to the
  next" (Weinreich 1997, doi:10.1121/1.418213, V abstract).
- Pérez Carrillo et al. (2011, doi:10.1121/1.3605291, V abstract) measured directional body
  responses from bowed glissandi with a bridge pickup and a microphone array.
- Meyer's book gives the cello's directivity (NV).

The microphone's distance and angle colour everything above 1 kHz: Iowa is anechoic at ~5 ft,
the others close in a room. A room adds a reverberant floor between the harmonics.

**Recommendation.**

- Model the body's output per microphone, as Maestre's shared modal bank allows (one set of
  poles, one set of residues per microphone). Fides's body already has left and right weights
  per mode.
- Measure Arcus against **anechoic** references first (Iowa, PHENICX). The room is a separate
  stage, and the noise figures of roomy references are upper bounds.

**The measurement.** The body band computed on Iowa alone and on the others: the difference is
the microphone and room colour, reported per family in §2.2.

### 3.13 What is particular to the viola and the violin

**The bow–bridge segment at 48 kHz.** In a waveguide the bridge side of the bow is β·(f_s/f0)
samples long. With the bow at distance d from the bridge on a string of open length L and open
pitch f_open, a stopped note has β = d·f0/(L·f_open). The segment is therefore

    N_bridge = d·f_s / (L·f_open)   samples

**independent of the note**: it depends only on the string and the bow's distance. At 48 kHz:

| String (open length) | d | Samples |
|---|---|---|
| Violin E (659 Hz, 0.325 m) | 10 mm | **2.2** |
| | 20 mm | 4.5 |
| Viola A (440 Hz, ~0.37 m) | 15 mm | 4.4 |
| Cello A (220 Hz, 0.69 m) | 30 mm | 9.5 |
| Bass G (98 Hz, 1.06 m) | 50 mm | 23 |

Arcus needs at least five samples there for the junction's sub-sample timing to work. The
violin's E and A strings and the viola's A string fall short when bowed near the bridge (forte).

**Recommendation.** Run the string, junction and body input at **an integer multiple of 48 kHz
per instrument**: ×4 for the violin (192 kHz) and ×2 for the viola (96 kHz). Then decimate with a
fixed linear-phase polyphase FIR: deterministic and bit-exact, a constant latency of a few
samples. That costs ×2–4 on the string, ~1–2 % of a core per voice, still within ADR 0157's
budget for a solo instrument.

A fractional-delay filter cannot replace it. A Lagrange or Thiran filter needs ≥ 1–2 samples of
its own delay, and the loss filter's group delay also lives in the loop, so a 2-sample segment
cannot hold them. A modal (or finite-difference) string near the bow is the alternative, at a
higher cost.

**The violin's body.** The CNSM set's three violins (CC BY 4.0) have measured bridge admittances.
Fit them (Maestre 2017), then apply the radiation tilt measured between their admittance and
their recordings (`cnsm_validation.json`: `tilt_db_per_oct_H_minus_Y`). That gives a violin
body from measurements of real instruments, which no free product has.

**The viola's size compromise.** The viola is tuned a fifth below the violin but built only
~15 % larger (body ~41 cm against the ~53 cm that scaling would ask; Hutchins's octet, NV). Its
A0 and main wood modes therefore sit **high relative to its open strings**. The bottom notes
(C3, G3) fall below A0 and are weak in the fundamental. That is the viola's "nasal", veiled
colour: more energy in the middle formant region than the violin, relative to its f0.

The database measures this directly. `body.marks.A0_hz` divided by the lowest open string's
pitch is **1.43 for the violin** (A0 281 Hz / G3 196) and **1.91 for the viola** (A0 223–250 Hz
/ C3 131); the cello sits at 1.61 and the bass at 1.70. In the viola's lowest octave the second
partial is the strongest (§2.4). Its response is also 10 dB down by 3.8 kHz, where the
violin's is at 6.0 kHz.

**Do not scale the violin's body by the viola's tuning ratio** (×1.5). Scale it by the
viola's size, ×1.12 (281 → 250 Hz, measured), which is what makes a viola. Then check the
viola's hill (2.2 kHz) and its upper fall against the band.

### 3.14 Aperiodicity and jitter

**What it is.** The period wanders slightly: flyback jitter. An audible Gaussian jitter is
about 5 cents (snippet, NV), and it grows with the flyback time. Schumacher 1992
(doi:10.1121/1.402730, V abstract) gives the analysis methods.

**Arcus today.** Deterministic and periodic, apart from the hair's noise and the closed loop's
trims.

**Recommendation.** Low priority. The physical jitter comes free with differential slipping or
a seeded perturbation of the friction (§3.2, §3.3). Do not add a pitch LFO for it.

**The measurement.** The residual's level (§3.2) covers it.

### 3.15 The release

**What it is.** When the bow leaves, the string rings freely with its own losses and the body's.
When the bow stops on the string, the note is damped at once. Players mostly lift at the end of
a long note, or let the bow slow.

**What the database adds** (§2.8): the level's decay rate after the bow leaves, per family and
register. Truncated or faded sample files are excluded.

**Recommendation.** Arcus's release should match those decay rates at its default
"release" articulation, with the string's measured losses (§3.9) and the finger damping.
Stopped notes decay faster than open ones.

**The measurement.** `release.decay_db_s` inside the band.

## 4. Analysis–synthesis tools for fitting

- **Sinusoids + noise (SMS)** (Serra & Smith 1990, doi:10.2307/3680788, V abstract).
  `bowlib_core.py` does it with the f0 track's phase. Harmonics are projected under a
  Blackman-Harris window that follows the vibrato; the residual is the noise. Validated
  (`validation.json`): harmonics ±0.4 dB, noise ±1 dB above 5·f0.
- **The harmonic-peak body estimate** (§2.2, `body.py`). Robust (alternating medians),
  validated on three violins against their measured admittances (`cnsm_validation.json`). It
  measures body × microphone × source roll-off, so **compare the model through the same
  estimator**.
- **LPC / cepstral envelopes.** These are an alternative envelope: an order-30–40 LPC on the
  harmonic amplitudes (discrete all-pole, El-Jaroudi & Makhoul) gives a smooth curve with
  peaks. The harmonic-peak median is preferred here: at low notes the harmonics sample the
  envelope too sparsely for LPC, and the median is robust to the β notches.
- **Perceptual distances**, the order to use them in:
  1. **Loudness-matched** comparison (BS.1770 here; better, Moore–Glasberg loudness for steady
     notes).
  2. The spectral centroid in harmonic numbers: the first dimension of every timbre space
     (McAdams 1995, Peeters 2011's Timbre Toolbox, doi:10.1121/1.3642604, V abstract).
  3. The attack time on a log scale: the second dimension (McAdams; NV for the exact result).
  4. A 1/3-octave or ERB-band spectral distance in dB inside the band (as `arcus-match.md`).
  5. **The partials' vibrato AM** (Mellody & Wakefield).
  6. The noise's HNR per band.

  An MFCC distance on loudness-matched notes summarises the spectral envelope in one number.
  It is no better than (4) for diagnosis, and harder to act on.
- **The tolerance.** Fritz 2007: 3–6 dB per band, 1.5–20 % per mode frequency. A model inside
  the instruments' band *and* within those tolerances of the median is indistinguishable in
  timbre from "another good instrument". The ABX then judges the rest: attack, noise, gesture.
## 5. What could not be verified

- **Papers whose numbers could not be read:**
  - Galluzzo 2017, Mansour 2016 Part 2 (and Mansour's thesis) and van Walstijn 2026: the
    Cambridge repository answered 503, and Cloudflare blocked the rest. Abstracts only.
  - Pitteroff & Woodhouse 1998, Woodhouse & Loach 1999 and MSW's "Aperiodicity": scanned PDFs
    with no OCR. Titles and the reviews' summaries only.
  - Not read at all: Demoucron's and Schoonderwaldt's theses (HAL bot check, DiVA timeout);
    Guettler, Schoonderwaldt & Askenfelt 2003, "Bow speed or bowing position"; Bynum & Rossing
    (the cello's mode table); Meyer's cello directivity; Jansson 2004; McAdams 1995's
    attack-time result.
- **No published values found for:**
  - a measured **level of bow noise** in dB re the harmonics. This study's §2.3 is, as far as
    found, the first such table for these instruments;
  - a **measured sympathetic-resonance level** on a bowed instrument;
  - the cello's **vibrato width** in cents (Geringer & Allen give only "smaller than the
    violin's");
  - an open **cello or double-bass admittance** or radiation dataset (Zenodo, DataCite,
    Figshare and Mendeley searched). Cambridge's Apollo answered 503, so data behind Zhang
    2016 may exist there unseen;
  - double-bass torsional and bending values (mass, tension and radius only).
- **Data caveats:**
  - Lampis's friction reconstructions have no description: the units N and m/s are inferred.
  - The CNSM admittance files state no units; (m/s)/N is presumed.
  - The Philharmonia and Aalto files are MP3: their noise figures are upper bounds.
  - Good-sounds' two cellists may share one instrument (not stated). They are counted as two
    instruments.
  - The Philharmonia samples come from several players. Each family is counted as one
    instrument.
- **Measurement method:**
  - The harmonic-peak body estimate measures body × microphone × source roll-off. It was
    checked only on violins (CNSM) against bridge admittance, not against radiation.
  - The attack's noise burst carries a +5–9 dB bias (validation).
  - The noise is over-read by +3.9 dB at 41 Hz with vibrato.
- **Licences (for a lawyer before anything ships):**
  - Iowa's "without restrictions" wording;
  - Aalto's "academic research";
  - the HSD stems: CC BY 4.0, which may ship;
  - Good-sounds via Zenodo: CC BY 4.0, which may ship.
- **Not in this study:**
  - Arcus itself was **not rendered or measured** here. The `arcus` builder runs the chain on
    its renders: §2's README tells how.
  - No listening test was run.
