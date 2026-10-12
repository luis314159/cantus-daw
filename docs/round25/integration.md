# Round 25 — integration: the merge, the reviewers' findings, the fixes

**Branch:** `r25/integration` (worktree `F:/projects/daw-r23/beta-hardening`). Base of
this report: `1226d3b` — the three builders merged (`r25/interaction` 90d1879,
`r25/matrix` d36b201, `r25/desk` 58fd29e, in that order, onto piece A's contract) plus
the integrator's Delete-on-cable fix. The look/icon builder (`F:/projects/daw-r25/look`)
works on themes and the app icon in parallel; nothing here touches `theme.rs`, the kit's
tokens or appearance, the logo or the window icon.

## The merge

The three branches merged without conflicts beyond the additive hooks the contract
declared (`drag::source` / `target` / `finish`, `column_hatch`, `kit::tap`). One
integration fix came with it: the keymap's Delete removed the piano roll's selected notes
when a cable was selected on the focused desk (`1226d3b`, test in `mixer_desk.rs`).

Then three adversarial reviewers read the merged branch. Every finding was checked
against the code; **all 27 were real** (and one more turned up while fixing them, below).
For each: a failing test first where practical, then the fix. "Seen failing" below means
the test was run against the code before the fix (or under the named mutation) and
failed.

## The owner's requirement not built: click mode (answer 2)

The owner's answer 2 says the gesture goes from the source to the destination by "press
on the source's port and release **or click** on the destination". Only the drag was
built. Now (`80d9536`, ADR 0044 §10, ADR 0041 §5, `interaction.md`):

- A click (a press with no travel) on a strip's **port** or the selected strip's **▼
  chip** arms a route from that strip: the port gets a ring and a lit core in the
  selection's colour, the chip a lit outline.
- While armed, every column answers as for a carried cable — refused columns hatched, the
  hovered one outlined, the rubber band follows the pointer, the chip beside the pointer
  says what a click does ("Send to 5 · 0 dB, post-fader"), the hint bar names the
  modifiers ("Click: send · Ctrl: output · Alt: key · Esc: cancel", short enough to read whole in the hint bar; the drag's is "Drop: …" likewise), the cursor is a crosshair (not-allowed over a refused
  column).
- The **second click** on a strip makes the route with the modifiers held *at that
  click* (plain = send, Ctrl = only output, Alt = key) — the same `route_gesture`, one
  undo step. On a refused strip it stays armed; on empty space, its own strip, outside
  the desk, a right-click or Esc it is let go.
- A **cover** over the desk, registered after every control while armed, takes the
  completing click, so the fader, mute or name under it never acts.
- The ▼ chip's destination list moved to its right-click (the output chip's click still
  opens it). The matrix needs no click mode: a cell already is one click.
- Found while testing it: a lit cable's end mark lay over the port beside it, so a press
  on that port moved the cable instead of starting one. The end mark's hit area now sits
  just under the port when they would overlap (first trimmed, then moved under the port
  when the entry-point tests showed the trimmed mark vanishing as lanes lifted the
  cable).
- Tests (`tests/mixer_routing.rs`, real ports, real frames):
  `a_click_on_a_port_then_on_a_strip_sends`, `ctrl_at_the_second_click_routes_the_output`,
  `an_armed_route_is_let_go_by_esc_empty_space_or_its_own_strip`,
  `the_selected_strips_chip_arms_a_route_too`.

## Model / engine

| # | Finding | Verdict | What was done |
|---|---|---|---|
| 1 | `move_strips`: a selection mixing a loose strip and a group member could split the group (`[9(G),2,10(G),11]`) | real | The block is ordered so the strips staying in the left neighbour's group come first and those in the right neighbour's last, other groups kept together (`5ef4aaa`). `strip_order.rs`: `a_mixed_selection_never_splits_the_group_it_lands_beside` (seen failing) and the property search `no_move_splits_a_group_by_accident` (3 000 desks; strip, selection and group moves and nudges; seen failing). |
| 2 | `move_route` (send, either end) and `send_to_output` orphaned automation riding `AutoTarget::Send` | real | `move_route` retargets every clip riding the send in the same batch, one `SetClipAutomation` per link group (siblings follow, inverse exact) (`15fa90f`). `send_to_output`: an output has no level to ride, so the clip is left waiting, unchanged, and rides again when the output is made a send (as after removing a send) — documented and tested rather than retargeted to something it does not mean. `routing.rs`: `a_moved_send_takes_its_automation_along` (seen failing), `make_this_the_output_then_a_send_again_gives_the_automation_back`; the undo fuzz now automates sends. |
| 3 | `spot_at`: a separator mapped to the group's first member in project order | real | It maps to the first member of *its own run in its dock* not carried by the drag (`1953e72`). `mixer_routing.rs`: `a_drop_before_a_separator_lands_before_that_runs_first_member` (seen failing: the drop did nothing). |

## UX in the real app

| # | Finding | Verdict | What was done |
|---|---|---|---|
| 4 | `POST | PRE` unlabelled on the Extra large send rows and the selected cable | real (pictures: two blank segments) | `kit::tap::chip`: one chip sized to its word — `PRE` lit in the modulation hue, `POST` dim in an outline — a click cycles `SendTap::ALL`, ← → when focused (`3c0e548`). On the send rows and beside the selected pill (placed, clipped and kept off other pills with it; the pill's own PRE tag dropped there, it would say it twice). The pill's PRE tag is in the label role and the pre-fader hue: readable at 75 % (it was not). `shoot_mixer` gained `-xl-75`, `-xl-200`, `-cables-pick(-75, -es)`. |
| 5 | Delete removed a cable the musician could not see | real | The band lets the selection go when the view is Off or the cable is not drawn; the keymap's Delete asks `MixerView::deletable_cable` — drawn on the last pass, so never on a desk not on screen (`8a8abd9`). `mixer_desk.rs`: `delete_never_removes_a_cable_that_is_not_drawn` (seen failing on the hidden-desk step; the Off step was already covered by the band row disappearing). |
| 6 | *Show the destination* from the matrix waited unseen; old requests acted late | real, and wider | A view drawn inside a dock tab toggled a checked-out placeholder workspace, so **every** pane request from inside a tab was lost — the matrix's *Show the destination*, Extra large's chain line opening the inspector (owner answer 7!), the channel rack's instrument-less channel. `CantusApp::show_pane` queues the request, applied once the dock is back (`cef7334`). `reveal()` drops requests older than the flash. Tests: `show_the_destination_from_the_matrix_brings_the_desk_forward` (window-level, seen failing), `the_chain_line_opens_a_hidden_inspector` (seen failing), `a_stale_reveal_request_is_dropped`. |
| 7 | Matrix focus killed the typing piano (stuck notes), skipped paste and bare M/S | real | `CantusApp::dispatch_keys` — keymap actions (minus the keys a focused view keeps), the bare M / S rule, Ctrl+V's paste, the typing piano — runs once a pass; the keymap and the matrix both call it (`b866a0f`). `routing_matrix.rs`: `the_typing_piano_plays_and_releases_while_the_matrix_has_the_focus` (seen failing). |
| 8 | Master's pan law a bare "-3" under OUTPUT | real | "−3 dB law" ("ley −3 dB"), its short form "−3 dB" where only that fits (`1ad5fb0`). `the_masters_pan_law_is_labelled` (en, es); `the_master_carries_the_pan_law` updated. |
| 9 | A track drag showed the cable gesture's hint | real | Its own words: "Drop on a strip: the track plays into it · Esc: cancel" (en + es) (`1ad5fb0`); asserted in `a_track_dropped_on_a_strip_routes_it`. |
| 10 | The arrangement drew a drop line while a track was held over the mixer | real | Outside the headers and lanes the target is the track's own row (`e1d6b0f`); held by finding 26's test (seen failing under the mutation). |
| 11 | Esc during a desk drag also ran *Clear selection*; the matrix's own Esc was dead | real | `MixerView::claims_escape` (a drag, a click-armed route, a selected cable) keeps *Clear selection* out (`e57f7aa`); the matrix adds `escape` to its focus filter. `esc_on_the_desk_cancels_its_own_things_and_clears_nothing_else` (seen failing under the mutation). |
| 12 | Shift+F10 hard-coded | real | `Action::RouteMenu` (`route-menu`, *Mixer*), keymap file version 6 with `NEW_IN_V6`, acting with the mixer focused; the strip menu and the Edit menu show the bound key (`4207d86`). Tests: `a_version_5_file_gets_the_route_menu_key` (every preset, no conflicts), the window test opening it only with the mixer focused and showing a rebound key. ADR 0044 §7. |
| 13 | `kit::tap::switch` not clipped to the dock | real | The chip senses only `rect ∩ pass.clip` (`3c0e548`). |
| 14 | FL's from-selected arrow ignored the multi-selection and failed silently | real | Through `route_gesture` (the whole selection, Ctrl / Alt, one step); a refusal is said in the hint bar (`9d3b603`). `the_from_selected_arrow_acts_for_the_selection_and_says_a_refusal`. |
| 15 | Spanish `cables.view.off = No` | real | "Ocultos" (`9d3b603`). |
| 16 | Receives' key row cryptic and middle-elided | real | "Comp, slot 1 ← key" with the chain line's short names, end elision (`0c36648`). `a_receives_key_row_names_the_insert_as_the_chain_line_does`. |
| 17 | Matrix: Shift both fine level and copy | real | Ctrl+drag copies down a column; Shift is the fine drag, from the start or mid-drag (`1788c8b`); hint (en, es), `matrix.md`, ADR 0045. `shift_drag_on_a_send_is_the_fine_level`, `ctrl_drag_down_a_column_copies_the_send`. |

## Tests that did not prove enough

| # | Finding | What was added |
|---|---|---|
| 18 | No test pressed the real port / end mark / ▼ chip / "+" row | `tests/route_entry_points.rs` (`4d32906`): a cable dragged from a real port (0 dB, output untouched, `cmd.add_send`); a real end mark moving a route (level and tap kept, `cmd.move_route`); the "+" row; `PRE` from each entry point — the send row's chip, the selected pill's chip, the inspector's switch, the route menu's radio (`cmd.set_send_tap`). The ▼ chip's click is held by the click-mode test. |
| 19 | Delayed pre send under a ride untested | `pdc.rs`: `a_delayed_pre_send_under_a_fader_and_pan_ride_does_not_hear_them` (`ffe2db9`) — the send edge itself waits on a compensation line; nulls at tolerance 0 against the un-ridden render; fails under `SendTap::Pre => (l, r, *level, *level)` (max diff 0.296). Found on the way: the old `a_pre_send_through_a_compensation_line…` put its line on the return's output, not the send edge — its comment now says so (`99aae57`). |
| 20 | `route_menu::open` rows untested | `route_entry_points.rs`: a send's menu (Level ▸ −6 dB, Send on, Pre, Post, Select routed here / its source, Make this the output, Remove), an output's (Make this a send, Show the destination, Remove (back to the master)), a key's (Open the keyed effect, Remove) — each the project change and the undo label. |
| 21 | `send_tap.rs`'s solo half could not fail | `a_solo_elsewhere_silences_a_pre_send_into_a_destination_it_keeps` (`130b36d`): X feeds the aux and is soloed, so the aux stays audible; the lead's pre send into it must be silent (null at tolerance 0 against the same solo without the send, the reference not silent, the send audible without the solo). Fails when pre sends ignore mute/solo. The vacuous half was removed with a pointer. |
| 22 | The `Key(None)` oracle used `plan::key_with` (which calls `route_check`) | `routing.rs` (`bf2d520`): the oracle applies raw `InsertEffect` + `SetEffectSidechain` to a clone and checks the result acyclic (compressor and gate must agree); crowded graphs (a strip with 16 sends, a host with 10 inserts, a host whose only keyable insert is keyed already) exercise `NoRoom` and `AlreadyThere`, asserted seen. Fails under "`key_anywhere` returns Ok when full", and under "refuse NoRoom when there is room" — which the old oracle let pass. |
| 23 | `frame_budget.rs` compared medians, Compact only | `the_band_in_wide_with_every_look_costs_no_allocation` (`1536334`): Wide, pre / off / doubled sends, pills, edge chips, a hovered cable; **worst frame**: at rest *All* allocates no more than *Off* (20 = 20 with the pointer reported every frame); a hovered cable ≤ 1 more than a hovered fader (37 against 36 — hovering any control costs the hint bar ~17); fastest build within 1.6 × *Off* + 2 ms (measured 1.24 ms against 0.96 ms). |
| 24 | No-overlap test did not look at words | `the_band_reads_without_overlaps_at_every_scale_and_language` now asserts no "…" in any word painted in the band (pills, PRE tags, edge chips, tap chips), the send rows' tap word whole and no name under its chip, en/es × 75/200 % × Wide/XL, non-vacuous (`97f501a`). |
| 25 | Cable geometry untested | `cables::tests::no_two_lit_cables_share_a_flat_run` (every order; nested hang above; faint keep theirs; fails when lanes are skipped) and `a_cable_with_both_ends_off_on_one_side_draws_nothing` (`04da05f`; the explicit check and the coincident-chip guard both hold it — the test fails when both go). |
| 26 | No whole-window header → strip test | `an_arrangement_header_dropped_on_a_strip_routes_it_and_keeps_the_order` (`e1d6b0f`): routed, no drop line while held over the desk, order unchanged. |
| 27 | `matrix.md` overstated "seen failing first" | Re-run against `38e1790^`: four of the builder's engine tests failed there, four passed; `matrix.md` names them (`8966318`). |

## Found while fixing (not in the reviewers' list)

- **Panes asked from inside a tab were lost** (see 6): the chain line's "open the
  inspector" (owner answer 7) never worked from a hidden inspector, nor the channel
  rack's instrument-less channel. `CantusApp::show_pane`.
- **A press on a port could move a cable** whose end mark lay over it (see click mode).
- **A desk not on screen took Esc** once a cable had been selected on it (the first cut of
  11): `claims_escape` counts only a desk drawn on the last pass; tested.
- **The hint bar cut the drag's own words** before its modifiers ("Drag to a strip: send ·
  Ctrl: only output · Alt…"): now "Drop: / Click: send · Ctrl: output · Alt: key · Esc:
  cancel" (en, es), read whole in the pictures.
- The old `pdc.rs` test said its pre send went through a send compensation line; the
  planner put the line on the return (comment corrected; the new ridden test covers the
  send line).
- Two clippy findings in tests (`strip_order.rs`, `mixer_desk.rs`).

## Totals

- `cargo test --workspace --no-fail-fast`: **2 135 passed, 0 failed, 34 ignored**, 207 test
  binaries (the binaries named `*setup*` / `*update*` ran; UAC did not refuse them on this
  run).
- `CANTUS_WORKERS=3 cargo test -p cantus-engine --features fuzz`: **407 passed, 0 failed,
  6 ignored**.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- rustfmt only on the files changed whose base was formatted (all were).

## Pictures

Looked at, whole window, both themes, English and Spanish, 75 % and 200 %:

- `cargo run --release -p cantus-ui --example shoot_mixer -- <dir> 1920x1080-16-cables
  1920x1080-16-xl 1280x720-16-cables-es` — 36 pictures; new variants `-xl-75`, `-xl-200`,
  `-cables-pick`, `-cables-pick-75`, `-cables-pick-es` (the selected pre send: pill and
  chip). Seen: `POST` / `PRE` read on every Extra large send row (they were two blank
  segments), the pill's PRE tag reads at 75 % (it did not), the selected pill no longer
  says PRE twice, the master reads "−3 dB law" / "ley −3 dB", Spanish *Ocultos*.
- `shoot_matrix` at 0.75 / 1 / 2 × en / es (48 pictures): unchanged in look; the copy hint
  says Ctrl+drag.
- `CANTUS_SHOTS=<dir> CANTUS_SHOTS_FILTER=armed|drag-send cargo test --release -p
  cantus-ui --lib drag::shots -- --ignored`: the drag pictures now press the **real**
  ports; the new `routing-armed` scene shows click mode (the port ringed, the ▼ chip
  outlined, columns answering, the rubber band, "Send to 6 · Strings · 0 dB,
  post-fader").

Pictures are written to `F:/projects/daw-r23/shots-r25/final/` (not in the repository).

## What is left

- **Click mode while the desk is off screen**: an armed route survives the desk being
  hidden and is still armed when it comes back (its ring shows it; Esc or a click lets
  it go). Letting it go on hide was tried and dropped — it also dropped a selected cable
  the musician expected to keep; a cleaner signal ("the desk was hidden") would allow it.
- **A typed level in the route menu** (*Level…* as a field) — still steps only
  (`interaction.md`).
- **The inspector's own Add send** could open `route_menu::open_add_send` for *New return
  ▸* (`interaction.md`).
- **A cable's lanes move when the pointer lights more cables** (hovering a strip lights
  its routes and the lanes are re-shared), so an end mark can shift under an approaching
  pointer. The tests find the mark where it is drawn under the pointer; a steadier rule
  (lanes from the selection only, hover not re-laning) is a design choice for the desk.
- **Hovering any desk control costs ~17 allocations a frame** (the hint bar), measured by
  the new budget test; the band adds at most one. Worth a look in the performance wave.
- STATUS.md is the integrator's to write at the end of the round (header, test count,
  "The next session").
