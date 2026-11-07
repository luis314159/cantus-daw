# ADR 0101 — Presets by style and role, heard before they are chosen

**Status:** accepted; built (round 37, library: the foundation, then the library) · **Date:** 2026-11-04 ·
**Schema:** none (57 stays free: see *Gain staging*) · **Relates to:** ADR 0023 (native
instruments), ADR 0029 (the device picker), ADR 0039 (instrument output calibration), ADR 0071
(effects that sound on insert), ADR 0030 / 0042 / 0050 (content), ADR 0005 (real time)

## Context

BACKLOG §5 asks for *dozens of musical presets per instrument and effect, made by ear, a shared
library with tags and audition*, and for instrument gain staging. Round 37's brief (library)
turns that into: an audit with a floor per unit, presets tagged by **style** (the styles most
produced today) and **role**, a render check that holds every preset clean and level, a browser
that filters, searches and **auditions without changing the project until a preset is
applied (one undo step)**, and the instruments calibrated to one reference level.

What existed at the start of the round (measured on the contract commit c95ee8a9):

- Factory presets are Rust arrays in `cantus-ui`: instruments `FactoryPreset { label_key,
  values, text, tags }` per kind, effects `(label_key, values)` per kind; a name is a locale
  key. The *tags* of instruments are descriptive (`preset.tag.keys`, `organ.tag.jazz`,
  `bass.tag.808`); effects have none. **No preset says what style it is for.**
- **Counts today** (`tests/preset_audit.rs`, printed): see *What the tests hold*.
- The preset menus (the instrument shell's well, the effect frame's well, Pulsus's kit list)
  load a preset at once, as one command — they are the only way to hear one.
- The sample browser already auditions files through an **audition voice after the master**
  (`cantus_engine::preview`, RT-safe: the sample arrives as an `Arc`, is handed back to be
  freed on the interface's thread).

The leaders ship dozens to hundreds of presets per instrument and ten to fifty per effect in
their browsers (FL Studio's plug-in presets, Ableton Live's Core Library, Bitwig's); FL 24's
browser rows show a picture, a mini waveform and tag chips, and a preview player at the foot
with *Sync BPM* (the integrator's notes, `research/ui/fl-look-2026-11/notes.md` item 9). Exact
counts per device are the scorecard's to measure from current manuals; the floors below are
set to beat the brief's (24 per instrument, 10 per effect) without padding the library with
near-duplicates.

## Decision

### 1. The vocabulary (`crate::preset_tags`)

- **Eleven styles**: trap · hip-hop, pop, house · EDM, R&B, lo-fi, rock, latin · reggaeton,
  ambient · cinematic, film · game — the brief's — plus **jazz · soul** (Organum, Lamina,
  Gravis's fingered basses and Chorda are made for it) and **worship · gospel** (one of the
  most produced styles in the world, and the owner's).
- **Roles**: an instrument's preset is a *bass, lead, pad, keys, pluck, effect* or *drums*
  part; an effect's goes on a *vocal, drums, bus* or the *master*, or is *creative*.
- A preset carries them as tag keys (`preset.style.trap`, `preset.role.bass`), beside the older
  descriptive tags, which are read too (`organ.tag.gospel` → worship, `preset.tag.bell` →
  pluck, `preset.tag.brass` → lead…). A preset that names no role takes its unit's natural one
  (Gravis a bass, Lamina/Organum/Chorda keys, Pulsus drums, Culmen/Terminus the master,
  Sibilus/Tonus a vocal).

### 2. The floor (the audit)

| Unit | Floor | Why |
|---|---|---|
| Every instrument that sounds (Unda, Calor, Modulus, Exemplar, Gravis, Lamina, Organum, Chorda) | **24** | the brief's floor; one preset per style and role pairing that unit is made for, not padding |
| Pulsus's kits | **16** | a kit covers a whole style; the 808/909/trap/lo-fi/house kits of round 37 plus today's 8 |
| Every effect | **12** | over the brief's 10: every role the unit serves, two or three styles each |
| Speculum (the analyser) | **8** | its presets are views (spectrum, sonogram, loudness, correlation…), not sounds |
| The dancer, a hosted plug-in | — | the dancer makes no sound (its presets are dances); a plug-in keeps its maker's |

Across the library every style has at least **12 instrument presets** and every role at least
**12 presets**, so a style filter is never a near-empty list. Every factory preset names at
least one style and one role.

### 3. The format: presets as data beside the arrays

- **New presets are data**: one TOML file per unit under `crates/cantus-ui/presets/`, built
  into the binary (`include_str!`), read once, at the first use of a unit's presets — never on
  the start-up path. A file has `[[preset]]` tables (a key, styles, roles, `values`, `text`)
  and `[[tag]]` records that give styles and roles to **presets written in code** — so
  Chorda's (piano's) and Pulsus's kits (drums') are tagged without editing their files.
- **The arrays stay**: starting presets (ADR 0071), demos and templates find presets by key;
  `views::inst::factory_presets` / `views::fx::factory_presets` now return the code presets
  followed by the data presets, as one `&'static` slice built once — so every reader (the
  menus, A/B, the agents' `list_presets`, the picker's previews, the demos) sees the data
  presets with no change. `code_presets` is the arrays alone.
- **Names stay locale keys** in both languages (the i18n test covers them), in the unit's own
  locale file — not inline in the data: names inline would have put a parse of every preset
  file on the interface's first `t()` call, i.e. on the start-up path.
- **Why data**: about 430 presets at the floor; as Rust they are ~25 lines each under rustfmt,
  with tags as untyped strings. As data they are a third of that, checked by a test (every
  key a parameter of the unit, every value in its range, every style and role a word of the
  vocabulary and a role of the unit, no key twice, names in both languages); the website's
  renderer, the agents and Cantus's own preset packs read the same files.
- A file that does not read is left out at run time with a log line (the code presets still
  work) and fails the tests.

### 4. Finding and hearing: the browser (`preset_browser`, `views::preset_browser*`)

- **Opened** from an instrument's preset menu (*Browse presets…*, its first row) and from an
  effect's (the effect frame's `FxHost::browse_presets`; the exported plug-ins and hosted
  units do not offer it). Pulsus's kit list is drums' file this round: its entry is one line at
  integration (the browser already lists kits: `Unit::Kit`).
- **Finding**: a search (every word must be found, without case or accents, in the name, a
  style's or role's name, or the unit's), **style chips and role chips with their counts** —
  styles are alternatives, roles too, the two narrow. Chips a unit has no preset for are not
  shown.
- **Rows** (FL 24's, in our kit): a ▶ key, the name, its role and styles, the **mini waveform
  of what was heard**, a gold dot on the one loaded; a click chooses, a double-click loads.
- **The player** at the foot: the preset heard (or chosen), *what it was heard on*
  ("a lead phrase at 120 BPM, rendered by Cantus" — honest), the large waveform with its
  playhead, the count, **Sync BPM** (audition at the project's tempo; off, at 120, the tempo
  the presets are levelled at) and **Load**.
- **Keys**: while open it reads the keyboard, as the device picker does — typing searches,
  ↑ ↓ choose and hear, Space hears again or stops, Enter loads, Esc closes.
- **Audition = an offline render of a copy**, played by the audition voice after the master:
  an instrument's preset (or a kit) on its **role's phrase** (two bars in C minor at 120 BPM:
  a bass line, a melody, held chords, comping, an arpeggio, held notes for an effect patch, a
  groove); an effect's preset on **the selected clip** of the user's project (its track soloed
  in the copy, the insert at the preset and not bypassed, at most 8 s), else the first clip
  its strip hears, else a **fixed source** made by Cantus's own instruments (a groove; a lead
  line standing in for a voice; a small band for a bus or the master). The render runs on a
  worker; the project, its undo history, the engine's playing snapshot and the saved file never
  see it. A finished render is cached by preset as its outline (the row's mini waveform).
- **Load = the unit's own preset command**, one step of undo (`cmd.fx_preset`, which the
  history already names): `apply_inst_spec`, `apply_fx_params`, Pulsus's `load_kit`.

### 5. Every preset rendered and measured (`tests/preset_render.rs`)

Every factory preset of every unit that sounds, on the material it is auditioned on, must have
no non-number, no silence, no clipping (sample peak ≤ 0 dBFS), **a loudness inside its window**
— an instrument within **±2 LU** of its role's target on its phrase; an effect within ±2 LU of
the dry source — and **a CPU share under its unit's budget** (asserted in a release build;
reported in debug). A limiter, a maximizer and a clipper make level by design: no window
for them, the clipping check holds. The presets made before round 37 that fail a check are
listed check by check with what was measured (`tests/data/preset_render_known.txt`,
rewritten by a run with `CANTUS_PRESET_KNOWN_DUMP=1`): the library's second delivery
retunes them by ear and empties the list; a failure not listed fails at once, and a listed
failure of a unit the library owns that is gone fails until it is struck off. Kits are heard
on the groove mapped to their own pads; an effect's fixed source sits 8 dB under unity so an
insert has headroom.

**The role targets** (`preset_measure::role_target`, LUFS integrated on the role's phrase):
bass −16, lead −18, pad −19, keys and pluck −21, effect patch −20, **drums −19**. Drums were
first at −16, above the instrument family's −18.7; a kit's groove has by far the highest crest
factor of any part, so matching −16 drove single hits near 0 dBFS. The integrator moved the
drums role to −19 ± 2 LU (round 37, with drums' kit gain stage, schema 57): a kit sits at the
family's level, a little under it, and the single-pad rule (a lone hit peaks under 0.6) is
kept. Keys and plucks moved from −19 and −20 to −21 for the same reason at a smaller scale —
struck and plucked parts need their velocity-127 headroom.

### 6. Gain staging

Measured at the start of the round (release build, `instrument_levels.rs` staging reports and
the dense-riff headroom test; `tests/instrument_reference.rs` holds it from now on). Each
kind's **new instance** on the staging triad (C4 E4 G4, velocity 100, two seconds; Gravis its
root an octave down), at unity, nothing on the master:

| Kind | LUFS | vs −18.7 | Peak on the velocity-127 dense riff |
|---|---|---|---|
| Unda | −18.7 | 0.0 | −6.4 dBFS |
| Calor | −19.2 | −0.5 | −6.3 |
| Modulus | −20.0 | −1.3 | −6.9 |
| Exemplar (placeholder) | −19.2 | −0.5 | −5.4 |
| Gravis (one note) | −15.7 | +3.0 | −8.9 |
| Lamina | −22.9 | −4.2 | −3.2 |
| Organum | −18.5 | +0.2 | −4.7 |
| **Chorda (model 1)** | **−30.1** | **−11.4** | −7.2 |

- **The brief's figure is out of date.** *Lamina and Exemplar 8–9 LU under* was round 19's
  measurement; ADR 0039 (round 24, schema 21) calibrated them with a hidden trim and a
  migration. Exemplar is level now; Lamina sits 4.2 LU under on purpose: at velocity 127 its
  default patch already peaks at −3.2 dBFS on the dense riff, so the +4 dB more it would take
  passes full scale (ADR 0039's ceiling, measured again).
- **Chorda is the one real gap** (−11.4 LU). Its fix is not a trim of model 1: schema 54
  (piano) makes every *new* Chorda model 2 and writes `model = 1` into every old one, so a
  trim on model 1 would change no new instance and only old songs — the opposite of what a
  calibration may do. **Model 2's level is the piano builder's**: its default patch at the
  reference (−18.7 LUFS on the staging triad, within ±4.5 LU), its dense-riff peak under
  0 dBFS. `tests/instrument_reference.rs` lists Chorda as *pending (piano: model 2)* and fails
  once a new Chorda is level and the list still names it.
- **So schema 57 is not needed and not taken.** The reference test holds every kind's new
  instance at −18.7 ± 4.5 LU.
- **The spread that matters is inside the kinds**: today's presets range from +8.6 LU over
  their role's target (Exemplar *Sub* on a bass line is fine; on keys it was not — roles fixed
  that) to −24.9 LU under (Unda *Stretched bells*). That is the presets' own `gain`, fixed
  preset by preset in the library's second delivery under the render check's ±2 LU window —
  copying a factory preset into a project is what changes a sound, so no saved project moves.

### 7. The second delivery (round 37): the library made, its levels fitted, heard outside

- **Every library preset is data now.** The arrays of the units the library owns were moved
  into their data files by `examples/preset_tool.rs export` (keys, values, texts and tags as
  they were) and emptied; their readers (each editor's demo set-up, the tests, the
  maximizer's own check) read `factory_presets(kind)`. So §3's "two homes" holds only for
  Chorda (piano's) and the dancer; the starting presets of `cantus-core/src/starting.rs`
  still name their keys, which the data keeps. A preset may carry a one-line `note` (what it
  is for, English), shown to agents and on the website.
- **Levels are fitted by measurement, not by hand.** `preset_tool fit` renders each preset
  on its role's material and moves its level key — an instrument's `gain`, an effect's `out`
  (or `gain`, `makeup`) — to its window's centre, up to five passes (a unit that saturates
  after its gain needs more than one); it rewrites the file in one order, so a change is one
  line. A preset whose quietness the level key cannot reach (its `gain` at +12 dB) was fixed
  in its sound (an oscillator's level, a mix) by a second pass. Units with no level key
  (Spatium, Sibilus, Porta, Delay, Speculum, Tonus) are levelled by their mix or not at all.
  The starting presets an insert loads (ADR 0071, `cantus-core/src/starting.rs`) are factory
  presets and follow them: ten took their new `out` (and Impetus *Punch* its gentler attack),
  so a fresh insert no longer jumps in level either. The demo songs do not follow: a song is a
  mix, balanced on the presets as they were, so `demos/as_mixed.rs` sets back the values this
  delivery changed in the presets a demo loads.
- **By design, no window**: a listen preset (Latitudo *Side solo*, Discrimen *Only
  transients* / *Only body*), a duck that needs its key (Porta *Duck*), Tonus *Octave down*
  (an octave down reads quieter by K-weighting) — `preset_measure::by_design`, each with its
  reason; the clipping check still holds them.
- **CPU budgets per unit**: a tenth of a core for every unit, a fifth for Chorda (ADR 0059)
  and Culmen (a multiband chain into a limiter, once per song), 15 % for Exemplar (many
  overlapping tails at its alias-free quality). The presets over budget were lightened —
  fewer unison voices, stacked voices, partials or grains — rather than the budget raised.
- **The older Reverb is no longer offered** (the picker, the insert and mixer menus:
  `EffectKind::offered`); old projects keep theirs and sound the same (`tests/legacy_reverb.rs`).
  Its floor is 0. Delay is a real unit and has its 13.
- **Cantus's own packs** (`examples/render_packs.rs`, `cantus-content/examples/pack_folder.rs`):
  *Cantus One-Shots* (a note of every library instrument preset, by role, 2.5 s, 44.1 kHz,
  24-bit) and *Cantus Loops* (two-bar loops at 120 BPM cut from the middle of three passes, so
  they loop without a seam: ten per instrument from the styles producers loop most, and every
  factory kit's groove), each file peak-normalised to −1 dBFS as a sample pack's are (a preset's
  mixing level is quiet for a file dropped on a pad; one gain per file, so a loop still
  loops). CC0, the dedication in each pack's `LICENSE.txt` (the owner's to
  confirm). Hosted as release assets of `luis314159/cantus-daw` under the tag `content-r37`,
  their size and SHA-256 pinned in the manifest; **the integrator uploads them**. A new pack
  category, `loops`, needs manifest **format 4** (an older Cantus refuses a format-4 list by
  name, as it refuses every newer format).
- **Packs from elsewhere, vetted** (licences read at their source, 2026-11-05): the manifest
  already holds Karoryfer *Big Rusty Drums* and VSCO 2 *Percussion*; vetted for the styles
  still thin on free sounds are the rest of VSCO 2 Community Edition (strings, winds, brass and
  keys for film and worship; CC0 1.0, `LICENSE` in `github.com/sgossner/VSCO-2-CE`) and Kenney
  *Sci-fi Sounds* and *Impact Sounds* (film and game effects; "Creative Commons CC0" on each
  asset's page, `kenney.nl/assets/…`). Recorded, not added: adding one means pinning its
  files' URLs and hashes after a download, a step for the round that adds them.
- **The website plays every unit's presets** (`examples/render_unit_sounds.rs` →
  `web/assets/units/sounds/*.mp3` and the generated `web/src/units_sounds.rs`): on a card a ▶
  that plays its first excerpt; on its page every excerpt with its name and what it is for,
  an effect's beside its dry source. Up to four per instrument and three per effect, chosen
  for the range of the unit (a role not heard yet, then a style not heard yet). MP3 at
  96 kb/s; the presets are at their own levels (the render check holds them within ±2 LU),
  so the excerpts need no matching after — the page says *rendered by Cantus … with no
  processing after*. README's unit grid links each unit to them. Delay, with no window of its own, has a card too:
  its pictures are its view in the inspector (`examples/shoot_delay.rs`).
- **Agents**: `find_presets` (every unit's presets, or one unit's, filtered by style, role
  and words, each with what it is for) and `audition_preset` (renders a preset on what the
  browser plays it on, answers its loudness, peak and length, plays it to the user, and
  changes nothing); `apply_preset` loads one, as before.
- **The integrated review's fixes (round 37, `docs/round37/fixes.md`).** The agent's tools
  answer in English *without* changing the window's language (`i18n::scoped_thread_locale`,
  a guard that gives the thread back what it spoke: the MCP pump runs in the frame, and a
  plain `set_thread_locale` had switched a Spanish window to English for good).
  `apply_preset`/`list_presets` know every factory kit by the name `find_presets` gives it —
  read from `preset_catalog::entries(Kit)`, with *Default kit* and *Studio kit* kept as
  aliases — and a test loads, for every unit, what `find_presets` names. The retired Reverb
  is offered to no agent either: the name `reverb` makes a Spatium (`args::effect_kind`), the
  schemas' effect lists and `list_units` leave it out, and a pad's chain's *Add insert ▾*
  filters by `offered()`; an old project's Reverb is still reached by its `effect_id`.
  **`audition_preset` is a job** (`ToolRun::Job`): the copy is prepared on the interface's
  thread, rendered and measured on the job's worker, and — played — handed to the browser's
  own audition (`Audition::begin_elsewhere`), which records what is heard and until when,
  so Stop, Esc (with the browser closed too) and the next audition stop it with a fade, and
  the frame polls it with the browser closed. Stopped while it renders, the job answers
  `stopped`; cancelled by the agent, before or after its render, it answers Cancelled and is
  never played. An old project's Reverb is `reverb_legacy` in every answer and in
  `get_unit`/`list_presets` (`args::any_effect_kind`), so an agent learns its own keys;
  `reverb` names Spatium only (second review).
- **Review fixes** (both reviews of the first delivery): the audition never blocks the
  interface (a cut-down copy, compiled without waiting; a silent result falls back to the
  fixed source), renders are cancelled (`RenderProgress`) by a new audition, by Stop and by
  another project, ↑ ↓ wait 150 ms for the selection to rest, an insert is heard only on a
  clip its strip hears (pad strips and buses included, `Project::tracks_on_strip` and the
  routes), unmuted in the copy, never through a hosted unit or an ARA clip; a frozen target
  is auditioned live and its *Load* refused, the player saying so; the browser takes only its
  own keys (Ctrl+Z and the rest reach the shortcuts) and starts a search on the first letter
  typed; the effect's window names the preset the browser loaded; the instrument preset list
  no longer closes on the click that opens it (a bug of the shell's pop-up, found by the e2e
  test).

## Consequences

- Every unit's presets are found by what they are for and heard before they are chosen, on a
  part like the one being written; the user's project changes only when one is loaded.
- Two homes for factory presets only for Chorda's and the dancer's (code); every library
  preset is data — the catalogue reads both the same way, so Chorda's can move into data
  without the browser changing.
- An audition costs a render of 5.5 s of sound: in a release build a median of 0.13 s; every
  preset now renders within its CPU budget, so the slowest is a fraction of a second; the ▶
  key shows an arc while it renders, and a new audition cancels the old render.
- While the browser is open it takes only its own keys (↑ ↓, Space, Enter, Esc, typing into
  its search): Space hears a preset rather than starting the transport; Ctrl+Z and every other
  shortcut still reach the application.
- Cantus's own packs are rendered by the program: re-rendering them after a preset changes is
  a command, but a new zip is a new pack version (its hash is pinned in the manifest).

## What the tests hold

- `cantus-ui` unit tests: `preset_tags` (every word reads back, every style's and role's name
  in both languages, the older tags sort where they belong); `preset_query` (styles are
  alternatives, roles too, the two narrow; every word found without case or accents, in a
  style's, role's or unit's name); `preset_data` (every shipped file reads and holds only what
  its unit has; the format reads presets and `[[tag]]` records, integers as numbers; the check
  refuses an unknown style, a role of the other family, a key a code preset has, an unknown
  value or text, a value out of range, tags for a preset not in code, an unknown field; an
  effect's preset has no texts).
- `tests/preset_audit.rs`: the counts per unit (printed: 304 factory presets in 35 units at
  the round's start, 10 with a style; 511 at its end, the 507 that sound all with one) and
  unique ids; every unit at its floor (Chorda's and the kits' too, since they landed), every
  preset with a style and a role, every style ≥ 12 instrument presets, every role ≥ 12.
- `tests/preset_render.rs`: every factory preset of every unit that sounds (300 renders at the
  start, 507 at the end), no non-number, no silence, no clipping, its loudness window, and its
  CPU (alone, after a warm-up, best of two — of five near its budget — in release builds).
  Role targets, LUFS on the phrase: bass −16, lead −18, pad −19, keys −21, pluck −21 (−19 and
  −20 until the velocity-127 headroom of the dense riff showed them too loud to play hard),
  effect patch −20, drums −19 (−16 until the integrator's decision, §5); window ±2 LU;
  effects ±2 LU of the dry source (a limiter, a maximizer and a clipper make level by design:
  no window). The headroom material is the dense riff and, for Unda (whose own test plays
  it), its loud eight-note chord after it. CPU: 10 % of a core per phrase, 20 % for Chorda and
  Culmen, 15 % for Exemplar; a debug build measures a preset over its budget again alone, against the
  run's median preset alone. The failures are listed in
  `tests/data/preset_render_known.txt` (147 at the start: 138 levels, 7 CPU, 2 clips; at the
  round's end none of the library's — Pulsus's kits drums has still to bring into the −19 window, and 14 of
  Chorda's, levelled by piano to the older keys target, 2.0 to 2.8 LU over) — any other
  failure fails, a listed one that gets worse by more than 1 LU, 0.5 dB or 25 % of its CPU
  fails, and a listed failure that is gone fails until struck off. A preset its velocity-127
  headroom keeps under its window is printed, not failed; a debug build checks CPU in units
  of the run's median.
- `tests/instrument_reference.rs`: every kind's new instance at −18.7 ± 4.5 LUFS on the
  staging triad (Chorda's too, since piano's model 2).
- `tests/preset_browser_e2e.rs`: through `CantusApp::assemble` and a manual engine — the
  instrument editor's preset menu, its *Browse presets…* row clicked, the browser on Unda;
  a row's ▶ clicked: the audition renders, its row gets its waveform, **the engine's output
  carries it, and the project and its undo history are unchanged**; *Load* clicked: the
  instrument is the preset, **one more step of undo**, undo brings the old one back; then an
  insert (Fervor) auditioned on the user's own clip (the project unchanged) and loaded in one
  step; Esc closes; and a Pulsus kit from its ⋯ menu's *Browse presets…* row, heard on its
  groove and loaded in one step. And `prepare`: an insert is heard on the user's clip with its
  track soloed, in a copy; with no clip, on a fixed source with the insert on its master.
- `tests/preset_browser_review.rs`: the reviews' fixes — routing through buses and pad strips
  (a split kit), the cut-down unmuted copy, hosted and ARA fallbacks, cancelled renders and the
  150 ms settle, the browser's own keys (Ctrl+Z reaches undo), the effect window's name after
  a load, the browser closed by a new project, a frozen strip heard live and its *Load*
  refused. `tests/legacy_reverb.rs`: the older Reverb not offered, an old project with it
  rendering the same.
- `tests/library_fixes.rs` (through `CantusApp::assemble`, its MCP server and the manual
  engine): a Spanish window stays Spanish after `find_presets` and `audition_preset`;
  `audition_preset` stopped while it renders answers *stopped*, played it is the browser's
  audition (heard, until when), no frame waits for it, its sound comes from the engine and
  Stop silences it; every name `find_presets` gives, for every unit, is in `list_presets`
  and loads (every kit, each unit's last); `reverb` is a Spatium on a strip and on a pad's
  chain, and no schema or `list_units` offers the old one. `tests/stated_preset_counts.rs`:
  the README's and the website's preset counts are the units'.
