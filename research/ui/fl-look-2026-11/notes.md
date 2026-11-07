# FL Studio 2024 — how it looks, and what Cantus takes from it (2026-11-04)

*The owner, round 37: "FL Studio developed its graphics from zero; the idea is to do ours
too — only be inspired, do not copy; note the details and the quality to improve ours. FL
Studio is crazy good; we have to beat it."* Looked at FL Studio 24.1.2 (Producer edition, the
copy on the owner's machine) with a stock demo song open, at 1920×1020 physical pixels.
Screenshots in `screenshots/` (private: the public rewrite drops `research/**` images).
Compared with Cantus's `docs/images/window-night.png` and `mixer.png` (round 36).

**Rule for everything below:** these are observations of *qualities* (density, legibility,
finish, consistency). Nothing is to be traced, recoloured or laid out pixel for pixel; every
Cantus answer is drawn by our kit (ADR 0024) in our own language and must be better, not
the same.

## What FL does well, view by view

### The whole window
- **One finish everywhere.** Every control — toolbar icons, knobs, LEDs, window chrome
  (its own minimise/maximise/close glyphs), scroll bars — is drawn in one vector language:
  the same stroke weight, the same corner radius, the same two-tone icons. Nothing looks
  borrowed from the OS.
- **Depth without noise.** Buttons and panels carry a very slight vertical gradient and a
  one-pixel lighter top edge; pressed and active states invert it. The background is a calm,
  desaturated blue-grey; saturated colour is spent only on state (orange = active tool,
  green = on, red = record) and on the user's own colours (channels, clips).
- **A hint line** under the menus names whatever the mouse is over ("Effect slot 3",
  "Swap left & right channels", "Zoom level 1") — instant, one line, always in the same
  place. Cantus has a status hint; FL's is closer to the pointer's work and never empty.
- **The big clock**: the song position in a large thin light face (≈ 40 px), the
  bars:beats:ticks unit tiny above it, readable from across a room.
- **System meters in the bar**: CPU and memory as tiny live graphs *with* their numbers
  (polyphony 4, 452 MB) — a glance, not a dialog.

### The mixer
- **Vertical names** in each strip: narrow strips (≈ 56 px) still show "Build Snare",
  "Closed hats", "CHORD BUS" whole. Cantus's strips are wider and still truncate the output
  ("→ 10 Drum…").
- **The whole strip tinted** by the user's colour (drums blue, keys orange, the master
  olive), header band saturated with an icon of the source. Groups read at a glance from
  across the screen; Cantus tints only the header band.
- **The meter inside the fader's track**: a thin level bar runs in the fader slot itself,
  so the strip spends no height on a separate meter block.
- **The selected strip's panel on the right**: ten slots (name, a mix knob, an on LED), a
  small three-band EQ, input and output — the same idea as Cantus's inspector.
- **Routing at the bottom**: a send arrow per strip toward the selected one, lit when
  routed — one click to route.

### The playlist (arrangement)
- **Track headers as coloured tiles**: the track's colour fills the header, its name top
  left, a small icon, the on LED bottom right. Tall rows by default.
- **Clips carry their content in their own hue**: a darker body with the notes, waveform or
  automation curve drawn in a lighter shade of the same hue, a name strip on top. An
  automation clip shows its curve filled — you see what it does without opening it.
- **The pattern picker** beside the playlist shows **a tiny preview of each pattern's
  notes** under its name — you find "Build Chords" by its shape.
- Section labels ("Buildup", "Drop") in the ruler.

### The channel rack
- Uniform rows: on LED, two mini knobs (pan, volume), a **numbered box for the mixer track**,
  a wide name button in the channel's colour, a select LED, the steps. Compact, regular,
  every row the same height; the eye scans down a column.

### The piano roll
- A **drawn keyboard** with shading on white and black keys and octave names on the C keys;
  alternating bar shading in the grid; velocity as **lollipop stems**. The tool icons above
  are the same family as the main toolbar.

### A plug-in (Fruity Parametric EQ 2)
- **Each band has one colour** used for its node on the curve, its fader, its frequency
  knob and its bandwidth knob — you follow a band across the whole window by colour.
- **The axis teaches**: note names along the top (C1 … C9) and **region names** (SUB, BASS,
  LOW MID, MID, HIGH MID, PRS, TREBLE) over the graph, frequencies along the bottom.
- The graph fills the window edge to edge; the controls are a thin strip below.

### The browser
- Tabs (All, Project, Plugins, Library, Sounds, Starred); each sound row has artwork, its
  length, a loop/one-shot icon, its name, **a mini waveform**, its BPM, *add* and a menu;
  **tag chips** (Trap, Keys, Piano, Wet, Melody, Reverb); a **preview player** at the
  bottom with a large waveform and *Sync BPM* (preview at the project's tempo).

## Where Cantus is already ahead (keep it)
- **Label/value units**: the mixer's row labels (NUMBER, NAME, LEVEL, VALUE, OUTPUT, SENDS)
  and every value in numbers (−3.5 dB) — FL shows almost no numbers on its strips.
- **Routing you can see**: the cable view under the strips (ADR 0041) — FL has arrows only.
- **Wide stereo meters with peak lines**, the inspector's full EQ with a numeric band table.
- Themes (Night, Paper and the family) — FL ships one look with hue shifts.

## What Cantus should do — proposals for the backlog (`BACKLOG §8`, round 37)
1. **Strip tint** (mixer, rack, arrangement headers): the whole strip/row tinted by its
   colour at a low mix (a theme token per surface, both themes), the header band saturated.
2. **Vertical names** on narrow mixer strips (Compact and Wide): the name turned 90°, whole;
   horizontal only when it fits.
3. **Meter in the fader track** as an option of Compact: the level bar inside the slot.
4. **One colour per band and region names on the frequency axis** — *Aequalis already has
   both (the side-by-side comparison showed it); extend them to every other spectrum display.*
5. **Clip content in its own hue** everywhere (notes, waveforms, automation curves filled),
   and **pattern/clip previews** in the clip browser's rows.
6. **A drawn keyboard** in the piano roll (shaded keys, octave names), lollipop velocity.
7. **The hint line** names everything under the pointer, never empty; **the big clock**
   in a light face at a size readable from a distance.
8. **System meters with numbers** in the top bar (CPU %, memory, voices).
9. **The browser's rows** (library builder, round 37): mini waveform, BPM, one-shot/loop
   icon, tag chips, a preview player that follows the project's tempo.
10. **One finish**: a review of every kit component for the same stroke weight, radius,
    gradient and top highlight; a test that fails on a component outside the kit's tokens.

Each item: measured with the `shoot` examples in both themes and at 75–200 %, held by the
frame budget, and judged by the owner's eyes.

## The side-by-side comparison (2026-11-04, the owner asked for a hard one)

Thirteen elements cropped at the same physical scale and magnified (crops made from the
screenshots above and `docs/images/`; the page is a private artifact,
https://claude.ai/artifact/9haNNcaDVADAwCC7AffnkZ). **FL wins 10, Cantus 2, split 1.**

| # | Element | Verdict | Why |
|---|---|---|---|
| 1 | Top bar | FL | two dense rows, ~18 px menus, CPU/RAM with numbers; ours 11 px menus and empty gaps |
| 2 | Transport, tempo, clock | FL (presence) | joined capsule, round red record, light large clock; ours carries more information |
| 3 | Toolbar icons | FL, clearly | one two-tone family, ~24 px glyphs; ours ~14 px, some look rasterised, active orange swallows them |
| 4 | Whole mixer | split | FL: whole-strip tint, vertical names; Cantus: labels, numbers, meters, chains, sends, cables |
| 5 | One strip ×2 | FL (finish) | panel relief, knob wells; ours flat circles and black boxes |
| 6 | Fader, LED, knob ×4 | FL | chrome cap with specular and shadow, glass LED, radial knob in a well; ours flat beige cap, hazy LED halo, 1 px knob |
| 7 | Arrangement | FL | clip content visible from afar; ours a line of tiny dots, section names repeated on every clip |
| 8 | Headers and clips ×3 | FL | saturated distinct tiles; ours three near-identical reddish browns |
| 9 | Channel rack | Cantus | velocity-visible steps, sub-rows, value arcs; name buttons flat |
| 10 | Piano-roll keyboard | FL, narrowly | wider keys, highlighted C; our note labels look doubled/blurred |
| 11 | Parametric EQ | Cantus | Aequalis does all FL's does and more (analyser, key, dynamics, table) |
| 12 | Browser | FL | rich rows and preview player; our shot shows the empty state (library builder, round 37) |
| 13 | Name type ×4 | FL | crisp condensed; ours shows a dark outline and soft edges (check the real app, not only the CPU shoot) |

**Proposed: a "finish" round after round 37** (the kit is shared, so it cannot change while
builders edit views): depth tokens per theme (light from above, inner shadow, specular,
bevel), controls redrawn as objects, one icon family, a type scale with a condensed face for
narrow places, a stronger user palette with tints, clip content at full height, the top bar —
accepted when the same thirteen shots win or tie against FL Studio 2026 in both themes at
100 % and 150 %, the frame budget held, and the owner's eyes agree.
