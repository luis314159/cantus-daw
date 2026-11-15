# Round 40 phase 3 — the builders' briefs

Read `docs/round40/BUILDING-p3.md` first (the contract, who owns what, numbers, order, rules),
then `CLAUDE.md`, then your brief. Read the research (`F:\projects\daw-r40\research-p3\`:
`summary.md` first, then `formats.md`, `packs.md`, `competitors.md`) and the design
(`F:\projects\daw-r40\design-p3\`: `notes.md`, the PNGs, `kit-spec.md`, `kit-model.json`,
`src/`, and the designer's `port-notes.md` when it lands). Every brief ends the same way:
`docs/round40/<name>.md` and a short report saying what the owner should look at or listen to.

## `tymp-engine` — Tympanum's engine, named outputs, the drum packs

ADRs 0123 (yours to finish), 0124 (yours), 0100 (what you generalise), 0050 (streaming), 0042
and 0125 §4 (licences, the three pack modes); `docs/instruments/pulsus.md`, `exemplar.md`;
`cantus-dsp/src/pulsus*.rs`, `cantus-core/src/padout*.rs`, `cantus-content/src/drumgizmo.rs`.

1. **Named outputs, generalised** (ADR 0123): `inst_out::plan` (*All to Main · Pieces to strips
   · Each to its own strip*) over `padout::split_with`/`merge`; `heard_on`, `carry_outputs`
   (by name, then family), `with_kit_strips_following`, templates, DAWproject notes, the
   agents' mixer JSON — each generalised or said Pulsus-only, with why. `family_of_name` (English
   and Spanish words, the packs' names). `OutMix` grown: the delay line (preallocated), solo
   across outputs. **Null first**: a default `o{n}.*` touches no sample (to the bit).
2. **The engine** (`cantus-dsp/src/tympanum*.rs`, the unit in `cantus-engine/src/inst/tympanum*.rs`):
   every mic on its own lane through the bleed matrix (`b{p}.{m}.*`, the kit's `bleed` sparing
   a mic's `main_for` pieces), lanes added in output order, `process_buses`; **one draw per hit**
   (`hit_draw`) shared by every mic, no take twice running; **CC4 continuous** (crossfade the two
   nearest recorded openness levels at the hit, follow during it, choke the open sample into the
   pedal chick; a modelled hat tightens); **modelled kits** on Pulsus's modal voices with virtual
   mics (position, distance, aim, pattern → fractional delay, 1/r, the pattern's gain, polarity,
   colour), movable by `m{n}.*`; the rooms (`room`: the kit's own by default, Aula's responses
   on the room outputs as the option).
3. **Streaming**: a recorded kit streams from disk (ADR 0050's reader; one multichannel stream
   per hit serves every mic); a mic whose `o{n}.on` is 0 or a bleed cell off loads nothing; the
   old kit plays until the new one is ready; purge per mic. `rt_memory.rs`: no allocation, lock
   or I/O on the audio thread with every output split and a kit loading during playback.
4. **Readers, tested with outside bytes**: DrumGizmo XML in full — `main`, `<chokes>`/`choketime`,
   `<image>`/`<clickmap>`, the metadata licence (cross-checked against the manifest), the MIDI
   map — from the published kit files; the SFZ mic rule (ADR 0123 §5: `output`, ARIA `<master>`
   + `amplitude_onccN` + `label_ccN` + `set_ccN`, folders, single — logged) in `sampler_sfz.rs`
   and a new `sampler_sfz_mics.rs`, which `chorda-src` uses for multi-mic SFZ pianos.
5. **Packs** (manifest format 5, `Plays::Tympanum`, `Build::multichannel` — 24-bit at the kit's
   rate), each licence and SHA-256 read from its source, in order: **Gogodze Phu II** (CC0, 7
   mics — build the engine on it), DRSKit per-mic, CrocellKit (CC BY 4.0), Virtuosity (CC0),
   Big Rusty (CC0). Each pack's `mode` (*Pack* for these). The download rules of the modes in
   `install.rs`/`fetch.rs`: *From the author's site* fetches only the author's URL, and a hash
   mismatch, a 404 or HTML instead of an archive opens `author_page` and offers *Install from a
   file…* (`chorda-src`'s `user_pack.rs`); *Manual* never fetches. Tested with an emulated server
   (the existing fetch tests) — and say what was verified only in emulation.
6. **Commands** (kit change carrying routes and keys, bleed, mic moves, the plans) with exact
   inverses and undo-fuzz arms; the model's version stored; migration 67's block in
   `write_fixture.rs` kept in step.
7. **Join `ALL`** in the commit that makes Tympanum sound: the null corpus's new item measured
   (`CANTUS_GOLDEN_DUMP`), the contract twins (`inst_contract.rs`) green, the bench's entry, the
   agents' `INSTRUMENTS` entry. `READY` stays false (the integrator turns it on).
8. **End to end through `CantusApp::assemble`**: a Tympanum on a track plays a groove into its
   mic strips (*Each mic to its own strip*), and the split sums to the unsplit **to the bit**,
   offline at 512 and 333 frames and live on the pool; block-size independence at 0; offline =
   real time. **Checkpoint B**: a WAV of Gogodze through its mics and the split stems to
   `F:\projects\daw-r40\compartir\p3\tymp-engine\`.

## `tymp-ui` — Tympanum's views, the instrument mixer, the grouped strips

Starts **after the owner approves the design's next delivery** (the integrator says when).
ADRs 0124, 0123, 0126 (you use `scene`'s module, never edit it), 0111 (contrast), 0100 §9;
the design's *Kit* and *Mixer* sheets and the Cantus mixer sheet (design item 4).

1. **The kit view**: the kits list (*Modelled* "instant · no download" and *Recorded* "real
   drums · free packs", each with its glyph; *Get · 2.8 GB*, the bar and *Cancel*; *Install from
   a file…*; the pack's mode said — *downloaded from <author>'s site*, *Manual* opening the
   page), the kit in its room through `scene::paint` and `scene::pick` (click plays and selects,
   hover lights the strip, a modelled mic dragged where it stands), the row of pieces with their
   keys, the piece's panel (articulations and keys, *Where it is struck*, its sound, which mics
   hear it), the room's segmented control.
2. **The instrument mixer** (`views/inst/inst_mixer*.rs`, shared — Chorda's mixer uses it):
   strips 72 pt wide under family heads that fold, *Kit* pinned left and *Ambience* right, the
   middle scrolling with its fade and pill; per strip bleed, ø, pan, M/S, the fader and meter,
   the **Out** cell (`inst_out::route_view`: *Main*, *Own · 12*, a chosen strip by name); the
   three actions with the result in words; the mic band (the kit with its mics, the selected
   ringed with its cone, the mic's card: pattern, distance, aim, *What it hears*); the bleed
   matrix. Every edit a command (through `tymp-engine`'s builders).
3. **The mixer's grouped instrument strips** (design item 4) in `views/mixer_group*.rs`, one
   call site in `mixer_desk.rs`: an instrument's own strips grouped under it on the desk, folded
   to one.
4. **Look at what you build** (`cargo run -p cantus-ui --example shoot`): Night and Paper, 1× and
   1.5×, Spanish at 1920 × 1080; `ux_whole_words` and the legibility tests across every theme
   (ADR 0111: non-text 3:1, text 4.5:1); nothing truncated. **Checkpoint D**: shots to
   `F:\projects\daw-r40\compartir\p3\tymp-ui\` and `docs/round40/tymp-ui/`.
5. An e2e through `assemble`: the kit view opened on a Tympanum track, a click on a piece plays
   it (the engine hears the note), *Each mic to its own strip* from the mixer makes the strips.

## `chorda-src` — Chorda's recorded source, its mixer, the piano packs

ADRs 0125 (yours), 0123, 0042 (amended), 0059/0098 (the model — not yours), 0050; the
`chorda` builder owns `cantus-dsp/src/chorda*.rs` and the engine's `inst::chorda`: **do not
touch them**. `docs/instruments/chorda.md`, `exemplar.md`, `docs/round26/content.md`.

1. **The recorded source through Exemplar's engine** (ADR 0125 §2): `snapshot_spec` turns a
   recorded Chorda into the Exemplar spec of its `rec.` zones, outputs carried; the engine plays
   it with Exemplar's unit. **Null first**: a model Chorda renders to the bit what it did; a
   recorded Chorda renders to the bit what an Exemplar with the same zones renders.
2. **Exemplar's buses**: a zone's or group's output (`sampler.rs`), `process_buses` in the engine
   unit, lanes in output order; Decent Sampler buses and `AUX_STEREO_OUTPUT_n` (`sampler_ds.rs`,
   tested with a published `.dspreset`); SF2 as one perspective; multi-mic SFZ through
   `tymp-engine`'s mic rule.
3. **The mixer**: perspectives and noises as named outputs (`o{n}.*`, the routes), the model's
   `MODEL_OUTPUTS` (*Hammers, Player, Audience, Room, Mechanics*) shown even before the `chorda`
   builder renders them (they then play on Main: say so in the view); Chorda's source and mixer
   views (`views/inst/chorda_source*.rs`, one call site in `chorda.rs`) on `tymp-ui`'s shared
   instrument mixer (until it lands, build against its contract and say so); presets storing the
   mic mix.
4. **The switch** (`chorda_source::switch`): one undo step; notes, applicable automation,
   strips and outputs kept; the source list: **Chorda 2**, **Chorda 1 — previous model**, then
   the recorded pianos, **the Salamander first** (the default until the owner's ABX). Undo-fuzz
   arm. `scene_piano`: concert for bitKlavier, grand for the Salamander, upright for Upright KW
   / Old Piano.
5. **Packs** (format 5, `Plays::Chorda`, each licence and SHA-256 read from the source; never
   bypass the proxy): **Upright Piano KW** (CC0, *Pack*), **bitKlavier Grand** (CC BY 4.0, *Pack*,
   via `PianoBuild` writing its SFZ from the naming — `cantus-content/src/pianobuild.rs`, tested
   with real file names), **Accurate-Salamander** only if its licence file allows; Piano in 162
   and Headroom Piano as *From the author's site*; Pianobook's as *Manual*; the Salamander ships.
   **Install from a file…** (`cantus-content/src/user_pack.rs`): a bought SFZ, Decent Sampler or
   SF2 piano — and, for `tymp-engine`, a kit — installed as a user pack with a receipt; a
   *Manual* or changed *From the author's site* file recognised by its pinned size and SHA-256.
6. **End to end through `assemble`**: a Chorda switched to the Salamander plays a phrase through
   Exemplar's unit, each perspective split to its strip sums to the unsplit to the bit, and undo
   brings the model back. **Checkpoint C**: the same phrases from the model, the Salamander,
   Upright KW and bitKlavier to `F:\projects\daw-r40\compartir\p3\chorda-src\` (FLAC), and the
   ABX kit (`chorda_abx`'s shape) for bitKlavier against the Salamander.

## `scene` — the instruments' scenes drawn as projected vectors

ADR 0126 (yours), 0104 (what you replace, §1; its §2 look stays), 0111, 0110 (frame budget),
0107 (memory). The design's `src/` (`geo.py`, `shade.py`, `piano.py`, `pieces.py`,
`hardware.py`, `draw_room.py`, `scene.py`), `kit-spec.md`, `kit-model.json` (metres) and the
designer's `port-notes.md`.

1. **Port the generator** into `cantus-ui/src/scene/**` through the kit's painter (no stock
   widgets, no literal colours — scene colours as named kit tokens per theme, no `FontId::new`):
   3-D model in cm → projection → facet lighting (key light, strip lights, reflected
   environment) → depth-sorted vector paths. **Checkpoint A** first: Tympanum's studio kit and
   Chorda's grand, Night and Paper, 1× and 1.5×, beside the designer's sheets, to
   `F:\projects\daw-r40\compartir\p3\scene\`.
2. **Every variant**: piano types concert / grand / parlour / upright, the finishes (maple
   among them), the lid, the perspective mics, the room pair; rooms booth / studio / hall /
   church; evening and daylight per theme; the kit's pieces and materials (maple shells, coated
   heads, chrome, lathed bronze), stands, pedals, its mics, rooms Small / Studio / Hall.
3. **Live**: `pick` from the drawn geometry (pieces with their zones, mics, the lid); lid and
   mics animate; the paths cached per frame state; **at rest, no repaint**; the 60 Hz budget at
   1× and 1.5×.
4. **Chorda's view switched** to the module, then the baked renderer **removed**
   (`chorda_scene_{raster,shade,room,gear,geo,mesh,piano}.rs`), its ~180 MB bake peak measured
   before and after (`memmap.ps1`: a map, not the scorecard); the scene goldens re-blessed **in
   one commit** with before/after shots; every other golden unchanged.
5. Tests: every variant drawn and finite, `pick` for every drawn object, the budget, the
   repaint at rest, legibility of what can be clicked (3:1) in every theme. An e2e through
   `assemble`: Chorda's editor opens on the new scene and a click on the lid opens it.
