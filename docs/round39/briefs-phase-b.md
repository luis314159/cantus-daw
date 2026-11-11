# Round 39 — Phase B briefs (the owner chose Slate, 2026-11-09)

**The owner's decisions (2026-11-09):**
- **Direction A, Slate, is the design system and the default theme.**
  `F:\projects\daw-r39\dir-a\docs\round39\design\dir-a\` holds its README, `system.png`, the
  windows, the parts sheets and the `src/` generator.
- **B (Graphite and gold) and C (Studio ink) become optional themes**, each in Night and Paper
  ("los demás modos se ven bien también, me gustaría tenerlos como opciones").
  Their systems are in `F:\projects\daw-r39\dir-b\…\dir-b\` and `F:\projects\daw-r39\dir-c\…\dir-c\`.
- **The icons' first set was not approved** ("más elegantes, bonitos y mejor pensados"). The
  `icons` agent is redrawing it against FL, Ableton, Bitwig, Logic and Studio One. Kit code uses
  today's glyphs until the owner approves the new ones.

Every rule of `BUILDING.md` and `briefs-design.md` holds: drawn by hand from a blank sheet for each
size and place, 1:1 and 4×, Night and Paper, 100 % and 150 %, beside FL at the same scale
(private, `research/ui/round39/<name>/`, `git add -f`), and every PNG read before it is shown.
The references for other DAWs are in `F:\projects\cantus\research\ui\screenshots\{ableton-live,
bitwig-studio,logic-studio-one,fl-studio}\`. **The owner approves each zoomed sheet.**

## Design agents (no cargo), in Slate

### `des-mixer`
**The mixer and its inspector, whole.**
- **Strips:** master, a track strip and a return, in Compact, Wide and Large.
- **States:** at rest, playing, clipping, armed, muted, soloed, selected, an insert bypassed,
  a send, sidechain and routing shown, a VCA, frozen.
- **The inspector:** every section — inserts, EQ, sends, routing, recording, automation,
  sidechain, level.
- **Empty strips:** how they look, and every overflowing case (13+ strips, long names, Spanish).
- **Deliverable:** the sheet, with the measured spec (sizes, spacing, type roles, tokens) the kit
  builder will turn into code.

### `des-controls`
**Every control family at every size.** Knobs (small, medium, large; bipolar; stepped; rest,
hover, dragged, automated, modulated, disabled), faders and their meters (silent, playing,
peak hold, clip), sliders, numeric fields (editing, invalid), drop-downs, menus with
submenus, check marks and shortcuts, context menus, tabs, segmented controls, toggles,
scrollbars, tooltips and the help line, the dialog frame and its buttons (primary,
secondary, destructive). Each with its states and the measured spec.

### `des-views`
**The other main views, whole.**
- **The piano roll:** the keys are the owner's open item since round 37 — black and white keys
  that read as a piano, the C labels, the note grid, velocity, notes selected, ghost notes.
- **The channel rack.**
- **The browser:** the tree, list rows, previews, the pack covers, search.
- **The arrangement's clip content in detail:** MIDI, audio and automation clips at every zoom.
- Night and Paper, 1× and 1.5×.

### `dir-a` (resumed)
**The top bar, the toolbar and the overflow panel as build sheets.** Every group, each at every
width it folds through, with the measured spec. Then Settings, the first-run audio setup and the
export dialog, whole.

## Builders (cargo through `slot.sh`)

### `kit` — the kit becomes Slate (ADR 0108 how it draws; ADR 0111 the design system)

1. **ADR 0108 first.** Decide how the kit draws the approved sheets: vector operations of the
   kit's own, or SVG rasterised per scale and cached. Measure the frame budget of each on the
   real workspace (`tests/frame_budget.rs`, `workspace_budget.rs`). **The new look may not cost
   frames.**
2. **The tokens.** Slate Night and Slate Paper from `dir-a`'s system (surfaces, inks, lines,
   accent, state colours, radii, spacing), as the default theme family.
   - Graphite and gold (`dir-b`) and Studio ink (`dir-c`) as optional themes, each Night and
     Paper, in Settings ▸ Appearance.
   - Old settings that name a theme map to Slate.
   - Each theme's contrast is tested.
3. **The type.** Bundle the OFL families: Barlow Semi Condensed for Slate, Sofia Sans for
   Graphite, IBM Plex Sans Condensed and Mono for Studio ink. They are in
   `F:\projects\daw-r39\fonts\`, with their licences.
   - Add each with its notice (`cargo run -p xtask -- notices`).
   - Give each its own text roles, including the large thin clock.
   - Keep the frame budget and the installer size in mind; report the sizes.
4. **The surfaces and keys** from the approved parts: dark flat wells, no gloss, the panels
   separated by value, idle states without "−∞". Every view inherits them from the kit:
   - main window, mixer and inspector, arrangement, piano roll, rack, browser;
   - every instrument and effect editor, dialogs, Settings, the first-run setup;
   - menus and context menus.
   - Both themes, 100 % and 150 %.
5. **Element by element**, as the design agents' sheets are approved: the integrator forwards
   each approval.
6. **Tests:**
   - the theme and contrast tests;
   - the kit lint stays (no literal colours outside the tokens);
   - the frame budget;
   - `shoot` renders of every view in each theme for the owner;
   - an end-to-end test that boots through `assemble` in Slate and switches themes.

`ux-a` and `ux-b` build behaviour in the views at the same time. Keep your edits to the kit and
the theme plumbing; where a view must change, keep the change small and say so.

### `conv` — the convolution reverb (ADR 0112, schema 62)

The owner asked for it (2026-11-09). 7 of the 8 market leaders have one.

**Name.** A new effect, **Aula** (Latin for the hall), with its own `EffectKind` and its
`fx/aula.rs` beside Spatium.

**Real time:** uniformly or non-uniformly partitioned FFT convolution, zero added latency if it
can be done exactly (a direct-form head); otherwise the latency is declared to delay
compensation.
- The audio thread never allocates.
- Impulse responses are loaded and partitioned off the audio thread and swapped in whole.
- `cantus_dsp::math` only; a deterministic FFT.

**Exactness:**
- **Null tests:** dry/wet at 0 % wet touches no sample, to the bit.
- **Block-size independence at tolerance 0**, offline equal to real time, parallel equal to serial
  — design the partitioning for it.
- **Measured against a reference:** a direct (time-domain) convolution of the same impulse
  response within a stated tolerance, and an outside impulse response file (a published one)
  read correctly — WAV of 16, 24 and 32-bit float, mono, stereo and true-stereo (4 channel).

**Impulse responses:**
- The user's own files (drag in a WAV, or the browser).
- A set made by Cantus itself: rooms, halls, plates, springs and chambers, generated
  deterministically.
- Free, CC0 or CC BY impulse responses offered as an on-demand pack (ADR 0030 and 0042: credit
  shown, never in the repository).

**Controls:**
- pre-delay, length (stretch or trim), the envelope (attack, decay, a gate);
- EQ (low and high cut, damping), width, reverse, mix and level;
- a waveform view of the impulse response.

**Integration (no leftovers):**
- `Command`s with undo-fuzz arms; schema 62 with migration and fixture;
- the editor drawn with the kit (it will follow Slate);
- starting presets and factory presets, auditioned like the library's;
- the MCP tools and the unit catalogue;
- the manual EN/ES, the website's unit card and sounds;
- CLAP/VST3 export, if the effects build includes every effect (ADR 0020: bit-identical);
- `rt_memory`, the CPU budget, freeze and stems.

**Ownership.**
- **You own:** `crates/cantus-dsp/src/aula*.rs`, `crates/cantus-core/src/fx/aula.rs`,
  `crates/cantus-engine/src/fx_aula*.rs`, `crates/cantus-ui/src/views/fx/aula*.rs`, and the
  locale area `areas/aula.{en,es}.txt`.
- **Shared files** (the effect kind enum, `crates/cantus-engine/src/effects.rs`'s factory, the
  editors' match, presets): only additive lines marked `// r39:conv`. `memory` owns the rest of
  the engine.
- Finish with `docs/round39/conv.md`.
