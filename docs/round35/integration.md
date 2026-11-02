# Round 35 — integration: a character that dances on screen

**The ask** (the owner, 2026-10-29, `docs/next-session.md` of round 34): BACKLOG §6i whole —
a character drawn over the interface that dances to the beat (FL Studio's Fruity Dance is
the reference for the *idea*; ours is our own, never its character or art), driven by
automation, modulators, MIDI learn and notes; a family of included characters and
characters people import, checked and sandboxed; the clip exported as a video with the
song's audio for social media or as a transparent layer for an editor; and a companion
while working and on stream — a small always-on-top window OBS captures cleanly, with a
light visualiser, cheap enough to leave on all day. The owner, midway: *"super super
professional, very cool and fun"* — and, after the family and the hero video: *"Se ve bien,
continúa."*

## How

- **Item 0 had nothing**: `macos.yml`, CI and Release on public `782dc960`/`0514b44f` green
  with nothing marked *FLAKY on macOS VM*, no issues on `cantus-daw`, no owner notes, the
  Mac session not done (beta.10 = Windows and Linux).
- **The contract** `b6f03bfe` on `r35/integration` (`docs/round35/BUILDING.md`: ownership,
  marked lines `// r35:<name>`, briefs, *The bar*): `cantus-core/src/inst/dancer.rs` (the
  keys, forever: 16 moves, 8 moods, the move map from C3), `cantus-engine/src/dance.rs`
  (`DanceFrame`, the `DanceFeed` seqlock, the offline timeline), the new GPL crate
  `cantus-dance` (the `Character` trait, the pure `pose_at`, the one tiny-skia renderer, a
  stand-in character, the import seam, `examples/preview.rs`) and
  `cantus-ui/src/dance_texture.rs`. **The design decision at the contract**: the dancer is a
  silent **native instrument** (`InstrumentKind::Dancer`, schema 49), so automation, the
  modulators, MIDI learn, presets, A/B, the catalogue and agents reach it for free; *React
  to* is a follower modulator routed to its `level`; the engine publishes a frame per
  segment; the pose is a pure function of the frame, so the screen, the companion and every
  frame of a video agree.
- **The risk first**: a video probe measured the paths before the video builder started
  (`F:\projects\daw-r35\videoprobe\REPORT.md`) and ADR 0088 was written from it (d4bede88):
  Media Foundation's H.264 and AAC encoders driven directly (not the sink writer: its
  B-frame offset, `moov` at the end, lost AAC priming) into our own MP4 writer;
  VideoToolbox on macOS; Cisco's OpenH264 downloaded on Linux and Windows N with the
  system's AAC or MP3; WebM by rav1e and Vorbis; PNG MOV and PNG sequences for editors; no
  FFmpeg shipped.
- **Five builders** in `F:\projects\daw-r35\{dancer,cast,import,stage,video}`; the slot
  script (`F:\projects\daw-r35\slot.sh`: three build slots and one WSL slot on a 31 GB
  machine). Each delivered in parts merged `--no-ff` into `r35/integration`.
- **Reviews, one agent each, read-only**: one reviewer per branch and one for every fix
  round (import four, video three, dancer two, cast one, stage two), one on the integrated
  tree and one on the fix rounds that followed. Every finding fixed with a test that fails
  without it (where a test could not fail first — a measurement, a picture — the builder
  said so).
- **Look at what you build**: every builder shot its work (`examples/shoot_*.rs`) and the
  cast drew, looked and redrew each character at least three times; the owner saw a preview
  GIF of the stand-in on the first day and the family's hero video before the rest was built
  on it.
- **API limits** cut every agent twice (a weekly and a five-hour limit); each was resumed
  with its context when the owner said *Reanuda*.

## What each part brought

| Part | ADR | What |
|---|---|---|
| dancer | 0087 | **Saltator**, the dancer instrument (schema 49, `TrackIcon::Dance`, the picker's new *Visual* shelf): its numbers (move, travel, speed ¼×–4×, energy, lift, facing, spin, size, x, y, mood, colour, react, where it shows) are automation and modulator targets; notes from C3 choose its 16 moves like a drum map; the engine's silent unit publishes a frame per segment (`DanceFeed`) and keeps a timeline only when a video asks; the choreography (`pose.rs`, `choreo*.rs`): to the beat by default, phrases of sixteen bars, blends that start from the old move's own cycle; *React to* (the whole song or a channel) as one undo step; presets (*To the beat*, *Party*, *Chill*, *Cool*) that never change who dances or where; the window — *Play* a small theatre (the stage with a spotlight on the accent, the characters' tiles dancing, 16 move pads, Dance/Body/Look/Ears) and *Edit*; the inspector's own rows; MIDI learn by name; `get_unit dancer`, `add_track {"kind":"dancer"}`; a dancer's track cannot be frozen (said); imports loaded off the interface's thread |
| cast | 0090 | **The family**, ours and original, on one shared rig (rubber-hose limbs, mittens, the shared face with every mood) and one choreography danced in each one's style: **Trino** the songbird (a new dancer's), **Miso** the cat, **Lana** the lamb, **Tic** the metronome robot, **Corchea** the eighth note, **Sol** the child — each drawn, looked at and redrawn three times; deterministic on every system (rotations through `rig::rotation`, `cantus_dsp::math` only, a test scans the sources); 0 allocations of the rig's own; bounds measured per character; 0.95–1.34 ms a frame at 600×600; the hero video of the six on one stage |
| import | 0089 | Characters people bring, measured first: **Lottie** (`.json`, `.lottie`) by our own player (velato panicked on four of six variants of a valid file), **Aseprite** and **TexturePacker** sheets, **DragonBones** 5.x (meshes, IK, nested armatures); our own image rasteriser (seamless meshes, 1.4–3× faster than tiny-skia's pattern); a folder sandbox, every size bounded before memory is given, no script ever run, a `catch_unwind` guard; content-addressed, immutable packages in `Characters/<name>-<8 hex>/` with `character.json` (mapping, licence) that the project carries; the card (licence first, the moves mapped by name in English and Spanish, *Edit moves…*); a 64 MB frame cache by recency; 0.34–0.54 ms a frame; 30 000 mutated files without a panic. **Rive** waits (BACKLOG §6i: a C++ runtime of its own round); glTF/VRM later; Spine and Live2D out |
| video | 0088 | `crates/cantus-video` (GPL): our own MP4, MOV (PNG + PCM) and WebM writers; H.264 + AAC by Media Foundation's encoders on Windows, VideoToolbox and AudioToolbox on macOS, Cisco's OpenH264 downloaded (SHA-256 pinned, its terms in the dialog) with the system's AAC or MP3 on Linux and Windows N; WebM by rav1e (its assembly in releases, nasm in CI) and Vorbis; the song's video through the export's own render, every frame bit for bit the screen's; *File ▸ Export video…*: a live preview of the scrubbed moment (rendered on a worker), square, vertical, wide and other sizes, 30/60 fps, the stage, transparent, three chroma keys or black, a title, the visualiser, the encoders said before it starts, an estimate from this machine's own pace; `export_video` and `get_video_export` for agents; one render at a time |
| stage | 0091 | The dancer **over the workspace**: floating (moved and resized, kept per project), docked or hidden, always on top, crisp at the display's pixels; **the companion** (Alt+Shift+D; Alt+D shows or hides the dancers): on Windows a layered window drawn from our own pixels with true per-pixel alpha (`cantus_rt::window::Layered`, `UpdateLayeredWindow`), soft shadows and glass see-through, clicks passing where it is clear; **streaming mode** (Ctrl+Shift+D): the title *Cantus Stage*, transparent or chroma background, a fixed size and position kept between sessions; **the visualiser** (the master's level and spectrum, bar and beat, the song's name and tempo) through a lock-free master tap, the same painter in the video; it pauses hidden or minimised, its cost shown; keymap 15 |
| Showcase | — | README: *A dancer for your songs*; the landing page: the family's video and the stage (EN/ES); the manual's pages in English and Spanish (dancer, import, stage, video) |

## Measured

- **The dancer is silent to the bit**: the null tests hold a project with dancers equal to
  the same project without them, offline and live, at every block size; `rt_memory` counts
  no allocation on the audio thread with dancers playing.
- **Its automation lands on its bars**: an automation clip of `move` over bars 3–4 claps on
  video frames **120…239** exactly at 30 fps, and those frames are bit for bit the screen's
  pictures at the same moments.
- **The window**: *Play* with the character dancing, median **3.67 ms**, p95 4.04 ms a frame
  (release). A hidden dancer costs the window **nothing** (`stage_budget::where_a_hidden_dancer_costs`);
  a floating one redrawn 30 times a second **+11.7 %** of a core while playing (was +26.3 % at
  60). A warm frame of the dancers allocates nothing.
- **The cast**: a frame at 600×600, size 1, every move, mood and side: songbird ≈ 1.34 ms, cat
  ≈ 1.27, child ≈ 1.21, note ≈ 1.09, lamb ≈ 1.05, robot ≈ 0.95 (budget 1.5).
- **Imports**: 0.34–0.54 ms a frame (Aseprite, TexturePacker, Lottie, `.lottie`,
  DragonBones), a hue sweep costing the same; load 6–25 ms; 30 000 mutated files, none
  panicked.
- **Video**: 30 s at 1080×1920 in **3.8 s** as MP4 30 fps (8× real time, 7.1 MB, PSNR
  44.5 dB mean against the lossless render), 7.5 s at 60 fps; WebM 6.0–6.6 fps with rav1e's
  assembly (2.8× without); **audio and picture in sync to 0.000 ms** decoded back for MP4
  (Media Foundation), MOV, WebM, OpenH264 + MP3 and OpenH264 + FFmpeg's AAC, at 30 and 60 fps.
- **The hour under OBS** (OBS 31.1.2 portable, Window Capture by Windows Graphics Capture,
  the welcome song looping, the main window minimised): **343 of 343 checks** over 60 minutes
  with transparent corners, the character drawn and moving; memory flat. With true alpha:
  86 of 86 checks with soft pixels in every capture; **the companion's own cost ≈ 3.4 % of a
  core** (was 14.7 % for the GL window). BitBlt shows the key colour; Game Capture cannot open
  a window drawn on another GPU (a two-GPU laptop).

## Causes found, not waits

- **A smudge under the feet on a chroma key**: the contact shadow came out as a darker
  shade of the key, which a keyer leaves behind; a stage over a key now draws no shadow
  (`Stage::shadows`), and every character honours it (`tests/cast.rs`).
- **The preview's empty visualiser**: the preview's own render now returns its sound,
  analysed on the export's time grid — the preview's visualiser equals the file's frame.
- **Minimising the companion moved its saved place**: a minimised window reports the
  system's parked rectangle, which the sizing step took for a new size and fitted onto the
  nearest monitor; nothing is fitted or stored while it is minimised now, a preset keeps its
  shape when it is fitted (both sides scaled together) and only a size chosen by hand is
  stored.
- **A shortcut macOS keeps for itself**: Ctrl+Alt+D is Cmd+Option+D on a Mac, the Dock's
  own; streaming mode is Ctrl+Shift+D, and a test keeps the stage's keys off Cmd+Option.
- **The ▸ the faces do not have**: the Windows N message used a glyph the interface's fonts
  lack (`font_coverage`); it reads `›` now.
- **A dancer drawing past its own bounds**: the stage's box holds what a character says it
  fills, and the jump test listed moves reaching past it (the songbird's Bow 551 px). The
  cast's measurements were drawing Idle whatever move they set (the pose blended from
  standing still at the song's start); measured honestly, the far reaches were capped and
  the bounds set to what remains.
- **A seek replayed old notes**: a segment that does not start where the last ended (a seek,
  a loop's wrap) now forgets the held notes and the last change of `move`.
- **Every render kept a timeline**: a WAV export, stems, a freeze or a listen kept dancer
  frames; only a video asks for them now (`RenderFeeds::want_timelines`).
- **The companion's 14.7 %**: an OpenGL window and an egui pass for 360×320 points at 60 fps;
  drawn from our own pixels into a layered window, at most 30 a second, with its chips
  cached, it costs about 3.4 %.
- **The preview blocked the interface**: compiling the project for the scrubbed moment ran on
  the interface's thread; it runs on one worker now, cancelled by a newer moment.
- **Media Foundation released after `CoUninitialize`**: a tail expression's temporaries
  outlive the block's locals — an access violation the dialog's test caught.
- **A test counted the whole process**: "a MOV asks for no encoders" read a count shared by
  every test in the binary, and another test's worker still running `ffmpeg -encoders`
  (seconds on WSL's Linux) moved it — the final Linux run's one failure. The window counts
  its own lookups now.
- **The null corpus asked the dancer to sound** (after the tag, GitHub's *Null hashes* on
  every runner): `cantus-bench hashes` renders every instrument and refuses a silent one —
  two builds agreeing on silence prove nothing — and the dancer is silent by design. The
  corpus leaves out what does not sound (`InstrumentKind::sounds`); the check had lived only
  in `xtask null-hashes`, so a test now renders the corpus's instruments and requires each
  to sound. The shipped program is unchanged.

## Results

On `720627d1` (every branch merged, the version bumped), on a quiet machine, with
`RUSTFLAGS=-D warnings`; the last commit, `2bc74573`, changed one test's count (above) and
ran again on its crate on Windows and whole on Linux:

- **Windows**: `cargo test --workspace` 5 972 passed, 0 failed, 85 ignored; the engine
  fuzz (`--features fuzz`, `CANTUS_WORKERS=3`) 1 011 passed; the engine's lib on the pool
  (`CANTUS_WORKERS=3`) 169 passed; `cargo clippy --workspace --all-targets` clean; `cargo fmt
  --all -- --check` clean.
- **Linux** (WSL Ubuntu 24.04, `tools/wsl/check.sh`, a clone of its own): clippy clean;
  5 934 passed, 0 failed, 85 ignored (on `2bc74573`).
- **macOS**: by the runner at publish (`macos.yml`).

## What only the owner can do

- **Look at the companion** on your desktop (Alt+Shift+D, then Ctrl+Shift+D for streaming mode) and
  capture it in your own OBS — a portable copy was used here; your settings were never
  touched. Windows Firewall may ask about the portable OBS: *Cancel* is safe.
- **Upload a test video** (vertical, 30 s) to TikTok, Instagram and YouTube; import the
  transparent MOV (`alpha_png.mov`) into the editors you use (Resolve, Premiere, CapCut).
- **OBS Game Capture on a single-GPU machine** (this laptop's two GPUs refuse the shared
  handle), and capture under Linux compositors (Xcomposite, PipeWire) on a real Linux desktop.
- **The Mac session** (`docs/mac-session.md` row 18: VideoToolbox and AudioToolbox, the
  companion's alpha on macOS).
- **The legal review** gains Cisco's OpenH264 binary terms (downloaded by the user, Cisco
  paying the patent royalties) beside what rounds 30–34 left.
- Everything rounds 30–34 left (`docs/STATUS.md`).
