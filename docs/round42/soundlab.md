# Round 42 — `soundlab`: one sound lab for every instrument

Branch `r42/soundlab` (from `r42/integration` 46aeca0b1). The owner (2026-11-25, translated):
*"Do we have good tools for making ABX tests and our own analyses comparing our sound against
recordings? I want to be sure everything is ready for when we develop more instruments."*
The answer before this branch: good tools, one set per instrument. After it: one lab
(`tools/soundlab/`, ADR 0160) that takes an instrument as a subject file and gives its measured
comparison and its blind kit; the protocol a new instrument follows is in
`tools/soundlab/README.md`.

## 1. The audit: what existed

| tool | what it does | generic | per instrument | gaps |
|---|---|---|---|---|
| `tools/chorda-voicing/` (`metric.py`, `metric2.py`, `ana/`, `ana2/` `accept*.py`, `tuning.py`, `tune_abx.py`, `preview.py`, probes) | Chorda's voicing lab: a phrase metric (bands, crest, dynamics, correlation), two independent analysts' scores (P1–P20: beats, two-stage decay, upper colour, knock, sub-bass…), the tuning gate, named previews at −20 LUFS | the phrase metric's bands and crest; `preview.py`'s loudness-matched named listens | almost all: every term targets a piano (beats, pedal, hammer knock, the Salamander's phrases), paths in `labpaths.py` | no split of references (one piano, the Salamander, is both fit and judge); no per-instrument spread (one recording); no outside-value test of its loudness; a second ABX page of its own (`tune_abx_kit.html`) |
| `tools/tympanum-fit/` (`fit.py`, `calibrate*.py`, `metrics.py`, `attack.py`, `grain.py`, `accept_b10.py`, `ladder*.py`, `ladder_kit.*`) | Tympanum's tables fitted from recorded kits; cymbal and attack measures; the fifteen-cue acceptance; the representation ladder with **its own MUSHRA + ABX page** and scorer | `ladder_kit`'s MUSHRA idea (hidden reference, anchor), `attack.py`'s octave bands, `grain.py`'s envelope fluctuation | the fits, the cues, the ladder's rungs; the kit hard-codes items, Spanish-first, simple names (10 random letters), no catch questions, no online mode | a MUSHRA that only one kit had; its loudness a hand-rolled K-weighting without gates; no post-screening in its scorer |
| `tools/arcus/` (`match.sh` → `examples/arcus_match/`, `study_compare.py`, `compare.sh`, `pass4/`, `pass5/`, `abx.sh`, `upper-listen.sh`) and **the reference study** (`tools/arcus/study/`: `bowlib_core`, `bowlib_feat`, `bowlib_note`, `catalog`, `aggregate`, `validate_synth`) | the cello and bass matched to ~1 060 recorded notes with a **held-out third** (`(key + dyn) % 3 == 0`); the study's estimator (phase-tracked harmonics, residual noise per band, vibrato with AM per partial, attack, release, pizzicato B and T60) validated on synthetic notes; the band across physical instruments; pass 5's vibrato cycle by cycle, strokes, YIN pitch | **the study's estimator** (nothing in it is bowed but its names), the held-out idea, the per-instrument band | the catalogue (strings' folders and names), the families' ranges, `arcus_match`'s own Rust features (a second estimator), the fit | the estimator ran on one family's catalogue; `study_compare.py` compared with the study's `aggregate` bands, not a seeded split of references; **the blind page (`upper-listen.sh`'s `abx.html`, `listen/abx-ciego`) named its files in clear** (`ref/arcus-cello.wav` beside `ref/grabacion-cello.wav`) and had no loudness match of its own, no catch questions, no results file |
| `crates/cantus-ui/examples/abx_kit/` (`abx.html`, `kit.rs`, `names.rs`, `abx_browser_check.mjs`) + `chorda_abx`, `tympanum_abx` | **the shared blind page** filled by a subject file: ABX + preferences + an extra ABX, loudness matched (BS.1770, ceiling), 64-bit random names, veiled tokens, catch questions, offline and published modes, the owner's summary; a Playwright check in three browsers | the page, the blinding, the matching, the check | each generator renders through the engine in Rust (fine for an engine instrument; not for a file-based one); the check's hide lists by subject id | no MUSHRA; a fixed order of parts; Arcus not on it; only the Rust side could fill it |
| `tools/listening/` (`README.md`, `abx_read.py`) | reading one listener's file: the score and its binomial p, every trial and preference, per item | all of it | the key's token names | one file at a time; no power; catch questions not flagged; no MUSHRA |

**Generic, kept and generalised:** the study's estimator; the held-out third and the
per-instrument band; the shared page, its blinding and its check; BS.1770 matching.
**Per instrument, kept where it is:** each model's fit (Chorda's analysts' scores,
Tympanum's tables, Arcus's match), each one's named listens. **The gaps closed here:** one
measurement package with a seeded split and a report; loudness held to outside values; a
MUSHRA mode on the one page; a file-based filler so any instrument (and recordings of any
rate) reaches the page; Arcus on it; results read pooled, with power, flags and MUSHRA
statistics; tests on outside sources.

## 2. The design

- `tools/soundlab/` — `lab.py` (check, refs, render, measure, report, all, kit); `soundlab/`:
  `core.py` and `notes.py` (the study's, ported unchanged in method), `features.py` (13
  features, one registry, two kinds of note), `loudness.py` (BS.1770-4 / R128), `band.py`
  (the held-out band method), `subject.py`, `measure.py` (references cached per file, three
  workers), `report.py` (JSON + an HTML page drawing its own SVG: strip plots per scalar,
  small multiples per vector, before → now), `kit.py` (the blind kit from files).
- The subject file: references' catalogue (free licences enforced; a source with terms as
  free only with its terms quoted), kinds, features, `exclude`, the split, the candidate's
  command and pattern, the `listening` section (items, voices from folders or render commands,
  parts, pairs, MUSHRA reference/conditions/anchors, the page's texts in Spanish and English,
  `hide`).
- The page (`abx_kit/abx.html`): a MUSHRA part and `SUBJECT.parts`; the extra part only when a
  kit has one; Chorda's and Tympanum's kits unchanged in behaviour (their mocks pass the same
  check). `kit.rs`: a cargo test that every lab subject is a page subject.
- `abx_read.py`: many files, power, flags, MUSHRA, pooled.

## 3. Arcus's cello, end to end

Subject `tools/soundlab/subjects/arcus-cello/` (references from the study's catalogue by
`adapters/study_catalogue.py`, free sources only: TinySOL 291 arco, Karoryfer Bigcat 136 arco +
13 pizzicato, Good-sounds 99 arco, Iowa MIS 19 arco + 20 pizzicato files; 578 files, 904 notes,
280 of them in held-out cells; Philharmonia, PHENICX and VSCO's sections left out). The split
is Arcus's own (`(note + dyn) % 3 == 0`), so the judged cells are those model 2's match never
saw.

```bash
export SOUNDLAB_REFS=F:/projects/daw-r42/downloads CANTUS_PY=F:/projects/daw-r42/study/venv/Scripts/python.exe
$CANTUS_PY -I tools/soundlab/lab.py check   tools/soundlab/subjects/arcus-cello/subject.json
$CANTUS_PY -I tools/soundlab/lab.py measure tools/soundlab/subjects/arcus-cello/subject.json p4-dc8 --from F:/projects/daw-r42/tmp/arcus/p4/dc8
$CANTUS_PY -I tools/soundlab/lab.py all     tools/soundlab/subjects/arcus-cello/subject.json r42-integration --before p4-dc8
$CANTUS_PY -I tools/soundlab/lab.py kit     tools/soundlab/subjects/arcus-cello/subject.json F:/projects/daw-r42/listen/soundlab-arcus-abx \
    --wav-twin F:/projects/daw-r42/listen/soundlab-arcus-abx-wav --run r42-integration
```

- **The report:** `F:\projects\daw-r42\soundlab-work\arcus-cello\runs\r42-integration\report.html`
  (+ `report.json`): run `r42-integration` (this branch's base, rendered by `arcus_match render`
  through the slot) against `p4-dc8` (the arcus builder's pass-4 renders, measured from their
  folder). The numbers: §3.1.
- **The blind kit:** `F:\projects\daw-r42\listen\soundlab-arcus-abx\` (FLAC, offline) and its
  WAV twin `…-abx-wav\`, the key in `…-abx-key\`. Four items (the ABX phrase — Arcus from
  `abx_phrase.mid` against the Karoryfer cello played by Cantus's sampler from the same MIDI —
  and A3 held at p, mf, f against Karoryfer's own A3 sustains), three voices (Arcus now; Arcus
  model 1, before the match; the recording), parts ABX (16) → MUSHRA (4 items × hidden
  reference, both Arcus voices, anchors 3.5 and 7 kHz) → preferences (8 + 2 catch), an extra
  ABX model 1 vs now; −23 LUFS, first onsets aligned at 80 ms, the phrase cut at 12 s. It
  replaces `listen/abx-ciego` (files named in clear).

### 3.1 What the report says (held-out cells, 12 arco and 11 pizzicato groups)

RESULTS_PLACEHOLDER

## 4. What was verified

TESTS_PLACEHOLDER

## 5. What is open

- **The owner's ears** on the Arcus kit (`F:\projects\daw-r42\listen\soundlab-arcus-abx\`) — not
  run by anyone yet; and his word on the gate's bar (README §7: 80 % power against p1 = 0.7 =
  37 pooled ABX trials; MUSHRA within the hidden reference's interval).
- The EBU loudness test set's own WAVs: tech.ebu.ch answers automated fetches with a
  browser challenge (not bypassed); the test reads them when fetched by hand into
  `SOUNDLAB_EBU`. The loudness is held meanwhile by the Tech 3341/3342 signals as those
  documents define them, BS.1770-4's published coefficients and the kits' Rust meter.
- A decaying note's noise between partials (`features.between_partials`) is new: validated on
  synthetic plucks only; it reads a synthetic pluck 50–100 dB clean where recordings carry
  their room and recorder floor (the Arcus pizzicato's HNR out of the band says as much about
  the recordings' floor as about Arcus). A floor reading per reference (the study's `pre`) would
  separate the two.
- Chorda's and Tympanum's subjects are page subjects only: their measurement still runs in
  their own labs. Moving Chorda's phrase metric and Tympanum's cymbal cues into features of
  the registry is the next step when either instrument is worked on again.
- Not run: Safari; the published (claude.ai) mode on the Arcus kit beyond the stubbed check.
