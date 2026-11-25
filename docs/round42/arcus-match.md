# Round 42 — Arcus matched to recorded cellos and double basses (model 2)

*The `arcus` builder's report on the owner's ask (2026-11-23): "do what we did with Chorda —
download high-quality recordings and do a spectral analysis so our base sound gets better".*

## The references (measurement only — none is shipped, none is in the repository)

| Set | What | Where it came from | Licence | Used for |
|---|---|---|---|---|
| **University of Iowa MIS** | cello and double bass, arco and pizzicato, pp / mf / ff, chromatic runs on each string, anechoic chamber | `theremin.music.uiowa.edu` (MIScello.html, MISdoublebass.html), 74 AIFF files (each string's lowest run and the top string's every run), bytes checked (`FORM`) | "may be downloaded and used for any projects, without restrictions" | measurement |
| **TinySOL** | cello (*Violoncello*) and double bass (*Contrabass*) *ordinario*, pp / mf / ff, every note on more than one string | Zenodo record 3685367 (Cella, Ghisi, Lostanlen et al.), `TinySOL.tar.gz`, gzip checked; only `Strings/{Violoncello,Contrabass}/ordinario` kept | CC BY 4.0 | measurement |
| **VSCO 2 CE** | solo double bass, sustain without vibrato and pizzicato, two dynamics (v1, v3), first round robin | `github.com/sgossner/VSCO-2-CE`, `Strings/Solo Contrabass/{SusNV,Pizz}`, bytes checked (`RIFF`) | CC0 1.0 | measurement |
| **Karoryfer Bigcat cello** | A3, D3, C3 sustains at p / mf / f | Karoryfer Samples (the ABX's reference) | CC0 | measurement, the ABX |
| **D. Smolken double bass** | D2 arco p / mf / f, pizzicato | the ABX's reference | CC0 | measurement, the ABX |

All of them are under `F:\projects\daw-r42\downloads\arcus\` (outside the worktree).

## The method

`tools/arcus/match.sh` (`crates/cantus-dsp/examples/arcus_match/`) cuts every reference into
notes (Iowa's runs by their silences, each note's key checked by its own fundamental), and
measures each note and Arcus at the same instrument, technique, key and dynamic (p, mf, f =
velocity 0.3, 0.6, 0.9; Arcus at its defaults, vibrato and body on) **the same way**: over a
0.6 s steady window (Hann, zero-padded FFT) the harmonics 1–40 against their energy sum and
against the fundamental, the third-octave spectrum 50 Hz – 12.5 kHz, the centroid, the noise
between harmonics 1–10 against them; from the onset the attack (10 % → 90 % of the steady
level); the vibrato (a heterodyne's phase every 10 ms: its rate and depth); for a pluck each
partial's T60 (1–8) and the thump (what lies under 0.7·f0 in the first 60 ms).

The **references' spread** at a note is every reference note of that instrument, technique and
dynamic within a semitone, as its minimum … maximum (± 1 dB): Arcus is fitted *into* the
spread, not onto one player. The **distance** of an Arcus note is the RMS of how far each
component lies outside the spread (harmonics 1–20; the spectrum 63 Hz – 10 kHz; dB), the
centroid's (semitones) and the noise's (dB). A third of the notes and dynamics —
`(key + dynamic) % 3 == 0` — is **held out**: the fit never sees them, the distances below are
theirs.

## What was found (before: model 1)

The references, cut and checked: the Iowa MIS 97 cello and 95 bass arco notes, 99 and 113
pizzicato; TinySOL 291 cello and 309 bass arco; VSCO 2 CE 28 bass arco, 20 pizzicato;
Karoryfer 4; Smolken 4 — about 1 060 notes, every key Arcus plays at p, mf and f.

- **The cello was dark and bass-heavy**: its centroid 494 Hz against the references' 1 015
  (median over every note), 7.1 semitones outside their spread on the held-out notes; its
  third-octave spectrum 8 dB outside it, its harmonics 1–20 8.7 dB.
- **The vibrato too wide**: the cello ±19.1 cents against ±15.4, the bass ±20.9 against ±8.9
  (rates alike, 5.7 Hz; the references' bass 7.5 Hz, few notes with vibrato at all).
- **The pizzicato rings too long** (the fundamental's T60: the cello 3.5 s against 3.4, the bass
  4.0 against 2.9) and is far darker than the recordings (centroid 240–310 Hz against ~2 kHz;
  see *Not closed*).
- **The bass's arco** was already within the spread on the centroid and close on the harmonics
  (4.3 dB) and spectrum (4.8 dB).
- **The attack** (10 → 90 % of the steady level): Arcus 5–25 ms, the recorded players 90 ms
  (bass) and 280 ms (cello): they swell into a sustained note; Arcus's Helmholtz motion starts
  clean within Guettler's limits (the gate) and its level is there at once.

## What changed (model 2: `MODEL` 2.0, stored with every new Arcus; model 1 kept)

The fit (`match.sh fit`, five passes on the two thirds not held out) moves, per instrument,
twelve peaking sections on the output (63 Hz – 10 kHz, Q 1.4; half the median gap a pass,
smoothed across bands, the mean taken out so the level is the presets'), the hair's noise
(only by how far it lies outside the spread), the vibrato's depth and rate and the pluck's
ring (the fundamental's T60) by the median ratio — `arcus_tables.rs`'s `CELLO_MATCH` and
`BASS_MATCH`, applied in `arcus::eq` and `Arcus::set`:

| | correction (dB at 63, 100, 160, 250, 400, 630 Hz, 1, 1.6, 2.5, 4, 6.3, 10 kHz) | noise | vibrato depth | pizz ring |
|---|---|---|---|---|
| cello | −9.7 −6.9 −5.5 −3.1 −0.8 −1.4 −2.9 +0.6 +2.8 +1.8 +8.1 +12.0 | 0 | × 0.80 | × 0.81 |
| bass | flat (see below) | 0 | × 0.44 | × 0.67 |

The bass's fitted correction (+6.4 dB at 10 kHz, −8.4 at 630 Hz) moved its held-out notes
*away* from the references (spectrum 4.76 → 5.07 dB, centroid 0 → 2.2 semitones outside), so
it is left flat: the references' upper bands carry the rooms' and the bows' noise more than the
instrument at a bass's pitches. The hair's noise was already inside the spread for both.

## The distances on the held-out third (median over the notes, before → after)

| | notes | harmonics 1–20 (dB) | spectrum (dB) | centroid (semitones) | noise (dB) |
|---|---|---|---|---|---|
| **cello arco** | 44 | **8.71 → 4.13** | **8.18 → 3.78** | **7.09 → 1.25** | 0 → 0 |
| cello pizzicato | 25 | 22.2 → 20.2 | 23.5 → 22.7 | 18.6 → 15.3 | 31.1 → 31.1 |
| bass arco | 40 | 4.34 → 4.51 | 4.76 → 5.32 | 0 → 0 | 0 → 0 |
| bass pizzicato | 37 | 8.61 → 8.94 | 13.8 → 14.0 | 4.8 → 5.8 | 16.4 → 16.4 |

The other measures, medians over every note (references / before / after): the cello's
vibrato ±15.4 / ±19.1 / **±15.5** cents, the bass's ±8.9 / ±20.9 / **±9.9**; the pizzicato's
T60 the cello 3.4 / 3.5 / 2.8 s, the bass 2.9 / 4.0 / **2.7**; the cello's centroid 1 015 /
494 / **768** Hz. Every note's numbers: `docs/round42/arcus-match/distances.csv`; a plot per
held-out mf note (harmonics and spectrum, the references' band blue, before grey, after red):
`docs/round42/arcus-match/*.svg`.

## Not closed (said plainly)

- **The pizzicato's brightness and noise**: the recordings' plucks measure ~2 kHz of centroid
  and an inter-harmonic floor 16–31 dB over Arcus's. Much of that is the rooms and the
  recordings' noise under a decaying note (Arcus's pluck decays into digital silence), and
  part is real — the finger's snap, the fingerboard's slap. A pluck model with a snap and a
  fingerboard contact is the next step, not an output correction.
- **The attack**: the references' players swell; matching their level rise would slow
  Arcus's onset past Guettler's limits (the gate). Left to the *Attack* control and to the
  note's velocity; the study's recommendations may change this.
- **The bass's tone colour**: within the spread on the centroid, its spectrum 4.8–5.3 dB
  outside — a body fitted from anechoic bass recordings (the bass's own body, not the cello's
  scaled) is the way, with the study's reference database.
- **The thump**: the cello's pluck carries more under 0.7·f0 than the recordings (−13 against
  −21 dB), the bass's less (−20 against −11).

## What did not change

The pitch gate (the ear listens before the correction; a linear filter keeps a steady
fundamental's phase slope); null at defaults, block sizes, offline = real time, the pool (the
correction is a fixed linear stage after the body, its states let go sample by sample);
model 1 renders as before. Measured by the tests listed in `arcus.md`.

## For the owner's ears

`F:\projects\daw-r42\listen\arcus\` (after) beside `F:\projects\daw-r42\listen\arcus\before\`
(the same set before the match), the ABX renders with them.

## Pass 2: the reference study (`F:\projects\daw-r42\study\`)

The study's database (7 cellos, 5 basses, about 5 100 notes; `refdb\README.md`) and its ranked
recommendations (`bowed-realism.md`). Arcus is now judged by **the study's own estimator**:
`tools/arcus/study_compare.py` analyses Arcus's held-out notes (`arcus_match render`: the third
`(key + dynamic) % 3 == 0`, pp / mf / ff, 168 notes) with the study's `bowlib_note`, gathers
them with its `aggregate.family` as one more instrument, and compares each feature with the
family's band across instruments ([p25, p75] with four or more, else [min, max]). "Before" is
this branch at `58cafea8e` (pass 1), rendered from a worktree of that commit.

**What changed** (the study's recommendation numbers):

- **1 — measured bodies.** Two body recipes only Arcus uses (`BodyKind::MeasuredCello`,
  `MeasuredBass`, not in Fides's list): the cello's A0 105 Hz, wood 203 / 281 / 335 / 410 /
  560 Hz, the hill at 1.9 kHz +6.3 dB; the bass's **own** — A0 70 Hz (1.70 × the open E), T1
  109, wood 178 / 228 / 412 Hz, the hill at 1.3 kHz — no longer the cello's ×1.72 (its 120 Hz
  radiation corner gone: the estimate is of radiated sound). Over them the match's correction
  was fitted again (`match.sh fit`, two thirds of the notes): the cello's is pass 1's shape
  again (−8.1 dB at 63 Hz … +12 at 10 kHz); the bass's is no longer flat (−5.9 … +8.6 dB),
  since over its own body it now moves the held-out notes towards the recordings (spectrum
  5.26 → 3.76 dB on `match.sh`'s measure).
- **2 — the players' dynamic.** pp → ff: force ×4.5, β ×0.5, speed ×1.8 (it was ×7). A
  sustain's start swells from 20 dB under (pp) / 10 under (ff) over 0.25 / 0.05 s on top of
  the clean Helmholtz start; a re-bow on a ringing string, staccato and spiccato keep their
  speech. The pitch table measured again (offsets now −33 … +7 cents; the loop takes the rest).
- **3 — bow noise**: raised towards the bands (+12 dB the cello, +18 the bass). The "louder
  when soft" slope was measured and **not** added: Arcus's soft-to-loud difference was already
  larger than the recordings'. Its spectrum and its pulsing with the slip are as before (the
  roughness modulates the friction at the junction, so the body colours it).
- **4 — legato**: the finger's change 30 ms (was 3) with a 3 dB dip; the détaché re-bow's dip is
  the bow's own.
- **6 — pizzicato**: a finger snap (band noise 1–6 kHz over the first 25 ms, at half the
  string's peak). Per-partial decays, finger damping and stiffness: **not done**.
- **7 — thermal friction (Woodhouse & Galluzzo 2025)**: **not done**; the existing thermal option
  is unchanged and not the default.
- **8 — vibrato**: depth by dynamic (×0.75 soft … ×1.25 loud), the cello ×0.48 at 5.06 Hz, the
  bass ×0.38 at 5.1 Hz. Bow motion tied to the vibrato and per-cycle variation beyond the
  existing wander: **not done**.

Arcus never shipped, so the defaults changed in place: `model` stays 2, and model 1 takes the
new bodies, map and swell too. The pitch gate holds (**168 of 168 held-out points within a
cent, worst 0.81**), the attacks stay inside Guettler's limits (**0 of 80** over), and the arcus
and fides DSP tests pass (32).

**The distances on the held-out notes, by the study's estimator.** Each cell gives Arcus's value
and, in brackets, how far it lies outside the band. Harmonics, noise bands and thump are RMS dB
outside; pizzicato T60s are in octaves; a dash means the cell has too few instruments:

| cello | before (outside) | after (outside) | band |
|---|---|---|---|
| centroid_hz.all | 399 (0.00) | 426 (0.00) | 352 … 787 |
| centroid_hz.soft | 343 (90.74) | 413 (21.37) | 434 … 893 |
| centroid_hz.loud | 387 (136.05) | 374 (148.16) | 523 … 741 |
| noise_db_re_harm.all | -38.5 (3.40) | -40.6 (5.43) | -35.1 … -33.7 |
| attack_t_rise_s.soft | 0 (0.14) | 0.062 (0.08) | 0.144 … 0.284 |
| attack_t_rise_s.medium | 0 (0.10) | 0.039 (0.06) | 0.0978 … 0.367 |
| attack_t_rise_s.loud | 0 (0.01) | 0.008 (0.00) | 0.00748 … 0.0726 |
| attack_t_steady_s.all | 0.0531 (0.19) | 0.089 (0.15) | 0.241 … 0.418 |
| attack_noise_burst_db.all | 47.9 (4.28) | 46.2 (2.63) | 17.6 … 43.6 |
| vibrato_rate_hz.all | 5.81 (0.00) | 5.37 (0.00) | 5.19 … 6.17 |
| vibrato_extent_cents.all | 16.6 (4.10) | 10 (0.00) | 6.19 … 12.5 |
| harmonics.oct0_medium | (1.27) | (3.01) |  |
| harmonics.oct1_medium | (1.83) | (3.47) |  |
| harmonics.oct2_medium | (0.00) | (0.94) |  |
| harmonics.oct3_medium | (6.03) | (7.33) |  |
| hnr.soft | (2.66) | (5.35) |  |
| hnr.medium | (0.28) | (1.37) |  |
| hnr.loud | (0.00) | (2.41) |  |
| body.bridge_hill_hz | 1.26e+03 (200.46) | 1.78e+03 (722.34) | 707 … 1.06e+03 |
| body.hill_detrended_hz | 1.33e+03 (0.00) | 1.89e+03 (0.00) | 891 … 2.38e+03 |
| body.trend_db_per_oct | -3.75 (0.00) | -4.1 (0.00) | -6.77 … -2.13 |
| body.above_hill_db_re_wood | -29 (0.00) | -27 (0.00) | -34.6 … -22.5 |
| dyn_slope.log2_centroid_oct | … | … |  |
| dyn_slope.level_db | … | … |  |
| pizz_t60_octaves | (0.26) | (0.28) |  |
| pizz_thump | (9.62) | (11.38) |  |
| pizz_t60_f1_s | 2.8 (0.01) | 2.79 (0.02) | 2.81 … 3.14 |

| bass | before (outside) | after (outside) | band |
|---|---|---|---|
| centroid_hz.all | 225 (10.95) | 183 (0.00) | 165 … 214 |
| centroid_hz.soft | 218 (32.76) | 195 (9.19) | 161 … 186 |
| centroid_hz.loud | 248 (14.66) | 181 (7.33) | 189 … 233 |
| noise_db_re_harm.all | -45.6 (9.85) | -42.1 (6.33) | -35.8 … -32.6 |
| attack_t_rise_s.soft | 0.03 (0.07) | 0.064 (0.03) | 0.0978 … 0.24 |
| attack_t_rise_s.medium | 0.028 (0.08) | 0.022 (0.09) | 0.107 … 0.151 |
| attack_t_rise_s.loud | 0.025 (0.04) | 0.004 (0.06) | 0.0688 … 0.124 |
| attack_t_steady_s.all | 0.0672 (0.08) | 0.0867 (0.06) | 0.146 … 0.359 |
| attack_noise_burst_db.all | 29 (8.07) | 47.7 (26.72) | 17.8 … 20.9 |
| vibrato_rate_hz.all | 5.57 (0.09) | 5.37 (0.00) | 3.74 … 5.48 |
| vibrato_extent_cents.all | 8.07 (0.22) | 7.24 (0.00) | 6.49 … 7.85 |
| harmonics.oct0_medium | (2.10) | (1.44) |  |
| harmonics.oct1_medium | (3.85) | (1.08) |  |
| harmonics.oct2_medium | (8.81) | (5.72) |  |
| harmonics.oct3_medium | (12.66) | (3.46) |  |
| hnr.soft | (2.41) | (2.78) |  |
| hnr.medium | (1.68) | (1.79) |  |
| hnr.loud | (3.06) | (1.28) |  |
| body.bridge_hill_hz | 707 (235.17) | 1.12e+03 (650.52) | 354 … 472 |
| body.hill_detrended_hz | 749 (0.00) | 1.12e+03 (0.00) | 354 … 1.59e+03 |
| body.trend_db_per_oct | -5.38 (0.00) | -3.6 (0.06) | -7.67 … -3.66 |
| body.above_hill_db_re_wood | -38.1 (3.13) | -30.2 (0.00) | -35 … -26 |
| dyn_slope.log2_centroid_oct | … | … |  |
| dyn_slope.level_db | … | … |  |
| pizz_t60_octaves | (0.62) | (0.64) |  |
| pizz_thump | (10.72) | (16.43) |  |
| pizz_t60_f1_s | 2.81 (0.00) | 2.73 (0.00) | 2.56 … 2.86 |


**Read plainly.** The **bass** moved towards the recordings almost everywhere:
- its centroid came into the band (225 → 183 Hz);
- the harmonics of its upper registers came closer (octave 3: 12.7 → 3.5 dB outside);
- its vibrato came into the band, and so did its level above the hill;
- its attack rises later, nearer the recordings.

The **cello** improved in part:
- its vibrato and its detrended hill came into the band, and its soft notes brightened
  (centroid 343 → 413 Hz);
- but its harmonics and noise moved 1–2 dB further out, and its loud notes stay dark (374 Hz
  against 523–741).

**Still outside for both:**
- **The attack's rise:** 0.04–0.06 s against the recordings' 0.1–0.3 s at soft and medium. The
  swell should be longer still.
- **The attack's noise burst:** the bass's grew with its noise.
- **The pizzicato's thump:** the snap moved it further from the band (the cello 9.6 → 11.4 dB,
  the bass 10.7 → 16.4). It should be retuned, or taken out, after the owner hears it.
- **The estimator's `bridge_hill_hz` mark** is outside, although the detrended hill is inside.

The renders before pass 2 are in `F:\projects\daw-r42\listen\arcus\before-pass2\`, the renders
after are in `F:\projects\daw-r42\listen\arcus\`, both with the same names. The measures are in
`F:\projects\daw-r42\tmp\arcus\pass2\{before,after}.json`.

## Pass 3 (before = `b1413c1db`, the same held-out method)

- **The cello's brightness, measured.** The bridge's corner (the one-pole on the bridge's
  reflection while bowed) was at 3 kHz for the cello: it rounded the partials above about the
  fourth, whatever the bow's position, so the bow near the bridge at ff could not brighten the
  note. Raised to 6 kHz (`corner_hz`), the pitch table measured again. The soft cello's centroid
  came into the band (413 → 444 Hz), its harmonics in the two low octaves came back under pass 1's
  distance (oct 0 3.0 → 1.6 dB, oct 1 3.5 → 1.5), the noise closer (5.4 → 3.7 dB outside), the
  attack's noise burst into the band (46.2 → 43.0). **Not closed:** the loud cello stays dark
  (386 Hz against 523–741), and octave 2 moved out (0.9 → 3.2 dB). The refitted after-body
  correction is not what darkens it: it raises 6–10 kHz.
- **The attack's onset as a gesture: tried, measured worse, reverted.** Building the bow's speed
  and force together from 30 % over the swell (their ratio kept, inside Helmholtz's region)
  shortened the estimator's rise (0.06 → 0.004 s), raised the noise burst by 17 dB, and put 17 of
  80 attacks past Guettler's limits, with one held-out pitch point over a cent. The level swell
  of pass 2 is kept. **Not closed:** the rise (0.02–0.06 s against 0.1–0.3).
- **Pizzicato:** the snap is off (its gain 0); the thump came back towards the band (cello
  11.4 → 10.7, bass 16.4 → 13.9 dB outside, still outside). Per-partial decays, finger damping
  and stiffness: **not done**.
- **Thermal friction (recommendation 7):** **not done.**
- **CPU** (`tests_gate2::probe_cpu`, best of five, two to three other builds on the machine — not
  a claim): one voice 0.837 % of a core (1.79 × Fides's bowed string) against 0.820 % (1.77 ×) at
  `b1413c1db` measured the same way; eight voices 3.03 % against 2.94 %.
- **Gates:** the pitch 168 of 168 within a cent (worst 0.82), the attacks 0 of 80, the arcus and
  fides DSP tests 32 of 32.


| cello | before (outside) | after (outside) | band |
|---|---|---|---|
| centroid_hz.all | 426 (0.00) | 447 (0.00) | 352 … 787 |
| centroid_hz.soft | 413 (21.37) | 444 (0.00) | 434 … 893 |
| centroid_hz.loud | 374 (148.16) | 386 (136.97) | 523 … 741 |
| noise_db_re_harm.all | -40.6 (5.43) | -38.8 (3.66) | -35.1 … -33.7 |
| attack_t_rise_s.soft | 0.062 (0.08) | 0.032 (0.11) | 0.144 … 0.284 |
| attack_t_rise_s.medium | 0.039 (0.06) | 0.017 (0.08) | 0.0978 … 0.367 |
| attack_t_rise_s.loud | 0.008 (0.00) | 0.006 (0.00) | 0.00748 … 0.0726 |
| attack_t_steady_s.all | 0.089 (0.15) | 0.0921 (0.15) | 0.241 … 0.418 |
| attack_noise_burst_db.all | 46.2 (2.63) | 43 (0.00) | 17.6 … 43.6 |
| vibrato_rate_hz.all | 5.37 (0.00) | 5.37 (0.00) | 5.19 … 6.17 |
| vibrato_extent_cents.all | 10 (0.00) | 10.2 (0.00) | 6.19 … 12.5 |
| harmonics.oct0_medium | (3.01) | (1.64) |  |
| harmonics.oct1_medium | (3.47) | (1.45) |  |
| harmonics.oct2_medium | (0.94) | (3.19) |  |
| harmonics.oct3_medium | (7.33) | (6.55) |  |
| hnr.soft | (5.35) | (5.49) |  |
| hnr.medium | (1.37) | (1.47) |  |
| hnr.loud | (2.41) | (2.79) |  |
| body.bridge_hill_hz | 1.78e+03 (722.34) | 1.78e+03 (722.34) | 707 … 1.06e+03 |
| body.hill_detrended_hz | 1.89e+03 (0.00) | 1.89e+03 (0.00) | 891 … 2.38e+03 |
| body.trend_db_per_oct | -4.1 (0.00) | -3.99 (0.00) | -6.77 … -2.13 |
| body.above_hill_db_re_wood | -27 (0.00) | -25.7 (0.00) | -34.6 … -22.5 |
| dyn_slope.log2_centroid_oct | … | … |  |
| dyn_slope.level_db | … | … |  |
| pizz_t60_octaves | (0.28) | (0.28) |  |
| pizz_thump | (11.38) | (10.67) |  |
| pizz_t60_f1_s | 2.79 (0.02) | 2.79 (0.02) | 2.81 … 3.14 |

| bass | before (outside) | after (outside) | band |
|---|---|---|---|
| centroid_hz.all | 183 (0.00) | 183 (0.00) | 165 … 214 |
| centroid_hz.soft | 195 (9.19) | 195 (9.24) | 161 … 186 |
| centroid_hz.loud | 181 (7.33) | 182 (7.30) | 189 … 233 |
| noise_db_re_harm.all | -42.1 (6.33) | -42.4 (6.61) | -35.8 … -32.6 |
| attack_t_rise_s.soft | 0.064 (0.03) | 0.064 (0.03) | 0.0978 … 0.24 |
| attack_t_rise_s.medium | 0.022 (0.09) | 0.022 (0.09) | 0.107 … 0.151 |
| attack_t_rise_s.loud | 0.004 (0.06) | 0.004 (0.06) | 0.0688 … 0.124 |
| attack_t_steady_s.all | 0.0867 (0.06) | 0.0867 (0.06) | 0.146 … 0.359 |
| attack_noise_burst_db.all | 47.7 (26.72) | 47.4 (26.44) | 17.8 … 20.9 |
| vibrato_rate_hz.all | 5.37 (0.00) | 5.37 (0.00) | 3.74 … 5.48 |
| vibrato_extent_cents.all | 7.24 (0.00) | 7.23 (0.00) | 6.49 … 7.85 |
| harmonics.oct0_medium | (1.44) | (1.44) |  |
| harmonics.oct1_medium | (1.08) | (1.08) |  |
| harmonics.oct2_medium | (5.72) | (5.71) |  |
| harmonics.oct3_medium | (3.46) | (3.45) |  |
| hnr.soft | (2.78) | (2.79) |  |
| hnr.medium | (1.79) | (1.85) |  |
| hnr.loud | (1.28) | (1.35) |  |
| body.bridge_hill_hz | 1.12e+03 (650.52) | 1.12e+03 (650.52) | 354 … 472 |
| body.hill_detrended_hz | 1.12e+03 (0.00) | 1.12e+03 (0.00) | 354 … 1.59e+03 |
| body.trend_db_per_oct | -3.6 (0.06) | -3.59 (0.06) | -7.67 … -3.66 |
| body.above_hill_db_re_wood | -30.2 (0.00) | -30.2 (0.00) | -35 … -26 |
| dyn_slope.log2_centroid_oct | … | … |  |
| dyn_slope.level_db | … | … |  |
| pizz_t60_octaves | (0.64) | (0.65) |  |
| pizz_thump | (16.43) | (13.90) |  |
| pizz_t60_f1_s | 2.73 (0.00) | 2.73 (0.00) | 2.56 … 2.86 |


## Pass 4 (before = `d534e86e9`, the same held-out method)

**The cause the earlier passes measured around: a DC force.** A probe of a pp note's envelope
(`tests_p4::probe_onset`, then the render's spectrum by bands) found most of the first second's
power **under 2 Hz**: the bow's static push on the string reaches the bridge as a DC force, the
string's loop keeps it (it decays with the string's own loss, about a second), and the body
passed it. No box radiates it and no microphone records it, but the estimator's level and its
harmonic model saw it — so its *rise* read 0.01–0.03 s (the DC step at once, the swell under
it), its *noise burst* read the DC as residual (+40–53 dB), and pass 3's measurements of the
onset gesture were read off it too. **Fix:** the measured bodies' radiation ends in a high-pass
at 8 Hz (two first-order stages, `RADIATION_DC_HZ`: −0.3 dB at the bass's E1, −0.1 at the
cello's C2). Alone it moved the cello's soft rise 0.032 → 0.13 s and every noise burst down
9–15 dB; the release decay came into the band (74 → 43 dB/s). The renders' peaks fell with it
(the bass's ff note −10.7 → −16.9 dBFS) at the same loudness (the ABX phrase −27.9 → −28.0 LUFS).

**1 — The attack rise.** With the DC gone, the level's swell (pass 2's) is fitted per family:
its length falls with the dynamic's square — the players hold back at soft *and* middle
dynamics (the study §2.5) — the cello 0.30 → 0.05 s from 20 → 10 dB under, the bass 0.30 → 0.16
s from 20 → 16 dB under (its band is narrower and its loud start's own transient stood over
10 dB). **Every rise class is now in the band** but the bass's medium (0.01 s outside).
*Tried and reverted* — the onset modelled from the measured bow kinematics:

- *The measurement.* Pàmies-Vilà 2024's cellist (82 strokes, `tools/arcus/pass4/bow_kinematics.py`): the bow
  reaches half its stroke's speed in 0.11 s (p10–p90 0.03–0.20), 90 % in 0.24 s (0.08–0.37),
  its peak acceleration 1.4–6.1 m/s² (median 3.1); where a note starts from near-silence (7.2 s)
  the level rises with the speed, ~20 dB over 0.2 s. Lampis's robot on a real cello G
  (Zenodo 13374477, 3 948 attacks, `lampis_attacks.py`): Helmholtz motion within 11–70 ms at every force
  0.3–1.9 N, the slowest at the lowest accelerations (0.15–0.3 m/s²).
- *An eased onset* (the speed `v_t(1 − (1 − t/T)²)`, `T` 0.45 → 0.05 s by velocity, the force
  in the model's Guettler region for the start's acceleration, no level swell): the rises came
  into the band for both (the cello 0.20 / 0.15 / 0.02 s), `t_steady` into the bass's band — but
  the noise bursts rose 10–15 dB, the build order turned positive, and **19 of 80 attacks went
  past Guettler & Askenfelt** (low velocities at low α, the bass's low strings).
- *A clean start at a share of the speed, then the ease* (the force in proportion to the speed):
  at 15 % 13 of 80 over, at 30 % and 50 % 3 of 80 (the bass's E1/A1/B1 at low velocity, never
  Helmholtz within 0.5 s); with the cello alone 0 of 80, but the pitch gate lost a point (cello C3
  at the softest, +1.9 cents: the loop still converging when the window opens) and the bite's
  own transient stood within 3 dB of the steady level — the cello's soft rise fell to 0.01 s.
  The level swell over the clean start measures better on every count; kept.

**2 — The loud cello's darkness: found, not fixed.** By measurement (`tests_p4::probe_lampis`,
`probe_brightness`, `probe_instrument_brightness`; a first probe's harmonic search missed the
flattened partials of high-force notes and read them dark — corrected, and said here):

- *The string against a real one.* Lampis's robot bowing a cello A string (Zenodo 13374477's
  static set, 94 points in Helmholtz motion, `lampis_harmonics.py`): a sawtooth at every point
  (h2…h6 −6, −9.5, −12, −13.5, −15 dB), harmonic centroid 2.01 (p10–p90 1.82–2.13), **+0.10
  octave a doubling of force, β and speed ≈ 0** (+0.02, −0.04). Arcus's A string on the same
  grid: the same sawtooth, centroid 2.49 — but it brightens with *relative* force (`Fβ/v`): +0.27
  octave a doubling of force at a fixed β and speed, and darker with speed.
- *Under the players' map* (force ×4.5, β ×0.5, speed ×1.8 — the relative force held at 0.32–0.35
  of Schelleng's) the string's spectrum does not move from pp to ff (D3, E4, A♯4, E5, within
  ±5 %, vibrato and noise off); the real string's law would brighten it ~+0.2 octave. Neither
  the bridge corner (6 or 12 kHz, or none), the torsion, nor the friction curve (Schoonderwaldt's
  or Lampis's fit) changes that.
- *Where it shows.* Per register the loud cello's harmonics are inside the cellos' band in
  octaves 0–1 and dark in 2–3 (h7–h12 up to 10 dB under at octave 3); the `centroid_hz.loud`
  class gap (385 Hz against 523–741) is partly the class's register mix (the per-register
  harmonic centroids lie inside or at the band's lower edge).
- *The fix it needs* is in the junction — the corner's sharpening following the *absolute*
  force (the hair's compliance, the finite-width bow, the study §3.7) — not an equaliser.
  **Open.**

**3 — Thermal friction (recommendation 7), behind the switch, not the default.** The option is
rebuilt on Woodhouse & Galluzzo 2025's rate-and-state law (`string::ThermalLaw`): `μ =
h(Δ)·y(T)`, the jump curve `h = 0.5(|Δ| + 0.27)^−0.4`, the yield `y(T)` tabulated from the
paper's steady fit `0.2(|v| + 0.011)^−0.4` (`T₀` 26.2 °C at their crossing; the cold static
friction comes out 1.22, Lampis's measured sticking μ 1.17 median), the contact's temperature
from two poles (0.5 ms, 20 ms; 60/40) whose steady sum is calibrated on `T₀` at 1 N — the
paper's conduction equation is not reconstructed (its text came garbled), said so. The slip by
Newton from `|u|`, eight fixed steps. Measured (`probe_friction_gate`, the held-out set with
`ARCUS_FRICTION=thermal`): Helmholtz holds from lighter forces (0.07 of Schelleng's limit at β
0.1, against 0.16 hyperbolic) as the papers say, but **1 of 80 attacks past Guettler**, the
noise bursts 5–15 dB further out, the noise 2–4 dB further, the soft centroid out of the band
(445 → 408 Hz). Kept as the option; the hyperbolic curve stays the default.

**4 — Pizzicato.** Per-partial decays from the references: each family's T60 by frequency
(`CELLO_PIZZ_T60`, `BASS_PIZZ_T60`, the study's `pizz_t60_by_freq`) sets the pluck's loss at its
fundamental and eighth partial (the match's ring factors 0.81 / 0.67 retired); the finger's
damping is in the table (the references are stopped notes). The pluck's noise share 0.15 →
0.03: its first 25 ms carried 30–47 dB more 4–12 kHz than the next 100 ms against the
recordings' 10–20. The thump came halfway in (cello 10.7 → 8.1, bass 13.9 → 7.4 dB outside),
the bass's T60s 0.65 → 0.47 octave outside, both fundamentals' T60 in the band. The snap stays
off and **its code is removed**. Stiffness: Arcus's pluck measures B ≈ 0 above octave 1 against
the references' 1e-5 … 2e-4 — a dispersion filter in the shared loop, **not done**.

| cello | before | after | band |
|---|---|---|---|
| attack_t_rise_s.soft | 0.032 (0.11) | **0.172 (0)** | 0.144 … 0.284 |
| attack_t_rise_s.medium | 0.017 (0.08) | **0.138 (0)** | 0.098 … 0.367 |
| attack_t_rise_s.loud | 0.006 (0) | 0.012 (0) | 0.0075 … 0.073 |
| attack_t_steady_s.all | 0.092 (0.15) | 0.118 (0.12) | 0.241 … 0.418 |
| attack_noise_burst_db.soft | 40.8 (18.8) | 32.2 (10.2) | 12.5 … 22.1 |
| attack_noise_burst_db.medium | 41.9 (16.7) | 32.6 (7.3) | 13.8 … 25.3 |
| attack_noise_burst_db.loud | 48.9 (23.8) | 34.1 (9.0) | 18.0 … 25.1 |
| release_decay_db_s.all | 74.2 (4.1) | 42.9 (0) | 13.3 … 70.1 |
| centroid_hz.loud | 386 (137) | 385 (138) | 523 … 741 |
| noise_db_re_harm.all | −38.8 (3.7) | −38.7 (3.6) | −35.1 … −33.7 |
| harmonics oct 0 / 1 / 2 / 3 (medium) | (1.6 / 1.5 / 3.2 / 6.6) | (1.6 / 1.5 / 3.2 / 6.6) | |
| pizz_thump | (10.7) | **(8.1)** | |
| pizz_t60_octaves | (0.28) | (0.27) | |
| pizz_t60_f1_s | 2.79 (0.02) | 2.88 (0) | 2.81 … 3.14 |

| bass | before | after | band |
|---|---|---|---|
| attack_t_rise_s.soft | 0.064 (0.03) | **0.184 (0)** | 0.098 … 0.240 |
| attack_t_rise_s.medium | 0.022 (0.09) | 0.162 (0.01) | 0.107 … 0.151 |
| attack_t_rise_s.loud | 0.004 (0.06) | **0.096 (0)** | 0.069 … 0.124 |
| attack_t_steady_s.all | 0.087 (0.06) | 0.126 (0.02) | 0.146 … 0.359 |
| attack_noise_burst_db.all | 47.4 (26.4) | 36.2 (15.3) | 17.8 … 20.9 |
| release_decay_db_s.all | 49.4 (8.6) | 48.1 (7.2) | 16.8 … 40.8 |
| centroid_hz.loud | 182 (7.3) | 182 (7.1) | 189 … 233 |
| pizz_thump | (13.9) | **(7.4)** | |
| pizz_t60_octaves | (0.65) | **(0.47)** | |
| pizz_t60_f1_s | 2.73 (0) | 2.80 (0) | 2.56 … 2.86 |

Everything else (vibrato, body marks, harmonics, HNR) within 0.1 dB of before (`tools/arcus/pass4/table.py`). The measures (`tools/arcus/pass4/measure.sh`):
`F:\projects\daw-r42\tmp\arcus\p4\{dc8,ramp1,ramp2,swell2,thermal1}.json` (`swell2` is the
result; `dc8` the DC fix alone; `ramp1`/`ramp2` the reverted onsets; `thermal1` the thermal law).

**Gates:** the pitch 168 of 168 held-out points within a cent (median 0.015); the attacks 0 of
80 over Guettler & Askenfelt (worst 0.67 of its limit); the arcus and fides DSP tests 32 of 32.
**Defaults changed in place** (Arcus never shipped): the radiation's 8 Hz corner, each family's
swell, the pluck's T60 table and noise, the match's pluck ring factors to 1, the thermal
option's law. **Shared files** (`r42/arcus-upper`): `arcus_string.rs` — the thermal law only
(`ThermalLaw`, its state, the `Friction::Thermal` arm; marked `r42:arcus-p4`; the hyperbolic
path and `slip_root` untouched, so Fides's `bow2` keeps its bits); `arcus_ear.rs` untouched.
`arcus_tables.rs` gains `swell_s`, `swell_db`, `pizz_t60` on `InstrumentSpec` (the violin's and
viola's entries carry the cello's old swell and no pluck table).

**CPU** (`tools/test/ab-compare.sh`, `tests_gate2::probe_cpu` alternately on `d534e86e9`'s Arcus
files and this pass's, three runs each, release; no other build held a slot when it started, the
other builders' may have run during it — not a claim): one voice 0.802 → 0.821 % of a core
(+2.4 %, the radiation's two stages per side), eight voices 2.90 → 2.74 %.

**Tests** (Windows, this branch at the end): `cantus-dsp` lib (release) 1 356 passed, 0 failed;
`cantus-engine` + `cantus-core` (every test, `rt_memory`, block sizes, the pool, rest among
them) 2 447 passed, 0 failed; `cantus-ui` `arcus_pick_e2e` 1 (+1 ignored until `ALL`),
`fides_bowed_pitch` 1, `fides_pick_e2e` 2, `kit_lint` 4; clippy `--workspace --all-targets -D
warnings` clean; rustfmt on `cantus-dsp` (`cargo fmt --all` hits Windows's command-line length
here — not a format error). Not run: the Linux check (WSL is the integrator's).

**Renders:** pass 3's set kept in `F:\projects\daw-r42\listen\arcus\before-pass4\`; the
listening set and the ABX files re-rendered with the same names (and their `-lufs23` copies).

**Still open:** the loud cello's brightness (a junction that sharpens with absolute force); the
noise bursts (7–22 dB outside per class: the clean start's pre-Helmholtz slips, under the
swell's gain); `t_steady` (the spectrum settles in 0.09–0.13 s against 0.24–0.42 — a level
swell does not move it, and the eased onsets that did failed the gates); the pluck's stiffness.

## Pass 5 (before = `11e6b8cad`; the faults a musical demo found, and the owner's vibrato)

The demo (`examples/arcus_demo`, `tools/arcus/demo.sh`) measured six faults no single-note
gate had. Each is measured here the same way before and after — the before on a worktree of
`11e6b8cad` with only the probes added (`arcus_tests_p5.rs`'s `probe_report`,
`arcus_tests_vib.rs`'s `probe_vibrato_band`) — and each has a gate now
(`arcus_tests_p5_gate.rs`, `arcus_tests_vib.rs`, `cantus-ui/tests/arcus_level_e2e.rs`). The
pitch is read as the study reads it: YIN (`tests_p5::yin_cents`, the same steps as
`bowlib_core.yin_track`; checked against it on the same renders: within 0.2 cents), because a
bow's reversal turns a Helmholtz sawtooth over — the fundamental's phase jumps half a turn — and
a phase-slope measure reads that as pitch.

**1 — The tremolo out of tune.** *Causes, by measurement* (`probe_tremolo_trace`,
`probe_tremolo_scan`, `probe_reversal`, the spectra of the dumps):

- the stroke kept the dynamic's force (0.9 ×) while its speed went through zero at every turn:
  the bow dragged the string, the motion raucous most of each 71 ms stroke, its periods long —
  the cello's open A at −90 cents, B2 −150, the bass's B2 −210;
- the strokes' turns reversed the bow's static push on the bridge at half the stroke rate (7 Hz
  and its harmonics): under the bodies' 8 Hz radiation corner it rang their lowest modes (the
  cello's air mode by B2: the 100–118 Hz band 5 dB over the note's own fundamental) and the
  output's strongest band was 0–60 Hz;
- the pitch loop's ear heard each reversal's half-turn of phase as an error — up to ±2 cents a
  window, a random walk the trim followed (±10 cents within a second);
- a tremolo note began with an attack to the dynamic's speed and dropped to zero within one
  control step when its strokes began.

*Fix.* A light stroke: the speed a half-sine clipped at 0.3 of its peak (1.2 × the dynamic's), the
force 0.3 of the Helmholtz region's lower limit riding the speed (0.2 of it left at the turn);
the strokes start at once (no attack); the ear is cut at every bow reversal (`Ear::cut`, also
at a détaché's) and the trim held through a tremolo; the tremolo's own pitch table (per key ×
five dynamics, by YIN, `tools/arcus/trem-table.sh`: offsets −12 … +10 cents, left after it ≤ 4.8);
its level the instrument's `trem_db` (+3 … +8 dB) over the light stroke's, glided 20 ms; the
bodies' radiation a second-order Butterworth at 0.55 of each instrument's lowest open string
(−0.4 dB on that string). Tried and set aside: the force following the speed at the dynamic's
force (still raucous, the cello's B2 −150), a share of the dynamic's force (scaled badly across
strings: the bass's E string never periodic at forte), 0.6–1.4 of the lower limit (louder, but
5–35 cents flat and the bass's E string aperiodic).

| tremolo, YIN cents (open and a fourth up on every string, alone and slurred) | before | after |
|---|---|---|
| cello: mean \|c\|, worst | 11.4, +39.8 (C2) | **0.3, +1.1** |
| bass | no periodic frame on the E string | **1.1, +2.5** |
| violin | 4.9, −11.5 (G3) | **0.7, −3.2** |
| viola | 5.4, −14.5 (C3 slurred) | **0.4, +0.9** |
| the gate's 128 (two velocities): mean, worst | — | **0.55, +3.2** (bass A1) |

The demo's own cases: the viola's G4/A4 were +12 … +23, the cello's D3 −24 … +36, its open A3
−45 slurred / −80 alone; now within 3 cents (the gate covers each string's open note). The bass's
E string at forte is the least periodic tremolo (YIN finds a period in 31 % of its frames; the
Philharmonia bass's arco tremolo phrases 36–50 %).

**2 — The violin's B5 flat at forte: not reproduced, gated.** Measured on the demo's own violin
stem (`listen/arcus-demo/stems/01 Violín.wav`, before this pass) the two B5s of the climax are
−0.5 and +0.3 cents by YIN and +0.1 / +0.2 by the phase slope of harmonics 1–3; the climax
re-played note by note in the DSP (`probe_violin_climax`) within 1.3 cents with the vibrato
off, and E5–A6 held at ff (`probe_violin_top`) within 0.6. Whatever the demo's estimator read
(−8 … −13) is not in the rendered audio by these two. The gate holds E5–A6 at CC1 = 1 within a
cent held and 1.5 in a half-second note (worst 1.03).

**3 — The presets' level in music.** *Cause:* the gains were fitted on the browser's audition
phrases — velocity-only, around middle C — where the violin sounds its loud low strings; in its
own register with CC1 it sat 8 LU under the cello, and its level fell 0.17 dB a semitone up its
range (the viola 0.12) with no register balance. *Fix:* the violin's and the viola's register
balance measured (`tests_gate2::probe_levels`, now over all four instruments) and set; every
bowed preset fitted on one musical phrase with CC1 dynamics in its instrument's register
(`cantus_ui::arcus_level`: the same cantabile line from D5, D4, D3, D2; −19 LUFS), never past
−1.2 dBFS on it or on its audition phrase (`fides_listen … fit arcus`); the pizzicato presets
keep their role's fit.

**4 — The bow change's tick.** *Cause* (`probe_rebow_trace` on the demo's bass line): through a
change the force stayed at 0.85 of the stroke's while the bow slowed to zero, then the new
stroke began with a start's attack force (nearly twice the stroke's) — the string dragged at the
turn, a 4–6 ms burst in 4–12 kHz up to 20 dB over the bass's own high end. Measured on the
references the same way (`tools/arcus/pass5/strokes.py`, Philharmonia's détaché phrases: the
band's loudest 3 ms within 15 ms of a note boundary over its median 25–65 ms after,
`bowed-refdb/strokes.json`): the cellos 0.0 dB median (p90 8.5), the basses 7.6 (16.2), the
violas −1.8 (2.7), the violins −3.7 (26 changes). *Fix:* the force eases with the speed into
the turn to half the stroke's; a re-bow's new stroke turns at twice the attack's acceleration
with the force building with the speed; a re-bow to a new note moves the finger over 6 ms
(not in one sample).

**5 — The spiccato that never left the string.** *Cause:* a 70 ms stroke, then the string
rang at its held decay — each eighth fell 4–8 dB before the next. *Fix:* a 40 ms contact and,
between bounces, the string's T60 × `spic_ring` (0.27 violin … 0.55 bass), fitted to the
references' strokes (VSCO's solo violin and contrabass single strokes; Philharmonia's cello and
viola phrases).

**6 — The plain re-bow's noise** was the same change (its force at the turn); the re-bow's ticks
are measured and gated with the détaché's.

| strokes (Rust `tick_parts` / `bounce_at` at the notes' own times) | before | after | references |
|---|---|---|---|
| the demo's bass line (section III, the cantabile preset, CC1 0.38–0.48): ticks over the band after, median / loudest | +17.2 / +21.0 dB | **+2.2 / +6.5** | the basses' median +7.6, p90 +16.2 |
| the demo's own bass stem, changes down / onto the same pitch / up (the boundaries found by `strokes.py`) | +18.3 / +14.6 / +14.7 | **+2.1 / +2.8 / +3.7** | (as above) |
| the demo's cello stem, changes down / same / up | +3.0 / −1.7 / +1.0 | +0.9 / +0.1 / −0.7 | the cellos' 0.0 (p90 8.5) |
| a détaché an octave over each instrument's lowest string (CC1 0.6, 120 BPM eighths), median / p90 / loudest: cello | +1.1 / +5.5 / +6.4 | +0.9 / +4.2 / +7.4 | 0.0 / 8.5 |
| bass | +5.2 / +6.4 / +9.2 | +2.2 / +3.2 / +4.2 | 7.6 / 16.2 |
| violin | −0.4 / +3.0 / +3.2 | +0.6 / +4.0 / +7.2 | the upper strings' −1.8 / 2.7 |
| viola | −0.2 / +0.8 / +3.1 | +1.0 / +2.3 / +4.4 | (as above) |
| a plain re-bow on one pitch, median / loudest: cello, bass, violin, viola | −0.2 / 0.8, +1.5 / 3.7, −0.6 / 0.3, 0.0 / 1.6 | +0.4 / 1.5, −1.0 / 1.5, +0.7 / 2.0, +0.6 / 3.4 | |
| spiccato at 74 BPM eighths, the fall 40 / 80 / 150 / 250 ms after a stroke's peak, and the gap before the next: cello | 2.5 / 5.3 / 5.1 / 8.0, gap 11.1 dB | **3.7 / 6.1 / 10.1 / 14.8, gap 20.0** | the cellos' phrases 1.9 / 3.7 / 8.7 / 14.3, gap 24 |
| bass | 5.8 / 8.5 / 9.2 / 10.4, gap 16.1 | **6.7 / 10.3 / 9.5 / 16.7, gap 19.3** | VSCO's strokes 3.4 / 6.6 / 9.7 / 13.2 |
| violin | 0.9 / 3.5 / 4.9 / 7.3, gap 10.6 | **7.2 / 10.2 / 17.8 / 27.2, gap 35.3** | VSCO's strokes 4.8 / 9.6 / 16.7 / 27.6 |
| viola | 0.5 / 1.8 / 3.9 / 5.5, gap 8.3 | **4.2 / 7.8 / 11.7 / 17.2, gap 26.8** | the violas' phrases 5.8 / 8.2 / 9.1 / 8.8, gap 16 (at 90 ms strokes) |

The synthetic détaché (CC1 0.6, an octave up) did not show the fault before — it lived in the
bass's low register at a soft CC1, where its own high end is 85–90 dB down; the gate holds both.
The spiccato's ring (`spic_ring`) is fitted on the single strokes where the references have them
(the violin, the bass) and on the phrases for the cello and the viola, whose room fills the
falls in (their lower band is loose).

**The presets' level in music** (`arcus_level_e2e`, integrated LUFS on the common phrase, the
presets as the browser loads them):

| | before | after |
|---|---|---|
| the violin's 7 bowed presets | −26.5 … −31.5 | −19.0 … −19.2 |
| the viola's 5 | −19.5 … −24.4 | −19.0 |
| the cello's 12 | −18.0 … −29.0 | −18.9 … −19.4 |
| the bass's 8 | −16.8 … −25.8 | −19.0 … −19.8 |
| the spread | **14.7 LU** | **0.97 LU** |

(The before is measured with this pass's tremolo, whose light stroke had not yet its level; the
round's own tremolo presets sat 5–10 LU under their instruments' sustains.) The gains moved:
the violin's by +6.4 … +8.0 dB, the viola's by +0.5 … +0.7, the cello's and the bass's as their
music level asked (`arcus_presets.rs`, `arcus_upper_presets.rs`); the audition phrases keep
their −1.2 dBFS headroom, and `instrument_levels`'s dense riff passes full scale on none.

**The demo after** (`listen/arcus-demo-p5/`, the violin's and the bass's faders at 0 dB — they
were +7 and −4 — every check passing): the stems at −18.8 (violin), −20.7 (viola), −19.9
(cello), −19.8 LUFS (bass); before, with the offsets, −19.2 / −21.7 / −20.2 / −22.0 — without
them the violin would have sat at −26.2 and the bass at −18.0. The four now lie within 1.9 LU
playing different parts, unaided.

## Pass 5's vibrato (the owner's ask: natural, studied, programmable)

**The study** (`tools/arcus/pass5/vibrato.py`, every single-note arco file of the reference
catalogue and Philharmonia's legato and crescendo phrases, cycle by cycle — the table is in
`research/instruments-next/bowed-realism.md` §2.6b and `bowed-refdb/vibrato.json`): the
players' vibrato is 5.0–5.6 Hz, faster loud and a little higher up; 15–32 cents peak to trough,
widening 1.4–1.9 × from soft to loud and with the register, but *not* following the level
cycle by cycle within a note; a quarter in at 0.2–0.25 s (later soft) and full 0.2–0.3 s
after; nearly sinusoidal; centred; and never metronomic — the period changes 11–23 % and the
extent 12–27 % from one cycle to the next.

**Round 42's model 2** measured the same way: rates and extents in or near the bands but
metronomic (rate jitter 0.04–0.08, extent 0.04–0.06), the extent only 1.25–1.33 × wider loud,
the rate not moving with the dynamic, the viola too fast and too wide, the onset 0.36–0.53 s on
the bass and the viola.

**Rebuilt** (`arcus_voice_step.rs`'s `vibrato_model2`, `tables::VibSpec` per family; model 1
keeps its own to the bit): the rate and the extent per family from the study, each rising with
the dynamic and the register by the study's slopes; a per-cycle variation (a new value each
cycle, glided through the cycle — no step, no metronome) and a per-note one (±4 % rate, ±10 %
extent), seeded by the note; the delay longer for a soft note (`onset_dyn`); the defaults'
delay and rise 130 / 330 ms (were 250 / 450). **Programmable:** `vib.delay`, `vib.attack` (the
rise), `vib.rate`, `vib.depth`, `vib.var` (the variation: 0.25 the players'), and **`vib.dyn`**
(new: how much the extent follows the dynamic, 1 the players'); the depth from CC21, CC2,
aftertouch, **the mod wheel** or **each note's MPE pressure** (`vib.src`, both new; the
controller then leaves the dynamic), MPE's per-note pitch passing through as always. Note-level
choices (*senza* / *molto vibrato*) are round 43's adaptive piano roll's — the hook is in ADR
0157's pass-5 amendment; no schema field added.

| long notes (3 s; three registers × three dynamics × four seeds; the study's estimator) | before | after | the players (p25 … p75) |
|---|---|---|---|
| cello: rate, extent, onset | 5.26 Hz, 21.1 ct, 0.22 s | 5.19, 25.6, 0.17 | 4.89–5.42, 17.2–32.8, 0.13–0.47 |
| cello: jitter rate / extent | 0.048 / 0.057 | **0.143 / 0.137** | 0.079–0.167 / 0.097–0.164 |
| cello: extent soft → loud, rate soft → loud | 18.7 → 24.9, 5.26 → 5.26 | 21.4 → 35.4, 5.00 → 5.26 | 19.3 → 32.3, 5.01 → 5.49 |
| bass: rate, extent, onset | 5.33, 15.9, 0.53 | 4.76, 15.8, 0.30 | 4.31–5.42, 12.1–22.2, 0.10–0.56 |
| bass: jitter | 0.062 / 0.063 | **0.241 / 0.261** | 0.154–0.319 / 0.191–0.357 |
| bass: soft → loud | 13.9 → 17.0, 5.26 → 5.33 | 12.8 → 18.2, 4.40 → 5.97 | 15.0 → 20.8, 4.61 → 5.35 |
| violin: rate, extent, onset | 5.41, 22.7, 0.39 | 5.13, 23.3, 0.19 | 4.89–5.35, 17.1–28.2, 0.15–0.60 |
| violin: jitter | 0.082 / 0.064 | **0.199 / 0.119** | 0.106–0.232 / 0.110–0.183 |
| violin: soft → loud | 19.7 → 25.0, 5.48 → 5.41 | 16.8 → 31.1, 4.88 → 5.26 | 19.5 → 37.8, 4.92 → 5.53 |
| viola: rate, extent, onset | 5.97, 33.5, 0.36 | 5.41, 24.2, 0.12 | 5.28–5.57, 19.5–32.4, 0.10–0.41 |
| viola: jitter | 0.042 / 0.039 | **0.145 / 0.134** | 0.088–0.237 / 0.096–0.165 |
| viola: soft → loud | 29.3 → 38.3, 5.97 → 5.97 | 20.0 → 32.0, 5.48 → 5.41 (the estimator's step) | 26.8 → 44.4, 5.49 → 5.73 |

**The timbre flicker** comes from the bodies as the string's length moves — verified by the
study's own `analyse_arco` on the renders: the partials 1–10 swing ±1.1 (cello), ±0.5 (bass),
±1.5 (viola), ±0.9 dB (violin), half to three quarters of them against the pitch (the players'
half), against the players' ±2.0–2.3 dB. The physics gives it; its depth is half the players'
because the measured bodies are smoother than real ones (pass 2's fit, §3.1 of the research):
said so, not added by hand. **Open:** the flicker's depth (the bodies' modal density), the
short notes' faster and wider vibrato (the study: 5.7–6.0 Hz, 38–48 cents under 1 s — not
modelled: a note's length is not known at its start), the rate's rise with the dynamic on the
viola (the study's is small, 0.24 Hz; the model's sampling shows none).

**Gates** (Windows, this branch at the end, the dev profile with cantus-dsp at opt-level 3):
`cantus-dsp` lib 1 390 passed, 0 failed (106 ignored probes) — among them the held-out bowed
pitch (168 of 168 within a cent), the attacks (0 of 80 past Guettler & Askenfelt), the pass-5
gates and the vibrato's; `cantus-core`'s Arcus table 2; `cantus-engine`'s `inst_arcus` 1,
`arcus_image` 9, `rt_memory` 67; `cantus-ui`'s `arcus_tuning_art_e2e`, `arcus_pick_e2e`,
`strings_rec_e2e`, `arcus_upper_e2e`, `arcus_level_e2e`, `instrument_levels`, the i18n keys —
all green; clippy `-p cantus-dsp -p cantus-core -p cantus-engine -p cantus-ui --all-targets -D
warnings` clean; rustfmt on every changed file. Not run: the Linux check (WSL is the
integrator's).

**Hashes.** `DSP_VERSION` 13; `upper_tests::LOWER_HASHES` re-made: the seven cello and bass
renders' RMS −0.3, +0.0, −0.4, −3.2, −0.8, −3.2, +1.3 dB against `11e6b8cad`, their sample
difference at the signal's level (the vibrato's phase and the tremolo's strokes decorrelate
them; `tests_p5::probe_dump_lower` in both trees). No golden hash moved (none holds Arcus).

**Renders for the owner** (pass 4's set stays in `listen/arcus/`, the demo's first take in
`listen/arcus-demo/`): `listen/arcus-p5/` — the cello and the bass (`tools/arcus/listen.sh`)
and the violin and the viola with their ABX pairs (`upper-listen.sh`), and `presets/` (every
preset on its audition phrase, its notes, and the bowed ones on the music phrase,
`NN-<preset>-music.wav`); `listen/arcus-demo-p5/` — the demo with the faders at 0 dB.

**For the ear:** the tremolo's sound (light, a little airy; its level `trem_db`), the bow
changes now soft-edged (the bass's 15–20 dB click gone; whether a little more bite is wanted at
the change is the ear's), the spiccato's bounce and ring, the new vibrato's life (its variation
per cycle and per note), the violin's level against the others in the demo.
