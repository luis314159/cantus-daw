# Round 42 — `arcus-upper`: Arcus's violin and viola

*The `arcus-upper` builder's report. Branch `r42/arcus-upper`, from `r42/arcus` b2691637e with
`r42/arcus-look` merged; `r42/arcus` 58cafea8e merged in again (its model 2, the rest fix).*

## What was built

**The violin and the viola** in Arcus, the modelled bowed string: the Instrument card offers
*Cello / Double bass / Violin / Viola*. Arcus stays out of the picker until the owner's
listening verdict, as the cello does.

### 1. The engine for the short segment (`cantus-dsp/src/arcus_upper*.rs`)

At 48 kHz the bow–bridge segment of a violin's upper notes is a few samples long; the voice's
`min_beta` forced β above the player's from E5 up (0.137 at E6, 0.275 at E7, against ~0.06 at
ff), and above E6 the motion broke. Three options, **measured** (`upper_tests_rate::probe_rate`,
`probe_corner`, `probe_top`; logs in `F:\projects\daw-r42\logs\arcus-upper\`):

| | 1× | 2× | 4× |
|---|---|---|---|
| β reached at ff, E6 / E7 (target ~0.06) | 0.137 / 0.275 | 0.069 / 0.137 | 0.063 / 0.069 |
| Bowed pitch, closed loop, top notes (E6…E7, pp/ff) | −33 … −51 ct (broken motion) | −20 … −33 ct at E7 | **within 0.03 ct** (with the corner and the lighter bow below) |
| Energy between harmonics (aliasing; noise and vibrato off), violin E5 … A6 | −13 … −21 dB | −24 … −36 dB | −30 … −40 dB |
| Cost, a note-second, the whole instance (release, other builds running: a ratio, not a claim) | 1.0 | 1.15 | 1.54 |

The viola, measured the same way with everything below in place: at 2× every key from C3 to A6
is Helmholtz and within 0.03 ct, β reaches its target to E6 (0.092 at A6 ff), between the
harmonics −20 … −47 dB; at 4× −28 … −52 dB for 1.28 × the cost of 2×.

- **Chosen: the violin's whole voice at 4×, the viola's at 2×** (the study's §3.13 predicted it:
  the bow–bridge segment is `d·f_s/(L·f_open)`, 2.2 samples on the violin's E bowed 10 mm from
  the bridge, 4.4 on the viola's A at 15 mm) — string, bow, pluck and pitch loop — back through a
  linear-phase half-band cascade (23 + 75 Kaiser taps: ripple ±0.0005 dB to 20 kHz, ≥ 90 dB down
  where anything folds under 20 kHz, 21.25 samples of latency). The control step runs as often
  in time as the cello's (every 32 samples at the rate: 128 of the voice's), so the bow's jitter,
  the vibrato and the loop behave alike. The body, the match and the output stay at the rate.
- **Set aside**: a locally oversampled bridge segment (a rate change inside the junction's loop
  puts an interpolator's delay and ripple round the loop every period — the pitch and the losses
  the segment was too short to model); a fractional junction placement (the cello has it already:
  it keeps the pitch, not the motion between bow and bridge).
- **Two physical additions for the top**, upper strings only: the bridge's corner rounding at
  least **6 × f₀** (at 3× the top fifth still broke; at 6× every top note is Helmholtz), and the
  bow **lighter and slower high on a string** (0.35 octave of force and ^0.25 of speed per octave
  stopped — two octaves up: −0.7 octave of force, 0.71 × speed), as players bow there; an ff
  E7 with the open string's bowing goes raucous on this model (−22 … −27 ct, +50 dB between
  harmonics), half an octave lighter brings it back.
- **The cello and the double bass are bit-identical**: they run the code they ran at 1×
  (`upper_tests::the_cello_and_the_bass_keep_their_bits`: seven settings × the full phrase,
  hashes measured on `r42/arcus` 58cafea8e in a worktree with only that test added).
- **No allocation**: the lines already hold the violin's lowest period at 4× (the bass's at 1×);
  a voice that changes rate retunes its string and pluck in place. A voice ends on its control
  grid and its way back rings out 64 samples from there, so the bits never depend on the blocks.

### 2. The instruments

- **Strings** (`arcus_upper_tables.rs`, every number cited): Percival's measured lengths
  (Glasgow thesis 2013, Table 3.2: violin 322 mm, viola 366 mm), Rossing's tension ranges
  (via Percival's Table 3.6; violin E 80 N, A 54, D 47, G 45; viola A 72, D 54, G 53, C 54),
  Percival's **measured decay rates** of open strings (Table 3.5: each string's fundamental T60
  and its partials' exponent `p = log₁₀(r₁₀/r₁)`).
- **The violin's body fitted to a measured admittance**: the CNSM dataset (Pauget Ballesteros,
  Lalitte, Lostanlen, Fritz; Zenodo 10.5281/zenodo.18696786, **CC BY 4.0**) — three violins'
  bridge admittance by laser vibrometer. Only its 22 MB admittance range of the 654 MB zip was
  fetched (byte ranges). A 1.218 ms acquisition delay makes the measured phase non-passive;
  taken out, every point's real part is positive (0 of 16 385 left negative) — the outside
  bytes check the model's passivity. Fitted (`examples/arcus_upper_body.rs`) by **positive**
  band-pass modes (non-negative least squares: a driving point's modes each add a positive mass
  term; an unconstrained fit let neighbours cancel and would have come apart under `body.ring`):
  41 modes, **1.72 dB rms from Klimke's admittance in twelfths of an octave (120 Hz – 10 kHz),
  where the three violins differ from each other by 3.1 – 3.2 dB**. Signature modes land where
  a violin's are (CBR 371 Hz, B1− 445, B1+ 542, the bridge hill 2.5 – 2.9 kHz).
- **The viola's body**: the violin's at **1.12 ×** its size — the study's database measured the
  viola's A0 at 250 Hz against the violin's 281 (its size, not its tuning ×1.5); no measured
  viola admittance exists in the open. Its strongest signature mode lands at 485 Hz against
  the violin's 543, its A0 about 1.9 × its C string (the database: 1.91 — the veiled lowest
  octave).
- **Ranges**: violin G3 – E7, viola C3 – A6. **Articulations**: arco, legato (the finger moves,
  the bow goes on), détaché with the bow's direction (alternating or the note's `art.bow`),
  staccato, spiccato, pizzicato, tremolo — the cello's, all reaching the upper strings.
- **The dynamic re-mapped the players' way** (the study's item 2), in the upper strings' own
  tables: pp → ff the speed ×3 (10 → 30 cm/s), β ×0.4, the force ×5; the bow lighter and
  slower high on a string (above).
- **Model 2's match** (`VIOLIN_MATCH`, `VIOLA_MATCH`): the output's correction from the CNSM
  violins' scales (14 recordings, 3 violins, the same players: their mean long-term spectrum
  over Arcus's two-octave G major scale); the hair's noise +22 dB (violin) / +18 (viola) and
  the vibrato ×0.5 depth ×0.88 rate (violin) / ×0.9 depth (viola) set by the study's own
  estimator on Arcus's notes (below); the pluck's ring × 0.27 / 0.32 from the Iowa notes'
  decays. The viola takes the violin's correction (no viola scales to fit its own — an
  assumption, said here).
- **Presets** (`views/inst/arcus_upper_presets.rs`, 14: violin plain, cantabile, sul tasto,
  without vibrato, détaché, portamento, pizzicato, tremolo; viola plain, cantabile, sul tasto,
  détaché, pizzicato, tremolo), the cello's four macros; the plain ones ring 1.6 s after the
  bow lifts (the study's release, 30 – 50 dB/s). Their gains are 0.0 until
  `tools/arcus/presets.sh <dir> fit` runs (it now writes this file's gains too): **not run**
  — a release UI build of the preset fit did not fit in the round's slots; the integrator's.
- **Pitch table** (`arcus_upper_pitch_data.rs`, `tools/arcus/upper-pitch.sh`): raw offsets
  −7.9 … +14.9 ct (violin), −6.6 … +12.6 (viola); at most 0.43 ct left after the first pass.

### 3. The picture

Design sheets v1 (`docs/round42/arcus-upper/sheet/`) and **v2** (`sheet-v2/`, after the
integrator's notes; `shoot_arcus --scene --only=vio`, and `card-four-instruments-night-100.png`:
the Instrument card's four choices): the violin and the viola as meshes on the same scene
engine (`Subject::Bowed` gained `Violin` and `Viola`; `scene/bowed_upper.rs`: **their own
outlines** from their measures — the 'Messiah' Stradivari, 356 mm, 104 / 54.5 / 83 mm
half-widths, corners at 147 and 229 mm; a 16-inch viola, 406 mm, 119 / 66 / 96 — with their ribs,
arching, set-up and string gauges; a closer camera for them; `scene/bowed_upper_stand.rs`: a chromed instrument stand
with a padded cradle and neck fork — nobody holds them, so they stand as a maker's photograph
shows them, leant back 11°, in the cello's studio with its two boom microphones). Both looks,
both themes, 100 % and 200 %. The live string, finger and bow are the cello's rig on the new
meshes. Chorda's and Tympanum's pictures untouched (`scene_identity`). **Sent to the integrator
for the owner**; the pose waits for his choice — (A) in an open velvet-lined case on a small
table, three-quarter from above, the bow across the strings at its contact when it sounds, or
(B) at the playing angle in the air with the bow — and is not built yet.

### 4. Studies and comparison

Three measures, every one a tool in `tools/arcus/` (logs in `F:\projects\daw-r42\logs\arcus-upper\`):

**(a) The gate** (`cantus-dsp` `arcus::upper_tests_gate`, `upper_tests`, all green):

| What | Measured |
|---|---|
| Bowed pitch, 96 points held out of the table (keys 23–49 ct off, dynamics and contacts between the grid's), table + closed loop | **96 of 96 within a cent, median 0.010 ct**, worst 0.14 ct |
| Plucked pitch (the four instruments) | within 0.1 ct (`a_plucked_note_is_within_a_tenth_of_a_cent`) |
| Attacks, every string at open, a fifth and a ninth up, velocities 0.15 – 0.95 | **0 of 120 over** Guettler & Askenfelt (worst: violin E5 ff, 51 ms of 90) |
| Schelleng on the violin's D and E (Schoonderwaldt 2008's strings) at 10 cm/s | a region at every β from 0.04 to 0.18: the lower limit 0.52 → 0.04 N on D (β^−1.7; Schelleng β⁻²), the upper flat in newtons; the wedge widens toward the fingerboard |
| Vibrato through the fitted body (violin D4) | each partial its own swing with the body (0.3 – 1.1 dB), under 0.5 without — **about half** the AM the study measured on real violins (±1.5 – 2.2 dB a partial): the fitted body is smoother than a violin's fine structure |
| The way back | ripple ±0.0005 dB to 20 kHz; what folds under 20 kHz ≤ −95 dB; 18.5 (2×) / 21.25 (4×) samples |
| House invariants | cello and bass bit-identical; violin and viola at 1×, 2× and their own factor the same bits at blocks 1, 37, 512 and ragged; the pool the serial bits; exact rest; no allocation (`rt_memory::an_arcus_violin_and_viola_never_allocate`); end to end through `CantusApp::assemble` (`tests/arcus_upper_e2e.rs`: the card's switch, a slurred fifth: **violin +0.14 / −0.02 ct, viola +0.08 / +0.06 ct**, then each pizzicato preset) |

**(b) The study's own estimator on Arcus's notes** (`tools/arcus/upper_study.py`: 44 notes per
instrument — every fourth key, arco pp/mf/ff and pizz — through the study's `extract.run` and
its `aggregate.ARCO_FEATS`, set beside the band of real instruments: violin 6, viola 3):

| Feature (all dynamics) | Violin: Arcus · band | Viola: Arcus · band |
|---|---|---|
| bow noise re harmonics | **−34.1 dB · [−35.7, −29.1] inside** | **−36.5 · [−37.1, −31.7] inside** |
| harmonic centroid (× f0) | 2.02 · [2.08, 2.35] just under | **2.10 · [1.84, 2.11] inside** |
| spectral centroid | 1 248 Hz · [1 530, 1 732] under | 895 · [986, 1 117] under |
| vibrato rate | **5.42 Hz · [4.93, 5.49] inside** | **6.01 · [5.48, 6.07] inside** |
| vibrato extent | **10.0 ct · [6.9, 10.4] inside** | **16.9 · [10.3, 22.0] inside** |
| attack noise burst | **23.2 dB · [14.6, 47.3] inside** | **24.7 · [11.9, 30.1] inside** |
| attack rise (3 dB) | 0.014 s · [0.13, 0.41] **under** | 0.014 · [0.07, 0.26] **under** |
| release decay | 150 dB/s · [17, 50] **over** | 111 · [2, 63] **over** |

Before the study's pass (the same estimator on the first renders): the noise −42 / −41 dB
(under), the violin's vibrato 17 ct at 6.2 Hz (over), the centroids lower still. The attack's
swell and the release are the shared gesture code (the arcus builder's pass 2): they will move
the upper strings with the cello.

**(c) Beside recordings, feature by feature** (`tools/arcus/upper-compare.sh`: 125 notes — the
Iowa MIS's violin and viola runs, VSCO 2 CE's solo violin — and Arcus on each, measured the same
way; the CNSM scales for the long-term spectrum):

- Long-term spectrum of a two-octave scale against the CNSM violins' (14 recordings, 3 violins):
  **Arcus 3.2 / 4.4 / 4.0 dB from Klimke / Levaggi / Stoppani; the violins among themselves
  2.3 – 3.4 dB** (before the match: 6.6 / 5.0 / 6.2).
- Held notes, Arcus–recording (rms over the notes): harmonics 1–12 10.1 dB (violin) / 8.9
  (viola), third-octave spectrum 8.0 / 8.2 dB, noise between harmonics −0.6 / +3.1 dB (mean);
  the two violin sources differ from each other by 11.3 dB (harmonics) and 10.2 dB (spectrum)
  on the one note both hold — **inside the recordings' own spread**; the centroid 4 semitones
  under (the study's centroid says the same).
- Pizzicato: the partials' T60 matched (+0.07 octave, rms 0.8); its spectrum is far from the
  recordings' (harmonics 1–12 49 dB rms — a pluck without the finger's snap, and the
  recordings' noise floor in the measure): the study's item 6, a real pizzicato, is the shared
  pluck's (the arcus builder's), left open here.

## Renders for the owner

`F:\projects\daw-r42\listen\arcus-upper\`: per instrument `*-note-{pp,mf,ff}`,
`*-phrase-legato-cc1`, `*-detache`, `*-pizzicato`, `*-tremolo`; **the blind ABX**:
`abx.html` (the house page with its `SETS` set to 12 pairs) and `ref/` beside it —
`<pair>-rec-lufs23.wav` (a recorded note: VSCO 2 CE's solo violin A4 f/p, E5 f, G3 f; Iowa MIS
violin D4 mf, A4 pizz; Iowa viola A4 pp/mf/ff, D4 mf, C3 mf, D4 pizz) and
`<pair>-arcus-lufs23.wav` (Arcus on the same key, dynamic and length), both at −23 LUFS —
18 pairs, six of them against the study's own references (TinySOL violin A4 and viola D4 at
pp, mf, ff, `downloads/study/abx-refs`). Levels: violin pp / mf / ff −23.4 / −17.8 / −11.8 LUFS,
viola −26.9 / −21.5 / −15.3 (12 dB pp → ff; the study: 13 – 18); no render over −3.4 dBFS.
`study/` holds the 88 single notes the study's estimator measured, `compare/compare.txt`
every note's distances.

## Verified only in emulation

- **The sound**: nobody has listened; every number is a measurement.
- **The picture**: shot on the CPU renderer.
- The viola's body (scaled) and its correction (the violin's) are assumptions, not measurements.

## For the integrator (merging with `r42/arcus`)

`r42/arcus` 58cafea8e is merged in already (conflicts in `arcus_render.rs`'s rest condition and
the ADR, resolved additively). Every line I touched in Arcus's files carries `// r42:arcus-upper`:

- `arcus.rs`: the `upper` module and the test modules; `ArcusSettings::oversample` (the tools'
  override, `None`); the `upper_body` field.
- `arcus_string.rs`: `min_hz` and `set_rate` (no allocation; at the built rate `min_hz` is
  `MIN_HZ`, so the cello's bits hold).
- `arcus_voice.rs`: the `upper` child module, the `rate` field, `SPAN` (the step buffers sized for
  4×), `active()` counting the way back's drain, `clear()` and `fade_out()`.
- `arcus_voice_step.rs`: `render` → `render_core` (the dispatcher is `upper::render`), the grid
  in the voice's samples (`CTRL · os`), the hair noise's `√os`, `upper::corner_hz`.
- `arcus_voice_run.rs`: `start`/`glide_to` enter the voice's rate; `force_trim`/`speed_scale`
  (zero and one for the cello and the bass).
- `arcus_play.rs`, `arcus_render.rs`: the upper body (`set`, `reset`, `at_rest`, `process`, the
  quiet flags).
- `arcus_tables.rs`: `spec()` points the violin and viola to `upper::tables`; their old consts
  removed. `arcus_pitch.rs`: the violin's and viola's rows read `upper::pitch_data`
  (`arcus_pitch_data.rs`'s empty rows kept referenced; its generator may drop them).
- `cantus-engine/src/inst/arcus.rs`: `oversample: None`. `tests/rt_memory.rs`: one test.
- UI: `scene/bowed.rs` (`Bowed::Violin/Viola`, `Dims::stand`, the outlines), `bowed_draw.rs`
  (materials, fit points, the camera), `bowed_fit.rs` (no endpin on a stand; the E string steel),
  `bowed_mesh.rs`, `bowed_props.rs`, `scene/mod.rs`, `kit/skin/scene_bowed.rs` (two palettes),
  `views/inst/arcus.rs` (the four choices; `FACTORY_PRESETS` joins the upper presets),
  `arcus_draw.rs`, `arcus_draw_body.rs`, `chorda_scene_px_bowed.rs`, `i18n.rs` (the
  `arcus_upper` locale area), `examples/shoot_arcus.rs`, `examples/fides_listen.rs` (the preset
  fit writes `arcus_upper_presets.rs` too), `tests/arcus_scene.rs`.
- **The cello's and bass's bits**: `upper_tests::LOWER_HASHES` were measured on 58cafea8e; any
  later change of theirs on `r42/arcus` moves them on purpose — re-measure the same way
  (`ARCUS_UPPER_DUMP=1`).
- Tools: `tools/arcus/upper-{body,pitch,compare,listen}.sh`, `upper_study.py` (DEVELOPING §4).
  The preset fit for the 14 upper presets was not run (their gains are 0.0).

## Left

- **The owner's ears**: the renders and the ABX pairs (`abx.html`); then violin and viola go
  wherever the cello goes (they are inside Arcus, which enters the picker in the arcus builder's
  `ALL` commit — its picker text "cello and double bass" would then name all four).
- **The owner's eyes**: sheet v2, and his choice of the pose (A, the open case; B, the
  playing angle) — then the pose is built.

*Left for the round (not owner items, shared with the arcus builder's pass 2):* the attack's
swell and the release (outside the study's band, the shared gesture), a real pizzicato (the
shared pluck), the body's fine structure for the vibrato's full AM, the 14 presets' level fit.

## Tests (Windows, this branch)

`cantus-dsp` `arcus::` 24 (+12 probes ignored: `probe_rate`, `probe_corner`, `probe_top` and the
cello's), among them `upper_tests` (the cello's and bass's bits; the upper strings' block sizes at
every factor) and `upper_tests_gate` (pitch, attacks, Schelleng, vibrato, body, the way back);
`cantus-engine` `rt_memory::an_arcus_violin_and_viola_never_allocate`; `cantus-ui`
`arcus_upper_e2e`, `arcus_scene` (four instruments), `arcus_pick_e2e`, `scene_identity`,
`kit_lint`; clippy `--all-targets` with `RUSTFLAGS=-D warnings` on `cantus-dsp`,
`cantus-engine`, `cantus-ui`; rustfmt. Not run here: the Linux check (WSL is the integrator's),
the whole workspace suite.
