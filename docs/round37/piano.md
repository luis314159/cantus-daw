# Round 37 — piano: Chorda's second model, voicing pass two, Sectio

ADR 0098 (Chorda's model 2, amended twice), ADR 0099 (Sectio) · schemas 54 and 55 · branch
`r37/piano`. The first delivery's report follows the two later sections.

## The integrated tree's review of Sectio (fixed)

One preset path for the editor, A/B, the library, the agents and the audition (a style
re-cuts only as shipped, by its own mode; A/B and saved presets restore exactly; the audition
re-cuts on its worker); each slice's settings follow its audio through add, remove and
*Apply*; a style's choke is set once; 64 slices spread over a long loop (strongest
transients, a coarser grid, said); a grid borrows the song's tempo; edited points on the
dump's samples; loops past 2²⁴ frames refused; non-finite samples silenced. Tests:
`tests/sectio_review3.rs` (8, through `CantusApp::assemble`) and `sectio_agent`'s
`apply_preset_with_a_style_recuts_the_users_loop` (through the MCP server).

## The closing list (Sectio's gaps, the second review)

- **The slice editor** (`views/inst/sampler_slices.rs`, `sectio_styles.rs`): points dragged
  (onto the nearest zero crossing), added (double-click), removed (right-click); *Slice again*
  by markers, transients (sensitivity) or a grid (1/4 … 1/32) with a preview and *Apply* /
  *Cancel* — each one undo step, the zones rebuilt from the points. The zone low-pass is a
  column of the zone table. An empty Exemplar keeps its keyboard map.
- **Entry points**: the sample browser's menu on a file (*Slice into a new Exemplar track*,
  *Slice into the open Exemplar*), the import menu, a drop on the card (one file, taken from
  the frame so the arrangement does not import it too), the agents. The e2e starts from the
  browser's menu.
- **Slicing styles**: six Exemplar presets in the library that re-cut the user's own loop
  (no audio ships); on an empty Exemplar they play built-in sounds to audition.
- **Security**: `slice_sample` is a *Files* tool on resolved paths (outside, `..`, files
  off: refused; tested through the MCP server). Hostile WAVs (huge cue/smpl counts, a data
  length past the file) read at once. `at_bar` held in range; a dump with no tempo uses the
  song's or refuses.
- **Exemplar's zone low-pass** decided at the trigger and applied under the instrument's
  filter only when that is a low-pass; **Chorda model 2's width** only the stage's (the keys
  keep their places; tested on the mono sum of A0 and C8).
- **ACID from outside**: three chunks Acidizer wrote, from uttori-audio-wave's MIT test
  assets, byte for byte, with uttori's own decode as the expectation.
- **The detector** (`onset::Options`, warp's path untouched — `detect` is bit-identical,
  warp's tests pass unchanged): banded picking kept (rendered set 90.0 → 91.9 %, MDB 73.9 →
  75.8 % within 10 ms); adaptive whitening (58.6 %) and backtracking (44.6 %) measured and
  dropped — the points already lie a median 3.5 ms before MDB's annotations, so 95 % within
  10 ms is not met (ADR 0099 has the reasons with the numbers).
- Chorda's 24 presets 2 dB lower (the keys role's −21 LUFS target).

## Sectio, Exemplar's slicer (ADR 0099, schema 55)

**What it is.** A loop dropped on an empty or sliced Exemplar's *Slices* card (from the
system or the sample browser), or *Slice a loop…* in Exemplar's import menu (two clicks),
is cut into slices — at its own markers (WAV `cue ` points, else `smpl` loops; an ACID chunk
gives the tempo), else at its transients (SuperFlux, two listenings: the signal and its first
difference, so a quiet hi-hat under a kick is found), on a beat grid, or as one slice by
hand — one undo step. Slice *i* is zone *i* on key C1 + *i* at its own pitch, in group *i*
(its envelope); per slice: pitch, gain, pan, reverse, choke, envelope and a low-pass of its
own (`z.lpf`, new). *Dump to piano roll* writes the slices as a clip that **replays the file
to the bit at the loop's tempo** (the points are snapped to the samples the engine starts
notes on); at another tempo the slices follow the grid. *Random*, *Stutter*, *Quantize* —
one undo step each, seeded. Agents: `slice_sample`, `dump_slices`. Manual EN/ES; the
tools' table in `ai-tools.md`.

**Measured** (slice points within 10 ms of the truth; hits closer than 30 ms are one):

| Material | Hits found | Points on a hit |
|---|---|---|
| 50 loops rendered by Cantus (Pulsus kits and Lamina chords, 80–160 BPM, ghost notes; exact truth), sensitivity 0.7 | **90.0 %** (778/864) | 99.6 % |
| — before the second listening | 74.2 % | 99.4 % |
| **MDB Drums** (23 real drum recordings, hand-annotated; 314 loops of 4 s), sensitivity 0.5 | **73.9 %** (4 401/5 956) | 81.1 % |
| — the same within 20 ms / 50 ms | 88.0 % / 91.0 % | |

The sensitivity does not move MDB's figures (0.5 and 0.8 give the same points: every onset
the detector reports is within 26 dB of the strongest) — what is missed is missed by the
detector, and a seventh of the hits it finds lie 10–20 ms from the annotation. By style on
MDB: disco 97 %, rockabilly 100 %, gospel 92 %, country/80s rock/Hendrix
88–90 %; brushed and ridden jazz 43–65 % (swing, bebop, free). **The brief's 95 % within
10 ms is not met** on either set; what misses on the rendered set is the sustained chords'
re-attacks under a ringing chord (Lamina) and a few soft hi-hats. MDB Drums is CC BY-NC-SA:
measured locally (`tests/sectio_mdb.rs`, ignored), never committed. CC0 loops annotated by
ear are the owner's item (`docs/beta-checklist.md`).

**Formats from outside**: three WAVs written by **libsndfile 1.2.2** (cue points in a
16-bit mono file, written out of order; sampler loops in a 24-bit stereo one; a float file
with both) — `cantus-dsp/tests/sectio_markers.rs`, provenance and hashes in
`tests/data/sectio/README.md` (87 KB, a `.gitignore` exception). The ACID chunk is tested
only with bytes laid out by hand from its published layout.

*At integration:* the closing list below built the browser's *Slice* menu, dragging points and
re-slicing from the card, and the sampler presets; only slices to separate outputs stay
unbuilt. The final accuracy is 91.9 % rendered / 75.8 % MDB, and the owner's kit is
`abx-kit3` (`docs/round37/integration.md`).

**Not built** (said plainly, at the time): a *Slice* item in the browser's context menu (the drop and
the import menu do it); moving a slice point by dragging it on the card (the points are
parameters; the zone table edits a zone's start); re-slicing at another sensitivity from
the card (the agents' `slice_sample` takes it); factory presets for Sectio (a preset would
need a loop to ship with it — packs download on demand, ADR 0030 — and the sampler's
presets are library's file); slices to separate outputs (each slice has its own group, so
drums's `process_buses` can take a per-group output later).

### Tests and marked lines of these two deliveries

- `cantus-dsp`: `chorda::tests_stage` (8: every control at its stored default plays the voiced
  piano to the bit; moved controls keep any block size and the threads; the perspective's
  room tail; the air −6.7 dB far / +2.0 dB near on the stage; a closed lid takes the top;
  the width never moves the mono sum; a larger room rings longer and still rests; a moved
  control glides — no step), all 50 `chorda::` tests; `sectio::tests` (9);
  `tests/sectio_markers.rs` (3, libsndfile's files). Model 1's pinned hash holds.
- `cantus-engine`: `inst_chorda` 14, `inst_contract` 34, `rt_memory` 32.
- `cantus-ui`: `sectio` 6 (with **the e2e**: a loop dropped through `CantusApp::assemble`
  plays its dump live, equal to the offline render to the bit — it waits for the zone tables
  the engine holds back until their starts are read, as the window's frames do),
  `sectio_view` 2, `sectio_accuracy` (holds the measured 88 %+ recall and 95 %+ precision —
  not the brief's 95 % recall), `sectio_mdb` (ignored, needs the dataset: holds 72 %+), `inst_chorda` 11, `chorda_model2_e2e`,
  `instrument_levels`, `preset_render` (Chorda's 24 pass; other units' CPU budgets failed on
  the loaded machine), `ai_mcp_boot` (the golden re-written: `slice_sample`, `dump_slices`),
  `locale_isolation`, the i18n lib tests. `chorda_kit/abx_check.js` (node) for the ABX page.
- `cantus-project`: every test, `sectio_schema` (the newest fixture's sliced loop).
- Clippy `-D warnings --all-targets` on the five crates: clean. `cargo fmt`: clean.
- Linux (WSL, `tools/wsl/check.sh r37/piano clippy`, workspace, `-D warnings`) on 34ab9373:
  clippy exit 0, the plug-ins without the editor exit 0.
- Marked lines (`r37:piano`): `.gitignore` (the marker files), `crates/cantus-dsp/src/lib.rs`
  (`sectio`, `DSP_VERSION` 6), `crates/cantus-dsp/src/exemplar*.rs` and
  `crates/cantus-engine/src/inst/sampler.rs` (the per-zone low-pass), `cantus-core`'s
  `lib.rs` and `inst/sampler.rs` (`sectio`, `z.lpf`), `cantus-ui`'s `lib.rs`, `ai_tools.rs`,
  `samples_drop.rs` (`Target::Slicer`), `views/inst/sampler*.rs`, `views/inst/frame.rs`
  (`window_in_sized`, for a whole view's shot), `views/inst/mod.rs`.
- **Merging into r37/integration** (checked with `git merge-tree`): two conflicts, both
  additive — `crates/cantus-ui/src/ai_tools.rs` (keep both registrations) and the tools/list
  golden (re-write it with `CANTUS_TOOLS_DUMP=1`); `format.rs` merges by itself. After the
  merge re-write the newest fixture (`write_fixture`: v57 must carry the schema-55 sliced
  loop, which `tests/sectio_schema.rs` reads).

## Voicing pass two: the owner's ABX (Chorda model 2)

The owner's ABX (2026-11-05): ABX 1 15/16, ABX 2 15/16 (p = 0.0003); Chorda 2 over Chorda 1
7/7, the Salamander over Chorda 2 7/7 — *"muchos medios, algo borroso, un poco nasal; el
Salamander más vivo, más fuerte, mejores graves y agudos"*. And his idea: perspective as a
control. ADR 0098's second amendment has the design.

**What changed** (`cantus-dsp/src/chorda_{body,stage,voicing,room,engine}.rs`):

- The body's response: each third-octave's modes carry the radiation's energy (a random
  draw of a few modes no longer makes a band honk — the old one had +5 dB at 1 kHz and
  1.6 kHz on every note), an octave EQ voiced against the Salamander's long-term spectrum,
  the two microphones sharing their modes below 200 Hz (one centre for the bass), the low
  modes' ring shorter (T60 at 100 Hz 0.39 → 0.12 s: less boom and blur), the direct path
  0.10 → 0.25 (attack definition).
- The strings: the bass hammers harder (felt ×3.9 at A0, interpolated to middle C: the bass
  has its 2–4 kHz bite), the slowest hammer slower (×0.6: a wider dynamic range — and at
  mezzo-forte fewer upper partials, the nasal), the bass decaying faster early, the treble
  ringing longer (its frequency-dependent loss halved).
- The room: 45 % → 35 % by default (drier), its size a control.
- **The perspective** (`stage`): *Mics* (over the hammers … the player, 35 % … the
  audience), *Width* (mid/side: the mono sum never changes), *Lid* (closed … open), *Size*;
  each glides sample by sample (no zipper), exact at its default, model 1 untouched; the
  perspective's level held about even (the audience heard 3 dB louder, the hammers 2 dB
  softer, before the trim). Cards: *Microphones and lid* on Play and Edit, *Room* on Edit,
  each with a picture (the piano from the side with its lid and the pair; the room's walls
  and its decay in seconds).
- The 24 presets placed: *Close and Intimate* over the hammers, *Pop Forward*, *Rock* and
  *Lo-fi* close, *Felt Piano* and *Old Upright* with the lid closed, *Jazz Club* half open,
  *Hall Grand* and *Ambient Halo* in the audience in a large room, *Cinematic* and *Theme*
  between; levels re-trimmed to the library's window.
- The ABX page counts every press (`plays`, and `heard` by slot) — the first kit recorded
  0 on every trial; `chorda_kit/abx_check.js` fails on the old page.

**Measured, before (the first delivery) → after, against the Salamander** (through the real
engine, `chorda_compare render`; `downloads/piano/v2-final3.txt`; level-free shapes):

| The owner's word | Measure | Salamander | Before | After |
|---|---|---|---|---|
| *muchos medios* | phrases' long-term spectrum, 250 Hz third (dB of total) | −14.8 | −10.9 | −15.2 |
| | 1 kHz third | −18.7 | −14.1 | −15.1 |
| *nasal* | middle mf notes: partials 3 / 5 against the fundamental (dB) | −13.0 / −22.0 | −5.1 / −21.4 | −6.5 / −19.6 |
| | middle mf, 1.6 kHz third (dB of the note) | −24.2 | −17.4 | −18.9 |
| *borroso* | middle mf: first 20 ms against 20–100 ms (dB) | −6.1 | −6.9 | −5.8 |
| | treble mf: first 20 ms against 20–100 ms | −1.7 | −2.1 | −0.7 |
| *graves* | phrases, 125 / 157 Hz thirds | −18.2 / −19.6 | −25.1 / −24.0 | −21.9 / −19.3 |
| | bass mf, > 2 kHz attack against its 0.1–0.3 s (dB) | +0.2 | −8.9 | −0.3 |
| | bass mf early decay (dB/s) | 8.5 | 4.3 | 6.0 |
| *agudos* | treble mf early decay (dB/s) | 36.4 | 49.1 | 34.9 |
| | treble mf attack peak against 0.5 s (dB) | 29.6 | 36.3 | 32.0 |
| | phrases, 2.5 / 4 kHz thirds | −27.5 / −39.4 | −32.5 / −42.9 | −29.0 / −40.1 |
| *vivo, fuerte* | ff − pp level, bass / middle / treble (dB) | 20.4 / 24.4 / 29.7 | 16.8 / 19.3 / 26.5 | 19.2 / 23.0 / 32.1 |
| | fortissimo chord: crest / peak against its loudest 400 ms (dB) | 25.4 / 14.5 | 17.7 / 8.7 | 19.6 / 9.6 |
| | middle C: crest (dB) | 23.3 | 21.0 | 22.4 |
| Score (`tools/s2.py`, lower is closer) | | — | 26.3 | 23.5 |

Still apart (measured): the middle notes' 1–1.6 kHz (+4–5 dB), the 3.2 kHz third in the
phrases (−6 dB), the fortissimo chord's crest (the body's response spreads a chord's attack:
without it, 23 dB), the mono sum's dips at 1.26 and 3.2 kHz (−3.6, −4.2 dB on middle notes:
the two microphones' responses are drawn independently above 200 Hz; holding each band's
correlation by turning modes over did not move the measured correlation and was taken out),
the treble's late decay (27 against 2.3 dB/s — the Salamander's high notes keep a ringing
floor near −80 dBFS). **The output ceiling is not what flattens the attacks**: at the staged
level the loudest chord peaks at −3.7 dBFS, where the knee begins (−4.4 dBFS) — but at equal
loudness (the ABX kit's −23.3 LUFS) the Salamander's fortissimo chord peaks 7.8 dB higher
than Chorda's (−4.0 against −11.7 dBFS): Chorda's chord sustains louder against its attack
(the crest above), which is what *más fuerte y vivo* measures. No piano-specific level target
is proposed: the ceiling leaves 3.7 dB unused, and lowering the target would not change a
crest.

**Levels and cost**: the staging triad at velocity 100 measures **−23.0 LUFS** (inside the
family's −18.7 ± 4.5; the wider dynamic range makes velocity 100 softer, so the output rose
from 0.0183 to 0.0244); all 24 presets within ±2 LU of their role's target
(`preset_render.rs`; the only failures in that run were other units' CPU budgets on a machine
running three builds). 64 pedal-down voices: model 1 18.9 %, **model 2 22.3 %** of a core
(the stage costs nothing measurable).

## What was done

- **The comparison tool** — `cargo run --release -p cantus-ui --example chorda_compare --
  all <downloads> <out>` (`examples/chorda_compare/`, `examples/chorda_kit/`): installs the
  Salamander Grand (CC BY 3.0, Alexander Holm) through Cantus's own pack code
  (`cantus_content::Manager`, SHA-256 checked, unpacked) into `<downloads>/content`, renders
  every phrase through the real engine (`cantus_engine::render`, the export's path; the
  Salamander in Exemplar as *Sound packs…* installs it, all 16 layers, streamed) for the
  Salamander, Chorda model 1 and model 2, and measures them: per note (A0…C8 every minor
  third, pp/mf/ff) the third-octave spectrum of the first 50 ms and at 0.5 s (level-free),
  the knock, the rise, the decay per octave band and early/late (read only 12 dB clear of
  the floor before the next note), the stereo image, the release; per phrase the loudness,
  the long-term spectrum, the centroid, the correlation. Writes `report.md`, `notes.csv`,
  `phrases.csv` and ~60 PNG plots. `lab <out> <name> <key=v,…>` drives model 2's DSP with a
  voicing to try (no rebuild) — what the voicing search used.
- **Model 2** (`cantus-dsp/src/chorda_{body,conv,room,noise,voicing}.rs`, changes marked in
  `chorda_engine.rs`, `chorda_voice.rs`, `chorda_string.rs`): the body (two synthesised
  radiation responses, convolved without latency), a room, the action's noises (the blow,
  the keybed, the dampers — into the body, never the sympathetic strings), a seeded
  variation per strike, the voicing set by measurement. Two new settings: `room`, `noise`.
- **Schema 54**: `MODEL = 2`; `migrate_53_to_54` writes `model = 1` into every Chorda saved
  before; fixture `v54.cantus`.
- **The owner's ABX kit** — `cargo run --release -p cantus-ui --example chorda_abx -- <dir>`.
- **Presets**: twenty-four (ADR 0101's floor), voiced for model 2, tagged with the library's
  `preset.style.*` and `preset.role.*` keys.
- Editor: `room` on Play's resonance card (model 2), `room` and `noise` in Edit's resonance
  card; a preset loaded on an old song's Chorda moves it to model 2. Manual (EN/ES), `docs/instruments/chorda.md`, ADR 0098,
  `docs/beta-checklist.md`.

## Measured (by measurement only — nobody has listened)

Against the Salamander Grand, through the real engine (`compare/report.md` has every table,
`compare/summary.md` the before/after by the owner's three complaints, `compare/*.png` the
plots). The voicing was set by a coordinate search of `chorda_compare`'s score (~120 lab
trials), then two hand trials on the bass's weight.

| | Salamander | Model 1 | Model 2 |
|---|---|---|---|
| Score (lower is closer) | — | 64.4 | 33.1 |
| Shape distance, 50 ms / 0.5 s (RMS dB) | — | 21.5 / 29.6 | 9.4 / 13.5 |
| **Thin, no body**: bass mf 125 Hz octave (dB of the note) | −2.8 | −14.7 | −9.0 |
| middle mf 125 / 250 Hz octaves | −34 / −25 | −60 / −50 | −44 / −27 |
| ballad, long-term spectrum vs reference, 63 / 125 / 1k / 8k Hz | 0 | −7 / −9 / +11 / +26 | −8 / −7 / +2 / +6 |
| centroid: ballad / bass phrase / middle C | 289 / 235 / 462 Hz | 748 / 586 / 996 | 330 / 270 / 603 |
| middle mf decay 0.1–0.6 s / late (dB/s) | 22 / 5.0 | 39 / 6.5 | 25 / 4.3 |
| **Synthetic**: middle mf knock (share < 0.7 f₀, 60 ms) | −22 dB | −45 | −19 |
| middle / treble mf rise 10–90 % | 16 / 7 ms | 1 / 1 | 20 / 12 |
| middle mf attack centroid (50 ms) | 520 Hz | 1 170 | 797 |
| two strikes of C4 at one velocity | — | identical | differ (0.09 dB, seeded) |
| **Space**: middle mf correlation / side under mid | 0.10 / −1.1 dB | 0.83 / −9.5 | 0.15 / −1.5 |
| ballad / ff chord correlation | 0.30 / 0.24 | 0.71 / 0.74 | 0.18 / 0.17 |

Still apart (measured): the bass's weight under 125 Hz (−9 against −3 dB), the bass's
early decay (10 against 4 dB/s) and its 2–4 kHz (−13 dB in the bass phrase), the treble's
early decay (52 against 34 dB/s), its release where it has no damper, and its brightness
growing faster with velocity (×2.0 pp → ff against ×1.5).

**Levels** (ADR 0098 §3, ADR 0101): a new Chorda measures **−22.7 LUFS** on the family's
staging triad (model 1: −30.1; the window −18.7 ± 4.5), with an output ceiling (unity to
−4.4 dBFS, then a tanh toward −0.72 dBFS) that bends only fortissimo chords' attacks; both
hands fortissimo with the pedal stay under full scale. Every one of the 24 presets passes
`preset_render.rs` (±2 LU of the keys/pad target, no clipping, CPU); the ten Chorda lines
are struck from `tests/data/preset_render_known.txt` and Chorda from
`instrument_reference.rs`'s pending list. The comparison's numbers above were measured
before this staging (shapes are level-free; only the fortissimo chord phrase's peaks would
now meet the ceiling).

**Cost**: 64 pedal-down voices at 256 frames, one thread, release, this PC: model 1 19.2 %,
**model 2 23.0 % of a core** (two convolutions and the room; the family's bound is 25 %).
Model 2's quiet floor is 1e-4 N at the bridge (about −115 dBFS at the staged output): its
decays are up to four times model 1's, and the family's contract asks for rest within 30 s.

## The review of the first delivery (fixed)

Strikes seeded from the note's id (not the unit's clock: live after idling = offline, a range
render = the same span of the whole); a model switch clears the side it leaves; the body's
length capped at 48 kHz's frames (96 kHz: model 1 39.8 %, model 2 51.9 %; 192 kHz: 78.3 / 98.0 %
of a core — the voices themselves double with the rate); the body warmed before the strings'
design; the status line says when a preset moves a Chorda to model 2; `room`/`noise` named
*(model 2)*; the ABX kit's files opaque; the quiet floor's figure (−115 dBFS) one everywhere.
Each has its test (ADR 0098's amendment).

## Tests

- `cantus-dsp` `chorda::*`: 40 (the 33 of round 28 untouched, `tests_hash` — model 1's
  pinned bits, computed on c95ee8a9 — and `tests_model2`: silence, any block size 1…512
  and four threads to the bit, exact rest, strikes vary only in model 2, the convolver equals
  a direct convolution to 1e-9, the two microphones' responses decorrelated (|r| < 0.5),
  64 voices; `measure_model_2s_…` ignored, release).
- `cantus-engine`: `inst_chorda` 12 (every parameter at its default changes no sample —
  `room`, `noise`, `model` included; offline = real time; headroom: ff both hands under
  −2 dBFS), `inst_contract` 34 (the family's contract for Chorda at model 2: twins, rest,
  exact silence six seconds after the last note), `rt_memory` 31, `null_tests` 26.
- `cantus-project`: every test, with `tests/chorda_model.rs` (every fixture v28…v53 opens as
  model 1 said explicitly; a Chorda with no `model` key opens as 1; a written model is kept;
  v54 is model 2); fixture `v54.cantus`.
- `cantus-core`: every test. `cantus-ui`: `chorda_model2_e2e` (**the end-to-end test**: a
  Chorda added through `apply_pick` on an app made by `CantusApp::assemble` with the
  manual engine plays model 2 live, equal to the offline render to the bit and unequal to
  model 1; the v53 fixture opened with `app.open` keeps model 1 and its Chorda renders to the
  hash round 36's code gave), `inst_chorda` 11 (24 presets, each with a library style and role,
  every tag named in both languages, model 2 stored), `instrument_levels`, `demos` 13, `inst_shell`,
  `inst_faces`, `skins_body`, `learn_everywhere`, `modulate_everywhere`, `gain_staging`,
  the i18n lib test.
- Clippy `-D warnings --all-targets`: `cantus-dsp`, `cantus-core`, `cantus-engine`,
  `cantus-project`, `cantus-ui` — clean. Linux (WSL): see below.

## Marked lines in shared files

- `crates/cantus-project/src/format.rs`: `SCHEMA_VERSION` (56 after the merge: 54 piano,
  55 piano's Sectio — an empty step for now —, 56 drums), the table, `migrate_53_to_54`, its
  helper `write_native_default`, `migrate_54_to_55`'s comment — `// r37:piano`.
- `crates/cantus-core/src/inst/mod.rs`: `automatable` — Chorda's `SET_KEYS` (`model`).
- `crates/cantus-engine/src/inst/chorda.rs` (Chorda's): the designer warms the body.
- `crates/cantus-ui/tests/instrument_reference.rs`: `PENDING` emptied (library's file).
- `crates/cantus-ui/tests/data/preset_render_known.txt`: the ten `chorda.preset.*` lines
  removed (they pass).
- `docs/beta-checklist.md`: *Chorda modelo 2 — tu ABX* (`<!-- r37:piano -->`).
- Fixtures: `v54.cantus` new, `v56.cantus` rewritten after the merge (its Chorda is model 2).

## Only the owner

- **The second ABX** (voicing pass two): the kit is written — `F:\projects\daw-r37\abx-kit2\index.html`
  (regenerated with the hashed names and the counter that now records every press; the
  phrases matched at −20 LUFS, the fortissimo chord at −23.3). Then the perspective by ear:
  *Mics* 0 → 100 %, *Lid*, *Size*, while a phrase plays (`docs/beta-checklist.md`), and which
  perspective should be the default.
- **Sectio by ear**: annotate a few CC0 loops (their sources and licences written down) and
  judge where the slices fall; the slicer met 90 % of its rendered set and 74 % of MDB Drums
  within 10 ms, not the brief's 95 %.

- **The ABX** (`docs/beta-checklist.md`): `cargo run --release -p cantus-ui --example
  chorda_abx -- F:\projects\chorda-abx`, then open `index.html`. A verification copy is in
  `F:\projects\daw-r37\downloads\piano\abx-kit\` (made before the level staging and the
  kit's second loudness pass; regenerate for the real test). Only your ears close Chorda:
  everything here was judged by measurement against one recorded piano, nobody listened.
- **The output ceiling on fortissimo** (ADR 0098 §3): whether staging a piano at the family's
  level, with its loudest chords bent by a few dB, sounds right — or pianos get their own
  target.
