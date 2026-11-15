# Round 40, phase 3 — recorded pianos in Chorda, Tympanum the acoustic drummer, mixers with outputs, the scenes

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians 3:23).
Phases 1–2 are `BUILDING.md` and `briefs.md`; this is the third phase's contract. Read it,
then `CLAUDE.md`, then your brief in `briefs-p3.md`.

**What the owner asked (2026-11-12)**, all this round:

1. **Chorda takes recorded piano packs** — free or bought, *following the standard* (SFZ,
   Decent Sampler, SF2, the Salamander/bitKlavier naming) — with a curated list of good free
   ones, while the **modelled** piano stays the goal (the `chorda` builder, phase 1, voices
   model 2 toward the Salamander).
2. **Chorda's internal mixer with outputs to mixer strips** (perspectives; a recorded pack's
   own mics; noises on faders of their own where the source has them).
3. **Tympanum**, a Superior Drummer-class acoustic drum instrument over Pulsus's engine
   (Pulsus stays the pad drum machine): modelled kits (movable virtual mics) and recorded
   multi-mic kits, the kit in its room, a mic mixer with a drum × mic bleed matrix, each mic
   to Main / its own strip / a chosen strip in one click, continuous CC4 hi-hat, one draw per
   hit shared by every mic, streaming and per-mic loading, its own room by default with ours
   as an option. **Grooves later** (BACKLOG §5).

**His decisions the same day** (the contract holds them): the name *Tympanum*, a new
instrument; modelled kits' mics move; a recorded kit plays in its own room by default;
**three pack modes** — *Pack* (CC0 / CC BY: one click from the official source), *From the
author's site* (free, no licence stated: one click from the author's own URL, never hosted
by us), *Manual* (a login: Cantus opens the page, the user drops the file) — ADR 0042
amended in ADR 0125 §4; Chorda 1 stays in the source list as *previous model*; the default
recorded piano is **the Salamander** until his ABX of bitKlavier; the modelled piano's mixer
is *Hammers, Player, Audience, Room, Mechanics* (movable or added piano mics later, BACKLOG);
**the scenes are the designer's projected-vector render** — a new scene module for Chorda
and Tympanum that replaces ADR 0104's baked renderer (*"vamos frescos con el diseño mejor"*).

**The owner's rules:** every gap a feature leaves is closed in the same round; open only
what needs the owner. **The owner accepts**; agents find defects but do not decide.
Configurable with a sensible default. **Show progress often** — pictures (and, for sound,
files) in `F:\projects\daw-r40\compartir\p3\<name>\`, and say in your report what he should
look at or listen to.

## The contract (commit on `r40/p3-contract`, from `r40/integration` d99eb543)

Compiles, every stub null; every old song renders to the bit, every old fixture loads.

| Piece | Where | What |
|---|---|---|
| Named outputs (ADR 0123) | `cantus-core/src/inst_out.rs` | `OutputId`, `OutputFamily`, `OutputSlot { id, name, family, strip }`, `OutputView`, `RouteView { Main, Own, Chosen }`, `OutputPlan { AllToMain, ByFamily, EachOwn }`, `OutputField` and the `o{n}.` keys (`OUTPUT_PARAMS`, `output_key`, `key`, `param`), `strips_of`, `bus_of`, `route_view`, `set_route` (works), `plan` (stub: refused), `MicRule`, `FoundMic`, `MicFinding`, `family_of_name` (stub) |
| | `inst/mod.rs` | `NativeSpec::outputs` (schema 66); `InstrumentKind::has_outputs()` (Chorda, AcousticDrums); `o{n}.` keys accepted for those kinds; `o{n}.on` set, never automated |
| | `instrument.rs`, `model.rs` | `InstrumentSpec::outputs()`, `feeds_strip()`; `output_strips()` for native slots; `Track::feeds_through_bus` asks `feeds_strip` (orphans, freeze, stems see native outputs) |
| | `command.rs`, `freeze_guard.rs` | `Command::SetInstrumentOutput { track, id, slot }` (`cmd.set_instrument_output`), its exact inverse, its freeze guard; undo fuzz `a_named_output_undoes_and_redoes_among_the_others` |
| | `cantus-dsp/src/outmix.rs` | `OutMix` (level, balance, width, polarity, mute): `is_unity` touches no sample |
| Tympanum (ADR 0124) | `cantus-core/src/inst/tympanum.rs`, `tympanum_kit.rs` | `InstrumentKind::AcousticDrums` (**not in `ALL`**, `READY` false); keys: kit, room, `p{n}.`, `m{n}.`, `b{p}.{m}.`; `KitRef`, `MicPattern`, `RoomChoice`; `KitDef`, `PieceDef`, `PieceKind`, `Place`, `Articulation`, `Openness`, `MicDef`, `BleedDefaults`, `RoomDef`, `ModelledKit`, `modelled_kits()`, `modelled_kit()` (stub) |
| | `cantus-dsp/src/tympanum.rs` | `hit_draw(seed, key, ordinal)`: one draw per hit, shared by every mic (works) |
| | `cantus-engine/src/inst/tympanum.rs` | the silent, at-rest unit; `inst::make`'s arm |
| | `cantus-ui/src/views/inst/tympanum.rs` | the editor's empty stub in the frame's dispatch; Pulsus's skin sheet stands in |
| Chorda's source (ADR 0125) | `cantus-core/src/inst/chorda_source.rs` | `SOURCE_KEY`, `REC_PREFIX`, `ChordaSource`, `RecordedPiano`, `SourceChoice`, `DEFAULT_RECORDED` (the Salamander), `MODEL_OUTPUTS` (Hammers, Player, Audience, Room, Mechanics), `rec.` keys (Exemplar's, prefixed), `snapshot_spec` (stub: as is — called once in `Snapshot::compile`), `switch` (stub: refused), `scene_piano` (stub) |
| Packs (ADR 0042 amended) | `cantus-content/src/manifest.rs` | format **5**: `PackMode { Pack, AuthorSite, Manual }`, `author_page`, `AUTHOR_TERMS`, `Plays { Exemplar, Tympanum, Chorda }` (a Tympanum pack instrument may be a DrumGizmo `.xml`), `Build::multichannel`, `PianoBuild`/`PianoNaming`/`PianoPerspective`; validation refuses mode *Pack* without an accepted licence (test `licences.rs`) |
| Scenes (ADR 0126) | `cantus-ui/src/scene/mod.rs` | `Room`, `Subject { Piano, Kit }`, `SceneView`, `Picked`, `paint` and `pick` (stubs) |
| Schema | `cantus-project/src/format.rs`, `examples/write_fixture.rs`, `fixtures/v68.cantus` | 66 outputs, 67 Tympanum, 68 Chorda's source — three migrations that write nothing; the fixture blocks; `schemas_66_to_68_keep_outputs_a_tympanum_and_a_chordas_source` |
| Strings | `locales/areas/{tympanum,chorda_source,inst_mixer}.{en,es}.txt`, `i18n.rs` (`// r40:p3`) | headers, plus the two keys the contract needs (`instrument.tympanum`, `cmd.set_instrument_output`) |
| ADRs | `docs/adr/0123`–`0126` | proposed for round 40 phase 3 |

## Who owns what

One worktree per builder, `F:\projects\daw-r40\<name>`, branch `r40/<name>`, from the contract
commit. **Touch only what you own**; anything else, ask through the integrator.

| Builder | Owns | ADR | Schema | Locale |
|---|---|---|---|---|
| `tymp-engine` | `cantus-core/src/inst_out.rs`, `inst/tympanum*.rs`, `padout*.rs` (generalised to named outputs); `cantus-dsp/src/tympanum*.rs`, `outmix.rs`, `pulsus*.rs`; `cantus-engine/src/inst/tympanum*.rs`, `pulsus_kit.rs`, `padout.rs`, **`processor*.rs`** (only if needed; say so), new stream files for multichannel hits; `cantus-content/src/drumgizmo.rs`, `kitbuild.rs`, `install.rs` (multichannel keep, the mode's download rules), `fetch.rs`; the SFZ mic rule: `cantus-ui/src/views/inst/sampler_sfz.rs` (read `<master>`, `amplitude_onccN`, `label_ccN`, `output`, `set_ccN`) and a new `sampler_sfz_mics.rs`; the app-side kit loading (`cantus-ui/src/tympanum_load.rs`, new) and its e2e tests; the agents' `INSTRUMENTS` entry; migrations 66 and 67 and their fixture blocks | 0123, 0124 | 66, 67 | `areas/tympanum` — only the block marked `# r40:tymp-engine` at its end |
| `tymp-ui` | `cantus-ui/src/views/inst/tympanum*.rs`, the shared instrument mixer `views/inst/inst_mixer*.rs` (Tympanum's and Chorda's), the mixer's grouped instrument strips (design item 4) in new `views/mixer_group*.rs` with one call site in `mixer_desk.rs` marked `// r40:tymp-ui`, Tympanum's skin sheet (`kit/skin/instruments.rs`: the `AcousticDrums` arm and a `TYMPANUM` sheet only) | 0124 (views) | — | `areas/tympanum` (but the engine block), `areas/inst_mixer` |
| `chorda-src` | `cantus-core/src/inst/chorda_source*.rs`; Exemplar's buses: `cantus-core/src/inst/sampler.rs` (a zone's or group's output), `cantus-dsp/src/exemplar*.rs`, `cantus-engine/src/inst/sampler*.rs` (`process_buses`); Decent Sampler buses and SF2 (`views/inst/sampler_ds.rs`, `sampler_sf2.rs`); the piano naming install step (`cantus-content/src/pianobuild.rs`, new, one call site in `install.rs` marked `// r40:chorda-src`); *Install from a file…* (`cantus-content/src/user_pack.rs`, new — Tympanum's kits use it too); Chorda's source and mixer views (`views/inst/chorda_source*.rs`, one call site in `views/inst/chorda.rs` marked `// r40:chorda-src`); migration 68 and its fixture block | 0125 | 68 | `areas/chorda_source` |
| `scene` | `cantus-ui/src/scene/**`; Chorda's view switch to it (`views/inst/chorda_scene_view.rs`, `chorda_scene.rs`) and the **removal** of `chorda_scene_{raster,shade,room,gear,geo,mesh,piano}.rs` once every variant draws; `kit/skin/scene.rs`; the scene tests (`chorda_scene*.rs` and their successors) and the goldens they move | 0126 | — | scene words, if any, in `areas/chorda_source`'s block marked `# r40:scene` (ask `chorda-src`) — prefer none |
| `chorda` (phase 1, running) | unchanged: `cantus-dsp/src/chorda*.rs`, the engine's `inst::chorda`, Chorda's presets, `DSP_VERSION`. After its candidate 5 it renders `MODEL_OUTPUTS` as model-2 buses, null at defaults | 0098 | — | — |

**Shared files** (additive; the integrator resolves): `cantus-core/src/command.rs` (new
variants each marked `// r40:<name>`), `cantus-core/tests/undo_invariants.rs` (your arms at
the end, marked), `cantus-content/manifest/content.json` (each pack its own entry),
`cantus-project/examples/write_fixture.rs` (your block, marked), `cantus-ui/src/i18n.rs`
(already lists every area), `docs/BACKLOG-excellence.md` (append), `docs/beta-checklist.md`
(a *Ronda 40 · fase 3* item under your name). **`cantus-ui/src/views/inst/chorda.rs`** is
nobody's in this phase but for `chorda-src`'s one call site and `scene`'s switch. Exemplar's
SFZ parser (`sampler_sfz.rs`) is `tymp-engine`'s; `chorda-src` uses its mic rule and asks for
changes.

**Numbers**: schema 66 / 67 / 68 are taken by the contract (migrations that write nothing; if
yours must write, say so). ADRs 0123–0126 as above. Manifest format 5 is taken. A number you
were not given: ask, never take the next free one.

## Order and checkpoints

1. **`scene`** starts at once: the module's first drawing of Tympanum's studio kit and of
   Chorda's grand, Night and Paper, 1× and 1.5×, to `compartir\p3\scene\` — **checkpoint A**
   (the owner sees the port beside the designer's sheets). Then every variant, the pick, the
   budget; Chorda's view switched and the baked renderer removed last.
2. **`tymp-engine`** starts at once: Gogodze Phu II (CC0) read and streaming, its mics on
   lanes, the split = unsplit to the bit, e2e through `assemble`; a short WAV of the kit
   through its mics and the split stems to `compartir\p3\tymp-engine\` — **checkpoint B**
   (the owner listens). Then the modelled kits and their mics, CC4, DrumGizmo full read,
   DRSKit per-mic, CrocellKit, Virtuosity, Big Rusty.
3. **`chorda-src`** starts at once: the Salamander as a recorded source through Exemplar, the
   switch, the mixer's model; Upright Piano KW and bitKlavier; phrases rendered from each to
   `compartir\p3\chorda-src\` — **checkpoint C** (the owner listens: bitKlavier against the
   Salamander, his ABX before either becomes default).
4. **`tymp-ui`** starts **after the owner approves the design's next delivery** (the
   integrator says when): the kit view on `scene`'s module, the mixer, the grouped strips —
   shots, **checkpoint D**.
5. **Mid-round integrated review** as soon as two builders have a first delivery merged
   (CLAUDE.md: cross-feature bugs are the worst): the integrator merges early into
   `r40/integration` and runs the integrated review (≤ 6 agents).

## Rules (the usual ones, short)

As phase 1 (`BUILDING.md` «Rules»), word for word: work in your worktree; commit WIP often;
files in pieces under ~300 lines per tool call; stage by explicit path (never `git add -A`,
`.` or a folder, never your `.cargo/`); `export PATH="$HOME/.cargo/bin:$PATH"`, `TMP`/`TEMP` =
`F:\projects\daw-r40\tmp`; **every cargo command through `bash /f/projects/daw-r40/slot.sh
cargo …`**; `cargo fmt -p <crate>` (not `--all`: os error 206); clippy `--all-targets -- -D
warnings` and the tests of every crate you touch; no WSL checks (the integrator runs Linux);
downloads to `F:\projects\daw-r40\downloads\<name>\`, never into a worktree — **never bypass
the proxy or TLS**; Zscaler may answer HTML for ~10 minutes: check magic bytes (`PK`, `RIFF`,
`fLaC`, `\x1f\x8b`), wait, retry; images of other products under `research/ui/round40/<name>/`
(`git add -f`). **CLAUDE.md applies**: every mutation a `Command` with an exact inverse and an
undo-fuzz arm; null tests first (a default touches no sample, to the bit); real time (ADR
0005, `rt_memory.rs`); schema bump with migration and fixture; start-up wiring through `boot` /
`CantusApp::assemble` with a test through it; both locales; the hand-drawn kit; **an e2e test
per feature through `assemble`**; **formats tested with bytes from outside** (published kit
files, a published `.dspreset`, real sample names), never only our own writer.

Finish with `docs/round40/<name>.md` (what changed, how it is tested, what was verified only
in emulation, what the owner should look at or listen to, what is left) and a short report.
