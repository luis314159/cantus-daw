# tympanum-fit — measuring a recorded kit into Pulsus model 5's tables

Off-line, never shipped (round 40 phase 3, ADR 0124's second amendment). Python 3 with
numpy, scipy and soundfile; run with `python -I`.

1. Render the recorded kit's hits through the engine (13 articulations x 4 velocities, 4 s
   each) — and, for the hat's half-open states, with `TYMP_HAT_OPEN=0.25|0.5|0.75`:
   `cargo run -p cantus-ui --example tympanum_measure -- hits <out> <content root> pack:drskit-tympanum/kit`
2. `fit.py <hits> fit.json` — modes, band envelopes, stick per piece and velocity
   (`fit_states.py fit.json 25=<hits> 50=<hits> 75=<hits>` adds the half-open states).
   `synth.py` is the same synthesis in Python, to check a fit before the engine.
3. `tables_blob.py <cantus-dsp/src> fit.json gains.json 0 [fit_virtuosity.json gains_virtuosity.json 16]`
   — the engine's blob (`pulsus_measured.bin`, format `CMT1`) and its small Rust header:
   the studio kit's DRSKit tables at ids 1–8, the jazz kit's Virtuosity tables at 17–24.
4. Render the modelled kit through the engine (step 1 with `model:studio`), then
   `calibrate.py <model hits> <recorded hits> gains.json` and step 3 again: each table's
   level, and its modes' weight against its bands, through the engine's virtual
   microphones (`cycle.sh` does one such round for the studio kit, `cycle_jazz.sh` for the
   jazz kit against Virtuosity Drums' own hits; two converge).
5. `metrics.py <hits> <hits> …` — the analysis's cymbal measures side by side;
   `make_b4.py` — the listening files (old, new, recorded at one loudness);
   `stereo_sim.py` — the overhead pair's correlation from the paths' own rules.

The fits themselves (`fit.json`, ~0.8 MB each) are not kept: step 2 makes them again from
the packs' recordings; `data/` keeps each kit's calibration (`gains.json`,
`gains_virtuosity.json`), and the engine's blob is the result.

## The hybrid's residual (round 40, `drums-hyb`; ADR 0124's third amendment)

`refit_hybrid.sh <jazz|studio> <content root> <work>` — one command, to run again whenever
model 5's blob changes: `tympanum_hybrid takes` (the recorded kit and the model, the same
schedule) and `probe` (the engine's own source → mix response), `residual_fit.py` (the
spectral residual, deconvolved to the source; its report says how much of each recording the
model already explains), `residual_zip.py` (the pack and its manifest entry; the zip is
uploaded as a `content-r40` release asset), `residual_manifest.py` (the entry written into
`content.json`). `residual_pack.py` is the `CRS1` format's own writer; `hybrid_measure.py`
measures generated, hybrid and recording side by side.

Before a refit whose zips are to be published, bump `VERSION` (and `DATE`) in
`residual_zip.py`: the zip is named `tympanum-hybrid-<kit>-<version>.zip`, a published file
name is never replaced, and a user holding the older version is offered the update. The model
hash the residual was fitted on lives in the residual itself (`CRS1`'s header, the fit's
report's last line), not in the manifest. Version 2 (`hyb-refit`, after `drums-gen`'s blob
merged): `docs/round40/hyb-refit.md`.

## drums-gen (round 40): the onset, the hat's states, the colour, the groove

6. `early.py <fit.json> <reference hits> <out.json>` — each drum mode's phase at the strike
   and its early part (blob version 2), the stick refitted after. Run on the fit before
   `tables_blob.py`.
7. Through the engine, against the recording: `calibrate.py` (levels), `fit_hat_tonal.py`
   (the hat's states: partials' level and T60 per layer, fitted in the synthesis against
   the recording's 6-16 kHz effective components at 0/50/200/400 ms — renders at
   `TYMP_HAT_OPEN=0.25|0.5|0.75` for the references), `calibrate_bands.py` (each drum's
   colour per band and region of the hit), `calibrate_tonal.py` (a cymbal's partials per
   frequency region). Their trims live in `data/gains*.json` (`<TABLE>_BAND_*`,
   `_TONAL_*`, `HAT_MODES_*`, `HAT_T60_*`); `tables_blob.py` applies them.
8. Measures: `groove_hf.py` (the groove's fine highs whole and per bar; the A/B's or
   `tympanum_measure model <out> <root> studio|jazz [name]`, with `TYMP_SEED=n` and
   `TYMP_GROOVE_KEYS=42,46` to read one part on several draws), `attack.py` (a drum's
   octave bands in its first 250 ms), `grain.py` (envelope fluctuation), `metrics.py`.
   `make_listen.py` writes a listen (previous · now · recording).

The fits used for the blob at drums-gen's end: DRSKit's `fit7` with `early.py` and the ride
refitted at 440 partials a layer (`fit.py ... ride`, `SPEC["ride"]["nmax"]`), Virtuosity's
`fitv2` with `early.py`.

## drums-gen 2 (round 40): the gap analysis's cues

9. `accept_b10.py <studio hits> <jazz hits> [--groove …] [--cues 1,2]` — the fifteen
   cues of `analysis-drums-2/report.md`, measured as the analyst did, PASS/FAIL per kit.
10. `fit.py` caps a cymbal band's noise by the recording's residual (`RESID`), reads a
   partial's T60 only above the floor (`FLOOR_T60`), cuts a drum's early noise under its
   fundamental (`SUBCUT`); each switch is `TYMP_FIT_*=0` to turn it off.
11. Calibration without a rebuild: a debug `tympanum_measure` reads the blob named by
   `CANTUS_MEASURED_BLOB`. Each round: `tables_blob.py` into a scratch directory, render
   the hits, then `calibrate.py`, `calibrate_bands.py`, `calibrate_stick.py` (the stick's
   burst and knock) and `calibrate_tonal.py` (the hat's closed stroke by its flatness).
   Two rounds converge. `tables_blob.py` clamps every band so it never rises after its
   peak (`no_swell`).
12. `TYMP_SPEC=o9.solo=1,o9.level=-8` on the measure example: a mic alone, a level moved.

## ladder (round 40): the representation ladder before a model 6

`ladder.py <dir>` resynthesises each recorded hit (12 articulations x v80/v127 x both kits)
through richer descriptions (R1 model 5's own fit ... R4 texture statistics, R5 anchor, R3o the
magnitude oracle); `ladder_dense.py <dir> all` adds R3b (the residual per 11.7-Hz bin);
`ladder_measure.py <dir>` measures every rung against the recording (with the cross-band
envelope correlation) into `measures.md`; `ladder_cut.py` and `ladder_seeds.py` are the cost
probe and the measures' noise ceiling; `ladder_whistle.py` reads the cymbal tails (the owner's "iuuu": bands that climb near the end); `ladder_kit.py build <dir> <kit>` writes the blind
MUSHRA + ABX page (`ladder_kit.html`, `ladder_kit.js`), `ladder_kit.py score` reads a
listener's file. Method and results: `docs/round40/ladder.md`.

## drums-gen 3 (round 40): the cost and the open cues, measured only

13. `dump_diff.py <before> <after>` — two renders of `cantus-engine`'s ignored `full_kit_dump`
   (`CANTUS_TYMP_DUMP=<dir>`: both kits, a groove with every piece) compared sample by sample:
   a saving that must not be heard stays under −80 dB re peak.
14. `variant.py <fit> <out> <pieces> <op>…` — a fit with one part of some pieces silenced
   (`knock=0`, `burst=0`, `early=0`, `glide=0`, `modes=0`, `bands=0`, `modes<HZ`, `modes>HZ`,
   `bands<HZ`, `modes@LO-HI`, `bands@LO-HI`): render it with `CANTUS_MEASURED_BLOB` to find
   which part makes a measure.
15. `trim_b11.py <fit> <out> studio|jazz` — the eleventh listen's table edits (the studio
   kick's long fundamental, the studio snare's wires, the drums' upper partials capped, a soft
   tom's highs left to its noise), on a fit before `tables_blob.py`. The blob at drums-gen 3's
   end: `trim_b11.py` on drums-gen 2's `fit_r4` / `fitv_r4` (kept in `tmp\dg3\keep\` as
   `fit_b11.json`, `fitv_b11.json`), with `data/gains*.json`.
16. `tables_blob.py` reads `<TABLE>_DECAY` = [lo Hz, hi Hz, T60 s]: past its peak each band in
   that range falls at least 60 dB in T60, after the colour's corrections.

## Paths (every script here)

Nothing names a machine's folders; the environment does (defaults in brackets):

| variable | what |
|---|---|
| `TYMP_WORK` [`$CANTUS_WORK/tympanum-work` for the cycles, `.` for the Python] | the measuring work folder: `meas9/` the recorded kits' hits (`tympanum_measure hits <TYMP_WORK>/meas9 <content root> pack:drskit-tympanum/kit pack:virtuosity-drums/kit …`), `out/` the fits and gains, one folder per render |
| `TYMP_MEAS` [`$TYMP_WORK/meas9`] | the recorded hits `accept_b10.py` and the analyses read |
| `TYMP_CONTENT` [`$CANTUS_REFS/tympanum/content`] | the content root holding the recorded packs (Cantus's own installs: DRSKit, Virtuosity Drums, Crocell, Muldjord, Big Rusty — downloaded on demand, never in the repository; `python -I tools/sound-refs/fetch.py fetch tympanum tympanum-extra --packs --cargo` puts them there) |
| `TYMP_LISTEN`, `TYMP_ANALYSIS_OUT` | the analyses' input (a listen's folder, `make_listen.py`'s files) and output |
| `CANTUS_PY` [the venv `$CANTUS_REFS/venv`, else `python`] | the Python with `requirements.txt` (or `tools/sound-refs/requirements.txt`, which covers every sound tool) |
| `SLOT` [`bash tools/build/slot.sh`] | the build-slot wrapper (`tools/build/README.md`); empty inside a slot |

## The two analyses (round 40: independent analysts, audio only)

Each analyst heard the listening files only, never the model's code, and named what sounds
synthetic with a measured target. Their scripts and reports are kept so a later model can be
measured the same way:

- `analysis-1/` — *why the cymbals sound glassy* (the b3 listen against five recorded kits):
  `report.md`, scripts `s01`…`s17` (`common.py`: `TYMP_LISTEN` the listen's folder — its
  `golpes-*` and groove files —, `TYMP_ANALYSIS_OUT`, `TYMP_MEAS` for `s14`; `s16` compares two
  listens, the earlier one in `TYMP_LISTEN_B2`).
- `analysis-2/` — *the element-by-element gap analysis* of the b9 listen: `report.md` with **the
  fifteen ranked "synthetic" cues and their targets** (the ones `accept_b10.py` measures),
  scripts `s00`…`s16` (`s01`, `s02`, `s04`, `s06` write the JSON/NumPy intermediates the later
  ones read, into `$TYMP_ANALYSIS_OUT/scripts/`: run them in order).

```bash
export TYMP_LISTEN=<listen folder> TYMP_ANALYSIS_OUT=<out> TYMP_MEAS=<meas9> MPLBACKEND=Agg
cd tools/tympanum-fit/analysis-2 && for s in s00_segment s01_drums s02_drum_summary s04_cymbals; do $CANTUS_PY -I $s.py > $TYMP_ANALYSIS_OUT/$s.txt; done
```

## Probes (`probes/`)

Small diagnostics from the fitting rounds, on hit files (`tympanum_measure hits`): `diag.py`
(a piece's tonal share per band, modal against noise power, with its fit), `diag2.py` (onset,
peak and band levels over time, recording against model), `diag3.py` (one band of one hit
against the fit's own synthesis), `hf.py` (the highs of named pieces, two files), `pr.py` (the
6–16 kHz band's effective components). Each prints its arguments in its first lines.

Python dependencies: `requirements.txt` (numpy, scipy, soundfile, matplotlib).
