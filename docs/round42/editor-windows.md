# Round 42 — editor windows that behave (builder `editor-windows`)

The owner, 2026-11-25 (translated): *"When you open an instrument (also a plug-in) to
configure it and then close it, closing it should bring back the screen you had before
opening it — the mixer, the arrangement, or whatever was showing. I'd also like to pin a
plug-in or instrument so I keep seeing it if I want, and/or bring several onto the screen, so
that clicking something else doesn't make them disappear."*

Decision record: [ADR 0161](../adr/0161-editor-windows-restore-and-pin.md) (provisional
number). Branch `r42/editor-windows`, based on `r42/integration` a223a36fc.

## What was wrong (before)

**Closing showed the wrong view.** An instrument's editor is a device tab of the dock, put in
the arrangement's leaf — or, when the arrangement is not on the main surface (the mixer
maximised by a double-click on its tab, or the arrangement hidden with `F5`), in the lower
zone's leaf, whose tabs are *Mixer, Piano roll, Channel rack*. The editor became the front
tab; egui_dock, removing a tab at or before the active one, activates **the tab before it**.
So:

- opened over the maximised mixer (or the mixer with the arrangement hidden), F8 → the
  editor docks over the mixer; ✕ → the **channel rack** is in front, and the central view
  follows it. This is the owner's case.
- opened over the **session view** (its tab is before the arrangement's), an instrument too
  tall for the leaf floats at once — but the frame it spent docked already moved the leaf's
  front: the **arrangement** came forward at *opening*, and stayed at closing.
- the tab's own ✕ was egui_dock's removal, with the same result.

**"It disappears".** Every family held one editor (`inst.open`, `fx.open`,
`eq_editor.open`, `comp_editor.open` — the three effect slots closed one another —
`pulsus.open`, `fx.hosted.inst_open`): opening the next instrument or effect closed the one
on screen. A docked editor also went behind the arrangement whenever the arrangement's tab
was clicked (they shared a leaf). A floating one did not go anywhere on a click — what
vanished was the editor replaced by the next one opened.

Reproduced by `tests/editor_windows_e2e.rs` and the unit tests `device_leaves_tests` in
`views/workspace.rs` — each fails on the old code at the assertion named in it.

## The rules (after)

1. **The view comes back.** A device tab that leaves its leaf *while it is in front* (closed
   by its ✕, its tab's ✕, Esc, F8, the tray, F12, its track or effect deleted, minimised — or
   floated) brings forward the tab it covered when it came to the front, if that tab is still
   in the leaf, with the keys' focus. **If it was behind another tab** — the user brought the
   mixer, the piano roll or the arrangement forward while it was open — nothing changes:
   closing never pulls the user back to a view they left. The layout is not touched; views
   keep their scroll.
2. **The pin** (a push pin left of minimise on every device window — instruments, Pulsus,
   effects, a hosted plug-in's Cantus view): a pinned editor is **not replaced** when another
   opens, **floats** if it shared a leaf (so no tab click can hide it), **stays above** the
   editors that are not pinned, and is **not closed by an Esc aimed elsewhere** or by F12.
   Its own ✕, Esc after a click in it, F8 on it, the tray and its track deleted close it.
3. **Several at once.** Settings ▸ Editing ▸ *Editor windows ▸ Editors open*: **One at a
   time** (default; today's behaviour: opening closes the family's unpinned editors) or
   **Several at once** (none close). Families as before: the effects across their three
   slots; native instruments, Pulsus and a hosted instrument's Cantus view each.
4. **Each its own state.** One editor per family in its slot, the others parked with their
   own state and swapped in to be drawn. Two keyboards each play their own track; an editor
   closing with keys held sends their notes off.
5. **The engine reports on the editor touched last** — one probe, one analyser, as before;
   the others show their meters at rest until clicked. Nothing new on the audio thread.
6. **Deleting a track or an effect closes its editor**, pinned or parked; **undo brings the
   track back and no editor**.
7. A new floating editor **opens where it covers the others least**: beside them on a wide
   screen.

## What changed

| File | What |
| --- | --- |
| `src/editor_windows.rs` (new) | The setting `EditorsOpen`, the parked editors (`Editors`), the rules (`opening_closes`, `live_of`), the generic family operations (park, promote, draw, close, prune, touch), the app side (`draw_editor_windows`, `sync_editor_engine`, `prune_editors`, `toggle_editor_pin`, `set_editors_open`, `room_for_*`, `unpark_*`, deferred opens). |
| `src/editor_windows_park.rs` (new) | The six families and what each parked editor keeps of its own (`InstSlot`, `PulsusSlot`, `HostedSlot`, `FxSlot` with its kind's state through `views::fx::Editor`, `EqSlot`, `CompSlot`). |
| `src/views/workspace.rs` | `device_leaves` (the view comes back), `detach_device`, `covered`/`fronts`/`watch_fronts`, `hide` for devices, `place` records what a device covers; F12 and the hovered Esc skip pinned devices; unit tests `device_leaves_tests`. |
| `src/views/workspace_frame.rs` | A device tab's ✕ is taken out by the workspace (`OnCloseResponse::Ignore`, then `hide`); `after` watches the fronts. |
| `src/views/workspace_device.rs` | `Entry::{pinned, layer}`, `Pending::Float`, `is_pinned`/`set_pinned`/`toggle_pin`, `note_layer`, `keep_pinned_above`, `free_spot`/`place_beside`; unit tests. |
| `src/views/device_chrome.rs` | `ctl` shows the pin; `on_event` handles `Event::Pin`; `Pending::Float` carried out; floats go through `detach_device` and `free_spot`; the instrument window helpers know parked editors; `Spot::InstKeys`. |
| `src/kit/window_ctl.rs`, `src/kit/shell.rs`, `src/kit/glyph.rs` | The pin key (`WindowCtl::pin`, `WindowRects::pin`, `WinHot::Pin`, `Event::Pin`), its paint and tooltips; `Glyph::Pin` (leaning) and `Glyph::Pinned` (upright). |
| `src/views/inst/mod.rs` | `open_inst_editor`/`close_inst_editor`: deferred while a parked editor draws, a parked one unparked, room made; the probe through `sync_editor_engine`; `inst_telemetry` at rest for a parked one. **Lines 246–289** of the file as merged (the three functions only). |
| `src/views/inst/frame.rs` | **Two places**: the `Event::Pin` arm joins the window buttons' arm (line 484), and the keyboard publishes `Spot::InstKeys` (lines 1156–1160, one statement). |
| `src/views/fx/mod.rs`, `eq_editor.rs`, `comp_editor.rs`, `fx/host.rs`, `fx/frame.rs` | The effects' opens make room or unpark; the analyser through `sync_editor_engine`; the pairs, telemetry, EQ deltas and compressor meter only for the analysed editor; the fx frame's `Event::Pin`; a first opening placed by `free_spot`. |
| `src/views/pulsus_editor.rs`, `src/hosted_view.rs`, `fx/fxd.rs`, `fx/fxd_window.rs` | Pulsus and the hosted view as families; the controllers' windows have no pin. |
| `src/app.rs` | `CantusApp::editors`; `device_editors` calls `draw_editor_windows`; `prune_editors` after exec, exec_merging, undo, redo; another project forgets the parked editors. |
| `src/settings.rs`, `views/clipfx_settings.rs` | `Settings::editors_open` (`#[serde(default)]`), its row in Settings ▸ Editing. |
| `locales/areas/editor_windows.{en,es}.txt` (new), `src/i18n.rs` | The pin's tooltips and status, the setting. |
| `examples/shoot_editors/scene.rs` (new), `examples/shoot.rs` | `shoot <dir> 1 editors`. |
| `docs/adr/0161-…`, `docs/manual/{en,es}/README.md` | The ADR; the manual's *Device windows* paragraph. |

No schema change: window state stays outside the project (ADR 0161 §5). Settings is not
versioned; the new field defaults.

## Screenshots

`cargo run -p cantus-ui --example shoot -- docs/round42/editor-windows 1 editors`:

- `docs/round42/editor-windows/editors-pin-before.png` — the title bar's keys as they were.
- `docs/round42/editor-windows/editors-pin-after.png` — the leaning pin left of minimise.
- `docs/round42/editor-windows/editors-pin-after-hover.png` — under the pointer.
- `docs/round42/editor-windows/editors-pin-after-pinned.png` — pinned: upright, lit.
- `docs/round42/editor-windows/editors-several-before.png` — one at a time: the second
  instrument replaced the first.
- `docs/round42/editor-windows/editors-several-after.png` — Calor pinned, Modulus and a
  Terminus open with it beside the mixer.

## Tests

New (19):

- `tests/editor_windows_e2e.rs` (7, its own binary, `CantusApp::assemble` on a real
  processor): `closing_an_instrument_opened_over_the_mixer_brings_the_mixer_back`,
  `an_instrument_opened_over_the_session_or_the_arrangement_leaves_it_in_front`,
  `closing_never_pulls_the_user_back_to_a_view_they_left`,
  `a_pinned_instrument_stays_through_an_effect_a_click_elsewhere_and_esc`,
  `several_at_once_keeps_every_editor_open_without_a_pin`,
  `two_instruments_open_play_their_own_held_notes_and_close_with_notes_off`,
  `deleting_a_track_closes_its_pinned_editor_and_undo_brings_no_editor_back`.
  **Checked against the old behaviour**: with the restore rule switched off, the first two
  fail exactly as the owner described (`left: ChannelRack` where the mixer was;
  `left: Arrangement` where the session was).
- `views::workspace::device_leaves_tests` (5): closed in front, floated out, closed behind
  another tab, stacked devices, over the session.
- `views::workspace_device::tests` (2 new): `a_device_opens_beside_the_others_where_there_is_room`,
  `a_pin_is_kept_by_the_registry_and_asks_a_shared_tab_to_float`.
- `editor_windows::tests` (2): rule 1 and rule 4.
- The glyph family (`kit_glyphs`) now walks `Pin` and `Pinned` (67 glyphs) and passes.

Run green on Windows (`-p cantus-ui`): the lib (635 passed, 2 ignored) and `editor_windows_e2e`
7, `device_ux` 22, `quick_gestures` 23, `hosted_instruments` 5, `hosted_plugins` 3,
`inst_frame` 4, `dancer_window` 11, `delay_window_e2e` 9, `comparatio_drag` 5,
`preset_browser_e2e` 3, `r41_whole` 4, `ux_signs_e2e` 1, `settings_scroll` 2, `kit_glyphs` 5,
`kit_lint` 4, `fx_lint` 1. `cargo clippy -p cantus-ui --all-targets -- -D warnings` clean.
Not run: the workspace suite and the WSL (Linux) check — the integrator's.

## Verified only in tests

Everything here was verified headless — egui frames fed as egui-winit feeds them, a real
processor driven block by block, the CPU renderer for the pictures. Not verified on a real
screen: dragging a pinned floating window against the dock's snapping, a pinned editor in a
second window of its own (ADR 0148), and a hosted plug-in's Cantus view pinned with a real
plug-in loaded (its family is the same code as the effects'; the e2e test uses native units).
These go in `docs/beta-checklist.md`'s editor-windows steps.
