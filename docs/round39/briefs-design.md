# Round 39 — the design briefs (`dir-a`, `dir-b`, `dir-c`, `icons`)

Read `docs/round39/BUILDING.md`, **BACKLOG §6j**, `research/ui/fl-look-2026-11/notes.md`
(in `F:\projects\cantus`) and look at every picture they name before you draw anything.
You **only design**: no Rust, no cargo. Everything is drawn **by hand from a blank sheet** —
no glyph or shape taken from the current kit, an icon set, a UI kit or FL Studio; nothing
generic scaled up or down; no shortcut because a rule or a test would pass. FL Studio is the
reference for *quality* — calm, clean, crisp, colour only for state — **inspired, never
copied**: no FL shape, layout or colour value reproduced. The result should be *better* than
FL, and clearly Cantus.

## What every direction agent delivers (Phase A — the owner picks one)

One **whole-window direction**: a complete system, not a mood board.

1. **The main window, whole**, as the owner sees it: the top bar (transport, tempo, position,
   meters), the toolbar, the arrangement with the same song as
   `docs/manual/img/startup.png` (*Neon Tide*'s second drop: Kick, Clap, Hats, Percussion,
   Bass, Chords, Pluck, Lead, Choir — clips with readable MIDI and audio), the mixer below
   (master, 13 strips, the Reverb and Delay returns), the inspector at the right, the status
   line. Drawn as SVG at **1280×720 logical** and rendered at **1× and 1.5×**
   (1280×720 and 1920×1080 px), in **Cantus Night and Cantus Paper**.
   - Idle must look alive and calm: no "−∞" slabs, no dead black meters (§6j item 5).
   - Words where words help, drawing where drawing is clearer (§6j item 3).
   - A focal point (§6j item 8); the user's colours muted and spent as accents (§6j item 1).
2. **The same window with the top bar's overflow panel open** (Metronome, Windows, CPU/RAM,
   "Customise top bar…" — §6j lists every defect of today's; design it again: hierarchy,
   alignment, each shortcut under its own key, live meters as small graphs with numbers).
3. **A parts sheet** at 1:1 and 4×: the transport keys and toolbar keys at **16, 20 and
   24 px and their 150 %** (each size drawn for itself), a knob (rest, hover, dragged,
   automated), a fader with its meter (silent, playing, clipping), a mixer strip, a tab,
   a field, a menu with a submenu, a segmented control, a toggle (off/on), a scrollbar —
   both themes.
4. **The system**, on one page: the palette as named tokens with hex values (surfaces,
   ink 1/2/3, lines, accent, state colours — each with its contrast ratio against its
   surface), the type (an **OFL** family — condensed or with character, as §6j item 6 asks —
   with its scale: sizes, weights, the clock; licence, version and source URL recorded), the
   spacing grid, corner radii, line weights, the density, how a button looks at rest / hover /
   pressed / on / disabled.
5. **Beside FL Studio at the same scale**: your window next to `fl-playlist.png` and
   `fl-mixer.png`, and today's `startup.png` (Cantus beta.13) as a third column — a
   comparison sheet under `research/ui/round39/<name>/` (private: it shows FL).
6. `docs/round39/design/<name>/README.md`: the idea in a paragraph, why it beats FL and
   today's Cantus, what it costs to build (anything a kit cannot draw cheaply), and an index
   of the files.

**Look at every render** (read the PNG) and redo what is not first-rank. Show restraint: the
owner found today's Cantus "ugly, plain" and FL "clean, beautiful". Quality over quantity —
one superb window beats five sketches.

### The three hypotheses (start from yours; depart from it if you find better)

- **`dir-a` — Slate.** A cool, quiet grey-blue workspace; panels separated by value, not by
  borders; one restrained accent; the user's track colours desaturated to a stripe or a tint
  and fully lit only when selected or playing. The calmest of the three.
- **`dir-b` — Graphite and gold.** A neutral graphite (neither brown nor blue) with Cantus's
  own warm accent — take it from the logo (`crates/cantus-ui/src/kit/logo.rs`, the app icon)
  — and type with real character (a condensed face, a large thin clock). Warmth and
  identity without mud; the most "Cantus" of the three.
- **`dir-c` — Studio ink.** Deep near-black surfaces with luminous content: clips, meters and
  waveforms carry the colour; chrome nearly disappears; precise thin lines, a technical
  type; the best ideas of Bitwig and Ableton's clarity on FL's level of finish. The most
  contrasty of the three; must still be calm at rest.

Each direction designs **both** Night and Paper (Paper is a light theme: it must be as
finished as Night, not an inversion).

## `icons` — the icon family (starts now, beside the directions)

The icons are the owner's sharpest complaint ("hay botones que se ven super feos"), and their
drawing is mostly independent of the palette. Design **one family** for Cantus:

1. **The grammar first** (one sheet): the pixel grid per size (16, 20, 24 px and 150 %:
   24, 30, 36 px), stroke weights per size, how a glyph sits in its key (§6j: about a third of
   the key's height, with air), the two inks (ink-1 the body, ink-2 the edges and inner lines),
   corners, terminals, how a filled glyph differs from an outlined one, the on-state.
2. **Then the glyphs, each drawn for each size** (never one drawing scaled), in this order:
   the transport (play, pause, stop, record, loop, metronome, count-in, return to start,
   skip back/forward, follow), the toolbar and top bar (snap/magnet, undo, redo, draw, select,
   slice, erase, mute tool, zoom, eyedropper, gear/settings, the window keys: mixer, piano
   roll, channel rack, browser, playlist/arrangement, plug-ins, inspector), the mixer strip
   and inspector (phase Ø, L/R swap, link, mono, the bulb/enable, the dot, solo, mute, arm,
   FX on/off, send, sidechain, routing), and the browser (folder, file, audio, MIDI, preset,
   plug-in, pack, favourite, search, history). Read `crates/cantus-ui/src/kit/glyph.rs` for the
   full list of what exists today — that is an inventory, not a source of shapes.
3. **Ink as tokens** (`ink-1`, `ink-2`, `ink-on`, `ink-off`) so every direction can colour it;
   show the family on a dark and a light key, at 1:1 and 4×, beside FL's equivalents cut from
   the screenshots at the same scale (under `research/ui/round39/icons/`), and beside today's
   Cantus (`docs/round37/finish/icons-*.png`).
4. `docs/round39/design/icons/README.md` with the grammar, the inventory and its state.

Deliver the grammar and the transport + top-bar set first, show it (a report to the
integrator with the sheet paths), then continue down the list while the owner looks.
