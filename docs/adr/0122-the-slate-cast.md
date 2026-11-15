# ADR 0122 — The Slate cast: twelve dancers drawn in the theme's own light, Compás the default

**Status:** accepted (round 40, cast builder) · **Date:** 2026-11-12 · **Extends:** ADR 0087
(the dancer, `Character`), ADR 0090 (the cast: one rig, opaque fills), ADR 0105 (Fuga and
Volta), ADR 0111 (Slate) · **Schema:** 65 (62–64 are other builders' and empty steps on this
branch; the integrator keeps theirs) · marked line in `cantus-core` (`DEFAULT_CHARACTER`).

## Context

Round 39 made Slate the default look; the dancer was still drawn for the old one. The owner
asked (2026-11-12) for the dancer redrawn for Slate, saw the designer's sheets and GIFs
(`docs/round40/design/dancer/`) through four rounds of answers, and approved twelve
characters: **Compás** (the new default), **Tono**, **Síncopa** (the noir variant),
**Bajo**, **Onda** (the oscilloscope, O1), **Voz**, **Glissando**, **Eco**, **Vinilo**,
**Cinta**, **Metrónomo** and **Espectro**. His bar is "super professional and fun", and he
judges what is built against the GIFs he approved.

The designs were drawn by a design rig in Python (`docs/round40/design/dancer/src/`): a pose
of the beat with a moving pelvis, two-bone IK for the legs with the ground rule, hand
targets or arms by angle and bend, a chest that isolates, snap time; and each character's
drawing as SVG pieces with a room-coloured cut. `cantus-dance`'s family rig (`rig_dance`'s
`Body`) has none of the IK nor the theme's colours.

## Decision

### 1. The design rig, ported number for number (`cantus_dance::slate`)

- `slate::fig` — the design's pose (`Fig`), its easing (a cubic ease-out, an
  ease-out-back with its overshoot), `ik2`, `snap_time` (pop and lock: the hit on the grid
  line, the settle after it; exactly on the line when it lands there).
- `slate::body` — `Props` and `solve`: the joints of a pose; a leg never stretches past its
  length, the pelvis drops instead (the ground rule).
- `slate::moves` — the shared choreography (`groove`, `steps`, `jump`, and the
  open-silhouette `pump`, `steps_open`, `star`, `steps_human`, `steps_wave`).
- `slate::paint` — the pieces as the rig's opaque fills through its pooled pens: straight
  bars with round ends and joins (a capsule a segment, one fill), tapered capsules, rounded
  rectangles (`rrect_into`), the room-coloured cut (a piece grown and filled in the room
  first), even-odd rings of one width all round, one translucent oval for the shadow.
- A character is a `slate::Figure` (its proportions, its own moves, its drawing) made a
  `Character` by `slate::Slate`. Matching the approved GIFs is the measure: each character
  is rendered by the app's renderer beside the design's GIF (`examples/slate_frames.rs`,
  `docs/round40/cast/side_by_side.py`, the GIFs in `docs/round40/cast/`).

### 2. The song's pose to a figure

- **Its own moves** for the three the designs dance: `Bounce` is its groove (two beats),
  `StepRight`/`StepLeft` its step-and-touch (four beats; `StepLeft` starts on its other
  half), `Jump` the bar on the drop — its **signature** in the first two beats, landing on
  the third, then the groove (four beats, the designer's bar). Its routine is
  `Bounce, StepRight, Jump`: the designer's 12-beat routine, one move a bar.
- **Every other move** is the family's choreography (`rig::dance`) turned into a figure
  (`slate::from_body`): arms by angle and bend (or the hand they reach), feet, lean, squash,
  the bob's rise as a lift past a small dip.
- A change of move blends as the pose says; the energy sizes the motion (the design's at
  the default 0.7, calmer below); the knob's turn narrows the shoulders.
- **The face from the mood**: Happy (the default) keeps the move's own eyes (the designer's
  dancing face), Neutral and Excited lean them, Calm closes them, Surprised widens, Sleepy,
  Cool (a visor, a scan, a bar of glints) and Shy (a wink); a blink closes open eyes.
- A Slate character `jumps_itself`: the pose adds no jump arc of its own to its `Jump`
  (the signature's lift is in the figure).

### 3. The theme's light on the stage (`render::StageInk`)

`Stage` and `Canvas` carry the theme's ink, its quieter inks, the logo's gold and vermilion
in the theme's tone, the shadow, and the **room** — the colour behind the dancer, which the
cuts are drawn in. The interface sets it (`dance_texture::stage_ink`): over the workspace
the panels' colour, on the docked Stage tab and in the companion window the stage's, in
the editor the theatre's ground, on a picker tile the tile's, in a video the frames'
background (a chroma key's colour over a key). Only the gold turns with the hue. The family
of rounds 35–37 and imported characters ignore it.

### 4. What a character adds: moods, the spectrum, a key

- **Moods of its own** (`Figure::mood`): beyond the eyes, a mood may set a level, a posture
  or its own eighth mood in Shy's place (Bajo's blast, Vinilo's spinning, Cinta's
  rewinding, Metrónomo's monocle off, Voz's hat tip, Eco's boo, Onda's square,
  Espectro's loud). The dancer's mood field shows the character's own words where it has
  them (`cast.<id>.mood.<mood>`, both locales; `dancer_controls::mood_words_of`).
- **The spectrum** (`Pose::bands`, `Character::hears_spectrum`; Espectro): four bands —
  lows, mids, snare, highs — of the master, reduced from the companion's third-octave
  analyser (`dance_live::spectrum_bands`). The analyser runs on the interface's side from
  the engine's master tap whenever a project's dancer hears it **and the song plays**, the
  companion window open or not; never on the audio thread. Stopped, or with no dancer left
  that hears it, the tap is off and nothing is analysed (amended, `fix-c` R2-C1: the last
  Espectro deleted left the tap on and the analyser running every frame — round 38's rule,
  nothing computes at rest). **Espectro chosen as the companion window's own character**
  hears the companion's analyser too, the window's visualiser on or off (amended, `fix-c`
  R2-C2: it was posed without bands). A video export analyses its own render with the same
  analyser on its own instants and the live ballistics (`dance_live::video_spectrum`,
  `cantus_video::dance::Spectrum`): deterministic, and within 0.15 of the live bands on
  the same audio (`tests/espectro_bands.rs`); **one** analysis feeds both Espectro and the
  video's visualiser (`video_export::job::analysed`, amended `fix-c` R2-C4: it was made
  twice). With none heard (a preview tile, a stopped
  song) the bars dance to a spectrum made from the beat, as the designs do.
- **Over a chroma key** (`render::key_safe`, `StageInk::key`): the designs' lapis (Cinta,
  Vinilo, Glissando) is 27–35 from the blue key by a keyer's measure, where ADR 0090 §3
  keeps 60. Drawn over a key — a video's chroma background, the companion window's key —
  every colour within 62 of that key moves toward its own grey the least that clears it;
  everywhere else the colours are the designer's. The cut (the room) is never moved: over
  a key it is the key. The owner was asked whether to redesign the blues instead.

### 5. Drawing without masks or blurs

Two values of a piece are computed, not clipped: the lit part of a tube, a rounded
rectangle or an outline (`paint::cel_tube`, `cel_rrect`, `cel_blob`), a convex clip for a
polygon (`clip_convex`, Metrónomo's wood). A glow is batched layers of the same lines — a
translucent halo, the line, a pale core — one path a layer (Glissando's neon, Onda's
trace: about 400 samples on the stack, segment quads in one path), never a blur.
Espectro's bars are computed against each part's outline (capsules, a polygon, an
ellipse), plain rectangles batched by colour. Eco's Feedback copies are one fill each.

### 6. Compás the default; old songs keep their dancer (schema 65)

`DEFAULT_CHARACTER` is `builtin:compas` and the Slate cast leads the picker; Fuga, Volta and
the family stay in it. A dancer saved from schema 59 to 64 that named no character showed
Fuga: loading it writes `builtin:ninja` into its text (the same step as schema 59's, which
wrote the songbird). The dancer stays off at start.

### 7. The picker scrolls

With twenty characters and *Import*, the picker keeps its tiles at least 48 pt and scrolls
the rows past its card with the wheel (`views/inst/dancer_pick.rs`: `grid`, `placed`), a
slim kit-drawn thumb showing where; the Slate cast first, then Fuga, Volta and the family.

## Consequences

- The twelve are built in the designer's waves (`docs/round40/cast.md`); each is matched to
  its GIFs before the next.
- A theme of any colour gets a dancer in its own ink; a room the interface does not know
  (a transparent window over the desktop) gets the panels' colour as its cut.
- Every character keeps the family's rules: opaque fills, no per-frame allocation outside
  tiny-skia's fills, `cantus_dsp::math` for every transcendental, inside its bounds, within
  the 1.5 ms frame.

## What the tests hold

- `slate::fig` / `slate::body` / `slate::moves` unit tests: snap time hits and holds, IK keeps
  its bones, the ground rule drops the pelvis, a step lands on the beat, the jump lands on it.
- `tests/slate.rs`: a pose held between snaps is still to the pixel; Compás is drawn in the
  theme's ink on Night and Paper; only the gold turns with the hue; every mood a face of its
  own; the drop is four beats and the routine the designer's; the cut follows the room.
- `tests/cast.rs` for every character (bounds, 48 against 2 160 pixels, chroma distance,
  blends from the old move's side), `tests/alloc.rs`, `examples/cast_bench.rs` (1.5 ms).
- `tests/cast.rs` checks the chroma distance per key with the stage told which key; the
  edges at 48 and 2 160 pixels in Paper's light (at night the neon's halo is a glow by
  design).
- `crates/cantus-project/tests/schema65.rs`; `crates/cantus-ui/tests/dancer_default_e2e.rs`
  (a new dancer is Compás through `CantusApp::assemble`; songs of 57 and 61 keep theirs);
  `crates/cantus-ui/tests/cast_e2e.rs` (each of the twelve picked as the picker does, one
  undo step, dancing to the transport from the engine's feed, saved and opened again; the
  mood words); `crates/cantus-ui/tests/espectro_bands.rs` (the export's bands are the
  live analyser's within 0.15, two exports identical, a video poses Espectro with them).
- `examples/slate_worst.rs` times each character's heaviest moment against 1.5 ms.
- `fix-c` (the second review): `crates/cantus-ui/tests/espectro_live_e2e.rs` through
  `CantusApp::assemble` on a real processor — a project's Espectro playing turns the master
  tap on, the closed companion's analyser analyses and the pose carries the song's bands;
  stopped, and with its track deleted, the tap is off and the analyser neither updates nor
  analyses; Espectro as the companion window's own character hears the song, its
  visualiser off or on. `video_cast.rs` — an export analyses its render once for Espectro
  and the visualiser (`job::analysed`, counted), the frames pose Espectro with those bands,
  and a song with Espectro exports with the visualiser on. `dancer_window.rs` — the picker
  scrolled by the wheel, a tile of its lower rows clicked chooses that character (one undo
  step), the same place at the top another.

## Amendment — review 2 of round 40 (builder `fix-d`)

- **The picker scrolls as the kit does** (R2-B8). Past the smallest tiles the grid scrolls with
  the kit's scrollbar (`kit::widgets::scrollbar`, draggable, as the instrument picker's), inside
  the card at its right with the tiles clear of it; the wheel and the bar move it by points, not
  whole rows, a tile partly in view clipped to the grid. `MIN_SIDE` (48 pt) is scaled with the
  interface (`grid`'s `floor`). A name longer than its tile (*Metrónomo* under 48-pt tiles) runs
  across its cell (the tile and the gap beside it) and takes the smaller role before it would be
  elided (`ux_whole_words`, every look, both languages, 100 and 150 %). Tested:
  `dancer_window::every_character_fits_the_picker` (at 1× and 1.5×) and
  `the_picker_scrolls_with_the_kits_scrollbar_and_the_wheel`.
- **Compás's own Shy** (R2-B8): it has no face, so the generic *Shy* (a wink) showed nothing
  of its own. Its Shy is *Cueing* / *Preescucha* — a DJ's cup lifted off one ear, up and out
  (`cast/compas.rs`), its word in the mood list (`cast.compas.mood.shy`). A posture added to an
  approved character: for the owner's eyes (`docs/round40/fix-d/cast-compas-moods.png`, the last
  column).
