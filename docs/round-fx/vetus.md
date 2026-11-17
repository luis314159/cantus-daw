# The effects side round — `vetus`: Vetus (lo-fi) and Libra (utility) (ADR 0130)

Branch `fx/vetus` from `fx/integration` 3b6f2eba. Vetus: Phase A (DSP, engine, the design
sheet, sounds), the owner's verdict (both objects, the Cassette face, controls live while a
module is off, no vents, a cleaner Slate), v2 approved ("síguele, se ve chido"), Phase B.
Libra: its DSP and engine, a design sheet, the owner's verdict (a brass balance at the
centre, the tuner's A beside the tuner, every element saying what it is, the Spanish words),
v2 (`compartir\libra\design\v2\`, approved), Phase B. `fx/integration` e5aedd09 (Cribrum,
Fragor, Lapsus v3) merged into `fx/vetus` on the way.

## Done

| Layer | Files (owned) | Shared files (lines marked `// fx:vetus` / `// fx:libra`) |
|---|---|---|
| Model | `cantus-core/src/fx/vetus.rs` (33 keys), `fx/libra.rs` (12 keys) | `starting.rs` (Vetus's *Worn cassette*; Libra neutral, with its reason) |
| DSP | `cantus-dsp/src/vetus.rs`, `vetus_unit.rs`, `vetus_run.rs`, `vetus_digital.rs`, `vetus_noise.rs`, `vetus_age.rs`, `vetus_filter.rs`, tests `vetus_tests.rs`, `vetus_tests2.rs`; `libra.rs`, `libra_tests.rs` | — |
| Engine | `cantus-engine/src/fx/vetus.rs`, `fx/libra.rs`, `tests/fx_vetus.rs`, `tests/fx_libra.rs` | `tests/support/heavy.rs`, `tests/rt_memory.rs` (`vetus_never_allocates`, `libra_never_allocates`), `cantus-bench/src/kernels.rs` (both heavy arms) |
| UI | `views/fx/vetus.rs`, `vetus_cassette.rs`, `vetus_record.rs`, `vetus_spectrum.rs`, `vetus_modules.rs`, `unit_view.rs` (a unit's chosen view), `libra.rs`, `libra_display.rs`, `kit/skin/records.rs`, locales, presets `lofi.toml` (14), `utility.toml` (14), examples `shoot_vetus.rs`, `render_vetus.rs`, `shoot_libra.rs`, tests `vetus_face.rs`, `vetus_e2e.rs`, `libra_e2e.rs`; `views/fx/libra_balance.rs`, `kit/skin/scales.rs` | `kit/skin.rs` (`pub mod records`), `kit/skin/side.rs` (Vetus's three colourways, Libra's face, the side round's distinctness test), `views/fx/frame.rs` (window sizes), `views/fx/mod.rs` (`unit_view`), `views/fx/controls.rs` (`card_lamp_live`), `settings.rs` (`unit_views`), `app.rs` (`apply_unit_views`, `set_unit_view`, in `assemble`), `agent_units.rs`, `preset_measure.rs` (a utility's level is by design), `demos/heart/desk.rs` (the percussion through *Dusty drums*, the bass through *Bass in mono*), `tests/demos.rs`, `tests/preset_audit.rs` (both floor lines removed) |
| Plug-ins | — | `tests/golden/ids.txt` (both units' ids appended), `tests/common/mod.rs` (both `Direct` arms and settings lines) |
| Docs | `docs/adr/0130-…`, `docs/plugins/vetus.md`, `docs/plugins/libra.md`, this file | manual EN/ES (both sections, the list of effects), `web/src/units_data.rs` (both cards) |

## Measured

- Raw, a 9 kHz sine held at 8 kHz folds to 1 kHz at −0.2 dB; behind the anti-alias filter at
  −53.6 dB. The hold's image of 1 kHz at 7 kHz: −22.8 dB held, −47.9 dB smoothed.
- The wow's swing measured from zero crossings: ±20 ct within 2 for a 20 ct setting.
- The tone band drawn = heard within 0.1 dB (four media, 40 Hz – 16 kHz).
- Hiss at 100 %: −24 dBFS RMS (tape and vinyl, within 1 dB); hum −30 dBFS.
- Rest: Vetus's heavy settings at rest 2.4 – 3.0 s into a silence; Libra's at once.
- Vetus's window playing (dev profile, another builder compiling): UI pass mean 1.81 ms
  (cassette), 2.45 ms (record) of the 16.7 ms frame.
- The presets fitted to their level by `preset_tool fit` (Vetus's moved −0.3 … −6.5 dB).
- Libra: the delay exact to the sample; the tuner within ±1 cent on sines (at A = 440 and
  432) and ±3 cents on the pitch tests' sung vowel.

## Shown to the owner

- `compartir\vetus\design\` (v1), `design\v2\` (the cassette, the Cassette face, the record
  as the other view, the colourways) — approved; `compartir\vetus\sound\` (guitar, drums,
  keys; README credits "Guitar: Carbonthief, CC0").
- `compartir\libra\design\` (v1) and `design\v2\` (the balance; rest, playing, mono with
  the right inverted, Slate EN and Paper ES, 75/150/200 %, the preview,
  `compare-references.png` beside Ableton's Utility and Tuner and FL's Fruity Balance and
  Stereo Shaper) — v2 approved.

## Verified only in emulation / needs the owner

- The sound by ear (the examples), the exported plug-in's window in a real host.

## Left

- The site's unit pictures (`web/assets/units/vetus-*.webp`) from the `units` reshoot at
  the integration (`tools/shots/reshoot.sh units`, `check_units.py --fix`).
- The site's pictures for Libra too (`web/assets/units/libra-*.webp`); its card's sizes are
  provisional until `check_units.py --fix`.
