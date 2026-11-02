# ADR 0090 — The cast: one family drawn on one rig, cel-painted with opaque fills, dancing one choreography in each one's style

**Status:** accepted (round 35, cast builder; first delivery: the rig and the songbird) ·
**Date:** 2026-10-31 · **Extends:** ADR 0087 (the dancer, `Character`, the one renderer and
its 1.5 ms budget), ADR 0047 (licences: `cantus-dance` is GPL) · no schema change · one
marked line in `cantus-core` (`DEFAULT_CHARACTER`) and one in `cantus-dance/src/lib.rs`
(`pub mod rig`)

## Context

Round 35 adds a character that dances on screen (BACKLOG §6i). The contract shipped a stick
figure as a stand-in; the owner saw it and asked for "something super, super professional —
really cool and fun". The cast builder draws Cantus's own characters: original (nothing of
FL's Fruity Dance nor any known mascot), for the whole family, every one of the sixteen
moves and eight moods, readable on every theme and over anything on a transparent stage,
never near a chroma key, crisp from 48 to 2 160 pixels, and within 1.5 ms a frame for one
character on a 600 × 600 stage (release) — on tiny-skia, on the processor, the same pixels
on screen, in the companion window and in a video.

## Decision

### 1. One rig for the family (`cantus-dance/src/rig*.rs`)

- **`rig.rs`** — the maths of motion: easing (smooth, in, out, in-out, back), arcs, pulses
  that wrap round a cycle, lag (follow-through), squash keeping the area, and a **turning
  ball** (`Ball::at(lon, lat, out)`): every head and body of the family is a ball whose
  features sit at longitudes and latitudes, so a yaw turns the face round — a three-quarter
  view for `turn` (the characters draw their own facing) and a real spin where the face goes
  round the back. `ground()` gives the floor under a character (its shadow, dust and
  confetti are drawn there, not lifted or spun), the inverse of the renderer's placement (a
  test holds the two together).
- **`rig_dance.rs`** — the choreography as numbers of a `Body` (offset, lean, squash, yaw,
  nod, tilt, two arms with bend and spread, two feet, a tail and a top that hang on, mouth,
  look, winks, squint), one function per move, from the pose's phase alone (**phase 0 is
  the beat**: a step lands, a clap meets, a kick is out on it; sides alternate by
  `pose.cycle`). A character dances it with its own `Style` (bounce, squash, arms, sway,
  lag, hop, stride) and may replace any move; `Body::lerp` blends from the move it leaves
  (yaw the short way round). The extras the beat adds come out of the same place as events
  at anchors (`Fx::Sparkle`, `Note`, `Dust`, `Confetti` at the mouth, the hands, the feet),
  scaled by `fx_strength` (none at low energy, joyful at full).
- **`rig_draw.rs`** and **`rig_fx.rs`** — shapes and painting (§2), and the extras drawn
  as pure functions of their age and a seed.

### 2. The family's look: cel bands, grown outlines, every fill opaque

Measured on this machine (release, 600-pixel stage, a body-sized ellipse): an **opaque
solid** fill costs 0.045 ms, a **gradient** 0.28–0.37 ms (concentric, focal or linear alike
— the focal one even falls back to tiny-skia's slow f32 pipeline), a **translucent** solid
0.20 ms, a **stroke** about two fills. The first songbird, painted with gradients and
stroked outlines, drew in 3.75 ms. So:

- **Cel shading**: a part is its shade colour, the base drawn smaller toward the key light
  (upper left — a band of shade remains, widest at the bottom right), and, on the big
  round parts, a light patch nearer the light: three opaque fills, the crisp banded look of
  today's vector games and 2D animation. Small parts take one or two bands.
- **Outlines are one fill**: every shape is built by a closure `(builder, grow)`; its
  outline is the shape grown by the ink and nudged down-right — thin where the light falls,
  about twice as heavy underneath — filled before the shape. Lines (a headphone band, a
  lid's edge, a feather line) are **ribbons**: an offset outline along the points, round at
  both ends, one fill.
- No gradients at all: the contact shadow is three nested translucent ovals, a blush two
  opaque ones (a gradient costs several fills and allocates its stops every frame).
- **Winding**: every closed shape of the rig winds the same way (positive area); a part
  made of overlapping sub-paths (a wing and its feathers, a leg and its toes) is filled by
  the non-zero rule, where opposite windings would cancel into holes — a test holds it.
- Fills of the same colour across both eyes are batched into one path per layer.

Result: **1.27 ms** a frame (mean over every move at eight phases, both sides, every mood,
full energy, best of 15 batches; slowest move 1.39 ms), from 3.75 ms painted with
gradients and strokes. Under heavy load from other builds (55–65 % CPU) readings swing up
to 40 % higher. The bench is `cargo run --release -p cantus-dance --example
cast_bench`; it exits with an error over 1.5 ms.

### 3. Colour

A palette per character (base, shade, light, rim; accents), all turned by `hue`
(`Canvas::color`) but the eyes, the ink and glass (`Canvas::fixed`). **No opaque colour
near a chroma key**: measured as the distance in the (Cb, Cr) plane (BT.709), the plane a
keyer works in — every opaque pixel at hue 0, over a spread of moves, moods and energies,
keeps ≥ 60 from green, blue and magenta (neutral greys sit about 75 away; any cyan sits
about 50 from blue and green, which is why the songbird's headphones are berry, not teal).

### 4. Honest bounds, crisp at any scale

`bounds()` includes everything a character draws (its extras too); a test renders every
move at four phases on both sides, at full and low energy, facing and turned, every mood
over several seconds, and finds no pixel outside. The same drawing at 48 and 2 160 pixels
covers the same share within 8 %, and at 2 160 pixels fewer than 2 % of the pixels above
the feet are partly covered (edges stay sharp; the contact shadow is soft on purpose).

### 5. The first of the family: Trino, the songbird (`builtin:songbird`)

Cantus means *song*: a round little bird in warm tangerine with a cream belly, a curling
three-feather crest with red tips, a perky tail, wings with fanning tip feathers for hands,
big glossy eyes with lids (blink, wink, squint, a sleepy droop), a two-part beak that opens
to sing, rosy cheeks, and berry headphones. Its style is bouncy and light (bounce 0.10,
squash 0.16, hop 0.26); its routine is sixteen bars ending each eight with a spin and a
little hop that lands on the downbeat. Moods: neutral, happy (smiling eyes), excited (star
highlights, a sparkle on the beat), calm (closed happy arcs), surprised (wide eyes, crest
up, an open beak), sleepy (droopy lids, crest down, a rising *z*), cool (sunglasses, a
smirk), shy (looking down and aside, a strong blush, a little heart). Extras: a sparkle
where the wings meet on a clap, music notes when it sings (arms up) and on the downbeat, a
dust puff on landing a jump, confetti at full energy, sparkles at the end of a spin.
`DEFAULT_CHARACTER` is its id; it is first in `cast::builtin()` (the picker's order).

### 6. The family (second delivery)

Six characters, one construction, each its own: **Trino** the songbird, **Miso** the cat,
**Lana** the lamb, **Tic** the metronome robot, **Corchea** the eighth note, **Sol** the child — in `cast::builtin()`'s (the picker's) order. The five with
arms and legs share `cast/kit.rs` (torso, rubber-hose limbs through shoulder, elbow and
wrist, mittens, shoes, the contact shadow, extras at a character's anchors) and all six the
face of `rig_face.rs` (moods, eyes with lids, cheeks, sunglasses, a cartoon mouth). Their
heads turn a little before their bodies (the body follows at 60 %).

### 7. Silhouettes and allocations (the coordinator's notes)

- Every move scales its extremes by `vigour²` — subtle at low energy, at full a silhouette
  that tells the move as a thumbnail (squash, lean, hops, the head leading, a deep bow).
- The rig draws through a pooled, transforming pen (shapes are `(sink, grow)` closures),
  fills untransformed, shades in place, draws lines as ribbons: 779 → 227 allocations a
  frame, all of them inside tiny-skia's `fill_path` (3 a fill); `tests/alloc.rs` holds the
  ceiling of three per fill.

### 8. Review 1 (round 35)

- **Deterministic rotations.** tiny-skia's rotation constructors call the platform's sin
  and cos; every rotation in the crate goes through `rig::rotation`/`rotation_at`
  (`cantus_dsp::math::sin_cos` + `Transform::from_row`), the renderer's spin too.
  `tests/determinism.rs` scans the crate's sources and fails on any of tiny-skia's.
- **Zero allocations of ours.** The counting allocator in `tests/alloc.rs` asks the rig
  whether it is inside a fill (`rig::draw::in_fill`) and requires no allocation outside
  one: it found builders lost to empty shapes (an empty pen now returns its builder to the
  pool). What remains is tiny-skia's own, inside its fills (139–205 a frame).
- **Honest bounds per character**, measured (`examples/cast_bounds.rs`) with 0.05 to spare,
  held by a dense test (16 phases a cycle and 0.99, both sides, lift up to 1, spins: each
  pixel inside the bounds as placed, or on the ground).
- **Crisp at any scale, measured**: the edge width (runs of partly covered pixels across
  an edge, 95th percentile) at 48 and 2 160 px; a blurred drawing fails it.
- **LEDs are light**: Tic's screen and lit bulb never turn with the hue (an amber turned
  passes near the green key).
- The pendulum counts the song's absolute beats (it swings through bar lines in any
  metre); cycle arithmetic wraps (no overflow on a song played for days).
- The note is **Corchea** in both languages ("Quaver" is a music-education product).

## Consequences

- Every character of the family is a palette, a style, its construction and its own moves
  where it needs them; the choreography, the painting and the extras are shared, so the
  family reads as one and each new member costs drawing, not plumbing.
- The look is cel-shaded by necessity as much as by taste; a smooth gradient look would
  need a GPU renderer (out of scope: one renderer for screen, companion and video).
- Names and blurbs are the locale files' (`cast.<id>.name`, `cast.<id>.blurb`;
  `cast::name_key`, `cast::blurb_key`); `CharacterInfo::name` keeps the English name.
- The contract's stand-in (`placeholder.rs`) stays only as `default_character()`'s last
  resort, never in the picker.

## What the tests hold

`cantus-dance/tests/cast.rs`: the default is the first and the songbird, ids are unique
lowercase ASCII, every name and blurb is in both locales; every move and mood draws;
nothing outside `bounds()`; no opaque pixel within 60 of a chroma key; the same drawing at
48 and 2 160 pixels; a blink and a hue change the pixels. `rig.rs`: easing ends, the ball,
the ground undoes lift and spin. `rig_dance.rs`: every move meets itself at the end of its
cycle (no pop on the beat), steps land on the beat and kicks are out on it. `rig_draw.rs`:
overlapping sub-paths add up (no holes) and ribbons are round at their ends.
