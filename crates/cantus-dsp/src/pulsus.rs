//! **Pulsus** — the built-in synthesized drum kit.
//!
//! Three voices — kick, snare and hi-hat — arranged as *pads*: one pad per pitch, each
//! with its own tuning, level, pan and **choke group**. Named like Firmus: *pulsus* is
//! the Latin for a beat, a striking.
//!
//! Synthesized rather than sampled, and that is a decision rather than a shortcut. ADR
//! 0001 requires bundled samples to be original or CC0, so every sampled kit is a
//! licensing question before it is a musical one; a synthesized drum is *tunable*, which
//! is the difference between a kit and a folder of wavs; and it costs kilobytes.
//!
//! ## What makes each voice sound like the thing it is
//!
//! - **Kick**: a sine whose pitch falls fast from about 110 Hz to about 45 Hz, plus a
//!   short noise click, through a soft saturator. The pitch envelope *is* the kick — a
//!   sine at a fixed 45 Hz is a test tone.
//! - **Snare**: two tuned bodies about a fifth apart, plus band-passed noise, with the
//!   noise decaying **separately** from the bodies. One shared envelope gives a tom.
//! - **Hi-hat**: the 808's method — six detuned squares through a band-pass and then a
//!   high-pass. Six inharmonic partials are what make a hat metallic; filtered noise on
//!   its own is only bright.
//!
//! ## Choke groups are not a refinement
//!
//! An open hi-hat that a closed hi-hat does not cut is not a hi-hat, it is two unrelated
//! noises. A pad names a choke group; triggering it silences every sounding voice in the
//! same group — over a few milliseconds rather than instantly, because a hard cut is a
//! click. It is a handful of lines and it is the difference between a kit and a list of
//! sounds.
//!
//! ## Everything here is one-shot
//!
//! A drum has no note-off worth aiming at, so [`Pulsus::note_off`] does nothing and a
//! voice ends when its own envelope does. Parameters are captured at the note-on, which
//! is also what makes the output block-size independent: there is no smoother whose
//! trajectory could depend on where the host chose to split the buffer.

use crate::filter::{FilterMode, Svf};
use crate::osc::{PolyBlepOsc, Wave};
use std::sync::Arc;

/// Pads a kit can have. A pad is a pitch, so this is also the widest a channel rack gets.
pub const MAX_PADS: usize = 32;
/// Simultaneously sounding drums. Generous: a busy bar of hats over a kick and a snare
/// with tails is perhaps six.
pub const MAX_DRUM_VOICES: usize = 24;

/// The six squares of the 808's hi-hat, as ratios of the pad's base frequency.
///
/// The absolute frequencies in the original are 205.3, 304.4, 369.6, 522.7, 540 and
/// 800 Hz; storing them as ratios is what lets the pad be tuned as one instrument.
const HAT_RATIOS: [f32; 6] = [1.0, 1.4827, 1.8003, 2.5461, 2.6303, 3.8967];
/// The first of those, and therefore the hi-hat pad's untuned base.
const HAT_BASE_HZ: f32 = 205.3;
/// Where a kick's pitch envelope lands, before tuning.
const KICK_BASE_HZ: f32 = 45.0;
/// The lower of the snare's two bodies, before tuning.
const SNARE_BASE_HZ: f32 = 180.0;
/// The upper body as a ratio of the lower — about a fifth and a bit, which is what stops
/// the two reading as one note.
const SNARE_BODY_RATIO: f32 = 1.8333;
/// How loud each shape is before the pad's own trim.
///
/// Two things at once. The *ratios* are the kit's internal balance, kept here rather than
/// in each pad's stored level so that "0 dB" means **as designed** on every pad. The
/// overall scale is headroom: a kit is a dozen sources landing on one channel, so the
/// loudest pad at full velocity peaks around −6 dBFS — the same place Firmus's default
/// gain puts it — and a busy bar does not arrive at the fader already clipped.
#[inline]
fn voice_level(voice: DrumVoice) -> f32 {
    match voice {
        DrumVoice::Kick => 0.50,
        DrumVoice::Snare => 0.40,
        DrumVoice::HiHat => 0.25,
        // A sample arrives at the level it was recorded at; the pad's own trim is the
        // only thing that should move it.
        DrumVoice::Sample => 1.0,
    }
}

/// Below this an envelope is inaudible and the voice is freed. −80 dB.
const ENV_FLOOR: f32 = 1.0e-4;
/// How long a choked voice takes to fall silent. Long enough not to click, short enough
/// that a closed hat reads as having *stopped* the open one.
const CHOKE_SECONDS: f32 = 0.004;

/// Which of the three shapes a pad is — or a sample.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DrumVoice {
    Kick,
    Snare,
    HiHat,
    Sample,
}

/// A decoded sample the kit can play: shared frames, so handing one to the audio thread
/// is an atomic increment and never a copy.
///
/// The kit holds one per pad and every sounding voice holds a clone of the one it started
/// on, so replacing a pad's sample mid-hit cannot pull the buffer out from under it. An
/// `Arc` drop on the audio thread is an atomic decrement unless it is the last reference,
/// and the loader's cache (`cantus_engine::samples`) keeps one for the life of the session
/// precisely so that it never is.
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

    /// Both channels at a fractional frame position, linearly interpolated. A mono sample
    /// answers the same value twice.
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
}

/// One pad's sound. Mirrored from `cantus_core::DrumPad` by the engine so this crate stays
/// free of internal dependencies — the same arrangement Firmus has.
///
/// Fields a voice does not use are ignored by it; `cantus_core::PadField::applies_to` is
/// what decides which ones a pad shows in the inspector.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PadSettings {
    pub voice: DrumVoice,
    /// Semitones. Tunes every frequency in the voice at once, so a pad can be moved
    /// without its parts drifting apart.
    pub tune: f32,
    /// Main amplitude decay to −60 dB, in seconds.
    pub decay: f32,
    /// Kick: how far the pitch falls, in semitones.
    pub pitch_drop: f32,
    /// Kick: how fast it falls (to −60 dB of the drop), in seconds.
    pub pitch_decay: f32,
    /// Kick: level of the click transient, 0..=1.
    pub click: f32,
    /// Kick: soft saturation. Zero is *exactly* clean — the sample is not touched.
    pub drive: f32,
    /// Snare: the noise's own decay, in seconds.
    pub noise_decay: f32,
    /// Snare: the noise band-pass centre. Hi-hat: the band-pass centre. Hz.
    pub tone: f32,
    /// Snare: 0 is all body, 1 is all noise.
    pub mix: f32,
    /// Hi-hat: the high-pass that follows the band-pass, Hz.
    pub hp_cutoff: f32,
    /// The user's level trim, linear. **Unity is the pad's designed level**: the balance
    /// between a kick, a snare and a hat is [`voice_level`], inside the voice, so that a
    /// pad at 0 dB is a pad the kit's designer balanced rather than a raw oscillator.
    pub gain: f32,
    /// −1 hard left ..= 1 hard right.
    pub pan: f32,
    /// Choke group. Zero is none; triggering a pad silences sounding voices that name the
    /// same group, its own earlier hits included.
    pub choke: u8,
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
}

impl Default for PadSettings {
    fn default() -> Self {
        PadSettings::kick()
    }
}

/// Deterministic white noise. Seeded from the pad and **that pad's own** hit counter, so a
/// render is reproducible sample for sample, two hats in a row are not phase-identical,
/// and — the part a global counter got wrong — adding a kick never changes how the next
/// hat sounds. A test that subtracts a hats-only render from a kick-and-hats render is
/// what caught it.
#[derive(Copy, Clone, Debug)]
struct Noise(u32);

impl Noise {
    fn seeded(pad: usize, counter: u64) -> Noise {
        // Any non-zero state; xorshift is stuck at zero.
        let mixed = (pad as u32).wrapping_mul(2_654_435_761)
            ^ (counter as u32).wrapping_mul(40_503)
            ^ 0x9E37_79B9;
        Noise(mixed | 1)
    }

    #[inline]
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32) * (2.0 / u32::MAX as f32) - 1.0
    }
}

/// Decay coefficient reaching −60 dB after `seconds`.
#[inline]
fn decay_coef(seconds: f32, sample_rate: f32) -> f32 {
    let n = (seconds.max(0.0005) * sample_rate).max(1.0);
    (-6.907_755_3 / n).exp()
}

/// Soft saturation, normalised so a full-scale input stays full-scale.
///
/// Peak-preserving normalisation means the *body* comes up rather than the top coming
/// down, which is what a driven kick actually does — and it is why the knob is kept
/// modest. At `k = 1 + 3·drive` the default 0.2 lifts the tail by about 4 dB and the
/// maximum by about 12; a `7·drive` version of this was 10 dB at the default, which is
/// not a character control, it is a hidden gain stage.
///
/// `drive == 0` returns the sample **untouched** rather than computing a `tanh` that is
/// only approximately the identity: a control at its default must not change a sample.
#[inline]
fn saturate(x: f32, drive: f32) -> f32 {
    if drive <= 0.0 {
        return x;
    }
    let k = 1.0 + drive * 3.0;
    (k * x).tanh() / k.tanh()
}

#[inline]
fn semitones(hz: f32, st: f32) -> f32 {
    hz * 2f32.powf(st.clamp(-48.0, 48.0) / 12.0)
}

/// Equal-power taper for a pad's pan, unity at the centre.
///
/// Deliberately **not** the project's pan law. A law compensates for placing a mono source
/// in a stereo field on a channel; a pad is one source inside an instrument that is itself
/// about to be placed on a channel, so applying the law here as well would apply it twice
/// — which is the mistake ADR 0012's master-pan note is about. Centre returns exactly
/// `(1, 1)`, so a pad that has never been panned does not touch a sample.
#[inline]
fn pan_gains(pan: f32) -> (f32, f32) {
    let p = if pan.is_finite() {
        pan.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    if p == 0.0 {
        return (1.0, 1.0);
    }
    let angle = (p + 1.0) * 0.25 * std::f32::consts::PI;
    (
        (angle.cos() * std::f32::consts::SQRT_2).min(1.0),
        (angle.sin() * std::f32::consts::SQRT_2).min(1.0),
    )
}

#[derive(Clone, Debug)]
struct Voice {
    active: bool,
    kind: DrumVoice,
    choke: u8,
    /// Monotonic start counter, for stealing.
    age: u64,
    gain_l: f32,
    gain_r: f32,
    /// Main amplitude envelope and its per-sample coefficient.
    amp: f32,
    amp_coef: f32,
    /// Second envelope: the kick's click, the snare's noise. Unused by the hat.
    aux: f32,
    aux_coef: f32,
    /// Pitch envelope, 1 → 0: the kick's fall, the snare's slight drop at the hit.
    pitch_env: f32,
    pitch_coef: f32,
    /// A second, much faster pitch stage: the beater's knock on a kick, gone in ~8 ms. Two
    /// stages are what separate a kick from a sine sweep — one fast for the transient, one
    /// slow for the body.
    knock: f32,
    knock_coef: f32,
    /// Sine phases in turns: the kick's body, and the snare's two.
    phase_a: f32,
    inc_a: f32,
    phase_b: f32,
    inc_b: f32,
    /// Kick: how far above the base the pitch starts, as a frequency ratio minus one.
    pitch_span: f32,
    base_hz: f32,
    drive: f32,
    mix: f32,
    hats: [PolyBlepOsc; 6],
    band: Svf,
    high: Svf,
    noise: Noise,
    /// Set while a choke is fading the voice out.
    choking: bool,
    choke_coef: f32,
    /// The sample this voice is playing, if it is a sample voice, and where it is in it.
    sample: Option<SampleRef>,
    pos: f64,
    inc: f64,
}

impl Voice {
    fn new(sample_rate: f32) -> Self {
        let mut band = Svf::new(sample_rate);
        band.set_mode(FilterMode::Band);
        let mut high = Svf::new(sample_rate);
        high.set_mode(FilterMode::High);
        let mut hats = [
            PolyBlepOsc::new(sample_rate),
            PolyBlepOsc::new(sample_rate),
            PolyBlepOsc::new(sample_rate),
            PolyBlepOsc::new(sample_rate),
            PolyBlepOsc::new(sample_rate),
            PolyBlepOsc::new(sample_rate),
        ];
        for h in &mut hats {
            h.set_wave(Wave::Square);
        }
        Voice {
            active: false,
            kind: DrumVoice::Kick,
            choke: 0,
            age: 0,
            gain_l: 0.0,
            gain_r: 0.0,
            amp: 0.0,
            amp_coef: 0.0,
            aux: 0.0,
            aux_coef: 0.0,
            pitch_env: 0.0,
            pitch_coef: 0.0,
            knock: 0.0,
            knock_coef: 0.0,
            phase_a: 0.0,
            inc_a: 0.0,
            phase_b: 0.0,
            inc_b: 0.0,
            pitch_span: 0.0,
            base_hz: 0.0,
            drive: 0.0,
            mix: 0.5,
            hats,
            band,
            high,
            noise: Noise(1),
            choking: false,
            choke_coef: 0.0,
            sample: None,
            pos: 0.0,
            inc: 0.0,
        }
    }

    fn silence(&mut self) {
        self.active = false;
        self.amp = 0.0;
        self.aux = 0.0;
        self.choking = false;
        self.band.reset();
        self.high.reset();
        // Let go of the buffer, so a replaced sample is freed by whoever holds the last
        // reference — which is never this thread (see `SampleRef`).
        self.sample = None;
    }
}

/// The kit. Owns a fixed voice pool and a fixed pad table; allocates nothing after
/// construction.
#[derive(Clone, Debug)]
pub struct Pulsus {
    sample_rate: f32,
    pads: [PadSettings; MAX_PADS],
    pitches: [u8; MAX_PADS],
    /// One per pad; `None` for a pad that is not a sample or whose file is not loaded.
    samples: Vec<Option<SampleRef>>,
    pad_count: usize,
    voices: Vec<Voice>,
    counter: u64,
    /// Hits per pad since the last reset; the noise seed.
    hits: [u32; MAX_PADS],
}

impl Pulsus {
    pub fn new(sample_rate: f32) -> Self {
        Pulsus {
            sample_rate,
            pads: [PadSettings::kick(); MAX_PADS],
            pitches: [0; MAX_PADS],
            samples: vec![None; MAX_PADS],
            pad_count: 0,
            voices: (0..MAX_DRUM_VOICES)
                .map(|_| Voice::new(sample_rate))
                .collect(),
            counter: 0,
            hits: [0; MAX_PADS],
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn pad_count(&self) -> usize {
        self.pad_count
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

    /// Give a pad its sample (or take it away). An `Arc` clone in, an `Arc` drop out;
    /// nothing is copied and nothing is freed here.
    pub fn set_sample(&mut self, index: usize, sample: Option<SampleRef>) {
        if index < MAX_PADS {
            self.samples[index] = sample;
        }
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    fn pad_of(&self, pitch: u8) -> Option<usize> {
        (0..self.pad_count).find(|i| self.pitches[*i] == pitch)
    }

    /// Trigger the pad mapped to `pitch`. A pitch with no pad is silently ignored — a kit
    /// is not a keyboard, and a note left over from a melodic instrument must not make a
    /// noise of its own choosing.
    pub fn note_on(&mut self, pitch: u8, velocity: u8, _tag: u64) {
        let Some(pad) = self.pad_of(pitch) else {
            return;
        };
        self.counter += 1;
        let counter = self.counter;
        self.hits[pad] = self.hits[pad].wrapping_add(1);
        let hit = self.hits[pad];
        let s = self.pads[pad];
        // The choke runs before the voice is allocated, so a pad can cut its own earlier
        // hit and then take the voice it freed.
        if s.choke != 0 {
            let coef = decay_coef(CHOKE_SECONDS, self.sample_rate);
            for v in &mut self.voices {
                if v.active && v.choke == s.choke {
                    v.choking = true;
                    v.choke_coef = coef;
                }
            }
        }
        let idx = self.pick_voice();
        let sr = self.sample_rate;
        let vel = (velocity.clamp(1, 127) as f32 / 127.0).powf(1.5);
        let (gl, gr) = pan_gains(s.pan);
        let v = &mut self.voices[idx];
        v.active = true;
        v.kind = s.voice;
        v.choke = s.choke;
        v.age = counter;
        let level = s.gain * voice_level(s.voice) * vel;
        v.gain_l = level * gl;
        v.gain_r = level * gr;
        v.amp = 1.0;
        v.amp_coef = decay_coef(s.decay, sr);
        v.pitch_env = 1.0;
        v.pitch_coef = decay_coef(s.pitch_decay, sr);
        v.knock = 0.0;
        v.knock_coef = decay_coef(0.008, sr);
        // Harder hits are brighter as well as louder, the way a drum is: the snare's noise
        // and the hat's tick scale with velocity; the level already does.
        let bright = 0.75 + 0.5 * vel;
        v.drive = s.drive.clamp(0.0, 1.0);
        v.mix = s.mix.clamp(0.0, 1.0);
        v.phase_a = 0.0;
        v.phase_b = 0.0;
        v.choking = false;
        v.noise = Noise::seeded(pad, hit as u64);
        v.band.reset();
        v.high.reset();
        v.sample = None;
        match s.voice {
            DrumVoice::Sample => {
                // No file, no sound: a missing sample is silence, not a click and not a
                // fallback to a synthesized voice nobody asked for.
                let Some(sample) = self.samples[pad].clone() else {
                    v.active = false;
                    return;
                };
                if sample.len_frames() == 0 {
                    v.active = false;
                    return;
                }
                // Play at the file's own rate, retuned by the pad: a 44.1 kHz sample on a
                // 48 kHz engine advances 0.919 frames per output sample.
                v.inc = (sample.rate / sr) as f64
                    * 2f64.powf((s.tune.clamp(-48.0, 48.0) / 12.0) as f64);
                v.pos = 0.0;
                v.sample = Some(sample);
            }
            DrumVoice::Kick => {
                let base = semitones(KICK_BASE_HZ, s.tune);
                v.base_hz = base;
                v.pitch_span = 2f32.powf(s.pitch_drop.clamp(0.0, 48.0) / 12.0) - 1.0;
                v.inc_a = base / sr;
                // The knock: three extra octaves of pitch for the first few milliseconds,
                // scaled by the click control, which is what a beater hitting a head does
                // before the shell starts to ring.
                v.knock = 2.5 * s.click.clamp(0.0, 1.0);
                // The click is a few milliseconds of noise, high-passed so it is a snap
                // and not a second thump on top of the first.
                v.aux = s.click.clamp(0.0, 1.0) * bright;
                v.aux_coef = decay_coef(0.004, sr);
                v.high.set(s.tone.clamp(200.0, 16_000.0), 0.0);
            }
            DrumVoice::Snare => {
                let base = semitones(SNARE_BASE_HZ, s.tune);
                v.base_hz = base;
                v.inc_a = base / sr;
                v.inc_b = base * SNARE_BODY_RATIO / sr;
                // The head's pitch drops a little at the hit — a quarter of an octave over
                // 15 ms — which is the difference between a drum and two organ notes.
                v.pitch_span = 0.35;
                v.pitch_coef = decay_coef(0.015, sr);
                v.aux = 1.0;
                v.aux_coef = decay_coef(s.noise_decay, sr);
                v.mix = (s.mix * bright).clamp(0.0, 1.0);
                // Two noise paths: the band-pass at `tone` is the wires' body, and a
                // high-pass an octave below it is the crack. One band alone is a paper bag.
                v.band.set(s.tone.clamp(200.0, 16_000.0), 0.2);
                v.high.set((s.tone * 0.5).clamp(200.0, 16_000.0), 0.0);
            }
            DrumVoice::HiHat => {
                let base = semitones(HAT_BASE_HZ, s.tune);
                v.base_hz = base;
                for (h, ratio) in v.hats.iter_mut().zip(HAT_RATIOS) {
                    h.set_frequency(base * ratio);
                    h.reset(0.0);
                }
                // The stick: two milliseconds of noise on top of the metal, so the hat has
                // an attack rather than just switching on.
                v.aux = 0.5 * bright;
                v.aux_coef = decay_coef(0.002, sr);
                v.band.set(s.tone.clamp(200.0, 18_000.0), 0.5);
                v.high.set(s.hp_cutoff.clamp(200.0, 18_000.0), 0.0);
            }
        }
    }

    /// A drum has no note-off worth aiming at: a voice ends when its envelope does.
    pub fn note_off(&mut self, _pitch: u8, _tag: u64) {}

    /// Transport stop. One-shots have no gate to release, so their tails ring out — which
    /// is what [`crate::Firmus::all_notes_off`] does for a held note too.
    pub fn all_notes_off(&mut self) {}

    /// Hard reset: silence immediately and clear every voice (seek, render start).
    pub fn reset(&mut self) {
        for v in &mut self.voices {
            v.silence();
        }
        self.counter = 0;
        self.hits = [0; MAX_PADS];
    }

    fn pick_voice(&self) -> usize {
        if let Some(i) = self.voices.iter().position(|v| !v.active) {
            return i;
        }
        // Steal the quietest voice rather than the oldest: a kick's tail is worth more
        // than a hat that has all but finished, whichever of the two started first.
        self.voices
            .iter()
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
    /// and placing them relative to each other is part of the instrument rather than of
    /// the channel it sits on.
    pub fn process_add(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let n = left.len().min(right.len());
        if n == 0 || !self.voices.iter().any(|v| v.active) {
            return false;
        }
        let sr = self.sample_rate;
        for v in &mut self.voices {
            if !v.active {
                continue;
            }
            for (l, r) in left[..n].iter_mut().zip(right[..n].iter_mut()) {
                let x = match v.kind {
                    DrumVoice::Sample => {
                        let Some(sample) = v.sample.as_ref() else {
                            v.silence();
                            break;
                        };
                        if v.pos >= (sample.len_frames() - 1) as f64 {
                            v.silence();
                            break;
                        }
                        let (sl, sr_) = sample.at(v.pos);
                        v.pos += v.inc;
                        // Samples are the one voice that is already stereo, so they take
                        // the per-channel path below and skip the mono `x`.
                        *l += sl * v.amp * v.gain_l;
                        *r += sr_ * v.amp * v.gain_r;
                        v.amp *= v.amp_coef;
                        if v.choking {
                            v.amp *= v.choke_coef;
                        }
                        if v.amp < ENV_FLOOR {
                            v.silence();
                            break;
                        }
                        continue;
                    }
                    DrumVoice::Kick => {
                        let hz = v.base_hz * (1.0 + v.pitch_span * v.pitch_env + v.knock);
                        v.knock *= v.knock_coef;
                        v.inc_a = hz / sr;
                        v.phase_a += v.inc_a;
                        if v.phase_a >= 1.0 {
                            v.phase_a -= 1.0;
                        }
                        v.pitch_env *= v.pitch_coef;
                        let body = (std::f32::consts::TAU * v.phase_a).sin() * v.amp;
                        let click = v.high.process(v.noise.next()) * v.aux;
                        v.aux *= v.aux_coef;
                        saturate(body + click, v.drive)
                    }
                    DrumVoice::Snare => {
                        let bend = 1.0 + v.pitch_span * v.pitch_env;
                        v.pitch_env *= v.pitch_coef;
                        v.phase_a += v.inc_a * bend;
                        if v.phase_a >= 1.0 {
                            v.phase_a -= 1.0;
                        }
                        v.phase_b += v.inc_b * bend;
                        if v.phase_b >= 1.0 {
                            v.phase_b -= 1.0;
                        }
                        let body = ((std::f32::consts::TAU * v.phase_a).sin() * 0.6
                            + (std::f32::consts::TAU * v.phase_b).sin() * 0.4)
                            * v.amp;
                        // The noise has its own envelope, which is what makes a snare a
                        // snare: the bodies stop and the wires keep going.
                        let white = v.noise.next();
                        let wires = v.band.process(white) * 1.7;
                        let crack = v.high.process(white) * 0.35;
                        let noise = (wires + crack) * v.aux;
                        v.aux *= v.aux_coef;
                        body * (1.0 - v.mix) + noise * v.mix
                    }
                    DrumVoice::HiHat => {
                        let mut sum = 0.0;
                        for h in &mut v.hats {
                            sum += h.next();
                        }
                        // Band-pass then high-pass: the 808's order, and the reason a hat
                        // is metallic rather than merely bright. The stick's noise joins
                        // before the high-pass so it sits in the same place as the metal.
                        let tick = v.noise.next() * v.aux;
                        v.aux *= v.aux_coef;
                        let bp = v.band.process(sum * (1.0 / 6.0));
                        v.high.process(bp + tick * 0.3) * v.amp * 3.0
                    }
                };
                *l += x * v.gain_l;
                *r += x * v.gain_r;
                v.amp *= v.amp_coef;
                if v.choking {
                    v.amp *= v.choke_coef;
                    v.aux *= v.choke_coef;
                }
                if v.amp < ENV_FLOOR {
                    v.silence();
                    break;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nulltest::{assert_null, peak, rms};

    const KICK: u8 = 36;
    const SNARE: u8 = 38;
    const CLOSED: u8 = 42;
    const OPEN: u8 = 46;

    /// A kit laid out on the General MIDI pitches, with the hats sharing a choke group.
    fn kit(sample_rate: f32) -> Pulsus {
        let mut k = Pulsus::new(sample_rate);
        let mut closed = PadSettings::hat(0.06);
        closed.choke = 1;
        let mut open = PadSettings::hat(0.45);
        open.choke = 1;
        k.set_pad_count(4);
        k.set_pad(0, KICK, &PadSettings::kick());
        k.set_pad(1, SNARE, &PadSettings::snare());
        k.set_pad(2, CLOSED, &closed);
        k.set_pad(3, OPEN, &open);
        k
    }

    /// Render `n` frames in `block`-sized chunks, hitting `pattern` at the given frame.
    fn render(k: &mut Pulsus, n: usize, block: usize, pattern: &[(usize, u8, u8)]) -> Vec<f32> {
        let mut l = vec![0.0; n];
        let mut r = vec![0.0; n];
        let mut done = 0;
        while done < n {
            // A hit that falls inside this block splits it, exactly as the processor's
            // event loop does — which is what makes the block-size test mean anything.
            let mut end = (done + block).min(n);
            for (at, pitch, vel) in pattern {
                if *at == done {
                    k.note_on(*pitch, *vel, 0);
                } else if *at > done && *at < end {
                    end = *at;
                }
            }
            k.process_add(&mut l[done..end], &mut r[done..end]);
            done = end;
        }
        // Interleave, so a difference in either channel shows up.
        l.into_iter().zip(r).flat_map(|(a, b)| [a, b]).collect()
    }

    fn beat() -> Vec<(usize, u8, u8)> {
        vec![
            (0, KICK, 127),
            (0, CLOSED, 90),
            (6_000, CLOSED, 70),
            (12_000, SNARE, 110),
            (12_000, OPEN, 100),
            (18_000, CLOSED, 80),
            (24_000, KICK, 120),
        ]
    }

    #[test]
    fn every_voice_sounds_and_falls_to_exact_silence() {
        for (pitch, name) in [(KICK, "kick"), (SNARE, "snare"), (OPEN, "open hat")] {
            let mut k = kit(48_000.0);
            let out = render(&mut k, 96_000, 512, &[(0, pitch, 127)]);
            let p = peak(&out);
            assert!(p > 0.1, "{name} is inaudible: peak {p}");
            assert!(p < 1.0, "{name} clips: peak {p}");
            assert!(out.iter().all(|x| x.is_finite()), "{name} produced NaN/inf");
            assert_eq!(k.active_voices(), 0, "{name} never released its voice");
            // Not "small": zero. A one-shot that only asymptotes leaves denormals behind.
            assert_eq!(out[out.len() - 1], 0.0, "{name} never reached silence");
        }
    }

    /// The guardrail every DSP module gets before it lands: where the host chose to split
    /// the buffer must not be audible.
    #[test]
    fn pulsus_is_block_size_independent() {
        let reference = render(&mut kit(48_000.0), 48_000, 512, &beat());
        assert!(peak(&reference) > 0.1, "silent reference");
        for block in [1usize, 7, 64, 480, 1_000, 4_096, 8_192] {
            let out = render(&mut kit(48_000.0), 48_000, block, &beat());
            assert_null(&reference, &out, 0.0, &format!("pulsus block {block}"));
        }
    }

    #[test]
    fn pulsus_is_deterministic() {
        let a = render(&mut kit(44_100.0), 40_000, 256, &beat());
        let b = render(&mut kit(44_100.0), 40_000, 256, &beat());
        assert_null(&a, &b, 0.0, "pulsus repeat render");
    }

    /// The one that says this is a kit and not two unrelated noises.
    ///
    /// The voices sum linearly, so the open hat's *own* contribution after the choke is
    /// exactly `(open + closed) − closed`. Comparing that against the same window of an
    /// unchoked open hat is a measurement of the choke itself, not of how loud the closed
    /// hat happens to be — and it fails outright if `choke` is ignored.
    #[test]
    fn a_closed_hat_chokes_an_open_one() {
        const CHOKE_AT: usize = 4_800; // 100 ms in.
        let window = CHOKE_AT * 2 + 960..CHOKE_AT * 2 + 19_200; // interleaved: ×2.

        let alone = render(&mut kit(48_000.0), 48_000, 256, &[(0, OPEN, 110)]);
        let both = render(
            &mut kit(48_000.0),
            48_000,
            256,
            &[(0, OPEN, 110), (CHOKE_AT, CLOSED, 110)],
        );
        let closed_only = render(&mut kit(48_000.0), 48_000, 256, &[(CHOKE_AT, CLOSED, 110)]);
        let residue: Vec<f32> = both.iter().zip(&closed_only).map(|(a, b)| a - b).collect();

        let free = rms(&alone[window.clone()]);
        let choked = rms(&residue[window]);
        assert!(free > 1e-4, "the unchoked open hat is silent to begin with");
        assert!(
            choked < free * 0.01,
            "the closed hat did not choke the open one: {choked} against {free}"
        );
    }

    /// A pad with no choke group must not be cut by anything, or every kick would be
    /// killed by the next hat.
    #[test]
    fn a_pad_without_a_choke_group_is_never_choked() {
        let kick_alone = render(&mut kit(48_000.0), 48_000, 256, &[(0, KICK, 120)]);
        let with_hats = render(
            &mut kit(48_000.0),
            48_000,
            256,
            &[(0, KICK, 120), (2_400, CLOSED, 100), (4_800, OPEN, 100)],
        );
        let hats_only = render(
            &mut kit(48_000.0),
            48_000,
            256,
            &[(2_400, CLOSED, 100), (4_800, OPEN, 100)],
        );
        let residue: Vec<f32> = with_hats
            .iter()
            .zip(&hats_only)
            .map(|(a, b)| a - b)
            .collect();
        assert_null(&kick_alone, &residue, 1e-5, "kick under hats");
    }

    #[test]
    fn a_pitch_with_no_pad_makes_no_sound() {
        let mut k = kit(48_000.0);
        let out = render(&mut k, 4_800, 256, &[(0, 60, 127)]);
        assert_eq!(k.active_voices(), 0);
        assert_eq!(peak(&out), 0.0, "a melodic note woke the kit up");
    }

    #[test]
    fn a_pad_at_the_centre_is_identical_in_both_channels() {
        let mut k = kit(48_000.0);
        let out = render(&mut k, 24_000, 256, &[(0, SNARE, 100)]);
        for pair in out.chunks_exact(2) {
            assert_eq!(pair[0], pair[1], "a centred pad is not centred");
        }
    }

    #[test]
    fn stealing_voices_never_panics_or_leaks() {
        let mut k = kit(48_000.0);
        for i in 0..(MAX_DRUM_VOICES * 4) {
            k.note_on([KICK, SNARE, OPEN][i % 3], 100, i as u64);
            let mut l = [0.0; 32];
            let mut r = [0.0; 32];
            k.process_add(&mut l, &mut r);
            assert!(l.iter().all(|x| x.is_finite()));
        }
        assert!(k.active_voices() <= MAX_DRUM_VOICES);
        let out = render(&mut k, 192_000, 1_024, &[]);
        assert_eq!(k.active_voices(), 0, "voices never freed");
        assert_eq!(out[out.len() - 1], 0.0);
    }

    /// A kit is a dozen sources landing on one channel, so the whole thing hit at once at
    /// full velocity has to leave headroom. Without this the kick alone reached 0 dBFS and
    /// every busy bar arrived at the fader already clipped — which is a mixing problem
    /// nobody can fix downstream.
    #[test]
    fn the_whole_kit_at_full_velocity_leaves_headroom() {
        let mut k = kit(48_000.0);
        let hits: Vec<(usize, u8, u8)> = [KICK, SNARE, CLOSED, OPEN]
            .iter()
            .map(|p| (0usize, *p, 127u8))
            .collect();
        let out = render(&mut k, 96_000, 512, &hits);
        let p = peak(&out);
        assert!(
            p < 0.95,
            "the kit clips when everything lands together: {p}"
        );
        assert!(p > 0.25, "the kit is too quiet to be usable: {p}");

        // And one pad on its own leaves considerably more, so a mix has somewhere to go.
        let loudest = peak(&render(&mut kit(48_000.0), 96_000, 512, &[(0, KICK, 127)]));
        assert!(
            loudest < 0.6,
            "the loudest pad alone is at {loudest}: no headroom for the rest of the kit"
        );
    }

    /// A decaying sine at 44.1 kHz — not the engine's rate, so resampling is exercised —
    /// long enough to outlast every block size in the test.
    fn test_sample(channels: usize) -> SampleRef {
        let rate = 44_100.0f32;
        let n = 22_050;
        let mut frames = Vec::with_capacity(n * channels);
        for i in 0..n {
            let t = i as f32 / rate;
            let env = (-t * 6.0).exp();
            let l = (std::f32::consts::TAU * 220.0 * t).sin() * env * 0.8;
            frames.push(l);
            if channels == 2 {
                // A different note on the right, so a stereo file's channels are told apart.
                frames.push((std::f32::consts::TAU * 330.0 * t).sin() * env * 0.8);
            }
        }
        SampleRef {
            frames: frames.into(),
            rate,
            channels,
        }
    }

    fn sampler(channels: usize, tune: f32) -> Pulsus {
        let mut k = Pulsus::new(48_000.0);
        let mut pad = PadSettings::kick();
        pad.voice = DrumVoice::Sample;
        pad.tune = tune;
        pad.decay = 4.0;
        k.set_pad_count(1);
        k.set_pad(0, 60, &pad);
        k.set_sample(0, Some(test_sample(channels)));
        k
    }

    #[test]
    fn a_sample_pad_plays_its_file_and_is_block_size_independent() {
        let reference = render(&mut sampler(1, 0.0), 40_000, 512, &[(0, 60, 127)]);
        let p = peak(&reference);
        assert!(p > 0.5 && p <= 0.8, "sample level is off: {p}");
        for block in [1usize, 7, 64, 480, 4_096] {
            let out = render(&mut sampler(1, 0.0), 40_000, block, &[(0, 60, 127)]);
            assert_null(&reference, &out, 0.0, &format!("sample at block {block}"));
        }
    }

    /// Tuning a sample up an octave plays it twice as fast, so it ends in half the time.
    #[test]
    fn tuning_a_sample_changes_its_speed() {
        let last = |tune: f32| {
            let out = render(&mut sampler(1, tune), 60_000, 256, &[(0, 60, 127)]);
            out.iter().rposition(|x| *x != 0.0).unwrap_or(0) / 2
        };
        let normal = last(0.0);
        let octave_up = last(12.0);
        // 22 050 frames at 44.1 kHz is 24 000 output frames at 48 kHz.
        assert!(
            (normal as i64 - 24_000).abs() < 50,
            "untuned length {normal}"
        );
        assert!(
            (octave_up as i64 - 12_000).abs() < 50,
            "an octave up should halve the length: {octave_up}"
        );
    }

    #[test]
    fn a_stereo_sample_keeps_its_channels_apart_and_a_missing_one_is_silent() {
        let out = render(&mut sampler(2, 0.0), 24_000, 256, &[(0, 60, 127)]);
        let differ = out.chunks_exact(2).filter(|f| f[0] != f[1]).count();
        assert!(differ > 1_000, "a stereo sample came out mono");

        let mut k = sampler(1, 0.0);
        k.set_sample(0, None);
        let out = render(&mut k, 4_800, 256, &[(0, 60, 127)]);
        assert_eq!(peak(&out), 0.0, "a pad with no file made a sound");
        assert_eq!(k.active_voices(), 0);
    }

    /// A sample obeys the pad's choke group like any other voice: a long sample cut by a
    /// pad in the same group stops.
    #[test]
    fn a_sample_is_choked_like_any_other_pad() {
        let build = || {
            let mut k = sampler(1, 0.0);
            k.set_pad_count(2);
            let mut long = PadSettings::kick();
            long.voice = DrumVoice::Sample;
            long.decay = 4.0;
            long.choke = 2;
            k.set_pad(0, 60, &long);
            let mut cutter = PadSettings::hat(0.03);
            cutter.choke = 2;
            k.set_pad(1, 61, &cutter);
            k
        };
        let alone = render(&mut build(), 48_000, 256, &[(0, 60, 127)]);
        let both = render(&mut build(), 48_000, 256, &[(0, 60, 127), (4_800, 61, 100)]);
        let cutter_only = render(&mut build(), 48_000, 256, &[(4_800, 61, 100)]);
        let residue: Vec<f32> = both.iter().zip(&cutter_only).map(|(a, b)| a - b).collect();
        let window = 2 * 4_800 + 960..2 * 4_800 + 19_200;
        assert!(rms(&residue[window.clone()]) < rms(&alone[window]) * 0.01);
    }

    #[test]
    fn drive_at_zero_is_exactly_the_identity() {
        for x in [-1.0f32, -0.3, 0.0, 1e-9, 0.7, 1.0] {
            assert_eq!(saturate(x, 0.0), x, "drive 0 changed {x}");
        }
        assert!(saturate(1.0, 1.0) <= 1.0);
        assert!(saturate(0.2, 1.0) > 0.2, "drive did not raise the body");
    }
}
