# Round 32 — a face of its own for each unit, and the score from the piano roll

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians
3:23). Tests before code, nothing half done, look at what you build (`shoot` examples).
**The owner's rule:** every gap a feature leaves is closed in the same round. Whatever you
would write under "Left" comes back to you as work; only list as open what truly needs the
owner (their ears, their hardware, a paid account, a decision).

**Run lean** (as rounds 30–31): read only what your brief needs, prefer `grep` and line
ranges to whole files, keep reports short and exact.

The ask (2026-10-27, `docs/next-session.md`):

0. What round 31 left: nothing — the last `macos.yml` run (public b95a9dd9) was green with
   nothing retried, no owner notes (listening, the Mac session) were given, no issues are
   open on `cantus-daw`. beta.7 stays Windows + Linux.
1. **BACKLOG §6h point 1 — each unit with a face of its own** (FL's Love Philter, Gross
   Beat, Vocodex, Harmor; FabFilter's family; Arturia's and u-he's instruments as the
   reference): each effect and instrument with its colour and material, its characteristic
   display, its own control shapes and typography accents, drawn with the kit's vector
   primitives (no bitmaps; crisp at any scale, both themes), still the family's rules. A
   style sheet per unit (`kit::skin`); the exported CLAP/VST3 builds wear the same face.
   **Done when** every unit has its skin shot in both themes at 100 % and 200 %, the frame
   budget holds with three editors open, and a test fails for a unit left plain.
2. **The score from the piano roll** (BACKLOG §0.2 *Notation*, §0.4 item 10): a track's or
   a clip's part as an engraved staff beside the piano roll; MusicXML and PDF; print; basic
   editing on the staff if it fits with quality. Golden engraving tests and a MusicXML
   round trip.
3. Ableton's core, the measured comparisons, the Mac: **not this round**.
4. The owner's frictions and the beta's users: none given at the start; the integrator
   hands out whatever arrives during the round.

Read, in order: this file (the rules and **your** section); `CLAUDE.md`; `docs/STATUS.md`'s
header; ADR 0024 (the kit) and the ADRs your brief names.

## The builders

All start from the contract commit on `r32/integration`. **Never more than three compile
at once** (a 31 GB machine): `skin`, `score` and `instruments` start now; `dynamics` starts
when the first of them is merged (it takes that worktree's `target/`), `modulation` when
the second is.

| Builder | Branch / worktree | Delivers | ADR | Schema | Keymap |
|---|---|---|---|---|---|
| **skin** | `r32/skin`, `F:\projects\daw-r32\skin` | the kit's skin painters (first delivery), then the tone/space units | 0074 | — | — |
| **score** | `r32/score`, `F:\projects\daw-r32\score` | item 2 | 0075 | 45 | 13 |
| **instruments** | `r32/instruments`, `F:\projects\daw-r32\instruments` | Unda, Calor, Modulus, Exemplar, Gravis, Lamina, Organum, Chorda, Pulsus | — | — | — |
| **dynamics** | `r32/dynamics`, `F:\projects\daw-r32\dynamics` | Comprimo, Terminus, Multiplex, Sibilus, Porta, Culmen, Tonsor, Impetus, Discrimen | — | — | — |
| **modulation** | `r32/modulation`, `F:\projects\daw-r32\modulation` | Concentus, Flexus, Phasis, Tremor, Vibrans, Anulus, Translatio, Fluctus | — | — | — |

The skin builder's tone/space units: Aequalis, Fervor, Spatium, Latitudo, Speculum, Tonus.
The empty locale areas are declared: `skin` (every skin builder adds its words there, in
blocks headed by its name) and `score`, in `crates/cantus-ui/locales/areas/` and `i18n.rs`.

## The contract: `kit::skin` (fixed by this commit)

`crates/cantus-ui/src/kit/skin.rs` — read it whole; it is short.

- A [`Skin`] is data: `surface` (a `Tint` of the theme's surface ladder), `accent` and
  `accent2` (`Pair`s: one `Hsl` per theme mode), `material`, `display`, `knob`
  (`KnobShape`, with `Custom(fn)` for a painter of the unit's own), `switch`
  (`SwitchShape`, the same), `lettering`, `plate`, `ornaments`.
- `Style` carries `skin: &'static Skin` (`skin::PLAIN` by default). `Style::skinned(skin)`
  tints the tokens (`Skin::tokens`), takes the accent as the hue and sets `skin`.
- `Shell` carries a skin (`Shell::new(id).skin(…)`) and hands the header, the body and the
  footer the skinned style. Wired: `fx::frame::shell_of(kind)` (so the exported CLAP/VST3
  windows wear it too), the instruments' frame, Pulsus.
- `skin::of(Unit)`; the sheets are one file per family under `kit/skin/`
  (`dynamics.rs`, `modulation.rs`, `tone.rs`, `instruments.rs`), each unit a `const` that
  starts as `Skin::plain("<key>")`. **A family's builder owns its sheet file only.**
- `skin::enabled()` / `set_enabled()`: the user's *Units wear their own faces* switch.
- `tests/skins.rs`: `every_unit_has_a_face_of_its_own` is `#[ignore]`d until the last
  family lands (the integrator turns it on); the keys are unique; the plain skin changes
  nothing.

**Who owns what.** The skin builder owns `kit/skin.rs` and every kit painter
(`kit/*.rs`): the enums' variants, how each shape, material, display and lettering is
drawn. A family builder who needs a shape the enums lack draws it as the unit's own
`KnobShape::Custom` / `SwitchShape::Custom` painter in its sheet file, or asks the
integrator for a variant — never edits `kit/*.rs`. A family builder owns its units' editor
files (`views/fx/<unit>*.rs`, `views/inst/<unit>*.rs`, `views/comp_editor*`,
`views/eq_editor*`, `views/pulsus_editor*`) and may add new files beside them.

**Shared seams.** `views/fx/frame.rs`, `views/fx/controls.rs`, `views/fx/canvas.rs`,
`views/inst/frame.rs`, `kit/shell.rs` belong to the skin builder; a family that needs a
change there names it in its report as a few marked additive lines (or asks). Locale:
`skin.*` keys per unit, `skin.<unit>.…`; each builder's block in `skin.{en,es}.txt`
separated by a comment line with its name, so a merge is additive.

## Every skin builder (skin, instruments, dynamics, modulation)

For **each unit** of yours:

- **A design note first** (in your report, five lines per unit): what it is in sound, the
  reference you drew from (an FL plug-in, a FabFilter, an Arturia or u-he instrument, a
  piece of hardware — *inspired by*, never a copy of a product's trade dress, logo or
  name), its colour and material, its characteristic display, its control shapes, its
  lettering. Siblings must not look alike: no two units share both accent hue (within
  20°) and material.
- **The sheet** in your family's file, in **both themes** (a `Pair` for every colour;
  check contrast: values and labels stay WCAG AA on the tinted surfaces — the kit's
  `color::contrast` helpers; the kit's contrast tests must still pass on every skin).
- **Its characteristic display** drawn in the skin's display look (the skin builder's
  helpers, `kit::skin`'s display painters, once merged; before that, your own display code
  with the tokens it is given): Concentus's BBD lines, Spatium's room, Tonsor's transfer
  curve, Chorda's soundboard, Lamina's tines, Organum's drawbars and rotor, Culmen's bands,
  Speculum's spectrum… If a unit's display is already good, it changes only its look.
- **The family's rules hold**: label/value units, nothing overlapping (the existing
  layout tests must pass for every unit), resizable (75–200 %), keyboard focus and the
  screen reader (AccessKit) as before, the same parameter menu (*Modulate…*, MIDI learn,
  automation) on every control — `tests/` already opens every editor for the menu; keep it
  green.
- **Look at it**: shoot every unit in Slate and Paper at 100 % and 200 % (the skin builder's
  `examples/shoot_skins.rs` once merged; until then, the existing `shoot_effects` /
  `shoot_devices` / `shoot_sampler` / `shoot_drums` examples), and put the best of each in
  `docs/round32/skins/`. Compare with the reference you named; redraw what falls short.
- **No regressions of speed**: a skin draws in the frame budget (the views' budget tests).

## Rules (the usual ones, short)

- Work only in your worktree. Commit WIP often (`r32/<name>`); a cut session resumes.
  Write files in pieces under ~300 lines per tool call.
- Machine: `export PATH="$HOME/.cargo/bin:$PATH"` (cargo is not on PATH); your untracked
  `.cargo/config.toml` sets `jobs = 3`, no debug info and incremental off (never `git add`
  it); `TMP`/`TEMP` = `F:\projects\daw-r32\tmp`. Test executables named `*setup*`/
  `*update*` need elevation on this machine: run them copied to a neutral name.
- Windows: `cargo clippy --workspace --all-targets` and the tests of every crate you touch,
  with `RUSTFLAGS=-D warnings`; `cargo fmt --all` before each commit (if it fails with
  *os error 206*, the command line is too long here: `cargo fmt -p <crate>` for each crate
  you touched). The Bash tool is Git Bash: a heredoc may fail to parse — write files with
  the Write tool. Linux, before you
  report: `wsl -d Ubuntu-24.04 -- bash -lc 'CANTUS_CI_DIR=$HOME/ci-r32-<name> bash
  /mnt/f/projects/daw-r32/<name>/tools/wsl/check.sh r32/<name> clippy'` (and `test` for
  the crates you touched) — your own clone, never `~/cantus-ci`. Code behind
  `#[cfg(windows)]` leaves Linux-only dead-code warnings. UI tests that open plug-in
  windows must pass headless (`env -u DISPLAY -u WAYLAND_DISPLAY` in WSL).
- Real time (ADR 0005): nothing in this round touches the audio thread; if you do, the
  usual null tests apply.
- **Start-up wiring goes in `boot`'s stages / `CantusApp::assemble`**, never only in
  `CantusApp::new` (the shipped app never calls it), with a test that fails if it leaves
  that path. Reviewers ask "is this reachable from `cantus_ui::run`?".
- UI: the hand-drawn kit (ADR 0024) — no stock egui widgets in views, no literal colours
  (`Hsl` values in a skin sheet are data; `Color32` literals stay forbidden by the lints),
  no `FontId::new` (a new font goes through `design.rs`'s roles). FL Studio is the visual
  reference; clear label/value units, nothing that overlaps, every overflowing area
  scrollable.
- A model change is a schema bump (score only: schema 45, `SCHEMA_VERSION`, a migration,
  `cargo run -p cantus-project --example write_fixture` for `fixtures/v45.cantus`); every
  `Command` has an exact inverse, an arm in `crates/cantus-core/tests/undo_invariants.rs`
  and a `cmd.*` string; a new default binding is a keymap bump with its migration.
- Strings in both locales (your area file); the manual (`docs/manual/`, EN and ES) for
  every feature a user touches; your ADR; `docs/round32/<name>.md`: what was done, how it
  was tested, the numbers, and **only** what needs the owner.
- Never touch `master`, never push, never stage files outside your worktree.

## skin — the kit's skin painters, then the tone/space units (ADR 0074)

Read ADR 0024 (the kit: §2 colour, §4 painters, §8 the device frame), `kit/mod.rs`,
`kit/skin.rs`, `kit/{knob,led,cap,band,well,shell,light,cache,text}.rs`,
`views/fx/{frame,controls,canvas}.rs`, `views/inst/frame.rs`, `tests/kit_gallery.rs`,
`tests/kit_tokens.rs`, the budget tests (`tests/{frame,views,workspace}_budget.rs`), and
`crates/cantus-plugins/src/editor.rs` (the exported windows).

**First delivery — the kit draws every sheet** (commit it as its own step and tell the
integrator: the other skin builders merge it as soon as it lands):

- Every variant of `Material`, `Display`, `KnobShape`, `SwitchShape`, `Lettering`,
  `Plate` and `Ornaments` drawn by the kit's painters, in vectors, through the lighting
  recipe, at the style's `k`, in both themes — the ground and cards (`shell`, `band`),
  knobs (`knob`; `Custom` called in place of the body), switches (`led`, segmented where
  it fits), wells and readouts (`well`; `mono_values`), the header's nameplate (`shell`;
  `Plate`, `Lettering`), and **display painters** the families call for their
  characteristic displays (`skin::display_ground(b, rect, s)`, a trace / glow / grid in
  the display's look, its text colours) — one way to light a display, not one per unit.
  Add variants if the families' references need them (a VU needle meter is a likely one);
  you own the enums.
- Caches keyed by the skin as well as size and theme (cached knob bodies and glows must
  not leak one unit's colours into another's window); the inspect mode and
  `kit_tokens.rs` keep working (a skin's colours registered under the unit's key).
- Editors that build their own `Style::current()` inside the body instead of using the
  one the shell hands them would lose the skin: find them (`grep`) and fix them, with a
  test that fails if a unit's body paints with the plain tokens.
- **The user's switch**: *Units wear their own faces* in Settings ▸ Appearance (on by
  default), kept in the settings, applied in the boot stages and when changed (a boot-path
  test), both locales.
- **`examples/shoot_skins.rs`**: every unit's editor in Slate and Paper at 100 % and 200 %
  (and one with the switch off), named `<unit>-<theme>-<scale>.png`, with a filter
  argument for one unit — every skin builder uses it.
- **The kit gallery** draws every material, shape and plate (goldens blessed on purpose).
- **The frame budget with three editors open** (say Culmen, Unda and Spatium, docked
  side by side at 100 % and floating at 200 %): a test in the budget suite, measured on
  this machine, the numbers in your report.
- **The exported plug-ins**: a CLAP/VST3 editor window wears its unit's face — a test or a
  shot through `cantus-plugins`' editor, both locales; the plug-ins have no settings file,
  so they always wear it.
- ADR 0074 (the skin system: data, the shapes, both themes, the switch, what hosted
  plug-ins keep), the manual (Appearance: the switch; a page on the units' faces).

**Second delivery — the tone/space units**: Aequalis, Fervor, Spatium, Latitudo,
Speculum, Tonus, as every skin builder does (the section above).

## score — the score from the piano roll (ADR 0075, schema 45, keymap 13)

Read BACKLOG §0.2 *Notation*; ADR 0024; the piano roll (`views/piano_roll*.rs`), the
song's key (`cantus_core::scale::Key`, ADR 0069), the tempo map with its metre changes
(`cantus_core::tempo::TempoMap`), the clip model (`Clip`, `Note`, PPQ), the MIDI export
(how a track's clips become one part), `design.rs` (fonts by role), and how a pane is added
to the workspace (`views/workspace.rs`, `Pane`).

- **A crate of its own, `crates/cantus-score`** (GPL-3.0-or-later; no egui): notes in →
  an engraved score out. **Reading** (quantised for reading, never touching the notes: a
  grid per passage that finds straight and triplet values, with an override), voices and
  chords, **ties** across beats and bar lines by the metre's beat groups, **rests**
  consolidated the way an engraver writes them, **beams** by the metre (4/4 by beats, 6/8
  by dotted quarters, …), stems by the staff's middle line, **tuplets** (triplets at
  least; quintuplets and sextuplets if the data asks), **key signatures** from the song's
  key with spelling that follows it (accidentals per bar, courtesy accidentals), **metre
  and tempo changes** from the tempo map (♩ = 120 marks), **clefs by range** (treble,
  bass, 8va/8vb where a part lives high or low), a **grand staff** for piano parts (a split
  that follows the hands, not only middle C), **dynamics from velocity** when asked, bar
  numbers, the part's name. **Engraving** after Dorico, MuseScore 4 and Sibelius, and
  Gould's *Behind Bars*: spacing by duration, systems and pages broken well, accidentals
  stacked without collisions, ledger lines, dots, ties' and slurs' curves, beams' slopes —
  in the SMuFL font **Bravura** (SIL OFL 1.1: add it to the assets and to
  `THIRD-PARTY-NOTICES.md` via `cargo run -p xtask -- notices`) with its engraving
  defaults and anchors. The output is a display list in staff spaces that the screen, the
  PDF and the tests share.
- **The view**: the part of a clip, or of a whole track across its clips, as a staff
  beside the piano roll (a toggle in its toolbar, the split resizable; and a Score pane
  for a track in the workspace) — in the kit (a music-font role in `design.rs`; no stock
  widgets; both themes), following the playhead, sharing the selection with the piano
  roll, scrolling and zooming like it, within the frame budget for a 200-bar part (layout
  cached, redone on change).
- **Per-track score settings** (schema 45: clef or auto, the grand staff's split, a
  transposition for transposing instruments, dynamics from velocity, the reading grid, the
  part's name) as `Command`s with exact inverses, undo-fuzz arms, `cmd.*` strings.
- **MusicXML 4.0** export (partwise; a track, or the song as a score with every MIDI track
  a part) that opens in MuseScore — check it in MuseScore 4 if you can install it on this
  machine (portable, for testing only, never shipped), otherwise against the XSD — and
  **import** of MusicXML as a clip if the round trip makes it cheap. **PDF** export
  (vector, the fonts embedded and subset, A4 and Letter, title and composer from the
  project) and **print** (the system's print of that PDF, honestly described per OS).
- **Editing on the staff** if it fits with quality (select, move by step, delete, enter
  notes by letter and duration as MuseScore does — every edit a `Command` on the notes);
  if not, measure what it would need and write it down for the owner.
- **Golden tests** of the engraving (a melody, a two-hand piano piece, triplets,
  syncopation, long tied notes, a metre change) as display-list snapshots plus shots;
  a **MusicXML round trip** test; PDF opened back and checked (pages, fonts).
- Keymap 13 if you add a default binding (*Show score*); the palette lists it.
- ADR 0075, the manual (the score view, MusicXML, PDF, printing), `score` strings, shots
  in `docs/round32/score/`, `docs/round32/score.md`.

## instruments — Unda, Calor, Modulus, Exemplar, Gravis, Lamina, Organum, Chorda, Pulsus

Read ADR 0024 §8, the instruments' frame (`views/inst/frame.rs`) and your editors
(`views/inst/*.rs`, `views/pulsus_editor*.rs`), `kit/skin.rs`, `kit/skin/instruments.rs`.
As every skin builder (above). Until the skin builder's first delivery lands, begin with
the design notes, the sheets' colours (they show at once: `Skin::tokens`) and the
characteristic displays (Unda's wavetable, Calor's filter and envelope, Modulus's
operators, Exemplar's zones and waveform, Gravis's string or 808 body, Lamina's tines and
pickup, Organum's drawbars and rotor, Chorda's soundboard and hammers, Pulsus's pads and
steps); then merge `r32/integration` when the integrator says the kit has landed, and give
each unit its shapes, material and lettering.

## dynamics — Comprimo, Terminus, Multiplex, Sibilus, Porta, Culmen, Tonsor, Impetus, Discrimen

Starts when the integrator says (it inherits a merged worktree's `target/`), from
`r32/integration` with the kit's skin painters in it. Read ADR 0024 §8, `kit/skin.rs`,
`kit/skin/dynamics.rs`, your editors (`views/comp_editor*.rs`, `views/fx/{limiter,
multiband,deesser,gate,maximizer*,tonsor,impetus,discrimen,dynfx}.rs`). As every skin
builder (above). Gain reduction is these units' story: their displays and meters (a VU
needle, a GR trace, Tonsor's transfer curve, Culmen's bands) carry their identity.

## modulation — Concentus, Flexus, Phasis, Tremor, Vibrans, Anulus, Translatio, Fluctus

Starts when the integrator says, from `r32/integration` with the kit's skin painters in it.
Read ADR 0024 §8, `kit/skin.rs`, `kit/skin/modulation.rs`, your editors
(`views/fx/{concentus,flexus,phasis,tremor,vibrans,anulus,translatio,fluctus*,modfx}.rs`).
As every skin builder (above). Movement is these units' story: their displays (Concentus's
BBD lines, Flexus's comb, Phasis's notches, Tremor's and Fluctus's shapes, Vibrans's pitch
wobble, Anulus's sidebands, Translatio's shifted spectrum) carry their identity.
