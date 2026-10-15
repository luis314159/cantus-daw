//! Speculum, the analyser (Wave Candy / SPAN class, ADR 0056): its parameter model.
//! Owned by Speculum; nothing else edits this file.
//!
//! Speculum is a **pure pass-through**: no parameter touches a sample. They are how it is
//! looked at, kept with the project so a mix opens with its analysers as they were left.
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them).
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `view` | overview, spectrum, spectrogram, oscilloscope, vectorscope, loudness | 0 … 5 | overview |
//! | `fft` | FFT size 1 024 … 16 384 | 0 … 4 | 8 192 |
//! | `avg` | the spectrum's averaging time constant (0: none) | 0 … 3 000 ms | 250 ms |
//! | `slope` | the spectrum's tilt around 1 kHz | 0 … 6 dB an octave | 4.5 |
//! | `hold` | the spectrum's peak hold | 0 / 1 | on |
//! | `chan` | mid, side, left, right, left and right | 0 … 4 | mid |
//! | `floor` | the display's floor: −72, −96, −120 dB | 0 … 2 | −96 |
//! | `sync` | the scope: free, triggered, on the beat | 0 … 2 | on the beat |
//! | `beats` | the synced scope's window: 1/4, 1/2, 1, 2, 4, 8 beats | 0 … 5 | 1 beat |
//! | `ms` | the free and triggered scope's window | 1 … 500 ms | 20 ms |
//! | `target` | the loudness the editor marks | −30 … −5 LUFS | −14 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.speculum";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[
    ("view", "speculum.view"),
    ("fft", "speculum.fft"),
    ("hold", "speculum.hold"),
    ("chan", "speculum.chan"),
    ("floor", "speculum.floor"),
    ("sync", "speculum.sync"),
    ("beats", "speculum.beats"),
];

/// The synced scope's windows, beats, by the `beats` parameter's index.
pub const SCOPE_BEATS: [f64; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
/// The display's floors, dB, by the `floor` parameter's index.
pub const FLOORS: [f32; 3] = [-72.0, -96.0, -120.0];

pub const PARAMS: &[EffectParamInfo] = &[
    p("view", "fx.speculum.view", 0.0, 5.0, 0.0, "", false, true),
    p("fft", "fx.speculum.fft", 0.0, 4.0, 3.0, "", false, true),
    p("avg", "fx.speculum.avg", 0.0, 3_000.0, 250.0, "ms", false, false),
    p("slope", "fx.speculum.slope", 0.0, 6.0, 4.5, "dB", false, false),
    p("hold", "fx.speculum.hold", 0.0, 1.0, 1.0, "", false, true),
    p("chan", "fx.speculum.chan", 0.0, 4.0, 0.0, "", false, true),
    p("floor", "fx.speculum.floor", 0.0, 2.0, 1.0, "", false, true),
    p("sync", "fx.speculum.sync", 0.0, 2.0, 2.0, "", false, true),
    p("beats", "fx.speculum.beats", 0.0, 5.0, 2.0, "", false, true),
    p("ms", "fx.speculum.ms", 1.0, 500.0, 20.0, "ms", true, false),
    p("target", "fx.speculum.target", -30.0, -5.0, -14.0, "LUFS", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}
