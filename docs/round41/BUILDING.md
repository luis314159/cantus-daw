# Round 41 — Fides, Nitor and Fragor's circuits as pedals

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians 3:23).
He chose this round's three builds on 2026-11-20 (memory: `decision-round41-content`) and
named the two new units: **Fides**, the resonator instrument (ADR 0150), and **Nitor**, the
one-knob enhancer (ADR 0151); and **Fragor's five circuits as pedals of their own** on
Tabula's board (ADR 0152), his ask of 2026-11-18. Microtuning moved to round 42. The briefs:
`docs/round41/briefs.md`.

**The owner's rules:** every gap a feature leaves is closed in the same round; list as open
only what needs the owner (eyes, ears, hardware, a decision). **The owner accepts**; agents
find defects but do not decide. He likes things **configurable with a sensible default**.
**Show progress often**: put pictures and sound files where the integrator can hand them to
him, and say in your report what he should look at or listen to. **A design sheet goes to the
owner before a full editor** (faces: hand-drawn, unlike every other unit's, both themes).

## The contract

Base: master `80ead93f` plus this contract (`r41/contract`). It declared every new kind as a
**stub** wired into every list and dispatch, so a builder replaces stubs and never adds
dispatch:

- `InstrumentKind::Resonator` (Fides): silent, `READY` false, **not in `InstrumentKind::ALL`**
  until it sounds (the `fides` builder adds it, with the null corpus's new item measured).
- `EffectKind::Enhancer` (Nitor) and `EffectKind::FragorOverdrive … FragorMuff`, appended to
  `EffectKind::ALL` (49; never reorder — board presets store a kind by index): wires times
  `out` (dB, default 0, a null to the bit). The five pedals are **board-only**
  (`offered()` false, `pedal_allowed` true; Tabula's shelves read
  `picker_catalog::board_devices`). Nitor is exported (its CLAP/VST3 ids are blessed and
  forever); the pedals are not.
- **The song's key reaches every effect** (ADR 0153, done): `FxUnit::set_song_key`, called
  after each spec on a strip, on the master, on a restarted plug-in and through a board.
- **Schema 78** (done): an empty step, `fixtures/v78.cantus` with a Fides track, a Nitor and a
  board of the five; its test.
- Locale files, empty preset files with floors of 0, placeholder faces (Nitor's
  `kit/skin/side_r41.rs`; Fides on Gravis's sheet), the pedals' Tabula face rows.

Every contract line in a shared file carries **`// r41:contract`**; a line that belongs to one
builder carries **`// r41:<name>`** (`r41:fides`, `r41:nitor`, `r41:pedals`). Find your anchors
by the marker — line numbers drift.

## Who owns what

One worktree per builder, `F:\projects\daw-r41\<name>`, branch `r41/<name>` from the contract
commit. **Touch only what you own**; anything else, say so in your report.

| Builder | Owns | ADR | Schema | Locale |
|---|---|---|---|---|
| `fides` | `crates/cantus-core/src/inst/fides*.rs`, `crates/cantus-dsp/src/fides*.rs` (new), `crates/cantus-engine/src/inst/fides*.rs`, `crates/cantus-ui/src/views/inst/fides*.rs`, its skin sheet (a new file under `kit/skin/`), its presets (`presets/instruments/resonator.toml` or `FACTORY_PRESETS`), `tests/*fides*`, `docs/manual/{en,es}/fides.md`, its marked lines (`r41:contract` → `r41:fides`) in the shared files below; the commit that puts `Resonator` in `InstrumentKind::ALL` and the picker | 0150 | 78 (done; a new number only by asking) | `areas/fides.{en,es}.txt` |
| `nitor` | `crates/cantus-core/src/fx/nitor*.rs`, `crates/cantus-dsp/src/nitor*.rs` (new), `crates/cantus-engine/src/fx/nitor*.rs`, `crates/cantus-ui/src/views/fx/nitor*.rs`, `kit/skin/side_r41.rs`, `presets/effects/enhancer.toml`, its plug-in test arms, `docs/plugins/nitor.md`, `tests/*nitor*` | 0151, 0153 (amend if needed) | 78 (done) | `fx/nitor.{en,es}.txt` |
| `pedals` | `crates/cantus-core/src/fx/fragor_pedal*.rs`, `crates/cantus-engine/src/fx/fragor_pedal*.rs`, the five pedals' rows in `views/fx/tabula_faces.rs`, the per-circuit drawing in `views/fx/tabula_board.rs`/`tabula.rs`/`tabula_edit.rs` (Fragor's special cases extended to the five kinds), `presets/effects/fragor*.toml`, a new factory board in `cantus-core/src/fx/tabula_presets.rs`, `tests/*pedal*` | 0152 | 78 (done) | `fx/fragor_pedal.{en,es}.txt` |

**Not owned by any builder** (ask the integrator): `cantus-dsp/src/fragor*.rs` (Fragor's
circuits: a pedal wraps `Fragor`, it does not change it — its golden hashes must not move),
`gravis*.rs`, `pulsus*.rs`, `tympanum*.rs`, `chorda*.rs` (reuse their public functions; to
expose something private, ask), `processor*.rs` (the integrator's this round), Cargo manifests
beyond a new dependency's line.

**Shared files** (additive, each line marked, conflicts resolved by the integrator):
`crates/cantus-core/tests/undo_invariants.rs` (arms at the end, each block marked
`// r41:<name>`), `crates/cantus-ui/src/i18n.rs` (lists every area already),
`crates/cantus-engine/tests/{fx_contract,inst_contract,rt_memory}.rs` and
`tests/support/heavy.rs`, `crates/cantus-bench/src/kernels.rs`, `crates/cantus-plugins/tests/common/mod.rs`,
`crates/cantus-ui/src/preset_measure.rs` (a budget line), `crates/cantus-ui/tests/preset_audit.rs`
(delete your floor-0 line when your presets land), `crates/cantus-ai/src/tools_args.rs` (the
`INSTRUMENTS` row for Fides; aliases), `crates/cantus-ui/tests/golden/mcp-tools-list.json`
(regenerate with `CANTUS_TOOLS_DUMP=1` when your words change, say so),
`web/src/units_data.rs` (a card when your unit is done; the site's manual test needs a page
linked from `/learn`), `docs/BACKLOG-excellence.md` (append only), `docs/beta-checklist.md`
(append a *Ronda 41* item under your name).

Numbers are reserved above. If you need one you were not given, say so — **never take the next
free one**.

## Where the `_` arms hide

A match with a `_ =>` arm compiles without your kind, so the compiler does not warn. The
contract added these by hand; when you change behaviour, check them: `EffectKind::takes_notes`,
`choices`, `takes_ir`, `is_look` (`effect.rs`); `fx::make` (engine); the window list,
`code_presets`, `Editor::of` and the per-kind dispatches in `views/fx/mod.rs`;
`frame::designed_size`, `picker_snap::effect_name`, `kit/skin/side.rs` `of`/`colourways`;
`InstrumentKind::stored_versions`, `automatable`, `trim_info` and `views/inst/frame.rs`'s `hue`;
`listen_fork`'s cost tables; `starting::start_of` (flat for every stub now: each builder sets
its starting preset, ADR 0071).

## Rules

- Work only in your worktree. Commit WIP often on `r41/<name>`; a cut session resumes. Write
  files in pieces under ~300 lines per tool call. Stage by explicit path, never `git add -A`,
  `git add .` or a folder; never your `.cargo/`.
- Machine: `export PATH="$HOME/.cargo/bin:$PATH"`; an untracked `.cargo/config.toml` with
  `jobs = 3`, no debug info, incremental off. **Every cargo command through
  `bash /f/projects/cantus/tools/build/slot.sh cargo …`** (three build slots machine-wide on
  31 GB). `df -h /f /c` before builds. Run long commands in the background. Test executables
  named `*setup*`/`*update*` may need elevation: run them copied to a neutral name. The Bash
  tool is Git Bash: a heredoc delimited by `EOF` fails — use `XEOF` or the Write tool.
- Downloads go to `F:\projects\daw-r41\downloads\<name>\`, never into a worktree. A corporate
  proxy (Zscaler) may hold a download ~10 minutes and answer with an HTML page: check what
  arrives, wait, retry — **never bypass the proxy or TLS checks**. Licences: `cantus-dsp` is
  MPL-2.0 — read GPL code, never copy it; dependencies MIT/Apache/BSD/ISC/Zlib only.
- **No CPU measurement for a claim while others build**: measure with tools/test/ab-compare.sh
  (its costs side by side) and say the machine's load; the integrator measures quiet.
- Windows: clippy `--all-targets` with `RUSTFLAGS=-D warnings` and the tests of every crate you
  touch; `cargo fmt -p <crate>` before each commit (`cargo fmt --all` fails here, os error
  206). **No WSL checks by builders** (the WSL image lives on C:): the integrator runs Linux on
  the merged tree. Mind Linux yourself — code behind `#[cfg(windows)]` leaves Linux-only
  dead-code warnings.
- **CLAUDE.md applies word for word**: every mutation a `Command` with an exact inverse and an
  undo-fuzz arm; tests first; real time (ADR 0005 — `rt_memory.rs`); null tests (a default
  touches no sample, to the bit; block-size independence at tolerance 0; offline = real time;
  parallel = serial); deterministic maths (`cantus_dsp::math`); no golden hash of another unit
  moves and `DSP_VERSION` stays 10 (a new unit is a new item in the null corpus, measured);
  strings in both locales; the hand-drawn kit (no stock egui widgets in views, no literal
  colours, no `FontId::new`); **an end-to-end test per feature** through `CantusApp::assemble`
  and the real engine; formats and protocols tested with bytes from outside sources.
- **Presets are made by ear** and handed to the owner to listen to (render them with
  `examples/render_unit_sounds` or the preset audition, and put the files where the integrator
  can send them). Levels fitted with `examples/preset_tool -- fit`.
- **Look at what you build**: `cargo run -p cantus-ui --example shoot -- <dir> …` renders PNGs
  on the CPU; read them. Put the best in `docs/round41/<name>/`.
- **Every tool you write goes in the repository** (`tools/…`, named in `docs/DEVELOPING.md`),
  the owner's standing rule since 2026-11-20.
- Finish with `docs/round41/<name>.md` (what changed, how it is tested, what was verified only
  in emulation, what the owner should look at or listen to, what is left) and a short report.
