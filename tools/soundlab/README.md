# tools/soundlab — the sound lab (round 42, ADR 0160)

One instrument-agnostic place to **measure** an instrument against recordings and to build its
**blind listening kit**, both from one subject file. Every instrument goes through it before it
is in the picker: the guitar family, the winds, the formant choir, struck bars and membranes
(`docs/PLAN-after-round-42.md`, track B) — and Arcus, Chorda, Tympanum when they change.

Python 3 with `requirements.txt` (numpy, scipy, soundfile; matplotlib is not needed: the report
draws its own SVG); every script runs with `python -I`. The tests run with one command:

```bash
python -m venv ~/venvs/soundlab && ~/venvs/soundlab/Scripts/pip install -r tools/soundlab/requirements.txt
export CANTUS_PY=~/venvs/soundlab/Scripts/python.exe        # bin/python on Linux
export SOUNDLAB_REFS=/f/projects/daw-r42/downloads           # the references: outside the repository
$CANTUS_PY -I tools/soundlab/tests/run.py                    # every test (--light: no downloads needed)
```

| variable | what | default |
|---|---|---|
| `SOUNDLAB_REFS` | the downloads folder the catalogues' paths are under | (required by the subjects) |
| `SOUNDLAB_WORK` | the lab's work: `<id>/refs/` (the references' cache), `<id>/runs/<run>/` | `<repo>/../soundlab-work` |
| `SOUNDLAB_LISTEN` | the listening renders a kit's voices may name | `<repo>/../listen` |
| `SOUNDLAB_WORKERS` | parallel measurers | 3 |
| `SOUNDLAB_STUDY`, `SOUNDLAB_EBU`, `SOUNDLAB_KIT_KEY` | optional test data: the Arcus study's records, the EBU loudness test set, a generated kit's key | none (those tests skip) |

## What is here

| | |
|---|---|
| `lab.py` | the one command: `check`, `refs`, `render`, `measure`, `report`, `all`, `kit` |
| `soundlab/features.py` | the estimators, one registry: loudness, pitch, harmonics, inharmonicity, centroid (and its trajectory), noise between harmonics, attack, t_steady, decay (per partial; the release), band decay (per octave), vibrato (and cycle by cycle), stereo, room — the table in its docstring |
| `soundlab/notes.py`, `core.py` | the Arcus study's note analysis and signal core (`tools/arcus/study/`), ported unchanged in method; `tests/test_agreement.py` holds the port to the study's own numbers |
| `soundlab/loudness.py` | ITU-R BS.1770-4 / EBU R128: integrated, momentary, short-term, LRA, true peak |
| `soundlab/band.py` | the held-out band method (below) |
| `soundlab/subject.py`, `measure.py`, `report.py` | the subject file, measuring (cached), the report (JSON + HTML) |
| `soundlab/kit.py` | the blind kit on the shared page `crates/cantus-ui/examples/abx_kit/abx.html` |
| `adapters/study_catalogue.py` | the Arcus study's catalogue as a lab catalogue (free sources only) |
| `subjects/arcus-cello/` | Arcus's cello: `subject.json`, `references.json` (578 files, CC0 / CC BY / Iowa MIS) |
| `subjects/example-tibia/` | **the worked example**: a hypothetical struck bar, synthetic references and candidate (`synth_bar.py`) — the shape a new instrument copies |
| `tests/` | `run.py` and the tests (below) |

## The protocol a new instrument follows

1. **References with licences.** Find recordings of the real instrument: CC0 or CC BY only
   (a source whose terms are as free, like the Iowa MIS's "used for any projects, without
   restrictions", only with its terms quoted in the subject's `also_allowed`; never NC, ND,
   SA or "listening only"). Download them **outside the repository** (`SOUNDLAB_REFS`,
   e.g. `F:\projects\daw-r42\downloads\<set>\`), with a `SOURCE.md` per set: the URL, the
   date, the licence verbatim, what was taken. Prefer several physical instruments over many
   notes of one: the band weighs each instrument as one.
2. **The subject file** (`subjects/<id>/subject.json`; copy `example-tibia`'s):
   - `kind` (`sustained` or `decaying`, per articulation), `f_range`, `lowest_note`;
   - `features` (per articulation; the registry's names) and `exclude` (what the recordings
     cannot be compared on — an absolute level, an axis);
   - `references`: the catalogue (`path` under the root, `source`, `licence` as SPDX, `credit`,
     `inst` the physical instrument, `art`, `note` in MIDI or `multi`/`auto`, `dyn`);
   - `split`: the seeded rule that holds a third of the cells (articulation, note, dynamic)
     out — `{"rule": "hash", "seed": N}`, or the instrument's own (`mod`);
   - `candidate`: the command that renders the instrument's notes (`{out}` the folder; a
     cargo example through `tools/build/slot.sh`), and the regex naming each file's cell.
   - `lab.py check <subject>` says what is wrong (a licence, a missing file, a field).
3. **The measured report.** `lab.py all <subject> <run> [--before <earlier run>]`: the
   references measured (cached), the candidate rendered and measured by **the same
   estimators**, the report at `<work>/<id>/runs/<run>/report.html` (+ `report.json`). A fit,
   by hand or by a tool, uses only the references of the cells **not** held out; the report
   judges the held-out third. Keep every run: `--before` puts one beside another.
4. **The owner's named listens.** Not blind: the candidate beside the recordings, plainly
   named (each instrument's listen script, e.g. `tools/arcus/listen.sh`), with a Spanish note
   of what changed and what to listen for. His ear decides what is tried next.
5. **The blind kit.** The subject's `listening` section (items, voices, parts, pairs, MUSHRA
   reference, conditions and anchors, the page's texts in Spanish and English):
   `lab.py kit <subject> <kit dir> [--wav-twin <dir>] --run <run>`. Then the page in real
   browsers: `PW_DIR=<playwright-core> node crates/cantus-ui/examples/abx_kit/abx_browser_check.mjs <kit dir>`
   (and on the twin). Zip the kit dir; publish the twin as `tools/listening/README.md` says.
   The key (`<kit dir>-key/`) is never sent.
6. **Reading the results.** `python -I tools/listening/abx_read.py <results.json>… <kit>-key/key.json`:
   per listener and pooled, the ABX's binomial p and power, the catch questions FLAGGED when
   failed, the MUSHRA per source (median and CI) with BS.1534-3's post-screening. Results
   files are personal data: never in the repository; record the scores in the round's notes.
7. **The gate.** An instrument is in the picker only when (a) the owner's named listen passes,
   (b) the blind kit says listeners cannot reliably tell it from the recording **or** prefer it
   / rate it within the recording's MUSHRA interval, on the pooled check-passing listeners at a
   trial count with 80 % power against p1 = 0.7 (37 ABX trials pooled; `abx_read.py` prints it),
   and (c) the report has no feature outside the band that the owner's or the listeners'
   notes name. What the measures say decides what is *tried*; what the ears say decides what
   *ships*.

## The held-out band method

The core report (`soundlab/band.py`). Each note's cell is (articulation, MIDI note, dynamic).
The subject's seeded rule holds a third of the cells out (`hash`: SHA-256 of `seed|art|note|dyn`,
one in three; `mod`: an arithmetic expression, Arcus's `(note + dyn) % 3 == 0`). The
held-out cells' reference notes are grouped (articulation · register octave from
`lowest_note` · dynamic class soft/medium/loud) and, per group and per feature, the band is
their **weighted 10th–90th percentile, each physical instrument weighing one**. The
candidate's median over its notes in the same held-out cells is placed in the band or out:
a scalar by its distance outside in band widths (a band of identical references counts 1 %
of its value), a vector element by element and summarised by the RMS of the distances outside
in the feature's units. A run beside an earlier one: the mean distance over the groups before
→ now, "closer"/"further" only past 5 % and 0.02.

## The tests (`tests/run.py`)

| file | what holds it |
|---|---|
| `test_loudness.py` | **outside values**: BS.1770-4's published 48 kHz coefficients (Tables 1–2); EBU Tech 3341 cases 1–5 and Tech 3342 cases 1–4 on the signals those documents define (±0.1 LU, ±1 LU); the one-channel 1 kHz sine reading −3.01; a true peak between samples; the EBU test set's own files (`SOUNDLAB_EBU`); the kits' Rust meter on a generated kit (`SOUNDLAB_KIT_KEY`: every file within 0.05 LU) |
| `test_features.py` | synthetic notes of known f0, partials, B, vibrato, noise, rise, T60, centroid trajectory, stereo, room; **outside labels**: TinySOL's published pitch of 34 cello and bass notes (every one within 50 cents with the note known; the pitch class over the family's range, at most one in ten an octave off) |
| `test_agreement.py` | the port against the Arcus study's own records on the same TinySOL files (at their rate: to 1e-4; through the lab's 48 kHz pipeline: within 1 cent and 1 dB) |
| `test_band.py` | the split (seeded, a third, no code run), per-instrument weighting, placement, vectors, before → after; `abx_read.py`'s binomial tail (12 of 16: p = 0.0384), critical counts, power, CI, sign test, the post-screening |
| `test_kit.py` | the worked example's kit built in a scratch folder: only what a listener receives, every file named and random, one loudness per item under the ceiling, catch and hidden reference one clip twice, the veil as the page (node) reads it, no source named in the page's data or readmes; every lab subject a page subject |

A light run (`--light`) needs nothing downloaded. `cargo test -p cantus-ui --example
tympanum_abx` runs `kit.rs`'s `the_sound_labs_subjects_are_page_subjects` (every lab subject a
page subject the Rust side takes too).
