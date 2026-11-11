# Round 39 — builder `el-cables`: the routing cables in three looks

Branch `r39/el-cables` (worktree `F:\projects\daw-r39\el-cables`), based on `r39/el-mixer`
(the Slate mixer). The approved design and its spec: des-mixer's
`docs/round39/design/des-mixer/options-cables-{night,paper}.png` and the README's "The
routing cables, designed". The owner's choice of 2026-11-09: **B (signal flow) by
default, all three selectable.**

## What changed

### A setting: *Cable style: Patch · Flow · Subway · Jack*, the theme's by default

- **The default is the theme's** (the owner, 2026-11-09: "make the default style a
  per-theme property"): `kit::system::System::cables` (`CableStyle`), **Flow** in every
  family today (Slate, Graphite, Studio ink, Classic); a theme file names its own
  (`"system": {"cables": "Jack"}` — the vintage theme will), and one written before it
  reads Flow. The enum lives in `kit/system.rs` so a theme can name it;
  `settings::CableStyle` re-exports it.
- **The user may choose another**: `Settings::cable_style: Option<CableStyle>` (`None` =
  the theme's), saved in `settings.json` (this machine's preference, never project data);
  `Settings::cable_style_in(&system)` is the one rule every painter asks.
- Chosen in the Cables key's options menu (a *Cable style* band at its top: Patch, Flow,
  Subway, Jack and *The theme's*, radio rows; the menu stays open, as for its other
  switches) and in **Settings ▸ Appearance** (a segmented row *The theme's · Patch · Flow
  · Subway · Jack* with a one-line note). Strings in both locales
  (`locales/areas/cables.{en,es}.txt`: Patch/Latiguillo, Flow/Flujo, Subway/Metro,
  Jack/Jack, The theme's/La del tema).

### The geometry (`views/mixer_wires.rs`, new; pure, no egui UI, no allocation)

- **Jacks**: two per strip in the foot row (18 pt), 9 above its bottom — the input at the
  centre −11, the output at +11 (Compact's 30-pt strip brings them in to ±9.5). A hole of
  radius 4 (`edge`; Paper `mix(well, edge, .6)`) with a 1.25-px ring, `mix(ink4, ink3, .5)`
  at rest, its strongest cable's colour when one is plugged in (output, then send, then
  key, among cables drawn at 45 % or more); the output carries a 1.5-px dot.
- **Patch / Flow**: one cubic per cable, P0 the source's output jack, P1 straight below,
  P2 straight below the target, P3 the target's input jack; sag
  k = min(58, 16 + 0.15·|dx|) (the deepest point 0.75·k below the jacks), flattened to 24
  segments.
- **Subway**: down, across in its lane, up, corners r 4; lanes below the jacks — outputs
  +14, sends +26, keys +40 — and sub-lanes 4 px apart where same-kind spans overlap
  (6 px margin), shortest span first (greedy interval colouring). Where a kind needs more
  sub-lanes than its band holds, the next kind's band moves down (8 px after the last
  sub-lane above), so **no two cables whose spans overlap share a depth**, and the channel
  grows to hold the deepest lane (to 160 pt; the layout's band row takes it the next
  frame).
- **Strokes**: an under-stroke 2 px wider in `mix(col, edge, .55)` (Paper
  `mix(col, ink1, .28)`), round caps. Widths: output 2; send 1 + 1.6·clamp((dB+36)/36);
  key 1.5 dashed 4/3; refused 2; a hovered route +0.5. Tokens (`wires::tokens`):
  `cable.out = accent`, `cable.send = mix(accent, ink1, .45)` (Paper
  `mix(accent, base, .38)`), `cable.key = mix(solo, rec, .25)` (Paper
  `mix(solo, #ff9a2e, .40)`), `cable.refused = rec`, `cable.channel = mix(base, gutter, .40)`
  (gutter = Slate's `ink` token).
- **Flow's gleam**: a 7-px dash every 64 px in `mix(col, white, .6)` (Paper `.7`),
  opacity 0.35 + 0.65·level, speed 40 + 80·level px/s; its offset is
  `transport seconds × speed mod 64` — from the transport's position, never accumulated.
  Level: the source strip's meter as the strip shows it (`Meters::level_db`), plus a
  send's level, mapped −48…0 dB → 0…1.

### D, "Jack" (des-mixer's `options-cables-jack-*`, README "Cable style D")

- **Jacks** on each strip's bottom edge (input −11, output +11): a nut 12 × 6 with its lit
  top 10 × 4, an empty one's hole 6 × 2.8; a drop target's halo r 10 at 25 %, a refused
  one's ring 14 × 7.6 in `rec`.
- **Seated plug** (22 long, hanging below the edge): collar 5 × 4, barrel 7 × 7 r 1 with
  three knurl lines, the kind band 7 × 1.5 in the cable's colour, the boot tapering
  6.6 → 4.2 with three ribs. **Free plug** while dragging (36 long, tip up): tip,
  insulator, ring, insulator, sleeve with its highlight, then the seated parts; a refused
  drag turns its boot `rec`. Both are des-mixer's SVG part for part (`wires::plug_svg`,
  the metal and boot gradients), rasterised **once per (shape, colours, scale)** into the
  kit's layer atlas and placed as one quad (`cables::plug`).
- **Cords**: output 3.2, send 2.2 + 1.2·level, key 2.8 with dark spiral marks (1.4/3.2).
  Back to front: a two-pass shadow (offset 1.5, 2.5; w + 6 and w + 3 at 12/30 % Night,
  7/18 % Paper; hovered, lifted to 3, 5 and lighter), the edge (w + 1.5,
  `mix(col, edge, .55)` / Paper `mix(col, ink1, .35)`), the body, the highlight (0.35·w,
  offset (−0.3, −0.22·w), 60 %, 85 % hovered). The materials (metal, boot, nut, hole,
  shadow) are the README's, per light (`wires::material`).
- **Sag**: a catenary, s = min(52, 12 + 0.16·span), its parameter solved by 80 steps of
  bisection **once per route** (kept while its ends and sag hold), sampled at 28 points,
  each end leaving its boot straight down through a quadratic (control 7 below). The
  dragged cord: a cubic with vertical tangents, k = min(60, 18 + 0.18·span),
  P2 = pointer + (0, 0.7·k).
- **Settle**: a route a drop made rings for 300 ms, s(t) = s0·(1 + 0.12·e^(−t/90)·
  cos(2πt/160)), a frame each tick (the ticker's *Animation*), then exactly s0 and still —
  nothing repaints at rest.
- Flow's gleam rides D's cords too while playing (an 8-px dash every 64,
  `mix(col, white, .75)`), by the same rule (`CableStyle::gleams`).
- The vintage panel (cloth cords, nickel, bakelite, the brushed rail) is the vintage
  round's: its theme will set `System::cables = Jack`.

### The band (`views/mixer_cables.rs`, rewritten on `mixer_wires`; `mixer_lanes.rs` removed)

- **What is drawn** is as before (the Cables key: the selection's cables in and out; *Show
  all*: every route), with des-mixer's opacities: the others at **45 %** in Show all; a
  strip that receives (a bus, a return) draws its way to the master in Show all, so a
  chain reads end to end.
- **Hover**: the cable under the pointer (nearest within 4 pt of its stroke) lights its
  **whole route to the master** — its own and the output chain from its target (Kick →
  Drum bus → Master, drawn even where Selected would not) — and everything else drops to
  **22 %**; a tip by the pointer and the hint bar name the route. The chain is drawn from
  the frame after the hover lands (one repaint asked for on the change, none at rest).
- **The channel** under the strips (`cable.channel`, r 4) is the cables' ground; the band
  row is the 18-pt foot plus the channel (46 pt, more for Subway's lanes).
- **Drag to connect** (`mixer_drag`): the output jack is the source (plain = send, Ctrl =
  output, Alt = key, as ADR 0041); the dragged cable hangs from the jack with
  P2 = (px, py + 0.55·k) and ends in a 3.5-radius plug (under ring, highlight), in the
  kind's colour, `rec` when refused. The target's input jack shows the 22 % accent halo
  (valid) or a red ring and slash (refused). The words: **"Route Chords to Drum bus"** for
  an output, **"Refused: Kick already feeds Drum bus (a loop)"** for a loop (new keys
  `cables.drag.route`, `cables.refused.loop`); a release on a refused target issues
  nothing.
- **An input jack** a lit cable plugs into is that route's end: pulling it moves the route
  (round 26's end marks are gone — the jack is the end).
- Pills (the *Send levels* option), edge chips, click-to-select, Delete, Esc and the route
  menu are kept; a pill rides the cable's lowest point.

## Frame budget

Flow asks for **no frame of its own**: it is drawn on the frames the playing desk already
asks for (the meters' and the transport's, through the ticker), and nothing when the
transport is stopped or the source silent. A hover asks for one frame when the hovered
cable changes. Each frame flattens the drawn cables once into one reused point list (no
allocation once grown), strokes into the desk's one brush (no draw call of its own).

Measured (`frame_budget`, deterministic, debug build, the 64-strip desk wired with 128
sends and 16 keys, *All*, playing, 1920 × 1080):

| | allocations a frame | draw calls | triangles |
|---|---|---|---|
| cables off | 19 | 9 | 32 416 |
| A Patch | 19 | 9 | 113 346 |
| B Flow (default) | 19 | 9 | 113 346 |
| C Subway | 19 | 9 | 90 510 |
| D Jack | 19 | 9 | 248 820 |

No style adds an allocation or a draw call; A–C stay under the 120 000 ceiling the band
test holds; D draws five strokes a cord and a plug at each end, and is held to its own
ceiling (260 000, `JACK_TRIS`) — that is *All* on 200-odd routes at once; the selection's
cables cost a few thousand. **At rest no style asks for a frame** (the desk-alone rig: every
style, stopped, `repaint == MAX`; the whole window through `assemble`: the ticker's
reasons empty). Playing, Flow's frames are the ones the desk asks for with the cables off
(the e2e compares the ticker's request with and without them). D's settle asks the ticker
for frames (*Animation*) for 300 ms after a drop, then nothing.

The unchanged budgets: `mixer64_alone`, `mixer16_alone`, `mixer64_window`,
`an_idle_desk_requests_no_repaint`, `slate_budget`, `workspace_budget`, `views_budget`
pass as before (the band is off at rest, option C).

## Tests

- `tests/cables39_e2e.rs` (new, through `CantusApp::assemble`, the demo song, the window
  as the program runs it — `eframe::App::ui`): **each style** chosen in the Cables menu
  draws the Kick's cables in its shape (A/B a cubic whose lowest point is 0.75·k below the
  jacks; C orthogonal runs at its lane; D a catenary 3 + s below the boots, from the plugs
  22 below the nut) and *Show all* (others at 45 %, the drum bus's way to the master);
  **hover** lights Kick → Drum bus → Master, dims the rest to 22 % and names the route
  (hint and tip), in every style; **drag** from the Chords' output jack says "Route Chords
  to Drum bus" and routes; from the drum bus to the kick says "Refused: Kick already feeds
  Drum bus (a loop)" and a release issues nothing; **Flow** draws gleams only while
  playing with signal, none stopped, none in Patch or Subway, rides the frames the desk
  already asks for and asks none at rest; **D settles**: its sag rings for 300 ms, a
  frame each tick, then exactly s0, the same cord frame after frame, and nothing asked;
  **the setting**: none chosen → the theme's (Slate's Flow), Subway chosen in the menu is
  kept across a restart, Patch chosen in Settings ▸ Appearance likewise.
- `views::mixer::wires` unit tests: the README's numbers (sag, widths, jacks), the cubic
  hangs from its jacks, the subway is orthogonal, **C's lanes never overlap** (a property
  test over 400 random desks of 2–64 strips: any two cables whose spans overlap are at
  least a sub-lane apart, kinds in their order), the gleam moves with the transport's
  time and stands still with it.
- `settings` unit test: the style follows the theme unless chosen; a theme file names
  its own.
- `frame_budget::every_cable_style_costs_no_allocation_and_no_draw_call` (new, above).
- Changed (the approved design supersedes them): `mixer_desk` — the port-to-port test
  (the cable ends at the target's *input* jack; in *All* the return's way to the master is
  drawn), the clicks on a cable's run (`cable_path`), the lanes test now holds that a
  cable's shape never moves under the hover and that pulling the plug out of an input jack
  moves the route; the two round-26 end-mark tests are replaced by
  `the_hovers_cables_are_pointable_and_hung_by_their_span` (no end marks: the jack is the
  end). `frame_budget` — the hovered bus's inputs are lit and whole (no "yielded"
  cables). `mixer_cables_view` — clicks on `cable_path`. `mixer_drag`/`mixer_routing` —
  the loop's words are des-mixer's ("Refused: … (a loop)"), Ctrl's "Route 2 to 5".
- Green on this branch: the lib (498), `cables39_e2e` (6), `mixer_desk` (34),
  `mixer_cables_view` (10), `frame_budget` (18), `mixer39_e2e`, `mixer_routing`,
  `route_entry_points`, `mixer_targets`, `mixer_vca`, `slate_budget`, `slate_e2e`,
  `workspace_budget`, `views_budget`, `kit_lint`, `panels`, `ux_whole_words`,
  `ux_help_line`. `click_paths::p09_change_the_tempo` fails as on the base (el-mixer's
  report: the top bar's Tempo). `frame_budget`'s `stage_budget::a_dancer_on_screen` failed
  twice when it ran beside a release build and passes run alone (its triangle count
  against the window without the dancer; not the band's — the cables are off at rest).

## Pictures (`docs/round39/el-cables/`)

`shoot_cables39` (`cargo run --release -p cantus-ui --example shoot_cables39 -- <dir>
[filter…]`): the whole window, 1920 × 1080 at 125 %, the demo song **playing**, the mixer
maximised, Kick selected, the Cables key on —
`cables39-<patch|flow|subway|jack>-<sel|all|hover|drag|refused|stopped>-slate_<night|paper>.png`
(48 pictures): the selection's cables, Show all, the pointer on Kick → Drum bus, a cable
held from the Chords over the drum bus (Ctrl: an output), one held from the drum bus over
the kick (a loop), and Show all stopped (no gleam).

Compared with des-mixer's sheets: the jacks, curves, widths, colours, opacities, the
hover's route and tip, the drag's plug, halo and refused ring and the words match the
spec. Differences: the window's desk is denser than the sheet's (16 strips, every demo
route), so *All* piles more cables at the channel's floor than the sheet's 7; the target
column is also washed while dragging (round 25's column wash, kept); C's channel deepens
with the lanes *All* needs, so the strips above it give that height back.

## Verified only in emulation

Everything is drawn by the CPU renderer (`raster.rs`) through `shoot_cables39` (the whole
window, the demo song, meters fed by the real engine running offline) and driven by
synthetic pointer events in the tests. The gleam's motion at 60 Hz on a real screen, the
plug following a real mouse, and the 125 %/150 % pixel placement of the 1.25-px rings need
the owner's eyes on the machine (beta checklist).

## What is left

- **For the kit builder / ADR 0111**: D's materials (metal, boot, nut, hole, shadow) are
  the README's fixed colours, in `views/mixer_wires.rs` (`material`) — a vintage theme
  will want them as tokens; the cable tokens (`cable.out/send/key/refused/channel`) are
  derived in `wires::tokens` from the theme, as the README defines them.
- The vintage panel (cloth cords, nickel, bakelite, brushed rail, engraved labels) is the
  vintage round's; its theme sets `System::cables = Jack`.
- D's strokes are tessellated into the desk's mesh each frame the desk repaints (as every
  desk part is); what is cached per route is its catenary (solved once), and per (shape,
  colours, scale) each plug's raster. The desk repaints only when something changes, so
  at rest this costs nothing.
- `kit/cable.rs` (round 25's cable look) is no longer used by the desk (the routing view
  and the gallery still use parts of it).
- Owner's eyes: the gleam's motion at 60 Hz, D's settle on a real drop, the plugs at
  125 %/150 % on a real screen; which style reads best on a 40-strip desk.
- An observation for the integrator: through `assemble` in a headless test, the window
  asks egui for a repaint every frame even with the cables off and the transport stopped
  (`repaint_delay` 0, the ticker's own reasons empty) — something outside the mixer asks
  `request_repaint` directly; the desk-alone budget rig is quiet.
