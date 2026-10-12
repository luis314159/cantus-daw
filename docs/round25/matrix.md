# Round 25 — builder *matrix*: the Routing tab (D) and pre-fader sends (F)

**Branch:** `r25/matrix` (base `57f3ccc`), worktree `F:/projects/daw-r25/matrix`.
Read with `docs/round25/mixer-routing-design.md` (§5, §7.3, §10), ADR 0041 and
**ADR 0045** (this piece's decisions).

## What was done

### F — pre-fader sends in the engine (null tests first)

- `StripSnapshot::sends` is `(strip index, level, SendTap)`; the strip slot keeps the tap
  per send and whether any is pre (`processor_snap.rs`).
- A **pre-fader send** takes the strip's pair after polarity, swap, the inserts, the EQ
  and width — before the fader and the pan law — at its level: undelayed in the
  consumer's gather (`graph/strip.rs`, `In::Send`), delayed through its compensation line
  (`graph/strip_run.rs`). **Mute and solo silence it** (the line is fed silence, so it
  drains). Without a fader ride the pre-fader pair *is* the post-chain pair (the fader is
  still in `gl`/`gr`), so nothing is copied and a **post-fader send is the same
  arithmetic, bit for bit**. Under a fader or pan ride (which scales the pair in place) a
  strip with a pre send keeps the pair before the ramp in its scratch and hands it over
  by a swap (`StripOut::pre_l/pre_r`, 8 KiB per strip slot, allocated with it). No
  allocation on the audio thread.
- No golden changed, no `DSP_VERSION` bump. `SendTap` stays extensible: every `match` is
  exhaustive, the switch draws one segment per `SendTap::ALL`.

### D — the Routing tab

- `views/routing.rs` (state, layout, input, keys, hints, the key menu),
  `routing_cells.rs` (the grid, derived once per revision and filter),
  `routing_paint.rs` (cells, frozen headers, band, corner) — one brush per area, visible
  cells only.
- **Strips → strips** (sources down in desk order, the master column first, dock
  dividers, names on their side over number chips in the strips' colours), **tracks →
  strips** under a *Tracks* band (a radio; a diamond where an insert hears the track's
  notes — answer 8).
- **Cells**: well; hatched = a send would close a loop; faded = no room; diagonal = the
  strip itself; dot = output (in the source's colour); ring with a level arc = send (the
  ring in the pre-fader hue with a mark in its foot when PRE, faded when off, the
  doubled-signal mark when the destination hears the row twice); dot in ring = both; a
  dynamics-hued corner and the key mark = keyed inserts.
- **Grammar** (the desk's): click = send at 0 dB post; Ctrl+click = the only output (on
  a send: make it the output); Alt+click = key (one insert at once, several in a menu,
  none: *Add a ducking compressor / a gate keyed from …*); master column = output; a
  refused cell issues nothing and says why (status bar, and the hint before the click).
  Send cells: drag = level (200 pt, Shift fine), double-click = 0 dB, right-click = the
  route menu (`views::route_menu`, owner `routing-matrix-menu`), **Ctrl+drag along a
  column copies the send** (Shift+drag at first; moved to Ctrl at integration so Shift is
  the fine drag, as on every knob) (level, switch, tap) — one step, `cmd.copy_send`. Track rows:
  click = route the track. Row/column headers: click selects the strip (or the track).
- **Keys** while the body has focus: arrows, Home/End, Enter (send on/off), Ctrl+Enter
  (output), Alt+Enter (key), + / − (±1 dB, Shift 0.1), Delete/Backspace (remove), Esc
  (leave). Everything else the keymap fires is passed on (Space, Ctrl+Z, F9…).
- **All · In use** in the toolbar, *In use* by default above 32 strips: rows = strips that
  route somewhere other than the master or are fed; columns = strips something feeds;
  "n hidden" readout. The hint bar names the cell's route and what a click does with the
  keys held (or the refusal, from `route_check`, naming the strip that closes a loop).
- **Opened with Alt+F9** (`Action::ViewRouting`, every preset; keymap file version 5
  fills it into older files), *View ▸ Routing*; a lower-zone tab (`Pane::Routing`,
  glyph *Send*).
- **The inspector's `POST | PRE` is live** (`kit::tap::switch`, issues
  `cmd.set_send_tap`): one segment per tap, lit in the tap's hue (post: the sends' hue,
  pre: the modulation hue), explaining itself on hover; widened to 84 pt so both words
  read whole.

## Decisions (ADR 0045)

1. The pre-fader pair is the post-chain pair unless a ride scaled it; kept apart only
   then, only for strips with a pre send.
2. The matrix's loop/no-room refusals come from per-strip reachability bitsets, proved
   equal to `route_check` on random graphs; the hint asks `route_check` for the hovered
   cell.
3. The focused matrix runs the keymap's other actions itself; only pointer clicks act on
   cells.
4. **The design's "pre = post to the bit at 0 dB / centre / 0 dB law" cannot hold in
   `f32`**: the 0 dB law's centre is `cos(π/4)·√2 = 0.99999994`. The test holds the exact
   relation (a pre send with the fader's gain folded into its level equals the post send
   to the bit, at centre, for three laws) and pins the law's value. Making the law exactly
   unity at centre is a separate DSP change for the owner (it would move goldens).
5. The column copy is a batch of `SetStripSend` built in the view (no new command; the
   batch is a known shape — the fuzz already covers `SetStripSend`).

## Tests

Seen failing first (re-checked at integration: the builder's `send_tap.rs` and `pdc.rs`
as committed in `38e1790`, run against the engine before it, `38e1790^`): **four** failed
there — `a_pre_send_does_not_follow_its_fader_or_its_pan`,
`at_a_0_db_fader_centre_pan_and_the_0_db_law_pre_is_post_to_the_bit`,
`a_pre_send_renders_the_same_at_every_block_size` (send_tap.rs) and
`a_pre_send_through_a_compensation_line_lines_up_and_keeps_its_level` (pdc.rs). Four
passed on the base and so were never seen failing: `a_post_send_renders_exactly_as_it_did_before_the_tap`
(a null test — it must pass on the base), `mute_and_solo_silence_a_pre_send`,
`a_pre_send_renders_the_same_in_real_time_as_offline` and
`a_pre_send_into_a_latent_return_lines_up_as_a_post_one_does`. The integration added
tests that do fail under the mutations that matter (a pre send hearing a ride through its
compensation line; a pre send ignoring solo) — see `docs/round25/integration.md`. The
golden was computed on the base and pinned.

- `cantus-engine/tests/send_tap.rs` (6, new), `pdc.rs` (+2), `rt_memory.rs` (+1),
  `pool.rs` (random sessions now randomise the tap), `graph_order.rs` (tuple), core
  `snapshot` test (the tap travels).
- `cantus-ui`: `routing_cells` unit tests (3), `kit::tap` (1), `tests/routing_matrix.rs`
  (10, new).
- Totals on this branch: `cargo test -p cantus-engine -p cantus-core -p cantus-ui
  -p cantus-project --no-fail-fast` — **1 374 passed, 0 failed, 24 ignored** (piece A's
  base: 1 351); `CANTUS_WORKERS=3 cargo test -p cantus-engine --features fuzz` — **405
  passed, 0 failed, 6 ignored**; `cargo clippy --workspace --all-targets -- -D warnings`
  clean.

## Pictures

`cargo run --release -p cantus-ui --example shoot_matrix -- <dir> [scale] [en|es]`
(`CANTUS_SHOOT_KINDS=matrix,matrix-inuse,matrix-64,insp-tap`, `CANTUS_SHOOT_SIZE=w,h` for a
close look). Looked at in Slate and Paper, English and Spanish, 75 %, 100 % and 200 %:
fixed on the way — the inspector's `PO…` (switch widened), pre sends hard to tell from
post (mark in the ring's foot), prose in the hint bar's figure font (prose moved to the
name, the level stays the value), *In use* showing destination columns nothing fed, the
ground around the grid.

## What is left

- The key cell's glyph and the doubled-signal mark are drawn by `kit::cable::key_mark` /
  `doubled_mark` — the desk's stubs until its branch lands (the dynamics corner shows a
  key meanwhile).
- The route menu (right-click) and its add-send list are the interaction builder's stubs
  until that branch lands.
- No name filter in the toolbar yet (design §5 mentions one); no matrix frame-budget test.
- The desk's *Matrix* key in the title row is the desk builder's (it can call
  `app.toggle_pane(Pane::Routing)`).
- `inspector.post_only` (inspector area) and `kit.inspector.pre/post` (kit area) are no
  longer used; their owners may retire them.

## Files outside the listed ones (additive)

- `views/transport.rs`: the View menu lists *Routing* with its key (two lines).
- `cantus-ui/tests/app_flows.rs`: one assertion on `StripSnapshot::sends` takes the tap.
- `cantus-core/src/snapshot.rs` (the `sends` field, owned by this piece per BUILDING),
  `cantus-engine/src/processor.rs` (an import), `processor_snap.rs`, `graph/plan.rs`,
  `graph/plan_compile.rs` (the tuple).
