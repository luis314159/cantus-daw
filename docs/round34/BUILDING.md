# Round 34 — AI: the MCP server first, then stems, audio to MIDI and the mastering assistant

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians
3:23). Tests before code, nothing half done, look at what you build (`shoot` examples).
**The owner's rule:** every gap a feature leaves is closed in the same round. Whatever you
would write under "Left" comes back to you as work; only list as open what truly needs the
owner (their ears, their hardware, a paid account, a decision).

**Run lean** (as rounds 30–33): read only what your brief needs, prefer `grep` and line
ranges to whole files, keep reports short and exact.

The ask (the owner, 2026-10-30, `docs/next-session.md`):

0. What round 33 left: nothing — `macos.yml` and CI on public 07867bfd green with nothing
   retried, no issues open on `cantus-daw`, no owner notes, the Mac session not done (beta.9
   stays Windows + Linux).
1. **An MCP server for Cantus** (BACKLOG §7 phase B, `research/ai/mcp-server.md`): Claude and
   other assistants drive Cantus end to end — tracks, clips, notes, the session's scenes, the
   mixer, effects and instruments with their parameters in their units, automation, export —
   **each tool one undo step** through the same `CommandBus`; with what BACKLOG §7 asks:
   listening with numbers back in one call (`audition`), per-track analysis and a masking
   matrix, the music linter, the machine-readable catalogue of every unit (generated from the
   parameter tables), the chunky tools (`write_part`, `voice_chords`, `automate`,
   `set_mix_targets`…) and what Cantus needs inside for them (a range render from a saved
   state, per-strip analysis taps in the offline render, "live = offline" checked). Safe by
   design: nothing outside the project without leave; the user sees and undoes everything.
2. **Stems**: a song into vocals, drums, bass and the rest, by a model downloaded when needed
   (never in the repository; the model's and the weights' licences checked, ADRs 0030/0042),
   in its own process, the quality measured (SDR) against the reference.
3. **Audio to MIDI** (melody, polyphony, drums) with the same care, and **the mastering
   assistant** on the master channel: it listens, proposes a chain of our effects and
   loudness targets per platform, and everything stays editable by hand.
4. The owner's frictions and the beta's users: none at the start; the integrator hands out
   whatever arrives during the round.

Read, in order: this file (the rules and **your** section); `CLAUDE.md`; `docs/STATUS.md`'s
header; `research/ai/README.md`; the research files and ADRs your brief names.

## The builders

All start from the contract commit on `r34/integration`.

| Builder | Branch / worktree | Delivers | ADR | Schema | Other numbers |
|---|---|---|---|---|---|
| **mcp** | `r34/mcp`, `F:\projects\daw-r34\mcp` | the server, its doors, permissions, confirmation, activity, setup, packaging | 0081 | — | keymap 15 (if it binds a key) |
| **tools** | `r34/tools`, `F:\projects\daw-r34\tools` | the project's tools, the unit catalogue, the music linter, the chunky tools, resources, prompts | 0082 | 50 (only if it must) | — |
| **listen** | `r34/listen`, `F:\projects\daw-r34\listen` | range renders, analysis taps, `audition`, analysis and masking, mix targets, live = offline; then the mastering assistant | 0083, 0084 | — | — |
| **stems** | `r34/stems`, `F:\projects\daw-r34\stems` | the ML helper process, model downloads, stem separation | 0085 | 48 | — |
| **transcribe** | `r34/transcribe`, `F:\projects\daw-r34\transcribe` | audio to MIDI: melody, polyphony, drums | 0086 | 49 (only if it must) | — |

`mcp`, `listen` and `stems` start now. **`tools` starts when mcp's first delivery** (the
dispatch over `cantus_ui::ai_tools::registry()`, the UI-thread pump with a real `ToolHost`,
permissions by class, an in-process client for tests) **is merged into `r34/integration`**;
**`transcribe` starts when stems' first delivery** (the helper process, the inference runtime,
the model download path, one job end to end) **is merged**. Each branches from that merge.

**Build slots.** Five builders, a 31 GB machine: every cargo command and every WSL check goes
through `bash /f/projects/daw-r34/slot.sh …` (three build slots, one WSL slot — read its
header). Waiting for a slot is normal; run long commands in the background.

**Disk.** F: had 145 GB free at the start and C: 41 GB (the WSL image). In your worktree never
run `cargo test --workspace` — test the crates you touch (`-p`, and `--test <name>` where you
can); the integrator runs the whole suite. Delete your WSL clone's `target/` as soon as your
check is done.

## Who owns what (one owner per hot file)

| File(s) | Owner | Others |
|---|---|---|
| `cantus-ai/src/tool.rs` (the contract's types), `provider.rs` | mcp | others ask mcp (through the integrator) for a change; never edit |
| `crates/cantus-mcp` (new crate: protocol, transports, the `cantus-mcp` stdio bridge binary), root `Cargo.toml` members, `LICENSE`'s and README's crate lists | mcp | stems adds its crate's lines, marked |
| `cantus-ui/src/ai_*.rs` except the three below (the pump, the host, permissions, the card, the activity view), `views/ai*.rs` (new), Settings ▸ AI, the top bar's agent chip, `keymap.rs` (keymap 15) | mcp | others: marked lines |
| packaging: `xtask/src/package*.rs`, `nsis.rs`, `packaging/**`, `.github/workflows/release.yml` | mcp | stems: marked lines for the helper binary |
| `cantus-ai/src/tools_project.rs`, `cantus-ai/src/tools_*.rs` (new), `bridge.rs`, `cantus-core/src/harmony*.rs` (new: the linter), resources and prompts | tools | — |
| `cantus-ui/src/ai_tools_listen.rs`, `cantus-engine/src/listen*.rs` (new), `cantus-engine/src/render.rs`, `stems.rs`, `stems_master.rs`, `cantus-dsp`'s analysis (`speculum*`, loudness), `cantus-ui/src/mastering*.rs` and `views/mastering*.rs` (new) | listen | marked lines |
| `cantus-ui/src/ai_tools_stems.rs`, `crates/cantus-ml` (new: the helper process and the runtime), `cantus-content/src/models*.rs` (new), `cantus-ui/src/stems_ui*.rs` (new), `views/arrangement_audio_menu.rs` | stems | transcribe: marked lines |
| `cantus-ui/src/ai_tools_transcribe.rs`, `crates/cantus-ml/src/transcribe*.rs` (new), `cantus-dsp/src/onset*.rs` (new), `cantus-ui/src/transcribe*.rs` (new) | transcribe | — |
| `cantus-project/src/format.rs` (`SCHEMA_VERSION`, migrations), `fixtures/` | stems (48); transcribe (49) and tools (50) only if they must | the integrator renumbers if needed |
| `cantus-ui/src/app.rs`, `boot.rs` | nobody's: **marked lines only** (mcp's pump, stems' and listen's services) | — |
| `cantus-ui/src/settings.rs` | anyone, **additive fields only**, `#[serde(default)]` | — |
| `crates/cantus-core/tests/undo_invariants.rs` | anyone, **in your own `srng` stream and helper fn** (round 32's pattern); never change `below(49)` | — |
| `cantus-ui/src/ai_tools.rs` (the registry) | the contract; nobody edits it | — |

**Marked lines.** When you must call your code from a file another builder owns, put your
code in a new file of your own (declare it with `#[path]` inside one of your files where you
can, so shared `lib.rs`/`mod.rs` stay untouched) and add the fewest lines that call it,
each marked `// r34:<your name>`. List every marked line in your report. Never reformat or
move code in a file you do not own.

## The contract (decided; the commit holds the code)

- **One tool table** (`cantus-ai/src/tool.rs`): a `ToolDef` is a stable name, a title, a
  description for the model, a `ToolClass` (Read, Edit, Destructive, Files, Transport), JSON
  Schemas in and out, and a `ToolRun`: `Read` (answered at once), `Edit` (plans **one**
  `Command` — a `Batch` when it needs several — and never executes it), `Job` (prepared on
  the UI thread, run on a worker, may end in an `Edit` planned back on the UI thread:
  `JobDone::Then`), `Host` (transport, selection: not an undo step). Every function runs on
  the UI thread, which owns the bus. A tool that needs the application (the engine, the ML
  helper) lives in `cantus-ui` and downcasts `ToolHost::as_any_mut` to the concrete host.
  Each builder fills **its own list** (`TOOLS` in `cantus-ai/src/tools_project.rs`,
  `cantus-ui/src/ai_tools_{listen,stems,transcribe}.rs`); `cantus_ui::ai_tools::registry()`
  joins them in a fixed order. The legacy `bridge.rs` tools move into tools' list.
- **The host executes**: a planned command runs with `Origin::Mcp`, labelled so the history
  reads "AI: <what>" (one undo step per call; an agent may group a turn with
  `begin_change`/`end_change`). Mutating tools take an optional `expected_revision`
  (`ToolError::Stale`). Errors go back as `isError` tool results, never protocol errors.
- **Permissions decide in Cantus**, by class (`tool::default_decision`: reads and transport
  run; edits and destructive tools ask in the Cantus window; files are denied until the user
  enables them) — the client's annotations are hints only. Nothing reads or writes outside
  the project's folder and the folders the user adds without leave.
- **Long jobs** (renders over a few seconds, separation, transcription) answer with a job
  handle; `wait_job(handle, timeout_s)` and `cancel_job(handle)` are mcp's generic tools;
  progress shows in the AI activity view.
- **The MCP protocol is written by hand** in `cantus-mcp` (JSON-RPC 2.0 over std threads, no
  async runtime in the app): Streamable HTTP on `127.0.0.1` (JSON responses, `Origin`
  checked, a bearer token per install) and a stdio bridge binary that reaches the running app.
  Both protocol generations: the session one (2025-06-18, 2025-11-25) and the stateless
  2026-07-28. If mcp finds in its first hour a reason this is wrong (e.g. `rmcp` is clearly
  better), it says so to the integrator before going on.
- **ML runs in a helper process** (`crates/cantus-ml`, the `cantus-ml` binary, GPL): a crash,
  a hang or a GPU failure never takes Cantus down; jobs over a pipe, cancellable, with
  progress. Weights are never in the repository or the installer: downloaded on demand
  through the content manager, licence shown and accepted, hash checked, each with its
  provenance card (ADRs 0030, 0042). The runtime (`ort`, `tract`, `candle`…) is stems'
  measured choice in ADR 0085; transcribe uses the same one unless it measures a reason not to.
- **Provenance** (`research/ai/legal.md` §3): an AI-separated source carries its model,
  version, date and licence in the project (schema 48); its clips show a small "AI" badge.
- **Licences (ADR 0047)**: everything AI lives in GPL crates (`cantus-ai`, `cantus-mcp`,
  `cantus-ml`, `cantus-ui`); the MPL crates (`core/dsp/engine/project/rt`) get only what is
  ours and generic (the range render, analysis taps, the harmony linter, onset detection).
  **Never port GPL code (Matchering, YourMT3+…) into an MPL crate.**

## Rules (the usual ones, short)

- Work only in your worktree. Commit WIP often (`r34/<name>`); a cut session resumes.
  Write files in pieces under ~300 lines per tool call.
- Machine: `export PATH="$HOME/.cargo/bin:$PATH"` (cargo is not on PATH); your untracked
  `.cargo/config.toml` sets `jobs = 3`, no debug info and incremental off (never `git add`
  it); `TMP`/`TEMP` = `F:\projects\daw-r34\tmp`. Every cargo command through `slot.sh`. Test
  executables named `*setup*`/`*update*` need elevation on this machine: run them copied to
  a neutral name. A corporate proxy (Zscaler) may hold a download ~10 minutes and answer with
  an HTML page meanwhile: check what arrives (a zip starts with `PK`, an ONNX file is
  protobuf, not `<html`), wait, retry — **never bypass the proxy or TLS checks**. Downloads
  (models, datasets) go to `F:\projects\daw-r34\models\` — never into a worktree.
- Windows: `cargo clippy --workspace --all-targets` and the tests of every crate you touch,
  with `RUSTFLAGS=-D warnings`; `cargo fmt -p <crate>` for each crate you touched before
  each commit (`cargo fmt --all` fails here with *os error 206*). The Bash tool is Git
  Bash: a heredoc delimited by `EOF` fails to parse — use `XEOF` or the Write tool.
  Linux, before you report: write a small script file and run it through
  `slot.sh --wsl wsl -d Ubuntu-24.04 -- bash /mnt/f/…/script.sh` (inline `bash -lc '…'`
  loses `$VAR`s), the script calling `CANTUS_CI_DIR=$HOME/ci-r34-<name> bash
  /mnt/f/projects/daw-r34/<name>/tools/wsl/check.sh r34/<name> clippy` (and `test` for the
  crates you touched) — your own clone, never `~/cantus-ci`; **delete that clone's `target/`
  when it finishes**. Code behind `#[cfg(windows)]` leaves Linux-only dead-code warnings. UI
  tests that open windows must pass headless (`env -u DISPLAY -u WAYLAND_DISPLAY`).
- **Real time (ADR 0005):** the audio thread never allocates, locks, blocks or does I/O
  (`rt_memory.rs` counts it — add your paths to it); `cantus-engine` is
  `forbid(unsafe_code)`; OS/unsafe code only in `cantus-rt`. Never let a spinning thread run
  inside MMCSS. The server, jobs and the helper's pipe are ordinary threads.
- **Null tests first**, before wiring: a feature at its default touches no sample, to the
  bit (the golden hashes do not move; `DSP_VERSION` stays 5 unless you change a sound on
  purpose, then one commit with the bump and the measured size — `CANTUS_GOLDEN_DUMP`);
  block-size independence at tolerance 0; offline render equals real time; parallel
  (`CANTUS_WORKERS`) equals serial. Deterministic maths: `cantus_dsp::math::*`.
- **Start-up wiring goes in `boot`'s stages / `CantusApp::assemble`**, never only in
  `CantusApp::new` (the shipped app never calls it), with a test that fails if it leaves
  that path. Reviewers ask "is this reachable from `cantus_ui::run`?".
- UI: the hand-drawn kit (ADR 0024) — no stock egui widgets in views, no literal colours,
  no `FontId::new`. FL Studio is the visual reference, with Ableton's and Bitwig's ideas
  where they are better; clear label/value units, nothing that overlaps, panels resizable,
  every overflowing area scrollable, both themes (Slate and Paper), 75–200 %, keyboard focus
  and the screen reader (AccessKit) as elsewhere, the frame budget held (the budget tests).
  **Look at it**: shoot what you build and put the best in `docs/round34/<name>/`.
- A model change is a schema bump (`SCHEMA_VERSION`, a migration even if it writes nothing,
  `cargo run -p cantus-project --example write_fixture` for `fixtures/vN.cantus`; every old
  fixture keeps loading); every `Command` has an exact inverse, an undo-fuzz arm and a
  `cmd.*` string (both locales; the history panel names it); a new default binding is a
  keymap bump with its migration.
- **One build works everywhere** (WASAPI and ASIO; Linux; macOS): no feature-gated editions.
  Code for macOS compiles on the macOS runner (`macos.yml`); mark what only a real Mac can
  confirm for `docs/mac-session.md`.
- Licences (ADR 0047) as in the contract above. A new dependency needs an approved licence
  and a notice (`cargo run -p xtask -- notices`; a test fails otherwise). A model's weights
  need a licence that allows redistribution and commercial use of the output (ADR 0042: CC0,
  CC BY, MIT, Apache-2.0, BSD; never NC, ND or SA), checked at the source and written down.
- Strings in both locales (your area file); the manual (`docs/manual/`, EN and ES) for
  every feature a user touches; your ADR; `docs/round34/<name>.md`: what was done, how it
  was tested, the numbers, the marked lines, and **only** what needs the owner.

## mcp — the server, its doors and its guard rails (ADR 0081)

Read: `research/ai/mcp-server.md` (all), `research/ai/legal.md` §5 and §7–8,
`research/ai/cantus-today.md`, ADR 0004, `cantus-core/src/bus.rs` (execute, merging,
history, audit), `cantus-ai/src/{tool,provider,bridge}.rs`.

**First delivery (tools starts from it — make it early and solid):**
1. `crates/cantus-mcp` (GPL): JSON-RPC 2.0; `initialize`/`initialized` and the 2026-07-28
   stateless form (`_meta` per request, `server/discover`), `ping`, `tools/list`
   (deterministic, from the registry, with `title`, annotations from the class,
   `inputSchema`/`outputSchema`), `tools/call` (`content` + `structuredContent`, `isError`
   on tool errors), `resources/list|read|templates/list`, `prompts/list|get`, notifications
   accepted, unknown methods `-32601`. Protocol versions negotiated: 2025-06-18, 2025-11-25,
   2026-07-28. Size limits on requests.
2. The UI-thread pump: requests queue to the UI thread (a channel the frame drains, and a
   repaint request so an idle window still answers), run against `ai_tools::registry()` with
   the real host (`ToolHost` for `CantusApp`), execute the plan with `Origin::Mcp`, answer
   through a oneshot. Jobs on worker threads with handles; `wait_job`, `cancel_job`,
   `list_jobs`. `begin_change`/`end_change` turn grouping, `undo_last_ai_action`,
   `list_ai_actions` (generic tools in a list file of your own inside `cantus-ui`,
   registered by one marked line in `ai_tools.rs` — the one exception to "nobody edits it").
3. Permissions by class with the user's grants (`settings.rs`, additive), rate limits.
4. An in-process client for tests (drives the protocol end to end without sockets).

**Then, the rest:**
- **Streamable HTTP** on `127.0.0.1` (configurable port, a fixed documented default):
  POST `/mcp` with JSON answers; GET answered as the spec allows when there is no stream;
  `Origin` validated (403), a bearer token (generated per install, regenerable, stored in
  the settings folder readable by the user only), `MCP-Protocol-Version` honoured. **Off by
  default**: *Settings ▸ AI ▸ Allow external agents*.
- **The stdio bridge** `cantus-mcp` (a binary beside `cantus.exe`): stdin/stdout to the
  running app (through the HTTP door with the token from a per-user discovery file, or a
  pipe — your call, in the ADR); "Cantus is not running — open it and allow external agents"
  as a clean MCP error when it is not.
- **The confirmation card** in the Cantus window, non-modal: "<client> wants to: <said>",
  with a preview (the planned command in words; ghosted notes in the piano roll if cheap),
  *Allow*, *Allow for this session*, *Deny*; a timeout answers "the user did not answer".
  Destructive and file tools always ask whatever a project's text says (prompt injection:
  names and notes are data, quoted).
- **Visible agent mode**: a chip in the top bar with the client's name while one is
  connected; one click disconnects and revokes. **The AI activity view**: the audit log by
  origin (refused calls included), jobs and their progress, *Undo all of this agent's
  changes* (stops at the user's own steps and says so).
- **Setup**: *Copy setup for…* Claude Code, Claude Desktop (and an `.mcpb` bundle if its
  manifest takes a native binary — check), Cursor, VS Code, Gemini CLI, LM Studio: the right
  snippet with the token. The manual's page (EN/ES).
- **Packaging**: the bridge in the Windows installer and zip, the Linux AppImage/tarball and
  the macOS bundle (`xtask package`, `release.yml`); stems adds the helper by marked lines.
- One switch turns the whole AI area off and hides it.
- **Tests**: protocol conformance per version (written from the spec), a golden
  `tools/list` (`CANTUS_TOOLS_DUMP=1` rewrites it — the integrator regenerates it after each
  merge), the token and `Origin` checks, the pump end to end (a call → one undo step → undo
  restores the project exactly), permissions per class, a refused call audited, the bridge
  with no app running. **A real client**: MCP Inspector's CLI
  (`npx @modelcontextprotocol/inspector --cli …`, if the proxy lets npm through) and
  `claude mcp add` + `claude mcp list` showing *connected* — no model tokens spent.

## tools — what an agent can do and know (ADR 0082)

Starts from mcp's first delivery. Read: `research/ai/mcp-server.md` §3 and §5, BACKLOG §7
("What an AI needs…"), `docs/round28/demo.md`, `cantus-ai/src/bridge.rs`, the parameter
tables (`cantus-core/src/fx/*.rs` `params()`, `cantus-core/src/inst/**` `NativeSpec`).

- **The catalogue** (about 30–45 tools, few and chunky, ids returned by every create,
  positions in bars:beats and beats): read (`get_project`, `get_track`, `get_clip_notes`,
  `get_mixer`, `get_device`, `get_selection`, `get_history`); transport (tempo and tempo
  points, metre, loop, markers, play/stop/locate — `Host`); tracks; instruments and their
  parameters by key in their units; notes and clips (add, replace, remove, transpose,
  quantise, humanise, move, duplicate, bounds); drums (pads, step patterns); the **session**
  (slots, scenes, launch settings); the mixer (strips, routing, sends, master); effects on
  any strip including the master (add, set, bypass, move, remove, replace); modulators and
  routes; **automation** (lanes, points in parameter units, shapes); presets
  (`list_presets`, `apply_preset`); comping; export (audio, MIDI, stems, DAWproject —
  `Files`, denied by default, into the project folder or a folder the user allowed). Every
  mutating tool is one `Command`.
- **The machine-readable catalogue of every unit** (`list_units`, `get_unit`, the resource
  `cantus://units`): keys, ranges, units, tapers, defaults, enumerations by name, presets and
  what they set, latency, typical uses, the model's limits up front (±12 dB trims, strip
  counts) — **generated from the parameter tables**, with a test that fails when an
  `EffectKind`, an `InstrumentKind` or a parameter is missing.
- **The music linter** (`lint_music`; the static analysis in `cantus-core/src/harmony*.rs`,
  MPL, ours): notes ringing across a chord change, clashes with the sounding chord, doubled
  notes, out-of-scale notes, notes outside an instrument's range; the round-28 demo's
  Fm7-under-E♭/G clash found by a test.
- **The chunky tools**, each one undo step: `write_part(track, chart, pattern, feel)`,
  `voice_chords(chart, voices, range)`, `pedal_from_chart`, `automate(target, points,
  shapes)`; the chart format documented in the schema (chord symbols per bar and beat).
- **Resources** (`cantus://project`, `tracks/{id}`, `clips/{id}/notes`, `mixer`, `units`,
  `history`) and **prompts** (*Explain my mix*, *Write a part*, *Arrange an intro*, *Tidy the
  project*, *Teach me this unit*; *Master for streaming* is listen's), both locales.
- **Tests**: every tool through mcp's in-process client; each mutating tool's undo restores
  the project exactly; a fuzz driving random tool calls and undoing them all
  (`undo_invariants.rs`, your stream); a "compose a song" test that builds a short piece only
  through tools (tracks, parts, chords, drums, automation, mix) and renders it.

## listen — hearing and numbers, then the mastering assistant (ADRs 0083, 0084)

Read: BACKLOG §7 ("What an AI needs…"), `docs/round28/demo.md`, `research/ai/
generative-audio.md` §6, `cantus-engine/src/{render,stems,stems_master,freeze_render}.rs`,
`cantus-dsp`'s loudness and Speculum, the export dialog's render path in `cantus-ui`
(hosted instruments included).

- **Inside Cantus (engine, MPL)**: a **range render from a saved state** — render section 5
  without 1–4: checkpoints of the processing state at section starts (markers or bars) taken
  during a full render and reused while nothing upstream changed; where a unit's state cannot
  be saved (a hosted plug-in), a measured pre-roll, said in the result. The test: a section
  rendered from its checkpoint equals the same frames of the full render **to the bit** for
  native units. **Per-strip analysis taps** in the offline render (Speculum's analysis
  headless on every strip, the sound untouched — a null test). **Live = offline, checked**:
  the render path an agent uses compiles and sends automation exactly as playback does; a
  test drives the real-time processor block by block and compares to the bit.
- **The tools** (`ai_tools_listen.rs`): `audition(range, solo/mute set, master on/off)` →
  loudness per section (integrated, short-term, LRA), true peak, octave spectrum against a
  reference curve (a few genre curves with their sources, or the user's WAV), stereo
  correlation, clicks at section boundaries, and a link to the WAV (in the project's cache);
  `analyse_tracks` (per-track loudness, spectrum, crest, peaks in the mix, not soloed);
  `masking_matrix` (who fights whom in which band); `set_mix_targets(per-track LUFS)`
  solving the trims as one undo step, within the model's limits. Long ones as jobs.
- **The mastering assistant** on the master channel (a panel opened from the master strip
  and the master chain): it listens to the mix (or a chosen range), shows what it hears
  (loudness, true peak, LRA, balance against the target curve, width, mono compatibility),
  takes a **platform target** (Spotify, Apple Music, YouTube, Tidal, Amazon, SoundCloud, CD,
  club — the published numbers with their sources and dates) and optionally a reference
  track, and **proposes a chain of our own effects** (EQ, compressor or multiband,
  saturation, stereo, limiter or maximiser — what the master chain offers) with their
  settings, applied as one undo step, every value editable by hand afterwards; A/B against
  the unmastered mix, loudness-matched; a `master_assist` tool for agents and the prompt
  *Master for streaming*. Matchering's ideas may be **re-implemented** from its paper and
  docs in a GPL crate, never copied into an MPL one. **Measured**: on the demo songs and a
  few references, the result within ±0.5 LU of the target and the true peak at or under the
  ceiling; the report says by how much.

## stems — a song into its parts (ADR 0085)

Read: `research/ai/generative-audio.md` §4, `research/ai/legal.md`, ADRs 0030, 0042, 0063
(the sandbox helper — its process and pipe patterns), `cantus-content` (manifests, fetch,
install).

**First delivery (transcribe starts from it):**
1. **The runtime, measured, chosen** (ADR 0085): HT-Demucs (MIT code and weights) — or a
   better model whose weights pass ADR 0042 — through `ort` (ONNX Runtime, its library
   downloaded on demand), `tract` or `candle`: speed for a 3-minute song on this CPU, memory,
   download size, op coverage, the three systems; GPU only as an option that falls back to
   CPU. Where the weights come from (source, hash, licence) and whether Cantus must host a
   converted file (then where, and its notice).
2. `crates/cantus-ml` and its `cantus-ml` binary: jobs over a pipe (start, progress,
   cancel, result), the helper started off the UI thread, a crash or a hang reported and
   cleaned up, nothing of it near the audio thread.
3. Models through the content manager: listed with size, licence (shown and accepted once),
   provenance card, hash; downloaded on demand, resumable; removable in Settings.
4. One separation end to end, from a test.

**Then, the rest:**
- **The feature**: right-click an audio clip (and a track, and a selection) → *Separate
  stems…* (4 stems; 6 if the model has them; quality or speed; the model's size and licence
  before the first download; progress; cancel) → new tracks under a group bus, each clip
  aligned to the source sample for sample, the original muted, not deleted — **one undo
  step**. Provenance in the model (schema 48), the "AI" badge on the clips, the source named.
  An agent tool `separate_stems` (a job).
- **Quality, measured**: SDR per stem on MUSDB18-HQ's test tracks (downloaded locally for
  measuring only, never committed, its licence noted) against the model's published numbers;
  our output against the reference implementation's on the same input (a faithful port is
  within a stated tolerance); a residual check (the stems' sum against the original). Tests
  that run in CI use a tiny synthetic model or a mix of our own rendered stems; full-model
  tests are `#[ignore]`d with how to run them, and the integrator runs them.
- Speed and memory on this machine in the report; the helper's crash and cancel tested.

## transcribe — audio to MIDI (ADR 0086)

Starts from stems' first delivery. Read: `research/ai/generative-audio.md` §5, ADR 0085 and
stems' first-delivery report, `cantus-project/src/midi/**` (the GM drum mapping), Pulsus.

- **Polyphony and melody**: Spotify's Basic Pitch (Apache-2.0) — the model, and its note
  post-processing re-implemented in Rust — in the helper, through stems' runtime; melody as
  a monophonic mode (Basic Pitch's top line, or a pitch tracker of our own if it measures
  better); pitch bends kept or dropped by choice.
- **Drums**: onsets detected and classified (kick, snare, hats, toms, cymbals) — a model
  whose weights pass ADR 0042, or our own DSP in `cantus-dsp/src/onset*.rs` (MPL) if it
  measures as well — mapped to GM drums and Pulsus's pads.
- **The feature**: *Convert to MIDI…* on an audio clip (melody, polyphonic, drums;
  sensitivity, shortest note, the project's tempo, quantise off by default) → a new
  instrument track (Unda or Exemplar; Pulsus for drums) with a MIDI clip aligned to the
  audio — one undo step; an agent tool `audio_to_midi` (a job).
- **Accuracy, measured**: note F-measure (onset ±50 ms, pitch ±50 cents, mir_eval's rules)
  on material rendered from known MIDI through our own instruments (reproducible: in CI with
  a small model, or `#[ignore]` with how to run), and on a public set whose licence allows
  local use; drums' F-measure per class; against Basic Pitch's reference output on the same
  input.

## Reports and reviews

- Finish each delivery with `docs/round34/<name>.md` (what, how tested, numbers, marked
  lines, owner-only items) and a short report to the integrator; commit everything.
- One read-only reviewer per branch, one per fix round, one on the integrated tree. Every
  finding is fixed with a test that fails without the fix (or the reason none can).
