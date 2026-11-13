//! Cribrum, the auto filter (ADR 0130, provisional) — Latin *cribrum*, the sieve.
//!
//! Two filters — the synths' zero-delay-feedback models ([`crate::va::filter`], through
//! [`filter`]) in series or in parallel — moved by three sources summed:
//!
//! - **an LFO** (the modulation family's, [`crate::modfx_lfo`]): on the song (1/64 … 8/1,
//!   dotted, triplet) or free in Hz, seven waves, a phase and a stereo offset;
//! - **an envelope follower** ([`crate::dynfx_follow`]) on the input or on a sidechain, its
//!   level in decibels as the movement ([`modulation::env_amount`]);
//! - **a drawn curve** on the song (Fluctus's points and the automation editor's segment
//!   shapes), smoothed as a function of the song position ([`modulation::CurveMod`]).
//!
//! Each source has a depth onto the cutoffs (octaves), the resonances and the vowels; the
//! second filter takes `track` of the movement. A stereo spread opens the cutoffs apart.
//! The filters may run oversampled (2× or 4×, minimum phase: a sample or two of latency,
//! reported, the dry path delayed to match).
//!
//! **The null**: at mix 0 (the default) nothing runs but the output gain, which at 0 dB is
//! not applied; every sample passes untouched. A mix turned up from 0 starts the filters
//! from a new filter's state, gliding in.
//!
//! **At rest** (ADR 0031 §5.1): the mix and the gain still and, while the filters run,
//! every filter, the oversamplers, the dry delay and the follower as a new unit's, every
//! glide landed. The sources' positions are the clock's (the follower's settles to +0.0),
//! so [`Cribrum::skip`] only moves the clock.

#[path = "cribrum_filter.rs"]
pub mod filter;
#[path = "cribrum_mod.rs"]
pub mod modulation;
#[path = "cribrum_response.rs"]
pub mod response;

use crate::dynfx_follow::{sane, Detect, EnvelopeFollower};
use crate::fervor::os::Oversampler;
use crate::fluctus::{CurvePoint, ShapeFn, POINTS};
use crate::math;
use crate::modfx_clock::{peak_db, peak_of, tel, Clock, Transport};
use crate::modfx_lfo::{Lfo, LfoSettings, Rate, Shape};
use crate::modfx_line::Glide;
use crate::va::filter::Model;
use filter::{Chan, Mode, Voice};
use modulation::{CurveMod, Wave};

/// One filter's settings.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FilterParams {
    pub model: Model,
    pub mode: Mode,
    pub slope24: bool,
    /// Hz.
    pub cut: f32,
    /// 0 … 1.
    pub res: f32,
    pub drive_db: f32,
    /// 0 … 4.
    pub vowel: f32,
}

impl Default for FilterParams {
    fn default() -> Self {
        FilterParams {
            model: Model::Svf,
            mode: Mode::Low,
            slope24: false,
            cut: 1_000.0,
            res: 0.25,
            drive_db: 0.0,
            vowel: 0.0,
        }
    }
}

/// A source's three depths.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Depth {
    /// Octaves.
    pub cut: f32,
    /// Of the resonance's range.
    pub res: f32,
    /// Vowels.
    pub vowel: f32,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LfoParams {
    pub synced: bool,
    /// Beats in a cycle, when synced.
    pub beats: f64,
    pub rate_hz: f32,
    pub wave: Wave,
    pub edge: f32,
    /// Degrees.
    pub phase: f32,
    /// Degrees the right channel runs ahead.
    pub stereo: f32,
}

impl Default for LfoParams {
    fn default() -> Self {
        LfoParams {
            synced: true,
            beats: 1.0,
            rate_hz: 1.0,
            wave: Wave::Sine,
            edge: 0.3,
            phase: 0.0,
            stereo: 0.0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EnvParams {
    /// The follower hears the sidechain (when one is routed).
    pub key: bool,
    pub att_ms: f32,
    pub rel_ms: f32,
    pub gain_db: f32,
    /// The levels it spans, dB below full scale.
    pub range_db: f32,
}

impl Default for EnvParams {
    fn default() -> Self {
        EnvParams {
            key: false,
            att_ms: 5.0,
            rel_ms: 150.0,
            gain_db: 0.0,
            range_db: modulation::ENV_RANGE_DB,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CurveParams {
    /// Beats in a cycle.
    pub beats: f64,
    pub smooth_ms: f32,
    pub points: [Option<CurvePoint>; POINTS],
}

impl Default for CurveParams {
    fn default() -> Self {
        let mut points = [None; POINTS];
        points[0] = Some(CurvePoint {
            x: 0.0,
            y: 1.0,
            tension: -0.6,
            shape: 0,
        });
        points[1] = Some(CurvePoint {
            x: 0.9,
            y: 0.0,
            tension: 0.0,
            shape: 0,
        });
        CurveParams {
            beats: 4.0,
            smooth_ms: 2.0,
            points,
        }
    }
}

/// Everything Cribrum is set to.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CribrumSettings {
    /// 0 … 1.
    pub mix: f32,
    pub out_db: f32,
    pub parallel: bool,
    /// Octaves between the two channels' cutoffs.
    pub spread: f32,
    /// 1, 2 or 4.
    pub os: usize,
    pub a: FilterParams,
    pub b: FilterParams,
    pub b_on: bool,
    /// −1 … 1.
    pub b_track: f32,
    pub lfo: LfoParams,
    pub lfo_depth: Depth,
    pub env: EnvParams,
    pub env_depth: Depth,
    pub curve: CurveParams,
    pub curve_depth: Depth,
}

impl Default for CribrumSettings {
    fn default() -> Self {
        CribrumSettings {
            mix: 0.0,
            out_db: 0.0,
            parallel: false,
            spread: 0.0,
            os: 1,
            a: FilterParams::default(),
            b: FilterParams::default(),
            b_on: false,
            b_track: 1.0,
            lfo: LfoParams::default(),
            lfo_depth: Depth::default(),
            env: EnvParams::default(),
            env_depth: Depth::default(),
            curve: CurveParams::default(),
            curve_depth: Depth::default(),
        }
    }
}

/// The oversampling factors, by the `os` parameter's index.
pub const OS_FACTORS: [usize; 3] = [1, 2, 4];
/// The random waves' seed.
pub const SEED: u64 = 0xC21B_2026;
/// The longest dry delay (4× minimum phase's latency, with room).
const DRY_MAX: usize = 8;
const SNAP: f32 = 1e-6;
/// The lowest and highest cutoff a movement may reach (the highest also under 0.47 of the
/// sample rate).
pub const CUT_MIN: f32 = 10.0;
pub const CUT_MAX: f32 = 22_000.0;

/// Cribrum's own telemetry, after the family's header ([`tel`]).
pub mod t {
    use crate::modfx_clock::tel::OWN;
    /// The cutoffs now, Hz: filter A left and right, filter B left and right.
    pub const CUT_A_L: usize = OWN;
    pub const CUT_A_R: usize = OWN + 1;
    pub const CUT_B_L: usize = OWN + 2;
    pub const CUT_B_R: usize = OWN + 3;
    /// The resonances now (0 … 1), left.
    pub const RES_A: usize = OWN + 4;
    pub const RES_B: usize = OWN + 5;
    /// The vowels now (0 … 4), left.
    pub const VOWEL_A: usize = OWN + 6;
    pub const VOWEL_B: usize = OWN + 7;
    /// The follower's movement (0 … 1) and its level, dBFS.
    pub const ENV: usize = OWN + 8;
    pub const ENV_DB: usize = OWN + 9;
    /// The curve's phase (0 … 1) and its smoothed value (0 … 1).
    pub const CURVE_PHASE: usize = OWN + 10;
    pub const CURVE: usize = OWN + 11;
    /// 1 while the filters run (the mix above 0).
    pub const ENGAGED: usize = OWN + 12;
    /// The oversampling factor and the latency in samples.
    pub const OS: usize = OWN + 13;
    pub const LATENCY: usize = OWN + 14;
}

/// The glides, by role.
#[derive(Clone, Debug)]
struct Glides {
    mix: Glide,
    out: Glide,
    /// Cutoffs as log₂ Hz.
    cut: [Glide; 2],
    res: [Glide; 2],
    drive: [Glide; 2],
    vowel: [Glide; 2],
    track: Glide,
    spread: Glide,
    /// LFO, follower, curve × cut, res, vowel.
    depth: [[Glide; 3]; 3],
}

impl Glides {
    fn new(sr: f32) -> Glides {
        let g = |v: f32, ms: f32| Glide::new(v, ms, sr, SNAP);
        Glides {
            mix: g(0.0, 10.0),
            out: g(1.0, 10.0),
            cut: [g(10.0, 20.0), g(10.0, 20.0)],
            res: [g(0.25, 10.0), g(0.25, 10.0)],
            drive: [g(1.0, 10.0), g(1.0, 10.0)],
            vowel: [g(0.0, 20.0), g(0.0, 20.0)],
            track: g(1.0, 10.0),
            spread: g(0.0, 20.0),
            depth: [[g(0.0, 10.0); 3]; 3],
        }
    }

    /// Every glide but the mix and the gain.
    fn inner(&mut self) -> impl Iterator<Item = &mut Glide> {
        self.cut
            .iter_mut()
            .chain(self.res.iter_mut())
            .chain(self.drive.iter_mut())
            .chain(self.vowel.iter_mut())
            .chain(std::iter::once(&mut self.track))
            .chain(std::iter::once(&mut self.spread))
            .chain(self.depth.iter_mut().flatten())
    }

    fn inner_still(&self) -> bool {
        self.cut
            .iter()
            .chain(self.res.iter())
            .chain(self.drive.iter())
            .chain(self.vowel.iter())
            .chain([&self.track, &self.spread])
            .chain(self.depth.iter().flatten())
            .all(Glide::still)
    }
}

#[derive(Clone, Debug)]
pub struct Cribrum {
    sr: f32,
    set: CribrumSettings,
    clock: Clock,
    lfo: Lfo,
    curve: CurveMod,
    shape: ShapeFn,
    follow: EnvelopeFollower,
    env_gain: f32,
    gl: Glides,
    /// [filter][channel].
    chans: [[Chan; 2]; 2],
    os: [Oversampler; 2],
    factor: usize,
    dry: [[f32; DRY_MAX]; 2],
    dry_pos: usize,
    latency: usize,
    /// The filters run (the mix is above 0, or gliding).
    engaged: bool,
    tel: [f32; 32],
    blocks: u32,
}

impl Cribrum {
    /// An auto filter whose curve's segments take `shape` (the automation editor's
    /// `Shape::at`).
    pub fn new(sample_rate: f32, shape: ShapeFn) -> Cribrum {
        let set = CribrumSettings::default();
        let mut c = Cribrum {
            sr: sample_rate,
            set,
            clock: Clock::new(),
            lfo: Lfo::new(sample_rate),
            curve: CurveMod::new(sample_rate, shape),
            shape,
            follow: EnvelopeFollower::new(sample_rate),
            env_gain: 1.0,
            gl: Glides::new(sample_rate),
            chans: [[Chan::default(); 2]; 2],
            os: [Oversampler::new(1, true), Oversampler::new(1, true)],
            factor: 1,
            dry: [[0.0; DRY_MAX]; 2],
            dry_pos: 0,
            latency: 0,
            engaged: false,
            tel: [0.0; 32],
            blocks: 0,
        };
        c.set(&set);
        c.reset();
        c
    }

    pub fn settings(&self) -> &CribrumSettings {
        &self.set
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// New settings: allocation-free; a new oversampling clears the filters.
    pub fn set(&mut self, s: &CribrumSettings) {
        let factor = if OS_FACTORS.contains(&s.os) { s.os } else { 1 };
        if factor != self.factor {
            self.factor = factor;
            for o in &mut self.os {
                o.configure(factor, true);
            }
            self.latency = self.os[0].latency().min(DRY_MAX - 1);
            self.clear_wet();
            self.dry = [[0.0; DRY_MAX]; 2];
            self.dry_pos = 0;
        }
        let g = &mut self.gl;
        g.mix.target = s.mix.clamp(0.0, 1.0);
        g.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            math::db_to_gain(s.out_db.clamp(-60.0, 24.0))
        };
        for (i, f) in [&s.a, &s.b].into_iter().enumerate() {
            g.cut[i].target = math::log2(f.cut.clamp(CUT_MIN, CUT_MAX));
            g.res[i].target = f.res.clamp(0.0, 1.0);
            g.drive[i].target = if f.drive_db == 0.0 {
                1.0
            } else {
                math::db_to_gain(f.drive_db.clamp(0.0, 36.0))
            };
            g.vowel[i].target = f.vowel.clamp(0.0, 4.0);
        }
        g.track.target = s.b_track.clamp(-1.0, 1.0);
        g.spread.target = s.spread.clamp(0.0, 4.0);
        for (i, d) in [s.lfo_depth, s.env_depth, s.curve_depth].iter().enumerate() {
            g.depth[i][0].target = d.cut.clamp(-8.0, 8.0);
            g.depth[i][1].target = d.res.clamp(-1.0, 1.0);
            g.depth[i][2].target = d.vowel.clamp(-4.0, 4.0);
        }
        let lfo = LfoSettings {
            rate: if s.lfo.synced {
                Rate::Beats(s.lfo.beats.max(1e-3))
            } else {
                Rate::Hz(f64::from(s.lfo.rate_hz.clamp(0.0, 40.0)))
            },
            // The family's settings carry the phase; the wave is drawn here (the drift).
            shape: Shape::Sine,
            offset: f64::from(s.lfo.phase.clamp(0.0, 360.0)) / 360.0,
            edge: s.lfo.edge.clamp(0.0, 1.0),
            swing: 0.0,
            seed: SEED,
        };
        self.lfo.set(&lfo, &self.clock);
        self.curve.set(
            &s.curve.points,
            self.shape,
            s.curve.beats,
            s.curve.smooth_ms,
        );
        self.follow.set(
            Detect::Peak,
            s.env.att_ms.clamp(0.01, 5_000.0),
            s.env.rel_ms.clamp(0.1, 20_000.0),
        );
        self.env_gain = if s.env.gain_db == 0.0 {
            1.0
        } else {
            math::db_to_gain(s.env.gain_db.clamp(-48.0, 48.0))
        };
        self.set = *s;
        self.settle_engaged();
    }

    /// Whether the filters run; disengaged, the inner glides jump to their targets (they
    /// move nothing heard) and the wet path waits empty.
    fn settle_engaged(&mut self) {
        let off = self.gl.mix.value == 0.0 && self.gl.mix.target == 0.0;
        if off {
            for g in self.gl.inner() {
                g.jump(g.target);
            }
            if self.engaged {
                self.clear_wet();
            }
        } else if !self.engaged {
            self.clear_wet();
        }
        self.engaged = !off;
    }

    fn clear_wet(&mut self) {
        for c in self.chans.iter_mut().flatten() {
            c.reset();
        }
        for o in &mut self.os {
            o.reset();
        }
        self.follow.reset();
    }

    pub fn set_transport(&mut self, t: &Transport) {
        self.clock.set(t);
        self.lfo.follow(&self.clock);
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        self.lfo.reset();
        self.gl.mix.jump(self.gl.mix.target);
        self.gl.out.jump(self.gl.out.target);
        for g in self.gl.inner() {
            g.jump(g.target);
        }
        self.clear_wet();
        self.dry = [[0.0; DRY_MAX]; 2];
        self.dry_pos = 0;
        self.engaged = self.gl.mix.value != 0.0;
        self.tel = [0.0; 32];
        self.blocks = 0;
    }

    pub fn latency(&self) -> usize {
        self.latency
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !self.gl.mix.still() || !self.gl.out.still() {
            return Some("the mix or the gain is gliding");
        }
        if !self.dry.iter().all(|d| crate::rest::all_zero(d)) {
            return Some("the dry delay holds sound");
        }
        if !self.engaged {
            return None;
        }
        if !self.gl.inner_still() {
            return Some("a glide is moving");
        }
        if !self.chans.iter().flatten().all(Chan::at_rest) {
            return Some("a filter rings");
        }
        if !self.os.iter().all(Oversampler::is_clear) {
            return Some("the oversampling holds sound");
        }
        if !self.follow.at_rest() {
            return Some("the follower is falling");
        }
        None
    }

    /// `frames` of silence skipped at rest: the clock and the follower's count of quiet
    /// samples move on; nothing else counts samples.
    pub fn skip(&mut self, frames: usize) {
        self.clock.skip(frames);
        if self.engaged {
            self.follow.skip(frames);
        }
    }

    /// The LFO at sample `i` of the next call: left and right.
    #[inline]
    fn lfo_at(&self, i: usize) -> (f32, f32) {
        let l = &self.set.lfo;
        let cycles = self.lfo.cycles(&self.clock, i);
        let at = |delta: f64| {
            let (p, c) = self.lfo.phase_at(cycles + delta);
            modulation::wave(l.wave, p, c, l.edge, SEED)
        };
        let left = at(0.0);
        let right = if l.stereo == 0.0 {
            left
        } else {
            at(f64::from(l.stereo.clamp(0.0, 180.0)) / 360.0)
        };
        (left, right)
    }

    /// The LFO's phase (0 … 1) at sample `i` of the next call.
    pub fn lfo_phase(&self, i: usize) -> f64 {
        self.lfo.phase(&self.clock, i).0
    }
}

impl Cribrum {
    /// The sample `x` of channel `ch` into the dry delay; what comes out `latency` later.
    #[inline]
    fn delay_dry(&mut self, ch: usize, x: f32) -> f32 {
        if self.latency == 0 {
            return x;
        }
        let d = &mut self.dry[ch];
        d[self.dry_pos] = x;
        d[(self.dry_pos + DRY_MAX - self.latency) % DRY_MAX]
    }

    /// One base-rate sample of channel `ch` through the filters (oversampled when set).
    #[inline]
    fn wet(&mut self, ch: usize, x: f32, a: &Voice, b: Option<&Voice>) -> f32 {
        let fs = self.sr * self.factor as f32;
        let parallel = self.set.parallel;
        let [fa, fb] = &mut self.chans;
        let mut run = |x: f32| -> f32 {
            let ya = fa[ch].tick(x, a, fs);
            match b {
                None => ya,
                Some(b) if parallel => 0.5 * (ya + fb[ch].tick(x, b, fs)),
                Some(b) => fb[ch].tick(ya, b, fs),
            }
        };
        if self.factor == 1 {
            return run(x);
        }
        let mut buf = [0.0f32; 8];
        self.os[ch].up(x, &mut buf);
        for v in buf.iter_mut().take(self.factor) {
            *v = run(*v);
        }
        self.os[ch].down(&mut buf)
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        if self.engaged {
            self.run(&mut l[..n], &mut r[..n], key);
        } else {
            for i in 0..n {
                let g = self.gl.out.next();
                let (x, y) = (self.delay_dry(0, l[i]), self.delay_dry(1, r[i]));
                self.dry_pos = (self.dry_pos + 1) % DRY_MAX;
                if g != 1.0 || self.latency != 0 {
                    l[i] = x * g;
                    r[i] = y * g;
                }
            }
        }
        let last = n.saturating_sub(1);
        self.telemetry_block(last, in_peak, &l[..n], &r[..n]);
        self.clock.advance(n);
        self.settle_engaged();
    }

    fn run(&mut self, l: &mut [f32], r: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = l.len();
        let tempo = self.clock.tempo();
        let cut_max = CUT_MAX.min(0.47 * self.sr);
        let keyed = if self.set.env.key { key } else { None };
        let b_on = self.set.b_on;
        let (fa, fb) = (self.set.a, self.set.b);
        for i in 0..n {
            let (xl, xr) = (sane(l[i]), sane(r[i]));
            let (lfo_l, lfo_r) = self.lfo_at(i);
            let (kl, kr) = match keyed {
                Some((a, b)) if i < a.len() && i < b.len() => (sane(a[i]), sane(b[i])),
                _ => (xl, xr),
            };
            let level = self.follow.run_pair(kl * self.env_gain, kr * self.env_gain);
            let env = modulation::env_amount(level, self.set.env.range_db);
            let cur = self.curve.at(self.clock.song(i), tempo);
            let g = &mut self.gl;
            let mut d = [[0.0f32; 3]; 3];
            for (s, row) in g.depth.iter_mut().enumerate() {
                for (k, gl) in row.iter_mut().enumerate() {
                    d[s][k] = gl.next();
                }
            }
            let mix = g.mix.next();
            let out = g.out.next();
            let cut = [g.cut[0].next(), g.cut[1].next()];
            let res = [g.res[0].next(), g.res[1].next()];
            let drive = [g.drive[0].next(), g.drive[1].next()];
            let vowel = [g.vowel[0].next(), g.vowel[1].next()];
            let track = g.track.next();
            let spread = g.spread.next();
            let mut voices = [[Voice::default(); 2]; 2];
            for (ch, lfo) in [lfo_l, lfo_r].into_iter().enumerate() {
                let src = [lfo, env, cur];
                let sum = |k: usize| src[0] * d[0][k] + src[1] * d[1][k] + src[2] * d[2][k];
                let (oct, dres, dvow) = (sum(0), sum(1), sum(2));
                let side = if ch == 0 { -0.5 } else { 0.5 } * spread;
                for (f, (p, t)) in [(&fa, 1.0f32), (&fb, track)].into_iter().enumerate() {
                    voices[f][ch] = Voice {
                        model: p.model,
                        mode: p.mode,
                        slope24: p.slope24,
                        cut: math::exp2(cut[f] + t * oct + side).clamp(CUT_MIN, cut_max),
                        res: (res[f] + t * dres).clamp(0.0, 1.0),
                        drive: drive[f],
                        vowel: (vowel[f] + t * dvow).clamp(0.0, 4.0),
                    };
                }
            }
            let mut y = [0.0f32; 2];
            for (ch, (raw, x)) in [(l[i], xl), (r[i], xr)].into_iter().enumerate() {
                let dry = self.delay_dry(ch, raw);
                let a = voices[0][ch];
                let b = voices[1][ch];
                let wet = self.wet(ch, x, &a, b_on.then_some(&b));
                // At mix 0 (gliding out, until the block ends and the filters stop) exactly
                // what the unit gives once they have: the dry, its zeros' signs and all.
                y[ch] = if mix == 0.0 {
                    if out == 1.0 {
                        dry
                    } else {
                        dry * out
                    }
                } else {
                    (dry + (wet - dry) * mix) * out
                };
            }
            self.dry_pos = (self.dry_pos + 1) % DRY_MAX;
            l[i] = y[0];
            r[i] = y[1];
            if i + 1 == n {
                let tl = &mut self.tel;
                tl[t::CUT_A_L] = voices[0][0].cut;
                tl[t::CUT_A_R] = voices[0][1].cut;
                tl[t::CUT_B_L] = voices[1][0].cut;
                tl[t::CUT_B_R] = voices[1][1].cut;
                tl[t::RES_A] = voices[0][0].res;
                tl[t::RES_B] = voices[1][0].res;
                tl[t::VOWEL_A] = voices[0][0].vowel;
                tl[t::VOWEL_B] = voices[1][0].vowel;
                tl[t::ENV] = env;
                tl[t::ENV_DB] = crate::dynfx_follow::to_db(level);
                tl[t::CURVE] = cur;
                tl[tel::MOD_L] = lfo_l;
                tl[tel::MOD_R] = lfo_r;
            }
        }
    }

    fn telemetry_block(&mut self, last: usize, in_peak: f32, l: &[f32], r: &[f32]) {
        self.blocks = (self.blocks + 1) % (1 << 24);
        let tempo = self.clock.tempo();
        let song = self.clock.song(last) as f64;
        let tl = &mut self.tel;
        tl[tel::SAMPLE_RATE] = self.sr;
        tl[tel::TEMPO] = tempo as f32;
        tl[tel::PLAYING] = if self.clock.playing() { 1.0 } else { 0.0 };
        let ph = self.lfo.phase(&self.clock, last).0;
        tl[tel::PHASE_L] = ph as f32;
        let st = f64::from(self.set.lfo.stereo) / 360.0;
        tl[tel::PHASE_R] = ((ph + st) - (ph + st).floor()) as f32;
        tl[tel::IN_PEAK] = peak_db(in_peak);
        tl[tel::OUT_PEAK] = peak_db(peak_of(l, r));
        tl[tel::BLOCKS] = self.blocks as f32;
        tl[t::CURVE_PHASE] = self.curve.phase(song, tempo) as f32;
        tl[t::ENGAGED] = if self.engaged { 1.0 } else { 0.0 };
        tl[t::OS] = self.factor as f32;
        tl[t::LATENCY] = self.latency as f32;
        if !self.engaged {
            // What the display shows at rest: the filters as set, unmoved.
            let s = self.set;
            tl[t::CUT_A_L] = s.a.cut;
            tl[t::CUT_A_R] = s.a.cut;
            tl[t::CUT_B_L] = s.b.cut;
            tl[t::CUT_B_R] = s.b.cut;
            tl[t::RES_A] = s.a.res;
            tl[t::RES_B] = s.b.res;
            tl[t::VOWEL_A] = s.a.vowel;
            tl[t::VOWEL_B] = s.b.vowel;
            tl[t::ENV] = 0.0;
            tl[t::ENV_DB] = -200.0;
            tl[t::CURVE] = 0.0;
            tl[tel::MOD_L] = 0.0;
            tl[tel::MOD_R] = 0.0;
        }
    }

    /// The telemetry: the family's header ([`tel`]) and Cribrum's own ([`t`]).
    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..32].copy_from_slice(&self.tel);
    }
}

#[cfg(test)]
#[path = "cribrum_tests.rs"]
mod tests;
