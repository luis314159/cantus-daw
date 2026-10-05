# Culmen — the mastering maximizer

`EffectKind::Maximizer` · parameters `crates/cantus-core/src/fx/maximizer.rs` · DSP
`crates/cantus-dsp/src/culmen.rs` (+ `culmen_engine.rs`, `culmen_telemetry.rs`, tests in
`culmen_tests.rs` and `culmen_tests_stages.rs`) · engine `crates/cantus-engine/src/fx/maximizer.rs` ·
editor `crates/cantus-ui/src/views/fx/maximizer.rs` (+ `maximizer_{display,meters,bands,curve,cards,learn,demo}.rs`) ·
strings `crates/cantus-ui/locales/fx/maximizer.{en,es}.txt` · tests
`crates/cantus-engine/tests/fx_maximizer.rs`, `crates/cantus-ui/tests/fx_maximizer.rs`, and the
plug-in suite in `crates/cantus-plugins/tests/`. Exported as **Cantus Culmen** (CLAP and VST3).
References: FL Studio's Maximus, iZotope Ozone 12's Maximizer, FabFilter Pro-L 2 and Pro-MB
(`research/fabfilter/pro-l.md`, `pro-mb.md`), ADR 0026.

*Culmen* is Latin for the summit, the top of a roof — the last thing on the master.

## What the references do well, and what their users miss

- **FL Maximus** is the owner's reference: three bands plus a *master* band, each with a curve
  you draw (a compressor above, an expander or upward compressor below), pre and post gain,
  a scrolling level picture, solo per band. Users love the flexibility and the picture; they
  complain that it is opaque (the curve's points have no numbers, attack and release are
  percent-like), that its "brickwall" is not true-peak safe, and that it has no loudness
  metering at all — so everyone puts a separate meter and a limiter after it.
- **Ozone 12's Maximizer** (IRC modes, IRC 5 multiband) is the loudness benchmark: *Learn
  Input Gain*, true-peak ceiling, a soft clipper, stereo independence split into transient
  and sustain, upward compression, delta, a gain-reduction trace. Its *Learn* listens once
  and guesses (iZotope says it is not for compliance), and its multiband limiting is a black
  box: four bands you cannot shape.
- **Pro-L 2** is the reference for one honest picture (input, output, reduction and loudness
  on one time axis) and for delivery discipline (targets, unity gain, audition). Its users
  ask for multiband and M/S, a knee, a scrollable loudness history and shown latency.
- **Pro-MB** is the reference for dragging bands on the spectrum; its users ask for two-sided
  dynamics per band, a visible threshold, real time units and a ratio that shows a number.

Culmen takes the shape of Maximus (bands plus a full band, each with a two-sided curve, into
a brickwall) and builds it from the family's tested parts, then adds what none of the four
has in one place.

## The signal path

```text
in ─┬─ band stage (Multiplex: 1–3 bands) ─ full band (Multiplex: 1 band) ─ × drive ─ brickwall (Terminus) ─┬─ Δ? ─ 1:1 / match ─ dither ─ out
    └─ delay (band latency) ─────────────────────────────────────────────── × drive ─ delay (brickwall) ────┘
```

- **The band stage** is a [Multiplex](multiband.md) with up to three bands — *Low*, *Mid*,
  *High* — split at `x1` and `x2` (a third of an octave apart at least). With two bands the
  split is at `x1` and the bands are Low and High; with one there is no band stage. The
  crossovers are Multiplex's three modes: **Dynamic** (no split: one shelf per crossover that
  moves only while a band moves — zero latency, a wire to the bit while nothing acts),
  **Minimum phase** (a Linkwitz–Riley split that sums to an all-pass), **Linear phase** (the
  telescoped linear-phase split: no phase shift at all, 48 ms of latency). Slope 6–48 dB/oct.
- **The full band** is a one-band Multiplex in its dynamic mode — a pure gain, a wire while it
  does not move: Maximus's *master* band, on the sum of the bands.
- Each of the four has a **two-sided curve**: above `thr` the upper ratio, below `thr_lo` the
  lower ratio, each within its range, with a knee, attack and release in milliseconds, a
  band gain after the dynamics, and a placement (stereo-linked, mid only, side only). One
  rule reads every ratio: **above 1:1 squeezes, below 1:1 stretches**, on either side — so a
  band can hold its peaks down and lift its quiet parts at once (upward compression), or
  expand its transients upward, or gate its floor.
- **The drive** (`gain`, −12 … +30 dB) pushes the result into **the brickwall**: a
  [Terminus](limiter.md) with its own gain at 0 dB — the look-ahead transient stage, the
  release stage, the true-peak safety stage verified by a 32× reference, the 4× clipper in
  front, the DC filter, its five styles as Culmen's **character** (Clean, Punch, Loud, Bus,
  Safe, each printed as the numbers it sets), and its **per-stage stereo link** (transients
  and release, Ozone's *stereo independence*).
- **What you hear** is Culmen's, because it has to cover every stage: the output; the
  **delta** — the input, driven and delayed through lines as long as the stages', minus the
  output: everything Culmen changes, exactly; at full level, at **1:1** (the drive undone), or
  **matched** to the input's measured short-term loudness. Then **dither** (Terminus's TPDF
  ditherer, 24–16 bits, flat, basic or optimized shaping, silent on digital silence).

Nothing is written twice: Culmen is 600 lines of glue and meters over three DSPs that already
have their own null tests, block-size tests and published numbers.

## Parameters

| Key | What | Range | Default |
|---|---|---|---|
| `gain` | drive into the ceiling (what *Learn* sets) | −12 … 30 dB | 0 |
| `ceiling` | the most the output reaches (dBTP with `tp`) | −30 … 0 dB | −1 |
| `style` | the brickwall's character: Clean, Punch, Loud, Bus, Safe | 0 … 4 | Clean |
| `look` | look-ahead | 0 … 5 ms | 2 |
| `lim_att`, `lim_rel` | the brickwall's release stage | 0.1 ms … 10 s, 1 ms … 10 s | 10, 200 ms |
| `link_t`, `link_r` | stereo link of the transient and the release stage | 0 … 100 % | 75, 100 % |
| `lim_knee` | soft knee under the ceiling | 0 … 12 dB | 0 |
| `tp`, `dc` | true-peak limiting; DC filter | off / on | on, off |
| `clip` | the clipper in front | 0 … 100 % | 0 |
| `bands` | 1 (no band stage), 2, 3 | 1 … 3 | 3 |
| `x1`, `x2` | the splits | 20 Hz … 20 kHz | 150 Hz, 5 kHz |
| `xmode` | Dynamic, Minimum phase, Linear phase | 0 … 2 | Dynamic |
| `slope` | crossover slope | 6, 12, 24, 48 dB/oct | 24 |
| `delta` | hear what Culmen changes | off / on | off |
| `monitor` | Output, 1:1, Match | 0 … 2 | Output |
| `dither`, `shape` | off, 24, 22, 20, 18, 16 bits; flat, basic, optimized | | off, flat |
| `target` | the loudness the meters judge and *Learn* aims at | −36 … −5 LUFS | −14 |
| `{lo,mid,hi,all}.on` | the band works (off: it keeps its place, does nothing) | | on |
| `{lo,mid,hi}.solo` | hear this band alone | | off |
| `{band}.thr`, `.ratio`, `.range` | the upper side | −60 … 0 dB, 0.1 … 100, 0 … 60 dB | −12, 1:1, 12 |
| `{band}.thr_lo`, `.ratio_lo`, `.range_lo` | the lower side | −90 … 0 dB, 0.1 … 100, 0 … 60 dB | −40, 1:1, 12 |
| `{band}.knee`, `.att`, `.rel` | knee; attack and release | 0 … 24 dB; 0.02 … 250 ms; 5 … 5000 ms | 6, 10, 150 |
| `{band}.gain` | the band's gain after its dynamics | −24 … 24 dB | 0 |
| `{band}.place` | stereo (linked), mid only, side only | 0 … 2 | stereo |

Every parameter is stored sparsely: a new Culmen writes nothing, and **at every default it
touches no sample** — the signal comes out delayed by the brickwall's look-ahead (and the
true-peak detectors), exactly, while it stays under the ceiling.

## The editor

The device frame (ADR 0024 §8), 1180 × 720 at 100 %, in the dynamics hue, in three rows:

1. **Gain over time** (ten seconds, 10 ms columns) beside **Loudness**.
   - The output's true peak per column as a fill, the input's peak as a line over it — lined
     up by the latency, so the gap *is* what Culmen did; the brickwall's reduction hangs from
     the top (12 dB fill a third of the plot); the **ceiling is a dashed line you drag**
     (double-click: −1 dB). Under the plot, **a lane per band** — Low, Mid, High, Full — shows
     what that band cut (below its line) or lifted (above), ±6 dB, with its value now. No
     reference shows every stage's gain on one time axis: Maximus shows levels, Pro-L the
     limiter, Ozone the limiter and a separate upward trace.
   - Hover reads that moment: how long ago, input, output true peak, brickwall reduction.
   - **Loudness**: the integrated loudness as the device's hero number; the verdict against
     the target in words *and* a shape (a filled disc passes, a hollow triangle does not);
     momentary, short-term, loudness range, true-peak maximum, **PLR** (peak to loudness:
     under about 8 dB a master starts to sound squashed) and the **input's** integrated
     loudness — all at once, all measured by Terminus's conformance-tested code (EBU Tech
     3341/3342), from the same 10 ms columns.
   - **The target**: Streaming −14 (Spotify, YouTube, Tidal), Apple −16, CD −9, Club −7,
     Broadcast −23 (EBU R128) — choosing one sets the target **and** the ceiling in one undo
     step and prints its rule; or drag the value for a custom one.
   - **Learn drive** (below) and **Reset**.
2. **Bands**, the **curve**, the **output**.
   - The band strip is the spectrum (20 Hz – 20 kHz, log) as the bands share it: each band a
     tinted column with its name and **its gain now as a bar** (down cuts, up lifts) and value;
     the **splits are dragged** (kept a third of an octave apart); a click selects a band,
     a double-click solos it; the **full band** is its own column at the right. Under it:
     how many bands (None / Two / Three), the crossovers (Dynamic / Minimum phase / Linear
     phase), the slope (6 / 12 / 24 / 48).
   - The curve card is titled with the selected band and carries its on/off lamp: **level in
     across, level out up**, −60 … 0 dB. The two threshold points slide along the curve;
     each side's slope is dragged up or down for its ratio (double-click: back to 1:1); the
     band's detector level now rides on it as a dot. The threshold and the ratio are printed
     beside each point (`−12.0 dB · 2.5:1`, `1:2.0` for an expansion) — Maximus's curve,
     with numbers.
   - The output card: Hear (Output / 1:1 / Match), Δ, true peak, DC; the brickwall's
     character (Terminus's styles by name); dither and its shaping.
3. **The selected band's cells** (threshold, ratio, range, lower threshold, lower ratio,
   lower range, knee, attack, release, band gain, stereo placement, solo) and **the
   brickwall's** (drive — the device's one large knob — ceiling, lookahead, attack, release,
   link transients, link release, clipper).

The footer has the help line and the latency chip ("Latency 3.9 ms" at the defaults; 52 ms with linear-phase
bands), always on screen; the output meter shows the true peak with the session's maximum
held. Every drag and knob turn is one step of the project's undo; a target is one step; a
whole *Learn* session is one step.

### Learn drive — a closed loop, not a guess

Ozone listens once and guesses; but a brickwall's loudness does not grow one for one with its
drive (the more it limits, the less each dB adds), so a guess lands short. Culmen's *Learn*
(`maximizer_learn.rs`) measures three seconds of the **output** (after half a second for the
stages to settle), sets the drive, and measures again; from the second step on, the next drive
comes from the slope measured between the last two (a secant search, the slope kept in
0.15 … 1). It stops when the output reads the target within **0.3 LU**, after eight steps,
when the drive reaches the end of its range, or after ten seconds of silence — and says
which, in words. Every step is the same gesture, so the session is one undo step.

## Decisions, and why

1. **Built from the family's parts.** The band stage *is* Multiplex, the brickwall *is*
   Terminus, the meters *are* Terminus's loudness code, the dither *is* Terminus's ditherer
   (made public for this). Each already has null tests, block-size tests and published
   measurements; Culmen's own code is the order, the drive, the reference path, the monitor
   and the telemetry. A fix in one of them reaches Culmen too.
2. **The drive goes between the bands and the brickwall,** not before the bands. Learning or
   turning the drive then pushes only into the ceiling, as Ozone's *Gain* does, and the bands'
   thresholds stay where they were set against the mix — a band that was set to catch the kick
   keeps catching the kick at any loudness.
3. **Delta, 1:1, match and dither are Culmen's, not Terminus's,** because they must cover
   every stage: Terminus's delta would only hear the brickwall. The reference path is the same
   multiplication by the same drive at the same sample through lines as long as the stages'
   latencies, so the delta of a Culmen at rest is exact silence, and out + Δ = the driven,
   delayed input to 1e-5.
4. **The stage is cut on the 16-sample grid and the 10 ms columns,** both counted from the
   start of the stream, and the band gains are read only on that grid — so neither the sound
   nor the telemetry depends on the host's blocks (both tested at tolerance 0).
5. **A true-peak margin of 0.05 dB.** Terminus holds its own corpus 0.019 dB under the   ceiling; band-processed drums and hats driven 18 dB into its clipper (minimum-phase split)   measured 0.032 dB over by the 32× reference. Culmen asks Terminus for the ceiling less   0.05 dB with true peak on, and the test holds the output at the ceiling itself in every   crossover mode. (A first measurement read 0.9 dB over; that one was the test — a buffer   cut off at full level reconstructs an overshoot at its last sample — so the test lets the   music end in silence, as a song does.)
6. **The first settings snap, later ones glide.** Settings given before the first sample are
   taken at once by every stage, however many came before (`Multiplex::snap_next`, a
   three-line addition) — the exported plug-in is activated with its defaults and then given
   its state, and must sound as the DAW's insert from the first sample. The stages are left unconfigured until the
   first `set`, so a project opened with Culmen working starts working on its first sample,
   with no glide in from the defaults.
7. **Stereo link is per brickwall stage, bands are linked or M/S.** Ozone's *stereo
   independence* and Pro-L's per-stage link are the same idea and the one that matters on a
   master; the bands detect the louder side (linked) or work on the mid or the side alone.
   Unlinked L/R band dynamics would move the image on a master, and Multiplex does not have
   them; written down so it is a decision.
8. **Telemetry as a ring of 10 ms columns with a counter** (Terminus's design, decision 4 of
   `limiter.md`), six columns of nine numbers — output and input energy, input and output
   peak, the brickwall's reduction, the four bands' gains — so the editor misses nothing up
   to 60 ms late (the plug-in window repaints at about 30 Hz); slot 9 is the brickwall's
   reduction now, for a strip's GR meter.
9. **No sidechain.** Terminus's stem-mastering trigger makes sense on a limiter; on a
   multiband maximizer every band would need its own key. Left for when it is asked for.

## Factory presets

*Transparent master* (a gentle full band, 3 dB of drive) · *Streaming −14* (low band held,
full band glue, −14 / −1 dBTP) · *Loud and dense* (Loud character, clipper, upward lift in the
full band, −8) · *Club master* (Punch, clipper, the low band on the mid only, −7 / −0.1) ·
*CD master, 16-bit* (−9 / −0.3, 16-bit optimized dither) · *Low-end control* (the low band
below 120 Hz squeezed on the mid only) · *Upward lift* (the mid band and the full band lifting
what is quiet) · *Mid/side polish* (side lows held down, side highs opened up) · *Broadcast
safe* (Safe character, −23 / −1) · *Classic three-band* (Maximus's own shape: three bands
compressing into a full band, minimum-phase crossovers).

## What the tests hold

DSP (`cargo test -p cantus-dsp --release culmen`, 17 tests):

| Test | Holds |
|---|---|
| `a_culmen_with_nothing_to_do_is_an_exact_delay` | at the defaults, and with every band at 1:1 in each crossover mode × 1/2/3 bands × every slope: output = input delayed by `latency()`, **to the bit**, both channels |
| `bands_that_are_not_reached_and_the_delta_at_rest_are_exact` | bands whose thresholds the signal never reaches: a delayed wire to the bit; the delta of a Culmen at rest is exact silence |
| `the_result_does_not_depend_on_the_block_size` | Dynamic, Split and Linear, everything working (bands both ways, full band, drive, clipper, DC, Match, optimized dither): blocks 1, 7, 64, 1000 equal 4096 at tolerance 0 — **and the telemetry too** |
| `it_is_deterministic_and_finite_on_nasty_input` | ×40 noise into +30 dB of drive, a full-scale square, 1e-30 sections: identical twice, all finite |
| `silence_in_is_silence_out_and_no_denormals_after_noise` | every mode with dither on: exact zeros a few seconds into silence after noise; no subnormal anywhere |
| `the_ceiling_holds_in_true_peak_by_a_32x_reference` | band-limited drums and hats through every stage, +18 dB of drive, the clipper, every crossover mode: output true peak by the independent 32× reconstruction at or under the −1 dBTP ceiling (0.005 dB of tolerance), both channels |
| `the_latency_reported_is_the_latency_measured` | an impulse arrives exactly `latency()` samples later: Dynamic and Linear bands, no band stage, look-ahead 0 / 2 / 5 ms |
| `each_band_acts_on_its_own_range` | the low band at 4:1 takes a 40 Hz tone to its static curve (within 0.6 dB of −10.5) and leaves a 6 kHz tone within 0.1 dB, in all three crossover modes |
| `the_full_band_follows_its_two_sided_curve` | downward compression (−10.5 dB) and upward compression (+5 dB) on steady tones within 0.3 dB of the curve |
| `two_bands_split_at_the_first_crossover_and_one_band_is_none` | with two bands the mid band does not exist and the high band starts at `x1`; with one, no band stage whatever the bands say |
| `solo_hears_one_band_and_a_band_switched_off_does_nothing` | the low band soloed: a 6 kHz tone more than 40 dB down, 60 Hz kept; a band switched off is a wire to the bit from the first sample |
| `delta_is_everything_culmen_changes` | output + delta = the input driven and delayed, within 1e-5, Dynamic and Linear |
| `unity_hears_the_processing_without_the_drive` | 1:1 is the output divided by the drive (1e-5) |
| `match_levels_the_output_to_the_input_loudness` | 12 dB of drive, Match: integrated loudness of output and input within 0.5 LU |
| `dither_lands_on_the_grid_and_silence_stays_silent` | 16-bit: every sample on the 16-bit grid; silence after music stays exactly silent |
| `telemetry_counts_columns_and_reports_every_stage` | a column per 10 ms; latency and rate in the header; the low band's cut, a band with nothing to do at 0, the brickwall's reduction, the output's true peak and the input's peak where they are |
| `settings_before_the_first_sample_are_taken_at_once` | defaults, then the real settings (a plug-in activated, then given its state) sound exactly as the real settings given once — no glide in from the defaults |

Engine (`crates/cantus-engine/tests/fx_maximizer.rs`): a new Culmen renders the mix delayed
by its latency to the bit, in each crossover mode; a busy Culmen renders identically at blocks
1, 7, 64, 1000, 4096; a mix driven 18 dB leaves the engine under −1 dBTP by the 32×
reference; the unit reads the spec and counts its columns. Every key's reading
(`fx::maximizer::settings`) is checked field by field against the DSP's defaults.

Editor (`crates/cantus-ui/tests/fx_maximizer.rs`, headless frames): it paints every stage and
the loudness from a demo run through the real DSP; dragging the ceiling line moves the ceiling,
one undo step; a split dragged on the strip moves `x1`, one undo step; a click selects a band,
a double-click solos it, the full band's column selects the full band; the curve's threshold
point and its upper slope are dragged (−24 dB; 2:1), one undo step each; and **Learn drive
against the real DSP** — each frame the editor reads what Culmen measured and Culmen runs with
what the editor set — ends *Done* within 0.3 LU of −14 LUFS, and one undo takes the whole
session back. The secant search alone (`maximizer_learn.rs`) lands on its target in at most five
steps where loudness grows one for one and where limiting flattens it, and stops on silence
and at the end of the drive's range. Every preset names real parameters in range.

Export (`crates/cantus-plugins/tests/`): Culmen runs every test the family runs — bit-identical
to `cantus-dsp` and to the engine's unit through CLAP and VST3, at block sizes 1 to 4096,
automation of the drive on the 16-sample grid, ids in `golden/ids.txt`, state both ways,
latency reported, zero allocations in `process`, every word in English and Spanish, its editor
drawn in the plug-in window, and hosted by Cantus's own CLAP host as the native insert.

## Feature matrix

| | FL Maximus | Ozone 12 Maximizer | Pro-L 2 | Culmen |
|---|---|---|---|---|
| Bands | 3 + master | IRC 5: 4, not shaped | — | **3 + full band, each shaped** |
| Curve per band | drawn, no numbers | — | — | **two-sided, dragged, with numbers** |
| Upward compression | in the curve | parallel "Upward" | — | in every band's lower side |
| Crossovers | one type | — | — | **Dynamic (no latency) / minimum / linear phase, 6–48 dB/oct** |
| Brickwall | not true-peak | true peak | true peak, 8 styles | **true peak verified by a 32× reference, 5 styles printed as numbers** |
| Stereo link | per band | transient / sustain | per stage | per stage; bands linked or M/S |
| Clipper | — | soft clip 4× | — | 4× clipper, exact wire when idle |
| Loudness metering | — | yes | yes | **M, S, I, LRA, TP, PLR, input — conformance-tested** |
| Targets | — | target LUFS | targets | **five deliveries, target + ceiling in one step, verdict in words + shape** |
| Learn gain | — | listens once, guesses | — | **closed loop until within 0.3 LU, one undo step** |
| Delta | — | yes | limiter only | **everything Culmen changes, exact** |
| Level-matched listening | — | gain match | unity gain | 1:1 and measured Match |
| Dither | — | yes | yes | 24–16 bit, three shapes, silent on silence |
| Display | levels | GR trace | in/out/GR/loudness | **in, out, brickwall GR and a lane per band on one time axis** |
| Latency shown | — | — | — | always, in the footer |
| Undo | the plug-in's | the plug-in's | the plug-in's | the project's, one step per gesture |

## Not done, and why

- **A sidechain key** (decision 9), **unlinked L/R band dynamics** (decision 7).
- **Per-band look-ahead and oversampling of the bands**: Multiplex has the look-ahead; it would
  add latency for little on a master. Not exposed.
- **A song-time loudness history** like Terminus's *Song* view: Culmen keeps ten seconds of
  history; the song-time view is Terminus's, one insert away.
- **Learn from the selection offline** (render the time selection through the chain and set
  the drive before playing): the closed loop needs the transport to play; an offline pass needs
  the render API from the editor, and is the next step.
