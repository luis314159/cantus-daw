# The bowed-strings reference study (round 42)

These scripts measure recordings of real cellos, double basses, violas and violins. They
produce the bands that Arcus is judged against, and they measure Arcus the same way.
Every gate uses the **same estimator** for the model and the recordings, so the estimator's
own biases cancel.

- **The data:** `research/instruments-next/bowed-refdb/`. Its README defines every feature
  and explains how a band is taken.
- **The findings:** `research/instruments-next/bowed-realism.md` (the eight ranked
  recommendations).
- **The users:** `tools/arcus/study_compare.py` and `upper_study.py` read the bands.

## Setting up

```
python -m venv <venv>
<venv>\Scripts\python.exe -m pip install numpy==2.3.3 scipy==1.16.2 matplotlib==3.10.6 soundfile==0.13.1
set CANTUS_STUDY_DOWNLOADS=<the folder holding the downloaded recordings>
```

The recordings never enter the repository. `catalog.py` lists the sources it expects under
`CANTUS_STUDY_DOWNLOADS`. Each source's own `SOURCE.md` there gives its licence.

## Running

Run from the repository's root with `<venv>\Scripts\python.exe -I tools/arcus/study/<script>`.

1. `validate_synth.py`: the extractor on synthetic notes with known values. Its output must
   equal the stored `validation.json`.
2. `catalog.py` → `extract.py --workers 3` (resumable) → `phrases.py` → `aggregate.py` →
   `plots.py`.
3. The extra studies: `cnsm.py` (body against measured bridge admittances), `friction.py`
   (rosin friction against the hyperbolic curve), `bowing.py` (centroid against force, speed
   and β).
4. `make_abx.py`: the blind-listening set. It writes under `CANTUS_STUDY_DOWNLOADS`, never
   into the repository.

The regenerable outputs stay out of git, as the refdb's `.gitignore` lists: the per-note
files, the catalogue, the phrases, the plots and the logs.
