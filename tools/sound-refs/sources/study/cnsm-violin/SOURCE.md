# CNSM violin dataset (Pauget Ballesteros, Lalitte, Lostanlen, Fritz): admittances and some recordings

- **URL:** https://doi.org/10.5281/zenodo.18696785. The concept DOI resolves to record 18696786,
  "CNSM Dataset" v1.0.0, 2026-02-19, file `dataset.zip` (654 025 212 bytes).
- **Paper:** arXiv 2605.18305, "Violin 'Playing-In': Disentangling Physical Change from Player
  Adaptation via Physical Measurements".
- **Code:** https://github.com/hugo-paugesteros/CNSM-dataset (not fetched).
- **Access date:** 2026-11-24
- **Licence:** the Zenodo licence field is `cc-by-4.0` (Creative Commons Attribution 4.0
  International). The paper's Data Availability statement says, verbatim:

  > The violin acoustics dataset generated during this study, including bridge admittance, motion capture (MoCap) trajectories, and audio recordings, is available in the Zenodo repository

- **What was taken** (range-extracted with CRC-32 checked):
  - all of `admittances/` (21 measurements plus `dataset.csv`, 50 MB);
  - `recordings/dataset.csv` and `mocap/dataset.csv`;
  - `recordings/Session 1/**.wav` (97 files, 168 MB).
- **Not taken:** recording sessions 2 and 3 (139 files, about 316 MB) and the motion-capture
  trajectories (`mocap/Phase …`, 377 files, about 1.3 GB).

## Violins (paper §2)

- **Klimke (2012):** the test violin, played daily for six months.
- **Levaggi (2012):** a stored control, of similar construction and varnish.
- **Stoppani (2018):** a stored control.

Phase 1 is "Before" and Phase 2 is "After".

## Admittance files: `admittances/Phase <1|2>/<violin>/measurement_<n>.csv`

`admittances/dataset.csv` indexes the files with the columns `violin,phase,measurement,filepath`.
Phase 1 has 4 measurements per violin. Phase 2 has 5 for Klimke and 2 each for Levaggi and
Stoppani.

- **Header:** `f,H_real,H_imag,Force_real,Force_imag,Velocity_real,Velocity_imag`, comma
  separated, one row per frequency bin.
  - `f`: frequency in Hz.
  - `H`: the complex input admittance, velocity over force (H1 estimate).
  - `Force`, `Velocity`: the complex spectra of hammer force and bridge velocity.
- **Units:** the files and the paper give none explicitly. H is presumably in (m/s)/N, with
  force in N and velocity in m/s from the vibrometer, but this has **not been verified**. The
  spectrum scaling is unknown.
- **Frequency grid:** 0 to 25 600 Hz.
  - Most files: Δf = 1.5625 Hz, 16 385 rows.
  - Two files (`Phase 1/Levaggi/measurement_1.csv` and `Phase 1/Klimke/measurement_4.csv`):
    Δf = 0.390625 Hz, 65 537 rows.
  - That implies a 51.2 kHz sampling rate with 0.64 s or 2.56 s blocks.
- **Method (paper §2.4.1/2.5.1):**
  - Excitation: impact hammer PCB 086E80 on the **G-string side** of the bridge.
  - Response: velocity measured by a laser Doppler vibrometer (Polytec OFV-505/OFV-3001) on the
    **E-string side** of the bridge.
  - Averaging: H1 estimator over 5 impacts. The paper says each phase was measured 3 times and
    reports the RMS across those blocks.
  - Mounting: the violin lay on foam with the strings damped.

## Recordings: `recordings/Session 1/<violin>/Player <id>/scale_<take>.wav`

- **Content:** each player plays a scale on each violin. Session 1 has 97 files, with blind and
  non-blind conditions; `recordings/dataset.csv` (236 rows for all sessions) lists the
  conditions, durations and takes.
- **Players:** 13 across the whole dataset.
- **Format:** WAV, mono, 16-bit, 48 kHz.
- **Recording setup:** a large, dry room of about 100 m², DPA 4010 microphones above the player,
  and an RME Fireface with fixed gain.

## Verdict

**CC BY 4.0: it may ship with attribution, and analysis is allowed.** These are three named,
independent violins, with measured bridge admittance for each.
