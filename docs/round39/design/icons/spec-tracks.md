# Spec for the kit — set 6, the track-icon library (316 icons, 17 families)

*The owner, 2026-11-09:* each track carries an icon of the user's choice beside its type
icon, so a track is easier to find. After his first review (the guitars read alike, a bass
guitar was missing) set 6 became an instrument library of 82 icons. After his second
("nos faltan címbalos… analiza cuáles faltan, 0 flojera, déjalo muy muy completo",
`set6-additions.md`) three designers added families in parallel (icons-a: cymbals and
percussion; icons-b: guitars, strings, winds, keys; icons-c: electronic, voice, notation,
Foley, mix, marks, letters), and a final pass (icons-final) made the 316 one family:
the same weights, optical size, inks and angles, the weak drawings redrawn, one spec.

This file replaces `spec-tracks-add-{a,b,c}.md`; their variant notes are kept below.

## Files

- **Drawings**, in load order (each later file may replace an earlier drawing):
  `glyphs-t-tracks.js`, `-tracks2.js`, `-tracks3.js` (set 6's constructions and the shared
  helpers `tk`, `track`, `sil`, `F`, `CUT`, `STK`, `lineW`, `cutW`), `-lib.js`, `-lib2.js`,
  `-lib3.js` (the first library), `-add-a.js` with `-add-a-names.js`, `-add-b.js`,
  `-add-c.js` (the additions), `-final.js` (the final pass: redraws, their variants, the
  chosen), `-families.js` (the picker's 17 families and every icon's words).
- **Sheets** (`sheet-tracks.html`, `picker.html`, rendered by `render.py`):
  - `set6-tracks-grouped-{night,paper}-{1,2,3}.png`: every family titled, every icon at the
    picker's 24 px shown at 125 % (30 device px) with its English and Spanish names; no
    scrolling. The page reports any label wider than its cell (none do).
  - `set6-tracks-{night,paper}-4x.png`: every icon at 12, 14, 16 and 24 px, 1:1, enlarged
    4× by nearest neighbour (every device pixel visible).
  - `set6-tracks-picker-{night,paper}-125.png`: the picker (search, None, Recent, the
    families), track headers and mixer strip heads at 125 %.
  - `sheet-tracks.html?review=a,b,c` draws any ids (variants too) at every size; it is how
    the variants below were compared.
- The additions' own review sheets (`set6-add-{a,b,c}-*.png`) are kept for the record; they
  show the drawings before the final pass.
- Private comparison: `research/ui/round39/icons/vs-daws-tracks-*.png`.

## How these are drawn

316 icons at 10 sizes (12, 14, 15, 16, 18, 20, 21, 24, 30, 36) is over 3 000 drawings, so
each icon is **built for each size** from one design on a 24-unit sheet (`track(id, t => …)`
with `tk(S)`). Nothing is scaled from a bitmap. The kit ports the constructions, not the PNGs.

- **Instruments and objects are silhouettes**, filled, as in Logic's track icons: a real
  thing seen flat, with the one feature that names it cut out (the surface shows through)
  or drawn in the second ink — a guitar's soundhole and bridge, a violin's f-holes, a
  drum's head, the cassette's reels, the lock's keyhole.
- **Signals are lines** on whole or half pixels: the wave shapes, the synth roles (synth,
  lead, pluck), the compressor's curve, EQ, filter, riser and downlifter, the plain shapes.
- **One weight per size** (final pass, in the shared helpers `lineW` and `cutW`):
  - a drawn line (stroke in ink 1 or ink 2: stands, rods, strings, sparks, outlines) is
    **1 device px below 30 and 2 px at 30 and 36**;
  - a cut (the surface showing through a silhouette) is **1 px at every size**;
  - a width under 1 is a device pixel already divided by an inner group's scale and keeps
    that ratio; a width over 1.6 is a mass (a tambourine's ring, a halo that lifts a stick
    off a drum), kept as drawn and rounded to whole pixels below 30;
  - the additions had used 1.2–1.4 px lines (soft edges at 12–16 px) and 1-px outlines at
    30 px (half the weight of set 6's lines there); both now follow the rule.
- **Optical size.** An icon fills the 24-unit sheet to about 21–23 units on its long side,
  centred on its mass, not its box. Thin objects (flutes, sticks) are drawn thicker than
  life so they keep 2 px at 14 px.
- **Inks.** Ink 1 is the object; ink 2 (the second ink) is what lies behind or is lesser:
  a drum's head, a stand, the bowl behind a vihuela, the second violin of a section.
  Never two different objects in ink 1 that touch: the one in front is lifted by a 1-px cut
  halo (sticks over a snare, the espiga over a requinto).
- **Angles.** Guitars and the plucked instruments that lie down (guitars, the guitar
  shapes, banjo, mandolin, oud, lute, bouzouki, the Mexican and Latin strings) lie at
  **45°, body forward**: body at the lower left, head at the upper right. The bass
  guitars, the violin family, ukulele, sitar, guitarrón and bajo sexto stand upright.
  Flutes lie at −28°. Orientation separates kin.
- **Detail by size.** D0 at 12–15 px keeps the silhouette; D1 at 16–21 px adds cuts and
  inner lines; D2 at 24 px and up has everything.
- **Pixel-exact where symmetry matters:** the cross, the step sequencer, the drum pads, the
  glitch's bands and the letter badges are placed on whole pixels per size, centred.
- **Letters & numbers** are badges with the character cut out, from bitmaps (`SMALL`,
  `LARGE`, `PLAN` in `glyphs-t-add-c.js`): a 5 × 7 face at 12–16 px, 7 × 11 at 18–24 px, the
  5 × 7 face doubled at 30 and 36; the badge is 11, 13, 13, 13, 15, 17, 17, 19, 24, 28 px.
- **The cross** is a Latin cross, the beam a quarter of the way down and 0.64 of the height
  wide, offered as a mark like any other.

## Legal care: guitar shapes and makers' names

The electric shapes are **generic stylised outlines** drawn from blank sheets, none traced
from a photograph or a maker's drawing; the headstock is one generic paddle for all of them.
The V body has rounded wing tips and a shallow notch; the pointed body is a four-point star
of straight edges. **Labels are generic** (double cutaway, single cutaway slab, carved single
cut, twin-horn solid body, pointed double cut, semi-hollow, offset body, V body, pointed
body). Model and maker names (strat, tele, les paul, sg, 335, superstrat, jazzmaster,
explorer, flying v, hammond, rhodes, wurlitzer, mpc, 303) appear **only as search aliases**
in the last column, never as labels. `families.js` sets the generic labels; the sheet
generator checks that no label carries one of these names.

## The final pass (icons-final)

**Consistency, across all 316:**
- the line and cut weights above (the helpers changed, so every construction follows);
- the 45° convention: the first electric and acoustic guitars, banjo, mandolin, oud, lute
  and bouzouki lay at 40° and were turned to 45°, as the additions' guitars;
- the first electric and acoustic guitars were drawn smaller than the additions' guitar
  shapes; they are now built with the shapes' construction (`bShape`), at their size;
- the drums: set 6's snare and tom were outlines, the only drums not drawn as silhouettes;
  they are silhouettes now, built on the additions' `tomShell`;
- the cymbals: set 6's hi-hat and cymbal were small and thin beside the additions' cymbals;
  both are redrawn with the same cymbal construction (`CYM`) and head the Cymbals family;
- the picker drew icons with the surface colour as their cut on its raised panel (a lighter
  ring round every cut); the cut is now the colour behind the icon (raised, hover, selected).

**Redrawn** (variants in `glyphs-t-final.js`, compared at 12, 14, 16 and 24 px, 1:1 and 4×,
with `sheet-tracks.html?review=…`; `TRACK_VARIANTS_FINAL` lists them):

| Icon | Variants | Chosen, and why |
|---|---|---|
| `eSingleCut` | eSingleCutA · **eSingleCutB** · eSingleCutC · eSingleCutD | The owner found it odd and ugly: a round lower bout, a clear waist, one cutaway leaving a short pointed horn on the treble side; B's horn reads at 14 px where A's and D's melt into the shoulder, C's binding line read as a crack. |
| `eOffset` | eOffsetA · eOffsetB · eOffsetC · **eOffsetD** | The owner found it odd and ugly (it was a double cutaway turned 13°): the bass horn long and broad, the treble side only a shoulder, the waist high on the bass side and low on the treble side; A–C still read as the double cutaway. |
| `snare` | **snareA** · snareB | A silhouette like every other drum; A (sticks crossed over a shallow shell) over B (the snares under it, which 14 px loses). |
| `tom` | **tomA** | The outline tom made a silhouette with its lugs. |
| `tambourine` | **tambourineA** · tambourineB | The ring read as the circle mark; the half-moon with jingles, tilted, over the upright one (a fan). |
| `hihat` | **hihatA** | The instrument on its stand with tripod and pedal, apart from the closed and open articulations. |
| `cymbal` | **cymbalA** | One level cymbal on a tripod at the size of the other cymbals (set 6 had a thin tilted bar). |
| `hihatOpen` | **hihatOpenA** | The top cymbal lifted further and tilted. |
| `cymbalStack` | **cymbalStackA** | A small cymbal on a big one with its wing nut — read as the open hi-hat before. |
| `cuica` | cuicaA · cuicaB · **cuicaC** | The drum seen from its head with the stick out of the open back; A read as a lantern, B as a can. |
| `vibraslap` | vibraslapA · **vibraslapB** | The U with box and ball, larger and with a heavier rod; the J lay flat at 14 px. |
| `crotales` | crotalesA · crotalesB · crotalesC · **crotalesD** | The set from above, two rows like a keyboard; side views were thin smears at 14 px. |
| `acid` | acidA · **acidB** | Knobs over the squelch: a box with keys read as every other box. |
| `theremin` | thereminA · **thereminB** | Below 16 px the instrument (cabinet, rod, loop); the hand by the rod from 16 px, where it reads. |
| `talkbox` | **talkboxA** · talkboxB | Lips twice as large, the tube out of their corner. |
| `vocalChop` | **vocalChopA** | A microphone head cut in three; the chopped lips read as a ball. |
| `surround` | **surroundA** · surroundB | Five speakers and the listener, the ring only from 16 px. |
| `aDread` | **aDreadA** | Square shoulders and a shallow waist. |
| `aConcert` | **aConcertA** | A deep waist under a small upper bout. |
| `aCutaway` | **aCutawayA** | The dreadnought with its treble shoulder scooped to the waist. |
| `aguitar` | **aguitarC** | At the shapes' size and 45°, with the steel-string's pickguard. |
| `eguitar` | **eguitarC** | At the shapes' size and 45°, with its cable from 16 px. |
| `jarana` | **jaranaA** | Slim, pinched, a wide head of eight strings. |
| `cuatro` | **cuatroC** | The Puerto Rican corners pushed out. |
| `requinto` | **requintoD** | The espiga across the body in the main ink. |
| `guzheng` | **guzhengC** | From the side without its stand (it read as a table): arch, strings, slanted bridges. |
| `euphonium` | **euphoniumA** | A small tuba with its bell to the side and the valve block (it read as a saxophone). |
| `piccolo` | piccoloA · **piccoloB** | A thicker short flute with the spark of its octave; alone it was a dash at 14 px. |
| `clavinet` | **clavinetA** · clavinetB | From above: strings and two pickups over the keys, on legs. |

The owner's two electric shapes are on their own sheet, before and after, beside the
double cutaway and the slab single cutaway: `guitars-before-after-{night,paper}-4x.png`
(12, 14, 16, 24, 36 px at 1:1, then 16 and 24 px in a track's colour, all enlarged 4×).

## The guitar family, v3 (icons-guitars)

The owner rejected the guitars twice ("aún dejan mucho que desear las guitarras"). The whole
family — guitars and basses, the electric and acoustic shapes, the Mexican and Latin strings —
was redrawn after a study of photographs (`guitars-study.md`). **This replaces the 45° rule
above for these icons:**
- the shapes, the archtop, resonator, basses and Mexican and Latin strings stand **upright**,
  body forward; the headline electric and acoustic guitars and the baritone keep **45°**;
- every coordinate is snapped to the size's pixels, and at **12, 14, 15 and 16 px** each
  icon is **pixel rows** (family.js's notation), fitted by `fit.py` and redrawn by hand in
  `glyphs-t-guitars-px-fix*.js`; the kit ports those rows for those sizes and the snapped
  constructions from 18 px.
- Files, in load order after `glyphs-t-final.js`: `glyphs-t-guitars.js` (helpers),
  `-e.js` (electric shapes, archtop), `-a.js` (acoustic shapes, resonator, basses, the 45°
  guitars), `-mx.js` (Mexican and Latin strings), `-pick.js` (the chosen, `GUITAR_PICK`, with
  why), `-px.js` (generated), `-px-fix.js` … `-px-fix5.js` (by hand). Sheets:
  `sheet-guitars.html` (`mode=set|ba|variants|fit|photo`), `guitars-v3-*.png`.
- The table above keeps the earlier choices for the record; for these ids `GUITAR_PICK` wins.

## Variants chosen by the additions' designers

Kept from their specs. Where the final pass redrew an icon (table above), the final pass wins.

### Cymbals and percussion (icons-a)

| Icon | Variants | Chosen, and why |
|---|---|---|
| brushes | one brush · a crossed pair | **One brush**, leaning. The pair read as a whisk or a bouquet at 14 px. |
| electronic drums | hexagonal pads · round pads with the module | **Hexagonal pads**. Round pads read as a drum kit; the hexagons say "electronic" at every size. |
| castanets | the shells open on their hinge · the pair face on | **Face on**, the front shell showing its hollow (second ink). The open pair read as a Pac-Man; without the hollow the pair read as cherries. |
| marimba | with its mallets · the bare instrument | **With mallets**. Bare, it is too close to the vibraphone. The legs, the filled graduated resonators and the mallets set it apart. |
| jawbone | from the side · the arch of teeth from above | **From the side**, leaning, the molars standing apart. A first side drawing with a flat underside and forward incisors read as a shoe; the arch read as a "U" magnet. |
| cabasa | a grid of beads · bead chains round the core | **The chains**. The grid read as a grater. |
| darbuka | upright · lying as it is held | **Upright**. The lying one read as a mushroom. Upright, the bowl wider than its head separates it from the djembe, whose head is its widest part. |
| steel pan | the drum, its notes, the stick · the notes seen from above | **The drum and stick**. From above, the petals read as a flower or a wheel. |
| gamelan gong | the bossed gong hung in its carved frame · a bonang row of kettle gongs | **The kettles**. The hung gong is too close to the existing `gong`. |
| crotales | discs in two rows on their board · two thick discs on a post | **The board**. The post read as one more cymbal stand among the cymbals. |

Other choices made by looking:
- **Cymbals:**
  - The cymbals are filled cones with a bell and a flat underside, like the existing
    `hihat` and `cymbal`, with a stand foot from 16 px.
  - Crash, ride, splash and china are told apart by how each is played and how it is shaped.
    The crash is tilted steeply, with sparks off its high edge. The ride is the largest
    and flattest, with a big bell and the stick's tip on it. The splash is small, with
    splash marks all round. The china is upside down, its rim flanged up.
  - Hi-hat open and closed read apart down to 12 px. Open has the top cymbal lifted and
    tilted. Closed has the two shut, with a hairline between them. Stacked cymbals form a
    bow-tie: a small cymbal turned over on a big one, bell to bell.
- **Kick pedal:** drawn, because it is distinct from the kick. It is seen from the side:
  the footboard, the post, and the beater on its shaft.
- **Taiko:** the sticks were taken off; above the barrel they read as a television's
  antennae.

### Guitars, strings, winds, keys (icons-b)

- **Electric shapes:**
  - T-type: redrawn with a straight ledge where the treble side meets the neck. At 14 px
    the first T and the single-cut read alike.
  - Single-cut: redrawn with a taller, narrower horn, for the same reason.
  - Offset: the S body turned 13° against the neck, which is what an offset body looks
    like. The first offset outline read as the S-type.
- **Dreadnought:** redrawn with squarer shoulders, so it does not read as the concert/OM.
- **Classical:** B (a slotted head and a two-tone rosette), chosen over A (the slotted head
  only).
- **12-string:** B (a long, wide head), chosen over A. A has pegs drawn as ticks, and they
  made a star at 24 px.
- **Resonator:** A (the cover plate in the second ink, ringed by a cut), chosen over B
  (cut rings), which was faint at 14 px.
- **Lap steel:** A (level), chosen over B (tilted −16°, busy).
- **Fretless bass:** A (the smooth fingerboard in the second ink, four pegs on one side).
  It was chosen over B (an outlined neck), which was too thin.
- **Requinto:** A (the slender body with the espiga, its long horn plectrum, in the second
  ink at the upper left). It was chosen over:
  - B, the espiga at the lower right;
  - C, the body alone, which reads as any small guitar.
- **Vihuela:** B (the bowl back larger behind the top), chosen over A, whose bowl hardly
  showed.
- **Charango:** A (three shell bands at 14/16 px, five at 24 px), chosen over B (two wide
  bands).
- **Cuatro:** A (the Puerto Rican body with violin-like corners), chosen over B (the
  Venezuelan small guitar, which reads as the travel guitar).
- **Tres:** A (three string lines over the body, no soundhole), chosen over B (a soundhole
  with three lines under it).
- **Viola:** A (the violin tilted to the shoulder), chosen over B (with a bow, busy).
- **Koto:** B (the bridges corner to corner), chosen over the first koto, whose bridges
  were too flat and read as a harmonica.
- **Guzheng:** A (side view: the arched board, its bridges and strings, on a stand), chosen
  over B (top view), which read as the koto.
- **Shamisen:** A (the square drum body, the long neck, three pegs), chosen over B (with
  the bachi), which merged into the body at 14 px.
- **Hammered dulcimer:** B (the hammers lie across the board, lifted by a cut), chosen over
  A, whose hammers below the board read as table legs.
- **Bass clarinet:** B (a straight body, the bell kicking forward), chosen over A (a J),
  which read as a small saxophone.
- **Bassoon:** redrawn twice. The first, a single pole with its bocal, read as a stick
  figure. It is now the twin tube: the long joint and the wing joint in the second ink, the
  crook on top of the wing joint, the boot at the foot.
- **Bandoneón:** A (square ends studded with buttons, straight bellows), chosen over B
  (opened in a V, busy).
- **Harpsichord:** A (from above: a tall wing with straight sides, the keyboard at its
  foot), chosen over B (from the side with the lid raised), which read as a table.
- **Clavinet:** its legs removed, so it does not read as the electric piano on its stand.
- **Toy piano:** A (a little arched upright with chunky keys and bun feet), chosen over B (a
  little grand), which read as the grand piano.
- **Keytar:** B (tilted −32°), chosen over A (−14°).

### Electronic, voice, notation, Foley, mix, marks (icons-c)

- **Treble clef:** one monoline construction (tail ball, spine past the top, the curl and
  the spiral round the G line), 1.25 px below 30, 2 px from 30. It holds at 14 px as a clef
  and is clean from 16 px.
- **C clef:** the heavy bar, the hairline and two bows meeting at the middle line, each
  ending in its ball.
- **Theremin:** a cabinet on two legs with the rod and loop read as an animal; a box on a
  single stand read as a cocktail table; a box with a rod and waves read as a router. The
  chosen one is **the hand playing in the air beside the rod** on its cabinet.
- **CDJ:** the first (a jog under a screen, upright) read as a studio monitor; it is now
  square with the pitch fader at the right.
- **Cassette:** reels and the head opening read as a sad face at 14 px; the reels now sit in
  their window and the opening is drawn only from 24 px.
- **Rest:** a quarter rest, chosen over a whole rest (a block under a line read as a
  button).
- **Harmony:** two parallel melodic lines read as chevrons; a three-note chord merged into a
  blob; chosen: **two notes on one stem, ringing**.
- **Vocal chop:** lips cut into slipped slices read as a spaceship; clip blocks read as
  chips; chosen: **the lips, sliced** by two cuts.
- **Field recording:** two capsules in a V read as rabbit ears; now the recorder's guarded
  XY head.
- **Sheet music:** notes over the staff read as "≠"; now one note on a three-line staff.
- **Chorus** drops its third voice below 16 px; **surround** keeps its ring at every size
  so the five speakers hang together.

## One drawing in two families, and old ids

- `bass` (set 6's bass clef, "bass (synth)") is listed in Electronic & production and in
  Notation, where it is labelled "bass clef / clave de fa" (`TRACK_LABEL_IN`).
- `metronomeTrack` is in Notation and Mix & routing; `fire` in Sound design & Foley and in
  Marks & shapes (labelled "flame / llama" there); `bell` in Sound design & Foley and in
  Marks & shapes ("bell / campana" there). The library counts each once: **316 icons**.
- `brass`, `woodwind` and `strings` stay as aliases of `trumpet`, `sax` and `violin` for any
  project saved before the library; the picker does not show them. The shape `triangle` and
  the instrument `triangleInst` are separate.
- Words that now name their own icon left the older icon's aliases: "viola" (violin),
  "bandoneón" (accordion), "marimba" (xylophone), "bongos" (congas), "crash, ride" (cymbal),
  "backing vocals" (choir), "sweep" (riser), "pads, mpc" (sampler).

## The icons (316): the picker's families, names in English and Spanish

Search matches the English name, the Spanish name and the other words, without accents or
case. Families in the owner's order (`set6-additions.md`, "The picker"); Guitars & basses
has two sub-rows, Electric shapes and Acoustic shapes.

### 1. Drums (15)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `kick` | kick | bombo | bass drum, bd |
| `snare` | snare | caja | redoblante, sd |
| `tom` | tom | tom | timbal de batería |
| `rackTom` | rack tom | tom aéreo | high tom, tom alto, tom de rack |
| `floorTom` | floor tom | tom de piso | low tom, timbal base, goliat |
| `drumkit` | drum kit | batería | drums, kit |
| `edrums` | electronic drums | batería electrónica | e-drums, pads, electronic kit |
| `sideStick` | side-stick | aro | rim, rimshot, cross-stick, golpe de aro |
| `sticks` | drumsticks | baquetas | sticks, palillos, palos |
| `brushes` | brushes | escobillas | brochas, cepillos, jazz |
| `kickPedal` | kick pedal | pedal de bombo | bass drum pedal, beater, mazo |
| `clap` | clap | palmas | aplauso, hand |
| `shaker` | shaker | maracas | shaker, huevo |
| `tambourine` | tambourine | pandereta | pandero |
| `triangleInst` | triangle | triángulo | instrument, instrumento, ting |

### 2. Cymbals (9)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `hihat` | hi-hat | charles | hat, hh, contratiempo, hi hat |
| `cymbal` | cymbal | platillo | cymbals, platillos, címbalo, plato |
| `hihatClosed` | hi-hat closed | charles cerrado | closed hat, ch, contratiempo cerrado |
| `hihatOpen` | hi-hat open | charles abierto | open hat, oh, contratiempo abierto |
| `crash` | crash | platillo crash | crash cymbal, choque |
| `ride` | ride | platillo ride | ride cymbal, campana, bell |
| `splash` | splash | platillo splash | splash cymbal |
| `china` | china | platillo china | china cymbal, trash |
| `cymbalStack` | stacked cymbals | platillos apilados | stack, efecto |

### 3. Latin & Mexican percussion (18)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `congas` | congas | congas | conga, tumbadora, quinto, tumba |
| `bongos` | bongos | bongós | bongo, bongó |
| `timbales` | timbales | timbales latinos | pailas, timbaletas, paila |
| `cajon` | cajón | cajón | box drum |
| `cowbell` | cowbell | cencerro | campana de vaca |
| `guiro` | güiro | güiro | guiro, guayo, rascador, scraper |
| `claves` | claves | claves | clave, palitos |
| `quijada` | jawbone | quijada | quijada de burro, charrasca |
| `cabasa` | cabasa | cabasa | afoxé, afuche, afoxe |
| `shekere` | shekere | chekeré | shekeré, agbe, güiro de cuentas |
| `agogo` | agogô | agogó | agogo, campana doble |
| `cuica` | cuíca | cuíca | cuica, puerca, zambomba |
| `surdo` | surdo | surdo | bombo brasileño, samba |
| `pandeiro` | pandeiro | pandeiro | pandero brasileño |
| `woodblock` | woodblock | caja china | wood block, bloque de madera, temple block |
| `castanets` | castanets | castañuelas | palillos |
| `vibraslap` | vibraslap | vibraslap | quijada (efecto), vibraeslap |
| `marimba` | marimba | marimba | marimba mexicana, marimba de Chiapas, tuned |

### 4. World percussion (12)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `djembe` | djembe | yembé | djembé, jembe |
| `darbuka` | darbuka | darbuka | doumbek, derbake, tabla árabe, goblet drum |
| `tabla` | tabla | tabla | tablas, bayan, dayan |
| `frameDrum` | frame drum | pandero de marco | bodhrán, bendir, daf, tar |
| `taiko` | taiko | taiko | tambor japonés, wadaiko |
| `handpan` | handpan | handpan | hang, pantam, tuned |
| `steelpan` | steel pan | tambor metálico | steel drum, steelpan, pan, tuned |
| `kalimba` | kalimba | kalimba | mbira, sanza, piano de pulgar, thumb piano, tuned |
| `rainstick` | rainstick | palo de lluvia | palo de agua, rain stick |
| `windChimes` | wind chimes | carillón de viento | mark tree, chimes, campanas de viento |
| `gong` | gong | gong | tam-tam |
| `gamelanGong` | gamelan gong | gong de gamelán | gamelan, bonang, gong ageng, kempul, tuned |

### 5. Tuned percussion (8)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `xylophone` | xylophone | xilófono | xylo, mallets, láminas |
| `vibraphone` | vibraphone | vibráfono | vibes |
| `glockenspiel` | glockenspiel | glockenspiel | lira, metalófono, carillón |
| `tubularBells` | tubular bells | campanas tubulares | chimes, orchestral bells, tuned |
| `crotales` | crotales | crótalos | antique cymbals, platillos antiguos, tuned |
| `bells` | bells | campanas | handbells, campanillas |
| `timpani` | timpani | timbales sinfónicos | kettle drum, tímpanos, timbal de orquesta |
| `musicbox` | music box | caja de música |  |

### 6. Guitars & basses (30)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `eguitar` | electric guitar | guitarra eléctrica | guitar |
| `aguitar` | acoustic guitar | guitarra acústica | guitarra, criolla |
| `bassguitar` | bass guitar | bajo eléctrico | electric bass, bajo |
| `archtop` | archtop guitar | guitarra archtop | jazz guitar, hollow body, guitarra de jazz, hueca |
| `resonator` | resonator guitar | guitarra resonadora | dobro, resophonic, resofónica, slide |
| `lapsteel` | lap steel | lap steel | steel guitar, hawaiian, guitarra hawaiana, slide |
| `pedalsteel` | pedal steel | pedal steel | steel guitar, country, guitarra de pedales |
| `baritone` | baritone guitar | guitarra barítono | baritone, barítono |
| `fretless` | fretless bass | bajo sin trastes | fretless, bajo fretless |
| `acousticbass` | acoustic bass guitar | bajo acústico | acoustic bass, bajo electroacústico |
| `guitaramp` | guitar amp | amplificador de guitarra | amp, combo, ampli |
| `bassamp` | bass amp | amplificador de bajo | amp, cabinet, stack, ampli, gabinete |
| `pedal` | guitar pedal | pedal de efectos | stompbox, effects pedal, pedalera, distortion, distorsión |
| | **Electric shapes** | | |
| `eStype` | double cutaway | doble cutaway | strat, s-type, tipo S, double cutaway, doble corte |
| `eTtype` | single cutaway slab | cutaway simple plana | tele, t-type, tipo T, single cutaway, un corte |
| `eSingleCut` | carved single cut | tallada, un corte | les paul, lp, single cut, tapa tallada |
| `eDoubleHorn` | twin-horn solid body | sólida de dos cuernos | sg, horns, cuernos, devil horns |
| `eSuperstrat` | pointed double cut | doble corte afilado | superstrat, shred, metal, pointed horns, cuernos puntiagudos |
| `eSemiHollow` | semi-hollow | semihueca | 335, es-335, f-holes, efes, semiacústica |
| `eOffset` | offset body | cuerpo offset | jazzmaster, jaguar, offset, asimétrica, surf, indie |
| `eVshape` | V body | cuerpo en V | flying v, v, metal |
| `ePointed` | pointed body | cuerpo puntiagudo | explorer, warlock, pointy, metal, picos |
| | **Acoustic shapes** | | |
| `aDread` | dreadnought | dreadnought | dread, western, folk, acústica |
| `aConcert` | concert / OM | concierto / OM | orchestra model, om, auditorium, auditorio |
| `aCutaway` | cutaway dreadnought | dreadnought con corte | cutaway, electroacústica, dreadnought con cutaway |
| `aJumbo` | jumbo | jumbo | big body, cuerpo grande |
| `aParlor` | parlor | parlor | parlour, salón, blues, pequeña |
| `classical` | classical guitar | guitarra clásica | nylon, spanish guitar, guitarra española, criolla, flamenco |
| `guitar12` | 12-string guitar | guitarra de 12 cuerdas | twelve string, doce cuerdas, 12 string |
| `aTravel` | travel / mini guitar | guitarra de viaje | mini, 3/4, baby, niño, small |

### 7. Mexican & Latin strings (8)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `requinto` | requinto | requinto | requinto jarocho, espiga, son jarocho, requinto romántico, trío |
| `vihuela` | vihuela | vihuela | mariachi, vihuela mexicana |
| `guitarron` | guitarrón | guitarrón | guitarron, mariachi, bass, bajo |
| `bajosexto` | bajo sexto | bajo sexto | bajo quinto, norteño, tejano, conjunto, 12 string bass |
| `jarana` | jarana | jarana | jarana jarocha, son jarocho, huasteca |
| `charango` | charango | charango | andean, andino, quirquincho, armadillo |
| `cuatro` | cuatro | cuatro | cuatro puertorriqueño, cuatro venezolano, llanero |
| `tres` | tres (Cuban) | tres cubano | tres, son cubano |

### 8. Strings (23)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `violin` | violin | violín | fiddle, violino |
| `viola` | viola | viola | alto, viola da braccio |
| `cello` | cello | violonchelo | chelo |
| `doubleBass` | double bass | contrabajo | upright bass |
| `stringsection` | string section | sección de cuerdas | strings, ensemble, orchestra, orquesta, cuerdas |
| `harp` | harp | arpa |  |
| `banjo` | banjo | banjo |  |
| `mandolin` | mandolin | mandolina |  |
| `ukulele` | ukulele | ukelele | uke |
| `sitar` | sitar | sitar |  |
| `erhu` | erhu | erhu | chinese fiddle, violín chino, huqin |
| `koto` | koto | koto | japanese zither, cítara japonesa |
| `guzheng` | guzheng | guzheng | zheng, chinese zither, cítara china |
| `shamisen` | shamisen | shamisen | sanshin, japanese, japonés |
| `pipa` | pipa | pipa | chinese lute, laúd chino |
| `oud` | oud | laúd árabe | ud, oud, arabic, árabe |
| `lute` | lute | laúd | renaissance lute, renacentista, barroco |
| `bouzouki` | bouzouki | buzuki | irish, greek, griego, irlandés |
| `balalaika` | balalaika | balalaica | russian, rusa |
| `hdulcimer` | hammered dulcimer | salterio | santur, santoor, cimbalom, yangqin, dulcémele |
| `mdulcimer` | mountain dulcimer | dulcémele apalache | appalachian, lap dulcimer, apalaches |
| `zither` | zither | cítara | concert zither, autoharp, autoarpa |
| `hurdygurdy` | hurdy-gurdy | zanfona | zanfoña, viella, organistrum |

### 9. Winds & brass (31)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `sax` | saxophone | saxofón | sax |
| `sopranosax` | soprano sax | saxofón soprano | straight sax, saxo recto |
| `trumpet` | trumpet | trompeta | brass |
| `cornet` | cornet | corneta | cornetín |
| `flugelhorn` | flugelhorn | fliscorno | flügelhorn, fiscorno |
| `trombone` | trombone | trombón | brass |
| `horn` | French horn | trompa | corno, horn |
| `euphonium` | euphonium | bombardino | eufonio, baritone horn, barítono |
| `tuba` | tuba | tuba |  |
| `sousaphone` | sousaphone | sousafón | helicón, tuba de banda, banda sinaloense |
| `hornsection` | horn section | sección de metales | brass section, metales, horns, vientos |
| `flute` | flute | flauta | traversa |
| `piccolo` | piccolo | flautín | octavín |
| `clarinet` | clarinet | clarinete |  |
| `bassclarinet` | bass clarinet | clarinete bajo | clarón |
| `oboe` | oboe | oboe |  |
| `englishhorn` | English horn | corno inglés | cor anglais |
| `bassoon` | bassoon | fagot | fagotto |
| `woodwindsection` | woodwind section | sección de maderas | winds, maderas, vientos |
| `recorder` | recorder | flauta dulce | flauta de pico, block flute |
| `tinwhistle` | tin whistle | flauta irlandesa | penny whistle, whistle, silbato irlandés |
| `panflute` | pan flute | zampoña | flauta de pan, siku, antara, panpipes |
| `shakuhachi` | shakuhachi | shakuhachi | bamboo flute, flauta de bambú |
| `ocarina` | ocarina | ocarina |  |
| `didgeridoo` | didgeridoo | didgeridú | didjeridu, yidaki |
| `bagpipes` | bagpipes | gaita | gaita gallega, highland pipes, dudelsack |
| `harmonica` | harmonica | armónica | blues harp |
| `melodica` | melodica | melódica | pianica, armónica de teclado |
| `kazoo` | kazoo | kazoo | mirlitón |
| `accordion` | accordion | acordeón | acordeon, squeezebox |
| `bandoneon` | bandoneon | bandoneón | tango, concertina |

### 10. Keys (16)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `piano` | grand piano | piano de cola | piano |
| `upright` | upright piano | piano vertical | piano |
| `harpsichord` | harpsichord | clavecín | clave, cembalo, clavicémbalo |
| `celesta` | celesta | celesta | celeste |
| `toypiano` | toy piano | piano de juguete | toy, juguete, kids |
| `organ` | organ | órgano | pipes |
| `tonewheel` | tonewheel organ | órgano de ruedas | drawbars, hammond, b3, órgano de ruedas fónicas, órgano eléctrico, jazz organ |
| `epiano` | electric piano | piano eléctrico | e-piano, ep |
| `tinepiano` | tine electric piano | piano de púas | rhodes, suitcase, ep, tines, e-piano, piano eléctrico de púas |
| `reedpiano` | reed electric piano | piano de lengüetas | wurlitzer, wurli, ep, reeds, e-piano, piano eléctrico de lengüetas |
| `clavinet` | clavinet | clavinet | clavi, funk keys |
| `mellotron` | mellotron / tape keys | teclado de cintas | mellotron, tape, cintas |
| `synthkeys` | synth keyboard | teclado sintetizador | synth, keyboard |
| `midikeyboard` | MIDI controller | controlador MIDI | midi keyboard, teclado midi, controller, pads |
| `keytar` | keytar | keytar | shoulder keyboard, teclado colgante |
| `keys` | keys | teclas | keyboard, teclado |

### 11. Electronic & production (33)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `synth` | synth | sintetizador | waveform, onda |
| `pad` | pad | pad | colchón |
| `lead` | lead | lead | melodía |
| `pluck` | pluck | pluck | pulsado |
| `arp` | arp | arpegio | arpeggio |
| `bass` | bass (synth) | bajo (sinte) | bass clef, clave de fa, fa, F clef, clef, clave, synth bass |
| `sub` | sub | sub | subwoofer, sub bass |
| `boom808` | 808 boom | 808 | sub kick, boom |
| `acid` | acid bassline | bajo acid | acid, 303, squelch, bassline |
| `drummachine` | drum machine | caja de ritmos | rhythm machine, ritmos, beat machine |
| `sampler` | sampler | sampler | sample, muestreador, sampling |
| `padgrid` | drum pads | pads de batería | pads, mpc, finger drumming, pad grid |
| `groovebox` | groovebox | groovebox | caja de ritmos, all-in-one, portable |
| `stepseq` | step sequencer | secuenciador por pasos | steps, pasos, pattern, patrón, sequencer |
| `modular` | modular synth | sintetizador modular | patch, eurorack |
| `theremin` | theremin | theremín | etherphone, termenvox |
| `vocoder` | vocoder | vocoder | robot voice, voz robótica |
| `talkbox` | talk box | talk box | talkbox, tubo, voice box |
| `turntable` | turntable | tocadiscos | dj, vinyl, vinilo |
| `djcontroller` | DJ controller | controlador DJ | dj, mixer, mezcladora, decks |
| `cdj` | media player | reproductor DJ | cdj, cd, player, jog |
| `laptop` | laptop | portátil | computer, ordenador, computadora, notebook |
| `midi` | MIDI | MIDI |  |
| `loop` | loop | bucle |  |
| `oneshot` | one-shot | muestra | sample |
| `cassette` | cassette | casete | tape, cinta, lo-fi |
| `radio` | radio | radio | transistor, am, fm, broadcast |
| `chiptune` | chiptune | chiptune | 8-bit, 8 bits, game, videojuego, gamepad, consola |
| `glitch` | glitch | glitch | error, fallo, broken |
| `waveSine` | sine wave | onda senoidal | sine, seno, sinusoidal, waveform |
| `waveSaw` | saw wave | onda de sierra | saw, sawtooth, sierra, diente de sierra |
| `waveSquare` | square wave | onda cuadrada | square, pulse, pulso |
| `waveTri` | triangle wave | onda triangular | triangle, tri |

### 12. Voice (14)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `mic` | vocal mic | micrófono | vocals, voz, mic, microphone |
| `leadVocal` | lead vocal | voz principal | lead, vocals, singer, cantante, voz líder |
| `voice` | voice | voz hablada | speech, diálogo |
| `choir` | choir | coro | chorus, ensemble, voces, coral |
| `backingVocals` | backing vocals | coros | bv, backing, voces de fondo, segundas voces |
| `harmony` | harmony | armonía | harmonies, voces, dueto, terceras |
| `rap` | rap / MC | rap / MC | mc, hip-hop, hip hop, rapero, verse |
| `vocalChop` | vocal chop | corte vocal | chop, chops, cortes de voz, slice |
| `adlib` | ad-lib | ad-lib | adlib, shout, grito, yeah |
| `whisper` | whisper | susurro | breath, aliento, quiet, soft |
| `beatbox` | beatbox | beatbox | mouth, boca |
| `whistle` | whistle | silbato | silbido |
| `podcast` | voice-over / podcast | locución / pódcast | vo, podcast, broadcast mic, narración, locutor |
| `talkback` | talkback | talkback | talk back, intercom, comunicación, control room |

### 13. Notation (14)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `trebleClef` | treble clef | clave de sol | sol, treble, G clef, clef, clave |
| `bass` (also above) | bass clef | clave de fa | bass clef, clave de fa, fa, F clef, clef, clave, synth bass |
| `cClef` | C clef | clave de do | do, alto, tenor, viola clef, C clef, clave |
| `percClef` | percussion clef | clave de percusión | neutral clef, drums, clef, clave |
| `quarterNote` | quarter note | negra | crotchet, nota, note |
| `eighthNotes` | eighth notes | corcheas | quavers, beamed, notas, notes |
| `wholeNote` | whole note | redonda | semibreve, nota, note |
| `rest` | rest | silencio | quarter rest, pausa, silencio de negra |
| `sharp` | sharp | sostenido | #, accidental, alteración |
| `flat` | flat | bemol | b, accidental, alteración |
| `fermata` | fermata | calderón | hold, pause, corona |
| `tuningFork` | tuning fork | diapasón | tuner, afinador, 440, a4 |
| `scoreSheet` | sheet music | partitura | score, notation, notación, sheet |
| `metronomeTrack` | click / metronome | clic / metrónomo | click, clic, metronome, metrónomo, tempo, guide |

### 14. Sound design & Foley (26)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `riser` | riser | subida | fx, uplifter, build-up, subida |
| `downlifter` | downlifter | bajada | fall, caída, fx, drop |
| `sweep` | sweep | barrido | chirp, filter sweep, fx |
| `whoosh` | whoosh | whoosh | swoosh, pass-by, paso, zumbido, swish |
| `impact` | impact | impacto | hit, golpe |
| `subDrop` | sub drop | sub drop | drop, boom, bajo, caída grave |
| `noise` | noise | ruido | white noise |
| `reverse` | reverse | reverso | reversed, al revés, rewind, reverse cymbal |
| `stutter` | stutter | tartamudeo | repeat, repetición, beat repeat, gate |
| `tapeStop` | tape stop | parada de cinta | slow down, frenado, power down |
| `vinyl` | vinyl crackle | crujido de vinilo | crackle, dust, lo-fi, polvo, vinilo |
| `scratch` | scratch | scratch | scratching, dj, rayado |
| `footsteps` | footsteps / Foley | pasos / Foley | foley, steps, pisadas, huellas |
| `ambience` | ambience | ambiente | atmos, room tone, atmósfera, paisaje, landscape |
| `rain` | rain | lluvia | storm, tormenta, weather |
| `wind` | wind | viento | air, aire, breeze, brisa |
| `water` | water | agua | drop, gota, liquid, líquido |
| `fire` | fire | fuego | flame, llama, hot |
| `birds` | birds | pájaros | bird, aves, nature, naturaleza |
| `crowd` | crowd / applause | público / aplausos | audience, applause, gente, multitud, aplauso |
| `siren` | siren | sirena | alarm, alarma, emergency, police |
| `explosion` | explosion | explosión | bomb, bomba, blast, boom |
| `phone` | phone / ringtone | teléfono / tono | ring, mobile, móvil, celular, ringtone |
| `fieldRec` | field recording | grabación de campo | recorder, grabadora, handy, field, location |
| `bell` | bell / notification | campana / aviso | notification, notificación, alert, alerta, ding |
| `video` | video | vídeo | film |

### 15. Mix & routing (24)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `bus` | bus | bus | group, grupo |
| `groupFolder` | group / folder | grupo / carpeta | group, folder, carpeta, stack |
| `aux` | aux | auxiliar | aux send, envío, send, effects return |
| `reverb` | reverb | reverberación | return, retorno |
| `delay` | delay | eco | retardo |
| `sidechain` | sidechain key | llave de sidechain | sidechain, key, ducking, clave |
| `compressor` | compressor | compresor | comp, dynamics, dinámica |
| `eq` | EQ | ecualizador | equalizer, eq, ecualización |
| `limiter` | limiter | limitador | brickwall, ceiling, techo, clip |
| `saturator` | saturator / distortion | saturador / distorsión | tube, válvula, drive, distortion, overdrive, warm |
| `filterFx` | filter | filtro | low-pass, lowpass, high-pass, resonance, resonancia |
| `chorus` | chorus | chorus | coro, ensemble, doubling |
| `phaser` | phaser / flanger | phaser / flanger | flanger, comb, modulation, modulación |
| `master` | master | máster | main |
| `mastering` | mastering | masterización | master, meter, vu, loudness, sonoridad |
| `stems` | stems | stems | pistas, split, separación, multitrack |
| `reference` | reference track | pista de referencia | reference, referencia, a/b, target |
| `metronomeTrack` (also above) | click / metronome | clic / metrónomo | click, clic, metronome, metrónomo, tempo, guide |
| `cue` | cue / guide | cue / guía | guide, guía, scratch, cue, pauta |
| `headphones` | headphones | auriculares | cascos, audífonos, phones, cue mix |
| `monitor` | speaker / monitor | altavoz / monitor | speaker, monitor, bocina, parlante, studio monitor |
| `monoMix` | mono | mono | monaural, one channel, un canal |
| `stereoMix` | stereo | estéreo | stereo, two channels, l/r, dos canales |
| `surround` | surround | envolvente | 5.1, 7.1, atmos, immersive, inmersivo |

### 16. Marks & shapes (22)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `star` | star | estrella |  |
| `heart` | heart | corazón |  |
| `flag` | flag | bandera |  |
| `circle` | circle | círculo |  |
| `square` | square | cuadrado |  |
| `triangle` | triangle (shape) | triángulo (forma) |  |
| `diamond` | diamond | rombo |  |
| `hexagon` | hexagon | hexágono | shape, forma |
| `bolt` | lightning bolt | rayo | bolt, flash, energy, energía |
| `fire` (also above) | flame | llama | flame, llama, hot |
| `sun` | sun | sol (astro) | day, día, bright |
| `moon` | moon | luna | night, noche |
| `leaf` | leaf | hoja | nature, naturaleza, organic |
| `cross` | cross | cruz | christian, cristiano, faith, fe, church, iglesia, worship, alabanza |
| `checkMark` | check | visto | done, hecho, ok, tick, palomita |
| `musicNote` | music note | nota musical | note, nota, music, música |
| `bookmark` | bookmark | marcador | save, guardar, marca |
| `pin` | pin | chincheta | location, ubicación, marker, alfiler |
| `eye` | eye | ojo | watch, ver, visible |
| `lock` | lock | candado | locked, bloqueado, closed |
| `smile` | smile | sonrisa | happy, feliz, face, cara, emoji |
| `bell` (also above) | bell | campana | notification, notificación, alert, alerta, ding |

### 17. Letters & numbers (17)

| id | English | Spanish | Also found by |
|---|---|---|---|
| `letterA` | letter A | letra A | A, badge, insignia, take, toma, letter, letra |
| `letterB` | letter B | letra B | B, badge, insignia, take, toma, letter, letra |
| `letterC` | letter C | letra C | C, badge, insignia, take, toma, letter, letra |
| `letterD` | letter D | letra D | D, badge, insignia, take, toma, letter, letra |
| `letterE` | letter E | letra E | E, badge, insignia, take, toma, letter, letra |
| `letterF` | letter F | letra F | F, badge, insignia, take, toma, letter, letra |
| `letterG` | letter G | letra G | G, badge, insignia, take, toma, letter, letra |
| `letterH` | letter H | letra H | H, badge, insignia, take, toma, letter, letra |
| `num1` | number 1 | número 1 | 1, badge, insignia, take, toma, number, número |
| `num2` | number 2 | número 2 | 2, badge, insignia, take, toma, number, número |
| `num3` | number 3 | número 3 | 3, badge, insignia, take, toma, number, número |
| `num4` | number 4 | número 4 | 4, badge, insignia, take, toma, number, número |
| `num5` | number 5 | número 5 | 5, badge, insignia, take, toma, number, número |
| `num6` | number 6 | número 6 | 6, badge, insignia, take, toma, number, número |
| `num7` | number 7 | número 7 | 7, badge, insignia, take, toma, number, número |
| `num8` | number 8 | número 8 | 8, badge, insignia, take, toma, number, número |
| `num9` | number 9 | número 9 | 9, badge, insignia, take, toma, number, número |

## Sizes and places

| Place | 100 % | 150 % | 125 % (the picker sheet) |
|---|---|---|---|
| Arrangement header | 16 (14 when the row is under 26 high) | 24 (21) | 20 |
| Mixer strip head, Wide | 14, before the name, both centred as one group | 21 | 18 |
| Mixer strip head, Compact | 12, under the number, above the rotated name | 18 | 15 |
| Picker grid | 24 in a 32 cell | 36 | 30 |

**Colour.** At rest the icon takes the track's colour mixed 35 % toward ink 2, with its
second ink at 50 % toward the surface. On a selected track it takes the track's colour
itself. A track without an icon shows its type icon only. In a header, "no icon" is a
1 × 10 ink-4 dash in the icon's place, which also marks where to click.

**Cut colour.** A cut is always the colour directly behind the icon: the header's surface,
the selected header's tint, the picker's raised panel, a hovered cell's `well-hi`, a selected
cell's accent tint. (The earlier picker sheet drew every cut in the surface colour.)

## The track header (Slate's row, 30 high)

At x: the 3-px colour stripe at 0. The type icon (the family's `audio`, `midi`, `bus`…)
at 9, 14 px, ink 3. The user's icon at 28, 16 px. The name at 52 (Regular 12; Medium on
the selected track). The icon is a button. On hover it gets a 20 × 20 `well-hi` square with
a 1-px accent ring, and the hint line reads "Change the track's icon". A click opens the
picker.

## The picker (a popover)

| Part | Measure (1×) |
|---|---|
| Popover | 300 wide; `raised` (Night `#2F373F`, Paper `#FBFCFD`), r 8, a 1-px `edge`, a 1-px lit top, a soft shadow (blur 4, offset 3/6, 45 % / 18 %). It opens beside the header it belongs to, its top 4 below the header's top, with a dashed accent leader line. |
| Padding | 12 |
| Title row | "Track icon" Medium 13 ink 1; the track's name Regular 12 ink 3, right-aligned |
| Search | 22 below: a 24-high well, r 4; the family's `search` at 14 px, ink 3; "Search icons — English, español or a word" Regular 12 ink 3. Typing filters the grid by the English name, the Spanish name and the other words (accents and case ignored); the families with no match fold away. |
| None | 32 below: a 66 × 32 well (two cells), "None" Regular 12 ink 2, and beside it "the track shows its type icon only" Regular 10 ink 3. Selected: as a selected cell. |
| Recent | 8 below: "Recent" SemiBold 11 ink 3, then one row of up to 8 cells, the icons this user chose last, newest first (kept in the user's settings, not the project). Then a hairline in `rule`, 8 above the families. Hidden while searching. |
| Families | the 17 families in the owner's order; a title SemiBold 11 ink 3, 16 high; rows of 8 cells; a sub-row title (Electric shapes, Acoustic shapes) Regular 10 ink 3, 13 high |
| Cell | 32 × 32, 2 apart; the icon at 24 px, centred, ink 2 with ink 3. Hover: `well-hi`, ink 1. Selected: `raised` + 20 % accent, a 1-px accent ring, the icon in accent. |
| Footer | a hairline, then "The icon takes the track's colour." Regular 10 ink 3. Hovering a cell puts its name (English or Spanish, the interface's language) in the hint line. |
| Keys | arrows move through the cells; Enter chooses; Esc closes; typing goes to the search |
| Height | the families scroll in a 520-high view (a 3-px thumb at the right, a 20-px fade at the foot); 316 icons in 17 families |

The whole picker is one undo step (a `Command` setting the track's icon, its inverse
restoring the old one). The icon is project data, so it needs a schema field, a
migration and a fixture: the kit builder's call, under CLAUDE.md's rules. The icon is
stored by its id (the ids above); the Spanish and English names are interface strings and
go in both locales (`locales/**/tracks.{en,es}.txt`, one key per id, the aliases with them).
