# Round 36, builder ara: ARA 2 hosting, slice 1

Decision, evidence and design: `docs/adr/0097-ara-2-hosting.md` (GO; slice 1 built, its
section "Slice 1 as built"). Branch `r36/ara`.

## What was built

| Where | What |
|---|---|
| `crates/cantus-ara` (new, GPL) | bindings per architecture (ARA_API 2.3.0), the VST3 ARA interfaces, the host callbacks, the host document (`HostDocument`: editing sessions, undo by deactivation, clone, partial archives), the pure project → document mapping |
| `crates/cantus-test-plugins/ara` (new, GPL, never shipped) | `cantus-test-ara`: an ARA 2 VST3 plug-in from source that renders its regions, keeps a gain per modification, archives it, and counts every breach of ARA's call rules |
| `cantus-host` | `vst3/ara.rs` (main factory, an instance bound before activation), `ara.rs` (`AraSession`: documents, instances, offline renders, archives, status), `Parts::create_bound` / `load_bound` (marked), the scan marks ARA plug-ins (`ARA` feature) and `PluginDb::ara_marked` rescans old databases once, `PluginHost::ara` + `ara_sync` + archives in `project_for_save` (marked) |
| `cantus-engine` | `clip_renders.rs`: renders made outside the engine played in place of sources (marked lines in `samples.rs`) |
| `cantus-core` | `ara.rs` (plain data), `AudioRegion::ara`, `Command::SetClipAra` (+ warp/pitch refusals), `links::same_content` without the archive, freeze-guard arm, undo-fuzz stream `arng` (all marked) |
| `cantus-project` | schema 53 (placeholders 50–52 for mix/external/editor), `fixtures/v53.cantus`, `tests/schema53.rs` |
| `cantus-ui` | `ara.rs` (bind, let go, status), `views/ara_menu.rs` (the clip menu's ARA section with "ARA plug-ins run inside Cantus, not isolated"), `ai_tools_ara.rs` (`list_ara_plugins`, `bind_ara_plugin`, `unbind_ara_plugin`), the engine sync hook (marked in `app.rs`), locale area `ara` EN/ES, `tests/ara_ui.rs`, `tools/list` golden |
| docs | ADR 0097, manual `docs/manual/{en,es}/ara.md` (+ a line in each README), notices, BACKLOG §0.4/§3c |
| `tools/ara/build-test-plugin.sh` | builds Celemony's ARATestPlugIn from the tagged SDK into `../downloads/ara/build` (never committed) |

## Decisions the integrator should know

1. **A render, not a live insert** (ADR 0097 §2): each bound clip's instance renders the
   region offline; the engine plays the render in place of the source. No ADR 0005
   exception, fades and clip gain keep working, offline = real time by construction.
2. **Schema 53 with placeholders 50–52**: `format.rs` has no-op migrations
   `migrate_49_to_50`, `_50_to_51`, `_51_to_52` marked `r36:ara placeholder`; replace them
   with mix's, external's and editor's at integration (or renumber 53).
3. **The engine plays `SampleCache::set_clip_renders`**: a general seam, no ARA in the engine.
4. A bound clip refuses warp and audible pitch edits (and those commands refuse a bound clip).

## Tests

`cargo test -p cantus-ara`, `-p cantus-host --test ara`, `-p cantus-engine --test
clip_renders`, `-p cantus-project`, `-p cantus-core --test undo_invariants`, `-p cantus-ui
--test ara_ui`. Every ARA test asserts zero breaches of ARA's call rules, counted by the test
plug-in.

**Not run here:** `celemonys_reference_plugin_when_built` skips (ARATestPlugIn was not
built: the automated session was not permitted to build fetched third-party code). Run
`bash tools/ara/build-test-plugin.sh` once (it needs CMake: Visual Studio's on Windows), then
`cargo test -p cantus-host --test ara`.

## Owner's

- Write to Celemony about naming "ARA 2"; try a commercial ARA plug-in by ear. Never claim
  compatibility with Melodyne or any named plug-in until then.
- Run the ARATestPlugIn build once (above).

## Slice 3 (next)

Out of process over ARA IPC; multichannel (surround) sources.



## Slice 2's essentials and the review (same round)

ADR 0097 §9–§13. In short: renders on a render thread (progress on the clip, the last render
plays meanwhile, a newer edit cancels, a drag renders once it rests, the plug-in's changed
range re-rendered alone, the plug-in's latency pre-rolled, its head and tail rendered, the
project's tempo map, window-only renders converted to the engine's rate there and placed by
their start, sources shared with the sample cache and decoded on its loader thread); the
plug-in's own editor from the clip (*Open {name}*); each edit gesture in it one undo step
(`SetClipAraState`, merged per gesture; undo restores the plug-in); its transport wishes
honoured; a crash marker (stopped at the next start, *Retry*) and a render watchdog
(stopped, the window never waits); *Play sources only*; fields checked against the
plug-in's tables; bindgen layout tests per architecture; the binding each clip's own
(linked clips, exact undo); fresh binds clean (*with its previous edits* on request);
reversed clips refused.

**Measured** (Windows, debug test build, the test plug-in; `cargo test -p cantus-host --test
ara memory -- --nocapture`):

| What | Measured |
|---|---|
| 20 comp clips of 30 s over a 10-minute stereo take | renders 231 MB (the take: 230 MB; a copy per clip would be 4.6 GB), rendered in 0.42 s |
| 1 s edited inside a 3-minute clip, the plug-in's word to the render taken in | 31 ms (the render itself 27 ms) |

A real plug-in's own rendering time adds to the second line.

**Schema 53** (review finding 6): regenerate `fixtures/v53.cantus` at integration, once the
placeholders 50–52 are the real migrations (`cargo run -p cantus-project --example
write_fixture`); `AraClip::binding` (serde default 0) was added this round, so an older v53
reads.

**Screenshots** (`cargo run -p cantus-ui --example shoot -- docs/round36/ara 1 ara`):
`docs/round36/ara/ara-clips-{slate,paper}.png` (a clip rendered by the test plug-in, one
whose plug-in is not installed playing its source), `ara-menu-{bound,missing,unbound}-…`.
