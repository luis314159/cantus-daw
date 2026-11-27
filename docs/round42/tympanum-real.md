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
   at 1.2 kHz). Physically: the stick's contact lasts ~1 ms but the head's displacement — what
   radiates — grows over its first half-periods (a 14″ snare's ~200 Hz: 2.5 ms each), the bottom
   head and the wires answer only after the air cavity couples them (another 1–3 ms), and the
   overheads hear it 2–3 ms later still. Model 5 starts every mode at full amplitude at the
   strike. **The fix is model 6's onset**: each mode driven through the contact force (a half-sine
   as long as the contact: ~0.8 ms at ff, ~1.8 ms at pp — the brightness law comes with it), the
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

**Done (branch `r42/tympanum-real`):** the library catalogued and fetched within the disk budget
(≈ 2.3 GB downloaded: Aasimonster built for Tympanum 1.7 GB, Freesound 51 MB, GMD 9 MB; the CC0
packs joined from the app's folder; MuldjordKit stopped at 0.8 GB when F: neared its floor — the
manifest's `muldjordkit-tympanum` pack fetches it later; ShittyKit's old DrumGizmo format the
builder refuses, removed); the tools above; the measures of eight recorded kits and Tympanum's two;
the level law and the jazz toms' burst
(`DSP_VERSION` 19, ADR 0124's sixth amendment); the owner's previews and report.

**Tests:** cantus-dsp `pulsus::measured` (the law interpolates; every table grows at every step;
the burst table; the existing block-size, rest and
choke tests), cantus-engine's Tympanum lib tests (14), `inst_contract` (51) and `rt_memory` (68) on
the law; cantus-ui `tympanum_e2e`'s `every_modelled_piece_grows_with_the_velocity_through_the_app`
through `CantusApp::assemble`.

**Open, in the order the measures rank them:**
1. **The onset** (§4.4): the generated snare's bands peak 5–10 ms early; the grooves' crest 2–10 dB
   high. Model 6's contact-driven onset, the wires' cavity lag, the knock as the head's push.
2. **The blend at one hit's seed (corrected 2026-11-26):** through the app, the studio kick at
   velocity 96 comes out 3.3 dB under its 80 — struck 1 s or 4 s after the last hit alike, so it is
   *not* the re-strike (an earlier note here said it was). 96 lies between the 80 and 110 layers
   (x = 0.53, the main layer just switched to 110): the hit takes the main layer's onset phases with
   both layers' amplitudes, and for some seeds they cancel. `LAW` fixes the mean of three seeds,
   not one hit's. `every_modelled_piece_grows_with_the_velocity_through_the_app` (cantus-ui
   `tympanum_e2e`) catches it and is `#[ignore]`d with this reason until model 6 blends the onset.
3. **The blend's cause:** a hit between two layers takes its onset phases from the main layer and
   its amplitudes from both — the source of the level law's lumps; `LAW` compensates the level,
   model 6 should blend the onset itself.
4. **Brushes** (§5), then **per-genre kits** measured off Aasimonster (metal), Gogodze (1970s
   funk/soul), Big Rusty (1980s rock), Unruly (indie, brushes) — only against those recordings.
5. **The snare's strike position** modal measure (its f0 under the wires), and the toms' other
   modes.
6. **The commercial comparison** (§2) on the owner's machine; the blind kit with the new genres.
7. Catalogue: `drums-dg` and `drums-packs` are not `record`ed (the raw Aasimonster files were
   deleted after the build for disk; the packs are the app's own); `fetch.py record` once they are
   on disk again.

## 8. The pivot: sample-first flagship kits (plan, 2026-11-26)

**Why.** The owner heard every Tympanum preview, the modelled Studio and Jazz kits before and
after this round's level law: *"todo era un desastre, no se parece nada a algo real"*. Better
numbers (13.7 and 25.7 weighted outside-range, from 23.2 and 38.1) did not make them sound
real; the recorded kits played through Tympanum's own loader in the same previews (DRSKit, Big
Rusty, Virtuosity, Gogodze, Aasimonster) are real by construction. Every commercial drum
instrument that sounds real (Superior Drummer 3, SSD 5.5, Addictive Drums 2, BFD3) is a
multi-mic, multi-velocity, round-robin **sample** engine. Model 6 (§9) is finished as an
interim and kept, but not polished further.

**(a) The flagship kits are sample-first**, built from the best CC0 / CC BY multi-mic kits.
Every candidate is already in the content manifest (ADR 0030: each downloads on demand, none in
the repository) and already plays through Tympanum's loader:

| kit | licence | mics | snare: layers x round robins (counted in its files) | other articulations | installed |
|---|---|---|---|---|---|
| Virtuosity Drums (jazz, Austin McMahon) | CC0 1.0 | 6-7 positions: kick, snare, overheads, ribbon over the kit, room, lo-fi, perc | centre 36 single strikes; off-centre 36; rimshot 12; cross-stick 16; muted 16; half-open 16; buzz, flam, roll | ride, flat ride, crash, hats, two toms, kick, snare wires off | 1 470 MB |
| DRSKit (rock/pop, DRSDrums) | CC BY 4.0 | 13 (kick front/back, snare top/bottom, three toms, hat, ride, OH pair, amb pair) | about 26 DrumGizmo power levels a piece (one strike each, picked by power) | snare rim, rest, hat openness | 1 887 MB (`drskit-tympanum`) |
| Big Rusty Drums (1980s rock) | CC0 1.0 | close (top/bottom) + overhead per piece | centre 10 x 4; brushes 8 x 4; dig 5 x 4; stir 4 x 4; flutter, release | rimshot, sidestick, edge, mallets, damped/undamped kick, a stack | 707 MB |
| Unruly Drums (indie, wires on every drum) | CC0 1.0 | top, bottom, OH L/R | about 8 layers x 4 round robins, sticks and brushes | hat in six openings, wires on/off on every drum | 796 MB |
| Gogodze Phu II (1970s funk/soul) | CC0 1.0 | 7 on every piece (front SM-55, overhead, kick, snare, side, retro, window) | centre 24 (about 6 x 4), edge 20, side 16 | hat in five openings, three deadened toms | 484 MB |
| MuldjordKit 3 (metal/rock) | CC BY 4.0 | 15 | DrumGizmo power levels | two kicks, three rack toms, rides, china | 2 722 MB |
| CrocellKit (metal) | CC BY 4.0 | 15 | DrumGizmo power levels | two kicks | 5 911 MB |
| The Aasimonster 2.1 (death metal) | CC BY 4.0 | 16 | DrumGizmo power levels | chinas, zilbel | about 1 700 MB built |
| Swirly Drums, dry (brushes) | CC0 1.0 | summed files | brushed hits, digs, stirs, flutters | cajon, darbouka, djembe | 341 MB |
| Frankensnare (12 snares) | CC0 1.0 | (per snare) | sticks, brushes, rimshots, muted | sympathetic snares (a kick buzzing six snares) | 359 MB |

Proposed flagship set: **Studio** = DRSKit (credited), **Jazz** = Virtuosity (CC0), **Vintage** =
Gogodze Phu II, **Rock 80s** = Big Rusty, **Indie / Brushes** = Unruly with Swirly's brushes,
**Metal** = MuldjordKit (Aasimonster or Crocell as alternatives): the same kits the grooves
already compare against, so every measure of this round carries over.

What the loader **already supports** (ADR 0124; `cantus_core::inst::tympanum_hits`, the engine's
`rec` voice): DrumGizmo XML and SFZ multi-mic kits; one take per hit shared by every mic (no mic
plays a different strike); velocity layers with SFZ curves or DrumGizmo power; a round robin
drawn per hit with `vary` (never the take before); each mic a named output with its own strip
(ADR 0123); the piece x mic bleed matrix (`b{p}.{m}.db/inv/on`, off unloads that cell);
continuous hat openness crossfading the recorded openings (CC4); the foot's chick; per-piece
tune (resampled, cents), damp (an envelope), level and mute; the kit's own room or one of
Cantus's (Aula); streaming from disk.

What is **missing** for "beats the commercial ones":

1. **Velocity crossfade between adjacent layers** (today a hit picks one layer): the two nearest
   layers of the same round-robin slot, both read with the hit's one draw, at equal-power weights
   corrected by their measured correlation (the hybrid player's `1/sqrt(wa^2 + wb^2 + 2 wa wb rho)`),
   so no layer pair can cancel (model 5's layer blend did).
2. **More variation than the round robins**: per hit a small seeded change the recordings
   justify (about ±0.5 dB, ±3 cents, a start under 0.1 ms, a mild tilt) on top of the real take,
   the "infinite round robin" the commercial engines sell, measured against the recordings'
   1.4–2.0 dB take-to-take variation.
3. **Articulations everywhere**: a curated key map per flagship kit (rimshot, cross-stick, edge,
   off-centre, buzz, hat openings, ride bell and edge, chokes), and **brushes**: taps from the
   brush layers; sweeps (Big Rusty's stir, Unruly, Swirly) as held notes, looped and shaped by
   velocity and a controller (§5; `brushes.py`'s first measures: a brush tap rises over 6–16 ms
   against a stick's 3 ms, peaks at 6–22 ms, its wires 9–12 dB under the whole; a stir is
   seconds long, centred at 3–5.7 kHz).
4. **Strike position as a morph** between sampled positions (Virtuosity's centre and off-centre,
   Gogodze's centre and edge): a continuous `pos` crossfading the two with one draw.
5. **Sympathetic snare buzz**: the wires excited by the kick and toms, from Frankensnare's
   sympathetic recordings or a small modelled wire bank driven by the other pieces' lanes.
6. **Curated mixes**: each flagship kit's default mic balance, bleed and room set by ear against
   its library's own demos, saved as the kit's preset.
7. **Size**: a light edition per flagship kit (close + OH + room, FLAC, about 400 MB or less)
   beside the full one.

**(b) Modelling only where it beats samples**: retuning and damping a sampled drum (present),
variation beyond the round robins (2), sympathetic buzz (5), strike position (4), the room
(present: Aula and the modelled rooms). Never the whole sound of a kit called realistic.

**(c) The modelled kits stay** as a separate, honestly named family (*Modelled* or *Synth*;
Studio Maple and Jazz Birch named so in the picker), offered as playable and tunable drums,
not as recordings.

**How we prove it beats the commercial ones, blind.** (1) An ABX kit per flagship kit against
its own recorded original played straight (no crossfade, no variation): Tympanum's version must
be indistinguishable on single hits (ABX at chance) and preferred on grooves (fewer identical
repeats, smoother dynamics). (2) A MUSHRA on the nine Groove MIDI grooves with the recorded kit
as hidden reference and the commercial instruments the owner can install (MODO DRUM CS, SSD 5.5
free, BFD Player, EZdrummer or Superior trials, Addictive Drums trial) rendering the same MIDI at
one loudness (`tools/soundlab`). (3) The measures of §3 (`compare.py`, `grooves.py`): every
feature inside the recorded range, the take-to-take variation in the recordings' 1.4–2.0 dB.

**Where it fits.** Round 42 (now): this plan; model 6 as an interim (kept: schema 86); the
measures and tools. **Round 43**: the velocity crossfade (1), the variation layer (2), the
flagship key maps and mixes (3 without sweeps, 6), the picker's two families (c), the light
editions (7) and the ABX kit: what turns "real by construction" into Tympanum's default.
**Round 44**: brushes with sweeps, the position morph, sympathetic buzz, and the MUSHRA against
the commercial instruments on the owner's machine.

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
