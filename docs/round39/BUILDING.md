# Round 39 — UX and graphics, the whole window (BACKLOG §6j), and the memory row

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians 3:23).
Round 39 is **UX and graphics**, judged by the owner on **whole windows**, in both themes —
not by crop judges. The owner, 2026-11-08: *"FL aún se ve a años luz de Cantus … hay botones
que se ven super feos"*; *"vamos por lo difícil, a mano, que nos dé los mejores resultados"*.
And 2026-11-09: *"esto es muy muy importante, pon varios agentes a trabajar en cada aspecto a
mejorar … pon más agentes a hacer los pendientes"*. Read **BACKLOG §6j** first (the
diagnosis, the button study, the overflow panel, how the round works).

**The owner's rule:** every gap a feature leaves is closed in the same round; list as open
only what needs the owner (eyes, ears, hardware, a decision). **The owner accepts**; agents
find defects but do not decide.

## Who owns what

Base: master `80d5f109` (beta.13). One worktree per agent, `F:\projects\daw-r39\<name>`,
branch `r39/<name>`. **Touch only what you own**; anything else, say so in your report.

| Agent | Kind | Owns | ADR | Schema | Locale area |
|---|---|---|---|---|---|
| `dir-a`, `dir-b`, `dir-c` | design (no cargo) | `docs/round39/design/<name>/**`, `research/ui/round39/<name>/**` | — (the chosen one becomes ADR 0111) | — | — |
| `icons` | design (no cargo) | `docs/round39/design/icons/**`, `research/ui/round39/icons/**` | — | — | — |
| `ux-a` | builder | the automated-control sign, the help line, truncation: `kit/hint.rs`, `views/hint.rs`, the sign in `kit/knob.rs`/`kit/fader.rs`/`kit/automate.rs`, text fixes in any view (one-line layout or wording changes only) | **0109** | none expected | `areas/ux_signs.{en,es}.txt` |
| `ux-b` | builder | panels, keyboard, click paths: `kit/shell.rs`, the dock/layout code (`app.rs` layout parts, `views/appearance_panels.rs`, `appearance_file.rs`), the keymap and the command palette | **0110** | **60** if a layout must live in the project (prefer settings) | `areas/ux_panels.{en,es}.txt` |
| `memory` | builder | `crates/cantus-engine/**` (processor, events, pools), the Pulsus kit's engine side, `crates/cantus-ui/tests/memory_e2e.rs` | **0107** | 61 if needed (avoid) | — |
| (later) `kit` | builder | `crates/cantus-ui/src/kit/**` once the owner has chosen a direction and approved sheets | **0108** (how the kit draws) | — | — |

ADR 0111 (the design system: palette, type, spacing, the button and icon family) is written
by the integrator from the direction the owner chooses.

## Rules (the usual ones, short)

- Work only in your worktree. Commit WIP often on `r39/<name>`; a cut session resumes.
  Write files in pieces under ~300 lines per tool call. Stage by explicit path, never
  `git add -A`, `git add .` or a folder; never `git add` your `.cargo/`.
- Machine: `export PATH="$HOME/.cargo/bin:$PATH"`; your untracked `.cargo/config.toml` sets
  `jobs = 3`, no debug info, incremental off. `TMP`/`TEMP` = `F:\projects\daw-r39\tmp`.
  **Every cargo command through `bash /f/projects/daw-r39/slot.sh cargo …`** (three build
  slots on a 31 GB machine; `--wsl` for the one WSL slot). Run long commands in the
  background. Test executables named `*setup*`/`*update*` may need elevation: run them copied
  to a neutral name. The Bash tool is Git Bash: a heredoc delimited by `EOF` fails — use
  `XEOF` or the Write tool.
- Downloads go to `F:\projects\daw-r39\downloads\<name>\` (fonts to `F:\projects\daw-r39\fonts\`),
  never into a worktree. A corporate proxy (Zscaler) may hold a download ~10 minutes and answer
  with an HTML page: check what arrives (a zip starts with `PK`), wait, retry — **never bypass
  the proxy or TLS checks**.
- **Never run the scorecard's start-up measurement** (`tools/scorecard/startup.ps1`): the
  integrator measures on a quiet machine, with every agent stopped.
- Windows: clippy `--all-targets` with `RUSTFLAGS=-D warnings` and the tests of every crate you
  touch; `cargo fmt -p <crate>` before each commit (`cargo fmt --all` fails here, os error 206).
  **No WSL checks by builders this round** (C: has about 16 GB free and the WSL image lives
  there): the integrator runs Linux on the merged tree. So mind Linux yourself — code behind
  `#[cfg(windows)]` leaves Linux-only dead-code warnings; gate or `cfg_attr` it.
- **CLAUDE.md applies word for word**: every mutation a `Command` with an exact inverse and an
  undo-fuzz arm; tests first; real time (ADR 0005: the audio thread never allocates, locks,
  blocks or does I/O — `rt_memory.rs`); null tests (a default touches no sample, to the bit;
  `DSP_VERSION` stays 6); start-up wiring in `boot`/`CantusApp::assemble` with a test through
  it; strings in both locales; the hand-drawn kit (no stock egui widgets in views, no literal
  colours, no `FontId::new`); **an end-to-end test per feature** through `assemble`.
- **Look at what you build**: `cargo run -p cantus-ui --example shoot -- <dir> …` renders PNGs
  on the CPU; read them. Put the best in `docs/round39/<name>/`.
- **Images:** anything that shows FL Studio (or any other product) goes under
  `research/ui/round39/<name>/` and is added with `git add -f` (that tree is private and is
  stripped from the public repository); Cantus-only pictures go under `docs/round39/`.
- Finish with `docs/round39/<name>.md` (what changed, how it is tested, what was verified
  only in emulation, what is left) and a short report to the integrator.

## Shared facts

- The app's real current look: `docs/manual/img/startup.png` (1280×720, Night),
  `docs/round37/finish/*.png` (controls, headers, icons, faders at 100/150 %, both themes).
- FL Studio 24.1.2 screenshots: `F:\projects\cantus\research\ui\fl-look-2026-11\screenshots\`
  (playlist, mixer, channel rack, piano roll, parametric EQ) and its `notes.md`. The owner's
  crops: `research/ui/owner-crops-2026-11-08/` (in the worktree). FL Studio is installed
  (`C:\Program Files\Image-Line\FL Studio 2024\FL64.exe`); REAPER at `F:\projects\daw-r37\reaper\`.
- **Rendering design sheets**: write SVG (inside an HTML page when it needs a web font) and
  render it with Edge headless at the scale you need, e.g.
  `"/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe" --headless=new --disable-gpu
  --hide-scrollbars --force-device-scale-factor=4 --window-size=W,H
  --screenshot="F:\\…\\out.png" "file:///F:/…/sheet.html"` (window size in CSS pixels; the
  PNG is W×H times the factor). Then read the PNG and iterate.
- Fonts must be OFL (or equally free); record the licence, version and source URL; nothing
  is shipped until the kit builder adds it with its notice (`cargo run -p xtask -- notices`).
