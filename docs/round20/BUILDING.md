# Round 20 — the brief every builder reads first

**The ask (the owner, 2026-10-05, in Spanish):** automation clips; closing and minimising
an instrument must be easy and graphical (today you have to find an option to unfold);
swapping one instrument for another — and one mixer effect for another — easy and
intuitive; *"some DAWs let you see every available instrument on screen to choose — it
looks very, very pro — I want that"*; maybe free, rights-free instruments preloaded in
the DAW (which also confirms compatibility at once); and, as backlog research: where to
get rights-free WAV files (kicks, snares, kits, sounds for users) and where to keep them
(not in the repo). The mission the owner states: the best DAW in the world, made with
excellence (Colossians 3:23). Build to that bar.

Read, in order: this file, your brief (`docs/round20/briefs.md`, your section),
`docs/STATUS.md` (top, Round 19, Round 18, Conventions), the ADRs your brief names, ADR
0005 (real-time rules) and ADR 0024 (the hand-drawn kit).

## The contract (commit on `r20/contract`)

- `crates/cantus-ui/src/picker.rs`: `PickPurpose` (AddInstrument, ReplaceInstrument,
  AddEffect, ReplaceEffect), `PickChoice` (Firmus, Pulsus, Instrument(kind),
  Effect(kind), Hosted(PluginInfo)), `CantusApp::open_picker`, `CantusApp::apply_pick`,
  and the field `CantusApp::picker`. **Keep these signatures**; extend the enums if you
  must and say so in your report.
- Empty locale areas, registered: `locales/areas/{automation,picker,swap,hosted}.*`.
- Numbers: ADR **0027** automation (automation builder), **0028** hosted instruments
  (hosted-inst builder), **0029** the device picker and swapping (browser and device-ux
  together: browser writes it, device-ux adds its section). Schema: the automation
  builder takes **19**; the hosted-inst builder also bumps to **19** on its branch, and
  the integrator renumbers one of the two to 20 at merge (migration and fixture).

## Rules (all builders)

1. **Your worktree and branch only** (`C:\projects\daw-r20\<name>`, `r20/<name>`). Never
   touch `C:\projects\daw` or another worktree; never push; never merge. WIP commits
   early and often.
2. **Write files in pieces under ~300 lines per tool call.** Prefer new files; keep edits
   to shared files (`app.rs`, `command.rs`, `processor.rs`, `arrangement*.rs`,
   `mixer*.rs`, `mod.rs` lists, `keymap.rs`) small and local.
3. **Tests first**: null tests before DSP or engine wiring; new commands into the undo
   fuzz (`crates/cantus-core/tests/undo_invariants.rs` — add arms, don't renumber others');
   gestures tested through published geometry; `cargo clippy -p <crate> --tests` clean;
   `cargo fmt`.
4. **Strings** only in your area's locale files, English and Spanish.
5. **The kit only** in views (no stock egui widgets, no literal colours, no `FontId::new`);
   FL Studio is the visual reference; render with `cargo run -p cantus-ui --example shoot
   -- <dir> ...` (extend it) and look at every view you build — fix what looks amateur.
6. Audio thread: no allocation, lock, block or I/O (ADR 0005).
7. Bash quirk: a heredoc delimited by `EOF` fails to parse — use `XEOF` or the Write tool.
8. `.cargo/config.toml` (untracked) has `jobs = 4`, `debug = 0`; several builders share
   the machine. Test your crates, not the whole workspace, until the end.
9. The network goes through a corporate proxy (Zscaler) that may quarantine a download
   for up to ~10 minutes and answer with an HTML page meanwhile: check what you download
   (a zip starts with `PK`), wait and retry. **Never bypass the proxy or TLS checks.**
10. **Finish with a report** (done / tests / files touched outside your area / left /
    notes for others) and `docs/round20/<name>.md`.

## Who owns what

| Builder | Branch | Owns |
|---|---|---|
| automation | `r20/automation` | automation clips and lanes end to end (ADR 0027, schema 19) |
| device-ux | `r20/device-ux` | device-window chrome (close, minimise, collapse), `apply_pick` (swap instrument / effect keeping what matters), every entry point that opens the picker |
| browser | `r20/browser` | the picker's view: a gallery of every instrument and effect, native and hosted, with drawn thumbnails, categories, search, favourites (ADR 0029) |
| hosted-inst | `r20/hosted-inst` | hosted CLAP/VST3 instruments on tracks (ADR 0028), proved with real free plug-ins |
| research | `r20/research` | research only: free instruments and rights-free samples, licences, distribution and storage (`research/content/`) |
| research-ai | `r20/research-ai` | research only: MCP for Cantus, LLM provider APIs, generative audio (`research/ai/`) |
