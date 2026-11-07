# Round 37 — library

ADR 0101. Branch `r37/library`. Schema: **none taken** (57 stays free: the levels were fitted in
the presets, not in the units). Two deliveries: the foundation (merged as 663016ad, its report
in the second half) and the library itself.

# Second delivery: the library made

## Done

- **Every unit at its floor, tagged by style and role.** 467 presets of the library's own
  units, every one with its styles and roles and a one-line note of what it is for:

  | Unit | Presets | Floor |
  |---|---|---|
  | Unda · Calor · Exemplar | 26 each | 24 |
  | Modulus · Gravis | 25 each | 24 |
  | Lamina · Organum | 24 each | 24 |
  | Aequalis · Concentus | 14 each | 12 |
  | Delay · Fervor · Latitudo | 13 each | 12 |
  | the other 18 effects | 12 each | 12 |
  | Speculum | 8 | 8 |
  | Reverb (the older one, no longer offered) | 0 | 0 |
  | Chorda (piano's) · Pulsus II kits (drums') | theirs | 24 · 16 |

  Each style and each role holds at least 12 presets (`preset_audit.rs`). Four effect-sound
  patches were added so the *fx* role is more than noise (Calor *Siren glide*, Unda *Impact*,
  Modulus *Sci-fi*, Exemplar *Wind*).
- **Every library preset is data** (`crates/cantus-ui/presets/**.toml`, 32 files): the arrays
  of the library's units were exported by `examples/preset_tool.rs export` and emptied; their
  readers call `factory_presets(kind)`. Chorda and the dancer stay in code (theirs).
- **The 147 known failures fixed and struck off.** Levels fitted by measurement
  (`preset_tool fit`: each preset rendered on its role's material, its level key moved to the
  window's centre, up to five passes, capped by its velocity-127 headroom on the dense riff);
  presets the level key could not bring in were fixed in their sound (an oscillator's level, a
  mix, a resonance, a playing technique). **`tests/data/preset_render_known.txt` holds no
  library preset** — only drums' kits and 14 of piano's Chorda presets (see *Open*).
- **Effects that jumped in level on insert are levelled**: every effect preset within ±2 LU
  of its dry source, except those that change level by design (`preset_measure::by_design`:
  Latitudo *Side solo*, Discrimen *Only transients* / *Only body*, Porta *Duck*, Tonus *Octave
  down*, each with its reason) and the units that set level by design (Terminus, Culmen,
  Tonsor). The clipping check holds them all.
- **CPU under budget** (a tenth of a core per preset; Chorda and Culmen a fifth, Exemplar
  15 %), measured in release on its render alone (best of two, of five near the budget: the
  machine is shared). Heaviest now: Exemplar *Celesta* 0.135, *Bells* 0.130, *Trap glock*
  0.129 (budget 0.15); every other library preset ≤ 0.10. Lightened rather than re-budgeted:
  Unda *Warp pad* (0.31 → 0.09: one warp, two unison voices), *Dark drone*, *Grain cloud*,
  *Stretched bells* (24 partials); Calor *Velvet pad*, *Unison saw*, *Cinematic swell*;
  Tonsor *Hard 808* (one oversampling step less). The check takes a second look at a preset
  over its budget, alone beside the median preset alone (the machine is shared by four
  builders, and their bursts are not a preset's cost).
- **The older Reverb is no longer offered** (`EffectKind::offered`: the picker, the insert and
  mixer menus); old projects keep it and sound the same (`tests/legacy_reverb.rs`). **Delay**
  has 13.
- **Cantus's own packs**, ready to upload (the integrator's step):

  | Pack | Files | Installed | Archive | SHA-256 |
  |---|---|---|---|---|
  | `cantus-one-shots-1.zip` | 182 | 119 744 350 B | 56 625 571 B | `121533707e7a873b0c0741ecf441659d90afddcc4c0020f3fb807cfd854ff78e` |
  | `cantus-loops-1.zip` | 87 | 91 028 790 B | 60 033 092 B | `872991f18a8010df4b1911813d1451244f77879ae9d6dd7f0b89ba7a4ae59e33` |

  Rendered again by the integrator at the round's final sound (2026-11-06), in `F:\projects\daw-r37\packs-final\` (outside the worktree). One-shots: a note of every
  library instrument preset, 2.5 s, 44.1 kHz 24-bit, in folders by role. Loops: 70 two-bar
  loops at 120 BPM (ten per instrument, from the styles producers loop most) and the grooves of
  all 16 factory kits, each cut from the middle of three passes so it loops without a seam.
  Every file peak-normalised to −1 dBFS, as a sample pack's are (one gain per file: a loop
  still loops). A CC0 `LICENSE.txt` in each. Their entries are in
  `crates/cantus-content/manifest/content.json` (manifest **format 4**, for the new `loops`
  category), at `https://github.com/luis314159/cantus-daw/releases/download/content-r37/`.
  **Upload both zips there under the tag `content-r37` before this manifest ships.** To
  rebuild after a preset changes: `cargo run --release -p cantus-ui --example render_packs --
  <dir>`, then `cargo run -p cantus-content --example pack_folder -- <dir>/<pack>
  <zip>` per pack (it prints size and hash for the manifest).
- **Packs from elsewhere, vetted** (licences read at the source, 2026-11-05): the rest of
  VSCO 2 CE and Kenney *Sci-fi Sounds* / *Impact Sounds*, all CC0 — recorded in ADR 0101 §7, not
  added (Big Rusty Drums and VSCO 2 Percussion were in the manifest already).
- **The website plays every unit's presets**: on each unit card a ▶ for its first excerpt; on
  the unit's page every excerpt with its name and what it is for, an effect's beside its dry
  source (which plays too). Four per instrument and three per effect, chosen for range (a role
  not heard yet, then a style not heard yet; never an *Init*), rendered by
  `examples/render_unit_sounds.rs` on what the browser plays them on, at the presets' own
  fitted levels (matched by the render check's ±2 LU), MP3 96 kb/s, nothing after. **111
  files, 9 893 606 bytes (9.4 MiB)** in `web/assets/units/sounds/`, listed in the generated
  `web/src/units_sounds.rs`; the web crate checks for wasm32. README's grid: each of the 32
  units says its preset count and links to *hear them*.
- **Delay has its unit card** on the site and in the README (it had none, so its excerpts went
  unheard): it has no window of its own, so its pictures are its view in the inspector, under
  its insert on *Dotted eighth* — `examples/shoot_delay.rs` (Slate and Paper, 100 and 200 %, EN
  and `--es`), then `web/tools/units_pictures.py <shots> delay` (which now takes unit names, and
  never scales a picture narrower than a card up: it takes the 200 % shot). Its page says the
  picture is the inspector's view; `Inspector::open_insert_view` opens that view for the shot.
  The site's line reads "nine instruments and twenty-four effects … twenty-three with a face
  of their own, and Delay, edited in the inspector".
- **Agents**: `find_presets` (read-only: every unit's presets or one unit's, by style, role
  and words, with what each is for) and `audition_preset` (renders a preset where the browser
  would, answers its loudness, peak and length, plays it, `changed_the_project: false`);
  `apply_preset` loads one, as before. Manual `ai-tools.md` EN/ES; the MCP golden rewritten.
- **Review fixes (both reviews of the first delivery)**, each with a test that fails without
  it (`tests/preset_browser_review.rs`, 8 tests, and the e2e): the audition compiles a cut-down
  copy without waiting; renders are cancelled by a new audition, Stop and another project;
  ↑ ↓ settle 150 ms; an insert is heard on a clip its strip hears (pad strips and buses
  included, `tracks_on_strip`), unmuted in the copy, never through a hosted unit, an external
  instrument or an ARA clip (the fixed source then, or after a silent render), worded "first
  clip on this channel"; a frozen target is auditioned live and its *Load* refused (*frozen —
  unfreeze to load*); the browser takes only its own keys (Ctrl+Z reaches undo) and starts a
  search on the first letter typed; the effect window names the preset the browser loaded;
  the known list has tolerances (1 LU, 0.5 dB, 25 % CPU) and the debug build holds a CPU check
  in units of the run's median; chip text at `s.tk()`; the browser and its render close with
  the project. The instrument preset list closed on the click that opened it — a shell pop-up
  bug the e2e found (`popup_pass`).
- **A kit end to end** (item 12, once drums' row landed): `preset_browser_e2e.rs` drives the
  kit's ⋯ *Browse presets…* row, clicks a kit's ▶ (heard on its groove from the engine, the
  project untouched) and *Load* (one step, dirty, the kit's pads; undo restores it).
- **At the merge of r37/integration** (finish's look, drums' kits, piano's first delivery): the
  render check's exemption for others' units is gone (Chorda's presets pass since model 2; the
  kits are listed) and so is the audit's for the kits.
- **The browser, from what its shots showed**: the STYLE and ROLE chips count what a click
  would answer now (within the search and the other band's choice; `Query::facets`) — before,
  *Pop 5* stood beside a search that left one pop preset; and a factory preset is found by its
  English name in any language (*tape* in Spanish finds *Cinta en el bus*), as a producer
  following an English tutorial types it.

## Tested

- Windows, `RUSTFLAGS=-D warnings`: `cargo clippy -p cantus-ui -p cantus-core -p cantus-engine
  -p cantus-content --all-targets` clean. The whole suite of those four crates
  (`--no-fail-fast`, after merging r37/integration at ffe3713a): **4 302 passed, 18 failed,
  62 ignored** — 8 of the failures the library's (the starting presets, the demos' loudness,
  the device header's value order, Unda's loud chord, the picker's Reverb), fixed after and
  re-run green in the targeted run below; the other 10 are others' (*For the integrator*).
  After merging r37/integration at 99c91ac3 (drums' kit gain stage): `--lib` and the tests
  `clipfx demos fx_frame inst_wavetable picker preset_render preset_audit preset_browser_e2e
  preset_browser_review legacy_reverb instrument_levels instrument_reference` — **533 passed,
  1 failed (drums' kits, below), 7 ignored**; `cantus-core` `starting` 4 passed.
- Linux (WSL Ubuntu 24.04, `tools/wsl/check.sh r37/library clippy` at efff174f): workspace
  clippy `--all-targets` exit 0, the plug-ins without their editor exit 0.
- `preset_render.rs` in release (`--test-threads 1`): every library preset in its window (or
  printed as limited by its headroom: 20, below), none clipping, none silent, every one
  within its CPU budget, Chorda's as listed. **It fails only on drums' kits** since the drums
  target moved to −19: 10 listed kits now pass (to strike) and *Electro* and *Trap* are 3 LU
  over — drums' to do, as the integrator decided. In debug the same, CPU in units of the
  run's median with a second look alone.
- `preset_audit.rs`: every unit at its floor, Chorda's and the kits' included; 511 factory
  presets, the 507 that sound all with a style and a role.
- `preset_browser_e2e.rs` 3 (instrument, insert, **kit**), `preset_browser_review.rs` 8,
  `legacy_reverb.rs` 2, `instrument_reference.rs`, `ai_mcp_boot` (golden rewritten: only the two
  new tools added), `cantus-content` (the manifest at format 4 with the two packs).
- The web crate: `cargo check --target wasm32-unknown-unknown` and `cargo fmt --check` (the
  site's CI) clean; the generated `units_sounds.rs` is `#[rustfmt::skip]`.
- Shots (`cargo run -p cantus-ui --example shoot_presets`), after finish's look merged:
  `docs/round37/library/` — Unda's leads with one heard (Night EN, Paper ES), Fervor searched
  for *tape* and heard on the user's clip (Night EN, Paper EN and ES: the Spanish one finds
  its two by their English names, the chips counting within the search). The example also
  writes Night ES, Paper EN and 200 %.
- Listening: the levels and CPU are measured; the sounds were made and fixed from their
  numbers and from what each parameter does — **no one has listened to them yet** (see *Open*).

## Shared files touched in this delivery (`// r37:library` where code)

- `cantus-core/src/effect.rs`: `EffectKind::offered()`; `cantus-core/src/starting.rs`: the ten
  starting presets with their levelled `out` (a fresh insert no longer jumps either). `cantus-engine/src/backend.rs`:
  `compile_for_audition`. `src/ai_tools.rs`: `library::TOOLS` and its module line.
- `src/app.rs`: the browser's own keys (`preset_browser_keys::take/act`) in
  `global_shortcuts`; `close_preset_browser()` in `reset_bus_to`.
- `views/inst/frame.rs` + `views/inst/mod.rs`: `popup_pass` (the list no longer closes on its
  opening click). `views/fx/frame.rs`, `views/fx/host.rs`: the window re-reads its preset's
  name after a browser load (`preset_epoch`).
- `picker_catalog.rs`, `views/inspector_menus.rs`, `views/mixer_menu.rs`: only offered effects.
- Every library unit's `*_presets.rs` / editor file: its array emptied (data now), readers on
  `factory_presets(kind)`; ~25 tests likewise.
- `src/demos/song.rs` + `src/demos/as_mixed.rs` (new): **the demos keep the presets they were
  mixed on** — the fit moved many presets' levels (a quiet pad up to 22 dB), which threw every
  demo's balance and master loudness off (`demos.rs` caught it: −8.3 to −11.9 LUFS against
  −16…−12); `native()` and `Song::effect` set back, for the 51 presets a demo loads that this
  delivery changed, the values they had (a generated table, `None` = the default).
- Tests whose expectations the data moved (each line marked): `clipfx.rs` (the starting
  presets, via `starting.rs`), `fx_frame.rs` (a data preset's values are in name order),
  `picker.rs` (the older Reverb not offered).
- `cantus-content`: `Category::Loops`, `FORMAT` 4 (`manifest.rs`), `content.json` (two packs),
  `tests/kit_build.rs` (one assertion: the current format holds the build step).
- `web/src/{main,units,units_data}.rs`, `web/style/units.css`, `README.md`: the excerpts and
  the counts.

## Open — only what needs the owner (or another builder)

- **Ears.** Every preset of the library is measured, none listened to. The owner's ear first
  on the 20 presets **limited by their headroom** — their velocity-127 peak on the headroom
  material (the dense riff; for Unda its eight-note chord too) holds them under their window:
  Unda *Hyper saw* −3.2 LU, *Formant talker* −5.5, *Anthem* −3.1, *Neon arp* −2.2; Modulus
  *House stab* −2.6, *Metal FX* −7.3, *Sci-fi* −5.6; Exemplar *Showcase* −3.8, *Nylon* −7.8,
  *Harp* −8.3, *Lo-fi nylon* −5.6, *Latin guitar* −8.8; Gravis *Motown* −5.8, *Lo-fi upright*
  −4.7; Lamina *Ballad* −2.4, *Reed* −4.9, *Reed soft* −6.3, *Pop ballad* −2.4, *Bolero* −2.9,
  *Reed tremolo* −3.8. They are dynamic sounds (plucked, struck, played soft, or stacked
  saws) that read quiet on a phrase at velocity 100 and peak hard at 127; a limiter in the
  voice would raise them but changes their sound — an ear's decision.
- **The CC0 dedication** of Cantus's own packs (`LICENSE.txt` in each): the owner's to
  confirm before they are uploaded.
- **Drums' kits.** The drums role's target is **−19 LUFS** now (the integrator's decision:
  a kit's crest factor; −16 drove single hits near full scale). The known file keeps drums'
  entries as their merge (99c91ac3) wrote them, measured against −16: drums strikes off the
  kits that land inside the new window and rebalances the rest (the library strikes nothing
  of theirs) — until then the render check names them: at this branch's last run 10 listed
  kits pass the −19 window (to strike) and *Electro* and *Trap* sit 3 LU over it (to
  rebalance).
- **Piano's Chorda presets** were levelled to the first delivery's keys target (−19); this
  delivery moved keys to −21 (headroom), so 14 of the 24 sit 2.0–2.8 LU over: listed; a 2 dB
  lower Chorda preset gain (piano's file) strikes them off.
- **Outside packs** (VSCO 2 CE, Kenney): vetted, not added (ADR 0101 §7).

## For the integrator

- *At integration:* the packs were rendered again at the round's final sound into
  `F:\projects\daw-r37\packs-final\` (sizes and SHA-256 in the manifest and
  `docs/round37/integration.md`); the website has 114 excerpts (10 161 046 bytes); the
  Chorda presets over 2 LU and the Electro/Trap kits were fixed in `cce72d6b` and `482950d1`.
- **Upload** `F:\projects\daw-r37\library-packs\cantus-one-shots-1.zip` and
  `cantus-loops-1.zip` as release assets of `luis314159/cantus-daw` under the tag
  `content-r37` (sizes and hashes above, pinned in `content.json`) before a build with this
  manifest reaches users; an older Cantus refuses the format-4 list by name.
- **Failing tests in the integrated tree that are not the library's** (the whole suite of
  `cantus-ui`, `-core`, `-engine`, `-content` on this branch after merging r37/integration at
  ffe3713a): finish's look — `app_flows` `the_toolbar_is_drawn_by_the_kit` (the clock paints
  `1.1` and `.000` apart, the test looks for `1.1.000`) and
  `the_mixer_draws_faders_with_a_scale_and_a_readout` (the scale has −40, the test looks for
  −30); `controllers_review` `without_ports_the_midi_cell_opens_the_settings` ("no MIDI
  cell" in the new top bar); `final_look` `the_pinned_strip_never_runs_a_caption_into_its_value`
  ("0.50" runs into "100%" at 1160 pt); `skins_body`
  `every_units_body_paints_in_its_face_never_with_the_plain_tokens` (Spatium: 32 vertices in
  the plain tokens); three short-row tests of the arrangement (`on_short_rows_a_clips_label_…`,
  `on_short_rows_the_name_field_…`, `the_letters_win_over_the_neighbours_bands_…`). And two
  timing tests under the machine's load of four builders: `sync_midi`
  `the_out_thread_sends_on_time` (median 26 ms late) and `stage_budget`
  `a_dancer_on_screen_stays_in_budget`.
- `views/preset_browser.rs`'s chips call `Query::facets`; anything else drawing those counts
  should too.

# First delivery (the foundation)

## Done

- **The vocabulary** (`cantus-ui/src/preset_tags.rs`): 11 styles (the brief's 9 + jazz · soul,
  worship · gospel), 7 instrument roles, 5 effect roles; the older descriptive tags read too;
  a unit's natural role when a preset names none.
- **Presets as data** (`preset_data.rs`, `crates/cantus-ui/presets/**.toml`): `[[preset]]`
  (key, styles, roles, values, text) and `[[tag]]` records for presets written in code — so
  Chorda's and Pulsus's are tagged without touching their files. `views::inst::factory_presets`
  and `views::fx::factory_presets` return code + data presets as one `&'static` slice (every
  existing reader sees them unchanged); `code_presets` is the arrays alone. Names stay locale
  keys. First file: `presets/instruments/sampler.toml` (Exemplar's roles).
- **The catalogue** (`preset_catalog.rs`): every unit's presets (factory, user's) with name,
  styles, roles and payload (instrument spec, effect values, Pulsus kit).
- **Finding** (`preset_query.rs`): search (every word, no case, no accents, also in style/role/
  unit names), style and role filters (alternatives within, narrowing across), counts.
- **Hearing** (`preset_audition.rs`): an offline render of a **copy** on a worker, played by the
  existing audition voice after the master. Instruments on their role's phrase (two bars, C
  minor, 120 BPM or the project's with *Sync BPM*); kits on a groove mapped to their own pads;
  effects on the selected clip (its track soloed in the copy, the insert not bypassed, ≤ 8 s),
  else the first clip the strip hears, else a fixed source by Cantus's own instruments (parts
  −8 dB so inserts have headroom). Outlines cached per preset (mini waveform) and for the
  player.
- **The browser** (`preset_browser.rs`, `views/preset_browser{,_rows}.rs`): FL 24's rows and
  player (the integrator's note) in our kit's public API — search + *Sync BPM*, STYLE and ROLE
  chips with counts, rows (▶ / stop / rendering arc, name, role · styles, mini waveform, gold dot
  on the loaded one), the player (name, what it was heard on, large waveform with playhead,
  count, *Load*). Movable by its title; keys ↑ ↓ (choose and hear), Space, Enter (load), Esc;
  AccessKit labels on rows, ▶ keys, chips and close. **Load = the unit's own command, one undo
  step** (`cmd.fx_preset`). No `kit/` file touched (finish's).
- **Entry points**: *Browse presets…* as the first row of the instrument shell's preset menu and
  of the effect frame's (`FxHost::can_browse_presets` / `browse_presets`, only the DAW's host
  says yes). Pulsus's kit menu is drums' file: one line at integration.
- **Tests**: unit tests in `preset_tags`, `preset_query`, `preset_data`; `tests/preset_audit.rs`;
  `tests/preset_render.rs`; `tests/instrument_reference.rs`; `tests/preset_browser_e2e.rs`
  (through `CantusApp::assemble`, see ADR 0101 *What the tests hold*).
- Manual: `docs/manual/{en,es}/presets.md`, linked from both indexes. Shots:
  `cargo run -p cantus-ui --example shoot_presets` (see *Shots*).

## Tested

- Windows, `RUSTFLAGS=-D warnings`: `cargo clippy -p cantus-ui --all-targets` clean;
  `cargo test -p cantus-ui` — **2 360 passed, 0 failed, 43 ignored** (the whole suite ran with
  the e2e test failing on the first-run windows over the menu — 2 359 + that one; after the
  fix the touched tests re-ran: lib `preset_*` 17, `preset_browser_e2e` 2, `preset_render` 1,
  `preset_audit` 1 + 1 ignored, `instrument_reference` 1). `preset_render` runs in 20 s in a
  debug build (CPU not asserted there) and 3 min in release (CPU asserted, sequential pass).
- Only `cantus-ui` touched (no engine, DSP, core or project change; no golden moved,
  `DSP_VERSION` 5, schema 53).
- Linux (WSL Ubuntu 24.04): `tools/wsl/check.sh r37/library clippy` in `~/ci-r37-library` at
  88979174 — workspace clippy `--all-targets` exit 0, no warning; the plug-ins without their
  editor exit 0; the clone's `target/` deleted after. Linux tests not run (the script's test
  mode is the whole workspace; the integrator's run covers it).
- Shots (`cargo run -p cantus-ui --example shoot_presets`): `docs/round37/library/` — Unda's
  leads with one heard (Night EN, Paper ES), Fervor searched for *tape* and heard on the
  user's clip (Night, Paper). What I looked at and fixed from them: an empty STYLE band (no
  preset names a style yet) is no longer drawn; outlines were invisible for quiet presets and
  are now on a 48 dB scale; the help line was cut and is shorter; Paper needs the app's light
  mode in the shoot.

## Counts per unit today (factory presets; `preset_audit.rs`)

| Unit | Presets | With a style | Floor |
|---|---|---|---|
| Unda | 15 | 0 | 24 |
| Calor | 12 | 0 | 24 |
| Modulus | 10 | 0 | 24 |
| Exemplar | 12 | 0 | 24 |
| Gravis | 14 | 3 | 24 |
| Lamina | 11 | 0 | 24 |
| Organum | 11 | 7 | 24 |
| Chorda (piano's) | 10 | 0 | 24 |
| Pulsus II kits (drums') | 8 | 0 | 16 |
| Aequalis 8 · Comprimo 8 · Terminus 8 · Spatium 8 · Multiplex 6 · Sibilus 6 · Porta 9 · Fervor 13 · Latitudo 13 · Culmen 10 | | 0 | 12 |
| **Delay 0 · Reverb 0** | | 0 | 12 |
| Concentus 10 · Flexus 8 · Phasis 8 · Tremor 9 · Vibrans 8 · Anulus 8 · Translatio 8 · Fluctus 10 · Tonsor 8 · Impetus 8 · Discrimen 8 · Tonus 9 | | 0 | 12 |
| Speculum | 6 | 0 | 8 |
| Saltator (no sound) | 4 | — | — |
| **All** | **304** | **10** | |

## Levels found (measured only — no listening yet)

- **Instrument kinds** (new instance, staging triad): Unda −18.7, Calor −19.2, Modulus −20.0,
  Exemplar −19.2, Gravis −15.7 (one note), Lamina −22.9, Organum −18.5, **Chorda −30.1** LUFS.
  The brief's "Lamina and Exemplar 8–9 LU under" was already corrected by ADR 0039 (round 24);
  Lamina's −4.2 LU is its headroom ceiling (−3.2 dBFS at velocity 127 on the dense riff).
  **Chorda is 11.4 LU under** — handed to piano's model 2 (target −18.7 ± 4.5 LUFS, dense-riff
  peak ≤ 0 dBFS); a model-1 trim would only change old songs. **Nothing corrected at kind level;
  no schema 57.**
- **Presets** on their role's phrase (`preset_render.rs`, release): **147 failures today** —
  138 out of their ±2 LU window (from +8.6 to −24.9 LU), 7 over 10 % of a core (Unda *Warp pad*
  31 %, Calor *Velvet pad* 20 %, Unda *Grain cloud* 15 %, *Glass choir*, *Stretched bells*,
  Calor *Unison saw*, Exemplar *Bells* 13 %), 2 clipping (Impetus *Punch* +3.4 dBFS and *Kick*
  +1.1 dBFS on the drum groove at −8 dB). Effects that change the level on insert: the phasers
  +2.4…+5.6 LU, flangers up to +4.7, Fervor *Overdrive* +4.2, Porta *Duck* −10, Comprimo *Duck*
  −12, Discrimen *Only transients* −15.8. All listed in `tests/data/preset_render_known.txt`;
  the second delivery retunes them by ear and strikes them off.
- Audition latency (release): median 0.13 s per preset, slowest 1.7 s (*Warp pad*).

## Marked lines in shared files (`// r37:library`)

- `src/lib.rs`: six `pub mod preset_*`.
- `src/views/mod.rs`: `pub mod preset_browser;`, `pub mod preset_browser_rows;`.
- `src/app.rs`: the `presets` field and its init; `views::preset_browser::window` in
  `dialogs`; `|| self.presets.open` in `global_shortcuts` (the browser reads its keys, as the
  picker does).
- `src/views/inst/frame.rs`: `factory_presets` → data; `code_presets`; *Browse presets…* row
  (indices shifted by one in the menu's click handler).
- `src/views/fx/mod.rs`: `factory_presets` → data; `code_presets`.
- `src/views/fx/frame.rs`: *Browse presets…* row and its click.
- `src/views/fx/host.rs`: `FxHost::can_browse_presets`/`browse_presets` (default no); `DawHost`
  implements them.
- `docs/manual/{en,es}/README.md`: one link each.

## Open — only what needs the owner

- **Ears**: nothing has been judged by listening yet; every level above is measured. The
  presets' retuning in the second delivery is by ear (mine, described), then the owner's.
- **Decisions** (for the integrator / owner): (1) Chorda's floor — my ADR says 24 for every
  instrument, piano's brief says ≥ 16; (2) Pulsus kits floor 16 (drums makes 5, there are 8);
  (3) the old **Reverb** is still offered in the picker beside Spatium — hide it (it stays for
  old projects) or give it 12 presets; Delay (offered in the picker) has none. *Decided by
  the integrator: Chorda 24, kits 16, the Reverb hidden, Delay to its floor — all done in the
  second delivery.*

## For the integrator (merge notes)

- **The vocabulary for piano and drums**: a preset names its styles and roles as tag keys in
  its `tags` — `preset.style.{trap,pop,house,rnb,lofi,rock,latin,ambient,film,jazz,worship}`
  and `preset.role.{bass,lead,pad,keys,pluck,fx,drums}` — or, without touching their files,
  as `[[tag]]` records in `crates/cantus-ui/presets/instruments/chorda.toml` /
  `presets/kits.toml` (a kits file needs one `FILES` line in `preset_data.rs`).
- **The render check sees Chorda's and the kits' presets.** When piano's model-2 presets or
  drums' new kits merge, any of them outside its window (or clipping, silent, over budget)
  is a *new* failure and fails `preset_render.rs`. Either they are levelled (the targets are
  in the test: keys −19 LUFS ± 2 on the keys phrase, drums −19 ± 2 on the groove — −16 in the first delivery), or the list
  is regenerated once at the merge: `CANTUS_PRESET_KNOWN_DUMP=1 cargo test --release -p
  cantus-ui --test preset_render` (in release, so CPU is measured).
- **`instrument_reference.rs` lists Chorda as pending** (piano's model 2): it fails once a new
  Chorda is level, until the line is struck off.
- `views/inst/frame.rs`'s preset menu has one row more at its top (*Browse presets…*): a
  test that clicks the menu by row index shifts by one (none in the tree today).
- Pulsus's kit menu (drums' `pulsus_editor.rs`) gets *Browse presets…* with one line:
  `app.open_preset_browser(crate::preset_audition::Target::Track(track))`.
