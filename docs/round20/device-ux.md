# Round 20 — device-ux: device windows that are easy to close, and swapping devices

Builder **device-ux**, branch `r20/device-ux`, on the contract commit 403ca06. Owner of the
device windows' chrome, `CantusApp::apply_pick` and every entry point that opens the picker.

The owner's words (2026-10-05): *"it is hard to know you have to unfold an option to close or
minimise an instrument — I want something more graphical"*; swapping an instrument for
another, and a mixer effect for another, *"easy and intuitive"*.

## For STATUS

**Device windows have their own buttons.** Every device window — the eight instruments and
Pulsus II (dock tabs), every effect and the hosted plug-ins' generic editor (floating) — now
carries at the right end of its title bar a **Swap** cap (a glyph and the word), then
**minimise**, **maximise / restore** and **close ×**, drawn with the kit (`kit::window_ctl`):
flat at rest, lit on hover, sunk while pressed, the close key turning the danger colour under
the pointer; each with a tooltip and the hint bar's line. Close used to sit at the bottom of
the ⋯ menu; minimise and maximise did not exist.

- **Minimise** sends a device to the **device tray** in the status bar: a chip with its mark
  in its hue and its name ("Calor (Keys)"); a click brings it back where it was, its × closes
  it. The device stays open (its state, its keyboard, its meters) — only off the screen.
- **Maximise** fills the workspace (a docked instrument floats over it; an effect's window
  grows to it); **restore** puts it back — docked again if it was docked, at its rect if it
  floated.
- **Esc** closes the device window clicked last; **F8** opens or closes the selected track's
  instrument window, and brings a minimised one back (`Action::ToggleInstrumentWindow`,
  editable in the keymap).
- **Where a device floats is remembered** per device for the session: an effect's editor
  closed and opened again comes back where it was dragged (the old frame shared one position
  across every effect), a floating instrument tab floats there again.

**Swapping keeps what matters** (`apply_pick`, `crate::swap`), one undo step each:

- *Instrument*: the track, its clips and notes, strip, inserts and colour stay; its name too,
  unless it was a default one ("Track 3", or the old instrument's own name), which then
  follows the new instrument. Between native instruments the **matching settings** come
  along (`CantusApp::swap_keep.matching`, on by default): the eight macros and their names,
  every value whose key the new kind also has, the modulation connections whose source and
  target it has. A kit → melodic swap keeps every note (tested on what the engine would play).
  The window that showed the old instrument shows the new one, where it was; its A/B and
  preset name are forgotten (they belonged to the old one).
- *Effect*: a new core command, `Command::ReplaceEffect { strip, effect, slot }`, puts the new
  slot **at the old one's index** — with its mix, bypass, and its key and note source where
  the new kind takes them — and its inverse puts the old slot back exactly. Refused, like
  its sibling commands, for a key or a note source the new kind cannot take and for an id
  already in the chain; in the undo fuzz.
- *Add*: an instrument becomes a new track named after it ("Organum", "Organum 2"), at the
  place asked, selected, its window open; an effect goes at the end of the chain, its editor
  open.
- A choice that does not fit (an effect for a track; a hosted **instrument**, until ADR 0028
  lands — `swap::available` is false and the picker can grey the tile) changes nothing and
  says why.

**Every entry point opens the picker** (`app.open_picker(...)`):

| Where | Gesture | Purpose |
|---|---|---|
| Every device window's title bar | the **Swap** cap | ReplaceInstrument / ReplaceEffect |
| Track header | the instrument icon is now a chip: hover raises it with a ▾, click | ReplaceInstrument |
| Track header | drop a device from the browsing panel | ReplaceInstrument |
| Arrangement's **+** key | click (right-click: instrument track, empty track, audio track) | AddInstrument |
| Arrangement's empty-timeline menu | *Add instrument track…* | AddInstrument |
| Channel rack's channel button | right-click, or the swap badge shown on hover (a left click now opens/closes the instrument, FL) | ReplaceInstrument |
| Inspector's instrument field | click (right-click: the old quick list) | ReplaceInstrument |
| Inspector's empty insert tile | click (right-click: the quick list, now headed by *All effects…*) | AddEffect |
| Inspector's insert tile menu | *Swap for another effect…* | ReplaceEffect |
| Mixer's empty slot (Extra large) | click (right-click: the quick list, *All effects…* first) | AddEffect |
| Mixer's filled slot | the swap badge shown on hover; right-click menu *Swap for another effect…*; drop a device | ReplaceEffect |
| Mixer's empty slot | drop a device | AddEffect |

## Files

Mine (new):

- `crates/cantus-ui/src/kit/window_ctl.rs` — the window buttons: layout, painter, tooltips.
- `crates/cantus-ui/src/views/device_chrome.rs` — the buttons' events, the tray, Esc and F8,
  where devices float and maximise (an `impl Workspace`), the chip, the swap badge, the drop
  targets, published geometry (`Spot`).
- `crates/cantus-ui/src/swap.rs` — what adding and swapping keep (`apply_pick`'s work).
- `crates/cantus-core/tests/replace_effect.rs`, `crates/cantus-ui/tests/device_ux.rs`.
- `crates/cantus-ui/examples/shoot_devices.rs` — the design review's renders.
- `crates/cantus-ui/locales/areas/swap.{en,es}.txt`, this file.

Mine by the contract: `crates/cantus-ui/src/picker.rs` (`apply_pick`).

Touched outside my area (small, local; each says "round 20"):

- `crates/cantus-core/src/command.rs` — `ReplaceEffect` (variant, label, apply with its
  inverse, `check_effect_routes`). `tests/undo_invariants.rs` — one arm inside arm 14's inner
  match (`rng.below(4)` → `rng.below(5)`, the new arm 4; no other arm renumbered).
- `crates/cantus-ui/src/kit/shell.rs` — `Header::window`, `Event::{Swap, Minimise, Maximise,
  Close}`, `HeaderRects::window`, `Hot::Window`, `Output::window`; the buttons sensed and
  painted; the swap cap goes compact when the name would be squeezed. `kit/glyph.rs` — four
  glyphs (Minimise, Maximise, Restore, Replace). `kit/mod.rs` — `pub mod window_ctl`.
- `crates/cantus-ui/src/views/workspace_device.rs` — the registry grew: minimised, maximised,
  area-hosted effects, places, focus, published rects.
- `views/workspace_frame.rs` (two calls), `views/workspace_status.rs` (the tray, one line),
  `views/mod.rs`, `lib.rs` (modules), `app.rs` (`theme_ctx` → `pub(crate)`, `swap_keep`, the
  Esc arm, the F8 arm), `keymap.rs` (`ToggleInstrumentWindow` on F8).
- `views/inst/frame.rs`, `views/pulsus_editor.rs`, `views/fx/{frame,host,mod}.rs` — the
  header's buttons and their events; `FxHost::{device_key, swap}` (defaults: none, so the
  exported plug-ins' windows are unchanged); `fx::device_id`; `open_window_sized` registers
  the effect as a device. `views/inst/mod.rs` — `InstEditors::forget`.
- Entry points: `views/arrangement.rs`, `views/channel_rack.rs`, `views/inspector.rs`
  (`Act::Pick`), `inspector_sheet.rs` (`pick_field`), `inspector_track.rs`,
  `inspector_channel.rs`, `inspector_menus.rs`, `mixer_parts.rs`, `mixer_menu.rs`,
  `mixer_strip.rs` (`Control::Slot`).
- `crates/cantus-ui/src/raster.rs` — `shoot_at` (a resting pointer, for hover renders).
- Tests updated for the header's new field: `tests/kit_parts.rs`, `tests/fx_frame.rs`,
  `examples/proof/device.rs`.

## Decisions worth knowing

- **One behaviour for minimise everywhere**: the tray, not FL's collapse-to-title-bar — a docked
  tab has no title bar to collapse to.
- **An instrument opens floating when its dock leaf cannot hold it.** ADR 0024 §5 docks
  devices by default; at 1600 × 1000 the arrangement's half of the window is ~380 pt tall and
  an instrument is designed at 680, so a docked instrument's cards overlapped (render
  `before`/`r2`). The first time a device opens, if the leaf it would dock in is shorter or
  narrower than the device's minimum size, it floats at its own size over the workspace;
  dragged into the dock, it stays docked (and is remembered so). Effects already floated.
- **Floating windows are moved by re-creating them.** egui keeps a window's old size for the
  frame it moves, so a restored window was pushed against the workspace's corner; restore and
  reopen detach a fresh window and settle its position for three frames. egui_dock 0.21 never
  fills `WindowState::rect`, so a floating tab's place is read from its leaf.
- **A swap gives the new effect a new id** (another device: nothing that pointed at the old
  one silently changes kind). Its editor reopens where the old one was (`inherit_place`).
- **F8** toggles the instrument window. FL's F8 is its plug-in picker; the browser builder did
  not bind a key, so there is no clash today — if the picker wants F8, the integrator picks.

## Tests

- `crates/cantus-core/tests/replace_effect.rs` (4): place, one undo step, master chain,
  refusals leave nothing behind, key and note source carried. Undo fuzz arm in
  `undo_invariants.rs` (green).
- `crates/cantus-ui/tests/device_ux.rs` (22), every gesture through published geometry
  (`workspace_device::buttons`, `device_chrome::spot`, `mixer::control_rect(Control::Slot)`):
  close by the ×; minimise to the tray and back by its chip; the tray's × closes; maximise and
  restore (floating and docked); an instrument too tall for its leaf floats; Esc closes the
  device clicked last; F8 opens and closes; an effect's window has the same buttons and they
  work; the swap button opens the picker in place; an effect reopens where it floated;
  instrument swap keeps the track and is one undo step; default names follow; kit → synth
  keeps every note (on what the engine would play); matching settings come along (and do not
  when off); effect swap keeps place, mix, bypass and key, one undo step each; adding names,
  places and opens; a choice that does not fit changes nothing; header chip, + key, rack
  button (click, right-click, badge), inspector field and empty tile, mixer empty slot and
  swap badge each open the picker with the right purpose.
- Whole `cantus-ui` suite run with `--no-fail-fast`: every binary green (49 binaries,
  goldens, lints, frame budgets included). `cargo clippy -p cantus-core -p cantus-ui --tests
  --examples`: clean. `cargo fmt`: applied.
- Renders: `cargo run -p cantus-ui --example shoot_devices -- <dir>` (instrument with its
  buttons, close hovered, an effect with swap hovered, the tray, maximised, the header chip,
  the mixer's and the rack's swap badges; Slate and Paper). Looked at each; fixed: the tray
  was painted under the status bar's ground, the chip's ▾ ran into the name, the rack's hint
  described the old click, the docked instrument overlapped (above).

## Left

- **Hosted instruments**: `swap::available` returns false for `PluginKind::Instrument` and
  `apply_pick` says so; when ADR 0028's API lands (`set_hosted_instrument` or what it is
  named), call it in `CantusApp::replace_instrument` / `add_instrument_track` and flip the TODO
  in `swap::available`.
- **Drop targets**: track headers and mixer slots take a `DevicePayload`; the inspector's
  tiles and the arrangement's empty lanes (→ AddInstrument) do not yet.
- **Keep matching settings** is a field (`CantusApp::swap_keep.matching`), on by default; the
  picker (browser) should offer it as a toggle next to *Replace*.
- A floating device tab shows egui_dock's tab × beside the shell's ×; both close it. Hiding the
  dock's for device windows is the workspace skin's call.
- Remembered places last the session, not across launches (a device is an effect id or a
  track, which belong to a project).
- Keyboard focus of the window buttons (Tab, Enter) is not wired: they are pointer buttons
  with tooltips and keys of their own (Esc, F8).

## Notes for others

- **Integrator**: replace ADR 0029's placeholder §9 with the text below. `picker.rs` here
  carries `DevicePayload` copied verbatim from `r20/browser` so both sides add the same lines;
  if git still conflicts, keep one copy. `CantusApp::swap_keep` sits after `pulsus`, away from
  the browser's `picker_view`. The browser's `views::picker::open` replaces `open_picker`'s
  body; nothing here depends on its contract body. `global_shortcuts` is off while the picker
  is open (browser), which also keeps F8 and Esc to the picker then.
- **Browser**: call `crate::swap::available(&purpose, &choice)` to grey a tile that cannot be
  applied (hosted instruments, until ADR 0028); `CantusApp::swap_keep.matching` is the
  "keep matching settings" choice.
- **Automation**: a swapped effect gets a new `EffectId`; automation targeting the old one
  should follow the slot or be dropped (the old id no longer exists). Decide in ADR 0027;
  `ReplaceEffect` is the place to hook it (its inverse restores the old id).
- **Hosted-inst**: a hosted instrument's generic editor gets the window buttons for free if it
  is shown through `DeviceWindow` with `.hue(...)` and passes `device_chrome::ctl(...)` as the
  shell header's `window`, handling `Event::{Close, Minimise, Maximise, Swap}` with
  `device_chrome::on_event` (see `views/inst/frame.rs`).

## ADR 0029 §9 — Swapping (device-ux)

The browser's draft of ADR 0029 (on `r20/browser`) leaves §9 to this builder. To avoid an
add/add conflict on the ADR file, the text is here; **the integrator replaces the placeholder
§9 with it** at the merge.

> ## 9. Swapping (device-ux)
>
> **What a swap keeps.** `apply_pick` (`crate::swap`) does every purpose as **one undo step**.
> An instrument swap is one `Batch` (`SetInstrument`, and `SetTrackName` when the name was a
> default one: empty, "Track *n*" in the interface's language, or the old instrument's own
> name); the track, its clips, notes, strip, inserts and colour are untouched, because an
> instrument is the only thing that changes — a note is a pitch for a kit and a synth alike,
> so a kit → melodic swap keeps every note. Between two native instruments the shared model
> (ADR 0023) lets the **matching settings** come along — the eight macros and their names,
> every number whose key the new kind also declares (clamped to its range), every
> non-file text it also takes, and the modulation connections whose source the new kind
> produces and whose target it has, compacted from slot 1. It is on by default and the
> application holds the choice (`CantusApp::swap_keep.matching`) for the picker to offer.
> The A/B side and the preset name of the old instrument are forgotten; the window that
> showed it shows the new one, where it was.
>
> **`Command::ReplaceEffect { strip, effect, slot }`** — rather than a remove and an insert —
> puts a whole new slot at the old one's index; its inverse is the same command with the old
> slot. It keeps the slot's **position**, **mix** and **bypass**, and its **key** and **note
> source** where the new kind takes them (a key into a kind with no detector, a note source
> into a kind that hears none, a key that would close a loop, an id already in the chain are
> refused, as their own commands refuse them; `apply_pick` drops a route the new kind cannot
> take, and retries without routes if the routing moved). The new slot has a **new id**: it is
> another device, and nothing that pointed at the old one (an open editor, an automation
> target, the engine's instance) silently changes kind under it. Parameters do not come along
> between effect kinds: they share no model.
>
> **Adding** names an instrument's track after it ("Organum", then "Organum 2"), inserts it
> where asked, selects it and opens its window; an effect goes at the end of the chain with its
> editor open (FL opens a plug-in it loads). A hosted instrument is not available until ADR
> 0028's API lands (`swap::available` says so, so the picker can grey it); choosing it changes
> nothing and says why.
>
> **The entry points** (every one calls `open_picker` with the purpose its place means): the
> **Swap** cap in every device window's title bar; the track header's **instrument chip**
> (the icon, raised with a ▾ under the pointer); the arrangement's **+** and its empty-timeline
> menu; the channel rack's channel button (right-click, or the swap badge shown on hover; its
> left click now opens and closes the instrument, as FL's does); the inspector's instrument
> field, its empty insert tiles and its tiles' menu; the mixer's empty slots, the swap badge
> on a filled slot and its menu. A right-click keeps the old quick list where there was one,
> headed by *All effects…*. **Drop targets** read `DevicePayload`: a track header (swap its
> instrument), a mixer slot (add to an empty one, swap a filled one), outlined in the
> selection colour while a device that fits hovers them.
>
> **The device window's own buttons** (`kit::window_ctl`): Swap, minimise, maximise/restore
> and close at the right end of the header — every OS window's place for them, and FL's
> plug-in windows'. Minimise sends the device to a **tray** in the status bar (a chip with its
> mark in its hue and its name; a click restores it, its × closes it) and keeps it open;
> maximise fills the workspace and restore returns it docked or to its floating rect; **Esc**
> closes the device clicked last; **F8** toggles the selected track's instrument window
> (FL's F8 is its plug-in picker; Cantus's picker has its own entry points, and F8 here does
> what a musician reaches for most). Where each device floated is remembered for the session.
> Rejected: collapsing a window to its title bar (FL's) — a docked tab has no title bar to
> collapse to, and one behaviour everywhere is easier to learn than two.
