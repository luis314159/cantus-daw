# Round 25 — mixer routing drawn as cables (BACKLOG §6d.3): the design

**Status:** proposal for the owner, not built. Written on `r24/integration` (schema 22,
next free ADR 0041) without running a build (a soak test held the machine). The research
behind it is `research/mixer-routing/notes.md`; every line reference below was read on
this branch.

## The ask (BACKLOG §6d.3, verbatim)

> A better mixer — cabling between channels, as in FL Studio. Routing drawn as cables
> between strips (send/route lines, with their level), made by dragging from one strip to
> another; a routing view/matrix; sidechain sources shown in the same way; the "Nothing
> feeds this strip yet" hint (seen on the Reverb/Delay strips) turns into a direct "route
> here" action. Remove the channel's effect slots from the Extra large view (the inserts
> live in the Inspector; the strip shows level, pan, sends and routing). Take the best of
> FL Studio's mixer and of Ableton, Bitwig, Logic, Studio One and Reaper's routing.

## The design in one paragraph

The routing graph Cantus already has — one **output** per strip, up to sixteen
**post-fader sends** with a level, and **sidechain keys** chosen on an insert — is drawn,
not changed. Under the strips runs a **cable band**: each route is an arc from the source
strip's port to the destination's, in the source's colour, styled by kind (output solid,
send thinner with its **level on the cable**, key dashed with a key mark), the selected
strip's cables lit and the rest faint or hidden (a view switch). A cable is **made by
dragging** from a strip's port onto another strip (plain = send, Ctrl = route the output
there, Alt = key a sidechain), **moved** by dragging its end, **removed** from its menu,
with Delete, or by dropping its end on nothing. A **Routing** tab shows the same graph as a
**matrix** (sources × destinations, tracks → strips beneath) for projects too big to read
as arcs. A strip nothing feeds shows a **Route here** plate that opens the right menu in
one click. **Extra large** loses its insert slots and gains what the owner asked the strip
to show: its own sends as named rows with level bars, and its output. The core needs **no
schema change** for any of this; one new command fixes an undo order the cables would
expose. Pre-fader sends are offered as a separate, optional piece (schema 23).

---

## 1. What exists today (read on this branch)

### 1.1 The model (`crates/cantus-core`)

| What | Where | Notes |
|---|---|---|
| A track feeds one strip, `None` = master | `model.rs:482` `Track::strip` | several tracks on one strip = a bus |
| `Send { to, level 0…1 linear, enabled }`, post-fader | `model.rs:509-535` | a switched-off send keeps its level and stays an edge |
| `MAX_SENDS = 16`, `MAX_INSERTS = 10` | `model.rs:539`, `:543` | |
| `MixerStrip` | `model.rs:702-760` | `output: Option<StripId>` (`:744`, `None` = master, **unity, no level**), `sends: Vec<Send>` (`:748`), `dock` (`:755`), `group` (`:758`) |
| `feeds()` = output + every send; `keys()` = the strips its inserts are keyed from | `model.rs:785`, `:792` | |
| `EffectSlot::sidechain: Option<StripId>` (the key), `midi_from: Option<TrackId>` (notes into an effect, ADR 0022) | `effect.rs:342`, `:349` | keyable kinds: `EffectKind::takes_sidechain` `effect.rs:231` (EQ, compressor, limiter, Space, multiband, de-esser, gate, …; hosted plug-ins per plug-in `:302`) |
| Cycle check over outputs, sends **and keys** | `model.rs:1140` `routing_would_cycle`, `:1162` `downstream` | |
| Solo with connected routes (outputs + sends, not keys) | `model.rs:1181` `connected_strips`, `:1222` `strip_audibility` | |
| Processing order (topological, stable) | `model.rs:1244` `strip_order` | |
| `senders_to(id)` | `model.rs:1234` | exists, unused by the hint |
| Strips on the audio thread | `snapshot.rs:108-130` `StripSnapshot` | `output: Option<usize>`, `sends: Vec<(usize, f32)>` enabled and non-zero only |
| Engine: a send is `post_l/post_r × gl × level` (post-chain, post-fader incl. pan law), dropped when the source is muted | `cantus-engine/src/graph/strip.rs:210-221` | delayed sends go through PDC lines (`In::Line`, `graph/ctx.rs:92-102`) |
| Engine: a key strip keeps `l × gl` — **post-fader, pre-mute** | `graph/strip_run.rs:247-258`, read at `:137-146` | so a *muted* ghost-kick strip still keys; FL's "sidechain-only" zero-level route is not needed here |
| `MAX_STRIPS = 256` | `cantus-engine/src/messages.rs:13` | sizes the matrix |

**Commands that already exist** (`command.rs`): `SetTrackStrip` (`:139`), `InsertStrip`
(`:147`), `RemoveStrip` (`:153`, refused while anything feeds/sends to it),
`SetStripOutput` (`:182`, refuses cycles), `SetStripSend` (`:213`, add-or-change matched
by destination, refuses cycles / self / a 17th), `RemoveStripSend` (`:217`),
`SetEffectSidechain` (`:394`, refuses non-keyable kinds, self, cycles), `InsertEffect`,
`SetEffectMidiSource`, `MoveStrip`, `SetStripDock`, `SetStripGroup`, `Batch`. App batches:
*route/send selected here* (`cantus-ui/src/app.rs:878` `route_strips_to`, `:898` `send_strips_to`), `set_strip_send` (`:923`,
merging), `create_submix` (`:937`), `route_track` (`:1362`), `duck_strip_from`
(`views/comp_editor.rs:740`: a ducking compressor keyed from a strip, one batch).

**A suspected undo fault the cables would expose.** `RemoveStripSend`'s inverse is
`SetStripSend`, which *pushes* (`command.rs:1296`, `:1310`): removing the first of two
sends and undoing leaves them in the other order, and the undo fuzz compares projects with
`==` (`tests/undo_invariants.rs:1453`). Audio is unaffected (sends add into different
buffers) but the project is not restored exactly. Not confirmed by running (no builds this
session); §6 makes it the first test.

### 1.2 The desk (`crates/cantus-ui/src/views/mixer*.rs`)

- **Width modes** Compact 24 / Wide 48 / Extra large 76 pt (`mixer_layout.rs:16-24`);
  rows in `Row` (`:113-150`) with `Row::Inserts` only at Xl (`:326-329`); fit-then-scroll
  with a drop order (`DROPS`, `:299-306`).
- **FL's send row** (`mixer_parts.rs:32-226`): relative to the *selected* strip — on the
  selected strip a ▼ chip with its output's number (click: the route menu); on the others a
  knob where it already sends (drag = level), a lit ▲ where its output goes, a hollow ▲
  where it would loop, a white ▲ otherwise (click = send, Ctrl+click = route only).
  Loops for the selection are worked out once per (revision, selection) in
  `mixer_desk.rs:147` (`Desk::routes`, `would_cycle`).
- **No cables are drawn.** The FL research lists them (`research/ui/fl-studio.md` §4.2
  row 13, "routing cables … optional") but nothing paints them.
- **Extra large insert slots**: `mixer_parts.rs:228-427` (`inserts`, `slot_drop`), called
  from `mixer_strip.rs:492`; the master's slots too (ADR 0026); the list height comes from
  `Desk::most_inserts` (`mixer_desk.rs:59`, used at `mixer.rs:506`, `:518`).
- **"Nothing feeds this strip yet"** is `mixer.fed_by_none` (`locales/en.txt:518`), the
  hover text of the strip's name cell (`mixer_strip.rs:579-591`). It looks only at
  **tracks** (`tracks_on_strip`), so a return fed by sends — every demo's Reverb and Delay
  (`demos/song.rs:183-193`, `demos/*.rs` `s.ret(..)`) — says nothing feeds it. **That is
  the bug the owner saw**, and a pure tooltip: there is no action on it.
- **Menus** (`mixer_menu.rs`): the strip menu's *Routing* band (`:332-362`: Output ▸,
  Send to ▸, Route/Send selected here, Create submix, Route "track" here, Duck this strip
  from ▸), the route menu and the arrow menu (`:410-452`), the lists built when they open
  (`:590-652`).
- **Cost**: everything derived from the project is cached per revision (`mixer_desk.rs`
  `Desk`); only visible strips are laid out; one `Brush` mesh; held by
  `tests/frame_budget.rs:263-390` (mixer 64 ≤ 4 ms, 16 ≤ 2.5 ms, allocation and
  draw-call counts asserted, idle = no repaint).
- **The kit** has `Brush::segment` / `polyline` (`kit/brush.rs:717`, `:737`) but no
  curve; `views/inst/fm_graph.rs:193-209` has a sampled Bézier and a dashed polyline (the
  Bézier allocates a `Vec`).

### 1.3 The inspector

- Sends, one row each: lamp, destination menu, a **disabled Pre/Post segmented control**
  that says "post only", the level (`views/inspector_sends.rs:16-163`); routing: chain
  delay, output latency, the output menu (`:174-220`).
- Inserts as tiles (`views/inspector_channel.rs:172`) — where the inserts already live,
  which is what lets Extra large drop them.
- Nothing lists what *feeds* a strip (no "receives").

---

## 2. Decisions taken from the research (summary; the evidence is in the notes)

1. **Keep the model: one output + sends + keys** (Ableton, Bitwig, Logic, Studio One — and
   what Cantus has). FL's "every route has a level, a track may go to many places at once"
   is the outlier; adopting it would be a schema, engine, solo and PDC change for a
   behaviour sends already give. The cables make the asymmetry *visible* instead: the
   output is the solid cable, a send is the thinner one with a level on it.
2. **Draw FL's cables, but legible at 64+ strips**: an arc band under the strips (never
   across a control), the master route implied rather than drawn for every strip, the
   selection's cables lit and the rest faint or hidden (Reason's cable-visibility switch,
   FL's own option to hide them), kinds told apart by stroke, not only by colour.
3. **Drag from a strip to a strip** (Reaper's drag from a track's route button onto another
   track, with Ctrl meaning "and cut the master send"; Reason's drag-to-patch and its
   drop-an-end-on-nothing to disconnect), with FL's click-the-arrow kept for speed.
4. **A matrix for the big picture** (Reaper's routing matrix), in a tab, sharing the
   desk's menus and commands.
5. **A strip that receives shows its senders** (Bitwig's FX-track send-receive pane,
   Reaper's *receives*): the inspector grows a *Receives* section — a mixer for one bus.
6. **New return from a send menu** (Logic creates the aux when you pick a free bus; Studio
   One adds an FX channel from a send): one batch makes the return in the right dock and
   the send, optionally with a reverb or a delay already in it.
7. **Route here** replaces the false hint: one click, the menu that fits the strip
   (a return offers *send* first, a channel offers *route a track*).
8. **Extra large shows sends and routing, not inserts** (the owner's ask; Logic and
   Studio One draw sends as named slots with a level; Bitwig names them in the strip).
9. **Avoid**: FL's "route to this track only" also deleting the tracks' effect sends (a
   documented complaint) — our Ctrl sets the output and never touches sends; FL's
   spaghetti when every cable is drawn; Ableton's routing hidden behind an
   optional I/O section and context menus; Logic's auto-created aux strips piling up
   unnamed; Reaper's matrix as the *only* readable view of a large project; long
   single-column destination menus at 100+ strips (we group by dock and filter).

---

## 3. The desk: cables

### 3.1 The cable band

- A new row **`Row::Cables`** at the bottom of every strip, under the send row: 32 pt by
  default, 20 pt minimum, up to 64 pt with a drag handle at its top edge (double-click
  resets), on/off in the desk's ▾ *Rows* menu like every row family. Its legend reads
  **ROUTING**.
- Every strip (and the master, and a collapsed group's separator) has a **port** at the
  top centre of the band: a 7-pt ring in the strip's colour, filled when the strip goes
  somewhere other than the master (an output to a strip, or any send), hollow when it goes
  only to the master.
- A cable is an **arc that hangs below the ports**: a cubic from the source port to the
  destination port, its depth proportional to the horizontal distance (capped by the band
  height), so cables from one strip nest instead of crossing and a short hop is a shallow
  hook (an arc diagram). Direction is read from the end mark: an output ends in a filled
  dot, a send in a small chevron, a key in the key glyph.
- **The master route is implied.** A strip whose output is the master draws no cable in
  *All* mode (sixty identical cables into the master say nothing); the **selected** strip
  draws its master cable, as FL does.
- Cables paint in a **fourth pass after the three docks**, clipped to the band across the
  whole desk width, so a cable from a middle strip to a return in the right dock crosses
  the dock divider and nothing else. Nothing in the band is a control except ports, pills,
  end marks and edge chips, so nothing collides with the strip's controls.

### 3.2 What is drawn, and how

| Route | Stroke (at 100 %) | Colour | End mark | Level |
|---|---|---|---|---|
| Output to a strip | 2.0 pt solid | source strip's colour | filled dot | none — unity; the level is the source fader |
| Send, on | 1.5 pt solid | source colour | chevron | **a pill at the arc's lowest point**: "−6.0" (dB). Thickness does not encode level (unreadable) |
| Send, off | 1.5 pt dashed, 40 % alpha | source colour | hollow chevron | pill dimmed |
| Send, pre-fader (piece F only) | as a send, with a 2-pt gap at the source end | source colour | chevron | pill reads "PRE −6.0" |
| Sidechain key | 1.25 pt dashed | the *Dynamics* function hue (a kit token), never the strip colour | key glyph | the keyed effect's short name ("Comp 2") |
| Output to the master | 2.0 pt solid | source colour | — | drawn for the selected strip only |

Kinds differ by **stroke and end mark**, so the band reads in grey scale and for
colour-blind users; colour carries identity only (Bitwig's rule, `research/ui/bitwig-studio.md`).

*Notes into an effect* (`EffectSlot::midi_from`, a track, not a strip) are not drawn in
the band: a track has no port on the desk. They appear in the matrix's tracks block and in
the effect's own chooser (open question 8).

**Visibility** is a desk preference kept per window layout like the width mode (not
project data): **Cables: Off · Selected (default) · All**, in the ▾ menu and as a
three-way key in the title row.

- *Selected*: the cables into and out of every selected strip, at full strength.
- *All*: every non-master cable; those not touching the selection or the hovered strip at
  35 % alpha and as unfeathered hairlines, so the selection still reads first.
- Hovering a strip lights its cables in either mode (and its senders' number cells);
  hovering a cable lights it and both ends' number cells; the hint bar names it
  ("Keys → Reverb · send −6.0 dB · post-fader").

**Off-screen ends.** When one end of a cable is scrolled out of the middle dock, the cable
runs to the middle's clip edge and ends in an **edge chip** with the far strip's number
("37 ›"); a click scrolls that strip into view (the existing `follow_selection` path).
Both ends off on the same side: not drawn. A collapsed group gathers its members' cables at
its separator with a "×n" chip.

**Level pills** are cached galleys (`kit::cache`) and appear only where they fit: in
Compact on hover and while dragging; in Wide and Extra large on the lit cables. They never
overlap: pills are placed left to right and one that would collide with the previous is
dropped (its value is still in the hint bar, the knob and the inspector).

### 3.3 Making, moving and removing a cable

**Make — drag from a port.** Press on any strip's port (or the selected strip's ▼ chip in
the send row) and drag: a rubber-band cable follows the pointer; the strip column under it
is the target.

| Drop with | Does | Command |
|---|---|---|
| nothing held | **add a send** at the default level; the source's output is unchanged | `SetStripSend` |
| **Ctrl** | **route the output there** (there is one output, so this is FL's "route to this track only") | `SetStripOutput` |
| **Alt** | **key a sidechain** from the source on the target: one keyable insert with no key → keyed; several → a menu of them; none → *Add a ducking compressor keyed from …* (`duck_strip_from`) or *Add a gate keyed from …* | `SetEffectSidechain` / the duck batch |
| onto the **master** | route the output to the master | `SetStripOutput { output: None }` |

- If the dragged strip is in the multi-selection, **every selected strip** sends, routes or
  keys (one `Batch`, one undo step — `route_strips_to` / `send_strips_to` already do it).
- **Refused targets show before the drop**: a loop target's column is hatched and the hint
  says why ("would make a loop: 9 already feeds 4"). `Desk::routes` already marks loop
  targets per selection; during a drag the same walk runs once for the dragged strip. The
  strip itself and a strip that already receives that kind of route are not targets. A
  refused drop issues nothing.
- The middle **auto-scrolls** while the pointer is within 24 pt of its edges.
- The modifier shows on the rubber band as it changes (solid / thin / dashed) and in the
  hint bar ("Send · Ctrl: route the output · Alt: key a sidechain").
- **FL's click is kept**: the send row's ▲ still sends the selected strip there and
  Ctrl+click routes it — the fastest path for one strip; dragging is the visible one.

**Select and act.** A click on a cable selects it (a 2-pt halo, both number cells
outlined); Delete or Backspace removes it; Esc deselects. Right-click a cable — or a send
knob, or a matrix cell — opens **the route menu**, the same in all three places:

- *Level…* (type dB) and the level itself (drag the pill: 200 px for the travel, Shift
  fine, double-click = 0 dB, the house knob rules) — sends;
- *Switch off / on* — sends;
- *Make this the output* (send → output) / *Make this a send* (output → send, the output
  going back to the master);
- *Pre-fader / Post-fader* (piece F);
- *Open the keyed effect* — keys; *Show the destination* (scrolls to it and flashes its
  number cell, Logic's Shift-click on a send, without changing the selection);
  *Select strips routed here / from here* (FL's Select submenu);
- *Remove*.

**Move an end.** Dragging a lit cable's end mark onto another strip moves that route,
keeping a send's level and switch (`Batch [RemoveStripSend, SetStripSend]`), an output's
source (`SetStripOutput`) or a key's effect (`SetEffectSidechain`). Releasing the end over
nothing (the band's ground, outside every column) **removes** the route, and the hint says
so while it is held ("Release to remove"). Undo brings it back exactly.

**Keyboard.** With a strip selected, the menu key (or Shift+F10) opens its menu, whose
*Routing* band lists the output, every send and every key with their actions (plain `R` is
taken by Record, so no letter is proposed; open question 9). Every matrix action has a key
(§5). Nothing is reachable only with the mouse.

### 3.4 The send row, the docks and the groups

- The send row keeps FL's meaning (relative to the selected strip). Its legend becomes
  **FROM SELECTED**, so it is not confused with Extra large's own send list (§4).
- Ports and cables work across the three docks; pinned strips are always valid targets,
  which is what makes "drag to the reverb on the right" one gesture at any scroll.
- A collapsed group refuses drops and springs open after 600 ms of hovering with a cable.

---

## 4. "Route here", Receives, and Extra large without inserts

### 4.1 Who feeds a strip — one function, and the hint fixed

`Project::feeders(id) -> Feeders` in `cantus-core` (tested, §7): the tracks on the strip,
the strips whose **output** is it, the strips that **send** to it (with level and switch),
and the strips that **key** one of its inserts. Every reader uses it: the name cell's hover
("Fed by Keys, Bass · sends from Pad −12 dB, Lead −6 dB"), the route-here plate, the
inspector's Receives, the matrix's column summary. `mixer.fed_by_none` is retired.

### 4.2 The Route here plate

A strip with **no feeders at all** draws, inside its meter well (which is silent anyway, so
nothing is covered):

- Wide and Extra large: a dashed `Well` with **+ Route here** in the label role;
- Compact: a **+** glyph centred in the well; the words go to the hint bar and tooltip.

A click opens the **Route here menu**, built when it opens, ordered by what the strip
probably is:

| On a strip in the middle (a channel) | On a strip in a pinned dock (a return or bus) |
|---|---|
| Route "‹selected track›" here | Send "‹selected strip›" here |
| Route a track here ▸ (tracks with no strip first, then the rest with their number) | Send the selected strips here (n) |
| Route the selected strips here (n) | Send a strip here ▸ (loops disabled, grouped by dock) |
| Send a strip here ▸ | Route the selected strips here (n) · Route a track here ▸ |

Each item is one command or one batch (`SetTrackStrip`, `route_strips_to`,
`send_strips_to`). The plate is also the drop target of §3.3, and its hover says "or drag a
cable here". The plate disappears the frame the strip receives anything.

### 4.3 New return from a send menu

Every *Send to ▸* list (strip menu, route menu, inspector's *Add send*, matrix) ends with
**New return ▸ Empty · Reverb (Spatium) · Delay**: one `Batch` of `InsertStrip` (the right
dock, named "Return n" / the effect's name, the return colour the demos use),
`SetStripSend` (source → new return, default level) and, for the two presets,
`InsertEffect` with the factory preset at 100 % wet. The new return is selected, so the
next thing typed renames it. (Logic's auto-aux, without Logic's unnamed "Aux 7" pile-up.)

### 4.4 The inspector's Receives section

Between *Sends* and *Routing* in the channel sheet (`views/inspector_channel.rs`, ADR 0024
§7's row grammar), for the selected strip:

- one row per **sender**: lamp (the send's switch), source name (click selects it), the
  send's level as the sheet's value well (drag = `SetStripSend`, merging per gesture);
- one row per **strip routed into it** (output): source name and "route" in the value
  column; its menu offers *Make this a send* and *Remove* (back to the master);
- one row per **keyed insert** of its own: "Comp 2 ← key from Kick";
- tracks feeding it are already the *Input* row at the top.

Empty: a placeholder well **"Nothing feeds this strip — route here"** that opens §4.2's
menu. This is Bitwig's send-receive pane: a return can be mixed from its own sheet.

The disabled **Pre/Post** control in each send row stays disabled (post only) unless piece
F is accepted, in which case it becomes live.

### 4.5 Extra large: level, pan, sends and routing

- **`Row::Inserts` is removed** from the desk, in every mode and for the master: the
  inserts live in the inspector (tiles, `inspector_channel.rs:172`), where the master's
  chain already shows when the master is selected (ADR 0026). `mixer_parts.rs` loses
  `inserts` / `slot_drop` (≈200 lines); `Desk::most_inserts` and the `slots` argument of
  `layout::fit` go; `Drop::FewerSlots` goes.
- The **FX row** stays in every mode (bulb = chain in/out; record dot). Extra large adds
  the **insert count** beside the bulb ("3"); a click on the count selects the strip and
  scrolls the inspector to its inserts.
- **New `Row::SendList` (Extra large only)**: the strip's **own** sends, one 16-pt row
  each — lamp (switch), destination number and name (elided in the middle), and a 2-pt
  level bar along the row's foot in the *Space* hue; drag on the row = level (house rules),
  right-click = the route menu, a click on the name selects the destination. Rows shown:
  as many as the strip with the most sends needs, 2 minimum, 6 maximum, then a "+n" row; a
  last empty well **+ Send** opens the add-send list (with *New return ▸*). Identical
  geometry on every strip, so the rows read across the desk.
- **New `Row::Output` (Wide and Extra large)**: a `MenuField` chip "→ Bus 12" / "→ M",
  a click opens the output list. In Compact the selected strip's ▼ chip in the send row
  already says it.
- The drop order when the desk is short (`DROPS`) becomes: EQ bars → width → phase/swap →
  readout → name shrinks → **send list shortens to 2** → **cable band to 20 pt** → output
  chip off. The cable band never drops entirely while *Cables* is on; it is the routing.

Extra large's strip, top to bottom: number · name · meter · mute/solo · pan · fader ·
readout · phase/swap/width · EQ · FX + count + record · **output** · **send list** ·
from-selected row · **cable band**. That is the owner's list — level, pan, sends,
routing — and nothing that is the inspector's.

---

## 5. The Routing tab: a matrix

A new view, `views/routing.rs` (+ `routing_paint.rs`, `routing_cells.rs`), a dockable tab
"Routing" (egui_dock, ADR 0024 §5) in the lower zone beside Mixer / Piano roll / Channel
rack; opened from a **Matrix** key in the desk's title row, *View ▸ Routing*, and a
keymap action (default key: open question 9).

**Layout.**

- **Strips → strips** (top block). Rows = sources in desk order, each with its number cell,
  colour and name (frozen at the left). Columns = destinations: **M** first, then the
  strips in desk order with **dock dividers** where the desk has them, names rotated
  (frozen at the top). Cells 16 pt square at 100 %.
- **Tracks → strips** (below, the same columns): one row per track; exactly one cell per
  row is lit (the strip it feeds, or M) — a radio. A click moves the track
  (`SetTrackStrip`).
- A **filter** in the tab's toolbar: *All · In use* (hides rows and columns with no route
  other than to the master and no track), on by default above 32 strips; and a name filter.
- Hovering a cell lights its row and column (a crosshair) and both names, and the hint bar
  says what a click would do.

**Cells.**

| State | Look |
|---|---|
| empty, routable | the sunken well |
| would loop | the well hatched at 30 % — refused, with the reason in the hint |
| self | a diagonal, no well |
| output | a filled dot in the source colour |
| send on | a ring with the level as an arc (a 12-pt mini-knob, from 0), source colour |
| send off | the same ring dimmed and dashed |
| key | the key glyph in the *Dynamics* hue (several keyed inserts: the glyph and a count) |
| output and a send to the same strip | the dot inside the ring (allowed today: `SetStripSend` does not refuse a send to where the output already goes — a doubled path; open question 5) |

**Interactions** — the same grammar as the desk:

- click an empty cell = add a send; **Ctrl+click** = route the output; **Alt+click** = key
  (menu when several keyable inserts, the duck/gate offers when none);
- click a send cell = select it; drag vertically on it = level (house knob rules);
  double-click = 0 dB; right-click = the route menu (§3.3); **Shift+drag down or up a
  column copies the send** (same destination, same level) to the rows it crosses, one
  `Batch` (Reaper's matrix);
- keys (claimed while the matrix has focus, as a focused knob claims the arrows): arrows
  move the focused cell; **Enter** add/remove a send; **Ctrl+Enter** route the output;
  **Alt+Enter** key; **+ / −** level by 1 dB (Shift 0.1); **Delete** remove; **Home/End**
  first/last column. No letters and not Space — Space is the transport and letters play
  notes when the typing keyboard is on.

**Why a matrix as well as cables.** Arcs are best for "where does *this* go" with a few
strips; a grid is best for "what goes where" across 64–256 (Reaper's experience: the
matrix is the one view that scales; its weakness — unreadable when it is the only view —
is answered by the desk). Both views are drawn from the same per-revision edge list.

**Later, not now.** Columns for the device's outputs (BACKLOG §6b's I/O routing matrix
page and a cue mix to a second pair) fit this grid as a third block; the design keeps the
column model open for them.

---

## 6. Rendering with the kit at many strips (ADR 0024 §13)

**Derived once per revision** (`mixer_desk.rs`, the existing `Desk` cache):

- `edges: Vec<Edge>`, capacity kept: `Edge { from: u16, to: To (Strip u16 | Master),
  kind: Output | Send { level, enabled } | Key { effect: u8 } }` — one walk over the
  strips, in desk order, so drawing order is stable;
- `fed: Vec<bool>` per strip (any feeder) for the route-here plate;
- prefix sums of item widths per dock for the current width mode, so the x of any strip
  (visible or not) is O(1) — needed for edge chips and arcs whose other end is off-screen.

The same edge list feeds the matrix (its own cell cache: N × (N+1) bytes, 66 KB at 256).

**Per frame:**

- Cables draw after the three docks into the **same `Brush`** (one mesh, no extra draw
  call), clipped to the band.
- Culling: a cable whose both ends are off the same side of the visible band is skipped
  before any arithmetic beyond two compares. In *Selected* mode only the selection's edges
  are visited (a per-strip edge index, also per revision).
- Each cable is a cubic sampled into a **stack array** (`[Pos2; 17]`, n = clamp(|dx|/24,
  6, 16) segments) and stroked with `Brush::polyline`; dashes with a dashed polyline moved
  from `views/inst/fm_graph.rs:209` into the kit (no `Vec`). Lit cables use the feathered
  stroke profile; faint ones the 1-px profile (k = 1), a third of the vertices.
- Two new glyphs in `kit/glyph.rs` (it has `Send` but no key or cable): **`Key`**
  (the sidechain end mark and matrix cell) and **`Cables`** (the title-row view key).
- A new kit painter **`kit::cable`** (`paint(brush, from, to, depth, &CableLook, state)`,
  `hit(from, to, depth, p) -> f32`) so the gallery test draws every kind × state × theme ×
  scale, and the matrix and the desk share one look.
- Hit-testing runs only when the pointer moves inside the band (or a drag is active),
  against the sampled points of visible cables kept in a capacity-kept scratch buffer.
- Text: pills and chips are cached galleys keyed by (value, theme generation, ppp); no
  `format!` in steady state (the existing `v.buf` pattern).
- **Idle stays idle**: cables are static; nothing animates (no "signal flowing" effect).
  A drag repaints only while the pointer moves.

**Budget** (added to `tests/frame_budget.rs`, the same rig): the 64-strip desk with
**128 sends and 16 keys** in *All* mode adds **0 allocations** in steady state, **0 draw
calls**, and stays inside the existing mixer-64 budget (≤ 4 ms p50 whole window); a
`cables_drawn` counter is asserted (*Selected* draws only the selection's; *Off* draws
none). Target cost of *All* at 64 strips: ≤ 0.3 ms of tessellation on the reference
machine, measured with the release timing test before the band is called done.

---

## 7. Commands, model and schema

### 7.1 No schema change for the cables, the matrix, route here or Extra large

Every route drawn is already in the project: `MixerStrip::output`, `MixerStrip::sends`,
`EffectSlot::sidechain` (and `Track::strip` for the matrix's tracks block). The cable
view, band height and matrix filter are **desk preferences** kept per window layout, like
the width mode (`mixer.rs` prefs), never project data — the same desk can show cables on a
studio screen and hide them on a laptop. **`SCHEMA_VERSION` stays 22** for pieces A–E.

### 7.2 Commands

| Action | Command(s) | Exists? |
|---|---|---|
| add / change a send (level, switch) | `SetStripSend` | yes (`command.rs:213`) |
| remove a send | `RemoveStripSend` | yes (`:217`) — **its inverse changes**, below |
| route the output | `SetStripOutput` | yes (`:182`) |
| key a sidechain / clear it | `SetEffectSidechain` | yes (`:394`) |
| key with no keyable insert | `Batch [InsertEffect, SetEffectSidechain]` (`duck_strip_from`) | yes (`comp_editor.rs:740`); a gate variant is new app code |
| route a track (matrix, route here) | `SetTrackStrip` | yes (`:139`) |
| several strips at once | `Batch` via `route_strips_to` / `send_strips_to` | yes (`app.rs:878`, `:898`) |
| move a send's end | `Batch [RemoveStripSend, SetStripSend]`, label `cmd.move_send` | new batch, no new variant |
| send → output | `Batch [RemoveStripSend, SetStripOutput]`, `cmd.send_to_output` | new batch |
| output → send | `Batch [SetStripOutput(None), SetStripSend]`, `cmd.output_to_send` | new batch |
| new return from a send menu | `Batch [InsertStrip, SetStripSend, InsertEffect?]`, `cmd.new_return` | new batch |
| **restore a removed send in place** | **`InsertStripSend { id, index, send }`** | **new variant** |

**`InsertStripSend`** exists to make undo exact: `RemoveStripSend`'s inverse becomes
`InsertStripSend { index }` (the send's old position), and `InsertStripSend`'s inverse is
`RemoveStripSend`. It refuses what `SetStripSend` refuses for a new send (unknown strips,
self, a loop, a 17th, a duplicate destination) and clamps `index`. It gets a `cmd.*`
string in both locales and an arm in the undo fuzz (random index, every refusal path), and
the fuzz's existing send arms start removing sends from the middle of the list. (A
batch-level alternative — sorting sends by destination — would change the order of every
existing file and need a migration; the new variant changes nothing on disk.)

While there: `send_strips_to` labels its batch `cmd.route_strips` (`app.rs:918`); it should
say `cmd.send_strips`.

**One place answers "can this route be made?"** — `Project::route_check(from, to, kind)
-> Result<(), RouteRefusal>` in a new `cantus-core/src/routing.rs` (kinds: output, send,
key; refusals: self, loop with the strip that closes it, no room, not keyable, already
there). The desk's hatching, the matrix's cells, the menus' disabled rows and the hint
texts all ask it; the commands keep their own checks (they must, for scripts and the MCP
server), and a property test holds that the two always agree. `routing.rs` also holds
`edges(&Project)` and `feeders(&Project, id)` (§4.1, §6).

### 7.3 Piece F (optional): pre-fader sends — schema 23

The inspector already draws a Pre/Post control that cannot move (`inspector_sends.rs:133-143`);
Bitwig, Logic, Ableton, Reaper and Studio One all offer pre-fader sends, and a cue mix
(BACKLOG §6b) needs them. If the owner wants them in this round:

- `Send::tap: SendTap { Post (default), Pre }`, `#[serde(default, skip_serializing_if =
  "SendTap::is_post")]`; **schema 23**, a migration that writes nothing, `fixtures/v23.cantus`
  with a pre-fader send; every old fixture keeps loading. `SetStripSend` carries the whole
  `Send`, so **no new command**; the fuzz's send arm randomises `tap`.
- Pre-fader = after the inserts, the channel EQ, width and swap; **before** the fader and
  the pan law; mute and solo still silence it (open question 4b).
- Engine: `StripSnapshot::sends` becomes `(usize, f32, SendTap)`; `In::Send`
  (`graph/strip.rs:210-221`) scales `post_l/post_r` by `level` instead of `gl·level` for a
  pre send; the PDC line feed does the same. One owner for the engine files.
- Null tests first: a post send at default renders **bit-identically** to today (no golden
  change, no `DSP_VERSION` bump); a pre send is unchanged by fader moves; at a 0 dB fader,
  centre pan and the 0 dB pan law a pre send equals a post send to the bit; block 7 against
  block 512 at tolerance 0; offline equals real time; `CANTUS_WORKERS=3` equals serial; a
  pre send into a latent return lines up as the post one does (`tests/pdc.rs`).
- DAWproject export/import maps the send type (to verify against the DAWproject schema).

---

## 8. Tests to write first

**Core** (`cantus-core`, no window):

1. *Undo restores a removed send in place*: two sends, remove the first, undo, `==`. Expected
   to fail today (§1.1); passes with `InsertStripSend`.
2. *Feeders*: tracks, outputs, sends (level and switch), keys — and a return fed only by
   sends is **not** empty (the owner's bug, pinned).
3. *Edges*: every output, send and key appears once, in desk order; disabled sends flagged;
   master outputs flagged; stable across a `MoveStrip` of an unrelated strip.
4. *route_check agrees with the commands*: on random graphs (the fuzz's generator), for
   every (from, to, kind), `route_check` is `Ok` iff the command applies.
5. Fuzz arms: `InsertStripSend`; the four new batches built from random strips.

**UI** (`cantus-ui`, the round-14 "paint and read back" style, no screen):

6. Layout: Extra large has no inserts row; the send list exists only at Extra large, the
   output chip from Wide; the cable band in every mode when on; the new drop order at 720 p.
7. Cable geometry (pure functions in `kit::cable`): depth grows with distance and is capped;
   two cables from one source with nested spans never cross; an off-screen end becomes an
   edge chip at the clip edge; both ends off on one side → nothing; hit distance.
8. Desk: a send from strip 2 to a right-dock return paints a cable ending at the return's
   port x (a `MixerView::cables()` debug view of what was drawn, like `strips_laid_out`).
9. Drag: press on 2's port, move over 5, release → one `SetStripSend`; with Ctrl →
   `SetStripOutput`; with Alt over a strip with one compressor → `SetEffectSidechain`;
   over a loop target → no command and the undo stack unchanged; from a multi-selection →
   one `Batch`.
10. Move an end; drop an end on nothing → removed; undo → the project `==` before.
11. Route here: an empty strip draws the plate; a demo return fed by sends does not; the
    menu's *Route "Kick" here* issues `SetTrackStrip`.
12. Inspector *Receives*: lists the senders with their levels; dragging one issues a
    merging `SetStripSend`.
13. Matrix: cells mirror the edges; click / Ctrl / Alt grammar; loop cells refuse; the
    keys of §5; *In use* hides untouched rows and columns.
14. Frame budget (§6): *All* mode with 128 sends and 16 keys at 64 strips — no new
    allocations in steady state, no new draw calls, `cables_drawn` counted, idle requests
    no repaint; the release timing test holds 4 ms.
15. The existing i18n test (every English key in Spanish) and the no-overlap test at 75 %
    and 200 % in English and Spanish, extended to the send list, pills, edge chips and the
    matrix headers.
16. The kit gallery: `kit::cable`, every kind × state × both themes × 1×/1.5×/2×.

## 9. Pictures to look at (`examples/shoot_mixer.rs`)

New variants (Slate and Paper, 1280 × 720 and 1920 × 1080, 16 and 64 strips), all from
`cargo run --release -p cantus-ui --example shoot_mixer -- shots <filter>`:

| Variant | Shows |
|---|---|
| `-cables` | a selected strip with an output to a bus, two sends (one off) and a key: the lit cables, pills, end marks |
| `-cables-all` | *All* mode on a demo project (the house demo has returns, a sidechain and buses) |
| `-cables-scrolled` | the selection's send to a strip scrolled out of view: the edge chip |
| `-drag` / `-drag-loop` | mid-drag over a valid strip; over a loop target (hatched, the reason in the hint bar) |
| `-routehere` / `-routehere-menu` | an empty return with its plate; the menu open |
| `-xl` (changed) | Extra large without inserts: output chip, send list, FX count |
| `-master` (changed) | the master's chain in the inspector, the desk without slots |
| `-receives` | the inspector of a return: the Receives section |
| `-matrix`, `-matrix-64` | the Routing tab, 16 and 64 strips, the *In use* filter on |
| `-cables-es`, `-cables-75`, `-cables-200`, `-xl-es` | language and scale |

Each picture is reviewed against `research/ui/screenshots/fl-studio/05-…` and `10-…`
(FL's cables and send icons) before the piece is called done.

---

## 10. Open questions for the owner

1. **Default cable view**: *Selected* (FL's, quiet) or *All* (everything faint)?
2. **What a plain drag makes**: a **send** (proposed — the output is unchanged, nothing
   disappears from the master) or a **route of the output** (FL's arrow click)?
3. **A new send's level**: 0 dB (today's `Send::new`), −6 dB, or silent and raised by hand
   (Ableton's sends start at −∞)?
4. **Pre-fader sends (piece F, schema 23)** in this round? (b) Should mute silence a
   pre-fader send (proposed: yes, as a send is part of the channel)?
5. **A send to the strip the output already goes to** doubles the signal; refuse it
   (proposed) or allow it as today?
6. Keep **FL's from-selected arrow row** in Extra large, now that the strip has its own send
   list? (Proposed: yes in every mode — it is the one-click path.)
7. Extra large: is the **insert count** beside the FX bulb enough, or a one-line chain
   summary ("EQ · Comp · +1") that opens the inspector?
8. **Notes into effects** (`midi_from`, ADR 0022): in the matrix's tracks block only, or
   also as a dotted cable from the strip the track feeds?
9. **Keys**: the Routing tab (Alt+F9 is free in `keymap.rs`; F9 is the mixer) and a
   direct key for a strip's route menu (plain R is Record, Alt+R arms a track).
10. **"Sends only"** (a strip that goes nowhere but its sends — Ableton's *Sends Only*,
    Reaper's master-send box): needed? Proposed **no**: keys are post-fader and pre-mute in
    our engine, so a muted ghost-kick strip still keys, which is what FL users route a
    zero-level track for.
11. Drag **a track or channel-rack channel onto a strip** to route it (FL's drag of a
    channel onto the mixer)? Small once the drop targets exist; proposed as part of C.

---

### Answers from the owner (2026-10-10) — these override the proposals above

1. **Cable view**: *Selected* by default; a switch on the desk shows *All* (and *Off*).
2. **The gesture** goes from the source to the destination (press on the source's port,
   release or click on the destination). A plain gesture makes a **send**: every strip
   already reaches the master by its output and keeps sounding there unless it is taken
   off the master. **Ctrl + the gesture replaces the output**: the existing output is
   removed and the destination becomes the strip's only output (sends untouched).
3. **A new send's level**: 0 dB (unity, post-fader — what a drag to a reverb return wants
   to hear at once); its level is on the cable at once, to drag.
4. **Pre/post-fader sends: yes, this round**, clear and easy in the interface: a
   `POST | PRE` switch on every send (the strip's send row, the cable's pill, the
   inspector), a pre send's cable marked *PRE*; mute silences a pre send too. The model is
   built to grow: `SendTap` is an enum (`Post` default, `Pre`), so other tap points can
   be added later without another schema change.
5. **A send to the strip the output already reaches** is allowed (parallel compression
   and the like), but never silent: its cable carries a *doubled signal* warning mark and
   its tooltip/inspector row says that the destination hears this strip twice.
6. **FL's from-selected arrow row stays in Extra large**, beside the strip's own sends.
7. **Extra large shows a one-line chain summary** ("EQ · Comp · +1") that opens the
   inspector, instead of the insert slots.
8. Notes into effects: in the matrix's tracks block (the proposal stands).
9. **Alt+F9 opens the Routing tab**; the route menu's key is the builder's choice among
   free keys, shown in the menus.
10. "Sends only": not needed (the proposal stands).
11. **Drag a track or a channel-rack channel onto a strip** to route it: yes.
12. **New (the owner asked to check it): strips and groups move by dragging.** Today a
    strip moves one place at a time (Alt+←/→) and a group cannot move as one. Add: drag a
    strip (or the selected strips) along the desk to reorder; drag a group by its
    separator to move all its members; Alt+←/→ moves the whole selection or group; one
    undo step each; docks respected.

## 11. ADR 0041 — draft decision text (not created as a file)

> **ADR 0041 — Routing is drawn: cables, a matrix and "route here", over the one graph**
>
> **Status:** Proposed · **Extends:** ADR 0014 (strips), 0015 (sends), 0016 (docks and
> groups), 0017 (a sidechain is a route), 0024 (the kit, §6 mixer, §13 budget)
>
> **Context.** The routing graph — one output per strip, up to sixteen post-fader sends,
> sidechain keys on inserts — has been complete since round 16, but it could only be read
> one strip at a time, from menus and from FL's arrows relative to the selection. The owner
> asked for FL's cables, a matrix, sidechains shown the same way, a direct "route here",
> and an Extra large strip without insert slots.
>
> **Decision.**
> 1. *The model does not change.* A strip keeps one output (unity) and its sends; FL's
>    many-routes-with-levels is not adopted. The output is drawn as the solid cable, a send
>    as the thin one with its level on it, a key as a dashed one in the dynamics hue.
> 2. *Cables live in a band under the strips*, as arcs whose depth follows their span;
>    they never cross a control. The master route is implied except for the selection.
>    Visibility is a desk preference (Off · Selected · All), never project data.
> 3. *Cables are made by dragging* from a strip's port to a strip: plain = send, Ctrl =
>    output, Alt = key; moved by dragging an end; removed from their menu, with Delete, or
>    by dropping an end on nothing. One route menu serves the desk, the matrix and the
>    inspector.
> 4. *A Routing tab draws the same edges as a matrix*, with tracks → strips beneath, the
>    same grammar and keys for every action.
> 5. *"Can this route be made" has one answer*, `Project::route_check`, which every view
>    asks; the commands keep their own checks and a property test holds that they agree.
> 6. *A strip with no feeders offers Route here*; who feeds a strip is
>    `Project::feeders`, which counts sends and keys as well as tracks.
> 7. *Extra large shows level, pan, sends and routing*: the insert slots leave the desk
>    (they are the inspector's), the strip's own sends and its output take their place.
> 8. *Exact undo for sends*: `RemoveStripSend` is inverted by a new `InsertStripSend
>    { index }`, so a removed send comes back where it was.
> 9. *Cost*: edges are derived once per revision; cables are stroked into the desk's one
>    mesh from stack-sampled curves; *All* at 64 strips adds no allocation and no draw call.
>
> **Consequences.** No schema change (pre-fader sends, if taken, are schema 23 with their
> own null tests). `Row::Inserts`, `Desk::most_inserts` and the slot code leave the desk;
> new rows `Cables`, `SendList`, `Output`; a new kit part `cable`; a new view `routing`; a
> new section *Receives* in the inspector. `mixer.fed_by_none` is retired.
>
> **What the tests hold.** §8 of `docs/round25/mixer-routing-design.md`.

---

## 12. Builder-sized pieces

Contract commit first (piece A's types and signatures, the empty locale areas
`locales/areas/{routing,cables,matrix}.{en,es}.txt`, ADR 0041, `docs/round25/BUILDING.md`
with the file owners below). Then B–E in parallel worktrees; F independent of all.

| Piece | Owner of | Delivers | Size |
|---|---|---|---|
| **A — core routing** (first; small) | `cantus-core/src/routing.rs` (new), `command.rs` (`InsertStripSend`, the inverse), `model.rs` (`feeders` re-export), `tests/undo_invariants.rs`, a new `cantus-ui/src/routing_actions.rs` (the batches: move send, convert, new return, route here, key-from incl. a gate; `send_strips_to`'s label) | §7.2, tests 1–5 | 1–2 days |
| **B — cables on the desk** | `kit/cable.rs` (new, with the dashed polyline moved from `fm_graph.rs`), `kit/glyph.rs` (`Key`, `Cables`), `views/mixer_cables.rs` (new: band, ports, arcs, pills, chips, hit test), `mixer_desk.rs` (edges, per-strip index, prefix sums), `mixer.rs` (the fourth pass, the Cables preference), `mixer_paint.rs` (title-row key, legend), `tests/frame_budget.rs` | §3.1–3.2, §6, tests 7, 8, 14, 16 | 3–4 days |
| **C — drag, route here, receives** | `views/mixer_drag.rs` (new: drag state, targets, drop, end moves), `views/mixer_routehere.rs` (new: plate + menu), `mixer_menu.rs` (the unified route menu, *New return ▸*), `views/inspector_receives.rs` (new; one hook line in `inspector_channel.rs`) | §3.3, §4.1–4.4, tests 9–12 | 3–4 days |
| **D — the Routing tab** | `views/routing.rs`, `routing_paint.rs`, `routing_cells.rs` (new), the dock-tab registration, the keymap action | §5, test 13, `-matrix` pictures | 3 days |
| **E — Extra large and the rows** | `mixer_layout.rs` (rows, drop order, tests), `mixer_strip.rs` (paint order, FX count, hooks for B's band and C's plate), `mixer_parts.rs` (slots removed, send list, output chip; the send row's drag hook calls C's `mixer_drag` API), `examples/shoot_mixer.rs` | §4.5, tests 6, 15, every picture of §9 | 2–3 days |
| **F — pre-fader sends** (if accepted) | `cantus-core` `Send::tap`, `cantus-project` (schema 23, migration, fixture), the engine's snapshot/strip/strip_run/PDC feed, `tests/null_tests.rs` / `pdc.rs`, `inspector_sends.rs` (Pre/Post live), DAWproject mapping | §7.3 | 2–3 days |

Hooks between pieces are named in the contract so the hot files have one owner each:
B paints the band from `mixer.rs`; C's plate is a function `mixer_strip.rs` (E) calls from
the meter row; C's drag API (`drag::source(..)`, `drag::target(..)`) is what B's ports and
E's send row call. The integrator merges A first, then B, C, D, E (additive), then F, runs
the suite and clippy, shoots §9's pictures for the owner, and writes STATUS.
