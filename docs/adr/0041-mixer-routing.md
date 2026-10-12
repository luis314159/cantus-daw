# ADR 0041 — Routing is drawn: cables, a matrix and "route here", over the one graph; pre-fader sends

**Status:** Accepted · **Date:** 2026-10-10 · **Round:** 25 · **Extends:** ADR 0014
(strips), 0015 (sends), 0016 (docks and groups), 0017 (a sidechain is a route), 0024 (the
kit, §6 the mixer, §13 the budget) · **Schema:** 23

The design and its research are `docs/round25/mixer-routing-design.md` (§10 holds the
owner's answers of 2026-10-10, which this ADR follows where they differ from the
proposal) and `research/mixer-routing/notes.md`.

## Context

The routing graph — one output per strip, up to sixteen sends, sidechain keys on inserts
— has been complete since round 16, but it could only be read one strip at a time, from
menus and from FL's arrows relative to the selection. Every demo's Reverb and Delay said
"Nothing feeds this strip yet": the hint counted tracks only, never the sends that fed
them. The owner asked for FL's cables, a matrix, sidechains shown the same way, a direct
"route here", an Extra large strip without insert slots, pre-fader sends, and strips and
groups that move by dragging.

## Decision

1. **The model keeps its shape.** A strip keeps one output (unity) and its sends; FL's
   many-routes-with-levels is not adopted. The output is drawn as the solid cable, a send
   as the thinner one with its level on it, a key as a dashed one in the dynamics hue.
2. **Sends have a tap** — `Send::tap: SendTap { Post (default), Pre }`, schema 23 (owner's
   answer 4). Pre-fader = after the inserts, the EQ, width and swap; before the fader and
   the pan law; mute and solo still silence it. `SendTap` is an enum **built to grow**:
   a new tap point is a new variant handled in every `match` (no wildcard arms), with a
   schema bump so an older build refuses a file that uses it; `Post` is never written, so
   a project without a pre-fader send is the file it was. `SetStripSend` carries the whole
   `Send`: no new command. DAWproject's send `type` maps both ways (`pre` / `post`).
   Until the round's engine piece lands, the snapshot renders every send post-fader
   (documented on `StripSnapshot::sends`); a post-fader send renders bit-identically to
   before the tap existed.
3. **One place reads the graph** — `cantus_core::routing`:
   - `Project::route_check(from, to, kind) -> Result<(), RouteRefusal>` is the one answer
     to "can this route be made?" (kinds `Output`, `Send`, `Key(Option<EffectId>)`;
     refusals: unknown strip or insert, from the master, a send to the master, itself, a
     loop naming the strip that closes it, no room, not keyable, already there). The
     desk's hatching, the matrix's cells, the menus' disabled rows and the hints all ask
     it; the commands keep their own checks (scripts and the MCP server issue them) and a
     property test on random graphs holds that the two agree — `AlreadyThere` is the one
     refusal under which the command still applies, as a change of the route.
   - `feeders(project, id)` — tracks, outputs, sends (with level, switch and tap), keys
     and note routes — is who feeds a strip, for every reader; `is_fed` decides the
     route-here plate without allocating. The old track-only hint is retired.
   - `doubled(project, strip, to)`: a sounding send to a strip the output already reaches
     (directly or downstream). **Allowed** (parallel compression), but never silent: its
     cable carries a warning mark and its hint and inspector row say the destination hears
     the strip twice (answer 5).
   - `edges(project)` — every output, send and key once, in a stable order — is what the
     cables and the matrix draw, derived once per revision.
4. **Every gesture is one batch from one builder.** `routing::plan` builds, from the
   project alone, the batches the views issue — add sends, replace the output, key (an
   insert, or a new ducking compressor or gate), move a route's end (a send keeps its
   place, level, switch and tap), remove a route, convert send ↔ output, a new return
   with its reverb or delay and the sends into it, tracks routed onto a strip, a send's
   tap — and `routing::order` the ones that move strips and groups. Each is one
   `Command::Batch` with its own `cmd.*` label, one undo step; the undo fuzz runs the same
   builders. `cantus-ui`'s `routing_actions` allocates the ids and is what every view
   calls.
5. **The gestures** (answers 2, 3): from the source to the destination — press on the
   source's port, release or click on the destination. **Plain = a send** at 0 dB,
   post-fader (every strip already reaches the master by its output and keeps sounding
   there). **Ctrl = the destination becomes the strip's only output** (the old output is
   gone; sends untouched — never FL's "route to this track only" deleting sends). **Alt =
   a key**: one keyable insert is keyed at once, several open a menu, none offer a ducking
   compressor or a gate keyed from the source. A drop on the master routes the output
   there. From a multi-selection every selected strip sends or routes; a key is the
   dragged strip's alone (an insert has one key). A track or a channel-rack channel
   dropped on a strip routes it (answer 11). **Click mode** (integration): a click on a
   port or the ▼ chip arms the route, the next click on a strip makes it with the
   modifiers held at that click; Esc or a click elsewhere lets it go (ADR 0044 §10).
6. **Cables live in a band under the strips**, as arcs whose depth follows their span;
   they never cross a control. The master route is implied except for the selection.
   Visibility is a desk preference (*Off · Selected* (default) *· All*, answer 1), never
   project data. Cables are moved by dragging an end and removed from their menu, with
   Delete, or by dropping an end on nothing. **One route menu** serves the desk, the
   matrix and the inspector, with a `POST | PRE` switch on every send (answer 4).
7. **A Routing tab draws the same edges as a matrix** (strips → strips, tracks → strips
   beneath, notes into effects in the tracks block — answer 8), the same grammar and keys
   for every action, opened with **Alt+F9** (answer 9).
8. **A strip with no feeders offers Route here** in its silent meter well; the inspector
   grows a *Receives* section.
9. **Extra large shows level, pan, sends and routing**: the insert slots leave the desk
   (they are the inspector's); the strip's own send rows, its output chip and a one-line
   chain summary ("EQ · Comp · +1") that opens the inspector take their place (answer 7);
   FL's from-selected arrow row stays (answer 6).
10. **Strips and groups move** (answer 12): a strip or the selection dragged along the
    desk, a group dragged by its separator as one block, Alt+←/→ moving the selection —
    or the group, when the selection is a whole group — one undo step each. Docks are
    respected (a drag names the dock it lands in; a nudge never leaves its dock) and a
    group is never split by accident: a strip dropped between two members joins the
    group, a member dragged away from every other member leaves it, a nudge hops a loose
    strip over a whole group and steps a member at its group's edge out of the group, a
    group dropped inside another group's run lands before it.
11. **Exact undo for sends**: `RemoveStripSend` is inverted by `InsertStripSend { index }`
    (round 24), so a removed send comes back where it was, tap and all.
12. **Cost**: edges and the fed flags are derived once per revision; cables are stroked
    into the desk's one mesh from stack-sampled curves; *All* at 64 strips adds no
    allocation and no draw call (`tests/frame_budget.rs`).

## Consequences

- Schema 23 (a migration that writes nothing; `fixtures/v23.cantus` holds a pre-fader
  send; every older fixture opens with every send post-fader). DAWproject import no longer
  reports pre-fader sends as a loss (`interop.report.d_pre_sends` is retired).
- New core module `routing` (with `routing::plan` and `routing::order`); new UI module
  `routing_actions`; new kit parts `cable` and `tap`; new views `mixer_cables`,
  `mixer_drag`, `mixer_routehere`, `route_menu`, `routing`, `inspector_receives`; new
  locale areas `routing`, `cables`, `matrix`. `Row::Inserts`, `Desk::most_inserts` and the
  slot code leave the desk. `mixer.fed_by` / `mixer.fed_by_none` are retired.
- `send_strips_to` / `route_strips_to` now go through the builders (their old batch was
  mislabelled `cmd.route_strips`); `duck_strip_from` builds its batch with
  `routing::plan::key_with`; the strip menu's and Shift+wheel's single-strip move follow
  the nudge's rules (docks respected).
- Round 25 is built by four pieces: A (this contract: core routing, the batches, the
  schema), then the desk (cables, Extra large), the interaction (drag, route here, the
  route menu, Receives, strip and group dragging) and the matrix (the Routing tab and the
  pre-fader engine and inspector) in parallel — `docs/round25/BUILDING.md`.

## What the tests hold

- `cantus-core/tests/routing.rs`: `route_check` agrees with `SetStripOutput`,
  `SetStripSend`, `InsertStripSend` and `SetEffectSidechain` for every (from, to, kind) on
  random graphs (a mutation that drops the send's loop check fails it); a loop names the
  strip that closes it; feeders count sends, keys and notes (a return fed only by sends is
  fed — the owner's bug pinned); `doubled` through a bus and not for a switched-off send;
  edges once each in order; every `plan` batch applies, is labelled, and undoes and redoes
  exactly; a removed pre-fader send comes back in place.
- `cantus-core/tests/strip_order.rs`: moves to a spot and into another dock, blocks keep
  their order, the group rules above, group moves and the snap out of another group's
  run, nudges at dock edges, hops over groups and steps out of them.
- `cantus-core/tests/undo_invariants.rs`: the send arms randomise the tap; arms for every
  `plan` and `order` batch, with a coverage check that each really applies.
- `cantus-project`: every older fixture's sends open post-fader; the v23 fixture's
  pre-fader send survives a round trip and is the only `tap` written; DAWproject carries
  `pre` both ways.
- `cantus-ui` `routing_actions`: the gestures from a selection, Ctrl, Alt's three cases,
  a refused gesture issues nothing, a new return is one step and selected, tracks dropped
  on a strip, the tap switch, Alt+←/→ through the keymap, the feeders hover.
- The desk, interaction and matrix pieces add theirs (design §8, tests 6–16).
