# Vetus — lo-fi

*Effect kind `LoFi`. References: XLN RC-20 Retro Color, iZotope Vinyl, FL Studio's Fruity
Squeeze, Ableton's Redux and Vinyl Distortion (ADR 0130).*

*Vetus* is Latin for old. Vetus ages a sound as an old medium aged it: six modules in the
order a recording travels — **bits** and **sample rate** (the cheap converter), **wow and
flutter** (the medium's speed), **age** (dropouts and wear), **noise** (crackle, hiss, hum)
and **tone** (the band of the medium that plays it back) — and **Character**, which takes
every module that is on towards its oldest.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/vetus.rs` (33 keys; the media's bands) |
| DSP | `crates/cantus-dsp/src/vetus.rs` (settings, Character, the tone's response), `vetus_unit.rs` and `vetus_run.rs` (the chain, the grid, rest, telemetry), `vetus_digital.rs` (bits, rate), `vetus_noise.rs`, `vetus_age.rs`, `vetus_filter.rs`; tests in `vetus_tests.rs`, `vetus_tests2.rs` |
| Engine glue | `crates/cantus-engine/src/fx/vetus.rs` |
| Editor | `crates/cantus-ui/src/views/fx/vetus.rs`, `vetus_cassette.rs` and `vetus_record.rs` (the two objects), `vetus_spectrum.rs` (what comes out), `vetus_modules.rs` (the six cards, the inspector's picture); `views/fx/unit_view.rs` (the object chosen) |
| Face | `crates/cantus-ui/src/kit/skin/side.rs`: three colourways — *Cassette* (`VETUS`, the face), *Sleeve*, *Walnut*; the objects' colours in `kit/skin/records.rs` |
| Strings | `crates/cantus-ui/locales/fx/vetus.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/lofi.toml` (fourteen; *Worn cassette* is the starting preset) |
| Tests | `crates/cantus-engine/tests/fx_vetus.rs`, `rt_memory::vetus_never_allocates`, the family's contract; `crates/cantus-ui/tests/vetus_e2e.rs`, `vetus_face.rs`; the CLAP/VST3 tests over every descriptor |
| Exported | `Cantus Vetus` (CLAP `com.github.lalmazan.cantus.vetus`) |
| Examples | `shoot_vetus` (the design sheet), `render_vetus` (the sound examples) |

## What the references do well, and what people complain about

- **RC-20** is the reference for the sound and the workflow (six modules, one *Magnitude*):
  its modules are opaque — a knob called *Flux* or *Sputter* says little — and nothing shows
  what the converter does to the spectrum.
- **iZotope Vinyl** draws a record and its era; its controls are few and its noise is a loop.
- **Fruity Squeeze** and the classic bit crushers alias at their sample rate with no choice.

## Ours, better

- Every module says what it is, in its units (bits, Hz, cents, the medium); Character is one
  knob over the modules that are on, never a hidden mix.
- The converter's Nyquist, the bits' noise floor and the medium's band are drawn on the
  output's spectrum and dragged there; the band drawn is the band heard (within 0.1 dB).
- The rate module offers the aliasing *or* an anti-alias filter (−54 dB of fold) *or* both
  filters.
- The noise is a function of the song, never a loop: the same crackle in the same places in
  any render, from any bar, at any buffer size; it can follow the music.
- With every module off it is a wire, to the bit; it rests in silence and costs nothing.
