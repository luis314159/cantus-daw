# Lampis 2024: bridge-force waveforms from systematic bowing of cello G-string models (subset)

- **URL:** https://doi.org/10.5281/zenodo.13374477, file `waveforms.zip` (3 007 564 144 bytes)
- **Paper:** JASA Express Letters, https://doi.org/10.1121/10.0034330 ("An experimental approach
  for comparing the influence of cello string type on bowed attack response").
- **Access date:** 2026-11-23
- **Licence:** the Zenodo licence field is `cc-by-4.0`.
- **Subset taken:** one Guettler-diagram session per string model, each from sample 2, the
  sample the paper analyses. Each was range-extracted with CRC-32 checked:
  - `model_A_2/2023-09-07`: 987 files
  - `model_B_2/2023-09-29`: 987 files
  - `model_C_2/2023-10-18`: 988 files
  - `model_D_2/2023-09-20`: 986 files

  The other 28 sessions (about 4.5 GB unpacked) were not taken.

## Content, from the record's description (verbatim excerpts)

> The recordings capture bridge force signals at different bow forces and bow accelerations. Various cello G2-strings (98 Hz) models were used
> The recording are single channel wav files sampled at 50kHz at a 64 bit rate. The bow force Fb and the bow acceleration a at which the bowing action was performed, is indicated in the file name.
> To obtain the bridge force in Newton, it is sufficient to multiply the signals by 10.
> Short recordings are around 0.22 seconds long, while longs last for about 0.9 seconds.

- **File names:** `Fb_<N>_a_<m/s²>.wav`.
- **Format:** WAV, IEEE float64, mono, 50 kHz.
- **Use:** these are attack transients, from which a Guettler diagram of the time to Helmholtz
  motion can be built for each string type.

**Files:** 3 948 WAV files, 540 MB.

## Verdict

**Measured data, CC BY 4.0: usable with attribution.** Analysis is allowed. These are
force signals, not instrument audio.
