# ADR 0105 — The action pair: Fuga the ninja leads the cast, Volta the mecha beside it

**Status:** accepted (round 37, chars builder) · **Date:** 2026-11-06 · **Extends:** ADR 0087
(the dancer, `Character`), ADR 0090 (the cast: one rig, cel-painted, opaque fills) ·
**Schema:** 59 (58 is an empty step held for another branch; the integrator renumbers) ·
marked line in `cantus-core` (`DEFAULT_CHARACTER`).

## Context

The owner asked (2026-11-06) for a cooler lead character, anime-inspired and original, to
replace the songbird as a new dancer's character; their first ideas were "a robot and a
ninja". Three design rounds followed, each shown to the owner as a model sheet
(turnaround, expressions, key poses, palette, both themes) before any rigging:

1. **v1**, the family's construction with anime touches. Too cute.
2. **v2**, "más cool, menos infantil": anime-teen proportions (about 1:5 head to body),
   almond eyes, sleeker armour. Still read sweet.
3. **v3**, "menos bonito, poco más serio, que se vea perrón": the look of an action-anime
   key visual, not a mascot. Approved. Two fixes were asked before the rig: Fuga's hair
   round the back, and Volta's moods each readable at stage size.

## Decision

### 1. Two characters, Fuga the default

- **Fuga** (`builtin:ninja`) is `DEFAULT_CHARACTER` and first in `cast::builtin()`. Its name
  is music's *fuga* (a fugue), and in Spanish a flight.
  - A near-black gi lit from behind in violet, with a high collar.
  - Gunmetal forearm and shin guards with crimson straps.
  - A fitted mask always over the lower face.
  - Narrow amber almond eyes (a small iris, a slit pupil, one diamond glint, a heavy
    winged lash line) under thick angled brows.
  - Spiky hair swept with the wind, carried round the back as hanging locks and a nape.
  - A deep crimson scarf whose two torn tails stream behind every move.
  - A gold fermata (music's *hold*) on the sash. No blade anywhere.
- **Volta** (`builtin:mecha`) is second. Its name is music's *volta* (the turn, a repeat's
  ending) and the volt it runs on.
  - Gunmetal plates with pearl armour over a graphite frame, coral light in its seams.
  - Exposed pistons at the elbows and knees.
  - An angular helmet with a narrow V visor of light, a sculpted pearl faceplate, and
    blades swept flat back from the temples.
  - A hexagonal speaker core that pulses and sends sound rings on the beat.
  - Back vanes, and jets that fire when it jumps.
- The family of round 35 stays, in its order, after them. Tic, the earlier robot, is kept.
- Names and blurbs are in both locales (`cast.ninja.*`, `cast.mecha.*`).

### 2. Their look: hard cel with a rim light (`cast/edge.rs`)

- **Shading and ink.** A piece is its rim colour, then its shadow drawn a little smaller
  toward the key light, then its lit colour smaller still. That leaves a hard two-tone cel
  and a coloured back-light edge on the far side: violet on Fuga, coral on Volta. The ink
  is thinner (0.016) and heavier underneath, as the family's is.
- **One piece at a time.** A part made of several pieces is shaded piece by piece
  (`hard_each`). Shrinking a whole compound path toward one point slides the light of one
  limb off into the gap between two; the first sheets showed it as a white bar between
  the legs.
- **Shapes.** They are angular polygons and plates (`poly_into`, `slab_into`), not blobs.
- **Rules kept from ADR 0090.** Every fill is opaque, with no gradients. Light (the visor,
  the core, the seams, the jets) is drawn in fixed colours and never turned by the hue.
  There is no cyan or saturated blue: every opaque pixel stays ≥ 60 from a chroma key.
- **Size.** Both are drawn about 1.3 units tall in teen proportions and scaled by 0.82 about
  the feet (`edge::at_scale`), so they stand as tall as the family. The ground under them
  is computed from the canvas as the renderer placed it (`edge::ground`), so the shadow
  and the dust stay on the floor through a lift and a spin.

### 3. Faces

- **Fuga** shows only its eyes and brows; the mask never comes off. Each mood is a lid, a
  lower lid and a brow:
  - determined, a confident smile in the eyes, intent (the eyes narrow, the glint grows);
  - closed, wide, half-shut, one brow raised for *cool*, worried and glancing for *shy*.
- **Volta's** visor slits are a different shape for every mood:
  - keen bars (neutral), a `^` grin (happy);
  - long thick bars rising at the ends with a brighter halo and flares (excited);
  - thin drooping lines on a dim V (calm), short tall blocks (surprised);
  - thin sagging bars (sleepy), a bold scan running the V (cool);
  - short bars glancing aside (shy), one side off (a wink).
  
  The chin's equalizer lights only when it sings (the body's mouth, never a mood's).
- **Neither** has a blush or star eyes.

### 4. Moves (`ninja/moves.rs`, `ninja/draw.rs`, `mecha/moves.rs`)

Both dance all sixteen moves from the family's choreography in their own `Style`, with
these moves of their own:

- **Fuga:**
  - idle is a low ready stance;
  - the kick is high (the foot thrown twice as far and as high, the body leaning away);
  - the jump is an aerial: a tucked flip in the picture's plane, alternating sides, landing
    upright on the beat;
  - the spin is a tornado off the ground;
  - the freeze is a crouched one-knee landing;
  - the bow is martial, fist to palm.
- **Volta:**
  - point, bounce and wave are pop and lock: the phase snaps to each half (or quarter)
    beat and locks, with a pop of the chest on the hit;
  - the jump is a rocket jump, a fist thrown up as the jets fire;
  - the freeze is a hero stance, fists on hips and chin up.
- **Routines and extras.** Each has its own sixteen-bar routine and faces itself (a
  three-quarter view, a real spin). The beat extras are the family's notes, dust and
  sparkles, plus Fuga's crimson petals for confetti, and Volta's core rings, ear-light on
  the accent, and jets.

### 5. Old songs keep their dancer (schema 59)

A dancer that names no character draws the default. To keep a song made before this round
looking as it did, `migrate_58_to_59` writes `builtin:songbird` into every saved dancer that
named none. A dancer that named one keeps it. A dancer saved at 59 without one shows the
default of today. The v49 fixture's dancer names the songbird by name.

Every way a track becomes a dancer names its character, so the rule holds forward too
(review, round 37):
- the picker (`swap::spec_of`, through `dance_live::new_spec`);
- `CantusApp::set_instrument`, which the inspector's instrument menu, a snap and the
  instrument window use (a bare `NativeSpec::new(Dancer)` gets the default there);
- *Swap instrument* keeping matching settings (`carry_over` rebuilds from a bare spec, and
  its result goes through `new_spec`).

## Consequences

- A new dancer's character changes from Trino to Fuga. Every saved song keeps the dancer it
  had.
- The action pair cost more fills than the family (Fuga ~90, Volta ~113 a frame, against
  52–58), but cover less area at the family's height. Measured by `cast_bench` (release, 600
  px stage): Fuga 1.16 ms, Volta 1.24 ms; the songbird read 1.36 ms on the same run. The
  budget is 1.5 ms.
- The stage shows a character as one texture: its triangles on screen do not depend on
  the character.
- Their slim silhouettes cover fewer pixels at a small stage than the round family, so the
  cast test's "draws something" floor is 1 200 pixels at a 160-pixel stage (it was 1 500).
- The model sheets are `cargo run -p cantus-dance --example char_sheet -- <dir> [ninja]
  [mecha]`.

## What the tests hold

- **`cantus-dance/tests/cast.rs`**, over all eight characters:
  - the default is the first and the ninja, the mecha is second, the songbird stays;
  - names and blurbs are in both locales;
  - every move and mood draws, and nothing is drawn outside `bounds()`;
  - no opaque colour is near a chroma key;
  - the drawing is the same at 48 and 2 160 pixels;
  - a blink and a hue change the drawing.
- **`tests/alloc.rs`** and **`tests/determinism.rs`**: no allocation of ours outside a
  fill, no platform trigonometry.
- **`tests/action_pair.rs`:**
  - Volta's light never turns with the hue;
  - every pair of Volta's moods differs at a 300-pixel stage (about 120 pixels tall);
  - the pair draw the same twice.
- **Unit tests:** the aerial turns once round in the air and lands upright, its sides take
  turns, and at low energy it only jumps. Pop and lock meets its cycle and locks between
  hits. The jets fire only in a jump.
- **`cantus-project/tests/schema59.rs`:** an old dancer without a character keeps the
  songbird, one with a character keeps it, a new one without stays without, and the newest
  fixture keeps its dancer.
- **`cantus-ui/tests/dancer_default_e2e.rs`:**
  - a dancer added through the picker in `CantusApp::assemble` names and draws Fuga;
  - a dancer given by the inspector's menu (`set_instrument` with a bare spec), and one
    swapped in with and without *keep matching settings*, each saves `builtin:ninja`, and
    a dancer that names Volta keeps it;
  - an old song opened through the app draws the songbird, and one that named Volta
    draws Volta.
