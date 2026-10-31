# ADR 0086 — Audio to MIDI: Basic Pitch ported note for note, our own tracker for a line, drums by our detector and Magenta's model

**Status:** accepted (round 34, transcribe builder) · **Date:** 2026-10-30 ·
**Extends:** ADR 0085 (the ML helper, ONNX Runtime, models on demand), ADR 0054 (the pitch
editor's tracker), ADR 0080 (per-note expression), ADR 0042 (licences) · the helper's job
`transcribe` · model manifest entries `basic-pitch`, `oaf-drums` · no schema change

## Context

The owner asked for audio to MIDI "with the same care" as stems (round 34, item 3):
melody, polyphony and drums, on an audio clip (*Convert to MIDI…*) and for agents
(`audio_to_midi`), with the accuracy measured — note F-measure by mir_eval's rules on
material rendered from known MIDI through our own instruments and on a public set, drums
class by class, and Cantus's output against the reference implementation's.

## Decision

### 1. Polyphony: Spotify's Basic Pitch, its note creation re-implemented in Rust

- **The model** is Basic Pitch's own `nmp.onnx` (230 KB, Apache-2.0, in Spotify's
  repository at commit fa5997af), downloaded from GitHub raw at that commit with its
  licence, hash pinned, run as it is: a new `prepare` kind, `onnx`, only checks the
  maker's file (`cantus-content/src/models_onnx.rs`). Training data, as the paper states:
  GuitarSet, MAESTRO, Slakh, iKala, MedleyDB-Pitch (public sets); quality published:
  note F without offsets 0.79 GuitarSet, 0.71 MAESTRO, 0.52 Molina, 0.49 Phenicx, 0.42
  Slakh.
- **The job** (`cantus-ml/src/transcribe.rs`) is `basic_pitch.inference.predict` step for
  step: mono at 22 050 Hz, 3 840 samples of silence in front, windows of 43 844 samples a
  hop of 36 164 apart, each through the network, the outputs joined with 15 frames dropped
  at each side and trimmed to `floor(len · 86 / 22050)` frames.
- **The note creation** (`transcribe_notes.rs`) is `note_creation.py` decision for
  decision: onsets inferred from the note posterior's rises (NumPy's `float64` promotion
  and its NaN when the posterior never rises kept), onset peaks strictly above both
  neighbours, walked latest first with eleven frames of tolerance, the energy taken with
  its neighbours, the Melodia trick, bends from the contour under a Gaussian (σ 5 bins),
  frames to seconds with Basic Pitch's own alignment constant, velocities `round(127 · a)`
  in `float32` with ties to even, a note's mean by NumPy's own pairwise summation. The one
  change is speed: the Melodia trick's "largest remaining energy" comes from a heap of
  the cells above the threshold, the same cell NumPy's `argmax` finds (energy only falls),
  instead of a scan of the matrix per note.
- **Bends** are kept or dropped by choice; kept, each note gets a per-note bend curve
  (ADR 0080) — every note, overlapping ones included, where Basic Pitch's MIDI has to drop
  them (one channel).

### 2. Melody: Cantus's own pitch tracker, because it measured better

Two rules on Basic Pitch's notes (the loudest note wherever two overlap; the highest) and
the pitch editor's own tracker (pYIN-style, ADR 0054) were measured on every monophonic
file (§4). The tracker won on both sets — rendered melodies 0.805 against 0.718 (loudest)
and 0.650 (highest); TRIOS's solo instruments 0.769 against 0.658 and 0.580 — and beats
Basic Pitch's unrestricted notes on the same files (0.746, 0.699). So **melody mode is the
tracker**: in process, on a worker, no model, nothing to download; its pitch curve becomes
the note's bend when asked. The helper keeps a `melody` mode (the loudest-note rule) for
the measurement.

### 3. Drums: our own detector, and Magenta's Onsets and Frames Drums when installed

- **Cantus's detector** (`cantus-dsp/src/onset_drums.rs`, MPL, ours): semi-adaptive NMF
  under the KL divergence on a square-root-compressed log-frequency spectrogram (8 bands
  an octave, 40 Hz – 16 kHz, 100 frames a second); seven templates drawn from measured
  average spectra of isolated hits (kick, snare, a bright snare, a low and a high tom,
  hi-hat, cymbal) pulled back toward their start less each iteration, three free
  templates for what is not a drum; hits picked per group (low drums, snares, metal) on
  their activations against a moving mean, named (tom when the toms' templates hold it and
  its body sits above 125 Hz; hat or cymbal by its own template and how it rings), a hit
  dropped when it is only another's leak, or quieter than −34 dB under the loudest group,
  or not louder in its register. The decisions' numbers were found by a coordinate search
  on MDB Drums' first eight tracks and Pulsus's seven factory kits weighed equally; the
  other fifteen MDB tracks are held out.
- **Onsets and Frames Drums** (Magenta, Callender, Hawthorne, Engel 2020; E-GMD, 444 h,
  CC BY 4.0), as an optional 6 MB download: Magenta's E-GMD checkpoint (published in its
  Apache-2.0 repository) converted to ONNX by a third party (`thebluescreen` on Hugging
  Face, Apache-2.0, pinned at commit 771fd552, hash checked; the converter states no weight
  changed). **The hi-hat, fixed at install (round 34's review):** Magenta trained the
  checkpoint with `drum_data_map='8-hit'`, which folds every hi-hat into pitch **46**; the
  conversion's output `Gather` (an int64 initializer `[15, 17, 27, 21, 30, 32, 28, 54]`,
  pitches − 21) takes pitch **42** for the hat — an output trained toward zero, which never
  answered (its largest probability 0.00 on every file). Exporting all 88 outputs shows
  pitch 46 carrying the hats (F 0.87 on the tuning stems at threshold 0.3). The content
  manager therefore copies the downloaded file and changes that one index to 25 (a
  `prepare` patch: the exact 64 bytes found once and replaced, the copy's hash checked,
  `oaf_drums.cantus.onnx`), every weight as the converter left it. The detector still says
  which hats were open (its open hat within 40 ms). The ride bell and the cowbell Magenta
  folds into pitch 53 play the ride (51): the studio kit has no bell pad. **What is
  verified and what is not:** the patch's effect is measured (below); the conversion was
  not compared with Magenta's TensorFlow inference — the converter's scripts
  (`Sillybit-io/audio-to-midi-app`, `scripts/drums/`) answer 404, and no TensorFlow run was
  made here — so beyond the hat the conversion is trusted by its measurements only.
  Thresholds (0.3, 0.4, 0.3, 0.3; the hat 0.2) and a tom dropped when the kick fires
  harder beside it were tuned on the same eight MDB tracks and the kits. ADTOF (the best
  published system) is CC BY-NC-SA and refused by ADR 0042.
- **The classes written** are the user's choice; kick, snare and hi-hat by default, toms
  and cymbals by default only with the model (said in the dialog).
- **From a mix**, the drums can be separated first by stems' HT-Demucs (the helper's
  `separate` job, then the drum stem transcribed) when it is installed.

### 4. Measured (all onset tolerances 50 ms, mir_eval's matching)

**Basic Pitch in Cantus against Spotify's `basic_pitch` 0.4.0** on the same 19 TRIOS files
(22 050 Hz): posteriorgrams bit-identical (the same ONNX Runtime 1.31.1), **3 286 of 3 286
notes identical**; in CI, the note creation on a 16-bit fixture of the network's output
under six settings (defaults, no Melodia, no inferred onsets, sensitive, strict, a band)
equals Basic Pitch's own — every frame, pitch, mean to the bit, bend. Speed: 47–73× real
time on this laptop's CPU (a 43-second trio in 0.6 s).

**Notes, F without offsets** (F with offsets in parentheses):

| Material | Polyphonic (Basic Pitch) | Melody mode (own tracker) |
|---|---|---|
| Rendered through our instruments: 7 melodies, 5 chord parts, 1 225 notes | 0.609 (0.399) | 0.805 on the melodies |
| — melodies: Unda / Calor / Modulus / Gravis / Lamina / Organum / Chorda | 1.000 / 0.980 / 0.839 / 0.605 / 1.000 / 0.206 / 0.873 | 1.000 / 0.891 / 0.864 / 0.909 / 1.000 / 0.000 / 0.979 |
| — chords: Chorda / Lamina / Organum / Calor / Unda | 0.696 / 0.612 / 0.063 / 0.739 / 0.746 | — |
| TRIOS (CC BY-NC-SA, local only): 19 files, 3 068 notes | 0.703 (0.333) | 0.769 on the 8 solo stems |
| — mixes / pianos / solo stems | 0.493–0.809 / 0.712–0.843 / 0.519–0.887 | 0.593–0.979 |

Organum (the tonewheel organ with its rotary speaker) is the one instrument neither
transcribes; the manual says so.

**Drums, F per class** (kick · snare · toms · hi-hat · cymbals; all classes pooled):

| Set | Detector alone | With the drum model |
|---|---|---|
| MDB Drums, 15 held-out drum stems | 0.961 · 0.849 · 0.311 · 0.647 · 0.115; 0.691 | 0.963 · 0.885 · 0.442 · **0.738** · 0.640; **0.811** |
| MDB Drums, the 8 tuning stems | 0.890 · 0.737 · 0.403 · 0.801 · 0.124; 0.686 | 0.937 · 0.842 · 0.879 · 0.826 · 0.425; 0.820 |
| Pulsus's 7 kits, a known 16-bar pattern (CI) | 0.986 · 0.833 · 0.286 · 0.847 · 0.250; 0.740 | 0.918 · 0.972 · 0.620 · 0.824 · 0.102; 0.774 |
| MDB full mixes, held out | 0.702 · 0.610 · 0.034 · 0.590 · 0.180; 0.539 | 0.841 · 0.579 · 0.051 · 0.673 · 0.298; 0.611 |
| MDB full mixes, held out, drums separated first (HT-Demucs) | 0.957 · 0.727 · 0.086 · 0.654 · 0.124; 0.648 | 0.941 · 0.842 · 0.345 · **0.771** · 0.564; **0.795** |

Published for MDB's five classes pooled: 0.87–0.89 (ADTOF and its successor, trained on
the NC-licensed ADTOF data). Toms stay under 0.6 on the held-out stems even with the model
(0.44: those tracks hold few toms, and the model also fires on low kicks); cymbals reach
0.64 with it; hi-hats rise from the detector's 0.65 to the model's 0.74 once its hat
output is the one it was trained on (on Pulsus's synthesized hats the detector stays a
little ahead, 0.847 against 0.824). On full mixes everything falls (0.61), and
**separating the drums first** with stems' HT-Demucs brings a mix back to 0.795, near a drum
stem's 0.81: the dialog offers *Separate the drums
first* when HT-Demucs is installed (about real time on this laptop: 13–120 s for MDB's
17–103 s tracks), the agent tool `separate_first`.

### 5. The feature

*Convert to MIDI…* on an audio clip's menu: mode, sensitivity (0.5 = each model's own
defaults), shortest note, pitch bends, the drum classes, instrument (Unda or Exemplar;
Pulsus II's studio kit on the GM map for drums), quantise (off by default); the model's
name, licence, size and state before it downloads (the click that converts accepts its
licence); progress and *Stop*. Sensitivity is the models' (melody's tracker decides what
is voiced by itself, so melody mode hides it); the shortest note applies to every mode.
The clip is cut into the pieces `Snapshot::compile` plays (one per period of a loop, the
last cut at the clip's end), each with its own mapping (tempo map from its own start,
warp, slip, offset; reversed in its own window); the frames they cover together are read,
mixed to mono, converted to the model's rate and written a block at a time on a worker
(bit-identical to converting the whole file); each piece places what its own frames hold
through its own mapping — a note cut by a piece's edge keeps the part of its bend over what
is kept, a reversed note's bend runs backwards, a drum hit lands at its onset — into a new
track under the source with a MIDI clip aligned to the audio clip —
`InsertTrack` and `InsertClip` in one `Batch` (`cmd.transcribe`), one undo step. The agent
tool `audio_to_midi` (class Edit, a job ending in `JobDone::Then`) takes the same path; it
names the clip and the track in what it says and never downloads a model. Only the
arrangement's audio clips are converted — a session clip has no place on the timeline for
the MIDI clip to line up with, and both paths refuse it in words that say so. Exemplar
starts with its first factory preset (an empty sampler is silent); every drum key written
has a pad in the studio kit, its pads named in the interface's language. An agent's
conversion holds stems' agent counter from its preparation until its job ends, so the
helper counts as in use between its helper jobs too (reading the take, separating, then
transcribing).

**Files and jobs.** A conversion works in `Cache/transcribe/<pid>-<n>/`, held by a lock on
`held.lock` while it lives and removed when its last holder lets go (a stop while the take
is still read included: the reader is cancelled between blocks); a folder whose lock no
process holds (a Cantus that ended mid-conversion) is swept when the next conversion
starts. The ML service's router never makes a job's state again from a late update, a
starter forgets its job when it ends (stop, agent tool, separation first), and the table
keeps at most 32 ended jobs whatever the starters do. The detector says it is alive at
each factorisation step, so an hour's take is not taken for a hung helper. A conversion
remembers the project it was started on (`CantusApp::epoch`): another project opened, the
dialog closes and the conversion stops with nothing made (an agent's job is refused by
the MCP layer's own epoch check); its workers wake the window when they finish; and a
running conversion counts as the helper in use, so a model is not removed under it. The
models' hashes the helper checks come from the manifest this build carries, never from the
marker on disk; at start *prepared* means the marker, the version and the graph's size
(stems' rule, review 2).

**An hour of take** (16-bit stereo, 48 kHz; measured peak working sets, this laptop): the
interface's reader **15 MB** (it was 0.97–1.27 GB with the whole take in memory); the
helper **1.49 GB** for notes and chords (the 22 kHz input, 0.32 GB, and the
posteriorgrams of the whole hour, 0.55 GB, which Basic Pitch's note creation needs at
once), **1.61 GB** of private memory (1.38 GB working set) for drums with the model (the
44.1 kHz input, 0.64 GB, and the detector's spectrogram). In a debug build the hour takes
1.4 minutes as notes and 3 as drums.

## Consequences

- A user without any download gets melody (own tracker) and drums (own detector); notes
  and chords need 230 KB plus the runtime (15 MB); better drums 6 MB more.
- No model change, no schema bump: transcribed MIDI is not synthetic audio (legal.md §3e).
- Weights' availability: Spotify's GitHub and a third party's Hugging Face repository; the
  latter's conversion could not be checked against Magenta's TensorFlow graph here
  (measured instead), and Cantus edits one index of it at install — a new upload of the
  file fails its hash and is refused rather than patched blindly.
- Basic Pitch's port carries Spotify's NOTICE in THIRD-PARTY-NOTICES.md (Apache-2.0
  §4(d)), from `xtask/notices/head.md`.

## What the tests hold

`cantus-ml`: the note creation against Basic Pitch's own notes under six settings; NumPy's
`float32` mean, Basic Pitch's times, windows and joins; the job through the real helper
with a fixture network (tones to their notes across window joins, melody's one line, drums
without a model at 44.1 kHz, a cancel within 3 s leaving no file, a crash reported, wrong
rates and settings refused, the fixture refused outside tests, a three-minute drum take
under a 1.5 s silence limit; every key the network and the detector write is one the kit
has; Spotify's NOTICE in both notice files); ignored: Basic Pitch
against Spotify's on files (`transcribe_real`), drums with the model on MDB and the kits
(`transcribe_drums_real`). `cantus-dsp`: the measures against mir_eval's rules; the
detector's decisions on made activations, timing, silence, velocity, rates; ignored: MDB
Drums per class and the tuning search. `cantus-engine`: Pulsus's kits per class, each held
to a floor (CI). `cantus-content`: the bundled entries, an ONNX model installed as it
arrived, validation, the drum model's patch (applied to a copy, its hash held, the hat at
pitch 46). `cantus-ui`: melody from the dialog as one undo step undone exactly, its
shortest note, bends and quantise, the drum classes, drums through the helper, the agent
tool as one named step that refuses a missing model and leaves no job behind, the strings
in both languages; placement (`transcribe_place`) against ticks worked out from the
engine's rules — a loop after a tempo change with a short last piece and a cut bend, a
reversed loop with a mirrored bend and a drum hit at its onset, an offset with a slip, a
warp — the kit's pads, Exemplar's sound; the block-wise conversion bit-identical to the
whole file's for seven rate pairs (`transcribe_read`); the cache swept, a stop while
reading leaving nothing, a cancelled read (`transcribe_workdir`); the service's job table
(`ml_service_jobs`). Ignored: the note F-measures (`transcribe_accuracy`), an hour's take
(`transcribe_long`).
