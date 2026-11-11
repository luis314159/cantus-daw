# Logo: #10 enriched, four variants (round 39, `logo`)

**The brief (owner, 2026-11-09).** "Haz una versión con más detalle del 10, con esos colores,
pero con un estilo con más detalles y brillos metálicos como el 03, un poco más medieval."
During the work he added: **"que la C sea continua, no en rayitas radiales."** So in every
variant and at every size, the C is one continuous gilded letter. #10's radial ticks are kept
only as ornament, behind or inside the C.

The variants keep #10's palette: the navy enamel (`#16295E`), the silver-blue (`#C9D3E6`) and
the gold cross (`#E2B84A`). From #03 they take metal you can see. The C is a raised gilded
capital with its ends cut upright. Its face is polished, with a diagonal leaf sheen. Its two
bevels change shade as they go round, like real metal under one light from the upper left.
The cross, which doubles as the playhead, is cut on its straight skeleton: a ridge along each
arm, hips at the ends and valleys into the inner corners. The medieval feel comes from
metalwork and manuscript technique: basse-taille enamel, pearl borders, punchwork, a diaper
ground and an astrolabe's limb. There is no extra religious imagery; the cross is the only
symbol.

## The variants

| | Idea | Small marks (48 → 16) |
|---|---|---|
| **1 · Relicario** | A reliquary. The gilded C has an engraved line along its face. Under the navy enamel, rays are cut into the metal (basse-taille). It has a gold rim, a border of silver pearls and a pearl in a collet where the arms of the cross meet. | 48 and 32 keep the gold rim; 48 keeps the pearl. |
| **2 · Astrolabio** | An instrument. The C is the limb, with a scale engraved across its face (longer marks every 30°). Inside the counter is a silver dial that carries #10's waveform ticks, small, round a gilded pin. | 48 keeps the silver dial ring and pin; 32 keeps the pin. |
| **3 · Roseton** | The most medieval. The C has an uncial weight (heavy belly, light ends) and two rows of punchwork. Its counter holds a small rose window: eight lancets of navy glass in gilded frames round a rosette. The ground is a diaper with a dot in each lozenge, with gold pearls round it. The arms of the cross flare. | 48 keeps the rose ring and boss; 32 keeps the boss. |
| **4 · Grabado** | The least ornament: an even-weight gilded C, the cut-gold cross and one gold rim over faint engraved rings. It is the calmest beside the interface. | Just the C and the cross. |

At 24 and 16 px all four come down to the same strong silhouette. On the pixel grid it is a
gilded C, light at the upper left and darker at the lower right, with a two-tone cross: the
lit column and the shaded column. The variants still differ in the C's weight. The C reads
at 16 px; see the ×6 zoom on the sheets.

## Files

- `logo-variants-night.png` and `logo-variants-paper.png` are the sheets. Each row shows a
  variant's 1024 drawing (at 260 px), its 256-px drawing at 1:1, the 24-px mark in a Slate top
  bar, the 48/32/24/16 marks at 1x on the bar, and their pixels magnified. Today's #2 (drawn
  from its small master, as `logo_small.rs` draws it), #10 and #03 are below them.
- `svg/<variant>-{1024,512,256}.svg` are the app-icon drawings, each with the detail its size
  can hold. At 512 the pearls are larger and fewer, and the rays and the scale are coarser.
  At 256 there are no pearls, rays, diaper or punchwork, and every line is at least 2 px.
- `svg/<variant>-{48,32,24,16}.svg` are the small marks. They fill the whole square with the
  tile's radius at 0.22 n, as #2's small master does. The cross sits on whole pixels.
- `src/`: `geo.py` holds the geometry and metal shading, `large.py` the icon drawings,
  `small.py` the marks, `ref2.py` a port of #2's small master for the comparison, and
  `sheet.py` builds the sheets (`python src/sheet.py`, using Edge headless and PIL).

## For whoever ships one

- Every colour carries a `data-role` (`tile`, `wave` for the C and the silver, `playhead`
  for the cross and the gold ornament, `neutral` for shadows and highlights). The rule in
  `kit/logo.rs` (`every_colour_in_every_logo_has_a_role`) passes on all 28 files, checked
  with the same scan in Python.
- The files use only paths, polygons, linear and radial gradients, one clip path, the tile's
  drop shadow and one blurred cast shadow. There are no fonts, no raster images and no
  patterns. **Rendered only in Edge so far, not in `resvg`.** The app's renderer should
  agree, but the first `cargo run -p cantus-ui --example shoot` after porting must confirm
  it. The 1024 files are 55–122 KB (the bevels are drawn in thin slices).
- To adopt a variant, add the 1024 drawing as logo #11 (`assets/logos/11.svg`). Then make
  `svg_for` choose the size-matched file: 512/256 for 128–511 px, and the 48/32/24/16 marks
  below 48 px, as #2's small master does today. The marks can be ported to `logo_small.rs`
  as fixed SVG text; they need no maths.

---

# The owner's choice: #03, and an accent to go with it

The owner chose **#03 ("the metal")** as the default logo. The four #10 variants above stay on
record.

## #03 small masters (`m03/03-{16,20,24,30,32,48}.svg`, `src/m03.py`)

Today the app scales the full #03 drawing down. That keeps the drawing's 100-px margin and
its shadow, so the tile fills only 80 % of the square. The corner flourishes turn to noise,
the waveform becomes an orange smudge and the C gets thin. The small masters fix this:

- The tile fills the square with its corners at 0.22 n.
- The C is thicker: its stroke is 2.2 px at 16.
- The waveform is drawn as whole-pixel vermilion bars: five at 30/32/48, three at 24/20/16.
- The gold rim is kept from 30 px. The flourishes and the dot field are dropped.
- The lozenge terminals stay from 30 px, but centred on the stroke. At the outer corner, as
  in the large drawing, they read as arrowheads, and the C becomes a reload sign.
- 30 px is the top bar at 125 %.

Every colour has its role, and the bars take `patina`, as #03's do. To port: give #03 a
`small_master` like #2's, used below 64 px.

## Accent options (`accent-options-{night,paper}.png`, `accent/window-*.png`, `src/accent.py`)

Each window is dir-a's Slate, built by its own renderer. It is imported from
`F:\projects\daw-r39\dir-a\…\src`, not copied and not changed. Only the accent tokens differ,
and #03's small master is in the top bar. Lead is soloed, so each accent sits beside solo's
amber. There is a window per option at 1× and 1.25×, in Night and Paper.

| | Night | Paper | Verdict |
|---|---|---|---|
| **A · Lapis** | accent `#7ea2f6`, ink `#0b1430` | accent `#2443a8`, ink `#ffffff` | **Recommended** |
| **B · Gold of the C** | `#e2b84a` | `#7a5a14` | Rejected |
| **C · Lapis, gilt time** | A, plus playhead `#f2d892` | A, plus playhead `#6e5414` | Possible, with a rule |

- **A** reaches 4.5:1 or more on every surface it sits on, in both themes, including the
  glyph on its lit key: Night 4.6–7.1, Paper 4.6–8.4. Its nearest state colour is
  automation's violet, at ΔE00 16 in Night and 15 in Paper. Every other state colour is
  more than 40 away.
- **B** is solo's own amber: ΔE00 3 in Night and 12 in Paper. A lit loop or snap key would
  read as a soloed track. In Paper it also falls under 4.5:1 on the gutter and on its lit
  key. I rejected it rather than move solo, because the brief keeps the rest of Slate
  unchanged.
- **C** keeps A for every state and gives the playhead, its cap and the loop range a pale
  gilt. The gilt is at ΔE00 11 from solo in Night and 16 in Paper. That holds only if gold
  never fills a key, so gold stays a hairline and solo stays a filled key. In Paper the gilt
  has to darken to bronze to reach 4.5:1 on white.
- One more distance to note: Night lapis is ΔE00 5 from the Choir track's own colour
  (`#6c93ea`). Track colours belong to the user, and the accent never fills a track's
  things, so this is not a collision in use.

**Recommendation: A.** It is #03's own ground, made to read on Slate. It passes everywhere
and stays clear of every state colour.
