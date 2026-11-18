# ADR 0148 — Several screens: any area in a window of its own, the windows remembered per set of screens

> **Provisional number** (branch `fx/multimon`): renumber at the merge if 0148 is taken.

**Status:** Accepted (the owner's answers of 2026-11-17, at the end; phase 2 builds it) · **Date:** 2026-11-17 · **Builds on:** ADR 0024 §5 and §13 (the dock, the repaint
ticker), ADR 0110 and 0111 (the top bar, maximise, the scale that follows the screen),
ADR 0146 (the window holds still), ADR 0091 §2 (the companion's window), ADRs 0019, 0065
and 0073 (plug-in editors in windows of their own), ADR 0050 (the boot path) · **Report:**
`docs/round-fx/multimon.md` · **Pictures:** `compartir/multimon/design/v1/`

## Context

The owner (2026-11-17): *"a feature for multiple monitors, to extend Cantus's screens over
several monitors if one wants; see how to do it, finish defining the idea for maximum
usefulness and do it in the best, excellent way."*

**What Cantus has.** One OS window. Its areas are tabs of one `egui_dock` dock
(`views::workspace`); an area can *float*, but only inside that window (an `egui_dock`
window surface). Named layouts (`layout.rs`, `layouts.json`, the user's — never the
project's) keep the dock's tree and two sizes; the View menu lists them. The main window's
place is not remembered: it opens maximised where the system puts it. Two things are
already OS windows: the companion's stage (ADR 0091: a layered Win32 window on Windows, a
deferred egui viewport elsewhere) and every hosted plug-in's own editor (owned by the main
window, so it stays above it). Every view is called with `&mut CantusApp` from the one
frame (`CantusApp::frame`), which also runs the frame's work once (the engine's poll, the
controllers, the MCP server, the autosave) and asks the ticker for the next frame.

**What the best do** (sources in the report):

| | How | Praised | Complained of |
|---|---|---|---|
| FL Studio | each window *Detached* from the FL desktop; one *Save custom layout*; *Into workspace* pulls stranded windows back | any window anywhere | one custom layout; detached plug-ins are separate apps (MIDI, focus); keys need a click first; global scaling with a restart; mixed-DPI crashes |
| Bitwig | 8 *display profiles* (single, dual studio, arranger/mixer, master/detail, studio/touch, triple) | one click to a good shape; a touch profile | fixed presets: "stuck with their decisions"; second window capped to the smaller screen |
| Ableton Live | a second window (Ctrl+Shift+W), Tab swaps Session/Arrangement | simple | only two windows since 2006; the browser stays in the main one |
| Studio One | the console detaches, spreads over screens | | view state not kept with the song; a song with a plug-in on screen 2 hangs on open |
| Logic | several main windows; **screensets** 1–99 on the number keys, saved as you leave, lockable, per project | the best recall model | a screen off at start scrambles the sets |
| Cubase | **workspaces**, global and per project, a key each; up to four mixers linked across screens; per-monitor DPI with a per-plug-in fallback | linked mixers | mixer will not open after a screen is unplugged; plug-in windows vanish behind |
| REAPER | dockers, floating dockers; **screensets** (global window sets, per-project track views) on F-keys | flexible | fiddly dock places; users script a start-up screenset |

Across all of them: windows reopen **off-screen on a monitor that is gone**, fixed by hand;
**mixed DPI** blurs or crashes; **keys** go to whatever window has the focus (the space bar
swallowed by a plug-in window); windows **hide behind** the main one; nobody ships a
measured cost for a second window.

**What egui and eframe 0.36 allow** (read in their sources; a throw-away spike measured on
the owner's laptop, one screen of 1 920 × 1 080 at 125 %, `tmp/multimon/spike`, not in the
repository):

- **Viewports.** `show_viewport_immediate` draws a child window inside the parent's pass
  (the closure may borrow `&mut CantusApp`) — the parent and every child draw together,
  always. `show_viewport_deferred` gives each window passes of its own, but the callback is
  `Fn + Send + Sync + 'static`: it cannot borrow the app.
- **One OpenGL context, a surface per window** (glow): textures — the font atlas, the kit's
  layer atlas, the instruments' pictures — are shared; nothing is uploaded twice.
- **Each window has its own system scale**: egui-winit reads every window's
  `scale_factor()` and re-lays it out on `ScaleFactorChanged`, so a window at 200 % beside
  one at 100 % is crisp on both, without a restart. **egui's zoom is one per context**
  (`Options::zoom_factor`): our interface scale is the same in every window unless we
  adjust a window's input (§5).
- **Starvation on Windows (measured).** When a deferred window asks for its next frame
  during its own pass — as egui does for any animation or drag — Windows hands `WM_PAINT`
  to the first window of the thread that needs one, and only that window is painted:
  window 1 at 125 passes a second, the main window and window 3 at **0.7**, with vsync,
  with a 17-ms request, with vsync off (1 950 against 0.7). Driving every window from the
  main window's clock alone works (all three at 58–87 a second) until one window animates
  itself; then it starves the others again. **A 25-line change to eframe's
  `check_redraw_requests`** — paint each due window directly, once per turn, as eframe
  already does for invisible windows — fixes it: all three at ~110 a second animating;
  one animating at 120 while the others stay at rest (~3 a second: the spike's own 1-Hz
  clock). No upstream report was found.
- **Monitors**: winit's `available_monitors()` through `eframe::Frame::winit_window()` in
  the main window's pass: name (`\\.\DISPLAY1` on Windows, the RandR output on X11, the
  localized name on macOS), physical size and position, scale. Full screen on a chosen
  monitor: `ViewportCommand::SetMonitor`. A window's *place* at creation goes through a
  logical position whose conversion winit does with a scale we do not choose; once the
  window exists, `OuterPosition` is exact (its own scale × points = pixels).
- **Linux** runs X11 (ADR 0065; XWayland on Wayland), so windows can be placed and a drag
  keeps reporting the pointer outside its window; **macOS** gives each full-screen window
  its own Space.

## Decision

1. **Any area can have a window of its own** — arrangement, piano roll, channel rack,
   mixer, Routing, inspector, browser, devices, clips, history, controllers, score,
   session, stage, and any instrument or effect editor. Each window is an OS window with the
   system's decorations (Snap layouts, Win+Shift+arrows, the macOS green button work as in
   every program) and **its own dock** (`Workspace`): tabs, splits and the kit's skin as in
   the main window, so a window can hold the mixer with Routing beside it, or the piano
   roll over the channel rack. The **main window** keeps the top bar, the status bar and
   the menus; closing it quits; a window of its own closes into the main window (its areas
   go back where they belong). Up to **eight** windows besides the main one.

2. **Making one, three ways, none hidden.**
   - A tab's menu: *Move to a new window* (a window on the screen the main window is not
     on, when there is one — else beside it), and *Move to ▸* the main window, another
     window by name, or *New window on screen N · its name*. The old *Float in its own
     window* becomes *Float over this window* (it was always inside the window).
   - **Drag a tab out of the window**: released outside every Cantus window, it becomes a
     window there; released over another Cantus window, it docks in the leaf under the
     pointer (the pointer keeps reporting while the button is held, on Windows, X11 and
     macOS).
   - View ▸ *New window*, and the built-in layouts (§7).
   A device editor opens **in the window it was opened from**: double-click an insert on
   the mixer's screen and its editor opens there.

3. **Every window is a full citizen of the keyboard and the project.** The keymap is read
   in the pass of the window that has the focus, with that window's input: the space bar,
   record, undo (Ctrl+Z — one history for the project), F5–F9, the palette and every
   binding work from any window, never needing a click first. A **text field** keeps the
   keys as in the main window. **Dialogs, the picker, the palette and menus open over the
   window they were asked from** (the window whose pass handled the input that opened
   them). F5/F6/F7/F9 act on their area wherever it lives (a window showing it hides it,
   and brings it back to the same window). Every change to the project is a `Command` as
   before; **windows are not the project**: moving, opening or closing a window is not a
   command and is not undone (as in every DAW).

4. **A window's bar.** A window of its own may show a thin **bar** at its top, made of the
   **top bar's own groups** (ADR 0052): by default the transport, the position, the tempo
   and the master meter, chosen with the same list as the top bar (*Choose what the bar
   shows…*). At its left the window's menu (*Show the bar*, *Choose…*, *Always on top*,
   *Full screen* F11, *Rename*, *Back into the main window*, *Close*) and its name; at
   its right *Back into the main window*. On by default for a window on another screen
   (you mix there and should not have to look across to stop the song); off by its menu.

5. **Each window at its own screen's scale.** The system's scale is honoured per window by
   egui-winit (§ Context). **Our** scale (ADR 0111 §7) follows each window's own screen:
   a window on the laptop at 200 % gets the rule's 90 %, one on a 1 920-px screen at
   100 % gets 125 % — otherwise the external screen would show everything at 72 % of the
   size it should have. egui has one zoom, so a window's own scale is given by its input,
   before its pass, in `raw_input_hook` (which eframe calls for every window): its
   `native_pixels_per_point` multiplied by *its scale ÷ the zoom*, and every point it
   carries (the screen's rect, the pointer, the viewport's rects) divided by the same,
   the commands we send it (its size, its place) multiplied back. The zoom itself never
   changes per pass — ADR 0146's loop cannot come back — and the follow rule decides
   **once per window and screen**, as ADR 0146 §1 does for the main window; a user's own
   scale (Ctrl +/−) applies to the window that has the focus and is kept per window
   ("this window at 110 %"); *Interface at 100 %* resets that window. Each window has
   its own `window_watch::Watch`.

6. **Windows are the user's, remembered per set of screens — never the project's.**
   A layout (`layouts.json`) gains the main window's place and the other windows: for each
   its dock, its place (outer rectangle in the desktop's **physical pixels**), maximised
   or full screen, its screen (`ScreenKey`: the system's name, its size and position in
   pixels, its scale), its bar's groups, *always on top*, its own scale. The **set of
   screens** connected (their keys, sorted) is a fingerprint, and Cantus keeps **the
   current layout for each set** it has seen (the last eight): the studio's three screens
   have their windows, the laptop alone has its own, and plugging the screens back in
   brings the studio's back, at start or while running. A project never opens windows: it
   travels between machines whose screens differ (Logic's and Studio One's complaints).
   Named layouts stay global; a named layout made on two screens and applied on one docks
   what had no screen into the main window (§8).

7. **Layouts for every desk, one key each.** The built-in layouts are chosen for the desks
   people have, not for a count of screens (the owner left the set to us, 2026-11-17):
   - *Default* — one screen, as today.
   - *Wide screen: arrange beside mix* — **one** window, the mixer to the right of the
     arrangement instead of under it; offered when the main window's screen is at least
     21 : 9 (3 440 × 1 440, 5 120 × 1 440). An ultrawide screen is two screens' room
     without a bezel, and a second window on it only overlaps.
   - *Two screens: arrange \| mix* — the main window keeps the arrangement, inspector,
     piano roll and rack; the other screen gets the mixer with Routing. The most common
     desk (a laptop with one screen; two screens with the lid closed) and the most asked
     for (FL's detached mixer, Bitwig's Arranger/Mixer, Studio One's console).
   - *Two screens: arrange \| edit* — the other screen gets the piano roll over the
     channel rack, the score beside the piano roll: for writing. On a **portrait** screen
     its split is top-to-bottom (the piano roll tall, the rack under it), which is what a
     portrait screen is good for.
   - *Three screens: edit \| arrange \| mix* — the arrangement in the middle, the
     editors left, the mixer right (the studio's three screens, Bitwig's Triple).
   The "other" screen is the largest screen the main window is not on (a laptop's own panel
   is the smallest, so the mixer goes to the external screen whichever is primary); with
   three, left and right follow the screens' positions. A built-in layout that needs more
   screens than are connected is not offered (phase 2: dimmed rows only lengthened a
   one-screen user's View menu); picking a layout made for
   more screens than are connected opens what fits and docks the rest (§8). **Alt+1 …
   Alt+9** recall the View menu's layouts in order (Alt is ⌥ on macOS); Ctrl+Alt is not
   used — it is AltGr on Spanish and many other keyboards (AltGr+2 is @). **F11** puts the
   window in front into full screen on its own screen and back, **Shift+F11** sends the area
   being worked in to a new window, **Ctrl+F11** opens the screens map. The View menu lists
   the built-ins the screens allow, then the user's layouts (*Reset layout* is the way back to
   the default), and Alt+1 … Alt+9 follow that order. The new bindings are checked against
   every keymap preset for collisions (a test).

8. **A window on a screen that is not there is brought back, always.** At start and
   whenever a window's place or screen changes (an event, never a timer), the screens are
   read again; a window whose screen is missing **docks back into the main window** (its
   areas where they belong) and the status line says *The screen "…" is not connected:
   its areas (…) came back into the main window. They go back when it returns.*; when that
   screen returns, its windows return where they were. A window whose screen is there but
   whose place is not (a resolution changed, a screen moved) is moved so that at least its
   title bar's middle 64 points lie on that screen's work area, its size clamped to it.
   A window is **created hidden**, placed with `OuterPosition` in its own scale and shown
   once placed: no window jumps across the desk, and no geometry command is sent twice
   (window_watch drops a loop as for the main window). View ▸ *Gather every window on this
   screen* is the manual rescue (a screen that is switched off but still connected is
   invisible to every program), and *Put every area back in the main window* undoes it all.

9. **The screens map.** View ▸ *Screens and windows…* shows every screen to scale where
   the system places it — its name, its size, the system's scale and ours — and the
   windows on each. Drag a window to another screen; double-click one to bring it to the
   front. It is also where a dimmed (missing) screen and its windows are seen.

10. **Nothing draws at rest — per window.** The ticker (ADR 0024 §13) keeps a slot per
    window. The main window's pass is the clock: whatever moves, moves for a reason
    (playback, recording, meters, a live analyser, an animation, a drag), and **only the
    windows that show an area moving for that reason** are asked for their next frame
    (`request_repaint_after_for`): a window holding only the browser draws nothing while
    the song plays; one holding the mixer draws its meters. A change made in one window
    (the project's revision, the selection) asks the others for one frame. Input repaints
    only its own window. The frame's once-a-tick work stays in the main window's pass, and
    every window's pass ends with what must follow a change at once (the engine's sync).
    **eframe is carried with the fairness change** (`third_party/eframe`, like `cpal`:
    each due window painted directly, once per turn, on Windows), offered upstream with
    the spike as its test; and eframe is asked for each window's winit handle
    (`eframe::native::window_of(ViewportId)`, a second small change) so a hosted plug-in's
    editor opened from a window is **owned by that window** and stays above it, and
    places are set in physical pixels.

11. **How the app reaches a deferred window's pass.** The application stays where it is
    (`boot::Shell`, owned by eframe) in name, but the running application lives in a
    cell of the UI thread (`thread_local!` holding a `RefCell<Option<Box<CantusApp>>>`):
    the main window's pass and each window's deferred callback — which captures only its
    window's id, so it is `Send + Sync + 'static` — borrow it for their pass (eframe runs
    every pass on that thread, one after another; no `unsafe`, no lock). A pass that finds
    it borrowed (never seen; a nested native dialog's message loop) draws nothing and asks
    again. In tests with `embed_viewports` the same window function is called with the
    app directly. The audio thread is untouched: nothing here runs on it.

## What we do not do

- **No window layouts in the project** (§6), and no undo of window moves.
- **No per-window copy of an area** (two mixers showing different strips, Cubase's linked
  mixers): one mixer exists; it can be as wide as a window across two screens of one
  scale. Two mixer views are a later decision (BACKLOG).
- **No top bar in another window** — the bar of §4 is made of its groups; the top bar
  itself goes where the main window goes.
- **No custom-drawn title bars** on secondary windows: the system's decorations keep
  Snap layouts, full screen and window managers working.
- **No touch layout for a second screen** in this round (Bitwig's *Studio/Touch*): noted
  in the BACKLOG with the tablet/iOS line.
- **Plug-in editors are not docked** into Cantus's windows; they stay windows of their own
  (owned by the window that opened them, §10).
- **No wgpu**: glow serves several windows (ADR 0146 §5 stands).

## Consequences

- The dock becomes one per window; `app.workspace` is the main window's and, during a
  window's pass, that window's (swapped in, as the frame already takes it out with
  `std::mem::take`); app-wide questions (*is the mixer showing?*) ask every window.
- `layouts.json` grows (new fields, `serde(default)`): an older Cantus reads the main
  window's dock and ignores the rest; no project schema change, no `SCHEMA_VERSION` bump.
  The keymap gains the layout keys (Alt+1 … Alt+9), F11 (full screen), Shift+F11 (*Move to a new window*) and Ctrl+F11 (*Screens and windows…*): **keymap 19**
  (provisional — the bus feature built in parallel on `fix/flicker` from the same base takes
  18 for Ctrl+G *route the selected to a new bus*; merge that branch first, then this one,
  and renumber if the order changes).
- A patched eframe is carried until upstream takes the two changes (`third_party/README.md`
  says what changed and why); each eframe upgrade re-applies them.
- Each extra window costs its surface's buffers in the driver (estimated, measured in
  phase 2: ~16–25 MB at 1 920 × 1 080, ~65–100 MB at 3 840 × 2 160, no multisampling) and,
  only while it draws, its views' frame time; at rest nothing.

## What the tests will hold (phase 2)

`tests/windows_e2e.rs`, through `CantusApp::assemble`, run as eframe runs windows
(`embed_viewports` off; the root pass, then each window's deferred callback in a pass of its
own with its own `RawInput`, as `glow_integration::run_ui_and_paint` does): a tab's
*Move to a new window* opens a deferred viewport whose pass draws the mixer, gone from the
main dock; a click in it mutes a strip (one command; Ctrl+Z in its input undoes it); the
space bar in its input starts the transport; the main window is asked for a frame after the
change; the layout is stored and a fresh assemble reopens it at its place; with that screen
missing the mixer is back in the main dock, the status line says so, and the screen's
return brings the window back; a window at 2.0 beside the main at 1.0 lays every word of the
mixer inside its rect at its own scale (the kit's text cuts recorded), the font atlas not
rebuilt between passes; at rest no window asks for a frame, playing asks only the windows
whose areas move; window_watch trips on a window's synthetic loop and only on its own;
placement rules as a table (missing, moved, shrunk, negative coordinates, a window larger
than its screen, the scale changed); `rt_memory` unchanged with two windows drawing; the
strings in both locales; the keymap migration. What only real screens confirm goes to
`docs/beta-checklist.md`.

## The owner's answers (2026-11-17)

1. A screen missing: its areas dock back into the main window and return with the screen —
   **yes** (§8).
2. A window's bar on by default on another screen — **yes** (§4).
3. The built-in layouts — "evaluate yourself, to make an excellent product": the set and
   its reasons are §7 (ultrawide, portrait, laptop + one screen, two screens with the lid
   closed, three screens).
4. Carry the patched eframe in `third_party/` — **yes** (§10).
5. Alt+1…9 for layouts, F11 for full screen — **yes**, checked against every keymap preset;
   keymap 19 provisionally (Ctrl+G is the bus feature's).
6. Each window follows its own screen's scale, Ctrl +/− per window — **yes** (§5).

And his emphasis: *"make sure it looks excellent on different screens and that it stays
stable and works well"* — phase 2 adds a screen matrix (1 366 × 768 to 3 840 × 2 160,
ultrawide, portrait, mixed pairs and three screens) through `CantusApp::assemble`, and a
stability suite (500 scripted open/close/move cycles with memory and textures flat, screens
unplugged and replugged at start and while playing, a soak with two windows animating, every
window at zero frames at rest, corrupt and old layout files).

## Phase 2 (2026-11-17): what was built, what was not

Built as decided: §§1–8, §10 and §11, keymap 19 (provisional; the bus feature's 18 merges
first). The tests are `tests/windows_e2e.rs`, `tests/windows_screens.rs` (11 kinds of screen,
3 desks) and `tests/windows_stability.rs` (500 window cycles, 50 unplug/replug cycles while
playing, 300 idle frames, corrupt layout files), run as eframe runs windows
(`tests/windows_rig`). Two details the building settled:

- **A window's pass number.** egui counts passes per viewport, so the application's
  once-a-pass guards (the keys, Esc, a text field's focus, the kit's brush and cache) use
  `windows::pass_nr`, which keeps the main window's count and tells a window's apart.
- **The central view.** Only the main window's lower zone sets it; an editor another window
  holds is never pulled back into the main dock by it.

The screens map (§9) is built (`views/screens_map.rs`), and the three left after the first
account are closed (the owner's "no leftovers"):

- **A plug-in's editor belongs to the window that opened it** (§10). cantus-host's editor
  window keeps its owner (`PluginWindow::owner`, `set_owner`: `GWLP_HWNDPARENT` on Windows,
  `WM_TRANSIENT_FOR` on X11, noted on macOS where the editor floats above every Cantus
  window anyway) and opens where it is asked (`window::place_next`: a little in from the
  corner of the window that opened it, on its screen). Before a Cantus window goes —
  closed, docked back, its screen unplugged, a layout applied — `PluginHost::reown_windows`
  gives its effects', instruments' and ARA editors to the main window, so Windows never
  destroys an editor with its owner: the editor stays open where it is. Held by
  `cantus-host`'s `owner_tests` on real Win32 windows (an editor given to another owner
  outlives the first) and `windows_e2e::a_windows_plug_in_editors_are_its_own_and_outlive_it`
  (the owner and place chosen, the release before every way a window goes).
- **A window's bar and name** (§4): *Choose what the bar shows…* lists eight of the top
  bar's groups to check and uncheck (transport, position, tempo, metronome, grid, undo, meter,
  performance), *Rename the window…* opens the kit's dialog; both are the user's settings,
  kept with the layout (`WindowShape::groups`, `name`) — **no undo**, as for every window
  setting: they are not the project. `windows_e2e::a_windows_bar_and_name_are_chosen_from_its_menu_and_kept`.
- **The cost, measured** — the section below.

**The cost, measured** (2026-11-17, `tools/scorecard/windows.ps1`, the owner's laptop on its
one 1 920 × 1 080 screen at 125 %, a dev-optimised build, the machine 67–100 % busy with other
builds — `docs/round-fx/multimon.md` §8 has the method and every run):

- **Private bytes:** 164–166 MB with one window, the same with two, about 5 MB more with
  three.
- **GPU memory charged to Cantus, at rest:** 51–53 MB with one window, 69–90 MB with two, and
  87–100 MB with three — **16–37 MB per 960 × 510 window**. The estimate in Consequences was
  low for a window this size.
- **At rest:** each window of its own woke about once in 5 s for 1.3–2 ms. The CPU share is
  unchanged within the noise (1.7–5.5 % of a core in every case).
- **Playing:** the mixer's window drew 67–77 frames a second at 0.9–1.25 ms each, against the
  main window's 2.0–3.1 ms.
- **Unreliable:** the GPU counters' samples while playing with two and three windows read more
  memory than the machine has, so they are not used.

To repeat on a quiet machine with a release build before these numbers go in the scorecard.

`docs/round-fx/multimon.md` §7 has the account, and `docs/beta-checklist.md` the steps only
a real desk can confirm.

## An instrument's editor in a window of its own (fx/final, 2026-11-18)

An instrument's editor moved to a window of its own (*Move to a new window*, a tab dragged
off a screen), or opened while the user works in another window, opens at **its own size**,
as it does floating in the main window (ADR 0125's amendment, `views::inst::editor_size`):

- **The size.** The window gets the editor's body, plus the window's bar, the tab bar and the
  margins, at the zoom that window will use on its screen (`screen_scale::for_screen` against
  the main window's rule). The window is not maximised. That size is clamped to the target
  screen's work area and centred on the drop point or the screen
  (`Workspace::device_place`). winit does not report a screen's work area, so the work area is
  taken as the screen less a 48-pixel taskbar and 8-pixel margins at its scale. Once the user
  resizes or moves the window, the saved place is used (§6), and their size is kept.
- **Two faults found on the way, both fixed.** An editor moved to a window of its own was
  forgotten on the next frame: the editors' registry dropped every entry the *main* window's
  pass had not seen, so it dropped the entry drawn in the other window. Each entry now
  remembers the window that drew it (`Entry::seen_in`) and is forgotten only by that window's
  own pass count. An editor in a window that was not the one hosting the dialogs was also never
  drawn, because only the host drew the editors. Now every window that holds a device's tab
  draws the editors (`CantusApp::device_editors`).

`windows_e2e::an_instruments_editor_in_a_window_of_its_own_opens_at_its_own_size` holds this
on an external screen and on a 1 366 × 768 screen too small for the editor: the window is on
that screen, inside its work area, the body is Chorda's 1 100 × 680 where it fits, and the
editor is still drawn after a click in the main window. It then goes back to the main window.
`an_instruments_editor_opened_in_a_window_opens_there_at_its_own_size` holds the editor
opened while working in the mixer's window: it opens there, whole.
