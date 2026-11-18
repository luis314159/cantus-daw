# Several screens (`fx/multimon`) — phase 1: research and design

Branch `fx/multimon` from `fix/flicker` 7d3be32c (the issue-#1 window work, ADR 0146).
**Phase 1 only**: research, a measured spike, the design (ADR 0148, *proposed*), its
pictures and the questions for the owner. Nothing of the feature is built; the branch adds
the ADR, this report, the strings the design uses (`locales/areas/screens.{en,es}.txt`,
registered in `i18n.rs`) and a picture scene (`examples/shoot_multimon/scene.rs`,
`shoot -- <dir> multimon`) that mocks what does not exist with the kit's own pieces.

## 1. What the best do, and what users say

Sources were the vendors' manuals first, then their forums, KVR, Gearspace and Reddit.
Studio One's manual and most of Cubase's and REAPER's current guides could not be fetched
(403s), so those rows lean on forums and Sound On Sound.

- **FL Studio.** Each window has a *Detached* flag. *Multi-monitor mode* stretches the
  desktop across screens. View ▸ Arrange windows has **one** *Save custom layout*, and
  *Into workspace* pulls back windows stranded on a monitor that is gone. *Detach all new
  plugins*, *Open detached plugins with the FL window*.
  - Detached plug-ins "are seen by Windows as separate applications" (MIDI input and
    device ownership break).
  - Scaling is global and needs a restart.
  - Users ask for named, switchable layouts and for auto-restored multi-monitor
    workspaces.
  - Mixed-DPI crashes were reported (a detached mixer on 1080p beside 300 %).
  - Shortcuts in detached windows need a click first.

  Sources: [toolbar](https://www.image-line.com/support/flstudio_online_manual/html/toolbar_panels.htm) ·
  [View menu](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/menu_view.htm) ·
  [General settings](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_general.htm) ·
  [layout presets](https://forum.image-line.com/viewtopic.php?p=2026929) ·
  [workspaces](https://forum.image-line.com/viewtopic.php?p=2073865) ·
  [mixed DPI](https://forum.image-line.com/viewtopic.php?p=1437165) ·
  [focus](https://forum.image-line.com/viewtopic.php?p=1360452)
- **Bitwig.** Eight *display profiles*: Single large/small, Tablet, Dual Studio,
  Arranger/Mixer, Master/Detail, Studio/Touch, Triple. The footer names both windows'
  views (ARRANGE-MIX). Complaints:
  - "Stuck with their decisions on which window goes to which monitor."
  - The second window is capped to the smaller screen.
  - Windows overlap on one wide screen.

  Sources: [user guide](https://bitwig.com/userguide/latest/a_musical_swiss_army_knife) ·
  [footer](https://bitwig.com/userguide/latest/the_window_footer) ·
  [KVR](https://www.kvraudio.com/forum/viewtopic.php?p=8071936) ·
  [KVR 2](https://www.kvraudio.com/forum/viewtopic.php?p=9232064)
- **Ableton Live.** A second window (Ctrl+Shift+W), and Tab swaps Session and Arrangement
  between them. The browser stays in the main window. Undockable panes and saved
  workspaces have been asked for since 2006. Full screen on macOS will not start on the
  second screen until the window is moved there.

  Sources: [shortcuts](https://www.ableton.com/en/manual/live-keyboard-shortcuts/) ·
  [concepts](https://www.ableton.com/en/live-manual/12/live-concepts/) ·
  [full screen](https://help.ableton.com/hc/en-us/articles/20210918971036-Full-screen-in-Ableton-Live) ·
  [forum](https://forum.ableton.com/viewtopic.php?p=1718421)
- **Studio One.** The console detaches and "can be spread across multiple screens". View
  tweaks are not kept with the song, and a song saved with a plug-in window on screen 2
  hung on open (v6).

  Sources: [SOS](https://www.soundonsound.com/node/4930250) ·
  [KVR](https://www.kvraudio.com/forum/viewtopic.php?p=8104466) ·
  [forum](https://studiooneforum.com/threads/v6-major-issues-anybody-else.1030/post-7543)
- **Logic Pro.** Several main windows. **Screensets** 1–99 sit on the number keys, are
  saved automatically when you leave them, and can be locked, duplicated, imported from
  another project or reverted. They are per project. Starting with the second screen off
  scrambles the sets.

  Sources: [screensets](https://help.apple.com/logicpro/mac/10.1/en.lproj/lgcp9bbbcb23.html) ·
  [Apple discussions](https://discussions.apple.com/thread/8243514)
- **Cubase.** **Workspaces**, global ("Window Layouts.xml") and per project, a key each.
  Up to four MixConsoles can be **linked** across screens. Per-monitor DPI came with a
  per-plug-in fallback. The mixer would not open after the second monitor was unplugged
  (14), and plug-in windows vanish behind the others.

  Sources: [organizer](https://www.steinberg.help/r/cubase-pro/15.0/en/cubase_nuendo/topics/customizing/customizing_workspaces_organizer_r.html) ·
  [dual monitors](https://forums.steinberg.net/t/how-do-you-use-dual-monitors-and-cubase/47594) ·
  [DPI](https://www.steinberg.help/r/cubase-11-using-dpi-unaware-plug-ins-on-windows) ·
  [unplugged](https://forums.steinberg.net/t/solution-mixer-does-not-open-after-disconnecting-the-second-monitor-in-cubase-14-elements/995219)
- **REAPER.** Dockers and floating dockers. Screensets (F3–F5) are global window sets,
  with per-project track views. "Multimonitor v2" per-monitor DPI brought tiny VST2 GUIs.
  Users script a start-up screenset.

  Sources: [docker](https://www.cockos.com/wiki/index.php/Docker) ·
  [SOS](https://www.soundonsound.com/techniques/setting-screen) ·
  [HiDPI](https://reaper.blog/2020/07/multimonitor-v2-hidpi)
- **Common to all of them.**
  - Windows reopen off-screen or on a missing monitor, and the fixes are by hand.
  - Mixed DPI blurs or misplaces.
  - Plug-in windows take the space bar.
  - Windows hide behind the main one.
  - No vendor publishes the cost of a second window.

  Sources: [JUCE forum](https://forum.juce.com/t/vst3-plugin-in-fl-studio-2021-spacebar-not-being-passed-onto-daw/55214) ·
  [Cakewalk](https://discuss.cakewalk.com/topic/64937-vst-plugins-wont-open-in-project-window)

**What we take.**
- From Logic: named, numbered recall on one key.
- From REAPER and Cubase: global windows, separate from the project.
- From Bitwig: ready layouts for two and three screens, without locking anyone in.
- From FL: any area can leave the window.
- From Cubase: windows that stay above the window that opened them.

**What none does, and we do:**
- Remember the windows *per set of screens*, automatically.
- Bring back what is stranded without being asked.
- Keep each window at its own screen's scale with no restart.
- Measure the cost of a window.

## 2. What egui and eframe allow — and the spike

The spike is in `C:\projects\daw-fx\tmp\multimon\spike`, outside the repository. It is
eframe 0.36.1 with glow, three windows (the root and two children) drawing 40 rows of text
each. It ran on the owner's laptop with one screen, `\\.\DISPLAY1`, 1 920 × 1 080 at 125 %.

| Mode | root | window 1 | window 2 (passes / s) |
|---|---|---|---|
| immediate, all animating | 91 | 91 | 91 — always together |
| deferred, all animating (vsync) | **0.7** | 125 | **0.7** |
| deferred, vsync off | **0.7** | 1 952 | **0.7** |
| deferred, children ask 17 ms ahead | **0.7** | 125 | **0.7** |
| deferred, the root's clock drives all | 58 | 115 | 115 |
| … and window 1 animates itself too | **0.7** | 125 | **0.7** |
| **patched eframe**, all animating | 110 | 109 | 110 |
| **patched**, only window 1 animating | 3.7 | 120 | 3.0 |
| **patched**, root's clock + window 1 animating | 124 | 124 | 63 |
| deferred, nothing animating | 2.4 | 1.4 | 1.4 |

**The finding.** On Windows, eframe asks the system to repaint (`request_redraw`, a
`WM_PAINT`). Windows gives `WM_PAINT` to the first window of the thread that needs one, so
a window that asks again during its own paint is the only one ever painted. The patch is
in `check_redraw_requests`, on Windows: paint each due window directly, once per turn, as
eframe already does for invisible windows. It fixes this.

**What else the sources say.**
- Each window has its own system scale.
- egui has one zoom.
- One GL context shares every texture.
- winit lists the monitors.
- A window's position at creation is converted with a scale we do not choose, but it is
  exact once the window exists.

The detail is in ADR 0148's Context.

## 3. The proposed design

The design is ADR 0148 §§1–11.
1. Any area can have a window of its own, with its own dock and the system's decorations.
2. A tab's *Move to a new window* or *Move to ▸* sends an area there, and so does dragging
   a tab out of the window (or onto another window). A device editor opens where it was
   asked.
3. Keys, undo and transport work from every window. Dialogs open over the window that
   asked for them.
4. A window's bar is made of the top bar's own groups.
5. Each window has its own screen's scale, given by its input; the zoom never flips.
6. Windows are the user's, remembered per set of screens, and never the project's.
7. Three built-in layouts for two and three screens. Alt+1…9 recall layouts and F11 is
   full screen.
8. A window whose screen is missing docks back and returns with its screen. Windows are
   created hidden and placed once.
9. A screens map.
10. Nothing draws at rest, per window. eframe is carried with two small patches: fair
    painting, and window handles for plug-in ownership and exact places.
11. The app lives in a UI-thread cell borrowed by each pass.

It also says what it does not do: no windows in the project, no second copy of an area, no
custom title bars, no touch layout yet.

## 4. Cost (estimates, to be measured in phase 2)

- **At rest:** zero frames in every window. The test is the same "at rest" test as the
  main window's.
- **Playing:** each window that shows something moving draws at ≤ 60 Hz. The cost is its
  views' frame time, e.g. a full-screen mixer at 1 080p, the same as the mixer in the main
  window today. Windows showing nothing that moves stay at rest.
- **Memory:**
  - **Per window, in the driver:** its surface's buffers, ~16–25 MB at 1 920 × 1 080 and
    ~65–100 MB at 3 840 × 2 160 (no multisampling, the default).
  - **egui per window:** under 1 MB.
  - **Textures** are shared.
  - Measured with the scorecard's private bytes, one and two windows.
- **The audio thread:** untouched. `rt_memory` is run with two windows drawing.

## 5. Questions for the owner (with my recommendation)

1. **When a screen is missing, its windows' areas dock back into the main window**, and
   return to their window when the screen does. The alternative is that they stay as
   windows on the remaining screen. *Recommended: dock back*, because a laptop alone with
   a mixer window over the arrangement is clutter.
2. **A window's bar** (transport, position, tempo, meter) **on by default** for a window on
   another screen. *Recommended: on.* The alternative is off, with only its menu.
3. **The built-in layouts:**
   - *Two screens: arrange | mix*
   - *Two screens: arrange | edit*
   - *Three screens: edit | arrange | mix*

   *Recommended: these three.* Others can be added, e.g. *mix | arrange* with the main
   window on the right.
4. **Carry a patched eframe** (`third_party/eframe`, as `cpal`) with two small changes —
   fair painting on Windows, and windows' handles — until upstream takes them.
   *Recommended: yes.* Without it a second window freezes the main one while it animates
   (measured). The only alternative is immediate viewports, where every window draws
   whenever any does.
5. **Keys:** Alt+1…9 for layouts (⌥⌘ on macOS) and F11 for full screen. *Move to a new
   window* gets no default key. *Recommended: as said.* Ctrl+Alt is AltGr on your
   keyboard.
6. **A window's own interface scale** follows its screen, with Ctrl +/− per window.
   *Recommended: yes.* Without it the reporter's 1 080p screen beside his 200 % laptop
   shows Cantus at 72 % of its size (90 % where the rule wants 125 %).

**Answered by the owner (2026-11-17):** 1, 2, 4, 5 and 6 yes. 3 was left to us: the set is
in ADR 0148 §7 — Default, *Wide screen: arrange beside mix* (21 : 9), *Two screens: arrange |
mix*, *Two screens: arrange | edit* (top-to-bottom on a portrait screen), *Three screens:
edit | arrange | mix*. Keymap 19, provisionally: `fix/flicker`'s bus feature takes 18
(Ctrl+G). **Merge order: the bus branch first, then `fx/multimon`.**

## 6. The build plan (phase 2, after approval)

Tests come first, each one red before its code. Two builder sessions are estimated, about
5–7 k lines with tests.

**2a — the core.**
- **eframe patches.** Vendor eframe 0.36.1 into `third_party/eframe` with the fairness
  change and `window_of`. The test is an ignored real-window test, plus the spike's
  numbers in the checklist.
- **The windows model.** `windows.rs`: `Window { id, name, workspace, place, bar, on_top,
  scale }`, up to eight. `views::workspace` runs per window, with `app.workspace` swapped in
  for its pass. App-wide queries go across every window.
- **The boot path.**
  - The app moves into the UI-thread cell; `Shell` borrows it.
  - Each window's deferred callback runs `CantusApp::window_pass(ui, id)`.
  - The frame splits into the once-a-tick work (the main window's pass), the views (each
    window) and the frame's end (each window).
- **Input and overlays.**
  - The keymap runs in every window's pass.
  - Overlays open in the window that asked for them (`overlay_host`).
  - F5–F9 act wherever the area lives.
- **The ticker.** A slot per window. Reasons map to the areas they move. A change repaints
  the others once.
- **Scale per window.** In `raw_input_hook`, with the follow rule per window and screen,
  and a `Watch` per window.
- **Layouts.** `Layout { main_place, windows }` and `ScreenKey`, with the current layout
  kept per set of screens (`serde(default)`; an old file reads).
- **Placement.** The rules as one tested function. Windows are created hidden, placed and
  shown. The missing-screen dock-back and return, and the notice.
- **The tab menu.** *Move to a new window* and *Move to ▸*. *Float over this window*.
- **Keymap 18:** Alt+1…9, F11, *Move to a new window*. Strings in both locales.
- **e2e:** `tests/windows_e2e.rs`, as in ADR 0148's last section. It also covers the
  placement table and an at-rest test per window.

**2b — the rest.**
- Drag a tab out or onto another window, tested with synthetic events outside the window's
  rect.
- The window's bar and its menu.
- The built-in layouts.
- The screens map, with its drag.
- Plug-in editors owned by the window that opened them (`cantus-host`'s window gets an
  owner and a place).
- A device editor opens in its window.
- The pictures, reshot from the real build.
- `docs/beta-checklist.md` steps for real screens:
  - laptop 200 % + 1 080p 100 %;
  - unplug and replug at start and while running;
  - sleep and wake;
  - a drag between windows;
  - F11 per window;
  - a plug-in editor from window 2;
  - Linux X11 and macOS.
- The scorecard: memory and frame time with one, two and three windows.
- STATUS.

**Seen only in emulation so far:** everything. The spike ran on one real screen. Mixed
scales, a second screen, unplugging and the drag need a real desk (§6 of the checklist
to come).

## 7. Phase 2 — what was built (2026-11-17)

The owner approved on 2026-11-17 (relayed): questions 1, 2, 4, 5 and 6 yes; 3 left to us
(ADR 0148 §7). Keymap **19**, provisionally: `fix/flicker`'s bus feature takes 18 (Ctrl+G).
**Merge order: the bus branch first, then `fx/multimon`; renumber if that changes.**

**What exists now.**
- **eframe, vendored and patched** (`third_party/eframe`, `third_party/README.md`, its
  notice in `xtask/notices/head.md`). There are two changes, marked `CANTUS PATCH`.
  - On Windows each due window is painted in turn, so no window starves the others.
  - `eframe::window_of(ViewportId)` gives each window's winit handle.
- **Windows of their own** (`src/windows*.rs`, `views/window_bar.rs`):
  - **Viewports.** Each window is a deferred viewport with its own dock, up to eight. The
    application is lent to its pass from the UI thread's cell; `boot::Shell` lends it in
    `raw_input_hook` and takes it back at its next call. The window's dock is swapped
    into `app.workspace` for its pass.
  - **Every dock answers.** Area queries look across every dock (`dock_of`,
    `pane_visible`, `reveal_pane`, `hide_pane`, `toggle_pane_anywhere`). F5–F9 act where
    the area lives, and an area hidden from its window brings that window back.
  - **The central view stays put.** It no longer pulls an editor back from its window.
  - **Moving areas.**
    - A tab's menu has *Move to a new window* and *Move to* (the main window, each
      window, a new window on each screen by name).
    - A tab dragged out of the window becomes a window where it was let go, or joins the
      Cantus window under the pointer.
    - *Float over this window* is the old in-window float.
  - **Keys and dialogs in every window.** The keymap runs in every window's pass with
    its input. Space, M, Ctrl+Z, F9 and Ctrl +/− work from any window.
    - Once-a-pass guards are told apart per window (`windows::pass_nr`; egui counts
      passes per viewport).
    - Dialogs and pickers draw in the window last clicked or typed in.
  - **Device editors and the bar.** A device editor opens in the window being worked in.
    A window's bar shows its menu, its name, the transport, the position, the tempo and
    the meter, with *Back into the main window*. Its menu has *Show the bar*, *Always on
    top*, *Full screen*, *Back* and *Close*.
- **Scale per window** (`windows_pass::adjust_input`):
  - The follow rule for the window's own screen, measured against the main window's.
  - Ctrl +/− in a window scales that window only; Ctrl+0 gives it its screen's scale back.
  - egui's zoom never moves.
- **Screens** (`src/screens.rs`):
  - Read from winit, or a test's list.
  - The placement rules: a moved screen moves its windows, renumbered displays are found
    by size and place, and a window is never off its screen (the title bar's middle
    64 pt stays on it).
  - A window whose screen is gone docks its areas into the main window and comes back
    with the screen, at start or while playing. The status line says both.
  - Windows are made hidden and placed through winit in physical pixels.
  - The main window goes back to its screen once at start.
- **Layouts** (`layout.rs`, `windows_layouts.rs`):
  - A layout keeps its windows, the main window's place and its set of screens. The
    layout last used with each set of screens is remembered (eight sets) and recalled
    when that desk returns.
  - Four built-ins: *Wide screen: arrange beside mix* (offered at 21 : 9), *Two screens:
    arrange | mix*, *Two screens: arrange | edit* (portrait-aware) and *Three screens:
    edit | arrange | mix*.
  - The View menu lists them with Alt+1…9 and has the band *Windows and screens*: *New
    window*, *Gather every window on this screen*, *Put every area back in the main
    window* and *Full screen* (F11).
- **The ticker, one slot per window.** The main window's pass is the clock: it asks a
  window for frames only while an area it shows moves. A click or key in one window
  repaints the others once.
- **Strings** in both locales (`locales/areas/screens.*`).
- **Real-desk steps** are in `docs/beta-checklist.md`, *Varias pantallas*.

**Tests** run as eframe runs windows (`tests/windows_rig`):
- **`windows_e2e`** covers:
  - moving to another screen and the deferred viewport;
  - the scale per window (1.8 and 1.25 on the reporter's desk);
  - keys from the window (Space, M, Ctrl+Z, main repaint asked);
  - at rest no window draws, and playing draws the mixer's window but not the
    browser's;
  - kept and reopened, unplugged at start, replugged while running, unplugged while
    playing;
  - F9 out and back;
  - Ctrl+ per window;
  - a tab torn out onto the other screen.
- **`windows_screens`** covers the main window on 11 kinds of screen and the mixer's
  window on 3 desks:
  - exact scale per window;
  - every word inside its window;
  - nothing dropped from the bars;
  - the window's menu opening in its window.
- **`windows_stability`** covers:
  - 500 open/move/close cycles (no area in two docks, no viewport kept, textures flat);
  - 50 unplug/replug cycles while playing (the song never stops);
  - 300 idle frames (no scale flip, no frame asked, the watchdog quiet);
  - five corrupt, old or strange layout files.
- **Unit tests**: `screens::tests` (the placement table), and the keymap
  (`every_preset_has_the_layout_keys_and_full_screen`).

- **The screens map** (View ▸ *Screens and windows…*, `views/screens_map.rs`): every
  screen to scale with its name, size and both scales, the windows on it (a waiting
  window's screen dashed, *Not connected*); drag a window onto another screen, double-click
  to raise it; Esc closes it.
- The View menu offers only the built-in layouts the screens allow (a one-screen user's
  menu grows by its *Windows and screens* band alone); the rescue rows show once a window
  is open.

**Closed after the first account** (the owner's "no leftovers", 2026-11-17):
- **Plug-in editors owned by the window that opened them**, placed on its screen, and given
  to the main window before that window goes. See ADR 0148's Phase 2 section and the
  checklist's real-plug-in steps.
- *Choose what the bar shows…* and *Rename the window…*. They are user settings kept with
  the layout, with no undo.
- **The measured cost.** See §8.

**Still not done:**
- `rt_memory` with two windows. The engine is untouched (no file under
  `cantus-engine`/`cantus-rt` changed), and the rig's processor runs on the test's thread,
  so the audio thread's allocations cannot be told apart there.

**Verified only in emulation:** everything above. The tests feed screens and passes as
winit and eframe do. The spike ran on one real screen. The checklist lists what a real desk
must confirm: placement, hidden-then-shown, unplugging, sleep, the drag between real
windows, F11, plug-in windows, and Linux and macOS.

**Run on this branch** (Windows, 2026-11-17):
- **The window tests:** `windows_e2e` 8, `windows_screens` 3 and `windows_stability` 4.
- **The library's unit tests:** 568 pass, 2 ignored. These include `screens::tests`, the
  keymap tests and the i18n test, which requires every English key in Spanish.
- **The existing suites this change touches:** `app_flows` 74, `keyboard_reach` 6,
  `menus_in_window` 2, `menus_at_scale` 2, `panels` 6, `at_rest_e2e` 6, `idle_repaint_e2e` 2,
  `frame_budget` 19, `click_paths` 17, `boot_shell` 1, `stage_companion` 13,
  `window_stability` 3 and `kit_lint` 4. All pass.
- **What this changed in the existing suites:**
  - The View menu keeps its height: no Default row and no band, and the rescue rows only
    appear once a window is open.
  - *Move to a new window* and *Screens and windows…* are actions, on Shift+F11 and
    Ctrl+F11. The test requiring every View action to have a key wants that.
  - The old *Float in its own window* key now reads *Float over this window*.
- **Not run:** the whole workspace, and Linux. The integrator runs both, with
  `tools/wsl/check.sh`. Nothing added is behind `cfg(windows)`.
- **Third-party notices:** `THIRD-PARTY-NOTICES.md` was regenerated, since eframe now comes
  from `third_party/`.

**Pictures as built:** `compartir/multimon/design/v2/` holds the main window on the laptop,
the mixer's window on the external screen, and the desk, in both locales. They match the v1
design.

## 8. The cost of a window, measured (2026-11-17)

**How it was measured.**
- **Machine:** the owner's laptop, one screen of 1 920 × 1 080 at 125 %.
- **Build:** this branch's `cantus.exe` (`f924e944`… `fdf6226a`, dev profile, optimised, no
  LTO — not a release build).
- **Script:** `tools/scorecard/windows.ps1`, following the scorecard's rule for *at rest*:
  measured from 30 s to 40 s after spawn. It took 3 runs each, plus one unmeasured warm-up.
- **Data folders:** `cargo run -p cantus-ui --example multimon_measure`. Each one opens the
  start-up song (the house drop, no dancer) in three cases:
  - **1 window:** the main window alone, maximised.
  - **2 windows:** the mixer in a window of its own, 960 × 510 px, side by side over the
    right half.
  - **3 windows:** the mixer plus the piano roll in a third window.
- **Playing** is the same start with `--play`.
- **Frames per window** are Cantus's own counts (`CANTUS_WINDOW_STATS=1`).
- Raw data: `docs/round-fx/multimon-measured/`.

**The machine was not quiet.** Five other builders were compiling: the whole machine was
67–100 % busy during every measured run. CPU shares are therefore noisy (±2 % of a core at
rest), and playing is dominated by the engine running a dense song on a loaded machine. The
memory numbers are steady across runs.

| | Private bytes | GPU memory charged to the process (shared + dedicated, at rest) | CPU at rest (share of a core) | Main window at rest | Each extra window at rest |
|---|---:|---:|---:|---:|---:|
| 1 window | 164.4–166.0 MB | 51–53 + 3–5 MB | 3.1–5.2 % | 0.3 passes/s, 8–11 ms | — |
| 2 windows | 164.4–165.8 MB | 69–90 + 5 MB | 1.7–4.5 % | 0.2–0.3 passes/s | 0.2 passes/s, 2.0 ms |
| 3 windows | 169.8–171.1 MB | 87–100 + 5–18 MB | 2.7–5.5 % | 0.3 passes/s | 0.2 passes/s each, 1.3–1.9 ms |

**Playing.**
- **CPU:** 168–199 % of a core with 1 window, 175–217 % with 2, 207–233 % with 3. The
  rise is within the noise of the busy machine; it is the engine's work, not the windows'.
- **Main window:** 44–78 passes a second at 2.0–3.1 ms each.
- **The mixer's window:** 67–77 passes a second at 0.9–1.25 ms each.
- **The piano roll's window** in the 3-window case logged no passes in the windows sampled,
  so its playing cost is not shown here.
- **GPU while playing:** with 2 and 3 windows the counters read 5–8 GB "dedicated" and
  16–23 GB "shared". That is more than the laptop has, so those samples are unreliable and
  are not used. The at-rest readings, taken the same way, are steady.

**What it says.**
- **Memory.** A second window costs no measurable private memory (within 1.5 MB). A third
  costs about 5 MB. The GPU driver charges 16–37 MB per 960 × 510 window: its surface's
  buffers, the estimate's size for a window that small. The ADR's 16–25 MB per 1 080p
  window was low by about that factor.
- **At rest nothing draws.** A window of its own woke about once every 5 s for 1.3–2 ms
  (the main window's own wake-ups, 0.3/s, were there before), so the CPU at rest is
  unchanged within the noise.
- **Playing.** The mixer's window costs about 1 ms a frame, a third to a half of the main
  window's frame. That is the cost of drawing the mixer, not of having a window.

To do on a quiet machine with a release build, before the beta's scorecard publishes these
numbers: run the same script, read the piano roll's passes while playing, and take the GPU
counters again.

**The whole cantus-ui suite** (2026-11-17, after the leftovers):
- 322 of the 323 test binaries pass, and cantus-host's library tests pass (64, including
  `owner_tests`). Clippy `-D warnings` and fmt are clean for both crates.
- **The one failure:** `scene_vector::a_lid_drag_rebuilds_within_its_budget`, a
  2-second wall-clock budget on Chorda's picture rebuild. Run alone on the same busy
  machine it failed twice and passed once. It times code this branch does not touch
  (`scene::image`), so the load is the likely cause; it should be run again on a quiet
  machine.
