# PHENICX-Anechoic: denoised cello and double-bass tracks with note annotations

- **URL:** https://zenodo.org/records/840025 (doi:10.5281/zenodo.840025), file `PHENICX-Anechoic.zip`
  (763 716 294 bytes). An older version is https://zenodo.org/records/1289821, marked
  cc-by-nc-sa-4.0.
- **How they were taken:** only `Readme.txt`, `audio/<piece>/cello*.wav`,
  `audio/<piece>/doublebass.wav` and `annotations/<piece>/{cello,doublebass}*.{mid,txt}`, by
  HTTP range requests (CRC-32 checked).
- **Access date:** 2026-11-23

## Licence, verbatim

The Zenodo record's licence field is "cc-by-4.0". The record text and the included
`Readme.txt` say:

> For the intellectual rights and the distribution policy of the denoised audio recordings contact Aalto University, Jukka Pätynen and Tapio Lokki.
>
> The annotations and the Roomsim configuration files in the PHENICX-Anechoic dataset are offered free of charge for non-commercial use only. You can not redistribute them nor modify them.
>
> This work is licensed under a Creative Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License.

## Content

- **Instruments and players:** the same Aalto anechoic performances as
  `../aalto-anechoic-orchestra/`, denoised by the MTG (UPF). Players are not named.
- **Format:** WAV, mono, 16-bit, 44.1 kHz. The denoising is score-informed, so it is not raw
  audio.
- **Files:** 9 WAV files (115 MB in total), 32 annotation files and `Readme.txt`.
  - Beethoven: cello 191 s, double bass 189 s
  - Bruckner: cello1 and cello2, double bass, 87 s each
  - Mahler: cello and double bass, 132 s
  - Mozart: cello and double bass, 227 s
- **Annotations:** `<instr>.txt` and `<instr>_o.txt` are CSV files of `onset_s,offset_s,note`
  (for example `3.988753,4.186757,D#3`), with MIDI copies.

## Verdict

**Measurement only; never ship.** The audio rights belong to Aalto ("academic research"), and
the readme puts the annotations under NC-SA 3.0 and non-commercial terms, which contradicts the
Zenodo field. Analysis for research is allowed. Do not redistribute or modify the annotations.

## Addendum, 2026-11-24: violin and viola

To keep within the disk budget, only one violin and one viola part per piece were taken:

| Piece | Violin | Viola |
|---|---|---|
| Mozart | `violin1.wav` | `viola.wav` |
| Beethoven | `violin1.wav` | `viola1.wav` |
| Bruckner | `violin1.wav` | `viola1.wav` |
| Mahler | `violin1.wav` | `viola.wav` |

That is 8 WAV files (44.1 kHz, 16-bit, mono), plus the matching violin and viola annotation
files.

**Not taken:** Beethoven violin2-4 and viola2, Bruckner violin2-12 and viola2-3, Mahler
violin2-4, and Mozart violin2.

The folder now holds 17 WAV files, 223 MB. The terms are unchanged: **measurement only**.
