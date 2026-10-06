# ADR 0029 — The device picker, and swapping one device for another

**Status:** Accepted · **Date:** 2026-10-05 · **Round:** 20 · **Builders:** browser (this text,
§1–§8), device-ux (its section, §9, added on its branch)

## Context

On 2026-10-05 the owner asked, among other things, for two that meet here:

> Some DAWs let you see every available instrument on screen to choose — it looks very, very
> pro — I want that.

> Swapping one instrument for another — and one mixer effect for another — easy and intuitive.

Until now an instrument was chosen from a menu of words (the inspector's instrument row, the
track header's chip) and an effect from a menu in the mixer slot; a hosted plug-in from a list
in Settings. Nothing showed what a device *looks like* before it was on a track, and nothing
put ours and the hosted plug-ins in one place.

The references (`research/ui/`): **FL Studio's plug-in picker** (F8 — a grid of plug-in
thumbnails, which are screenshots of the plug-ins' own windows) and its browser's *Plugin
database*; **Native Instruments' Komplete Kontrol / NKS browser** (a grid of artwork tiles,
categories and vendors at the left, a details column); **Bitwig's pop-up browser** (four
columns: sources, filters, results, file info with an UPPERCASE key / value grid; keyboard
first — type to search, ↓ into the results, numbers file into collections; `bitwig-studio.md`
§8); **Ableton's browser** (a sidebar of categories; double-click or Enter loads onto the
selected track; `ableton-live.md` §3.2); Logic's library.

The contract commit fixed the types every builder codes against (`crates/cantus-ui/src/
picker.rs`): `PickPurpose` (AddInstrument, ReplaceInstrument, AddEffect, ReplaceEffect),
`PickChoice` (Firmus, Pulsus, Instrument(kind), Effect(kind), Hosted(PluginInfo)),
`CantusApp::open_picker` (the browser builder's) and `CantusApp::apply_pick` (device-ux's).

## Decisions

### 1. One catalogue of every device, ours and hosted

`picker_catalog::build` makes one list of [`Device`]s: ours in a **designed order** (Unda,
Calor, Modulus, Firmus, Lamina, Organum, Gravis, Pulsus II, Exemplar; Aequalis, Comprimo,
Porta, Sibilus, Multiplex, Terminus, Culmen, Spatium, Delay, Reverb, Fervor, Latitudo — a
scaffold kind stays out until `ready()`), then every plug-in of the scan database that an
insert slot or a track can hold (`PluginKind::Instrument`, or `Effect` with audio out; note
effects are left out; a plug-in found in two folders is offered once).

- **A stable key** names each device — `cantus:<slug>`, `clap:<plug-in id>`, `vst3:<class
  id>` — never its display name, so favourites and recents survive a rename, a translation or
  a rescan.
- **Shelves.** A device is filed on one or more categories, the first its primary: instruments
  — Synths, Keys, Bass, Drums, Samplers, Other; effects — EQ, Dynamics, Space, Distortion,
  Modulation, Stereo, Mastering, Utility. Ours are filed by hand (Terminus is Dynamics *and*
  Mastering); a plug-in by the words its format gives it (CLAP features, VST3 subcategories)
  and its name, with Other / Utility when nothing matches.
- **Words.** Each device carries its own words for the search: name, what it is ("Wavetable
  synth"), vendor, shelves in **both languages** (so *batería* and *drums* both find Pulsus
  II), format, the plug-in's features, and the names of its **factory presets** (so typing
  *Rhodes*-like preset names finds Lamina). Text is folded to lower case without accents.
- Pure data, no egui: tested without a window.

### 2. The picker: a large panel over the window, gallery first

`views/picker*.rs`. It opens over a dimmed workspace (a click on the dimmed part closes it),
large (up to 1360 × 860 pt, the window less 24 pt a side):

- **Header**: what is being chosen, large, and for what, dimmed ("Replace the instrument · on
  Keys"); the **search** (a well with a magnifier, "Search 21 devices…"); the **order**
  (Curated · Name · Vendor · Recently used); the **tile size** (S · M · L); the details key; ×.
- **Shelves** at the left with their counts: *Library* (All, Favourites, Recent), *Categories*
  of what is being chosen (empty ones left out), *Sources* (Cantus, CLAP, VST3).
- **The gallery**: tiles in rows, grouped under each shelf's heading (glyph, name, count, a
  hairline) when the order is Curated and nothing is typed, one flat run otherwise. A tile is a
  **16:10 picture** of the device with its name and what it is beneath (vendor too for a
  plug-in); the star at the picture's corner (always when set, on hover otherwise); a
  *CURRENT* chip on the device being replaced; hover raises a card behind it, the selection
  rings it in the selection colour. Tiles stretch to fill the row.
- **Details** at the right: the picture large, the name, what it is, a star; label | value rows
  (vendor, format, shelves; for a plug-in its version, audio in/out and sidechain, its editor,
  its file); a **description** written for each of ours (EN and ES) from its documentation;
  its **factory presets**, as many as fit, "+N more". It folds away for a wider gallery.
- **Footer**: the count, the keys, *Cancel* and the primary button, whose word says what will
  happen: *Add* (a new track), *Insert* (an effect), *Replace*.
- Ranking: while typing, a name that starts with the word beats one that contains it, beats a
  match anywhere else; every word must match; the chosen order breaks ties.

### 3. Thumbnails: the device's own editor, drawn once and kept

A native device with an editor is pictured by **its real editor**, not by an illustration:
the same painters draw the thumbnail and the device, so the picture never drifts from it
(FL's picker does this with screenshots). `picker_snap` draws the editor in a private egui
context around a scratch application (the dummy engine, one track set up as the device's
screenshot is — `demo_setup`, and `demo_feed` where it is cheap), tessellates the frame, and a
**worker** fills it on the CPU (`raster`), crops it to 16:10 keeping the top-left (the header
with the name; the help line and the right edge go first), shrinks it with a box filter to
416 × 260 px and rounds its corners. The UI-thread part costs a few frames of one editor
(25–200 ms at `opt-level 1`, most under 100 after the first); the worker part 40–350 ms.

- **Cache**: one 1664 × 1820 texture **atlas** (28 slots; ours need 18) so the whole gallery
  draws every picture as **one mesh**; and PNG files under the app's cache folder
  (`cache/thumbs/`), so a later start reads them on a worker and makes nothing. A picture's key
  is the device, the look (every theme token hashed, plus roundness, depth, density and text
  size), the language (an editor has words in it) and a recipe version (`REV`). A change of
  theme asks for other pictures; nothing goes stale.
- At most **one capture a frame**, only while the picker or the panel shows; until a picture
  is in, its tile shows the **drawn cover** (§3a), so the picker is complete from its first
  frame. The first opening on a new look fills in over a second or two; after that, never.
- Effects whose demo feed runs real DSP over seconds of audio (Terminus, Culmen, Spatium,
  Sibilus) are pictured idle: a capture must never stall the UI.

**3a. Drawn covers** — for a hosted plug-in, for Firmus and the classic Delay and Reverb (no
editor of their own), and as the placeholder: a ground in the device's hue (a function hue for
ours; for a plug-in one of the six, turned a little, chosen from its **vendor's name**, so a
vendor's plug-ins share a colour), a quiet pattern of arcs seeded by its key, the category's
glyph large (a wave, keys, a drum, a bell curve, a knee, echoes, a clipped sine…), the vendor in
small capitals, the name set large, and the format badge (CLAP / VST3).

Rejected: **artwork per device as image files** (Komplete's way) — another asset per device
and per theme to keep in step by hand; **vector miniatures drawn per device** — eighteen more
painters to keep in step with eighteen editors; **rendering every editor at start-up** — seconds
of start-up for a window most sessions open later.

### 4. Keyboard first

Typing goes into the search **whatever has focus** (no click needed); the arrows move between
tiles **by where they are on screen** (↓ from the end of one group lands on the nearest column
of the next; `picker_keys::step` is a pure function over the published tile rects); Page Up /
Down by a screenful; Home / End; **Enter** chooses; **Ctrl+D** stars (the browser's bookmark
key — a letter would type); **Esc** clears the search, then closes. Once the search field is
clicked into, ← → edit its text until another key moves the selection. Double-click a tile to
choose it. While the picker is open the application's own shortcuts are off
(`global_shortcuts`), so typing *m* never mutes a strip behind it.

### 5. Favourites, recents and preferences are the user's

`picker_prefs::Prefs` — favourites, the last 16 devices chosen (most recent first), the order,
the tile size, whether the details show — kept in `devices.json` in the user's data folder,
**never in a project**: a favourite is the musician's, not the song's. Written through a
temporary file on every change. The picker opens on the device being replaced (a *Replace*)
or the last one chosen of that kind.

### 6. The devices panel and drag payloads

*View ▸ Devices* docks a browsing panel at the left of the workspace (`Pane::Devices`, FL's
browser place): Instruments | Effects, a search, rows with a small picture grouped by shelf.
Double-click adds (an instrument on a new track, an effect at the end of the selected strip);
⤢ opens the full picker. A row **drags** a `picker::DevicePayload(PickChoice)` (egui's
drag-and-drop payload), with the name following the pointer; drop targets read it with
`dnd_release_payload::<DevicePayload>()` and call `apply_pick` with the purpose their place
means. *View ▸ Device picker…* opens the picker to add an instrument.

### 7. Choosing goes through `apply_pick`

The picker never changes the project itself: choosing records the device as recent and calls
`CantusApp::apply_pick(purpose, choice)` — one undo step, what keeps and what swaps decided
there (§9). Closing without choosing changes nothing (tested: the bus revision is unchanged).

### 8. Cost

The gallery is laid out by a pure function, and only tiles inside the viewport are sensed or
drawn; each frame is three passes — ground, headings, faces and covers through one brush; every
picture as one mesh on the atlas; names, stars and rings through a second brush — so 75 devices
draw in a fixed handful of meshes (`tests/picker.rs`). The catalogue is rebuilt only when the
language or the plug-in database changes.

## Consequences

- One place to choose any device, ours or hosted, that shows what each looks like.
- `CantusApp` gains `picker_view` (the catalogue, the search, prefs, the thumbnails' cache);
  the workspace gains `Pane::Devices`; the contract file gains `DevicePayload`.
- New user files: `devices.json`, and `cache/thumbs/*.png` (about 30 KB each, a set per look
  and language; safe to delete).
- Left open: an audible **preview** of an instrument (a phrase rendered offline through the
  device) — the engine can render offline, the picker has the place for a play key; preset
  browsing inside the picker (→ shows a device's presets, Bitwig); user collections beyond
  favourites; pictures for hosted plug-ins taken from their own editor windows (needs a
  plug-in instantiated and its window captured — not for a picker to do on its own).

## 9. Swapping (device-ux)

**What a swap keeps.** `apply_pick` (`crate::swap`) does every purpose as **one undo step**.
An instrument swap is one `Batch` (`SetInstrument`, and `SetTrackName` when the name was a
default one: empty, "Track *n*" in the interface's language, or the old instrument's own
name); the track, its clips, notes, strip, inserts and colour are untouched, because an
instrument is the only thing that changes — a note is a pitch for a kit and a synth alike,
so a kit → melodic swap keeps every note. Between two native instruments the shared model
(ADR 0023) lets the **matching settings** come along — the eight macros and their names,
every number whose key the new kind also declares (clamped to its range), every
non-file text it also takes, and the modulation connections whose source the new kind
produces and whose target it has, compacted from slot 1. It is on by default and the
application holds the choice (`CantusApp::swap_keep.matching`) for the picker to offer.
The A/B side and the preset name of the old instrument are forgotten; the window that
showed it shows the new one, where it was.

**`Command::ReplaceEffect { strip, effect, slot }`** — rather than a remove and an insert —
puts a whole new slot at the old one's index; its inverse is the same command with the old
slot. It keeps the slot's **position**, **mix** and **bypass**, and its **key** and **note
source** where the new kind takes them (a key into a kind with no detector, a note source
into a kind that hears none, a key that would close a loop, an id already in the chain are
refused, as their own commands refuse them; `apply_pick` drops a route the new kind cannot
take, and retries without routes if the routing moved). The new slot has a **new id**: it is
another device, and nothing that pointed at the old one (an open editor, an automation
target, the engine's instance) silently changes kind under it. Parameters do not come along
between effect kinds: they share no model.

**Adding** names an instrument's track after it ("Organum", then "Organum 2"), inserts it
where asked, selects it and opens its window; an effect goes at the end of the chain with its
editor open (FL opens a plug-in it loads). A hosted instrument (ADR 0028) goes through `CantusApp::set_hosted_instrument`, which loads
the plug-in first; one that will not load changes nothing and says why.

**The entry points** (every one calls `open_picker` with the purpose its place means): the
**Swap** cap in every device window's title bar; the track header's **instrument chip**
(the icon, raised with a ▾ under the pointer); the arrangement's **+** and its empty-timeline
menu; the channel rack's channel button (right-click, or the swap badge shown on hover; its
left click now opens and closes the instrument, as FL's does); the inspector's instrument
field, its empty insert tiles and its tiles' menu; the mixer's empty slots, the swap badge
on a filled slot and its menu. A right-click keeps the old quick list where there was one,
headed by *All effects…*. **Drop targets** read `DevicePayload`: a track header (swap its
instrument), a mixer slot (add to an empty one, swap a filled one), outlined in the
selection colour while a device that fits hovers them.

**The device window's own buttons** (`kit::window_ctl`): Swap, minimise, maximise/restore
and close at the right end of the header — every OS window's place for them, and FL's
plug-in windows'. Minimise sends the device to a **tray** in the status bar (a chip with its
mark in its hue and its name; a click restores it, its × closes it) and keeps it open;
maximise fills the workspace and restore returns it docked or to its floating rect; **Esc**
closes the device clicked last; **F8** toggles the selected track's instrument window
(FL's F8 is its plug-in picker; Cantus's picker has its own entry points, and F8 here does
what a musician reaches for most). Where each device floated is remembered for the session.
Rejected: collapsing a window to its title bar (FL's) — a docked tab has no title bar to
collapse to, and one behaviour everywhere is easier to learn than two.

**Automation follows a swapped effect (integration, ADR 0027):** the swap's batch also points
every automation clip on the old effect at the new one for each parameter key the new kind
also has, so a lane survives a swap between kinds that share a parameter (Terminus's and
Culmen's `ceiling`) and one undo brings both back; a lane on a key the new kind lacks keeps its
clip and goes quiet (`tests/swap_automation.rs`).
