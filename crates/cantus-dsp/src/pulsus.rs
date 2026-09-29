//! **Pulsus** — the built-in drum instrument. Pulsus II since round 17.
//!
//! Pads are arranged by pitch, each with its own voice, tuning, level, pan and **choke
//! group**. Named like Firmus: *pulsus* is the Latin for a beat, a striking.
//!
//! ## The voices
//!
//! - **Kick**: Pulsus I's sine with a two-stage pitch fall and a noise click; an **808**
//!   (a resonator that rings down on its own, a little sweep, the *sigh* as it fades);
//!   a **909** (a saw shaped to a near-sine, a deep fast pitch fall, a click of noise and
//!   pulse).
//! - **Snare**: Pulsus I's two tuned bodies and noise with its own decay; a **modal**
//!   snare (the head's membrane modes, wires that rattle while the head moves, a tail).
//! - **Metal**: the 808's six squares through a band-pass and a high-pass (the hat); the
//!   **808 cymbal** (the same six through two bands with their own decays); the **808
//!   cowbell** (two squares, a band-pass, a two-stage decay); a **modal cymbal** (a dense
//!   inharmonic bank and a shimmer that swells before it decays).
//! - **Clap**: band-passed noise re-triggered a few times, then the room.
//! - **Tom**: a modal bank — membrane, bar, wood, plate or bell — struck at a position
//!   with a mallet of a hardness, with the tension glide of a real head.
//! - **Sample**: a file, or up to sixteen velocity-zoned, round-robin layers. Layers ride
//!   along on a synthesized pad too (a synth body with a sampled click).
//!
//! Every voice goes through one **section**: an attack–hold–decay envelope with a curve,
//! a pitch envelope, a filter that follows the envelope, 2× oversampled drive, a transient
//! shaper, velocity to level, tone, pitch and decay, and a choke with its own fade.
//!
//! ## Model 1 and model 2
//!
//! A kit carries a model ([`Pulsus::set_kit`]). **Model 1** is Pulsus I, kept bit-exact so
//! an old project sounds exactly as it did — with its two defects: noise seeded from a
//! per-pad hit count that a seek resets, and a kick based on 45 Hz, 52 cents sharp of F1.
//! **Model 2** seeds everything a hit chooses from the note (its tick, its id, the pad and
//! the kit's seed), so a render from bar 5 equals bars 5… of a render from the top, and
//! tunes the kick so Tune 0 is F1 (43.65 Hz). Model 2 also lets drive reach every voice.
//!
//! ## Choke groups are not a refinement
//!
//! An open hi-hat that a closed hi-hat does not cut is not a hi-hat. Triggering a pad
//! fades every sounding voice in its group over that voice's own fade time; a pad can
//! also be limited to a number of voices (one is a self-choke), and choked by its own
//! note-off.
//!
//! ## Determinism
//!
//! Parameters are captured at the hit, so there is no smoother whose trajectory could
//! depend on where the host split the buffer; time-varying parts (the filter's envelope,
//! the modal glide) update on a grid counted from the hit, never from a block.

use std::sync::Arc;

#[path = "pulsus_modal.rs"]
mod modal;
#[path = "pulsus_render.rs"]
mod render;
#[path = "pulsus_shape.rs"]
mod shape;
#[path = "pulsus_start.rs"]
mod start;
#[path = "pulsus_voice.rs"]
mod voice;

use shape::{half_band_taps, HALF_TAPS};
use voice::Voice;

/// Pads a kit can have. A pad is a pitch, so this is also the widest a channel rack gets.
pub const MAX_PADS: usize = 32;
/// Simultaneously sounding drums in a model 2 kit (modal voices and layers need room).
pub const MAX_DRUM_VOICES: usize = 48;
/// Pulsus I's pool, which a model 1 kit keeps so it steals exactly as it did.
pub const LEGACY_VOICES: usize = 24;
/// Sample layers per pad, preallocated.
pub const MAX_LAYERS: usize = 16;
/// Where a model 2 kick sits at Tune 0: F1.
pub const KICK_F1_HZ: f32 = 43.653_53;
/// Where a model 1 kick sat: 45 Hz, 52 cents sharp of F1.
pub const KICK_LEGACY_HZ: f32 = 45.0;

/// Which voice a pad is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DrumVoice {
    Kick,
    Snare,
    /// The metal voice: hat, 808 cymbal, 808 cowbell, modal cymbal.
    HiHat,
    Sample,
    Clap,
    Tom,
}

/// A decoded sample the kit can play: shared frames, so handing one to the audio thread
/// is an atomic increment and never a copy.
///
/// The kit holds one per pad and per layer, and every sounding voice holds a clone of the
/// one it started on, so replacing a sample mid-hit cannot pull the buffer out from under
/// it. The loader's cache (`cantus_engine::samples`) keeps one for the life of the session
/// so that an `Arc` dropped here is never the last.
#[derive(Clone, Debug)]
pub struct SampleRef {
    pub frames: Arc<[f32]>,
    pub rate: f32,
    /// 1 or 2; interleaved when 2.
    pub channels: usize,
}

impl SampleRef {
    pub fn len_frames(&self) -> usize {
        self.frames.len() / self.channels.max(1)
    }

    /// Both channels at a fractional frame position, linearly interpolated (Pulsus I,
    /// model 1). A mono sample answers the same value twice.
    #[inline]
    fn at(&self, pos: f64) -> (f32, f32) {
        let n = self.len_frames();
        if n == 0 {
            return (0.0, 0.0);
        }
        let i = pos.floor() as usize;
        let frac = (pos - i as f64) as f32;
        let j = (i + 1).min(n - 1);
        let ch = self.channels.max(1);
        let l0 = self.frames[i * ch];
        let l1 = self.frames[j * ch];
        let l = l0 + (l1 - l0) * frac;
        if ch >= 2 {
            let r0 = self.frames[i * ch + 1];
            let r1 = self.frames[j * ch + 1];
            (l, r0 + (r1 - r0) * frac)
        } else {
            (l, l)
        }
    }

    /// Both channels by 4-point, 3rd-order Hermite interpolation (model 2): a drum tuned
    /// up an octave no longer rings with the images linear interpolation leaves.
    #[inline]
    fn at_hermite(&self, pos: f64) -> (f32, f32) {
        let n = self.len_frames();
        if n == 0 {
            return (0.0, 0.0);
        }
        let i = pos.floor() as isize;
        let t = (pos - i as f64) as f32;
        let ch = self.channels.max(1);
        let get = |k: isize, c: usize| -> f32 {
            if k < 0 || k as usize >= n {
                0.0
            } else {
                self.frames[k as usize * ch + c.min(ch - 1)]
            }
        };
        let h = |c: usize| {
            let (xm1, x0, x1, x2) = (get(i - 1, c), get(i, c), get(i + 1, c), get(i + 2, c));
            let c1 = 0.5 * (x1 - xm1);
            let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
            let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
            ((c3 * t + c2) * t + c1) * t + x0
        };
        let l = h(0);
        if ch >= 2 {
            (l, h(1))
        } else {
            (l, l)
        }
    }
}

/// One sample layer of a pad, as the engine hands it over.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LayerSettings {
    pub vel_lo: u8,
    pub vel_hi: u8,
    pub xfade: u8,
    /// Semitones on top of the pad's.
    pub tune: f32,
    /// Linear.
    pub gain: f32,
    pub pan: f32,
    /// Seconds into the file.
    pub start: f32,
    pub reverse: bool,
}

impl Default for LayerSettings {
    fn default() -> Self {
        LayerSettings {
            vel_lo: 1,
            vel_hi: 127,
            xfade: 0,
            tune: 0.0,
            gain: 1.0,
            pan: 0.0,
            start: 0.0,
            reverse: false,
        }
    }
}

impl LayerSettings {
    /// Equal-power velocity gain; the same law as `cantus_core::PadLayer::velocity_gain`.
    pub fn velocity_gain(&self, velocity: f32) -> f32 {
        let lo = self.vel_lo.min(self.vel_hi) as f32 - 0.5;
        let hi = self.vel_lo.max(self.vel_hi) as f32 + 0.5;
        let w = self.xfade as f32;
        let fade = |d: f32| -> f32 {
            if w <= 0.0 {
                return if d >= 0.0 { 1.0 } else { 0.0 };
            }
            ((d / w + 0.5).clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2).sin()
        };
        let fade_in = if self.vel_lo.min(self.vel_hi) <= 1 {
            1.0
        } else {
            fade(velocity - lo)
        };
        let fade_out = if self.vel_lo.max(self.vel_hi) >= 127 {
            1.0
        } else {
            fade(hi - velocity)
        };
        fade_in * fade_out
    }
}

/// Where a hit comes from: the note's id, its tick and its ordinal (the how-many-th hit
/// of its pitch on the track, from the top of the piece). Live input has no tick and
/// counts its own ordinals.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Hit {
    pub id: u64,
    pub tick: i64,
    pub ordinal: u32,
    /// From the sequencer (reproducible), as opposed to a key pressed live.
    pub sequenced: bool,
}

/// One pad's sound. Mirrored from `cantus_core::DrumPad` by the engine
/// (`cantus_engine::pulsus_kit`) so this crate stays free of internal dependencies.
///
/// Fields a voice does not use are ignored by it. Pulsus II's fields default to the value
/// that changes no sample.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PadSettings {
    pub voice: DrumVoice,
    /// Semitones. Tunes every frequency in the voice at once.
    pub tune: f32,
    /// Main amplitude decay to −60 dB, in seconds.
    pub decay: f32,
    /// Kick: how far the pitch falls, in semitones.
    pub pitch_drop: f32,
    /// Kick: how fast it falls (to −60 dB of the drop), in seconds.
    pub pitch_decay: f32,
    /// Kick: level of the click transient, 0..=1.
    pub click: f32,
    /// Saturation. Zero is *exactly* clean. A model 1 kit applies it to the kick only.
    pub drive: f32,
    /// Snare: the noise's own decay, in seconds.
    pub noise_decay: f32,
    /// Where the voice's noise or partials sit, Hz.
    pub tone: f32,
    /// Snare: 0 is all body, 1 is all noise.
    pub mix: f32,
    /// Metal: the high-pass that follows the band-pass, Hz.
    pub hp_cutoff: f32,
    /// The user's level trim, linear. Unity is the pad's designed level.
    pub gain: f32,
    /// −1 hard left ..= 1 hard right.
    pub pan: f32,
    /// Choke group. Zero is none.
    pub choke: u8,
    // ---- Pulsus II ------------------------------------------------------------------
    /// The pad's identity in its kit (seeds, round robin, choke on note-off).
    pub id: u16,
    pub model: u8,
    /// Which output the pad plays into: 0 is the kit's own; the engine's multi-out reads
    /// the others ([`Pulsus::process_add_bus`]).
    pub bus: u8,
    pub attack: f32,
    pub hold: f32,
    pub curve: f32,
    /// Semitones and seconds.
    pub pitch_env: f32,
    pub pitch_env_time: f32,
    /// 0 off, 1 low-pass, 2 high-pass, 3 band-pass.
    pub filter: u8,
    pub cutoff: f32,
    pub resonance: f32,
    pub filter_env: f32,
    /// dB.
    pub trans_attack: f32,
    pub trans_sustain: f32,
    pub drive_mode: u8,
    /// Degrees.
    pub phase: f32,
    /// Percent, percent, semitones, percent.
    pub vel_level: f32,
    pub vel_tone: f32,
    pub vel_pitch: f32,
    pub vel_decay: f32,
    /// Seconds.
    pub choke_fade: f32,
    pub choke_on_off: bool,
    pub polyphony: u8,
    /// 0 off, 1 cycle, 2 random, 3 no-repeat.
    pub round_robin: u8,
    pub pedal: bool,
    pub openness: f32,
    pub open_decay: f32,
    pub sigh: f32,
    pub wires: f32,
    pub material: u8,
    pub modes: u8,
    pub damping: f32,
    pub position: f32,
    pub hardness: f32,
    pub glide: f32,
    pub shell: f32,
    pub band_mix: f32,
    /// Seconds.
    pub swell: f32,
    pub bursts: u8,
    /// Seconds.
    pub spacing: f32,
    pub q: f32,
    /// How many of the pad's layers are set.
    pub layer_count: u8,
}

impl PadSettings {
    pub fn kick() -> Self {
        PadSettings {
            voice: DrumVoice::Kick,
            tune: 0.0,
            decay: 0.36,
            pitch_drop: 15.5,
            pitch_decay: 0.04,
            click: 0.35,
            drive: 0.2,
            noise_decay: 0.2,
            tone: 1_800.0,
            mix: 0.5,
            hp_cutoff: 7_000.0,
            gain: 1.0,
            pan: 0.0,
            choke: 0,
            id: 0,
            model: 0,
            bus: 0,
            attack: 0.0,
            hold: 0.0,
            curve: 0.0,
            pitch_env: 0.0,
            pitch_env_time: 0.05,
            filter: 0,
            cutoff: 1_000.0,
            resonance: 0.1,
            filter_env: 0.0,
            trans_attack: 0.0,
            trans_sustain: 0.0,
            drive_mode: 0,
            phase: 0.0,
            vel_level: 100.0,
            vel_tone: 50.0,
            vel_pitch: 0.0,
            vel_decay: 0.0,
            choke_fade: 0.004,
            choke_on_off: false,
            polyphony: 0,
            round_robin: 0,
            pedal: false,
            openness: 0.0,
            open_decay: 1.2,
            sigh: 0.3,
            wires: 0.5,
            material: 0,
            modes: 12,
            damping: 0.5,
            position: 0.3,
            hardness: 0.5,
            glide: 1.0,
            shell: 0.5,
            band_mix: 0.5,
            swell: 0.04,
            bursts: 4,
            spacing: 0.01,
            q: 2.0,
            layer_count: 0,
        }
    }

    pub fn snare() -> Self {
        PadSettings {
            voice: DrumVoice::Snare,
            decay: 0.13,
            noise_decay: 0.19,
            tone: 1_900.0,
            mix: 0.55,
            ..PadSettings::kick()
        }
    }

    pub fn hat(decay: f32) -> Self {
        PadSettings {
            voice: DrumVoice::HiHat,
            decay,
            tone: 9_500.0,
            hp_cutoff: 7_000.0,
            ..PadSettings::kick()
        }
    }

    /// A voice and model at its designed numbers, for tests and the editor's previews.
    pub fn of(voice: DrumVoice, model: u8) -> Self {
        let base = match voice {
            DrumVoice::Snare => PadSettings::snare(),
            DrumVoice::HiHat => PadSettings::hat(0.06),
            _ => PadSettings::kick(),
        };
        PadSettings {
            voice,
            model,
            drive: if voice == DrumVoice::Kick { 0.2 } else { 0.0 },
            ..base
        }
    }
}

impl Default for PadSettings {
    fn default() -> Self {
        PadSettings::kick()
    }
}

/// How loud each shape is before the pad's own trim: the kit's internal balance, kept here
/// so "0 dB" means **as designed** on every pad, and headroom — the loudest pad at full
/// velocity peaks around −6 dBFS.
#[inline]
fn voice_level(voice: DrumVoice, model: u8) -> f32 {
    match (voice, model) {
        (DrumVoice::Kick, 0) => 0.50,
        (DrumVoice::Kick, 1) => 0.52,
        (DrumVoice::Kick, _) => 0.48,
        (DrumVoice::Snare, 0) => 0.40,
        (DrumVoice::Snare, _) => 0.185,
        (DrumVoice::HiHat, 0) => 0.25,
        (DrumVoice::HiHat, 1) => 0.135,
        (DrumVoice::HiHat, 2) => 0.32,
        (DrumVoice::HiHat, _) => 0.22,
        // A sample arrives at the level it was recorded at.
        (DrumVoice::Sample, _) => 1.0,
        (DrumVoice::Clap, _) => 0.48,
        (DrumVoice::Tom, _) => 0.225,
    }
}

/// The kit. Owns a fixed voice pool, a fixed pad table and a fixed layer table; allocates
/// nothing after construction.
#[derive(Clone, Debug)]
pub struct Pulsus {
    sample_rate: f32,
    /// 1 = Pulsus I (bit-exact), 2 = Pulsus II.
    version: u8,
    seed: u32,
    hat_cc: u8,
    hat_invert: bool,
    pads: [PadSettings; MAX_PADS],
    pitches: [u8; MAX_PADS],
    /// One per pad; `None` for a pad that is not a sample or whose file is not loaded.
    samples: Vec<Option<SampleRef>>,
    /// `MAX_LAYERS` per pad, pad-major.
    layers: Vec<LayerSettings>,
    layer_samples: Vec<Option<SampleRef>>,
    pad_count: usize,
    voices: Vec<Voice>,
    counter: u64,
    /// Hits per pad since the last reset: model 1's noise seed.
    hits: [u32; MAX_PADS],
    /// Live hits per pad: the ordinal of a hit that has none from the sequencer.
    live: [u32; MAX_PADS],
    /// The hi-hat pedal's openness, once one has arrived.
    pedal: Option<f32>,
    taps: [f32; HALF_TAPS],
}

impl Pulsus {
    pub fn new(sample_rate: f32) -> Self {
        Pulsus {
            sample_rate,
            version: 1,
            seed: 1,
            hat_cc: 4,
            hat_invert: false,
            pads: [PadSettings::kick(); MAX_PADS],
            pitches: [0; MAX_PADS],
            samples: vec![None; MAX_PADS],
            layers: vec![LayerSettings::default(); MAX_PADS * MAX_LAYERS],
            layer_samples: vec![None; MAX_PADS * MAX_LAYERS],
            pad_count: 0,
            voices: (0..MAX_DRUM_VOICES)
                .map(|_| Voice::new(sample_rate))
                .collect(),
            counter: 0,
            hits: [0; MAX_PADS],
            live: [0; MAX_PADS],
            pedal: None,
            taps: half_band_taps(),
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn pad_count(&self) -> usize {
        self.pad_count
    }

    /// The kit's model, seed and hi-hat pedal controller. Called once per snapshot.
    pub fn set_kit(&mut self, version: u8, seed: u32, hat_cc: u8, hat_invert: bool) {
        self.version = version.max(1);
        self.seed = seed;
        self.hat_cc = hat_cc;
        self.hat_invert = hat_invert;
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    /// How many pads the kit has. Called once per snapshot from the audio thread, which is
    /// why the table is fixed-size and written in place rather than replaced.
    pub fn set_pad_count(&mut self, n: usize) {
        self.pad_count = n.min(MAX_PADS);
    }

    pub fn set_pad(&mut self, index: usize, pitch: u8, settings: &PadSettings) {
        if index >= MAX_PADS {
            return;
        }
        self.pitches[index] = pitch;
        self.pads[index] = *settings;
    }

    /// Give a pad its sample (or take it away). An `Arc` clone in, an `Arc` drop out.
    pub fn set_sample(&mut self, index: usize, sample: Option<SampleRef>) {
        if index < MAX_PADS {
            self.samples[index] = sample;
        }
    }

    /// Set layer `layer` of pad `index`. The pad's `layer_count` says how many are used.
    pub fn set_layer(
        &mut self,
        index: usize,
        layer: usize,
        settings: &LayerSettings,
        sample: Option<SampleRef>,
    ) {
        if index < MAX_PADS && layer < MAX_LAYERS {
            let at = index * MAX_LAYERS + layer;
            self.layers[at] = *settings;
            self.layer_samples[at] = sample;
        }
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    fn pad_of(&self, pitch: u8) -> Option<usize> {
        (0..self.pad_count).find(|i| self.pitches[*i] == pitch)
    }

    fn voice_limit(&self) -> usize {
        if self.version <= 1 {
            LEGACY_VOICES
        } else {
            MAX_DRUM_VOICES
        }
    }

    /// Trigger the pad mapped to `pitch` from live input (or a caller with no note
    /// position). A pitch with no pad is silently ignored — a kit is not a keyboard.
    pub fn note_on(&mut self, pitch: u8, velocity: u8, tag: u64) {
        self.trigger(
            pitch,
            velocity,
            Hit {
                id: tag,
                ..Hit::default()
            },
        );
    }

    /// Trigger the pad mapped to `pitch` for a note at a known place in the piece.
    pub fn trigger(&mut self, pitch: u8, velocity: u8, hit: Hit) {
        let Some(pad) = self.pad_of(pitch) else {
            return;
        };
        self.counter += 1;
        let counter = self.counter;
        self.hits[pad] = self.hits[pad].wrapping_add(1);
        let legacy_hit = self.hits[pad];
        let s = self.pads[pad];
        // The choke runs before the voice is allocated, so a pad can cut its own earlier
        // hit and then take the voice it freed. Each choked voice fades over its own
        // pad's time (Pulsus I's 4 ms unless the pad says otherwise).
        if s.choke != 0 {
            for v in &mut self.voices {
                if v.active && v.choke == s.choke {
                    v.choke_now();
                }
            }
        }
        if s.polyphony > 0 {
            self.limit_polyphony(s.id, s.polyphony as usize);
        }
        let ordinal = if hit.sequenced {
            hit.ordinal
        } else {
            self.live[pad] = self.live[pad].wrapping_add(1);
            self.live[pad] - 1
        };
        let where_ = if hit.sequenced {
            hit.tick
        } else {
            counter as i64
        };
        let ctx = start::HitCtx {
            legacy: self.version <= 1,
            pad_index: pad,
            legacy_hit,
            seed: shape::hit_seed(self.seed, s.id, where_, hit.id),
            velocity,
            age: counter,
            note_id: hit.id,
            openness: if s.pedal {
                self.pedal.unwrap_or(s.openness)
            } else {
                s.openness
            },
        };
        let has_layers = s.layer_count > 0;
        // A sample pad with layers plays the layers; every other pad plays its voice, and
        // its layers ride along.
        if !(s.voice == DrumVoice::Sample && has_layers) {
            let idx = self.pick_voice();
            let sample = self.samples[pad].clone();
            self.voices[idx].start(&s, &ctx, sample, self.sample_rate);
        }
        if has_layers {
            self.start_layers(pad, &s, &ctx, ordinal);
        }
    }

    /// Voices a pad already holds beyond its polyphony fade out, oldest first.
    fn limit_polyphony(&mut self, id: u16, poly: usize) {
        loop {
            let mut count = 0;
            let mut oldest: Option<usize> = None;
            for (i, v) in self.voices.iter().enumerate() {
                if v.active && v.primary && !v.choking && v.pad_id == id {
                    count += 1;
                    if oldest.is_none_or(|o| self.voices[o].age > v.age) {
                        oldest = Some(i);
                    }
                }
            }
            match oldest {
                Some(o) if count >= poly => {
                    let note = self.voices[o].note_id;
                    // A hit's layers go with it.
                    for v in &mut self.voices {
                        if v.active && v.pad_id == id && v.note_id == note && !v.choking {
                            v.choke_now();
                        }
                    }
                }
                _ => return,
            }
        }
    }

    fn start_layers(&mut self, pad: usize, s: &PadSettings, ctx: &start::HitCtx, ordinal: u32) {
        let n = (s.layer_count as usize).min(MAX_LAYERS);
        let base = pad * MAX_LAYERS;
        let vel = ctx.velocity.clamp(1, 127) as f32;
        let mut first = true;
        for k in 0..n {
            let zone = self.layers[base + k];
            let key = (zone.vel_lo, zone.vel_hi);
            // Each zone once, at its first layer.
            if (0..k).any(|j| {
                let o = self.layers[base + j];
                (o.vel_lo, o.vel_hi) == key
            }) {
                continue;
            }
            let g = zone.velocity_gain(vel);
            if g <= 0.0 {
                continue;
            }
            let members = (k..n).filter(|j| {
                let o = self.layers[base + j];
                (o.vel_lo, o.vel_hi) == key
            });
            let count = members.clone().count();
            let pick = if s.round_robin == 0 {
                None
            } else {
                let seed = shape::mix64(
                    ((self.seed as u64) << 24) ^ ((s.id as u64) << 8) ^ zone.vel_lo as u64,
                );
                Some(shape::round_robin(s.round_robin, count, ordinal, seed))
            };
            for (m, j) in members.enumerate() {
                if pick.is_some_and(|p| p != m) {
                    continue;
                }
                let Some(sample) = self.layer_samples[base + j].clone() else {
                    continue;
                };
                let layer = self.layers[base + j];
                let idx = self.pick_voice();
                let primary = first && s.voice == DrumVoice::Sample;
                self.voices[idx].start_layer(s, ctx, &layer, g, sample, primary, self.sample_rate);
                first = false;
            }
        }
    }

    /// A note-off chokes the pad's hit when the pad asks for it (a held pedal, a cymbal
    /// grabbed). Otherwise a drum has no note-off worth aiming at.
    pub fn note_off(&mut self, pitch: u8, tag: u64) {
        let Some(pad) = self.pad_of(pitch) else {
            return;
        };
        let s = self.pads[pad];
        if !s.choke_on_off {
            return;
        }
        for v in &mut self.voices {
            if v.active && v.pad_id == s.id && v.note_id == tag && !v.choking {
                v.choke_now();
            }
        }
    }

    /// A controller from live MIDI, value 0…1. The kit's hi-hat pedal (CC4 unless the kit
    /// says otherwise) opens and closes every metal pad that follows it, continuously; a
    /// pedal closing on a ringing hat chokes it with the foot's chick.
    pub fn controller(&mut self, cc: u8, value: f32) {
        if cc != self.hat_cc || !value.is_finite() {
            return;
        }
        let v = value.clamp(0.0, 1.0);
        let open = if self.hat_invert { 1.0 - v } else { v };
        let before = self.pedal.replace(open);
        let closing = before.is_some_and(|b| b >= 0.35) && open < 0.1;
        let mut chick = [false; MAX_PADS];
        for voice in &mut self.voices {
            if !voice.active || !voice.pedal || voice.choking {
                continue;
            }
            if closing {
                voice.choke_now();
                if let Some(i) = (0..self.pad_count).find(|i| self.pads[*i].id == voice.pad_id) {
                    chick[i] = true;
                }
            } else {
                voice.set_openness(open);
            }
        }
        if closing {
            let speed = before.unwrap_or(1.0) - open;
            let velocity = (40.0 + 70.0 * speed).clamp(1.0, 127.0) as u8;
            for (i, hit) in chick.iter().enumerate().take(self.pad_count) {
                if *hit {
                    self.chick(i, velocity);
                }
            }
        }
    }

    /// The foot closing the hats: a short, closed hit of the pad.
    fn chick(&mut self, pad: usize, velocity: u8) {
        self.counter += 1;
        let mut s = self.pads[pad];
        s.openness = 0.0;
        s.pedal = false;
        s.decay = s.decay.min(0.035);
        s.choke = 0;
        s.layer_count = 0;
        let ctx = start::HitCtx {
            legacy: self.version <= 1,
            pad_index: pad,
            legacy_hit: 0,
            seed: shape::hit_seed(self.seed, s.id, self.counter as i64, u64::MAX - 1),
            velocity,
            age: self.counter,
            note_id: u64::MAX - 1,
            openness: 0.0,
        };
        let idx = self.pick_voice();
        self.voices[idx].start(&s, &ctx, None, self.sample_rate);
        // The chick is itself in the pad's group, so the next open hit cuts it.
        self.voices[idx].choke = self.pads[pad].choke;
    }

    /// Transport stop. One-shots have no gate to release, so their tails ring out.
    pub fn all_notes_off(&mut self) {}

    /// Hard reset: silence immediately and clear every voice (seek, render start).
    pub fn reset(&mut self) {
        for v in &mut self.voices {
            v.silence();
        }
        self.counter = 0;
        self.hits = [0; MAX_PADS];
        self.live = [0; MAX_PADS];
    }

    fn pick_voice(&self) -> usize {
        let pool = &self.voices[..self.voice_limit()];
        if let Some(i) = pool.iter().position(|v| !v.active) {
            return i;
        }
        // Steal the quietest voice rather than the oldest: a kick's tail is worth more
        // than a hat that has all but finished, whichever of the two started first.
        pool.iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.amp
                    .partial_cmp(&b.amp)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.age.cmp(&b.age))
            })
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Render into `left`/`right`, **adding**. Returns `false` when nothing was added.
    ///
    /// Stereo, because a pad carries its own pan: a kit is a dozen sources on one track,
    /// and placing them relative to each other is part of the instrument.
    pub fn process_add(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.process_add_bus(None, left, right)
    }

    /// Render only the voices of pads on output `bus` (`None`: every voice). The hook for
    /// per-pad outputs: the engine renders bus 0 into the kit's strip and each other bus
    /// into the strip the pad names, each voice exactly once per segment.
    pub fn process_add_bus(
        &mut self,
        bus: Option<u8>,
        left: &mut [f32],
        right: &mut [f32],
    ) -> bool {
        let n = left.len().min(right.len());
        if n == 0
            || !self
                .voices
                .iter()
                .any(|v| v.active && bus.is_none_or(|b| v.bus == b))
        {
            return false;
        }
        let taps = &self.taps;
        for v in &mut self.voices {
            if !v.active || bus.is_some_and(|b| v.bus != b) {
                continue;
            }
            for (l, r) in left[..n].iter_mut().zip(right[..n].iter_mut()) {
                let Some((a, b)) = v.render(taps) else {
                    break;
                };
                *l += a * v.gain_l;
                *r += b * v.gain_r;
                if !v.advance() {
                    break;
                }
            }
        }
        true
    }
}

#[cfg(test)]
#[path = "pulsus_legacy_tests.rs"]
mod legacy_tests;
#[cfg(test)]
#[path = "pulsus_tests.rs"]
mod tests;
