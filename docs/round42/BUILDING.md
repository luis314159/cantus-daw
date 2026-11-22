# Round 42 — the project's tuning, and Arcus the bowed string

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians 3:23).
Round 42 is `docs/next-session.md`'s proposal, decided by the integrator under the owner's
mandate of 2026-11-22 (*"de todo lo pendiente decide lo que busque la excelencia"*):

1. **Microtuning** (BACKLOG §0.2; ADR 0155 the project's tuning, ADR 0156 MTS-ESP).
2. **Strings** (BACKLOG §3e, the owner's ask of 2026-11-20): **Arcus**, the modelled bowed
   string — the cello and the double bass first (ADR 0157); the **recorded strings** on
   Exemplar and the **note's articulation and bow** (ADR 0158).

The effect the proposal offered "if it fits" (§6l.B: the multiband delay) waits for round 43:
two features with four builders is the round's size (CLAUDE.md, *rounds stay small*).

Research, verified on 2026-11-22: `research/instruments/microtuning-verified.md`,
`research/instruments-next/strings-verified.md`; where each instrument turns a key into a
pitch: `docs/round42/pitch-map.md`. The briefs: `docs/round42/briefs.md`.

**The owner's rules:** every gap a feature leaves is closed in the same round; list as open
only what needs the owner (ears, eyes, hardware, a lawyer). **The owner accepts**; agents find
defects but do not decide. Configurable with a sensible default. **Show progress often**:
pictures and sound files where the integrator can hand them to him, and say what he should
look at or listen to. **A design sheet goes to the owner before a full editor.**

## The contract

Base: `r42/contract` (master `7eb64c6e` + beta.17's version commit + this contract). Every
contract line in a shared file carries **`// r42:contract`**; a line one builder owns carries
**`// r42:<name>`**. Find anchors by the marker; line numbers drift.

- **Schema 80** — `Project::tuning: Tuning` and `Track::tuning: TrackTuning`, both defaulted
  and not written at their defaults; `fixtures/v80.cantus` (a project in Werckmeister III in
  E flat at 415 Hz, one track `Equal`, one `Own` 31-EDO).
- **Schema 81** — `InstrumentKind::BowedString` (Arcus: a silent stub, `READY` false, **not
  in `InstrumentKind::ALL`**) and `Note::art: Option<NoteArt>`; `fixtures/v81.cantus`.
- **Schema 82** — reserved for `strings-rec` (Exemplar's articulation map, if it needs one).
- `cantus-core::tuning` — the types of ADR 0155, `Tuning::offsets()` (complete for `Equal` and
  the diapason; the rest `tuning` fills), `TuningTable`; the commands `SetProjectTuning`,
  `SetTrackTuning`, `SetNoteArt`, applied and inverted (done) with undo-fuzz arms (done).
- `cantus-engine`: `InstrumentUnit::set_tuning(&TuningTable)` and `note_art(id, NoteArt)`,
  no-ops by default; `ExternalTuning` (the MTS-ESP seam, a no-op implementation).
- Locale areas `areas/tuning.{en,es}.txt`, `areas/arcus.{en,es}.txt`,
  `areas/strings_rec.{en,es}.txt`; Arcus's stub views and face placeholder.

## Who owns what

One worktree per builder, `F:\projects\daw-r42\<name>`, branch `r42/<name>` from
`r42/contract`. **Touch only what you own**; anything else, say so in your report.

| Builder | Owns | ADR | Schema | Locale |
|---|---|---|---|---|
| `tuning` | `cantus-core/src/tuning*.rs` (the parsers `tuning_scala.rs`, `tuning_mts.rs`, the tables `tuning_temper.rs`), `cantus-engine/src/processor*.rs` **this round**, `cantus-engine/src/tuning*.rs`, the insertion lines in every pitched instrument and effect (`r42:tuning`, per `pitch-map.md`), `cantus-host`'s note-on tuning, `tools/test/scala-archive.sh`, `crates/*/tests/*tuning*` (not the UI's), the MIT `tuning-library` test files and their notice | 0155 | 80 (done) | — |
| `tuning-face` | `cantus-ui/src/views/tuning*.rs` (Project ▸ Tuning, the circle, the audition), the track inspector's override row (`r42:tuning-face`), Chorda's Edit ▸ Tuning section, `cantus-rt/src/mts*.rs` and the engine's `ExternalTuning` implementation, the fake-`LIBMTS` test library, `docs/manual/{en,es}/tuning.md`; and round 41's leftover: the picker's pictures warmed during idle after start-up (BACKLOG §6j) | 0156 (0155's surface) | — | `areas/tuning.*` |
| `arcus` | `cantus-core/src/inst/arcus*.rs`, `cantus-dsp/src/arcus*.rs` (new), `cantus-engine/src/inst/arcus*.rs`, `cantus-ui/src/views/inst/arcus*.rs`, its skin sheet, its presets, `tests/*arcus*`, `docs/manual/{en,es}/arcus.md`, `docs/round42/arcus.md`; round 41's leftover: Fides's bowed pitch (a model version of Fides, its goldens moved only in that commit, with `DSP_VERSION` 12 if a stored default changes) | 0157 | 81 (done) | `areas/arcus.*` |
| `strings-rec` (wave 2, starts when a build slot frees) | `cantus-core/src/note_art*.rs`, the piano roll's articulation lane (`views/piano_roll*` lines marked `r42:strings-rec`), the engine's delivery of `note_art` (with `tuning`'s OK in `processor*.rs`), `cantus-dsp/src/exemplar_art*.rs`/`exemplar_legato*.rs` (new) and marked lines in `exemplar*.rs`, the packs' manifests and presets, `tools/content/strings/` (loop finding, the octave fix), `tests/*strings_rec*` | 0158 | 82 (reserved) | `areas/strings_rec.*` |

**Not owned by anyone** (ask the integrator): `fides*.rs` beyond Arcus's public reuse and
the bowed-pitch commit, `chorda*.rs` beyond `tuning`'s marked insertion, `gravis*`,
`pulsus*`, `tympanum*`, Cargo manifests beyond a dependency's line.

**Shared files** (additive, each line marked): `cantus-core/tests/undo_invariants.rs`,
`cantus-ui/src/i18n.rs`, `cantus-engine/tests/{inst_contract,rt_memory}.rs`,
`tests/support/heavy.rs`, `cantus-bench/src/kernels.rs`, `cantus-ui/src/preset_measure.rs`,
`cantus-ui/tests/preset_audit.rs`, `cantus-ai/src/tools_args.rs` (an `INSTRUMENTS` row for
Arcus; a `set_tuning` tool if `tuning-face` adds one), `cantus-ui/tests/golden/mcp-tools-list.json`,
`web/src/units_data.rs` (a card when done; mind the wasm budget, 250 000 B gzip),
`docs/BACKLOG-excellence.md` (append), `docs/beta-checklist.md` (a *Ronda 42* item under your
name).

Numbers are reserved above. Need another? Ask — **never take the next free one**.

## Rules

- Work only in your worktree. Commit WIP often on `r42/<name>`; a cut session resumes. Files in
  pieces under ~300 lines per tool call. Stage by explicit path, never `git add -A`, `.` or a
  folder; never your `.cargo/`.
- Machine: `export PATH="$HOME/.cargo/bin:$PATH"`; an untracked `.cargo/config.toml` with
  `[build] jobs = 2`, no debug info, incremental off. **Every cargo command through
  `bash /f/projects/cantus/tools/build/slot.sh cargo …`** (three slots machine-wide, each waiting
  for ≥ 3 GB free). `df -h /f /c` before builds. Long commands in the background. Test
  executables named `*setup*`/`*update*` need elevation: run them copied to a neutral name. The
  Bash tool is Git Bash: a heredoc delimited by `EOF` fails — use `XEOF` or the Write tool.
- Downloads go to `F:\projects\daw-r42\downloads\<name>\`, never into a worktree. The proxy
  (Zscaler) may hold a download ~10 minutes and answer with an HTML page: check what arrives
  (a zip starts with `PK`), wait, retry — **never bypass the proxy or TLS checks**. Licences:
  `cantus-dsp`, `-core`, `-engine`, `-project`, `-rt` are MPL-2.0 — read GPL code, never copy it
  (Surge XT's code and scale library are GPL; its `tuning-library` is MIT); dependencies
  MIT/Apache/BSD/ISC/Zlib/0BSD only.
- **No CPU claim measured while others build**: `tools/test/ab-compare.sh` gives costs side by
  side; say the load; the integrator measures quiet.
- Windows: clippy `--all-targets` with `RUSTFLAGS=-D warnings` and the tests of every crate you
  touch; `cargo fmt -p <crate>` before each commit. **No WSL checks by builders**; mind Linux
  yourself (`#[cfg(windows)]` code leaves Linux-only dead-code warnings).
- **CLAUDE.md applies word for word**: every mutation a `Command` with an exact inverse and an
  undo-fuzz arm; tests first; real time (`rt_memory.rs`); null tests (a default touches no
  sample, to the bit; block sizes at tolerance 0; offline = real time; parallel = serial);
  deterministic maths (`cantus_dsp::math`); no other unit's golden hash moves; strings in both
  locales; the hand-drawn kit; **an end-to-end test per feature** through
  `CantusApp::assemble` and the real engine; formats and protocols tested with bytes from
  outside sources; timing tests compare and take turns (`docs/RELEASING.md` L3–L5).
- **Tools stay in the repository** (the owner, 2026-11-20): every script you use to test,
  listen, render or measure goes under `tools/` with a usage header and a line in
  `docs/DEVELOPING.md` §4.
- **Look at what you build** (`cargo run -p cantus-ui --example shoot -- <dir> …`) and **listen**
  (render to WAV); the best pictures and sounds go to `docs/round42/<name>/` (sounds: say where,
  they stay out of git if large).
- Finish with `docs/round42/<name>.md`: what was built, measured, verified only in emulation,
  and **Left** (only owner items).
