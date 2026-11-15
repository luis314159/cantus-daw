# Direction B — Tono, the knob: what the rig draws today, what it would need

**Files:** `tono-sheet.png` (and `.html`), `tono-moves-night.gif`, `tono-moves-paper.gif`,
`tono-window-night.gif`, `tono-window-paper.gif`. Design source: `src/tono.py`; it dances the
shared `groove` and `jump` and its own `steps_wave` (`src/moves.py`).

## What it is

Slate's own parts come alive.

- **Head: Slate's knob.** Knob body `#2F373F`, the knob's rim `#76818D` (which holds the
  silhouette at 3.9:1 on Night), a lit lip at the top, and a dark cut outside it.
- **The scale round the head: a visualiser.** 11 dots from 7 to 5 o'clock, lit gold up to the
  level heard.
- **The pointer: a tuft.** A gold notch on the rim that swings late on the beat.
- **Body: a key** of the theme's surface (rim, lip and a well). The well holds the logo's
  five vermilion bars, which move with the level.
- **Limbs: cables.** One curve each, `#7D8996`.
- **Hands and feet:** jack tips for hands; fader caps with a gold grip line for feet.
- **It wears the theme.** In Paper it is Slate Paper's knob: a white cap with a `#7A8692` rim
  (3.4:1), a paper key, the same gold and vermilion.
- **Style:** bouncy. Bounce 0.16 L, squash 0.16, and arms × 1.45 so its gestures clear its
  wide head. The cables follow through.
- **Moods:**

  | Mood | Face |
  |---|---|
  | Neutral | pill eyes with a glint |
  | Happy | ^ arcs |
  | Excited | arcs, an open mouth, every dot lit |
  | Calm, sleepy | closed curves (sleepy: the head tipped and the dots dim) |
  | Surprised | wide pills, an open mouth |
  | Cool | a dark glass visor with a gold lip |
  | Singing | closed eyes, an open mouth |

  No blush, no stars.

## What `cantus-dance` already draws

Tono is the closest of the three to today's rig: the family's construction (`cast/kit.rs`)
with Slate's parts.

| Tono needs | Today |
|---|---|
| Cable arms and legs, one curve each | `kit::hose_into` (rubber-hose limbs through shoulder, elbow and wrist) and `kit::arm_points` (angle and bend) |
| Jack-tip hands | `kit::mitten_into` → a circle (simpler) |
| Fader-cap feet | `kit::shoe_into` → a rounded rect plus a gold ribbon |
| The knob, rim, lip, dots | ellipses and circles: `ellipse_into`; the lip is a value step (two discs), as `bands_of` does |
| The pointer swinging late | `Body::top` ("what is on top leans") already follows late with `Style::lag`: the pointer's angle is `top` |
| Eyes, visor, mouth by mood | `rig_face.rs`: moods, lids, sunglasses and a cartoon mouth; the pills, arcs and visor are new shapes on the same switch |
| Squash and stretch, landing squash | `Body::squash`, `rig::squash` (area kept) |
| Contact shadow | `kit::shadow` |

## What it would need (for a builder)

1. **Two palettes, Night and Paper, from the stage's theme light.** Today a character's
   palette is fixed and turned by hue. Tono takes the theme's knob and key colours, as Compás
   takes the inks: a small `StageInk`/theme-light flag in `Canvas`, set by the interface.
   Gold, vermilion and the eyes are fixed: they never turn, like Tic's screen. `hue` turns
   only the cables and the key (or nothing; the owner chooses).
2. **A rounded rect shape**, `rrect_into(c, w, h, r, deg)`, for the key, the well, the bars,
   the fader caps and the visor. Today `rect_into` has square corners.
3. **The level for the dots and the bars.** `Pose::energy` already carries the knob, the
   level heard by *react* and a note's velocity. Tono reads it for the 11 dots (lit up to
   `energy × 11`) and the five bars.
   - With no *react*, the companion already measures the master's peak (ADR 0091,
     `companion::SOUNDING`), so feed it the same way.
   - Stopped, the dots rest at two or three.
4. **Its own Step left/right** (`steps_wave`): nothing crosses the body. The near arm waves up
   on the close and the far one swings out low, because a crossing cable hides the bars.
   The other moves are the family's, with its `Style`.
5. **Cost:** about 45–55 fills, like the family (52–58).
   - The 11 dots are one batched path per state (lit and unlit), as the rig already batches
     both eyes.
   - The five bars are one path.
6. **Tests to keep:**
   - every mood differs at a 300-pixel stage (as Volta's test does);
   - gold and vermilion never turn with the hue;
   - the rim holds 3:1 on both rooms;
   - the chroma-key distance (every Tono colour is ≥ 120 from a key);
   - the bounds include the dots.

## Verified only in these renders

Drawn by Edge from the design rig's SVG, not by tiny-skia. The cable curves are quadratic
curves through the elbow, which the rig's `hose_into` approximates.
