# arcus-violin — how this subject was built (round 42, strings-refs)

Arcus's violin against recorded violins by the lab's held-out band method (ADR 0160,
`tools/soundlab/README.md`), built like `arcus-cello`.

## References (`references.json`, 642 files)

| instrument (each weighs one) | source | licence | what | format, room |
|---|---|---|---|---|
| `iowa2012-violin` | Iowa MIS 2012: Tambovsky violin (1998), Leonid Iogansen | Iowa-MIS | runs arco + pizz, pp / mf / ff (72 files) | 16-bit 96 kHz (11 at 48 kHz), anechoic, Earthworks QTC40 at 5 ft |
| `iowa-violin` | Iowa MIS, pre-2012 | Iowa-MIS | runs arco + pizz, pp / mf / ff | 16-bit 44.1 kHz, anechoic, KM 84 at 5 ft |
| `ircam-violin` | TinySOL (IRCAM) | CC BY 4.0 | single notes, ordinario, pp / mf / ff | 16-bit 44.1 kHz |
| `goodsounds-violin-laia`, `-raquel` | Good-sounds (MTG-UPF) | CC BY 4.0 | single notes ("mf") | 24-bit 48 kHz, studio, Neumann |
| `vsco-solo-violin` | VSCO 2 CE | CC0 | sustains p / f, pizz | 44.1 kHz |

Made by (the study's free catalogue, then the strings-hq set — `tools/sound-refs/catalog.toml`):

```bash
python -I tools/soundlab/adapters/study_catalogue.py $CANTUS_STUDY_REFDB/catalog.json violin $CANTUS_REFS base.json
python -I tools/soundlab/adapters/strings_hq.py violin $CANTUS_REFS base.json tools/soundlab/subjects/arcus-violin/references.json
```

Left out: Philharmonia (its own terms, MP3), PHENICX (non-commercial), VSCO's sections.

## The candidate

`arcus_match render <out> violin`: the held-out cells ((key + dynamic) % 3 == 0) of every key
G3–E7, arco and pizzicato at pp / mf / ff — 92 notes. (The r42 analysis had only every fourth
key, arco, and the pluck at mf only: one Arcus note per group.)

## Phrases (`phrases.json`, the phrase measures)

Real music, not in the band report: the Musopen K465 session's 1st- and 2nd-violin spot mics
(public domain, 96/24; the windows where each dominates its mic — the others bleed ~10–13 dB
under it) and HSD's K465 violin stems (CC BY 4.0, recorded one instrument at a time):
`python -I tools/soundlab/adapters/strings_hq_phrases.py pick|measure …`.

```bash
SOUNDLAB_REFS=$CANTUS_REFS python -I tools/soundlab/lab.py all tools/soundlab/subjects/arcus-violin/subject.json <run>
```
