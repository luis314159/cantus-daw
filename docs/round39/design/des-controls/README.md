# Slate controls — every control family, as build sheets (round 39, `des-controls`)

**The idea.** Each control in Slate is one of three objects: a **well** (a field, a key, a
groove, a meter), a **cap** (a knob, a fader, a slider's thumb) or a **sheet** (a menu, a
tooltip, a dialog). Wells are recessed and quiet. Caps stand on the panel, lit from above.
Sheets float with one soft shadow. Colour is spent only on state, as Slate says. The accent
means *on, held or focused*. Play, record, solo and clip keep their own colours. A choice
among equals (a tab, a segment) lifts and stays neutral. The one new idea is the **driven
sign**: when automation, a modulator, a MIDI controller or a VCA drives a value, the control
says so with a shape *and* a colour. An automated knob turns its arc, its pointer and its
value violet, and carries a small lane glyph in the groove's open quarter. Its tooltip and
the help line say which clip drives it and how to take it back. The owner must never again
take an automated knob for a broken one.

Everything here is drawn by hand from a blank sheet in Slate's tokens (`dir-a`). Each size
is drawn for itself at 100 % and at 150 %; the 150 % sheets are not scaled copies.

## Why it beats FL Studio and today's Cantus

- **Against today's Cantus (beta.13):** the brown glossy caps, the teal rings on every knob
  and the raised MUTE plates are gone. Today one knob looks the same whether it rests, is
  automated or is modulated. Here every driven state has its own drawing. Fields have the
  states today lacks: scrubbing, typing, an invalid value with the allowed range under it,
  disabled. Menus get a check column, radio choices, shortcuts, a heading with the object's
  name and colour, and an overflow that scrolls. Dialogs get one primary button, a
  destructive one that is never the default, and a footer.
- **Against FL Studio:** FL's caps are as clean, but FL marks no automated control on the
  control itself, and its modulation is invisible on the knob. Its menus are plain system
  lists. Its hint bar is a single sentence. Slate adds the driven signs, Bitwig-style
  modulation rings drawn quieter, value bars for dense rows, and a help line with keycaps,
  the value and the gesture. It is calm and has one finish across both themes, as FL's is.
  Paper is designed as fully as Night; it is not an inversion.
- **The honest risk:** the knobs keep Slate's dark caps and grey value arcs, quieter than
  FL's mixer. A 13-strip mixer reads calm; a single effect editor full of grey arcs may look
  plain until des-mixer and des-views put colour where the content is. The fader cap was
  lightened after the comparison with FL (see `research/ui/round39/des-controls/`).

## Tokens this sheet adds to Slate

| token | Night | Paper | contrast | used for |
|---|---|---|---|---|
| `mod` | `#EE86C8` | `#B83A8A` | 5.9 : 1 / 4.3 : 1 on surface | a modulator drives the value |
| `fcap_hi / mid / lo` | `#8A95A1` `#66707B` `#525B65` | `#FFFFFF` `#F3F6F8` `#DAE1E7` | index line 4.7 / 15.2 : 1 on mid | fader and slider caps (the strip's brightest grey) |
| `cap_hi / mid / lo` | `#4A545E` `#353E47` `#272E35` | `#FFFFFF` `#F5F7F9` `#DDE3E9` | — | knob caps (radial, 40 % / 28 %, r 78 %) |
| `danger` | `#C8443F` | `#C8352F` | white text 4.8 / 5.2 : 1 | the destructive button |
| `primary_ink` | `#0D1822` | `#FFFFFF` | 7.5 / 5.2 : 1 on accent | the primary button's label |
| `scrim` | rgba(8,11,14,.62) | rgba(34,46,58,.34) | — | behind a dialog |

The driven colours keep Slate's: `auto` `#B48DF4` / `#7A4FD6` (5.3 / 4.4 : 1 on surface),
`accent` for MIDI, `ink2` for VCA. Shadows: menus and popovers `0 6 18` at 50 % (Paper
18 %), tooltips `0 3 10`, dialogs `0 14 44`.

## The driven signs (`signs.py`, sheet `knobs-*.png`, lower left)

| sign | shape | colour | heights (100 % · 150 %) | stroke |
|---|---|---|---|---|
| automated | a lane: low, a ramp, high; a node at each bend | `auto` | 5 7 9 12 · 8 11 14 18 | 1 · 1.25 · 1.5 · 2 |
| modulated | one period of a sine | `mod` | same | same |
| MIDI learned | a socket: a ring, its pin, a dot | `accent` | same | same |
| VCA | a V | `ink2` | same | same |

The sign is 1.8 × its height wide and has a 1-px halo of whatever it sits on. On a **knob**
it sits centred in the groove's open quarter, just below the cap. On the small knob it is a
4 (6) px dot in the sign's colour. On a **fader** it sits 7 (10.5) px left of the cap. On a
**value bar** or a **field** it comes before the number, and the number takes the sign's
colour. In a **tooltip** and the **help line** it leads the first line.

## Knobs (`knobs.py`, `knobs-{night,paper}-{1x,1.5x}.png`)

| size | where | D | groove | gap | cap Ø | pointer | sign | focus ring |
|---|---|---|---|---|---|---|---|---|
| S | strips, inline rows | 20 · 30 | 2 · 3 | 2 · 3 | 12 · 18 | 1.5 · 2 | dot 4 · 6 | +2 · +3 |
| M | panels, inspector, effects | 32 · 48 | 2 · 3 | 3 · 4 | 22 · 34 | 2 · 2.5 | 6 · 9 | +2 · +3 |
| L | an editor's main controls | 52 · 78 | 3 · 4 | 4 · 6 | 38 · 58 | 2.5 · 3.5 | 8 · 12 | +3 · +4 |

- **Groove:** 270° (−135° to +135° from the top), round ends. Night uses `well`; Paper uses
  `well→edge 55 %`.
- **Value arc:** on the groove, the same width. For a bipolar knob it runs from the top;
  otherwise from −135°.
  - Rest: `ink2→ink3 25 %`. Hover: `ink2→ink1 60 %`. Dragged: `accent`. Focused: `ink1`.
    Disabled: `ink4`.
  - Automated: `auto`. Automated and held by the hand: `accent`, with a 1.5 (2) px violet
    tick across the groove where the clip would put it.
- **Bipolar:** a 1 × 2 (1.5 × 3) px `ink3` notch above the top of the groove.
- **Stepped:** detent dots replace the groove, r = 0.5 × groove (the chosen one 0.75 ×, in
  `ink2`, `accent` while held). The large knob names its detents around it (Regular 9.5,
  `ink3`, the chosen one SemiBold `ink1`).
- **Modulated:**
  - The user's value keeps its arc and the pointer.
  - The swing is a band 1 (2) px wider than the groove, in `mod` at 38 %.
  - The live value is a `mod` dot with a halo in the panel's colour.
- **Cap:**
  - Its shadow is the cap 1 px down (Night black 35 %, Paper 16 %), with a 1-px rim in
    `edge`.
  - The radial `cap` gradient fills it; hover adds `ink1` at 7 % (Paper white at 50 %).
  - The light from above is a crescent ±58°, 1 px, 1.5 px inside the rim, `cap_hi` lightened
    18 % at 75 %.
- **Pointer:** it runs from 0.20 / 0.28 / 0.34 of the cap radius (S / M / L) to the rim
  − (pointer/2 + 1.5), with round caps. It is `ink1`, `auto` when automated, `ink4` when
  disabled.
- **Labels:**
  - M: the name above, Regular 11; the value below, Regular 12 `ink1`, with its unit
    Regular 10 `ink3` 2.5 (3.75) px after it.
  - L: the name Medium 12; the value Regular 15 with a unit 13.
  - The name is `ink2` at rest and `ink1` on hover, held or focus.
  - The value is `accent` while held, `auto` when automated, `ink4` disabled.
  - S has no labels: while held, a raised 18 (26) px value pill sits 6 px to its right.

## Faders, meters, sliders, value bars (`faders.py`, `sliders.py`, `faders-*.png`)

**Strip fader** — the travel is 126 (189) px, mapped +6 → 0, 0 → .20, −6 → .36, −12 → .51,
−24 → .70, −48 → .90, silence → 1.

| part | 100 % | 150 % | colour |
|---|---|---|---|
| groove | 2 wide, the travel ± 2 | 3, ± 3 | `edge` (Paper `edge→well 30 %`), round |
| cap | 20 × 18, r 2 | 31 × 28, r 3 | `fcap` gradient, rim `edge` (held `accent`, automated `auto`), lit top 1 px |
| index | 1 px on a 1-px dark line above | 2 px | `#F4F8FB` (Paper `ink1`); held `accent`; automated `auto` |
| grips | ± 4 px, 1 px, inset 5 | ± 6, 1, inset 7.5 | `fcap_lo→edge 55 %` |
| 0 dB detent | a 5-px tick either side | 7 | `ink3` |
| scale ticks | +6 0 −6 −12 −24 −48, 3 px (0 dB 5) | 4 (7) | `surface→ink3 55 %` |
| meter | 2 bars × 3, gap 1, 5 px right of the ticks | 4, gap 2 | its own well: `well→ink4 22 %` (Paper `well→edge 35 %`) |
| colour | `meter_lo` to −18, `meter_mid` at −9, `meter_hi` from −3 | — | cut from one full-height gradient, so a colour belongs to a height |
| peak hold | 1 px | 2 px | `ink1`; `clip` above 0 dBFS |
| clip LED | 3 px, 2 px above the pair | 4, 3 | `clip` lit; idle `well→ink4 25 %` |
| readouts | level Regular 12 `ink1`; peak Regular 10 `ink3`, 14 px under it | ×1.5 | the peak shows only once a signal has passed: never "−∞"; at clip, Medium in `clip` |
| VCA | a dashed ghost cap (2-2) at the VCA's level, the V sign 6 px left | ×1.5 | `ink3`; under the readouts a tag "VCA 1 −4.0", SemiBold 9 `ink2` |
| frozen / disabled | the whole fader at 55 % | — | — |

The **master** has a 196 px travel, 5-px bars with a 1-px gap (7 and 2 at 150 %), and the
scale numbered on the left (Regular 9 `ink3`, the 0 SemiBold `ink2`).

**Slider:**
- **Groove:** 3 (4) px, `edge`. The fill is the value, in the knob arc's colours.
- **Thumb:** 9 × 15 (13 × 22), r 2 (3), drawn like a fader cap with a vertical index
  1 (2) px.
- **Bipolar:** it fills from the centre; a centre mark 1 × 9 (2 × 13) in `ink3`.
- **Range:** two thumbs with the fill between them.

**Value bar** (dense inspector and editor rows):
- **Shape:** a well 20 (30) px high, r 3 (4).
- **Fill:** it is the value, `well→ink3 30 %` (Paper 26 %). Held, it becomes `accent` at
  30 % (22 %); automated, `auto` at 30 %.
- **The value's edge:** 1 (2) px, `ink2`; `ink1` on hover, `accent` held, `auto` automated.
- **Text:** the name at the left, Regular 11; the number at the right, Regular 12, with its
  unit Regular 10 `ink3`. Padding 8 (12).
- **Modulated:** a 3 (4.5) px `mod` band along the bottom at 55 %, and a `mod` line at the
  live value.
- **Focus ring:** 1 px `accent`, 2 (3) px outside.
- **Gestures:** drag sideways or vertically; Shift for fine; double-click to type;
  right-click for automation and MIDI learn.

## Fields and drop-downs (`fields.py`, `fields-*.png`)

| | regular | compact |
|---|---|---|
| height | 24 (36) | 20 (30) |
| text | Regular 12 (18) | Regular 11 (16.5) |
| unit | Regular 10 (15) `ink3` | same |
| padding | 8 (12) | 6 (9) |
| corners | 4 (5) | 3 (4) |

- **States:**
  - Rest: Slate's well.
  - Hover: `well_hi`, with spin chevrons 5 × 3 (7 × 4) in a 12 (18) px column at the right.
  - Scrubbing: the number turns `accent`, with a 2 (3) px `accent` rule along the bottom.
  - Typing: the well goes to `edge` (Paper white), inside a 2 (3) px ring 2 (3) px out at
    55 %. The text moves to the left; selected text sits on `accent` at 35 %; the caret is
    1 (2) px `accent`.
  - Invalid: the ring and the text turn `rec`. Under the field go a 6 (9) px `rec` disc with
    "!" and the allowed range, Regular 10.5 `rec`.
  - Disabled: `ink4`.
- **Search:** a lens (ring r 4 (6), stroke 1.25 (1.75), a handle at 45°) and a 12 (18) px
  clear disc with an ×.
- **Position:** bars `ink1`, beats and ticks `ink2`, dots `ink3`. Each segment is its own
  drag target; Tab moves to the next segment.
- **Drop-down:** the value at the left; a chevron 7 × 4, 1.25 px (10 × 5.5, 1.75) in an
  18 (27) px zone. The chevron is `ink2`, `ink1` on hover and open; open, it points up and
  the field gets the focus ring. The list is a menu at the field's width, 4 px below it, the
  current item ticked.
  - **Quiet** form (a heading): no well until hover; the value Medium.
  - **With a label inside:** the label Regular 11 `ink3`, then the value.

## Menus and context menus (`menus.py`, `menus-*.png`)

- **Sheet:** `raised`, corners 6 (8), a 1-px edge (Night `raised→ink1 9 %`, Paper `line`),
  the shadow above, 5 (7.5) px of padding at the top and bottom.
- **Rows:**
  - A row is 24 (36) px. The label is Regular 12 `ink1` at 26 (39) px from the left.
  - The 26-px column holds the check or the radio dot, so labels align either way.
  - A shortcut sits right in Regular 11 `ink3`, 12 (18) px in, written `Ctrl+X`.
  - A dimmed note may follow the label ("Undo *Move clip*").
- **Hover:** an inset pill 4 (6) px from the sides, corners 4 (5), `raised→accent 24 %`
  (Paper 13 %).
  - A destructive row: `raised→rec 20 %` (12 %), and its label turns `rec`.
- **Check:** a tick 9 × 6 (13 × 9), 1.6 (2.2) px, `accent`. **Radio:** a dot r 3 (4.5)
  `accent`; off, a 1-px ring `raised→ink3 60 %`.
- **Heading:** 24 (35) px, SemiBold 10.5 `ink3`, Title case, never a bar. A context menu's
  heading names its object ("Clip · Lead 2") and may carry its colour, 8 px square.
- **Separator:** 9 (13) px, a 1-px line `raised→ink4 50 %`, inset 10 (15).
- **Submenu:** a chevron 4 × 7 at the right (`ink1` on the hot row, else `ink3`). The
  submenu overlaps 4 (6) px, its first item level with the parent row.
- **Disabled:** label and shortcut `ink4`. The row stays, so the user still learns the
  shortcut.
- **Colour submenu:** the user's colours as 18 (27) px tiles, 6 to a row, 6 (9) apart. The
  chosen tile is ringed 1.5 (2) px `ink1` 3 (4.5) px out, followed by "Use the track's
  colour".
- **Overflow:**
  - A filter field goes on top. At the bottom, a 34 (51) px fade to `raised`, a chevron
    and a 2 (3) px thumb along the right edge.
  - The wheel scrolls; typing filters.
- **Menu bar:** words Regular 12 `ink2`, 8 (12) px padding. The open one is a `raised` pill,
  22 (33) px, its words `ink1`, with the menu attached under it.
- **Context menu of a control** (`menus-*` right):
  - It is headed with the control's name and owner ("Cutoff · Lead").
  - Automated: *Edit its automation clip*, *Clear automation*.
  - Then: *Learn a MIDI controller…*, *Modulate ▸*, *Type a value… Enter*,
    *Reset to default (Double-click)*, *Copy / Paste value*.
  - These are the entries this design proposes; `ux-a` holds what the app does today.

## Tabs, segmented controls, switches, keys, scrollbars (`switches.py`, `switches-*.png`)

- **Panel tabs** (a dock panel's head, 28 (42) px):
  - The active tab is cut from the panel body (`base`, corners 6 (8) on top) and merges
    with it. Its label is Medium 12 `ink1`.
  - Only while the panel has the keyboard focus, a 2 (3) px `accent` bar sits inside its
    top, inset 4.
  - The others are Regular 12 `ink3`; hover gives `ink2` and a pill `surface→raised 70 %`.
  - A close × (3 px arms, 1.25) shows on the active and the hovered tab.
  - Tabs that do not fit fold into a `+3 ⌄` key.
- **Section tabs** (inside a panel): words, 18 (27) px apart. The active one is Medium 12
  `ink1` over a 2 (3) px `ink1` rule; hover is `ink2` over 1 px `ink4`; rest is `ink3`.
- **Segmented:**
  - A 24 (36) or 20 (30) px well. The chosen segment lifts, inset 2 (3): Night a `raised`
    pill with a lit top, Paper a white one with an `edge` frame. Its label is Medium
    `ink1`.
  - Hover gives `well_hi` and `ink2`; the others are `ink3`. 1-px dividers stand between
    unchosen neighbours.
  - Never coloured. Glyph segments take the same rules.
- **Switch** 28 × 16 (42 × 24):
  - Off: a well with a knob r 6 (9) in `ink3` (Paper white with an `edge` ring).
  - On: an `accent` track with a white knob.
  - Hover lightens it 12 %. The focus ring is 1 px `accent`, 3 (4.5) px out. Disabled is
    45 %.
- **Check box** 14 (20), r 3 (4): off, a well (hover adds an `ink3` frame); on, `accent`
  with a `primary_ink` tick 1.75 (2.5) px; mixed, a dash.
- **Radio** 14 (20): off, a well circle; on, an `accent` disc with an `accent_ink` dot
  0.2 × its size.
- **Strip keys** M · S · R, 16 or 20 (24 or 30) high, 1.2 × as wide:
  - Off: a well, the letter SemiBold 10.5 (12) `ink2`.
  - Mute on: `ink2` (Paper `ink3`) with a dark letter — muting takes sound away, so it
    greys out.
  - Solo on: `solo` with a dark letter (Paper white). Arm on: `rec` with a white letter.
  - Each on-key has a white lit top at 18 %. These are proposals; des-mixer owns the strip.
- **Enable lamp** 8 (12):
  - On: `play`, a 2.5 px halo at 18 %, a highlight.
  - Bypassed: a hollow 1.5 px ring in `ink4`, `ink2` on hover.
- **Scrollbars** float over the content:
  - Rest: a 4 (6) px thumb, `base→ink3 55 %`, 3 px from the edge, no lane.
  - Under the pointer: a 12 (18) px lane with an 8 (12) px thumb `well→ink2 55 %`. Dragged,
    the thumb turns `ink2`.
  - A list that scrolls fades out over its last 26 (39) px.
- **Scroll-and-zoom bar** (arrangement): a 12 (18) px well with the body inside.
  - On hover, grips show at each end: two 1-px lines, `ink1` (Paper white).
  - The end under the pointer lights as a 10 (15) px `accent` cap, its grips white.
  - Drag the body to scroll; drag an end to zoom.

## Tooltips and the help line (`sheet_tips.py`, `tips-*.png`)

- **Tooltip:**
  - Timing: 500 ms at rest, then 12 (18) px below-right of the pointer. The next one shows
    at once while the pointer moves between controls.
  - Sheet: `raised`, corners 5 (7), a 1-px edge, shadow `0 3 10`. Padding 7/10/8 (×1.5).
    Max width 320 (480), then it wraps.
  - Line 1: the name Medium 12 `ink1`, then the value Regular 12.
  - Line 2: what it does, Regular 11 `ink2`.
  - Line 3: how, Regular 10.5 `ink3`, with keycaps.
- **A driven control's tooltip:** the sign leads it. The words say what drives it and how
  to take it back, e.g. *Automated by the clip 'Cutoff' on Lead — While the song plays,
  the clip moves this knob. — Right-click ▸ Clear automation, or edit the clip.* Never just
  "Automated".
- **A cut name:** its tooltip is the whole name.
- **A disabled control:** its tooltip says why.
- **Keycap:** 15 (22) px, an outline `raised→ink3 50 %`, corners 3 (4), SemiBold 9.5
  `ink2`, padding 4 (6).
- **Help line:**
  - It is the status line's left part, 22 (33) px, `surface`, with a 1-px top line
    `surface→gutter 70 %`. Regular 11.5; the subject Medium `ink1`, the rest `ink2`,
    separators `ink3`.
  - It is **never empty**. Over nothing it describes the selection and the next useful key.
  - Dragging shows the live value in `accent` and what Esc does.
  - A warning starts with a 7 px `solo` dot, an error with a `rec` dot. An action word
    ("Open") is Medium `accent`.
  - The right side keeps the session facts in `ink3`. The Spanish line, the longest, fits
    1 086 px at 100 %.

## Buttons and dialogs (`sheet_dialogs.py`, `dialogs-*.png`)

| kind | rest | hover | pressed | focus | disabled | busy |
|---|---|---|---|---|---|---|
| primary | `accent`, `primary_ink`, lit top 22 % | `accent→white 12 %` | `accent→black 18 %`, no lit top | the 2 (3) px ring at 60 %, 3 (4) px out | `surface→well 60 %`, `ink4` | a turning arc 5 (7.5) before the label, the fill 25 % toward surface |
| secondary | Night `raised→ink1 8 %` + lit top 10 %; Paper white + `edge` frame | `→ink1 14 %` / `#F3F6F9` | `well` | ring | as primary | arc |
| destructive | `danger`, white, lit top | `→white 12 %` | `→black 18 %` | ring | as primary | arc |
| quiet | words `ink2`, no fill | `well_hi`, `ink1` | `well` | ring | `ink4` | arc |

- **Sizes:** 28 (42) px in dialogs, 24 (36) compact; corners 4 (5); a shadow 1 px down.
  Labels are Medium 12 (11.5 compact), padding 16 (12). The minimum width is 84 (126)
  px, or 64 (96) for compact.
- **Placement:** one primary per dialog, at the right end. Enter presses it and Esc
  presses Cancel. A destructive dialog starts its focus on **Cancel**, and its red button
  says what it deletes ("Delete tracks").
- **Dialog frame:**
  - The window darkens under the `scrim`. The frame is `raised`, corners 8 (12), a 1-px
    edge, shadow `0 14 44`.
  - Title: Medium 15 `ink1` at 20 (30) px, baseline 31 (46.5).
  - A close × (4-px arms, 1.5 px, `ink2`) sits 24 (36) px from the right.
  - The body text is Regular 12 `ink2`, with 18 (27) px leading.
  - The footer is 56 (84) px, `raised→surface 65 %`, under a 1-px line. It holds the
    buttons at the right, 10 (15) apart, a quiet *Reset* at the left, and an `Esc cancels`
    keycap where there is room.
- **A form dialog:** labels in a 70 (105) px column, Regular 11 `ink3`; fields 24 (36);
  rows 40 (60) px apart.

## What it costs to build (for the kit builder)

- **Simple fills:** everything is flat fills, 1-px frames and arcs. There are three linear
  gradients (the fader cap, the slider thumb, a fade) and one radial (the knob cap), each
  with 2–3 stops, plus one soft shadow per sheet type. egui's painter does all of it;
  arcs are polylines (16 segments per 90° at 100 %, 24 at 150 %). The meter's colour by
  height is one vertical gradient mesh clipped to the bar's height.
- **The signs:** four tiny polylines per size (`signs.py` has the exact points as fractions
  of the box). At 5–7 px they must be snapped by hand: the nodes on whole pixels, the ramp
  on a 45° diagonal.
- **Not designed here:** the glyphs inside keys (the `icons` agent), the strip's layout
  (`des-mixer`), the views (`des-views`), the top bar and the overflow panel (`dir-a`).
  Whether grabbing an automated knob overrides the clip is `ux-a`'s finding; the
  *Automated, held* state shows the knob at the hand's value with a violet tick where the
  clip would put it. Re-check it once that behaviour is known.
- The tooltips and the help line are laid out by the browser here (`foreignObject`), so
  their gaps are exact. In the kit, use the kit's text runs: 4 (6) px between runs and
  keycaps 4 (6) px apart.

## What the owner should judge

1. **The automated knob** (`zoom/knobs-*-4x-driven.png`, `tips-*.png`): it must be
   unmistakable at a glance. If violet is too quiet, the arc can widen by 1 px or the cap
   can take a violet rim.
2. **The fader cap** (`compare-controls-4x.png`): lighter and larger after the comparison
   with FL. Is it the right weight beside FL's?
3. **Menus and dialogs in Paper** (`menus-paper-1.5x.png`, `dialogs-paper-1x.png`): is the
   light theme as finished as the dark one?
4. **Value bars** for dense rows (`faders-*` bottom): adopt them in the inspector, or keep
   knobs there?
5. **Mute on = grey** (`switches-*`, keys): or should Mute keep a colour of its own?

## Files

| file | what |
|---|---|
| `knobs-{night,paper}-{1x,1.5x}.png` | three sizes × rest, hover, dragged, focused, disabled, automated, automated held, modulated, MIDI learned, bipolar, stepped; the signs; the anatomy ×6 |
| `faders-…` | the strip fader in silent, playing, peak hold, clip, hover, dragged, automated, VCA, frozen; the master; the anatomy ×5; sliders; value bars |
| `fields-…` | numeric fields (regular and compact) in six states; text, search and position fields; drop-downs (closed, hover, open with the list, disabled, quiet, labelled) |
| `menus-…` | a menu-bar menu with a submenu, checks, radios, shortcuts, a heading; a clip's context menu with the colour submenu; an automated knob's context menu; an overflowing menu with its filter |
| `switches-…` | panel and section tabs; segmented controls (24 and 20, text and glyph); switches, check boxes, radios; M/S/R keys and the enable lamp; scrollbars and the scroll-and-zoom bar |
| `tips-…` | tooltips (key, control, automated, modulated, cut name, disabled) and the help line (six cases, one in Spanish) |
| `dialogs-…` | buttons: four kinds × six states, plus compact; a destructive confirmation and a form dialog on a scrim |
| `zoom/<sheet>-<theme>-4x-<crop>.png` | each 100 % sheet cut and enlarged ×4, nearest-neighbour (the real pixels) |
| `src/` | the design source: `slate.py` (tokens, primitives), `signs.py`, `knobs.py`, `faders.py`, `sliders.py`, `fields.py`, `menus.py`, `switches.py`, `sheet_*.py`. `python -I build.py` renders every sheet with Edge headless; `python zoom.py` cuts the ×4; `python compare.py` makes the private comparisons |
| `research/ui/round39/des-controls/compare-controls-1x.png` | private: FL Studio 24.1.2 at 1:1 · Slate at 100 % · Cantus beta.13, knobs, faders, meters and slots |
| `…/compare-controls-4x.png` | private: one knob and one fader cap of each, ×4 nearest |
| `…/compare-menus-dialogs.png` | private: FL's context menu (its manual's picture, about 125 %) · Slate's menus at 150 % and a Paper dialog · today's Convert-to-MIDI dialog |
