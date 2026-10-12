# Building a native instrument (round 17)

The brief every instrument builder works from. The decisions behind it are ADR 0023; the
research is `research/instruments/`. Screenshots the reports cite are local and
git-ignored, in `C:\projects\daw\research\instruments\screenshots\` — open them with the
Read tool to see what you are beating.

## Where you work

- Your worktree `C:\projects\daw-inst\<name>` on branch `inst/<name>`. **Only there.** Never
  touch `C:\projects\daw` (the owner's working tree, full of uncommitted work), another
  worktree, `master`, or a remote. Never push.
- Commit early and often on your branch (a WIP commit in the first half hour). Messages end
  with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Write big files in pieces of under ~300 lines per tool call; a stream watchdog kills long
  single writes. In this machine's Bash a heredoc delimited by `EOF` fails; use the Write
  tool, or another delimiter.
- The machine is shared by about a dozen builders: builds are slow. Iterate with targeted
  runs (`cargo test -p cantus-dsp <filter>`), and run the whole workspace at the end.
- Do not work around anything the permission system blocks.

## What you own

Your scaffold files, one per layer, already dispatched (ADR 0023). Put more modules of your
instrument in files of their own, declared with `#[path = "…"] mod x;` **inside your own
files**, so no shared `lib.rs` / `mod.rs` changes:

| Layer | Files |
|---|---|
| model | `crates/cantus-core/src/inst/<kind>.rs` |
| DSP | `crates/cantus-dsp/src/<latin>.rs` (+ `<latin>_*.rs`) |
| engine | `crates/cantus-engine/src/inst/<kind>.rs`, tests `crates/cantus-engine/tests/inst_<kind>.rs` |
| editor | `crates/cantus-ui/src/views/inst/<kind>.rs` (+ `<kind>_*.rs`), tests `crates/cantus-ui/tests/inst_<kind>.rs` |
| strings | `crates/cantus-ui/locales/inst/<kind>.en.txt` and `.es.txt` — every English key needs its Spanish twin |
| document | `docs/instruments/<latin>.md` — what was built, the parameter table, what the tests hold, measured numbers, what is not done |

**Shared files you do not edit**: the dispatches (`inst/mod.rs` in each crate, `lib.rs`,
`views/mod.rs`), `command.rs`, `model.rs`, `snapshot.rs`, `processor.rs`, `backend.rs`,
`format.rs` (no schema bump — the coordinator does it at merge), `en.txt`/`es.txt`, the
frame (`views/inst/mod.rs`, `frame.rs`), the workspace `Cargo.toml`. If you truly need a
change there (a dependency in one crate's `Cargo.toml`, a decoder in `samples.rs`, a hook
in the frame), make it minimal, put it in its own commit whose message starts with
`SHARED:`, and explain it in your final report.

**Dependencies**: only MIT, Apache-2.0, BSD, ISC or Zlib crates (the core, DSP and engine
crates are MPL-2.0, ADR 0047); check the licence file. No code copied from GPL projects
(Vital, Surge, Dexed, OB-Xd, setBfree, sfizz is BSD but check) — read their documentation,
implement from the published literature.

## The contract and the house rules

Read `crates/cantus-engine/src/inst/mod.rs` (`InstrumentUnit`), `crates/cantus-core/src/
inst/mod.rs` and `modulation.rs`, `crates/cantus-dsp/src/voices.rs` and `modmatrix.rs`, and
`crates/cantus-ui/src/views/inst/{mod.rs,frame.rs}`. For the standard of finish, look at a
plug-in built the same way: `crates/cantus-dsp/src/spatium*.rs`,
`crates/cantus-engine/src/fx/space.rs`, `crates/cantus-ui/src/views/fx/space*.rs`,
`docs/plugins/space.md`.

1. **Deterministic**: identical output at every block size and live versus offline. Seed
   any randomness (drift, round robin, noise) from the note — its id, key, start — never
   from a global counter, a hit count that resets on seek, or the clock. Glides and
   modulation step on a fixed sample grid counted from the start of the stream (Aequalis
   does this, ADR 0017), never from the start of a block.
2. **Headroom**: an eight-note chord at velocity 127 stays under 0 dBFS; a typical preset
   peaks around −6 dBFS.
3. **Exact silence** once the notes are over: flush denormals, gate tails below about
   −140 dB, and return `false` from `process` only when you wrote exact zeros.
4. A control at its default changes no sample; a modulation route at depth 0 is skipped.
5. Keep `gain` (output, dB, −48…+12, default 0) — the fixture relies on it.
6. Real units: Hz, ms, s, dB, cents, semitones. No unlabelled 0…1 where a unit exists.
7. The audio thread never allocates, locks, does I/O or panics (ADR 0005): preallocate in
   `new`, build tables in `new` or on the UI side.
8. Use the shared pieces: the modulation model (macros + 32 slots; resolve source tags and
   target keys to your own indices in `configure`, evaluate with `cantus_dsp::modmatrix`),
   `cantus_dsp::voices::Allocator` for voices, and report telemetry slots 0–2 (voices,
   polyphony, stolen) plus your own from slot 8. List the sources you produce in `SOURCES`.
9. When the instrument is finished, set `READY = true`.

`crates/cantus-engine/tests/inst_contract.rs` checks rules 1–3 for every kind; it must stay
green with your instrument making sound.

## Tests

Tests go in before the wiring (the repo's habit): block-size independence at tolerance 0,
determinism, "the default changes nothing", pitch accuracy in cents across the range,
aliasing measured where it matters (an FFT of a high note), a CPU budget (render N voices
for T seconds under a generous ceiling, as `stress_project_renders_within_budget` does),
and whatever your research report's test section asks. Editor: it opens in Play, Edit and
Matrix and paints its name; a gesture is one undo step; presets set what they name.

Before you finish: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace`. All green.

## The editor is the product's face

Beat the benchmark's look, measured against the screenshots: the display is the control
(drag on the curve, the table, the waveform, the keyboard), every value printed in real
units, knob edges at 3:1 or better, one hue for the instrument, no decorative motion. Use
`crate::widgets::Knob`, `views::inst::param_knob` (modulation rings and drop targets come
with it), the `design` text styles and spacing grid and the `theme::p()` tokens. Give the
**Play** view the instrument's one live picture and its key parameters, and the **Edit**
view everything.

See it: `cargo run -p cantus-ui --example shoot -- <out-dir> 1 <kind>` (for example
`organ`) writes light and dark PNGs of your editor through the software renderer; open them
with Read and iterate until they are better than the benchmark's. `demo_setup` sets the
instrument up to show its display off; `demo_feed` injects telemetry
(`app.inst.injected`) so the picture looks alive.

**Presets**: 8–16 factory presets with tags, covering the instrument's range, each a real
sound, each naming what its macros do.

## Your final report (to the coordinator)

What was built, checked against the P0 and P1 lists of your research report; the `test
result` lines; clippy; the screenshots you took (paths); any `SHARED:` commits and why;
measured numbers (CPU per voice, aliasing floor, pitch error); what is not done and why.
