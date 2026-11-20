//! One Fides voice (a child of `fides`): its exciter, its two resonators, its pitch and its
//! modulation, stepped on the control grid and rendered between the steps.

use super::exciter::{Biquad, ExcStep, Exciter, ExciterKind, Rng};
use super::modal::{Modal, ModalStep};
use super::string::{Guide, GuideStep};
use super::tables::{partials, Partial, ResKind, MAX_MODES};
use super::{
    target as tg, FidesSettings, ResSettings, CTRL, GLOBAL_SOURCES, SOURCES, VOICE_SOURCES,
};
use crate::lamina::voice::HammerShape;
use crate::math;
use crate::modmatrix::ModMatrix;

/// What every voice reads while it renders (the instance's, shared).
pub struct Shared<'a> {
    pub s: &'a FidesSettings,
    pub sr: f32,
    pub matrix: &'a ModMatrix,
    pub globals: &'a [f32; GLOBAL_SOURCES],
    pub bend_st: f32,
    pub hammer: &'a HammerShape,
    pub hammer_area: f32,
    /// The stream's sample at the start of the slice being rendered.
    pub clock: u64,
    /// The live input for the slice (mono), for the Input exciter.
    pub input: Option<&'a [f32]>,
}

/// A resonator's gains: what it takes of the strike and of what is sustained, and what it
/// gives out.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Gains {
    pub strike: f32,
    pub sustain: f32,
    pub out: f32,
}

/// Each resonator's trims, so every kind sounds about as loud at its defaults: the strike's
/// (a pulse into it), the sustained input's and the bowed output's — measured on a C4 at
/// velocity 100 with no body (`examples/fides_sounds -- levels`), each brought to about
/// −12 dBFS peak.
fn trims(kind: ResKind) -> (f32, f32, f32) {
    match kind {
        ResKind::String => (0.275, 0.248, 0.50),
        ResKind::Bar => (3.76, 0.44, 0.079),
        ResKind::Marimba => (2.15, 0.48, 0.35),
        ResKind::Tine => (1.18, 0.52, 0.097),
        ResKind::Plate => (1.85, 0.224, 0.46),
        ResKind::Membrane => (0.64, 0.32, 0.18),
        ResKind::Drumhead => (0.50, 0.28, 0.16),
        ResKind::Tube => (0.56, 0.19, 1.41),
        ResKind::Pipe => (0.275, 0.248, 0.50),
        ResKind::Bell => (0.55, 0.25, 0.17),
        ResKind::Glass => (0.97, 0.285, 0.195),
        ResKind::Pan => (1.0, 0.26, 0.234),
    }
}

/// The material's exponent: a partial's T60 falls as `(f/f0)^(−p)`, from 3 (wood: the
/// highs die at once) to 0 (glass, metal: everything rings as long as the fundamental).
pub fn material_p(material: f32) -> f64 {
    let m = (1.0 - material.clamp(0.0, 1.0)) as f64;
    3.0 * m * m
}

/// One resonator of a voice: a waveguide or a bank of modes, whichever its kind is.
#[derive(Clone, Debug)]
pub struct Res {
    sr: f64,
    kind: ResKind,
    pub guide: Guide,
    pub modal: Modal,
    pos: f64,
    listen: f64,
    pub f0: f64,
    pub t60: f64,
}

impl Res {
    fn new(sr: f32) -> Self {
        Res {
            sr: sr as f64,
            kind: ResKind::String,
            guide: Guide::new(sr),
            modal: Modal::new(sr),
            pos: 0.13,
            listen: 0.2,
            f0: 261.6,
            t60: 1.0,
        }
    }

    pub fn kind(&self) -> ResKind {
        self.kind
    }

    pub fn is_live(&self) -> bool {
        self.guide.is_live() || self.modal.is_live()
    }

    fn clear(&mut self) {
        self.guide.clear();
        self.modal.clear();
    }

    fn start(&mut self, r: &ResSettings, f0: f64, t60: f64, p: f64, pos: f64, keep: bool) {
        let keep = keep && self.kind == r.kind && self.is_live();
        if !keep {
            self.clear();
        }
        self.kind = r.kind;
        self.pos = pos;
        self.listen = r.listen as f64;
        self.f0 = f0;
        self.t60 = t60;
        if r.kind.is_guide() {
            let st = GuideStep {
                f0,
                t60,
                p,
                beta: pos,
            };
            self.guide.start(r.kind, r.inharm as f64, &st, keep);
        } else {
            let st = ModalStep { f0, t60, p };
            self.modal.start(
                r.kind,
                r.inharm as f64,
                r.modes,
                pos,
                self.listen,
                &st,
                keep,
            );
        }
    }

    fn step(&mut self, f0: f64, t60: f64, p: f64, pos: f64, span: u32) {
        self.f0 = f0;
        self.t60 = t60;
        if self.kind.is_guide() {
            let st = GuideStep {
                f0,
                t60,
                p,
                beta: pos,
            };
            self.guide.design(self.kind, &st, span);
        } else {
            if (pos - self.pos).abs() > 0.002 {
                let mut table = [Partial {
                    ratio: 0.0,
                    m: 0,
                    n: 0,
                }; MAX_MODES];
                partials(self.kind, &mut table);
                self.modal.place(&table, pos, self.listen);
                self.modal.design_again();
            }
            self.modal.design(&ModalStep { f0, t60, p });
        }
        self.pos = pos;
    }

    #[allow(clippy::too_many_arguments)]
    fn render(
        &mut self,
        imp: &[f32],
        sus: &[f32],
        bow: &[f32],
        bowing: bool,
        slope: f32,
        out: &mut [f32],
    ) {
        let (ti, ts, tb) = trims(self.kind);
        let out_gain = if bowing { tb } else { 1.0 };
        if self.kind.is_guide() {
            // A pulse's harmonics in a loop of P samples are 2/P of its area, and as many
            // as the pulse is short: the strike is scaled by √P (around C4's), so a pulse of
            // one length is about as loud (in RMS) low as high, as a bar's strike is.
            let p_ref = self.sr / 261.63;
            let g = Gains {
                strike: ti * (0.5 * (self.guide.period() * p_ref).sqrt()) as f32,
                sustain: ts,
                out: out_gain,
            };
            self.guide.render(imp, sus, bow, bowing, slope, &g, out);
        } else {
            let g = Gains {
                strike: ti,
                sustain: ts,
                out: out_gain,
            };
            self.modal.render(imp, sus, bow, bowing, slope, &g, out);
        }
    }

    /// The loop's or the fundamental's decay per sample, squared off for a sustained
    /// exciter: `√(1 − r²)`, so steady noise into it comes out at about its own power.
    /// A waveguide's comb gains by its loop's decay per period, a mode by its own per
    /// sample.
    fn sustain_comp(&self, sr: f32) -> f32 {
        let per_s = 6.907_755 / self.t60.max(1e-3);
        let rate = if self.kind.is_guide() {
            self.f0.max(1.0)
        } else {
            sr as f64
        };
        let r = math::exp(-per_s / rate);
        ((1.0 - r * r).max(0.0)).sqrt() as f32
    }
}

/// One voice.
#[derive(Clone, Debug)]
pub struct Voice {
    sr: f32,
    active: bool,
    ended: bool,
    released: bool,
    first: bool,
    key: u8,
    id: u64,
    velocity: f32,
    exc: Exciter,
    pub a: Res,
    pub b: Res,
    b_on: bool,
    serial: bool,
    step: ExcStep,
    bowing: bool,
    /// Key, transpose and fine, in semitones; the glide still to go; MPE's tuning.
    note: f32,
    glide: f32,
    tuning: f32,
    press: f32,
    slide: f32,
    has_slide: bool,
    vib_phase: f64,
    age: u64,
    rnd: f32,
    pan_l: f32,
    pan_r: f32,
    gain: f32,
    ga: f32,
    gb: f32,
    couple: f32,
    fade: f32,
    fade_step: f32,
    peak: f32,
    pos: f32,
    pressure: f32,
    imp: [f32; CTRL],
    sus: [f32; CTRL],
    bow: [f32; CTRL],
    out_a: [f32; CTRL],
    out_b: [f32; CTRL],
    sus_b: [f32; CTRL],
}

impl Voice {
    pub fn new(sample_rate: f32) -> Self {
        Voice {
            sr: sample_rate,
            active: false,
            ended: false,
            released: false,
            first: false,
            key: 60,
            id: 0,
            velocity: 0.0,
            exc: Exciter::new(sample_rate),
            a: Res::new(sample_rate),
            b: Res::new(sample_rate),
            b_on: false,
            serial: false,
            step: ExcStep::default(),
            bowing: false,
            note: 60.0,
            glide: 0.0,
            tuning: 0.0,
            press: 0.0,
            slide: 0.0,
            has_slide: false,
            vib_phase: 0.0,
            age: 0,
            rnd: 0.0,
            pan_l: 1.0,
            pan_r: 1.0,
            gain: 1.0,
            ga: 1.0,
            gb: 0.0,
            couple: 0.0,
            fade: 1.0,
            fade_step: 0.0,
            peak: 0.0,
            pos: 0.13,
            pressure: 0.0,
            imp: [0.0; CTRL],
            sus: [0.0; CTRL],
            bow: [0.0; CTRL],
            out_a: [0.0; CTRL],
            out_b: [0.0; CTRL],
            sus_b: [0.0; CTRL],
        }
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn key(&self) -> u8 {
        self.key
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn released(&self) -> bool {
        self.released
    }

    /// What its fade-out has left (1 when it is not fading).
    pub fn fade(&self) -> f32 {
        self.fade
    }

    /// The last slice's peak.
    pub fn peak(&self) -> f32 {
        self.peak
    }

    /// Where it is excited now, 0 … ½.
    pub fn position(&self) -> f32 {
        self.pos
    }

    /// The bow's pressure now (its settings, the modulation and MPE).
    pub fn pressure(&self) -> f32 {
        self.pressure
    }

    /// The bow's velocity now.
    pub fn bow_velocity(&self) -> f32 {
        self.exc.bow_fraction() * self.step.bow_v
    }

    pub fn exciter(&self) -> &Exciter {
        &self.exc
    }

    /// It ended since the last call (the instance hands its slot back).
    pub fn take_ended(&mut self) -> bool {
        std::mem::take(&mut self.ended)
    }

    /// Silence at once and forget everything.
    pub fn clear(&mut self) {
        self.active = false;
        self.ended = false;
        self.released = false;
        self.exc.clear();
        self.a.clear();
        self.b.clear();
        self.fade = 1.0;
        self.fade_step = 0.0;
        self.peak = 0.0;
        self.glide = 0.0;
    }

    /// Fade out over 4 ms (a voice taken by a new note, a choke), then end.
    pub fn fade_out(&mut self, sr: f32) {
        if self.active && self.fade_step == 0.0 {
            self.fade_step = 1.0 / (0.004 * sr);
            self.id = u64::MAX;
        }
    }

    /// Take `other`'s ringing resonators (a bar struck again is the same bar); `other`
    /// ends at once.
    pub fn take_resonators(&mut self, other: &mut Voice) {
        std::mem::swap(&mut self.a, &mut other.a);
        std::mem::swap(&mut self.b, &mut other.b);
        other.clear();
    }

    pub fn release(&mut self, _s: &FidesSettings) {
        if self.active && !self.released {
            self.released = true;
            self.exc.release();
        }
    }

    /// Legato: the voice moves to `key`, gliding when the glide is on, and is not struck.
    pub fn glide_to(&mut self, s: &FidesSettings, key: u8, id: u64) {
        let to = key as f32 + s.transpose_st + s.fine_ct * 0.01;
        if s.glide_ms > 0.0 {
            self.glide += self.note - to;
        }
        self.note = to;
        self.key = key;
        self.id = id;
        self.released = false;
    }

    /// A per-note expression: 0 tuning (semitones), 1 pressure, 2 slide.
    pub fn expression(&mut self, which: u8, value: f32) {
        match which {
            0 => self.tuning = value,
            1 => self.press = value.clamp(0.0, 1.0),
            _ => {
                self.slide = value.clamp(0.0, 1.0);
                self.has_slide = true;
            }
        }
    }

    /// A note starts. `keep`: its resonators ring on from the note it took them from.
    pub fn start(
        &mut self,
        sh: &Shared,
        key: u8,
        id: u64,
        velocity: f32,
        mut rng: Rng,
        keep: bool,
    ) {
        let s = sh.s;
        self.active = true;
        self.ended = false;
        self.released = false;
        self.first = true;
        self.key = key;
        self.id = id;
        self.velocity = velocity.clamp(0.0, 1.0);
        self.note = key as f32 + s.transpose_st + s.fine_ct * 0.01;
        self.glide = 0.0;
        self.tuning = 0.0;
        self.press = 0.0;
        self.slide = 0.0;
        self.has_slide = false;
        self.vib_phase = 0.0;
        self.age = 0;
        self.rnd = rng.bipolar();
        self.fade = 1.0;
        self.fade_step = 0.0;
        self.peak = 0.0;
        if !keep {
            self.a.clear();
            self.b.clear();
        }
        self.exc.rng = rng;
        let span = (CTRL as u64 - sh.clock % CTRL as u64) as u32;
        self.tick(sh, span);
        self.first = false;
    }
}

impl Voice {
    /// The control step: the sources, the routes, and every value of the next `span`
    /// samples (on a note's first step, the exciter struck and the resonators started).
    fn tick(&mut self, sh: &Shared, span: u32) {
        let s = sh.s;
        let sr = self.sr;
        let mut src = [0f32; SOURCES];
        src[0] = self.exc.env_level();
        src[1] = math::sin(std::f32::consts::TAU * self.vib_phase as f32);
        src[2] = self.velocity;
        src[3] = ((self.key as f32 - 60.0) / 60.0).clamp(-1.0, 1.0);
        src[4] = self.rnd;
        src[5] = (self.tuning / 48.0).clamp(-1.0, 1.0);
        src[6] = self.press;
        src[7] = self.slide;
        src[VOICE_SOURCES..].copy_from_slice(sh.globals);
        let mut m = [0f32; tg::COUNT];
        if !sh.matrix.is_empty() {
            sh.matrix.apply(&src, &mut m);
        }
        let md = |t: u16| m[t as usize];
        let e = &s.exc;
        let kind = e.kind;
        // ---- the exciter's step ----
        let sens = s.velocity.clamp(0.0, 1.0);
        let vel_gain = (1.0 - sens) + sens * math::powf(self.velocity, 1.5);
        let level = math::db_to_gain(e.level_db + md(tg::EXC_LEVEL));
        let tone_hz = e.tone_hz * math::exp2(md(tg::EXC_TONE));
        let tone = if tone_hz >= 19_999.0 {
            0.0
        } else {
            1.0 - math::exp(-std::f32::consts::TAU * tone_hz.min(0.45 * sr) / sr)
        };
        let pressure =
            (e.bow_pressure + md(tg::BOW_PRESSURE) + s.mpe_press * self.press).clamp(0.0, 1.0);
        let speed = (e.bow_speed + md(tg::BOW_SPEED)).clamp(0.0, 1.0);
        let press_gain = if matches!(kind, ExciterKind::Noise | ExciterKind::Input) {
            1.0 + 2.0 * s.mpe_press * self.press
        } else {
            1.0
        };
        let comp = self.a.sustain_comp(sr);
        let layer = (s.noise.level + md(tg::NOISE_LEVEL)).clamp(0.0, 1.0);
        let own = if kind == ExciterKind::Noise {
            level
        } else {
            0.0
        };
        let color = (s.noise.color_hz * math::exp2(md(tg::NOISE_COLOR))).clamp(20.0, 0.45 * sr);
        self.pressure = pressure;
        self.step = ExcStep {
            level: match kind {
                ExciterKind::Input => level * vel_gain * comp * press_gain,
                k if k.is_impulsive() => level,
                _ => level * vel_gain,
            },
            tone,
            noise: (e.noise + md(tg::EXC_NOISE)).clamp(0.0, 1.0),
            bow_v: (0.02 + 0.25 * speed) * level * vel_gain,
            bow_slope: 5.0 - 4.0 * pressure,
            noise_level: (own + layer) * vel_gain * press_gain * comp * 4.0,
            noise_band: Biquad::band(color, s.noise.width_oct, sr),
        };
        self.bowing = kind == ExciterKind::Bow;
        // ---- where, and the pitch ----
        let slide = if self.has_slide {
            s.mpe_slide * (self.slide - 0.5) * 0.4
        } else {
            0.0
        };
        self.pos = (e.pos + md(tg::EXC_POS) + slide).clamp(0.01, 0.5);
        let fade_in = if s.vib_delay_ms <= 0.0 {
            1.0
        } else {
            (self.age as f32 / (s.vib_delay_ms * 0.001 * sr)).min(1.0)
        };
        let vib = src[1] * (s.vib_depth_ct + md(tg::VIB_DEPTH)) * 0.01 * fade_in;
        if s.glide_ms > 0.0 && self.glide != 0.0 && !self.first {
            let k = math::exp(-4.6 * span as f32 / (s.glide_ms * 0.001 * sr));
            self.glide *= k;
            if self.glide.abs() < 1e-5 {
                self.glide = 0.0;
            }
        } else if s.glide_ms <= 0.0 {
            self.glide = 0.0;
        }
        let note = self.note + sh.bend_st + self.tuning + self.glide + vib + md(tg::PITCH);
        let pitch_of = |r: &ResSettings, tune: f32| {
            60.0 + (note - 60.0) * r.keytrack + r.tune_st + r.fine_ct * 0.01 + tune
        };
        let hz = |p: f32| s.a4 as f64 * math::exp2((p as f64 - 69.0) / 12.0);
        let t60_of = |r: &ResSettings, p: f32, dmod: f32| {
            let t = r.decay_s * math::exp2(dmod) * math::exp2(-r.decay_key * (p - 60.0) / 12.0);
            let t = if self.released { t.min(s.release_s) } else { t };
            t.max(0.005) as f64
        };
        let pa = pitch_of(&s.a, md(tg::A_TUNE));
        let (fa, ta) = (hz(pa), t60_of(&s.a, pa, md(tg::A_DECAY)));
        let mat_a = material_p(s.a.material + md(tg::A_MATERIAL));
        let pb = pitch_of(&s.b, md(tg::B_TUNE));
        let (fb, tb) = (hz(pb), t60_of(&s.b, pb, md(tg::B_DECAY)));
        let mat_b = material_p(s.b.material + md(tg::B_MATERIAL));
        // ---- the gains ----
        self.b_on = s.b_on;
        self.serial = s.serial;
        let mix = (s.mix + md(tg::MIX)).clamp(0.0, 1.0);
        let (ga, gb) = if s.b_on {
            ((2.0 * (1.0 - mix)).min(1.0), (2.0 * mix).min(1.0))
        } else {
            (1.0, 0.0)
        };
        self.ga = ga * math::db_to_gain(s.a.level_db + md(tg::A_LEVEL));
        self.gb = gb * math::db_to_gain(s.b.level_db + md(tg::B_LEVEL));
        self.gain = math::db_to_gain(md(tg::GAIN));
        let pan = (s.width * ((self.key as f32 - 60.0) / 30.0).clamp(-1.0, 1.0) + md(tg::PAN))
            .clamp(-1.0, 1.0);
        let (sn, cs) = math::sin_cos((pan + 1.0) * std::f32::consts::FRAC_PI_4);
        self.pan_l = cs * std::f32::consts::SQRT_2;
        self.pan_r = sn * std::f32::consts::SQRT_2;
        // ---- the resonators ----
        let pos = self.pos as f64;
        if self.first {
            let keep = self.a.is_live();
            self.a.start(&s.a, fa, ta, mat_a, pos, keep);
            if s.b_on {
                let keep = self.b.is_live();
                self.b.start(&s.b, fb, tb, mat_b, pos, keep);
            }
            let contact = e.contact_ms * math::exp2(md(tg::EXC_CONTACT));
            let es = super::exciter::ExciterSettings {
                contact_ms: contact.clamp(0.02, 40.0),
                ..*e
            };
            let rng = self.exc.rng;
            self.exc.start(
                &es,
                &s.noise,
                &self.step,
                self.velocity,
                sens,
                sh.hammer_area,
                fa as f32,
                rng,
            );
            // Its comp now that A is designed.
            let comp = self.a.sustain_comp(sr);
            self.step.noise_level = (own + layer) * vel_gain * press_gain * comp * 4.0;
        } else {
            self.a.step(fa, ta, mat_a, pos, span);
            if s.b_on {
                // B switched on mid-note waits for the next note (it was never struck).
                self.b.step(fb, tb, mat_b, pos, span);
            }
        }
        self.couple = if s.serial {
            2.0 * self.b.sustain_comp(sr)
        } else {
            0.0
        };
        self.vib_phase = (self.vib_phase + s.vib_rate_hz as f64 * span as f64 / sr as f64).fract();
    }
}

impl Voice {
    /// Render `l.len()` samples from `sh.clock`, **adding** into `l`/`r`: cut at the grid,
    /// a control step at each of its points.
    pub fn render(&mut self, sh: &Shared, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let mut at = 0;
        self.peak = 0.0;
        while at < n && self.active {
            let t = sh.clock + at as u64;
            let phase = (t % CTRL as u64) as usize;
            let k = (CTRL - phase).min(n - at);
            if phase == 0 {
                self.tick(sh, CTRL as u32);
            }
            let input = sh.input.map(|x| &x[at.min(x.len())..(at + k).min(x.len())]);
            self.run(sh, input, &mut l[at..at + k], &mut r[at..at + k]);
            at += k;
        }
    }

    fn run(&mut self, sh: &Shared, input: Option<&[f32]>, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        self.imp[..n].fill(0.0);
        self.sus[..n].fill(0.0);
        self.exc.render(
            &self.step,
            sh.hammer,
            input,
            &mut self.imp[..n],
            &mut self.sus[..n],
            &mut self.bow[..n],
        );
        let slope = self.step.bow_slope;
        self.out_a[..n].fill(0.0);
        self.a.render(
            &self.imp[..n],
            &self.sus[..n],
            &self.bow[..n],
            self.bowing,
            slope,
            &mut self.out_a[..n],
        );
        let b_on = self.b_on;
        if b_on {
            self.out_b[..n].fill(0.0);
            if self.serial {
                // A into B: what A gives out is what B is driven by.
                for i in 0..n {
                    self.sus_b[i] = self.couple * self.out_a[i];
                }
                self.imp[..n].fill(0.0);
                self.b.render(
                    &self.imp[..n],
                    &self.sus_b[..n],
                    &self.bow[..n],
                    false,
                    slope,
                    &mut self.out_b[..n],
                );
            } else {
                self.b.render(
                    &self.imp[..n],
                    &self.sus[..n],
                    &self.bow[..n],
                    self.bowing,
                    slope,
                    &mut self.out_b[..n],
                );
            }
        }
        let (ga, gb, g) = (self.ga, self.gb, self.gain);
        let (pl, pr) = (self.pan_l, self.pan_r);
        let mut peak = self.peak;
        for i in 0..n {
            let mut y = ga * self.out_a[i];
            if b_on {
                y += gb * self.out_b[i];
            }
            y *= g;
            if self.fade_step > 0.0 {
                self.fade = (self.fade - self.fade_step).max(0.0);
                y *= self.fade;
            }
            l[i] += y * pl;
            r[i] += y * pr;
            peak = peak.max(y.abs());
        }
        self.peak = peak;
        self.age += n as u64;
        let over = self.exc.done() && !self.a.is_live() && (!b_on || !self.b.is_live());
        if over || (self.fade_step > 0.0 && self.fade <= 0.0) {
            self.clear();
            self.ended = true;
        }
    }
}
