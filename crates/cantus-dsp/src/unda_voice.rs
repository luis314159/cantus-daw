//! One Unda voice: its sources, filters, amplifier and modulation. The synth
//! (`unda_synth.rs`) owns the voices, decides when they start and stop, and sums them.

use super::filter::FilterSlot;
use super::grana::Grana;
use super::modsrc::{self, mix, unit, Env, EnvSettings};
use super::osc::{LaneRandom, Osc, MAX_UNISON};
use super::partes::Partes;
use super::table::Table;
use super::{factory, target, Engine, Settings, TableChoice, SOURCES};

/// A voice's level at full velocity: one saw peaks at about −16 dBFS, so a full chord
/// keeps its headroom.
pub const VOICE_GAIN: f32 = 0.16;

#[inline]
fn db(v: f32) -> f32 {
    if v <= -47.9 {
        0.0
    } else {
        10f32.powf(v / 20.0)
    }
}

/// Seeded white noise with a tilt: −1 darker (towards brown), +1 brighter.
#[derive(Clone, Debug, Default)]
struct Noise {
    s: [u32; 2],
    lp: [f32; 2],
}

impl Noise {
    fn seed(&mut self, seed: u64) {
        self.s = [(mix(seed) as u32) | 1, (mix(seed ^ 0xABCD) as u32) | 1];
        self.lp = [0.0; 2];
    }

    #[inline]
    #[allow(clippy::needless_range_loop)] // two channels, three parallel arrays
    fn next(&mut self, color: f32, width: f32) -> (f32, f32) {
        let mut out = [0.0f32; 2];
        for c in 0..2 {
            let mut x = self.s[c];
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.s[c] = x;
            let w = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let a = if color < 0.0 { 0.03 } else { 0.35 };
            self.lp[c] += a * (w - self.lp[c]);
            out[c] = if color < 0.0 {
                w + (3.0 * self.lp[c] - w) * -color
            } else {
                w + (1.6 * (w - self.lp[c]) - w) * color
            };
        }
        (out[0], out[0] + (out[1] - out[0]) * width)
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct LfoVoice {
    phase: f32,
    cycle: u64,
    r0: f32,
    r1: f32,
    y: f32,
    done: bool,
}

/// What the synth tells every voice at a control tick.
pub struct Globals<'a> {
    pub sr: f32,
    pub oversample: usize,
    /// Macros, wheel, aftertouch, bend, expression, sustain — already in the source slots.
    pub src: &'a [f32; SOURCES],
    /// The free-running LFOs' values.
    pub free_lfo: &'a [f32; 8],
}

#[derive(Clone, Debug)]
pub struct Voice {
    pub active: bool,
    pub key: u8,
    pub id: u64,
    pub age: u64,
    vel: f32,
    relvel: f32,
    /// The note being sounded, in semitones (gliding towards `target`).
    pub pitch: f32,
    target: f32,
    glide: f32,
    pub env: [Env; 4],
    lfo: [LfoVoice; 8],
    pub osc: [Osc; 3],
    partes: [Partes; 3],
    grana: [Grana; 3],
    /// Each oscillator's last sample (lane 0), what another one's FM reads.
    last_out: [f32; 3],
    /// The oversampled rate this voice runs at.
    sr_int: f32,
    rnd: [LaneRandom; 3],
    sub: Osc,
    noise: Noise,
    filt: [FilterSlot; 2],
    pub src: [f32; SOURCES],
    /// The modulated values of this tick.
    pub val: [f32; target::COUNT],
    since: u64,
    last_tick: u64,
    amp: f32,
    amp_to: f32,
    seed: u64,
    drift_ct: f32,
    drift_oct: f32,
    rnd_value: f32,
    /// Per-note expressions: tuning (st), pressure, slide, volume (linear), pan.
    pub tuning: f32,
    pub pressure: f32,
    pub slide: f32,
    pub volume: f32,
    pub pan: f32,
    run: [bool; 3],
    hear: [bool; 3],
    sub_run: bool,
    noise_run: bool,
    flt_on: [bool; 2],
    mix: [f32; 2],
    routing: f32,
    noise_color: f32,
    noise_width: f32,
    noise_gain: [f32; 2],
}

impl Voice {
    pub fn new(sr: f32) -> Self {
        let r = LaneRandom {
            phase: [0.0; MAX_UNISON],
            detune: [0.0; MAX_UNISON],
        };
        Voice {
            active: false,
            key: 0,
            id: 0,
            age: 0,
            vel: 0.0,
            relvel: 0.0,
            pitch: 60.0,
            target: 60.0,
            glide: 0.0,
            env: Default::default(),
            lfo: [LfoVoice::default(); 8],
            osc: Default::default(),
            partes: Default::default(),
            grana: Default::default(),
            last_out: [0.0; 3],
            sr_int: sr,
            rnd: [r; 3],
            sub: Osc::default(),
            noise: Noise::default(),
            filt: [FilterSlot::new(sr), FilterSlot::new(sr)],
            src: [0.0; SOURCES],
            val: [0.0; target::COUNT],
            since: 0,
            last_tick: 0,
            amp: 0.0,
            amp_to: 0.0,
            seed: 0,
            drift_ct: 0.0,
            drift_oct: 0.0,
            rnd_value: 0.0,
            tuning: 0.0,
            pressure: 0.0,
            slide: 0.0,
            volume: 1.0,
            pan: 0.0,
            run: [false; 3],
            hear: [false; 3],
            sub_run: false,
            noise_run: false,
            flt_on: [false; 2],
            mix: [1.0; 2],
            routing: 0.0,
            noise_color: 0.0,
            noise_width: 1.0,
            noise_gain: [0.0; 2],
        }
    }

    /// Start a note. `from`: the pitch to glide from, if it glides. `stolen`: the voice was
    /// sounding, so its phases and filters carry on (no click) and its envelopes restart
    /// from where they are.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        s: &Settings,
        key: u8,
        id: u64,
        velocity: f32,
        stolen: bool,
        from: Option<f32>,
        age: u64,
        base_sr: f32,
    ) {
        self.active = true;
        self.key = key;
        self.id = id;
        self.age = age;
        self.vel = velocity.clamp(0.0, 1.0);
        self.relvel = 0.0;
        self.seed = mix(s.seed ^ mix(id) ^ ((key as u64) << 48));
        let seed = self.seed;
        for o in 0..3 {
            for i in 0..MAX_UNISON {
                self.rnd[o].phase[i] = unit(&[seed, 1, o as u64, i as u64]);
                self.rnd[o].detune[i] = unit(&[seed, 2, o as u64, i as u64]);
            }
        }
        if !stolen {
            for o in 0..3 {
                let os = &s.osc[o];
                self.osc[o].start(os.unison, os.phase, os.rand, &self.rnd[o]);
                let pos = s.base[target::OSC + o * target::OSC_N + target::POS];
                match (os.engine, table_of(&os.table)) {
                    (Engine::Partes, Some(t)) => {
                        let phase = os.phase + os.rand * self.rnd[o].phase[0];
                        self.partes[o].start(t, pos, phase);
                    }
                    (Engine::Grana, _) => self.grana[o].start(mix(seed ^ (0x6_7A_00 + o as u64))),
                    _ => {}
                }
            }
            self.last_out = [0.0; 3];
            self.sub.start(1, 0.0, 0.0, &self.rnd[0]);
            self.noise.seed(mix(seed ^ 0x004E_015E));
            for f in &mut self.filt {
                f.reset();
            }
            self.amp = 0.0;
        }
        self.drift_ct = s.drift * 5.0 * (2.0 * unit(&[seed, 4]) - 1.0);
        self.drift_oct = s.drift * (1.0 / 6.0) * (2.0 * unit(&[seed, 5]) - 1.0);
        self.rnd_value = 2.0 * unit(&[seed, 3]) - 1.0;
        self.target = key as f32;
        match from {
            Some(p) if s.glide_s > 0.0 => {
                self.pitch = p;
                self.glide = (self.target - p) / (s.glide_s * base_sr).max(1.0);
            }
            _ => {
                self.pitch = self.target;
                self.glide = 0.0;
            }
        }
        self.tuning = 0.0;
        self.pressure = 0.0;
        self.slide = 0.0;
        self.volume = 1.0;
        self.pan = 0.0;
        for (e, env) in self.env.iter_mut().enumerate() {
            env.trigger(&env_settings(s, &s.base, e), base_sr);
        }
        for (l, lfo) in self.lfo.iter_mut().enumerate() {
            let ls = &s.lfo[l];
            *lfo = LfoVoice {
                phase: ls.phase,
                cycle: 0,
                r0: 2.0 * unit(&[seed, 6, l as u64, 0]) - 1.0,
                r1: 2.0 * unit(&[seed, 6, l as u64, 1]) - 1.0,
                y: 0.0,
                done: false,
            };
        }
        self.since = 0;
        self.last_tick = 0;
    }

    /// Move to another note without a new attack (legato, or a mono voice returning).
    pub fn glide_to(&mut self, s: &Settings, key: u8, id: u64, base_sr: f32) {
        self.key = key;
        self.id = id;
        self.target = key as f32;
        self.glide = if s.glide_s > 0.0 {
            (self.target - self.pitch) / (s.glide_s * base_sr).max(1.0)
        } else {
            self.pitch = self.target;
            0.0
        };
    }

    pub fn release(&mut self, velocity: f32) {
        self.relvel = velocity.clamp(0.0, 1.0);
        for e in &mut self.env {
            e.release();
        }
    }

    pub fn kill(&mut self) {
        self.active = false;
        for e in &mut self.env {
            e.kill();
        }
    }

    pub fn releasing(&self) -> bool {
        self.env[0].stage == modsrc::Stage::Release
    }
}

/// An envelope's settings from the (modulated) values.
fn env_settings(s: &Settings, v: &[f32; target::COUNT], e: usize) -> EnvSettings {
    let b = target::ENV + e * target::ENV_N;
    let c = s.env_curves[e];
    EnvSettings {
        delay: v[b + target::DELAY],
        attack: v[b + target::ATTACK],
        hold: v[b + target::HOLD],
        decay: v[b + target::DECAY],
        sustain: v[b + target::SUSTAIN],
        release: v[b + target::RELEASE],
        a_curve: c.attack,
        d_curve: c.decay,
        r_curve: c.release,
    }
}

/// The table an oscillator reads.
#[inline]
pub fn table_of(choice: &TableChoice) -> Option<&Table> {
    match choice {
        TableChoice::Factory(i) => factory::tables().get(*i),
        TableChoice::Imported(t) => Some(t),
        TableChoice::Missing => None,
    }
}

#[path = "unda_voice_run.rs"]
mod run;
