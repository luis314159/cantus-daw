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
