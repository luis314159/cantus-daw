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
<venv>\Scripts\python.exe -m pip install -r tools/sound-refs/requirements.txt
python -I tools/sound-refs/fetch.py fetch arcus strings-rec study-*   # the recordings, ~7 GB
```

The recordings never enter the repository: they are the sets of `tools/sound-refs/catalog.toml`
under `CANTUS_REFS` (default `F:\projects\cantus-refs`; `CANTUS_STUDY_DOWNLOADS` overrides).
`catalog.py` lists the sources it expects there; each source's `SOURCE.md` gives its licence
(the repository keeps a copy in `tools/sound-refs/sources/`).

The scripts write into `CANTUS_STUDY_REFDB` when it is set, else into
`research/instruments-next/bowed-refdb/` (the committed distributions). `bash
tools/sound-refs/rebuild_refdb.sh` runs the whole chain below into `$CANTUS_REFS/derived/refdb`
and compares the result with the committed files.

## Running

Run from the repository's root with `<venv>\Scripts\python.exe -I tools/arcus/study/<script>`.

1. `validate_synth.py`: the extractor on synthetic notes with known values. Its output must
   equal the stored `validation.json`.
2. `catalog.py` → `extract.py --workers 3` (resumable) → `phrases.py` → `aggregate.py` →
   `plots.py`.
3. The extra studies: `cnsm.py` (body against measured bridge admittances), `friction.py`
   (rosin friction against the hyperbolic curve), `bowing.py` (centroid against force, speed
   and β).
4. `make_abx.py`: the blind-listening set. It writes under `CANTUS_REFS/study/abx-refs`, never
   into the repository.

The regenerable outputs stay out of git, as the refdb's `.gitignore` lists: the per-note
files, the catalogue, the phrases, the plots and the logs.
