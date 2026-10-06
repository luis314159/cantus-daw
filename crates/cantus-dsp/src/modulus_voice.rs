//! One Modulus voice: its operators' phases, envelopes and amplitudes, the matrix weights
//! it ramps between control points, its history for back edges and feedback, and its
//! decimators. `tick` works out the next control period; `render` (in `modulus_render.rs`)
//! runs the operators sample by sample.

use super::env::{dx_amp, dx_env_steps, dx_rate_scaling, DxEnv, Lfo, ModAdsr, TimeEnv, DX_STEP_DB};
use super::guard::{decide, Graph};
use super::tables::{Decimator, Tables};
use super::{db_to_lin, source, target, OpSettings, Patch, OPS};
use crate::math;

/// Where the detune macro pushes each operator, in tenths of its range: alternate signs so
/// a stack spreads rather than shifts.
const DETUNE_PATTERN: [f32; OPS] = [0.0, 1.0, -1.0, 0.5, -0.5, 0.75, -0.75, 0.25];
/// LFO amplitude sensitivity 0…3 as a share of `lfo1.amp`.
const AMS: [f32; 4] = [0.0, 0.3, 0.6, 1.0];
/// A voice whose envelopes are over fades this long if it is not silent yet.
const FADE_S: f32 = 0.01;

/// What every voice reads during a control tick.
pub(super) struct Ctx<'a> {
    pub p: &'a Patch,
    pub g: &'a Graph,
    pub t: &'a Tables,
    pub fs: f32,
    pub clock: u64,
    /// Global source values (macros, wheel, pressure, bend, expression, pedal).
    pub globals: &'a [f32; source::COUNT],
    /// Pitch bend in semitones, already scaled by the bend range.
    pub bend: f32,
}

/// How a note starts on a voice.
#[derive(Copy, Clone, Debug)]
pub(super) struct Start {
    pub key: u8,
    pub id: u64,
    pub velocity: f32,
    pub copy: usize,
    pub copies: usize,
    /// The voice was sounding: envelopes restart from where they are and phases run on.
    pub stolen: bool,
    /// Glide from this pitch (semitones).
    pub from: Option<f32>,
}

#[derive(Copy, Clone, Debug, Default)]
pub(super) struct OpVoice {
    pub phase: u32,
    pub inc: u32,
    pub amp: f32,
    pub damp: f32,
    pub mip: u8,
    pub hz: f32,
    pub time: TimeEnv,
    pub dx: DxEnv,
    /// Level, key scaling and velocity, linear (time mode; DX folds them into its steps).
    pub gain: f32,
    pub drift: f32,
}

#[derive(Clone, Debug)]
pub(super) struct Voice {
    pub active: bool,
    pub released: bool,
    dying: bool,
    pub fade: f32,
    pub key: u8,
    pub id: u64,
    pub vel: f32,
    relvel: f32,
    copies: usize,
    uni_cents: f32,
    uni_pan: f32,
    pub pitch: f32,
    pitch_to: f32,
    glide_rate: f32,
    /// Oversampling factor and the rate the operators run at.
    pub factor: usize,
    pub rate: f32,
    pub tame: f32,
    pub bandwidth: f32,
    pub ops: [OpVoice; OPS],
    pub y: [f32; OPS],
    pub hist: [[f32; OPS]; 16],
    pub hpos: usize,
    /// Matrix weights in phase units per unit of the source's output, and their steps.
    pub w: [[f32; OPS]; OPS],
    pub dw: [[f32; OPS]; OPS],
    pub fbw: [f32; OPS],
    pub dfbw: [f32; OPS],
    /// Output gains per operator (0 for one not heard), and their steps.
    pub gl: [f32; OPS],
    pub gr: [f32; OPS],
    pub dgl: [f32; OPS],
    pub dgr: [f32; OPS],
    /// Whether any weight or gain is moving this control period.
    pub ramping: bool,
    pub dec: [Decimator; 2],
    lfo: [Lfo; 2],
    lfo_rate_off: [f32; 2],
    menv: ModAdsr,
    rnd: f32,
    /// Seconds since the note began, and when it was released.
    pub elapsed: f32,
    rel_at: f32,
    penv_at_rel: f32,
    /// MPE: tuning (st), pressure, slide, pan, volume.
    pub mpe: [f32; 5],
    /// Stream sample of the start: age for stealing is the allocator's, this is the display's.
    pub started: u64,
}

impl Default for Voice {
    fn default() -> Self {
        Voice {
            active: false,
            released: false,
            dying: false,
            fade: 1.0,
            key: 60,
            id: 0,
            vel: 1.0,
            relvel: 0.0,
            copies: 1,
            uni_cents: 0.0,
            uni_pan: 0.0,
            pitch: 60.0,
            pitch_to: 60.0,
            glide_rate: 0.0,
            factor: 1,
            rate: 48_000.0,
            tame: 1.0,
            bandwidth: 0.0,
            ops: [OpVoice::default(); OPS],
            y: [0.0; OPS],
            hist: [[0.0; OPS]; 16],
            hpos: 0,
            w: [[0.0; OPS]; OPS],
            dw: [[0.0; OPS]; OPS],
            fbw: [0.0; OPS],
            dfbw: [0.0; OPS],
            gl: [0.0; OPS],
            gr: [0.0; OPS],
            dgl: [0.0; OPS],
            dgr: [0.0; OPS],
            ramping: false,
            dec: [Decimator::default(); 2],
            lfo: [Lfo::default(); 2],
            lfo_rate_off: [0.0; 2],
            menv: ModAdsr::default(),
            rnd: 0.0,
            elapsed: 0.0,
            rel_at: 0.0,
            penv_at_rel: 0.0,
            mpe: [0.0, 0.0, 0.0, 0.0, 1.0],
            started: 0,
        }
    }
}

/// A note's own random stream: from its id, key and unison copy, never from a clock.
pub fn seed(id: u64, key: u8, copy: usize) -> u32 {
    let mut x = id
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((key as u64) << 32 | copy as u64);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    (x as u32) | 1
}

fn next(r: &mut u32) -> f32 {
    *r ^= *r << 13;
    *r ^= *r >> 17;
    *r ^= *r << 5;
    (*r as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Keyboard level scaling at `key`, dB.
pub fn key_scaling_db(s: &OpSettings, key: u8) -> f32 {
    let d = key as i32 - s.ks_break as i32;
    if d == 0 {
        return 0.0;
    }
    let (depth, curve) = if d < 0 {
        (s.ks_ldepth, s.ks_lcurve)
    } else {
        (s.ks_rdepth, s.ks_rcurve)
    };
    if depth == 0.0 {
        return 0.0;
    }
    let x = d.unsigned_abs() as f32 / 24.0;
    let shape = match curve {
        1 | 2 => (math::exp2(3.0 * x) - 1.0) / 7.0,
        _ => x,
    };
    let sign = if curve <= 1 { -1.0 } else { 1.0 };
    (sign * depth * shape).clamp(-96.0, 24.0)
}

/// Velocity sensitivity, dB: 0 at full velocity, `−40·sens` at velocity 0.1.
pub fn velocity_db(sens: f32, vel: f32) -> f32 {
    if sens <= 0.0 {
        0.0
    } else {
        sens * 40.0 * math::log10(vel.max(0.01))
    }
}

impl Voice {
    fn level_db(&self, s: &OpSettings) -> f32 {
        s.level_db + key_scaling_db(s, self.key) + velocity_db(s.vel, self.vel)
    }

    /// The highest amplitude the operator can reach on this note (the guard's input).
    fn peak(&self, s: &OpSettings) -> f32 {
        let db = self.level_db(s);
        if s.dx {
            let top = s
                .dxenv
                .levels
                .iter()
                .map(|l| dx_env_steps(*l))
                .fold(0f32, f32::max);
            dx_amp(top + db / DX_STEP_DB)
        } else {
            let t = &s.time;
            let top = t.l[..t.n].iter().fold(t.l0, |m, l| m.max(*l));
            top * db_to_lin(db)
        }
    }

    pub fn start(&mut self, n: Start, c: &Ctx) {
        let p = c.p;
        let fresh = !(n.stolen && self.active);
        self.active = true;
        self.released = false;
        self.dying = false;
        self.fade = 1.0;
        self.key = n.key;
        self.id = n.id;
        self.vel = n.velocity.clamp(0.0, 1.0);
        self.relvel = 0.0;
        self.copies = n.copies.max(1);
        let mut r = seed(n.id, n.key, n.copy);
        self.rnd = next(&mut r);
        let spread = if self.copies > 1 {
            n.copy as f32 / (self.copies - 1) as f32 * 2.0 - 1.0
        } else {
            0.0
        };
        self.uni_cents = spread * 0.5 * p.uni_detune;
        self.uni_pan = spread * p.uni_spread;
        self.pitch_to = n.key as f32;
        self.pitch = n.from.unwrap_or(self.pitch_to);
        self.glide_rate = if p.glide > 0.0 {
            (self.pitch_to - self.pitch).abs() / p.glide
        } else {
            0.0
        };
        self.mpe = [0.0, 0.0, 0.0, 0.0, 1.0];
        self.elapsed = 0.0;
        self.started = c.clock;
        self.lfo_rate_off = [0.0; 2];
        self.menv.start();
        for i in 0..2 {
            let ph = if p.lfo_sync[i] {
                0.0
            } else {
                (p.lfo_rate[i] as f64 * c.clock as f64 / c.fs as f64).fract()
            };
            self.lfo[i].start(ph, r ^ (0x5bd1_e995 * (i as u32 + 1)));
        }
        // Frequencies at the note-on (for the guard and free-running phases).
        let mut hz = [0f32; OPS];
        let mut peak = [0f32; OPS];
        for o in 0..OPS {
            self.ops[o].drift = next(&mut r) * p.drift;
            hz[o] = self.op_hz(o, c, &[0.0; target::COUNT], 0.0);
            peak[o] = self.peak(&p.ops[o]);
        }
        let d = decide(p, c.g, &hz, &peak, c.fs);
        if fresh || d.factor != self.factor {
            for dec in &mut self.dec {
                dec.reset();
            }
            self.hist = [[0.0; OPS]; 16];
            self.y = [0.0; OPS];
            self.hpos = 0;
        }
        self.factor = d.factor;
        self.rate = c.fs * d.factor as f32;
        self.tame = d.tame;
        self.bandwidth = d.bandwidth;
        for (o, (s, &f0)) in p.ops.iter().zip(hz.iter()).enumerate() {
            let db = self.level_db(s);
            let vel = self.vel;
            let op = &mut self.ops[o];
            if s.dx {
                let qd = dx_rate_scaling(n.key, s.ks_rate);
                op.dx.start(&s.dxenv, db / DX_STEP_DB, qd, !fresh);
                op.gain = 1.0;
            } else {
                // Key and velocity scale every segment's time.
                let kv = math::exp2(-s.time.key_time * (n.key as f32 - 60.0) / 12.0)
                    * math::exp2(-s.time.vel_time * (vel - 0.5) * 2.0);
                op.time.start(&s.time, [kv; 3], !fresh);
                op.gain = db_to_lin(db);
            }
            if fresh {
                op.amp = 0.0;
                op.phase = if s.key_sync {
                    (s.phase / 360.0 * 4_294_967_296.0) as i64 as u32
                } else {
                    let cyc = f0 as f64 * c.clock as f64 / c.fs as f64;
                    (cyc.fract() * 4_294_967_296.0) as i64 as u32
                };
                if self.copies > 1 && s.key_sync {
                    op.phase = op.phase.wrapping_add((next(&mut r) * 1.0e9) as i32 as u32);
                }
            }
        }
        if fresh {
            self.w = [[0.0; OPS]; OPS];
            self.fbw = [0.0; OPS];
            self.gl = [0.0; OPS];
            self.gr = [0.0; OPS];
        }
    }

    pub fn release(&mut self, relvel: f32, p: &Patch) {
        if self.released {
            return;
        }
        self.released = true;
        self.relvel = relvel;
        self.rel_at = self.elapsed;
        self.penv_at_rel = self.penv(p);
        self.menv.release();
        for (o, op) in self.ops.iter_mut().enumerate() {
            op.dx.release();
            op.time.release(&p.ops[o].time);
        }
    }

    /// Legato: move to a new note without a new attack.
    pub fn glide_to(&mut self, key: u8, id: u64, p: &Patch) {
        self.key = key;
        self.id = id;
        self.pitch_to = key as f32;
        self.glide_rate = if p.glide > 0.0 {
            (self.pitch_to - self.pitch).abs() / p.glide
        } else {
            f32::INFINITY
        };
    }

    pub fn expression(&mut self, which: usize, value: f32) {
        if let Some(v) = self.mpe.get_mut(which) {
            *v = value;
        }
    }

    /// The pitch envelope, semitones.
    fn penv(&self, p: &Patch) -> f32 {
        let [l0, l1, l2, l3] = p.penv_l;
        if l0 == 0.0 && l1 == 0.0 && l2 == 0.0 && l3 == 0.0 {
            return 0.0;
        }
        let lerp = |a: f32, b: f32, x: f32, t: f32| {
            if t <= 0.0 {
                b
            } else {
                a + (b - a) * (x / t).min(1.0)
            }
        };
        let [t1, t2, t3] = p.penv_t;
        if !self.released {
            let e = self.elapsed;
            if e < t1 {
                lerp(l0, l1, e, t1)
            } else {
                lerp(l1, l2, e - t1, t2)
            }
        } else {
            lerp(self.penv_at_rel, l3, self.elapsed - self.rel_at, t3)
        }
    }

    /// An operator's frequency now, Hz.
    fn op_hz(&self, o: usize, c: &Ctx, off: &[f32; target::COUNT], vib: f32) -> f32 {
        let p = c.p;
        let s = &p.ops[o];
        let pe = if s.penv { self.penv(p) } else { 0.0 };
        let dmac = (p.detune_macro + off[target::MDETUNE]).clamp(-1.0, 1.0);
        let det = s.detune
            + off[target::DETUNE + o]
            + dmac * 10.0 * DETUNE_PATTERN[o]
            + self.ops[o].drift;
        let f = if s.fixed {
            s.fixed_hz * math::exp2(pe / 12.0)
        } else {
            let note = self.pitch
                + p.transpose
                + (p.tune + off[target::TUNE] + self.uni_cents) / 100.0
                + c.bend
                + self.mpe[0]
                + pe
                + vib;
            let mut ratio = s.ratio * math::exp2(off[target::RATIO + o]);
            let harm = (p.harm + off[target::HARM]).clamp(-1.0, 1.0);
            if harm != 0.0 && !c.g.carrier[o] {
                ratio = math::powf(ratio, 1.0 + 0.25 * harm);
            }
            440.0 * math::exp2((note - 69.0) / 12.0) * ratio
        };
        f * math::exp2(det / 1200.0) + s.offset * (1.0 + dmac)
    }
}

#[path = "modulus_tick.rs"]
mod tick;
