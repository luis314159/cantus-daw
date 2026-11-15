# D · Bajo, the speaker: what the rig draws today, what it would need

**Files:**
- `bajo-sheet.png` / `.html`
- `bajo-moves-night.gif`, `bajo-moves-paper.gif`
- `bajo-window-night.gif`, `bajo-window-paper.gif`
- Design source: `src/bajo.py`

## What it is

A studio monitor with a heavy, calm groove.

- **The cabinet** (0.46 × 0.80, rim, lip, baffle) is its body and head. It is rigid and turns
  about the pelvis.
- **Two tweeters are its eyes:** dark rings with gold domes for pupils. **Lids** of the baffle
  make the moods; its neutral is a heavy, unimpressed half-lid.
- **The woofer is its chest** and pumps with the level: the cone and the gold dust cap grow
  with `energy` and the beat.
- **The bass port is its belt.**
- **Limbs:** short thick legs on isolation pads; short arms with big mitts.
- **Colour:** graphite in both themes, held at 3:1 on Night by the knob's rim `#76818D`.
- **Moves:**
  - *the heavy nod*: rock side to side, squash on the beat;
  - *the stomp*: steps lifted high and landed hard, with a squash and dust;
  - signature, **Drop the bass**: it winds up (crouched, mitts raised, eyes shut), holds,
    then the woofer blasts three gold rings while the cabinet jolts up and shakes.

## What `cantus-dance` draws today

| Bajo needs | Today |
|---|---|
| Tweeters, woofer, dust cap, mitts | `ellipse_into`, flat fills |
| Legs and arms | `ribbon_into` (or `kit::hose_into` straightened) |
| Squash, dust on landing | `Body::squash`, `rig_fx::dust` |
| Rings | an even-odd ring (`fixed_even_odd`) per ring: an opaque ring whose width shrinks (the design's opacity fade becomes a width fade) |

## What it would need

1. **`rrect_into`** for the cabinet, baffle, port and pads (shared).
2. **A rigid torso part.** The cabinet is drawn in the torso's frame. `kit::torso(body,
   anchor_y)` already gives that transform.
3. **Two-bone IK** for the knees: short legs take the weight, so the pelvis drops (shared).
4. **The level** for the woofer, from `Pose::energy`, as Tono's dots.
5. **Its own moves:**
   - *Bounce* → heavy;
   - *Step left/right* → stomp;
   - *Jump* → Drop the bass.

   The shake is a pure function of the phase (a sine at a fixed rate times an envelope), so
   it stays deterministic.
6. **Cost:** about 35 fills, or about 40 with the rings.
7. **Tests:**
   - the rings stay inside `bounds()` (the bounds must include their largest radius, about
     0.73);
   - the moods differ;
   - the rim at 3:1;
   - the gold never turns with the hue.

## Risk

A box with legs can read as a prop. Its charm is the weight of its motion and the blast:
judge the GIFs.
