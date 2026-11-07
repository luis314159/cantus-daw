# Round 37 — chars: a cooler lead character (ADR 0105)

Builder: **chars** (branch `r37/chars`). The owner's idea (2026-11-06, BACKLOG §6i): *"hacer
otro personaje más cool, algo de anime o algo así pero más cool, para cambiar el
principal… quizás un robot y un ninja"*.

## What was done

- **Fuga the ninja** (`builtin:ninja`), now a new dancer's character (`DEFAULT_CHARACTER`,
  first in `cast::builtin()`), and **Volta the mecha** (`builtin:mecha`) second. The six
  of round 35 stay after them; Tic the metronome robot stays.
- Both are original, family-friendly and carry no weapon. The emblem is the fermata on
  Fuga's sash. The palettes keep clear of the chroma keys.
- **The look** (`cast/edge.rs`): hard two-tone cel shading with a coloured rim light
  (violet on Fuga, coral on Volta), angular plates, a thin crisp ink, light that never
  turns with the hue. Both are scaled 0.82 to stand as tall as the family, with the ground
  computed for that scale.
- **Faces:**
  - Fuga is always masked. Narrow amber eyes and thick brows carry the moods.
  - Volta's V visor shows a different shape of light for every mood, plus a wink. Its chin
    equalizer lights when it sings.
  - Neither has a blush or star eyes.
- **Moves:** all sixteen moves, the moods, a sixteen-bar routine each, the beat extras,
  each facing itself.
  - Fuga's own: a low ready stance, a high kick, the aerial flip, a tornado spin, a
    crouched landing, a martial bow.
  - Volta's own: pop and lock on point, bounce and wave; a rocket jump with jets; a hero
    stance. The speaker core pulses and rings on the beat.
- **Old songs keep their dancer:** schema 59 writes `builtin:songbird` into dancers saved
  without a character. 58 is an empty step held for another branch; the integrator
  renumbers. There is a v59 fixture.
- **The model sheet:** `cargo run -p cantus-dance --example char_sheet -- <dir> [ninja]
  [mecha]` writes the turnaround, expressions, key poses, palette and a hero, on the
  checker, the dark and the light themes.
- **Names in both locales:** Fuga and Volta (both music's Italian, both read in English and
  Spanish), and their blurbs. The manual's dancer page in EN and ES names the cast.

## The design rounds (shown to the owner before rigging)

1. **v1**, the family's build with anime touches: "más cool, menos infantil".
2. **v2**, anime-teen proportions (about 1:5), almond eyes, sleeker armour, attitude poses:
   "menos bonito, poco más serio, que se vea perrón".
3. **v3**, an action-anime key visual. Approved, with two fixes before the rig: Fuga's hair
   carried round the back (and no ear discs), and Volta's moods each readable at stage
   size.

Images (`docs/round37/chars/`):
- `ninja-sheet.png`, `mecha-sheet.png`: the v3 model sheets.
- `ninja-moves.png`, `mecha-moves.png`: every move at four phases and the other side.
- `ninja-8-bars.gif`, `mecha-8-bars.gif`: the routine at 120 BPM on Cantus Night.

The larger previews (16 bars, both themes, the pair side by side) are outside the
repository, in `F:\projects\daw-r37\tmp\chars\`.

## How it was tested — the numbers

- **`cargo test -p cantus-dance`: green.** The cast tests run over all eight characters:
  - bounds: measured with `cast_bounds`, Fuga −0.88…0.79 × −1.46, Volta ±0.76 × −1.46,
    declared with a margin;
  - no opaque colour within 60 of a chroma key;
  - the same drawing at 48 and 2 160 px;
  - allocations only inside fills, no platform trigonometry.
- **`tests/action_pair.rs`:** Volta's light never turns with the hue; every pair of its
  moods differs at a 300 px stage (about 120 px tall); both draw the same twice.
- **Unit tests:** the aerial lands upright and alternates sides; pop and lock meets its
  cycle and locks; the jets fire only in a jump.
- **`cargo test -p cantus-project`: green**, with `schema59.rs` (four cases) and the
  schema-49 test naming the songbird.
- **`cantus-ui/tests/dancer_default_e2e.rs`:** a dancer added through the picker in
  `CantusApp::assemble` names and draws Fuga; an old song opened through the app draws the
  songbird, or Volta when it named Volta. `dancer_window.rs` is green too.
- **Cost** (`cast_bench`, release, 600 × 600, every move at eight phases, both sides, every
  mood, full energy):
  - Fuga 1.16 ms (slowest move 1.24 ms);
  - Volta 1.24 ms (slowest 1.31 ms);
  - the songbird 1.36 ms on the same run, against a budget of 1.5 ms;
  - fills a frame: Fuga about 90, Volta about 113 (after trimming the seams' halos and the
    knees), the family 41–58.
- **The stage budget** (`cantus-ui/tests/frame_budget.rs`, `stage_budget`): the stage
  shows a character as one texture, so the character adds no triangles on screen. With
  Fuga as the default, a dancer playing reads 54 765 triangles against 50 225, +4 540.
  That is the same +4 540 as on the integration tree with the songbird (55 214 against
  50 674). *At integration:* not another builder's change — two applications built apart
  sometimes differ by a whole inspector section; the test now measures the dancer against
  itself in one application (`93144d44`), and the cause is still open
  (`docs/round37/integration.md`). Drawing Fuga
  costs 268 allocations a frame, all inside tiny-skia's fills.

## Review fixes (after the merge, on 4661c74c)

1. **Every path names the dancer's character.** Before, two paths made a dancer with no
   character: the inspector's instrument menu (`set_instrument` with a bare spec) and
   *Swap instrument* keeping matching settings (`carry_over` rebuilt from a bare spec).
   Now `CantusApp::set_instrument` and the swap's carry-over both go through
   `dance_live::new_spec`. The e2e test `every_way_to_a_dancer_names_its_character`
   (`cantus-ui/tests/dancer_default_e2e.rs`, through `CantusApp::assemble`) saves the
   project and reads it back for each path.
2. **The character lists are eight now, Fuga first and Volta second:**
   - `README.md`, with a new hero GIF of the eight, `docs/round37/chars/family-hero.gif`;
   - `web/src/dance.rs`;
   - the landing page's clip, `web/assets/dance/family.mp4` and its poster `family.webp`,
     rendered again by `cantus-video --example hero -- mp4 … 1440 318 30 0.98 0.95`;
   - the manual pages, built again by `web/build-manual.ps1`. Only `dancer.html` and
     `index.html` are committed (the rest differ only in line ends).

   The web crate still checks for `wasm32-unknown-unknown`.

3. **Second review: the leftovers closed.**
   - `xtask manual` drops HTML comments (`strip_comments`) outside fenced code blocks and
     inline code, and now renders fenced blocks as `<pre><code>` (the Linux and AI-tools
     pages' commands were run together into one line before). The test is
     `html_comments_go_but_not_from_code`. Only the pages whose content changed are
     committed: `index`, `linux` and `ai-tools`, EN and ES.
   - The screenshots are shot again with Fuga:
     - `docs/images/dancer-window.png` and `web/assets/dance/window.webp`: `shoot_dancer`,
       Play on Slate at 100 %, 1 040 × 680;
     - `docs/images/dance-companion.png` and `web/assets/dance/companion.webp`: the
       companion's own pixels from `shoot_stage native` (which now also writes them alone,
       `native-*-alpha.png`), laid over a drawn desktop picture, 500 × 560.

     Their alt texts say what they show.
   - Fuga's and Volta's blurbs (EN and ES) now describe the v3 designs (no headband, no
     pearl armour).

## Marked lines (in files I do not own)

- `crates/cantus-core/src/inst/dancer.rs`: `DEFAULT_CHARACTER = "builtin:ninja"; // r35:cast, r37:chars (Fuga)`.
- `crates/cantus-project/src/format.rs`: `SCHEMA_VERSION = 59`, `migrate_57_to_58` (empty)
  and `migrate_58_to_59`, `// r37:chars`.
- `crates/cantus-ui/src/app.rs` (`set_instrument`) and `crates/cantus-ui/src/swap.rs`
  (`replace_instrument`): a dancer's character through `new_spec`, `// r37:chars`.

## Verified only here

Everything was seen as pictures from the one renderer (the sheets, the move sheets, the
GIFs). The app's own stage and companion window draw through the same renderer, and the
e2e test reaches the boot path. Nobody has watched them in the running app yet.
