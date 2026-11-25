# The violin's left hand, measured from photographs (r42:arcus-hand)

The owner, 2026-11-25: "URGENT LA MANO IZQUIERDA DEL VIOLIN, ESFUÉRZATE BIEN EN ESO" — after
three passes the realistic musicians' left hand on a violin or a viola "sigue siendo un
desastre". This note holds what the photographs measure and the rules `left_hand.py` solves
the table against, so a later pass starts from numbers, not from memory.

**The photographs are study material only.** They are never in the repository or the app,
and no face or likeness is taken from them. They live outside the tree:
`F:\projects\daw-r42\refs-violin\` (40 Wikimedia Commons photographs of professional
violinists and violists — soloists, orchestra sections, close-ups of the left hand, the
player's side — each with its page, author and licence in `sources.md`; a contact sheet;
crops of the eight clearest front views; `annotated/` with the landmarks drawn) and the
owner's five in `docs/round42/arcus-upper/ref-owner/` (untracked).

## How the pictures are measured

On a front view (the audience's side, the scroll to the right — the side the app's camera
sees), by eye on a grid, then computed (`hand_study.py` measures our own renders the same
way, `picture()`):

- **forearm–neck**: the angle between the forearm's line (elbow side → wrist) and the
  fingerboard's E edge in the picture;
- **hand–neck**: the same for the hand's line (wrist → the base knuckles' middle);
- **wrist**: the bend between the two in the picture;
- **knuckles**: the base knuckles' height over the fingerboard's top edge, in *neck
  thicknesses* (the E edge's top to the neck's underside, as the picture shows it; negative:
  in front of the neck's side);
- **arch**: the highest finger joint over the same edge, in neck thicknesses;
- **back w/l**: the back of the hand's width at the knuckles over its length (wrist →
  knuckles) — how squarely the back faces the camera (a square back of a hand reads ~0.95).

## Front views (1st to 3rd position)

| picture | forearm–neck | hand–neck | wrist | knuckles | arch | back w/l | thumb |
|---|---|---|---|---|---|---|---|
| owner's 5 (front, scroll right) | 51° | 74° | 23° | −0.2 | +0.3 | ~0.7 | hidden |
| owner's 2 (wikiHow, front, level) | ~65–80° | 74° | 6° | +0.7…+1.8 | +1.3…+3.1 | ~0.5 | hidden |
| ref-08 (soloist, plain ground) | 83° | 79° | 3° | −0.7 | +1.3 (index lifted +2.8) | ~0.9 | hidden |
| ref-11 (soloist, 3rd position) | 85° | 89° | 4° | −0.4 | +0.7 | ~0.65 | hidden |
| ref-19 (soloist, b/w) | ~60° | 83° | ~20° | −0.7 | +1.3…+2.1 | ~0.63 | hidden |
| ref-04 (soloist, 1st position) | ~60° | ~70° | ~10° | ~−0.5 | +1.2 | ~0.7 | hidden |
| ref-06 (soloist, 1st position) | ~55° | ~70° | ~15° | −1.0 | +1.25 | ~0.7 | hidden |
| ref-02 (soloist, high position) | ~70° | ~80° | ~10° | −1.5 | +0.3 | — | hidden |

(The first four are drawn in `refs-violin/annotated/`. "~": read by eye on the grid, ±5°
or ±0.3; the rest computed from the marked points.)

What every front view shares:

1. **The back of the hand faces the audience, at a slant**: its width shows at 0.5–0.9 of
   its length (never square, never the palm, never edge-on). The palm faces the neck's G
   side and up toward its underside; a gap under the neck.
2. **The forearm rises from below** at 50–85° to the neck (median ~65°), the elbow toward
   the body; **the hand stands steeper than the forearm** by 3–23° (median ~10°): the wrist
   straight but for a little turn toward the little finger.
3. **The base knuckles sit against the neck's near (E) side**, from level with the
   fingerboard's top to one neck thickness below it in the picture; the knuckle line runs
   along the neck, the little finger's a little further out.
4. **The fingers arch up and over**: their first phalanges rise from the knuckles, the
   middle joints stand 1–2 neck thicknesses over the fingerboard's top (photo 5: 0.3), the
   last phalanges come down onto the strings; from the front we see the curved backs of the
   fingers and their knuckles, not their pads.
5. **The thumb is hidden** from the front in every picture; from the player's side (owner's
   1 and 3, ref-38, ref-39) it lies along the G side, its tip about level with the
   fingerboard or a little over it, opposite the first and second fingers (1st position),
   under the neck's shoulder in 3rd.
6. **High positions** (ref-02, ref-11, ref-12, ref-40): the hand's heel at the body's
   shoulder, the knuckles lower against the neck's side, the fingers shorter over the top
   (arch 0.3–0.7).

## From the pictures to the solver's rules (instrument frame: x the E side, y toward the scroll, z the fingerboard's normal)

| rule (`left_hand.py`) | value | from |
|---|---|---|
| the back of the hand's way (`back_way`, `ROLL`, `YAW`) | the E side, rolled down 18° (G string) … 34° (E string), turned 14° toward the scroll; held within 12° (`PALM_MIN`) | 1, 5: the slanted back, the palm up toward the neck's underside; the roll eases on the G string where the elbow swings in |
| each middle joint over the fingerboard's top (`ARCHES`) | 3.0, 3.3, 3.1, 2.4 cm (index … little) | 4: 1–2 neck thicknesses (a violin's neck and fingerboard ~2.6 cm at the hand) |
| each first phalanx rising (`RISE_COS`) | within 40° of the fingerboard's normal | 4 |
| the playing tip onto its string (`UPRIGHT_COS`) | its last phalanx within 20° of the fingerboard's normal, into the string | 4 (the gate: 25°) |
| the wrist under the neck (`WRIST_X`, `WRIST_Z`) | −1.7 … +2.0 cm across from the E edge, 5 … 9.5 cm under the top | 2, the photographs from the player's side |
| the wrist's turn toward the little finger (`WRIST_DEV`) | 3–13°, resting at 8° (the forearm's way kept per row in the table, version 2) | 2 (the gate: 15°) |
| the index's base against the E side (`crook_x`, `crook_z`) | 0.85 cm out, −0.7 … +0.3 cm from the top | 3 |
| a finger not playing (`HOVER_SLACK`) | within 1.4 cm of over its note | the fingers fall toward their own strings, loosely |
| the thumb (`thumb_x/z/y`) | the G side, its tip −0.6 … +0.3 cm from the top, opposite fingers 1–2 | 5 |

## Our hand, measured the same way (table v12, `hand_study.picture`)

The first position is the A string, first finger; the high one the E string an octave up.
"app (new)" is the realistic violinists' camera (`bowed_draw::REAL_HELD_VIEW`, azimuth −20°,
5° up); the earlier one looked from 59° round the neck toward the scroll, where no reachable
pose showed the back narrower than 1.4 times its length.

| figure | instrument | case | camera | forearm–neck | hand–neck | wrist | knuckles | arch | back w/l |
|---|---|---|---|---|---|---|---|---|---|
| woman | violin | first | app (new) | 48° | 53° | 5° | −0.6…−0.9 | +1.4 | 0.66 |
| woman | violin | first | photo 5's | 50° | 55° | 5° | −0.5…−0.7 | +1.4 | 0.61 |
| woman | violin | high | app (new) | 78° | 77° | 0° | −0.6…−1.6 | +1.2 | 0.85 |
| woman | viola | first | app (new) | 37° | 42° | 5° | −0.6…−0.4 | +1.3 | 0.55 |
| woman | viola | high | app (new) | 65° | 65° | 0° | −0.5…−1.4 | +1.0 | 0.78 |
| man | violin | first | app (new) | 60° | 64° | 4° | −0.6…−1.7 | +1.3 | 0.75 |
| man | violin | first | photo 5's | 61° | 65° | 4° | −0.5…−1.4 | +1.4 | 0.67 |
| man | violin | high | app (new) | 88° | 87° | 1° | −0.7…−2.2 | +1.4 | 0.99 |
| man | viola | first | app (new) | 50° | 54° | 4° | −0.6…−1.3 | +1.4 | 0.75 |
| man | viola | high | app (new) | 77° | 76° | 0° | −0.7…−1.9 | +1.2 | 0.94 |

The photographs: forearm–neck 50–85°, the hand steeper by 3–23°, knuckles 0 … −1 neck
thicknesses, arches +0.3 … +2, back w/l 0.5–0.9. Ours sit inside those ranges, except the
high positions' back (0.85–0.99: the forearm straight up under the neck shows the hand end-on;
ref-02, ref-11 and ref-12 show the same broadening) and the wrist's turn, kept at 0–6° (the
coordinator's gate: 8°).

**The hand's outline** (`widths.py`-style, the gate in `arcus_hand_table`): her palm at its
middle over the wrist's crease 1.05–1.21 in first position (the photographs 1.1–1.3; before
the thumb's base cap and the palm's slimming 1.55); with the little or ring finger reaching,
or high up, up to 1.63 (gated at 1.7, open).

**Measured causes found on the way** (kept here so nobody reintroduces them): MakeHuman's
finger joints 1.2–1.5 cm past the knuckles (fixed: `knuckle_fix`); the thumb's base turned
65° by an unconstrained solve (capped at 30°: the "mitten"); the wrist lagging the hand's
turn by 30 % (now 5 %); the camera 30° round toward the scroll from where photographs are
taken (the realistic violinists' camera).
