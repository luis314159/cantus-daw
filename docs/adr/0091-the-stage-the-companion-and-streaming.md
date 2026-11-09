# ADR 0091 — The stage: dancers over the workspace, on a Stage tab, in a companion window and on stream

**Status:** accepted (round 35, stage builder) · **Date:** 2026-11-01 · **Extends:** ADR
0087 (the dancer, its feed, the one renderer), ADR 0024 (the kit; §5 the workspace's docks;
§13 the frame budget and the ticker), ADR 0022 (what is heard), ADR 0005 (the audio thread)
· `Pane::Stage`, `Glyph::Stage`, keymap 15, `cantus_engine::master_tap`, settings
`stage` (additive) · no schema change

## Context

Round 35 puts a character that dances on screen (BACKLOG §6i). The dancer is a silent
instrument whose unit tells the screen where the song is (ADR 0087); this ADR is where its
character is *shown*: over the workspace while a person works, on a tab of its own, in a
small window beside any other program, and in a window OBS captures for a stream. The owner
asked that it look first-rank ("super, super professional — really cool and fun") and that
it never cost the workspace its frame budget.

## Decision

### 1. The stage over the workspace and the Stage tab

- **Where a dancer shows is its `view.mode`** (ADR 0087's key, set, never automated):
  **floating** (0) over the workspace, **docked** (1) on the workspace's **Stage** tab,
  **hidden** (2) nowhere. `view.top` keeps a floating dancer above the floating panels
  (egui's foreground order); otherwise it lives among them (middle order) and comes to the
  front when clicked.
- **Floating, the workspace is the stage.** A dancer's feet stand at its `x`/`y` share of
  the workspace (the dock's area), and it is `size × 0.30` of the workspace's height tall
  (`geom::FLOAT_STANDARD`). So its place and size are its ordinary numbers — saved with the
  project, undone like any edit, automatable (an automation of `x` walks it across the
  window, FL's Fruity Dance's idea). No window rectangle is stored anywhere.
- **Docked, the Stage tab is the stage**: the same numbers on the tab
  (`size × 0.50` of its height), on the theme's stage — its deepest ground (`ink`), a soft
  spotlight from above in the theme's accent, a floor line fading to the sides. The tab is
  placed under the inspector (its lower 40 %) the first time, and moves, stacks and floats
  like any tab; a layout keeps it. An empty tab says how to put a dancer there.
- **Frameless, see-through, kit-drawn grips on hover.** Nothing but the character shows
  until the pointer is over it; then, kit-drawn: a thin accent line with a soft glow around
  its body, a pill above it (the stage's mark, the track's name, *pin on top*, *dock* or
  *float*, *hide*), a round grip at its top-right corner. Dragging the body moves it (`x`,
  `y` in one batch, one undo step per drag: *Move dancer*), the grip resizes it about its
  feet (`size`, one step per drag). The frame lingers 0.6 s after the pointer leaves.
  Tooltips and the screen reader's name ("Dancer of ‹track›") in both locales.
- **Each dancer is drawn into a box of its own** (`stage::geom`), not the whole workspace:
  the character's bounds as its pose places them (lift, spin and facing included), padded,
  cut to what can be seen, rounded out to whole physical pixels and up to 32-pixel buckets
  so its pixmap keeps its size while it moves. A sub-stage puts the character in the box
  exactly where the whole area would — a test draws both and compares them pixel by pixel
  (`a_box_draws_what_the_whole_area_draws`: at most 24/255 apart on fewer than 32 edge pixels,
  nothing outside the box). The body the pointer grabs is the standing box (no lift, no
  spin), steady while it jumps.
- **One path to the pixels**: `cantus_dance::render` through `DanceTexture` at the exact
  physical pixel size shown — drawn again from its vectors at every zoom, never stretched.
- **Drawn only when its pose changed, and capped**: at most 60 redraws a second while the
  song plays or a dancer is dragged; stopped, a slow breath at 15 a second (the ticker asked
  for one wake-up 1/15 s later, never a steady 60); hidden or minimised, nothing at all —
  no redraw, no wake-up, its texture let go. The time spent drawing and the pixels drawn are
  counted (`StageState::draw_seconds`, `drawn_px`) for the cost readout.
- **Where the poses come from** (`stage::source`): the dancer's live list
  (`dance_live::DanceLive`): every dancer track, its character, its pose at the position
  **heard** (ADR 0022) from the engine's feed — its numbers after automation and
  modulation, its notes — or from the transport before its unit arrives. The stage adds the
  track's name and where it shows.
- **View ▸ Stage** (Alt+D, keymap 15) shows or hides every dancer on screen at once without
  touching their `view.mode`: hidden, nothing over the workspace and the Stage tab closed.

### 2. The companion window

- **An OS window of its own** (an egui viewport, deferred), titled **`Cantus Stage`**, no
  decorations, always on top unless asked otherwise, **see-through** (per-pixel alpha),
  opened by *View ▸ Companion window* (Alt+Shift+D) and reopened at start when it was open.
  Dragged by anything in it (the system's own move), resized by its corner grip; right-click
  (or its ⋯ button on hover) for its menu: character (the cast, then *the project's
  dancers*), size (three companions and the square, vertical and wide shapes), background,
  visualiser, bar and beat, song name and tempo, always on top, streaming mode, *Copy OBS
  setup*, its cost, close. Each row steps its value in place; every choice is kept in the
  settings file (`stage`, additive fields).
- **It dances with whatever plays, with no automation**: the song's position as heard while
  the transport runs; stopped while the master sounds (a preview, an input), a beat of its
  own at the project's tempo from when the sound began; still in silence, breathing. Its
  energy follows the master's level (`react` 55 %). Or, asked, the project's own dancers as
  they dance on the stage.
- **One GL configuration with alpha (Linux, macOS; on Windows see §3).** Every window of the application shares the
  configuration chosen when the first window (the splash) opens, so the splash is made
  transparent and the main window turns opaque when the splash becomes it. A driver with no
  such configuration makes eframe fail before any window: the program then starts itself
  again with `CANTUS_NO_ALPHA` set (no restart loop: the second run asks for no alpha), and
  the companion's see-through background is drawn as the theme's. While the companion is
  open the windows are cleared to transparent (the main window paints over all of itself);
  closed, to the theme's panel as before.
- **Paused when minimised or hidden**: its pass returns at once and asks for no frame, the
  main window stops handing it frames, the master tap is turned off. While the *main*
  window is minimised (eframe then runs no pass of it, only `App::logic`), the companion
  keeps dancing: the engine's reports are read and the window handed its frames from
  `logic`, at 60 a second playing and the breath's 15 stopped.
- **Its cost, measured and shown**: the CPU time of its own pass (the dancer drawn, the
  visualiser, the layout), smoothed, with its frame rate and the share of a core they make,
  in its menu; GPU time is said in words to be not measured — eframe's glow backend gives
  no timer query to an application, and an estimate would be a guess.

### 3. The streaming mode

The companion as a capture source: the same fixed title and class every session (Windows:
`Cantus Stage`, winit's class `Window Class`, `cantus.exe`; Linux: app id `cantus-stage`),
nothing else of the interface in it (no frame, grip or button on hover — right-click still
opens its menu), its size and place fixed (not resizable, not draggable) and kept between
sessions, the background **transparent**, a **chroma key** (green, blue, magenta — the
renderer's `CHROMA_*`, the cast keeps every colour away from them) or **the theme's stage**.
Over a chroma key nothing is half-transparent: the visualiser's plates are opaque, its glows
off, and the characters draw no contact shadow (`Stage::shadows`, on both the Windows window
and the egui one through `geom::Area::with_shadows`), so a key cuts clean. *Copy OBS setup* puts the steps for the chosen background on the
clipboard.

**What keeps the alpha** (measured with OBS 31.1.2 on Windows 11, `tools/obs/`, an hour):
*Window Capture* with the Windows Graphics Capture method keeps it (corners alpha 0 in all
343 checks); BitBlt shows the key colour (black); *Game Capture* did not capture on a
two-GPU laptop (OBS cannot open the shared texture across adapters, even with multi-adapter
compatibility). On Linux the compositor's alpha is not measured here.

**True per-pixel alpha on Windows: a layered window drawn from our own pixels.** On that
machine a GL window's alpha did not reach the desktop (DWM's blur-behind showed black; the
driver offers no `WGL_TRANSPARENT_ARB` configuration), and a colour key — tried first —
turned every soft pixel into a dark rim. So on Windows the companion is not a GL window at
all: `cantus_rt::window::Layered` (the only unsafe code, Windows only) is a layered popup
window updated with `UpdateLayeredWindow(…, ULW_ALPHA)` from a premultiplied BGRA DIB
section, and `stage::companion_paint` draws everything in it with tiny-skia at its exact
pixel size — the backdrop, the dancers through the renderer's placement, the visualiser
(the same `Painter` the video frames use), the hover frame, ⋯ and grip. Soft shadows,
glass and anti-aliased edges are real alpha on any GPU; the system lets the pointer through
pixels of alpha 0 (so on hover the grip sits on a small plate: the corner the system sizes
from must be drawn); it moves and resizes the window itself (`HTCAPTION`, `HTBOTTOMRIGHT`
from `WM_NCHITTEST`, its least size from `WM_GETMINMAXINFO`); its events (placed, resizing,
minimised, hover, menu, close) are queued and wake the application's next frame. **While
the person sizes it** (between `WM_ENTERSIZEMOVE` and `WM_EXITSIZEMOVE`) it is drawn at the
size the system gives it, and the new size is stored when they let go: `UpdateLayeredWindow`
sizes the window to the bitmap it is given, so a frame drawn at the stored size would size
it back under the pointer (a test shows that snap-back and its cure). **Where it is** is kept
in physical pixels (`StageSettings::native_pos`) with its size in points: points are a
different size on each monitor, so a corner divided by one monitor's scale and multiplied
back by another's lands elsewhere (150 % beside 100 %: ×1.5, off the screen). It is created
directly at that rectangle — the size at the scale of the monitor it lands on, not the one
its corner is on — fitted inside that monitor's work area (`MonitorFromRect`, the nearest
when it is on none: a monitor unplugged), shrunk whole when it is larger (its shape kept: a
9:16 stays 9:16), never smaller than the smallest companion. Only its corner is stored for
that: the size kept is the one chosen (a menu preset stays that preset, so the menu's cycle
goes on), and only a resize by the person's own hand stores a new one. Minimised, its
rectangle is the system's parked one: nothing is fitted or stored then. A size out of
reason in the settings is brought back before anything is drawn. **Its menu** is the kit's, drawn by an
egui context of its own on the CPU (`crate::raster`) into a small layered popup at the
pointer, at that monitor's scale and inside its work area: an egui viewport can only be
opened from a pass, and while the main window is minimised — a streamer's usual set-up —
eframe runs none, only `App::logic`. It draws when something changed, closes on a choice
that is done with it, on Esc, or when another window is chosen. *Export video…* from it
first restores and brings forward the main window, where the dialog opens. Its window class
is `Cantus Stage` too. On Linux and
macOS the companion stays an egui window, see-through by the GL configuration's alpha.

### 4. The light visualiser

- **The master's samples**: `cantus_engine::master_tap` — at each block's end, while a
  reader has it on, the audio thread copies the output's first two channels into an `rtrb`
  ring (16 384 frames; a full ring drops and counts); nothing else, nothing allocated
  (`rt_memory`'s `the_master_tap_never_allocates`). Off — the default, and whenever the
  visualiser is not shown or the companion is minimised — it costs one relaxed load a block.
- **The analysis on the interface's side** (`stage::vis`), at most 30 times a second:
  `cantus_dsp::spectrum` (Hann, 2 048 points, both channels' power averaged), 28 bands a
  third of an octave wide from 40 Hz to 16 kHz, −66…−6 dBFS on a 0…1 scale; a band rises at
  once and falls 1.4 of the scale a second.
- **Drawn like a broadcast overlay** (`stage::vis_paint`): rounded bars in the theme's
  accent with a soft glow on a glass plate, or one level bar with round ends; a glass chip
  with the song's name and tempo; one with the bar and its beats, the landing beat lit and
  pulsing (the bar's first in the brand's colour). Each part on or off.
- **In a video** (ADR 0088's `SongFrames.overlay`): `stage::vis_video` analyses the export's
  own render once, at the same 30 instants a second, and draws each frame by tiny-skia at
  its pixel size as a **pure function of its time** — a band is the highest analysis within
  the meter's fall before that time, less the fall since; nothing carried between frames,
  so a frame drawn twice, out of order or on another thread gives the same pixels. Chips
  along the top and the band under them (clear of the dancer and of the video's title line,
  whose song chip it then leaves out); the dialog's *Visualiser* choice, on by default.

### 5. Shortcuts and menus

Keymap 15: `toggle-stage` (Alt+D), `toggle-companion` (Alt+Shift+D), `toggle-streaming`
(Ctrl+Shift+D, Cmd+Shift+D on a Mac — not Ctrl+Alt+D, which a Mac reads as Cmd+Option+D,
the Dock's; every arrangement and view action has a key); a version-14 file gains the
three where they are free.
*View ▸ Stage*, *Companion window*, *Streaming mode*.

## Consequences

- A floating dancer captures the pointer only over its body; the rest of its box is
  see-through to clicks as to the eye.
- With a dancer on screen the whole window keeps its budget: one texture redrawn and
  uploaded a frame while playing, one draw call more (numbers in `docs/round35/stage.md`).
- Stage code lives in `cantus-ui` (GPL): `stage.rs` and the files it declares
  (`stage_geom.rs`, `stage_source.rs`, `stage_companion.rs`, `stage_vis.rs`,
  `views/stage_{float,dock,chrome,companion_view,vis_paint}.rs`); the master tap is the
  engine's (MPL, `master_tap.rs`); the shared files hold marked lines only
  (`// r35:stage`).
- On X11 without an ARGB visual the companion is opaque: its chroma backgrounds serve.

## What the tests hold

`stage_geom.rs`'s tests (the box equals the whole area cut out; buckets; cut to the
visible; the body steady through a jump and a spin; dragging the top sets the size);
`tests/stage.rs` through `CantusApp::frame` (reached from the frame; see-through at physical
pixels at 100 % and 200 %; pinned is foreground, else middle; docked on the tab under the
inspector, hidden nowhere; redraws within 15/s stopped and 60/s playing, none for the same
moment; the breath's wake-up, none hidden or minimised; the frame on hover, lingering);
`tests/frame_budget.rs`'s dancer module (the window with a dancer against the same dancer
hidden; the release timing); `tests/stage_companion.rs` (its key opens it, it draws and
measures itself, closed no pass; its menu's choices carried out and kept; stopped it dances
to a sounding master at the tempo and to the song's position playing; the visualiser reads
the master and the tap runs only while shown; minimised it pauses; the View menu's
switches; the project's dancers followed; an old settings file reads; *Export video…*
under `logic` brings the minimised main window back; a size out of reason brought back);
on Windows `window_layered.rs`'s tests (drawn at the sizing loop's size it keeps it, at the
stored one it snaps back; a size change is no restore; a popup hears its pointer, clicks,
keys and activation, the companion does not; the least size; any rectangle fitted onto a
monitor), `window.rs`'s (`fit_in`), `companion_win`'s (a placed window keeps its physical
corner and its size in points; a window of a pixel kept at the least size; the menu popup
takes clicks with no pass of the main window and closes on Esc) and `companion_menu_win`'s
(below-right of the pointer, flipped at the edges); `stage_vis.rs`'s
tests (a tone lights its band; the meter's fall; the log axis); `master_tap.rs`'s and
`rt_memory`'s; `keymap_14_gains_the_stage_keys`; `tools/obs/stage_hour.py` (the hour).

## Amended by ADR 0106 (round 38: at rest costs nothing)

The stopped breath (12 redraws a second since round 35's measure) **stops once nothing has
happened for `dance_live::DOZE_AFTER` (10 s)**: the dancer holds its pose and the window asks
for no frame for it; the pointer over the window, a key, Play, a drag or a hover wakes it
from the pose it held (the breath's clock stops while it holds). Each breath redrew the whole
window — 17 points of a core at rest, measured. Not while the companion window is open.

Also by ADR 0106 (the owner's decision, 2026-11-08): **the song Cantus opens on has no dancer**
— the house demo's excerpt is built without Trino; the whole demo, the picker and any saved
song with a dancer are as before.
