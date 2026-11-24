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
