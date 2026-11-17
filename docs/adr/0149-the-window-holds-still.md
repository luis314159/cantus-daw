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
- Run on the owner's laptop (2026-11-17, a fresh data folder, 1 920 × 1 080 at 125 %): the
  window opened on its Radeon iGPU (`opengl renderer: AMD Radeon(TM) Graphics`, driver
  22.40.01.54), the log named it, one scale decision and no other change in 20 s, with and
  without `--safe-graphics`. That screen never looped; the screens that did were verified
  only in emulation. What only the reporter's machine (or an AMD iGPU at 100/150 %) can
  confirm is in `docs/beta-checklist.md`.

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

## Amendment 1 — menus at a high scale (the same issue's follow-up, 2026-11-17)

The reporter wrote again (beta.14 portable, 2 880 × 1 800 at Windows' 200 %): *"the
instrument preset selection menus are opening up transparent and far away from where they
should be … in the right-click context menu on the mixer channels, some of the sub-menus are
popping up way higher than they're supposed to … everything opens in the right spot when I
switch scaling back to 100 %. That change doesn't fix the issue with the instrument preset
menus."* Two bugs, neither a points/pixels mix-up — egui works in points throughout, and
the scale only made the window 852 points tall instead of 1 752:

- **The desk's submenus** (`views/mixer_menu.rs`, and `views/route_menu.rs` the same way)
  were one egui `Area` for every submenu of a menu. An area remembers its size; the scroll
  area inside took that size as its room, so a submenu opened after a shorter one (the
  colour swatches, *Dock*) was squeezed into a 64-pt scroll — egui's least — while it was
  **placed by its whole height** (`at.y.min(bottom − 8 − h)`). In a 1 752-pt window that
  lift mattered only near the foot; in an 852-pt window it threw most submenus hundreds of
  points above their rows (*Add effect* at 200 %: its row at y 508, its submenu at 186).
  The emulation measured it at 100 % too (*Send to*: row 1 409, submenu 1 254).
- **An instrument's pop-ups** (`views/inst/frame.rs`: the preset list, the ⋯ menu, the
  sizes) opened at a fixed place, `rect.right() − 260 × k` under the header — the window's
  right edge, wherever the preset well was (it is centred) — and in a bare `Area`.
  `widgets::menu_list` decided "in a popup, the popup's frame is the sheet" from the layer
  alone (round 39), so on a bare foreground area its rows were painted on nothing: in
  Slate and its siblings (the default) the list was **transparent**, at every scale. The
  same held for every other bare-area list: the audio popover's lists, the settings' lists,
  the dock tab's menu, the mastering assistant's list, the route menu.

**Decision.**

6. **A menu's scroll is as tall as its list** (at most the window's room), never the room
   its area offers: the desk's lists, the route menu's and the modulators' list set
   `max_height` and `min_scrolled_height` to it (as `kit::menu::fit` already did for egui's
   sizing pass), and **each submenu is an area of its own** (named by its row, or by its
   kind), so none inherits another's size or scroll; the placement counts the sheet's edge.
7. **A list opens at the control that opened it**: `kit::menu::place` (the rule
   `fx::frame::popup_list` had: under it, above it, raised over it, or scrolling) is the
   kit's, used by the effects' pop-ups and the instruments'. `shell::Output::parts` gives
   the header's parts where the shell drew them; an instrument's preset list opens under
   its preset well from its left edge, the ⋯ menu and the sizes end at their keys' right
   edges, a band's mix field (Chorda's, Tympanum's) opens the list under itself
   (`InstEditors::popup_from`). The list sits on the kit's popup sheet
   (`kit::automate::frame`) and scrolls when the window is shorter.
8. **A menu list paints its own sheet unless it lies on one**: `widgets::sheeted` asks
   egui's `UiStack` whether the frames up to the area fill it opaquely (an egui popup's
   menu frame, the kit's sheet), not only which layer it is on.

**What the tests hold.** `tests/menus_at_scale.rs`, through `CantusApp::assemble`, fed as
egui-winit feeds a 2 880 × 1 800 screen (`paths_rig::Physical`) at the system's 100, 125,
150 and 200 %, with the zoom the screen-following scale picks at 200 % (0.9) and a user's
1.25 on top: a strip's right-click menu with each of its seven submenus hovered (scrolled
to where the window is short) — each beside the menu, level with its row, raised no
further than the window's foot asks, inside the window, no row hidden behind a scroll; an
instrument's preset list, ⋯ menu and sizes — each at its control, inside the window, and
opaque: a colour painted over everything beneath the pop-ups shows through none of their
points. Both failed on beta.14's code at every scale (see the round's notes for the
numbers) and pass. `tests/menus_opaque_themes.rs` holds the sheet itself: in Slate Night,
Slate Paper, Slate, Paper, Bordeaux, Lagoon, Print and High contrast light, once the
opening fade has settled, every row's empty part of the preset list is the theme's menu
colour in the unit's face, alpha 255, and the list reaches its last preset (whole where
the window holds it; by the wheel in a 1 280 × 720 window at 200 %). The first
after-pictures caught the list in egui's opening fade (the shoot scene drew six frames
after the click); the scene now lets it settle. What only a real 200 % screen confirms is
in `docs/beta-checklist.md`.
