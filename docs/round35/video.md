# Round 35 — video (ADR 0088)

The builder of video export. This file grows with each delivery.

## First delivery — `cantus-video` (2026-10-31)

### What was done

A new GPL crate, `crates/cantus-video`, beside `cantus-export`:

- `mp4.rs` — our MP4 writer: one H.264 track (`avc1`/`avcC`, `colr` BT.709 limited, `pasp`,
  `stss`, no `ctts`, no video edit list) and one AAC-LC (or MP3) track (`mp4a`/`esds`) with
  an audio edit list from the encoder's lead to exactly the video's length; the movie on a
  30 000 Hz timescale, so every duration is exact; `moov` in front.
- `mov.rs` — QuickTime MOV with `png ` frames (RGBA straight alpha, depth 32, or RGB) and
  `sowt` 16-bit PCM.
- `seq.rs` — PNG frames (`encode_png`) and the PNG sequence + WAV (`Name/Name_00000.png`…,
  `Name/Name.wav`).
- `mux.rs` — what both containers share: the index written in front without copying the
  file (a reserved region, a `free` box, the `wide`/64-bit `mdat` trick, `stco`/`co64`), the
  sample tables, chunks of consecutive samples, uniform PCM samples.
- `yuv.rs` — premultiplied RGBA to NV12, BT.709 limited range, 16-bit fixed point with
  rounding (the standard's reference colours in a test).
- `pipeline.rs` — frames drawn on worker threads, handed over in order, bounded memory,
  stops on an error or a cancel, a lost worker said.
- `mf/` (Windows, the crate's only unsafe code) — Media Foundation's H.264 and AAC encoder
  transforms driven directly (`Session` starts COM and MF for the thread; "class not
  registered" is `VideoError::NoEncoder` — Windows N).
- `mf/sys.rs` — Media Foundation's five functions looked up in `System32` on first use,
  never imported: a binary importing `mfplat.dll` does not start on Windows N or a server
  without it (the first build did import it — caught by reading the executable's import
  table; a test now holds it).
- `export.rs` — `export(&Request, &dyn FrameSource, path, &Control) -> Done`: checks the
  request in words, writes `name.ext.part` and places it as audio exports do (never
  replacing unless asked; a sequence goes into `Name.part/` and is renamed), progress and
  cancel through `Control`, nothing left behind when stopped or failed.
- `timing.rs` — frame `n` at `n/fps` exactly; frame `n`'s audio `⌊n·rate/fps⌋…`; 24, 25, 30,
  50, 60 fps; 44.1 and 48 kHz.
- `examples/measure.rs` — the 30-second measurement with the stand-in character.

### How it was tested (39 tests in `-p cantus-video`)

- **Decode-back sync** (`tests/sync.rs`, FFmpeg as a test tool only — `CANTUS_FFMPEG`,
  never in the repository; without it the check says so and stops, `CANTUS_REQUIRE_FFMPEG=1`
  makes that a failure): a 12-second 480×854 clip with each frame's number in binary blocks,
  a flash on every beat and a click on the same samples. MP4 (Media Foundation) at 30 and
  60 fps, MOV with alpha at 30, opaque MOV at 60: every frame decoded once, in order, at
  `n/fps` (±0.1 ms); every flash where it belongs; **every click on its flash at 0.000 ms**
  (half a frame allowed); the click at time 0 present and whole; the audio exactly the
  video's length; MP4 frames only I and P, High profile, `tv, bt709`, `moov` before `mdat`;
  PNG frames bit-exact.
- **Windows' own decoders** (`tests/mf.rs`): one access unit a frame, key frames at 0 and
  60 (two seconds), the AAC config as stated; Windows' player stack plays every frame at
  its time and every click 21.333 ms late — exactly as late as FFmpeg's own AAC MP4 of the
  same clip (it ignores audio edit lists; ADR 0088 amendment 1).
- **Nothing of Media Foundation imported** (`tests/mf.rs` reads its own executable's
  import table); the tests needing the encoders skip, said, on a system without them unless
  `CANTUS_REQUIRE_MF=1` (run here with it).
- **Containers read back without FFmpeg** (`tests/writers.rs`, every system): every MP4
  table checked and every sample's bytes found where the index says; the MOV's PNG frames
  and PCM bit-exact; the sequence's names, frames and WAV; refusals in words (odd size, no
  frames, 29 fps, 96 kHz); a file there kept unless replacing; a stop leaves nothing.
- Unit tests: BT.709 colours, chroma averaging, demultiply, the pipeline's order, bound,
  errors and per-worker state, box reading, `stsc` runs, the index moving the samples up,
  NAL parsing and `avcC`, AAC configs, the audio cuts never drifting.
- Clippy `-D warnings --all-targets` and `fmt` on Windows; the Linux check (workspace
  clippy, `cantus-video`'s tests) in WSL.

### The numbers (30 s, 1080×1920, the stand-in through `cantus_dance::render`)

| Output | Wall | Size | Peak memory |
|---|---|---|---|
| MP4 30 fps (H.264 + AAC 192 kbit/s) | 3.8 s (8× real time) | 7.1 MB (1.9 Mbit/s) | 449 MB |
| MP4 60 fps | 7.5 s | 11.8 MB | 449 MB |
| MP4 1080×1080 / 1920×1080, 30 fps | 2.2 s / 3.6 s | 4.4 MB | 296 / 449 MB |
| MOV PNG RGBA + PCM, 30 / 60 fps | 2.2–3.0 s / 4.9 s | 79 / 153 MB | 149 MB |
| PNG sequence + WAV, 30 fps | 2.4 s | 79 MB | 149 MB |

MP4 picture quality against the lossless render: PSNR 44.5 dB mean, 42.2 dB the worst
frame. Media Foundation's worker threads capped at 8 (its default peaked at 646 MB for the
same speed). Frames looked at: `docs/round35/video/` (MP4 at 1.5 s and 7.25 s, a 100 % crop,
the MOV's alpha frame).

### Marked lines

- `Cargo.toml` (root): `"crates/cantus-video", # r35:video …` in `members` and in
  `default-members`; `cantus-video = { path = "crates/cantus-video" } # r35:video` in
  `[workspace.dependencies]`.

## Second delivery — every path, the song's video, the dialog, agents, packaging (2026-11-01)

### What was done

- **Linux / Windows N** (`openh264.rs`, `audio_alt.rs`, `select.rs`): Cisco's OpenH264
  fetched by the content manager over https from `ciscobinary.openh264.org` (the `.bz2`'s
  and the library's SHA-256 pinned for linux x86-64/arm64 and windows x86-64; unpacked by a
  pure-Rust bzip2 decoder with a 16 MB bound; Cisco's words and licence in the dialog; an
  off switch); AAC from the system's FFmpeg or GStreamer through pipes (ADTS taken apart);
  else MP3 in the MP4 with the warning. `select::plan` says which encoders an MP4 will use
  before it starts (the dialog shows it).
- **WebM** (`av1.rs`, `vorbis.rs`, `webm.rs`): rav1e at a constant quantizer, Vorbis with
  each packet timed from the setup header's modes, our streaming WebM writer (clusters at
  key frames, cues, SeekHead, sizes patched).
- **macOS** (`apple/`): VideoToolbox H.264 and AudioToolbox AAC through self-contained FFI,
  compiled for both Apple targets here; row 18 of `docs/mac-session.md`.
- **The song's video** (`dance.rs`; `cantus-ui/src/video_export_job.rs`): the export's own
  render with the dancers' units keeping their timelines (the dancer's
  `RenderProgress::dance`), resampled to 48 kHz when the session runs at another rate,
  every frame sampled with `cantus_engine::dance::sample`; a project without dancers gets
  the default character dancing to the beat; the title drawn as vector glyphs (`title.rs`,
  skrifa outlines filled by tiny-skia).
- **The dialog** (`cantus-ui/src/video_export.rs`, `views/video_export*.rs`; File ▸ Export
  video…): the live preview of the frame at a scrubbed time (drawn by the renderer at the
  preview's pixels, the title and background as in the file, a checkerboard under
  transparency), Picture (format, shape with other sizes, 30/60 fps, quality), Look
  (stage / transparent / three keys / black, with the notes on alpha and on 4:2:0 keys;
  title), What (song, selection, loop, and its length), Encoders (the plan in words,
  Cisco's switch, download and progress, the system-AAC switch, the MP3 warning, the line
  on Windows' player and AAC), an estimate of time and size, progress with time left,
  Stop, how it ended. Choices remembered in the settings.
- **Agents** (`ai_tools_video.rs`): `export_video` (Files class, asked, never overwriting
  unless told, the dialog's own start) and `get_video_export`.
- **Packaging and CI**: rav1e's assembly on in releases (`av1-asm` → `cantus-ui/video-asm`
  → `cantus/video-asm`; `xtask package` refuses without nasm; nasm from Chocolatey,
  apt and Homebrew in `release.yml`, `packaging/linux/deps.sh`, `build-dmg.sh`); nothing
  new shipped (OpenH264 is downloaded, FFmpeg only ever the user's); `cantus-video` is a
  workspace member, so CI builds and tests it on all three systems.
- **Found and fixed**: Media Foundation's encoders probed on a thread with no session were
  released after `CoUninitialize` (a tail expression's temporaries outlive the block's
  locals) — an access violation the dialog's test caught; a test now asks on a bare thread.

### Tested (the numbers)

- Decoded back (FFmpeg), every click on its flash: **0.000 ms** for MP4 (Media Foundation)
  at 30 and 60 fps, MOV at 30 and 60, **WebM at 30 and 60**, **OpenH264 + MP3** and
  **OpenH264 + FFmpeg AAC** (`tests/sync.rs`, `tests/alt_audio.rs`); a hostile download
  (a proxy page, wrong bytes) refused and removed.
- **The song's video** (`tests/song.rs`): an automation clip of `move` over bars 3–4 claps
  on frames **120…239** exactly at 30 fps; the frames in the file are bit for bit the
  screen's pictures at the same moments.
- **rav1e's assembly**: 2.1–2.3 fps without, **6.0–6.6 fps with** at 1080×1920 (2.8×),
  byte-identical streams.
- **Linux (WSL, the same branch)**: workspace clippy `-D warnings` and fmt clean; Cisco's
  linux64 library installed from its `.bz2` and decoded back at **0.000 ms** with MP3 and
  with FFmpeg's AAC; MOV and WebM at 0.000 ms; the dialog's tests pass headless.
  GStreamer with gst-libav was on neither machine: its path is said and skipped.
- The dialog (`cantus-ui/tests/video_export.rs`): renders in both themes and languages for
  every format, the preview keeps the shape, an export lands as a file and is remembered,
  a stop leaves nothing, a missing selection says why, the tools are registered, every key
  has words in both languages. Shots: `docs/round35/video/dialog-*.png`.

### Marked lines (besides the first delivery's three in the root `Cargo.toml`)

- `cantus-ui/src/lib.rs`: `pub mod video_export; // r35:video …`
- `cantus-ui/src/views/mod.rs`: `pub mod video_export; // r35:video …`
- `cantus-ui/src/app.rs`: the `video` field, its `Default::default()`, and
  `crate::video_export::frame(self, ctx); // r35:video …` after the audio export's dialog.
- `cantus-ui/src/settings.rs`: the `video` field (two lines) and its default.
- `cantus-ui/src/views/transport_menus.rs`: `crate::video_export::menu_item(app, ui);`
- `cantus-ui/src/ai_tools.rs`: the `#[path]` module (two lines) and `video::TOOLS,`.
- `cantus-ui/Cargo.toml`: `cantus-video.workspace = true` and the `video-asm` feature (two
  lines); `cantus-app/Cargo.toml`: the `video-asm` feature (three lines).
- `xtask/src/package.rs`, `.github/workflows/release.yml`, `packaging/linux/deps.sh`,
  `packaging/macos/build-dmg.sh`: nasm and the feature (marked `r35:video`).
- `docs/manual/{en,es}/README.md`: the link to `video.md`; `docs/mac-session.md`: row 18.

### For the integrator

- `cantus-ui --lib`: two of the stage's tests (`stage::geom::tests::a_box_draws_what_the_whole_area_draws`,
  `the_body_stays_put_while_the_dancer_jumps_and_spins`) fail on this branch after
  merging `r35/integration` at f0a3d752; the video code does not touch `stage_geom.rs`.

### Only the owner

- Upload a test MP4 (vertical, 30 s) to TikTok, Instagram, YouTube; import the MOV layer in
  the editors you use (Resolve, Premiere, CapCut); the Mac session's row 18.
- The legal review gains Cisco's OpenH264 binary terms.

## Review 1 — fixed (2026-11-01)

Each with a test that fails without the fix:

1. **One render at a time**: `export_in_progress()` counts the video export (marked line in
   `app.rs`), so freezing, bakes, the frame ticker and the audio export go by it; the video
   export yields a background freeze refresh and waits for no bake, as the audio export
   does (`views/export.rs` start goes by `export_in_progress()`, marked line). Test:
   `a_video_export_holds_the_render_as_an_audio_export_does`.
2. **Losses said**: the end of a video export reads `render_failures()` before
   `finish_render()` and fails or warns as the audio export does, with the late warped clip
   said. Test: `a_plug_in_lost_in_the_render_is_said_not_finished_silently`.
3. **nasm in every job that builds `video-asm`**: `macos.yml`'s dmg job installs it; xtask's
   `every_job_building_the_av1_assembly_installs_nasm` reads every workflow's jobs.
4. **No blocking on the interface**: Cisco's library is hashed once for each size and
   modification time (`a_library_is_hashed_once_until_it_changes…`); the encoders are found
   on a worker, the dialog saying "checking…" (`the_encoders_are_found_off_the_interface_thread`).
5. **No console windows**: every program started (`ffmpeg`, `gst-*`) has
   `CREATE_NO_WINDOW` (`every_program_started_here_opens_no_window`).
6. **The title reads**: its ink is chosen by contrast with the background (dark over Paper's
   stage), with an outline of the other colour; ≥ 4.5:1 over every background offered in
   both themes (`the_ink_reads_over_every_background_offered`,
   `the_title_reads_over_every_background_in_every_theme`).
7. **Nothing of the user's touched**: the work in progress is `.<name>.cantus-part-<pid>-<n>`,
   made new; a user's `x.mov.part` or `Mix.part` is never opened or removed
   (`what_the_user_keeps_beside_the_target_is_never_touched`).
8. **The real file asked about**: the save dialog's name with the format's extension added
   is asked about when it is there (`must_ask`). The audio export already asks in the same
   case (`choose_and_start`: `right != path && right.exists()`), so it is unchanged.
9. **Cisco in the About box** while its library is installed (marked line in
   `views/about.rs`); switched off with the library present, the dialog offers *Remove
   OpenH264* (`Install::remove`, tested).
10. **The estimate is this machine's**: each export keeps its pace (ms a megapixel-frame, by
    format) in the settings and the estimate goes by it, said rounded ("a few seconds",
    "about 20 s", "about 2 min") — `the_pace_is_measured_and_the_estimate_is_honest`.

## Review 2 — fixed (2026-11-01)

1. **Encoders only for an MP4, never on the interface's thread**: `start` asks for the plan
   only when the format is MP4 and hands it to the job; `plan_now` waits for a worker's
   answer (the agent tool too). Test: `only_an_mp4_asks_for_encoders_and_never_on_this_thread`.
2. **No work in progress left behind**: `close_session` stops a video being made and waits
   for it to remove its part (marked line in `recovery_session.rs`); each export sweeps the
   parts beside its target whose process has ended (`.<name>.cantus-part-<pid>-<n>` only).
   Tests: `closing_stops_a_video_and_removes_its_part`,
   `the_parts_an_ended_export_left_are_swept_and_only_those`.
3. **One busy rule, said in words**: the bake (`views/modulator_bake.rs`), the audio export's
   start (`views/export.rs`), the agent's `export_audio` (refused before any folder is made,
   `agent_export.rs`) and `get_selection`'s `busy` (`agent_tools.rs`) go by
   `export_in_progress()`, naming the export that runs (marked lines). Test:
   `an_audio_export_during_a_video_is_refused_in_words`.

## The integrated tree's review — fixed (2026-11-02)

- **#2 Imported characters**: an export resolves every dancer's character as the screen
  does; one still being read is waited for (up to 30 s, the dialog and `get_video_export`
  saying "Loading the characters…") and then drawn; one that cannot be read stops the
  export in words naming it (the agent tool too). Tests:
  `an_import_still_loading_is_waited_for_and_drawn`,
  `a_character_that_cannot_be_read_stops_the_export_in_words` (`tests/video_cast.rs`).
- **#4 The preview is the file's frame**: it renders a few seconds of the song around the
  scrubbed moment through the export's own offline render (automation, notes, modulators;
  hosted plug-ins left out), on a worker, debounced while the bar is dragged; until it is in
  the dancers dance to the beat. The picture's cache key includes whether that render and
  every character are in. The manual says so. Test:
  `the_preview_is_the_moments_own_render_with_its_automation`.
- **#6 Entry points**: `Action::ExportVideo` ("export-video", unbound, File; marked lines in
  `keymap.rs` and `app.rs`'s `run_action`) — in the palette; the public entry for the
  dancer's window and the stage is `cantus_ui::video_export::open(app)`. Test:
  `export_video_is_a_palette_action_that_opens_the_dialog`.
- **#7 Hidden dancers** (`view.mode` 2) are left out of the video and the preview, as on the
  stage. Test: `a_hidden_dancer_is_left_out`.
- Docs: the manual's video page links the dancer, stage and import pages and says how the
  preview is made and which dancers show; `README.md`'s crate table lists `cantus-video`;
  `docs/DEVELOPING.md`'s tool table has nasm with its install lines.

## The fix rounds' review — fixed (2026-11-02)

- **#1 The preview never blocks the interface**: this thread only copies the project; the
  compile (`Snapshot::compile`, without the export's waits for audio, pitch and warp renders)
  and the short render run on one preview worker. At most one runs: a newer moment stops
  the older through the render's cancel flag and starts once it has ended; closing the
  dialog or another project stops it; a render that fails (or a worker that cannot start or
  panics) ends in words — "preview unavailable: …" in a chip over the preview — and the
  repaint stops; while it renders the dancers dance to the beat under a "rendering…" chip.
  Tests: `the_preview_never_compiles_or_renders_on_the_interfaces_thread` (every frame under
  a second while it renders; the compile on `cantus-video-preview`),
  `scrubbing_keeps_one_render_and_closing_cancels_it`.
- **#5 A waiting export belongs to its project**: tied to `CantusApp::epoch`; New or Open
  meanwhile drops it, said ("The video export was asked for another project: not made.").
  Test: `a_waiting_export_is_dropped_when_the_project_changes`.
