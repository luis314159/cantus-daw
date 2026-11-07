# Round 37 — finish: first delivery (steps 1–3)

*The finish builder (ADR 0103). Branch `r37/finish`. This is the first delivery — the
material tokens, the controls redrawn as objects, one icon family — for the integrator to
merge early; steps 4–8 follow on the same branch.*

## What changed

**1. Material tokens per theme** (`kit/tokens.rs`, `kit/light.rs`, `theme_family.rs`).
Six tokens in the *Depth* group, each a colour whose alpha is its strength:
`depth.bevel`, `depth.bevel_shade`, `depth.inner_shadow`, `depth.lip`, `depth.specular`,
`depth.drop` — named in both languages. All twelve shipped themes define them: Night as
warm metal and glass (a hot specular, deep inner shadows), Paper and the light themes
matte, the cool dark themes between, the high-contrast pair firmer. `light::Material`
scales them by the theme's depth. Saved custom themes need **no format change**: a theme
file without the six keys derives materials of its kind (dark or light) and saves them
from then on — tested. The Night knob body is a step lighter (`#3a2b21`) so a knob reads
as a lit object.

**2. Controls as objects** — new recipes `light::object` (drop, silhouette, face lit from
above, bevel light and shade; pressed = sat down with the lip's shadow), `light::recess`
(lit lower lip, rim, hollow floor, inner shadow), `light::glass`; cached layers for the
small round objects (`kit/raster_parts.rs`: a lamp in its socket, a knob's well).
- Knob (S/M/L/XL): stands in a round well; track in the well; value arc from the default;
  the cached body has its shadow, a skirt lit from above, a domed cap with a bevel and the
  specular; the pointer casts a shadow. Mini knob (mixer, rack): the same object smaller,
  a matte cap in light themes, the value arc in the dark ring of its well.
- Fader: a slot cut into the strip; a cap with a drop shadow, a lit upper face, an
  engraved index line, a darker lower face, sides turned away, the specular across its
  top. Selected: the index line and the slot below in the selection colour, the face
  tinted a little (no lime slab).
- LED: glass in a socket, lit from inside (hot top, colour, darker foot) or dark tinted
  glass; the specular; a tight halo (radius 1.9 r, 55 %) instead of the haze (2.6 r, 90 %).
- Caps, LED buttons, toolbar keys: raised at rest, sat down while held. **On = sunk in,
  the highlight's tint across the floor, a thin highlight rim and a lit line at the
  foot, the glyph in the highlight** — no orange block. The top bar's view keys are the
  kit's `cap::key_strip` (one marked call in `transport.rs`).
- Fields, menu fields, readouts (recessed displays under the theme's glass), segmented
  controls, the meter's well (lip and inner shadow, still one batched mesh), rack slots
  (bevels).

**3. One icon family** (`kit/glyph.rs`, rewritten). A glyph is a list of ops in units of a
12-unit grid; no op carries a width or a radius, so every glyph has one stroke (1.2 units,
whole physical pixels, short strokes snapped to the pixel grid) and one corner radius;
two tones (ink, and a 30 % tint inside closed shapes); one optical size (ink within
±5.7 units, none under 8 units across). 63 glyphs: the 61 there were, all redrawn, plus
`SendOpen` and `Mono`, which the mixer drew on its own (now the kit's, marked lines).
The Session and Stage marks, which views drew, are in the family. A toolbar key gives its
glyph 0.72 of its side (was 16 px of 28, now 20 px).

## Pictures

`docs/round37/finish/` — before (round 36) / after, Night and Paper, 100 % and 150 %:
`strip-*.png` (two mixer strips, magnified), `keys-*.png` (the top bar's view keys,
magnified), `topbar-*.png`, `mixer-*.png` (the desk); `icons-*.png` (every glyph at 14,
18, 24 pt, on a key, lit) and `controls-*.png` (knobs, mini knobs, faders, LEDs, a
readout, a field, keys in four states, LED buttons). The side-by-side crops against FL
Studio are private (not committed).

## Tests

- New `tests/kit_glyphs.rs` (4): live area, optical size, centring, convex fills; ink
  within 0.35–2.4 × the median at 18 pt × 1, 18 pt × 1.5, 24 pt × 2; whole-pixel strokes.
- New in `kit::tokens`: every shipped theme defines its materials with their character
  (bevel lighter than its shade, shadows dark, the specular a highlight; Night glossier
  than Paper); a theme file without them derives them by its kind and keeps them.
- `tests/kit_tokens.rs`: every part registers exactly the tokens it paints with (the
  material tokens added to knob, mini knob, fader, LED, LED button, cap, toolbar key, well,
  readout, menu field, segmented, meter, tile, the dial skin's knob).
- `tests/kit_gallery.rs`: golden images **regenerated on purpose** (see below).
- Frame budget: see below.
- **The whole `cantus-ui` suite on Windows**: every target green — 2 365 passed, 40
  ignored, 0 failed. (The first run was cut half-way by a session break: the targets from
  `record_mode` on died with `STATUS_DLL_INIT_FAILED` and `record_mode` timed out baking;
  those 66 targets and the doc tests were rerun and all passed.)
- `cargo clippy -p cantus-ui --all-targets` with `-D warnings`: clean. `cargo fmt -p
  cantus-ui`: applied. Linux (WSL, `tools/wsl/check.sh r37/finish clippy`, the whole
  workspace with `-D warnings`, and the plug-ins without their editor): clean; the
  clone's `target/` deleted.

## Frame budget

`tests/frame_budget.rs` (debug, triangles a frame, worst of the run), the base commit
`7c0bc748` against this branch:

| Scene | Before | After |
|---|---|---|
| mixer64, desk alone | 90 685 | 69 653 (−23 %) |
| mixer64 wired, cables all | 96 650 | 76 011 |
| mixer64 wide | 67 121 | 53 766 |
| mixer16, desk alone | 25 519 | 20 375 |
| mixer64, whole window (budget 121 000) | 120 478 | 97 775 (−19 %) |
| mixer16 + a track, whole window | 48 158 | 41 900 |

Allocations and bytes a frame unchanged (19 / 21 KB desk, 316 / 235 KB window), draw
calls unchanged (9 / 87). A first draft of the objects as plain geometry cost 153 038
triangles in the window; caching the lamps and the knobs' wells as layers (as the knob
bodies already were) and drawing the bevels and inner shadows as plain quads brought it
under the old figure. `workspace_budget.rs` holds.

## Marked lines (`// r37:finish`)

- `views/transport.rs` `key_strip`: the strip's drawing is `cap::key_strip` (4 lines).
- `views/mixer_strip.rs` `mono_glyph`: the kit's `Glyph::Mono` (2 lines).
- `views/mixer_parts.rs`: the hollow send arrow is the kit's `Glyph::SendOpen` (2 lines).
- `theme_family.rs`: the six material tokens in each of the eight family themes (data),
  and Night's knob body (1 line marked).
- `examples/shoot_readme.rs`: `CANTUS_SHOOT_THEME`, `CANTUS_SHOOT_PPP`, and the shots on
  a thread with a large stack (a debug build overflowed the main thread's).

## The thirteen elements now (my judgment; the owner's eyes decide)

| # | Element | Now | Why |
|---|---|---|---|
| 1 | Top bar | FL | the bar's layout and type are step 7/4; only its keys changed |
| 2 | Transport, tempo, clock | FL | step 7 |
| 3 | Toolbar icons | FL, narrower | one family now and *on* is a tint and an edge, but the bar's keys are 22 px with 16 px glyphs against FL's ~24 px glyphs — the bar grows in step 7 |
| 4 | Whole mixer | split | unchanged in kind; tint and vertical names are steps 4–5 |
| 5 | One strip | FL | its controls are objects now, but the strip's panel is flat, untinted, its name horizontal |
| 6 | Fader, LED, knob | tie | knob in a well with a dome and specular, glass LED, cap with index and specular; FL's chrome cap still has more presence (larger, more contrast) |
| 7 | Arrangement | FL | step 6 |
| 8 | Headers and clips | FL | steps 5–6 |
| 9 | Channel rack | Cantus | kept; its mini knobs are objects now |
| 10 | Piano-roll keyboard | FL, narrowly | step 4 (labels) and the keyboard's drawing |
| 11 | Parametric EQ | Cantus | kept |
| 12 | Browser | FL | the library builder's rows |
| 13 | Name type | FL | step 4 |

Tally: FL 9, Cantus 2, split 1, tie 1 (was FL 10, Cantus 2, split 1).

Known nits for the next pass: in Paper a lit LED of the dark `led` green still reads
darker than the old haze made it look; the rack and session glyphs' faint cells are near
noise at 16 px.

## Steps 4–8 (next)

4. **Type**: menus, hint line and labels at a reading size; an OFL condensed face for
   narrow places (licence in the notices); vertical names on narrow strips; text
   sharpness checked in the real release app at 100/125/150 % and the doubled note labels
   fixed.
5. **User colour**: a stronger palette with a minimum-distance test per theme; the whole
   strip, rack row and track header tinted by its colour; rack name buttons in colour.
6. **Content you can see**: notes, waveforms and automation at the clip's full height in
   a lighter shade of its colour; a clip's name only where it differs from its section;
   previews in the clip browser's rows.
7. **The top bar**: 32–36 px keys with 20–24 px glyphs, a transport capsule, a round
   record, a large light clock, the tempo as a display, system meters with numbers, no
   gaps; the strip panels' relief.
8. **Acceptance**: the thirteen crops in both themes at 100/150 %, budgets held, UI tests
   green, accessibility unchanged; README and site pictures re-shot.

## Open (owner)

- The owner's eyes on the before/after crops and on the real app at 100/125/150 %.


---

# Second delivery: the owner's bar — priorities 1 and 2 (and a start on 3–5)

The owner's verdict on the first delivery: *"¿Es lo mejor que puedes hacer? Haz algo
excelente hasta que tenga el detalle y la calidad superior a FL Studio."* The integrator's
read: the change barely read at 100 %; the keys' active state was weaker than the old
orange; grid glyphs were noise; Paper's lit LED was dimmer. New bar: every element must
**win** at 100 % first.

## What changed

**Strips (priority 1)**
- Every strip is a **plate** (`light::plate`): lit from above, bevelled edges, the whole
  strip **tinted by its colour** (strongest at the top), the number tile in full colour
  with its bevels. The tint is on by default (*Colourful: Medium*, 24 %).
- A **pod**: the meter, the lamps and the pan knob set in one hollow (a recess), as a
  console's top section is.
- **Larger controls where the strip has room** (`mixer_layout::grow`: Compact 1.0, Wide
  1.15, Extra large 1.4): pan and width knobs 28 pt, lamps 12.6 pt, the fader's cap
  20 × 31 pt; the rows that hold them grow with them (marked lines in `mixer_layout.rs`).
- The chain, output and pan-law boxes are **chips** standing off the strip
  (`light::chip`), not black holes in it; Night's well a step off black.
- Vertical names are set whole where a size down fits ("Percussion", not "Per…on").
- **Stronger track colours** (`safe_track`'s bands: saturation 0.42–0.72 where FL keeps
  0.25–0.40), Night's veil over large fills lighter (110 → 50).
- Paper's lit lamp is bright again: a hotter core and a fuller halo in light themes.

**The top bar (priority 2)**
- Drawn **a size up, following the window's width** (`transport::bar_scale`: as before to
  1366 pt, 1.12 at 1920, 1.2 from 2400): keys of 31 and 27 pt at 1920, words, clock and
  tempo larger; the bar is 66 pt tall (was 54). At 1920 undo and the inputs fold into ⋯
  (Spanish also the system meters); from 2560 nothing folds (tests updated, documented).
- **The transport is a capsule** with a round red record; other key rows are **separate
  caps**, each an object; **a key that is on is lit through in the highlight** (a lit dome
  with its bevel, a dark glyph) — unmistakable in both themes.
- **The clock**: bar and beat large, the ticks a size down; **the tempo**: the whole BPM
  large, its tenth small; the clock is a recessed display under glass.
- **System meters** (CPU, RAM, voices, dropouts) narrower so they stay on the bar, in a
  recessed well.
- Glyphs redrawn for their real size where they read as noise (rack, session, piano
  roll); the menus' gaps tightened.

**Content and the rest (a start on 3–5)**
- **Clips**: a clip whose name repeats the one above it at the same bar is **quiet** — no
  title bar, its content at the clip's full height; notes drawn a row per pitch in use,
  centred (a drum line sits across the middle), in a lighter shade of the clip's hue.
- **The rack**: every channel's name button in its own colour, as an object.
- **The piano roll**: every C a shade down.
- **Type in the real app**: the release build captured at 100/125/150 %
  (`tools/look/shots_of_cantus.ps1`, its own data folder): text is crisp, grey-scale
  anti-aliased, with no dark outline — the soft edges seen in the CPU shots are the CPU
  renderer's, not the app's.

## Frame budget (debug, triangles a frame)

| Scene | Base (round 36) | First delivery | Now |
|---|---|---|---|
| mixer64, desk alone | 90 685 | 69 653 | 79 273 |
| mixer64, whole window (budget 121 000) | 120 478 | 97 775 | 107 346 |
| mixer16 + a track, whole window | 48 158 | 41 900 | 44 595 |

Still below round 36 everywhere: the plates, pods and chips cost what the cached lamps and
wells saved.

**A pre-existing flake, not this delivery's:** `frame_budget`'s
`stage_budget::a_dancer_on_screen_stays_in_budget` fails when run alone (+3 552 triangles
with the dancer floating) and passes in some full runs; the first delivery's sources (the
merged `37c14d15`) fail it alone too (+3 812). Order-dependent — something shown once per
process lands in the floating rig's frames. For the integrator / the dancer's owner.

## Tests (second delivery)

`cantus-ui` lib (451), `kit_gallery` (re-blessed on purpose: cap, fader, led, led_button,
tile, toolbar_key, cable), `kit_glyphs`, `kit_tokens`, `kit_lint`, `kit_parts`, `topbar`,
`topbar_layout` (updated: the bar's scale follows the width — documented folds at 1920,
none from 2560; wants computed at each width), `frame_budget` (but the flake above),
`workspace_budget`, `mixer_desk`, `mixer_routing`, `mixer_vca`, `views_shots`,
`theme_roundtrip`, `appearance_look`: green. Clippy `-D warnings --all-targets`: clean.

## The owner's elements now (my harsh read at 100 %, Night; blind pairs in `F:/projects/daw-r37/tmp/finish/judge/`)

| # | Element | Now | Why |
|---|---|---|---|
| 1 | Top bar | tie | denser and it carries more (clock, tempo, meters, chip); FL's keys and words are still larger and calmer |
| 2 | Transport | FL, narrowly | ours a capsule with a red record, a big clock and tempo; FL's transport is a 44-px instrument, ours a 27-px row |
| 3 | Toolbar keys | tie | lit keys now unmistakable, separate caps; FL's glyph line work is still finer |
| 4 | Mixer | Cantus | whole-strip colour, pods, objects, numbers, chains, sends, cables; FL is calmer |
| 5 | One strip | Cantus, narrowly | colour, pod, larger knob, glass lamps, readout, chips; FL's vertical name and icon still read better at a glance |
| 6 | Fader, LED, knob | Cantus, narrowly | domed knob in its pod, glass lamp, metal cap with index and specular |
| 7 | Arrangement | Cantus | distinct colours, hits and notes at full height, sections named once |
| 8 | Headers and clips | Cantus | saturated tiles with number tab, icon, controls; content visible |

# Third delivery: the blind judge's list, the owner's fader and the 24-pt targets

The blind judge (Cantus 7 of 8, FL Studio narrowly on the toolbar keys) named the gaps; the
owner asked twice more on the way: *"Me gustaba más el mixer fader original"*, then *"Los
dos están chidos, ¿puedes hacerlo configurable?"*, and *"los tres botones (fase, swap,
mono) son muy chicos y es difícil darles click"*. All of it is closed here.

## What changed

**The fader, the owner's choice** (`kit/fader.rs`, `kit/appearance.rs`, `views/appearance_look.rs`)
- Round 36's fader is back, exactly — long throw, its scale, its cap — as **Classic**, the
  default. The second delivery's metal cap is kept as **Metal**. *Appearance → Fader*: a
  segmented control (arrows move it, the screen reader reads "Fader: Classic"), its tip
  and caption in both languages; it switches live, everywhere a fader is drawn (strips,
  VCAs, master, the units) — one global the painters read (`fader::set_cap`, set in
  `apply_appearance`, which `assemble` calls).
- Stored with the appearance settings (`appearance.fader`), not the project: no schema
  bump; a settings file without it reads as Classic (tested, with a round trip).
- The kit gallery has a golden for each (`fader` and `fader_metal`, Slate and Paper, 1×,
  1.5×, 2×). Pictures: `fader-classic-{night,paper}-100.png`,
  `fader-metal-{night,paper}-100.png`, `fader-restored-{night,paper}.png`.

**Every target on a strip is a real one — 24 × 24 pt at least** (`mixer_layout.rs`, `mixer_strip.rs`, `mixer_parts.rs`, `mixer_paint.rs`)
- Phase, swap and mono are **keys** (the kit's `toolbar_key`): the family's glyph, lit
  through in the highlight while on, a hover state, a tip that names the action and its
  state ("Phase: on"), AccessKit as a checkbox with the same words; side by side where
  the strip holds three, a second row in Compact (mono is not on a Compact strip).
- The same floor for mute and solo, FX and record, the send's lamp, its POST/PRE chip and
  its row, "+" (add a send), "+n", the master's polarity key, the ratio and band handles.
  The rows that hold them grow with the width mode (`keys_height`, `layout_target_row`).
- `tests/mixer_targets.rs`: in Compact, Wide and Extra large, at 75, 100, 125, 150 and
  200 %, every strip's mute, solo, phase, swap, mono, FX, record and send target is at
  least 24 pt each way, and no two of a strip's targets overlap.
- Pictures: `strip-buttons-{night,paper}.png` (before and after).

**The top bar**
- The **transport is the bar's whole height** (first in the bar, an instrument as FL's
  is), the record a round red button; PAT / SONG are two word keys, the lit one in the
  highlight, over the tempo.
- The displays (clock, tempo) in **IBM Plex Mono Light** (OFL, in the notices beside the
  other Plex faces); values in Plex Mono Regular, a finer hand than Medium. The clock's
  width is measured as it is drawn (the ticks a size down), not as all-display digits.
- The **audio chip never cuts a name**: where its whole line does not fit it shows the
  short one (name · latency), and that short line is the chip's least on the bar.
- Menus in the Title role on the larger bar, the project's name a label, the rest of the
  menu row in the label colour.
- Pictograms told apart at their size: the **browser** is a folder and its tree, the
  **inspector** a panel of three values on their tracks (they were both "a panel at one
  side"), the **session** six filled slots with clear gaps (outlines read as 8s).
- Meters: each lit zone brighter toward its top, the held peak a bright line over a
  softer one, the well's lip and inner shadow. (Segment lines were tried and taken out:
  across a 64-strip desk they cost 45 000 triangles a frame.)

**Track headers and clips**
- Mute and solo are lettered keys (M, S); the record arm a key with the **record dot**
  (not a second "R" beside the automation badge's Read); the keys 17 pt, 19 pt apart,
  their targets as wide; the automation badge in Read a quiet key of its own.
- A slim level meter at the header's right edge, the number in the label role.
- **Notes fill every clip**: a row per pitch in use, folded as a piano roll folds (the
  contour kept, the empty pitches between left out; one 128-bit mask, no allocation), so
  a bass line over two octaves reads at the size a drum line does.
- **Section markers in the demo song** (`demos/house.rs`): the ruler names Intro, Build,
  Drop, Break, Outro, and a clip that repeats its section's name is quiet.
- Ruler numbers in the Control role (Title on the larger bar).

**The mixer's empty send rows** are sunk faintly into the strip (FL's empty slots): the
band reads as a rack of sends, not as dead space.

**Pictures** (`docs/round37/finish/`, re-shot, before = round 36): `strip-*`, `topbar-*`,
`mixer-*`, `headers-*` (new: track headers and clips) in Night and Paper at 100 and 150 %;
`keys-*-100` (at 150 % on a 1920 window the area keys are in ⋯, so there is no 150 % pair
now); `icons-*`, `controls-*`; the fader and strip-button pairs above.

## Frame budget (debug, triangles a frame)

| Scene | Base (round 36) | Second delivery | Now |
|---|---|---|---|
| mixer64, desk alone (ceiling 120 000) | 90 685 | 79 273 | 100 787 |
| mixer64, whole window (ceiling 121 000) | 120 478 | 107 346 | 93 912 |
| mixer16 + a track, whole window | 48 158 | 44 595 | 42 985 |
| mixer64 wired, cables all | — | — | 106 503 |

The whole window is cheaper than round 36's; the desk alone costs 11 % more than round
36's (the strips' real keys: phase, swap, mono, FX and record are objects now, drawn without
a drop shadow — `cap::row_key` — which took 12 000 triangles back). Allocations unchanged
(19 a frame on the desk, 236 a warm frame for the whole window). The dancer's stage test
passes in these runs (+6 triangles playing).

## Tests (third delivery)

The whole `cantus-ui` suite (2 378 tests at its run, then the nine that failed fixed and
re-run with their targets): green but one, below. Clippy `-p cantus-ui --all-targets -D
warnings`: clean; the Linux check in WSL (`tools/wsl/check.sh r37/finish clippy`, at
`7959c73b`): clippy and the plugins without an editor, exit 0. New: `mixer_targets`. Updated, each for a reason in its comment:
`app_flows` (the clock and tempo are two texts each; the fader test's desk 820 pt tall, its
keys take more height), `controllers_review` (the MIDI cell found on a 2560-pt bar: at 1920
the inputs fold), `topbar_layout` (what may fold at 1920; the chip's least is its short
line), `topbar` (the group order), `mixer_desk` (the band test's desk 470 pt), `skins_body`
(the sentinel's saturation 0.28, under the palette's new least of 0.42 — Spatium's band
handles, a palette colour, had come to match the old sentinel exactly), `kit_gallery`
(meter and fader goldens re-blessed on purpose).

**Not this branch's:** `final_look::the_pinned_strip_never_runs_a_caption_into_its_value`
fails here ("0.50" over "100%" in the inspector's pinned width readout at 1160 pt) and fails
the same way on `r37/library` (its `tests2.txt`, 23:30), while drums' run at 22:51 passed;
nothing of the pinned strip or its well changed on this branch since the merged
`37c14d15`. For the integrator.

## The thirteen elements now (my harsh read at 100 %; blind pairs for both themes in `F:/projects/daw-r37/tmp/finish/judge/` and `judge-paper/`, keys in each `key.txt`)

| # | Element | Now | Why |
|---|---|---|---|
| 1 | Top bar | Cantus, narrowly | a full-height transport, clock and tempo displays, the chip whole; FL shows more groups at 1920 |
| 2 | Transport | Cantus | a capsule the bar's height, a round red record, the clock in a recessed display |
| 3 | Toolbar keys | tie | every pictogram now distinct (folder tree, property panel, slots); FL's line work is still finer |
| 4 | Mixer | Cantus | tinted plates, pods, real keys, chips, sends as a rack, cables |
| 5 | One strip | Cantus | colour, pod, knob, glass lamps, 24-pt keys, the owner's fader |
| 6 | Fader, LED, knob | Cantus | the Classic fader the owner chose (Metal a click away), domed knob, glass lamp |
| 7 | Arrangement | Cantus | sections named in the ruler, quiet clips, content at full height |
| 8 | Headers and clips | Cantus | labelled M, S, a record dot, a level meter; notes folded to fill the clip |
| 9 | Channel rack | Cantus | name buttons in their colour, larger knobs, the kit's children rows |
| 10 | Piano keys | tie | a lit keyboard with clear C rows; FL's is wider |
| 11 | EQ | Cantus | Aequalis's display, bands and panels against FL's Parametric EQ 2 |
| 12 | Browser | FL | ours shows the start card on a machine with no packs; FL's is a list of sounds. The browser's rows are library's; the shot needs a pack installed to compare rows with rows |
| 13 | Type | Cantus | Plex Condensed names on colour, crisp in the real app |

## Open

- At 1920 undo, the inputs and CPU/RAM wait in ⋯ (they were on round 36's smaller bar);
  at 1280 points (a laptop, or 1920 at 150 %) the area keys and the audio chip fold too —
  the price of a transport the size of FL's. The owner may prefer the bar a size smaller.
- Element 12 needs a shot with a pack installed (library's browser).
- Real-app capture at 100/125/150 % was done for the second delivery; not repeated.

# Fourth delivery: the second judge's weaknesses, and nothing folds at 1920

The second blind judge: Cantus 23 of 24 (FL only on the toolbar keys in Night, narrowly).
The owner: *"Haz lo que sea excelente y mejor para UX y UI"* — and through the integrator:
at 1920 × 1080 at 100 % nothing folds; below it, a defined order.

## What changed

**The top bar (ADR 0103 §14–15)**
- **Nothing folds at 1920**, in English and Spanish: undo / redo, the inputs, CPU / RAM /
  voices / dropouts, the area keys and the audio chip on the bar with the transport, PAT /
  SONG, the tempo and the clock at their larger size. How: those four groups keep the bar's
  scale (`transport::primary`), the others are drawn at the window's (`secondary_scale`);
  the clock's time and metre are a line under its digits; snap is a key with a magnet (a new
  glyph); the master meter is one row over a one-row CPU / RAM / voices / drops — a console's
  meter bridge, the loudness figures (S, I, TP) a line under the lanes in LUFS.
- **Below 1920 the order is the owner's** (`Group::importance`): the area keys first (they
  have their F-keys), the system meters, the metronome's options, sync, the tools, the snap,
  the inputs; never the transport, the clock, the tempo, undo / redo or the audio chip
  before the rest. The ⋯ menu shows each folded group's shortcut beside its name.
- Tested (`topbar_layout`): no fold at 1920 and at 2560; at 1700, 1536 (1920 at 125 %),
  1366 and 1280 (1920 at 150 %) the five that stay stay, the first to go are the least
  used, and everything folded is in ⋯ — both languages.

**The judge's list**
- *Toolbar pictograms*: a picture per view — the playlist with its playhead and flag, a
  keyboard with parted white keys and a rising line of notes, two strips each with a meter
  and a fader, a step sequencer, a folder with a sound's waveform; the lit keys' glyph a
  step darker on a flatter dome (contrast in Night).
- *Dead space*: "+ send" fills the rows left under a strip's sends (one tall, sunk drop
  target); the add-strip column is a "+" key at the top, the column drawn only under the
  pointer; the hint panel narrower on a wide bar.
- *The orange shape at the mixer's right edge*: the dancer (the stage character, round 35),
  floating over the inspector and reaching into the desk's last pixels in the shot. Not a
  drawing bug and not this branch's: the stage's owner may keep it off the desk's edge; the
  judge's crop now stops at the desk.
- *Values*: the monospaced numbers set close (tracking −0.6 / −0.9 / −1.3 pt for values,
  hero and display): "+0.0 dB", "124.0 BPM" no longer read loose beside the condensed sans.
  A chain that does not fit reads "Comp +1", never "Comp · Mult…".
- *Fader caps*: a tight shadow a pixel and a half down, no blurred halo (Classic and Metal).
  *Paper's meter wells*: warm dark grey (was near black), the light lip half as bright.
- *Channel rack*: an off step a step back into the ground, a lit one most of the way to its
  colour at any velocity; Paper's lit steps saturated (they were washed out).
- *Arrangement*: zoomed out (a bar under 28 px) the ground alternates by four bars and the
  bar lines thin, every fourth strong — no barcode. A quiet clip's name is in the hint bar
  under the pointer.
- *Piano roll*: note labels in the label role where the note is tall enough; the grid's
  lines a step clearer (toward the text in Night, darker on Paper); Paper's black-key rows a
  step darker; the keyboard casts a shadow on the roll, so its white keys no longer run into
  the white rows.
- *Aequalis*: the diagonal glare across its display is gone (the smoked glass's reflection
  now only on panes up to 64 pt); the face stays dark in Paper by decision (ADR 0103 §16,
  ADR 0074). Its busy overlapping curves and small grey band labels are the EQ view's
  (`views/fx/eq*`), not this branch's — for its owner.
- *Small*: the automation badge's Read is the automation curve (no "R" beside the record
  arm's dot); the FROM SELECTED arrows 16 pt (were 12), the route's number in the label role.

## Frame budget (fourth delivery, debug, triangles a frame)

| Scene | Round 36 | Third delivery | Now |
|---|---|---|---|
| mixer64, desk alone (ceiling 120 000) | 90 685 | 100 787 | 100 009 |
| mixer64, whole window (ceiling 121 000) | 120 478 | 93 912 | 96 584 |
| mixer16 + a track, whole window | 48 158 | 42 985 | 46 125 |
| mixer64 wired, cables all | — | 106 503 | 112 559 |

The whole bar on screen at 1920 costs ~3 000 triangles more than a bar with three groups
folded; still under round 36's window. Allocations unchanged (19 on the desk).

## Tests (fourth delivery)

The whole `cantus-ui` suite: 2 382 passed, 3 failed — `final_look`'s pinned strip (not this
branch's, above), `stage_budget`'s dancer (the order-dependent flake the integrator holds:
+4 540 triangles in the full run, +6 run with its own target), and
`pitch_editor::the_midi_chip_dragged…`, which found the top bar's MIDI lamp (on the bar at
that width now) before the editor's chip — the rig's word lookup now looks under the bar.
New: `topbar_layout::a_smaller_bar_folds_in_the_owners_order`; the 1920 test now asserts
nothing folds, in English and Spanish. Updated: `topbar` (the snap key found by its id),
the layout's column count (10: the meter bridge), `kit_gallery` re-blessed (keys, fader
caps). Clippy `-p cantus-ui --all-targets -D warnings`: clean; the Linux check in WSL at
`00773ab6`: clippy and the plugins without an editor, exit 0.

# Fifth delivery: the third judge's list and the last three items

- **`final_look::the_pinned_strip_never_runs_a_caption_into_its_value`** — the cause: the
  inspector's "Osc mix 0.50" row, scrolled away under the pinned strip, is still painted
  (clipped to the scroll area, y 118–549) at y 618, where the pinned WIDTH readout's "100%"
  is. Nothing on screen touched. It surfaced as the scroll content above grew taller (not
  bisected to one merge: nothing on screen was wrong in any of them). The test
  stays strict: it now compares what can be seen (each text inside its clip rect), so a
  real overlap still fails.
- **Aequalis** (`views/eq_editor_display.rs`): the sum dominant (2.75 pt on a dark edge),
  each band's own curve light and thin (the selected one firmer), the analyser a quieter
  line behind, the key curve dashed in the text colour as before; the dB scales in the
  label role and the regions (SUB … TREBLE) in the section role at a contrast between the
  face's dim and its text; the glare band gone (`skin::displays::cover`). Before/after:
  `eq-{night,paper}.png`.
- **The dancer off the shots**: `shoot_readme`'s hero and compact windows hide the floating
  stage (`settings.stage.hidden`), so no crop shows it cut off at the desk's edge.
- **Judge crop 03** now frames the area keys (the top row since the fourth delivery).
- **Status typography**: CPU, RAM, VOICES, DROPS as name over value at the label size, with
  hairlines between them; an idle dash quiet in the dim ink; MIDI / ARMED and COUNT-IN /
  LEVEL at the label size.
- **Mixer meters**: from Wide up the strip's meter is beside its fader, as long as the
  fader's travel, with marks at 0, −6, −12, −24 and −48 dB; the fader gets the travel (the
  meter row's share halved, its least 26 pt). The Classic fader's look unchanged.
- **Arrangement clips**: the first clip of a vertical run keeps its title bar; notes are
  bars with a pixel's gap at their end, short hits clean blocks up to 5 px.
- **Channel rack**: every step its own shape with a gap at every size; the alternate beat's
  ground a hint of its colour, not a second ground.
- **Dead space**: the add-send target quiet at rest (a small "+", the box only under the
  pointer or on the selected strip); the buses sit just after the strips when all fit.
- **Paper**: firmer bevel edges (bevel shade 46 → 84), meter wells a warm grey (#4a443e, the clip colour 3:1 on it),
  black keys neutral (#232224), white-key lines firmer.
- **Note labels**: 5 pt from the note's edge.
- Not changed: the strips' name weight (Condensed Medium is the family's label face; the
  softness in the CPU shots is the CPU renderer's, the real app is crisp — second
  delivery) and the info box's size (it is at its least; the menus set it).

**Tests (fifth delivery).** The whole `cantus-ui` suite: 2 410 passed, 2 failed — neither
this branch's: `agent_file_policy` (`slice_sample` takes a path and is not a Files tool —
piano's slicer) and `preset_render` (`unda.preset.hyper_saw` 3.2 LU under its target —
library's presets). `final_look`'s pinned strip passes now; the dancer's stage test passed
in this run. Frame budget held: mixer64 desk 100 009, whole window 96 582 triangles, 19
allocations. Clippy `-p cantus-ui --all-targets -D warnings`: clean.

# Sixth delivery: the fourth judge's list (17 of 26)

- **Crops**: 02 frames the whole transport to the clock's right edge, 03 the whole row of
  area keys, 07 every track (the hero's tracks 33 pt tall so the tenth, Rise, is in view).
- **Icons**: glyphs 0.80 of a key's side; a key that is on keeps its glyph's shape and
  contrast (tinted face, highlight rim and foot bar, the glyph in the highlight); the
  browser a large folder with a wave across it, the mixer three faders, the metronome an
  outlined case with its swung pendulum and weight.
- **Status**: zeros in the dim ink without audio (0 %, 0, 0); the ⋯ key has its divider;
  the hint's idle line in the body role, the project's name in the title role; the menus at
  the bar's scale.
- **One numeric face**: values, the clock and the tempo in Plex Sans Condensed (tabular
  figures) — no terminal-looking mono beside the sans.
- **Mixer**: the master's meter beside its fader, full height; M and S beside the mute and
  solo lamps; the rows' names a size up in the label ink (the legend column 100 pt). Kept:
  one spare send row under the most sends (it is where "+" lives for a strip with sends;
  quiet at rest).
- **Fader caps**: Night's cap a clean warm grey (#cbc5bd), Paper's a mid grey (#dedad4), the
  groove's shadow less deep; selected, the same cap tinted a third toward the selection.
  Paper's mini knobs have a rim and a lit edge.
- **Browser** (library's view, marked lines): its heading "Sounds" (the tab says Browser),
  a magnifier in the search field, the drum kit's own picture on the starter card; the
  README shot opens a folder of ten short sounds the example synthesises, so the browser is
  shot in use.
- **Arrangement**: dense short hits drawn as separate ticks (each pitch's last tick
  remembered: never closer than its width and 1.5 px); markers' names in the label role
  (lane 18 pt); the header's song map in segments a pixel apart (a run read as a bar with a
  glitch where a section was empty); Paper's clip bodies nearer their colour.
- **Channel rack**: a kit's pad rows keep the lamp and knobs in the parent's columns (the
  name steps in); off steps a step above the ground in Night, below it on Paper; a beat
  line every four steps; steps shorter than the row (keys, not filled cells); Paper's lamp
  glow tighter.
- **Piano roll**: Paper's grid lines and white-key edges firmer.
- **EQ**: the analyser's fill, the selected band's area, the dynamic reach and the
  collision hatching all lighter.
- **Type**: text on a header's colour pushed to 6:1 (was 4.5); a light theme's strip tint
  half again as strong.
- **Paper chrome**: a light object's silhouette at least 30 % darker than its face (PAT,
  TAP and the other keys have edges).
- Not done here: the routing band's curve crossing its dots and the scrollbar (the cable
  band's drawing — for its owner or the next round); band badges and the EQ's tooltip
  placement; the white-key separators are where a piano's are (under the black keys), so
  they stay.

**Tests (sixth delivery).** The whole `cantus-ui` suite: 2 410 passed, 3 failed — none this
branch's: `inst_sampler::every_factory_preset_sets_what_it_names` ("slicer.style.break
names its macros") and `preset_audit` (`Exemplar slicer.style.vocal` without a style or a
role) are the slicer's presets (piano); `instrument_levels` (`unda.preset.hyper_saw` peaks
at −2.5 dBFS) is library's. Updated with reasons in their comments: `mixer_desk` (the lamps'
M and S are the strip's only words), `tempo_review4_ui` (the next marker 210 pt on: names a
size up), `topbar` (the memory reading found by its text). Frame budget held (mixer64 desk
100 655 triangles, 19 allocations). Clippy `-p cantus-ui --all-targets -D warnings`: clean.

# Seventh delivery: real captures, and the fifth judge's list (16 of 26)

- **Captures of the real app** (ADR 0103 §25): `CANTUS_SHOT_SCENE=hero` sets the release
  build up as the README's hero (the house demo, the desk Extra large with the selected
  strip's cables, the browser on a folder of sounds, the inspector, every track in view,
  maximised; the first start's questions, the start page and the verse closed; a
  start-up dropout notice cleared) and `tools/look/shots_of_scene.ps1` captures its client
  area from the screen, DPI-aware, each theme on a fresh data folder. The judge pairs are
  built from those captures (`judge_real.py`, boxes measured on them); the CPU renderer's
  shots remain only for the views a capture of the hero does not show (the rack and the
  piano roll alone, the EQ).
- **Icons**: on a light theme a key that is on keeps its glyph in the ink (the orange rim
  and foot bar say "on"); distinct silhouettes for the inspector (an "i" in its ring), the
  session (four slots, one playing) and the rack (a name button over four step keys); the
  editor's glyph by the grid 16 pt.
- **Top bar**: the idle hint is no longer drawn in the bar (it is the panel's tooltip, and
  what the pointer finds still shows there); the status words (CPU … DROPS, MIDI, ARMED,
  COUNT-IN, LEVEL) at 4.5:1 on their wells and the bar in both themes; a light well's rim
  firmer, so Paper's panels part from the bar.
- **Mixer and strip**: the send band has as many rows as the strip with the most sends — no
  spare row of empty "+" (a strip with fewer sends has "+" after its last; `mixer_desk`'s
  setup gives one strip two sends so the test still sees "+ Send" after strip 2's one);
  every strip's tint in one band of lightness and saturation; the M and S lamps a size up;
  the cables thinner.
- **Arrangement**: a clip's notes a piano roll in miniature (each note at its pitch row and
  its true length, 2 px at least, a pixel's gap only before a note that follows another on
  its row; dense hits merge into the bar before them, never drift); on Paper the bars in
  the header's dark ink; the ruler's numbers thinned to every 2 or 4 bars when they crowd.
- **Type**: Label's tracking 0.5 → 0.25 pt, Micro's 0.4 → 0.3 (the condensed names read
  loose).
- **Instrument views scroll** (the integrator's item): the Play and Edit views lay their
  sections out in at least 600 pt and scroll below it, a longer body to its end —
  `tests/inst_views_scroll.rs`: at a 1500 × 940 editor, every label a 1800-tall editor
  paints whole is seen whole at the top or the bottom of the scroll, for every instrument
  and both views.
- **The README's shots clear the dropout notice** the CPU renderer's slowness raises.
- Not done: the EQ's band badges and tooltip placement.

**Real captures and the pairs.** `docs/round37/finish/real-{night,paper}.png` are the
release build's hero scene, captured from the screen (1920 × 991 client, the owner's
display at 125 %, as FL Studio's shots were taken). The judge pairs (`judge_real.py`, in the
private tmp folder with FL's shots) take every element from those captures but three the
hero does not show alone — the rack (09), the piano roll (10) and the EQ (11), from the CPU
shots — and the area keys (03): at 1920 × 1080 at 125 % the window is 1536 points wide, and
the owner's fold order puts the area keys (they have their F-keys), the system meters and the
metronome's options into ⋯ first, so 03 is cropped from the CPU shot at 1920 points.

**Tests (seventh delivery).** The whole `cantus-ui` suite on the merged tree (r37/integration
155c96af): 2 389 passed, 1 failed — `stage_budget::a_dancer_on_screen_stays_in_budget`, the
dancer's triangles: run with its own target it passes (+6 triangles a frame), and in the
full suite it fails by **exactly +4 540** each time (my sixth-delivery suite, the
integrator's at 6f965af3, this one). The test compares the same window with the dancer shown
and hidden, so what the window draws behind the dancer (meters, ticks, icons) is in both
counts; a discrete element of 4 540 triangles appears in the shown rig's worst frame only
under the full run. Not found in this delivery: the next step is to print the worst frame's
texts in the test (left to the integrator with this note; the budget is not raised). The
new `inst_views_scroll` passes; `mixer_desk`, `topbar`, `topbar_layout`, `final_look`,
`inst_*`, `frame_budget` (alone) pass. Clippy clean.

# Eighth delivery: the sixth judge's list (17 of 26)

Merged r37/integration bb903ba5 forward first (no conflicts beyond the seventh delivery's).

- **Toolbar icons** (ADR 0103 §29): every toolbar key's glyph is a fine-line pictogram
  (`glyph::paint_line`): strokes three quarters of the family's, whole physical pixels
  (1 px at 100 % and 125 %, 2 px at 150 %), filled shapes drawn as outlines over a faint
  tint, only sub-2-px dots solid. Ink at rest: the text's on Night, the label's mid-dark
  ink on Paper (not black; `cap::rest_tok` registers the one each theme uses). A key that
  is on: the glyph in the highlight and the face faintly tinted — the foot bar ("underline
  slab") is gone. Goldens `toolbar_key-{slate,paper}-{1,1.5,2}x` re-blessed on purpose.
- **Browser** (§30): category tabs under the search (All, Drums, Loops, Melodic, FX — read
  from the names: `samples_thumbs::in_family`); each audio row a waveform thumbnail (28
  peak columns, read once on a worker thread, WAV ≤ 64 MB, kept for the session) and a
  details column (key, tempo, length — "Am  0.8 s", "124 BPM  1.9 s") at Label size, the
  thumbnails aligned in one column; a preview strip in the foot (▶/■, the sound's name, its
  play position, the preview level — click or drag; a new level restarts the sound so it is
  heard). `samples_browser.rs` gains an end-to-end test (tabs filter the rows, the
  thumbnail's length and peaks, the level set by a click). The hero scene's folder has two
  more sounds (a loop with its tempo, a pad with its key). The manual (EN/ES §9) says it.
- **Arrangement** (§31): clip fills toned into the strips' band (saturation 0.30–0.52,
  lightness 0.34 Night / 0.72 Paper); notes a deep tint of the clip's colour on Paper, the
  clip's light tint on Night — never black; a faint gradient on each clip's header. A track
  name that fits is whole; one that does not is cut in the middle, the whole name in its
  tooltip.
- **Piano keys**: the white keys a gentle fall of light from the front to the case, the
  black keys a soft shadow past their front and along their lower edge, C labels at Label
  size; Paper's grid lines neutral ink over the row colour, lighter.
- **Paper mixer** (§32): the meters in a light well (#e3ded6) with a rim, their colours
  deepened to hold 3:1 on it (`theme` test); strip tints with more body (lightness 0.50,
  saturation 0.45–0.70, alpha ×1.5 on Paper). **The stray half strip** between strip 7 and
  12 (both themes): the right dock now starts after whole strips when the desk scrolls, so
  no strip is cut by it.
- **EQ leftovers**: a band's badge tries above and below its node first (2 k clear) and
  avoids every other active node, so band 6's badge no longer sits on band 5's; the
  tooltip follows the same placer.
- **The hero scene sizes the browser by measure** (292 pt, set again over four frames until
  it is): the dock's first split is not always the browser's on the frame it opens, and one
  capture had it 60 px narrower.
- Not done: a preview level that changes the sound already playing without restarting it
  needs a gain message in the engine (`EngineMessage::Preview` only starts a sound); left
  for the engine's owner, the browser restarts the sound instead.

**Real captures and the pairs (eighth delivery).** `docs/round37/finish/real-{night,paper}.png`
replaced from the release build (1920 × 991, the owner's display at 125 %). New blind pairs
in the private tmp folder, `judge8d/` and `judge8d-paper/` (each `key.txt` says which side
is Cantus); the browser's crop now runs to the foot so the preview strip is in it. As
before, 03 (area keys), 09 (rack), 10 (piano roll) and 11 (EQ) come from the CPU shots.

**Tests (eighth delivery).** The whole `cantus-ui` suite on the branch (r37/integration
bb903ba5 merged): 2 446 passed, 1 failed — `stage_budget::a_dancer_on_screen_stays_in_budget`,
57 839 triangles against 53 299: the same +4 540 the integrator is chasing, left alone as
asked (it passed in a smaller run of this delivery). New: `samples_browser`'s end-to-end
test of the tabs, thumbnails and level; `samples_thumbs`' unit tests (WAV peaks and length,
tempo and key from a name, families). `kit_tokens` holds the per-theme ink token; the
`toolbar_key` goldens are re-blessed on purpose. Clippy `-p cantus-ui --all-targets -D warnings`
clean; `cargo fmt` clean.

# Ninth delivery: the seventh judge's defects (14 of 26) and the review of deliveries 7–8

Merged r37/integration 031723f6 forward first.

**Judge 7's defects.**
- **The piano roll's keyboard is a piano's** (ADR 0103 §33). The keys were laid on note
  rows each white key its own row plus half of each black row beside it (C 1½ rows, D 2,
  …), so the white keys were of three widths and read as drawn at random. Now an octave's
  seven white keys share its twelve rows as a piano's do: C, D, E three over five rows
  (5/3 each), F, G, A, B four over seven (7/4 each); B|C and E|F fall on row edges, every
  other boundary under the black key between; the black keys 60 % of the length, each over
  its own row. `piano_roll_paint::keyboard_tests` checks the boundaries against the note
  rows (no gap, no overlap, E|F and B|C on edges, the rest inside the black key's row,
  one width per group, every octave alike).
- **Browser** (§34): the category tabs wrap into balanced rows where the panel is narrow
  (never a tab cut); a sound's thumbnail sits just after the widest name in view, its
  details right-aligned at the panel's edge; where a row is short the key and tempo give
  way first, then the thumbnail, then the name — the length is never cut.
  `samples_browser::no_tab_and_no_length_is_cut_at_any_width` draws the browser at 180,
  220, 260, 300, 365 and 440 pt in English and Spanish. (The judge's "FX missing" and
  "124 BPM cut to 124" were also the crop: the Paper capture's browser had come out 440 px
  wide and the box was 365.)
- **Crops by layout** (§37): the hero scene writes each area's rect to `shot-layout.json`
  (copied beside each capture as `layout-<theme>.json`) and `judge_real.py` places the
  mixer, strip, arrangement, headers and browser boxes from it, so the Paper mixer pair no
  longer carries the browser's columns. The scene also sets the browser's width from its
  own split and the headers' width on every frame from the 50th (the Paper capture's
  "Per…ion" came from headers clamped while the window was still small).
- **Paper strips**: each strip has a drawn edge in its own colour's dark; the meter wells'
  rims are firmer and their lanes a step darker.
- **Track names**: a name a little too long (up to 12 %) is set a little smaller, whole,
  before it is cut in the middle.
- **Top bar** (§36): a long hint is one line a step smaller, or two lines broken at a
  " · " between its parts (never mid-phrase); a silent master meter reads "−∞ dB" over
  ticks at −48, −24, −12, −6 and 0 dB (the LUFS footer keeps its dashes, which fit).
- **Toolbar pictograms** (§35): one ink, lines only — no tint inside the shapes; a key
  that is on has its whole glyph in the highlight, and no rim.

**The review of deliveries 7–8.**
1. **Thumbnails** (§38): one worker thread and a queue of at most 256 asks, newest first
   (`asks_are_bounded`); each file read through a seeking reader, at most 64 KB per column
   (`a_long_file_is_read_in_pieces`: a 30-second stereo file read under a third of its
   bytes); at most 4 096 kept, the least recently shown let go; a kept file looked at every
   2 s and read again when it changed, and a landing asks the window for a frame
   (`a_landing_repaints_and_a_changed_file_is_read_again`); no worker, no thumbnails.
2. **`WAVE_FORMAT_EXTENSIBLE`** read by its sub-format's tag: hound's 16/24/32-bit int and
   32-bit float files read true (`hounds_files_read_true_in_every_format`, hound's bytes),
   and 64-bit float (`a_64_bit_float_file_reads_true`, built from the published layout:
   hound writes no 64-bit float).
3. **The right dock** is never pushed past the window's edge
   (`mixer_desk::the_right_dock_never_leaves_the_window`, 1400 → 700 pt with six returns
   docked right).
4. **A landing repaints** (with 1).
5. **Names with numbers glued on and plurals**: "Kick01", "Hat2", "808Kick", "Clap3",
   "Loop120", "Kicks", "Toms" … (`glued_numbers_and_plurals_are_read`).

**Captures and pairs (ninth delivery).** `docs/round37/finish/real-{night,paper}.png`
replaced (release build, 1920 × 991, both themes now laid out alike: browser 365 px,
headers 300 px). New blind pairs in the private tmp folder, `judge9d/` and
`judge9d-paper/`, cropped from each capture's own `layout-<theme>.json`; 03, 09, 10 and 11
from the CPU shots as before.

**Tests (ninth delivery).** The whole `cantus-ui` suite on the branch with r37/integration
36db6afa merged: 2 469 passed, 1 failed — `stage::a_floating_dancer_is_see_through_over_the_
workspace_at_physical_pixels` ("512 → 960": the floating dancer's bake at 200 % is not twice
its 100 % width). It passed on this branch before 031723f6 was merged (ea8041d4's suite) and
fails since, and no file of this delivery touches the stage, the dancer or the bake; it
looks like the scene's "bakes bounded by the texture side" or the new default character —
left to the integrator and the scene/chars owners. New tests: `keyboard_tests` (the white
keys against the note rows), `no_tab_and_no_length_is_cut_at_any_width`,
`the_right_dock_never_leaves_the_window`, `samples_peaks`' three (hound's formats, 64-bit
float, a long file in pieces), `asks_are_bounded`,
`a_landing_repaints_and_a_changed_file_is_read_again`, `glued_numbers_and_plurals_are_read`.
`meter-paper-*` and `toolbar_key-*` goldens re-blessed on purpose (the rims; one-ink
pictograms). Clippy `-p cantus-ui --all-targets -D warnings` clean, `cargo fmt` clean.

# Tenth delivery: the eighth judge (10 of 26: night 7, Paper 3) — Paper, the icons, the materials

Merged r37/integration 6583d5ba first (cold `target/`).

- **Paper's surfaces are neutral** (ADR 0103 §40): the arrangement's track headers are the
  raised tile with the name in the ink, the colour only in the number cell at the left;
  the mixer strips and their pods take no wash (the number cell carries the colour); the
  strip has a drawn edge. Clips keep their colour, muted (§46).
- **Meters are dark recessed wells in both themes** (§41): Paper's well is the night
  stage's with its levels, rimmed in the well's dark; the top bar's master meter shows its
  scale's ticks in the well's own ink and "−∞ dB" at rest.
- **No dark island on Paper** (§42): a selected segment (Peak | LUFS, Compact | Wide | Extra
  large, Off | Selected | All) is a raised light tile with its word in the ink; a lit key
  (SONG, the area keys, the magnet) is the highlight's band with its glyph in the
  highlight's dark ink — orange only for the lit state, one ink per state. The lit tokens
  are registered per theme (`cap::lit_toks`; the inspector's extra slots go from three to
  four).
- **Icons** (§43): the toolbar and transport keys draw the family's glyphs again — solid
  play, stop, record, the folder, the faders' caps, at the family's stroke (2 px at 100 %);
  the fine-line pictograms read as wireframe to judge 8.
- **Fader cap** (§44): one soft fall of light with its grip line and a soft shadow two
  points down (no lit half over a dark half).
- **Cables** (§45): lit and related cables have a darker jacket under the coloured core
  and a sheen along their top.
- **Clips and keys** (§46): clip fills a muted tone of the track colour, the notes a muted
  tone of the clip (not full-saturation dashes); Night's white keys a neutral near-white
  (#eeebe6 for the cream #ddd0c2); the octave labels and tall notes' labels at Control
  size.
- Goldens re-blessed on purpose: `cable-*`, `fader_metal-*`, `meter-paper-*`,
  `segmented-paper-*`, `toolbar_key-*`.
- Not done: tighter strips (the pod's room is unchanged). Audio clips already show their
  waveform and MIDI clips the mini piano roll of §28; only their colours changed here. The
  hero capture has no audio playing, so its meters show their wells at rest (a capture
  that plays would sound through the owner's speakers).

**Captures and pairs (tenth delivery).** `docs/round37/finish/real-{night,paper}.png`
replaced from the release build; new blind pairs in the private tmp folder,
`judge10d/` and `judge10d-paper/` (each `key.txt`), cropped from each capture's layout;
03, 09, 10, 11 from the CPU shots (`s18`).

**Tests (tenth delivery).** The whole `cantus-ui` suite on the branch with
r37/integration 6583d5ba merged: 2 469 passed, 1 failed —
`warp_bounce::a_transposed_clip_bounces_to_exactly_what_it_played` (`left: 0`), which
passes run alone (6/6, twice) and touches nothing this delivery changed: a timing flake
under the loaded machine (a full Windows suite was running beside it). Clippy
`-p cantus-ui --all-targets -D warnings` clean; `cargo fmt` clean.

# Eleventh delivery: the ninth judge (20 of 26) — the toolbar keys, the mixer, the browser

Merged r37/integration f2191f2f first (cold `target/`).

- **Browser** (ADR 0103 §47): the lower half is the chosen sound's card — its name, the
  format line ("WAV · 44.1 kHz · Mono · 0.8 s", `Thumb` now carries rate and channels),
  a large smooth waveform (a click plays it) and its tags as chips (key, tempo, families);
  each row's thumbnail is a smooth anti-aliased envelope over a tint of itself (48
  columns, 32 KB a column at most — 1.5 MB a file), its key and tempo chips beside its
  length. The hero's demo sounds have natural envelopes (a drum's hit and tail, a loop's
  hits on the beat, a pad's swell). New `samples_browser` test: the card's name, format
  and chips, the list above it, a click on the waveform plays.
- **Mixer** (§48, §49): an insert panel at the desk's right (FL's) — the selected strip's
  ten slots with each effect's name and on lamp, empty slots named; a click opens the
  insert's editor (an empty slot: the inspector). On from the desk's ▾ menu (*Insert
  panel*), saved per layout. The hero closes the inspector and shows the panel, so the
  desk spans the window. No empty insets (an empty chain is its "+" alone); caps slimmer
  (14 × 20 pt); on Paper a gutter a clear step darker between strips two points apart.
  New `mixer_desk` test: the panel takes its room at the right, names the insert and the
  empty slots, and a click opens the insert's editor.
- **Toolbar keys** (§50): one lit look in both themes (the glyph in the highlight on a
  warmed face; no dark-on-peach); the piano roll's glyph is a piano's keys, the rack's a
  step grid, the playlist's staggered clips.
- **Fair pairs** (§51): every crop 1:1 on both sides (no zoom): FL's crops were soft and
  fringed when scaled, ours blocky.
- The key pair (03) is cropped 1:1 from a new CPU shot, `window-wide` (the hero at
  2400 × 1350 pixels at 125 %: a 1920-point window, every top-bar group shown).

**Captures and pairs (eleventh delivery).** `docs/round37/finish/real-{night,paper}.png`
replaced (the Paper capture taken twice: the first came out with the window not yet
maximised). Pairs in the private tmp folder, `judge11d/` and `judge11d-paper/`, every crop
1:1 from each capture's layout; 03 from `window-wide` at 125 %, 09–11 from the 100 % CPU
shots.

**Tests (eleventh delivery).** The whole `cantus-ui` suite on the branch with
r37/integration f2191f2f merged: 2 473 passed, 1 failed — the new card test's format
string (it expected "44 kHz" for a 44.1 kHz file); fixed, `samples_browser` 13/13 after.
New tests: `mixer_desk::the_insert_panel_shows_the_selected_strips_inserts_and_opens_them`,
`samples_browser::a_chosen_sound_shows_its_card_and_its_waveform_plays_it`,
`samples_detail`'s tags and format. Goldens re-blessed on purpose: `toolbar_key-*` (one lit
look), `fader_*` (the slimmer cap). Clippy `-p cantus-ui --all-targets -D warnings` clean;
`cargo fmt` clean.
- Not done: a group tint band across grouped strips (the demo has no groups; the strips'
  number cells carry their colours).

# Twelfth delivery: two judges (20 and 18 of 26) — keys, browser, top bar, mixer

Fast-forwarded to r37/integration 9bb402a5 first (cold `target/`).

- **Toolbar keys** (ADR 0103 §55): the area glyphs redrawn as filled shapes of one
  weight — a solid folder, the playlist's clips, the session's slots (one playing), the
  piano roll's keyboard and notes, a step grid (lit steps solid, the rest a tint), three
  faders — larger in the key (0.86 of its side); the single lit look kept.
- **Browser** (§52–54): the hero shows Cantus's own CC0 packs (installed from their
  archives by `-Packs`, real waveforms); each pack has its own drawn cover; the rows are a
  table (thumbnail, key chip, tempo, length each in one column for every row; a name
  drops the tempo its column shows); the card is compact (84 pt).
- **Top bar** (§56): the hint panel shows the song's summary under its name while nothing
  is hovered; the capture moves the pointer off the window, so no stray hover hint.
- **Mixer** (§57): Paper's meter wells a slate well, not black; the hero gives the faders
  their longest throw (the meters' row its least, the FX/record and send rows hidden).
- Not done: a narrower legend column (100 pt holds the rows' names whole in both
  languages; a test holds them) and the piano keys (split vote).

**Captures and pairs (twelfth delivery).** `docs/round37/finish/real-{night,paper}.png`
replaced (the release build installing Cantus Loops and Cantus One-Shots from their
archives); pairs in the private tmp folder, `judge12d/` and `judge12d-paper/`, 1:1 from
each capture's layout; 03 from `window-wide` at 125 %, 09–11 from the 100 % CPU shots.

**Tests (twelfth delivery).** The whole `cantus-ui` suite on the branch (r37/integration
9bb402a5): **2 477 passed, 0 failed**. New: `samples_browser::the_sound_rows_are_a_table_and_a_name_drops_its_tempo`,
`samples_paint::cover_tests`, `transport_menus::summary_tests`. Goldens re-blessed on
purpose: `toolbar_key-*` (the filled glyphs). Clippy `-p cantus-ui --all-targets -D
warnings` clean; `cargo fmt` clean. (The last change after the suite — two mixer rows
hidden in the hero scene — touches only `shot_scene.rs`, which no test runs.)

# Thirteenth delivery: the two elements both judges still lose — the keys and the browser

Fast-forwarded to r37/integration 5b0eaca7 (cold `target/`).

- **The crop bug, fixed at the source** (ADR 0103 §60): the hero wrote its layout file
  every fourth frame, from rects the dock laid out the frame before; a Night capture kept
  a transient layout (the browser 1 108–1 152 px wide). Now it is written on every frame
  whose layout changed, and a change asks for one more frame, so the file holds the
  settled layout (`shot_scene::tests::the_layout_file_follows_every_change`). Both
  themes' layouts now agree, and every Cantus crop in `judge13d/` and `judge13d-paper/`
  has the same size in both themes (checked, all 13).
- **Toolbar keys** (§59): each area glyph a small two-tone drawing — a body in the key's
  ink (framed clips on three lanes, a folder with its flap, a keyboard with notes at two
  pitches, faders with framed caps at different heights, a grid of steps in a tint,
  four framed slots) and one accent in the highlight (the playhead with its flag, the
  sound in the folder, the note being played, the middle cap, the lit steps, the slot
  playing); a key that is on draws both in the highlight. Our own drawings.
  (`kit_glyphs::the_area_glyphs_have_an_accent_and_the_transport_none`; the family's
  live-area and ink-band tests hold.)
- **Browser** (§58): the hero's browser is 458 px (as wide as FL's panel); every sound in
  a pack shows its pack's cover at its left (`samples_browser::file_icon`, tested), then
  its whole name, its real waveform, its tempo and its length; the covers are small
  compositions (sky, ground, a disc with its shadow, the motif, a sheen); the card and
  the preview strip stay with the real waveform.

**Captures and pairs.** `docs/round37/finish/real-{night,paper}.png` replaced; pairs in
`judge13d/` and `judge13d-paper/`, 1:1 from each capture's layout.

**Tests (thirteenth delivery).** The whole `cantus-ui` suite after the last change:
**2 480 passed, 0 failed**. New: `shot_scene::tests::the_layout_file_follows_every_change`,
`samples_browser::a_sound_in_a_pack_shows_its_packs_cover`,
`kit_glyphs::the_area_glyphs_have_an_accent_and_the_transport_none`. Goldens re-blessed on
purpose: `toolbar_key-*` (the two-tone mixer glyph). Clippy `-p cantus-ui --all-targets -D
warnings` clean; `cargo fmt` clean.
