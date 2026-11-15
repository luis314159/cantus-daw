# Round 40 — `cast`: the Slate cast (ADR 0122)

**Status:** done — all twelve characters the owner approved are built, matched to his GIFs and
shot: Compás (the new default), Tono, Síncopa (noir), Bajo, Onda (the oscilloscope), Voz,
Glissando, Eco, Vinilo, Cinta, Metrónomo, Espectro. Branch `r40/cast`.

## What changed

- **The Slate rig** (`crates/cantus-dance/src/slate/`): the designer's Python rig ported
  number for number — `Fig` (the pose), the design's easing, two-bone IK with the ground rule
  (the pelvis drops before a leg overstretches), snap time (the hit on the grid line, the
  settle after it), the shared choreography (groove, step-and-touch, the jump; pump,
  steps_open, star, steps_human, steps_wave), and the pieces (straight bars with round ends
  and joins, rounded rectangles, the room-coloured cut, rings of one width, two-value cels
  computed without masks, convex clipping). A `Figure` is made a `Character` by `Slate`;
  its own moves are the groove (`Bounce`), the step-and-touch (`StepLeft`/`StepRight`) and
  the bar on the drop (`Jump`: its signature, landing on the third beat, then the groove),
  so the routine `Bounce, StepRight, Jump` is the designer's 12-beat routine; every other
  move is the family's choreography adapted. A character that `jumps_itself` gets no jump
  arc from the pose on top of its signature.
- **The theme's light on the stage** (`render::StageInk`): ink, quieter inks, the logo's gold
  and vermilion in the theme's tone, the shadow and the room the cuts are drawn in. Every
  stage the interface draws sets it (floating, docked, the editor's stage, the picker's
  tiles, the companion window, a video and its preview). Only the gold turns with the hue.
- **The twelve**, each with its own groove, step and signature:
  - wave 1 — Compás (motion marks, the beat ring), Tono (dots, LED visor, Turn it up), Bajo
    (the first not a skeleton: a rigid cabinet; Drop the bass's rings);
  - wave 2 — Síncopa (gold and cool rims as offset fills), Vinilo (the record turning with
    the song), Cinta (the tape, the reels, Rewind), Metrónomo (wood by convex clip, the
    pendulum on the song's beat whatever the move);
  - wave 3 — Voz (cloth in two values without masks, the hat tip), Glissando (neon as
    batched layers — halo, line, core — three fills a colour, never a blur; trails, the
    Halo), Eco (echoes a quarter beat late; Feedback's copies one fill each);
  - wave 4 — Onda (the phosphor trace: about 400 samples a frame on the stack, segment quads
    in one path a layer, the halo from a third of the samples and the core from half; the
    Lissajous head, the beam, the afterglow), Espectro (bars computed against each part's
    outline, batched by colour; the real spectrum).
- **Espectro hears the song.** `Pose::bands` (four bands: lows, mids, snare, highs) for a
  character that `hears_spectrum`. Live, the companion's analyser (third-octave bands of the
  engine's master tap, on the interface's side, never the audio thread) runs whenever such
  a dancer is on, the companion window open or not (`dance_live::spectrum_bands`). In a video
  the export analyses its own render with the same analyser on its own instants and the live
  ballistics (`dance_live::video_spectrum`, `cantus_video::dance::Spectrum`) —
  deterministic; the dialog's preview uses its preview render's. With none heard (a picker
  tile, a stopped song) the bars dance to a spectrum made from the beat, as the designs do.
- **A character's own moods** (`Figure::mood`): what a mood means beyond the eyes, and an
  eighth mood of its own in Shy's place (Bajo's blast, Vinilo's spinning, Cinta's rewinding,
  Metrónomo's monocle off, Voz's hat tip, Eco's boo, Onda's square, Espectro's loud). The
  dancer's mood field shows the character's words (`cast.<id>.mood.<mood>`, both locales).
- **Over a chroma key** (`render::key_safe`): the designs' lapis is 27–35 from the blue key by
  a keyer's measure (ADR 0090 §3 keeps 60). Drawn over a key, a colour within 62 of it is
  moved toward its own grey the least that clears it; everywhere else the colours are the
  designer's. The owner was asked whether he prefers the blues redesigned.
- **Compás is the default** (`DEFAULT_CHARACTER = builtin:compas`); the Slate cast leads the
  picker; Fuga, Volta and the family stay. **Schema 65**: a dancer saved from 59 to 64 that
  named no character gets Fuga written in (62–64 are empty steps here; the integrator keeps
  pads' 62). `fixtures/v65.cantus`. The dancer stays off at start.
- **The picker scrolls**: twenty characters and *Import* do not fit a card at a readable
  size, so its tiles keep at least 48 pt and the rows past the card are reached with the
  wheel over them, a slim thumb at the grid's edge showing where (`pick::grid`, `placed`).
- **Shots**: the floating and docked stage in Slate Night and Paper with Compás in its own
  groove (`shoot_stage` with `CANTUS_SHOT_SLATE=1`), the picker and the editor's stage.

## How it is tested

- Unit tests in `slate::{fig, body, moves, mod}`; `tests/slate.rs` (holds are still to the
  pixel, the theme's ink, only the gold turns, a face per mood, the drop's bar and routine,
  the cut follows the room, the lapis is the designer's except over blue).
- `tests/cast.rs` for every character: bounds, coverage and edges at 48 and 2 160 px (in
  Paper's light: the night neon's halo is a glow by design; Onda's half-pixel trace has its
  edges measured at 240 px), chroma distance per key with
  the stage told which, blends from the old move's side, blink and hue; `tests/alloc.rs`
  (no allocation outside tiny-skia's fills).
- `crates/cantus-project/tests/schema65.rs` and `schema59.rs`.
- End to end through `CantusApp::assemble`: `dancer_default_e2e.rs` (a new dancer is Compás;
  songs of 57 and 61 keep the songbird and Fuga), `cast_e2e.rs` (each of the twelve picked
  as the picker does — one undo step — dancing to the transport from the engine's feed,
  saved and opened again; the mood words), `espectro_bands.rs` (an export's bands equal the
  live analyser's within 0.15 on the same audio, two exports identical, a video poses
  Espectro with its render's bands).
- `cantus-video`'s tests with the spectrum field.

## Frame cost (release, 600 px, a machine shared with other builds — not a claim)

`examples/cast_bench.rs` (mean a frame) and `examples/slate_worst.rs` (the heaviest moment),
budget 1.5 ms; Fuga measured 1.09–1.46 in the same runs. The last runs: Onda 1.21 mean,
its Square 1.20 (the halo from a quarter of the samples, the core from a third; below 140
pixels tall it draws the line alone); Espectro 0.95, Pico 0.76–0.83; Glissando 1.01, its
Halo 1.17–1.43; Eco's Feedback 0.88–1.21; Bajo's blast 0.72–0.90; the rest 0.36–0.79. The integrator's quiet measure decides; Glissando's halo
and Onda's trace are the nearest to the line, and both can narrow their halo (2.6× the line)
without a new look.

## Matching the designs

`examples/slate_frames.rs` renders a character with the app's renderer timed as the
designer's GIFs (125 BPM, 12 frames a beat); `side_by_side.py` puts the approved GIF above
and the app's below: `<name>-moves-<theme>.gif` (groove, step-and-touch, the drop) and
`<name>-routine-<theme>.gif` (the 12-beat routine through `pose_at` itself), for all twelve,
Night and Paper, in `docs/round40/cast/`.

## Verified only in emulation

Everything was seen through the CPU renderer and synthetic frames; the live spectrum through
the analyser fed by the test, not a real interface. Not run on Linux (the integrator runs
it). The frame costs are from a busy machine.

## What the owner should look at

The side-by-side GIFs (all twelve), `app-stage-float-*.png` and `app-stage-dock-*.png`, and
on his screen the items under *Ronda 40 — cast* in `docs/beta-checklist.md`: each character
dancing, Espectro hearing a song and in an exported video, the blues over a blue key.

## What is left

- The owner's answer on the blues over a blue key (kept as designed elsewhere).
- The quiet frame-cost measure (the integrator's).
