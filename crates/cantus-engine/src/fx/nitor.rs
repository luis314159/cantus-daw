//! Nitor, the one-knob enhancer (round 41, ADR 0151): the engine's unit. Owned by the
//! `nitor` builder.
//!
//! **The contract's stub** is a wire times `out` (exactly the input at 0 dB) and keeps the
//! song's key the engine hands it ([`FxUnit::set_song_key`], ADR 0153) and the tonic it
//! resolves to (`fx::nitor::tonic`): what the builder's low end will tune to.

use super::FxUnit;
use cantus_core::scale::Key;
use cantus_core::EffectSpec;

#[derive(Clone, Debug)]
pub struct Unit {
    gain: f32,
    /// The song's key as the engine last handed it (`None`: the song has none).
    song: Option<Key>,
    /// The `key` parameter (0 follows the song; 1 … 12 fix C … B).
    key: f32,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Unit {
        let _ = sample_rate;
        Unit {
            gain: 1.0,
            song: None,
            key: cantus_core::fx::nitor::KEY_FOLLOWS_SONG,
        }
    }

    /// The tonic the low end tunes to (0 = C … 11 = B): the `key` parameter's, or the song's.
    pub fn tonic(&self) -> u8 {
        cantus_core::fx::nitor::tonic(self.key, self.song.map(|k| k.root))
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
        self.gain = cantus_dsp::math::db_to_gain(spec.get("out"));
        self.key = spec.get("key");
    }
    fn set_song_key(&mut self, key: Option<Key>) {
        self.song = key;
    }
    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        if self.gain == 1.0 {
            return;
        }
        for x in left.iter_mut().chain(right.iter_mut()) {
            *x *= self.gain;
        }
    }
    fn reset(&mut self) {}
    fn at_rest(&self) -> bool {
        true
    }
}
