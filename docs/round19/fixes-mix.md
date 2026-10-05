# Round 19 — fixes-mix: the mix side of what the builders found

Builder **fixes-mix**, branch `r19/fixes-mix`, on the integration commit 5190b92. Five fixes
that the master, Culmen, Fervor, Latitudo, demos and Gravis reports surfaced, each with its
test first.

## 1. Culmen on the master's reduction meter

`FxUnit::gain_reduction_db()` for the Maximizer unit (`cantus-engine/src/fx/maximizer.rs`),
modelled on Terminus's: the deepest of the brickwall's reduction now
(`Culmen::reduction_now_db`, telemetry `GR_NOW`), the last 10 ms column's largest
brickwall reduction, and the deepest band cut in that column (a band lifting is not a
reduction). Positive dB, so `Meters::master_gr` shows it.

Test: `cantus-engine/tests/master_chain.rs::a_maximizer_on_the_master_shows_on_its_reduction_meter`
(a default Culmen on a quiet mix reads under 0.5 dB; driven 18 dB into −12 dB it reads over
6 dB; bypassed, 0).

Fervor and Latitudo need nothing: neither is a dynamics processor (Fervor's level match is
static, Latitudo has no gain that moves with the signal), so `None` — the trait's
default — is the right answer for both.

## 2. The demos play their master chain

The master chain did not run when the demos were mixed; with it running (Aequalis →
Multiplex or Comprimo glue → Terminus *streaming*, +3/+4 dB) the songs measured:

| Song | Before (Terminus, round 19 engine) | After (Latitudo + Culmen) |
|---|---|---|
| Neon Tide | −16.1 LUFS, −1.0 dBTP | **−14.1 LUFS, −1.1 dBTP** |
| Morning Light | −18.2 LUFS, −1.1 dBTP | **−14.4 LUFS, −1.1 dBTP** |
| Sunday Strut | −14.0 LUFS, −1.0 dBTP (J bridge bass) | **−14.1 LUFS, −1.1 dBTP** (slap bass) |
| Paper Lanterns | −15.8 LUFS, −1.0 dBTP | **−14.1 LUFS, −1.1 dBTP** |
| Aurora | −18.1 LUFS, −1.1 dBTP | **−14.4 LUFS, −1.2 dBTP** |
| Midnight Static | −17.3 LUFS, −1.0 dBTP | **−14.8 LUFS, −1.1 dBTP** |
| Startup (Neon Tide's drop, looped) | — | **−13.2 LUFS, −1.16 dBTP** |

Whole songs rendered offline (`levels`, `FULL=1`), true peak by the meters' 4× detector.
Nothing was off by much — the demos builder had left 3–4 dB for Terminus — but four of the
six sat 2–4 LU under −14.

Each master chain now ends in a mastering chain built from the new tools, Terminus taken
out:

- **Latitudo** mono below the low end — *Master: tighten the low end* (mono under 100 Hz,
  low band 85 % wide) on Morning Light, Sunday Strut, Paper Lanterns and Midnight Static;
  *Master: wider top, mono bass* (mono under 90 Hz, top 130 % wide) on the two that live on
  width, Neon Tide and Aurora.
- **Culmen**, *Streaming −14* (three bands, the low band held 2:1, a gentle full-band
  squeeze, a −1 dBTP ceiling), its drive set per song by measurement: the song rendered up
  to Culmen once, then Culmen run offline at drives in half-dB steps
  (`demos.rs::master_drive`, ignored: it prints the table). Drives: Neon Tide 6.5 dB,
  Morning Light 7.5, Sunday Strut 4.5, Paper Lanterns 6.0, Aurora 7.0, Midnight Static 7.5
  (it sits at −14.8 LUFS rather than push an 808 trap mix past 7 dB of peak reduction).
- **Fervor**: *Tape on the bus* (6 dB, level-matched) on Sunday Strut's parallel drum bus,
  between the compressor and the bus limiter; worn tape on Paper Lanterns' keys (9 dB,
  character 70 %, the top cut at 9 kHz, level-matched).

The startup project (Neon Tide's second drop) inherits Neon Tide's master; it still opens
at once (the chain adds two inserts to the master, Culmen in its minimum-phase mode: 186
samples of latency, no linear-phase memory).

Tests (`crates/cantus-ui/tests/demos.rs`): `every_demo_renders_clean_and_at_a_sane_loudness`
now holds every song at **−16…−12 LUFS** integrated and a true peak **≤ −0.9 dBTP** (the
meters' 4× detector, `measure::true_peak_db`); the startup test at −16…−11 LUFS and
≤ −0.9 dBTP. `levels` prints true peak too, and `NOPARTS=1` skips the soloed parts. The
loudness and true-peak code is shared in `tests/measure/mod.rs`.

## 3. Gravis's slap

**The cause** (`cantus-dsp/src/gravis_string*.rs`): the slap's two excitations were never
level-calibrated against the finger pluck. The **thumb** is a velocity step whose edge
reaches the pickup coil — a derivative — in a third of a millisecond: at unity it peaked
~20 dB over a finger pluck of the same velocity (+11…+12 dBFS on one note) and ~10 dB over
its loudness. The **pop** pulls 3.5× the finger's displacement off the frets (+3…+8 dBFS).
Fret buzz was not the cause (the peaks are the same with buzz 0). On a funk riff at full
velocity the string engine peaked at **+12.6 dBFS** (the preset at +15 dBFS in a song).

**The fix**: what the pickups hand on is scaled per excitation — × 0.2 after a thumb
(`THUMB_LEVEL`), × 0.3 after a pop (`POP_LEVEL`), × 1 for everything else. The physics (the
step, the pull, the frets they slap) is unchanged, so the timbre is the same; finger, pick
and palm mute are bit-identical (× 1.0). Slap now peaks at **−1.8…−3.5 dBFS** (thumb) and
**−2.3…−7.3 dBFS** (pop) per note, the funk riff at −2.5 dBFS, with a loudness within 6 dB
of a finger pluck at the same velocity.

Tests: `gravis::tests::a_slap_is_calibrated_against_a_finger_pluck`,
`every_technique_keeps_its_headroom_on_a_dense_riff` (every technique on the slap preset's
string and the default one, full and ¾ velocity, under −1 dBFS).

**Sunday Strut is back on the slap preset**, its channel trim down 4 dB (−1 dB; the slap is
louder than the J bridge it had stood in for) so the bass sits with the kick.

**Every factory preset's headroom** (`crates/cantus-ui/tests/instrument_levels.rs::no_factory_preset_passes_full_scale_on_a_dense_riff`):
all 85 factory presets of the seven native kinds and Pulsus II's 8 kits (93 renders), at unity, on a
dense riff at velocity 127 (sixteenths over two octaves under four-note chords on every
beat; a kit plays a busy four-limb groove with a tom fill), must peak at or under 0 dBFS.
Three Exemplar presets did not: *Nylon guitar* +6.7 dBFS, *Bells* +1.6, *Keys* +2.7. Their
factory `gain` is lowered (nylon +3 → −5 dB, bells 0 → −3, keys 0 → −4); a factory preset is
copied into a project when chosen, so **no saved project changes**. The demos that use them
keep their sound (Morning Light's guitar +8 dB and bells +3 dB of channel trim, Paper
Lanterns' guitar +8 dB). Worst presets now: Exemplar *Organ* −0.5 dBFS, Gravis *Reese*
−0.7, Exemplar *Nylon* / *Keys* −1.3.

## 4. Instrument gain staging (measured; nothing changed)

Each instrument's **default patch** (`NativeSpec::new(kind)`, no preset) on a C-major
triad C4–E4–G4 at velocity 100 for two seconds, at unity, nothing on the master
(`instrument_levels.rs::default_patches_staging_report`, ignored). Gravis is mono: its
default plays C3 alone. Pulsus II: the default kit's kick, snare and closed hat together on
every beat.

| Instrument | Integrated | Peak | True peak |
|---|---|---|---|
| Unda (wavetable) | −18.7 LUFS | −10.4 dBFS | −9.8 dBTP |
| Calor (analog) | −19.2 LUFS | −8.6 dBFS | −8.6 dBTP |
| Modulus (FM) | −20.0 LUFS | −11.9 dBFS | −11.9 dBTP |
| Exemplar (sampler) | −27.2 LUFS | −21.6 dBFS | −21.6 dBTP |
| Gravis (bass, one note) | −15.7 LUFS | −10.2 dBFS | −10.2 dBTP |
| Lamina (electric piano) | −27.9 LUFS | −14.1 dBFS | −14.1 dBTP |
| Organum (organ) | −18.5 LUFS | −7.8 dBFS | −7.8 dBTP |
| Pulsus II (kick, snare, hat) | −25.3 LUFS | −8.8 dBFS | −8.5 dBTP |

The five synths sit within 4.3 LU of each other (−15.7…−20.0). **Lamina and Exemplar sit
8–9 LU under the synths' median (−19.2)** and 12 LU under Gravis — under the 10 dB line the
brief draws against the median, over it against the loudest. Pulsus's integrated figure is
sparse hits, gated, and is not comparable; its peaks are where a kit's should be.

The demos builder's −30 LUFS was one part of a song at velocity ~90 — a line or sparse
comping, not a held triad — so it is mostly the parts, not the instruments; but the two low
ones are real.

**Recommendation (not done this round, it would change saved projects' sound):** raise
Lamina's and Exemplar's *default* output by about +6 dB in a round that migrates projects
(a schema step that lowers the stored `gain` of existing Lamina/Exemplar instruments by the
same amount, so nothing saved changes), and then take the matching trims out of the demos.
Exemplar's factory presets are already balanced by their own `gain` (now ≤ 0 dBFS on the
dense riff), so the default patch is what a new instance would gain.

## 5. Terminus's true-peak overshoot

**Reproduced** (`cantus-dsp/src/terminus_tests_tp.rs`): drums, hats and a pad split into
three bands by Culmen (with its brickwall far above, so a delay), squeezed, driven +18 dB,
then Terminus at a −1 dBTP ceiling — every crossover mode, two seeds, the clipper off and at
20/40/70/100 %, four style/look-ahead/gain cases. By the independent 32× reference the
output went **+0.032 dB over with the clipper off and +0.244 dB at 100 %** (Culmen's
builder had seen +0.032 at 40 %), while Terminus's 4× detector read the output exactly at
the −0.05 dB the safety stage aimed for.

**The cause**: the 4× reconstruction behind every true-peak detector here is flat to 0.44 of
the sample rate and rolls off to −6 dB at Nyquist. Energy in that last tenth — what a fast
gain spreads around bright hats, and above all what a clipper makes — is read 0.08–0.29 dB
low. Terminus's corpus is band-limited to 0.44, so it never showed.

**The fix** (`cantus-dsp/src/terminus.rs`, `terminus_os.rs`):

- The clipper's residual (what clipping changes, computed at 4×) comes back through its
  own low-pass at **0.40 of the sample rate** (`os::lowpass`, a Kaiser-windowed sinc of the
  prototype's length and centre, so the latency is unchanged) instead of the prototype —
  nothing is left in the band the detectors read low. At 100 % clip: +0.244 → +0.080 dB
  with the old margin.
- The safety stage aims **0.15 dB** under the ceiling (`SAFE_MARGIN_DB`, was 0.05) — a
  margin the size of the detector's measured error, as Culmen's `TP_MARGIN_DB` is.

Measured after: the band-processed grid's worst **−0.016 dB** re the ceiling (clipper at
100 %, Safe, 5 ms, +6 dB; −0.068 dB with the clipper off); Terminus's own corpus
**−0.100 dB** (was −0.019). What Terminus does where it already held is unchanged: a
signal under the ceiling is still the exact delay (null tests green), the first stage is
untouched, a clipper that clips nothing still adds nothing; only a peak the safety stage
works on sits 0.1 dB lower. All 31 Terminus tests pass. Justified in
`docs/plugins/limiter.md`, decision 3, and its tests table.

## Tests run

All green; `cargo clippy -p cantus-dsp -p cantus-engine -p cantus-ui --tests` clean;
`cargo fmt` on the three crates.

- `cantus-dsp`: 421 passed (6 ignored), among them the new
  `terminus::tests_tp::band_processed_drums_driven_into_the_clipper_stay_under_the_ceiling`,
  `gravis::tests::a_slap_is_calibrated_against_a_finger_pluck`,
  `gravis::tests::every_technique_keeps_its_headroom_on_a_dense_riff`; all 31 Terminus
  tests and Culmen's (`the_ceiling_holds_in_true_peak_by_a_32x_reference` included).
- `cantus-engine`: 259 passed, among them
  `master_chain::a_maximizer_on_the_master_shows_on_its_reduction_meter`.
- `cantus-ui`: 505 passed (all targets), among them
  `instrument_levels::no_factory_preset_passes_full_scale_on_a_dense_riff` and the demos'
  tightened `every_demo_renders_clean_and_at_a_sane_loudness` and
  `the_startup_project_is_a_full_session_that_loops`. The demos' tests also pass in
  release (`cargo test --release -p cantus-ui --test demos`: 8 passed, 395 s).
- Reports (ignored, release): `demos.rs::levels` (`FULL=1 NOPARTS=1`),
  `demos.rs::master_drive`, `instrument_levels.rs::default_patches_staging_report`.
- Screens: `cargo run -p cantus-ui --example shoot -- <dir> 1 master maximizer startup`.
  `master` now shows the startup project's own chain (Aequalis, Multiplex, Latitudo,
  Culmen on the master's slots) with Culmen's window open from it, titled *Culmen
  maximizer (Insert 4 · Master)*, latency 3.9 ms; looked at in both themes. The window
  shows *Default* in its preset box: the demos write a preset's values into the slot, not
  its name.

## Files touched

Mine (new): `crates/cantus-dsp/src/terminus_tests_tp.rs`,
`crates/cantus-ui/tests/instrument_levels.rs`, `crates/cantus-ui/tests/measure/mod.rs`,
this file.

Changed:

- `crates/cantus-engine/src/fx/maximizer.rs` — `gain_reduction_db` (Culmen's file).
- `crates/cantus-engine/tests/master_chain.rs` — one test.
- `crates/cantus-dsp/src/terminus.rs` — `SAFE_MARGIN_DB` 0.15, the clipper's decimator
  through `os::lowpass(CLIP_BAND, CLIP_BETA)`, the test module's line.
- `crates/cantus-dsp/src/terminus_os.rs` — `lowpass()`, `Decim::with_filter()`
  (`Decim::new()` is `with_filter(&prototype())`, unchanged).
- `crates/cantus-dsp/src/gravis_string.rs`, `gravis_string_engine.rs` — `THUMB_LEVEL`,
  `POP_LEVEL`, the guide's `level`; `gravis_tests.rs` — two tests and their helpers.
- `crates/cantus-ui/src/views/inst/sampler_presets.rs` — `gain` of *Nylon guitar*, *Bells*,
  *Keys*.
- `crates/cantus-ui/src/demos/{ambient,ballad,funk,house,lofi,trap}.rs` — the master
  chains, Fervor on Sunday Strut's drum bus and Paper Lanterns' keys, Sunday Strut's bass
  back on the slap (trim −1 dB), the trims that keep Morning Light's and Paper Lanterns'
  Exemplar parts where they were.
- `crates/cantus-ui/tests/demos.rs` — the shared measures, the tighter loudness and
  true-peak bounds, `levels` with true peak and `NOPARTS`, `master_drive`.
- `crates/cantus-ui/examples/shoot.rs` — `master` on the startup project's chain.
- `docs/plugins/limiter.md` (decision 3, the clipper, the tests table, the feature row),
  `docs/instruments/gravis.md` (techniques, tests, measured).

## Left

- **Instrument staging** (section 4): Lamina's and Exemplar's default output ~6 dB up, with
  a migration that keeps saved projects as they sound — a decision for the instruments'
  owner.
- **Culmen's `TP_MARGIN_DB`** (0.05 dB) now adds to Terminus's 0.15: Culmen's output sits
  ~1.15–1.2 dB under its ceiling by the 4× meter (−1.14 dBTP measured on the demos).
  Culmen's builder may drop its own margin now that Terminus holds band-processed material
  by itself (its ceiling test would say); its doc comment still quotes Terminus's old
  −0.019 dB. Not changed here: it is Culmen's.
- The demos' master inserts carry a preset's values but not its name (*Default* in the
  window's preset box); naming a slot's preset is the demos builder's `Song::effect`.
- Pumping was judged by numbers, not ears: the most peak reduction any song asks of Culmen
  is about 7 dB (Paper Lanterns, Midnight Static, input true peak + drive − output), on
  transients; the songs that needed more than 7.5 dB of drive were left a little quieter.
