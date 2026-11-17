# ADR 0150 — Route the selected to a new bus, in one step

> **Provisional number** (branch `fix/flicker`): renumber at the merge if 0150 is taken.

**Status:** Accepted (the owner approved building it, 2026-11-17) · **Date:** 2026-11-17 ·
**Builds on:** ADR 0041 (routing gestures are pure batches in `cantus_core::routing::plan`),
ADR 0016 (groups and bands on the desk), ADR 0113 (folders in the arrangement), ADR 0004
(every mutation a command with an exact inverse) · **Amends:** the round-25 *Create submix
from selected* (`CantusApp::create_submix`)

## Context

The owner asked (after GitHub issue #1's follow-up) for a way to *group tracks and route
them to a bus* in one step: Ctrl+click several strips or tracks, then one action. What
Cantus offered before this decision:

| Path | Steps (after the strips exist) |
|---|---|
| Mixer: click, Ctrl+click / Shift+click strips, right-click one → *Create submix from selected* | select (1 + n−1 clicks) → right-click → click: **2 actions after the selection** |
| Mixer: make a strip (*Add strip*), select the sources, right-click the new one → *Route selected strips here* | 4–5 actions + the selection |
| Mixer: *Where it goes…* (Shift+F10) → *Route the selected strips here (n)* on an existing strip | 2 + the selection |
| Drag a track (header or channel) onto a strip | one drag per track |
| *New group from selected* (a band on the desk, optional group VCA) | groups the faders; **no audio bus** |
| Arrangement folders (ADR 0113) | visual only; **no audio bus** |

So the mixer already had a two-action submix, hidden in the strip menu's Routing band with
no key, named "submix", always to the master, with no way at all from the arrangement: a
track header selected one track only, and its menu had no routing.

How others do it:

- **FL Studio** (the reference): no one-step bus. Select the source inserts, select (or
  first create) an empty insert, then its routing knob's right-click → *Route to this track
  only*, or *Channel routing ▸ Route selected channels to this track*; channels get there
  with **Ctrl+L** (*link selected channels to this track*) from the Channel Rack. Three to
  five actions, and an empty insert must exist.
- **Studio One**: select channels → right-click → *Add Bus for Selected Channels*: one
  action, the bus named *Bus 1, 2, …* next to them.
- **Ableton Live / Bitwig**: select tracks → **Ctrl+G** *Group Tracks*: a group track that
  is both a folder and a summing bus, one key.
- **Logic**: *Create Track Stack* (⌘⇧D) → a summing stack; **Pro Tools**: Option+Shift on
  an output selector → *New Track…* applies to all selected.
- **REAPER**: *Make folder from selected tracks* (a folder is a bus), or the routing matrix.

## Decision

1. **One action routes the selection to a new bus**: *Route the selected to a new bus*,
   in the strip's right-click menu (where *Create submix from selected* was, its words
   now saying what happens), in a track header's menu, in the keymap as
   `route-to-new-bus` (**Ctrl+G**, as Live and Bitwig group — free in every preset; keymap
   version 18 fills it in for older maps) and so in the command palette. The selection is
   what the user made with Ctrl+click and Shift+click: the strips on the desk, or the
   tracks in the arrangement — whichever area is being edited; a single selected strip or
   track is a selection of one. The owner's idea is the best of the designs above: it is
   Studio One's one action and Live's one key, without FL's empty insert first.
2. **The track headers select several**: Ctrl+click adds or removes a track, Shift+click
   takes the run from the selected one (`CantusApp::track_selection`, UI state only, as the
   desk's strip selection is); a plain click selects one again.
3. **Where the bus goes and what it is** (`cantus_core::routing::plan::new_bus`, a pure
   batch labelled `cmd.new_bus`, one step of undo): right after the last selected strip, in
   their dock when they share one and in their group when they share one; its output is the
   output they all share (a nested bus stays nested), else the master; its colour borrows
   the first feeding track's (the model's rule for a strip with none); its name *Bus 1*,
   *Bus 2*, … — the first number no strip has — or *‹group› Bus* when they share a group.
   Each selected strip's output goes to the bus; a selected track's strip does (once, if
   several tracks share it); a track that feeds the master with no strip of its own feeds
   the bus. Sends are left as they are; a VCA (no audio) and the master are never sources.
   The new bus is selected.
4. **No model change**: the batch is made of `InsertStrip`, `SetStripOutput` and
   `SetTrackStrip` — no schema bump. The undo fuzz gets an arm calling `plan::new_bus`.
5. **Not now:** folding the selected tracks into an arrangement folder with the bus (Live's
   group is both). Folders stay visual (ADR 0113); if the owner wants Live's behaviour, the
   same action can add `ArrangeCommand::SetTrackFolder` to the batch.

## Consequences

- Grouping to a bus is the selection plus **one action** (Ctrl+G, or two clicks through a
  menu), from the mixer or the arrangement; FL needs three to five and an empty insert.
- `CantusApp::create_submix` stays as the same action for the strips it is given.
- The strings `mixer.menu.submix`, `mixer.submix_name` and `cmd.create_submix` give way to
  `routing.menu.new_bus`, `routing.new_bus_name`, `routing.new_bus_group_name` and
  `cmd.new_bus`.

## What the tests hold

- `cantus-core`'s `routing_plan` tests: the bus's place, dock, group, output (shared,
  mixed, the master), the tracks with and without strips, a strip shared by two tracks
  routed once, VCAs left out, nothing to do gives `None`; the undo fuzz arm.
- `tests/bus_from_selection_e2e.rs`, through `CantusApp::assemble` with the real engine:
  strips selected by click and Ctrl+click on their heads, Ctrl+G, one new strip after them
  fed by them, heard through it (its mute silences them; the master's output is the same
  to the bit as before), one undo gives the project back to the bit; the same from the
  track headers with Ctrl+click and the header menu's row; at 100 % and 200 %.
