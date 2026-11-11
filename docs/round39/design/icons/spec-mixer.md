# Spec for the kit — the mixer strip and inspector glyphs (set 3)

The drawings are in `glyphs-m-mixer.js` and `glyphs-m-mixer2.js`. Each size is its own
drawing: pixel cells (`seg`, ring templates, `head`) or snapped SVG. Port them per size as
they are. **Never scale one size to make another.** The places and sizes are des-mixer's
(`des-mixer/README.md`, `states-*`, `inspector-*`). This family replaces their drawing,
not their size or place.

## Grammar (the same as set 2)

- The canvas is the glyph's own box (12–27 device px), centred where des-mixer puts the
  glyph. Lines are 1 px up to 24 px and 2 px at 27 (and at 30 for routing). A 45° line or a
  ring below 24 px is pixel cells. Heads are solid pixel triangles that grow from their
  stroke.
- **Inks by state** (Slate's tokens):

  | State | Main ink | Second ink | Well |
  |---|---|---|---|
  | quiet | ink 3 | ink 4 | none |
  | rest, in a well | ink 2 | ink 3 | well |
  | hover | ink 1 | ink 2 | well-hi |
  | on | the state's colour | that colour mixed 45 % into the well | well + 20 % of the colour |
  | off | ink 4 | ink 4 | well |

  "Quiet" is for bare glyphs: des-mixer's polarity row, glyphs inline in fields and rows.
- **On colours.** The accent by default. `arm` is `rec`. `enable` and the insert lamp are
  `play` (green). The rest use the accent.

## Glyphs, sizes and places (100 % → 150 %)

| Glyph | Sizes | Where (des-mixer) | Notes |
|---|---|---|---|
| `phase` Ø | 16, 18, 12 → 24, 27, 18 | Wide polarity row 16 (bare, quiet; on: a well tinted, accent glyph); Large 18; inspector Channel 12 beside its word | ring (pixel ring 8/10) + the slash past the ring (x+y = const cells); 27: circle r 7 + line, 2 px |
| `swap` ⇄ | 16, 18, 12 → 24, 27, 18 | beside `phase` | two arrows passing; heads are 2–3-column pixel triangles |
| `mono` | 16, 18, 12 → 24, 27, 18 | Large polarity row; inspector Channel | ring + a 2×2 centre |
| `link` | 16, 18, 12 → 24, 27, 18 | stereo link / linked strips (Large; menus) | two overlapping pills, 1 px (2 px at 27) |
| `arm` | 14, 16, 18 → 21, 24, 27 | Compact arm dot (14, quiet at rest, no well); Wide arm key 16; Large / inspector record key 18 in a 24 well | a filled circle 0.4 of the box; armed: `rec` and the well tinted |
| `enable` | 12, 14, 16 → 18, 21, 24 | a device's or an insert's on/off, the strip enable | power: a ring open at the top (55°) and its stem, 1 px; on: `play` |
| `fx` | 14, 16 → 21, 24 | the strip's insert chain on/off key | three rows: a lamp (main ink) and a slot (second ink) |
| `send` | 12, 14, 16 → 18, 21, 24 | sends rows, "Add a send", routing menus | the signal line goes on; a 45° branch leaves it with an L head |
| `out` | 12 → 18 | output row (→ a bus), inspector Output field | arrow; 3-column head (4 at 18) |
| `into` | 12 → 18 | inspector Input field, Recording input | arrow into a bar |
| `ret` | 12 → 18 | a return's head ("↳"), receives list | hook down then right |
| `key` | 12, 14 → 18, 21 | a keyed insert (12, row's right); Sidechain "Key into" field (14) | bow (pixel ring 5/7/8) + shaft + two teeth |
| `routing` | 16, 20 → 24, 30 | the mixer header's "show cables" toggle, the Rows menu | two sockets (pixel rings) + a patch cable (cubic, 1 px; 2 px at 30) |
| `freeze` | 12, 14 → 18, 21 | strip head (12, `accent`), inspector Freeze key (14) | snowflake: eight arms; from 18 px the four main arms carry branch ticks |
| `vca` | 14, 16 → 21, 24 | the VCA strip head, the VCA field, menus | a slot with its cap + two follower slots ticked at the cap's level (des-mixer's follower tick) |
| `lamp` / `lampOff` | 10 → 15 | the insert row's LED | on: dot r 2.5 + halo r 4.5 at 18 %, `play`; bypassed: a 1-px ring, ink 4 |
| `grip` | 12 → 18 | insert row under the pointer | 2 × 3 dots of 2 × 2 px, ink 3 |
| `miniView` | 12, 16 → 18, 24 | insert row under the pointer (the 16 × 16 key) | a window with its title line |
| `more` | 12, 16 → 18, 24 | the inspector head's more key | three 2 × 2 dots; 24: circles r 1.5 |
| `add` | 10, 12 → 15, 18 | "Add an effect", "Add a send", the empty insert row | a 1-px plus |
| `foldOpen` / `foldShut` | 8, 10 → 12, 15 | section titles (open: down; folded: right), ink 4 / ink 3 | pixel chevrons |

**Not glyphs:** M, S and the strip's numbers are type (Barlow Semi Condensed SemiBold 9–10,
des-mixer's spec). "Silenced by a solo", the meter, the clip cell and the fader's tick are
des-mixer's drawing.

## Where the half pixel goes

Some even boxes hold glyphs of odd width: the power stem, the snowflake's centre, the VCA's
slots, `add` at 10 and 12. These sit 0.5 px right of and below the box's centre, never
blurred across two pixels. Place the box on whole pixels and the glyph lands as drawn.
