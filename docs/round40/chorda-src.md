# Round 40 phase 3 — `chorda-src`: Chorda's recorded source, its mixer, the piano packs

Builder: **chorda-src** · branch `r40/chorda-src` · base `r40/integration` 45f9fa7a · ADR 0125
(§5 *As built*).

## What changed

### The recorded source through Exemplar's engine (ADR 0125 §2)
- `cantus_core::inst::chorda_source`: `snapshot_spec` compiles a Chorda whose source names a
  piano **and holds its zones** into the Exemplar spec of its `rec.` keys (unprefixed), the
  `o{n}.` mix and the routes carried; the model's keys stay behind. A source with no zones (the
  v68 fixture) plays the model. An output turned off (`o{n}.on` = 0) loads nothing; all off is
  silence, never Exemplar's starting sine.
- `with_source`, `switch`, `switch_choice` (Chorda 2 · Chorda 1 · a recorded piano): **one undo
  step** (`cmd.chorda_source`) — the source key, the `rec.` zones written or cleared, the
  outputs carried by name then family (`carry_outputs`: strip and mix move with them), the
  model's own keys kept so their automation plays again after a switch back; notes never
  touched. `current_choice`, `outputs`/`outputs_of_zones`, `scene_piano` (concert: bitKlavier;
  grand: the Salamander and Accurate-Salamander; upright: Upright KW, Old Piano FB).

### Exemplar
- **Outputs**: a group's output `g{m}.out` (set, never automated); `Exemplar::process_outputs`
  renders the sounding layers output by output; the unit applies each lane's mix
  (level, pan, width, ø, mute, solo) and hands non-Main outputs to their buses
  (`process_buses`) — and renders exactly the old way when nothing asks for outputs.
- **Zones sized to the instrument** (coordinator, decision 1): keys reach `z8191`; a snapshot
  stamps an Exemplar needing more than 1024 zones with its size (`zcap`, next power of two);
  `UnitKind::Zones(n)` makes the engine build that unit off the audio thread and swap it in with
  the snapshot (`units_for` matches by unit kind; one line in `backend.rs`, one arm in
  `listen_fork.rs`). Small instruments keep the 1024 table.
- **Decent Sampler**: `AUX_STEREO_OUTPUT_n` (directly or through `BUS_k`) → the group's output.

### Packs (manifest format 5)
- `pianobuild.rs`: the Salamander/bitKlavier naming → SFZ at install; `pianobuild_flac.rs`:
  every integer-PCM WAV kept as **FLAC, decoded again and compared sample for sample** before
  the WAV goes (decision 4). `PianoBuild::{set, unpacked_size}`, `PianoPerspective::output`.
- **bitKlavier Grand**: nine packs, one per image (output ids House ORTF 0 … Binaural 8;
  *Releases* 16), CC BY 4.0, archive.org URLs from bitklavier.com, SHA-256 our own, archive.org's
  SHA-1 checked; `gain_db` −7 (the Salamander's level). **Accurate-Salamander 6.2e** (CC BY 3.0
  by its README; its *minimum* edition). **Upright Piano KW**, **Old Piano FB** (CC0).
  *Manual*: Piano in 162, Helsinki 1967 Pro (listed with their pages). Headroom dropped.
- `user_pack.rs`: *Install from a file…* — a pack's own file by size + SHA-256; a loose SFZ /
  `.dspreset` / SF2 / DrumGizmo file played where it is; an archive unpacked; user packs never
  credited. `Manager::refresh`.

### The window
- **Play**: the source list at the left (one call site in `chorda.rs`), drawn in Slate's hand
  with the shared parts (`inst_mixer::ui`, as Tympanum's kits list): *All · Recorded ·
  Modelled*; *Modelled* (Chorda 2, *Previous models* folding to Chorda 1), *Recorded* (the
  Salamander first; *Get · size* — the size alone when the name needs the room — its progress
  and *Cancel*; *Choose mics…* for bitKlavier; the *Manual* pianos' *Open page*), *Your own
  pianos*, *Install from a file…*, the pianos on disk and *Manage…*. A name too long to share
  its line puts its control on the line under it — nothing is cut; every control says what it
  does on the help line. Beside the list, the Play view's side column keeps 280 pt (one line in
  `chorda.rs`): at the default window nothing of the pedals' words is cut in either language.
- **Mixer** (`View::Mixer`, the frame's fourth view for Chorda and Tympanum), **on `tymp-ui`'s
  shared instrument desk** (`chorda_source_{mixer,card,strip,room}.rs`): the piano in its room
  (the scene module, the recorded piano's body) with a tag at its foot; the selected output's
  card (its name *as bitKlavier names it*, who recorded it, the credit; a set's *Images on
  disk* with its bar, *used · n more ≈ size* and *Get the other n*; the model's perspective);
  *Outputs* — *All to Main* · *Each mic to its own strip* (`inst_out::plan`, which now names a
  recorded Chorda's outputs) · what they leave · the mix's preset; then *Piano* pinned left,
  the outputs under their families' heads (folding, scrolling, *n more ›*), *Room* pinned
  right. An image not on disk is a dashed placeholder with its download; an image on disk
  loads or unloads from its top cell (*as recorded* · *unloaded*), one undo step. Meters: each
  output's peak from Exemplar's telemetry (`tel::OUT_PEAK`), a strip's own meter once routed.
- ***Install from a file…* for Tympanum too**: `chorda_source::install_any` (a pack's own file
  by size and hash; anything else a user pack playing in the asking instrument). Tympanum's
  kits list shows the user's kits under *Your own kits* and plays one at once
  (`tympanum::kits::install_kit_path`; two marked edits in `tymp-ui`'s file).

### After the coordinator's decisions (end of phase 3)
- **Output delay, lines only where asked**: `UnitKind::Exemplar { zones, delays, room }` —
  `UnitKind::of` reads a snapshot's Exemplar (no allocation): its zone table, a bit per
  output whose `o{n}.delay` is not 0 (`sampler::outs::delay_mask`, the shared rule) and
  whether it adds a room; the engine builds that unit off the audio thread and swaps it in
  with the snapshot. A unit with no delay and no room owns none (renders as before, to the
  bit). The lines are `tymp-engine`'s `OutDelay`; a lane is mixed, then delayed; a ringing
  line is fed silence to the block's end and keeps the unit awake (`at_rest`).
  `sampler_outs.rs` holds Exemplar's outputs (moved out of `sampler.rs`). For Tympanum to
  feed its lines the same way, `tympanum_unit.rs` would take `delay_mask` for its unit's
  size — `tymp-engine`'s file: through the coordinator.
- **Add our room** on a recorded piano's *Room* strip: Exemplar's `room.add` (Off · Small room
  · Studio · Hall — Tympanum's boxes) and `room.level`, a Chorda's `rec.room.*`; the room is
  `cantus_dsp::tympanum::room::Room` fed the mono sum of every output, its tail on the main
  output, built only when asked (off: no room, no memory, the same bits).
- **Chorda's Play view to the design** (`chorda_play.rs`, one call in `chorda.rs`; the
  frame's keyboard drawn under the picture; no macro or modulation rows in Play or Mixer).
  Where the old Play view's controls went: *Room* (level) → the Mixer's Room strip fader,
  *Width* → the Mixer's Piano strip, *Dampers* → a row of the pedals; Hammer, Resonance
  (Sympathy, Board, Decay), Output as designed; Lid and Perspective in *The piano*. The
  picture and the room card over it are `scene`'s (`chorda_scene_view::scene_card`; its room
  plate lands with `scene`'s merge). Macros stay in the Edit view, routes in Matrix.
- Tests: `cantus-engine/tests/exemplar_delay_room.rs` (lines and room only when asked; a
  delay shifts its lane by whole samples at 512/333/64 with the main untouched to the bit; the
  hall rings after the notes and off touches no sample); the e2e sets a mic's delay and the
  hall through the app and gets the split master back to the bit at zero/off.

- **Where the moved controls went, said in the app**: a one-time notice on the status line
  the first time the new Play view opens (`settings.notices_seen`, key
  `chorda.play.moved.r40`); the panel's rows and *Open the mixer* say on the help line where
  Room level and Width went and what Dampers is (both locales).
- **The manual**: `docs/manual/{en,es}/chorda-pianos.md` (the source list, recorded pianos,
  Install from a file, the Play view, the Mixer, bitKlavier's images, Add our room, a mic's
  delay, disk and memory), linked from both READMEs. `docs/beta-checklist.md` (Ronda 40 · fase
  3): bitKlavier at 64 frames in duplex, four images' CPU and the cache limit, the rooms by
  ear, a mic's delay, the new Play view.

## How it is tested
- `cantus-content/tests/piano_build.rs` — bitKlavier's **published README listing**, the
  Salamander's names, key/velocity rules, the SFZ, a build; the FLAC step with written WAVs
  and **a published bitKlavier WAV** (`CANTUS_BITKLAVIER_WAV`).
- `cantus-content/tests/user_pack.rs` — recognition by size + hash, a bought SFZ, an archive.
- `cantus-core/tests/chorda_source.rs`, `undo_invariants.rs` (`chordas_source_switch_…`); the
  zone keys reach `MAX_ZONES` (`indexed_keys_parse_one_spelling_only`).
- `cantus-engine/tests/exemplar_outputs.rs` — default output null to the bit; split + buses =
  unsplit to the bit at 512/333/64; `exemplar_zones.rs` — a small instrument keeps its unit, a
  1500-zone one gets `Zones(2048)` and plays its zone 1400.
- `cantus-ui/tests/ds_outputs.rs` — the Decent Sampler guide's own examples.
- **`cantus-ui/tests/chorda_source_e2e.rs`, through `CantusApp::assemble`**: a piano (two
  perspectives + releases, SFZ by the piano build) installed with *Install from a file…* plays
  through Chorda — the same bits as an Exemplar with its zones; the editor draws the list and
  the Mixer on the shared desk; **a click on *Each mic to its own strip*** makes a strip per
  output in output order, routed into the piano's strip, and the master is the same to the
  bit (333-frame blocks); undo twice → the model to the bit. A second test: Tympanum's
  *Install from a file…* takes an SFZ kit the manifest does not know, plays it, lists it.
- The UX suites over the whole window: `ux_whole_words` (nothing of Chorda's cut but the
  shell's tab word, which `tymp-ui` fixes in `kit/shell.rs`), `ux_help_line`, `font_coverage`,
  `inst_chorda` (no text elided in English or Spanish at 75/100/200 %).

## Measured (bitKlavier, the `ballad` phrase — pedalled chords; `chorda_pianos --measure`)
Exemplar's unit as the engine runs it, the samples **streamed as the live engine streams them**
(ADR 0050), 512-frame blocks at 48 kHz (10.7 ms), one core, the dev profile (DSP and engine at
`opt-level` 3), on the owner's machine with two other builds running — the second pass, the
stream cache warm:

| images on | zones | zone table | voices at most | CPU of real time | block mean / max | sample memory | working set |
|---|---|---|---|---|---|---|---|
| 1 (Stage Lid ORTF) | 662 | 1024 | 37 | 6.0 % | 0.64 / 1.6 ms | 124 MiB | +203 MiB |
| 2 (+ Piano Bar) | 1232 | 2048 | 63 | 12.6 % | 1.35 / 4.3 ms | 231 MiB | +166 MiB |
| 4 (+ House ORTF, Decca Tail) | 2372 | 4096 | 117 | 25.3 % | 2.70 / **317 ms** | 445 MiB | +309 MiB |

- The cost grows with the images, as it must: each image is its own voices (one note sounds
  every image). Four images take a quarter of a core.
- **The 317 ms block at four images is the stream cache at its limit**: four images' decoded
  cache files (f32, with their mip-maps) are ≈ 15 GB — the automatic limit is 16 GB — so the
  sweep evicts files the phrase then reads again (offline, the read blocks). Live, those reads
  would be late voices, not a stall. A recorded piano of several images wants a larger cache or
  a cache kept in the pack's own format (FLAC-backed streaming): a question for the engine's
  owner (ADR 0050), in BACKLOG.
- **Disk**: bitKlavier's nine images keep as FLAC at install — 1.41–1.64 GB each (all nine
  installed once to measure it) instead of 3.80 GB of WAV (−60 %), each file decoded again and
  compared sample for sample; the manifest's installed sizes are these (the whole set 13.8 GB,
  not 34.2). Accurate-Salamander: 0.79 GB. The downloaded zips are gone once installed (their
  SHA-256 is in the manifest); four images stay installed for the tests here.
- **Loading as the tools load** (whole files, not streamed — the measure's first try) took
  ≈ 5 GB of memory per image and ran this machine out of memory at two: only the streaming
  path is fit for these pianos, and the app's engine streams.

## Verified only in emulation / not yet
- The model's five outputs play together on Main until the `chorda` builder's renderer plays
  them apart (`chorda_source::MODEL_RENDERS_OUTPUTS` is his to flip); the Mixer and *Each mic to
  its own strip* say so.
- `MAX_ZONES` stays as the bound on what a zone key names (8192): the unit's table is sized to
  the instrument; every bitKlavier image at once (≈ 6 000 zones) fits.
- Piano in 162 and Pianobook's files were never downloaded (torrent, login): *Manual*, listed
  with their pages.
- Per-output meters come from Exemplar's telemetry; seen in the shots only with the demo's
  silence (no engine runs in the shot).

## What the owner should look at or listen to
- `docs/round40/chorda-src/side-*.png` — each view beside the design's sheet (the design at the
  left): `side-chorda-play-*` (the source list), `side-chorda-mixer-*` (the model's five),
  `side-chorda-mixer-pack-*` (bitKlavier: four images on disk, five as placeholders), Night and
  Paper, 1× and 1.5×; `chorda-*-es.png` in Spanish. Copies in
  `F:\projects\daw-r40\compartir\p3\chorda-src\shots\`.
- `F:\projects\daw-r40\compartir\p3\chorda-src\images.csv` and `sizes.json` — the measure
  above; `more-*/sizes-more.json` the other five images' sizes.
- `F:\projects\daw-r40\compartir\pianos-c\` — the phrases and the ABX (bitKlavier vs the
  Salamander), from checkpoint C.

## What is left (needs the owner or another builder)
- The owner's ABX decides the default recorded piano (the Salamander until then).
- A larger or FLAC-backed stream cache for several images at once (engine, ADR 0050).
- The model's five outputs rendered apart (`chorda` builder; flip `MODEL_RENDERS_OUTPUTS`).
- Others' clippy lints with toolchain 1.97 that stop `-D warnings` on the merged tree:
  `scene/draw.rs:245`, `scene/rooms.rs:903`, `tympanum_load*.rs`, `sampler_sfz_mics.rs:338`
  (cantus-ui); `inst_out.rs:409`, `tympanum_hits.rs:217` (cantus-core); `tympanum_room.rs:66`
  (cantus-dsp). None in `chorda-src`'s files.
- The shell's Spanish tab word *Mezclador* is cut at 100 % (`kit/shell.rs`, `tymp-ui` fixing).
