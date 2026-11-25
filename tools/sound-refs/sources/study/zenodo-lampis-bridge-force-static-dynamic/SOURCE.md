# Lampis et al. 2024: static and dynamic bridge-force components of a bowed cello string (processed part)

- **URL:** https://doi.org/10.5281/zenodo.10619541, file `Database-Examination_of_bridge_force.zip`
  (1 310 765 535 bytes)
- **Access date:** 2026-11-23
- **Licence:** the Zenodo licence field is `cc-by-4.0`.
- **Taken:** `DATA/processed/*.wav` (168 files), `DATA/meta/meta.csv`, `features.csv`,
  `helmholtz_n_ref.csv`, `A.mat`, the WIKI PDF describing the data, and the MATLAB scripts
  `main.m`, `preproc_filtering.m` and `setPlot.m` (not executed).
- **Not taken:** `DATA/raw/*.wav` (168 files, about 2.7 GB) and the photographs.

## Content, from the record and `WIKI/Examination of the static and dynamic bridge force.pdf`

> We measured and analysed the static and dynamic components of the bridge force in the process of bowing a cello string using a robotic arm.
> Conversion of the signals from (V) to (N) is done by multiplying the raw and processed data by a factor of 10.846.
> In the processed files, there are 4 channels, respectively: channel 1: vertical bridge force (perpendicular to bowing direction) channel 2: horizontal bridge force (parallel to bowing direction) channel 3: bow force channel 4: transverse bow force

- **`meta.csv`:** `beta` (bow-bridge ratio), `Z` (bow elevation index, 1 for the lowest force
  to 7 for the highest), `v` (bow speed in m/s) and `n_ref` (the WAV number).
- **`features.csv`:** `beta`, `vb` (m/s), `Fb` (N), `Ft` (transverse force, N), `pp` (the
  peak-to-peak dynamic horizontal force, N), `f0` (Hz), `SC` (spectral centroid, Hz) and
  `n_ref`. Only Helmholtz cases are included.
- **`helmholtz_n_ref.csv`:** the recordings that reached Helmholtz motion.
- **WAV format:** 4 channels, 16-bit, 125 kHz.

**Files:** 176 files, 27 MB.

## Verdict

**Measured data, CC BY 4.0: usable with attribution.** Analysis is allowed. These are force
signals, not instrument audio.
