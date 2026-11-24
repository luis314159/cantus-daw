# Arcus reference database: cello, double bass, viola, violin

Measured 2026-11-24 by the scripts in `tools/arcus/study/` (its README says how to run them),
with `python -I` in a venv that pins numpy 2.3.3, scipy 1.16.2, matplotlib 3.10.6 and
soundfile 0.13.1. **No audio is stored here.** The recordings stay under
`CANTUS_STUDY_DOWNLOADS`, and each source's `SOURCE.md` gives its
licence.

## Files

| File | What |
|---|---|
| `catalog.json` | every reference file as a job: source, instrument, family, technique, dynamic, lossy, section (`catalog.py`) |
| `notes/<instrument>.jsonl` | one line per analysed note, with every feature below (`extract.py`) |
| `phrases.jsonl` | transitions, re-bows and vibrato in played phrases (`phrases.py`) |
| `<family>.json` | the distributions across instruments: cello, bass, viola, violin (`aggregate.py`) |
| `summary.json` | the headline numbers per family |
| `validation.json` | the extractor run on synthetic notes with known values (`validate_synth.py`) |
| `cnsm_validation.json` | the harmonic-peak body estimate against three violins' measured bridge admittances (`cnsm.py`) |
| `friction.json` | measured rosin friction on cello G strings against the hyperbolic curve (`friction.py`) |
| `bowing.json` | the string's own centroid against bow force, speed and β (`bowing.py`) |
| `plots/*.png` | the figures (`plots.py`) |

**Run order:** `validate_synth.py`, then `catalog.py` → `extract.py --workers 3` → `phrases.py` →
`aggregate.py` → `plots.py`. The extra studies are `cnsm.py`, `friction.py` and `bowing.py`. To
make the ABX set, run `make_abx.py`.
`extract.py` can be resumed. To measure Arcus the same way, render its notes to WAV, add them to
the catalog as one more instrument, and run the same chain: the model is then judged by the
**same estimator** as the recordings, so the estimator's own biases cancel.

## How a distribution is taken

Each feature is first reduced to one value per instrument per cell: the median over that
instrument's notes in a family × register × dynamic cell. The family's distribution is then
taken over those per-instrument values: median, min, max, plus p25/p75 when the cell has four
or more instruments. Pooled per-note p10/p50/p90 are given beside them.

A model is **inside the band of real instruments** when it lies within [min, max] across
instruments, or within [p25, p75] when there are four or more. One cello with 300 notes
counts once, as does one with 30.

- **Dynamics** are ordinals: pp 1, p 2, mp 3, mf 4, f 5, ff 6. They are grouped as soft
  (1–2), medium (3–4) and loud (5–6). VSCO's v1 counts as p and its v3 as f.
- **Registers** are octaves from the family's lowest note: cello C2 (MIDI 36), bass E1 (28),
  viola C3 (48), violin G3 (55).
- **Kept apart:**
  - VSCO's **sections** (`section_apart`), since a section is not one instrument;
  - **lossy** sources (Philharmonia's MP3s, `lossy: true`). They are included, but MP3 coding
    noise sits under −30 dB re the harmonics above 16 kHz and between harmonics: read their
    noise figures as an upper bound;
  - **Iowa** is anechoic. Every other source is a room at some distance, which adds
    reverberant energy between the harmonics and in the LTAS.

## Feature definitions (per note)

- **`f0_hz`, `midi`:** the median YIN f0 over the steady window, refined by phase-locking the
  strongest low harmonic.
- **Steady window:** from max(0.25 s, 20 %) after the onset to 15 % before the end, at most
  1.6 s.
- **`h_db_re_f1` / `h_db_re_max`:** partials 1–40, dB re the fundamental and re the strongest.
  Each is the complex amplitude of exp(j2πn·φ(t)), where φ is the tracked phase, so vibrato is
  followed, under a 6-period Blackman-Harris window, hop one period. The value is the median
  over the window. `h_db_all_re_max` takes every harmonic up to 16 kHz.
- **`centroid_hz`, `centroid_harm`:** the power-weighted centroid of the harmonics up to 10 kHz,
  in Hz and in harmonic numbers.
- **`ltas_db`:** the long-term average spectrum over the whole note (onset to end) in 1/6-octave
  bands, 25 Hz–16 kHz, dB re the note's total.
- **`noise.*` (the bow noise):**
  - the **residual** of the harmonic model (SMS: x − Σ cₙ(t) e^{j2πnφ}) in 1/3-octave bands;
  - each band corrected for the noise the projection absorbs (0.54·f0 per harmonic, measured
    on white noise; +3.4 dB where harmonics are dense);
  - `hnr_band_db` is harmonic ÷ residual per band;
  - `res_valid_db_re_harm` is the residual in the **valid bands** (≥ 300 Hz, ≥ 5·f0, ≤ 0.42·fs)
    re the total harmonic power.

  Below 5·f0 the harmonics' own amplitude modulation (vibrato) leaks into the residual. When a
  `pre` background exists (Iowa's silences, a file's lead-in), `res_over_floor_db` says how far
  the residual stands above the room and recorder floor; under 6 dB the band is floor, not bow.
- **`attack.*`:**
  - **onset:** the first 2 ms frame above max(peak − 40 dB, floor + 6 dB);
  - `t_rise_3db_s`: from the onset to within 3 dB of the steady level;
  - `t_steady_s`: when the harmonic shape (partials 1–12 within 30 dB of the strongest,
    mean level removed) stays within 3 dB rms of the steady shape for 100 ms, with the level
    within 6 dB. 4-period Hann window, fixed f0;
  - `build_up_s`: when each of partials 1–8 reaches its steady − 6 dB;
  - `build_order_rho`: Spearman's ρ of those times against n. Positive means the high partials
    arrive later; negative means earlier;
  - `noise_burst_db_over_steady`: the residual ÷ harmonic ratio in the first 50 ms, minus the
    steady one.
- **`release.*`:** the level's decay rate (dB/s) from steady − 6 to steady − 40 dB (or floor + 8)
  after the steady window. `truncated` marks a file that ends or fades before the fit's range;
  those are excluded.
- **`vibrato.*`:**
  - `rate_hz`: the peak of the f0 track's spectrum in 3–9 Hz;
  - `extent_cents`: the ±cents of the sinusoid at that rate, corrected for YIN's window;
  - the slow drift is removed by a moving average two vibrato periods long, whose response is
    zero at the rate;
  - `present` when the extent exceeds 4 cents and the peak stands 6× over the track's median
    spectrum;
  - `am_db`: each partial's (1–20) amplitude modulation at the rate, ±dB;
  - `am_phase_rad`: its phase against the FM. Partials swinging in opposite phase are the
    body's signature (Fletcher & Sanders 1967).
- **`partial_dev_cents`:** each partial's frequency against n·f0, from its phase drift. A bowed
  string in Helmholtz motion is **mode-locked**, so this is ≈ 0 even though the free string is
  inharmonic.
- **Pizzicato:**
  - `B`: the inharmonicity, from fₙ² = a·n² + b·n⁴ fitted to the long-FFT peaks 30–630 ms after
    the pluck;
  - `t60_s`: each partial's T60, from a linear fit of its dB track (8-period window, 10 ms
    hop) between its peak + 20 ms and floor + 10 dB;
  - `t60_early_s`: the first 0.3 s of that fit;
  - `thump_excess_db`: the 1/3-octave power of the first 25 ms over that of 60–160 ms;
  - `h_peak_db_re_max`: each partial's peak level.
- **Phrases (`phrases.jsonl`):**
  - a stable note is ≥ 80 ms within ±50 cents of its median;
  - a transition is the time between two stable notes of different pitch: `trans_ms`, plus the
    level dip `dip_db` and YIN's aperiodicity peak `ap_max`;
  - re-bows are level dips of ≥ 4 dB and ≥ 10 ms inside a stable note.

## The extractor's measured accuracy (`validation.json`, synthetic notes)

The synthetic bowed notes have known values throughout:

- f0 at 41, 98, 196, 440 and 880 Hz;
- a 1/n spectrum with a ±6 dB ripple;
- vibrato of 5.8 Hz, ±18 cents, with a known AM of 1–4 dB per partial;
- coloured noise at −30 dB;
- harmonics that arrive 4 ms apart;
- a 40 dB/s release.

The synthetic plucks have B = 4e-5 to 1.2e-4 and T60 of 0.6–2.5 s per partial.

| Feature | Error measured |
|---|---|
| f0 | ≤ 0.6 cent |
| harmonics 2–20 | median 0.05–0.2 dB, max 0.41 dB |
| centroid | ≤ 0.9 % (−0.9 % at 41 Hz) |
| vibrato rate | ±0.03 Hz |
| vibrato extent | ±0.1 cent (98–880 Hz); +1.1 cent at 41 Hz |
| per-partial AM | median 0.01–0.14 dB (98–880 Hz); 0.52 dB at 41 Hz |
| noise in the valid bands | +0.4 … +1.1 dB (98–880 Hz); **+3.9 dB at 41 Hz with vibrato**, where the highest harmonics leak |
| per-band HNR | median 0.4–0.6 dB, max 1.7–4.7 dB (98–880 Hz); median 2.8 dB at 41 Hz |
| rise time | +3 … +7 ms (the 10 ms RMS window) |
| harmonic build-up order | ρ 0.97–0.99 against the true +1 |
| release decay | −0.2 … −0.4 dB/s of 40 |
| mode-locking deviation | ≤ 0.06 cent (0.23 at 880 Hz) |
| pizzicato B | < 0.01 % |
| pizzicato T60 | median 0.3–0.4 %, max 3–12 % (the weakest partials) |

**The attack's noise burst** reads +5 … +9 dB on a synthetic attack that has none. Read it as
"+5–9 dB is nothing"; compare instruments and the model by the same measure.
