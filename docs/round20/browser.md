# Round 20 — browser: every device on one screen

**Builder:** browser · **Branch:** `r20/browser` · **ADR:** 0029 (§1–§8; device-ux adds §9)

The owner: *"some DAWs let you see every available instrument on screen to choose — it looks
very, very pro — I want that."* This branch is that screen: the device picker, a gallery of every
instrument and effect Cantus can use — its own and the hosted CLAP/VST3 plug-ins — with a
picture of each, and a browsing panel in the workspace.

## Done

- **The catalogue** (`src/picker_catalog.rs`): Firmus, Pulsus II and the seven native instruments,
  all twelve native effects, and every scanned plug-in a track or a slot can hold (instruments;
  effects with audio out; no note effects; one entry for a plug-in installed twice), each with a
  stable key (`cantus:unda`, `clap:<id>`, `vst3:<class id>`), what it is, vendor, format, shelves
  (primary first; hosted ones filed from CLAP features / VST3 subcategories and the name) and its
  search words — shelf names in English and Spanish, accents folded, and its factory presets'
  names.
- **Query** (`src/picker_query.rs`): filter by role (from the `PickPurpose`), shelf (All,
  Favourites, Recent, a category, a source) and words (ranked: name prefix > word prefix > inside
  the name > anywhere else; every word must match); sort Curated / Name / Vendor / Recently used;
  grouped under shelf headings in the curated order.
- **Preferences** (`src/picker_prefs.rs`): favourites, the last 16 chosen, order, tile size,
  details shown — `devices.json` in the user's data folder, never in the project.
- **The picker** (`views/picker.rs`, `picker_layout.rs`, `picker_side.rs`, `picker_grid.rs`,
  `picker_details.rs`, `picker_keys.rs`): a large panel over the dimmed window — header (what and
  for what, search, order, tile size S/M/L, details toggle, close), shelves with counts, the
  gallery (16:10 pictures, names, what it is, stars, a *CURRENT* chip, hover card, selection ring,
  headings with glyph and count, culled to the viewport, always-visible scrollbar, empty states),
  the details pane (picture, name, star, vendor / format / shelves rows, plug-in version, audio,
  editor and file, a description of each of ours in EN and ES, its factory presets), the footer
  (count, keys, Cancel, Add / Insert / Replace).
- **Keyboard**: type to search without focusing anything; arrows by screen position (across
  groups), Page Up/Down, Home/End; Enter chooses; Ctrl+D stars; Esc clears then closes. The app's
  shortcuts are off while it is open. Double-click chooses.
- **Opening** for every `PickPurpose` (`CantusApp::open_picker` → `views::picker::open`): role
  filter, empty search, the device being replaced (or the last one chosen) selected and scrolled
  into view. **Choosing** records it as recent and calls `CantusApp::apply_pick` (device-ux's).
- **Thumbnails** (`views/picker_snap.rs`, `picker_thumbs.rs`): each native device's *own editor*
  drawn off screen (dummy-engine scratch app, the screenshot set-up), CPU-filled on a worker
  through `raster`, cropped 16:10 from the top-left, box-filtered to 416 × 260, corners rounded;
  one texture atlas (one mesh for every picture on screen) and PNGs in `cache/thumbs/` keyed by
  device, look (all theme tokens), language and recipe version; at most one capture a frame.
- **Drawn covers** (`views/picker_cover.rs`) for hosted plug-ins, Firmus, Delay, Reverb and as
  the placeholder: hue ground (vendor-hashed for plug-ins), seeded arc pattern, a category glyph
  (14 drawn glyphs), vendor, large name, CLAP/VST3 badge.
- **Devices panel** (`views/picker_panel.rs`, `Pane::Devices`, *View ▸ Devices*): Instruments |
  Effects, search, rows with small pictures grouped by shelf; double-click adds (new track /
  selected strip); rows **drag a `picker::DevicePayload`**; ⤢ opens the full picker. *View ▸
  Device picker…* opens it for a new instrument.
- **ADR 0029** §1–§8, with §9 left for device-ux.
- **Design review**: `cargo run -p cantus-ui --example shoot_picker -- <dir> <view> <theme> [es]`
  (`instruments`, `effects`, `search`, `replace`, `panel`; `dark` / `light`; `thumbs`; `nested`).

## Tests

`crates/cantus-ui/tests/picker.rs` (14): the catalogue lists every native kind and every offered
plug-in once (note effects out, duplicates once, keys unique); search by name, vendor, category in
both languages, accents, presets, AND of words; shelves group and count, recents in order of use;
grid layout and arrow geometry across groups; opening per purpose filters instruments/effects;
replacing selects the current device; typing → Esc → → → Enter applies through `apply_pick`
(effect added, last choice recorded, recent first); clicking a tile, its star, then the choose
button adds a track with Organum, Esc closes with no project change; favourites/recents persist to
`devices.json` and are not in the project; clicking a shelf filters; 75 devices draw only the
visible tiles in ≤ 24 meshes; thumbnails made once, then read back from disk with no capture; the
panel drags a `DevicePayload` and double-click adds.

## Files touched outside my area

- `src/app.rs`: the `picker_view` field and its init; in `assemble`, loading `devices.json` and
  turning thumbnails on (cache folder); `views::picker::frame` in the frame; `global_shortcuts`
  returns while the picker is open.
- `src/picker.rs` (contract): `open_picker`'s body; **new type `DevicePayload`** (drag payload,
  with `is_instrument`). Signatures unchanged; `apply_pick` untouched.
- `src/views/workspace.rs` (`Pane::Devices`: title, toggle key, glyph, placement at the left),
  `src/views/workspace_frame.rs` (one dispatch arm), `src/views/transport.rs` (two View-menu
  items), `src/views/mod.rs`, `src/lib.rs` (module lines).
- `locales/areas/picker.{en,es}.txt` (my area).

## Left

- **Preview** an instrument by ear (a phrase rendered offline through the device) — not built.
- **Drop targets**: device-ux wires them (track header, empty lanes, mixer slots) reading
  `DevicePayload`; until then a drag lands nowhere.
- Hosted *instruments* are listed; choosing one does what `apply_pick` does with it (the contract
  version ignores it; hosted-inst + device-ux make it work).
- Presets inside the picker (→ to browse a device's presets, Bitwig), user collections, pictures
  of hosted plug-ins from their own windows.
- The overlay's very first appearance in a session is egui's sizing pass: one invisible frame.

## Notes for others

- **device-ux**: open with `app.open_picker(purpose)`; the picker calls `apply_pick(purpose,
  choice)` then sets `app.picker = None` (yours closes too — harmless). Drop targets:
  `resp.dnd_release_payload::<crate::picker::DevicePayload>()`. ADR 0029 §9 is yours.
- **hosted-inst**: hosted instruments arrive as `PickChoice::Hosted(info)` with
  `info.kind == PluginKind::Instrument`.
- **Integrator**: `views::picker::frame` must stay after the workspace in `CantusApp::frame`
  (it draws on top). `tests/picker.rs` is self-contained.
