# Round 29 — a beta that competes with FL Studio and Ableton in what people use every day

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians
3:23). Tests before code, nothing half done, look at what you build (`shoot` examples).
**The owner's rule, this round and every round:** every gap a feature leaves is closed in
the same round. Whatever you would write under "Left" in your report comes back to you as
work — so finish end to end (reachable from the real app, drawn, translated, documented,
tested), and only list as open what truly needs the owner (their ears, their hardware, a
decision, a lawyer).

The ask (2026-10-16), six items:

1. **Warp / time-stretch and pitch-shift of audio clips**, first-rank quality (élastique
   class; our own implementation by default, on round 28's voice base and a phase-locked
   STFT; licences reviewed before anything external is used): warp markers on the clip,
   modes (rhythm, melodic, complex), tempo and transient detection at import, the clip
   follows the project's tempo, audio quantised to the grid; offline = real time;
   non-destructive (a schema change with its migration).
2. **MIDI learn and controller mapping**: any parameter (native, hosted plug-in, mixer,
   transport) to a knob, fader, button or pad; absolute / relative / toggle, range and
   curve, soft takeover; maps saved per project and globally; a view of the mappings; a
   move records automation while recording.
3. **Isolate third-party plug-ins**: out-of-process hosting for CLAP and VST3 (ADR 0031
   phase 6), so a plug-in that crashes does not take the session down; the user is told,
   can reload, and the project loses nothing; latency and CPU measured against in-process
   hosting, with an option per plug-in.
4. **A tempo track with ramps and metre changes along the song, and named markers** (jump,
   loop, export between markers); the whole engine, automation, modulators and synced
   effects follow the tempo map; MIDI and DAWproject export with the tempo map.
5. **Freeze tracks, bounce in place, and low-latency monitoring while recording** (BACKLOG
   §4c steps 1 and 2): freezing frees CPU and unfreezes without loss; bounce in place to
   audio with or without effects; an armed track is monitored without the latency of other
   tracks' plug-ins.
6. **The owner's frictions** — none were given at the start; any that arrive mid-round are
   added below and given to a builder.

Read, in order: this file (the rules and **your** section completely); `CLAUDE.md`;
`docs/STATUS.md`'s top ("The next session" and round 28's section);
`docs/BACKLOG-excellence.md` at the sections your brief names; the ADRs your brief names;
**the integrator's map of your area** in `F:\projects\daw-r29\maps\` (outside the
repository: `time.md`, `audioclips.md`, `midiparams.md`, `hosting.md`, `freeze.md`,
`ui.md`, `contract.md` — exact files, lines, call sites and risks, written for you; check
the line numbers, they were taken at `e82ff86`).

## The builders

Never more than three builders compile at once (a 31 GB machine). Wave 1 starts from the
contract commit; each wave-2 builder starts from `r29/integration` as it is when a slot
frees (so it already holds the wave-1 work merged before it).

| Builder | Wave | Branch / worktree | Delivers | Schema | ADR |
|---|---|---|---|---|---|
| **tempo** | 1 | `r29/tempo`, `F:\projects\daw-r29\tempo` | item 4: the tempo track, ramps, metre changes, markers; everything follows the map; MIDI and DAWproject | 33 | 0060 |
| **warp** | 1 | `r29/warp`, `F:\projects\daw-r29\warp` | item 1: the stretcher and shifter, analysis at import, warp markers, modes, quantise; part B (playback under the tempo map) after tempo merges | 34 | 0061 |
| **isolation** | 1 | `r29/isolation`, `F:\projects\daw-r29\isolation` | item 3: out-of-process CLAP/VST3, crash notices and reload, per-plug-in option, measured | 36 | 0063 |
| **controllers** | 2 | `r29/controllers` | item 2: MIDI learn, controller maps, the mappings view | 35 | 0062 |
| **freeze** | 2 | `r29/freeze` | item 5: freeze, bounce in place, low-latency monitoring | 37 | 0064 |

The **integrator** (the session that wrote this) merges each branch into `r29/integration`,
runs adversarial reviews on it (several lenses, a refuter per finding — reviewers always
ask *"is it reachable from `cantus_ui::run` → `boot::Boot` → `CantusApp::assemble`?"*),
sends every confirmed finding and every "Left" item back as work, runs the whole suite +
fuzz + clippy on Windows and Linux, writes STATUS, the manual and the release notes, builds
the test version, and pushes to `master` only when everything is green (the owner's
permission, given in advance for this round).

**The contract commit** (on `r29/integration`) declares:
- **`cantus_core::tempo`** (`crates/cantus-core/src/tempo.rs`): `TempoPoint`,
  `MetreChange`, `MarkerId`, `Marker`, and **`TempoMap`** — `constant`, `new`,
  `is_constant`, `points`, `metres`, `bpm_at`, `seconds_at`, `tick_at_seconds`,
  `sample_at`, `tick_at_sample`, `seconds_between`, `next_change_after`, `signature_at`,
  `bar_start`, `bbt`, `bar_lines`, `span_key` — and `Project::tempo_map()`. The bodies read
  only the first tempo and metre (what a project without a tempo track is) and give the
  same `f64`s as `Ticks::to_seconds` to the bit. **Every builder codes against this API
  from day one** (warp keys renders on `span_key`, freeze measures lengths with it,
  controllers edits the tempo through it); the tempo builder replaces the bodies. Add a
  method if you need one and say so; never change a signature without the integrator.
- **Empty locale areas** `tempo`, `warp`, `controllers`, `isolation`, `freeze`
  (`crates/cantus-ui/locales/areas/<area>.{en,es}.txt`, registered in `i18n.rs`).
- **Undo-fuzz arms 43–47** (`crates/cantus-core/tests/undo_invariants.rs`): `rng.below(48)`,
  one arm per builder calling `round29_<area>_command` in a section at the end of the file
  (today each renames the project). Fill **your** helper, add your coverage counter and its
  assert in the same section's style, and never touch another arm.
- This file, with the rules, the file owners and the five briefs.

## Rules (as in rounds 20–28)

- Your worktree and branch only. The untracked `.cargo/config.toml` is there: `[build]
  jobs = 3`, `debug = 0` and `incremental = false` for dev and test, and `TMP`/`TEMP` on
  `F:\projects\daw-r29\tmp` (**C: is nearly full** — never write big files there). **Never**
  touch `F:\projects\cantus` (the owner's tree, with the owner's uncommitted files), never
  push, never merge, never change a remote, **never `git add` the `.cargo/` folder**.
- **Commit WIP early and often** on your branch (sessions get cut by API errors; you will
  be resumed with your context, or a fresh agent will continue from your commits). Write
  files in pieces under ~300 lines per tool call.
- Shell: Git Bash has no cargo on `PATH` — prefix `export PATH="$HOME/.cargo/bin:$PATH" &&`.
  A heredoc delimited by `EOF` fails to parse: use `XEOF` or the Write tool. UAC refuses
  test binaries named `*setup*` / `*update*` (os error 740): run a copy under a neutral
  name from the crate folder. The Zscaler proxy may answer a download with an HTML page for
  ~10 minutes: verify what arrives (a zip starts with `PK`, gzip with `1f 8b`), wait, retry;
  **never bypass the proxy or TLS checks**.
- **RAM:** do not start a second `cargo` while one runs; prefer `-p <crate>` (and
  `--test <name>`) over `--workspace` until the end; the whole workspace once, at the end.
- **The base is fmt-clean and CI-clean**: run `cargo fmt --all` before your last commit;
  `cargo clippy --workspace --all-targets -- -D warnings` must be clean (and
  `cargo clippy -p cantus-plugins --no-default-features --all-targets -- -D warnings` if you
  touch the plug-ins). Code behind `#[cfg(windows)]` must not leave Linux dead-code
  warnings (CI is Linux too; `docs/DEVELOPING.md` §4); anything OS-specific gets a Linux
  twin or a clean stub.
- **Tests first**, each seen failing before the code that makes it pass. Every mutation is a
  `Command` (ADR 0004) with its exact inverse, a `cmd.*` string in your area file (both
  languages; a missing key shows as the raw key — no test catches it, reviewers will), and
  coverage in your fuzz arm (43–47).
- **Null tests before DSP or engine wiring**: a control at its default touches no sample, to
  the bit; block-size independence at tolerance 0 (1, 37, 64, 512 frames); offline render
  equals real time; parallel equals serial. **A project that does not use your feature
  renders bit-identically to today** (no golden hash moves: `null_tests.rs` `GOLDEN_V5`,
  `inst_pulsus.rs`, `send_tap.rs`, the null-hash corpus). A golden moves only on purpose,
  with `DSP_VERSION` and the measured size of the change in one commit — ask the
  integrator first.
- **Deterministic maths:** `cantus_dsp::math::*`, never std's transcendentals (clippy
  enforces it); every random choice is a hash of a seed.
- **Real time (ADR 0005):** the audio thread and the pool workers never allocate, lock,
  block or do I/O (`rt_memory.rs` counts it — add your scenario). `cantus-engine` is
  `forbid(unsafe_code)`; OS/unsafe code lives only in `crates/cantus-rt` (MPL) and, for
  FFI, `cantus-host` / `cantus-plugins` / the new sandbox crate (GPL) — every `unsafe` with
  a `// SAFETY:` line. Never let a spinning thread run inside MMCSS.
- **A model change is a schema bump** with the number in the table above: `SCHEMA_VERSION`,
  `migrate_N_to_N+1` (even if it writes nothing), a `// Schema N (round 29, <you>)` block
  in `crates/cantus-project/examples/write_fixture.rs`, `fixtures/vN.cantus`, and a
  `schema_N_…` test in `format.rs`'s style; every old fixture keeps loading and rendering to
  the bit. New fields `#[serde(default, skip_serializing_if = …)]` so a project that does
  not use them saves the same bytes. The integrator chains the numbers at merge.
- **Start-up wiring goes in the boot path.** The shipped app never calls `CantusApp::new`:
  start-up is `cantus_ui::run` → `boot::Boot` stages → `CantusApp::assemble`. Anything
  wired at start (a service, a cache, a file read, a setting applied, a MIDI tap) is wired
  there, with a test that fails if it leaves that path (`tests/pitch_boot.rs` and
  `the_boot_font_stage_arms_the_fallback` are the models).
- **Kit only in views** (ADR 0024): no stock egui widgets, no literal colours, no
  `FontId::new`; add every new view file to `ON_THE_KIT` in `crates/cantus-ui/tests/fx_lint.rs`.
  Labels in clear label/value units, nothing that overlaps, every overflowing area
  scrollable, FL Studio as the visual reference. New glyphs go in `kit/glyph.rs` (your own
  variants, appended) and show in `examples/shoot_glyphs.rs`. **Look at what you build**
  with a `shoot_<you>` example: 75 % and 200 %, English and Spanish, Cantus Night and Paper,
  a narrow window; PNGs go to `F:\projects\daw-r29\shots\<builder>\`, never into the
  repository. Index-matched menus (`arrangement_audio_menu.rs`, `track_menu`) break when two
  builders insert rows: add your entries as a separate `menu_list` call from your own file,
  chained like `pitch_lane::track_menu` (`-> bool`).
- **Strings:** your locale area only (`locales/areas/<area>.{en,es}.txt`), both languages,
  good Spanish. Never edit `en.txt` / `es.txt`; `i18n.rs` already has your area.
- **Keymap:** new actions are allowed (`keymap.rs`: the `Action` variant, `ALL`, `id`,
  `category`, the default binding if any — no collision, the keymap tests say so — and an
  arm in `run_action`, preferably through a pre-hook in your own file like
  `pitch_action`). `FILE_VERSION` becomes **9**: the first builder who adds an action bumps
  it and adds `NEW_IN_V9`; the others append to that array. The integrator reconciles.
- **Licences (ADR 0047):** `cantus-core/dsp/engine/project/rt` are MPL-2.0 — never copy
  GPL/LGPL code into them (read GPL references, never port them); MIT/BSD/Apache/MPL code
  may be used with its notice (`cargo run -p xtask -- notices`). A new crate needs an
  approved licence and its notice; prefer our own implementation, and record the licence
  review in your ADR before using anything external.
- **Manual and release notes:** write your section of the manual in both languages
  (`docs/manual/{en,es}/README.md` — your own subsection, appended where your brief says)
  and list in your report the known-issue lines of `docs/releases/v0.1.0-beta.1.md` and the
  manual's §14 your work retires; the integrator edits those.
- **Finish:** your crates with `cargo test -p <crate> --no-fail-fast` (UI work: `-p
  cantus-core -p cantus-project -p cantus-ui` at least), the engine's fuzz if you touch the
  engine (`CANTUS_WORKERS=3 cargo test -p cantus-engine --features fuzz`), clippy as above,
  `cargo fmt --all`; then `docs/round29/<name>.md` (done / measurements / tests and what
  each holds / files outside your area / *left* — empty or owner-only / notes for the
  integrator), your ADR, and a short report as your final message. Then delete nothing:
  the integrator deletes your `target/` after the merge.

## File owners (one owner per file; anything else: an additive hook, said in your report)

| Owner | Files |
|---|---|
| **tempo** | `crates/cantus-core/src/tempo.rs`, `time.rs`; the tempo/metre/marker fields of `Project` (`model.rs`, additive) and their commands (`command.rs`, one block delegating to `crate::tempo::apply_*`); `crates/cantus-engine/src/{transport.rs, processor_pass.rs, processor_snap.rs (the clock parts), processor.rs (song_position_of, heard_position), render.rs, stems.rs (lengths and stop), recorder.rs (tpf)}`, `graph/master_run.rs` (the click), `fx/mod.rs` (`SongPosition`), `cantus-dsp/src/modfx_clock.rs` (the synced phase), `modulator_eval.rs` (`ModTime`, gates), `modulator_bake.rs`, `crates/cantus-host/src/{backend.rs (TransportInfo), unit.rs (transport_of), clap/audio.rs, vst3/audio_thread.rs}` (the transport part only), `crates/cantus-project/src/{midi/*, dawproject/*}` (tempo, metre, markers), `crates/cantus-export/src/settings.rs` (`Scope`), a new `crates/cantus-core/src/audio_time.rs` (the one function that maps an **unwarped** clip's ticks to source frames, used by the engine and every UI site — warp extends it in part B), the UI's ruler/grid/BBT sites and a shared helper (e.g. `views/bars.rs`), `views/arrangement_{tempo,markers}.rs` (new), `views/transport_tempo.rs`, `transport_clock.rs`, `recovery*.rs` (the tempo they store), `cantus-ai/src/bridge.rs` (tempo), `locales/areas/tempo.*`, `examples/shoot_tempo.rs`, ADR 0060, `docs/round29/tempo.md` |
| **warp** | `crates/cantus-dsp/src/{stretch*.rs, onset*.rs, beat*.rs}` (new), `crates/cantus-core/src/warp*.rs` (new) and `AudioRegion::warp` (`audio.rs`, additive) with its commands (one block in `command.rs`), the analysis/render service (`crates/cantus-engine/src/{warp*.rs}` new; generalising `pitch.rs`/`pitch_attach.rs`/`pitch_disk.rs` is allowed and said), `samples.rs` (the attach chain, additive), `backend.rs` (`compile_for_render`'s wait), `crates/cantus-ui/src/{warp*.rs, views/warp*.rs, views/inspector_warp.rs}` (new), `views/arrangement_audio*.rs` (the warp display and gestures), `audio_clips.rs`, `audio_files.rs` (the import hook), `pitch.rs::ClipTime` (made warp-aware), DAWproject `<Audio>`/`<Warps>` (`dawproject/*`, after tempo's part is merged), `locales/areas/warp.*`, `examples/shoot_warp.rs`, ADR 0061, `docs/round29/warp.md` |
| **isolation** | `crates/cantus-host/**` except tempo's transport lines, a new `crates/cantus-sandbox/**`, `crates/cantus-rt/src/{ipc*.rs, shm*.rs}` (new), `crates/cantus-test-plugins/**` (the fault parameter), `crates/cantus-bench/src/` (a new isolation suite), `xtask/src/{package*.rs, nsis.rs, notices.rs (ROOTS), package_meta.rs}`, `installer/`, root `Cargo.toml` (members), `.github/workflows/*release*.yml` (build the child), `ExternalPlugin` hosting field (`crates/cantus-core/src/external.rs`, additive) and its command, `crates/cantus-ui/src/views/{plugins.rs, plugin_notice.rs (new), audio_notice.rs (the shared card stack)}`, `app.rs::host_notices` arms, `settings.rs` (the default, additive), `locales/areas/isolation.*` (and `host.*`), `docs/signing/code-signing-policy.md`, `docs/DEVELOPING.md` (the helper binary), ADR 0063, `docs/round29/isolation.md` |
| **controllers** | `crates/cantus-engine/src/midi.rs`, `crates/cantus-core/src/{controller*.rs, automation/set.rs}` (new) and the project's controller map (`model.rs`, additive) with its commands, `crates/cantus-ui/src/{controllers*.rs, views/controllers*.rs}` (new), `kit/automate.rs` (`Choice::Learn`/`Forget`, `Offer`), `views/modulator.rs::modulation_choice` (the new arms), `views/fx/controls.rs` (passing them), `views/mixer_menu.rs` (learn entries, typed `Act`s appended), `boot.rs` / `app.rs::Files` (the global map file and the MIDI tap — additive), `views/workspace*.rs` (the mappings pane), `settings.rs` (additive), `locales/areas/controllers.*`, `examples/shoot_controllers.rs`, ADR 0062, `docs/round29/controllers.md` |
| **freeze** | `crates/cantus-core/src/freeze*.rs` (new) and the frozen state in the model (additive) with its commands, `crates/cantus-engine/src/{freeze*.rs (new), graph/strip.rs, graph/strip_run.rs (the frozen source and the low-latency bypass), graph/plan*.rs (frozen inputs), pdc.rs, processor_rec.rs (monitoring), processor_snap.rs::compensate}`, `recorder.rs` (latency kept at start) after tempo's merge, `crates/cantus-ui/src/{freeze*.rs, bounce*.rs, views/freeze*.rs}` (new), `views/audio_chip.rs`, `views/record_options.rs`, `views/inspector_record.rs` (rows), `locales/areas/freeze.*`, `examples/shoot_freeze.rs`, ADR 0064, `docs/round29/freeze.md` |

`app.rs`, `keymap.rs`, `views/mod.rs`, `kit/mod.rs`, `lib.rs`, `arrangement.rs`,
`inspector.rs`, `mixer_strip.rs`, `command.rs`, `model.rs`, `snapshot.rs`, `messages.rs`,
`processor_msg.rs`: additive edits only (a field, a variant, a `mod`, an arm, a one-line
call into your own file) — list each in your report. Wave-2 builders start after tempo has
merged, so the clock files are tempo's code by then: build on them, do not rewrite them.

---

## tempo — the tempo track, metre changes and markers (BACKLOG §3 *Time*; item 4)

Read BACKLOG §3 (*Time*), §0.2 (*Sync*: what Link/MIDI clock will need from the map
later), STATUS's round-12 plan *"Time: markers, signatures, arrangements"* (around line
1769, and 1865/1889/1906), ADRs 0006, 0021 (MIDI/DAWproject), 0025 §1, 0027, 0052 (the
top bar), 0055 (the modulation family's clock), 0057 (modulators), and the map
`F:\projects\daw-r29\maps\time.md` — **its §3 is the migration scope: every call site that
assumes one tempo or one metre. All of them move to the map in this round**; a site left
on `project.tempo_bpm` is a defect the review will find. Compare what FL Studio
(tempo automation, time-signature markers, playlist markers with actions), Ableton (tempo
and time-signature changes, locators), Cubase/Studio One (the tempo track with ramps,
signature track, marker track, cycle markers) and Logic (tempo curves) offer, and take the
best of each.

**1. The model (schema 33).** The project gains its tempo points, its metre changes and its
markers (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`); `tempo_bpm` and
`time_signature` stay as the values at tick 0 and agree with the first point. Commands with
exact inverses for every edit (a point added, moved, removed, its value or ramp changed; a
metre change added, moved, removed, changed; a marker added, moved, renamed, removed),
merged per drag (`MergeKey`), validated (a metre change on a bar line, tempo within
`MIN_BPM..MAX_BPM`, ticks ≥ 0 except the first), your fuzz arm 43 with its counter.
**Decide in ADR 0060** what the top bar's tempo field edits when a tempo track exists (the
value at the playhead's segment is the usual answer; then say how tap tempo behaves) and
what a ramp is (linear in beats, as most hosts — Cubase, Studio One, Bitwig — draw it, or
in time); integrate a ramp analytically with `cantus_dsp::math` (or the core equivalent:
never std transcendentals).

**2. `TempoMap` for real** (`tempo.rs`): segments precomputed at compile (start tick, start
second, start bpm, slope), binary search, no allocation on lookups, the region before tick
0 extends the first tempo and metre (count-in), `span_key` covering exactly what decides
timing over a span. Property tests: `seconds_at`/`tick_at_seconds` inverse to < 1e-9 s over
random maps; monotonic; a constant map equal to today's arithmetic to the bit (the
contract's test stays).

**3. The engine follows the map.** The transport counts song samples through the map, so a
tempo point or a ramp is **not** a jump (only seeks, wraps and edits re-anchor); passes end
at `next_change_after` (or, inside a ramp, decide per-pass constant tempo with CLAP's
`tempo_inc` against finer splitting — measure the CPU and say it in the ADR); notes, audio
clips (the **unwarped** rule, below), automation (`auto_point`, `samples_until`), loops
(`Wraps` loop length exact from the map), the click and its accent across metre changes,
the count-in, `SongPosition` (real song samples, the tempo now, `bar_start_beats` from the
metre map, `earlier()` through the map), the modulation family's synced phase (from beats,
bit-identical at a constant tempo), modulators (`ModTime`, envelope gates in seconds
through the map; the bake), Speculum's scope chunks, delays and anything synced
(`Spatium`'s predelay sync: decide whether it follows live), the offline renders' lengths
and stop sample (`render.rs`, `stems.rs`), the recorder's tick conversion (a take recorded
across a ramp lands where it was played), the recovery journal, hosted plug-ins'
`TransportInfo` (CLAP `song_pos_seconds`, `tempo`, `tempo_inc`, `bar_start`,
`bar_number`; VST3 `projectTimeSamples` as real samples, `barPositionMusic`, `tempo`,
`timeSigNumerator/Denominator`) and the exported plug-ins (they only read their host).
**The unwarped audio clip:** write `crates/cantus-core/src/audio_time.rs` — the single
function every site uses to map an audio clip's tick to a source frame and back (engine
`add_clips`, `snapshot::audio_room`, the UI's `audio_clips.rs`, `pitch.rs::ClipTime`,
`arrangement_audio.rs::wave`, comping, the inspector…): an unwarped clip plays at its
natural speed, anchored at its start (decide and document how `offset` converts — at the
tempo where the clip starts is the natural reading, Ableton's unwarped clip); a test proves
the UI and the engine agree sample for sample. The warp builder extends this file for
warped clips after your merge.

**4. The interface.**
- **The ruler and every grid count bars through the map**: one helper for bar lines, beats,
  BBT and bar/beat snapping (the 66 `snap` sites and ~48 metre sites of the map, the piano
  roll, the channel rack, the pitch editor, automation, the clocks, nudge by bar, default
  windows); `views/mod.rs`'s `("grid.bar", PPQ * 4)` asks the metre.
- **A tempo track** lane in the arrangement (global, under the ruler, shown/hidden from the
  view menu and the ruler's menu): points drawn on a scaled bpm axis, steps and ramps
  visible, add (double-click), drag (time snapped, value with Shift for fine), Alt/menu to
  make a ramp, delete, type a value (the kit's value field), the tempo at the mouse shown in
  label/value units; FL's tempo automation clip as the visual reference, Cubase's tempo
  track as the editing one.
- **Metre changes** on the ruler (a signature chip at its bar: add, move along bar lines,
  edit with the top bar's metre editor, delete); the arrangement, piano roll and rack
  redraw their bars accordingly.
- **Markers** in a marker lane on the ruler: add at the playhead (a key; FL uses
  `Alt+T`-style — choose without collisions), rename by a click on the name (ADR 0051's
  gesture), drag, delete, colour from the kit's palette; jump to previous/next (keys), click
  a marker to jump; **loop between markers** (from a marker to the next, or between two
  selected), **select between markers**, **export between markers** (`Scope` in
  `cantus-export`, offered in the export dialog with the two markers named); markers listed
  in a small menu or panel to jump to by name.
- **The top bar**: the tempo field shows the tempo at the playhead (ramps included), the
  metre the one in force; editing follows the ADR; the clock's BBT counts through metre
  changes.
- `shoot_tempo` with a song that has a ramp, a 7/8 bar, 3/4 and 4/4 sections and five
  markers, at the usual scales, languages and themes.

**5. Interchange.** MIDI export writes the whole conductor track (every tempo point; ramps
stepped at a stated resolution; every metre change; markers as meta 0x06), and MIDI import
keeps the file's tempo map losslessly into the tempo track (a new default beside the
`Beats`/`Seconds` modes, with metre changes and markers; the import report updated).
DAWproject writes `<TempoAutomation>`, `<TimeSignatureAutomation>` and `<Markers>` (the XSD
test passes) and reads them (the drop counts at `read_timeline.rs` become real imports).
The AI bridge sees the map in its context and can set a tempo point.

**6. Tests.** A project without a tempo track is bit-identical (every golden unchanged);
block-size independence at tolerance 0 across points and ramps; offline = real time across
a ramp and a metre change; parallel = serial; the click lands sample-exact on every beat of
a ramp and accents bar 1 of each metre; a note at a tick after a ramp sounds at the
analytic sample; an automation clip and a synced LFO stay on the beat through a ramp; a
loop spanning a tempo change wraps exactly; a hosted test plug-in sees the right transport
(CLAP and VST3); MIDI and DAWproject round trips keep the map; recording across a ramp
places the take; `rt_memory` with a tempo map; your fuzz arm.

**7. Finish.** ADR 0060 (cites 0006, 0021, 0025 §1, 0027, 0055; amends 0021's dropped
items), the manual's subsection *The tempo track and markers* (§5, after *Clips in the
arrangement*) in both languages, `docs/round29/tempo.md`. Tell the integrator the moment
parts 1–3 are committed and green: warp's part B and the wave-2 builders wait for your
merge, so **land the engine and model first**, the interface second.

**Done when** a song with a ramp from 90 to 140 BPM, a bar of 7/8 and named markers plays,
loops between two markers, exports between two others, records a take across the ramp, and
round-trips through MIDI and DAWproject — with every synced unit, modulator, automation
lane and hosted plug-in on the beat, and every old project bit-identical.

---

## warp — time-stretch and pitch-shift of audio clips, élastique class (BACKLOG §3 *Time*; item 1)

Read BACKLOG §3 (*Time*, *Editing*), §3c (the pitch editor — your neighbour), ADRs 0023,
0025 (§1: the time contract you replace), 0047 (licences), 0050 (streaming), 0053 and 0054
(the voice base and **the pitch render path, your model**: a worker, a disk cache, renders
attached to the snapshot, bounce = playback), `research/engine/dsp-quality.md` (lines
60–90), `research/instruments/sampler.md` (52–54, 445–454), `research/engine/README.md`
(229–231), and the map `F:\projects\daw-r29\maps\audioclips.md` (every tick↔frame site,
the render key, the risks). **The quality bar is élastique Pro / Ableton's Complex Pro /
Logic's Flex Time**: listen-level clean on drums, a full mix, a sung voice, a solo
instrument, a chordal pad, at ratios from 0.5 to 2 (and usable from 0.25 to 4), and
pitch-shift ±12 semitones with formants kept.

**Licences first** (write it in ADR 0061 before any code): our own implementation in
`cantus-dsp` (MPL) is the default, as the owner asked — built on round 28's voice base
(`pitch*.rs`, `psola*.rs`, the epochs) and a **phase-locked STFT**. Rubber Band (GPL /
commercial) and SoundTouch (LGPL) may be *read*, never ported into the MPL crates; élastique
is commercial (a listening reference only); Signalsmith Stretch (MIT) and Bungee (MPL-2.0)
may be studied and, if one is used, carried with its notice — say which ideas came from
where. Read the papers: Laroche & Dolson (phase locking), Driedger & Müller (HPSS-based
TSM, the TSM review), Průša & Holighaus (PGHI, phase-gradient heap integration), Röbel
(transient detection and preservation in the phase vocoder), Bonada (spectral shape),
Röbel & Rodet (true envelope for formants), Ellis (beat tracking by dynamic programming),
Böck/Dixon (onset detection: spectral flux, SuperFlux).

### Part A — from the contract

**1. The stretcher and shifter** (`cantus-dsp/src/stretch*.rs`), offline first (a render of
a whole clip, deterministic, block-size independent), each mode a choice of method:
- **Complex** (default for mixes and anything polyphonic): STFT with phase locking (or
  PGHI), transient detection with phase reset / transient-preserving (HPSS split: the
  percussive part time-shifted without stretching, the harmonic part phase-vocoded),
  stereo coherence (shared phase across channels or M/S), window and hop chosen for
  quality, formant preservation for pitch-shift (a true-envelope or cepstral spectral
  envelope moved back); the output exactly the requested length.
- **Rhythm** (drums, percussive loops — Ableton's *Beats*, FL's *Slice*-style): segments
  between transients played at original speed and cross-faded or looped to fill, with the
  transient kept sample-sharp; options for the tail (loop back and forth, fade).
- **Melodic** (a voice or a solo instrument — Ableton's *Tones*, Melodyne's monophonic
  quality): TD-PSOLA on round 28's epochs (the pitch tracker's analysis), with time and
  ratio curves (`psola_offline::TimeMap` already maps output to source), formants kept;
  falls back to Complex in unvoiced or polyphonic stretches.
- **Re-pitch** (tape: speed and pitch together, the polyphase resampler of
  `exemplar_resample.rs`) as the free fourth mode.
Every mode: warp markers honoured sample-exactly (a marker's source frame lands on its
tick), transpose in semitones and cents, formants on/off where it applies, an identity
setting (ratio 1, no transpose, markers on the identity) that returns the source **to the
bit**.

**2. A measurement bench** (tests and a `cantus-bench` suite): on generated material
(impulse trains, drum loops built from our kits, chords, chirps, a vibrato voice from
`pitch_testkit`) and on the demo songs' stems — transient sharpness (attack time and
pre-echo energy before each onset), pitch accuracy of shifted tones (cents), phasiness
(inter-channel and inter-frame phase coherence), length and marker timing error (samples),
CPU per second of audio per mode. Targets stated in the ADR and met. Render an A/B
listening set (each mode × ratios 0.5, 0.8, 1.25, 2 × material) into
`F:\projects\daw-r29\shots\warp\listen\` for the owner, with a short `README.txt`.

**3. Analysis at import** (`onset*.rs`, `beat*.rs`): onsets (spectral flux / SuperFlux with
an adaptive threshold and a minimum gap), tempo (onset-strength autocorrelation or comb
filter, the octave ambiguity resolved toward 70–180 BPM with a confidence), a beat grid and
downbeat fit, loop detection (a file that is a whole number of bars), the WAV `acid`
chunk's tempo when present. On a worker (generalise `PitchService` into one analysis/render
service, or a sibling — say which), cached on disk like `pitch_disk.rs` (keyed by file
identity and the analyser's version), never on the interface thread (import decodes on it
today: move what you add off it). **Decide in the ADR** (and make it reachable from every
import path: File ▸ Import, an OS drop, the browser drop, a recording, a paste): a short
loop whose tempo is confident is warped to follow the project at once (Ableton's
auto-warp for loops), a long file or a recording plays at natural speed until the user
warps it, and the analysis lands as one undo step with the import. Accuracy measured on a
corpus (generated click and drum tracks at known tempos 60–200, with swing; report the
percentage within ±1 BPM and the octave errors).

**4. The model (schema 34).** `AudioRegion::warp: Option<Box<Warp>>` — the mode, the warp
markers (each a **source frame** paired with a **content tick**, so split, trim, slip,
glue, loops and links keep them valid, as `PitchEdit` does), transpose, formants, the
source's detected tempo and grid, *follows the project tempo*. Commands with exact inverses
(set the whole warp, add/move/remove a marker, quantise, mode, transpose — merged per drag),
fuzz arm 44 with its counter; linked clips share it; split/glue/trim/slip/reverse/loops/
comp seams/clipboard defined (comp seams: either warp the seam window or exclude warped
windows — say which); the pitch edit chains (pitch in source frames first, then warp).

**5. The editor.** Ableton's warp markers and FL's audio clip stretch are the references:
in the arrangement, a warped clip draws its waveform through the warp map (piecewise
columns), shows transient ticks faintly and its warp markers as handles; double-click a
transient or the waveform's top band to add a marker, drag to move (the audio between
neighbours stretches), delete with the menu or Del, Alt-drag to move the content under a
marker; the clip's right edge stretches the whole clip with a modifier (say which; FL uses
one too); a **Warp** mode beside *Pitch* in the audio clip editor (ADR 0054's window —
amend 0051 §4 if the double-click changes) for detailed work with a beat ruler; the
inspector's warp section (mode, transpose st/ct, formants, detected tempo, *Follow the
project tempo*, *Set the project tempo from this clip*, *Warp from here*); the clip menu
(`Warp ▸` modes, *Quantise audio…* — grid, strength, which transients — *Detect tempo*,
*Reset warp*, *Bounce warped audio*), as a separate menu list from your own file. Every
string in both languages; `shoot_warp` (a drum loop and a voice, markers, transients, the
inspector, the editor, the menus).

### Part B — after tempo merges (the integrator tells you; merge `r29/integration` into your branch)

**6. Playback under the tempo map.** Extend `cantus_core::audio_time` (tempo's file) for
warped clips: content ticks → song seconds through the map → source frames through the warp
markers; the render laid out so `add_clips` plays it at one frame per sample. The render
key holds the source, the pitch key, the warp, the engine rate and the map's `span_key` over
the clip's window and anchor; keep the last renders (`HISTORY`) so undo plays at once; while
a render is on its way the clip must never play at the wrong speed (a real-time stretcher
for the clip being edited, latency reported and never allocating — or the last render plus
a varispeed bridge: decide, and the owner must not hear a glitch while dragging a marker or
the tempo); `compile_for_render` waits for warp renders (export, stems, freeze, bounce);
offline = real time; memory bounded (renders of long files on disk or evicted safely,
never freed on the audio thread).

**7. Interchange.** DAWproject exports audio clips at all (today it writes notes only) with
`<Audio>` and `<Warps>`, and reads them back (the drop counts become imports); the XSD test
passes; a round trip keeps the warp markers.

**8. Tests.** No warp = today to the bit (every golden unchanged); identity warp = source to
the bit; a marker lands sample-exact at its tick at 44.1/48/96 kHz and under a ramp; block
size and offline = real time; parallel = serial; the analysis accuracy; undo fuzz; the
analysis cache wired in `assemble` (a boot test); `rt_memory` if anything new runs on the
audio thread.

**9. Finish.** ADR 0061 (licences, methods, modes, the time contract replacing ADR 0025 §1,
the import rule, the render key, the measurements), the manual's subsection *Warping and
stretching audio* (§5, after the pitch editor) in both languages, `docs/round29/warp.md`.

**Done when** a drum loop at 92 BPM dropped into a 128 BPM project plays in time with
sharp hits, a sung phrase slowed by 25 % and shifted −3 semitones sounds natural with its
formants, a full mix stretched 0.8–1.25 has no audible phasiness on the A/B set, warp
markers drag smoothly with the sound following, quantise pulls a loose take to the grid,
and the whole thing exports exactly what plays.

---

## isolation — third-party plug-ins out of process (ADR 0031 phase 6; BACKLOG §3 *Hosted instruments*; item 3)

Read ADR 0031 completely (§2.7, §6, §8's phase-6 targets, R15), ADRs 0005, 0019 (hosting;
its "not built: crash isolation"), 0020/0026 if they touch hosted state, BETA.md
(blocker 3 names this a known limitation until now), `research/instruments/standards.md`
(lines 40–52), `research/engine/engines.md` §6 (Bitwig), `research/engine/README.md` §8,
`research/engine/scheduling-papers.md` (~247, activation records),
`research/engine/verification.md` (~38, loom), `docs/perf/plan.md` (180–197), and the map
`F:\projects\daw-r29\maps\hosting.md` (the `Backend` seam: a proxy `Backend` whose
`MainThread`/`AudioThread` are proxies leaves the engine and `HostedUnit` unaware). Study
how Bitwig (per-plug-in / together / by vendor sandboxes), Reaper (bridging, "run as
separate process" / dedicated process), FL Studio (*Use bridge* / *Make bridged*) and
Studio One do it, and what each costs.

**1. The design (ADR 0063, decided first and measured).** The audio-thread side: lock-step
(write the block into shared memory, signal, wait for the completion with a deadline) or
ADR 0031 §6's external task, or the asynchronous option (+1 block of latency reported to
delay compensation). Lock-step **waits on another process from the audio thread** — ADR
0005 forbids blocking calls there: if you choose it, the wait is a bounded spin then a
kernel wait with a deadline, the ADR amends 0005 for this one case with the reasoning (the
plug-in's own processing time, moved out of process) and the measurement, and the spinner
budget of R15 holds. Pick by measuring: round trip at 64/128/256 frames (p50, p99), CPU per
instance, the phase-6 target (≤ 5 % of a 64-sample budget per group, *Together* with 32
test instances ≤ 1.2× in process on `dawbench.dsp.b128`). **Modes**: *Within Cantus*,
*Together* (one process for third-party plug-ins), *Individually* (one per plug-in) — and
*By vendor* if cheap. Choose the default for third-party plug-ins by the numbers and the
owner's goal ("a plug-in that crashes must not take the session down"); our own units are
always within.

**2. Shared memory and signalling** in `cantus-rt` (MPL): `ipc`/`shm` modules — Windows
`CreateFileMapping`/`MapViewOfFile` and named events (or a cross-process futex-like scheme);
Linux `shm_open`/`mmap` with a futex or eventfd (it must build and pass on Linux: CI is
Linux; a clean "unsupported" stub is acceptable only for the child-process half if the
Linux app has no out-of-process hosting yet — say so). Fixed-size, allocated at load; the
job/completion protocol sequence-numbered and tested with loom (or model-checked) and Miri
where it applies. Amend ADR 0031 §2.7's line budget for `cantus-rt` if needed.

**3. The child process** — a new crate `crates/cantus-sandbox` (GPL; its own `build.rs` with
its version resource): runs `cantus-host`'s existing CLAP and VST3 backends unchanged;
a main thread that pumps Win32 messages (and `OleInitialize` for VST3); processing
thread(s) promoted per `cantus_rt::thread` (never spinning inside MMCSS), `DenormalGuard`
so its output equals in-process output **to the bit**; `quiet_crashes`; a Windows Job
object with kill-on-close so no child outlives Cantus; a heartbeat; found beside the
program (`Scanner::beside_this_program`'s pattern) with an env override for tests.

**4. The proxy backend** in `cantus-host` (`src/bridge/`): `BridgeAudio` (copies inputs,
params, notes and transport into the instance's slot, waits per the ADR, copies outputs and
the plug-in's parameter events back; on a timeout or a dead child the block passes dry for
an effect and silent for an instrument, counted — `EngineStats` gets a `sandbox_late` /
crash counter shown with the other drops); `BridgeMain` (every RPC with a timeout, a
parameter mirror read without RPC — the UI calls it every frame —, texts cached, state
blobs, latency changes and restarts, editors); **a last-good state cache** refreshed on
`StateDirty`, at gesture ends and periodically, so a crash never loses what the user did
(`project_for_save` uses it); `PluginHost::backend_for_plugin` replacing the eight
hard-coded `backend_for` sites (map §7.C); editors as **top-level windows owned by
Cantus's window, created by the child** (a hung child must not freeze our window — say how
`fit`/front/close are proxied); hosted instruments called per note segment today mean many
round trips per block — move hosted units to whole-block processing with notes at their
offsets if the round trips cost too much (ADR 0019 lists it as open; measure).

**5. Crash, tell, reload, lose nothing.** `Notice::Crashed`, `HostNotice::{PluginCrashed,
InstrumentCrashed}` with their arms in `app.rs::host_notices`; the strip slot (and the
channel rack / track header for an instrument) drawn crashed in the danger colour; a corner
card — *"<plug-in> stopped working. Cantus kept playing without it."* with **Reload** and
**Keep it off** — in one shared card stack with the audio notices (no overlap); *Reload*
restarts the process (or the instance) and restores the last-good state; *Reload all*
when several died; a plug-in that crashes three times in a session is offered *Run
individually*; the project saves the plug-in's last-good state and reopens with it. The
user can choose per plug-in (the slot menu's *Isolation ▸ Within Cantus / Together /
Individually*, stored in the project — schema 36, `ExternalPlugin`'s hosting field, a
command with its inverse, fuzz arm 46) and the default in Settings' plug-in manager
(per machine); the plug-in database remembers a plug-in that crashed.

**6. Test plug-ins that fail on purpose.** A hidden *Fault* parameter in `clap-gain`,
`clap-synth` and both VST3 test plug-ins: abort in `process`, hang in `process`, abort in
`save_state`/`load_state`, abort on `activate`, abort on editor open, plus an export that
reports the process id so tests see the instance really lived elsewhere. Tests (real child
processes): each fault leaves the session running and the audio flowing (dry / silent), the
notice raised once, *Reload* brings the plug-in back with its last-good state, the project
saved after a crash keeps that state, a hang never blocks the audio thread beyond the
deadline nor the interface thread at all; **offline renders through isolated plug-ins wait
without a deadline and equal in-process renders to the bit** (export, stems, freeze and
bounce use `render_units`/`render_instruments`: they must isolate too, or say why not).

**7. Measured.** A `cantus-bench` suite: in process vs Together vs Individually at 64, 128,
256 frames — round trip µs p50/p99, CPU per instance, the dawbench row with 32 instances —
the numbers in the ADR and `docs/perf/`, and on the plug-in's own row (a per-plug-in CPU
figure in the slot's tooltip or the inspector, BACKLOG §3 asks for a per-plug-in CPU meter:
deliver it since the child measures it anyway).

**8. Shipped.** `xtask` packaging (the new exe staged, built, checked, its version
resource, `package_tests.rs`), the installer (`nsis.rs` waits for it), the signing policy's
file list, `notices.rs` `ROOTS` and `THIRD-PARTY-NOTICES.md` regenerated, the release
workflow builds it, `docs/DEVELOPING.md` (building the helper in development), and the test
version in `F:\projects\daw-r29\builds\` will carry it — run the real app with a real
crashing test plug-in in a strip and show the card in a `shoot_hosted` image.

**9. Finish.** ADR 0063 (amends 0031 §6/R15, 0019, and 0005 if lock-step), the manual's
subsection *When a plug-in crashes* (§10) in both languages, `docs/round29/isolation.md`.

**Done when** a test plug-in that aborts mid-song leaves Cantus playing, says so, reloads
with what the user had set, and the project loses nothing; isolation costs a measured,
published amount within ADR 0031's target; and the installer ships the child process.

---

## controllers — MIDI learn and controller mapping (BACKLOG §1 *MIDI controllers*, §3 *Automation*; item 2)

*Wave 2: starts from `r29/integration` after tempo's merge (the tempo is a target).*

Read BACKLOG §1 (*MIDI controllers*, around line 170), §3 (*Automation*), §0.2 (*Sync*,
*Hardware integration*), ADRs 0027 (automation), 0052 (the top bar's MIDI lamp), 0057
(modulators, hosted parameters, recording from a plug-in's window), and the map
`F:\projects\daw-r29\maps\midiparams.md`. The references: FL Studio (*Link to controller*
on any control, *Multilink*, *Last tweaked*, takeover modes, generic controller script),
Ableton (MIDI Map mode: everything mappable highlighted, click, move, done; min/max per
mapping, the mapping browser), Bitwig (learn, ranges, relative encoders), Reaper (learn
with relative modes and soft takeover). Take the best of each.

**1. The input.** The MIDI callback tees what the interface needs into a second lock-free
ring (port identity and timestamp kept) and wakes the interface (a repaint hook set at
boot); mapped messages are **consumed** before the engine's ring (a lock-free set of
`(port, kind, channel, number)`), so a mapped knob no longer bends the instrument and a
mapped pad no longer plays a note; port identity by name plus ordinal, survives a rescan
and a restart; channel pressure, poly pressure and program change routed where they belong
(today parsed and dropped). Wired in `boot.rs`/`assemble` and in `restart_audio`/
`rescan_midi`, with a boot test.

**2. Learn, everywhere.** `kit/automate.rs`: *Learn MIDI controller* and *Forget* in every
parameter's menu (a probe test like `tests/modulate_everywhere.rs` fails on a control
without it), a mapped mark on the control; **a learn mode** (Ableton's MIDI Map mode: a key
and a top-bar button; every mappable control outlined, click one, move a hardware control,
the next); *Link the last tweaked parameter to a controller* (FL's, an action) — the only
way into a plug-in's own window; and the targets that have no `AutoTarget` today: mixer
mute, solo, arm, insert bypass and mix, master; the transport (play/stop, record, loop,
metronome, tap tempo, the tempo — through the tempo map's rule —, previous/next marker,
pattern/song) as keymap actions run through `run_action`. Core gets the inverse of
`value_of_command` (`AutoTarget::command(project, value)`), round-trip tested for every
target.

**3. The mapping.** Absolute, relative (two's complement, binary offset, sign bit — and
auto-detected on learn where the hardware makes it clear), toggle, momentary/trigger;
14-bit CC pairs, pitch bend, notes and velocity (pads), program change; range (min/max of
the target's travel, inverted allowed), curve (the automation's `Shape` bend), soft
takeover (jump, pick up, scale) with the current value from `base_pos`; one undo step per
gesture (per-mapping merge keys, `end_gesture` after an idle time); a moved control while a
pass records **records automation** through the existing `record_tweak` path, at the
message's own time; a parameter under an automation clip follows the clip in playback
unless recording (say what FL/Ableton do and match the better). **Feedback** to the
controller (MIDI out: the value sent back on the mapped CC when the parameter changes —
for motor faders and LED rings), per mapping, off by default.

**4. Where maps live.** Per project (schema 35: the project's controller map, commands with
inverses, fuzz arm 45, a mapping whose target vanished kept dormant and shown) and **global**
(per user: a file beside `keymap.toml`, read in the boot's Files stage, for transport,
master, *the selected track/strip* and *the focused device's first 8 parameters* — Ableton
and FL both have these "relative" targets), with a stated precedence (project over global).

**5. The mappings view.** A pane (`Pane::Controllers`, on the View menu and a key): every
mapping with its source (port, channel, message), target (its readable name), mode, range,
curve, takeover, feedback; edit in place with the kit's fields, learn again, delete, filter,
conflicts flagged (one source on two targets: allowed or not — decide); the global and the
project maps in two sections; a live activity dot when a message arrives. FL's *Remote
control settings* and Ableton's mapping browser are the references; it must look
first-rank: `shoot_controllers` at the usual scales, languages and themes.

**6. Tests.** The parse/filter/tee units; absolute/relative/toggle/takeover arithmetic;
the round trip `command(value_of_command(c)) == c` over every target; *learn everywhere*;
a mapped CC no longer reaches the instrument (engine test); a mapped CC during a recording
pass yields one automation clip in one undo step with the right times; the global map
loaded at boot; undo fuzz; nothing on the audio thread allocates.

**7. Finish.** ADR 0062, the manual's subsection *MIDI controllers* (§4, after *MIDI*) in
both languages, `docs/round29/controllers.md`.

**Done when** a cheap MIDI keyboard with eight knobs and pads drives a filter, a fader, a
plug-in's parameter, mute and the transport by learn in two clicks each, soft takeover never
jumps, a knob moved while recording leaves automation, and the maps come back with the
project and across projects.

---

## freeze — freeze, bounce in place, low-latency monitoring (BACKLOG §4c steps 1–2, §3 *Editing*; item 5)

*Wave 2: starts from `r29/integration` after tempo's merge (render lengths and the
recorder are tempo's code by then).*

Read BACKLOG §4c **completely** (the owner's design: the render taken after inserts, sends
and effect automation and before the fader and pan; the per-strip content hash; streaming
from disk; step 2's limit and the *constrain delay compensation* idea of 2026-10-15), §3
(*Editing*), §4b (background freeze shares the cache), ADRs 0018/0022 (soft bypass, PDC),
0025 (recording and its latency), 0031 §4 (the render cache design), 0045 (the pre-fader
engine), 0050 (streaming from disk — your reader), and the map
`F:\projects\daw-r29\maps\freeze.md` (the boundary before `apply_width`, the frozen strip's
dead inputs, the planner, the take-placement risk). The references: Ableton (*Freeze* and
*Flatten*, frozen tracks still mixable), Bitwig (*Bounce in place*, bounce with or without
the chain), FL Studio (*Render to audio / consolidate*, *Freeze* in the mixer since FL 21),
Cubase (*Freeze* with tail, *Constrain delay compensation*), Logic (*Freeze* pre/source
only, *Low Latency Mode*), Studio One (*Transform to audio*, *Low latency monitoring*), Pro
Tools (*Commit*). Take the best of each.

**1. Freeze (schema 37).** Decide in ADR 0064 what freezes, from the owner's words
"congelar pistas" and §4c: a track (its instrument, clips, pattern, the inserts of the
strip it feeds when that strip is its own) and a mixer strip (everything it gathers), with
the boundary **before width, pan and fader**: those, mute, solo, the sends' levels and their
automation stay live; everything before is a file. The render is keyed by a content hash of
everything that makes the sound (the strip and what it gathers, recursively; instrument
specs; clips, notes, patterns; effect specs and hosted state blobs; the automation and
modulators that reach them; the tempo map's `span_key`; the identity of each audio source;
pitch and warp edits; the rate; `DSP_VERSION`) — SHA-256 over a canonical form, never
`DefaultHasher`. The files live beside the project in a cache folder (decide whether
*Collect* copies them or treats them as regenerable), are **streamed** from disk by a
reader built on ADR 0050's `Stream` (never whole in memory, never I/O on the audio thread,
pins at the playhead and loop start, offline waits), and the frozen strip gathers nothing
— its sources become dead nodes, so the CPU is freed (measured: a heavy instrument track
frozen drops the callback's load by what it cost). A frozen track or strip shows it (a
snowflake glyph on the track header and the strip's head, the clips greyed with a frozen
tint, the inspector's row); an edit that would change a frozen sound is refused with a
notice that says how to unfreeze (every `Command` classified: a new command fails a test
until classified — §4c's rule); **Unfreeze** gives back exactly the live chain, bit for
bit; re-freezing renders only what changed; hosted plug-ins of a frozen chain are
deactivated (coordinate with isolation's code if it has merged); a freeze shows progress
and can be cancelled; commands with inverses and fuzz arm 47.

**2. Bounce in place.** From the track menu, the clip menu and the mixer strip menu:
*Bounce in place* with or without the inserts (and with or without the fader/pan — say
which, Bitwig's *pre/post fader*), for the whole track, the selected clips or the time
selection, with the tail (a reverb's) — the result an audio clip at the same place on the
same track (the instrument muted or set aside, Bitwig's way) or on a new track (Ableton's,
FL's), the original kept muted or removed (the user's choice, remembered), one undo step;
hosted plug-ins rendered through `render_units`/`render_instruments`; an instrument track
becomes an audio track when asked. Offline = what plays, to the bit, at the export rate.

**3. Low-latency monitoring (§4c step 2).** A setting (the limit in ms, default chosen with
a reason) and a switch where a recording musician looks (the record popover and the audio
chip; a key): while on, the inserts **on a monitored path** whose latency exceeds the limit
are bypassed hard (named on screen: "Monitoring without: Aequalis linear phase, …"), and
the monitored path is **not held back** for other strips' latency (the constrained delay
compensation of Cubase): `fx_tonus::monitoring_waits_for_the_projects_largest_latency`
becomes two tests (off: as today; on: the monitored voice arrives after Tonus's 128
samples only). Decide what happens to a bus that mixes monitored input and backing. The
take is still placed to the sample: keep the recording latency captured **at record start**
in the pass (today it is read at finish — a bypass restored on stop would misplace the
take); test that a take recorded with the mode on lands where it does with it off. The
audio chip shows the monitored path's round trip (driver + queue + the monitored chain).

**4. Tests.** Frozen playback equals live playback to the bit (the null test, with the
fader, pan, sends and mute live and automated); block size and offline = real time;
parallel = serial; unfreeze restores the live render to the bit; the hash changes for
exactly the changes that alter the sound (and not for a fader move); a frozen strip's
sources cost nothing (a CPU test or the node counter); bounce = playback; low-latency
monitoring: the arrival time with and without, the take placement; `rt_memory` scenarios
(`frozen_playback_never_allocates`, `low_latency_monitoring_never_allocates`); fuzz arm 47;
the freeze cache folder wired from `assemble` (a boot test).

**5. Finish.** ADR 0064 (amends 0031 §4 and 0025's latency rule), the manual's subsections
*Freeze and bounce in place* (§6) and *Monitoring with low latency* (§4) in both languages,
`docs/round29/freeze.md`, `shoot_freeze` (frozen track and strip, the menus, the notice, the
low-latency switch and its list).

**Done when** a project with a heavy instrument and a linear-phase EQ freezes that track and
plays it with the CPU freed and the fader still live, unfreezes to the identical sound,
bounces a clip in place with and without effects, and a singer monitored through Tonus
hears themself 2.7 ms late instead of 96 ms while the take lands to the sample.

---

## frictions — the owner's frictions (item 6)

None were given at the start (2026-10-16). Any friction the owner reports during the round
is written here and assigned to the builder whose area it is (or to a small fix branch).

## Reviews and fix rounds (the integrator's)

Every branch is reviewed adversarially before and after its merge with a Workflow: five
lenses — **correctness**, **real time** (ADR 0005, `rt_memory`, locks, allocation, I/O,
spinning), **reachability** (from `cantus_ui::run` → `boot::Boot` → `CantusApp::assemble`;
every menu entry, key and button reaches the code), **tests** (seen failing, null tests,
offline = real time, block size, parallel = serial, fuzz arms), **interface** (the kit, both
languages, no overlap, scrollable, FL's look; the shots) — plus two focus lenses per branch,
and an independent refuter per finding. Confirmed findings and every "Left" item go back to
the builder (or to a fix branch `r29/fixN`), each fix with a test seen failing first; the
fixes are reviewed again; the integrated tree is reviewed as a whole at the end, until a
review finds nothing above "low" — and those are closed too.
