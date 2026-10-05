# Round 19 briefs — the UI side

Both UI builders depend on the engine builders' work, which lands later. Code against the
contract's signatures (`cantus_engine::recorder`), test through `EngineHandle::manual`
and the commands, and keep the UI honest when the stubs return nothing (no input device:
say so, offer the settings).

## ui-rec — recording, as the best recording DAWs do it

The bar: Pro Tools, Cubase, Logic, Studio One, Reaper, and FL Studio's look.

- **Audio tracks.** *Add audio track* (arrangement's add menu, the track context menu, a
  key): a track with `input: Some(mono 0)` (or the first stereo pair on a 2-in device),
  monitor Auto, a microphone icon, no note clips. Any track can still take an input.
- **Track header** (`views/arrangement_paint.rs::header` and the header's input
  handling): record-arm button (a round red lamp, lit when armed), monitor button
  cycling Off / Auto / On (a speaker glyph with a state), input picker (In 1, In 2, …,
  In 1-2 … from `EngineHandle::input_channel_count`, and *No input*), and a small input
  meter on armed tracks (`Meters::inputs`). Shift+click arms exclusively; arming many
  tracks records them all at once (multitrack recording).
- **Mixer strip**: FL's input menu and arm on every strip whose tracks have an input
  (STATUS lists it as blocked on this round). **Inspector**: an *Input* section on the
  channel sheet (device channel(s), trim, monitor mode, arm).
- **Transport**: the record button records every armed track — audio from inputs, notes
  from the keyboard/MIDI on armed instrument tracks (the existing MIDI recording keeps
  working when nothing is armed: the selected track). A recording-options popover:
  count-in (off, 1, 2, 4 bars), punch in/out (on/off; the range is a red brace in the
  ruler, dragged like the loop brace, *set punch from selection*), loop takes, take mode
  (Stack, Replace, and Overdub for MIDI), click mode and level. While recording: elapsed
  time, a pulsing record lamp, and dropped-frame warnings (`FinishedTake::dropped`,
  `RecordMark::Overflow`).
- **A pass, start to end**: the recordings folder is `<project folder>/Audio` when the
  project has a path, else a session scratch folder under the app's data dir (ui-audio's
  save step moves recorded files into the project). On record: `begin_recording(dir)`,
  count-in by seeking back N bars and `set_record_start`, play with recording on. While
  recording draw each armed track's take growing in red with its live waveform
  (`poll_recording` → `waveform::paint_peaks`). On stop: `finish_recording`, then **one
  undo step** (`Command::Batch`) that inserts each file as an `AudioSource` (recorded:
  true) and the clips: one per loop pass (`wraps`) — the last pass on lane 0, the earlier
  ones on take lanes 1… — windowed to the punch range or the record start, with the
  count-in and the tail kept as handles. `TakeMode::Stack` moves what the take covers on
  lane 0 to a take lane; `Replace` trims/splits it away. MIDI gets the same count-in,
  punch, loop takes and take modes (`recording.rs`).
- **Take lanes and comping** (ADR 0025 §5; a new file such as
  `views/arrangement_takes.rs`): a disclosure on the header opens the lanes under the
  track (`SetTrackTakesOpen`), each lane a row with its take's clips (draw them with the
  same clip painter lane 0 uses, so ui-audio's waveform body appears there too). **Swipe
  comping**: drag across a take lane to make that range of lane 0 a window on that take
  (split/trim the lane-0 clips there, insert the window) — one undo step per swipe; click
  a take's name to promote the whole take; *delete unused takes*; *explode takes to new
  tracks*. The comped ranges are highlighted on their take lanes.
- Keys (in the keymap file, both locales): arm selected track, record, toggle count-in,
  toggle punch, show takes. Commands from the contract; their undo-fuzz arms are yours.
- Tests: arming/monitor/input commands round-trip and undo; a fake pass
  (`FinishedTake`s you build) becomes the right clips, lanes and windows for loops,
  punch and both take modes, as one undo step; a swipe comp makes the lane-0 windows you
  expect and undoes in one step; the header publishes its button geometry and clicking it
  arms. Render the arrangement with armed tracks, take lanes open and a comp, and look.

## ui-audio — audio clips you can see and shape

- `waveform.rs` (owns it; keep `paint_peaks`'s signature): peaks computed on a worker
  thread per source (min/max at several levels of detail, e.g. 64, 512, 4096 frames per
  peak), cached in memory and on disk next to the user's data (keyed by the file's path,
  size and mtime) so a reopened project draws at once; drawn as one mesh per clip with
  LOD chosen by zoom; stereo shown as two halves when the clip is tall enough; the clip's
  gain and fades applied to the drawing.
- **Audio clips in the arrangement** (the clip body painter in
  `views/arrangement*.rs`, `clips_chrome.rs`): the waveform in the clip's tint, the name
  bar, **fade handles** (small squares at the top corners; drag sets the fade, the curve
  drawn over the waveform, right-click picks the curve), a **clip gain** handle (drag the
  top line; the value pill shows dB), reverse and gain shown in the chrome. Context menu:
  fades and curves, gain reset, normalise (peak to −0.1 dBFS via clip gain, computed from
  the peaks), reverse, *show in file browser*, rename, colour. Everything through
  `SetClipAudio` (one undo step per gesture). Split, trim, slip, move, copy and glue
  already work through the clip window — check they do for audio and fix what does not
  (glue of two audio clips only when they are contiguous windows on one source).
- **Import**: drop WAV/AIFF/FLAC/Ogg/MP3-if-decodable files from the OS onto the
  arrangement (on a track, or below the last to make a new audio track) and *Import
  audio…* in the File menu; each becomes an `AudioSource` (recorded: false) and a clip
  whose length is the file's at the project tempo, snapped. Multiple files go to
  consecutive tracks. Read the format with the engine's decoders (no new decoders unless
  trivial).
- **Saving with audio** (`app.rs` save path, the project crate if it fits better):
  recorded sources still outside the project folder are **moved** to `<project
  folder>/Audio/`, imported ones **copied** there (*Collect and save* also for an existing
  project), paths rewritten relative (`SetAudioSourcePath`), names made unique; *Save as*
  to another folder copies the audio along. Missing files: the clip draws an outline with
  *missing*, the inspector offers *Locate…*, which relinks by `SetAudioSourcePath`
  (and every clip of that source follows).
- **Inspector**: an audio-clip section — source name, format, length, path (elided), gain,
  fades with curves, reverse.
- Tests: peaks of a known file at each LOD; a fade drag through the published handle
  geometry sets the fade and undoes in one step; normalise sets the gain the peaks call
  for; import of a generated WAV makes the source and clip with the right length at 120
  and 90 BPM; save moves a recorded file and copies an imported one, rewrites the paths,
  and reopening finds them; a missing file shows as missing and relinks. Render the
  arrangement with audio clips (fades, gain, a missing one) and look.
