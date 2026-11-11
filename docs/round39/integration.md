# Round 39 — integration: the whole window redrawn by hand, Slate by default, and the memory row

**The ask** (the owner, 2026-11-08, after beta.12): *"FL aún se ve a años luz de Cantus … hay
botones que se ven super feos"*; *"vamos por lo difícil, a mano, que nos dé los mejores
resultados"*. Round 39 is UX and graphics only (BACKLOG §6j), judged by the owner on whole
windows in both lights, not by crop judges. On 2026-11-09 he widened it: *"esto es muy muy
importante, pon varios agentes a trabajar en cada aspecto a mejorar … pon más agentes a hacer
los pendientes"*. On the way he asked for four things outside the look: a convolution reverb,
the memory row of the scorecard, a website review, and every picture on the site and in the
README reshot in the new look; at the close-out, Delay's own window and the older themes back,
redrawn, every one easy to read.

## How

Three phases, from master `80d5f109` (beta.13), every branch merged into `r39/integration`
(`F:\projects\daw-r39\integration`), one worktree per agent. Design agents ran no cargo; at most
three cargo builds ran at once through `F:\projects\daw-r39\slot.sh` (31 GB of RAM).

1. **Phase A — three directions and the first builders.** `dir-a` (Slate), `dir-b` (Graphite
   and gold) and `dir-c` (Studio ink) each drew a whole window in Night and Paper at 1× and
   1.5× beside FL Studio and beta.13; `icons` drew the icon family's grammar and first sets.
   Beside them, three builders worked in today's look: `ux-a`, `ux-b` and `memory`.
2. **Phase B — the owner chose Slate** (2026-11-09) and approved element sheets one by one:
   `des-mixer`, `des-controls`, `des-views`, `des-arrange`, and dir-a's top bar, toolbar and
   dialogs. Builders turned them into code: `kit`, then `el-topbar`, `el-controls`,
   `el-mixer`, `el-cables`, `el-views` and `el-arrange`. Beside them: `conv` (Aula), `fixes`
   (six BACKLOG §8 items), `logo`, `lapis` (Slate Lapis), `stack` (a start-up crash of the
   release build), the icon sets 3–6 (`icons-a`, `icons-b`, `icons-c`, `icons-guitars`), and the
   website (`web`, `web-content`, `web-struct`, `web-extra`).
3. **Close-out.** `lo-arrange`, `lo-mixer` and `lo-rest` closed every builder's "What is
   left"; `shots` reshot the pictures; this account, STATUS, the manual and the release notes
   (`docs`). Then the late branches, after the owner's look at the merged tree: `lo-mixer`'s
   second delivery (the desk fills its width), `lapis2` (Slate Lapis told from Slate at a
   glance), `delay` (Delay's own window, ADR 0115), `themes` (the ten older themes back,
   redrawn, and four new ones) and `fix2` (the integrated tree's defects; the goldens
   re-blessed and the pictures reshot after `themes`).

| Branch | Up to | What | Report |
|---|---|---|---|
| `r39/ux-a` | `46e646ac` | A driven control says so; the help line never empty; nothing truncated (ADR 0109) | `ux-a.md` |
| `r39/ux-b` | `fa348c76` | Panels fold and maximise; keyboard reach; the daily click paths counted (ADR 0110, keymap 16) | `ux-b.md` |
| `r39/memory` | `433e4c7c` | The engine's memory grows with the song (ADR 0107) | `memory.md` |
| `r39/kit` | `602e1f77` | The kit becomes Slate: six themes, faces, wells, icon set 2 (ADRs 0108, 0111) | `kit.md` |
| `r39/conv` | `bb6af5b9` | Aula, the convolution reverb (ADR 0112, schema 60) | `conv.md` |
| `r39/fixes` | `6fbce05c` | Six BACKLOG §8 items, and the picker's subtitle | `fixes.md` |
| `r39/el-topbar` | `26371fda` | dir-a's top bar, More panel and dialogs | `el-topbar.md` |
| `r39/el-controls` | `a91de9e4` | Every control family in Slate, icon set 5, `kit::dialog` | `el-controls.md` |
| `r39/el-mixer` | `0f9a7416` | des-mixer's strips, desk and inspector (first pass), icon set 3 | `el-mixer.md` |
| `r39/el-cables` | `2fa98e01` | The cables in four styles, Flow by default | `el-cables.md` |
| `r39/el-views` | `f6a75691` | The piano roll's keyboard, the rack, the browser, clip content at four zooms, icon set 4 | `el-views.md` |
| `r39/el-arrange` | `32027e86` | The arrangement in Slate; sections, folders, a mode per lane, a track's icon (ADR 0113, schema 61) | `el-arrange.md` |
| `r39/icons` | `b06bde78` | The icon family, sets 2–6 (316 track icons in 17 families) | `icons.md` |
| `r39/logo` | `b2cfda00` | Logo #03's small masters; the accent study | `design/logo/README.md` |
| `r39/stack` | `19f4f9df` | The release build's stack overflow at start | the commit (`b578fc23`) |
| `r39/lapis` | `e7bc3657` | Slate Lapis, an optional theme | `lapis.md` |
| `r39/web` | `e7a90ed9` | The site's review and plan P0–P3, built | `web/analysis.md`, `web/web-struct.md`, `web/web-extra.md` |
| `r39/lo-rest` | `41647fd9` | The idle repaint; the rack's and the browser's leftovers | `lo-rest.md` |
| `r39/lo-mixer` | `a086c151`, then `fa769a57` | The mixer's and the inspector's leftovers; then the overflow pill in a slot of its own and the desk filled to its width with the next empty strips | `lo-mixer.md` |
| `r39/lo-arrange` | `cb90ac0e` | The arrangement's tools, Follow, the grid menu (ADR 0114) | `lo-arrange.md` |
| `r39/docs` | `e0bbdcf6` | This account, STATUS, the round-40 prompt, beta.14's notes, the checklist, the manual, ADR 0111 accepted | this file |
| `r39/lapis2` | `f38090d8` | Slate Lapis told from Slate: a night-blue room, lapis on the selection, gold readouts and loop edges; Slate's window golden | `lapis2.md` |
| `r39/delay` | `65ab2356` | Delay's own window: the echoes drawn in time and dragged, what is heard, a tape echo's face (ADR 0115) | `delay.md` |
| `r39/shots` | `ede983d7` | Every interface picture of the site, the README and the manual in Slate | `shots.md` |
| `r39/themes` | `9baa7b63` | The ten older themes back, redrawn (Pewter and Chalk renamed), four new; every theme measured legible; whole menus and status texts in every face | `themes.md` |
| `r39/fix2` | `36312c86` | The icon picker's click away; `Project::strip_is_free`; the mixer folds only what does not fit; the desk's stripes at 3:1 on light themes; the goldens re-blessed, the pictures reshot | `fix2.md` |

**The merge order** (the first parent of `r39/integration`): `ux-a`; `ux-b`, `memory`, `kit`;
`conv`, `fixes`; `el-topbar`, `el-controls`, `el-mixer`, `el-cables`, `el-views`,
`el-arrange`, `icons`, `logo`; `stack` (twice); `lapis`; `web`; `lo-rest`, `lo-mixer`
(`a086c151`), `lo-arrange`; the seven design branches; `docs`; `lo-mixer` again (`fa769a57`);
`lapis2`; `delay`; `shots` (it had merged the integration at `1e92c38b`, so it was shot with
`lapis2` and `delay` in); `themes`; `fix2` (it had merged the integration at `9c973bc5`, so it
was built, tested and shot with `themes` in).

The design branches (`dir-a`, `dir-b`, `dir-c`, `des-mixer`, `des-controls`, `des-views`,
`des-arrange`) hold their sheets under `docs/round39/design/<name>/` and the private
comparisons under `research/ui/round39/<name>/`; they are documents only, and the builders'
reports and ADR 0111 point at them. They were not in `r39/integration` at `3e63b72b`; the
coordinator merged all seven into it at `5bdf4df2`, so their sheets are in the tree (the FL
comparisons stay under `research/`, which the public rewrite strips).

Version **0.1.0-beta.14**. Schema **61** (60 Aula's response from a file, 61 the arrangement's
structure; each with its migration and fixture). Keymap file **16**. `DSP_VERSION` stays **6**:
no golden hash moved. ADRs 0107–0114, numbered in the contract and not renumbered; ADR 0115
(Delay's window) came with the late `delay` branch.

## What each part brought

### The look: Slate, chosen from three (ADR 0111)

- **Three directions, whole windows.** Slate: a cool grey-blue room, panels apart by value,
  never by borders, one azure accent spent on state, the user's colours as a stripe and a tint
  until a track is selected. Graphite and gold: a neutral graphite lit by the logo's metal, a
  humanist face in three widths. Studio ink: near-black chrome and luminous content. Each in
  Night and Paper, beside FL Studio and beta.13 (`F:\projects\daw-r39\review\tres-direcciones.png`).
- **The owner chose Slate as the default** (2026-11-09) and kept the other two as options
  (*"los demás modos se ven bien también, me gustaría tenerlos como opciones"*). Slate's base is
  `#1D2329` and its surface `#262D34`, with the azure `#5CAFE9` (Night) / `#1F6EC2` (Paper)
  where FL uses orange and lime; its face is Barlow Semi Condensed (OFL).
- **Twenty-four themes ship** (ADR 0111 §2), on Settings ▸ Appearance in four groups: round
  39's families — Slate, Slate Lapis, Graphite and gold and Studio ink, each Night and Paper;
  **the ten themes of rounds 24–38, redrawn** in round 39's system (`themes`), each in its own
  light and character — Cantus Night · Paper, Blue Hour · Daylight, **Pewter · Chalk** (the
  FL-derived *Slate* and *Paper*, renamed so they collide with neither the new Slate nor
  anyone's mark), Ink & Gold · Parchment, Ultraviolet, Lagoon; **four new** — Evergreen Night ·
  Paper, Bordeaux, Print; and the high-contrast pair. The round first retired the ten older
  themes (a setting that named one showed Slate in its light); the owner asked for them back
  (2026-11-10: *"Recover all of them, and add more if you want"*), so the retirement is
  reversed: every old id names its own theme again, its slot's changes on top, and users' own
  themes are untouched. Each is written by `tools/themes/themes.py` over Slate's own ladder,
  with recessed keys; no two themes of one light are closer than CIELAB ΔE 14.
- **Every theme easy to read, measured** (the owner: *"Todos deben ser fáciles de trabajar y de
  leer"*): `views/arrangement_legibility.rs` holds every shipped theme to 3:1 for what carries
  meaning (WCAG 1.4.11) and every text to 4.5:1. At the cause, in every theme, Slate's
  included: the arrangement's bar lines to 3:1 and beat lines to 1.35:1 on the lanes (they were
  1.2 and 1.0:1), solo's ink to 3:1 on its key; in every light theme, the track's colour marks
  to 3:1 on the bars (`kit::color::track_mark`), the clip bodies at rest ΔE 3 apart and more,
  the paper fader cap's rim to 3:1, and (`fix2`) the desk's strip-head stripes to 3:1
  (`mixer_look::Pal::stripe`). Slate Night moved only in its grid lines and solo's ink. **The
  menus and the status line are whole in every face**: their width caches were keyed per
  context, so a second window in Sofia or Plex lost *Help* and cut the status line;
  `tests/themes_whole_words.rs` starts all twenty-four in one process.
- **The interface's size follows the screen** until the user picks one: 1.25 × (physical width
  ÷ 1 920), 100–200 % (ADR 0111 §7). A settings file from before keeps its scale.

### The kit (ADR 0108, how it draws; ADR 0111, the system)

- A theme carries a **system** (`kit::system`): its face and how its keys are built. Slate's
  keys and fields are flat **wells** (a 1-px dark edge, a 1-px light lip); hover lifts, on tints
  with the accent, green for play, red for record; a radio choice is a lifted neutral pill.
- **Faces** per family: Barlow Semi Condensed (Slate), Sofia Sans in three widths (Graphite),
  IBM Plex Sans Condensed and Mono (Studio ink), each with tabular figures by default
  (`tools/fonts/build_faces.py`); 1.59 MB added, with their OFL notices.
- Static richness is rasterised once per (shape, size, inks) into the layer atlas and drawn as
  one quad on the pixel grid; vectors are the kit's own (ADR 0108). Idle shows no "−∞".

### The icon family and the logo

- **The icon family**, drawn by hand for each device size (16, 20, 24, 30, 36 px; no size scaled
  from another). The first pass was not approved (*"más elegantes, bonitos y mejor pensados"*);
  a study of Ableton, Bitwig, Logic, Studio One and FL led to **set 2** (transport, tools, window
  keys, 29 glyphs), **approved on 2026-11-09**: the family. Then set 3 (the mixer strip and
  inspector, 23), set 4 (the browser, 17) and set 5 (the rest of `kit::Glyph`, 30; the bulb
  retired), drawn in set 2's grammar and **in use in the app, pending the owner's review**
  (ADR 0111 §6), and **set 6**:
  **316 track icons in 17 families**, each drawn for ten device sizes, searchable in English,
  Spanish and aliases (`assets/icons/tracks.txt`). The guitars were redrawn twice, the last time
  from a photo study (`icons-guitars`).
- **Logo #03, "the metal"**, is the default logo and the application's icon (ADR 0111 §9): its
  small masters below 48 px, the `.ico`, the Linux sizes, the `.icns` and the splash rendered
  from it at build time. The window's own icon is #03 in its own colours; a colour the user
  chooses still paints the mark. Four enriched variants of #10 stay on record (`design/logo/`).
- **Slate Lapis**, an optional theme for #03. The first (`lapis`) was Slate with a lapis
  accent, and the owner found the two "look the same"; `lapis2` built the three changes he
  approved: **lapis on the selection** (`#7EA2F6` / `#2443A8`, and `accent.selection_tint` laid
  over every selected fill, a lapis mark on the active tab's top edge), **gold on the readouts
  and marks** (`text.readout` and `text.readout_secondary` for the clock, the tempo and the
  rulers' bar numbers, at 4.5:1 and more; `editor.loop_edge` along the loop band), and **a
  night-blue room** (Slate's 28 surface tokens at the same WCAG luminance, turned to the lapis's
  hue, so every contrast Slate holds is kept). The four new tokens are clear in every other
  theme, and `tests/slate_window_golden.rs` holds Slate's whole window to the pixel. The app's
  default accent did not change; lapis is the website's accent (the owner, 2026-11-09).

### The window, element by element (dir-a's phase B and the des-* sheets)

- **The top bar** (`el-topbar`): one row, 44 high, two clusters packed from their edges, every
  key a recessed well; the clock in Barlow ExtraLight 30; CPU as a graph with its number; the
  master meter as two 3-px lanes. A fold order measured on the sheet; what folds goes to the
  **More** panel, laid out in sections (Metronome, Windows with each shortcut under its key,
  Performance with 30-s graphs, Inputs, Tools and zoom). Spanish folds rather than collides. The
  help line moved to the status line. Settings, the first-run audio setup and export redrawn
  (the driver as a segment, the buffer's latency scale, the inputs as slim lanes).
- **The controls** (`el-controls`): knobs with a lit cap and their driven states, faders with the
  0 dB detent and a meter in its own wells, sliders, value bars, fields, drop-downs, menus as one
  sheet (no boxes inside boxes) that keep inside the window, segmented controls, tabs, scrollbars,
  tips, buttons and **one dialog frame** (`kit::dialog`) for every dialog. The driven signs each
  have a shape and a colour: automation (a lane, violet), a modulator (a sine), a controller (a
  socket), a VCA (a V and a dashed ghost cap).
- **The mixer** (`el-mixer`, `lo-mixer`, `fix2`): des-mixer's rows in signal order, Compact /
  Wide / Large at 30 / 60 / 94 pt, no label column, no empty slot rows; M|S as one pill, the arm
  a dot, the send level a small knob; each strip's real EQ curve; a track's icon on its strip
  head; the desk's header sharing the workspace's tab row. The desk fills its width with FL's
  next empty strips, numbered (painted only; a click makes the strips up to it in one undo
  step); it folds only what does not fit, the overflow's "n ›" in a slot only on a side with
  strips hidden, sized to its count (30 pt for one digit, 36 for two). Whole window with 64
  strips: 96 584 → 46 855 triangles.
- **The inspector, second pass** (`lo-mixer`): Source, Inserts with a mini view, Equaliser with
  its band table, Sends, Receives, then folded rows: Routing, Sidechain, Recording, Automation
  (its mode, a sparkline per lane, **Show in the arrangement**), **Level** in place of the pinned
  strip (volume, pan, width, a slim meter, Mute · Solo, Safe from snapshots and from solo) and
  **Freeze** ("2 inserts rendered · 12 MB").
- **The cables** (`el-cables`): Patch, **Flow** (the default, the theme's), Subway and Jack;
  chosen in the Cables key's menu or in Settings ▸ Appearance; hover lights a whole route to the
  master; drag to connect, with the refused loop said in words; Flow's gleam rides the frames the
  playing desk already asks for, and nothing is asked at rest.
- **The views** (`el-views`, `lo-rest`): the piano roll's keyboard as an instrument (ivory and
  ebony at a real piano's widths, every C named), ghost notes, the velocity lane, the ruler; the
  rack's steps filled to their velocity, a melodic channel as a small roll, two bars a page, a
  pattern map, plates at des-views' measures, **a kit's pads folded under its row**, **Add a
  channel** and a drop zone; the browser's rows with a sound's own waveform, pack covers drawn
  from their content, **grouped search** with chips, a **preview card** with its star, loop and
  level, **favourites**, and the **Sound packs** window as 128-px covers with their state;
  clip content at four zooms (MIDI, drums, audio, automation).
- **The arrangement** (`el-arrange`, `lo-arrange`; ADRs 0113, 0114): headers with the user's
  colour as a stripe, M S ● A as letters at rest, heights 24–96; **sections** and **folders**;
  **a mode per automation lane**; **a track's own icon** from set 6; the song map as the scroll
  bar; the empty song's starters. The tool row: **Select, Draw, Slice, Erase, Mute, Zoom** as a
  radio set (keys from FL's letters and the number row, Zoom on 8), **Follow** (on at start),
  the grid menu in musical order with Triplets (Shift+3) and *Follow the zoom*, the panel menu
  "…"; clips lit, dim, moving and resized as the sheets draw them; the loop notch; automation
  values at the close zoom; the piano roll's head with its clip menu and the selection readout;
  a maximised area keeps a row of tabs for the hidden ones.

### UX (ADRs 0109, 0110)

- **A driven control says so**: its sign, and a hint that names what drives it and how to take
  it back. **The help line is never empty**: 210 of 1 268 controls wrote nothing on hover; now
  none of more than 1 700. **Nothing truncated**: a probe over 440 view-looks found 97 cut texts;
  now none.
- **Click paths counted** through `assemble` (13 daily paths, against FL and Live): an instrument
  in 2 clicks (was 3), an effect in 2 (was 4), a drawn loop loops, Ctrl+F finds a preset, Settings
  remembers its page. **Every area** folds, maximises, floats and scrolls; every top-menu row has
  a key or a palette entry; every kit menu walks with the keys; Ctrl+F6 moves between areas; Esc
  closes what opened last.

### Delay's own window (`delay`, ADR 0115)

Delay was the one effect on the site and in the README with no window of its own. It has one
now, in a face of its own (a tape echo at night: oxblood vinyl, the echoes on a phosphor
screen), and stays in the inspector as the other windowed effects do: a click on its insert
opens the window, the mini view under it keeps the quick controls. **The display is the
control**: each repeat a stem at its time, on its side with ping-pong, at its level and
coloured by what the tone takes from it; drag across for the time, up and down for the
feedback, one undo step. *At the tempo* sets the time once from a note value (the unit has no
sync, and no DSP was added); *What is heard* gives the repeats to −60 dB, the tail and the
tone's loss per repeat. No DSP file changed: Delay sounds the same to the bit. A test feeds the
real unit an impulse and finds every drawn repeat in its output, at its time, side, level and
8 kHz content.

### Aula, the convolution reverb (ADR 0112, schema 60)

Zero latency (64 taps direct, then partitions of 64 / 512 / 4 096 / 32 768, each level's work
spread over its slack), the same bits at any block split, within −130 dB of a direct convolution;
fifteen responses made by Cantus, files in mono, stereo or true stereo, and the free *OpenAIR
Impulse Responses* pack (CC BY 4.0, credited); a CLAP and VST3 build; an agent tool. Its twelve
presets fitted into their loudness window at the integration.

### Fixes (`fixes`, BACKLOG §8)

A shared helper's crash laid on the plug-in whose job was running, never on one waiting behind it;
Chorda model 2 made without waiting for its body's responses (16 ms off the interface's thread);
Chorda's scene baker keeps one pool for all its passes; the `stage_budget` variance found (the
taskbar's poll re-applied the whole look and every install chilled the caches); the arrangement's
stored header width no longer clamped; a preview's level changes without restarting the sound;
the device picker names the strip.

`fix2`, on the integrated tree, each at its cause with a test that fails without it: the icon
picker's click away closes it and lands on nothing (over the desk it had made mixer strips; over
a fader it moved it); **`Project::strip_is_free`** counts a strip free only when nothing routes
into it and nothing was set on it (`MixerStrip::fresh`), so a new track no longer takes a bus
that other strips feed (the house demo's dancer had taken the drum bus and renamed it); the
mixer's fold above; the desk's stripes at 3:1 on light themes. Slate's window goldens were
re-blessed after each pixel of the difference was looked at, and the pictures reshot.

### Memory (ADR 0107) and the start-up crash

- The processor before any song: **95.8 → 8.0 MB**; a start on the demo, live heap, 145.5 →
  59.7 MB. Pulsus kits and note-effect chains move between slots only while a track needs them;
  the block's report goes through a mailbox (`EngineEvent` 3 880 → ≤ 64 B); the compensation pool
  starts at 32 lines; the slots grow with the song (16 track and 32 strip slots at first); scope
  rings are made when first watched. The audio thread never allocates (`rt_memory.rs`).
- **The release build overflowed its 1 MB main thread at start** after the merges (the processor
  and the application moved by value through deep frames). The large parts live on the heap now,
  size bounds hold them, and `cantus.exe` links an 8 MB main-thread stack as a guard.

### The website (`web`)

A review (`web/analysis.md`): the content honest, the structure not first-rank. Built from its
plan: the real program in the hero, a proof strip of three wins and one loss, a phone menu, every
page pre-rendered (86 pages, EN and ES addresses with hreflang), a nine-chapter home with
sub-pages (download with SHA-256 and the SmartScreen steps, measured, studio, AI, roadmap, learn,
a page per unit), JSON-LD and a sitemap, the release feed, the site's budget in CI; #03's gold
for accents, distinct bands. Lighthouse mobile **98–99 / 100 / 100 / 100**, LCP 2.03 s, the first
visit 8.2 MB → 0.8 MB. The dancers are off the site and the README (the owner, 2026-11-09); they
stay in the app.

## The integrator's own fixes

`integ-fix` and `integ-2` merged the branches in turn and fixed what the merges broke, each at
its cause (the `integ-fix:` and `integ-2:` commits): the fifteen cantus-ui failures of the first
merge; an engine event back to 64 B (Aula's request waits in a per-unit mailbox); one dialog
frame for the recessed family; the help line in the status line registered as the hint panel;
the VCA's ghost cap shared by the kit's fader and the desk; a group's separator on des-mixer's
desk; set 6 plugged into the track icon picker (ids in kebab case, a recent row); logo #03 as the
default and the application's icon; the ten desk and inspector tests des-mixer's layout moved;
the agents' tool list taking `set_track`'s icon; Aula's outside test file, which the repository's
`*.wav` rule had kept out; `THIRD-PARTY-NOTICES.md` with the OpenAIR pack's credit; and
`memory_e2e` measuring Exemplar's built-ins as a step of their own. The close-out's own fixes are
in `lo-arrange.md`, `lo-mixer.md` and `lo-rest.md` (*the coordinator's second look*).

## The frame budget

Counted in every run (`frame_budget`, `slate_budget`, `workspace_budget`, `views_budget`):
no more draw calls or triangles than beta.13's, allocations within 8.

| Measure | beta.13 | Round 39 |
|---|---:|---:|
| The whole 1080p window playing (Slate Night against Cantus Night) | 40 641 triangles, 90 draws | 40 446 triangles, 90 draws (el-controls) |
| The 64-strip desk alone | 100 787 triangles | 27 814 triangles, 9 draws |
| The 64-strip desk, whole window | 96 584 triangles | 46 855 triangles, 86 draws |
| The 64-strip desk, all cables (Flow) | — | 113 346 triangles (ceiling 120 000); Jack 248 820 (ceiling 260 000) |
| The arrangement's warm frame, stress song (release; the column before is the round's base before el-views) | 0.86 ms | 0.34 ms |
| The whole window, build + tessellate, p50 (release, a shared machine) | 1.73 ms | 1.49 ms |

Every cable style adds no allocation and no draw call. **At rest nothing asks for a frame**:
`idle_repaint_e2e` holds 120 idle frames each asking nothing sooner than a second, at 100 % and
125 %, over the arrangement, the mixer with every cable drawn, the piano roll, the rack, the
browser and the Sound packs window (the repaint el-cables saw was its test rig's input events).

## Measured

**Private memory** (`docs/round39/memory-measured.md`), the scorecard's script and settings,
five starts after one unmeasured start, against beta.13 in the same session. The machine was
**not quiet** (builds ran beside it), so only the memory column is valid; it does not depend on
load.

| Build — what it opens | Private (median of 5) | At rest, 30–40 s |
|---|---:|---:|
| beta.13 — an empty song | 236.5 MB | 227.5 MB |
| **Round 39 — an empty song** | **141.2 MB** | **137.2 MB** |
| beta.13 — the *Beat* starter | 237.9 MB | 228.6 MB |
| **Round 39 — the *Beat* starter** | **144.6 MB** | **140.6 MB** |

−95 MB (−40 %) on an empty song. **Still lost**: 141 MB against REAPER's 62 (by 79 MB), 145 MB
against FL Studio's 133 (by 12 MB), from round 38's quiet evening. Cantus's own live heap on an
empty song is about 25 MB; the rest is the heap's reserved segments, the graphics driver's
buffers, thread stacks and the drivers' heaps, which `tools/scorecard/memmap.ps1` maps.

**On a quiet machine, the final release** (2026-11-11, the release build at `81f88171`;
`docs/scorecard/README.md`, `docs/round39/memory-measured.md`): no builds, 13 % of the CPU busy
before the runs, 0.6–0.9 of a core of other load during them; the same script and settings,
fresh copies of round 38's data folders and projects. Medians of five:

| beta.14 — what it opens | Window (best) | Quiet | Private | At rest, 30–40 s (cycles) | Private at rest | Working set |
|---|---:|---:|---:|---:|---:|---:|
| An empty song | 755 ms (728) | 1 485 ms | **139.7 MB** | 1.1 % (1.5 %) | 135.7 MB | 171 MB |
| The *Beat* starter | 770 ms (760) | 1 515 ms | **142.7 MB** | 2.0 % (1.7 %) | 138.7 MB | 178 MB |

Private memory −97 MB (−41 %) on an empty song and −96 MB on *Beat* against beta.13; window,
quiet and at rest the same as beta.13's within the noise, and still won against REAPER and FL
Studio. **Memory is still lost**: 140 MB against REAPER's 62 (by 78 MB), 143 MB against FL
Studio's 133 (by 10 MB).

## Checks

- **Windows**, the full suite on `d854ec3d`: **6 781** passed (cantus-ui 2 649, the rest
  4 132), 0 failed, 106 ignored (54 and 52). After it come `1e88dd03` (documents only) and
  `81f88171` (the version). The engine fuzz (`cargo test -p cantus-engine --features fuzz`,
  `CANTUS_WORKERS=3`): **1 145** passed, 0 failed, 17 ignored. Clippy
  `--workspace --all-targets -D warnings`: clean.
- **Linux** (WSL Ubuntu 24.04, `tools/wsl/check.sh`) on `1e88dd03`: **6 742** passed, 0
  failed, 106 ignored; clippy clean; `cargo fmt --all -- --check` clean. (Before the close-out,
  on `42ebe18e`: 6 686 passed, 0 failed.)
- **The release exe** (the release build at `81f88171`, `cantus.exe` 0.1.0-beta.14, 77 MB)
  started and kept running 30 s in three cases, each with its own data folder (the suite does
  not catch a stack overflow at start): none fell, every window open at 30 s — an empty song
  136 MB private at 30 s, the *Beat* starter 138 MB, a first start that opens the demo song
  *Neon Tide* 179 MB.
- `cantus-sandbox::session_faults9` (beta.12's) failed on both Windows runs at the first beta.14
  tag (2 and 6 of 100): a real race, fixed at the root (`abort-blame.md`, ADR 0063 §9) and the
  tag moved. Known flaky, each in BACKLOG §8: `views_nav`'s
  scroll-bar zoom test (failed once in 124 runs); `agent_tools::export_writes_only_where_allowed…`
  failed once on "Access is denied" in the shared temporary folder and passed 14 of 14 alone.
- macOS: at the tag, on GitHub's runners.

## What was verified only in emulation

Everything in this round was drawn by the CPU renderer (`raster.rs`) and driven by synthetic
pointer and key events through `CantusApp::assemble`; nothing was seen on a monitor or touched
with a real mouse. The owner's checks are in `docs/beta-checklist.md` (*Ronda 39*):

- Slate and Slate Lapis at 100, 125 and 150 % on a real screen, Night and Paper; the scale that
  follows the screen (tested with a synthetic monitor size, never Windows' own scaling).
- The twenty-four themes on a real screen: their legibility was measured on the colours the
  painters use and seen on the CPU renderer's shots (`docs/round39/themes/`); no theme's
  strength was judged on a monitor.
- Delay's window with a real interface playing: its drag and the repeats' glow were driven by
  synthetic pointer events and analyser pairs.
- The top bar on a real window, the header in the tab row with real tab drags and floating
  windows, the menus walked with a real keyboard.
- The mixer cables' gleam at 60 Hz, Jack's settle after a real drop, the plugs and the 1.25-px
  rings at 125 % and 150 %.
- Every icon set and the track icons at 125 % and 150 % on a real screen (egui's arcs and pixel
  runs may differ from the sheets' Chromium renders).
- The logo in the taskbar, the `.exe` and the installer.
- Aula by ear (Cantus's fifteen responses are generated, never recorded), a live swap's timing on
  a real interface, and the OpenAIR pack's CC BY notice (read on the Internet Archive's copy).
- The Sound packs window with a real download (its fetching state was drawn through a test hook).
- The MIDI lamp, the device names and the dropout graph with a real device.
- The website on Cloudflare, `site.yml` on the runner, a real phone and a screen reader.

## The owner's decisions still open

Collected from every report; each is the owner's alone. The builders built the sheet or the
safer reading and said so.

**The window and its areas**
1. **The right dock's place** (`lo-mixer`): the mixer's right dock keeps round 37's place
   right after the last strip; des-mixer's 1280-pt sheet pins it at the window's edge. Since
   lo-mixer's second delivery the room after the dock holds the next empty strips, numbered (as
   FL's desk), so there is no dead space either way, and a desk that folds keeps the dock at its
   edge (`fix2`).
2. **What maximise hides** (`lo-arrange`): des-arrange's `max-*` keeps the inspector beside the
   maximised arrangement and folds only the docks below; ux-b's maximise gives the area the
   whole workspace. The row of tabs is built on ux-b's (every hidden area in it, the inspector
   too); keeping the inspector would change maximise for every area.
3. **Panels** (`ux-b`): the folded rail's look, five recent devices, Ctrl+Alt+Enter for maximise.

**The top bar** (`el-topbar`)
4. The bar no longer carries the automation arm and its modes nor the typing-keyboard key (the
   sheet has neither; their keys, the palette and the status line keep them); the seventh window
   key is the plug-in window (F8), the session view on Tab and in View; the record options open
   by a right-click on the record key; at 1920 × 1080 and 100 % the bar folds the tools (English)
   or the tools and the inputs (Spanish), the tabular clock being ~30 px wider than the sheet's.

**The arrangement** (`lo-arrange`, `el-arrange`)
5. **"1/2" or "½ bar"** in the grid menu: the sheet says "½ bar"; the menu says "1/2", because
   in 3/4 a half note is not half a bar.
6. **Follow on at start** (FL's default); it turns the page only when the playhead leaves the
   view.
7. **The section drag that ripples the song** (BACKLOG §3): the sheets' drag moves a section's
   bars on every track (Ctrl copies, Shift inserts); today it moves the section alone. A feature
   of its own, with insert and delete time.

**The mixer and the inspector** (`lo-mixer`, `el-mixer`, `el-cables`)
8. **The meter in Level**: the slim meter under the volume is the pinned strip's, kept (the
   sheet draws none).
9. The click targets on des-mixer's 30 / 60-pt strips: a key's target is its share of the strip
   (Wide 20 × 24, Compact 15 × 20), not round 37's 24 × 24.
10. Which cable style reads best on a 40-strip desk; Flow is the default.
11. Two engine features the inspector sheet draws and the engine lacks, in BACKLOG §6j:
    *Hear the key* (sidechain audition) and *Record: Before FX · After FX*.

**The controls** (`el-controls`, from des-controls' sheet)
12. The automated knob's violet; the fader cap's weight; Paper's menus and dialogs; value bars
    in the inspector; Mute on = grey.

**The browser and the rack** (`lo-rest`, BACKLOG §6j)
13. **The browser's kinds switch** (*All · Sounds · Plug-ins · Presets*, one search across them,
    results grouped by kind): a structural change for an ADR (which panel owns the drag onto a
    track, what the picker becomes).
14. **A pad's own mixer track** in the rack (des-views' well on each pad): a model change and a
    schema bump.

**Icons, faces and themes**
15. **Icon sets 3–5 review**: set 1 was rejected and set 2 approved; sets 3, 4 and 5 are in
    use in the app and the owner has not reviewed them (ADR 0111 §6, `icons.md`), with the
    sheet's open questions (the plug solid at 16–20 px, the rack's unlit steps in the second
    ink, the six-tooth gear at 16 px, the soft 12-px star, the session's lit cell at 16 px).
    The piano roll's Paint and Playback tools have no family drawing yet.
16. **The units with wooden faces** (`shots`): Anulus, Phasis, Calor, Organum and others are
    still brown because wood or a period material is their own face (ADR 0074, *Units wear their
    own faces*, on by default), not the theme. Keep, or redraw those faces for Slate.
17. **Slate Lapis** (`lapis2`): the arrangement's playhead *line* is the accent (lapis), as
    Slate's line is its accent, and only its cap is gold; gilding the line too is one token
    more. The line keeps Slate's 1.5 pt (a thinner one needs a per-theme width). And the
    strength, judged only on the CPU renderer's shots: the selection tint (18 % Night, 10 %
    Paper) and the room's saturation gain (× 1.7 / × 1.8).
18. **Each redrawn and new theme's strength** (`themes`), on a real screen: Parchment's cream,
    Bordeaux's wine, Ultraviolet's violet, Pewter's mid-grey. Print's playhead line is ink and
    only its cap vermilion (the line is each theme's accent); the user's track colours go
    through each theme's veil, so in Print they stay tints, not grey (a monochrome mode for
    clips would be a feature of its own).
19. Names cut in the middle where a place is narrower (the desk's rotated strip names at 150 %
    on a 1280 × 720 window, ADR 0024 §3): whether those places should grow.
20. The vintage theme's round; what a dancer should be in the Slate look, or whether there
    should be one (BACKLOG §6j).
21. The legal reading of Slate beside FL Studio (`docs/beta-checklist.md` §2): low risk in the
    integrator's reading (different values, accent, icons, type, logo and name), a lawyer's
    opinion pending.

**Delay** (`delay`, ADR 0115; BACKLOG §3b)
22. **A synced time** (Delay's time follows the tempo): a new parameter and DSP, not added
    (*At the tempo* sets the time once and says it does not follow); and **exporting Delay as
    a CLAP/VST3 plug-in**, which it never was (a plug-in's ids are forever).

## What is left

- **The owner's acceptance** of the whole windows in both lights, on a real screen
  (`docs/beta-checklist.md`, *Ronda 39*). The round's prompt made it the acceptance test.
- **ADR 0111** is accepted: §8 holds each element sheet's measured spec and where the build
  differs from it on purpose, from the sheets now in `docs/round39/design/` and the builders'
  reports.
- **Memory** (ADR 0107, the scorecard): the final release was re-measured on a quiet machine
  (2026-11-11, *Measured* above) and the scorecard's rows changed with it — 140 MB and 143 MB,
  still lost; next, map the private bytes outside the live heap with
  `tools/scorecard/memmap.ps1`, the next memory item. Slots, parts and lines are
  kept for the session once made; giving them back when a smaller song opens is not done.
- **Shots**: done. `shots`' phase 2 reshot every whole-window picture of the site, the README
  and the manual from the merged tree, the crops measured on the layout; `fix2` reshot them
  again after `themes` and its own fixes (`tools/shots/reshoot.sh rest`), and Delay's unit
  pictures are its window's. Every picture still to be seen by the owner on a real screen.
- **The website**: Cloudflare (Brotli, the trailing-slash redirects, `404-page` under `/es/`),
  `site.yml` and `site-budget.yml` on the runner, a real phone, a screen reader, Lighthouse in the
  dark theme; P2-1 (a muted loop of Cantus playing), P2-2 (a copy pass) and P2-3 when the owner
  places them.
- **Round 40** (proposed in `docs/next-session.md`): Chorda's third voicing pass and ABX 4,
  ripple edit / insert and delete time (with the section drag), and memory's next item.
- The owner's own items in `docs/beta-checklist.md`.
