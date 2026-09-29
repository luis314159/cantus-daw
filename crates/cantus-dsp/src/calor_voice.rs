//! One Calor voice: three oscillators, sub and noise into one BLEP line, the mixer, the
//! high-pass, the filter and the amplifier, with its envelopes and LFOs. Controls update
//! every [`CONTROL`] base samples counted from the voice's own start and ramp linearly in
//! between; envelopes, cross-mod, poly-mod and filter FM run every (oversampled) sample.

use super::settings::{src, CalorSettings, Target, TARGETS};
use super::{HpfPos, LfoRestart, LfoSettings, LfoShape, OscRev, CONTROL};
use crate::modmatrix::ModMatrix;
use crate::va::drift::{drift, start_phase};
use crate::va::math::{bipolar, db_gain, exp2, hash3, midi_hz, sin_turns, tan_pi, tanh, Noise};
use crate::va::{
    Bandwidth, Blep, CardTrims, EnvCoefs, EnvModel, EnvSettings, FilterCoefs, FilterSettings,
    Kernel, OnePole, Shape, Solve, SubOsc, VaEnv, VaFilter, VaOsc,
};

/// A voice's level into the instance sum (the headroom for a chord).
pub const VOICE_SCALE: f32 = 0.3;

/// Per-note expressions (the contract's seven).
pub mod x {
    pub const TUNING: usize = 0;
    pub const PRESSURE: usize = 1;
    pub const BRIGHTNESS: usize = 2;
    pub const PAN: usize = 3;
    pub const VOLUME: usize = 4;
    pub const VIBRATO: usize = 5;
    pub const EXPRESSION: usize = 6;
    pub const DEFAULT: [f32; 7] = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0];
}

/// What the instance hands every voice for one render.
pub struct Shared<'a> {
    pub s: &'a CalorSettings,
    pub fs: f32,
    pub fs_os: f32,
    pub factor: usize,
    pub solve: Solve,
    pub matrix: &'a ModMatrix,
    /// The global sources (macros, wheel, pressure, bend, expression, pedal) in their
    /// slots; a voice writes its own over the rest.
    pub globals: &'a [f32; src::COUNT],
    /// Pitch bend, semitones.
    pub bend: f32,
    pub wheel: f32,
    pub trims: &'a [CardTrims; super::CARDS],
    /// The filter as set, when no route moves its shape.
    pub coefs: FilterCoefs,
    /// `log2` of the cutoff as set.
    pub cutoff_log2: f32,
    pub routes_shape_filter: bool,
    pub routes_env: bool,
    /// Stream position (base samples) of the first frame rendered.
    pub clock: u64,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Lfo {
    /// Cycles since the start (not wrapped: its whole part counts cycles for S&H).
    phase: f64,
    value: f32,
}

impl Lfo {
    fn start(&mut self, set: &LfoSettings, seed: u64) {
        self.phase = match set.restart {
            LfoRestart::Random => crate::va::math::unipolar(hash3(seed, 0x1f0, 1)) as f64,
            _ => set.phase as f64,
        };
    }

    /// Advance by `dt` seconds (at `rate` Hz) and read. `clock_s`: the stream's time, for a
    /// free-running LFO (a pure function of it, so every voice agrees).
    fn tick(
        &mut self,
        set: &LfoSettings,
        rate: f32,
        clock_s: f64,
        t_note: f64,
        dt: f32,
        seed: u64,
    ) -> f32 {
        let seed = if set.restart == LfoRestart::Free {
            0x5eed
        } else {
            seed
        };
        if set.restart == LfoRestart::Free {
            self.phase = clock_s * set.rate as f64 + set.phase as f64;
        } else {
            self.phase += (rate * dt) as f64;
        }
        let ki = self.phase.max(0.0) as u64;
        let p = (self.phase - ki as f64) as f32;
        let mut v = match set.shape {
            LfoShape::Triangle => {
                if p < 0.25 {
                    4.0 * p
                } else if p < 0.75 {
                    2.0 - 4.0 * p
                } else {
                    4.0 * p - 4.0
                }
            }
            LfoShape::Sine => sin_turns(p),
            LfoShape::SawUp => 2.0 * p - 1.0,
            LfoShape::SawDown => 1.0 - 2.0 * p,
            LfoShape::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LfoShape::SampleHold => bipolar(hash3(seed, 0x5a4, ki)),
            LfoShape::Smooth => {
                let a = bipolar(hash3(seed, 0x5a4, ki));
                let b = bipolar(hash3(seed, 0x5a4, ki.wrapping_add(1)));
                a + (b - a) * (p * p * (3.0 - 2.0 * p))
            }
        };
        if set.delay > 0.0011 {
            v *= ((t_note as f32) / set.delay).min(1.0);
        }
        if set.unipolar {
            v = 0.5 * (v + 1.0);
        }
        self.value = v;
        v
    }
}

/// One voice.
#[derive(Copy, Clone, Debug)]
pub struct Voice {
    pub active: bool,
    pub released: bool,
    pub key: u8,
    pub id: u64,
    pub card: usize,
    /// −1…1 across a unison stack (0 alone).
    pub stack_pos: f32,
    pub vel: f32,
    pub rel_vel: f32,
    rnd: f32,
    seed: u64,
    /// Base samples since the note started, counted a control tick at a time.
    age: u64,
    ctl_left: usize,
    first_tick: bool,
    /// Semitones: where the note is going and where the voice is.
    pub target: f32,
    pub pitch: f32,
    glide_step: f32,
    glide_coef: f32,
    pub x: [f32; 7],
    osc: [VaOsc; 3],
    sub: SubOsc,
    noise: Noise,
    pink: [f32; 3],
    blep: Blep,
    ac: OnePole,
    /// The mixer's bandwidth (22 kHz), when oversampled.
    bw: Bandwidth,
    bw_on: bool,
    hpf: OnePole,
    shelf: OnePole,
    filter: VaFilter,
    pub env: [VaEnv; 2],
    gate: VaEnv,
    env_c: [EnvCoefs; 2],
    gate_c: EnvCoefs,
    env_level: [f32; 2],
    lfo: [Lfo; 2],
    mods: [f32; TARGETS],
    fb: f32,
    // Ramped controls: a value and its step per oversampled sample.
    dt: [f64; 3],
    ddt: [f64; 3],
    cut: f32,
    dcut: f32,
    pw: [f32; 3],
    dpw: [f32; 3],
    mix: [f32; 5],
    dmix: [f32; 5],
    gl: f32,
    dgl: f32,
    gr: f32,
    dgr: f32,
    // Constant between ticks.
    shape: [Shape; 3],
    coefs: FilterCoefs,
    g_hpf: f32,
    g_ac: f32,
    g_shelf: f32,
    env_amt: f32,
    fm: f32,
    drive: f32,
    feedback: f32,
    vca_drive: f32,
    vel_gain: f32,
    pm_env: f32,
    pm_osc3: f32,
    xmod: f32,
    last_cut: f32,
    g: f32,
    /// Output multiplier: 1, or falling to 0 while a stolen voice fades as a ghost.
    pub fade: f32,
    pub fade_step: f32,
    /// Largest level into the filter since last read.
    pub in_peak: f32,
    /// The cutoff at the last tick, Hz.
    pub cut_hz: f32,
    /// The amplifier's envelope, last sample.
    pub amp_level: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Voice {
            active: false,
            released: false,
            key: 0,
            id: 0,
            card: 0,
            stack_pos: 0.0,
            vel: 0.0,
            rel_vel: 0.0,
            rnd: 0.0,
            seed: 0,
            age: 0,
            ctl_left: 0,
            first_tick: true,
            target: 60.0,
            pitch: 60.0,
            glide_step: 0.0,
            glide_coef: 0.0,
            x: x::DEFAULT,
            osc: [VaOsc::default(); 3],
            sub: SubOsc::default(),
            noise: Noise::new(0),
            pink: [0.0; 3],
            blep: Blep::new(Kernel::Wide),
            ac: OnePole::default(),
            bw: Bandwidth::default(),
            bw_on: false,
            hpf: OnePole::default(),
            shelf: OnePole::default(),
            filter: VaFilter::default(),
            env: [VaEnv::default(); 2],
            gate: VaEnv::default(),
            env_c: [EnvCoefs::default(); 2],
            gate_c: EnvCoefs::default(),
            env_level: [1.0; 2],
            lfo: [Lfo::default(); 2],
            mods: [0.0; TARGETS],
            fb: 0.0,
            dt: [0.0; 3],
            ddt: [0.0; 3],
            cut: 10.0,
            dcut: 0.0,
            pw: [0.5; 3],
            dpw: [0.0; 3],
            mix: [0.0; 5],
            dmix: [0.0; 5],
            gl: 0.0,
            dgl: 0.0,
            gr: 0.0,
            dgr: 0.0,
            shape: [Shape::default(); 3],
            coefs: FilterCoefs::default(),
            g_hpf: 0.0,
            g_ac: 0.0,
            g_shelf: 0.0,
            env_amt: 0.0,
            fm: 0.0,
            drive: 1.0,
            feedback: 0.0,
            vca_drive: 0.0,
            vel_gain: 1.0,
            pm_env: 0.0,
            pm_osc3: 0.0,
            xmod: 0.0,
            last_cut: f32::NAN,
            g: 0.0,
            fade: 1.0,
            fade_step: 0.0,
            in_peak: 0.0,
            cut_hz: 0.0,
            amp_level: 0.0,
        }
    }
}

#[path = "calor_voice_run.rs"]
mod run;
