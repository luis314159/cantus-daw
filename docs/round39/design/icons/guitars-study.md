# The guitar family — study and redraw (round 39, `icons-guitars`)

*The owner, 2026-11-09, after two rounds:* "aún dejan mucho que desear las guitarras, busca
imágenes en internet para que veas". At 14–16 px every electric shape read alike; the single
cut and the offset looked odd and ugly. This note is what the photographs taught, what failed,
and how each type now reads apart. Sheets: `guitars-v3-{night,paper}-{1x,4x}.png`,
`guitars-v3-before-after-{night,paper}-{1x,4x}.png`; the private comparison with one study
photograph beside each icon is `research/ui/round39/icons/guitars-v3-vs-photos-{night,paper}.png`.

## Sources

Photographs from Wikimedia Commons, studied only — nothing traced (the list of files and their
Commons pages is the private `research/ui/round39/icons/guitars-sources.txt`; the photos
themselves stay outside the repository and appear only in the private comparison). Per
type: three photographs where Commons had them — front views of each electric body, the
archtop, the acoustic bodies (with a chart of acoustic sizes side by side), the basses, and
the requinto jarocho, vihuela, guitarrón, bajo sexto, jarana, charango, Puerto Rican cuatro
and Cuban tres.

Other icon sets at small sizes: DAW track icons are drawn for 32 px and up (Logic's are
128-px photo-real pictures shown smaller; FL's mixer icons are a few generic shapes), so none
of them solves a 16-px guitar body; the stock pictogram sets that list "guitar shapes" are
all upright silhouettes at 48 px and more. The lesson taken from both: a body shape is told
by its outline, and outlines are told apart only when the outline sits on the pixel grid.

## Why set 6's shapes read alike

- **45° antialiases everything.** At 16 px the body was about 10 px along the diagonal, and
  every edge of it fell between pixels. Horns of 1–2 px and waists of 1 px became 30–50 %
  grey fringes; what remained was the same blob with a stick. Exaggerating the horns did not
  help (round 39's second pass), because the exaggeration was blurred too.
- **The body did not use the key.** A diagonal guitar fills a triangle of the square.
- **The detail was the wrong one.** Pickups cut as 1-px lines at 45° were dotted smears.

## The decision: upright portraits, fitted to the pixel

- The shapes stand **upright, body forward** — a portrait of the body under a short neck and
  a small head. The axis, the horns, the bouts and the waists sit on whole pixels; the body
  gets 12–16 of the key's 16 pixels across.
- The headline **electric and acoustic guitars keep 45°** (the emoji-like "guitar" sign, its
  neck long), and so does the **baritone** (its long scale is its name). So the headline is
  never a twin of a shape, and the baritone never a twin of the bass.
- The **basses** stay upright as before but are now told from the guitars by proportion: a
  small body low on a neck as long as the body, four tuners in line.
- The constructions are snapped to the pixel at every size (`snap` in `glyphs-t-guitars.js`);
  at **12, 14, 15 and 16 px** each drawing is **pixel rows**: fitted from the construction
  (`fit.py`), then drawn again by hand where a feature fell between pixels
  (`glyphs-t-guitars-px-fix*.js`). From 18 px the snapped vector holds.
- No larger optical size was needed: the picker's 24 px (30 at 125 %) and the header's 16 now
  read without one.

## What tells each type apart, in the order it survives at 16 px

Electric shapes:

| Type | 1. Outline | 2. Horns / cutaway | 3. Waist | 4. Inner detail (from 16 px) | Head |
|---|---|---|---|---|---|
| Double cutaway | contoured, 14 px across the lower bout | two blunt horns, the bass horn one row higher | yes, on both sides | three single-coil pickups | six in line, leaning to the bass side |
| Single cutaway slab | a slab: straight sides | one blunt horn, low; a flat ledge into the neck on the bass side | none | the slanted bridge pickup | six in line |
| Carved single cut | rounder, smaller upper bout | one sharp horn pointing up; a round dome on the bass side | deep | two humbuckers (wide cuts) | three a side |
| Twin-horn solid body | slim | two sharp horns spreading outward, symmetric (a lyre) | slight | two humbuckers | three a side |
| Pointed double cut | as the double cutaway | needle horns pointing straight up, the bass horn far longer; deep cutaways | yes | humbucker, single, humbucker | pointed |
| Semi-hollow | the widest electric (16 px) | two round ears, symmetric | slight | f-holes down both sides (cut at every size) | three a side |
| Offset | skewed: the lower bout fuller to the treble side | a broad bass horn up high, a short treble horn low | high on the bass side, low on the treble side | two wide soapbars | six in line |
| V body | two wings and a notch | — | — | — | three a side |
| Pointed body | straight edges, asymmetric points | the bass horn up and out | — | a long tail to the lower right | pointed |
| Archtop | big, round, the widest with the semi-hollow | one deep round (Venetian) cutaway, a low round horn; a tall dome on the bass side | slight | f-holes | three a side |

Acoustic shapes — the soundhole says "acoustic"; the widths say which (upper bout / waist /
lower bout, in pixels at 16): dreadnought **10 / 8 / 14**, square shoulders; concert / OM
**6–8 / 6 / 12**, a small round upper bout; jumbo **8–10 / 6 / 16**, the widest and roundest;
parlor **6 / 4 / 8**, low on a long neck. The cutaway dreadnought is the dreadnought with its
treble shoulder scooped to the waist. The **classical** has a window in its head (the slotted
head) at every size; the **12-string** a comb of tuners down both sides of a long head; the
**travel** guitar is the whole instrument small, its head well under the top of the key. The
**resonator** carries its round plate in the second ink, ringed by a cut, the cone in the
middle.

Basses: **bass guitar**, an offset body, the split pickup; **fretless**, its fingerboard in
the second ink (no frets); **acoustic bass**, an acoustic body with its hole on the bass neck,
two tuners a side.

Mexican and Latin strings (upright like the acoustic shapes; the small ones drawn large, the
long neck still telling their size): **requinto** — the slender body and the espiga standing
beside it (a bar across the body read as a "no guitar" sign); **vihuela** — a small round
body, its vaulted back showing beside it in the second ink; **guitarrón** — the huge body on a
short neck, the V of its bowl back under it; **bajo sexto** — a tall pear met high by the neck,
a comb of twelve tuners; **jarana** — slim, pinched, its hole low, a wide head; **charango** —
the banded shell, the bands in the second ink so the outline stays whole; **cuatro** — the
corners pushed out at the waist; **tres** — three doubled courses down the body, in the second
ink.

## Variants (all in `glyphs-t-guitars-{e,a,mx}.js`; `GUITAR_PICK` names the chosen)

Each type was drawn three ways and compared at 12, 14, 16, 24 and 30 px, 1:1 and 4×, with its
neighbours (`sheet-guitars.html?mode=variants`): the electrics as A (outline and pickups),
B (the pickguard in the second ink), C (a more exaggerated outline); the acoustics as A (hole
and bridge), B (with the pickguard), C (wider); the others with their own alternatives.
The pickguards (B) read at 30–36 px but were a muddy second tone at 16, so the cuts won; the
exaggerated outlines (C) won for the double cutaway, slab, carved single cut, twin horn,
pointed double cut, offset and pointed body.

## Legal care

Generic outlines from blank sheets after the study; nothing traced. Labels unchanged and
generic (`glyphs-t-families.js`); makers' model names remain only search aliases. The study
photographs stay outside the repository and in the private `research/` comparison.
