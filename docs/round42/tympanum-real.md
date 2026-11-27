# Round 42 — `tympanum-real`: Tympanum against real drums of many genres

Branch `r42/tympanum-real` (from `r42/integration` e63ceddd2). The owner (translated):
*"Tympanum still lacks a lot. Download professional-quality recordings that are free of
copyright. If the drums aren't separated, convert the hits to MIDI, then analyse the sound
against what we have now and iterate. Consider the original kit type, where each drum is
struck and at what levels. Listen to several kinds of kits from different genres and adapt
Tympanum well, so it reaches levels no generated drum kit reaches today."*

This document is written as the work goes; its last section says what is done and what is
open.

## 1. The library (`tools/sound-refs/catalog.toml`, the `drums-*` sets)

Everything is under `$CANTUS_REFS/drums/` (`F:\projects\cantus-refs\drums`), outside the
repository; the catalogue holds each kit's licence, credit, the kit, room and mics, genre,
layers and whether it is multitrack or real playing (`[[set.kits]]`).

| set | what | licence | genre | kind |
|---|---|---|---|---|
| `tympanum` (round 40) | DRSKit 2.1, 13 mics | CC BY 4.0 | rock/pop studio | multi-mic hits |
| `tympanum` | Virtuosity Drums, 6 mic positions, up to 36 layers | CC0 | jazz (sticks) | multi-mic hits |
| `drums-dg` | The Aasimonster 2.1, 16 ch | CC BY 4.0 | death metal | multi-mic hits |
| `drums-dg` | MuldjordKit 3, 16 ch (Tama Superstar) | CC BY 4.0 | metal/rock | multi-mic hits |
| `drums-dg` | ShittyKit 1.2, 8 ch (Stagg GIA, cast-iron cymbals) | CC BY 4.0 | lo-fi/garage | multi-mic hits |
| `drums-packs` | Big Rusty Drums (Szpaderski 24″ kick, 14×8 snare; sticks, brushes, mallets) | CC0 | 1980s rock | close + overhead |
| `drums-packs` | Unruly Drums (wires on every drum; sticks and brushes) | CC0 | indie pop, brushes | close + overhead |
| `drums-packs` | Gogodze Phu II (7 mics, deadened toms, "Kim Jung Mi's Now, 1973") | CC0 | 1970s funk/soul | multi-mic hits |
| `drums-packs` | Frankensnare (12 snares 10″–22″, sympathetic buzz) | CC0 | snares | hits |
| `drums-packs` | Swirly Drums dry (brushes), VSCO 2 percussion (timpani) | CC0 | brushes, orchestral | hits |
| `drums-freesound` | unfa's 15-minute jam (a real kit, XY pair, no click); RutgerMuller's funk grooves and rides; Srirachasocks' brushes; soneproject's jazz-bar kit (sticks, brushes, many velocities); Matias.Reccius's metal kit; sandyrb's snares with overheads | CC0, CC BY 4.0 (per sound) | rock, funk, jazz, metal | real playing + hits (MP3 previews) |
| `drums-gmd` | The Groove MIDI Dataset's MIDI: 1 150 human performances, ten drummers | CC BY 4.0 | rock, funk, jazz, Latin, soul, gospel, punk, hip-hop … | performances (MIDI) |

Not taken, on licence: Salamander Drumkit and the AVL kits (CC BY-SA); MDB Drums, ENST,
IDMT-SMT-Drums, RBMA, MUSDB, MedleyDB, Cambridge-MT (NC, ND or proprietary); ADTOF (NC-SA);
madmom's and LarsNet's transcription models (NC). Not measured, on what the audio is: E-GMD
(a TD-17 module, no microphones), StemGMD and the WaivOps sets (rendered from sample
libraries). No CC0/CC BY multitrack of a real song with a real kit was found (ccMixter's CC BY
stems are programmed or unverified); the real playing is unfa's jam and RutgerMuller's grooves.

**How the kits are played.** Every recorded kit is read by Tympanum's own loader (the app's
DrumGizmo and SFZ readers) and played through the engine's offline render, each with its own
default mix — as a user hears it on loading it — so recorded and modelled kits go through the
same path: `tools/tympanum-real/content_root.py` gives Cantus's CC0 packs (as the app installs
them, joined by junction) and the DrumGizmo kits a receipt naming the programs Tympanum plays.

## 2. What Tympanum is measured against, and how we would show it is better

**The instruments to beat** (research 2026-11-26, nothing bought or downloaded):

| instrument | kind | how it could be measured without buying it |
|---|---|---|
| IK Multimedia MODO DRUM 1.5 | modal synthesis + samples (hybrid), 13 kits, size/tension/shell per drum | the free **MODO DRUM CS** (the Studio kit, full engine): the owner installs it on his machine (an IK account), renders `tympanum_real`'s probe MIDI and the GMD grooves; the measures run on its bounce |
| AAS Chromaphone 3 | physical resonators (drumhead, membrane) | 15-day full trial; official demo MP3s for listening only |
| Reason Kong (Physical Bass Drum / Snare / Tom) | physical models | Reason's trial |
| Roland Prismatic Sound Modeling (TD-50X and later) | modelled acoustic drums (no samples played) | hardware: official videos only (not measured) |
| Nord Drum 3P | modelling percussion synth | a CC0 Freesound sample exists (RutgerMuller 463597) — one hit, too little |
| Toontrack Superior Drummer 3 / EZdrummer 3, XLN Addictive Drums 2, Steven Slate Drums 5.5 (free tier), BFD3 (BFD Player), GetGood Drums, NI Abbey Road / Studio Drummer, Logic Drum Kit Designer (90-day trial), Steinberg Groove Agent 5 | sampled (the realism bar) | the free tiers and trials the owner can install; the same probe MIDI and grooves |

**How "better" is shown.** The same three tests for every instrument, against the recorded
kits' spread:

1. **The measures** (`compare.py`): every per-hit feature of §3 — dynamics, brightness with
   level, glide, decay per band, wires, wash, take-to-take variation, image — counted inside or
   outside the recorded kits' range, per piece; and the grooves' (`grooves.py`). An
   instrument's score is the share of features inside, and how far outside the others sit.
2. **A blind MUSHRA** per piece and per groove, with a recorded kit as the hidden reference and
   the instruments as the conditions, at one loudness (`tools/soundlab` already runs MUSHRA
   kits); listeners from the owner's circle.
3. **"Recording or synthesis?"**: each clip alone, the share of listeners who say "a
   recording" — the one number a generated kit has never reached at 50 %.

Nothing in this round can rank a commercial instrument: it needs the owner's machine and his
accounts (BACKLOG). Tympanum's own numbers before and after are below.

## 3. The measures (`tools/tympanum-real`)

| tool | what |
|---|---|
| `tympanum_real` (cantus-ui example) | `info` (a kit's pieces, keys, mics), `hits` (every articulation at velocities 16, 32, 48, 64, 80, 100, 127, three takes each, a drum's 4 s window and a cymbal's 6 s; a closed hat at CC4 0 as a song without a pedal plays it), `notes` (a CSV of hits: a transcription, a performance), `dg-build` (a DrumGizmo kit built as the app installs it) |
| `pipeline.py` | `hits`, `grooves`, `compare` over every kit (`KITS`) and Groove MIDI excerpt (`GROOVES`) |
| `features.py`, `measure.py` | per hit: level (first 80 ms, peak), attack (rise, crest, 0–3 ms centroid, the stick's > 5 kHz share), centroid in five windows, octave bands per window and T30 per band, a drum's f0, glide and asymmetric-mode share, the snare wires' level, decay and flatness, a cymbal's 4–12 kHz flatness, bloom and centroid over time, late 4–10 kHz (sympathetic buzz and room), side/mid early and late, L/R correlation, C50, a third-octave spectrum for take-to-take variation |
| `summary.py`, `compare.py` | per piece kind, the numbers a kit's whole velocity range gives (span, steps that fall, brightness per 10 dB, glide hard and soft, …); each Tympanum kit against the recorded kits' range, ranked by a rough audibility weight; the charts |
| `grooves.py` | the same performance through every kit: loudness, crest, LRA, width, octave balance, 6–16 kHz flatness |
| `gmd_levels.py` | at what velocities ten real drummers play, per genre (GMD) |
| `transcribe.py` | real drumming to MIDI: partially-fixed NMF on third octaves, templates from a kit's measured hits, onsets as 6-dB rises, a class's share of the frame |
| `position.py` | centre against edge strokes from the packs' own sample files |
| `law.py`, `blob_read.py` | model 5's level law (below), the tables blob read back |
| `preview.py`, `report.py` | the owner's named previews and the Spanish report |

**Real drummers' levels** (GMD, 1 150 performances): the median snare stroke is velocity 45 and
**41 % of snare strokes are ghost notes** (< 40) — 50 % in funk, 51 % Afro-Cuban, 64 % reggae, 26 %
gospel; the kick's median is 53. Soft playing is most of what a kit plays: the soft layers and the
level law under velocity 60 are where a modelled kit is heard most.

**The transcriber**, scored on Groove MIDI performances rendered through recorded kits (±30 ms):
the kick F 0.62–0.98, the closed hat 0.71–0.94, the snare 0.72 on DRSKit but 0.09 on Big Rusty
(its rimshots and its 14×8 snare are far from DRSKit's template); the toms and cymbals are rare in
the excerpts. Good enough to give a real performance's timing and dynamics back to a kit; not a
transcription to publish.

## 4. What was wrong, what changed (`DSP_VERSION` 19; ADR 0124's sixth amendment)

Ranked by `compare.py`'s audibility weight; every finding below held on three takes and across
velocities before anything changed.

1. **The level law (changed).** Measured through Pulsus (`level_law_dump`), every model 5
   table had velocity steps that went the wrong way: the studio kick at 56 louder than at 80,
   its 100 → 127 step +0.7 dB (DRSKit's own: 12, 6, 5.5, 3, 3.5, 3 dB over the same steps), the
   12″ tom −2.8 dB from 80 to 96, the jazz ride 5.7 dB quieter at 127 than at 104. Each table now
   follows a law (`pulsus_measured_law.rs`, written by `law.py`): the studio kit's DRSKit's, the
   jazz kit's its own made monotone; the hardest hit unchanged. After: every table grows at every
   step (the dump at 17 velocities; the jazz ride flat within 0.7 dB above 104). Weighted measures
   outside the recorded kits' range (with the burst below): **studio 23.2 → 13.7, jazz 38.1 → 25.7**; no step falls; the jazz toms' 0–3 ms centroid at the hardest 4 875 → 1 745 Hz (the recordings 549–2 083).
2. **The jazz rack toms' attack (changed).** Table 22 alone had a stick burst; its first 3 ms
   ranged 0.4–10 kHz from take to take at one velocity (Virtuosity's own tom 1.3–2.1 kHz, every
   recorded tom 0.4–2.1 kHz). `BURST` scales it to 0; the knock stays.
3. **The jazz closed hat's variation (diagnosed).** 6.1 dB from one closed stroke to the next
   (third octaves, 250 Hz – 16 kHz) against 1.4–2.0 for every recorded closed hat and 1.65 for the
   studio hat. Scaling its per-hit level draw (modes ± 1.5 dB, bands ± 0.7) to 0.35 changed it by
   nothing measurable (6.13 → 6.13 dB), so it was taken out: the change is the phase draw — a few
   strong, close partials in its table beat differently each hit inside one third octave. The
   fix is in the table (close partials merged, or their phases drawn together), a refit.
4. **The crest (diagnosed, not changed).** In the grooves Tympanum's crest (peak − loudness) is
   2–10 dB above the recorded kits' in almost every genre. Per hit, the snare is the cause: its
   waveform peaks at 0.6–1.2 ms (DRSKit 7.5 ms, Virtuosity 7.1 ms), and **every band of the
   generated snare peaks early** — 4–12 kHz at 0.5–1.4 ms, 1.5–4 kHz at 1.5 ms, 0.4–1.5 kHz at
   2–3 ms, 60–400 Hz at 4–5.5 ms — where in both recordings every band peaks at 5–11 ms (the lows
   last). The bands' tables rise over 3–5 ms as measured, so the early energy is the modes' onset
   and the stick's knock (the snare's knock is 4.9 at velocity 110–127, a 2-ms half-sine low-passed
   at 1.2 kHz). Physically (corrected after the drums physics survey,
   `research/instruments-next/drums-physics-survey.md` on `r42/drums-research`, 499435d9c): the
   stick's contact on a snare or a tom lasts **3–8 ms** at mezzoforte (Wagner 2006, a
   contact-sensing stick; Dahl 2011's survey), shorter for harder strokes and towards the rim,
   with bumps as the head's wave returns from the rim — not ~1 ms, as this section first said.
   Besides, the head's displacement — what radiates — grows over its first half-periods (a 14″ snare's ~200 Hz: 2.5 ms each), the bottom
   head and the wires answer only after the air cavity couples them (another 1–3 ms), and the
   overheads hear it 2–3 ms later still. Model 5 starts every mode at full amplitude at the
   strike. **Model 6's onset** drives each mode through a contact force — but a half-sine of
   0.8 ms (ff) to 1.8 ms (pp), three to five times shorter than the measured contact (the survey's
   §4; the modelled family's next step is a Hunt–Crossley stick on the modal head, §8) — the
   wires' noise gated by the bottom head's motion with the cavity's lag, the knock as the head's
   first push rather than a separate pulse. A limiter or a fixed delay would only hide it.
5. **The 1 kHz band (diagnosed).** Per hit, the studio kit's 1 kHz share is inside the recorded
   range for every piece; the jazz kit's snare (−7.5 dB re its whole first 200 ms against
   −10 … −15) and toms (−4.6 against −9 … −25) are above it — their knocks (4.4 and 2.3 at 127).
   The grooves' 1 kHz excess on the studio kit is the mix's balance (its snare against its kick
   and hats), not a piece's colour. It goes with item 4: the knock re-made as the head's push.
6. **Measured and left:** the closed hat's length (241 ms at 8 kHz with the pedal closed, the
   recordings 193–224); brightness rising with level (true for kicks, toms and cymbals in the
   recordings, and in Tympanum; not for a snare — four of seven recorded snares are brighter soft,
   the wires carrying a ghost note — so a snare must not be made to brighten).
7. **Where the stick lands (measured, not changed).** `position.py` reads centre and off-centre
   strokes from the packs' own files. On Virtuosity's rack tom the (1,1) region (1.35–1.75 × the
   fundamental) over the fundamental is −16 … −24 dB struck at the centre and −7 … −13 dB off it,
   layer by layer — the nodal-diameter modes a centre stroke leaves still; Big Rusty's snare
   (wires on) moves the same way by 3–6 dB at its louder layers; Virtuosity's snare, wires on or
   off, gives no steady difference (its f0 is hard to read under the noise). A first try that
   raised a measured drum's (1,1)/(2,1) regions with `p{n}.pos` did not show the rise through
   Pulsus (its own test measured the ratio falling: the 3-dB-an-octave tilt and the glide move
   the modes it names), so it was taken out: the edge stroke waits for model 6's modes driven
   through the strike point (each mode's shape at the point, not a band boost).

## 5. Brushes — the plan (not built)

Tympanum has no brushes; the jazz and ballad grooves need them, and real players' soft strokes
(41 % ghost notes) are where brushes live. The references are in the library: Big Rusty
(`snare_14/{brush,bdig,bstir,bflutter,brelease}`, 4 takes a layer, close + overheads), Unruly
(`02-kit-brushes.sfz`: dig, stir, flutter, the hat with a brush in six openings), Swirly dry
(`snare_main`, `snare_dig`, `snare_stir`, `snare_flutter`, `snare_edge`; a brushed kit and
percussion), Srirachasocks' and soneproject's brushed kits (Freesound CC0). The measures to take
first: a brush tap's excitation (many bristles over 5–30 ms instead of one ~1-ms contact: the
onset's length and its spectrum's top), a sweep's level and colour against the hand's speed (the
stir loops), the wires' share (a brushed snare is mostly wires), the release's lift-off. The
model: a tap as model 5 tables measured off those layers (the head's modes as they are, the
excitation spread over the bristles' arrival); a sweep as a sustained friction noise through the
head's bands, held by the note and shaped by its velocity and a controller (a hand's speed), the
wires gated by the head's motion. Keys: GM has none — the articulation keys past 88 (ADR 0042),
`brush`, `sweep`, `dig`, `flutter`, `release` on the snare, a brushed hat and ride. Built only
against those recordings, as the drums were.

## 6. For the owner's ears

`F:\projects\cantus-work\listen\tympanum-real\` — `escuchar.html` (Spanish): nine Groove MIDI
performances (rock, pop, funk, jazz, samba, soul, gospel, punk, hip-hop), each through the recorded
kits of its genre and Tympanum's Studio and Jazz kits before and after; three real Freesound
recordings (funk, a funk ride, slow metal) transcribed by `transcribe.py` and played back by
DRSKit and Tympanum beside the original. 16-bit WAV, one loudness (−20 LUFS, lowered together for
−1 dBFS peaks). `informe.html` (Spanish): the library, the method, what changed, every chart,
every measure as the annex. Named, not blind: the blind kit (`tympanum_abx`'s, with the new
genres) comes after he has heard these.

Rebuild everything (after a change to model 5):

```text
bash tools/build/slot.sh cargo build --release -p cantus-ui --example tympanum_real
python -I tools/tympanum-real/content_root.py
python -I tools/tympanum-real/pipeline.py hits <work> --tag after        (TYMPANUM_REAL_EXE=…)
python -I tools/tympanum-real/pipeline.py grooves <work> --tag after
python -I tools/tympanum-real/preview.py <work> <listen> --tags before,after
python -I tools/tympanum-real/pipeline.py compare <work> <work>/report --tags before,after
python -I tools/tympanum-real/report.py <work>/report <listen> <informe.md>
cargo test --release -p cantus-dsp --lib level_law_dump -- --ignored --nocapture > law.log
python -I tools/tympanum-real/law.py law.log <work>/hits <crates/cantus-dsp/src/pulsus_measured_law.rs>
```

## 7. Done, and open

**Done (branch `r42/tympanum-real`), first session:** the library catalogued and fetched within the disk budget
(≈ 2.3 GB downloaded: Aasimonster built for Tympanum 1.7 GB, Freesound 51 MB, GMD 9 MB; the CC0
packs joined from the app's folder; MuldjordKit stopped at 0.8 GB when F: neared its floor — the
manifest's `muldjordkit-tympanum` pack fetches it later; ShittyKit's old DrumGizmo format the
builder refuses, removed); the tools above; the measures of eight recorded kits and Tympanum's two;
the level law and the jazz toms' burst
(`DSP_VERSION` 19, ADR 0124's sixth amendment); the owner's previews and report.

**Done, second session (ADR 0124's seventh amendment):**

- **Model 6** for a drum's onset (`pulsus_measured_onset.rs`; Tympanum's `model` 2; schema
  **86**, `v86.cantus`):
  - the contact phasor;
  - one level draw per mode, with the modal power restored;
  - the bands' rise and the wires' lag;
  - the knock as the head's push;
  - the moving-head re-strike;
  - `LAW6`;
  - a hat's stick fixed stroke to stroke.
- **Model 1 renders bit-identical** to the previous model 5 binary (the studio drums' hits
  compared byte for byte).
- **Tools:** `onset.py`, `onset_fit.py`, `law6.py`, `quick_preview.py`, `brushes.py`,
  `elements.py` and `elements_page.py`; `preview.py` takes `--labels`.
- **For the owner** (`F:\projects\cantus-work\listen\tympanum-real\`):
  - `m6/index.html`: the interim preview;
  - `escuchar.html`: model 5 against model 6, all nine grooves and the three real recordings;
  - `informe.html` with §4b;
  - `analisis-elementos/index.html`: 12 pieces, each with clips, spectrograms, thirds, decay,
    partials, variation and mics, a verdict per piece and a ranking.
  - The previous pages are kept as `escuchar-ley-r42.html` and `informe-ley-r42.html`.
- **§8, the pivot plan**, with the drums physics survey folded in.

**Model 6's numbers** (weighted outside-range; groove crest, recorded range in brackets):

| | model 5 | model 6 |
|---|---|---|
| studio, weighted outside | 13.7 | 12.8 |
| jazz, weighted outside | 25.7 | 24.1 |
| rock (studio) crest, dB [19.1–23.8] | 27.7 | **22.8** |
| funk (studio) [20.9–27.8] | 30.2 | 29.1 |
| gospel (studio) [19.4–27.2] | 27.8 | 29.8 |
| hip-hop (studio) [20.5–25.1] | 26.5 | **23.2** |
| jazz (jazz) [18.5–20.8] | 25.7 | 23.7 |
| pop (studio) [23.7–29.3] | 27.9 | 28.5 |
| punk (studio) [17.6–23.9] | 26.5 | 24.5 |
| samba (studio) [15.5–24.7] | 26.4 | 26.3 |
| soul (studio) [21.7–26.1] | 27.5 | **24.8** |

The jazz kit's grooves improve more: punk 31.0 → 25.5, pop 34.4 → 27.8, rock 28.7 → 26.1.

Per-band peak times of hard strokes, recorded median → studio → jazz (ms; lows / mids /
1.5–4 kHz / 4–12 kHz / waveform):

- snare: 9.7 / 13.6 / 18 / 26 / 10.4 → 3.5 / 6.0 / 25.5 / 29.4 / 2.6 → 11.7 / 6.0 / 39 / 34 / 6.1
- kick: 7.2 / 9.3 / 8.7 / 7.1 / 10.1 → 21 / 2.2 / 1.9 / 3.9 / 5.8 → 18.7 / 31 / 31 / 29 / 17.3

The first 10 ms crest over every drum, against the recorded median: studio −0.2 dB (model 5
+0.1), jazz +1.7 dB (model 5 +2.8).

**The owner's ear:** model 6's rock, *"suena horroroso"*; everything modelled, *"todo era un
desastre"*. Hence §8.

**Open:**

1. **The plan (§8, revised)**: realistic modelled flagship kits, rounds 43–44; recorded kits a supported second family.
2. **The modelled family's onset**: model 6's half-sine is 3–5× shorter than the measured 3–8
   ms contact. Its next step is a Hunt–Crossley stick on the modal head, with the energy-driven
   glide and air loading (§8 (c)).
3. **The tables' faults** found piece by piece (`analisis-elementos`):
   - the studio kick has no highs after its attack;
   - hats, crash and toms carry 6–12 dB of extra lows at 100–200 Hz;
   - the ride's bell lacks its 1 kHz note;
   - the studio snare is hollow at 500–800 Hz, with a 200 Hz ring that lasts too long.

   These matter only for the modelled family.
4. **The hybrid's residual** was fitted on model 5's power, so it is stale for model 6
   (`refit_hybrid.sh`, if the hybrid is kept at all once kits are sampled).
5. **An old song cannot be moved to model 2** from the app (Chorda's preset path does it for
   Chorda): a small command plus a line in the Kit view, if the owner wants it for the modelled
   family.
6. **Brushes** (§5, §8: sampled taps and sweeps); `brushes.py` holds the first measures.
7. **The commercial comparison** and the blind kits (§2, §8).
8. Catalogue: `drums-dg` and `drums-packs` are not `record`ed (`fetch.py record` once on disk).

## 8. The plan after this round: realistic generated kits (2026-11-26, revised)

**What the owner wants.** After hearing every preview he said *"todo era un desastre, no se
parece nada a algo real"*, and of model 6's rock *"suena horroroso"*. On the plan he said: *"La
idea es que Tympanum sean sonidos generados (aunque permita sonidos grabados)"*. Tympanum's
identity is a **generated, modelled drummer**:

- **The flagship kits are the modelled ones**, made realistic by physics.
- **The recorded kits stay supported** (the loader, every mic, bleed, rooms) and remain **the
  references** a modelled piece is measured against. They are not the flagship sound.

The physics behind each choice is `research/instruments-next/drums-physics-survey.md`
(`r42/drums-research`, 499435d9c). The measure is the element-by-element analysis
(`tools/tympanum-real/elements.py`; the owner's page `analisis-elementos/index.html`): every
piece inside the recorded kits' spread on spectrum, decay, partials, take-to-take variation and
mic image.

### 8.1 What the analysis says is wrong, and what the physics prescribes, per element

**Kick.**
- *Wrong:* no highs after the attack. From 1 to 16 kHz the studio kick sits 12–39 dB under
  the darkest recording; in practice it is a low sine.
- *Fix:* refit the kick table's bands to DRSKit's recording. Then a **Hunt–Crossley beater** (a
  mass meeting the head through a nonlinear damped contact) with the measured **3–8 ms**
  contact, replacing model 6's 0.8–1.8 ms half-sine (Avanzini & Rocchesso 2001; the
  non-iterative scheme of Ducceschi et al. 2021). Add a real **front/back head coupling through
  the air**: the two heads' lowest modes split into close pairs and beat (the survey's §3.3).

**Snare.**
- *Wrong:* hollow at 500–800 Hz (5–7 dB under), and a ~200 Hz tone ringing 1.2–1.3 s against
  0.3–1.0 s in the recordings (the "boing").
- *Fix:* both are table refits.
- *Wires:* a **cheap stochastic collision** driven by the snare-side head's displacement over a
  threshold (a one-sided contact gating a noise bank), reached through the air cavity's lag.
  This is the survey's §5.3 cheaper model; the NESS-class simulation is far from real time.
- *Ghost notes are mostly wires:* the wires' share must rise as the stroke softens, as four of
  seven recorded snares show.

**Toms.**
- *Wrong:* +12 dB at 100 Hz (studio rack tom); −6 to −8 dB at 6–10 kHz (studio floor tom).
- *Fix:* table refits, plus **per-mode air loading** (the air's mass lowers the modes with
  nodal diameters; the shell's air stiffens the round ones) and the **energy-driven pitch
  glide** (Avanzini, Marogna & Bank, JASA 2012) in place of the fitted per-layer glide.
- *Strike position* by each **Bessel mode's shape at the point**, `J_m(j_mn r/a)`, not a band
  tilt.

**Hats, ride and crash.**
- *Hats and crash:* remove the 100–200 Hz lows (hats +6 to +9 dB; the crash's boom +7 to +11
  dB), probably other mics' bleed baked into the tables.
- *The cascade:* a real-time stand-in for von Kármán wave turbulence, which full simulation
  cannot fit for a kit. Either modes plus an energy-driven spectral migration (energy moved
  upward at a rate set by the plate's energy), or a time-varying noise shaped by the plate's
  energy.
- *Ride:* the **bell's ping** (the 1 kHz note, now 10 dB short) and a **sustained ride body of
  7–10 s** (the studio ride's lows die in 0.3–3 s).
- *Hat:* **two plates with continuous openness**: their contact sets how much of each plate's
  ring the other chokes, and the "chick" (1.3–2.5 kHz, now 4–6 dB short) is that contact.

**Variation.**
- *From physical causes*, not drawn noise: strike position, velocity, stick angle and the
  head's state, each varying by the amounts the recordings' take-to-take statistics measure
  (third-octave levels, f0 in cents, onset timing). Kept inside the recordings' 1.4–2.0 dB.
- Model 6's hat click fixed per stroke is the interim answer to drawn-noise variation.

**The stick.**
- *Its own ring:* modes near 0.4, 1.0, 1.7 and 2.7 kHz for about 100 ms.
- *Its secondary contacts:* the bumps as the head's wave returns from the rim (the survey's
  §4.1–4.5).

**Stays sampled even in the modelled kits:** rimshots, cross-stick, rolls and brush sweeps.
No simulation literature beats the recordings for these (the survey's §0.3).

### 8.2 The recorded kits: a supported second family

The candidates, all in the content manifest (ADR 0030: on demand, none in the repository), already
playing through Tympanum's loader, and the references for §8.1:

| kit | licence | mics | snare: layers x round robins (counted in its files) | other articulations | installed |
|---|---|---|---|---|---|
| Virtuosity Drums (jazz) | CC0 1.0 | 6-7 positions: kick, snare, overheads, ribbon over the kit, room, lo-fi, perc | centre 36 single strikes; off-centre 36; rimshot 12; cross-stick 16; muted 16; half-open 16; buzz, flam, roll | ride, flat ride, crash, hats, two toms, kick, snare wires off | 1 470 MB |
| DRSKit (rock/pop) | CC BY 4.0 | 13 | about 26 DrumGizmo power levels a piece | snare rim, rest, hat openness | 1 887 MB |
| Big Rusty Drums (1980s rock) | CC0 1.0 | close (top/bottom) + overhead per piece | centre 10 x 4; brushes 8 x 4; dig 5 x 4; stir 4 x 4 | rimshot, sidestick, edge, mallets, a stack | 707 MB |
| Unruly Drums (indie) | CC0 1.0 | top, bottom, OH L/R | about 8 layers x 4, sticks and brushes | hat in six openings, wires on/off on every drum | 796 MB |
| Gogodze Phu II (1970s funk/soul) | CC0 1.0 | 7 on every piece | centre 24 (about 6 x 4), edge 20, side 16 | hat in five openings | 484 MB |
| MuldjordKit 3 / CrocellKit / The Aasimonster (metal) | CC BY 4.0 | 15 / 15 / 16 | DrumGizmo power levels | two kicks, chinas | 2 722 / 5 911 / about 1 700 MB |
| Swirly Drums dry (brushes), Frankensnare (12 snares, sympathetic) | CC0 1.0 | summed / per snare | brushed hits, digs, stirs / sticks, brushes, rimshots | — | 341 / 359 MB |

The loader already has DrumGizmo and SFZ multi-mic kits; one take shared by every mic;
velocity layers; round robin without repeats; each mic its own strip; the bleed matrix;
continuous hat openness; per-piece tune, damp and level; the kit's own room or Aula; streaming.

What it gains, after the modelled family's work, because these serve both families:

1. **A velocity join by spectral morphing**: one take's spectrum morphed toward the
   neighbouring layer's, never two takes summed (the root of model 5's kick at 96 under its 80;
   the survey's §8.5). The same rule joins a modelled kit's tables.
2. **A measured variation layer**: each kit's take statistics reduced to a few components, one
   draw per hit shared by every microphone, inside the recordings' 1.4–2.0 dB (§8.4). The same
   statistics drive the modelled variation of §8.1.
3. Sympathetic snare buzz and tom resonance **only where the kit differs from its recording**
   (Clifford, Lindsay Smith & Reiss, DAFx 2012).
4. Retune and damp by **modal decomposition** (Kirby & Sandler 2020).
5. Curated key maps, brush taps and sweeps, mixes, and light editions.

### 8.3 How we prove it

1. **A blind ABX of each modelled piece** against the recorded kits of its kind (the kick
   against DRSKit's, Big Rusty's, Virtuosity's and Gogodze's kicks, and so on, at pp, mf and
   ff). Done when listeners are at chance, piece by piece.
2. **The element analysis inside the recorded spread**: no third-octave band more than 3 dB
   outside, the decay per octave and the partials within the recordings' range, the
   take-to-take variation inside 1.4–2.0 dB.
3. Then a MUSHRA on the nine Groove MIDI grooves against the recorded kits and the commercial
   instruments the owner can install (MODO DRUM CS, SSD 5.5 free, BFD Player, EZdrummer or
   Superior trials).

### 8.4 Where it fits

**Round 42 (now):** model 6 as an interim (schema 86), this plan, the measures and tools
(`onset.py`, `elements.py`, `brushes.py`, the survey).

**Round 43, the modelled flagship's realism:**
- the table refits the analysis names (kick highs; snare hollow and ring; tom, hat and crash
  lows; ride bell and body);
- the Hunt–Crossley stick and beater at 3–8 ms, with the stick's ring and secondary contacts;
- the kick's coupled heads;
- the stochastic wire collision;
- per-mode air loading and the energy glide;
- the per-piece ABX kit, and the element analysis rerun after each change.

**Round 44:**
- the cymbals' cascade approximation and the hat as two plates;
- strike position by Bessel shapes;
- physical variation from the take statistics;
- then the second family's spectral layer morph, measured variation layer, sympathetic buzz
  and decomposition retune;
- the MUSHRA.

## 9. Where the session stopped (2026-11-26)

Clean at the last commit. Verified on the final code: cantus-dsp `pulsus::measured` 32/32
(the jazz ride's monotone test at 1 dB included), cantus-engine's Tympanum lib tests 14/14; on
the law alone (before the burst): `inst_contract` 51/51, `rt_memory` 68/68. Still running when
the session stopped (`F:\projects\cantus-work\tr-batch4.log`): cantus-ui `tympanum_e2e`,
`tympanum_pick_e2e`, `tympanum_hybrid_e2e` and `cargo clippy --release -D warnings` on
cantus-dsp (lib + tests) and cantus-ui's `tympanum_real` example and `tympanum_e2e` — read that
log first; the dsp tests' maths were moved to `cantus_dsp::math` for the disallowed-methods lint.
Not run: the WSL Linux check, the whole workspace suite.

**Next, in the coordinator's order (2026-11-26):**
1. **Model 6's onset** (§4.4): each mode driven through the contact force (a half-sine ≈ 0.8–1.8
   ms by velocity, longer for a mallet), the wires lagged through the cavity, the knock as the
   head's push; fitted to the recorded snares' band-peak timing (5–11 ms, lows last) and checked
   by the grooves' crest against the seven kits. Model 5 stays loadable and bit-identical for old
   projects (a new model number, as model 4 → 5 was).
2. **The re-strike** (§7.2) through the same contact force; then the e2e strikes 1 s apart again.
3. **The jazz hat's table refit** (§4.3).
4. **Brushes** (§5).
Then re-render the previews (`tools/tympanum-real/pipeline.py`, `preview.py`, `report.py`; the
work folder `F:\projects\cantus-work\tympanum-real`, the report's text
`tools/tympanum-real/informe-r42.md`).

## 10. At the integration (2026-11-27): the two reds, fixed in the model

The whole Windows suite on `r42/integration` ran this branch's last two commits' checks for the
first time; two were red, both in the model (no merge touched Pulsus).

- **A drum struck while it rings** (`onset_tests::a_model_6_drum_struck_while_it_rings_never_loses_level_on_average`):
  the studio rack tom (table 6) came out 2.56 dB under the same hit at rest on average.
  d8d3b6cd5 had turned the cymbals' re-strike bloom off for model 6's drums; turning it back on
  changed nothing (a second after its last hit a drum rings far under the bloom's 10 dB). The
  loss was in the bands: a kick's or a tom's low bands (`split`) are few and narrow, and over
  a stroke's first 80 ms their level is the noise's draw as much as the envelope's — the same
  hit drawn on other noise came out +1.9, −0.8, −1.3 and +1.7 dB. A fresh piece always began
  on one draw (the one `LAW6` was measured on); a re-struck one on whatever its noise had
  reached. Now every model 6 kick and tom stroke starts its bands on that draw
  (`FRESH_NOISE`): the same stroke sounds the same struck fresh or ringing, and the law holds
  stroke by stroke. The test: kick +0.15, snare +0.14, tom +0.41 dB on average (all above
  −0.5). The bloom is back for model 6's drums.
- **The jazz ride at 127 under its 112** (`tympanum_e2e`, seed 1, −3.5 dB): the law was right
  (+6.9 dB from 112 to 127); the table's stick burst was not — the jazz ride's 110 take bursts
  0.50, its 127 take 0.18 (`pulsus_measured.bin`), and the burst rules a ride's first 80 ms
  from 96 up, so the 127 stroke fell under the 112 as the burst's noise was drawn. For a
  `model` 2 instance a layer's burst is now at least every softer layer's of its state: the
  ride grows 112 → 127 by +8.0, +3.3 and +6.4 dB on the three seeds.

The law test over both kits, every piece and the three seeds: no step falls (the engine's own
render, as `tympanum_e2e` strikes them). Model 1 (`model` 1, model 5) is untouched — both
changes are model 6's (`gen` ≥ 6).
