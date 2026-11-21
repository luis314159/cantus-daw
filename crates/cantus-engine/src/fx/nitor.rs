//! Nitor, the one-knob enhancer (round 41, ADR 0151), behind the engine's plug-in contract.
//! Owned by the `nitor` builder.
//!
//! [`settings`] reads the parameters (`cantus_core::fx::nitor`) into the DSP's terms
//! (`cantus_dsp::nitor`) with the tonic resolved: the `key` parameter's, or — at 0 — the
//! song's key the engine hands every effect ([`FxUnit::set_song_key`], ADR 0153). The
//! exported plug-in has no song and so tunes to its own `key` (C when it follows the song).
//! The editor draws from the same function, so the picture and the sound come from one
//! reading of the spec.
//!
//! ## Telemetry (64 floats, refreshed after every block)
//!
//! | Slot | What |
//! |---|---|
//! | 0 | blocks processed, modulo 2²⁴ |
//! | 1, 2 | the block's input and output peaks, dBFS |
//! | 3 | the level match's gain, dB |
//! | 4 | the pre-gain into the character, dB |
//! | 5 | the tonic in force, 0 = C … 11 = B |
//! | 6 | the song's tonic, or −1 when the song has no key |
//! | 7 | the sample rate, Hz |
//! | 8 | 1 while something runs, 0 while Nitor is a wire |
//! | 9–12 | the character's four band gains now, dB |
//! | 13–15 | the exciter's level in force (body, presence, air), 0 … 1 |
//! | 16 | the sub's peak, dBFS |
//! | 17 | the low harmonics' peak, dBFS |
//! | 18 | the tightening's lowest gain in the block, dB |
//! | 19 | the sub's frequency now, Hz (0: none) |
//! | 20 | the tonic's fundamental (octave 1), Hz |

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::scale::Key;
use cantus_core::EffectSpec;
use cantus_dsp::nitor::{Character, Nitor, NitorSettings, SubMode};

pub const T_BLOCKS: usize = 0;
pub const T_IN_PEAK: usize = 1;
pub const T_OUT_PEAK: usize = 2;
pub const T_MATCH: usize = 3;
pub const T_PRE: usize = 4;
pub const T_TONIC: usize = 5;
pub const T_SONG: usize = 6;
pub const T_RATE: usize = 7;
pub const T_ACTIVE: usize = 8;
pub const T_BANDS: usize = 9;
pub const T_EXCITE: usize = 13;
pub const T_SUB: usize = 16;
pub const T_HARM: usize = 17;
pub const T_TIGHT: usize = 18;
pub const T_SUB_HZ: usize = 19;
pub const T_TONIC_HZ: usize = 20;

/// Nitor's parameters as the DSP wants them, the tonic resolved against `song` (the
/// project's tonic, `None` when it has no key). Also what its editor draws from.
pub fn settings(spec: &EffectSpec, song: Option<u8>) -> NitorSettings {
    let on = |k: &str| spec.get(k) >= 0.5;
    NitorSettings {
        amount: spec.get("amount"),
        character: Character::from_index(spec.get("char").round().max(0.0) as usize),
        sub: spec.get("sub"),
        sub_mode: SubMode::from_index(spec.get("submode").round().max(0.0) as usize),
        harm: spec.get("harm"),
        tight: spec.get("tight"),
        lift: spec.get("lift"),
        xover_hz: spec.get("xover"),
        tonic: cantus_core::fx::nitor::tonic(spec.get("key"), song),
        excite: [spec.get("body"), spec.get("pres"), spec.get("air")],
        color: spec.get("color"),
        level_match: on("match"),
        mix: spec.get("mix"),
        out_db: spec.get("out"),
    }
}

fn dbfs(x: f32) -> f32 {
    20.0 * cantus_dsp::math::log10(x.max(1e-9))
}

#[derive(Clone, Debug)]
pub struct Unit {
    dsp: Nitor,
    /// The settings as last read, and the `key` parameter they came with (0 follows the
    /// song): when the song's key changes, only the tonic is read again — no allocation.
    s: NitorSettings,
    key: f32,
    /// The song's key as the engine last handed it (`None`: the song has none).
    song: Option<Key>,
    blocks: u32,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Unit {
        Unit {
            dsp: Nitor::new(sample_rate),
            s: NitorSettings::default(),
            key: cantus_core::fx::nitor::KEY_FOLLOWS_SONG,
            song: None,
            blocks: 0,
        }
    }

    /// The tonic the low end tunes to (0 = C … 11 = B).
    pub fn tonic(&self) -> u8 {
        self.s.tonic
    }

    /// The song's key as last handed (for the tests).
    pub fn song_key(&self) -> Option<Key> {
        self.song
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.key = spec.get("key");
        self.s = settings(spec, self.song.map(|k| k.root));
        self.dsp.set(&self.s);
    }

    fn set_song_key(&mut self, key: Option<Key>) {
        self.song = key;
        let tonic = cantus_core::fx::nitor::tonic(self.key, key.map(|k| k.root));
        if tonic != self.s.tonic {
            self.s.tonic = tonic;
            self.dsp.set(&self.s);
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right);
        self.blocks = (self.blocks + 1) % (1 << 24);
    }

    fn snap(&mut self) {
        self.dsp.snap();
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn at_rest(&self) -> bool {
        self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        let m = self.dsp.meter();
        t[T_BLOCKS] = self.blocks as f32;
        t[T_IN_PEAK] = dbfs(m.in_peak);
        t[T_OUT_PEAK] = dbfs(m.out_peak);
        t[T_MATCH] = m.match_db;
        t[T_PRE] = m.pre_db;
        t[T_TONIC] = f32::from(self.tonic());
        t[T_SONG] = self.song.map(|k| f32::from(k.root % 12)).unwrap_or(-1.0);
        t[T_RATE] = self.dsp.sample_rate();
        t[T_ACTIVE] = if m.active { 1.0 } else { 0.0 };
        t[T_BANDS..T_BANDS + 4].copy_from_slice(&m.band_db);
        t[T_EXCITE..T_EXCITE + 3].copy_from_slice(&m.excite);
        t[T_SUB] = dbfs(m.sub);
        t[T_HARM] = dbfs(m.harm);
        t[T_TIGHT] = m.tight_db;
        t[T_SUB_HZ] = m.sub_hz;
        t[T_TONIC_HZ] = self.dsp.tonic_hz();
        t
    }
}
