# Lampis 2025: friction force reconstructed at the bowing point for cello strings

- **URL:** https://doi.org/10.5281/zenodo.15879128 (https://zenodo.org/records/15879128)
- **Access date:** 2026-11-23
- **Licence:** the Zenodo licence field is `cc-by-4.0`. The record has no description text.
- **Files downloaded:** `output_measurements.zip` (unzipped into `output_measurements/` and the
  zip then deleted; its MD5 matched Zenodo), `function.zip` and
  `main_measurements_reconstruction.m`. The last two are MATLAB source, **not executed**, kept
  as documentation.

## Content

`output_measurements/output_measurements/` holds:

- **`reconstructions/friction_velocity_reconstruction_<n>.csv`** (109 files): columns `Fr_rec`,
  `V_rec`, `F_b` and `F_n`. These are the reconstructed friction force at the bow (N), the
  string velocity at the bow (m/s), the bridge force (N) and the nut force (N). The units are
  inferred from the method, which follows Woodhouse in reconstructing the friction force from
  the bridge and nut signals. About 16 000 samples each.
- **`reconstructions/bowing_parameters_<n>_<k>.csv`** (108 files): `beta`, `Fb` and `v0`, that
  is bow-bridge ratio, bow force in N and bow speed in m/s.
- **`bowed_best_params/*.mat` and `pluck_best_params/*.mat`:** the fitted string and reflection
  parameters from plucks and bowed steady states.
- **`string_properties/`:** a MATLAB file holding the string property estimates.

The strings are D'Addario Helicore and Thomastik Dominant cello G strings on a robotic monochord
at mdw Vienna. The script header lists the β values used, from 0.020 to 0.200.

**Files:** 352 data files (217 CSV, 134 MAT, 1 M), plus `function.zip` and the main `.m`.
103 MB.

## Verdict

**Measured data, CC BY 4.0: usable with attribution** (A. Lampis, mdw Vienna). Analysis is
allowed. It is not audio.
