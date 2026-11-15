# The dancer cast for Slate (round 40, `dancer`, design only)

The owner asked for the dancer redrawn for the Slate look (2026-11-12, round 39's answer 20).
His bar: characters **super professional and fun**, with animated previews before anything
is built.

**v1** offered three directions: Compás, Tono and Síncopa (kept in `v1/`). The owner's answer:

> *"Hazme más cool a Síncopa y a the knob (Tono), crea más personajes, Compás está chido."*

So **v2** is a cast of six in one family:
- Compás, approved and unchanged;
- Tono and Síncopa, redrawn to be cool rather than cute;
- three new characters: Bajo, Onda and Voz.

Nothing is built; the owner chooses.

## The latest (the owner's answers so far)

- **v1 → v2:** *"Hazme más cool a Síncopa y a the knob (Tono), crea más personajes, Compás
  está chido."* Tono and Síncopa were redrawn; Bajo, Onda and Voz were added.
- **v2 Síncopa:** *"Síncopa aún no me gusta."* Three new answers were drawn: S1 graphic, S2
  the sidekick, S3 noir (all in `variants/`, with their comparison GIFs and note).
- ***"Noir y paper se ven cool, ahora haz uno basado en Tron, medio luz neón, acá futurista."***
  - **S3 noir is now Síncopa.** Its files are the canonical `sincopa-*`, with the note
    `sincopa.md`.
  - **Glissando is new:** a figure of light. A dark suit carries lapis neon along its edges
    (a periwinkle lapis, not the films' cyan), with gold eyes and headphones and the logo's
    vermilion bars on its chest. Fast moves leave light trails. Signature: **Halo**. It is our
    own design: no disc, vehicle, helmet or circuit layout from the films.
  - The cast is now **seven**.

- ***"Están chidos … dale a todos a ver"*** (five more, all of them):
  - **H · Eco**, the delay's ghost: its echoes repeat each move late. Signature: *Feedback*.
  - **I · Vinilo**, the DJ: a record for a head, the tonearm for an arm. Signature: *Backspin*.
  - **J · Cinta**, the cassette: a b-boy whose tape swirls. Signature: *Rewind*.
  - **K · Metrónomo**, the serious one: a wooden butler, the pendulum exact on the beat.
    Signature: *Doble tiempo*.
  - **L · Espectro**, the analyser: a body of spectrum bars. Signature: *Pico*.
  - The cast is now **twelve**. The cast GIFs are two rows of six (twelve side by side would
    be too narrow to read).

- ***"Me he equivocado, Metrónomo está chido, Onda es el que no me gusta"***:
  - **Metrónomo (the butler) stays.** Two unfinished explorations made before the correction
    are kept in `variants/metronomo/`.
  - **Onda (the ribbon) is rejected**, kept in `variants/onda-ribbon/`. There are three new
    answers, each a character that *is* a waveform:
    - **O1 · the oscilloscope:** a phosphor trace with a Lissajous head; signature *Square*.
    - **O2 · the wave-jelly:** its top is the waveform, a crown and then shark teeth;
      signature *Shape shift*.
    - **O3 · the surfer:** rides its own wave as it turns square, then saw; signature
      *Square set*.
  - Open **`onda-variants-night.gif`** / **`-paper.gif`** and **`onda-variants.png`**, then
    each variant's `onda-o1-…`, `onda-o2-…`, `onda-o3-…` (sheet, moves and window GIFs).
    The note is `onda-variants.md`; my reading: O3 the coolest, O2 the most fun.
  - The cast GIFs and overview still show the ribbon in Onda's place. They are rebuilt once
    he picks (`python gifs.py cast`, `python overview.py`, with the chosen module in
    `anim.CHARS`).

- ***"El osciloscopio está chido"*** (final): **O1, the oscilloscope, is Onda.** Its files are
  the plain `onda-*`, with the builder's note `onda.md`; O2, O3 and the comparison are in
  `variants/onda/`. **The cast is final: twelve.**

**Files to show him now:**
- `cast-night.gif` and `cast-paper.gif` (all twelve, two rows of six), and `overview-cast.png`;
- for each of the five new ones: `<name>-moves-night.gif` / `-paper.gif`,
  `<name>-window-night.gif` / `-paper.gif`, `<name>-sheet.png`, where `<name>` is `eco`,
  `vinilo`, `cinta`, `metronomo` or `espectro`;
- `glissando-moves-night.gif` / `-paper.gif`, `glissando-window-night.gif` / `-paper.gif`,
  `glissando-sheet.png`;
- `sincopa-sheet.png`, `sincopa-moves-*.gif`, `sincopa-window-*.gif` (the chosen noir).

## Open first

1. **`cast-night.gif`** and **`cast-paper.gif`**: all twelve in two rows of six, dancing the same 12-beat
   routine on Slate's stage.
   - The routine: groove 4 beats, step-and-touch 4, **the drop 2** (each does its
     **signature move** there), groove 2.
   - 125 BPM, 25 frames a second.
2. **`overview-cast.png`**: the same twelve as a still, each in its groove and its signature,
   Night over Paper.
3. For each character:
   - `<name>-window-night.gif` / `-paper.gif`: floating over the real Slate window at 150 %,
     its default view, the same routine;
   - `<name>-moves-night.gif` / `-paper.gif`: its groove, step and signature side by side,
     4 beats looping;
   - `<name>-sheet.png` (and `.html`): idea, the whole window in Night and Paper, picker
     tiles at 88 px, 48 px, the companion window, nine key poses and eight moods in both
     themes, the palette with each colour's contrast, the construction, the risk;
   - `<name>.md`: the builder's note (what `cantus-dance` draws today, what it would need).
4. **Síncopa's history:** v2 in `v2/`; S1, S2 and the comparison of the three answers in `variants/`.
5. **v1, for comparison:** `v1/` holds v1's Tono and Síncopa sheets, GIFs and notes, the v1
   comparison GIFs and overview, and v1's sources (`tono_v1.py`, `sincopa_v1.py`).

## The cast

| | Who | Silhouette | Signature on the drop | Fills | Status |
|---|---|---|---|---|---|
| **A · Compás** | the pictogram: one ink, bars with round ends, gold headphones; pop and lock | thin figure | Star jump | ~20 | **approved, unchanged** |
| **B · Tono** | the knob: a knob-headed robot, LED visor, a vermilion patch cable round the neck, high-tops | round head on a lean body | **Turn it up**: turns its own knob, three clicks, the dots fill | ~50 | v2 |
| **C · Síncopa** | noir: one dark ink, a gold rim light, a hood, a coat whose tails swing, two gold glints | tall hooded silhouette | **Arm wave**: a wave hand to hand, then a look | ~40 | **chosen** (S3) |
| **D · Bajo** | the speaker: a studio monitor, tweeter eyes under heavy lids, a pumping woofer | a tall box | **Drop the bass**: wind up, then gold rings, a jolt and a shake | ~35 | new |
| **E · Onda** | the oscilloscope: a phosphor trace (amber) that draws a figure, a Lissajous head, a beam dot, afterglow | a figure of light lines | **Square**: the ripples snap square, the poses go angular, then a saw | ~8 fills, ~3 000 points (measure first) | **chosen** (O1) |
| **F · Voz** | the microphone: a mic-capsule head, a hat, a dinner jacket that inverts with the theme | slim, dapper | **Hat tip**: a bow, then ta-da | ~55 | new |
| **G · Glissando** | the figure of light: lapis neon on a dark suit, gold eyes, light trails | sleek athletic | **Halo**: a circle of light drawn round the head, dropped to the floor on the beat | ~30 (one translucent layer; measure first) | new |
| **H · Eco** | the delay's ghost: a small ghost in the theme's ink; its echoes repeat each move a quarter beat late | a dome and a hem | **Feedback**: echoes multiply to fill the frame, then all go out on the beat | ~35 (Feedback: make copies 1 fill each) | new |
| **I · Vinilo** | the DJ: a record for a head (face on the label), the tonearm for an arm, a gold chain | round head on a plinth | **Backspin**: brake, spin back, land on the drop | ~35 | new |
| **J · Cinta** | the cassette: reels for eyes, a backwards cap, track pants, its tape swirling | a landscape box | **Rewind**: the dance backwards at ×8, then a freeze | ~45 | new |
| **K · Metrónomo** *(kept by the owner)* | the serious one: a wooden pyramid butler, monocle and moustache, the pendulum exact | a tall pyramid | **Doble tiempo**: the pendulum runs away, he loses and recovers his composure | ~45 | new |
| **L · Espectro** | the analyser: a body of spectrum bars (legs bass, torso mids, arms snare, head highs) | a figure of bars | **Pico**: every bar hits the top and breaks into sparks | ~40 batched (many rects; bench first) | new |

**What makes them one family:**
- the Slate rooms and their inks;
- gold, with the logo's lapis and vermilion as the only other colours (Glissando glows lapis, Síncopa's rim is gold);
- values instead of outlines;
- the room-coloured cut that separates a piece from what is behind it;
- the same skeleton conventions and choreography (a groove, a step-and-touch, a signature on
  the drop);
- the same moods.

**Every colour measured:**
- each silhouette holds 3:1 on its room (the sheets list each colour's contrast);
- every opaque colour is at least 78 from a chroma key; the rule is 60 (ADR 0090 §3). The
  nearest are Glissando's Paper line (78) and Onda's lapis (83).

## What changed from v1 to v2

**Tono** (cute → cool):
- **Proportions:** a smaller head on a lean robot body with straight limbs, joints and fists.
- **Face:** v1's pill eyes and mouth are gone. Now an **LED visor** with gold segments that
  change with the mood, and no mouth (the chest's five bars are its voice).
- **Details:** a vermilion **patch cable** scarf with a swinging gold plug, a gold fin
  pointer, high-top fader-cap sneakers.
- **Motion:** control instead of bounce: a hard nod with fist hits that land and overshoot,
  a mechanical glide.
- **A signature** that is the app's own gesture: Turn it up.

**Síncopa** (friendly → swagger):
- **Proportions:** taller, about 8 heads.
- **Outfit:** an open oversized vermilion **bomber** over a cream tee, lapis **cargo**
  trousers with a strap, chunky **high-tops**, gold headphones **round the neck**.
- **Hair:** long **braids** in a high ponytail with gold beads and a hoop; they are the
  secondary motion that sells it.
- **Face:** **half-lidded** eyes and a **smirk**.
- **Motion:** hips that hit and overshoot, a shoulder pop on the off-beat, a finger snap,
  chin up.
- **A signature:** Shades.

Both kept their ideas (the knob; the figure in values) and their places in the family.

## The build order across the final twelve

**Wave 1: the shared pieces, with Compás, Tono and Síncopa.**
- **Compás**, as the new default: approved, and the calmest fit for Slate.
- **Tono** beside it.
- **Síncopa (noir):** the owner's pick; it proves the human rig and `edge.rs`'s rim on one ink.
- **They build what every later character reuses:**
  - **theme light in the canvas** (`StageInk`: ink, accent, room);
  - **two-bone IK with the ground rule**;
  - **`Style::snap`**;
  - **`rrect_into`**;
  - **a per-character extra in `Body`** (Tono's pointer; later Vinilo's turn, Metrónomo's
    pendulum, Voz's hat).

**Wave 2: the cheap, strong ones, all opaque.**
- **Metrónomo:** a pure-function pendulum; about 45 fills.
- **Vinilo:** a turn angle and rings; about 35.
- **Cinta:** the kit's limbs and a tape ribbon; about 45.
- **Bajo:** a rigid torso and rings; about 35.

**Wave 3: a new technique each.**
- **Voz:** IK hands, the hat; about 55.
- **Eco:** a non-skeleton character, echoes as earlier poses. Feedback's copies must be one
  fill each.
- **Glissando:** neon as three batched layers (one translucent), trails as one ribbon per
  limb. It builds the batched glow that Onda reuses.

**Wave 4: the two that must be measured first.**
- **Onda (the oscilloscope):** about 3 000 trace points a frame, tessellated once into one
  mesh, glow as three batched widths. See `onda.md` for what to benchmark first.
- **Espectro:** the master's spectrum on the dance feed, and hundreds of bar rectangles
  computed analytically (never clip masks). See `espectro.md`.

**Whatever is built:**
- **The existing cast stays.** Fuga, Volta and the family stay in the picker, and old songs
  keep their dancer.
- **A new default needs a migration.** Making Compás the default needs a schema step like
  59's, so that a saved dancer naming none keeps Fuga.
- **Off at start.** The dancer stays off at start.

## How it was made (and what is not verified)

- **The design rig** (`src/`): poses are pure functions of the beat, in `cantus-dance`'s
  conventions (beat 0 is the beat, index 0 is the viewer's left, the figure 1 unit tall).
  - `rig.py`: maths, `Pose`, `snap_time`.
  - `body.py`: two-bone IK with the ground rule, arms by angle and bend.
  - `moves.py`: the shared choreography.
  - Each character's module holds its own moves and drawing: `compas.py`, `tono.py`,
    `sincopa.py`, `bajo.py`, `onda.py`, `voz.py`.
  - Shapes are the rig's: opaque fills, ribbons, the outline as a grown fill (here in the
    room's colour), two values per piece.
- **Rendering:**
  - SVG drawn by Edge headless (`render.py`).
  - GIF frames are laid out in a grid, shot once, sliced and joined with one palette per GIF
    (`anim.py`, `gifs.py`).
  - Pillow merges identical frames and keeps their time: each 12-beat GIF lasts 5.76 s.
- **Rebuild** (Python 3 with Pillow, Edge; no cargo):

  ```
  cd docs/round40/design/dancer/src
  python sheet.py tono sincopa bajo onda voz && python overview.py && python gifs.py all tono sincopa bajo onda voz
  ```

  - Compás's files were left exactly as approved. `python sheet.py compas` and
    `python gifs.py moves compas` rebuild them; the only difference would be one stage label
    ("On the drop").
  - Quick looks: `try_poses.py <name> <move>`, `try_head.py <name>`.
  - Contact sheets: `contact.py`.
  - `build/` is not committed.
- **Not verified:** nothing was drawn by tiny-skia or timed against the 1.5 ms budget. The fill
  counts are counted from the drawings.
- **Tools and licences:**
  - **Pillow** 12.3.0, already installed (MIT-CMU / HPND licence); nothing downloaded.
  - **Microsoft Edge** (the system's) for rendering.
  - **Barlow Semi Condensed** (SIL OFL 1.1), the app's own copy in
    `crates/cantus-ui/assets/fonts/`.
- **Names:** working names, musical words as the cast's habit has it (Fuga, Volta, Corchea):
  - *Compás*: the bar;
  - *Tono*: tone;
  - *Síncopa*: syncopation;
  - *Bajo*: bass;
  - *Onda*: wave (the oscilloscope's);
  - *Voz*: voice;
  - *Glissando*: the slide;
  - *Eco*: echo (the Delay unit's effect: its ghost);
  - *Vinilo*: vinyl;
  - *Cinta*: tape;
  - *Metrónomo*: the metronome (not Tic, the old family's robot);
  - *Espectro*: the spectrum.
