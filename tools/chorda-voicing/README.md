# chorda-voicing — voicing Chorda against a recorded piano

The voicing lab round 40 used to bring Chorda's model 2 toward the Salamander Grand (a recorded
Yamaha C5, CC BY 3.0, Alexander Holm): sixteen candidates, each rendered, measured by two
independent analysts' scripts and a tuning gate, listened to, and the sixteenth shipped as the
default (`DSP_VERSION` 7, ADR 0098's third amendment). Off-line, never shipped. The history and
every number: `docs/round40/chorda.md`. What a candidate *is*: **`SPEC.md`**; every knob:
**`KNOBS.md`** (generated, `knobs.py`).

Bash (Git Bash on Windows, or Linux) and Python 3 with `requirements.txt`; every Python script
runs with `python -I`.

## Setting up a lab (once per machine)

```bash
python -m venv ~/venvs/chorda && ~/venvs/chorda/Scripts/pip install -r tools/chorda-voicing/requirements.txt
export CANTUS_PY=~/venvs/chorda/Scripts/python.exe      # or the shared venv: tools/sound-refs/setup.md
export CHORDA_LAB=/f/chorda-lab                          # optional: default $CANTUS_WORK/chorda-lab
bash tools/chorda-voicing/build_lab.sh base              # chorda_compare in release → <lab>/bin/chorda_compare_base
export CHORDA_EXE=chorda_compare_base                    # a bare name is looked up in <lab>/bin
bash tools/chorda-voicing/references.sh [<downloads dir>] # the Salamander (~1 GB; default $CANTUS_REFS/piano) + the reference renders
```

| variable | what | default |
|---|---|---|
| `CHORDA_LAB` | the lab: renders, results, the lab builds in `bin/` | `$CANTUS_WORK/chorda-lab` (tools/sound-refs) |
| `CHORDA_SHARE` | previews and kits for the owner | `<lab>/share` |
| `CHORDA_TMP` | scratch (the lab's TMP/TEMP) | `<lab>/tmp` |
| `CANTUS_PY` | the Python with `requirements.txt` | the venv `$CANTUS_REFS/venv`, else `python` |
| `CHORDA_EXE` | the voicing lab: a path, or a name in `<lab>/bin` | `<repo>/target/release/examples/chorda_compare` |
| `CHORDA_PARTIALS`, `CHORDA_LEVELS` | the candidate's two tables, read by every lab render | none (probes: `PARTIALS`/`LEVELS`, default candidate 16's) |
| `BASE` | a probe's base spec | `candidates/c16/spec.txt` |

`env.sh` (sourced by every shell script) and `labpaths.py` (imported by every Python one) are
the only places that know where things are. Disk: a full evaluation writes ~1 GB of renders (a
`notes_*.wav` is 69 MB — **a smaller one is a render truncated by a full disk**, silently);
the `c11`/`c16` probes delete theirs unless `KEEP=1` (the `c7` ones keep them in `<lab>/c7/`).

The references (`references.sh`): the Salamander through the engine (`eng40/wav/salamander`
the listening phrases, `notes5/wav/salamander` the single notes A0–C8 every minor third at
pp 30 / mf 75 / ff 115, `keys/wav/salamander/keys_mf.wav` every key at mf, `c7/vs/wav/salamander`
the treble's velocity sweep), beta.14 through the lab (`eng40/wav/chorda2`, `notes5/wav/b14`),
and the first analysis's slot 2 (`lab4/wav/candB`, candidate 1: `candidates/candB`).

## What is here

| | |
|---|---|
| `candidates/c16/` | the shipped voicing: `spec.txt`, `partials.csv`, `levels.csv` (= `chorda_voicing_c16.rs`) |
| `candidates/candB/` | candidate 1, the first analysis's comparison slot |
| `ana/` | the **first analysis** (an analyst who heard only the audio, candidate 2): `notes_analysis`, `decay_beats`, `fund_decay`, `bursts`, `phrases_spectra`, `octave_diff`, `space`, `phrase_time`, `onset_shape`, `fill` (+ plots); **`accept.py`** — its findings P1–P8 as one score, each term 1 at the edge of the analyst's target; `try.sh` renders a candidate and runs them |
| `ana2/` | the **second analysis** (candidate 5; `report.md`, its targets): `s01`…`s26`; **`accept2.py`** (+ P9 beats, P10 upper colour, P11 rise), **`accept3.py`** (+ P12–P20: sustain, upper tone, treble knock, pp steps, release, sub-bass, evenness, trill tail, tuning), **`tuning.py`** (the gate), **`keylev.py`** (evenness, level-table fit), **`fit_partials2.py`** (partial-table fit), **`run6.sh`** (the full evaluation), `try2.sh`, quick runs (`quick6.sh`, `phr.sh`, `beats.sh`, `hfprobe.sh`, `spaceprobe.sh`, `colour.sh`, `hf2.py`, `subbass.py`), **`search11.py`** (the coordinate search) |
| `metric.py` | round 38's phrase metric (bands, crest, dynamics, correlation on the seven phrases; target ≤ 3.7) |
| `metric2.py` | the same with an audibility floor (ISO 226; the reference's recording hum under middle C stops counting) — printed beside, never instead |
| `levels.py`, `bassobj.py`, `bass.py`, `heard.py`, `bandenv.py`, `tails.py` | the phrases' balance against the staging triad, the bass phrase's own score, octave levels as heard at −20 LUFS, a note's bands over time, the room after a release |
| `stage.py` | stages a spec: the output gain that keeps the ff chord's peaks under the knee |
| `preview.py` | the owner's named previews (not blind): every phrase from each source at −20 LUFS, 24-bit FLAC |
| `probes/c7`, `c11`, `c16` | the quick probes the candidates were found with (below) |
| `ship_rust.py` | a candidate's files → the shipped Rust module |
| `build_lab.sh`, `references.sh`, `env.sh`, `labpaths.py`, `knobs.py` | the lab itself |

The analysts' scripts keep their own labels — "3 candidato 5", source `c5` in `ana2`, `c2` in
`ana` — for whatever try they are pointed at (`ANA2_NOTES`, `ANA_RAW3`…): the label names a
slot, not a candidate.

## The method

Distilled from the round's handovers (candidates 6–16). Each step names its tool.

### 1. Make a candidate

Start from the last candidate's three files (`SPEC.md`). A change that the existing knobs can
express is data only (candidate 16 needed no Rust at all). A new knob is Rust: added to
`Voicing` (`crates/cantus-dsp/src/chorda_voicing.rs`) **neutral at beta.14's value** — the
default renders bit-identical, a pinned hash holds it — with its test, then a lab build kept by
tag (`build_lab.sh c17a --check`: the Chorda DSP tests and clippy too). Set `CHORDA_EXE` to that
build for every number of the candidate, so its numbers can be rerun on the exact binary.

### 2. Measure it

The full evaluation, ~20 min:

```bash
CHORDA_EXE=chorda_compare_c17a CHORDA_PARTIALS=$PWD/c17/partials.csv CHORDA_LEVELS=$PWD/c17/levels.csv \
  bash tools/chorda-voicing/ana2/run6.sh c17a c17/spec.txt
```

It renders the phrases, the single notes and every key; runs both analyses; prints the first
analysis's terms and total (`accept`), the second's (P9–P20, `ACCEPT3`), round 38's metric
(`TOTAL`, `TOTAL(audible)`), the phrases' balance (`LEVELS`), the keyboard's evenness, the
**tuning gate**, the release, the sub-bass and the soft treble's knock. Results stay in
`<lab>/ana/out_<tag>` and `<lab>/ana2/out_<tag>` (`accept3.py <ana out> <ana2 out> <keylev.log>`
prints the score again). Lower is closer everywhere; a term near 1 is at the edge of its
analyst's target.

Candidate 16 (run `c16e`): first analysis 6.935 (P1 two-stage beats 0.499, P5 endings 0.69),
accept3 11.815 (10.955 without P20), P10 upper colour 1.48, metric 4.488 (audible 3.960), gate
ok (worst key C1 +9.7 cents), C4 beat depth 8.4 dB, top octave unchanged (14.6 / 18.8 in the
round's C7–C8 probe).

Between full runs, the probes (each ~10 s to 2 min; `bash <probe> <tag> "<extra spec>"` renders
`BASE` + the extra keys, prints one table, deletes its renders):

| probe | what it answers |
|---|---|
| `probes/c11/q.sh` | the metric per phrase, each phrase's presence band, middle C's partials 8–9, the notes' presence by register (add a third argument to render the notes) |
| `probes/c16/q16.sh` | round 38's metric and presence on the phrases (10 s) |
| `probes/c16/p10.sh`, `pb.sh` | P10 upper colour (A4–A5 partials 2–4 re the fundamental); with the first analysis's P1 |
| `probes/c16/ks.sh` | every key at mf: the worst tuned keys, the flanger outliers A0–C5, one key's draw row (`DK`) |
| `probes/c16/dk.sh`, `probes/c11/drawsearch.sh` | per-key draw searches (`DRAWS`, `K`/`KEYS`; with a chord's beats) |
| `probes/c11/hfk.sh` | an `hf_key.60` sweep (`HS`) |
| `probes/c7/p89.sh`, `pp.sh`, `pfq.sh`, `pp2.sh` | middle C's partials 8–9 at 0 / 1.5 s; the C3–D#5 partial profile and frequencies vs the reference |
| `probes/c7/trial.sh`, `trialk.sh`, `vtrial.sh`, `hfq.sh`, `hft.sh`, `tunek.sh` | the treble's dynamics; the top fundamentals' decay; the treble velocity sweep; the bass 4–6 kHz and beats; the treble's 4–6 kHz; chosen keys' tuning |
| `probes/c7/st.sh`, `ana2/spaceprobe.sh` | the stereo terms (P3, P4) alone |
| `ana2/quick6.sh`, `beats.sh`, `hfprobe.sh`, `phr.sh`, `colour.sh` | the beats and sustain; the 4–6 kHz sustain; the metric in detail; the upper middle's partial balance |

The Python measures under them (`probes/c7/tune.py`, `chordbeat.py`, `pprof.py`, `pfreq.py`,
`fslope.py`, `tail2.py`, `ltas.py`; `probes/c11/flank.py`, `drawq.py`, `vbright.py`, `runatk.py`,
`hum*.py`; `probes/c16/tunek.py`, `ptime.py`, `penv.py`, `pabs.py`, `pkeys.py`) run on any render;
each says its arguments in its first lines.

### 3. Gate it — the owner's bar

A candidate goes to the owner only when all four hold (the coordinator's bar, candidate 11 on):

- **(a) the tuning gate** — `ana2/tuning.py <keys_mf.wav>`: every key's first partial within
  10 cents of the reference's stretch after the median offset is taken out (exit 1 when not);
  it also prints partials 1–3's rms and the consonant intervals' beats in the test chords.
  Added after the owner heard D3 46 cents flat (a clamped allpass) — run it on every candidate.
  Its gate 2 (round 40, the technician's pass, `docs/round40/chorda-tune.md`) measures the
  candidate on its own, all 88 keys, as the suite's kit does: the ear's pitch of each key
  against its neighbours (2 cents A0–E6) and every octave and fifth against a smooth fit of
  its own partials (0.5 Hz and 4 cents; a 4:2, 1 Hz and 7). It is a second view of
  `chorda::tests_tune_gate`, which holds the shipped voicing to the designed curve itself.
- **(b) none of his complaints measurable** — out of tune (the gate, `tunek.py`), the nasal
  middle C (`p89.sh`: partials 8–9 against the reference's), the ff chord's vibration, the odd
  bass; and **the top octave unchanged** (he liked it: C7–C8 mf→ff, `trial.sh`).
- **(c) no regression** against the previous candidate — term by term in both analyses, and no
  flanger over 0.2 A0–C5 (`flank.py` through `ks.sh`).
- **(d) both analyses better** than the last listened candidate, and round 38's metric moving
  toward ≤ 3.7.

### 4. Refit the tables

After a change that moves the keyboard's evenness, refit the levels (`keylev.py … fit`,
`SPEC.md` §3); the partial table with `fit_partials2.py` (`<notes dir> <old table> <old amount>
<new table> [gain] [clamp] [lo] [hi]`) or by hand with `mkpart.py`. A key whose unison landed
badly gets a draw (`draw.<key>`) from a draw search, not a register knob — and a draw that fixes
one measure can break another (candidate 13's `draw.50=2` stretched D3's partials and the soft
chords' fifth beat 4.5 Hz for three candidates): rerun the gate and the chord beats after one.

### 5. Search (when a few knobs are to be balanced)

```bash
python -I ana2/search11.py <tag> body.eq4k,hf_decay,itd_shape <start spec> [rounds 1] [scale 1] [parallel 4]
```

A coordinate search, each trial a full `run6.sh` without the every-key render, scored by
accept3 + 0.5 × the metric + 0.7 × the balance; log and best in `<lab>/search/`. Keep the
integrator's bounds in its table (`P`: hf_decay 0.9–1.4 — the measured loss law — and so on): an
unbounded search overfits the analysts' terms. Four trials at once take ~20 min a batch; a
trial whose analysis ran out of memory (other builds compiling) is retried, never scored.

### 6. Listen

- **Named previews** for the owner: `python -I preview.py "$CHORDA_SHARE/chorda-preview-17"
  "1 Chorda 2 (beta.14)=$CHORDA_LAB/eng40/wav/chorda2" "2 candidato 16=…" "3 candidato 17=$CHORDA_LAB/lab4/wav/c17a"
  "4 Salamander=$CHORDA_LAB/eng40/wav/salamander"`, a `NOTA.md` in Spanish beside it (what
  changed, what to listen for, phrase by phrase) and a zip.
- **The blind test** (`chorda_abx`): `tools/listening/README.md`. The owner's verdict outranks
  every number here.

### 7. Ship

`python -I ship_rust.py <spec> <partials> <levels> crates/cantus-dsp/src/chorda_voicing_<cN>.rs
"candidate N"` writes the module (run on `candidates/c16` it reproduces
`chorda_voicing_c16.rs` byte for byte). Shipping is then the usual DSP change: `DSP_VERSION`
bumped, golden hashes moved in one commit with their measured size, ADR 0098 amended, the
previous voicing kept by name and pinned (beta.14's is), the test that holds
`Voicing::default` to the files, and the candidate's files copied to `candidates/cN/`.

## Lessons the round paid for

- The lab drives the DSP directly; the engine's render path (`chorda_compare render`) is the
  truth every reported number must agree with — since round 40 they stage alike.
- Two measures can disagree on the same thing (an LTAS normalised on 0.2–5 kHz read middle C's
  nasal peak +8 dB; the A-weighted metric ~0): find out why before tuning to either.
- A static cut cannot fix what is a decay (C5–B5's partial 2 was right at the attack and
  +7…+23 dB in the sustain); look at the time windows before reaching for the table.
- The 64-voice CPU test fails when three builds share the machine: rerun it quiet before
  calling it a regression. Run lab renders through `tools/build/slot.sh` only for builds;
  the renders themselves are one thread per phrase.
