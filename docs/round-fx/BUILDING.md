# The effects side round — Cribrum, Fragor, Lapsus, Vetus and Libra

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians 3:23).
Tests before code, nothing half done, look at what you build.

**What the owner asked (2026-11-13)**, in a second session beside round 40: more effects that
make Cantus complete (BACKLOG §3b), built in parallel without getting in round 40's way. He
chose five units, and added: ***"que cada efecto tenga un panel con personalidad propia y sus
gráficos bien pulidos y profesionales como FL Studio y otros"*** — every effect a panel with a
personality of its own and polished, professional graphics, as FL Studio's and the others'.

**The owner's rules:** every gap a feature leaves is closed in the same round; open only what
needs the owner (his ears, his hardware, a decision). **The owner accepts**; agents find
defects but do not decide. Configurable with a sensible default. **Show progress often** —
pictures (and, for sound, WAV/FLAC files) in `C:\projects\daw-fx\compartir\<name>\`, and say
in your report what he should look at or listen to.

Read, in order: this file (the rules and **your** section of `briefs.md` completely);
`CLAUDE.md`; `docs/round28/BUILDING.md` (*Rules* — they all hold here, except where this file
says otherwise); `docs/BACKLOG-excellence.md` §3b; ADRs 0017, 0018, 0055, 0056, 0071, 0074,
0101, 0106, 0112, 0115 and your own stub ADR.

## Why this round is apart, and what that means for you

Round 40 runs at the same time in `F:\projects\daw-r40\*` (another session, schema 62–70,
ADRs up to 0126). This round builds on **master** (`dd4ad68b`, beta.14) and is merged **after
round 40 lands on master**, as round-41 material. So:

- **Never touch** `F:\projects\cantus` (the owner's tree, with his uncommitted files) or
  anything under `F:\projects\daw-r40\`; never push, never merge, never change a remote.
- **Schema 62 and ADRs 0130–0133 are provisional**; the integrator renumbers them at the merge.
  Do not write those numbers into user-facing text (manual, site, help lines).
- **Stay out of what round 40 is changing**: Delay and its window, Chorda, Tympanum, Pulsus's
  pads, the arrangement's ripple, the scenes, the dancer, memory work. If your unit seems to
  need a change there, say so in your report instead.
- In shared files, edit **only your own line or block**, marked `// fx:<name>`. Prefer a new
  file of your own over growing a shared one.

## The builders

**Never more than two compile at once** (RAM is shared with round 40's builders). Wave 2 starts
from `fx/integration` once wave 1 is merged into it.

| Builder | Wave | Branch / worktree | Delivers | Schema | ADR |
|---|---|---|---|---|---|
| **cribrum** | 1 | `fx/cribrum`, `C:\projects\daw-fx\cribrum` | Cribrum, the auto filter | 62 if needed | 0130 |
| **fragor** | 1 | `fx/fragor`, `C:\projects\daw-fx\fragor` | Fragor, the pedals (distortion and fuzz) | 62 if needed | 0131 |
| **lapsus** | 2 | `fx/lapsus`, `C:\projects\daw-fx\lapsus` | Lapsus, the time shaper | 62 if needed | 0132 |
| **vetus** | 2 | `fx/vetus`, `C:\projects\daw-fx\vetus` | Vetus (lo-fi) and Libra (utility) | 62 if needed | 0133 |

The **integrator** (the session that wrote this) merges each branch into `fx/integration`,
runs a review on it (≤ 6 agents per review step), sends confirmed findings back to their
builder, runs the suites and clippy on Windows and Linux, and asks the owner before anything
leaves `fx/integration`.

**The contract commit** on `fx/integration` declares the five kinds
(`EffectKind::{AutoFilter, Pedal, TimeShaper, LoFi, Utility}`, `ALL[26..31]`, never reorder)
as exact pass-through stubs with one placeholder parameter `out` (dB, default 0 — a null),
wired into every list, plus schema 62 (`fixtures/v62.cantus`), the per-stem locale files and
the stub ADRs. Its report (`docs/round-fx/contract.md`) lists, per stem, the files you own and
the anchor in each shared file. Rewrite `PARAMS` freely but **keep the `out` key** (an output level in dB, default 0): its
plug-in ids are blessed, and the golden ids are append-only (add yours with
`CANTUS_BLESS_IDS=1 cargo test -p cantus-plugins --test ids`).

## A panel with a personality of its own — the owner's bar for every unit

Every unit gets a **window that looks like itself** — the way Love Philter, Gross Beat,
Vocodex, FabFilter's Pro-Q or Soundtoys' Decapitator each read at a glance — **inspired,
never copied** (no shape, layout or colour value of a product reproduced). It is the skin
system (ADR 0074: material, display, knob and switch shapes, lettering, plate, ornaments) plus
**a characteristic display that is the unit's centre**, drawn with the kit's vector primitives,
live and interactive (dragged, not only watched). Delay's window (ADR 0115) and Aula's are the
house precedents; Aequalis's curve and Speculum's views the quality bar.

1. **Design first.** Before the full editor, a **design sheet** rendered by your `shoot`
   example: the whole window at 100 % in Slate (the default) and Paper, at rest and with audio
   playing, with your display, controls and preset bar; beside it, at the same scale, the
   reference products' windows (`research/fabfilter/screenshots`, `research/ui/`; search the
   web for public screenshots — keep them under `C:\projects\daw-fx\refs\`, never commit
   third-party pictures). Put it in `compartir\<name>\design\` and **stop for the owner's
   approval** (the integrator relays it); keep building DSP and tests meanwhile.
2. **What "polished and professional" means here**, checked on your own pictures before you
   show them: one focal point (the display); controls grouped by function with clear
   label/value units; nothing overlapping, truncated or colliding in English **and** Spanish,
   at 75 %, 100 %, 150 % and 200 %; live readouts that move smoothly at 60 Hz and rest calmly
   (no dead black slabs, no "−∞"); every theme legible (text 4.5:1, non-text 3:1 — the themes
   test); the frame budget held with the window open and audio playing (measure it).
3. **The whole window, not crops**: shots of the window as the owner will see it — opened
   from the mixer's insert, docked and floating — and its mini view under the insert in the
   inspector.
4. The exported CLAP/VST3 window is the same editor (`FxHost`), so check it there too.

## The family's rules (every unit)

As round 28's *Effects* rule, in full: a null test at the defaults (`crates/cantus-engine/
tests/fx_<stem>.rs`), the table-driven `fx_contract.rs` (block size 1/37/64/512 at tolerance
0, split invariance, exact rest twin), exact rest (`at_rest`/`skip`; a free-running LFO is
never at rest), latency through `FxUnit::latency`, oversampling where nonlinear (measured
aliasing), tempo sync from `SongPosition` (a render from bar 5 equals bars 5… of a full
render; offline = real time), deterministic maths, no allocation on the audio thread
(`rt_memory.rs`), a **starting preset** that makes the insert sound at once (ADR 0071 — the
parameter defaults stay a null), **factory presets made with care** (named, musical, ten or
more), every control a `Command` with undo and its parameter menu (automate, modulate, MIDI
learn), a page `docs/plugins/<stem>.md`, a manual section in English and Spanish, the AI
tools' unit description, the site's unit card (`web/src/units_data.rs`), and the CLAP/VST3
export bit-identical to the engine. **An end-to-end test** through `CantusApp::assemble`:
insert the unit from the picker on a playing track, change a control, hear it in the master
(not only unit tests). **Ours better than the reference**: measure against the reference's
published behaviour where it exists, and say how you measured.

## Machine and process

- Your worktree and branch only; the untracked `.cargo/config.toml` is there (`jobs = 4`,
  `debug = 0`, `incremental = false`, `TMP` on `C:\projects\daw-fx\tmp`). F: is nearly full —
  keep big files on C:.
- Git Bash has no cargo on `PATH`: `export PATH="$HOME/.cargo/bin:$PATH" &&`. A heredoc
  delimited by `EOF` fails to parse: use `XEOF` or the Write tool. UAC refuses test binaries
  named `*setup*`/`*update*` (os error 740): run a renamed copy. The Zscaler proxy may answer
  a download with an HTML page for ~10 minutes: verify what arrives, wait, retry; **never
  bypass the proxy or TLS checks**.
- **One `cargo` at a time** in your worktree; `-p <crate>` and `--test <name>` until the end,
  the workspace once at the end. Never measure performance while another build runs — say
  when you measured.
- Commit WIP early and often, staging by explicit path (never `git add -A`, `git add .`, or
  `.cargo/`). Write files in pieces under ~300 lines per tool call. Sessions get cut — you
  will be resumed with your context.
- **Finish:** `cargo test -p cantus-core -p cantus-dsp -p cantus-engine -p cantus-project
  -p cantus-plugins -p cantus-ui -p cantus-ai --no-fail-fast`; `cargo clippy --workspace
  --all-targets -- -D warnings`; `cargo clippy -p cantus-plugins --no-default-features
  --all-targets -- -D warnings`; `cargo fmt -p <crate>` for every crate you touched (`cargo fmt --all` fails on this machine with
  *filename too long*, os error 206), then `cargo fmt --all -- --check` if it runs. Then `docs/round-fx/<name>.md` (done /
  measured / tests and what each holds / shared files touched / verified only in emulation /
  *left* — empty or owner-only), your ADR, and a short final report.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
