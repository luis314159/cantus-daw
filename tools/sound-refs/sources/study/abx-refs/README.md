# ABX listening references: cello, double bass, viola, violin

Made by `tools/arcus/study/make_abx.py` on 2026-11-24. `index.json` lists every
file with its source file, licence, use, measured loudness and peak.

- **Format:** mono, 24-bit, 48 kHz (resampled from 44.1 kHz), a 10 ms fade at each end.
- **Loudness:** every file is at **−23 LUFS integrated** (ITU-R BS.1770-4, checked against the
  standard's 997 Hz sine: −3.01 LUFS at full scale). No file needed lowering; the peaks are
  −5 to −15 dBFS.
- **The dynamics are matched in loudness**, so pp, mf and ff differ only in timbre. Compare
  Arcus's pp, rendered at −23 LUFS, with the recording's pp. How much louder ff is than pp in
  each source is in `$CANTUS_STUDY_REFDB` (`$CANTUS_REFS\derived\refdb`) (`level_db_steady` per note).

Per family (`<family>\`):

| File | What | Licence | Use |
|---|---|---|---|
| `*_tinysol_<note>_{pp,mf,ff}.wav` | IRCAM's instrument, ordinario, second string: cello A3, bass D2, viola D4, violin A4 | CC BY 4.0 | may ship with credit |
| `cello_karoryfer_A3_{p,mf,f}.wav` | Karoryfer x bigcat cello, down bow, no vibrato (the file `A2_*_d.wav` sounds A3) | CC0 | may ship |
| `bass_smolken_D2_{pp,mf,ff}.wav` | Smolken's Rubner bass, down bow, no vibrato | CC0 | may ship |
| `violin_vsco_A4_{p,f}.wav` | VSCO 2 CE solo violin, with vibrato | CC0 | may ship |
| `*_philharmonia_phrase_legato.wav` | Philharmonia legato phrase, mf (cello D3, bass E2, viola D4, violin A4) | not to be redistributed as samples | listening and measurement only (decoded MP3) |
| `*_hsd_phrase_k465-excerpt.wav` | 10 s of Mozart K465, each instrument recorded alone, close-miked (MKH 800): cello phrase 1, the viola's and first violin's long excerpt. **The only freely licensed solo viola found** | CC BY 4.0 (Hochschule Düsseldorf, Zenodo 3601097) | may ship with credit |
| `*_phenicx_phrase_anechoic-excerpt.wav` | the densest 8 s of the Mozart part (*Don Giovanni* aria), anechoic, a real performance with vibrato | academic research use (Aalto) | listening and measurement only |

Each file states which instrument it is. Listen to Arcus against **each** instrument, not only
one: two real cellos differ from each other by about as much as the model may (`refdb\`, the body
and harmonic spreads).
