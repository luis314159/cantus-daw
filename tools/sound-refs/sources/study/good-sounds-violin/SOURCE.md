# Good-sounds dataset (MTG-UPF): violin part (Neumann microphone)

- **URL:** https://zenodo.org/records/820937 (doi:10.5281/zenodo.820937), `good-sounds.zip`
  (13 906 331 564 bytes). The cello part, with the CORA copy (doi:10.34810/DATA2314, truncated)
  and the metadata JSON files, is in `../good-sounds/`.
- **How it was taken:** by range requests with CRC-32 checked.
  - **Taken:** `sound_files/violin_raquel_*/neumann/*.wav` and
    `sound_files/violin_laia_improvement_recordings/neumann/*.wav`: 799 WAV files, 548 MB. These
    violin packs have no `annotations.csv`.
  - **Not taken:** `violin_violin_scales_laia_recordings` (253 scale files, 600 MB), to save disk,
    and the akg, sony and iphone takes.
- **Access date:** 2026-11-24
- **Licence:** the Zenodo licence field is `cc-by-4.0`. The CORA readme says, verbatim:

  > Creative Commons Attribution 4.0 International (CC BY 4.0)

## Content

- **Players:** two violinists, "raquel" (469 sounds) and "laia" (330), recorded at the UPF studio.
  The instruments are not named. They are independent of the other violins here.
- **Format:** WAV, mono, 24-bit, 48 kHz.
- **Packs:**

  | Pack | Files | Content |
  |---|---|---|
  | `raquel_dynamics_stability` | 144 | crescendo, decrescendo, tremolo |
  | `raquel_richness` | 74 | includes near the bridge and sul tasto |
  | `raquel_attack` | 74 | includes pressure and rebound faults |
  | `raquel_pitch_stability` | 72 | includes vibrato |
  | `raquel_reference` | 68 | |
  | `raquel_timbre_stability` | 37 | |
  | `laia_improvement_recordings` | 330 | |

- **Range:** single sustained notes, 37 pitches from G3 to G6.
- **Dynamics:** mf is labelled on 68 sounds; `<`, `>` and `~` on 110.
- **Labels:** quality tags such as `good-sound` (141) and the `bad-*` faults, with manual
  attack, sustain and release marks in samples. They are in `../good-sounds/meta/sounds.json` and
  `../good-sounds/zenodo/good-sounds/database.sqlite`.
- **Not included:** phrases and pizzicato.

## Verdict

**May ship (CC BY 4.0), with attribution.** Analysis is allowed.
