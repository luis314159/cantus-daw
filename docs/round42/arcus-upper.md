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
  bow lifts (the study's release, 30 – 50 dB/s); their levels fitted (§5).
- **Pitch table** (`arcus_upper_pitch_data.rs`, `tools/arcus/upper-pitch.sh`): raw offsets
  −7.9 … +14.9 ct (violin), −6.6 … +12.6 (viola); at most 0.43 ct left after the first pass.

### 3. The picture

Design sheets v1 (`docs/round42/arcus-upper/sheet/`) and **v2** (`sheet-v2/`, after the
integrator's notes; `shoot_arcus --scene --only=vio`, and `card-four-instruments-night-100.png`:
the Instrument card's four choices): the violin and the viola as meshes on the same scene
engine (`Subject::Bowed` gained `Violin` and `Viola`; `scene/bowed_upper.rs`: **their own
outlines** from their measures — the 'Messiah' Stradivari, 356 mm, 104 / 54.5 / 83 mm
half-widths, corners at 147 and 229 mm; a 16-inch viola, 406 mm, 119 / 66 / 96 — with their ribs,
arching, set-up and string gauges; a closer camera for them; in v1 and v2 a chromed instrument stand, since replaced by pose B). Both looks,
both themes, 100 % and 200 %. The live string, finger and bow are the cello's rig on the new
meshes. Chorda's and Tympanum's pictures untouched (`scene_identity`).

**Pose B, built (sheet v3, `docs/round42/arcus-upper/sheet-v3/`)** — the owner chose (B), the
playing angle in the air with the bow, over (A), the open case. The stand is gone
(`bowed_upper_stand.rs` deleted; `Dims::stand` became `Dims::held`): `bowed_upper::held` sets the
instrument at a seated player's shoulder (112 cm), its way end-button to scroll toward the
player's left and the hall (`TOWARD`), its top rolled ~30° toward the bow (`UP`). Held under the
chin, the lowest string lies on the player's left, so `string_x` mirrors for a held instrument
(the cello's and the bass's unchanged), the bow's frog comes from the player's right and the
left hand's finger from the player's left (`arcus_draw_body.rs`); a Guarneri-style ebony
chinrest (cup, raised lip, foot, two nickel barrels) on the bass side; the fingerboard's
overstand raised (1.3 / 1.5 cm) so it clears the arching. The camera looks from the hall's
middle a little above (azimuth 4°, +7° elevation), closer, and fits a margin round the
instrument; no microphones and no chair round a held instrument (at that distance their
stands crossed it). The varnish is an amber-brown oil varnish (less gloss, the maple's flame
stronger), the Photographic picture's toning kept nearer that brown. Shots: `scene-*` (the
baked picture alone) and `arcus-{violin,viola}-play-*`, `arcus-violin-pizz-*` (the card with the
live string, finger and bow), both looks, both themes, 100 % and 200 %. **Waits for the
owner's approval.**

**Who holds it (sheets v4–v8)** — the holders (ADR 0157 amended), the realistic figures from
MakeHuman's CC0 assets and the figure module every instrument can use (ADR 0159; the
handoff `arcus-upper-next.md` has the detail). Sheet v8 (`docs/round42/arcus-upper/sheet-v8/`,
2026-11-24): the violin's and the viola's left hand posed from an authored table on the
figure's own hand (`figure::neck_hand`, `tools/arcus-figures/left_hand.py`), beside the
owner's photographs; *Figure detail* (High, the default; Light within the +1 ms budget;
Auto); the stylised figures posed through the same PlayRig; her original face and the
reference's styling (hair, earrings, posture, light); soft background focus.

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
| House invariants | cello and bass bit-identical; violin and viola at 1×, 2× and their own factor the same bits at blocks 1, 37, 512 and ragged; the pool the serial bits; exact rest; no allocation (`rt_memory::an_arcus_violin_and_viola_never_allocate`); end to end through `CantusApp::assemble` (`tests/arcus_upper_e2e.rs`: the card's switch, a slurred fifth: **violin −0.06 / −0.04 ct, viola +0.06 / +0.03 ct**, then each pizzicato preset) |

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

### 5. The integrator's three items (after the first report)

Measured each time by `tools/arcus/upper-measure.sh` — 88 notes through the study's own estimator,
the 125 recorded notes, the CNSM scales — over ten passes (`F:\projects\daw-r42\logs\arcus-upper\m1.log`
… `m11.log`; the chosen state is `m11`).

**(1) Preset levels.** `tools/arcus/presets.sh <dir> fit` (the house tool, crest-aware: it stops
where the phrase would pass −1.2 dBFS) now writes `arcus_upper_presets.rs` too: **all 14 at their
role's target (0.0 LU off: lead −18 LUFS, pluck −21)**, the phrase peaks −4.6 … −9.7 dBFS, the
held notes' peaks −3.3 … −14.2 dBFS — none near clipping. The cello's and the bass's 24 were
left as the arcus builder has them.

**(2) The vibrato's AM — why it was small, and what changed.**

- *Why.* The measured admittance itself (CNSM, Klimke) moves a partial only ±0.6 dB at
  ±10 cents and ±1.0 dB at ±17 (its level swept over that span, every frequency from 400 Hz to
  8 kHz) — the recorded violins move ±1.5 – 2.2 dB. The rest is radiation: a microphone hears
  each body mode with its own sign and strength above the signature modes (Weinreich's
  directional tone colour), so neighbouring modes add or cancel, a fine structure the bridge
  admittance does not have. And the fit (41 modes on a smoothed admittance) resolves less fine
  structure than the measured admittance has. With a vibrato of the real band's width (the
  violin's now ±10 ct) the fitted body moved each partial ±0.3 dB.
- *What.* The body (`arcus_upper_body.rs`): each mode above 700 Hz radiates to each microphone
  with its own sign (45 % in opposite phase) and strength (× 0.3 – 1.7); a **cloud of 60 modes
  the fit does not resolve** (900 Hz – 9 kHz, Q 63 – 117, at the fitted response's level,
  Woodhouse 2014's growing modal density) carries the fine structure; the fit itself at
  1/24 octave (1.94 dB from Klimke's admittance; the three violins 3.4 – 3.5 dB apart at that
  resolution). Its power stays one; the high band held by a measured trim so the brightness
  stays in the band (below).
- *Result* (the study's `vibrato_am_db_per_partial`, the estimator's own): **violin partials
  2 – 12 ±1.38 dB (the real band's median ±1.71), 12 of 20 partials inside [min, max], 45 %
  against the pitch (real: 45 – 50 %)**; **viola ±1.54 dB (real ±1.92), 58 % against**, its
  partials mostly just under the band's lower edge (3 of 20 inside: the real violas' band is
  narrow, min ≈ 1.5 dB). Before: ±0.31 / ±0.47 dB. The gate's vibrato test now reads up to
  4.3 dB with the body, 0.5 without (`upper_tests_gate`).

**(3) Brightness.** Where the darkness came from: the corner rounding (one-pole per round trip)
and the correction's high bands; then the cloud added brightness of its own, and the CNSM
recordings (a room, a microphone placement) want more air above 6 kHz than the six instruments
of the study's band have. Settled by measurement: the corners 6 kHz (violin) / 5 kHz (viola)
or 6 × f₀, the correction re-derived from the CNSM spectrum each pass but its top two bands held
under the band's (−4 / −2 dB), the viola's own correction (no cut under 160 Hz where its C
string sounds, +2 dB from 1.6 kHz), the body's high band ×0.8 on the violin.

| Study estimator, all dynamics | Before | Now | Real band |
|---|---|---|---|
| Violin spectral centroid | 1 248 Hz | **1 580 Hz inside** | 1 530 – 1 732 |
| Violin harmonic centroid | 2.02 | 2.58 (over) | 2.08 – 2.35 |
| Viola spectral centroid | 895 Hz | 982 Hz (0.4 % under the edge) | 986 – 1 117 |
| Viola harmonic centroid | 2.10 | 2.49 (over) | 1.84 – 2.11 |
| Bow noise re harmonics (violin / viola) | −34.1 / −36.5 | **−30.7 / −34.4 inside** | −35.7…−29.1 / −37.1…−31.7 |
| Long-term spectrum to the CNSM violins (Klimke / Levaggi / Stoppani) | 3.2 / 4.4 / 4.0 dB | 4.9 / 3.8 / 5.1 dB | violins among themselves 2.3 – 3.4 |
| Iowa/VSCO held notes, third-octave spectrum (violin / viola) | 8.0 / 8.2 dB | **7.7 / 7.7 dB** | the two sources 10.2 dB apart |

What is not inside, said plainly: the **harmonic centroid** (the partials' balance, in harmonic
number) sits over the band on both — the fine structure that makes the AM also lifts partials
3 – 6 a little; every pass that brought it inside (pass 4: 2.43) took half the AM away. The
**CNSM distance grew by about a decibel** (the CNSM recordings and the study's six instruments
disagree above 6 kHz, and the band was followed there). The viola's spectral centroid sits at
its band's edge. These three trade against each other on this body; the next step is a measured
radiation (CNSM's own recordings beside its admittance, the study's `cnsm_validation.json`:
−4.2 dB/oct between them) in place of the statistical one — a round-43 item for the body.

The gate after all of it (`cantus-dsp` `arcus::`, 24 green): pitch 96 of 96 held-out points
within a cent (median 0.009 ct), attacks 0 of 120 over, the cello's and the bass's bits
unchanged. The renders and the ABX's `ref\` copies were made again from this state.

### 6. Pass 4 merged (`r42/arcus` 2c2b6f202)

`r42/arcus`'s passes 2 – 4 merged in (conflicts: `arcus_tables.rs` — its old violin and viola
entries, kept in `upper::tables`; `DEVELOPING.md` and the ADR — both sides kept). What pass 4
brought, applied to the upper strings and measured the same way (`upper-measure.sh`; logs
`p4m0.log`, `p4m1.log`):

- **The swell** (`swell_s`, `swell_db`), fitted to the study's rise bands as pass 4 fitted the
  cello's: the violin 0.70 → 0.08 s from 20 → 10 dB under (its players hold back longest:
  the band's soft rise 0.33 – 0.50 s), the viola 0.30 → 0.10 s from 20 → 12 dB. **Every rise
  class inside on both, at the first fit** (below).
- **The pluck's T60 by frequency** (`pizz_t60`): `VIOLIN_PIZZ_T60` and `VIOLA_PIZZ_T60` from the
  study's `pizz_t60_by_freq` (3 violins, 2 violas; points held where their notes thin out); the
  first match's ring factors (× 0.27 / 0.32) retired to 1. Against the Iowa plucks
  (`upper-compare.sh`): the pluck's spectrum came far closer (harmonics 1–12 49 → 26 dB rms on the violin, 38 → 17 on the
  viola: the pluck's noise 0.15 → 0.03), **its decay measured worse against Iowa** (partials 1–4 +0.07 → +0.39
  octave of T60 on the violin, +0.03 → +0.54 on the viola): the table is the study's band across
  instruments, the old factor was fitted to Iowa's one violin and one viola — kept, as pass 4 keeps the cello's.
- **The 8 Hz radiation high-pass**: measured with and without on the upper strings — the rise
  is the same either way (their body's modes are band-passes: it never passed the bow's DC as
  Fides's cello body did), the attack's noise burst 1.2 dB (violin) and 2.0 dB (viola) lower
  with it. Kept, as the cello's.
- **The thermal law** (`arcus_string.rs`, `r42:arcus-p4`): the hyperbolic default untouched;
  the upper strings' hyperbolic path unchanged to the bit.

| Gate | Before the merge | After |
|---|---|---|
| Bowed pitch, violin + viola, 96 held out | 96 / 96 within a cent, median 0.009 ct | 96 / 96, median 0.009 ct (unchanged) |
| Bowed pitch, cello + bass, 168 held out | 168 / 168, median 0.012 ct | 168 / 168, median 0.015 ct |
| Attacks, violin + viola (Guettler & Askenfelt) | 0 / 120 over, worst violin key 90 ff 63 ms of 90 | 0 / 120, worst violin key 90 ff 63 ms of 90 (unchanged) |
| Attacks, cello + bass | 0 / 80 over, worst bass key 33 219 ms of 321 | 0 / 80, worst bass key 28 285 ms of 428 |
| The cello's and bass's bits | 7 hashes on 58cafea8e | re-measured on 2c2b6f202 (moved on purpose by pass 4), held |

| Study estimator, all dynamics | Before the merge (violin / viola) | After | Band (violin / viola) |
|---|---|---|---|
| attack rise, soft | 0.012 / 0.016 s | **0.404 / 0.182 inside** | 0.33–0.50 / 0.06–0.44 |
| attack rise, medium | 0.010 / 0.018 s | **0.288 / 0.142 inside** | 0.26–0.38 / 0.07–0.27 |
| attack rise, loud | 0.010 / 0.022 s | **0.090 / 0.078 inside** | 0.03–0.11 / 0.07–0.09 |
| attack to steady | 0.078 / 0.058 s | 0.207 / 0.102 (under, better) | 0.48–0.54 / 0.27–0.47 |
| attack noise burst | 22.9 / 22.8 dB | 21.1 / 19.9 inside | 14.6–47.3 / 11.9–30.1 |
| release decay | 153 / 112 dB/s | 153 / 113 (over, **unchanged**) | 17–50 / 2–63 |
| spectral centroid | 1 580 / 982 Hz | 1 579 / 980 (unchanged) | 1 530–1 732 / 986–1 117 |
| vibrato AM, partials 2–12 | ±1.38 / ±1.54 dB | ±1.44 / ±1.56 (13 / 5 of 20 inside) | median ±1.71 / ±1.92 |
| CNSM long-term spectrum | 4.9 / 3.8 / 5.1 dB | 4.8 / 3.7 / 5.1 dB | violins 2.3–3.4 |

**Measured worse:** the plucks' decay against Iowa (above); the release is still over the band — the bow's lift
and `ring_s` are the shared gesture's (the cello's came in with the DC fix, the upper strings'
did not move, since they never carried the DC); the plain presets ring 1.6 s.

The renders: `F:\projects\daw-r42\listen\arcus-upper\` made again from this state; the earlier
set in `before-pass4\` (with its `abx.html` and `ref\`).

## Renders for the owner

`F:\projects\daw-r42\listen\arcus-upper\`: per instrument `*-note-{pp,mf,ff}`,
`*-phrase-legato-cc1`, `*-detache`, `*-pizzicato`, `*-tremolo`; **the blind ABX**:
`abx.html` (the house page with its `SETS` set to 12 pairs) and `ref/` beside it —
`<pair>-rec-lufs23.wav` (a recorded note: VSCO 2 CE's solo violin A4 f/p, E5 f, G3 f; Iowa MIS
violin D4 mf, A4 pizz; Iowa viola A4 pp/mf/ff, D4 mf, C3 mf, D4 pizz) and
`<pair>-arcus-lufs23.wav` (Arcus on the same key, dynamic and length), both at −23 LUFS —
18 pairs, six of them against the study's own references (TinySOL violin A4 and viola D4 at
pp, mf, ff, `downloads/study/abx-refs`). Levels (the instrument at its defaults; the presets are fitted, §5): violin pp / mf / ff
−27.6 / −22.1 / −16.2 LUFS, viola −30.3 / −25.3 / −18.9 (11 – 11.5 dB pp → ff; the study: 13 – 18);
no render over −9.4 dBFS. Made again after §5.
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
- UI: `scene/bowed.rs` (`Bowed::Violin/Viola`, `Dims::held`, the outlines, `string_x` mirrored when held), `bowed_draw.rs`
  (materials, fit points, the camera), `bowed_fit.rs` (no endpin when held; the chinrest; the E string steel), `bowed_head.rs` (`paddle` shared),
  `bowed_mesh.rs`, `bowed_props.rs`, `scene/mod.rs`, `kit/skin/scene_bowed.rs` (two palettes),
  `views/inst/arcus.rs` (the four choices; `FACTORY_PRESETS` joins the upper presets),
  `arcus_draw.rs`, `arcus_draw_body.rs`, `chorda_scene_px_bowed.rs`, `i18n.rs` (the
  `arcus_upper` locale area), `examples/shoot_arcus.rs`, `examples/fides_listen.rs` (the preset
  fit writes `arcus_upper_presets.rs` too), `tests/arcus_scene.rs`.
- **The cello's and bass's bits**: `upper_tests::LOWER_HASHES` were measured on 58cafea8e; any
  later change of theirs on `r42/arcus` moves them on purpose — re-measure the same way
  (`ARCUS_UPPER_DUMP=1`).
- Tools: `tools/arcus/upper-{body,pitch,compare,listen}.sh`, `upper_study.py` (DEVELOPING §4).
  `upper-measure.sh` runs the study's estimator and the comparison in one pass.

## Left

- **The owner's ears**: the renders and the ABX pairs (`abx.html`); then violin and viola go
  wherever the cello goes (they are inside Arcus, which enters the picker in the arcus builder's
  `ALL` commit — its picker text "cello and double bass" would then name all four).
- **The owner's eyes**: sheet v3, pose B as he chose it — not done until he approves it.

*Left for the round (not owner items, shared with the arcus builder's pass 2):* the attack's
swell and the release (outside the study's band, the shared gesture), a real pizzicato (the
shared pluck), and the body's radiation measured rather than statistical (§5: the harmonic centroid over its band, the CNSM distance up a decibel, the AM ±1.4 / ±1.5 dB against the real ±1.7 / ±1.9).

## Tests (Windows, this branch)

`cantus-dsp` `arcus::` 24 (+12 probes ignored: `probe_rate`, `probe_corner`, `probe_top` and the
cello's), among them `upper_tests` (the cello's and bass's bits; the upper strings' block sizes at
every factor) and `upper_tests_gate` (pitch, attacks, Schelleng, vibrato, body, the way back);
`cantus-engine` `rt_memory::an_arcus_violin_and_viola_never_allocate`; `cantus-ui`
`arcus_upper_e2e`, `arcus_scene` (four instruments), `arcus_pick_e2e`, `scene_identity`,
`kit_lint`; clippy `--all-targets` with `RUSTFLAGS=-D warnings` on `cantus-dsp`,
`cantus-engine`, `cantus-ui`; rustfmt. Not run here: the Linux check (WSL is the integrator's),
the whole workspace suite.
