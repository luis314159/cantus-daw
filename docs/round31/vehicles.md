# Round 31 — the cores' vehicles, redrawn as illustrations

The owner found the *Audio cores* models ugly (polygon silhouettes) and asked for them to
be much better, drawn after looking properly at photographs of the real ones. This is that
redraw. The table, the mapping, the strings and the scenes are unchanged; only how each
model is drawn changed.

## What changed

- **Ten SVG illustrations of our own**, `crates/cantus-ui/assets/vehicles/*.svg` (GPL like
  the application; no text, no raster image, no logo), one per model, side profile facing
  right — the SR-71 from above, nose right, where its chined planform and nacelles read
  better than its very thin side. Each was drawn from Wikimedia Commons photographs of the
  exact model for its **proportions** (wheelbase, overhangs, roof and glasshouse, hood
  length, rake) together with the published dimensions, then checked by laying the drawing
  over the photograph at the same wheelbase; never traced. The photographs stay outside the
  repository (`daw-r31/tmp/vehicle-refs`); the list of source pages is in the builder's
  report to the integrator, since their titles name the makers.
- What makes each one known: the Grom's fat 12-inch tyres, red shroud, gold fork, angular
  lamp and tall mirrors; the Scrambler's L-twin in a trellis frame, yellow teardrop tank
  with its aluminium panel, brown bench, round headlight with its X, spoked wheels on
  block-tread tyres; the GR Yaris's short chunky three-door body, dark roof, roof spoiler,
  rear quarter glass, flared haunch and red brakes; the Fulvia's narrow body, tall airy
  glasshouse on slim chrome-framed pillars with a quarter-light, chrome blades and slotted
  wheels; the GT-R's wing on stalks, fastback C-pillar, fender gills, round tail light and
  orange brakes; the Mustang's long hood, fastback with louvred rear pillar, Kamm tail,
  chrome and 5-spoke mags; the Viper's huge hood with gills, cabin far back, ducktail,
  side pipes and the stripes over the top; the Enzo's fender pods over a low pointed nose,
  canopy, side intake and high tail; the Chiron's C-line round the side intake dividing
  the two-tone paint; the SR-71's chines, delta, nacelles with spikes, fins and burners.
- **Theme colours by placeholder** (`engines::palette`, `engines::svg`): `{paint}`,
  `{paint_hi}`, `{glass}`, `{trim}`, `{tyre}`, `{rim}`, `{chrome}`, `{lamp}`, `{tail}`,
  `{accent}`, `{edge}`, `{shadow}`… are filled at draw time. Each model keeps its colour
  identity (red Grom, yellow Scrambler, white GR Yaris, dark-blue Fulvia, gunmetal GT-R,
  green Mustang, blue-and-white Viper, red Enzo, two-blue Chiron, black SR-71) **in the
  theme's own hues** (danger, brand, the function hues, the panels' neutral) at a designed
  saturation and lightness, so it reads the same on Night and Paper and follows a theme's
  hue. The trim, tyres and a faint rim light (`{edge}`) answer the ground's lightness, so a
  black tyre still shows on a near-black well.
- **Rendered by `resvg`** at the exact physical pixel size it is shown at into the kit's
  layer atlas (`Brush::layer` / `layer_quad`, ADR 0024 §1 and §12, as the knobs and logos
  are), keyed by model, colours and size: redrawn only when one of them changes.
- **Two levels of detail**: `{detail}` and `{simple}` are opacities. Below 40 physical
  pixels of height (a list row at 100 %) the spokes, discs, panel lines, vents and gills
  give way to simple rims; at 200 % the row shows the full drawing.
- `engines::paint(b, rect, engine, Look::of(&tokens, ground))` is the whole interface;
  the settings rows pass the menu's face as the ground, the picture the well.
- The picture under *Audio cores* grew from 160 × 56 to 210 × 72 points so the drawing
  shows its detail there (`settings_audio::engine_picture`, three numbers).

## Tests

`engines.rs`: every count still has an engine, a word in both languages, a model and a
drawing; **every SVG parses, declares the 560 × 200 box, holds no text, image or style,
has every placeholder filled (none left, none unknown) and covers a vehicle-sized share
of its box at 53 × 19, 106 × 38, 235 × 84 and 470 × 168 in a dark and a light theme**;
the colours come from the theme (Night and Paper fill the same drawing differently, its
paint is used) and the small drawing differs from the large one.

## Shots

`workflow-engines-night-en.png`, `workflow-engines-paper-es.png` (all ten, large);
`workflow-cores-night-en.png`, `workflow-cores-paper-es.png` (the list, 100 %),
`workflow-cores-200-night-en.png`, `workflow-cores-200-paper-es.png` (200 %);
`workflow-cores-picture-night-es.png` (100 %), `workflow-cores-picture-200-paper-en.png`
(200 %) — the earlier shots of the same names replaced.
