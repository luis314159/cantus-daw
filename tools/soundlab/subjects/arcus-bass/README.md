# arcus-bass — how this subject was built (round 42, strings-refs)

Arcus's double bass against recorded basses by the lab's held-out band method (ADR 0160,
`tools/soundlab/README.md`), built like `arcus-cello`.

## References (`references.json`, 784 files)

| instrument (each weighs one) | source | licence | what | format, room |
|---|---|---|---|---|
| `iowa2012-bass` | Iowa MIS 2012: Anton Krutz bass (2004), Mike White | Iowa-MIS | runs arco + pizz, pp / mf / ff (78 files) | 24-bit 96 kHz, anechoic, Earthworks QTC40 at 5 ft |
| `goodsounds-bass-alejandro` | Good-sounds (MTG-UPF), player alejandro | CC BY 4.0 | 80 bowed notes ("mf"), 79 plucked (hard "f", soft "p") | 24-bit 48 kHz, studio, Neumann |
| `iowa-bass` | Iowa MIS, pre-2012 | Iowa-MIS | runs arco + pizz, pp / mf / ff | 16-bit 44.1 kHz, anechoic |
| `ircam-bass` | TinySOL (IRCAM) | CC BY 4.0 | single notes, ordinario, pp / mf / ff | 16-bit 44.1 kHz |
| `smolken-bass` | D. Smolken (Otto Rubner bass, tuned in fifths) | CC0 | arco pp–ff, pizz | 32-bit float 44.1 kHz |
| `vsco-solo-bass` | VSCO 2 CE solo contrabass | CC0 | sustains p / f, pizz | 44.1 kHz |

Six basses (four before). Made by:

```bash
python -I tools/soundlab/adapters/study_catalogue.py $CANTUS_STUDY_REFDB/catalog.json bass $CANTUS_REFS base.json
python -I tools/soundlab/adapters/strings_hq.py bass $CANTUS_REFS base.json tools/soundlab/subjects/arcus-bass/references.json
```

## The candidate

`arcus_match render <out> bass`: the held-out cells, keys E1–G4, arco and pizzicato at
pp / mf / ff — 80 notes (the same renders as the r42 analysis, to the byte).

## Phrases

None yet: no free solo-bass phrase recording of quality was found (Aalto's anechoic bass is
"free for academic research" and MP3; the Musopen sessions' bass mics are an orchestra section).

```bash
SOUNDLAB_REFS=$CANTUS_REFS python -I tools/soundlab/lab.py all tools/soundlab/subjects/arcus-bass/subject.json <run>
```
