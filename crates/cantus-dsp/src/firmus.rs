//! **Firmus** — the built-in subtractive polysynth.
//!
//! Two PolyBLEP oscillators → SVF low-pass with its own envelope → amp envelope.
//! Fixed voice pool, no allocation after construction, deterministic output.

use crate::env::Adsr;
use crate::filter::{FilterMode, Svf};
use crate::osc::{PolyBlepOsc, Wave};
use crate::smoother::Smoother;

pub const MAX_VOICES: usize = 16;

/// Sound settings, mirrored from `cantus_core::FirmusParams` by the engine so this crate
/// stays free of internal dependencies.
#[derive(Clone, Debug, PartialEq)]
pub struct FirmusSettings {
    pub osc1_wave: Wave,
    pub osc2_wave: Wave,
    pub osc2_semitones: f32,
    pub osc2_detune_cents: f32,
    pub osc_mix: f32,
    pub cutoff_hz: f32,
    pub resonance: f32,
    pub filter_env_octaves: f32,
    pub key_track: f32,
    pub amp_attack_s: f32,
    pub amp_decay_s: f32,
    pub amp_sustain: f32,
    pub amp_release_s: f32,
    pub filter_attack_s: f32,
    pub filter_decay_s: f32,
    pub filter_sustain: f32,
    pub filter_release_s: f32,
    pub gain_db: f32,
}

impl Default for FirmusSettings {
    fn default() -> Self {
        FirmusSettings {
            osc1_wave: Wave::Saw,
            osc2_wave: Wave::Saw,
            osc2_semitones: 0.0,
            osc2_detune_cents: 8.0,
            osc_mix: 0.5,
            cutoff_hz: 2_200.0,
            resonance: 0.25,
            filter_env_octaves: 2.0,
            key_track: 0.5,
            amp_attack_s: 0.005,
            amp_decay_s: 0.15,
            amp_sustain: 0.8,
            amp_release_s: 0.25,
            filter_attack_s: 0.01,
            filter_decay_s: 0.3,
            filter_sustain: 0.3,
            filter_release_s: 0.3,
            gain_db: -6.0,
        }
    }
}

#[derive(Clone, Debug)]
struct Voice {
    pitch: u8,
    /// Opaque tag from the sequencer (note id) so a note-off releases the right voice
    /// even when the same pitch is stacked.
    tag: u64,
    velocity: f32,
    osc1: PolyBlepOsc,
    osc2: PolyBlepOsc,
    amp_env: Adsr,
    filter_env: Adsr,
    filter: Svf,
    /// Monotonic start counter for voice stealing.
    age: u64,
    base_hz: f32,
}

impl Voice {
    fn new(sample_rate: f32) -> Self {
        let mut filter = Svf::new(sample_rate);
        filter.set_mode(FilterMode::Low);
        Voice {
            pitch: 0,
            tag: 0,
            velocity: 0.0,
            osc1: PolyBlepOsc::new(sample_rate),
            osc2: PolyBlepOsc::new(sample_rate),
            amp_env: Adsr::new(sample_rate),
            filter_env: Adsr::new(sample_rate),
            filter,
            age: 0,
            base_hz: 440.0,
        }
    }

    fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }
}

#[derive(Clone, Debug)]
pub struct Firmus {
    sample_rate: f32,
    settings: FirmusSettings,
    voices: Vec<Voice>,
    counter: u64,
    gain: Smoother,
    cutoff_log2: Smoother,
}

#[inline]
fn pitch_hz(pitch: f32) -> f32 {
    440.0 * 2f32.powf((pitch - 69.0) / 12.0)
}

impl Firmus {
    pub fn new(sample_rate: f32) -> Self {
        let settings = FirmusSettings::default();
        let mut s = Firmus {
            sample_rate,
            voices: (0..MAX_VOICES).map(|_| Voice::new(sample_rate)).collect(),
            counter: 0,
            gain: Smoother::new(db_to_lin(settings.gain_db), 0.02, sample_rate),
            cutoff_log2: Smoother::new(settings.cutoff_hz.log2(), 0.01, sample_rate),
            settings,
        };
        s.apply_settings();
        s
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn settings(&self) -> &FirmusSettings {
        &self.settings
    }

    pub fn set_settings(&mut self, settings: &FirmusSettings) {
        if *settings != self.settings {
            self.settings = settings.clone();
            self.apply_settings();
        }
    }

    fn apply_settings(&mut self) {
        let s = &self.settings;
        self.gain.set_target(db_to_lin(s.gain_db));
        self.cutoff_log2.set_target(s.cutoff_hz.max(10.0).log2());
        for v in &mut self.voices {
            v.osc1.set_wave(s.osc1_wave);
            v.osc2.set_wave(s.osc2_wave);
            v.amp_env.set(
                s.amp_attack_s,
                s.amp_decay_s,
                s.amp_sustain,
                s.amp_release_s,
            );
            v.filter_env.set(
                s.filter_attack_s,
                s.filter_decay_s,
                s.filter_sustain,
                s.filter_release_s,
            );
            if v.is_active() {
                Self::tune(v, s);
            }
        }
    }

    fn tune(v: &mut Voice, s: &FirmusSettings) {
        v.base_hz = pitch_hz(v.pitch as f32);
        v.osc1.set_frequency(v.base_hz);
        let p2 = v.pitch as f32 + s.osc2_semitones + s.osc2_detune_cents / 100.0;
        v.osc2.set_frequency(pitch_hz(p2));
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active()).count()
    }

    pub fn note_on(&mut self, pitch: u8, velocity: u8, tag: u64) {
        self.counter += 1;
        let idx = self.pick_voice();
        let v = &mut self.voices[idx];
        v.pitch = pitch.min(127);
        v.tag = tag;
        v.velocity = (velocity.clamp(1, 127) as f32 / 127.0).powf(1.5);
        v.age = self.counter;
        // Deterministic phase reset: identical input → identical output.
        v.osc1.reset(0.0);
        v.osc2.reset(0.0);
        v.filter.reset();
        v.amp_env.reset();
        v.filter_env.reset();
        Self::tune(v, &self.settings);
        v.amp_env.gate_on();
        v.filter_env.gate_on();
    }

    pub fn note_off(&mut self, pitch: u8, tag: u64) {
        let mut released = false;
        for v in &mut self.voices {
            if v.is_active() && !v.amp_env.is_releasing() && v.tag == tag && v.pitch == pitch {
                v.amp_env.gate_off();
                v.filter_env.gate_off();
                released = true;
            }
        }
        if !released {
            // Tag mismatch (e.g. live keyboard): release the oldest held voice of that pitch.
            if let Some(v) = self
                .voices
                .iter_mut()
                .filter(|v| v.is_active() && !v.amp_env.is_releasing() && v.pitch == pitch)
                .min_by_key(|v| v.age)
            {
                v.amp_env.gate_off();
                v.filter_env.gate_off();
            }
        }
    }

    /// Release every held note (transport stop). Tails still ring out.
    pub fn all_notes_off(&mut self) {
        for v in &mut self.voices {
            if v.is_active() {
                v.amp_env.gate_off();
                v.filter_env.gate_off();
            }
        }
    }

    /// Hard reset: silence immediately, clear all state (seek, render start).
    pub fn reset(&mut self) {
        for v in &mut self.voices {
            v.amp_env.reset();
            v.filter_env.reset();
            v.filter.reset();
            v.osc1.reset(0.0);
            v.osc2.reset(0.0);
        }
        self.gain.snap(db_to_lin(self.settings.gain_db));
        self.cutoff_log2
            .snap(self.settings.cutoff_hz.max(10.0).log2());
        self.counter = 0;
    }

    fn pick_voice(&self) -> usize {
        // 1. A free voice.
        if let Some(i) = self.voices.iter().position(|v| !v.is_active()) {
            return i;
        }
        // 2. The oldest releasing voice.
        if let Some((i, _)) = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.amp_env.is_releasing())
            .min_by_key(|(_, v)| v.age)
        {
            return i;
        }
        // 3. The oldest voice.
        self.voices
            .iter()
            .enumerate()
            .min_by_key(|(_, v)| v.age)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Render `out.len()` mono samples, **adding** into `out`. Returns `false` when the
    /// synth was idle for the whole block (nothing was added).
    pub fn process_add(&mut self, out: &mut [f32]) -> bool {
        let s = &self.settings;
        let mix2 = s.osc_mix.clamp(0.0, 1.0);
        let mix1 = 1.0 - mix2;
        let env_oct = s.filter_env_octaves;
        let key_track = s.key_track;
        let resonance = s.resonance;
        let any_active = self.voices.iter().any(|v| v.is_active());
        if !any_active {
            // Idle: settle the smoothers instantly. This keeps the output independent of
            // how the host splits idle time into blocks (null-test invariant).
            let g = db_to_lin(s.gain_db);
            let c = s.cutoff_hz.max(10.0).log2();
            self.gain.snap(g);
            self.cutoff_log2.snap(c);
            return false;
        }
        for sample in out.iter_mut() {
            let gain = self.gain.next();
            let cutoff_log2 = self.cutoff_log2.next();
            let mut acc = 0.0;
            for v in &mut self.voices {
                if !v.is_active() {
                    continue;
                }
                let amp = v.amp_env.next();
                let fenv = v.filter_env.next();
                // Cutoff in octaves: base + envelope + key tracking relative to C4.
                let key_oct = key_track * ((v.pitch as f32 - 60.0) / 12.0);
                let fc = (cutoff_log2 + env_oct * fenv + key_oct).exp2();
                v.filter.set(fc, resonance);
                let o = v.osc1.next() * mix1 + v.osc2.next() * mix2;
                let f = v.filter.process(o);
                acc += f * amp * v.velocity;
            }
            *sample += acc * gain;
        }
        true
    }
}

#[inline]
fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(f: &mut Firmus, n: usize) -> Vec<f32> {
        let mut buf = vec![0.0; n];
        for chunk in buf.chunks_mut(256) {
            f.process_add(chunk);
        }
        buf
    }

    #[test]
    fn silent_when_idle() {
        let mut f = Firmus::new(48_000.0);
        let buf = render(&mut f, 4_096);
        assert!(buf.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn note_produces_sound_and_release_decays() {
        let mut f = Firmus::new(48_000.0);
        f.note_on(60, 100, 1);
        let held = render(&mut f, 24_000);
        let peak = held.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.05 && peak < 1.0, "peak {peak}");
        f.note_off(60, 1);
        let tail = render(&mut f, 96_000);
        assert_eq!(f.active_voices(), 0);
        assert!(tail[tail.len() - 1] == 0.0);
        assert!(tail.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn deterministic() {
        let run = || {
            let mut f = Firmus::new(44_100.0);
            f.note_on(64, 90, 7);
            let mut out = render(&mut f, 10_000);
            f.note_on(67, 120, 8);
            f.note_off(64, 7);
            out.extend(render(&mut f, 10_000));
            out
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn voice_stealing_never_panics() {
        let mut f = Firmus::new(48_000.0);
        for i in 0..(MAX_VOICES as u64 * 3) {
            f.note_on((40 + i % 40) as u8, 100, i);
            render(&mut f, 64);
        }
        assert_eq!(f.active_voices(), MAX_VOICES);
        f.all_notes_off();
        render(&mut f, 96_000);
        assert_eq!(f.active_voices(), 0);
    }
}
