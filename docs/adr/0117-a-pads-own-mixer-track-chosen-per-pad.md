# ADR 0117 — A pad's own mixer track, chosen per pad from the rack

**Status:** Accepted (round 40, builder *pads*) · **Date:** 2026-11-12 · **Round:** 40 ·
**Schema:** unchanged by this ADR (the model is round 37's; schema 62 is taken by ADR 0118) ·
**Extends:** ADR 0004 (commands), ADR 0100 (Pulsus III: outputs, taps, chains), ADR 0108 /
0111 (the kit) · **Design:** des-views, *Rack* ("Mixer track: a well of 24 × 18, Medium 11 in
ink 2", a well on each pad: "3" under the kit's "3") · **Owner:** round 39's decision 14,
answered 2026-11-12: *"a pad's own mixer track: yes, chosen per pad as in FL Studio (a pad's
mixer track picker, not one forced per pad)"*

## Context

FL Studio's channel rack gives every channel a mixer-track number; changing it routes that
channel alone. Round 39's sheet drew the same well on every pad of a kit; the rack built it on
the kit's row only, and BACKLOG §6j recorded the rest as "a model change and a schema bump".

Looking first (the brief asked for it): **the model already holds it.** Since round 37 (ADR
0100) every pad has `DrumPad::output: Option<StripId>` — `None` the kit's own strip, a strip
an output of its own — saved in the project, routed by the engine as a bus of the kit carried
by a *tap* track (ADR 0100 §2), with `Command::SetPadOutput` and its exact inverse, and an
undo-fuzz arm (round 37's kinds 3 and 4 send pads to random strips). What was missing was the
**way in**: only *Split to strips* (every pad at once, to new strips) and a pad's chain set a
pad's output; no gesture chose one pad's track, and the rack never showed it.

## Decision

1. **No new model, no schema bump.** A pad's mixer track is `DrumPad::output` (through its
   chain when it has one). Schema 62 stays with ADR 0118 (*Record: Before FX · After FX*),
   which does change the model.
2. **`padout::pad_track`** is what a pad's well shows: the strip the pad is heard on — its
   own (`heard_on`, following a chain) or the kit's (`Track::strip`; `M` when the kit plays
   straight into the master).
3. **`padout::set_pad_track(project, track, pad, to)`** is the one command a choice makes
   (`cmd.set_pad_track`, one undo step): `SetPadOutput` for a pad without a chain,
   `SetStripOutput` of its chain for a pad with one (**the chain stays the pad's** and plays
   into the track chosen). `None`, or the kit's own strip named, is the kit (stored as `None`,
   as every pad on it). Refused, before any command is made: not a kit / no such pad; a strip
   that cannot carry a pad (`Refusal::Unfit`: gone, a VCA, a pad's chain — the desk does not
   show one — or a loop through the pad's chain); already there (`Nothing`, quiet); no room in
   the engine (`NoRoom`, round 37's 256 tracks and taps). A **free strip with no name takes
   the pad's name** in the same step (FL names the track it routes a channel to), so the desk
   reads "Open hat", not the kit's name again.
4. **Any audio strip can be a pad's track** — a free one, a bus, another channel's strip (FL
   lets two channels share a mixer track). The pad leaves the kit's strip for it: the kit's
   inserts and fader no longer touch that pad (FL's way). *Split to strips* still routes its
   new strips into the kit's strip (ADR 0100 §4); the well is the one-pad, any-strip gesture
   beside it.
5. **The rack** draws the well on every pad row (des-views' measures, the track's own well
   painter `kit::well::readout`); a click opens a kit menu: *The kit's strip (n)* (the
   default, radio), *First free strip (n)* when the desk has one, then every strip that can
   carry a pad as `n · name`, scrolling inside the window on a long desk. A strip left empty
   by a pad moved off it is kept (with its inserts), as FL keeps an insert.

## Consequences

- `routing::is_fed` counts a kit's pad playing on a strip (`Track::feeds_through_bus`): a
  pad's own strip — the well's, or *Split to strips*' since round 37 — read as "Nothing feeds
  this strip — route here" in the inspector's Receives and as unfed on the desk.

- **Null when unset, to the bit:** nothing changes for a pad on the kit — the engine path is
  ADR 0100's, untouched; a pad sent back (by its inverse, or by *the kit's strip*) renders the
  song as it was to the bit (`pad_track.rs::a_pad_sent_back_renders_the_song_as_it_was_to_the_bit`).
- Every old song loads unchanged: no field was added.
- The kit's track routed elsewhere carries its pad-made strips (round 37's
  `with_kit_strips_following`) but **not** a strip the user chose for a pad — a user's strip
  is the user's routing.

## What the tests hold

- `cantus-core` (`padout_track_tests.rs`): every pad shows the kit's track until one is
  chosen; to a free track and back, one step each, undo and redo exact; a pad sharing another
  channel's strip (`tracks_on_strip` sees the kit); a chained pad keeps its chain and the
  chain plays into the track chosen; every refusal named.
- Undo fuzz (`undo_invariants.rs`, `// r40:pads`): pads sent to random strips, back, chains
  made, among the general generator's commands; a choice built on a strip the bus never had
  refused whole; undo all / redo all exact.
- `cantus-engine/tests/pad_track.rs`: a pad sent to its own track **sounds there and only
  there** — that strip's stem is the pad alone, the kit's strip's stem the kit without it, both
  to the bit, for three pads; block-size independence at tolerance 0 (1 … 4096); sent back, the
  song to the bit; a pad sharing another channel's strip follows that strip's fader.
- `cantus-ui/tests/pad_track_e2e.rs`, through `CantusApp::assemble`: the rack in front, a
  click on the kick's well and on *First free strip (n)*; the well reads the new number; the
  stems through the engine's compile as above, to the bit; saved and read back; one undo.

## Amendment — the mid-round review (round 40, builder `fix-b`)

- **Merge back keeps a strip the user chose** (review B-2). `padout::merge` removed every strip
  a pad was heard on that nothing else used — a strip given to a pad from its well, with the
  user's inserts on it, among them, against §5. It now removes only strips **made for pads**
  (`pad_made`: *Split to strips*' and chains); a user's strip stays with its inserts and the pad
  goes back on the kit.
- **A user's strip belongs to no kit** (review B-3). `padout::kit_of_strip` matched any strip a
  pad plays on, so the strip menu offered the kit's *Split / Merge* on a user's bus and *Bounce*
  refused it as "a pad's strip". It now answers only for strips made for pads; a user's strip
  bounces as any strip does, the pad in it (`bounce::strip_plan` counts a kit whose pad plays
  into the strip among the tracks that reach it).
- **The well lists each strip once** (review C-8): the kit's own strip only as *The kit's strip
  (n)*, the first free one only as *First free strip (n)*.
- Tested: `padout_track_tests::merge_back_keeps_a_strip_the_user_chose_and_it_belongs_to_no_kit`
  and, through `CantusApp::assemble`, `pad_user_strip_e2e.rs`: the well's list unique, no kit
  on the user's strip, its *Bounce* (the song afterwards bit-identical), *Merge back* keeping
  the strip and its insert.

## Amendment — review 2 (round 40, builder `fix-d`)

- **Merge back undoes *Split to strips*, and only that** (review 2, R2-B3). After the amendment
  above, *Merge back* still took a pad off a strip the user chose for it (the well) and put it
  back on the kit — the user's routing reset by a command that never made it. Decided: a pad on
  a strip the user chose is the user's routing, and *Merge back* leaves it there.
  `padout::split_onto` is where a pad is heard when *Split to strips* put it there (`heard_on` a
  strip `pad_made`); `can_merge` and `merge` go by it, so *Merge back* is offered only when there
  is something split to merge, and it removes only those strips. A render's copy
  (`merge_in_place`, a bounce of the kit's track) still takes every pad back, the user's strips
  included: the bounce holds the whole kit (ADR 0100 §9). `is_split` keeps its meaning (some pad
  heard off the kit's strip), which freezing and the DAWproject writer need. Asked to merge a kit
  whose pads play only on strips the user chose, the status line says so and how to bring a pad
  back (`padout.merge_users`: choose *The kit's strip* in its mixer track).
- Tested: `padout_track_tests::merge_back_keeps_the_users_routing_and_a_strip_the_user_chose_belongs_to_no_kit`
  (nothing to merge on the user's strips; after a split, *Merge back* removes the strips it made
  and leaves the kick and the snare where the user put them; one undo step),
  `merge_in_place_for_a_render_takes_every_pad_back`, and end to end `pad_user_strip_e2e.rs`,
  which now opens the desk's own strip menu with a right click on the strip's head and clicks
  its rows (review 2, R2-B10): no *Split / Merge* on the user's strip; the kit's strip offers
  *Merge back*, which changes nothing and, asked, says why; *Split to strips* from that menu,
  then *Merge back* from a strip it made: those strips gone, the kick still on the user's strip
  with its insert, the song to the bit as before.
