# ADR 0149 — The window holds still: one scale decision per screen, a watchdog, a safe-graphics start

> **Provisional number** (branch `fix/flicker`): renumber at the merge if 0149 is taken.

**Status:** Accepted (fix for GitHub issue #1 on `cantus-daw`) · **Date:** 2026-11-17 ·
**Amends:** ADR 0111 §7 (the interface's scale follows the screen), ADR 0024 §12 (the
splash becomes the main window) · **Builds on:** ADR 0005's spirit for the UI thread (no
work that grows per frame), ADR 0050 (the boot path)

## Context

An outside user reported (issue #1, beta.14 portable, Windows 11 25H2, Ryzen AI 9 465 with
a Radeon 800M iGPU): *"The user interface keeps flickering along both the horizontal and
vertical axes"*, every start. We could not reproduce it on our machines.

Reading the code paths that can make a window or its layout change frame after frame — the
viewport commands (`boot::become_main_window`, `stage_companion::show_main_window`, the
title and the icon), the zoom (`set_zoom_factor` at assemble, Ctrl +/−, and
`screen_scale`), the DPI handling, the splash's transition — found one that loops by
construction:

- ADR 0111 §7 makes a fresh installation's interface scale **follow the screen**: our zoom
  is chosen from the monitor's width in physical pixels. `screen_scale::wanted` read that
  width as `monitor_size × native_pixels_per_point`, believing egui's `monitor_size` to be
  in points of the system's scale. **It is not**: egui-winit 0.36
  (`update_viewport_info`) divides the monitor by `egui_winit::pixels_per_point`, which is
  `zoom_factor × native_pixels_per_point`. So the width measured was `physical ÷ zoom`, and
  the zoom chosen changed the width measured, which changed the zoom: on a 1 920-px screen
  at 100 % the zoom went 1.0 → 1.25 → 1.0 → … every other frame (a zoom set in a pass
  applies at the start of the next, after the viewport's info was read), and the whole
  interface was redrawn 25 % larger and smaller, both axes, forever.
- It loops on most screens: 1 920 px at 100 %, 175 % and 200 %; 2 560 px at 100–150 %;
  2 880 px (the usual Ryzen AI laptop panel) at **every** scale; 3 840 px at 100 % and
  125 %. It holds still on a handful — among them 1 920 × 1 080 at 125 %, the owner's
  laptop, which is why nobody here saw it. It never depended on the GPU.
- The unit test of `wanted` fed `monitor_size` by hand in the units the code assumed, so it
  agreed with the code's mistake (the lesson of round 36: test against the outside
  source — here egui-winit's own conversion).

A test that drives `CantusApp::assemble` with frames fed as egui-winit feeds them
(`tests/window_stability.rs`) failed on the unfixed code on 39 of 50 screen × scale ×
maximised cases (the scale alternating, the top bar's rectangle and every word moving);
the remaining 11 settled.

## Decision

1. **The screen's width is measured in the units egui-winit gives**
   (`screen_scale::monitor_px`: monitor points × the zoom in effect before the pass × the
   system's scale), **only while the zoom has held for a pass** (the pass after a change
   was measured with the other zoom), and **decided once per screen** (its width in pixels
   and the system's scale): `screen_scale::Follow`, kept in `CantusApp::screen_follow`. The
   zoom it sets can no longer change what it measures; moving to another monitor decides
   again; the user's own scale stops it as before.
2. **A watchdog, always on** (`window_watch::Watch`, in `CantusApp::window_watch`): every
   frame it compares the window's state (inner and outer rectangles, the system's scale,
   our zoom, maximised, full screen, the monitor) with the last. A change with no input
   (a click, a key, the wheel, a zoom gesture, focus, a drop) that **returns** to one of
   the last four states is a loop's signature — a border dragged makes new sizes, not the
   same two. Six such returns within one second trip it: the last sixteen transitions go
   to the log as a warning, the screen-following scale stops (the zoom in effect stays),
   and geometry commands sent through `window_watch::send_geometry` are dropped for the
   session. Fixed arrays, no allocation per frame. `CANTUS_WINDOW_TRACE=1` logs every
   change and every scale decision.
3. **Cantus's log is a file**: a release on Windows has no console (`windows_subsystem`),
   so `env_logger`'s output went nowhere a user could send. `crates/cantus-app` now writes
   it to `logs/cantus.log` in the data folder as well as the console (portable:
   `<folder>\data\logs\`; installed: `%LOCALAPPDATA%\Cantus\logs\`; Linux: the XDG data
   folder's `logs/`), the previous start's kept as `cantus.previous.log`, with
   `egui_glow=debug` by default so the OpenGL driver's version, renderer and vendor are on
   record. The graphics options of the start are logged on one line.
4. **A safe-graphics start** (`graphics_options::Graphics`, `LaunchOptions::graphics`):
   `--safe-graphics` (no splash, scale 100 %, vsync on, no multisampling, no transparency)
   and its parts `--vsync on|off`, `--msaa N`, `--no-splash` (the main window opens at
   once, decorated, maximised or at `--window`'s size; the boot sends it only its title),
   `--scale F` (our scale for the session, set once, never stored); the same as words in
   `CANTUS_GRAPHICS`, a flag winning over the variable. `crate::native_options` and
   `boot::main_window_commands` are the tested functions the start goes through.
   `docs/troubleshooting-graphics.md` documents them in English and Spanish.
5. **The wgpu renderer is not added now.** It would be a second renderer in the one build
   (eframe's `wgpu` feature: wgpu, naga and the DX12/Vulkan/GL back ends) — several MB in
   each of the two programs the zip carries (`x86-64-v2` and `-v3`) and minutes on every CI
   build — to work around a GPU fault nobody has shown: the loop found is not the GPU's.
   It stays a proposal, measured first, if a report survives this fix and the safe-graphics
   start.

## Consequences

- A fresh installation's scale settles on the rule's value in a few frames and never moves
  again on any screen; the window state is checked every frame for the next loop of the
  kind, which will be logged and broken on a machine we never see.
- `screen_scale::wanted` (a free function) is replaced by `Follow::wanted`.
- The splash, when the main window opens at once, is drawn at its own size in the middle.
- What only the reporter's machine (or an AMD iGPU at 125/150 %) can confirm is in
  `docs/beta-checklist.md`.

## What the tests hold

- `tests/window_stability.rs`: through `CantusApp::assemble` on a fresh data folder, with
  egui-winit's conversion, 5 screens × 5 system scales × maximised/restored, the default
  top bar and its automation/typing variants: once settled no frame sends a viewport
  command, the scale holds, the top bar's rectangle and every painted word stay put; the
  scale reached is the rule's; a pinned `--scale` holds and is not stored.
- `screen_scale::tests::the_followed_zoom_settles_and_never_measures_itself`: 8 widths ×
  5 scales through a bare egui context.
- `window_watch::tests`: a 1-frame and a 2-frame synthetic oscillation trip it within a
  second, once; a resize drag, changes that came with input and slow returns never do; a
  tripped watch drops geometry commands; the log file keeps the previous start's.
- `graphics_options::tests` and `tests/graphics_options.rs`: every flag and word parses,
  reaches `NativeOptions` (vsync through `glow_options`, multisampling, the viewport's
  decorations, size, maximise, transparency) or the boot's command list, and the boot and
  `run` go through those tested functions.
