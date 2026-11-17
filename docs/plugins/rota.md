# Rota — rotary speaker

*Effect kind `Rotary`. References: the rotary cabinets of the organ world (as measured by
Herrera, Hanson & Abel, AES 127, 2009), setBfree, the rotary simulators in organ plug-ins
and pedals (ADR 0133).*

*Rota* is Latin for the wheel. Cantus's organ cabinet (`organum_rotary`) as an effect for any
track: a horn and a drum, each read at its Doppler delay, with its directivity and the
wall's reflection, heard through two microphones; the cabinet's valve amp (a 12AX7 stage,
2× oversampled) for drive.

| Switch | Horn | Drum | Ramp (×1) |
|---|---|---|---|
| **Stop** | the brake: both rotors slow to a creep and park | | |
| **Slow** (chorale) | 40.32 rpm | 36 rpm | down: horn 0.32 s, drum 1.37 s |
| **Fast** (tremolo) | 423.36 rpm | 357.3 rpm | up: horn 0.16 s, drum 4.13 s |

The ramp scales every time (0.25–4×). A note on the track throws the switch (to the
sample) when *Switched by notes* is on. **The null**: the mix starts at 0 — the signal
untouched, to the bit. No latency.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/rota.rs` (14 keys) |
| DSP | `crates/cantus-dsp/src/rota.rs` (wrapping `organum_rotary`); tests `rota_tests.rs` |
| Engine glue | `crates/cantus-engine/src/fx/rota.rs` (settings, notes, telemetry: each rotor's angle and speed, the switch) |
| Editor | `crates/cantus-ui/src/views/fx/rota.rs` (the cards), `rota_display.rs` (the cabinet cut away, from above, the ramps), `rota_art.rs` (its motif on Tabula) |
| Face | `crates/cantus-ui/src/kit/skin/side.rs` (`ROTA`: walnut, a pale steel horn and a vermilion drum; Cherry and Black colourways) |
| Strings | `crates/cantus-ui/locales/fx/rota.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/rotary.toml` (thirteen); starts on *Chorale* |
| Tests | `crates/cantus-engine/tests/fx_rota.rs`, `rt_memory`, `fx_contract`; `crates/cantus-ui/tests/rota_e2e.rs`, `arca_rota_drag.rs`, `arca_rota_whole.rs`; the CLAP/VST3 suites and `notes.rs` |
| Exported | `Cantus Rota` (CLAP and VST3), hearing notes on its note port |

## The window

- **Cut away**: the walnut cabinet with its front left at the sides (louvres, cloth), the
  horn turning in the upper chamber (blurred at speed), the motors and the amp's valves on
  the shelf, the drum with its scoop and baffle over the bass speaker — all turning from the
  engine's telemetry.
- **From above**: the horn's arm and the drum's opening passing the two microphones,
  **dragged round** (their angle) **and away** (their distance).
- **The ramps**: each rotor's speed since the switch, a dot where it is now.
- **The switch** along the top: Stop, Slow, Fast (a click; automation; MIDI learn).

## On Tabula

A wide pedal in walnut; the motif a horn rotor throwing its sound in arcs; knobs Speed,
Drive, Balance, Mix.

## Ours, better

- The published spin-up and spin-down times, each rotor landing exactly; the brake parks.
- At rest in closed form: a constant-speed rotor's angle skips silence exactly.
- Notes throw the switch to the sample, in Cantus and in the exported plug-in.
