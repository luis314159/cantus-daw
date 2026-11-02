# Round 35 — a character that dances on screen (BACKLOG §6i, whole)

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians
3:23). Tests before code, nothing half done, look at what you build (`shoot` examples).
**The owner's rule:** every gap a feature leaves is closed in the same round. Whatever you
would write under "Left" comes back to you as work; only list as open what truly needs the
owner (their eyes and ears, their hardware, a paid account, a decision).

**Run lean** (as rounds 30–34): read only what your brief needs, prefer `grep` and line
ranges to whole files, keep reports short and exact.

The ask (the owner, 2026-10-30, `docs/next-session.md`):

0. What round 34 left: nothing — `macos.yml`, CI and Release on public `782dc960`/`0514b44f`
   green with nothing marked *FLAKY on macOS VM*, no issues on `cantus-daw`, no owner notes,
   the Mac session not done (beta.10 stays Windows + Linux).
1. **A character that dances** (FL's Fruity Dance is the reference for the *idea*; ours is
   our own, never its character or art): a visual unit of its own on a track, drawn over the
   workspace in vectors, crisp at any scale, floating or docked, always on top if asked, on
   a transparent background; off the audio thread (it reads the transport and its numbers
   and never touches a sample — a null test) and within the frame budget.
2. **Driven by automation**: its numbers (move, step, speed, jump, turn, spin, size,
   position, mood, colour…) are ordinary automation and modulator targets (ADR 0057), MIDI
   learn reaches them, notes choose its moves (a key per move, like a drum map); by default
   it dances **to the beat** with no automation, and it can react to a track's level.
3. **Characters included**, ours and original (for the whole family, on both themes), each
   with its moves, in a picker with previews; **characters imported** — Rive, Lottie,
   sprite sheets (Aseprite, TexturePacker), DragonBones evaluated first by licence and
   capability; glTF/VRM 3D later; Spine and Live2D out unless their licence serves a free
   GPL app. A character file is checked and sandboxed (no scripts) and shows its licence.
4. **Export the clip as video** with the song's audio (MP4/WebM, square and vertical for
   social media, or a transparent layer for an editor; a chroma option). **The round's
   risk** — measured and decided before the video builder starts (ADR 0088).
5. **A companion while working and on stream**: a small always-on-top window that dances
   with whatever plays (transport, the master's level and beat) without automation; a
   **streaming mode** OBS or Streamlabs captures cleanly (its own window title, a
   transparent or chroma background, a fixed size and position kept between sessions,
   nothing else of the interface); a light visualiser beside it (the master's spectrum or
   level, bar and beat, the song's name and tempo); cheap enough to leave on all day (its
   CPU and GPU cost measured and shown; it pauses hidden or minimised).
6. **Done when** an included character dances to the beat with no automation, an automation
   clip changes its moves exactly on the bars it was drawn, an imported Rive or Lottie
   character (and a sprite sheet) plays with its moves mapped to our parameters, the frame
   budget holds with it on screen, a 30-second clip exports as video with the audio in
   sync, and OBS captures the streaming window with a transparent background while the
   character follows the music for an hour, its cost measured. An MCP agent can move it too
   (its parameters in the unit catalogue).

Read, in order: this file (the rules and **your** section); `CLAUDE.md`; `docs/STATUS.md`'s
header; BACKLOG §6i; the contract's files (below); the ADRs your brief names.

## The builders

All start from the contract commit on `r35/integration` (worktrees in
`F:\projects\daw-r35\<name>`, branches `r35/<name>`).

| Builder | Delivers | ADR | Schema | Other numbers |
|---|---|---|---|---|
| **dancer** | the instrument end to end: the model and its commands, the engine's silent unit and its feed, the choreography (`pose.rs`), the live feed on the interface, the instrument's window and the character picker, notes and the move map, "React to", MIDI learn, the catalogue and agents | 0087 | 49 | — |
| **cast** | the included characters (the family, every move and mood, both themes), the rig and drawing helpers, the renderer's speed, their shoot sheets | 0090 | — | — |
| **import** | the formats' evaluation, then the importers (Lottie, Aseprite and TexturePacker sprite sheets, DragonBones, Rive as measured), the sandbox, the licence card, mapping moves, copying into the project | 0089 | 50 (only if it must) | — |
| **stage** | the stage over the workspace (floating, docked, on top), the companion window, the streaming mode, the visualiser, the measured cost, the hour under OBS | 0091 | 51 (only if it must) | keymap 15 |
| **video** | video export (ADR 0088's paths), audio in sync, the dialog, `export_video` for agents, packaging and CI | 0088 | — | — |

`dancer`, `cast`, `import` and `stage` started from the contract. **`video` starts from the
commit that adds ADR 0088** (the measured decision: codecs, containers, licences and
patents, sync, a 30-second export). Everyone draws with the
contract's stand-in character until cast's first delivery is merged, and builds poses with
the contract's `pose_at` until dancer's is.

**Merges during the round.** The integrator merges each builder's *first delivery* into
`r35/integration` as soon as it is reported, and tells the others to merge it into their
branch (`git merge r35/integration`): cast's characters, dancer's live feed and
`pose_at`, import's first format, stage's stage. Say in your report when a delivery is
ready to merge.

**Build slots.** Five builders on a 31 GB machine: every cargo command and every WSL check
goes through `bash /f/projects/daw-r35/slot.sh …` (three build slots, one WSL slot — read its
header). Waiting for a slot is normal; run long commands in the background.

**Disk.** F: had 137 GB free at the start and C: 38 GB (the WSL image). In your worktree never
run `cargo test --workspace` — test the crates you touch (`-p`, and `--test <name>` where you
can); the integrator runs the whole suite. Delete your WSL clone's `target/` as soon as your
check is done.

## Who owns what (one owner per hot file)

| File(s) | Owner | Others |
|---|---|---|
| `cantus-core/src/inst/dancer.rs` (keys are forever: change one only with the integrator), `InstrumentKind::Dancer` and every dispatch arm it needs (`cantus-core/src/inst/mod.rs`, `cantus-engine/src/inst/mod.rs`, `cantus-ui/src/views/inst/{frame,face}.rs`, `kit/skin/instruments.rs`, `cantus-ai/src/tools_{args,music}.rs`, `cantus-bench`), `cantus-engine/src/dance.rs`, `cantus-engine/src/inst/dancer.rs` (new), `cantus-dance/src/pose.rs` and `choreo*.rs` (new), `cantus-ui/src/dance_live*.rs` (new), `views/inst/dancer*.rs` (new), the `dance` locale area | dancer | cast: the one line `DEFAULT_CHARACTER` (marked) |
| `cantus-project/src/format.rs` (`SCHEMA_VERSION`, migrations), `fixtures/` | dancer (49); import (50) and stage (51) only if they must | the integrator renumbers if needed |
| `cantus-dance/src/cast/**`, `cantus-dance/src/rig*.rs` (new), `cantus-dance/src/render.rs` (speed only — its signatures are the contract's), `cantus-ui/examples/shoot_cast*.rs`, the `cast` locale area | cast | — |
| `cantus-dance/src/import/**`, `cantus-dance/tests/data/**` (fixtures you make), `cantus-ui/src/dance_import*.rs`, `views/dance_import*.rs`, the `dance_import` locale area | import | — |
| `cantus-ui/src/stage*.rs`, `views/stage*.rs`, the companion and streaming windows, the visualiser, `keymap.rs` (keymap 15), `layout.rs`'s Stage tab, `cantus-ui/src/dance_texture.rs` (the contract's helper; additions only), `tools/obs/**`, the `stage` locale area | stage | dancer and video use `dance_texture` |
| `crates/cantus-video` (new, GPL: encoders, containers, the frame pipeline), `cantus-ui/src/video_export*.rs`, `views/video_export*.rs`, `cantus-ui/src/ai_tools_video.rs`, packaging (`xtask/src/package*.rs`, `nsis.rs`, `packaging/**`) and workflows (`.github/workflows/*.yml`) for what video needs, the `video` locale area | video | marked lines |
| `cantus-ui/src/app.rs`, `boot.rs`, `cantus-ui/src/lib.rs`, `views/mod.rs`, menus (`views/menu*.rs`), `cantus-engine/src/processor*.rs`, `render.rs` | nobody's: **marked lines only** | — |
| `cantus-ui/src/settings.rs` | anyone, **additive fields only**, `#[serde(default)]` | — |
| `crates/cantus-core/tests/undo_invariants.rs` | dancer, **in its own `srng` stream and helper fn**; never change `below(49)` | — |
| `cantus-ui/src/ai_tools.rs` (the registry) | nobody; video registers `ai_tools_video` by one marked line | — |

**Marked lines.** When you must call your code from a file another builder owns (or nobody
owns), put your code in a new file of your own (declare it with `#[path]` inside one of your
files where you can, so shared `lib.rs`/`mod.rs` stay untouched) and add the fewest lines
that call it, each marked `// r35:<your name>`. List every marked line in your report.
Never reformat or move code in a file you do not own.

## The contract (decided; the commit holds the code)

- **The dancer is a native instrument that makes no sound** (`cantus_core::inst::dancer`,
  ADR 0087): a track hosts it, so its numbers are `AutoTarget::Instrument { track, key }` —
  automation clips, modulators (ADR 0057), MIDI learn and the unit catalogue reach them with
  no new path; notes on its track choose moves through the move map (`BASE_KEY` + the
  index in `Move::ALL`, sixteen keys); `move` = 0 dances the character's routine to the
  beat. Its keys, `Move`'s and `Mood`'s order and `BASE_KEY` are **forever** once shipped.
  The `react`/`level` pair is how it reacts to a track: "React to ‹track›" adds a
  **follower modulator** (ADR 0057) on the dancer's track, listening to that strip, routed
  to `level`, and sets `react` — one undo step; no second level path exists.
- **The engine is the one source of what it shows** (`cantus_engine::dance`): the unit runs
  like any instrument, outputs silence (a track with a dancer renders bit-identical to the
  track without it — the null test), allocates nothing, and at each block's end publishes a
  `DanceFrame` into its `DanceFeed` (atomics; the interface reads `latest()`). The feed is
  made with the unit off the audio thread and handed over through a new
  `InstrumentUnit::dance_feed() -> Option<Arc<DanceFeed>>` (default `None`) before the unit
  goes to the engine. **Offline** (`set_offline(true)`), the feed also keeps every frame;
  the video export renders the song with the export's own render and samples that timeline
  at its frame times. The screen shows what is **heard** (ADR 0022: the output's latency).
- **The pose is a pure function** (`cantus_dance::pose::pose_at(frame, character, wall)`):
  no state between calls, phases counted from the song's grid (the bar for the routine and
  an automated `move`, the note's start for a note) — the same song position gives the same
  pose at 30 or 144 frames a second and in a video. Stopped, a character breathes on the
  wall clock. Deterministic maths: `cantus-dance` follows `clippy.toml`'s rule
  (`cantus_dsp::math::*`), so poses and pixels match on every system.
- **One renderer** (`cantus_dance::render`, ADR 0087 §3): tiny-skia on the processor, into
  a pixmap at the exact physical pixel size it is shown or encoded at — crisp at any scale,
  identical on screen, in previews, in the companion and in a video. A character draws in
  its own units (origin at its feet, y down, one unit tall); the renderer places it
  (position, size, lift, spin, facing). The interface shows a stage through
  `cantus_ui::dance_texture::DanceTexture`. Measured budget: **one character on a
  600×600-pixel stage draws in ≤ 1.5 ms on this machine (release)**; cast holds it with a
  bench, stage holds the whole stage's cost on screen.
- **A character** is `cantus_dance::Character` (`info`, `has_move`, `move_beats`,
  `routine`, `bounds`, `faces_itself`, `draw`): included and imported alike. Every
  character answers every `Move` (its own or the move's `fallback`) and every `Mood`.
  Stored as the `character` text key: `builtin:<id>` or `file:<path relative to the project
  folder>`; a missing one draws the default character and says so — never an error.
- **Imports are data, never code** (ADR 0089): no script runs (Lottie expressions, Rive
  scripting, anything executable — refused or ignored, and said), nothing is fetched,
  nothing outside the file's own package is read, every size is bounded before memory is
  given, parsers are fuzzed; the file is copied into the project's folder; its licence is
  shown before use (`Licence::Unknown` says "make sure you may use it").
- **Video** (ADR 0088): the measured decision is committed before the video builder starts.
- **Licences (ADR 0047)**: everything of this round lives in GPL crates (`cantus-dance`,
  `cantus-ui`, `cantus-export`) except what is the engine's and generic: the silent unit
  and its feed (MPL, ours). A new dependency needs an approved licence and a notice.

## Rules (the usual ones, short)

- Work only in your worktree. Commit WIP often (`r35/<name>`); a cut session resumes.
  Write files in pieces under ~300 lines per tool call.
- Machine: `export PATH="$HOME/.cargo/bin:$PATH"` (cargo is not on PATH); your untracked
  `.cargo/config.toml` sets `jobs = 3`, no debug info and incremental off (never `git add`
  it); `TMP`/`TEMP` = `F:\projects\daw-r35\tmp`. Every cargo command through `slot.sh`. Test
  executables named `*setup*`/`*update*` need elevation on this machine: run them copied to
  a neutral name. A corporate proxy (Zscaler) may hold a download ~10 minutes and answer with
  an HTML page meanwhile: check what arrives (a zip starts with `PK`, an exe with `MZ`, a
  JSON file is not `<html`), wait, retry — **never bypass the proxy or TLS checks**.
  Downloads go to `F:\projects\daw-r35\downloads\<name>\` — never into a worktree; nothing
  downloaded is committed unless its licence allows it and the brief says so.
- Windows: `cargo clippy` on the crates you touch with `--all-targets` and the tests of
  every crate you touch, with `RUSTFLAGS=-D warnings`; `cargo fmt -p <crate>` for each crate
  you touched before each commit (`cargo fmt --all` fails here with *os error 206*). The
  Bash tool is Git Bash: a heredoc delimited by `EOF` fails to parse — use `XEOF` or the
  Write tool. Linux, before you report: write a small script file and run it through
  `slot.sh --wsl wsl -d Ubuntu-24.04 -- bash /mnt/f/…/script.sh` (inline `bash -lc '…'`
  loses `$VAR`s), the script calling `CANTUS_CI_DIR=$HOME/ci-r35-<name> bash
  /mnt/f/projects/daw-r35/<name>/tools/wsl/check.sh r35/<name> clippy` (and `test` for the
  crates you touched) — your own clone, never `~/cantus-ci`; **delete that clone's `target/`
  when it finishes**. Code behind `#[cfg(windows)]` leaves Linux-only dead-code warnings. UI
  tests that open windows must pass headless (`env -u DISPLAY -u WAYLAND_DISPLAY`).
- **Real time (ADR 0005):** the audio thread never allocates, locks, blocks or does I/O
  (`rt_memory.rs` counts it — add your paths to it); `cantus-engine` is
  `forbid(unsafe_code)`; OS/unsafe code only in `cantus-rt`. Never let a spinning thread run
  inside MMCSS.
- **Null tests first**, before wiring: a feature at its default touches no sample, to the
  bit (the golden hashes do not move; `DSP_VERSION` stays 5); block-size independence at
  tolerance 0; offline render equals real time; parallel (`CANTUS_WORKERS`) equals serial.
- **Start-up wiring goes in `boot`'s stages / `CantusApp::assemble`**, never only in
  `CantusApp::new` (the shipped app never calls it), with a test that fails if it leaves
  that path. Reviewers ask "is this reachable from `cantus_ui::run`?".
- UI: the hand-drawn kit (ADR 0024) — no stock egui widgets in views, no literal colours,
  no `FontId::new`. FL Studio is the visual reference, with Ableton's and Bitwig's ideas
  where they are better; clear label/value units, nothing that overlaps, panels resizable,
  every overflowing area scrollable, both themes (Slate and Paper — and the theme family),
  75–200 %, keyboard focus and the screen reader (AccessKit) as elsewhere, the frame budget
  held (`tests/frame_budget.rs`, `workspace_budget.rs`: add the dancer on screen to them).
  **Look at it**: shoot what you build and put the best in `docs/round35/<name>/`.
- A model change is a schema bump (`SCHEMA_VERSION`, a migration even if it writes nothing,
  `cargo run -p cantus-project --example write_fixture` for `fixtures/vN.cantus`; every old
  fixture keeps loading); every `Command` has an exact inverse, an undo-fuzz arm and a
  `cmd.*` string (both locales; the history panel names it); a new default binding is a
  keymap bump with its migration.
- **One build works everywhere** (WASAPI and ASIO; Linux; macOS): no feature-gated editions.
  Code for macOS compiles on the macOS runner (`macos.yml`); mark what only a real Mac can
  confirm for `docs/mac-session.md`.
- Strings in both locales (your area file); the manual (`docs/manual/`, EN and ES) for
  every feature a user touches; your ADR; `docs/round35/<name>.md`: what was done, how it
  was tested, the numbers, the marked lines, and **only** what needs the owner.
- **Original art only.** Nothing of FL Studio's Fruity Dance, nor any known mascot or
  character, nor anything traced or adapted from someone's art. Family-friendly. Test files
  for importers are made by us (generated by code or drawn by us) unless their licence is
  CC0/MIT/Apache/BSD and stated at the source.

## dancer — the instrument end to end (ADR 0087, schema 49)

Read: BACKLOG §6i; ADRs 0023 (native instruments), 0057 (modulators), 0068 (MIDI learn),
0022 (what is heard), 0082 (the tools and the unit catalogue); `cantus-core/src/inst/mod.rs`
and one small instrument (`organ.rs`) on both sides; `cantus-engine/src/inst/mod.rs`
(`InstrumentUnit`, `make`, `units_for`, `set_offline`, `set_position`/`follows_song`);
`cantus-ui/src/views/inst/frame.rs` (the instrument window's dispatch); the contract's files.

**First delivery (others merge it — make it early and solid):**
1. `InstrumentKind::Dancer` wired everywhere a kind is dispatched (READY once it works end
   to end), schema 49 (`InstrumentSpec::Native` with the new kind; a migration that writes
   nothing; `fixtures/v49.cantus` with a dancer track holding automation of `move`); its
   `TrackIcon` (a new variant if the existing six say nothing right — then it is part of 49).
2. The engine's unit (`cantus-engine/src/inst/dancer.rs`): silent, `at_rest`, no voices,
   `follows_song` so it knows the position, notes and held keys tracked, a `DanceFrame`
   published at each block's end; `dance_feed()` on the trait; offline keeps the timeline.
   **Null tests first**: a project with a dancer renders bit-identical to the same project
   without it (whole song, with automation and notes on the dancer); block-size
   independence; parallel = serial; `rt_memory` counts nothing on its path.
3. The live feed on the interface (`dance_live.rs`): for every dancer track, the feed of the
   unit the live engine runs (taken when `units_for` makes it — wired where the app sends
   units to the engine, marked lines), read once per frame, at the **heard** position; a
   project switch and a track's removal drop their feeds. An API the stage draws from:
   `DanceLive::dancers(&app) -> Vec<LiveDancer { track, character: Arc<dyn Character>,
   pose: Pose }>` (the character resolved from the `character` key — `builtin:` from the
   cast, `file:` from import's loader, a cache by id).
4. `pose.rs` grown into the real choreography (`choreo*.rs` beside it): the routine per bar
   at the character's style, a note held plays its move from its start and finishes its
   cycle, the move map, travel (`step`), jumps (`jump` + the Jump move's own arc), spin and
   facing, energy and `react`/`level`, blends between moves (a pure function: blend from
   the previous move only where the start of the current one is known — a bar line, a
   note), velocity → energy of a note's move. Every rule tested as a pure function.

**Then, the rest:**
- **The instrument's window** (kit, FL-like, `views/inst/dancer*.rs`): a live preview stage
  of the character dancing; the **character picker** — every included character with an
  animated preview (and imported ones, through import's loader), names from the `cast`
  area, licence line, *Import character…* (import's flow); the numbers as kit controls in
  label/value units (move and mood by name, speed as ¼×…4×, size ×, position, hue °…);
  the move map as a keyboard strip naming each key's move (like Pulsus's pads); *React to
  ‹track›* (the follower modulator, one undo step) with its depth; where it shows
  (`view.mode`, `view.top`) — the stage builder draws it.
- **Adding one**: the instrument picker (a "Visual" group — no sound), the track's
  default name and icon, a new dancer's character = `DEFAULT_CHARACTER`; a session clip's
  notes move it too.
- **MIDI learn** of any of its numbers (ADR 0068, nothing new expected — prove it with a
  test), **modulators** on them (an LFO on `hue`, a step sequencer on `move`) and
  **automation clips** — a test: an automation clip of `move` drawn over bars 3–4 changes
  the pose exactly from bar 3's first beat to bar 5's (frame times at 48 kHz, 30 fps).
- **Agents**: the unit catalogue lists the dancer with its keys, units, moves and moods by
  name and the move map (`list_units`/`get_unit`); `add_track` can make one (the
  instrument choice "Dancer"); an in-process MCP client test makes a dancer, automates
  `move` and reads it back; the `tools/list` golden regenerated if it changes.
- The undo fuzz: every new command in your own `srng` stream; `cmd.*` strings.
- **Done when**: an included character dances to the beat with no automation in a new
  project (a test on frames and poses, and a shoot of it); an automation clip changes its
  moves exactly on the bars drawn; notes play moves; React to works; an agent moves it; the
  null tests hold.

## cast — the characters Cantus includes (ADR 0090)

Read: BACKLOG §6i; `research/ui/` (FL Studio's look, our kit's tokens: `kit/tokens.rs`,
`theme*.rs`), the contract's `cantus-dance` (character, pose, render, the stand-in).

**The family.** Six original characters (not fewer than five; quality first), one family
look — the same line weight, the same construction (soft rounded shapes, a clear silhouette
at 48 px, big readable eyes), each with its own personality and dance style — for the whole
family, warm and kind. The first (a new dancer's, `DEFAULT_CHARACTER`) is **a songbird**
(Cantus means *song*). Then your designs, for example: a living eighth note, a little
metronome robot, a lamb, a fox or a cat, a child (simple and friendly) — propose, draw,
look, refine. Ids are
lowercase ASCII and forever (`builtin:<id>`); names and a one-line blurb in the `cast`
area (`cast.<id>.name`, `cast.<id>.blurb`) in English and Spanish. Never anything of FL's
Fruity Dance or a known mascot.

**The bar (the owner, 2026-10-31: "super, super professional — something really cool and
fun").** The level of the best 2D animated characters in today's apps and games, not a
programmer's drawing: confident shapes and silhouettes, a real colour palette per character
(base, shade, light, accent), soft shading and a rim light, clean outlines of varying
weight, a soft contact shadow, expressive eyes and mouths, props that say who it is
(headphones, a scarf, a little hat…). And **fun**: personality in every move — a wink on a
beat, a little hop at the end of a phrase, a cheeky look at the camera; beat-reactive
extras drawn with it (sparkles or little music notes on strong beats, a dust puff on a
landing, confetti on a big jump — subtle at low `energy`, joyful at high), never noisy. The
stand-in in the contract is the *floor*, not the target. Draw, shoot, look, compare with the
bar, redraw — at least three passes per character, said in your report with before/after
images. The integrator shows the owner the GIFs before a character counts as done.

**Each one:**
- **Every `Move` of its own** (16) with real animation craft — anticipation, follow-through,
  squash and stretch, overlapping action, arcs, secondary motion (a tail, a crest, ears)
  from `pose.seconds`/`phase` (never a clock), easing; the steps land **on the beat**
  (phase 0 is the beat), alternate sides by `pose.cycle`; a `routine()` of its own style;
  `move_beats` per move; blends between moves look right.
- **Every `Mood`** (8) on its face, and blinks (`pose.blink`).
- **Colour**: its palette turns with `hue` (`Canvas::color`), eyes and outlines stay
  (`Canvas::fixed`); readable on Slate and Paper and on the theme family's stages, and
  **on a transparent background over anything**; **no colour near the chroma keys**
  (`render::CHROMA_*` — a test measures every fill's distance at `hue` 0).
- `bounds()` honest (a test draws every move at 4 phases and finds no pixel outside),
  `faces_itself` if it draws a three-quarter view for `turn`.
- **Crisp at any scale**: the same drawing at 48 px and at 2 160 px (a test and a shoot).
- **Fast**: a bench — every character, every move, on a 600×600 stage, release: ≤ 1.5 ms a
  frame (mean) on this machine; optimise the renderer (`render.rs`'s internals, never its
  signatures) and the rig if needed. Report the numbers.

**Also:** the rig and drawing helpers (`rig*.rs`: limbs as curves, joints, easing, shapes
with an outline and a soft shade, a shadow under the feet that shrinks with `lift`);
`cast::builtin()`'s order is the picker's; replace the stand-in (keep `placeholder.rs` only
if a test needs it). **Shoot sheets** (`cantus-ui/examples/shoot_cast.rs`): every
character × every move at 4 phases, every mood, both themes, transparent over a checker,
three sizes; the best in `docs/round35/cast/`. **First delivery** = the songbird with every
move and mood (the others merge it); then the rest of the family.

**Done when**: six characters, each with every move and mood, crisp, fast, on both themes
and transparent, at the bar above; the sheets and an animated GIF per character (the
routine over eight bars, `crates/cantus-dance/examples/preview.rs` grown to every character)
looked at and refined (say what you changed after looking).

## import — characters people bring (ADR 0089)

Read: BACKLOG §6i; ADRs 0030 and 0042 (content and licences), 0047; the contract's
`cantus-dance` (character, pose, render, `import/mod.rs`); how a project carries its audio
files (*Save As* copying them: `cantus-ui/src/save_job.rs`, `samples*.rs`).

**First, the evaluation (ADR 0089), measured, in your first hours:** for **Rive**
(`.riv`; the runtimes' licences — `rive-runtime` C++, `rive-rs`; what building it here
costs; drawing it through our tiny-skia renderer or not; state machines and their inputs),
**Lottie** (the JSON; Rust options: `velato`'s model, our own player over tiny-skia;
expressions are JavaScript — never run), **sprite sheets** (Aseprite's JSON with frame
tags, TexturePacker's JSON hash/array; the free editors that write them: LibreSprite,
Piskel), **DragonBones** (JSON skeleton + atlas, MIT runtimes), and on paper **glTF 2.0 /
VRM** (later: BACKLOG) and **Spine / Live2D** (their runtime licences against a free GPL
app — out unless they allow it). For each: the licence of the format and of what we would
use, what it can express, where people get characters (and their usual licences), the
attack surface, the cost. Decide which go in this round — **Lottie and sprite sheets are
required**; DragonBones and Rive go in if the measurement says they can be done well this
round (say why if not, with what it would take). Tell the integrator the decision before
building past it.

**Then, each chosen format as a `Character`:**
- **Lottie**: shape layers (groups, paths, rectangles, ellipses, stars, fills, strokes,
  linear and radial gradients, trim paths, repeaters, transforms with every keyframe
  easing), precomps, null parents, masks and track mattes (the common ones), image layers
  (embedded or inside the package only); text layers drawn if their glyphs are in the file,
  else skipped and said; **expressions ignored and said**. Its **markers** become moves
  (by name: `clap`, `wave`, `idle`… and Spanish synonyms), else the whole timeline is one
  loop. `hue` turns its colours.
- **Sprite sheets**: Aseprite (`frameTags`, frame durations, `pingpong`/`reverse`, slices
  ignored) and TexturePacker (hash and array; rotated and trimmed frames) with a PNG atlas;
  tags become moves; `hue` tints.
- **DragonBones / Rive** as decided: bones, slots, display frames, meshes as far as decided;
  Rive's state-machine inputs bound to our numbers (`move`, `energy`, `mood`…) by name.
- **Beat sync**: a move's frames span its `move_beats` at the song's tempo (the file's own
  frame rate is the pace only when the person asks for *its own speed*).
- **The sandbox**: no script, no network, nothing read outside the file's folder (`..`,
  absolute paths, URLs refused), every size bounded before allocation (bytes, JSON depth,
  layers, keyframes, frames, image pixels, nesting), decoding errors in words; a fuzz test
  per parser (random mutations of valid files never panic, never exceed the bounds).
- **The import flow** (kit, `views/dance_import*.rs`): *Import character…* from the
  dancer's picker → the file is checked, a **card** shows the preview dancing, the format,
  the author and **the licence** (stated in the file, a `LICENSE`/`license.txt` beside it,
  or the person's choice from a short list — kept with the copy), the moves found and a
  **mapping** of our sixteen moves to the file's animations (guessed by name, every one
  editable, a fallback shown) → *Use* copies it into the project's folder
  (`Characters/<name>/…`), sets `character` to `file:<path>` (one undo step), and *Save As*
  carries it. A loader with a cache for the dancer: `import::load(project_dir, rel) ->
  Result<Arc<dyn Character>, ImportError>`.
- **Test files are ours**: generate Lottie, sprite sheets and DragonBones files by code (a
  little character with every move as markers/tags/animations), plus any file whose licence
  is CC0/MIT/Apache/BSD at the source (record where). A Rive file only if one with such a
  licence exists or can be made here.
- **Done when**: an imported Lottie character and an imported sprite sheet (and the others
  decided) dance with their moves mapped to our numbers — the routine, automation of
  `move`, notes — in a test and a shoot; a hostile file never panics or escapes its folder;
  the licence is shown and kept; the project carries the character.

## stage — where it dances: over the workspace, the companion, streaming (ADR 0091)

Read: BACKLOG §6i; ADR 0024 (the kit; §5 the workspace's docks and floats), 0022 (what is
heard), 0052 (the top bar's meters); `cantus-ui/src/layout.rs`, `lib.rs`'s `run` (eframe's
options), `boot.rs`; `kit/window_ctl.rs`; the contract's `dance_texture.rs` and
`cantus-dance`; the frame budget tests.

**First delivery (others merge it):** the **stage over the workspace**: every dancer whose
`view.mode` is floating draws over the workspace — a frameless, transparent area,
movable and resizable (its frame and grips appear on hover, kit-drawn: move, resize, pin on
top, pop out, hide), `view.top` keeping it above the other floating panels; docked (`1`)
as a **Stage** tab of the workspace's docks on the theme's stage colour; hidden (`2`)
nowhere. Positions within it from `x`/`y`, sizes from `size`. Drawn through
`DanceTexture` at physical pixels; redrawn only when a pose changed; capped (60 fps playing,
a slow idle breath stopped, nothing while hidden); the frame budget tests extended with a
dancer on screen and holding. Until dancer's live feed is merged, draw the stand-in from
the transport alone.

**Then, the rest:**
- **The companion window**: a small OS window (eframe's viewports, deferred), always on
  top, no decorations, **transparent** (per-pixel alpha), dragged by the character,
  resized by a corner; it dances **with whatever plays** without any automation — the
  transport, the master's level and its beat — a character chosen in it (or the project's
  dancers, if the person asks); right-click: character, size, background, on top, the
  visualiser, close. It **pauses** when minimised or hidden (no redraw, no CPU) and its
  cost is **measured and shown** (CPU ms a frame and % of a core; GPU time where the driver
  says, by a timer query, else "not measured" in words).
- **The streaming mode**: the companion as a capture window: a fixed, documented window
  title (`Cantus Stage`) and class that OBS and Streamlabs window capture find again every
  session; background **transparent**, **chroma** (green, blue, magenta) or the theme's;
  **fixed size and position** kept between sessions (settings, additive); nothing else of
  the interface in it; a *Copy OBS setup* help line (Game Capture with *Allow transparency*,
  or Window Capture + a Chroma Key filter). Measure which OBS capture methods keep the alpha
  on Windows (Game Capture, Window Capture WGC/BitBlt) and on Linux (Xcomposite, PipeWire)
  and say it in the manual.
- **Looks first-rank**: the stage, the companion and the streaming window are what people
  show on stream and in videos — polished like a broadcast overlay (clean type, a soft
  glow, the theme's accents, smooth motion, nothing jittery), never like a debug panel.
- **The light visualiser** beside the character (on/off, each part on/off): the master's
  **spectrum** (a few dozen bands, smoothed, kit style) or its **level**; **bar and beat**;
  the **song's name and tempo**. The spectrum needs the master's samples: copy them on the
  audio thread into a lock-free ring at most (no FFT there, nothing allocated — marked lines
  in `processor*.rs`, counted by `rt_memory`), analyse on the interface's side; or reuse an
  existing tap if one fits. Cheap: measured.
- **Shortcuts** (keymap 15, with its migration): show/hide the stage, open/close the
  companion; menu entries (View ▸ Stage, View ▸ Companion window, View ▸ Streaming mode).
- **The hour under OBS** (`tools/obs/`): copy the installed OBS
  (`C:\Program Files\obs-studio`) to `F:\projects\daw-r35\downloads\obs-portable\` and run
  it **portable** (`portable_mode.txt`) so the owner's own OBS settings are never touched;
  drive it with obs-websocket (built in): a scene capturing `Cantus Stage` with alpha; a
  song looping in Cantus; every few seconds a source screenshot checked — the corners
  transparent, the character's pixels moving with the beat; Cantus's CPU and memory sampled
  over **one hour**. Report the numbers and keep the script.
- **Done when**: the stage floats, docks, stays on top and hides, transparent over the
  workspace, within the frame budget; the companion dances with whatever plays and pauses
  when hidden; the streaming window is captured with alpha by OBS for an hour with its cost
  measured and shown; the visualiser is light and measured.

## video — the clip as a video with the song's audio (ADR 0088)

Read: **ADR 0088** (the decision) and the probe's report and code
(`F:/projects/daw-r35/videoprobe/REPORT.md`, `src/` — reuse what is good: the MP4/MOV/WebM
writers, the MF and OpenH264 drivers, the decode-back check; it is ours); ADR 0087 and the
contract's `cantus_engine::dance` (the offline timeline) and `cantus_dance::render`;
`cantus-export` (how audio export renders, encodes, names and places files, cancels and
reports progress), `cantus-content` (downloads on demand: `fetch.rs`, `manifest.rs`, `models.rs`
as the model), `cantus-ui/src/agent_export.rs` and the export dialog's views.

**First delivery:** `crates/cantus-video` (GPL) with the MP4 writer, the MOV (PNG) writer,
the PNG sequence + WAV, Media Foundation's H.264 + AAC driven directly (B-frames off, AAC
priming in the edit list, fast start), the frame pipeline (worker threads, ordered, bounded
memory) — and the **sync test** that decodes back (FFmpeg as a test tool only, kept out of
the repository and the packages; download it into `F:/projects/daw-r35/downloads/video/` or
reuse the probe's) at 30 and 60 fps: each beat's flash against its click within half a
frame. A 30-second 1080×1920 export measured.

**Then, the rest:**
- **The song's video**: the export's own audio render with the dancers' feeds offline
  (every dancer of the project, at its `x`/`y`/`size`, through `cantus_dance::render`), the
  range (the song, the loop, a selection, a clip's span), the poses sampled from the
  timeline at each frame's time — **a test**: an automation clip of `move` drawn over bars
  3–4 changes the video's poses exactly on those bars' frames; the video's frame at time t
  equals the screen's pose at t.
- **The dialog** (kit; File ▸ Export ▸ Video…, and from the dancer's window and the stage):
  format (MP4 for social, WebM, MOV with transparency, PNG sequence), shape (vertical
  1080×1920, square 1080×1080, landscape 1920×1080, custom), 30 or 60 fps, quality, the
  background (the theme's stage, transparent where the format allows, a chroma key colour,
  a solid colour), the visualiser and the song's title on or off (stage's visualiser drawn
  into the frames if it is merged by then — else the integrator hands it over), a live
  preview of the first frame and of a scrubbed frame, the time it will take, progress and
  cancel, the file named and placed as audio exports are; words for every refusal (Windows
  N, no OpenH264 yet, no system AAC on Linux).
- **Linux**: Cisco's OpenH264 through the content manager (hash checked, the off switch,
  Cisco's words shown, `CameraVideoRealTime`), AAC from a system ffmpeg/GStreamer if found,
  else MP3-in-MP4 with the warning. **macOS**: AVFoundation/VideoToolbox + AudioToolbox
  behind `cfg(target_os = "macos")`, compiled on the runner, rows added to
  `docs/mac-session.md`. **WebM**: rav1e + Vorbis (tiles, threads; measure nasm's gain in
  CI and decide; the first packet's offset fixed to 0 ms).
- **Agents**: `export_video` (Files class, a job with progress, one file out) in
  `cantus-ui/src/ai_tools_video.rs`, registered by one marked line in `ai_tools.rs`; the
  `tools/list` golden regenerated.
- **Packaging and CI**: nothing new shipped on Windows/macOS; the notices
  (`cargo run -p xtask -- notices`) for every new crate (rav1e and its tree, `png`,
  `openh264` if used, `windows`' features); CI builds `cantus-video` on all three systems.
- **Done when**: a 30-second clip of a dancing character exports as MP4 (Windows: Media
  Foundation; Linux: OpenH264 path), WebM, MOV with alpha and a PNG sequence, each decoded
  back in sync (the test), measured (time, size, memory), looked at (frames shot into
  `docs/round35/video/`), and the dialog is FL-class.
