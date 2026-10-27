# Round 31 — the daily gaps an FL or Ableton user hits first

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians
3:23). Tests before code, nothing half done, look at what you build (`shoot` examples).
**The owner's rule:** every gap a feature leaves is closed in the same round. Whatever you
would write under "Left" comes back to you as work; only list as open what truly needs the
owner (their ears, their hardware, a paid account, a decision).

**Run lean** (as round 30): read only what your brief needs, prefer `grep` and line ranges
to whole files, keep reports short and exact.

The ask (2026-10-25, `docs/next-session.md`, BACKLOG §0.4 item 3):

0. What round 30 left: the real cause (not a longer wait) of three tests that fail now and
   then on GitHub's runners — `session_faults5::an_instance_removed_during_a_dialog_is_ended_after_it`
   (Windows: 2 instances destroyed under the dialog in the beta.5 release run),
   `session_faults5::a_state_answered_late_becomes_the_last_good_one` and
   `bridge_faults4::a_pause_while_a_job_is_late_ends_nothing` (macOS VM, passed on a
   retry). The Mac session has not happened yet: beta.6 stays Windows + Linux.
1. **Automation**: write / touch / latch, and the piano roll's lanes for CC, pitch bend,
   velocity and release.
2. **MIDI**: arpeggiator, chords and scale as note effects, scale highlighting and snap,
   groove templates.
3. **Editing**: crossfades between overlapping clips, clip gain envelopes.
4. **Workflow**: a command palette, an undo history panel, project and track templates.
5. **Effects that sound on insert** (BACKLOG §3b, option B, decided 2026-10-16).
6. The owner's frictions and the beta's users: none given at the start, no open issues on
   `cantus-daw`. The integrator hands out whatever arrives during the round.

Read, in order: this file (the rules and **your** section); `CLAUDE.md`; `docs/STATUS.md`'s
header; `docs/BACKLOG-excellence.md` §3 (and §3b for editing); the ADRs your brief names.

## The builders

All start from the contract commit on `r31/integration`. **Never more than three compile
at once** (a 31 GB machine): automation, midi and workflow start now; editing starts when
the first of them has delivered and been merged.

| Builder | Branch / worktree | Delivers | ADR | Schema | Keymap |
|---|---|---|---|---|---|
| **automation** | `r31/automation`, `F:\projects\daw-r31\automation` | item 1 | 0068 | 42 | 12 |
| **midi** | `r31/midi`, `F:\projects\daw-r31\midi` | item 2 | 0069 | 43 | 13 |
| **workflow** | `r31/workflow`, `F:\projects\daw-r31\workflow` | item 0, then item 4 | 0072 | 45 | 11 |
| **editing** | `r31/editing`, `F:\projects\daw-r31\editing` | items 3 and 5 | 0070, 0071 | 44 | 14 |

A schema or keymap number is yours only if you need it (a model change is a schema bump:
`SCHEMA_VERSION`, a migration, `cargo run -p cantus-project --example write_fixture` for
`fixtures/vN.cantus`; a new default binding is a keymap bump with its migration). Say in
your report which you took; the integrator renumbers them in merge order. The empty locale
areas are already declared: `automodes` (automation), `notefx` (midi), `clipfx` (editing),
`workflow` (workflow), in `crates/cantus-ui/locales/areas/` and `i18n.rs`.

### The seams between builders (fixed by this contract)

- **The note path.** midi owns the track's note entry points in
  `crates/cantus-engine/src/graph/track.rs` (`note_on`, `note_on_seq`, `note_off`,
  `controller`) and puts the note-effect chain there, in a new
  `crates/cantus-engine/src/notefx/` (declared from its own file with `#[path]` or one
  `mod` line). automation adds the **release velocity** to `note_off` (one argument, every
  caller) and delivers the clips' CC and pitch-bend lanes through `controller` (natives)
  and the hosted plug-ins' event lists. Note effects pass controllers through untouched and
  keep each note's release with the note they produce. Both of you: write the change so a
  3-way merge is mechanical, and name the lines in your report.
- **The model.** Additive fields only, each with `#[serde(default)]` that reads every old
  file as before: automation on `Note` (release) and `Clip` (controller lanes); midi on
  the track/channel (its note-effect chain), the project (key and scale) and `Clip` (a
  groove); editing on `AudioRegion` (the gain envelope, crossfades). `model.rs`/`audio.rs`
  conflicts are additive; the integrator resolves them.
- **The piano roll.** automation owns `views/piano_roll.rs` and `piano_roll_paint.rs`
  (the lanes); midi draws scale rows and snaps in its own files (`piano_roll_scale.rs`,
  …) called from at most a few marked lines in those two.
- **The arrangement.** editing owns `views/arrangement_audio*.rs` (crossfades and the gain
  line on audio clips); automation owns `views/arrangement_auto.rs`, `auto_curve.rs`,
  `auto_editor.rs`, `automation.rs`, `record_pass.rs`.
- **Commands.** Everyone adds `Command` variants with exact inverses, arms in
  `crates/cantus-core/tests/undo_invariants.rs` and `cmd.*` strings — additive; the
  integrator resolves the conflicts in `command.rs`. A new `Action` (keymap) needs its
  label in both locales: workflow's palette lists every `Action`, so a new one appears in
  it by itself — check that yours does.
- **Shared hot files** (`app.rs`, `workspace*.rs`, `keymap.rs`, `processor*.rs`,
  `cantus-project`'s format and migrations): additive lines only, named in your report.

## Rules (the usual ones, short)

- Work only in your worktree. Commit WIP often (`r31/<name>`); a cut session resumes.
  Write files in pieces under ~300 lines per tool call.
- Machine: `export PATH="$HOME/.cargo/bin:$PATH"` (cargo is not on PATH); your untracked
  `.cargo/config.toml` sets `jobs = 3`, no debug info and incremental off (never `git add`
  it); `TMP`/`TEMP` = `F:\projects\daw-r31\tmp`. Test executables named `*setup*`/
  `*update*` need elevation on this machine: run them copied to a neutral name.
- Windows: `cargo clippy --workspace --all-targets` and the tests of every crate you touch,
  with `RUSTFLAGS=-D warnings`; `cargo fmt --all` before each commit. Linux, before you
  report: `wsl -d Ubuntu-24.04 -- bash -lc 'CANTUS_CI_DIR=$HOME/ci-r31-<name> bash
  /mnt/f/projects/daw-r31/<name>/tools/wsl/check.sh r31/<name> clippy'` (and `test` for
  the crates you touched) — your own clone, never `~/cantus-ci`. Code behind
  `#[cfg(windows)]` leaves Linux-only dead-code warnings.
- Real time (ADR 0005): no allocation, lock, block or I/O on the audio thread
  (`rt_memory.rs` counts it); `cantus-engine` stays `forbid(unsafe_code)`; DSP calls
  `cantus_dsp::math::*`.
- Engine work starts with null tests: a feature at its default touches no sample, to the
  bit; block-size independence at tolerance 0; offline render equals real time; parallel
  equals serial (`CANTUS_WORKERS`); freeze, bounce, stems and export hear what playback
  hears. Golden hashes change only on purpose (a `DSP_VERSION` bump and the measured size).
- **Start-up wiring goes in `boot`'s stages / `CantusApp::assemble`**, never only in
  `CantusApp::new` (the shipped app never calls it), with a test that fails if it leaves
  that path. Reviewers ask "is this reachable from `cantus_ui::run`?".
- UI: the hand-drawn kit (ADR 0024) — no stock egui widgets in views, no literal colours,
  no `FontId::new`. FL Studio is the visual reference; clear label/value units, nothing
  that overlaps, every overflowing area scrollable. **Look at it:** shoot PNGs
  (`cargo run -p cantus-ui --example shoot -- <dir> …`) of every new surface in both
  themes, and put the best ones in `docs/round31/`.
- Strings in both locales (your area file); the manual (`docs/manual/`, EN and ES) for
  every feature a user touches; your ADR; `docs/round31/<name>.md`: what was done, how it
  was tested, the numbers, and **only** what needs the owner.
- Never touch `master`, never push, never stage files outside your worktree.

## automation — item 1 (ADR 0068)

Read ADR 0027 (automation clips and lanes), 0057 (modulators; *Bake to automation*),
`crates/cantus-core/src/automation/`, `crates/cantus-ui/src/{automation,record_pass}.rs`,
`views/{arrangement_auto,auto_curve,auto_editor,piano_roll,piano_roll_paint}.rs`, and how
the engine plays a clip's notes into `graph/track.rs`.

- **Write / touch / latch** (Pro Tools, Logic, Cubase, Studio One are the reference;
  FL's *record automation* and Ableton's *automation arm* the simple case): each track's
  automation mode (Read — plays it, the default; Write — overwrites every armed target
  while the transport rolls; Touch — writes while a control is held, then glides back to
  the curve already there; Latch — writes from the first touch until Stop, holding the
  last value), a global arm and *set all tracks to…* in the top bar, the mode on the track
  header and in the inspector. It covers every automatable target: mixer level, pan,
  sends, effect and instrument parameters, hosted plug-ins' parameters (their gestures),
  modulator depths. A pass is one undo step; recorded curves are thinned (a tolerance in
  dB / % that the test states); loop recording and punch ranges behave as the ADR says
  (the last pass wins inside the loop); the touch glide time is a setting. What exists
  (round 28: every control moved while a pass records becomes an automation clip) becomes
  the Latch case, not a second path.
- **The piano roll's lanes** (FL's event editor and Ableton's clip envelopes): under the
  notes, beside the velocity lane that exists, lanes for **release velocity** (a new
  `Note` field; `note_off` carries it to native instruments, CLAP and VST3), **pitch bend**
  (a smooth curve, the instrument's range shown), **any CC 0–127** (named: mod wheel,
  breath, expression, sustain, …) and channel pressure — added and removed per clip, drawn
  with the pencil, line and curve tools, selected, moved, copied and pasted with the notes,
  scaled by the clip's stretch and loops, sample-accurate in playback (natives through
  `controller`, hosted plug-ins through their event lists, our exported plug-ins too).
- **No leftovers:** live MIDI recorded into a clip keeps its CC, pitch bend, pressure and
  release velocity in those lanes; MIDI file import and export carry them; freeze, bounce,
  stems and export render them; the channel rack's step sequencer and Pulsus see them as
  they should (say what they do); `Clip` duplication, split, glue and the clipboard keep
  them.
- Null tests first: a clip with no lanes plays as before to the bit; Read mode with no
  gesture changes no automation; a written curve played back reproduces the gesture within
  the stated tolerance; touch returns to the old curve; latch holds.
- ADR 0068, the manual (automation modes; the piano roll's lanes), `automodes` strings,
  shots of the lanes and the modes, `docs/round31/automation.md`.

## midi — item 2 (ADR 0069)

Read ADR 0022 (notes into effects), the channel rack and its channel settings
(`views/channel_rack*.rs`, `views/inspector_channel.rs`), `crates/cantus-engine/src/graph/
{track,notes}.rs` and `midi.rs` (live input), and how a pattern/clip schedules its notes.

- **Note effects** on a track/channel, before its instrument, in a chain (FL's channel
  settings *Arp* and *Chord*, Ableton's MIDI effects, Bitwig's note FX are the reference):
  an **arpeggiator** (up, down, up-down, down-up, played order, chord, random with a seed;
  1–4 octaves; rate synced to the tempo with dotted and triplet values; gate; swing; hold
  /latch; velocity from the played note or fixed), **chords** (major, minor, 7ths, sus,
  add9, power, … with inversion and spread; or *from the scale*: diatonic chords of the
  project's key), **scale** (constrain to the key: snap to the nearest note, or drop notes
  outside it). They act on clip notes and on live input alike, sample-accurately, with
  deterministic randomness, offline equal to real time; freeze, bounce and export hear
  them; a chain at its default (empty) touches nothing. *Bake to notes* writes a clip's
  result into a new clip (one undo step).
- **Key and scale**: the project's key and scale (and a clip's own, if the ADR says so);
  the piano roll highlights the scale's rows (and the root), an option folds to the scale,
  and drawing, moving and transposing snap to it when *Snap to scale* is on; the top bar or
  the piano roll shows the key. FL's piano roll *scale highlighting* and Ableton 12's
  *scale awareness* are the reference.
- **Groove templates**: a groove pool — built-in swings (8ths and 16ths, MPC-style 50–75 %)
  and grooves extracted from a MIDI clip (and from an audio clip's transients, if the
  existing transient detection makes it clean); applied to a clip non-destructively with
  amounts for timing, velocity and length; *Commit* writes it into the notes. Patterns
  (the channel rack's step sequencer) take a groove too.
- **No leftovers:** MIDI file export of a clip with note effects or a groove says what it
  writes (the played notes or the source — the ADR decides and the export dialog says);
  the clipboard, duplicate and project templates keep the chain; every parameter of a note
  effect is automatable and modulatable like any other.
- ADR 0069, the manual (note effects, scales, grooves), `notefx` strings, shots,
  `docs/round31/midi.md`.

## workflow — item 0 first, then item 4 (ADR 0072)

**Item 0 first.** Read ADR 0063 (plug-ins in their own process) and the three tests
(`crates/cantus-sandbox/tests/session_faults5.rs`, `bridge_faults4.rs`). Find what really
happens on a slow runner — instrument the code (temporary logs, `why_changed`, timings),
reproduce under load (`CANTUS_WORKERS`, a parallel CPU hog, several runs), and fix the
cause: in the sandbox if it is a real race a user could meet, in the test if the test
assumes an order the code does not promise. No longer waits. Prove each fix by a run that
failed before (e.g. 50 loaded runs). The macOS ones are reproduced on Windows/Linux under
the same timing if you can; otherwise the reasoning goes in the report and the integrator
watches the runner. Commit this before item 4 (the integrator may merge it alone).

**Item 4.**

- **A command palette** (VS Code's, Bitwig's *Commander*, Ableton 12's search are the
  reference): one shortcut (choose one free on Windows, Linux and the macOS preset; keymap
  11), a search box that finds every `Action` (with its shortcut shown), every view and
  Settings page, every effect and instrument to insert on the selected strip/track, recent
  projects and templates — in the interface's language and in English, fuzzy, ranked by
  use; Enter runs it, the arrows move, Esc closes; drawn with the kit. A test fails if an
  `Action` has no palette entry or no label.
- **An undo history panel** (Photoshop's and Studio One's history): every step by its name
  (the commands' `cmd.*` labels, with what they touched where it fits: "Move 3 notes",
  "Insert Concentus on Lead"), the current position marked, a click undoes or redoes to
  that step (one undo per step, or one move — the ADR says), steps since the last save
  marked, scrollable, dockable like the other panels.
- **Templates:** *Save as template* and *New from template* for projects (the start page,
  the File menu, the palette), with a few bundled starters made from our own instruments
  and effects (an empty song, a beat, a band recording, mastering), and **track
  templates** — save a track or channel with its instrument, inserts, sends, colour and
  routing, insert it from the browser/picker and the palette; one undo step; the user's
  templates in the data folder, listed and deleted from the UI.
- ADR 0072, the manual (palette, history, templates), `workflow` strings, shots,
  `docs/round31/workflow.md`.

## editing — items 3 and 5 (ADRs 0070, 0071)

Read ADR 0025 (audio clips), 0058 (comping seams: equal-power seam crossfades already
rendered in the engine), 0018 (the plug-in contract and the null rule), 0055 (the
modulation family), `crates/cantus-core/src/audio.rs` (`AudioRegion`: `gain_db`, fades,
`xfade`), the engine's audio-clip playback, `views/arrangement_audio*.rs`.

- **Crossfades between overlapping clips** (Studio One, Cubase, Reaper are the
  reference): two audio clips that overlap on a track's playing lane crossfade over the
  overlap, created when an edit makes the overlap (a setting can turn automatic
  crossfades off), with curve (equal power, linear, S) and length editable on the clip
  (drag the overlap's handles; double-click for its editor), drawn as Studio One draws
  them; one engine path with comp seams, not two. Butt joins stay butt joins.
- **Clip gain envelopes**: a gain line on each audio clip (shown with a toggle, points
  added, moved and deleted, curves between them like automation's), in the clip's content
  time so slip edits carry it (the ADR says what trim, split, glue, reverse and warp do),
  sample-accurate in the engine, smoothed; on top of `gain_db` and the fades. MIDI clips
  are out (velocity covers them; say so). DAWproject export/import maps fades, crossfades
  and the gain envelope where the format has them.
- Null tests first: no envelope (or a flat 0 dB one) plays as before to the bit; an
  overlap with crossfades off plays as before; block-size independence; offline equals
  real time; freeze, bounce, stems and export hear them.
- **Effects that sound on insert** (§3b option B): inserting an effect loads a musical
  **starting preset** (Concentus's *Juno I*, …) while each parameter's default stays the
  null that ADR 0018 requires — so its rule and tests are unchanged. Review every unit,
  old and new (Aequalis … Tonus, the strip EQ), and decide per unit (some sound at their
  defaults already; a limiter must not surprise). The insert stays one undo step. A
  setting turns it off (*Insert effects at their neutral settings*). Say what our exported
  CLAP/VST3 plug-ins do when another DAW inserts them (their parameters' defaults stay the
  null; the initial state is the ADR's decision). A test fails when a unit has no starting
  preset and is not in the list of units that need none.
- ADRs 0070 and 0071, the manual (crossfades, clip gain, the starting presets), `clipfx`
  strings, shots, `docs/round31/editing.md`.
