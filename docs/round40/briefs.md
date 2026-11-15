# Round 40 — the builders' briefs

Read `docs/round40/BUILDING.md` first (ownership, numbers, rules), then `CLAUDE.md`, then your
brief. Every brief ends the same way: `docs/round40/<name>.md` and a short report saying what the
owner should look at or listen to.

## `chorda` — the third voicing pass and ABX 4 (phase 1)

**The owner's goal, in his words: that Chorda sound as good as the Salamander.** His baseline
(beta.14, `docs/round39/abx/2026-11-12-luis-almazan-beta14.json`, `docs/round39/after-beta14.md` §6):
Chorda 2 / Salamander told apart 16 of 16, the Salamander preferred 7–0 in every phrase; Chorda 2
preferred over Chorda 1 6–1, **but model 1 wins on *Bass notes alone* — look at the bass first.**

Read `docs/round38/chorda.md` §2 (the pass begun: `chorda_registers.rs`, the search in
`F:\projects\daw-r38\voicing\` — copy it to `F:\projects\daw-r40\voicing\`), ADR 0098 and its
amendments, the round-37 Chorda reports.

1. **The bass first**: compare model 1 and model 2 on the bass notes alone (spectrum, decay,
   attack, the low-mid thickness ≈ 500 Hz) against the Salamander's same notes; find why model 1
   reads better there, and fix model 2's bass without losing what made model 2 preferred elsewhere.
2. The *fortissimo* attack: a velocity-dependent direct path (crest back up).
3. The treble's stereo coherence (correlation 0.43 → 0.56 got worse; bring it to the reference's)
   and the nasal register near middle C.
4. A second search with the metric's brake; then **ship the chosen voicing as the default**:
   `DSP_VERSION` 6 → 7 with the goldens in one commit and the measured size of the change
   (`CANTUS_GOLDEN_DUMP`), the 24 presets re-trimmed to ±2 LU, the full Chorda suite, rt_memory,
   clippy and fmt, ADR 0098's third amendment.
5. **The ABX 4 kit** (`chorda_abx --flac`, a ~21 MB zip that fits an e-mail; each listener types
   their name and downloads a JSON named after them): Chorda 3 (the new default) / Salamander,
   Chorda 3 / Chorda 2 (beta.14), the preference tests, the same phrases as the baseline, with
   *Bass notes alone* among them. Put the zip in `F:\projects\daw-r40\compartir\`.
6. **Show the owner early**: as soon as a candidate voicing exists, render the phrases (FLAC/WAV) to
   `F:\projects\daw-r40\compartir\chorda-preview\` and tell the integrator — the owner's ear decides,
   not the metric. No spectral number replaces his preference.

## `ripple` — insert and delete time, the section drag, the ripple (phase 1)

Six of the eight leaders have it (`docs/scorecard/features.md`, E11). Read BACKLOG §3 (the ripple
and the section drag), ADR 0113 (the arrangement's structure: sections), ADR 0114 (the tools),
`docs/round39/design/` des-arrange's sheets (the drag that moves a section's bars on every track:
Ctrl copies, Shift inserts) and `crates/cantus-core/src/arrange.rs`.

1. **Research first** (write it in your report, with the pages): how FL Studio, Live, Logic, Cubase,
   Pro Tools, REAPER and Studio One do insert time, delete time and ripple edit — the gestures,
   what moves (clips, automation, tempo and metre changes, markers, sections, loop range,
   punch range), how clips across the edge are split, and whether ripple is a mode, per track or
   global. Copy the best behaviour; say where we choose differently and why.
2. **Insert time / delete time over a range**: clips split at the edges, everything after moved
   with its automation, its tempo and metre changes, its markers and its sections; the time
   selection's menu, the palette and keys (keymap 16 → 17).
3. **The section drag moves its bars on every track** (Ctrl copies, Shift inserts), as the
   des-arrange sheets draw it.
4. **A ripple mode** in the arrangement if the leaders do it that way (moving or deleting a clip
   shifts what follows on its lane, or on every lane) — off by default, one clear sign when on.
5. Each operation one `Command` with its exact inverse and its undo-fuzz arm; property tests
   (insert then delete the same range is the identity, to the tick, on random songs); a render
   test (an inserted bar of silence moves the audio exactly by its length, sample-exact); an
   end-to-end test through `CantusApp::assemble`; ADR 0116; the manual's arrangement page.
6. Shots of the gestures (`docs/round40/ripple/`), both lights.

## `memory` — the private bytes outside the live heap (phase 1)

The scorecard's one lost row: 140 MB private on an empty song against REAPER's 62, 143 on *Beat*
against FL Studio's 133 (quiet machine, 2026-11-11, `docs/scorecard/README.md`). The live heap is
about 25 MB. Read ADR 0107, `docs/round39/memory.md`, `docs/round39/memory-measured.md`,
`tools/scorecard/memmap.ps1`.

1. Build the release from master (`cargo build --release -p cantus-app` through the slot) and
   **map** an idle empty song and *Beat* with `memmap.ps1`: every private region by what owns it
   (the heap's reserved/committed segments, each thread's stack, the graphics driver and wgpu's
   buffers, the audio and MIDI drivers' heaps, loaded DLLs' private pages, fonts, textures, the
   content index). A table in your report, with method. Your numbers are maps, not the
   scorecard's — say so.
2. Pick the levers by size and risk (examples to check, not conclusions: the heap's segment
   policy or decommit thresholds, thread stack reservations and commits, how many threads start
   idle, wgpu's staging belts and atlas sizes, a texture or a font kept twice, drivers opened
   before they are needed, the content index's resident size). Each lever: its ADR section (0120,
   and 0107 amended), a test that holds it (`memory_e2e` fails if a start-up grows a tenth), no
   real-time regression (`rt_memory.rs`, the engine's latency tests), the frame budget unchanged.
3. Stop when the next lever costs risk the owner would not take; say what is left and how big.
   **Never say we beat REAPER or FL**: the integrator measures the scorecard on a quiet machine.

## `look` — the owner's answers on the look (phase 1)

Read `docs/round39/integration.md` «The owner's answers» (1, 2, 4, 12, 17, 18), ADR 0110, ADR 0111,
the des-* sheets in `docs/round39/design/`, `docs/round39/lo-mixer.md`, `el-controls.md`,
`lapis2.md`, `themes.md`, `el-topbar.md`. Everything is configurable **with today's look as the
default** unless the answer says the default changes.

1. **The mixer's right dock pinned at the window's edge** (the sheet), the room before it filled
   with the empty numbered strips; a desk that folds keeps it there. The manual gets a word on
   groups for FL users (buses and VCA groups, round 36).
2. **Maximise keeps the inspector beside the area** — and a setting: *the whole workspace* or
   *with the inspector*. Default: with the inspector (the owner's answer).
3. **The top bar configurable**: a setting that brings back the automation arm and its modes and
   the typing-keyboard key; the default is today's bar without them. Mind the fold at 1920 × 1080,
   100 %, in Spanish.
4. **Paper's menus and dialogs improved** (des-controls' sheet); **value bars in the inspector**;
   **Mute on in colour** (the M key of a muted strip lit, not grey) — in every theme.
5. **Slate Lapis's room bluer**: the whole window's surfaces (panels, the arrangement's and the
   mixer's grounds, the wells) visibly bluer and further from plain Slate — a stronger saturation
   gain than × 1.7 Night / × 1.8 Paper; accents, the selection tint, the playhead line and plain
   Slate unchanged. **Shoot two or three strengths side by side, in Night and Paper**, each keeping
   ADR 0111's contrast, into `docs/round40/look/lapis/`, and stop there for the owner to choose
   (tell the integrator; keep building the rest meanwhile).
6. **Bordeaux and Pewter softer**, legibility kept; before/after shots.
7. Shots of every change at 100 and 150 %, Night and Paper (`docs/round40/look/`); the
   legibility test across all twenty-four themes; ADR 0110/0111 amended.

## `dancer` — the dancer for the Slate look (phase 1, design only, no cargo)

The owner (2026-11-12): **the dancer redrawn for the Slate look**. His bar (2026-10-31): the
characters **super professional and fun**, and he sees **animated previews (GIF) before anything is
built**. Read BACKLOG §6i and §6j, ADRs 0087–0091 (the dancer, its rig, the family), ADR 0111 (Slate:
palette, type, the kit's line weights), `crates/cantus-dance/` (what the rig can draw today), and
look at Slate's windows in `docs/manual/img/` and `docs/round39/`.

1. Two or three directions for what a dancer is in Slate (its line, its fill, its accent, how it sits
   in the window), each drawn as a sheet (SVG, rendered with Edge headless — `docs/round39/BUILDING.md`
   «Shared facts» has the command) at the sizes the app shows it, in Night and Paper.
2. For each direction, **animated previews** of two or three moves (frames rendered and joined into
   a GIF; Python with Pillow if present, else find a free tool and record its licence), into
   `docs/round40/design/dancer/`.
3. What the rig would need for each (a note per direction: what `cantus-dance` already draws, what
   it does not), so a builder can follow once the owner chooses.
4. Stop there. The owner chooses.

## `pads` — a pad's own mixer track, Hear the key, Record before/after FX (phase 2)

Read `docs/round39/integration.md` «The owner's answers» (11, 14), BACKLOG §6j, des-views' and the
inspector's sheets in `docs/round39/design/`, `crates/cantus-core/src/pads*.rs` and `padout*.rs`
(look at what already exists), the engine's sidechain and recording paths.

1. **A pad's own mixer track, chosen per pad as in FL Studio**: the rack's well on each pad picks the
   pad's mixer track (none = the instrument's track, today's behaviour, the default). Model change:
   schema 62, migration, `fixtures/v62.cantus`, old fixtures load unchanged; a `Command` with its
   inverse and fuzz arm; the routing in the engine, null when unset (to the bit); a render test that
   a pad sent to its own track sounds there and only there; e2e through `assemble`. ADR 0117.
2. **Hear the key** (sidechain audition): the inspector's key on a unit with a sidechain plays the
   key signal to the monitor instead of the output, as the leaders do; real-time safe, null when off.
3. **Record: Before FX · After FX**: where a track's recording taps its signal; default before FX
   (today's). Render/record tests for both taps; ADR 0118.
4. Both in the inspector through your own file and one call site (`// r40:pads`), the kit only;
   shots; strings in both locales; the manual.

## `delay` — Delay synced to the tempo, and Delay as a CLAP/VST3 plug-in (phase 2)

Read ADR 0115 (Delay's window), `docs/round39/delay.md`, `crates/cantus-dsp/src/delay.rs`, how other
units follow the tempo, `crates/cantus-plugins/` (our effects exported, bit-identical, ADR for
round 17) and `desc.rs` (the ids).

1. **A synced time**: a sync switch and a note value (1/64 … 4 bars, dotted and triplet), the time
   following tempo changes and the tempo track smoothly (no zipper, no click); off by default and
   null at its default (to the bit). Block-size independence at tolerance 0; offline = real time; a
   test through a tempo change. *At the tempo* (sets the time once) becomes the switch, or is kept
   beside it — say which and why.
2. **Delay as a CLAP/VST3 plug-in**: added to the exported set, bit-identical to the built-in unit
   (the existing equivalence tests), its window in the plug-in editor if the others have theirs.
   **A plug-in's ids are forever**: follow `desc.rs`'s scheme, write them in ADR 0119, and put the
   ids in your report for the owner to see before the round merges.
3. Shots of the window with the sync on; the manual; strings in both locales.

## `cast` — the twelve dancers for Slate (phase 2)

The owner approved the whole cast (2026-11-12: "están chidos", then "los doce"): **Compás** (the new
default), **Tono** v2, **Síncopa** (the noir variant), **Bajo**, **Voz**, **Glissando**,
**Eco**, **Vinilo**, **Cinta**, **Metrónomo**, **Espectro** and **Onda** as the oscilloscope (variant O1, chosen after the owner rejected the ribbon: a phosphor trace for a body, a Lissajous head). The designs, one sheet, GIFs and a
builder's note each (`<name>.md`), and the build order are in `docs/round40/design/dancer/`
(README first; merge `r40/dancer` into your branch to have them). Read BACKLOG §6i, ADRs 0087–0091,
`crates/cantus-dance/`.

1. **Build in the designer's waves**, committing and shooting each wave before the next: (1) Compás,
   Tono, Bajo (the first character that is not a skeleton, in Onda's place) — they bring the shared pieces (the theme's colours handed to the canvas, two-bone IK
   with the pelvis dropping rather than a leg overstretching, hit-then-settle timing, a rounded
   rectangle, a per-character extra, a non-skeleton character); (2) Síncopa, Metrónomo, Vinilo, Cinta; (3) Voz, Eco (its copies single-fill silhouettes), Glissando (glow as batched
   layers, never a blur); (4) Onda (the ~3 000-point trace tessellated once per frame into one mesh, its glow as batched widths; benchmark it first) and Espectro (four bands of the master's spectrum computed outside the
   audio thread and handed to the dancer; bars computed directly, no per-frame mask).
2. **Match the designs**: render each character with the app's own renderer (tiny-skia) next to the
   sheet and compare; the owner judges against the GIFs he approved. Put side-by-side GIFs of the
   built characters (app renderer) in `docs/round40/cast/`.
3. **Frame cost**: each character measured against the 1.5 ms budget (the existing dancer budget
   tests), Eco's *Feedback* and Espectro at their worst frame.
4. **Compás becomes the default dancer**: schema 65 with a migration (a saved dancer that names no
   character keeps Fuga, as schema 59 did), `fixtures/v65.cantus`, old fixtures load unchanged. Fuga,
   Volta and the old family stay in the picker. **The dancer stays off at start.**
5. Each character in the picker with its name and moods, both locales; an e2e through
   `CantusApp::assemble` (pick each character, it dances to the transport, saved and reloaded);
   ADR 0122; the manual's dancer page.
