# Round 41 — the builders' briefs

Read `docs/round41/BUILDING.md` first (ownership, rules, markers), then `CLAUDE.md`, your ADR
(0150, 0151 or 0152; 0153 for the song's key), and the BACKLOG item named in your brief. Each
brief's **done when** is the owner's bar from `docs/BACKLOG-excellence.md`; what only his ears
or eyes can settle is handed to him, never decided for him.

## `fides` — the resonator instrument (ADR 0150; BACKLOG §3e item 6, §6l A2)

**What:** an exciter into a resonator with a body, the class of Ableton's Collision and
Tension, Logic's Sculpture and AAS Chromaphone — and the answer to FL's Sakura and Plucked!.

- **Exciters:** pluck, pick, a bow by a friction curve (stick-slip), a mallet (Lamina's
  hammer shape and contact law as a reference: `lamina_voice::HammerShape`, `contact_s`),
  noise, and a live input (a track's audio into the resonator).
- **Resonators:** waveguide strings reusing `cantus_dsp::gravis::string`'s public functions
  (`lagrange`, `loss_design`, `dispersion_fit`, `ap_delay` …; its `Guide` is private — build
  your own string on those functions); modal bars, plates, membranes and tubes. Pulsus's modal
  tables (`pulsus_modal.rs`) are private to Pulsus: keep Fides's own (the published mode
  ratios, cited) or ask the integrator to expose them additively — Pulsus's and Tympanum's
  golden hashes must not move.
- **A body** (Chorda's `chorda_board`/`chorda_body` show the method: radiation responses built
  off the audio thread, convolved), **coupling** between two resonators, **MPE** pressure and
  slide on bow pressure and position.
- **Presets** (24 at least, by ear): marimba, vibraphone, kalimba, steel pan, harp, nylon guitar,
  bowed cello, glass — then pads, plucks and textures the family is loved for.
- The kit shell (Play / Edit / Matrix), its face from a design sheet the owner approves, its
  manual page, its card on the site, `--editor fides`.

**Done when:** pitch within 0.1 cent at any bend (measured with `tunekit` across the range);
renders repeat to the bit (offline = real time, block sizes 1/37/64/512, parallel = serial with
voices across the pool, ADR 0038); a stored model version (`MODEL`); its CPU budget measured
(the bench's `inst.fides.v32`); the eight named presets judged "convincing in solo" by the
owner (the 10-listener panel is his to run); then **`Resonator` in `InstrumentKind::ALL` and the
picker** in one commit, with the null corpus's new item measured and an end-to-end test that
picks Fides through `CantusApp::assemble`, plays a note and hears it.

## `nitor` — the one-knob enhancer (ADR 0151; BACKLOG §6l B9, B10, B12)

**What:** FL's Soundgoodizer, Low Lifter and the Maximus/Emphasis exciters in one unit.

- **One amount knob** over a few curated multiband **characters** (named, chosen by a switch),
  on Multiplex's crossovers (`cantus_dsp::multiplex*`: Linkwitz–Riley splits that sum to an
  allpass, or linear phase).
- **A low end**: sub-harmonics (Saltus's octave divider, `saltus_octave::Divider`, shows the
  phase-locked method) tuned to the song's tonic — `fx::nitor::tonic(key, song)` with the song's
  key handed by the engine (`FxUnit::set_song_key`, ADR 0153; the stub keeps it) — and
  tightening.
- **An exciter by band**: harmonics generated per band, oversampled through
  `cantus_dsp::fervor::os`.
- **A level-matched output** so a preset changes the sound, not only the loudness:
  `preset_render` holds an effect's preset within ±2 LU of its dry source — meet it, or argue an
  exemption in `preset_measure::by_design`.
- Its face (a design sheet the owner approves; replace the placeholder in
  `kit/skin/side_r41.rs`), its starting preset (ADR 0071: it sounds on insert at a gentle amount),
  12 presets by ear, `docs/plugins/nitor.md`, the manual section, the site's card.

**Done when:** a null to the bit at amount 0 (and every default); block-size independence at
tolerance 0; exact rest; offline = real time; the CLAP/VST3 export bit-identical to the DSP (the
plug-in tests' `Direct` arm names its DSP); the key followed live (a test changes the project's
key and hears the sub move) and fixed in the plug-in; its CPU budget measured; the owner's ear on
the characters.

## `pedals` — Fragor's five circuits as pedals of their own (ADR 0152; BACKLOG §3b)

**What:** the green overdrive, the op-amp distortion, the silicon distortion, the germanium
fuzz and the triangle muff, each a stompbox on Tabula's board, anywhere in the chain.

- Each pedal's engine unit is **Fragor's DSP with only its circuit in the first slot**
  (`cantus_dsp::fragor::Fragor`, `BoardSettings { chain: [Some(model), None, None], .. }`) — do
  not change Fragor (`fragor*.rs` is nobody's this round; its golden hashes must not move).
- Each pedal's table under Fragor's keys: its circuit's three knobs (`od.drive` …), its
  footswitch, `in`, `out`, `mix`, `gate`, `mono`, `os` — so a pedal and a one-slot Fragor read the
  same values; every default a null to the bit.
- Faces: Fragor's per-circuit art on the board (`views/fx/fragor.rs`'s `SHAPES`, `TEXTURES`,
  `LETTERS`, `MOTIFS`, `kit/skin/pedals.rs`'s `FRAGOR` finishes) for each kind; the knobs on
  the face its three.
- A few presets each, by ear; a new factory board in `tabula_presets::factory()` that shows
  several drives in a chain; the Tabula editor's shelves offer the five (they read
  `picker_catalog::board_devices`).

**Done when:** each pedal equals Fragor with only that slot filled, **to the bit**, at defaults,
at heavy settings and at drawn ones, at block sizes 1/64/1024 (a test of all five); one circuit's
CPU measured against a two-pedal Fragor board (about a third); the faces approved by the owner;
an end-to-end test through `CantusApp::assemble` that puts two of them on a board with another
pedal between and hears the chain.
