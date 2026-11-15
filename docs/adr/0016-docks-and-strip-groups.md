# ADR 0016 — Docks and strip groups: how the desk is organised

**Status:** Accepted · **Date:** 2026-09-23 · **Extends:** ADR 0014 (mixer strips), ADR 0015 (sends, solo, submixes)

## Context

The user asked for "a separator in the mixer, with about three channels by default that
can be pinned so they are always in view, and more groups that can be made and coloured".
FL answers the first half with three **docks** — left, middle, right — where the left and
right ones do not scroll, and its right dock is where returns and buses usually live. It
answers the second half with separators and "auto colour group".

The question to settle first was whether a group is a *signal* thing or a *view* thing,
because ADR 0014 and 0015 already say that the only grouping that changes what is heard
is a bus (a submix), and that a second grouping must not compete with it.

## Decisions

**A dock is a property of the strip, stored in the project.** `MixerStrip::dock` is
`Left`, `Middle` (the default) or `Right`. Left and right are pinned; the middle scrolls
and takes the width the pinned docks leave. It is stored in the project for the reason a
track's height is: it changes nothing that renders, but a desk laid out by hand has to
travel with the file.

**A new project pins its last three strips on the right.** That is FL's layout and the
user's ask. A new track still takes the first free strip, but the middle is searched
first, so an instrument does not land on a return while a channel is free. An old project
opens exactly as it was laid out (migration 10 → 11 writes nothing): pinning strips in a
file the user already arranged would move things they placed.

**A group is how the desk is organised, not how the signal flows.** `StripGroup` has a
name, a colour and a *collapsed* flag; `MixerStrip::group` points at one. A group is drawn
as a separator in its colour, with its name running up it, the member count and a
chevron; clicking it folds the members away. Nothing about a group reaches the snapshot.
If the members should also be *heard* together, that is a submix (ADR 0015), and the two
compose: a group can hold a bus and its inputs.

**Members are gathered.** Making a group, or adding strips to one, moves the members next
to each other in desk order, right after the first of them, and into its dock — one run
behind one separator. A group whose members were scattered across the desk would need a
separator per run and would not fold as one thing. The moves are `MoveStrip`s inside the
same `Batch`, so one undo puts every strip back where it was.

**Density and meter-wave are preferences, not project data.** Compact / normal / wide
(FL's layout sizes, reduced to the three that differ in *what* is shown) and the
meter-wave view live in `settings.json`: the same desk is compact on a laptop and wide on
a studio screen. Compact drops the width bar, the EQ bars and the send rows (the sends
become a count); wide gives every bar more travel.

## Consequences

- Commands: `SetStripDock`, `SetStripGroup`, `InsertStripGroup`, `RemoveStripGroup`
  (refused while a strip is in the group; the app dissolves in one batch),
  `SetStripGroupName`, `SetStripGroupColor`, `SetStripGroupCollapsed`. All in the undo fuzz.
- The strip menu gained *Dock to*, *Group* (new from selection, join, leave) and *Route
  "track" here* — FL's *route selected channels to this track*, where the channel rack's
  selection is the selected track. `Alt+W` toggles the meter-wave view.
- A strip without a colour of its own or a track feeding it wears its group's colour.
- Schema 11; fixture `v11.cantus` has a left-docked return and a collapsed group.

## Amendment — round 40 phase 3: a group is a band (the owner, 2026-11-12)

The owner approved the designer's groups sheets (`design-p3/groups/`): a user group is drawn
the way an instrument's family is (ADR 0123, design item 4), so both read as one system.

- **A band, not a separator.** A group is a band over its strips from its first to its last:
  a thin line in its colour along the top, its name, its member count at its end, a chevron.
  A 12-pt gap with a seam sits before and after every top-level band (the right dock's own
  device), so the group reads as one block and its end is as visible as its start. The
  members keep their own stripes. The band carries what the separator did: a click folds,
  a double-click renames, a right-click opens the group's menu, a drag moves the group, a
  drop on it is a drop on the group.
- **Folding summarises, never hides.** A folded group is one strip: the members' numbers and
  the group's name in its head, the members named, their meters side by side, *Unfold*.
  `SetStripGroupCollapsed` is unchanged (the song's state, one undo step).
- **The edge takes in and gives back.** Dragging the band's right edge past the next strip's
  middle takes it in; back past the last member's middle gives that one back. A dashed ghost
  and a label (*Add Guitar to Voces*) say which; the release is one undo step; Esc cancels.
- **Docked right**, a group keeps its band and folds the same.
- **One band a strip** (with ADR 0123's families): a strip is in at most one family; a group
  either sits inside one family (every member from it — drawn as a head under the family's
  band, folding on its own) or outside families, holding a family whole (a strip of a family
  taken into it brings the whole family; folding the group folds the family inside it). One
  nesting level: family ▸ group.
- Code: `views/mixer_group.rs` (the bands, `lay`), `mixer_group_paint.rs`; the desk's
  `Item::Gap`; `CantusApp::create_strip_group` and `add_strips_to_group` apply the rule
  (`mixer_group::whole_families`). Tests: `tests/mixer_bands_e2e.rs` through
  `CantusApp::assemble`.
