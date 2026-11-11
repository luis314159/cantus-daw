# ADR 0113 — The arrangement's structure: sections, folders, a mode per lane, a track's own icon

**Status:** Accepted (round 39, builder *el-arrange*) · **Date:** 2026-11-09 · **Round:** 39 ·
**Schema:** 61 · **Extends:** ADR 0004 (commands), ADR 0027 (automation lanes), ADR 0060
(markers, tempo), ADR 0068 (automation modes), ADR 0111 (the design system) ·
**Design:** `docs/round39/design/des-arrange/` (approved by the owner 2026-11-09)

## Context

The owner approved des-arrange's build sheets for the arrangement in Slate. They draw four
things the model did not hold:

1. **Sections** — a lane of named spans above the tracks, each with its length ("Drop ·
   8 bars"), beside the markers. Markers (ADR 0060) are points; a section is a span, and
   the sheets draw both at once (the "Vox in" marker inside the "Drop" section), so a
   section cannot be derived from two markers.
2. **Folders** — a row of its own above its tracks, open or closed (a closed folder is one
   44-px row with its summary), M S acting on its tracks. Nothing grouped tracks: strip
   groups (ADR 0041) organise the mixer's desk, not the arrangement's rows.
3. **A mode per automation lane** — the sheets draw two lanes of one track in Read and
   Touch, a third in Write. Modes were per track (ADR 0068).
4. **A track's own icon** (the owner, 2026-11-09): an icon of the user's choice before
   the track's name, to find it at a glance, beside the type icon the instrument gives.
   `Track::icon` (a `TrackIcon`) is already that type icon's override.

Each changes what a project file says, so each is stored.

## Decision

### 1. The model (schema 61; `cantus-core/src/arrange.rs`)

- `Project::sections: Vec<Section>` — `{ id, start, length, name, color }`. Sections never
  overlap (an insert or a set that would is refused); a section of no length is refused.
- `Project::folders: Vec<TrackFolder>` — `{ id, name, color, open }` — and
  `Track::folder: Option<FolderId>`. A folder is **a row of the view, not a mix bus**: it
  renders nothing (a folder's M and S are a batch of its members' own mutes and solos, one
  undo step; where its tracks go is their strips'), so the engine and the null tests are
  untouched. A folder holding tracks cannot be removed (*Ungroup* empties it first, one
  batch). One level of folders.
- `Track::lane_modes: Vec<LaneMode { target, mode }>` — a lane's own mode over the
  track's; `arrange::lane_mode(track, target)` resolves it. The write pass asks it
  (`own_auto_mode`), a track "wrote" when it or any of its lanes is in Write, and after a
  pass a lane in Write turns to Touch as its track does (ADR 0068's safety).
- `Track::glyph: Option<String>` — the id of an icon of the track icon library (lower-case
  letters, digits and hyphens, at most 48; anything else is refused). An id this build does
  not know is kept and drawn as none, so a file from a later build keeps its icons.

Every field defaults to none and is not written when empty: an older project means what it
meant. The migration 60 → 61 writes nothing; `fixtures/v61.cantus` holds two sections, a
closed folder of two tracks, a lane in Touch and an icon. The freeze signature ignores
`folder`, `lane_modes` and `glyph` (nothing they say is heard).

### 2. The commands (`Command::Arrange(ArrangeCommand)`)

`InsertSection`, `RemoveSection`, `SetSection`; `InsertFolder`, `RemoveFolder`, `SetFolder`;
`SetTrackFolder`; `SetLaneMode` (`None`: follow the track); `SetTrackGlyph`. Each returns its
exact inverse; `SetSection`, `SetFolder` and `SetTrackGlyph` merge in a gesture (a section
dragged is one step). The undo fuzz has an arm of its own
(`the_arrangements_structure_undoes_and_redoes_among_the_others`: every kind applied, the
refused ones refused). The agent's `set_track` tool sets the icon and `track_summary`
reads it.

### 3. How the view lays them out

One function lays the rows out for the headers and the lanes alike
(`arrangement_folders::rows_into`): each track in the song's order; a folder's row where its
first member comes, every other member gathered under it; a closed folder's members at no
height; a folder with no member at the end. `TimelineGeometry::rows` stays indexed by
track, so every painter and test that asks "where is track i" keeps working; a hidden row has
height 0 and draws nothing. A header dropped between two members of a folder joins it;
anywhere else it leaves any (one batch with the move).

### 4. The library and the picker (the integration, round 39)

The library is the icon family's set 6 (`icons`, the owner's request and review of
2026-11-09: `docs/round39/design/icons/set6-additions.md`): **316 icons in the owner's 17
families**, guitars and basses with their electric and acoustic shapes as sub-rows. Each is
drawn for the ten device sizes it is shown at (12, 14, 15, 16, 18, 20, 21, 24, 30, 36 — the
arrangement's 14/16, the mixer's 12/14, the picker's 24, each at 125 % and 150 %), never one
drawing scaled (ADR 0108), and rasterised once per (icon, size, inks) into the layer atlas.
`tools/icons/track_glyphs.js` runs the design's own scripts (in `picker.html`'s order) and
writes `cantus-ui/assets/icons/tracks.txt`: the families in order, each icon's English and
Spanish names and search words, and its drawings. The sheets' camelCase ids are stored in
kebab case (`treble-clef`), which is what §1 accepts. The ids el-arrange's first build used
before the library landed map to their icons (`drums` → `drumkit`…).

- **Search** reads both languages' names, the id and the aliases (`sol`, `treble`, `G clef`
  find the treble clef), case and accents aside; every word must match; an exact name comes
  first, then names that begin with it, then an exact alias. Enter takes the first found.
- **Recent**: a row at the top holds the icons chosen last on this machine (8, newest
  first; `Settings::track_icons_recent`, a preference, never project data).
- **The picker** follows the icons sheet (`set6-tracks-picker-*`): 300 points wide, the
  title and the track's name, the search (it has the keyboard when the picker opens),
  *None* with what it means, the recent row, then the families scrolled in a 520-point view
  (less on a short window; the wheel and a scrollbar), each family titled; the foot names
  the icon under the pointer. 32-point cells, 8 a row, the icon at 24 (30 at 125 %).
- **Inks**: in a header the icon takes the track's colour (full when the track is
  selected, toward ink 2 at rest, ink 4 when muted), its second ink half-way to the
  header's ground; in the picker ink 2 at rest, ink 1 under the pointer, the accent chosen.

## Consequences

- The arrangement can be built to the approved sheets: the sections lane, folders and their
  summaries, lanes in their own modes, the icon before a name.
- A **section drag moves the section**, along the bars; the sheets also say it "moves its
  bars on every track" (with Ctrl copying and Shift inserting). That is ripple editing of
  the whole song — clips split at the edges, the rest shifted, automation and tempo with
  them — a feature of its own, left for a round of its own (BACKLOG).
- Folders do not nest yet; the layout's depth is there for when they do.
- The mixer's strip head and the clip browser can show a track's icon through
  `views::track_glyphs::track_icon`.

## What the tests hold

- `cantus-core/src/arrange_tests.rs` — each edit and its inverse; overlaps and empty spans
  refused; a folder that holds tracks not removed; a lane's mode over its track's; a bad
  icon id refused; an older project reads with none of it.
- `cantus-core/tests/undo_invariants.rs` — the arm above.
- `cantus-project` — `fixtures/v61.cantus` and every older fixture load.
- `cantus-ui/src/views/track_glyphs.rs` — the library whole (316 icons, 17 families, each
  named in both languages, each id one the project keeps), every icon drawn at every size
  and inking its canvas, search by English, Spanish and aliases with accents aside, old ids
  mapped, the size a box takes.
- `cantus-ui/tests/arrange_e2e.rs` — through `CantusApp::assemble`, the whole window drawn:
  a folder made, closed by its chevron (members hidden), undone; a section added, dragged
  two bars along, one undo step; a lane's own mode honoured by the write pass; an icon
  picked in the picker (found by typing "estrella" into its search) and shown before the
  name, the recent row leading with it when the picker opens again, undone; each saved and
  loaded back.
