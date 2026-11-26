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
- **`tuning.strings`**: the violin's cross-tunings (standard, AEAE, GDGD, ADAE, calico, DDAD).
- **`body.mic`**: one close microphone instead of the pair (§3.1).
- **`body.bridge`**: a fiddle's bridge. Its rocking mode's hill is raised (+4 dB at 2.3 kHz,
  ±1.1 octaves) and its top falls from 4.5 kHz (−8 dB), on the body's modes themselves; it is
  not an EQ.

Every new key is off at its default: nothing changes, to the bit. The *Fiddle* preset turns
them on, with the bow nearer the bridge (×0.73) and heavier (×1.32).

The demos (`crates/cantus-ui/examples/arcus_demo/fiddle.rs`, `fiddle2.rs`), as measured:

- **Drones and double stops** go in the fiddle's own clip (the second Arcus is removed), at the
  melody note's velocity.
- **Separate strokes** change bow on the string (2 ms between, not 0.04 beat).
- **The lilt** rises to 0.05 beat (the off-beat at 0.55 of the beat).
- **The long (on-beat) eighth** takes the accent (92/89 against 84).
- **The onsets** are looser (±0.02 beat).
- **The chop** is `Articulation::Chop`.

## 3. Before and after

### 3.1 Why the songs were dark (and the lab's phrases were not)

The study read our song stems at centroid / f0 1.24 and −15 … −17 dB above 2 kHz, against
the references' 2.21 and −6.6, with a hole at 1.6–2.5 kHz (−25 … −32 dB re the total against
−12). The lab's phrases, in the same register and at the same CC1-like dynamic, read 2.0–2.2
and −8 … −9 (`fiddle_ltas.py`, `fiddle_quick.py`).

- **The dynamic, the force, the contact, steel and register** each moved the lab's phrases by
  tenths: `dyn` 0.4 / 0.62 / 0.85 gave 2.17 / 2.10 / 1.90.
- **The difference was the channels.** The lab wrote the left side only, while the study sums
  a stem's two sides. The violin's body has a *pair* of microphones (`upper::body::radiation`):
  above 700 Hz each mode reaches each side with its own sign (45 % in opposite phase) and
  strength, so the sum cancels part of it.
  - A Hoedown stem's sides are correlated 0.64; each side alone reads 1.89 / 1.93 and
    −10.9 / −11.9 dB.
  - In the lab, the mono sum of the pair reproduces the songs: the passage reads 1.19 and
    −16.1 dB.
- **The fix, in the fiddle character:**
  - **`body.mic`** = one close microphone (both sides hear the left one's radiation), as a
    fiddle is miked for country. The mono sum loses nothing: the passage reads 2.02 and
    −9.1 dB.
  - The preset's bow sits **nearer the bridge (×0.73) and heavier (×1.32)**. In the lab this
    gives:
    - the fast notes' re-articulation cues 0.48 → 0.17 of the onsets (references 0.10–0.17);
    - the steady noise between the harmonics −16.5 → −10.7 dB;
    - the previous note at +60 ms −9.5 → −5.8 dB.
- The pair stays the classical presets' default. Each side of a stereo listener's pair sounds
  as the match measured it, and the classical presets' held-out rows are untouched.

### 3.2 The fiddle's own stem, before and after (`fiddle_quick.py`)

| | Tren before | Tren after | Hoedown before | Hoedown after | references |
|---|---|---|---|---|---|
| centroid / f0 | 1.29 | 2.16 | 1.24 | 2.10 | 2.21 (bluegrass) |
| above 2 kHz, dB re total | −13.8 | −8.2 | −15.4 | −10.1 | −6.6 |
| fast onsets with a re-articulation cue | 0.71 | 0.08 | 0.77 | 0.04 | 0.10–0.17 |
| the previous note 60 ms into the next, dB | −29.6 | −8.2 | −31.2 | −4.3 | −10 … −16 dry, −6 … −9 rooms |
| fast notes starting an octave off | 0.00 | 0.00 | 0.00 | 0.00 | 0.06 |
| a double stop's lower minus upper voice, dB | (two instruments) | −2.4 | (two) | −4.0 | +1.7 |

- "After" here is the second render (the close mic, the bow nearer and heavier); the first
  one-bow render, before the close mic, measured flux 0.25 / 0.24 and the previous note −15.2 /
  −19.9 dB, its timbre still the pair's mono sum (1.25 / 1.23).
- The noise between the harmonics in the songs reads −3 dB, but it counts a double stop's
  second voice as noise. On single lines the lab reads −10.7 dB (the references −6.7 with
  their rooms, dry classical −22).
- AEAE *Tren* (the take for the owner to compare): 2.13 and −8.2 dB, its double stops'
  lower voice +1.5 dB over the upper.

### 3.3 The lab's phrases (`examples/fiddle_lab.rs`, both sides summed)

| | rapidas before | rapidas after | the references |
|---|---|---|---|
| fast onsets with a re-articulation cue | 0.68 | 0.17 | 0.10–0.17 |
| the previous note at +60 ms | −28.7 dB | −5.8 dB | −10 … −16 dry |
| steady noise between the harmonics (2–8 kHz) | −17.4 dB | −10.7 dB | −22 dry classical, −6.7 fiddle in rooms |
| the sympathetic tail 0.2–1 s after the bow lifts (re the note) | −40.2 dB | −29.6 dB (sym 1) | — |
| a chop's periodicity (the open D, 60 ms) | 0.72 (a staccato) | 0.00 | — |

### 3.4 The gates (Windows, this branch, release)

- **`cantus-dsp` `arcus::`**: 123 passed, 0 failed, 48 ignored probes. Among them:
  - `LOWER_HASHES`, `the_player_off_keeps_pass_six_s_bits` and every held-out gate: unchanged
    — at their defaults the new keys change no sample;
  - `tests_fiddle`'s eight:
    - the same bits at block sizes 1, 31, 64, 256 and 1000, everything on;
    - rest;
    - a double stop as one voice, F#5 tuned 0.6 of the way to 5/3;
    - no pair: a slur as before;
    - a crossing's ring;
    - the sympathetic ring;
    - the chop's periodicity 0.00 against a staccato's 0.72;
    - every cross-tuning's open strings.
- **`cantus-engine`**:
  - `inst_arcus` 9: the fiddle's keys at their defaults null, a fiddle shape at every block
    size;
  - `null_tests` 26;
  - `rt_memory` 68: `an_arcus_fiddle_never_allocates`, on the audio thread and the pool;
  - `arcus_image`: 770 KB a fork, `approx_bytes` moved;
  - `strings_rec_art` 4.
- **`cantus-ui`**:
  - `arcus_fiddle_e2e`, through `CantusApp::assemble`, the picker and the preset browser:
    - both notes of a one-bow double stop at −10.3 dB of each other, against −23.1 without
      the pair;
    - F#5 at −9.3 cents (the just sixth × 0.6: −9.4);
    - the sympathetic tail +4.1 dB;
    - the close mic and the fiddle's bridge +13.0 dB at 1.6–3.2 kHz;
  - `arcus_level_e2e`: every bowed preset's music level in the spread, the fiddle's included;
  - `arcus_upper_e2e`, `arcus_tuning_art_e2e`, `arcus_pick_e2e`;
  - `ai_mcp_boot`: the tools list re-dumped with the chop;
  - i18n: every English key in Spanish.
- **`cantus-core`**: the undo fuzz 19 (its `SetNoteArt` arm draws from `Articulation::ALL`, the
  chop included); the lib tests 55.
- **`cantus-project`**: `schema_85_keeps_arcus_s_strings_under_one_bow` (fixture v85; v83 reads
  the defaults) and every fixture loading.
- **`cargo clippy --release --all-targets -- -D warnings`** on cantus-core, -dsp, -engine, -ai,
  -project, -score and -ui: clean; rustfmt on every changed file. The Linux check
  (`tools/wsl/check.sh`) was not run: no `cfg(windows)` code was added.
- **`DSP_VERSION` 17**: at the defaults the measured size of the change is zero — no golden
  hash and no `LOWER_HASHES` render moved. The fiddle preset sounds otherwise (§3.2).

## 4. For the owner's ears

`F:\projects\cantus-work\listen\fiddle\escuchar.html` (Spanish, `tools/arcus/fiddle/listen_page.py`).
`chat\` holds 16-bit WAV copies:

- the demos: `ahora-tren.wav`, `ahora-hoedown.wav`, `ahora-pasaje-country.wav`,
  `ahora-pasaje-clasico.wav`, each beside its `antes-*.wav`, and `ahora-aeae-tren.wav` (*Tren*
  in AEAE);
- the lab's phrases: `lab-{dobles,rapidas,bordon,simpatia}-{antes,ahora}.wav`;
- the comparison: `comparar-hornpipe-ref.wav`, a CC BY 4.0 hornpipe phrase by
  TheFlyFishingFilmmaker, beside `comparar-hornpipe-cantus.wav`, our fiddle playing the notes
  `transcribe.py` read from it (automatic, approximate). This is for the owner's ear only and is
  never shipped.

The full takes are in `ahora\` and `ahora-aeae\`: each 24-bit export, its stems and its
`.cantus`.

To render them again:

- the demos: `cargo run -p cantus-ui --release --example arcus_demo -- <dir> fiddle`
  (`FIDDLE_TREN_AEAE=1` for the AEAE *Tren*);
- the lab: `cargo run -p cantus-dsp --release --example fiddle_lab -- <dir> [key=value …]`;
- the page: `python -I tools/arcus/fiddle/listen_page.py`.

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
- **The 5 kHz band** sits about 4 dB over dry classical violins (and 9 over the bluegrass
  previews). If the owner hears sizzle, the next knobs are the bridge's top (`BRIDGE_TOP_DB`)
  or less steel.
- **The owner's ear decides.** Everything here is measured, not yet heard by him. The takes he
  has are listed in §4.
- **Numbers for the integrator:** schema 85 (84 left for Chorda's branch) and `DSP_VERSION` 17
  (16 left) may be renumbered; ADR 0164 is provisional.
  *At integration (`r42/integration`, after chorda-c17): kept as they are — Chorda took
  schema 84 and `DSP_VERSION` 16; v85.cantus written again by the merged code.*
