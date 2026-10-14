# Round 26 — linked: linking by name, the inspector's link row (BACKLOG §6d.2)

Branch **`r26/linked`** (worktree `daw-r26/linked`, base the contract a3b6355). Decision
record: **ADR 0037**, amendment *2026-10-13, round 26*. No schema change, no new command,
no golden hash moved.

## Done

| Part | What |
|---|---|
| `cantus_core::links` | `Kind` (notes / audio / automation), `Group` and `groups(project)` — every link group once (shared name, kind, size, first member, its track and start) then every clip of its own, in a stable order (groups first; by name ignoring case, then exactly; then place) — and `Refusal` with `would_link(project, sel, target)`, which says beforehand what `link_to` would do or why it would do nothing |
| `cantus_ui::links` | `open_link_picker(anchor)`, `PickRow` and `picker_rows(project, sel, filter)` (filter by name or track, case and accents ignored, the hint "Drums · 5.1", the refusal), `kind_key`, `refusal_key` |
| the picker | `views/link_picker.rs` (state, keys, choose) and `views/link_picker_paint.rs` (the kit panel): title, "“Beat B” becomes the clip you choose", the filter field, two bands (*Linked clips*, *Clips of their own*), two-line rows with kind and ×N, refused rows dimmed with the reason and not choosable, ↑ ↓ over the rows that link, Enter / click chooses (one undo step, `link_clips_to`), Esc / click outside closes, the kit scrollbar when the list overflows, placed at its anchor / the pointer / centred, fixed while typing |
| the action | `Action::LinkTo` (*Link to…*, clip category), **Alt+N**; keymap file **version 7** (`NEW_IN_V7`) so a saved map gets it; in the clip's right-click menu (every clip), the menu bar's Clip menu, the inspector |
| the inspector | `views/inspector_clip_link.rs`: the **Name** row edits in place (click → the whole name selected; Enter / click away renames, Esc leaves; a linked clip's well ends with the chain mark and ×N, the hint says renaming renames every copy; one `SetClipName`, one undo step for the whole group); the **Link** row: "3 linked copies" opening *Select linked copies · Make unique · Link to…* (`MenuId::ClipLink`), or *Link to…* on a clip of its own |
| strings | `links.{en,es}.txt`: `action.link-to`, `clip.link_to`, `links.status.select_first`, `links.inspector.*`, `links.kind.*`, `links.picker.*` |
| shots | `examples/shoot_links.rs` gains `picker`, `picker-filter`, `inspector`, the args `light` (Cantus Paper), `z0.75` / `z2`, `kind=…`, and redirects its settings files to a temporary folder |

Decided on the way, with the reasons in the ADR amendment: **clips of their own are in the
list** (the first link of a pattern is to a clip in no group; without them the picker is
empty until something was linked by other means); refused targets **stay listed, dimmed,
with the reason** (the brief), and the arrows skip them; **Alt+N** for the key (*N for
name*; `app_flows::every_clip_action_has_a_key_bound_to_it` requires a key; Ctrl+Alt+L
refused because Ctrl+Alt is AltGr on many European layouts); the inspector's link actions
live in a menu field rather than three buttons, because three buttons do not fit the value
column at 220 pt.

## Measurements

- A still frame of the inspector on a linked clip: **9 allocations, the same as a clip of
  its own** (egui's bookkeeping; the inspector's own test allows 40). With the "3 linked
  copies" text formatted every frame instead of cached, 14 — the test fails.
- `groups` is one pass over the clips plus a sort; it runs when the picker opens and when
  the filter or the project changes, never in a still frame.

## Tests, and what each holds

- `crates/cantus-core/tests/links_groups.rs` (7, new) — every group once with name, kind,
  count, first member, place; clips of their own after; two of one name told apart; order
  (groups first, name ignoring case, place) and its independence from storage order; a
  group of one is a clip of its own; `would_link` agrees with `link_to` for every target ×
  every selection of one or two clips; each refusal's reason; a rename through any member
  renames the entry, undo restores it. Seen failing against stubs (7/7).
- `crates/cantus-core/tests/undo_invariants.rs` (**additive**, no new arm — no new command
  shape): the *Link to* arm aims half its steps at an entry of `links::groups` (chosen from
  the project's next id, so every other arm draws the same numbers and the coverage check
  holds) and asserts `would_link` agrees with the batch. Seen failing with `would_link`
  broken.
- `crates/cantus-ui/tests/links_picker.rs` (13, new) — rows (count, kind, hint, reason);
  filter (case, accents, track); nothing selected → status, no picker; type + Enter links,
  keeps the place, one undo step back to the project; arrows skip refused rows, a refused
  row cannot be clicked (seen failing with refusals off); Esc / click outside change
  nothing; the inspector's group row (×3, "3 linked copies") and *Link to…* on a clip of
  its own; rename from the inspector renames every copy in one undo step, Esc leaves it
  (seen failing: "BeatGroove" before the field opened a frame after the click); the link
  menu's three actions; no text collisions in the clip section (220/280/420 pt) and the
  picker, en/es, 75 %/200 %, the picker on a 1280×720 screen; the clip menu's *Link to…*
  opens the picker for the clip right-clicked (seen failing before the entry); a still
  linked clip allocates no more than one of its own (seen failing uncached); Alt+N, no
  conflict, a version-6 keymap gets it, the label in both languages (seen failing before
  the binding).
- Unchanged and passing: `tests/links.rs` (14), `app_flows` (73, including the clip-key
  rule that failed until Alt+N), `cantus-ui` lib (keymap tests), `tests/inspector.rs`.

Results at the close (this worktree, shared machine):

- `cargo test -p cantus-core -p cantus-project -p cantus-ui --no-fail-fast`: **1 099
  passed, 0 failed, 21 ignored** in 100 result lines (the `audio_setup*` binaries ran).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all`: run.

## Pictures (looked at)

`F:\projects\daw-r26\shots\linked\` — `links-picker`, `links-picker-filter`,
`links-inspector`, `links-menu`, `links-arrangement`, `links-timeline-menu`, each in
English and Spanish, Cantus Night and Cantus Paper (`-paper`); `links-picker` and
`links-inspector` also at 75 % and 200 % (`-z0.75`, `-z2`). Seen: the bands, the dimmed
rows with their reasons, the ×N and kind column, the filter narrowing to one row, the
inspector's name well with the chain mark and ×4 and its "4 linked copies" field, the clip
menu's *Link to… Alt+N*; nothing overlaps; the picker fits at 200 %. Fixed after looking:
a stock egui scrollbar in the list (now the kit's, and only when the list overflows), a
list one hairline too short (it scrolled by 2 px), the picker placed from a stale window
size at 200 % (it now centres from the current one), and the light-theme shots that came
out dark (the shot now sets the app's appearance before `attach`).

## After the review, and the owner's additions (2026-10-14)

An adversarial review confirmed nine findings; the owner asked to close the "Left" items
in this round. Each fixed with a test seen failing first (ADR 0037's second round-26
section has the detail):

| # | Finding / ask | What was done |
|---|---|---|
| 1 | wrong reason in a mixed selection | `would_link`: *already* wins (then another control, then another kind) |
| 2 | rows × selection² (3.3 s to open with 2 000 selected) | `links::Selection` (the selection counted once, each entry O(1), property-tested against `would_link` and in the fuzz); `ui::links::PickList` built per revision and language, a keystroke only filters |
| 3 | every row laid out each frame (18–21 ms still) | virtualised list (`show_viewport`), rows sensed then painted with one brush; still frames allocate the same with 2 000 rows as with 12 |
| 4 | decomposed (NFD) names not found | `ui::links::fold` drops combining marks after the precomposed fold, both sides |
| 5 | unnamed clip shown as “” | *Untitled clip* in the lead line and the hint bar |
| 6 | module doc said "a key the user binds" | says Alt+N |
| 7 | doubled frame around the filter | one-well `search_field` (text over the whole well, kit margin; focus ring on the well); the inspector's name field the same |
| 8 | the lead line lost its verb | the name is cut (binary search on the width), the verb never |
| 9 | "Groove 10" before "Groove 2" | natural order (digit runs as numbers), equal names by place |
| owner | the browser of groups to drag from | the **Clips** pane (`views/clip_browser.rs`, `clip_browser_paint.rs`): FL's picker panel as tiles; drag onto the arrangement → a linked copy at the snapped drop point, one undo step (`place_linked_copy`, `cmd.place_linked_copy`); click selects the copies; Alt+F5, View ▸ *Clip browser* |
| owner | a *groups only* switch | *Linked only* beside the filter, kept for the session (not saved: a way of looking, and *all* is the right default for a first link) |
| owner | keys beside the inspector menu's rows | rows that run an action show their binding (the link menu: Shift+Alt+L, Alt+U, Alt+N); other inspector rows are choices, not actions |

Measured (release, 20 × 100 clips, all selected, the whole window's frame; before → after):
opening 1 400 → 5.7 ms, a keystroke 806 → 2.1 ms, a still frame 10.2 → 1.8 ms (closed
1.7–2.0 ms). Still-frame allocations: 4 040 → 28 (the same as with 12 rows; egui's
area and text field).

Found on the way: an egui area recolours its text while it fades in, which costs ~6
allocations per text for its first frames; the allocation tests measure after the fade.

New tests: `links_groups.rs` +3, `links_picker.rs` +6 (19 in all), `links_picker_big.rs`
(2 + 1 timing, `--ignored`, release), `links_browser.rs` (6). Shots: `links-browser`,
`links-browser-drag` in every language, theme and scale, looked at (the pane was first
too narrow and the ghost spilled over the inspector; both fixed).

At the close: `cargo test -p cantus-core -p cantus-project -p cantus-ui --no-fail-fast`
**1 116 passed, 0 failed, 22 ignored** (102 result lines); `cargo clippy --workspace
--all-targets -- -D warnings` clean; `cargo fmt --all` run.

## Files outside my area (all additive)

- `crates/cantus-ui/src/app.rs`: the fields `link_picker` and `clip_browser` with their
  `Default`s, the `run_action` arms `LinkTo` and `ToggleClips`, and
  `views::link_picker::frame(self, ctx)` after the device picker's frame.
- `crates/cantus-ui/src/keymap.rs`: `Action::LinkTo` (Alt+N, clip category) and
  `Action::ToggleClips` (Alt+F5, view category) — `ALL` 78 → 80 — `FILE_VERSION` 7 with
  `NEW_IN_V7 = [LinkTo, ToggleClips]` and its fill-in.
- `crates/cantus-ui/src/views/mod.rs`: `pub mod clip_browser;`, `pub mod link_picker;`.
- `crates/cantus-ui/src/views/workspace.rs`: `Pane::Clips` (its title, toggle, glyph, and
  where it is placed: a fifth of the arrangement's area at its left).
- `crates/cantus-ui/src/views/workspace_frame.rs`: one dispatch arm.
- `crates/cantus-ui/src/views/transport.rs`: a row in the menu bar's Clip menu (*Link
  to…*) and a checkbox in the View menu (*Clip browser*).
- `crates/cantus-ui/src/views/inspector.rs`: `Act::Link` and its arm in `apply`.
- `crates/cantus-ui/src/views/inspector_menus.rs`: `MenuId::ClipLink`, `Choice::Link`, the
  three rows in `build`, the arm in `choose`; `Entry::key` (a row's binding, shown) and a
  wider list when rows carry keys.
- `crates/cantus-ui/src/views/inspector_track.rs`: the clip section calls `name_row` and
  `link_row`, and declares `inspector_clip_link.rs`.
- `crates/cantus-ui/src/views/arrangement.rs`: one entry in the clip menu.
- New files: `views/link_picker.rs`, `views/link_picker_paint.rs`,
  `views/inspector_clip_link.rs`, `views/clip_browser.rs`, `views/clip_browser_paint.rs`,
  `tests/links_picker.rs`, `tests/links_picker_big.rs`, `tests/links_browser.rs`,
  `cantus-core/tests/links_groups.rs`.

## Paint with the chosen tile, and waveforms (2026-10-14, the coordinator's last two)

| Item | What was done |
|---|---|
| paint with the chosen tile (FL's B) | a click on a tile chooses it as the paint source (ringed; again lets it go; double-click now selects its copies); **B** with the arrangement focused (the existing paint key, so no new binding) or the new toolbar key brings out the arrangement's paint tool — an overlay above the timeline (`views/clip_browser_paint_tool.rs`): click places a linked copy at the snapped time, drag paints copies end to end along the row (one clip length apart, skipping places a copy already starts), right-drag or Alt-drag erases **only the source's copies** (FL erases any clip; decided and recorded in ADR 0037), each stroke one undo step (`cmd.paint_linked_copies`, `cmd.erase_linked_copies`), ghosts and crossed-out copies while it lasts, Esc puts it away, the hint bar names the source |
| waveforms on audio tiles | from `PeakStore` (no decoding on the UI thread), the clip's window, reversed for a reversed clip; columns kept per clip with revision and width; a still frame with a waveform allocates as much as one without (16) |

Tests: `links_browser.rs` +4 (click places one; a stroke places five end to end, painting
again adds nothing; right-drag erases only the source's copies; hint, Esc, double-click),
`links_browser_waves.rs` (2). Seen failing: the paint tests before the tool and two at run
time (hint cut short, a ghost off the lanes); the wave tests with the cache off and the
reverse dropped. Shots: `links-browser-paint` (en/es, Night/Paper, 75 %/200 %) and the vocal
take's waveform on its tile and clips — looked at.

At the close: `cargo test -p cantus-core -p cantus-project -p cantus-ui --no-fail-fast`
**1 122 passed, 0 failed, 22 ignored** (103 result lines); clippy `-D warnings` clean (after
three findings in the new code: a type alias, two `+=`); `cargo fmt --all` run.

Files outside my area added by this step: `app.rs` (a guard arm: `ToolPaint` with the
arrangement focused toggles its paint tool), `views/arrangement.rs` (the toolbar's paint key
and the `paint_tool` call after the corner).

## The second review of the paint tool and the browser (2026-10-14)

Eleven findings, all fixed on `r26/linked` after merging `r26/integration` into it (ADR
0037's last section has each):

| # | Finding | Fix |
|---|---|---|
| paint 1 | strokes from another grid point (or a click inside a copy) stacked copies | `ui::links::paintable`: a start whose span overlaps a copy of the source on the track, or another of the stroke, is skipped; the ghosts show only what will be placed |
| paint 2 | gestures did not focus the arrangement | a stroke, a tile click or double-click call `focus_arrangement` |
| paint 3 | Esc anywhere put the tool away | Esc reaches it through `ClearSelection` (after the mixer, device-window and picker claims), only with the arrangement focused |
| paint 4 | automation ghosts on the pointer's row | ghosts, strokes and erase on the automated track, in the tool and the browser drag |
| paint 5 | leftward strokes came up one copy short | the copy whose span holds the pointer is placed |
| visual 1 | the browser's scrollbar was dead | its drag sets the scroll offset |
| visual 2 | a filter could leave Enter's row out of view | a change of the rows shown brings the highlight into view |
| visual 3 | walking up to the first row hid its band | the band comes back with the row |
| visual 4 | browser-queued peaks never asked for a frame | the context goes to the store |
| visual 5 | a letter after an arrow was lost | the field locks the arrows while focused |
| cross 1 (high) | `font_coverage` red on the merged tree: a combining-mark char literal | the range written as numbers; the test passes, its rule unchanged |

Tests: `links_browser.rs` +6, `links_picker_big.rs` +2, each seen failing first (the
automation one also with the row fix switched off); `font_coverage` seen failing on the
merged tree, then green.

At the close (after merging `r26/integration`): `cargo test -p cantus-core -p cantus-project
-p cantus-ui --no-fail-fast` **1 173 passed, 0 failed, 23 ignored** (109 result lines);
clippy `-D warnings` clean; `cargo fmt --all` run. The paint shot re-rendered and looked at.

## Left

- Nothing of §6d.2.
- The owner, by hand: Alt+N on a clip, type part of a name, Enter; rename a linked clip in
  the inspector and watch every copy; Alt+F5, click a tile, B, paint along a track, then
  right-drag over it.

## Notes for the integrator

- **Keymap version 7** is taken here (Alt+N, Alt+F5); if another branch also bumps it,
  renumber one (the `a_version_N_file_gets…` tests and `NEW_IN_V*` follow).
- `inspector_menus.rs`, `inspector.rs`, `workspace.rs`, `workspace_frame.rs` are touched
  additively; the mixer builder's *New return ▸* work may touch `inspector_menus.rs` too.
- The undo fuzz change is assertions inside the existing *Link to* arm; it draws no new
  numbers from the generator.
- The timing test needs release: `cargo test --release -p cantus-ui --test
  links_picker_big -- --ignored --nocapture`.
