# Round 39 — the builder briefs (`ux-a`, `ux-b`, `memory`)

Read `docs/round39/BUILDING.md`, `CLAUDE.md` and, for the UX builders, **BACKLOG §6j** first.
The look itself (palette, type, buttons, icons) is being designed by the design agents and
changes later through the kit; **you build behaviour and structure** with today's kit tokens,
so the new look can be dropped in. Never add a literal colour or a stock widget.

## `ux-a` — the automated control says so, the help line, nothing truncated (ADR 0109)

1. **An automated control says so.** In round 38 the owner thought some of Chorda's knobs
   were broken; automation clips were overriding them. When a parameter is driven by an
   automation clip, a modulator, MIDI learn/a controller or a VCA, every control that shows it
   (knob, fader, slider, inspector row, effect and instrument editors) shows **a sign** (a
   kit-level mark, drawn with today's tokens), moves with the value it is driven to, and its
   hover hint says what drives it and how to take it back (e.g. *"Automated by the clip
   'Cutoff' on Lead — right-click ▸ Clear automation, or turn the knob to write over it"*, as
   the app's real behaviour is — find it, do not invent it). Grabbing a driven knob does what
   the app does today, plus a clear hint. One end-to-end test through `CantusApp::assemble`:
   Chorda with an automation clip on *Hardness* → its knob shows the sign and the hint; the
   clip removed → the sign goes.
2. **The help line is never empty.** Every control a pointer can rest on gives the help line
   (`kit::hint`, the status line) a sentence. A test walks every view (main window, mixer,
   inspector, arrangement, piano roll, channel rack, browser, every instrument and effect
   editor, Settings, the first-run setup, the export dialog, menus) through `assemble`, hovers
   each interactive rect and fails on any without a hint; fill every gap in both locales.
3. **Nothing truncated.** A test renders every view at 100 % and 150 %, both themes, in both
   languages (Spanish is longer), and fails on any kit text that was elided or clipped (find
   where the kit elides — `kit/text.rs`, `kit/field.rs` — and count it); fix each by layout
   or wording, not by shrinking type. Report what you cannot fix without the new design.

## `ux-b` — panels, keyboard, click paths (ADR 0110)

1. **Click paths.** List the paths the owner uses daily (add an instrument, add an effect to
   a strip, open the piano roll on a clip, mute/solo, set the loop, record-arm and record,
   route a send, open an insert's editor, change the tempo, save, export, find a preset in the
   browser and audition it, open Settings ▸ Audio). Drive each through `assemble` and **count
   clicks, keys and pointer travel**; write the table in `docs/round39/ux-b.md`; shorten each
   that is longer than FL's or Ableton's way (cite how they do it), with a test per path that
   holds its new count.
2. **Every panel resizable and movable.** Survey what exists (dock, splitters, the inspector,
   the browser, the mixer, floating editors); close the gaps: a panel that cannot be resized,
   moved, collapsed or restored; layout kept between sessions (in settings, unless a layout
   truly belongs to the song — then schema 60, migration and fixture); every overflowing area
   scrollable. Tests through `assemble`.
3. **Keyboard reach.** Every action in the menus and context menus reachable by a key or the
   command palette; focus visible and moving in a sensible order; Esc closes what opened last.
   A test that enumerates the actions and fails on one without a path.

## `memory` — the engine's memory grows with the song (ADR 0107)

The scorecard's one lost row: Cantus holds 237 MB on an empty song against REAPER's 62 and FL
Studio's 133. `docs/round38/scorecard.md` §4 and `crates/cantus-ui/tests/memory_e2e.rs` map
it: the processor before any song holds **96 MB**, ≈ 60 MB of which an empty song never
touches. Cut it **without breaking the audio thread's contract** (it never allocates, locks,
blocks or does I/O — everything a snapshot may need is built off the audio thread):

1. **Pulsus kits built as the native instruments are** (−31 MB): today every one of 256 track
   slots holds a Pulsus drum machine (120 KB) whether the track is a drum track or not. Build
   the kit's DSP on the UI/worker side and send it in, as the other native instruments
   already are; a slot holds nothing until a kit is assigned.
2. **The meters out of the event queue** (−15 MB): the queue holds 4 096 events of 3 880 B
   because an event is as large as its largest kind (the block's meters). Give meters a
   mailbox of their own (a triple buffer or a seqlock of fixed size), so the queue's events
   are small.
3. **A delay-compensation pool that grows from small** (−9 MB): 192 lines of 2 × 32 KB
   preallocated; size it from what the project needs, grown off the audio thread before a
   snapshot that needs more is sent.
4. Anything else `memory_e2e`'s map shows as capacity an empty song never touches (the pass
   buffers for 256 tracks and strips, 17 MB), if it can follow the same rule.

**Tests first:** the null and twin tests stay bit-identical (`resting_changes_no_sample_of_the_demo`,
the golden hashes, offline = real time, parallel = serial, block-size independence);
`rt_memory.rs` proves no allocation on the audio thread on every new path (assigning a kit,
growing the pool, the meters' mailbox); the undo fuzz and the engine fuzz
(`CANTUS_WORKERS=3 cargo test -p cantus-engine --features fuzz`) pass; **lower
`memory_e2e`'s bounds to what you reach** and report the numbers per step. Do not run the
scorecard's start-up measurement (the integrator does, on a quiet machine).
