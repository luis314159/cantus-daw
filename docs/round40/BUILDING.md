# Round 40 — Chorda by ear, insert/delete time and the ripple, the memory row, and the owner's answers

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians 3:23).
The prompt is `docs/next-session.md`. The owner answered round 39's twenty-two decisions on
2026-11-12 (`docs/round39/integration.md`, «The owner's answers») and took Chorda's listening
test (`docs/round39/after-beta14.md` §6: the Salamander preferred 7–0; model 1 preferred on
*Bass notes alone*). His direction: **Chorda as good as the Salamander**.

**The owner's rules:** every gap a feature leaves is closed in the same round; list as open
only what needs the owner (eyes, ears, hardware, a decision). **The owner accepts**; agents
find defects but do not decide. He likes things **configurable with a sensible default**.
**Show progress often**: put pictures (and, for sound, files) where the integrator can hand
them to him, and say in your report what he should look at or listen to.

## Who owns what

Base: master `dd4ad68b`. One worktree per agent, `F:\projects\daw-r40\<name>`, branch
`r40/<name>`. **Touch only what you own**; anything else, say so in your report. The
integration branch is `r40/integration` (this contract).

| Agent | Phase | Owns | ADR | Schema | Locale area |
|---|---|---|---|---|---|
| `chorda` | 1 | `crates/cantus-dsp/src/chorda*.rs`, Chorda's presets, the `chorda_abx`/`chorda_kit` tools, `DSP_VERSION` and the golden hashes it moves, `F:\projects\daw-r38\voicing\` (copy to `F:\projects\daw-r40\voicing\`) | 0098 (third amendment) | — | — (Chorda's existing keys only) |
| `ripple` | 1 | insert/delete time and the ripple: new `crates/cantus-core/src/ripple*.rs`, the section drag in the arrangement view, its commands and undo-fuzz arms, the keymap | **0116** | **63** only if a setting must live in the project (prefer none) | `areas/ripple.{en,es}.txt`; keymap 16 → **17** |
| `memory` | 1 | the private bytes outside the live heap: the allocator, thread stacks and spawning (`cantus-rt`, worker and service threads), the graphics set-up and its buffers, drivers' start-up (lazy where it is safe), `tools/scorecard/memmap.ps1`, `crates/cantus-ui/tests/memory_e2e.rs` — **not** `processor*.rs` | **0120** (and 0107 amended) | — | — |
| `look` | 1 | the owner's look answers: `crates/cantus-ui/src/kit/**`, the themes, the top bar, the shell (maximise), the mixer's right dock, Paper's menus and dialogs, the inspector's value bars, Mute's colour | 0110 / 0111 amended (no new ADR unless structural — then **0121**) | — (settings, not the project) | `areas/owner_look.{en,es}.txt` |
| `dancer` | 1 | design only (no cargo): `docs/round40/design/dancer/**` | — (the integrator writes it once approved) | — | — |
| `pads` | 2 | a pad's own mixer track (chosen per pad, as FL), *Hear the key* (sidechain audition), *Record: before FX · after FX*: the rack/pad model, `crates/cantus-project` (schema), **`crates/cantus-engine/src/processor*.rs`** and the recording tap | **0117** (pad's track), **0118** (hear the key, record before/after FX) | **62** | `areas/pads.{en,es}.txt` |
| `delay` | 2 | Delay's tempo-synced time (a new parameter, null at its default) and Delay as a CLAP/VST3 plug-in: `crates/cantus-dsp/src/delay*.rs`, Delay's engine unit, `crates/cantus-plugins/**`, `locales/fx/delay.*`, Delay's window | **0119** | — (a unit parameter goes in the unit's state; if it must be a schema bump, take **64**) | `fx/delay.{en,es}.txt` (existing) |
| `cast` | 2 | the twelve dancers the owner approved (`docs/round40/design/dancer/`): `crates/cantus-dance/**`, the dancer picker and companion window views, the default character | **0122** | **65** (the default dancer changes; old songs keep Fuga) | `areas/cast.{en,es}.txt` (register it in `i18n.rs`, marked `// r40:cast`) |

**Shared files** (additive, conflicts resolved by the integrator): `crates/cantus-core/tests/undo_invariants.rs`
(add your arms at the end of the dispatch, each block marked `// r40:<name>`), `cantus-ui/src/i18n.rs`
(already lists every area above), `docs/BACKLOG-excellence.md` (append only), `docs/beta-checklist.md`
(append a *Ronda 40* item under your name). **The inspector** is `look`'s; `pads` adds its rows in
its own file and a single call site in the inspector, marked `// r40:pads`. **`processor*.rs`** is
`pads`'s; `delay` asks `pads` (through the integrator) if it needs the transport there — the
engine already hands effects the tempo for other synced units, look first.

Numbers are reserved above. If you need one you were not given, say so — do not take the next free one.

## Rules (the usual ones, short)

- Work only in your worktree. Commit WIP often on `r40/<name>`; a cut session resumes.
  Write files in pieces under ~300 lines per tool call. Stage by explicit path, never
  `git add -A`, `git add .` or a folder; never `git add` your `.cargo/`.
- Machine: `export PATH="$HOME/.cargo/bin:$PATH"`; your untracked `.cargo/config.toml` sets
  `jobs = 3`, no debug info, incremental off. `TMP`/`TEMP` = `F:\projects\daw-r40\tmp`.
  **Every cargo command through `bash /f/projects/daw-r40/slot.sh cargo …`** (three build
  slots on a 31 GB machine). Run long commands in the background. Test executables named
  `*setup*`/`*update*` may need elevation: run them copied to a neutral name. The Bash tool is
  Git Bash: a heredoc delimited by `EOF` fails — use `XEOF` or the Write tool.
- Downloads go to `F:\projects\daw-r40\downloads\<name>\`, never into a worktree. A corporate
  proxy (Zscaler) may hold a download ~10 minutes and answer with an HTML page: check what
  arrives, wait, retry — **never bypass the proxy or TLS checks**.
- **Never run the scorecard's start-up or memory measurement for a claim** (`tools/scorecard/*.ps1`):
  the integrator measures on a quiet machine, with every agent stopped. `memory` may map a
  process (what each region is) while others run, and must say its numbers are not the
  scorecard's.
- Windows: clippy `--all-targets` with `RUSTFLAGS=-D warnings` and the tests of every crate you
  touch; `cargo fmt -p <crate>` before each commit (`cargo fmt --all` fails here, os error 206).
  **No WSL checks by builders** (C: has ~11 GB free and the WSL image lives there): the
  integrator runs Linux on the merged tree. Mind Linux yourself — code behind `#[cfg(windows)]`
  leaves Linux-only dead-code warnings; gate it or `cfg_attr` it.
- **CLAUDE.md applies word for word**: every mutation a `Command` with an exact inverse and an
  undo-fuzz arm; tests first; real time (ADR 0005 — `rt_memory.rs`); null tests (a default
  touches no sample, to the bit; only `chorda` moves `DSP_VERSION`, 6 → 7); a schema bump with a
  migration and `fixtures/vN.cantus`; start-up wiring in `boot`/`CantusApp::assemble` with a test
  through it; strings in both locales; the hand-drawn kit (no stock egui widgets in views, no
  literal colours, no `FontId::new`); **an end-to-end test per feature** through `assemble`.
- **Look at what you build**: `cargo run -p cantus-ui --example shoot -- <dir> …` renders PNGs on
  the CPU; read them. Put the best in `docs/round40/<name>/`. Every new or changed theme keeps
  ADR 0111's contrast (non-text 3:1, text 4.5:1 — `arrangement_legibility.rs`).
- **Images:** anything showing another product goes under `research/ui/round40/<name>/`, added
  with `git add -f` (private, stripped from the public repository).
- Finish with `docs/round40/<name>.md` (what changed, how it is tested, what was verified only in
  emulation, what the owner should look at or listen to, what is left) and a short report.
