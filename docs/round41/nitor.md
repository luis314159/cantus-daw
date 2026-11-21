# Round 41 — `nitor`: Nitor, the one-knob enhancer (ADR 0151)

Branch `r41/nitor` from `r41/contract`. **Stopped for the owner twice over**: the face's design
sheet v1 (his OK before the full editor), and the presets' sound (his ears). Everything else
below is built and measured.

## What it is

FL Studio's Soundgoodizer, Low Lifter and Maximus/Emphasis exciters in one unit, level-matched:

- **One knob** (`amount`) over five curated **characters** — Clear, Punch, Warm, Air, Loud —
  each a recipe for Multiplex's dynamic-phase dynamics (four bands, downward *and* upward
  compression, a static tilt) plus what it adds to the exciter and the low end.
- **Level independence** (better than the reference: Soundgoodizer acts on how hot you drive
  it): a K-weighted loudness of the input brings any programme to −18 LUFS before the
  dynamics and back after. Measured: band peaks and gains of the test programme identical at
  0 dB and −20 dB of input (`band_levels_of_programme_at_the_reference`).
- **The low end**: a sub that follows the bass an octave down (Saltus's divider) or sits on the
  **song's tonic** (the project's key, ADR 0153), the bass's harmonics for small speakers,
  *tight* (each hit opens the low band 30 ms, then a 120 ms fall toward −12 dB), a *lift* on
  the tonic (+6 dB) and its octave (+3 dB).
- **The exciter by band** (body 400 Hz, presence 2.5 kHz, air over 7 kHz): only harmonics,
  even … odd by *colour*, 4× oversampled; the same share at any level (measured within 0.5 dB
  from −6 to −46 dBFS).
- **The level match** (on by default, every preset keeps it): what you hear is the colour, not
  the loudness.

Design in full: ADR 0151's *builder's decisions*; the page `docs/plugins/nitor.md`; the
manual's section (EN/ES) after Comparatio.

## Done

| Layer | Files (owned) |
|---|---|
| DSP | `cantus-dsp/src/nitor.rs` (settings, characters, recipes, meter), `nitor_run.rs` (glides, `set`, `process`, rest, `snap`), `nitor_chunk.rs` (a grid cell's audio; the dynamics parked and woken), `nitor_level.rs` (pre-gain and level match), `nitor_excite.rs`, `nitor_low.rs`; tests `nitor_tests.rs`, `nitor_tests_measure.rs`, `nitor_testkit.rs`, `tests/nitor_alloc.rs` |
| Core | `cantus-core/src/fx/nitor.rs`: 16 parameters, `tonic` |
| Engine | `cantus-engine/src/fx/nitor.rs`: the unit, the song's key (re-reads only the tonic, no allocation), 21 telemetry slots |
| UI | `views/fx/nitor.rs` (the editor: corona + cards), `nitor_corona.rs` (the corona); `kit/skin/side_r41.rs` (the face); `presets/effects/enhancer.toml` (13); locales `fx/nitor.{en,es}.txt`; examples `shoot_nitor.rs`, `nitor_listen.rs`; `tests/nitor_e2e.rs` |
| Docs | `docs/plugins/nitor.md`, the manual EN/ES, ADR 0151, this file, `docs/beta-checklist.md` (Ronda 41 — Nitor), `docs/DEVELOPING.md` (the two tools) |

### Shared files touched (each line marked `r41:nitor`)

`cantus-dsp/src/lib.rs` (one line), `cantus-core/src/starting.rs` (the starting preset),
`cantus-engine/tests/support/heavy.rs` and `cantus-bench/src/kernels.rs` (heavy settings),
`cantus-plugins/tests/common/mod.rs` (test settings, the `Direct::Nitor` arm),
`cantus-plugins/tests/golden/ids.txt` (the new parameters, blessed),
`cantus-ui/src/views/picker_snap.rs` (`effect_name`), `cantus-ui/tests/preset_audit.rs` (the
floor-0 line deleted), `docs/manual/{en,es}/README.md`, `docs/beta-checklist.md`,
`docs/DEVELOPING.md`. Not touched: `web/src/units_data.rs` — **the integrator's card**.

## Measured

(filled in below as the runs finish)
