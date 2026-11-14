# The fx side round — `fragor`: Fragor, the pedals (ADR 0131, provisional)

Fragor (`EffectKind::Pedal`): a board of up to three circuit-modelled distortion and fuzz
pedals in any order — green overdrive (TS808), op-amp distortion (RAT), silicon distortion
(DS-1), germanium fuzz (Fuzz Face, Bias = a dying battery), triangle muff (Big Muff π) —
with a gate before them, input, mix, output, mono and 2×/4×/8× oversampling. Phase A
(the DSP, its tests, the table, the engine unit, the design sheet) was reported at its
checkpoint; Phase B followed the owner's verdicts (palette A, a valve amp's character,
elaborate original pedal art, pedals of any size, energy in the cables; v3 approved).

## Done

| Layer | Files (owned) | Shared files (lines marked `// fx:fragor`) |
|---|---|---|
| DSP | `cantus-dsp/src/fragor.rs` (the board), `fragor_dk.rs` (the DK circuit solver), `fragor_circuits.rs` (the five pedals), `fragor_gate.rs`, tests `fragor_tests.rs`, `fragor_spice_tests.rs`, example `examples/fragor_sounds.rs` | — |
| Model | `cantus-core/src/fx/fragor.rs` (the table) | `starting.rs` (the starting preset, at its fitted level) |
| Engine | `cantus-engine/src/fx/fragor.rs`, `tests/fx_fragor.rs` | `tests/support/heavy.rs` |
| UI | `views/fx/fragor.rs` (editor), `fragor_view.rs` (display, its worker thread), `pedal_draw.rs`, `pedal_art.rs`, `valve_draw.rs` (reusable), `kit/skin/pedals.rs` (finishes, shapes, valve colours), locales `fx/fragor.{en,es}.txt`, `presets/effects/pedal.toml` (15), `tests/fragor_e2e.rs`, examples `shoot_fragor.rs`, `fragor_guitar.rs` | `views/fx/mod.rs`, `views/fx/frame.rs` (designed size 1140 × 800; hue Tone), `views/inspector_mini.rs` (hue Tone), `kit/skin.rs` (`pub mod pedals`, `FRAGOR_COLOURWAYS`), `kit/skin/side.rs` (palette A as the face; B and C as data), `agent_units.rs` (the AI's description), `tests/preset_audit.rs` (Pedal's floor line removed), `demos/heart/desk.rs` (Fragor on the showcase's guitar), `tests/demos.rs` (the showcase checks it), `preset_measure.rs` (Fragor's CPU budget, 0.25), `tests/learn_everywhere.rs` (a footswitch of a pedal not on the board draws nothing) |
| Plug-ins | — | `tests/golden/ids.txt` (ids appended), `tests/common/mod.rs` (Fragor's settings line) |
| Docs | `docs/adr/0131-fragor-the-pedals.md`, `docs/plugins/fragor.md`, this file | `docs/manual/{en,es}/README.md` (Fragor's section) |
| Site | `web/assets/units/fragor-{dark,light,thumb}{,-es}.webp` | `web/src/units_data.rs` (the card) |

The circuits, the values, the method and what each test holds are in the ADR.

## Measured

- **Against the schematics** (an independent SPICE-class analysis of each whole circuit,
  `fragor_spice_tests`): TS808, RAT, DS-1, Fuzz Face within 0.1 dB from 80 Hz to 3 kHz
  and 0.25 dB at 8 kHz; Big Muff within 0.08 dB to 1 kHz, 0.5 dB at 8 kHz — unchanged by
  Phase B's optimisations.
- **Published operating points:** Fuzz Face Q2 collector 4.7 V, Q1 0.7 V (ElectroSmash's
  targets 4.5 V and 0.5 – 0.7 V); Big Muff input booster collector 7.0 V (ElectroSmash 7 V).
- **Aliasing** (2.5 kHz at 300 mV, op-amp distortion at 80 %, 48 kHz): −30.0 dB at 2×,
  −53.6 dB at 4× (default), −63.1 dB at 8×.
- **The dying battery:** fresh, a decaying note's output holds within 1 dB while its input
  falls 24 dB; at Bias 10 % it collapses 20 dB faster than the input.
- **CPU per pedal**, mono, 48 kHz, 4× (the default), release build, a plucked note
  (`fragor_tests::cpu_per_pedal`, `--ignored`). Every figure was taken **while other
  builders kept this machine at full load** — upper bounds, to be measured again quietly:

  | Pedal | Phase A | Phase B |
  |---|---|---|
  | Green overdrive (1 port) | 4.5 % | 5.3 % (load: nothing changed in it) |
  | Op-amp distortion (2) | 6.5 % | 8.3 % (load) |
  | Silicon distortion (2 ‖ 2 → 2 ‖ 1) | 13.7 % | 12.8 % |
  | Germanium fuzz (4) | 9.7 % | 8.2 % |
  | Triangle muff (2 ‖ 3 ‖ 5 → 2 ‖ 3 ‖ 3) | 22 % | 17.2 % |

  Normalised against the machine's speed at each run (a fixed exponential loop timed
  beside it), the Muff is about 40 % faster, and the Silicon about 35 % faster from its
  op-amp alone (linear: its slew rate is never reached) and 26 % more from the quadratic
  predictor, the earlier stop and the devices' divisions done once.
- **The editor's frame** with the window open and the song playing (1140 × 800 at 100 %,
  dev profile, loaded machine, `FRAGOR_TIME=1 cargo run -p cantus-ui --example
  shoot_fragor`): 34.6 ms mean before the display's curves moved to their own thread,
  **7.6 ms mean, 6.7 median, 14.7 at the 95th percentile** after (the worst, 46 ms, a frame
  that met the machine's load).

## Tests

`cantus-dsp` (`fragor_tests`, `fragor_spice_tests`), `cantus-engine` (`fx_fragor`, the
contract's `fx_contract` and `rt_memory` arms), `cantus-ui` (`fragor_e2e` on the boot path,
`demos`, the presets' tests, i18n, the lints), `cantus-plugins` (CLAP and VST3 exports
bit-identical to the engine's unit; ids), `cantus-ai` (the description). The finishing
suite's result is in the builder's report.

## Shown to the owner

`C:\projects\daw-fx\compartir\fragor\`: `design\v2`, `design\v3` (approved) with
`design\v3\anim` (the cables' energy, 30 frames and an animated WebP), `design\v3.1` (the
never-eliding lettering, the brighter filaments); `sound\` (Phase A, synthesised guitar) and
`sound\guitar\` (a real guitar: Carbonthief's CC0 samples, DI at −6 dBFS peak, each pedal,
the chain, the battery sweep, two presets, level-matched; its README credits "Guitar:
Carbonthief, CC0"). The samples are not in the repository.

## Verified only in emulation

- Every sound was judged by measurement and by listening to rendered files; nobody has
  played a guitar through Fragor live through an interface (for `docs/beta-checklist.md`:
  a guitar into an interface, Fragor on the armed track, monitoring at 4×, which adds
  1.1 ms).
- The plug-in exports were tested in our own harness, not in a third-party DAW.

## Left for the integrator or the owner

- **The site's preset excerpts** (`web/assets/units/sounds/fragor-*.mp3` and
  `web/src/units_sounds.rs`) are generated by `cargo run --release -p cantus-ui --example
  render_unit_sounds -- web`, which renders every unit: best run once at integration for
  all the side round's units.
- **Colourways**: palettes B and C wait as data (`kit::skin::FRAGOR_COLOURWAYS`) for the
  colourway mechanism (Cribrum's, at integration).
- **The pedal sizes** are a view preference of the editor's session; keeping them in the
  project would be a model change (the kit already takes them as data).
- **Reduced motion**: the app has no such setting yet; the cables' energy would honour it.
- **The round shape** recalls the classic round fuzz enclosure as a form; its colours,
  art and lettering are original.
- **Claxon reads no 32-bit integer FLAC** (found with the CC0 guitar samples): a user's
  32-bit FLAC will not load in Cantus. For BACKLOG.
- **CPU** measured again on a quiet machine.
