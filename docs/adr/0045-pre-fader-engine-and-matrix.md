# ADR 0045 — Pre-fader sends in the engine; the Routing matrix's one answer and its keys

**Status:** Accepted · **Date:** 2026-10-10 · **Round:** 25 (builder *matrix*, pieces D
and F) · **Extends:** ADR 0041 (routing drawn, `SendTap`), 0031 (the graph engine), 0005
(real time), 0024 (the kit) · **Schema:** none (piece A's 23)

## Context

ADR 0041 put `Send::tap: SendTap { Post, Pre }` in the model and left the engine rendering
every send post-fader. A pre-fader send taps after the inserts, the EQ, width and swap,
before the fader and the pan law; mute and solo silence it (the owner's answer 4). The
engine's strip leaves its consumers the **post-chain pair** with the fader still in its
gains (`gl`, `gr`), so an undelayed post send is `pair × gl × level` — except under a
**fader or pan ride** (ADR 0027), which scales the pair itself in place, sample by sample.

The Routing tab draws the graph as a matrix (design §5). "Can this route be made?" has
one answer, `Project::route_check`; asking it per cell walks the graph per cell (256
strips: 65 000 walks per revision). And a focused egui widget switches the application's
whole keymap off (`egui_wants_keyboard_input`), Space included.

## Decision

1. **The snapshot carries the tap**: `StripSnapshot::sends: Vec<(usize, f32, SendTap)>`;
   the strip slot keeps `send_tap` per send and `has_pre`. Every `match` on the tap is
   exhaustive (a new tap point is a new arm, as ADR 0041 requires).
2. **Pre-fader is the same pair with the fader left out**: an undelayed pre send is
   `pair × level`; a delayed one feeds its compensation line `pair × level` (silence when
   muted, so the line drains). Without a ride the pre-fader pair *is* the post-chain pair:
   no copy, no new buffer touched — so a post send's arithmetic is the expression it was,
   and a post-fader send renders **bit-identically** (no golden change, no `DSP_VERSION`
   bump).
3. **Under a ride the pre-fader pair is kept apart**, only when the strip has a pre send:
   copied into the slot's scratch `dry` pair before the ramp, handed to the consumers by a
   swap with the strip's own `pre_l`/`pre_r` (allocated with the slot: 8 KiB per slot,
   2 MiB for 256 slots). Nothing is allocated on the audio thread.
4. **The matrix derives refusals from reachability**, once per revision: one bitset per
   strip of what it reaches through outputs, sends and keys; a send `r → c` is refused as
   a loop when `c` reaches `r`, as no room when `r` has sixteen sends. A unit test holds
   that this agrees with `route_check` on every cell of random graphs; the hint asks
   `route_check` itself for the one cell under the pointer (which names the strip that
   closes the loop).
5. **The matrix holds the focus and passes the keymap on**: while its body has the focus
   it takes its own keys (arrows, Home, End, Enter with Ctrl/Alt, + / −, Delete,
   Backspace, Esc) and runs every other action the keymap fires (Space, Ctrl+Z, the view
   keys) itself — the application's dispatch is off while any widget holds the focus.
   Only the pointer's clicks act on a cell (`clicked_by(Primary)`): egui's Space/Enter
   "click" on a focused widget is not a click on whatever cell the pointer is over.
6. **The tap switch is the kit's** (`kit::tap`): a segmented control with one segment per
   `SendTap::ALL`, lit in the tap's hue (`kit::tap::function`: the sends' hue for post,
   the modulation hue for pre), explaining itself on hover.

## Consequences

- `StripSnapshot::sends`' type changed (core); the engine's graph test models the tuple.
- At a 0 dB fader, centre pan and the 0 dB law a pre send is **not** bit-equal to a post
  one: in `f32` the law's centre is `cos(π/4)·√2 = 0.99999994`, one step under unity. The
  test holds the exact relation instead — at centre, a pre send whose level carries the
  fader's gain equals the post send to the bit, for every law — and pins the law's value.
  Making the 0 dB law exactly unity at centre is a separate DSP change (a `DSP_VERSION`
  bump) left to the owner.
- Keymap file version 5 (`view-routing`, Alt+F9, filled into older files where free).

## What the tests hold

- `cantus-engine/tests/send_tap.rs`: post sends (undelayed, through lines, under a ride,
  muted, off) render as on the contract commit (a pinned hash); a pre send is unchanged by
  fader and pan moves and rides; pre = post to the bit at centre with the fader in the
  level (three laws); block sizes 7, 1, 64, 1000 against 512 at tolerance 0; real time
  equals offline; mute, solo and a muted ride silence a pre send. The whole engine suite
  under `CANTUS_WORKERS=3 --features fuzz`; `pool.rs` randomises the tap.
- `tests/pdc.rs`: a pre send into a latent return lines up as a post one does; a pre send
  through a compensation line lines up, keeps its level when the fader moves, and is
  silenced by mute.
- `tests/rt_memory.rs`: pre sends (undelayed, delayed, under a ride, switched, muted) never
  allocate on the audio thread.
- `cantus-ui`: `routing_cells` (refusals agree with `route_check`; cells mirror the edges;
  *In use*; the tracks radio and notes), `tests/routing_matrix.rs` (the click / Ctrl / Alt
  grammar, the duck / gate offer, a loop refused with its reason, the keys and the keymap
  passed on, level drag and double-click, Ctrl+drag copy as one step (Shift is the fine level drag), the tracks block,
  *In use* above 32 strips, Alt+F9 in every preset, the tap switch).
