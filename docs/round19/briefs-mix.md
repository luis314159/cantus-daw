# Round 19 briefs — the master chain, three mastering effects, the demos

## master — effects on the master channel (ADR 0026)

The owner called this "very, very important". FL's master track is the reference: the
master is a mixer track with its own ten slots, and everything that works on a strip's
slot works there.

- **Commands**: every effect command (`InsertEffect`, `RemoveEffect`, `MoveEffect`,
  `SetEffectBypass`, `SetEffectParam`, `ClearEffectParam`, `SetEffectSidechain`,
  `SetEffectMix`, `SetPluginParam`, `SetPluginState`, `SetEffectMidiSource`, …) accepts
  `StripId::MASTER` and acts on `Project::master.effects` — one resolver
  (`Project::effects_of(strip) / effects_of_mut`) rather than a special case per command.
  Add `SetMasterFxEnabled`. A sidechain *from* the master is refused (cycle); a sidechain
  *into* a master insert from any strip is allowed and orders that strip before the master
  (it always is). Undo fuzz arms for the master's chain.
- **Engine** (`processor.rs`, the section after the strips where the master bus is
  summed — engine-rec edits the block start and the track loop, stay out of those): run
  `Snapshot::master_effects` on the summed mix **before** the master's gain, balance,
  width and polarity; respect bypass, `fx_enabled`, each slot's mix; build and dispose
  units exactly as strips do (`known_effects`, garbage paths); hosted CLAP/VST3 on the
  master work too. The chain's latency is added to the output latency ADR 0022 reports (the
  heard position) — nothing inside the mix moves. `Meters::master_gr` from the chain's
  dynamics. The mix recorder's master take is post-chain; stems (ADR 0021) — decide and
  document whether stems pass through the master chain (default: no, as most DAWs; an
  option *through master chain*).
- **UI**: the master strip on the desk (`views/mixer*.rs`) shows its insert slots like any
  strip (add, reorder by drag, bypass, mix, open the device window); the inspector's sheet
  for the master shows the chain with mini views; device windows, presets, A/B, the
  analyser and the hint line all work with `StripId::MASTER`; the strip menu's *add
  effect* lists everything (the three new effects included). Find every place that does
  `project.strip(id)` to reach an effect and route it through the resolver.
- **Tests (first)**: an empty chain and a chain of bypassed effects are bit-exact against
  today's master; the chain runs before the master fader (a limiter at −6 dB ceiling with
  the master at +6 dB peaks at 0 dBFS, not −6); latency on the master moves the heard
  position by exactly that; undo/redo of every master command; the master chain survives
  save/open; the UI opens a master effect's window.

## fervor, latitudo, culmen — three effects to the family's bar

Each builder owns one `EffectKind` end to end, in the way the existing seven were built
(round 16, ADR 0017/0018; look at Porta — `fx/gate.rs` in core, `cantus-dsp/src/porta*.rs`,
`cantus-engine/src/fx/gate.rs`, `cantus-ui/src/views/fx/gate.rs`, `docs/plugins/gate.md`,
its entries in `views/fx/mod.rs` and `cantus-plugins/src/desc.rs`):

1. Research the references (your knowledge, and the web if your tools allow): what makes
   the best of each class excellent, what users complain about. Write
   `docs/plugins/<name>.md` (what it is, every parameter, the DSP, the decisions).
2. The parameter model in `cantus-core/src/fx/<module>.rs` (replace the stub; keys stable
   from here on; every default a null).
3. The DSP in `cantus-dsp/src/<name>*.rs` with **null tests first**: at defaults the output
   is the input to the bit (or exactly delayed by the reported latency), block-size
   independence at tolerance 0, determinism, no denormals, no NaN on silence or on a full-
   scale square, headroom tests.
4. The unit in `cantus-engine/src/fx/<module>.rs` (replace the wire), telemetry for the
   editor, latency reported for delay compensation.
5. The editor in `cantus-ui/src/views/fx/<name>.rs` on the device frame (kit only), with
   factory presets; take your kind out of the contract's `has_editor` exclusion; register
   it in `views/fx/mod.rs`; add it to the CLAP/VST3 build (`cantus-plugins/src/desc.rs`,
   and whatever the build lists) and prove the exported plug-in is bit-identical to the
   native one (`crates/cantus-plugins/tests/hosted_in_cantus.rs`).
6. Render the editor with the shoot example and look at it next to the existing editors.

- **fervor** (`EffectKind::Saturator`, module `saturator`): saturation and colour. Models:
  tape (soft, level-dependent with a gentle high-end loss), tube (asymmetric, even
  harmonics, bias), transistor/overdrive (Blood Overdrive's idea: pre-amp drive, a tone
  stage, post gain, a "x100" boost), fuzz/hard clip, foldback, bit/rate reduction, and a
  **drawn waveshaper** (Fruity WaveShaper's editable transfer curve — points with tension,
  symmetric or not, presets). Oversampling (off, 2×, 4×, 8× with linear- or minimum-phase
  filters; report latency), pre/post tone (tilt or low/high cut), bias, dry/wet, output
  with auto level-match (hear colour, not loudness), and optionally 2–3 bands
  (Saturn-style) if it fits the time. The display shows the transfer curve and the
  harmonics it adds (a spectrum of a test tone).
- **latitudo** (`EffectKind::Stereo`, module `stereo`): stereo control and mid/side.
  Width (0 mono … 200 %), mid gain, side gain, mono below a frequency (bass mono with a
  steep crossover), balance, rotation, swap, polarity per side, side high-pass, a
  mid/side tone section (a tilt or a two-band shelf on mid and on side — Patcher's M/S
  preset built in), haas widening as an explicit option with a mono-compatibility warning.
  The display: a **vectorscope** (goniometer, Lissajous) and a **correlation meter**, a
  balance meter; optional per-band width (three bands) if it fits.
- **culmen** (`EffectKind::Maximizer`, module `maximizer`): the mastering maximizer,
  FL's Maximus to our bar, iZotope Ozone's Maximizer and FabFilter Pro-L/MB in mind. Up to
  three bands (linear-phase or minimum-phase crossovers, adjustable) plus the full band,
  each with a compressor/expander **curve** you shape (threshold below/above, ratio up and
  down — Maximus's two-sided curve), attack/release, band gain, solo/bypass; then a
  look-ahead brickwall stage with true-peak ceiling and release character; stereo link;
  loudness readouts (momentary/short/integrated LUFS, true peak, PLR) and target presets
  (streaming −14 LUFS, club, CD); a *learn input gain* button; delta listen; dither on the
  output. Reuse `cantus-dsp`'s Multiplex crossovers and Terminus's limiter and loudness
  code where it fits rather than writing them again. The display: the bands' gain
  reduction over a scrolling waveform, the curve editor, the meters.

## demos — songs that show what Cantus can do, and the startup project

The owner: *"we're missing example tracks, and the default song when the program opens
should change, to show more of what our DAW can do and make a good impression."*

- Build **five or six demo songs** as Rust builders (like `welcome_project` in `app.rs`,
  but in a new module, e.g. `crates/cantus-ui/src/demos/`), each 1–2 minutes with a real
  arrangement (intro, verse, chorus/drop, bridge, outro — sections as clips across bars,
  automation-free since automation does not exist yet), in different styles: e.g. a
  modern pop/worship ballad (Lamina EP, Unda pads, Gravis bass, Pulsus II kit), a
  house/EDM track (Calor supersaw lead, Gravis 808/bass, sidechained Comprimo), lo-fi
  hip-hop (Lamina with a Fervor-ish colour once it exists; until then Spatium and
  Aequalis), a cinematic/ambient piece (Unda + Modulus bells + Spatium), a funk/soul groove
  (Organum with its rotary, Gravis bass, kit), a trap beat (808 slides). Use **every
  native instrument at least once** and the effects on strips (Aequalis, Comprimo,
  Spatium, Multiplex, Sibilus, Porta, Terminus) with sensible, musical settings; put a
  limiter (Terminus) — and later Culmen — **on the master** (`Master::effects`, the
  engine plays it once the master builder lands). Real harmony (voice leading, inversions),
  humanised velocities and timing, drum programming with ghost notes and fills. Name
  tracks, colour them, set track icons, mixer strips named and coloured, sends to a shared
  reverb return.
- **The startup project**: replace `welcome_project` with the strongest demo's first
  section (8–16 bars, looping, immediately impressive, using several native instruments
  and effects, the mixer populated), keeping it light enough to load instantly. Keep
  `starter_project` (tests depend on it) untouched.
- **A demo picker**: File → *Demo songs* (a list with name, style, tempo, length) and a
  key; opening one replaces the project like *New* does (asking to save first, as New
  does). Optionally the splash/welcome offers it.
- **Prove they sound right** without ears: offline-render each demo
  (`cantus_engine::render`) in a test: not silent, no NaN, peak below 0 dBFS, integrated
  loudness in a sane range (e.g. −20 … −9 LUFS; Terminus has the loudness code), every
  track contributes (render with each track muted and compare), no clip or note outside
  the song. Also a test that every demo round-trips through save/open and that every
  instrument kind appears in at least one demo. Render the arrangement and mixer of the
  startup project with the shoot example and look at them.
- Strings in `locales/areas/demos.*`.
