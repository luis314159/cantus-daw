# arcus-viola — how this subject was built (round 42, strings-refs)

Arcus's viola against recorded violas by the lab's held-out band method (ADR 0160,
`tools/soundlab/README.md`), built like `arcus-cello`.

## References (`references.json`, 437 files)

| instrument (each weighs one) | source | licence | what | format, room |
|---|---|---|---|---|
| `iowa2012-viola` | Iowa MIS 2012: Anton Krutz viola (2001), Manuel Tabora | Iowa-MIS | runs arco + pizz, pp / mf / ff (66 files) | 24-bit 96 kHz, anechoic, Earthworks QTC40 at 5 ft |
| `iowa-viola` | Iowa MIS, pre-2012 | Iowa-MIS | runs arco + pizz, pp / mf / ff | 16-bit 44.1 kHz, anechoic, KM 84 at 5 ft |
| `ircam-viola` | TinySOL (IRCAM) | CC BY 4.0 | single notes, ordinario, pp / mf / ff | 16-bit 44.1 kHz |

Three violas now (two before). No other free solo-viola single notes were found that pass the
licence rule (TU-Note is CC BY-ND; VSCO has a section only; Philharmonia is MP3 under its own
terms). The phrases add two more violas (Musopen's and HSD's), for the phrase measures only.

Made by:

```bash
python -I tools/soundlab/adapters/study_catalogue.py $CANTUS_STUDY_REFDB/catalog.json viola $CANTUS_REFS base.json
python -I tools/soundlab/adapters/strings_hq.py viola $CANTUS_REFS base.json tools/soundlab/subjects/arcus-viola/references.json
```

## The candidate

`arcus_match render <out> viola`: the held-out cells of every key C3–A6, arco and pizzicato at
pp / mf / ff — 92 notes (the r42 analysis: every fourth key, the pluck at mf only).

## Phrases (`phrases.json`)

The Musopen K465 session's viola spot mic (public domain, 96/24; windows where the viola
dominates its mic, the others ~10–13 dB under it) and HSD's K465 viola stems (CC BY 4.0):
`python -I tools/soundlab/adapters/strings_hq_phrases.py pick|measure …`.

```bash
SOUNDLAB_REFS=$CANTUS_REFS python -I tools/soundlab/lab.py all tools/soundlab/subjects/arcus-viola/subject.json <run>
```
