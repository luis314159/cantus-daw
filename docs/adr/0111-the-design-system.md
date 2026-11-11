# ADR 0111 — The design system: Slate by default, the families beside it

**Status:** Accepted (round 39; drafted by builder *kit*, completed at the close-out from the
owner's approvals and the builds) · **Date:** 2026-11-09, completed 2026-11-10 · **Round:** 39 · **Extends:** ADR 0024 (the kit), ADR 0046
(the theme family), ADR 0103 (materials) · **Supersedes:** ADR 0046 §1–2 (the default and the
shipped list) · **Schema:** none (the project model is untouched; a theme file gains an
optional `system` with a serde default) · **Measured by:** ADR 0108

## Context

The owner, 2026-11-08: *"FL aún se ve a años luz de Cantus … hay botones que se ven super
feos"*. Round 39's design agents drew three directions from blank sheets (BACKLOG §6j):
**Slate** (`dir-a`), **Graphite and gold** (`dir-b`) and **Studio ink** (`dir-c`), each in a
Night and a Paper light, at 1× and 1.5×, beside FL Studio. On 2026-11-09 the owner chose
Slate as the design system and the default, and asked to keep the other two as options
(*"los demás modos se ven bien también, me gustaría tenerlos como opciones"*). The icons'
first set was not approved; the redrawn set 2 was (§6). With logo #03 he added Slate Lapis as
a fourth, optional family (§2, §9).

## Decision

### 1. A theme carries a design system, not only colours

A theme is its tokens (every colour role, ADR 0024 §2) **and its system**
(`kit::system::System`): the **face** the interface is set in and how a **key** is built.
Both travel in the theme file (`system`, absent = Classic) and in the tokens every painter
is handed (`Tokens::system`), so a painter asks its style, never a global, how to draw.

| Family | Face | Keys |
|---|---|---|
| Slate (default) | Barlow Semi Condensed | recessed wells |
| Graphite and gold | Sofia Sans, three widths | recessed wells |
| Studio ink | IBM Plex Sans Condensed + Plex Mono for values | recessed wells |
| Classic (high contrast, users' own older themes) | IBM Plex (round 24–38) | raised (round 37) |

### 2. The shipped themes

Twenty-four, listed on Settings ▸ Appearance in four groups (the owner, 2026-11-10: *"Recover
all of them, and add more if you want"*):

- **Round 39's families** — Slate Night (the default) and Slate Paper; Slate Lapis Night and
  Paper (`theme_slate_lapis.rs`, below); Graphite Night and Paper; Studio Ink Night and Paper.
- **From earlier rounds, redrawn** (`theme_redrawn.rs`) — the ten themes of rounds 24–38,
  each in its own light and character, re-expressed in round 39's system (below).
- **New in round 39** (`theme_added.rs`) — Evergreen Night and Paper, Bordeaux, Print.
- **For low vision** — High contrast dark and light (every text role at 7:1).

Round 39 first retired the ten older themes and showed Slate for a setting that named one;
**that retirement is reversed**: every old id (`cantus_night`, `cantus_paper`, `daylight`,
`blue_hour`, `ultraviolet`, `lagoon`, `slate`, `paper`, `ink_gold`, `parchment`) names its
own theme again, so an `appearance.json` of beta.13 opens in the theme it names, its mode's
own changes on top; `Appearance::retire`, `RETIRED` and `successor` are gone. The old
constructors stay as they were (Classic: raised keys, the gradients), named nowhere in the
interface: the kit's tests and golden sheets draw with them, and `appearance::CLASSIC` lists
them by id so a pre-appearance `theme.json` that is exactly one of them is that preset
chosen — it opens in the preset of its id, redrawn, in its own mode (Paper, which every
release of that era wrote on its first run, leaves a fresh install's appearance).

**How a theme is redrawn** (`tools/themes/themes.py`, which wrote both files and measured
every promise below before writing them): Slate's own ladder, token by token, turned to the
theme's hue with its own amount of colour, at Slate's luminance scaled in WCAG's terms —
(L + 0.05) × f − 0.05, which keeps every contrast between two surfaces as Slate has it (the
rule `lapis2` used with f = 1) — its inks turned to the theme's ink, the accent's tones
derived as Slate derives its own, then the theme's own colours from its old palette on top,
each moved the least that holds a promise. So each has Slate's structure (gutter, base,
surface, raised, well/edge/lip, ink 1–3, one accent, the lane veil), recessed keys and no
gradient; the selection tint, the readouts' ink and the loop's edges are used where they
are the theme's character.

| Theme | Light | System (face · keys · cables) | Character |
|---|---|---|---|
| Cantus Night | dark | Graphite (Sofia) · recessed · Flow | espresso room, ivory inks; copper keys, verdigris selection and tint, amber readouts, playhead and loop |
| Cantus Paper | light | Graphite · recessed · Flow | the website's cool stone; copper-deep keys and readouts, verdigris-deep selection |
| Blue Hour | dark | Graphite · recessed · Flow | deep navy; electric-blue keys and selection, ice-blue readouts and loop |
| Daylight | light | Graphite · recessed · Flow | white with blue: pale-blue bars, a vivid blue on keys, readouts and loop, the meters in a dark slate well |
| Pewter (id `slate`) | dark | Slate (Barlow) · recessed · Flow | round 24's FL-derived Slate, renamed: a lighter, bluer slate grey over a dark gutter, orange keys, lime selection, playhead and meters |
| Chalk (id `paper`) | light | Slate · recessed · Flow | its twin: grey chalk paper, deep-green keys and selection, lime meters in a dark well |
| Ink & Gold | dark | Studio ink (Plex) · recessed · Flow | the first theme: blue-black ink, warm ivory, gold keys, readouts and playhead, soft-blue selection |
| Parchment | light | Studio ink · recessed · Flow | warm cream sheet, burnt-umber keys, sepia readouts, gold playhead and loop |
| Ultraviolet | dark | Slate · recessed · Flow | deep violet; magenta keys and loop, cyan selection, playhead and readouts |
| Lagoon | dark | Slate · recessed · Flow | cool teal room, teal keys, amber selection, readouts and playhead |
| Evergreen Night | dark | Graphite · recessed · **Jack** | new: forest green with brass keys, selection and readouts; cream inks; real patch cords |
| Evergreen Paper | light | Graphite · recessed · **Jack** | new: pale sage; forest-green keys and selection (brass is muddy as a key on paper), brass readouts, playhead and loop |
| Bordeaux | dark | Graphite · recessed · Flow | new: deep wine; rose-gold keys and selection, champagne readouts, playhead and loop |
| Print | light | Studio ink · recessed · **Subway** | new: black ink on white paper and one vermilion — greys everywhere, ink keys and selection, vermilion playhead, loop, record and clip, grey meters, a heavier veil, cables as a printed diagram |

**Classic is kept only where a theme is about it**: the high-contrast pair (Plex's open
forms at 7:1, raised keys with bright edges) and the user's own older themes. No restored
theme was about raised keys — each was its colours, and Pewter's FL character is its
grey chrome and lime, not the relief — so each takes recessed keys, round 39's standard.

**Every theme easy to read, measured** (the owner, 2026-11-10: *"Todos deben ser fáciles de
trabajar y de leer con los diferentes temas"*): `views/arrangement_legibility.rs` holds every
shipped theme to 3:1 (WCAG 1.4.11) for what carries meaning — a clip's content on its body for
every track colour, the selection, the playhead, the grid's bar lines, the ruler, the fader
cap, solo and record, the lit key, the meters, the focus ring, the colour cells — or to Slate's
own measure where Slate stands under it; beat lines visible (1.3:1); the demo's colour cells
ΔE 12 apart. It moved, at the cause and in **every** theme Slate's included: the arrangement's
bar lines to 3:1 and beat lines to 1.35:1 on the lanes (Slate's were 1.2:1 and 1.0:1), and
solo's ink to 3:1 on its key; and, in single themes, Pewter's room, two papers' meter wells,
Print's and Bordeaux's veils (`docs/round39/themes.md`). The three first left open were closed in every light theme, Slate
Paper included (the owner: *"seeking excellence"*): the track's colour marks at 3:1 on light
bars (`kit::color::track_mark`, `Ink::stripe`: the same hue, darker), the clip bodies at rest
told apart at CIELAB ΔE 3 and more (a stronger band and half the veil on a light theme), calm
under chroma 20, and the paper fader cap's rim at 3:1 (`fader_slate::paper_rim`).
**The FL-derived pair** carry new names (Pewter, Chalk) that collide with neither the new
Slate nor anyone's mark; their ids `slate` and `paper` still load them.

**Slate Lapis** (logo #03's theme; reworked in round 39's `lapis2` after the owner found it
"looks the same" as Slate in whole-window shots, and approved this) is Slate's system —
structure, text, faces, keys, knob and fader caps, meters, hues, status colours, corners —
with three things of its own, each carried by a token Slate leaves clear, so Slate does not
move by a pixel (`tests/slate_window_golden.rs`):

- **Lapis on the selection**: the accent `#7EA2F6` / `#2443A8` (its tones derived as Slate
  derives its azure ones) and `accent.selection_tint` (lapis at 18 % / 10 %), laid over every
  selected fill — a clip's body, a note, a mixer strip, the selected track's header and
  lane, the channel rack's row, Settings' page row — and marking the active tab's top edge.
  Clear, each view keeps its own selection (Slate's are neutral or the track's colour).
- **Gold on the readouts and marks**: `text.readout` / `text.readout_secondary` — the top
  bar's position, time and tempo and the rulers' bar numbers in the arrangement and the
  piano roll — `#E2B85A` / `#C9A55A` at night, `#6E5008` / `#7A5A04` on paper (4.5:1 and more
  on the bar, its wells and the panel; the floor holds it under the knobs); the playhead
  (`#E2B85A` / `#9A6E1C`); the loop region's ends and, through `editor.loop_edge`, its edges
  along its whole length. Body text, labels and buttons keep Slate's inks.
- **A night-blue room**: every surface, well and line of Slate's ladder is Slate's own colour
  at the same WCAG luminance, its hue turned to the lapis's (222° at night, 226° on paper)
  and its saturation × 1.7 / × 1.8 — deep night blue in Night, a cool blue-grey paper in
  Paper. The same luminance keeps every contrast pair Slate holds (the tests measure it).

### 3. Tokens, as designed

Each family's tokens are its design's values, token by token (`theme_slate.rs`,
`theme_graphite.rs`, `theme_studio_ink.rs`): Slate's gutter → `ink`, base → panels and
lanes, surface → bars, strips and cards, raised → menus, well/edge/lip → keys and fields,
ink 1–3 → text, secondary, placeholder; one accent for state. Where a design colour falls
short of a promise the kit makes, it moves the least that holds it, and the file says so
(Paper's record red 4.5:1 on the base; Paper's meters in a white groove at 3:1; function
hues, which no design drew, at 4.5:1 on cards and displays). No family paints gradients or
glows: depth is value, a 1-px edge and a 1-px lip (materials kept faint). The user's track
colours are spent as Slate spends them — a tint at rest — through the veil (`editor.track_veil`,
ADR 0046 §8): each family's lane colour at 59 % over a track's large fills; the colour cell
keeps the bright hue.

### 4. Type: a face per family, roles unchanged

Roles stay the only way to ask for text (ADR 0024 §3); each face sets them its own way
(`kit::faces::of`): Slate's clock ExtraLight 30, names Regular 12, titles Medium 13, labels
Regular 11, section titles SemiBold 11 in title case (never capital bars), tags SemiBold 9;
Graphite's clock Extra Condensed 200 at 34; Studio ink's clock ExtraLight 31 and values in
Plex Mono. Family *names* are stable (`cond`, `value`, `clock`…) and a face points them at
its files, so every view follows a theme switch with no change of its own. IBM Plex stays
registered always: the units' lettering (ADR 0074), the score (ADR 0075) and the fallback
after every other face.

The files are built by `tools/fonts/build_faces.py` from the published ones: **tabular
figures by default** (egui applies no OpenType feature; without the remap a counting value
shifts), unhinted, Sofia as static instances. Both families are OFL with no Reserved Font
Name, so a modified version may keep its name. Bundled sizes: Barlow 385 KB (5 files),
Sofia 977 KB (6), Plex Condensed ExtraLight and Light 226 KB — 1.59 MB in all.

### 5. Keys and wells

A recessed key is a flat well a shade darker than its bar, a 1-px dark edge round its top
and sides, a 1-px light lip along its foot; no gradient, no drop, no gloss. Hover lifts
the well a step and turns the glyph from ink 2 to ink 1; pressed darkens it to its edge;
on mixes the accent into the well and draws the glyph in the accent (record stays red); a
radio choice (Pat | Song, a segment) is a lifted neutral pill, never a colour. Fields and
readouts are the same well (no glass over a readout).

### 6. The icon family

The icon family is drawn by hand for each device height it sits at — 16, 20, 24, 30 and
36 px (sets 3 and 4 also at 8–18 px and their 150 %) — and never scaled from another size
(`docs/round39/design/icons/`, its grammar and specs). The kit draws the designer's drawings
as they are (`kit::icons`, `kit::glyph_mixer`, `kit::glyph_browser`, `kit::glyph`'s set-5
data, each generated from the sheets' sources by `tools/icons/*.js`): rasterised once per
(glyph, size, inks) into the layer atlas, one quad on the key's pixel grid (ADR 0108). Inks per
state as the family says: ink 2 / ink 3 at rest, ink 1 / ink 2 under the pointer, the accent
when on (green for play and enable, red for record, arm and a stopped plug-in, amber for a
favourite and a warning), ink 4 unavailable. Where a set has no drawing yet (the piano roll's
Paint and Playback tools), the kit's own glyph stays.

Where each set stands with the owner:

| Set | What | State |
|---|---|---|
| 1 | the first pass: grammar, transport, tools, windows (`icons/v1/`, kept for the record) | **rejected** by the owner (*"más elegantes, bonitos y mejor pensados"*, compare with FL and other DAWs) |
| 2 | transport, tools and window keys, 29 glyphs | **approved** by the owner (2026-11-09): the family |
| 3 | the mixer strip and the inspector, 23 glyphs | **in use** (el-mixer, lo-mixer); **pending the owner's review** |
| 4 | the browser, 17 glyphs | **in use** (el-views, lo-rest); **pending the owner's review** |
| 5 | the rest of `kit::Glyph`, 30 glyphs, and `sliders` (lo-mixer) | **in use** (el-controls); **pending the owner's review** |
| 6 | 316 track icons in 17 families, each for ten device sizes | **in use** in the track icon picker (ADR 0113 §4); asked for by the owner; its guitars, rejected twice, redrawn from a photo study; the set as built awaits his look in the app |

Sets 3–5 were drawn in set 2's grammar and built so the views could land, but the owner has
not reviewed them; the sheets' own open questions (the plug solid at 16–20 px, the rack's
unlit steps in the second ink, the six-tooth gear at 16 px, the soft 12-px star, the
session's lit cell at 16 px) go with that review. A set he sends back is redrawn and its data
regenerated; no view code changes for it.

### 7. The interface's size follows the screen

The owner (2026-11-09): the default interface scale on a 1 920-px screen is **125 %**; a
user's saved scale is kept. The rule for every width (`screen_scale`): the whole interface —
the system's scaling and ours together — is drawn at **1.25 × (physical width ÷ 1 920)**,
never under 100 % nor over 200 %, ours the nearest zoom step. 1 280 and 1 366 → 100 %;
1 920 → 125 % (100 % of ours if Windows already scales 125 %); 2 560 → 175 %; a 3 840 screen
at the system's 150 % → 125 % of ours (200 % whole). It follows the screen under the window
until the user picks a scale (Ctrl +, Ctrl −, Ctrl 0, Settings); a settings file written
before round 39 keeps its scale.

### 8. The element sheets, as approved and as built

The owner approved each element sheet on 2026-11-09 (`docs/round39/design/<name>/`, each
README the full redline; logical px at 100 %, 150 % multiplies by 1.5; FL comparisons stay in
`research/ui/round39/`). Below is each sheet's measured spec in short, and where the build
differs from it on purpose. All of it is Slate's tokens and roles; a sheet adds no colour
except where it says **new**.

#### 8.1 The grid and the scale (`dir-a`, `system.png`)

- A **2-px unit**; layout on even numbers so 125 % and 150 % land on whole or half pixels.
- Keys **16, 20, 24** high (and their 150 %: 24, 30, 36); glyph strokes 1, 1, 1.5, 2, 2 px.
- Rows: a track **44** in the arrangement (des-arrange; 30 in the phase-A window), a panel head
  **28**, the top bar **44**, the status line **20–22**.
- Radii 2, 3, 4, 6, 8. Mixer strips 30 / 60 / 94 at a pitch of width + 2.
- The type scale: clock ExtraLight 30 (ticks Light 15 ink 3); values Regular 15; titles Medium
  13; names Regular 12; labels Regular 11; section titles SemiBold 11 ink 3 in Title case;
  units Regular 10; tags SemiBold 9; tabular figures on.
- Ink 3 reaches 4.5:1 on every surface it sits on; ink 4 is for disabled glyphs and idle marks,
  never for text someone must read.

#### 8.2 The top bar and More (`dir-a` phase B; built by `el-topbar`)

- **44 high** on `surface` with one lit top edge; every key a recessed well **24 high at y 10**
  (fill `well`, a 1-px `edge` on top, a 1-px `lip` under it, r 4); joined keys share one well,
  divided by the edge colour inset 4. Groups 10 apart, 12 from the edges, at least 24 between
  the two clusters.
- Groups and widths (full / compact): menus and project 34 + menus (≈ 204); Pat / Song 74;
  transport well 122 (30 · 32 · 30 · 30) + loop 30; metronome 46; tempo and metre 116 / 78;
  position ≈ 124 / ≈ 73 (bar.beat ExtraLight 30); grid and snap 80 / 40; undo 56; windows
  196 (seven 28-px keys); CPU and RAM 142 (a 34 × 12 graph); inputs 112; master meter 132 / 92
  (two 3-px bars, ticks −48 −24 −12 −6 0, grooves at rest); device 104 / 30; tools 84;
  Settings 28; More 30 with the folded count in a 13 × 11 raised pill.
- **Fold order**: tools → inputs → CPU/RAM → windows → device compact → metronome → meter
  compact → tempo compact → position compact → menus compact → Pat/Song → grid compact →
  menus minimal → undo → meter; the transport last. A unit test reproduces the sheet's widths
  table at 1920 … 800.
- **More** is built from the folds: sections Metronome, Windows, Performance, Inputs, Tools
  and zoom; small folds under *Also here*; columns 284 wide with a 24 gutter, one column up to
  three sections, two from four; `raised`, r 8, one shadow; rows 26 apart; shortcuts as
  outline keycaps 13 high under each key; Performance as four graphs with their numbers.
- **As built, on purpose**: the tabular clock and the take's time cost ~30 px more than the
  sheet's proportional digits, so at 1920 × 1080 and 100 % the tools fold (and the inputs in
  Spanish). The bar carries neither the automation arm nor the typing keyboard (the sheet has
  neither; their keys and the palette keep them). Spanish is measured per group and folds
  rather than collides. The old hint panel is gone: the help line is the status line's.

#### 8.3 The panels' toolbar (`dir-a` `b-toolbar-*`; built by `el-views` and `lo-arrange`)

- **28 high**, 20-px keys at y 4, gaps 12, edges 12. Left: the panel's name (Medium 13), the
  tools as a radio set of six 26-px keys (the chosen a lifted `raised` pill with a 1-px light
  top; never coloured), the grid ("Grid" 11 ink 3 + a 58 well). Right: Follow (a 24 key +
  "Follow" 11) and the panel menu (a 24 key).
- Fold order: the grid's word → Follow's word → the name to its glyph → the tools to the
  chosen one and a chevron (drawn at 1032, 860, 700, 560, 420, 340 px; `clips_chrome::fold`).
- The grid menu in musical order with a check, Triplets (Shift+3) and *Follow the zoom*
  (ADR 0114). **As built**: "1/2" where the sheet says "½ bar" (in 3/4 a half note is not half
  a bar) — the owner's to decide.

#### 8.4 Dialogs (`dir-a` `dlg-*`, `des-controls` `dialogs-*`; built by `el-topbar`, `el-controls`, merged into `kit::dialog`)

- A scrim over the window (Night `rgba(6,9,12,.58)` / Paper `rgba(32,44,56,.34)`, the token
  `depth.scrim`); the frame `surface`, r 10, one shadow (dy 14), a lit top edge, a title row
  44 high (Medium 15, a 26 × 24 close key); a footer band 56 high with the buttons at the right
  10 apart, a quiet *Reset* at the left and an *Esc cancels* keycap where there is room.
- Buttons 28 high (24 compact), r 4, Medium 12, minimum width 84: **primary** an accent fill
  with `primary_ink` (one per dialog, at the right end; Enter presses it), **secondary** a
  well, **destructive** `danger` with white (its dialog's focus starts on Cancel), **quiet**
  words in ink 2; each with rest, hover, pressed, focus (a 2-px ring at 60 %), disabled and
  busy (a turning arc).
- Settings: 888 × 590, the side list on `base`, the chosen page a raised pill with a 2 × 14
  accent bar, rows 32 apart with a 150-px label column, section titles SemiBold 11 ink 3.
  Audio's buffer drawn as a latency scale (green to 6 ms, amber to 12, grey past) with a
  sentence; Appearance with the families as pictures. First run in four numbered steps;
  export with its presets at the left and a 4-px accent progress bar while it runs.
- **As built**: one frame for every dialog of the recessed family (`kit::dialog`: unsaved
  changes, recovery, the record mode's questions, the demos, MIDI import and export, the
  shortcuts, update, about, stems, templates, quantise audio, transcription, the clean
  editor, video export, the layout's name, the dance import); Settings, first run and export
  keep their layout on it.

#### 8.5 The controls (`des-controls`; built by `el-controls`)

- **New tokens** (both lights, in every theme file and the theme editor): `accent.modulated`
  (`mod`, Night `#EE86C8` / Paper `#B83A8A`, moved to `#B23886` for 4.5:1), the fader caps
  `mixer.fader_cap_top/middle/foot` and knob caps `knob.cap_lit/middle/shade`,
  `accent.destructive` (`danger`), `accent.primary_ink`, `depth.scrim`, `text.disabled`,
  `meters.mid`; `accent.automated` is Slate's `auto` (`#B48DF4` / `#7A4FD6`, Paper moved to
  `#784DD2`). Each is held to its contrast in `Tokens::pairings` and `floor()`.
- **Knobs** in three sizes — S 20 (strips), M 32 (panels, the inspector, effects), L 52 (an
  editor's main controls) — with a 270° groove, a value arc (from the top when bipolar, a
  notch above), a cap lit from above (shadow, rim, radial light, crescent) and a pointer;
  rest, hover, dragged (accent), focused, disabled, automated (`auto` arc, pointer and value),
  automated and held (a violet tick where the clip would put it), modulated (a `mod` swing
  band at 38 % and a live dot), MIDI learned, stepped (detent dots). M: name above in Regular
  11 ink 2, value below in Regular 12 ink 1 with its unit a size down in ink 3.
- **The driven signs**, each a shape and a colour of its own, one entry point
  (`kit::automate::signs`, ADR 0109): automation a lane with two nodes in `auto`; a modulator
  one period of a sine in `mod`; a MIDI controller a socket in the accent; a VCA a V in ink 2
  and a dashed ghost cap at the VCA's level.
- **Faders**: a 2-px groove, the cap 20 × 18 r 2 (`fcap` gradient, lit top, grips, an
  engraved index), a 0 dB detent tick either side, the scale on the right; the meter as two
  3-px bars in wells of their own, colour cut from one gradient by height, a 1-px peak hold, a
  clip lamp; the master's travel 196 with 5-px bars and a numbered scale. **Sliders** (a 3-px
  groove, a 9 × 15 thumb) and **value bars** (a 20-px well filled to the value, the name left
  and the number right).
- **Fields** 24 (compact 20), r 4; typing is the well to its edge in a 2-px accent ring at 55 %;
  invalid in `rec` with the allowed range; **drop-downs** with a 7 × 4 chevron in an 18-px
  zone, up when open.
- **Menus**: a `raised` sheet, r 6, 24-px rows, a 26-px check column, shortcuts in ink 3, a
  submenu's chevron, Title-case headings (never bars), the hover an inset accent pill; **in a
  popup the popup's frame is the sheet** and each list paints its rows only (no boxes inside
  boxes); a menu taller than the window scrolls (`kit::menu::fit`).
- **Switches**: segmented controls (a well, the chosen segment a lifted neutral pill, never
  coloured), panel tabs (cut from the body; an accent bar only while the panel has the focus),
  switches 28 × 16, check boxes and radios 14, the enable lamp (a lit `play` disc with a halo,
  or a hollow ring), floating scrollbars (a 4-px thumb at rest; a lane and grips under the
  pointer), the focus ring a hairline 2 px out.
- **Tooltips**: `raised`, r 5, up to three lines (name and value Medium 12; what it does
  Regular 11 ink 2; how, Regular 10.5 ink 3 with keycaps), the driven sign leading; keycaps
  15 high (`kit::hint::keys`). The help line never empty (ADR 0109).
- **As built**: Classic families (high contrast, users' older themes) draw every control as
  before; the knob's groove and cap and the fader's cap are one cached layer each. The sheet's
  own questions stand for the owner (the automated knob's violet, the fader cap's weight,
  Paper's menus and dialogs, value bars in the inspector, Mute on = grey).

#### 8.6 The mixer and its inspector (`des-mixer`; built by `el-mixer`, `lo-mixer`)

- **The desk**: a 28-high header on `surface` (the density, **Rows ▾**, a solo chip
  "1 soloed · Clear", the cable view's key, Snapshots ▾, `+`); the body `base`; the master at
  x 8; strips at a pitch of width + 2 (value between strips, never borders); the right dock
  (VCAs, then returns) behind a 1-px seam; overflow fades with "‹ 6" / "6 ›" pills; a 3-px
  scrollbar only when there is something to scroll.
- **Widths** Compact 30 / Wide 60 / Large 94 (master 40 / 68 / 110).
- **Rows in signal order** (Wide; Compact and Large in the sheet): head 36 · inserts 68 (4 ×
  16) · EQ 22 · polarity, swap and width 16 · pan 28 · M · S · arm 16 · the fader and meter
  (flexible, min 112) · level 16 · sends 28 · output 14 · socket 18; gaps 4 (Large 6). A short
  desk gives way in this order: EQ, polarity, the inserts to one summary line, the sends, the
  socket, the level, the output — then scrolls. No label column, no "−∞", no empty slot rows.
- **The fader's law**, shared by the meter so one tick reads both: +6 → 0, 0 → .16, −6 → .31,
  −12 → .45, −18 → .56, −24 → .65, −36 → .79, −48 → .88, −∞ → 1.
- **States**: playing, clipping, armed, muted (stripe 32 %, name ink 3, meter 38 %), soloed,
  silenced by a solo (M ringed in solo at 45 %), selected (body tinted 13 %, stripe whole),
  bypassed inserts, sends off, sidechain key, VCA and follower, frozen (the sound block
  hatched), empty (number, groove, a ghost cap, the socket).
- **The inspector** (246 wide at 1280): 12 padding; the head 28 with a 3-px stripe; Source a
  28-high well; sections under a hairline, open titles SemiBold 11 ink 3, folded rows 28 high
  with their summary; insert rows 26 high with the lamp and a mini view (46 high, three values);
  the EQ a 72-high graph and a band table; sends 26 high with a 64-px level bar; Routing,
  Sidechain, Recording, Automation (a mode segment, a 56 × 12 sparkline per lane, *Show in the
  arrangement* in the accent), Level (Volume, Pan, Width as bars of 96, the VCA, the safe
  toggles 26 × 14), Freeze (a key and what it did).
- **As built, on purpose**, each said to the owner: the owner's four choices of 2026-11-09 —
  M|S as one segmented pill and the arm a dot in the head (option C), polarity and swap one
  pill, the send level a small knob, cables off at rest with words under a strip and B's curves
  on demand, the sidechain key saying "← Kick"; a key's target is its share of the strip (Wide
  20 × 24, Compact 15 × 20), not 24 × 24; the header shares the workspace's tab row when the
  room allows; the inspector's labels sit in an 80–120 column (the sheet's 64 cut Spanish);
  Level keeps a slim meter (the old pinned strip's); Cantus keeps what the sheet does not draw
  (Receives, the inserts' and EQ's power, an insert's Mix and Sidechain rows, the delay
  compensation, the trim, Mute · Solo in Level, the Modulators fold); the right dock keeps
  round 37's place after the last strip. The sheet's *Hear the key* and *Record: Before FX ·
  After FX* are not built (the engine has neither; BACKLOG §6j).

#### 8.7 The routing cables (`des-mixer`, *The routing cables* and *Cable style D*; built by `el-cables`)

- A style per theme (`kit::system::System::cables`, **Flow** in every family today), which the
  user may override (Settings ▸ Appearance, the Cables menu): **Patch** (one cubic per cable,
  sag k = min(58, 16 + 0.15·|dx|)), **Flow** (Patch with a 7-px gleam every 64 px moving at
  40 + 80·level px/s, from the transport's position), **Subway** (orthogonal lanes below the
  jacks, outputs +14, sends +26, keys +40, sub-lanes 4 apart, no two overlapping spans at one
  depth) and **Jack** (seated and free plugs rasterised once per shape and colours, a catenary
  sag s = min(52, 12 + 0.16·span), a 300-ms settle after a drop, then still).
- Jacks: two per strip in the 18-pt foot row, input at −11 and output at +11 from the centre;
  strokes with an under-stroke; widths output 2, send 1 + 1.6·level, key 1.5 dashed; tokens
  derived from the theme (`cable.out/send/key/refused/channel`).
- Hover lights a whole route to the master and dims the rest to 22 %; *Show all* draws the
  others at 45 %; a drag says what it will do or why it is refused. No style adds an
  allocation or a draw call; at rest none asks for a frame.

#### 8.8 The piano roll, the rack, the browser and clip content (`des-views`; built by `el-views`, `lo-rest`, `lo-arrange`)

- **New tokens, derived** (`views::room`, from each theme's tokens, not spelled out): the
  keyboard's bed `sep` (Night `#59636E` / Paper `#A3ADB8`), `ivory` (`#CFD5DB` / `#FBFCFD`)
  and `ivory_hi`, `ebony`, `ebony_top`, `ebony_front`; in Slate they land on the sheet's values.
- **Piano roll**: the head 28 (the clip menu a 128 × 20 well, the tools a joined well of
  6 × 26 × 20, the grid 54 × 20, word lamps, the selection readout at the right); the ruler 24
  in two tiers; the keyboard 56 / 72 / 88 wide at 8 / 12 / 18-px rows, each white key its own
  piece with a 1-px gap, black keys 0.60 deep and one row high with a 1-px lit edge and two
  shadow steps, every C named (middle C SemiBold 10); notes the row's full height, r 2, filled
  by velocity with a brighter attack column; ghosts inset a pixel, no edge; the velocity lane
  106 high. **As built**: the keyboard's corner is the sheet's *Rows* well (Compact 8, Default
  12, Tall 18), not FL's note tab; Paint and Playback keep the kit's glyphs.
- **Channel rack**: rows 30 (a kit's pad 26) with a 3-px stripe; the lamp at 16, knobs at 36
  and 58, the mixer track 24 × 18 at 72, the name plate at 104 (150 × 20, r 3), the target mark
  at 262; a kit's pads indented 14 with a tree line, its chevron folding them; steps 18 × 22 at
  a pitch of 20 (+4 a beat, +12 a bar), filled from the floor to the velocity; melodic and audio
  rows as one well over the page; the pattern map 8 × 22 a bar; the foot's *Add a channel*
  150 × 24 and a 56-high dashed drop zone. **Not built**: the well for a pad's own mixer track
  (a model change; the owner decides, BACKLOG §6j).
- **Browser**: 288 wide; rows 22 (search results 36 with the place under the name), one mark a
  row type (a pack its cover, a folder the folder glyph, a sound its waveform); the selection a
  lifted well with a 2-px accent stripe; the match lit in the accent; chips 20 high, r 10; the
  preview card 118 high (name Medium 13, the star, a 38-high waveform well, play, loop, a 2-px
  level slider, the numbers); pack covers 128, r 8, drawn from their content, with Installed /
  Get / Getting. **Not built**: the kinds switch *All · Sounds · Plug-ins · Presets* and its
  chips (a structural change; the owner decides, BACKLOG §6j).
- **Clip content at four zooms** (px a beat: far < 6, default 6–20, near 20–60, close ≥ 60):
  MIDI as a pitch contour → rows ranked by pitch → true length by velocity → plus an attack
  column; drums as a pulse → a row a piece (hats at 62 %) → hits at their length; audio as
  loudness → the peak envelope → peak and loudness in two tones (L/R from 44 px lanes);
  automation as a line → points → larger points → handles and values (Medium 9 ink 2). A
  13-px name strip in lanes of 44 and taller.

#### 8.9 The arrangement (`des-arrange`; built by `el-arrange`, `lo-arrange`; ADRs 0113, 0114)

- **Layout**: the tool row 28; top lanes — the ruler 24, sections and markers 20, tempo 40 (on
  demand); the song map 14 at the panel's foot; the header column 120–360 (200 by default);
  track heights 24 / 32 / **44** / 64 / 96 and any height from 24 to 240 by a drag (Alt: every
  track); a folder's row 30; an automation lane 36.
- **The header**: `surface`, the user's colour a 3-px stripe (whole when selected, ink 4 when
  muted), the number right-aligned at x 24, the name at x 32 (Medium when selected), line 2 at
  y + 33 with the kind glyph; M S ● A as letters at rest, one recessed well under the pointer,
  only the engaged key lit (M an ink-2 pill, S solo, ● rec, A auto); a slim meter at the right;
  line 3 a level slider from 60 high, line 4 the inserts in words from 88.
- **Folders**: open a 30-px row and one quiet body with the members' clips as 5-px bars;
  closed a 44-px row "Folder · n tracks → Drum bus" with 8-px bars; children indented 12.
- **The ruler**: the loop as a band in its top 5 px with 2 × 7 ends; bar numbers Medium 11
  from 26 px a bar; beats named "41.2" from 40; the playhead a 9 × 7.5 accent cap, where play
  started a hollow cap. Sections as named spans with their length; markers as a line, a
  pennant and a name.
- **The canvas**: 4-bar groups by value; clip states rest, `lit`, `sel`, `play`, `mute` and
  `dim`; the moving clip's dashed slot and shadow; the resize edge's light; a 9-px loop notch;
  the drop sentence below the last track; a veil after the song's end; the empty song's three
  starter wells 236 × 64.
- **As built, on purpose**: a section drag moves the section, not its bars on every track (the
  ripple is a feature of its own, BACKLOG §3); `dim` steps 62 % toward the lane (notes 32 %)
  rather than the sheet's × 0.6, to read as silenced; maximise is ADR 0110's (the area takes
  the whole workspace and the hidden areas, the inspector among them, wait in the row of tabs)
  where `max-*` keeps the inspector — the owner's to decide.

#### 8.10 Spacing, adopted

Every element above is built on the 2-px unit; the rows (track 44, panel head 28, top bar 44,
status 20) and the keys (16 / 20 / 24) are what the sheets measure. Where a build moved a
measure from its sheet, the builder's report says so and the list above repeats it. The
budgets hold the look to its cost: no more draw calls or triangles than beta.13's whole
window, and nothing asked at rest (`slate_budget`, `frame_budget`, `idle_repaint_e2e`).

### 9. The logo: #03, "the metal" (the owner, 2026-11-09)

Logo #03 is the default logo (`Logo::DEFAULT`; #2 before) and the application's icon. Below
48 px it is drawn from its designer's small masters (`assets/logos/small/03-{16,20,24,30,32,
48}.svg`, byte-identical to `docs/round39/design/logo/m03/`): the tile filling the square,
the C thicker, the waveform in whole-pixel bars, the rim and the lozenge terminals from 30
px. The executable's `.ico`, the Linux icon sizes, the macOS `.icns` and the splash are
rendered from it at build time (`cantus-app/icon.rs`). The running window's and taskbar's
icon (ADR 0046) is #03 in its own colours (*Automatic*, *Original*); a colour the user
chooses still paints the mark in it. A user's saved logo is kept. No accent changes with
it: Slate's accent stays as it is (the Lapis direction ships as a theme of its own).

## Consequences

- Every view inherits the family's colours, face, keys and wells from the kit; views that
  paint their own shapes (strips, piano keys, clips) take the element sheets in turn.
- A theme file written now carries `system`; an older build ignores the field and shows the
  colours in its own way.
- Text measured in Plex before (fixed widths in a view) may now fit differently: the
  no-overlap tests and the shoots per theme hold it.

## What the tests hold

- `theme::tests` — the default is Slate Night with the design's values; every shipped
  theme meets the kit's promises and round 25's floor; high contrast at 7:1.
- `kit::appearance::tests::every_old_preset_id_names_its_own_theme_again_keeping_the_slot_s_changes`
  (the retirement reversed; an old `theme.json` that is exactly an old preset opens in it).
- `tests/themes39.rs` — every redrawn and new theme keeps every promise as written and
  resolves to its tokens; is of round 39's system (recessed, a round-39 face, flat); stands on
  Slate's ladder in its light; the restored keep their light and id, the FL pair is renamed,
  no name collides or carries a mark; no two themes of a light are closer than ΔE 14 (room
  and half the accent; Slate and Slate Lapis are 16.8); Slate and Slate Lapis untouched.
- `views::arrangement::legibility` — the measure above, on the arrangement's own painters.
- `tests/themes_whole_words.rs` — every theme started through `assemble` in one process at
  1920 × 1080 125 % and 1600 × 900 100 / 125 %: the menus whole (Help kept), no word elided
  (the widths' caches are keyed by a process-wide fonts number, `kit::cache::fonts_seen`).
- `tests/themes_e2e.rs` — through `assemble`: Cantus Night (redrawn) and Print (new) chosen on
  Settings ▸ Appearance, each painting its room (and Cantus Night's verdigris on the chosen
  row, Print's vermilion playhead), kept across a restart; beta.13's `ultraviolet` /
  `parchment` and a round-24 `theme.json` open in their themes; the page holds a tile for
  every preset, none overlapping or cut, the last scrolled into view.
- `kit::system::tests`, `kit::faces::tests` (every role names a family; the new faces'
  figures are tabular, read from the files).
- `tests/slate_e2e.rs` — a fresh install boots through `CantusApp::assemble` in Slate
  (tokens, keys, face), a beta.13 setting naming Cantus Night and Daylight opens in them
  (redrawn), every theme switches through `apply_appearance` and back to Slate exactly.
- `tests/slate_budget.rs` — ADR 0108's budget.
- `kit::icons::tests` — every family glyph drawn at every size, its inks all named, inked.
- `tests/slate_lapis_e2e.rs` — Slate Lapis chosen in Settings ▸ Appearance through
  `assemble`: its night-blue room, accent, tint and gold installed, the chosen page's row
  laid with the lapis, the playhead and the ruler's numbers painted in the gold, kept across
  a restart; Slate's own tokens leave all three clear.
- `theme_slate_lapis::tests` — only the room, the accent's tones, the tint and the metal
  differ from Slate; the room is Slate's at the same luminance turned to lapis (text
  contrast within 5 %); the readouts are gold at 4.5:1 on the bar, its wells and the panel.
- `tests/slate_window_golden.rs` — Slate Night and Paper's whole window (the arrangement,
  the mixer in Wide, the piano roll, each with a selection) against golden images.
- The element sheets' builds, each through `assemble`: `topbar`, `topbar_layout` (the widths
  sheet at every width, zoom and language), `menus_in_window`, `mixer39_e2e`,
  `mixer_icon_e2e`, `cables39_e2e`, `el_views_e2e`, `arrange_e2e`, `lo_arrange_e2e`,
  `rack_foot_e2e`, `samples_browser`, `idle_repaint_e2e`; `kit::glyph_mixer`,
  `kit::glyph_browser` and `kit::glyph::family_tests` (every set-3, -4 and -5 glyph at every
  drawn size).
- `screen_scale::tests` — 125 % at 1 920 and the rule's table; a fresh install follows the
  screen, an old file keeps its scale, a user's choice stops the following.
