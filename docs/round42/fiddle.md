# Round 42 — `fiddle`: the country fiddle

**Builder:** `fiddle` (worktree `daw-r42/fiddle`, branch `r42/fiddle`, from `r42/arcus-expr`
a8ba0ee41; `r42/arcus-expr` 87d664753 merged). **The owner (2026-11-25)**, on the fiddle demos
(*Hoedown en Re*, *Tren de la medianoche*): "It still sounds not very real. Can you improve it?
Make it sound like a country fiddle." Then: "the two notes sound like two violins harmonising,
and the fast notes lack something, they sound fake there."

ADR 0164 · schema 85 (84 reserved for Chorda's branch) · `DSP_VERSION` 17 (16 reserved).

## 1. The study: what makes a fiddle sound like one, measured

### 1.1 The references (`tools/sound-refs/catalog.toml`, set `fiddle`, measure only)

31 files, 70.6 MB, `fetch.py verify fiddle` 31/31. `tools/sound-refs/sources/fiddle/SOURCE.md`
quotes each licence, and `manifest.json` gives each file's page, author and licence.

- **Modern fiddling**:
  - TheFlyFishingFilmmaker on Freesound: 13 clips (bluegrass fiddle, a hornpipe, bluesy licks,
    double stops), CC BY 4.0, the HQ previews;
  - BoilingSand: a run, CC BY 3.0;
  - ecfike's *Denver Belle*: CC0, with guitar, timing only;
  - on Wikimedia Commons: *Willy Wagtail* (CC BY 3.0) and *Devil's Dream* (CC BY-SA 3.0).
- **US public-domain 78s** (published 1925 or earlier: the Music Modernization Act's line on
  2026-01-01), from archive.org:
  - Eck Robertson: *Sallie Gooden* (unaccompanied), *Ragtime Annie*, and *Done Gone* and *Sallie
    Johnson* (Library of Congress transfers);
  - Gilliland and Robertson: *Arkansaw Traveler*;
  - Don Richardson: *Mississippi Sawyer* (1916);
  - Fiddlin' Powers: *Cripple Creek* (1924).

  Band-limited (about 200–4000 Hz) with surface noise, so they are used only for timing, pitch and
  rhythm.
- **Classical**, already in `$CANTUS_REFS`:
  - CNSM scales: six players;
  - PHENICX's anechoic first violins: Mozart, Beethoven;
  - the HSD K465 violin stems;
  - Commons' double stops and a Bach Chaconne.
- **What failed:**
  - loc.gov (the Henry Reed collection) answers with a browser challenge (HTTP 403), so it was
    not worked around (SOURCE.md gives the hand route);
  - one Commons file got HTTP 429, and the same recording is in the set from the LoC transfer.
- **Limits:**
  - few CC0 or CC BY solo country-fiddle recordings exist, so the modern side is essentially one
    player plus three tunes;
  - previews are MP3: encoding our own WAV moved the noise and HNR measures by ≤ 0.3 dB.

### 1.2 The tool (`tools/arcus/fiddle/`)

- **The parts:**
  - `fiddle_lib.py`: the YIN track, 2–8 kHz comb-residual noise, band-flux onsets and a
    harmonic-sieve two-pitch detector;
  - `fiddle_measure.py`: the measures (`run`, `table`);
  - `fiddle_groups.py`: the files by group;
  - `fiddle_report.py`: the pooling;
  - `fiddle_truth.py`: a `.cantus`'s notes as our renders' ground truth;
  - `fiddle_quick.py`: the headline measures for a few files, side by side, for the lab.
- **Estimator reliability:**
  - onsets against our notes: recall 0.93–0.99, precision 0.85–0.93;
  - the two-pitch detector: 2 % false dyads on single notes, blind to octaves and unisons;
  - **the room is the big confound**: the references have room sound and our stems are dry, so
    the fast-note noise and ring measures are compared with the dry classical recordings
    (CNSM, PHENICX anechoic).
- Results: `$CANTUS_WORK/arcus/fiddle/fiddle_measures.{md,json}`.

### 1.3 What the measures say (median [p25, p75])

| measure | real fiddle (bluegrass / 78s) | classical | ours (round 42 demos, dry stems) |
|---|---|---|---|
| fast notes with a re-articulation (flux) cue | 0.15 / 0.10 | 0.16 (CNSM), 0.43–0.59 (anechoic) | **0.73** |
| the previous pitch 60 ms after the next onset | −5.8 / −8.8 dB (rooms) | −10.5 (CNSM); −5 … −22 (anechoic) | **−30 dB** |
| fast notes whose first 20 ms read an octave off | 0.06 | 0.06 | **0.33** |
| steady noise-to-harmonic 2–8 kHz | −6.7 dB (rooms) | −22 (dry) | −16.6 |
| level dip at a bow change | 4.7 / 4.9 dB | 3.2–5.6 | 5.6 |
| swing (long / short eighth) | 1.24 / 1.18 | 1.09 (floor) | 1.09 |
| accent, long eighth minus short | +1.1 / +1.1 dB | +0.4 | **−1.3** |
| vibrato on notes ≥ 0.3 s | 12 % / 14 %, ~54 c p-p, 6 Hz | 61 %, 39 c | 0 % |
| major third over the minor | 96 / 92 cents (78s: 14 % neutral) | 53–92 | 100 |
| double stops: lower minus upper voice | **+1.7 / +2.4 dB** | −1.2 | **−4.6** |
| double stops: interval off equal temperament | 8.7 / 16.4 cents | 7.6 | **0.7** |
| double stops: interval wobble | 5.5 / 5.8 cents sd | 7.4 | 4.0 |
| centroid / f0 | 2.21 | 2.00–2.35 | 1.24 |
| decay after the sound stops (T60) | 1.35 s (rooms) | 0.36–0.93 | 0.21 dry |

Read plainly:

- **Fast notes.** Ours were re-articulated one by one (the demo lifted the bow between strokes)
  and stopped dead: the string the bow had left fell silent at once, where a real fiddle's
  rings on under the next note.
- **Double stops.** Ours were two players: a quieter lower voice in perfect equal temperament,
  two bows' attacks and noises. Real ones are level or lower-heavy and lean toward just
  intervals.
- **Rhythm.** The swing is wider and the accent falls on the long (on-beat) eighth.

### 1.4 The literature (the style's teaching, as `arcus-expr.md` §4.1 summarised it)

- Stacy Phillips, *Complete Country Fiddler*; Brian Wicklund, *American Fiddle Method*; Mark
  O'Connor's *Fiddle Method*; Anger & Eggleston, *Chops & Grooves*.
- The bow nearer the bridge and heavier, a grittier sound, open strings let ring, double stops
  and drones on a flatter bridge, the Nashville and Georgia shuffles, the chop on 2 and 4.
- Exact pages were not checked in this session.

## 2. What changed (ADR 0164)

See the ADR for the model. In short:

- **`bow.pair`**: one bow on two strings in one voice: double stops, drones, a crossing's ring;
  `bow.tilt` and `dstop.just` shape the double stop.
- **`sym`**: the open strings ring in sympathy through the bridge.
- **`bow.grit`**: the hair's noise rises near the bridge and with the force.
- **`steel`**: steel strings.
- **`Articulation::Chop`**: from the air at the frog, landing past the Helmholtz region at 2.5 ×
  its upper limit, at 0.3 of the stroke's speed, the hair noise +12 dB, stopped on the string
  after 60 ms.

Every new key is off at its default: nothing changes, to the bit. The *Fiddle* preset turns
them on.

The demos (`crates/cantus-ui/examples/arcus_demo/fiddle.rs`, `fiddle2.rs`), as measured:

- **Drones and double stops** go in the fiddle's own clip (the second Arcus is removed), at the
  melody note's velocity.
- **Separate strokes** change bow on the string (2 ms between, not 0.04 beat).
- **The lilt** rises to 0.05 beat (the off-beat at 0.55 of the beat).
- **The long (on-beat) eighth** takes the accent (92/89 against 84).
- **The onsets** are looser (±0.02 beat).
- **The chop** is `Articulation::Chop`.

## 3. Before and after

(Filled in below as the renders are measured.)

## 4. For the owner's ears

`F:\projects\cantus-work\listen\fiddle\escuchar.html` (Spanish); `chat\` holds 16-bit WAV copies.

## 5. Open

- **Bow rocking** (the survey's item 4): the tilt as a curve moving between the strings at
  the stroke rate (a per-note expression or a CC, and a player rule rocking a shuffle's drone).
  The tilt is a setting and a velocity balance today.
- **The bridge's coupling back into the bowed strings.** The sympathetic strings are driven
  one-way, and a double stop's two strings do not pull on each other.
- **The cross-tunings' own pitch rows.** An open string tuned up or down is corrected by its
  key's row, which was measured on a standard string; the closed loop trims the rest.
- **The chop against a recording.** No chop under a free licence was found, so its 2.5 ×
  force, 0.3 × speed and +12 dB noise are the design's (`arcus-expr.md` §4.3), not a
  measurement.
- **The per-string level of the violin.** The probe reads the open A (and C#5) about 8 dB over
  D-string and E-string notes at one velocity (`probe_double_stop_levels`). That is the
  instrument's, not the pair's, and is for the violin's next pass.
- **Turns, rolls and the swung feel as a groove** stay in the notes, as `arcus-expr.md` said.
