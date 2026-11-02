# Round 35 — cast: the characters Cantus includes (ADR 0090)

Builder: **cast** (branch `r35/cast`). This file grows with each delivery.

## First delivery — the rig and Trino, the songbird

### What was done

- **The rig** (`crates/cantus-dance/src/rig.rs`, `rig_draw.rs`, `rig_dance.rs`,
  `rig_fx.rs`), shared by the whole family:
  - motion maths (easing, arcs, wrapping pulses, lag, squash keeping the area), the
    **turning ball** (pseudo-3D: features at longitudes and latitudes, so a yaw gives a
    three-quarter view for `turn` and a spin where the face goes round the back), and the
    **ground** under a character (shadow, dust, confetti; the inverse of the renderer's
    lift and spin, tested against `render::placement`);
  - the **choreography** of all sixteen moves as a `Body` (offset, lean, squash, yaw,
    nod, tilt, arms with bend and spread, feet, tail, top, mouth, look, winks, squint)
    from the phase alone, phase 0 on the beat, sides by `cycle`, a character's `Style`,
    `Body::lerp` for blends; the **beat extras** as events at anchors;
  - **painting**: cel bands (shade, base toward the light, a light patch), outlines as one
    fill of the shape grown by the ink (thin on the lit side, heavier below), ribbons for
    lines, a rim arc on round parts, soft glows only for small things; every shape wound
    the same way so compound parts never get holes;
  - **extras**: sparkles, music notes (an eighth or two beamed sixteenths), dust puffs,
    confetti, a sleepy *z*, a heart — pure functions of their age and a seed.
- **Trino, the songbird** (`crates/cantus-dance/src/cast/songbird/`): every move, every
  mood, its routine (sixteen bars, each eight ending with a spin and a hop that lands on
  the downbeat), the extras, honest bounds, its own facing. `DEFAULT_CHARACTER` is
  `builtin:songbird` (marked line); it is first in `cast::builtin()`.
- **Names** in both locales (`cast.songbird.name` = Trino, `cast.songbird.blurb`) and
  `cast::name_key` / `cast::blurb_key` for the picker.
- **Looking at it**: `cantus-dance/examples/preview.rs` grown to every character (one
  folder each); `cantus-dance/examples/cast_sheet.rs` (sheets, and `one` for a single pose
  large); `cantus-ui/examples/shoot_cast.rs` (the same sheets on every shipped theme's
  stage); `cantus-dance/examples/cast_bench.rs` (the budget); both share
  `cantus-dance/examples/common/sheet.rs`.

### Draw, look, redraw (passes on the songbird)

1. **Pass 1** — gradients and stroked outlines, teal headphones. Looked: cute but small
   pupils (a worried look), wings like carrots (ridged leaves with red tips), the happy
   lower lid a pale band, a tiny white note hard to see on the checker, the motion too
   subtle at the size it is seen.
2. **Pass 2** — bigger glossy pupils, the smiling lower lid in the body's colour, rounder
   wings with three fanning tip feathers (hands that clap, wave and point), a bigger
   curling crest, stronger cheeks, gold notes; shoulders lowered and wings 14 % longer
   (the clap covered the face, arms-up read as hands over the ears); arms-up became a "V";
   amplitudes up (bounce 0.07 → 0.10, squash 0.13 → 0.16, sway, stride); the two marks on
   the belly removed (they read as a second face).
3. **Pass 3** — the bench said 3.75 ms (budget 1.5): repainted in **cel bands with grown
   outlines**, every fill opaque (§ numbers). Looked again: holes along the wings' edges —
   opposite windings cancelling — fixed in the rig with a test; the spin's back view was a
   blob with the tail sticking up like a tongue: the tail now hangs down when it faces us
   and the bird has a darker **mantle** on its back; in three-quarter view the belly poked
   out of the body — belly and mantle are now decals pulled inside the silhouette; the
   clap's sparkle drifted to the belly as the wings opened — it now stays where they met;
   the right ear cup's shading read as a blush beside the eye — softened. Last, for margin
   under load: the cheeks became cel blushes (two opaque ovals instead of a gradient), the
   legs and crest one band — the same look, 1.43 → 1.27 ms.
4. **Colour** — the teal headphones measured ~50 from the blue and green keys in the
   chroma plane (any cyan does); they are **berry** now (≥ 99), and the confetti dropped
   teal.

Images: `docs/round35/cast/songbird-passes.png` (pass 1 → pass 3 → now, the same pose),
`songbird-8-bars.gif` (the routine over eight bars at 120 BPM, 12 fps, 210 px, on
Cantus Night; the 24 fps, 360 px one is outside the repository), `cast-songbird-moves.png`
(every move at four phases and the other side), `cast-songbird-themes.png` (every mood on
the checker and on all twelve themes' stages), `cast-songbird-moods.png`,
`cast-songbird-sizes.png` (48, 96, 480 px).

### How it was tested — the numbers

- `cargo test -p cantus-dance`: 12 unit tests (rig, choreography, winding, pose, render)
  and 6 in `tests/cast.rs` — default and ids, both locales, every move and mood draws,
  **no pixel outside `bounds()`** over 608 poses, **no opaque pixel within 60 (CbCr) of a
  chroma key**, **48 px and 2 160 px cover the same share within 8 %** and < 2 % soft
  pixels at 2 160, a blink and a hue change the pixels.
- **Bench** (`cargo run --release -p cantus-dance --example cast_bench`, 600 × 600, size
  1, every move × 8 phases × both sides × every mood, full energy, best of 15): **1.27 ms
  mean**, slowest move arms-up 1.39 ms (from 3.75 ms at pass 1). The machine is shared with
  four other builders: the same binary read 1.27–1.28 twice in a row, but earlier versions
  swung by up to 40 % (to 1.6–2.1 ms) while the CPU sat at 55–65 % load.
- What a fill costs (release, 600-pixel stage, a 190 × 200-pixel ellipse): opaque solid
  0.045 ms, gradient 0.28–0.37 ms, translucent solid 0.20 ms, a 6-pixel stroke 0.09 ms;
  a 30-pixel circle 0.007 ms. This is why the family is cel-painted (ADR 0090 §2).
- `cargo clippy -p cantus-dance --all-targets` with `-D warnings`: clean.

### Marked lines (in files I do not own)

- `crates/cantus-core/src/inst/dancer.rs`: `pub const DEFAULT_CHARACTER: &str =
  "builtin:songbird"; // r35:cast`
- `crates/cantus-dance/src/lib.rs`: `pub mod rig; // r35:cast`

### Ready to merge

Yes: the songbird replaces the stand-in as the default and first character; others
resolve it through `cast::default_character()` / `cast::by_id("builtin:songbird")`. The
stand-in's id is now `builtin:placeholder` (kept only as the last resort).

### For the owner

- Your eyes on the GIF and the sheets: is Trino the songbird you want first?

## Second delivery — Trino's next pass, the allocations, and the rest of the family

### The coordinator's three notes on Trino

1. **Wings that read as wings.** Each wing is now coverts (the arm of the wing) whose outer
   side is a smooth **leading edge**, four overlapping **primaries** fanning from the wrist
   (the outermost longest, carrying the edge on to the tip), thin dark lines between them,
   and a **lighter underside** with a lit line along the leading edge. They fan wide on
   clap, wave and arms up (`spread` 1). Looked: the first try (narrow, separated primaries)
   read as gloved fingers; broader, overlapping feathers with smaller fan steps read as a
   bird's wing at 64 px (`docs/round35/cast/cast-songbird-moves.png`).
2. **Silhouettes that tell the moves apart as thumbnails.** Every move has an *extreme*
   factor `x = vigour²` (≈ 0.12 at low energy, 1 at full): deeper squash and stretch (the
   jump lands as a pancake and flies tall), real hops on the standing foot in kick and on
   every half beat in shuffle, a clear lean on step, sway, point and twist (5–20°), the
   head leading (the ball turns toward the step, the point, the sway before the weight
   arrives), a deep bow (the face going down 64° with the body folding), arms up as a wide
   "V". At low energy all of it stays subtle.
3. **Allocations.** The rig draws through a **pen** that maps points by the canvas's
   transform into path builders from a per-thread pool, fills them untransformed (with a
   transform tiny-skia clones the path first), transforms a part's cel bands *in place*,
   and draws every line as a filled ribbon (no strokes); extras go in a fixed array; the
   two wings are one part through a mapping sink. **779 → 227 allocations a frame**, all of
   them tiny-skia's own (a bare `fill_path` of a ready path allocates 3 inside: its edges
   and coverage runs — removing those would mean patching tiny-skia). `tests/alloc.rs`
   counts it for every character with a counting allocator (ceiling 300: three per fill).

### The family

| | Id | Name (en / es) | Who | Style |
|---|---|---|---|---|
| 1 | `songbird` | Trino | tangerine songbird, berry headphones | bouncy, light, sings on the downbeat |
| 2 | `cat` | Miso | smoky grey tabby, gold scarf, amber eyes | slinky: sways, twists, ends a phrase striking a pose |
| 3 | `lamb` | Lana | cream wool, cocoa face, pink bow, gold bell | hoppy: bounces, skips, jumps for joy |
| 4 | `robot` | Tic | ivory metronome robot, LED face, pendulum | precise: pops, points, robot shuffles |
| 5 | `note` | Quaver / Corchea | plum eighth note, gold bow tie, white gloves | jazzy showman: claps, points, kicks, bows |
| 6 | `child` | Sol | curly-haired kid, star T-shirt | all energy: claps, jumps, spins, hands up |

All share the rig (`cast/kit.rs` for the five with arms and legs: torso, rubber-hose limbs
through shoulder–elbow–wrist, mittens, shoes, the shadow, extras at their own anchors) and
the shared face (`rig_face.rs`: every mood, eyes with lids, cheeks, sunglasses, a cartoon
mouth). Each has its own palette, `Style`, sixteen-bar routine, construction and
personality pieces: the cat's head turns before its body and its tail trails every move;
the lamb's bell swings and her ears flop late; the robot's pendulum ticks side to side on
the beats (`beat_in_bar`), its antenna bulb lights on the `accent` and a wind-up key turns
in its back; the note's flag waves late and it leans back like a printed note head; the
child's curls bounce late.

### Draw, look, redraw (three passes each)

- **Miso.** 1: the scarf showed only its hanging end, a busy gold-crescent iris, a bare grey
  ball from behind. 2: head raised, scarf band lowered and thicker, warmer amber iris,
  tabby stripes on the back of the head. 3: the forehead stripes converged over the eyes
  like frowning brows — shorter, vertical, higher; and the cause of the "worried" look in
  the whole family found and fixed in the shared face: a sliver of upper lid whose lash
  line bowed *down*; lids now follow the eye's own curve and are drawn only once they
  close enough to matter.
- **Lana.** 1: wool like a smooth sack, a small head, the tuft a flat cap, the collar under
  the head, a bare cocoa head from behind. 2: deeper scallops, bigger head, collar lower
  (the bell shows), wool over the back of the head. 3: the fringe as three round puffs,
  more bumps, thicker front legs.
- **Tic.** 1: a small screen, the black pendulum slot dominating, the key invisible from
  behind. 2: a bigger screen, a narrow slot with copper scale ticks, a bigger key. 3: a
  taller shell (it stood shorter than the family).
- **Quaver.** 1: read at once as an eighth note. 2: a gloss on the note head, a thicker stem
  and a bigger flag. 3: a wider mouth (a speck on the dark head).
- **Sol.** 1: the hair read as a beret, the shorts hidden. 2: curls framing the face down
  to the ears with a taller crown, the shorts lower and the legs from them. 3: curls round
  the back of the head.

### How it was tested — the numbers (second delivery)

- `cargo test -p cantus-dance`: 36 unit tests, `tests/cast.rs` (6, now over all six
  characters: ids, both locales, every move and mood draws, nothing outside `bounds()`,
  no opaque pixel within 60 of a chroma key, 48 px vs 2 160 px), `tests/alloc.rs` (the
  counting allocator), `tests/choreo_song.rs` (the dancer's, 4) — all pass; clippy
  `-D warnings` clean.
- **Allocations a frame** (worst move, full energy, excited): songbird 227, cat 227, lamb
  212, robot 196, note 171, child 217 (was 779 for the songbird) — every one inside
  tiny-skia's `fill_path`, three a fill.
- **Time a frame** (600 × 600, size 1, every move × 8 phases × both sides × every mood,
  full energy, best of 15). The machine was loaded by the other builders throughout, so
  each run is calibrated against the binary of the first delivery's songbird (1.27 ms when
  quiet), run alternately in the same minutes. Quiet-equivalent means: songbird ≈ 1.34,
  cat ≈ 1.27, child ≈ 1.21, note ≈ 1.09, lamb ≈ 1.05, robot ≈ 0.95 ms — all within 1.5.
  Trino's new wings and silhouettes cost about 3 % (A/B under identical load).

### Marked lines (second delivery)

- `crates/cantus-dance/tests/choreo_song.rs` (the dancer's): the jump's phase half a beat
  in follows the character's `move_beats` (the contract lets a character set it; Trino's
  jump lasts two beats) — `// r35:cast`. The test failed on `r35/integration` as merged.

### Images

`docs/round35/cast/`: `family.png`; `<id>-8-bars.gif` for the six (180 px, 12 fps; the
360 px, 24 fps ones are outside the repository); `cast-<id>-moves.png` and
`cast-<id>-themes.png` (every mood on the checker and all twelve themes' stages).

### For the owner

- Your eyes on the six GIFs and `family.png`: are these the six you want, and is Trino
  still first?

## Review 1 — fixes

Each with a test that fails without it: rotations only through `rig::rotation` (a scan
of the sources in `tests/determinism.rs`, and the matrix equals tiny-skia's); the
allocation test now splits tiny-skia's allocations (inside its fills) from the rig's, and
the rig's must be zero — it caught builders lost to empty shapes, now returned to the
pool; the shadow without a gradient; bounds measured per character (songbird ±0.69, cat
±0.68, lamb −0.62…0.61, robot ±0.63, note −0.69…0.72, child −0.64…0.65; top −1.45) and a
dense bounds test with lift and spin; the edge-width crispness test (and a blurred
drawing failing it); Tic's LEDs fixed against the hue; the pendulum by absolute beats
(3/4, 5/4, 7/8 bar lines); wrapping cycle arithmetic (`u32::MAX`); the note renamed
**Corchea** in English too ("Quaver" is QuaverEd's; no product or mascot named Corchea
found; "Tuplet" and "Notello" were near existing apps). A blend's old move still starts on
cycle 0 until `Pose::from_cycle` lands (an ignored test waits for it).

## After `Pose::beats` and `Pose::from_cycle`

- Tic's pendulum swings by the song's own beats (`26° × cos(180° × beats)`): continuous
  through bar lines and changes of metre (tested 4/4 → 3/4 → 7/8).
- A blend starts from the old move's own side (`from_cycle`), in `kit::body` and
  `Songbird::body` (tested: the first frame of a blend is the old move as it was).
- The bounds, found wanting by the stage's all-characters test: since the dancer's
  `pose_at` blends from standing still at the song's start, the cast's test and
  measurement bases (taken from it) were drawing Idle whatever move they set — they now
  clear the blend. Measured honestly, a few moves reached far at full energy (a jump's
  landing squash widening the body by half, sway leaning 20°, the cat's tail through the
  floor); they are capped (squash 0.76–1.3, leans at most 12–14°, the cat's tail kept
  above the floor and counter-leaning) and the bounds set to what remains plus 0.05:
  songbird ±1.0, cat −0.78…0.96, lamb −0.76…0.86, robot −0.86…0.9, note −0.83…0.96, child
  −0.76…0.91; tops −1.45…−1.73. A new test draws the dancer's own poses (`pose_at`, the
  move automated) at energy 0 and 1 through two bars, lifted to 0, 0.5 and 1.5.

## The images, again — and the hero

- Every image in `docs/round35/cast/` redrawn from the capped moves: the six small GIFs
  (`<id>-8-bars.gif`, 180 px, 12 fps), `family.png`, and the moves and themes sheets (now
  palette PNGs, a fraction of their old size). The 360 px, 24 fps GIFs for the owner are
  in `F:\projects\daw-r35\tmp\cast\deliver\` (outside the repository).
- **The hero** for the README and the landing page: the six dancing together on one
  stage, each its own routine (the songbird in the middle), drawn by
  `cargo run -p cantus-video --release --example hero` (`png` frames or an `mp4`):
  - `family-hero.gif` — 864 × 270, 24 fps, four bars (bars 5–8 of the routines: claps,
    points, shuffles, arms up, a spin), Cantus Night stage, 72-colour palette, 3.85 MB.
    Eight bars at 24 fps would not fit in 4 MB (every pixel of six dancers moves every
    frame; 8.5 MB at 64 colours), so the GIF is a four-bar loop and the eight bars are
    the MP4's.
  - `family-hero.mp4` — 960 × 360, 24 fps, eight bars at 120 BPM with a simple beat,
    through `cantus_video::export` (Media Foundation H.264 + AAC), 2.55 MB.
