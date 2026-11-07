# ADR 0099 — Sectio: Exemplar's slicer

**Status:** Accepted · **Date:** 2026-11-05 · **Extends:** ADR 0023 (native instruments),
ADR 0061 (onsets and the beat grid) · **Schema:** 55

## Context

BACKLOG §3e item 1 asks for a slicer at the level of FL Studio's Slicex and Fruity Slicer 2,
Ableton Simpler's slice mode, Logic's Quick Sampler and Bitwig 6.1's *Sliced*: drop a loop,
it is cut; markers read first, then transients (SuperFlux), a beat grid or the user's hand;
the points stored in the project; one slice per key from C1; per-slice pitch, gain, pan,
envelope, filter, reverse and choke; *Dump to piano roll*, replaying the loop to the bit at
its tempo; Random, Stutter and Quantize/Swing as one undo step each; drop-to-sliced in two
clicks or fewer.

## Decisions

### 1. A mode of Exemplar, not a new instrument

A sliced loop is an Exemplar whose zones are its slices: zone `z{i}` plays frames
`[sl.p{i}, sl.p{i+1})` of the one file, on key C1 + `i`, at its own root (its pitch), in its
own group `g{i}` (its envelope). Everything a slice carries of its own is a zone's or its
group's — pitch (`z.tune`), gain, pan, reverse (`z.rev`), choke (`z.excl`), envelope
(`g{i}.attack…release`), and a low-pass of its own (`z.lpf`, schema 55). The engine plays it
as any Exemplar: nothing new on the audio thread, every Exemplar test holding. At most 64
slices (one group each).

### 2. The points are in the project (schema 55)

`sl.n`, `sl.p{i}` (frames), `sl.len`, `sl.rate`, `sl.bpm`, `sl.mode`, `sl.sens`, `sl.grid` are
Exemplar parameters (`cantus_core::sectio`), so moving a point is a command with an exact
inverse and the zones are rebuilt from them. Schema 55's migration writes nothing (no older
file holds them); an older build must refuse a v55 file.

### 3. Where to cut

`cantus_dsp::sectio`: the WAV's `cue ` points (else its `smpl` loops' starts), the ACID
chunk's beats for the tempo; else SuperFlux onsets (`onset.rs`) within `6 + 40·sens` dB of the
strongest, never two within 30 ms; or a grid at the tempo (`beat.rs` when nothing says it),
`sl.grid` steps a beat; or by hand. Slice 0 always starts at the file's first frame.

### 4. The dump is exact at the loop's tempo

When the tempo is known every point is moved to the sample the engine starts a note on for
the nearest tick at that tempo (`cantus_engine::transport::sample_at`); the dumped clip puts
slice `i` at that tick until the next. Played at the loop's tempo, from the clip's start, the
slices meet sample for sample: the render equals the file to the bit (the test). At another
tempo the notes move with the grid — slice replay — and each slice plays at its pitch.

### 5. Three gestures, one undo step each

`cantus_core::sectio_ops` — Random (the slices shuffled, the times kept), Stutter (some
slices retriggered, seeded), Quantize with swing — each a pure, seeded function; the app sends
each as one `Command::Batch`.

### 6. Where a loop comes in

The sample browser's menu on a file (*Slice into a new Exemplar track*, *Slice into the open
Exemplar*), an Exemplar's *Import ▸ Slice a loop…*, a loop dropped on a sliced Exemplar's
*Slices* card (one file; the drop is the card's, so no other handler imports it too) and the
agents' `slice_sample` — each one undo step. An empty Exemplar keeps its keyboard map: the
card shows once there are slices. Each slice is its own zone *and its own group* (key C1 + i,
group i), so routing slices to separate outputs is a per-group output index on drums's
multi-output contract (`process_buses`, ADR 0100) — designed for, not built.

### 7. The editor: points are commands, zones follow them

On the card: drag a point (it lands on the nearest zero crossing, within 2 ms), right-click
one to remove it, double-click the waveform to add one; *Slice again* by markers, transients
(sensitivity) or a grid (1/4 … 1/32), previewed on the loop and kept with *Apply*. Each is one
`Command::Batch` that sets the instrument with its zones rebuilt from the points (a slice that
keeps its index keeps its own pitch, gain, pan, reverse, choke and low-pass). The zone low-pass
is a column of the zone table.

### 8. Slicing styles: presets that cut the user's own loop

Six styles — *Drum Break*, *One-Shots*, *Stutter Grid*, *File Markers*, *Vocal Chop*,
*Melodic Phrase* — are Exemplar presets in the library (`sl.style`, tagged by role and style
and *Slicing style*). Applied to a sliced Exemplar, a style re-cuts that Exemplar's own loop
its way (mode, sensitivity, grid) and plays it its way (each slice choking the last or not,
one-shot or gated, its envelope, legato) in one undo step (`styles::through_style`, beside the
dancer's hook in `apply_inst_spec`). On an empty Exemplar it plays built-in sounds (a kit on C1
up, the choir, a plucked string) so the browser can audition it; no audio ships.

### 9. Files from anywhere are not trusted

`slice_sample` is a *Files* tool: its path is resolved on disk (`resolve_touch`) and refused
outside the project's folder and the folders the user allowed; the resolved path is what is
read and stored. A WAV's `cue `/`smpl` counts are bounded by their chunk's size and its `data`
length by the bytes the file holds; an ACID chunk's tempo is taken from its beats only when it
agrees with the tempo written (within 2 %), from its beats alone only for a loop (a one-shot's
beats are a default), and only inside 30 … 300 BPM. A dump with no tempo uses the song's, or
says there is nothing to dump.

### 10. What the integrated review changed (round 37)

- **One preset path** (`views::inst::preset_target`): the editor, A/B, the library, the
  agents' `apply_preset` and the browser's audition all take it. A style re-cuts only as the
  library ships it (no `sl.n` of its own), by **its own** mode; A/B, a saved user preset or an
  undo carry their points and every slice's settings and are applied exactly. The audition of
  a style on a sliced Exemplar re-cuts the user's loop on the audition's worker (it reads the
  file) — what *Load* gives.
- **Each slice's settings follow its audio**: after an add, a remove or *Apply*, a slice takes
  them from the old slice its start lies in. A style's choke and trigger are set when the
  style is applied or a new loop cut, never again on a point edit.
- **64 slices at most, over the whole loop**: transients keep the strongest 64; a grid is
  made coarser until it fits (the status says so); markers past the 64th are counted.
- **A grid needs a tempo**: a loop with none of its own borrows the song's (said). Edited
  points land on a zero crossing and then on the sample the dump plays the tick at.
- **2²⁴ frames** (5 min 49 s at 48 kHz) is the longest loop (points are `f32` parameters):
  longer is refused, in both languages, for the UI and the agents. A sample that is not a
  number is silence to the analysis.

## Consequences

- A sliced loop is an Exemplar: every Exemplar feature works on slices with nothing new on
  the audio thread; the zone low-pass (`z.lpf`) is the one engine addition — decided at the
  trigger (the zone's filter alone when the instrument's is off; under the instrument's only
  when that one is a low-pass), off at 0 Hz: every Exemplar without one renders as before.
- The points live in the project (schema 55); old files have none and open unchanged.
- **Measured** (a point within 10 ms of a hit, the slicer's default sensitivity): see *What
  the tests hold*: **91.9 %** of the rendered set's hits (99.6 % of the points on a hit),
  **75.8 %** of MDB Drums' 5 956 hand-annotated hits (80.3 % of the points; 88 % within
  20 ms, 91 % within 50 ms). **The brief's 95 % within 10 ms is not met**, and the reason is measured:
  on MDB Drums' hand annotation the points that find a hit lie a median 3.5 ms *before* it,
  a quarter more than 7.7 ms before, a twentieth 18 ms before (the detector finds where the
  attack starts; the annotators marked where it is heard) — so a tolerance of 10 ms counts as
  misses hits that are found; within 50 ms 91 % are. Three refinements were measured and two
  dropped: banded picking 73.9 → 75.8 % (kept), adaptive whitening 58.6 %, backtracking to
  the detection function's minimum 44.6 % (it moves early points earlier). The styles that
  stay far under: brushed and ridden jazz (MDB's swing, bebop, free jazz: 43–65 %), whose soft
  hits the detector does not report at all at any sensitivity. Moving every point later to
  meet the annotations would cut each slice's attack, so it is not done.
- Hand-annotated CC0 loops are the owner's to annotate (`docs/beta-checklist.md`).

## What the tests hold

- `cantus-dsp` `sectio::tests` (9): RIFF `cue `/`smpl`/`acid` laid out by hand from the
  published layouts; clicks at known frames; the grid. `tests/sectio_markers.rs` (3): WAVs
  written by **libsndfile 1.2.2** (cue points, sampler loops, a float file with both) — an
  outside writer's bytes, provenance and hashes in `tests/data/sectio/README.md`.
- `cantus-core` `sectio`/`sectio_ops`: the parameters, the zones, the dump's ticks; Random,
  Stutter, Quantize pure and seeded.
- `cantus-ui` `tests/sectio.rs`: markers and an ACID tempo, transients, **the dump renders
  the file to the bit** at its tempo (from bar 1 and shifted), a tempo change keeps the
  slices on the grid, Random/Stutter/Quantize **one undo step each**, and **the end-to-end
  test**: a loop dropped through the app (`CantusApp::assemble`, the manual engine) becomes
  a sliced Exemplar in one step, its dump plays live, equal to the offline render to the
  bit. `sectio_accuracy.rs`: fifty loops rendered by Cantus's own instruments (exact truth).
  `sectio_mdb.rs` (ignored, needs the dataset): MDB Drums' hand annotation.
  `sectio_view.rs`: the card in both languages, shots in both themes. `ai_mcp_boot`'s
  tools/list golden holds `slice_sample` and `dump_slices`.
- `cantus-project` `tests/sectio_schema.rs`: the newest fixture's sliced loop comes back
  whole; no file before schema 55 holds a slice.
