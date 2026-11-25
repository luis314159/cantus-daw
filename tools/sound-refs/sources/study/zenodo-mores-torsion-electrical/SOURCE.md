# Mores 2018: transverse and torsional vibration on a bowed cello G steel string (electrical pickups)

- **URL:** https://doi.org/10.5281/zenodo.1257872 (https://zenodo.org/records/1257872).
  A copy is at HAW Hamburg, doi:10.48441/4427.266.
- **Supports:** R. Mores, "Further empirical data for torsion on bowed strings", PLOS ONE
  14(2): e0211217, 2019, doi:10.1371/journal.pone.0211217. Its Data Availability statement
  names this record. The article has no separate supporting-information files; its data are
  these three Zenodo records (see also `../zenodo-mores-torsion-optical/` and
  `../zenodo-mores-torsion-letter/`).
- **Access date:** 2026-11-23

## Licence

The Zenodo licence field is `cc-by-4.0` (Creative Commons Attribution 4.0 International).
DataCite lists the HAW copy as cc-by-4.0 too. The PLOS article itself is "distributed under the
terms of the Creative Commons Attribution License".

## Content, from the record's description (verbatim excerpts)

> Transverse and torsional vibrations from a cello G string made of steel, product name "Pirastro Chromcor medium".
> Transverse signal taken electrodynamically below bow contact point,while string is mounted on a monochord and tuned to 98 Hz.
> The number in the filename indicates the distance of the bow contact point from the bridge in cm, while the string has a free length of 68 cm [...] Upstrokes/downstrokes are indicated by *_u.mat and *_d.mat
> Manual bowing by experienced player [...] always exciting regular Helmholtz motion on the string. Only the file session_*_doppelzahn represents data for a non-Helmholtz case.
> For all *.mat files: column 1 of table Y is the transverse time signal (0.1 per mm), column 2 the torsional time signal (0.004 per 1°). Sampling frequency is 48 kHz.

- **`torsion_zum_steg`:** the torsion pickup is 10 mm from the bow on the bridge side.
- **`torsion_zum_sattel`:** the torsion pickup is 10 mm from the bow on the nut side.
- **Bow positions:** 4 to 16 cm from the bridge, up-bow and down-bow.
- **Start markers:** `startpunkte_2017112{0,1}.m` give the analysis-window start times in
  seconds. They are text in German; do not execute them.
- **String properties** (PLOS Table 1 and the optical record): 6.15 g/m, 1.19 mm, nominal
  tension 121 N, transverse wave speed 133 m/s, Z0 0.93 kg/s, torsional fundamental 543 Hz,
  torsional wave speed 738 m/s, vibrating length 680 mm.

## Files

55 files in `data/`, 83 MB: 53 MATLAB v5 `.mat` files and 2 `.m` text files. Every MD5 matches
Zenodo.

## Verdict

**Measured data, CC BY 4.0: it may be used and redistributed with attribution** (R. Mores, HAW
Hamburg). Analysis is allowed. It is not audio for shipping.
