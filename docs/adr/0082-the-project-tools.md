# ADR 0082 — The project's tools: one table of chunky, typed tools, the unit catalogue, the music linter

**Status:** accepted (round 34) · **Date:** 2026-10-30 · **Extends:** ADR 0004 (commands),
ADR 0081 (the MCP server), ADR 0018/0023 (parameter tables), ADR 0027 (automation), ADR
0057 (modulators), ADR 0071 (starting presets), ADR 0076 (the session) ·
`research/ai/mcp-server.md` §3 and §5, BACKLOG §7 ("What an AI needs…"), `docs/round28/demo.md`

## Context

ADR 0081 gave Cantus an MCP server whose pump runs a tool table on the UI thread and turns
each planned `Command` into one undo step with `Origin::Mcp`. What an agent can *do* and
*know* was still the legacy bridge's eight thin tools. Round 28's demo builder, who composed
and mixed a song in code exactly as an agent would, listed what was missing: a catalogue of
every unit generated from the parameter tables (it had read Rust to learn that `b1.shape = 0`
is a bell), the model's limits up front, chunky musical tools (`write_part`, `voice_chords`,
`pedal_from_chart`, `automate`), and a music linter — its Fm7 held under E♭/G was found only
by a reviewer.

## Decision

1. **Few, chunky, typed tools in one table.** `cantus_ai::tools_project::TOOLS` holds 36
   tools that need only the project (read, the song's frame, tracks and instruments, clips and
   notes, drums, the session, the mixer and effects, automation and modulators, the linter,
   the writers, removal); `cantus_ui::ai_tools::project_app::TOOLS` holds the 10 that need the
   application (transport, the selection, the unit catalogue and presets — their names and
   factory lists live with the editors —, comping, export). 47 in all, joined in
   `ai_tools::registry()` right after each other. The legacy `bridge.rs` is now the
   assistant panel's door to the same table (`tool_specs`, `to_command` over a bare bus):
   `describe_project` became `get_project`, `set_track_gain` `set_track`, `set_synth_param`
   `set_instrument_param`; `set_tempo`, `set_tempo_point`, `add_track`, `add_notes` and
   `add_effect` keep their names and accept their old arguments.
2. **Conventions, kept forever.** Ids are numbers and every create answers the ids it made.
   A position is beats (quarter notes) from the song's start *or* `"bar:beat"` text (bar and
   beat from 1, the beat the metre's own, through every metre change); answers give both. A
   length is beats or `"N bars"`/`"N beats"`. Values are in the parameter's own unit; a value
   out of range is **refused with the range**, never clamped, so the model learns it. Unknown
   arguments are refused (`additionalProperties: false`). Limits are stated in
   `get_project`/`get_mixer` (`limits`) and in the schemas: 4096 notes and 2048 automation
   points a call, the command ranges (−96…+12 dB, 10 inserts, 16 sends, 16 modulators a
   host).
3. **One tool, one `Command`, one undo step** (a `Batch` labelled `cmd.ai.<tool>` when it
   needs several; no new `Command` was added). Classes follow ADR 0081's rule: a planned edit
   is Edit or Destructive; **removing or replacing the user's material is Destructive**
   (`remove_items` — every removal in one tool —, `replace_notes`, `replace_effect`,
   `set_instrument`, `apply_preset`, `replace_automation`, `tidy_takes`); an Edit tool never
   takes the user's material silently: `automate` and `pedal_from_chart` refuse a span that
   already holds points, the writers make a clip of their own (refused where the track has
   notes, unless `clip_id` names one), notes never land where nothing sounds; export is Files (`export_audio` runs the dialog's export
   and is followed through `get_selection`; `export_project` writes MIDI or DAWproject as a
   job, opening its file on the worker through `open_touch` — new unless `overwrite`); transport is Transport and refused while Cantus records. Every sentence (`Said`) **names its target** in the user's
   words — `add 32 notes to “Bass” from 9:1`, `remove clip “Verse” on “Piano” (bars 1–4)` —
   names quoted as data. File tools open the path `cantus_ai::resolve_touch` gives (the
   project's folder or a folder the user allowed), never overwrite unless asked.
4. **The catalogue is generated** (`list_units`, `get_unit`, `cantus://units`,
   `cantus://units/{id}`) from `EffectKind::params()`, `InstrumentKind::params()`, Firmus's and
   Pulsus's fields: key, English name (the editors' locale), group (`Band 4`), unit, range,
   default, taper, a stepped parameter's choices by number and name
   (`controllers_words::effect_choice_words`, `inst_choices::words` — the same tables the
   exported plug-ins and the controllers pane use), factory and user presets and what they
   set, the starting preset or why there is none (ADR 0071), latency at 48 kHz from the
   engine's own unit, sidechain and note inputs, and what each unit is for. Exhaustive
   `match`es make a new kind fail to compile until it is named and described; a test fails
   when a kind or a parameter is missing or has no English name.
5. **The music linter is ours, in `cantus-core` (MPL)**: `cantus_core::harmony` — chord
   symbols (the lead sheet's grammar: `Fm9`, `Dbmaj7`, `Eb/G`, `C7(b9,#11)`, `Gø7`, `C6/9`,
   `C7alt`…, each tone tagged third/seventh/tension/root/fifth), a chart in bars (`Fm9 |
   Dbmaj7 | Eb Eb/G`, `.` holds, `%` repeats, `N.C.`) or explicit spans, and `lint`: notes
   ringing into a change whose chord does not hold them (an error when a semitone against
   it), notes held under a chord a semitone from its tones, the same pitch written twice at
   once, notes outside the key and the chord, notes an instrument cannot play (a kit's
   missing pads, Chorda's A0–C8). A clash is **a semitone above** a chord tone (the avoid
   note: the 11th over a major third, the ♭6 over the fifth, the ♭9 over a non-dominant's
   root); a 9th, a 13th, a major 7th or a ♯11 is a colour. Tolerances: a thirty-second of
   legato overlap, an eighth for a clash. Each chord is prepared once and a note checks only
   the chords it overlaps (24 000 notes over 1 600 chords in under 40 ms).
6. **The writers** — `write_part` (block, pulse, arpeggio, bass; a rate or a step rhythm;
   swing; accents; a seeded humanisation), `voice_chords`, `pedal_from_chart` — voice each
   chord by an exhaustive search over octave placements that keeps the third and the seventh
   first and moves least from the chord before (with low-interval and spacing penalties,
   narrowed to where the music is when the search would be large), and **never let a note
   ring into the next chord** (common tones may tie), round 28's fix as a property of the
   writer. A pitch never doubles itself after humanisation. A bass line's fifth is the
   chord's own; a step rhythm runs on from the chart's start across bar lines.
7. **Resources and prompts**: `cantus://project`, `cantus://mixer`, `cantus://history`,
   `cantus://tracks/{id}`, `cantus://clips/{id}/notes`, `cantus://units`,
   `cantus://units/{id}`; prompts *Explain my mix*, *Write a part*, *Arrange an intro*, *Tidy
   the project*, *Teach me this unit* in both languages (listen adds *Master for streaming*).
8. **Notes keep a canonical order** (start, pitch, id) in `InsertNotes` and `SetNotes`, so
   an undo restores two notes at one place in the order they had (the tools' fuzz found the
   tie that `(start, pitch)` left open).

## Consequences

- An agent reads one table, with descriptions and schemas written for a model: units in
  names, limits stated, every error an `isError` it can correct itself from.
- Every sentence is worded whole in the interface's language: the phrases inside it (a clip
  and its bars, a strip, a parameter, a list) are `Said`s of their own (`tools_words.rs`,
  keys `ai.phrase.*`/`ai.field.*`), the user's names quoted by the host, one line each.
- The catalogue costs nothing to keep: a parameter added to a table appears in it.

## What the tests hold

`crates/cantus-core/src/harmony_tests.rs` (chord symbols, charts, the demo's Fm7 under E♭/G,
clashes, doubles, scale and range, voicings, parts that never ring into the next chord);
`crates/cantus-ai/tests/tools.rs` and `tools_music.rs` (every tool through a bare host: one
undo step that gives the project back exactly and a redo that gives the edit again, the
errors, the classes, a song composed only through tools and undone to nothing, resources
and prompts); `tools_fuzz.rs` (64 seeds × 80 random calls, every step exact, all undone and
redone); `crates/cantus-ui/tests/agent_tools.rs` (through the MCP server with the
application as host: the catalogue complete, every sentence in both languages, a song
composed, linted clean, rendered offline and taken back by the user's Undo, comping, export
refused without leave and outside the allowed folders).
