# Round 35 — stage (ADR 0091)

The stage builder's account: where the dancers dance on screen. **First delivery** — the stage over the workspace: floating, docked on a Stage tab, on
top, hidden; drawn through `DanceTexture`; the frame budget held with a dancer on screen.
**Second delivery** — the companion window, the streaming mode, the visualiser (in the
companion and in videos), keymap 15, the hour under OBS.

## First delivery: the stage over the workspace

### What was built

- **`view.mode` decides where a dancer shows**: floating (0) over the workspace, docked (1)
  on the **Stage** tab (`Pane::Stage`, placed under the inspector the first time, its lower
  40 %), hidden (2) nowhere. `view.top` puts a floating dancer in egui's foreground order,
  above the floating panels; otherwise it is among them (middle order) and comes forward
  when clicked.
- **The workspace is the stage**: a floating dancer's feet at its `x`/`y` share of the
  workspace, `size × 0.30` of its height tall; on the tab, `size × 0.50` of the tab's
  height. No rectangle stored anywhere — place and size are the dancer's own numbers (saved,
  undone, automatable). No schema change.
- **Frameless and see-through; kit-drawn grips on hover** (`views/stage_chrome.rs`): an
  accent line with a soft glow around the body, a pill above it (the stage's mark — a new
  `Glyph::Stage` —, the track's name, pin / dock or float / hide), a round resize grip at
  its top-right corner (resizes about the feet). Dragging the body moves it: `x` and `y` in
  one batch, merged into one undo step per drag (*Move dancer*); the grip merges `size` the
  same way. The frame lingers 0.6 s. Tooltips and the screen reader's name in both locales.
- **One box per dancer** (`stage_geom.rs`): the character's bounds as its pose places them
  (lift, spin, facing), padded, cut to what is seen (the workspace within the screen),
  rounded to whole physical pixels and up to 32-pixel buckets; a sub-stage draws the
  character in the box exactly where the whole area would. Never bigger than the graphics
  card's largest texture.
- **Drawn through `DanceTexture`, only when the pose changed, capped**: 60 a second playing
  or dragged, 15 a second stopped (one wake-up per breath frame, not a steady tick),
  nothing hidden or minimised (texture let go). Draw time and pixels counted
  (`StageState::{redraws, draw_seconds, drawn_px}`) for the cost readout.
- **Poses** (`stage_source.rs`): every track whose native instrument's name key is the
  dancer's; character from the `character` key through a cache (`cast::by_id`, else the
  default — never an error); `pose_at` of the song **as heard** (`EngineHandle::position`,
  through the tempo map and the metre changes) and the dancer's numbers from the project.
  This is the "stand-in from the transport" until dancer's `dance_live` is merged; the swap
  is one function (`source::gather`).
- **Guests** (`StageState::set_guests`, doc-hidden): dancers on no track, danced to the same
  song, for the tests and the screenshots until the dancer's instrument is merged; their
  gestures change nothing.

### How it was tested

- `stage_geom.rs` (4 tests): a box draws what the whole area draws (four poses with spin,
  lift and facing: ≤ 24/255 apart on < 32 edge pixels, nothing outside the box); buckets and
  cutting; the body steady through a jump and a spin; dragging the top sets the size.
- `stage_source.rs` (2): a missing character draws the default; the modes' values.
- `tests/stage.rs` (8), through `CantusApp::frame`: the stage is reached from the frame;
  see-through corners, > 1 000 opaque pixels, at physical pixels (200 % draws twice the
  pixels, the same body in points); pinned = foreground, else middle (egui's
  `layer_id_at`); docked on the tab under the inspector, not floating; hidden: no texture,
  nothing drawn; redraws ≤ 15 + 2 a second stopped (frames at 240 Hz), ≤ 60 + 2 playing
  (frames at 144 Hz), none for the same moment twice; the breath's wake-up asked (≤ 1/15 s),
  nothing asked hidden or minimised; the frame on hover, lingering, gone after 0.6 s; the
  Stage tab under the inspector and kept by a layout.
- `tests/frame_budget.rs` + `tests/stage_budget/frame.rs`: the whole window (16 strips,
  playing, the processor running a block a frame) with a dancer against without it, and a
  release timing.
- Shots: `cargo run -p cantus-ui --example shoot_stage -- <dir>` — floating, hovered,
  docked, empty, Slate and Paper, 100 % and 200 %; the best in `docs/round35/stage/`.

### The numbers

Whole window, 16 strips, 1920 × 1080, the song playing (debug build, medians of 30 frames;
`stage_budget::a_dancer_on_screen_stays_in_budget`):

| | allocations | bytes | triangles | draw calls |
|---|---|---|---|---|
| without a dancer | 282 | 81 KB | 55 797 | 87 |
| with one floating (redrawn every frame) | 412 | 733 KB | 55 801 | 88 |

Of the +130 allocations, 118 are drawing the character itself (tiny-skia paths and edge
lists: the renderer's and the cast's, reported to them); the stage's own share is 12. Of the
+652 KB, 528 KB is the box's image (135 168 pixels × 4: egui takes an owned image to update
a texture) and 51 KB drawing. Stopped, on the frames between two breaths, the window with a
dancer allocates no more than without it (234 against 282, noise). Redraws: 50 in 50
playing frames; ≤ 17 a second stopped; none hidden.

Release, this machine (`frame_budget -- --ignored`, 200 frames, build + tessellate): the
whole window p50 3.10 ms / p95 4.14 ms without a dancer, **3.35 / 4.51 ms with one** (+0.25
/ +0.37 ms, asserted under twice the 2.5 ms budget as the others are); the dancer's own
redraw **0.51 ms** (mean of 230, the box 135 168 pixels at 100 %) — against the contract's
1.5 ms for a 600 × 600 stage.

Checks: Windows `cargo clippy -p cantus-ui --all-targets` with `-D warnings` clean; the
`cantus-ui` lib (418), `stage` (8), `frame_budget` (16 + 8 ignored), `kit_lint`,
`kit_gallery`, `workspace_budget`, `views_nav`, `boot_window` green. Linux (WSL, 1a0f0b96):
workspace clippy and the plug-ins without editor clean; lib 427, `kit_lint` 4, `stage` 8
green, headless.

### Marked lines (files the stage does not own)

- `crates/cantus-ui/src/lib.rs`: `pub mod stage;`
- `crates/cantus-ui/src/app.rs`: the `stage` field and its `Default::default()`.
- `crates/cantus-ui/src/views/workspace.rs`: `Pane::Stage` (variant and its doc line), its
  `title_key`, `toggle_key`, `glyph` arms, and its `place` arm (`stage::dock::place`).
- `crates/cantus-ui/src/views/workspace_frame.rs`: the tab's `ui` arm, and
  `stage::float::show(app, ui, area)` at the end of `workspace::show`.
- `crates/cantus-ui/src/kit/glyph.rs`: `Glyph::Stage` (variant, its doc line) and its paint
  arm.
- `crates/cantus-ui/tests/frame_budget.rs`: `#[path] mod stage_budget;` (two lines).

## Second delivery: the companion, streaming, the visualiser, keymap 15, the hour under OBS

### What was built

- **The live dancers** (dancer's `DanceLive`, merged at f0a3d752): `stage_source::gather`
  takes them once a frame; the guests seam is gone — every test, the budget module and the
  shots use real dancer tracks. Dragging a dancer (one undo step) and its pill (dock → the
  Stage tab opens) are tested end to end.
- **Each dancer's box reaches the floor under it** (cast's finding): the bounds are taken
  through the lifted, spun placement *and* through the grounded one (lift 0, spin 0), where
  `rig::ground` draws the contact shadow, dust and confetti — tested for the whole cast (six
  characters), every move, at lift 0, 0.5 and 1.5: no pixel outside the box.
- **The companion window** (`stage_companion.rs`, `views/stage_companion_view.rs`): an egui
  deferred viewport titled `Cantus Stage`, no decorations, always on top unless asked,
  dragged by anything in it (`StartDrag`), resized by its corner (`BeginResize`); the menu
  by right-click or its ⋯ button (character → the cast → the project's dancers; six sizes;
  five backgrounds; spectrum / level / off; bar and beat; song; on top; streaming; *Copy OBS
  setup*; its cost; close); every choice in `settings.stage` (additive, `#[serde(default)]`),
  reopened at start. It dances with whatever plays: the song as heard; stopped but sounding
  (master above −50 dBFS), its own beat at the project's tempo; still, breathing. Energy from
  the master's level (`react` 55 %).
- **Paused when minimised**: its pass returns at once, no frame asked, the tap off. **While
  the main window is minimised** eframe runs only `App::logic`: `boot::Shell::logic` (marked)
  calls `companion::logic`, which reads the engine's reports and hands the window its frames
  — the hour below ran that way.
- **See-through on Windows by a colour key.** A GL window's alpha did not reach the desktop
  here (a two-GPU laptop: DWM's blur-behind showed black, and the driver offers no
  `WGL_TRANSPARENT_ARB` configuration). `cantus_rt::window::key_colour` (new, Windows only,
  the only unsafe code, in cantus-rt) makes this process's `Cantus Stage` window layered with
  a colour key, and drops Windows 11's rim and rounded corners; the windows clear to the key
  `(1, 0, 1)` with alpha 0 while the companion is open (`Shell::clear_color`, marked), so a
  capture that reads alpha gets it too. Over the key the visualiser's plates are opaque and
  its glows off (a soft edge would turn into a dark rim). On Linux and macOS the splash asks
  for an alpha GL configuration (the main window turns opaque when the splash becomes it);
  a driver with none makes eframe fail before any window, and the program starts itself
  again with `CANTUS_NO_ALPHA` (`lib.rs`, marked), the transparent background then drawn as
  the theme's.
- **Streaming mode**: not resizable, not draggable, no frame or buttons on hover (the menu
  still on right-click), the same title every session, size and place kept.
- **The light visualiser**: `cantus_engine::master_tap` (MPL; a lock-free `rtrb` ring of the
  output's first two channels, copied at each block's end only while a reader has it on —
  one relaxed load a block otherwise; marked lines in `processor.rs`, `processor_pass.rs`,
  `listen_fork.rs`, `backend.rs`, `lib.rs`); `stage::vis` analyses on the interface's side
  (28 third-octave bands 40 Hz–16 kHz, −66…−6 dBFS, rise at once, fall 1.4 of the scale a
  second, at most 30 analyses a second); `stage::vis_paint` draws it like a broadcast
  overlay (glass plates, accent bars with a glow, the bar's beats pulsing).
- **In videos** (video's `SongFrames.overlay` hook): `stage::vis_video` analyses the
  export's own render once (30 instants a second) and draws each frame as a pure function of
  its time (a band = the highest analysis within the fall's reach less the fall since —
  the companion's ballistics, no state between frames); chips along the top, the band under
  them (clear of the dancer and the title line), the song's chip only when the title line is
  off. The dialog's **Visualiser** row (on by default; `VideoSettings::visualiser`) and its
  preview (the bars at rest until the export renders the sound).
- **Keymap 15**: Alt+D shows or hides the dancers (*View ▸ Stage*), Alt+Shift+D opens the
  companion, *Streaming mode* unbound (palette); a version-14 file gains the two keys where
  free. *View ▸ Stage / Companion window / Streaming mode*.

### How it was tested (all on Windows; Linux below)

- `tests/stage.rs` 10 (real dancer tracks), `tests/stage_companion.rs` 8 (its key opens
  it, its pass draws and measures itself, closed no pass; the menu's choices carried out and
  kept in the settings file; stopped it dances to a sounding master at the tempo and to the
  song playing; the visualiser reads the master through the manual engine and the tap runs
  only while shown; minimised it pauses; the View switches; the project's dancers followed;
  an old settings file reads), `tests/stage_video.rs` 5 (a tone lights its band; the
  meter's fall; a frame is a pure function of its time, corners untouched; drawn through
  `SongFrames`, the same frame twice the same pixels; the dialog's choice on by default and
  kept), `stage_geom` 5, `stage_vis` 3, `master_tap` 2, `rt_memory`'s
  `the_master_tap_never_allocates` (off, on and drained, a full ring, off and on again: no
  allocation, no free), `keymap_14_gains_the_stage_keys`.
- Shots (`shoot_stage -- <dir> companion | video`) looked at and refined: feet above the
  band, the menu button clear of the chips, round level bar, whole tempos, tall windows;
  in videos the band moved from the bottom (it hid the dancer's feet) to under the top chips,
  and capped at six times its height (wide frames made blobs of the bars).

### The numbers

**Frame budget, a dancer on screen** (whole window, 16 strips, playing, debug medians; the
stage's share against the same dancer hidden): +13 allocations beside the character's own
drawing (149 a redraw for the songbird as merged at 20b0871e — 595 before the cast's speed
work), +2 triangles, +1 draw call, +1 321 KB (the box's image, 229 376 pixels, and the
drawing's 311 KB).

**Characters drawing past their own `bounds()`** (energy 1, listed by the jump test — the
cast's to fix; the stage's box holds what a character says it fills, so these pixels are
cut on screen and in the companion): songbird Twist 345 px and Bow 551 px, robot Bow 183,
note Bow 107, lamb Bow 12. A dancer track itself (hidden) costs the
rest of the window +45 allocations, +7 713 triangles, +4 draw calls (the dancer builder's
views).

**The hour under OBS** (`tools/obs/stage_hour.py`, OBS 31.1.2 portable, Windows 11, a
two-GPU laptop, release build, the welcome song looping at −30 dB, the streaming window
transparent 320 × 360 points, Cantus's main window minimised):

| Capture | Captures | Keeps the alpha |
|---|---|---|
| Window Capture, Windows Graphics Capture | yes | **yes** — corners alpha 0 every check |
| Window Capture, BitBlt | yes | no (the key colour, black) |
| Game Capture (*Allow transparency*, multi-adapter compatibility) | **no** — "init_shtex_capture: failed to open shared handle" (OBS and Cantus on different GPUs) | — |

- **343 checks over 60 minutes, every one passed**: corners transparent, the character drawn
  (≈ 62 000 opaque pixels), and moving within each burst of four screenshots a sixth of a
  second apart (4 distinct frames, ≈ 11 % of pixels changed).
- **Cantus's CPU**: 14.7 % of one core mean (21.4 % max) with the companion dancing and the
  main window minimised, against 78.7 % mean for the main window alone playing the same song
  (the baseline, 5 minutes; that cost is the main window's own frames at 60 a second).
- **Memory steady**: working set 266.0 → 261.6 MB, private bytes 236.4 → 229.4 MB over the
  hour (no growth).

On Linux (Xcomposite, PipeWire) the alpha is the compositor's: not measured here (no Linux
desktop on this machine) — for the Mac and Linux sessions.

### Checks (second delivery)

Windows: `cargo clippy -p cantus-ui -p cantus-engine -p cantus-rt --all-targets` with
`-D warnings` clean; cantus-ui lib 423, `stage` 10, `stage_companion` 8, `stage_video` 5,
`frame_budget` 16 (+8 ignored), `kit_lint`, `kit_gallery`, `workspace_budget`, `views_nav`,
`boot_window`, `video_export`, `dance_live`; cantus-engine lib, `rt_memory` 27, cantus-rt 18 —
green. Linux (WSL, ce2664c6): workspace clippy and the plug-ins without editor clean;
cantus-ui lib 432, `kit_lint` 4, `stage` 10, `stage_companion` 8, `master_tap` 2 — green,
headless.

### Marked lines added in the second delivery

- `crates/cantus-engine/src/lib.rs`: `pub mod master_tap;`
- `crates/cantus-engine/src/processor.rs`: the `master_tap` field (+ doc line) and its
  `Default::default()`; `processor_pass.rs`: `self.master_tap.push(…)`; `listen_fork.rs`:
  `master_tap: _,`; `backend.rs`: the handle's field, its two attachments and `None`.
- `crates/cantus-engine/tests/rt_memory.rs`: `#[path] mod stage_tap;` (two lines).
- `crates/cantus-rt/src/lib.rs`: `pub mod window;`; `Cargo.toml`: two windows-sys features.
- `crates/cantus-ui/src/settings.rs`: the `stage` field (+ doc line) and its default.
- `crates/cantus-ui/src/app.rs`: `crate::stage::action` (three lines) and the
  `ToggleStage | ToggleCompanion | ToggleStreaming` arm.
- `crates/cantus-ui/src/views/transport_menus.rs`: `crate::stage::view_menu(app, ui);`
- `crates/cantus-ui/src/boot.rs`: `Shell::running`, `clear_color`, `logic` (marked block).
- `crates/cantus-ui/src/lib.rs`: `.with_transparent(alpha_wanted())`, the kept result and
  the restart without alpha.
- Video's files: `video_export.rs` (`visualiser` field, doc line, default),
  `video_export_job.rs` (`Work::vis`, its setup, its destructuring, `for_render` and
  `with_title`), `views/video_export.rs` (the preview's overlay),
  `views/video_export_rows.rs` (the Visualiser row).
- `docs/manual/{en,es}/README.md`: the link to `stage.md`.

## Third pass: true alpha on Windows, the cost for all day, what a dancer costs

### True per-pixel alpha on Windows

The colour key is gone. On Windows the companion is a **layered window drawn from our own
pixels** (`cantus_rt::window::Layered`, `window_layered.rs`, the only unsafe code): a
popup window made per-monitor DPI aware (crisp, placed in physical pixels), updated with
`UpdateLayeredWindow(…, ULW_ALPHA)` from a premultiplied BGRA DIB section — no OpenGL for
it, so the two-GPU problem is gone and soft shadows, glass and anti-aliased edges are real
alpha. The system lets clicks through wherever alpha is 0, moves the window by any opaque
pixel (`HTCAPTION`) and resizes it by its corner (`HTBOTTOMRIGHT`) itself; right-click or
the ⋯ button opens the kit's menu in a small egui window at the pointer. Everything in it is
drawn by `stage::companion_paint` with tiny-skia at the window's exact pixel size: the
backdrop, the dancers through the renderer's placement, the visualiser (the same `Painter`
the video frames use — chips' plates and words kept in a cache until they change, only the
beat dots, bars and dancers drawn each frame), the hover frame. Linux and macOS keep the
egui window and the GL alpha path.

Shots: `docs/round35/stage/companion-desktop-true-alpha.png` (the real window over the
desktop: glass and the soft shadow see-through), `native-transparent-hover.png`.

### The companion's cost (release, this machine, main window minimised)

| 15 minutes, OBS capturing it | Cantus's CPU, median (mean / max) |
|---|---|
| the song playing, no companion | 3.7 % of a core (3.4 % in the previous run) |
| the song playing, the companion dancing (true alpha, 30 fps) | 7.1 % (7.1 / 9.5) |
| **the companion's own** | **≈ 3.4 % of a core** (was 14.7 % for the whole process with the GL companion) |

What changed: no GL window and no egui pass for it, drawn at most 30 times a second while
the song plays (12 for the breath), at its exact pixel size into a kept pixmap, the chips
cached, the system moving and resizing it, the live analysis at 20 a second. OBS (Windows
Graphics Capture): **86 of 86 checks** with transparent corners, the character drawn and
moving, and **soft pixels (0 < alpha < 255) in every capture** (≈ 33 900 per frame); BitBlt
still shows black. Memory flat (working set 260.8 → 258.8 MB).

### What a dancer costs the main window (release, the welcome song playing, 5 min each)

| Main window playing | CPU, median | the dancer's own |
|---|---|---|
| no dancer track | 76.0 % of a core | — |
| a dancer track, hidden | 74.5 % | **0** (within noise) |
| a dancer floating, redrawn 60 a second (before) | 102.3 % | +26.3 % |
| no dancer track (second run) | 68.9 % | — |
| a dancer floating, redrawn 30 a second (now) | 80.6 % | **+11.7 %** |

A dancer's redraw (the songbird into a 230 000-pixel box, egui taking an owned image and
uploading it) now happens 30 times a second while playing, 60 only while it is dragged, 12
for the breath. **A hidden dancer costs the window nothing in the arrangement, the mixer or
the channel rack** (`stage_budget::where_a_hidden_dancer_costs`); the +45 allocations /
+7 713 triangles / +4 draw calls a frame are **the inspector showing the dancer track's
instrument section** when that track is selected (the inspector alone: +57 allocations,
+1 362 triangles; the whole window +66 / +7 553) — the dancer builder's inspector section.

### Review fixes

- The project's dancers keep moving in the companion while the main window is minimised:
  `companion::logic` reads their feeds and gathers them itself when following the project
  (`following_the_project_it_keeps_dancing_while_the_main_window_is_minimised`).
- The tap is checked every frame against the engine's own, so an engine made again (the
  audio device reopened) gets it armed again (`the_tap_is_armed_again_on_a_new_engine`).
- *Export video…* in the companion's menu opens the video dialog
  (`its_menu_opens_the_video_export`).
- The manual's stage page links the dancer, import and video pages (EN, ES).

### Checks (third pass)

Windows: clippy `-D warnings` clean on cantus-ui, cantus-rt, cantus-engine (all targets);
cantus-ui lib 425, `stage` 10, `stage_companion` 11, `stage_video` 5, `frame_budget` 17 (+8
ignored), `kit_lint` 4, `video_export` 13 — green. Linux (WSL, 5d13291b): workspace clippy
and the plug-ins without editor clean; lib 434, `stage` 10, `stage_companion` 11, `kit_lint`
4, `master_tap` 2 — green, headless.

## Fourth pass: the review of the fix rounds (#2, #3, #4, #6)

- **#2 — resizing by the corner snapped back.** `UpdateLayeredWindow` sizes a layered window
  to the bitmap it is given, and each frame presented at the stored size while the sizing
  loop was changing it (its `WM_SIZE`s woke frames). Now the window says when the system is
  sizing it (`WM_ENTERSIZEMOVE` … `WM_EXITSIZEMOVE`, `Layered::is_sizing`, a `Resizing`
  event per size); meanwhile the companion is drawn at the window's own size, and the size
  is stored when the person lets go. A `WM_SIZE` that is no restore no longer queues
  `Minimised(false)`; the least size is the system's (`WM_GETMINMAXINFO`). Found on the
  way: on a see-through backdrop the corner's pixels are alpha 0, which the system passes
  to the window under it — so the grip now sits on a small plate on hover. The window's
  rectangle and pointer are read per-monitor aware (a thread of another awareness is told
  scaled numbers — the first run of the new test showed 475 × 325 as 380 × 260).
- **#3 — the menu while the main window is minimised.** eframe runs no pass then, only
  `App::logic`, and an egui viewport can only be opened from a pass. The menu is now a
  layered popup of ours (`stage_companion_menu_win.rs`): the kit's menu drawn by an egui
  context of its own on the CPU (`crate::raster`), its pointer, clicks, Esc and activation
  from the window procedure, opened at once — from a pass or from `logic` — where it was
  asked, at that monitor's scale, flipped at the work area's edges; drawn when something
  changes (and once a second for the cost line); closed by a choice that is done with it,
  Esc, or another window chosen. *Export video…* first restores, shows and focuses the
  main window (viewport commands, which `logic` can send).
- **#4 — mixed-DPI placement.** The corner is kept in physical pixels
  (`StageSettings::native_pos`, additive; an older file's points read at the primary's
  scale as before), the size in points; the window is created directly at that rectangle
  (no 1 × 1 window at (0, 0) first, nothing queued while it is made), the size at the scale
  of the monitor it lands on, fitted into that monitor's work area (`MonitorFromRect`, the
  nearest when on none) and never below the smallest companion; `sane_size` brings a
  settings file's size back within 240 × 300 … 4096 × 4096 on every platform.
- **Found by hand behind #2:** the corner drag never even started — a `WS_POPUP` without
  `WS_THICKFRAME` is not sizable by the system, whatever `WM_NCHITTEST` answers. The
  companion now has a sizing frame, none drawn (`WM_NCCALCSIZE` makes the whole window its
  client; the frame is measured again once the procedure answers). And a size stepped from
  the menu grew it past the bottom of the screen (520 + 575 > 1 032): it is now fitted back
  onto its monitor, moved from its corner.
- **Keymap 15** (the integration suite's `every_arrangement_action_has_a_key_bound_to_it`):
  streaming mode on **Ctrl+Alt+D** (free in every preset; the D family of the stage), in
  `NEW_IN_V15` so a version-14 file gains it where it is free; manual EN/ES and ADR.
- **#6** — "Exportar video…" in `stage.es.txt` and in `dance.es.txt` (both lines, marked).
  `interop.es.txt:105` still says "vídeo" (not this round's).

Tests: `window_layered.rs` 5 (the snap-back and its cure with a real window; a size change
is no restore, a minimise told once; a popup hears pointer, clicks, keys, activation, the
companion does not; the least size; any rectangle fitted onto a monitor), `window.rs` 4
(`fit_in`), `companion_win` 3 (physical corner and size in points; a window of a pixel
kept at the least size; the menu popup takes clicks with no pass of the main window and
closes on Esc), `companion_menu_win` 2 (where it opens), `stage_companion` +2 (*Export
video…* under `logic` sends Minimized(false) and Focus and opens the dialog; a size out of
reason brought back).

By hand, release build, this machine (one monitor, 125 %), the real window driven with the
pointer (`SendInput`), before → after:

| check | before (68c4c7b3) | after |
|---|---|---|
| drag the corner +140, +90 (theme backdrop) | stays 400 × 450 | 428 × 468 … 540 × 540 during the drag, kept |
| the same on the see-through backdrop | stays 400 × 450 | 540 × 540, kept |
| right-click with the main window minimised | no menu (stored, popped later) | the popup in 30–130 ms at the pointer, flipped above-left at the screen's corner; Esc closes it |
| its *Size* row, main window minimised | — | the companion steps 400 × 450 → 500 × 575, the menu stays open |
| its *Export video…*, main window minimised | dialog in the hidden window | the main window restored, in front, the dialog open |
| no stored place, three starts | — | 1470, 520 each time (bottom-right of the work area) |
| stored at (5000, 3000), size 1 × 2 points | — | opened at 1620, 645, 300 × 375 (240 × 300 points), inside the work area; stored so |

Mixed DPI could not be tried by hand here (one monitor); the placement is covered by the
tests above (physical corner, fitting onto the nearest monitor's work area).

### The pictures again, with the real characters

`cargo run -p cantus-ui --example shoot_stage -- <dir> [case] [200]` now shoots the program's
own opening song (the house demo's drop, its whole desk) **played by the real processor** to
bar 5 — the meters live, the audio chip an interface — and its own dancer track placed for
the picture: Trino, *Arms up*, energy 100 %, excited (`CANTUS_SHOT_MOVE`, `_ENERGY`, `_MOOD`, `_X`, `_Y`, `_SIZE`, `_BEAT`
try another without building again). In `docs/round35/stage/`, 100 %, English:

- `stage-float-slate-100.png` (Cantus Night) — **the hero**: Trino floating over the mixer's
  empty middle, large enough to read, the song playing; `stage-float-paper-100.png`; and in
  Spanish `stage-float-es-slate-100.png`.
- `stage-dock-{slate,paper}-100.png` (the Stage tab under the inspector),
  `stage-hover-slate-100.png` (frame, grips, its bar), `stage-empty-slate-100.png`.
- `companion-{theme,transparent,green,vertical,menu}-{slate,paper}-100.png` — the companion
  with the visualiser; `companion-streaming-{transparent,green}-{slate,paper}-100.png` —
  streaming mode over a stream's picture and on a chroma key; `native-transparent-hover.png`.

Looked at, one by one; what did not read was redrawn: the vertical companion cut Trino's
wings (his arms up reach wider than he is tall) — the companion now scales a character by
its reach where the window is narrow (`companion_paint::standard_of`, both paths, tested);
the old 200 % companion shots are gone. Seen and left to the character's owner: on a chroma
key the contact shadow under the feet is a darker green that a key leaves as a faint smudge
(the renderer has no switch for it; a `Canvas` flag would let the stage turn it off there).

### The export dialog's preview draws the file's visualiser

Its spectrum box was always empty: `vis_video::preview` built the visualiser with no
analysis, while the dialog promises the file's own frame. The preview's render
(`video_export_job::preview_render`, a few seconds of the song up to the moment, through the
export's own render) now answers its sound analysed too — `vis_video::analyse_from`, on the
export's own instants (video time 0 at the range's start, the render's offset in samples),
silent before its first second — and the preview draws from it (`video_export::
PreviewAnswer`, `preview_bands`; marked lines in `video_export.rs` and `video_export_job.rs`).
Tests first (`tests/stage_video.rs`): at bar 4 of the welcome song the preview's bars are lit
and **equal the export's bands, and its visualiser is the file's frame pixel for pixel**;
with every clip muted the box is the empty one. `shoot_video` (EN and ES) shot again.

### Checks (fourth pass)

Windows (after merging integration 092417c4): clippy `-D warnings` clean on cantus-ui and
cantus-rt (all targets); cantus-rt `window` 10, cantus-ui lib 432, `app_flows` 73, `stage`
10, `stage_companion` 13, `stage_video` 7, `video_cast` 8, `video_export` 13, `kit_lint` 4,
`score` 14, `dancer_demo` 2 — green. Linux (WSL, d1e6bcd8): workspace clippy clean; lib 436,
`stage` 10, `stage_companion` 13, `stage_video` 7, `kit_lint` 4, `master_tap` 2 — green,
headless.

## Fifth pass: the last review (on integration 45f23bd0)

- **#1 Minimising no longer moves its saved place.** The size step (`companion_win::size_step`,
  pure) fits nothing while the window is minimised (its rectangle is the system's parked
  one, far off every screen) — `minimised_nothing_is_fitted_or_moved`.
- **#2 A preset keeps its shape and its place in the cycle.** `fit_in` shrinks both sides by
  one factor (9:16 stays 9:16 — `a_shape_taller_than_the_screen_keeps_its_shape`); a fit
  stores only the corner, and a placement stores a size only when the person resized it by
  hand (another size at the same scale); moved, or opened, it keeps the size chosen —
  `a_preset_taller_than_the_screen_keeps_its_shape_and_its_place_in_the_cycle`,
  `moved_it_keeps_its_size_resized_it_keeps_the_new_one`.
- **#3 No contact shadow over a key in the egui window either.** `geom::Area` carries
  `shadows` (`with_shadows`), the companion view turns it off over a chroma key —
  `over_a_chroma_key_nothing_but_the_character_touches_the_key` (the window's own last
  stage, rendered over the key against the character alone: no pixel off the key where the
  character draws nothing; over the theme's stage the shadow is there).
- **#4 It opens at the size of the monitor it lands on** (`place_on`: sized at the corner's
  scale, fitted, sized again at the landing monitor's) and the opening stores only the
  corner — `it_opens_at_the_scale_of_the_monitor_it_lands_on` (a two-monitor layout, 150 %
  and 100 %).
- **#5 Streaming mode is Ctrl+Shift+D** (Cmd+Shift+D on a Mac). Ctrl+Alt+D was Cmd+Option+D
  there, the Dock's. Free in every preset; `NEW_IN_V15` gives it to a version-14 keymap;
  manual EN/ES and ADR. Test: `the_stage_keys_are_no_systems_own` (in every preset, the
  stage's keys are bound, never Cmd+Option, and shared with no other action).
- **#7** The orphaned doc comment on `Shared::slots` is gone (no test: a comment).

Checks (fifth pass): Windows clippy `-D warnings` clean on cantus-ui and cantus-rt (all
targets); cantus-rt `window` 11, cantus-ui lib 438, `app_flows` 73, `stage` 10,
`stage_companion` 13, `stage_video` 7, `kit_lint` 4 — green. Linux (WSL, 74bae64d):
workspace clippy clean; lib 438 and the stage suites green, headless; the clone's target
deleted.
