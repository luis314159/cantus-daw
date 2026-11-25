# Good-sounds dataset (MTG-UPF): cello part

- **URLs:**
  - CORA copy, the one asked for: https://doi.org/10.34810/DATA2314 (dataverse.csuc.cat, file id 280585, `good-sounds.zip`, 2 168 063 959 bytes).
  - Zenodo original, used for the audio: https://zenodo.org/records/820937 (doi:10.5281/zenodo.820937), `good-sounds.zip`, 13 906 331 564 bytes, licence field "cc-by-4.0".
- **Why the audio came from Zenodo:** the CORA zip is **truncated**. It has no central
  directory or end-of-central-directory record, and it ends in the middle of the clarinet files,
  at about 2.17 GB of the original 13.9 GB. Its cello entries could not be located. The CORA
  metadata files (`meta/`: readme, `packs.json`, `sounds.json`, `takes.json`, `ratings.json`) were
  downloaded whole. The audio was range-extracted from the Zenodo zip (CRC-32 checked).
- **What was taken:** every `sound_files/cello_*/neumann/*.wav` (one microphone, the Neumann; the
  akg, sony and iphone takes were left out), the `annotations.csv` files, `README.txt` and
  `database.sqlite`.
- **Access date:** 2026-11-23

## Licence, verbatim (CORA readme, `meta/Good-sounds-dataset-readme.txt`)

> 1. Creative Commons License of the dataset:
> Creative Commons Attribution 4.0 International (CC BY 4.0)

The CORA licence field is "CC BY 4.0" (http://creativecommons.org/licenses/by/4.0). The Zenodo
licence field is "cc-by-4.0".

## Content

- **Instrument and players:** two cellists, "margarita" (572 sounds) and "nico" (376 sounds),
  recorded at the UPF studio in Barcelona. These are independent of the instruments already in
  hand. The cello make is not stated.
- **Format:** WAV, mono, 24-bit, 48 kHz.
- **Files:** 948 WAV files, 752 MB.
- **Packs:**

  | Pack | Files | Content |
  |---|---|---|
  | `open_strings` | 222 | strings 1-4, with bow velocity slow/medium/fast and position tasto/middle/ponticello for part of them |
  | `dynamics_stability` | 128 | crescendo, decrescendo, tremolo and errors |
  | `timbre_richness` | 64 | includes ponticello and sul tasto |
  | `pitch_stability` | 62 | includes vibrato |
  | `reference` | 32 | |
  | `timbre_stability` | 32 | |
  | `attack` | 32 | good attacks and pressure faults |
  | `nico_improvement_recordings` | 376 | |

- **Dynamics:** p, mf and f are labelled on 254 sounds, with `<`, `>` and `~` for 96 more. Notes
  run from C2 to A4.
- **Labels:** `good-sound`, `bad-*` and the playing-fault tags are in `database.sqlite` and
  `meta/sounds.json`. They include manual attack, decay, sustain, release and offset marks in
  samples.
- **Not included:** legato phrases. The dataset has single notes and scales only. Vibrato
  appears as a labelled pitch-stability fault.

## Verdict

**May ship (CC BY 4.0), with attribution:** Romaní Picas, Parra Rodríguez, Dabiri, Serra,
MTG-UPF, plus the dataset DOI. **Analysis is allowed.**
