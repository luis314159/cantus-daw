# arcus-cello — how this subject was built (round 42: arcus, then strings-refs)

Arcus's cello against recorded cellos by the lab's held-out band method (ADR 0160,
`tools/soundlab/README.md`).

## References (`references.json`, 698 files)

| instrument (each weighs one) | source | licence | what | format, room |
|---|---|---|---|---|
| `iowa2012-cello` | Iowa MIS 2012: Charles Quenoil cello (1923), Yoo-Jung Chang | Iowa-MIS | runs arco + pizz, pp / mf / ff (48 files) — **strings-refs** | 24-bit 96 kHz, anechoic, Earthworks QTC40 at 5 ft |
| `goodsounds-cello-margarita` | Good-sounds (MTG-UPF) | CC BY 4.0 | single notes; **strings-refs** added her open strings at p / mf / f (bow at the middle of the string: 72 files) | 24-bit 48 kHz, studio, Neumann |
| `goodsounds-cello-nico` | Good-sounds | CC BY 4.0 | single notes | the same |
| `iowa-cello` | Iowa MIS, pre-2012 | Iowa-MIS | runs arco + pizz, pp / mf / ff | 16-bit 44.1 kHz, anechoic |
| `ircam-cello` | TinySOL (IRCAM) | CC BY 4.0 | single notes, pp / mf / ff | 16-bit 44.1 kHz |
| `karoryfer-bigcat-cello` | Karoryfer Bigcat cello | CC0 | sustains p / mp / mf / f, pizz | 24-bit 44.1 kHz |

Six cellos (five before). Made by:

```bash
python -I tools/soundlab/adapters/study_catalogue.py $CANTUS_STUDY_REFDB/catalog.json cello $CANTUS_REFS base.json
python -I tools/soundlab/adapters/strings_hq.py cello $CANTUS_REFS base.json tools/soundlab/subjects/arcus-cello/references.json
```

## The candidate

`arcus_match render <out> cello`: the held-out cells, keys C2–G5, arco and pizzicato at
pp / mf / ff — 88 notes.

## Phrases (`phrases.json`)

The Musopen K465 session's cello spot mic (public domain, 96/24; windows where the cello
dominates its mic, the others ~10–13 dB under it) and HSD's K465 cello stems (CC BY 4.0):
`python -I tools/soundlab/adapters/strings_hq_phrases.py pick|measure …`.
