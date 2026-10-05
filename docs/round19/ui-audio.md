# Round 19 — ui-audio: audio clips you can see and shape

Builder: **ui-audio** (`r19/ui-audio`). ADR 0025 (audio clips), ADR 0024 (the kit).

## What is done

| Area | What |
|---|---|
| Peaks (`waveform.rs`, `waveform_store.rs`, `waveform_disk.rs`) | `SourcePeaks`: per-channel and mono-summed min/max at three levels of detail (64, 512, 4096 frames per peak; each level built from the last). `columns()` folds the coarsest level still finer than a screen column, so any zoom costs one pair per pixel. `PeakStore` (on `CantusApp::peaks`) decodes on a worker thread through the engine's decoders (`SampleCache::load`: WAV, AIFF, FLAC, Ogg), keeps peaks for the session, and writes `<data>/cache/peaks/<key>.peaks` (FNV-1a of path + size + mtime, `i16` pairs) so a reopened project draws at once. A missing file is known without queuing anything. An import hands its decoded samples straight to the worker (no second read); a file moved by *save* takes its peaks along (`follow`). `paint_peaks` keeps the contract signature and draws through `fill_peaks`: one strip of the brush's mesh, two vertices a column. |
| Audio clips in the arrangement (`views/arrangement_audio*.rs`) | The waveform in the clip's own ink, a column per physical pixel, the clip's gain and fades applied to the drawing; a stereo file shows two halves when the row is ≥ 44 pt. Fades drawn over the waveform: the part a fade takes away veiled, the curve as a line; overlapping fades shortened in proportion as the engine plays them. **Handles** (selected or hovered clips): a square at each top corner for the fades (drag sets the fade; double-click removes it; right-click picks the curve), a diamond on the gain line at the top centre (drag up/down 0.2 dB/pt, Shift 0.02; double-click resets; the line shows where a full-scale peak lands). The value pill shows ms/s and dB while dragging (the hint system). Badges in the title bar: clip gain (`+2.0 dB`) and `REV`; the name elides before them. A **missing** file draws a dark body, a dashed outline in the danger colour and `MISSING  file.wav`. Handle geometry is published: `ArrangementView::audio_handles(clip)`. Inspect mode registers the clip body. |
| Clip menu | For an audio clip: the loop entry is gone (audio plays once); a section with the fade-in and fade-out curves as two rows of drawn keys, *Normalize*, *Reset clip gain*, *Reverse* (checked), *Remove fades*, *Rename clip*, and *Show in file browser* — or *Locate file…* when missing; colours below as before. Every change is `SetClipAudio`, one undo step (a drag merges per clip, closed by `end_gesture`); a selection is edited as one batch. |
| Normalise | Each selected audio clip's loudest sample *inside its window* to −0.1 dBFS through clip gain, from level 0 of the peaks (exact sample peak). |
| Split / glue / trim / slip / move / copy | Split keeps fades at the outer edges (left keeps the fade in, right the fade out). Glue joins audio only when the clips are **contiguous windows on one source** (each starts where the last ended, on the timeline and in the file), same gain and direction, same lane; the result takes the last clip's fade out. Trims and slips are clamped live so a window never leaves its file (`audio_clips::clamp_edit`). A **reversed** clip maps its window backwards — split, trims and slips follow that (`reversed_edit`, `split_audio`). The loop switch is refused for audio. Move, copy, duplicate, paste already carried `audio` (contract). Fixed on the way: a left-edge trim's *preview* drew note clips' notes shifted (the offset did not follow in the preview). |
| Import | Drop WAV/AIFF/FLAC/Ogg from the system: on a track row the files land on that track and the ones below at the snapped tick under the pointer; below the last track or off the arrangement, on new tracks (named after the file, a free strip each, the next palette colour). *File ▸ Import audio…* puts each file on a new track at the playhead's bar. Each file becomes an `AudioSource` (`recorded: false`, its rate/channels/frames) and a clip as long as the file at the project's tempo. The whole import is one undo step (`cmd.import_audio`). MIDI and DAWproject drops keep working; the drop hint names audio too. MP3 is not offered (no decoder in the tree). |
| Saving with audio (`cantus-project/src/collect.rs`, `audio_files.rs`) | Before the file is written, every source outside the project's folder is brought into `<folder>/Audio/`: a recording not inside a saved project is **moved**, an import (or a file of another project's folder: *Save as* elsewhere) is **copied**; names kept and made unique (`Kick (2).wav`); one file used by two sources is copied once; nothing is ever overwritten. Paths are rewritten relative (`Audio/x.wav`) by one `SetAudioSourcePath` batch (`cmd.collect_audio`). A missing file is left alone; when the project changes folder its relative path is written absolute so it is found if it comes back. *File ▸ Collect audio and save* runs the same for an existing project. |
| Missing files | Drawn as missing in the arrangement and the inspector; *Locate…* (clip menu, inspector) picks the file, checks it decodes, and relinks by `SetAudioSourcePath` — every clip of the source follows; one undo step. |
| Inspector (`views/inspector_audio*.rs`) | Under the clip's rows (whose *Loop* is dimmed and note count hidden for audio): an **overview** of the whole file with the clip's window lit and the rest veiled; *File* (name, a danger dot when missing), *Format* (`48.0 kHz · stereo · 0:08.00`), *Path* (elided in the middle, full in the hint), *Show in file browser* / *Locate file…*; *Clip gain*, *Fade in* and *Fade out* (dragged values, ms/s), each fade's *Curve* as five drawn keys, *Reverse*, *Normalize*. |

## Tests

- `crates/cantus-ui/tests/audio_clips.rs` (+ `audio_clips/gestures.rs`, `audio_clips/files.rs`), 14 tests:
  peaks of a known stereo file at each LOD (pairs, mono sum, columns, peak); the disk
  cache gives back what was computed; a missing file is known at once; import makes the
  source and a clip of the right length at 120 and 90 BPM and undoes in one step; files
  dropped on a track land on it and the ones below; **a fade-in drag through the
  published handle geometry sets a beat of fade and undoes in one step**; fade-out and
  gain drags (−6 dB for 30 pt) are a step each; **normalise sets the gain the peaks call
  for**; split keeps the fades outside and glue gives the clip back, curves and all; glue
  refuses windows that do not continue each other; a trim stops at the end of the file;
  **save moves a recording, copies an import, writes relative paths, and a reopened
  project finds and reads them**; **a missing file paints as missing and relinks** (and
  undo puts the path back); the inspector shows an audio clip.
- `crates/cantus-project/tests/collect.rs`, 4 tests: move vs copy and relative paths, then save →
  load → resolve; names made unique and one file copied once; *Save as* to another folder
  copies the audio along; a missing file is left alone and kept findable.

## Files touched outside my own

- `crates/cantus-ui/src/app.rs`: the `peaks` field (+ its default, + the disk cache dir
  in `assemble`); `save_to_path` calls `collect_audio` first; `commit_clip_drag` routes a
  reversed audio clip's trims/slips to `audio_clips::reversed_edit`; `reveal` made
  `pub(crate)`.
- `crates/cantus-ui/src/clips.rs`: `split` calls `audio_clips::split_audio`; `glue`
  hands audio to `audio_clips::glue`; `set_loop` skips audio clips.
- `crates/cantus-ui/src/views/arrangement.rs` (the clip body, as agreed with ui-rec): the
  `audio` module and state, `Hit::handle`, the audio paint call in `clips()`, handle
  gestures in `timeline()`, the clamp of trims/slips, the clip menu's audio section,
  `slip_preview` now also previews the offset of a left trim (and of reversed clips),
  `normalize_selection`, re-exports (`AudioHandle(s)`, `curve_icon`, `CURVE_KEYS`).
- `crates/cantus-ui/src/views/arrangement_paint.rs`: `ClipLook::reserve` (room for the
  badges) and `title_height`.
- `crates/cantus-ui/src/views/inspector.rs` (+ `inspector_track.rs`): the `audio`
  section after the clip's rows, `Act::Audio`; *Loop* dimmed and note count hidden for
  audio clips.
- `crates/cantus-ui/src/views/interop.rs`: dropped audio files go to
  `audio_files::drop_audio` before the MIDI/DAWproject handling; the drop hint and the
  unsupported message use `audioclips.*` strings.
- `crates/cantus-ui/src/views/transport.rs`: *Collect audio and save* and *Import
  audio…* in the File menu (stock `ui.button`, like the rest of that menu).
- `crates/cantus-ui/examples/shoot.rs`: the `audio` kind (`examples/shoot_audio/scene.rs`).
- `crates/cantus-project/src/lib.rs`: `pub mod collect`.

## Notes for the other builders and the integrator

- **engine-rec**: stored paths are relative to the project file's folder once saved. The
  app must tell the cache: when `SampleCache::set_project_dir` lands, call it with
  `app.audio_base()` on open, save (after `collect_audio`), *Save as* and new project.
  `waveform::resolve` / `collect::resolve` are the same rule. A reversed clip plays
  `offset .. offset + length` backwards (content at `start + d` is `offset + length − d`):
  the UI's split/trim/slip assume exactly that.
- **ui-rec**: `waveform::paint_peaks` keeps its signature (now one mesh). To draw a take
  lane clip like a lane-0 one, the arrangement's `audio::paint` is the body painter;
  when a take finishes, `app.peaks.insert_data(path, data)` avoids reading the file again
  (or the first lookup reads it). Recorded sources must be `recorded: true` with an
  absolute path until the first save — `collect_audio` moves them into `Audio/`.
- **Piano roll** (whoever owns it next): selecting an audio clip still opens it in the
  piano roll, where notes can be drawn that never play (audio clips fire no notes). It
  should show the clip as audio instead.
- Undoing a *Collect audio files* step after a **move** points the source back at the
  scratch location (the file is no longer there: it shows as missing; redo or *Locate…*).
- Autosave writes the pool as it is; a relative path in an autosave of a saved project is
  relative to the project's folder, not the autosave's.

Run on the branch: `cargo test -p cantus-project -p cantus-ui` all green (cantus-ui: 125
unit + every integration file, including `app_flows`, `kit_lint`, `views_budget`, the
i18n test); `cargo clippy -p cantus-project -p cantus-ui --tests --examples` clean;
`cargo fmt` applied.

## Looked at

`cargo run -p cantus-ui --example shoot -- <dir> 1 audio` writes
`audio-arrangement-{slate,paper}.png` and `audio-inspector-{slate,paper}.png` from a
synthesised scene (drum loop with fades, a vocal at −4.5 dB with an S-curve fade in and
an equal-power fade out, a tall stereo pad, a reversed bass, a missing guitar take). Fixed
from the first renders: the fade veil was invisible (now the canvas token: darker on
Slate, washed out on Paper), handles disappeared on Paper's pale bodies, the gain badge
read `DB`, and the inspector's buttons had grey text on a grey cap.

## What is left

- Playback of audio clips is engine-rec's (parallel); nothing here plays sound yet.
- A crossfade gesture between two overlapping clips (the model has no overlap on a lane;
  fades are per clip) and per-clip gain envelopes.
- MP3 import (no decoder in the tree).
- Drag-and-drop *from* Cantus's own browser panel (only OS drops are handled).
- A keymap action for *Import audio…* (the File menu calls it directly, as its other
  entries do).
- The piano roll should refuse audio clips (see the note above).
