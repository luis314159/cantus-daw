//! r42 fiddle (ADR 0164) — one bow on two strings (a child of `arcus::voice`): a voice's
//! second string, bowed by the same gesture as its first (the same speed, the same hair's
//! noise, the same strokes and bow changes to the sample), its force its own stroke's split
//! by the bow's tilt; or left ringing when the bow crosses to another string.
//!
//! **The two slots.** The *first* string is the one the allocator's note is on (or, after
//! its note ended under a held drone, the drone's: the voice is then an *orphan*, its note
//! one the allocator never saw). The *second* holds a double stop's or a drone's note
//! (`held`), or the string the bow last left, ringing free (`on`, not `held`). A note on the
//! second's string makes the two change places (the ringing string is bowed again); a note
//! on a third string takes the first's string afresh when a drone holds the second, and
//! otherwise leaves the first ringing in the second's place.
//!
//! **Null.** With `pair` off nothing here runs: no second string is ever on, the first's
//! force share stays exactly 1 and is never multiplied in.

use super::*;

/// Cents from the tempered interval to the just ratio, by semitones above the lower note
/// (16/15, 9/8, 6/5, 5/4, 4/3, 45/32, 3/2, 8/5, 5/3, 16/9, 15/8): a double stop's tuning
/// at `just` 1, the stopped string tuned to the open one.
pub const JUST_CT: [f32; 12] = [
    0.0, 11.73, 3.91, 15.64, -13.69, -1.96, -9.78, 1.96, 13.69, -15.64, -3.91, -11.73,
];

/// How fast a string's share of the bow's force follows the tilt, s (a player's bow arm
/// leans onto a string or off it in about 10 ms).
const TILT_S: f32 = 0.008;

/// A voice's second string and its note.
#[derive(Clone, Debug)]
pub struct Second {
    pub(super) string: BowedString,
    /// It holds a string, bowed or ringing.
    pub(super) on: bool,
    /// Its note is held: the bow is on it.
    pub(super) held: bool,
    /// Its string starts afresh at the next step (a string the voice had not been on).
    pub(super) fresh: bool,
    pub(super) key: u8,
    pub(super) id: u64,
    pub(super) velocity: f32,
    pub(super) note: f32,
    pub(super) tune: f32,
    pub(super) tuning: f32,
    pub(super) glide: f32,
    pub(super) glide_step: f32,
    pub(super) string_ix: usize,
    pub(super) open: bool,
    pub(super) trim: f64,
    pub(super) ear: super::super::ear::Ear,
    pub(super) f0: f64,
    pub(super) balance: f32,
    /// r42 arcus-classical — the player's intonation on this string (`shape::intonation`):
    /// its note's error and tendency (cents) and the time into it (s), as the first string's
    /// `Voice::into`; it changes places with the first's at a crossing (a string left ringing
    /// keeps the pitch it was played at).
    pub(super) into: (f32, f32, f32),
    /// Its share of the bow's force at the last step's end and this one's.
    pub(super) w: (f32, f32),
    /// The bow on it in this step.
    pub(super) bowed: bool,
    pub(super) fbuf: [f32; SPAN],
    pub(super) out: [f32; SPAN],
}

impl Second {
    pub fn new(sample_rate: f32) -> Self {
        Second {
            string: BowedString::with_reach(sample_rate, reach(sample_rate)),
            on: false,
            held: false,
            fresh: false,
            key: 60,
            id: 0,
            velocity: 0.0,
            note: 60.0,
            tune: 0.0,
            tuning: 0.0,
            glide: 0.0,
            glide_step: 0.0,
            string_ix: 0,
            open: false,
            trim: 0.0,
            ear: super::super::ear::Ear::default(),
            f0: 261.6,
            balance: 1.0,
            into: (0.0, 0.0, 0.0),
            w: (0.0, 0.0),
            bowed: false,
            fbuf: [0.0; SPAN],
            out: [0.0; SPAN],
        }
    }

    pub(super) fn clear(&mut self) {
        self.on = false;
        self.held = false;
        self.fresh = false;
        self.bowed = false;
        self.w = (0.0, 0.0);
        self.glide = 0.0;
        self.glide_step = 0.0;
        self.into = (0.0, 0.0, 0.0);
        self.string.clear();
    }
}

/// Zeros for the second string's strike (it is never plucked).
static ZERO: [f32; SPAN] = [0.0; SPAN];

impl Voice {
    /// Whether one bow plays two strings on this voice (`pair` on).
    pub(super) fn paired(s: &ArcusSettings) -> bool {
        s.pair >= 0.5
    }

    /// The second string's note is this one.
    pub fn second_is(&self, key: u8, id: u64) -> bool {
        self.active && self.sec.on && self.sec.held && self.sec.key == key && self.sec.id == id
    }

    /// The second string's fundamental now, when it holds one.
    pub fn second_f0(&self) -> Option<f64> {
        (self.active && self.sec.on).then_some(self.sec.f0)
    }

    /// The second string holds note `id` (bowed or ringing).
    pub fn second_is_id(&self, id: u64) -> bool {
        self.active && self.sec.on && self.sec.id == id
    }

    /// The first string's note is one the allocator never saw (a drone left under one that
    /// ended) and it is this one.
    pub fn orphan_is(&self, key: u8, id: u64) -> bool {
        self.active && self.orphan && !self.released && self.key == key && self.id == id
    }

    /// The strings this voice holds now (bowed or ringing): the first's and the second's,
    /// as a mask of the instrument's four.
    pub fn strings_held(&self) -> u8 {
        if !self.active {
            return 0;
        }
        let mut m = 1u8 << self.string_ix.min(3);
        if self.sec.on {
            m |= 1u8 << self.sec.string_ix.min(3);
        }
        m
    }

    /// The two strings change places (their notes and pitch loops with them).
    pub(super) fn swap_roles(&mut self) {
        use std::mem::swap;
        let c = &mut self.sec;
        swap(&mut self.string, &mut c.string);
        swap(&mut self.key, &mut c.key);
        swap(&mut self.id, &mut c.id);
        swap(&mut self.velocity, &mut c.velocity);
        swap(&mut self.note, &mut c.note);
        swap(&mut self.tune, &mut c.tune);
        swap(&mut self.tuning, &mut c.tuning);
        swap(&mut self.glide, &mut c.glide);
        swap(&mut self.glide_step, &mut c.glide_step);
        swap(&mut self.string_ix, &mut c.string_ix);
        swap(&mut self.open, &mut c.open);
        swap(&mut self.trim, &mut c.trim);
        swap(&mut self.ear, &mut c.ear);
        swap(&mut self.f0, &mut c.f0);
        swap(&mut self.balance, &mut c.balance);
        swap(&mut self.into, &mut c.into); // r42 arcus-classical
        swap(&mut self.w1, &mut c.w);
        swap(&mut self.restring, &mut c.fresh);
    }

    /// The string a bowed note of key `note` (semitones, tuned) is played on.
    pub(super) fn string_of(&self, s: &ArcusSettings, note: f32, how: &NoteHow) -> usize {
        let spec = self.spec();
        if s.player > 0.0 {
            bow::finger(spec, note, how.string, true, None).string
        } else {
            spec.string_for(note)
        }
    }

    /// A new note of the allocator's goes on string `ix`: the two strings placed so that the
    /// first is on it (the module's notes). Returns whether the first string starts afresh.
    pub(super) fn place(&mut self, ix: usize) -> bool {
        if ix == self.string_ix {
            // The same string: a re-bow or a slur, as without a pair (a drone there gives way).
            self.orphan = false;
            return false;
        }
        // The first string's note, still held under the new one, goes on bowed (a drone, or a
        // crossing's overlap until its key is let go); one let go rings.
        let drone = !self.released;
        if self.sec.on && self.sec.string_ix == ix {
            // The string still ringing (or a drone's): bowed again.
            self.swap_roles();
            self.sec.on = true;
            self.sec.held = drone;
            self.orphan = false;
            self.released = false;
            return false;
        }
        if self.sec.held {
            // A drone on a third string stays: the first string is taken to the new one.
            self.string_ix = ix;
            self.orphan = false;
            self.restring = true;
            return true;
        }
        // The string the bow leaves rings on; the new one starts from rest.
        self.swap_roles();
        self.sec.on = true;
        self.sec.held = drone;
        self.sec.bowed = false;
        self.orphan = false;
        self.released = false;
        self.string_ix = ix;
        self.restring = true;
        true
    }

    /// A note that starts with the voice's (the same sample) on another string joins it as a
    /// double stop under the same bow. Returns `false` when it cannot (the same string and no
    /// string beside it reaches, a voice not bowed): the caller plays it as without a pair.
    pub fn join(&mut self, sh: &Shared, key: u8, id: u64, velocity: f32, how: &NoteHow) -> bool {
        let s = sh.s;
        if !self.active
            || self.released
            || !Self::paired(s)
            || !matches!(
                self.tech,
                NoteTech::Arco | NoteTech::Staccato | NoteTech::Chop
            )
            || self.sec.held
        {
            return false;
        }
        let sh_rate = self.at_rate(sh);
        let sh = &sh_rate;
        let spec = self.spec();
        let tune = crate::tuning::at(sh.tuning, key);
        let note = key as f32 + s.transpose_st + 0.01 * s.fine_ct;
        let mut ix = self.string_of(s, note + tune, how);
        let reaches = |st: usize, k: f32| {
            let o = spec.strings[st].open as f32;
            k + 1e-3 >= o && k <= o + bow::REACH + 1e-3
        };
        let mine = self.note + self.tune;
        if ix == self.string_ix {
            // A unison against an open string, or two notes the first position puts on one
            // string: the lower one stopped on the string below.
            if note + tune <= mine && ix > 0 && reaches(ix - 1, note + tune) {
                ix -= 1;
            } else if self.first && self.string_ix > 0 && reaches(self.string_ix - 1, mine) {
                self.string_ix -= 1;
                self.open = (mine - spec.strings[self.string_ix].open as f32).abs() < 0.05;
            } else {
                return false;
            }
        }
        if ix == self.string_ix {
            return false;
        }
        // r42 arcus-classical: the second note's own intonation (`shape::intonation`), its
        // semitone step's tendency from the note this string last held (none from rest).
        let p_shape = super::super::shape::classical(s.player, super::super::shape::fiddler(s));
        let into = if p_shape > 0.0 {
            let knobs = s.shape_try.unwrap_or(super::super::shape::KNOBS);
            let step = if self.sec.on {
                note + tune - (self.sec.note + self.sec.tune)
            } else {
                0.0
            };
            let (e, tend) =
                super::super::shape::intonation(&knobs, p_shape, step, false, s.seed, key, id);
            (e, tend, 0.0)
        } else {
            (0.0, 0.0, 0.0)
        };
        let c = &mut self.sec;
        c.into = into;
        let was = c.note + c.glide + c.tune;
        let ringing = c.on && c.string_ix == ix;
        if !ringing {
            c.string.clear();
            c.fresh = true;
            c.trim = 0.0;
            c.ear.reset();
        }
        c.on = true;
        c.held = true;
        c.key = key;
        c.id = id;
        c.velocity = velocity.clamp(0.0, 1.0);
        c.note = note;
        c.tune = tune;
        c.tuning = 0.0;
        c.string_ix = ix;
        c.open = (note + tune - spec.strings[ix].open as f32).abs() < 0.05;
        c.glide = 0.0;
        c.glide_step = 0.0;
        let finger = super::run::REBOW_FINGER_MS * 0.001 * sh.sr;
        if ringing && finger >= 1.0 {
            c.glide = was - c.note - c.tune;
            c.glide_step = c.glide / finger;
        }
        // Struck together, the two strings share the bow's force at once.
        let (lo, hi) = self.tilt_shares(s);
        let w = if ix < self.string_ix { lo } else { hi };
        self.sec.w = (w, w);
        self.w1 = if ix < self.string_ix {
            (hi, hi)
        } else {
            (lo, lo)
        };
        true
    }

    /// The lower and the upper string's shares of the bow's force in a double stop: the
    /// tilt, leaning by the two notes' velocities.
    fn tilt_shares(&self, s: &ArcusSettings) -> (f32, f32) {
        let (v_lo, v_hi) = if self.sec.string_ix < self.string_ix {
            (self.sec.velocity, self.velocity)
        } else {
            (self.velocity, self.sec.velocity)
        };
        let t = (s.tilt + 0.5 * (v_lo - v_hi)).clamp(0.0, 1.0);
        ((2.0 * t).min(1.0), (2.0 * (1.0 - t)).min(1.0))
    }

    /// The double stop's note is let go: the bow leans off its string, which rings on.
    pub fn release_second(&mut self) {
        self.sec.held = false;
    }

    /// The allocator's note is let go (`note_off`): under a held drone the drone takes the
    /// bow on (the voice an orphan, the released note's string ringing); otherwise the bow
    /// lifts as without a pair.
    pub fn release_note(&mut self) {
        if self.active && !self.released && self.sec.on && self.sec.held {
            self.swap_roles();
            self.sec.held = false;
            self.orphan = true;
            return;
        }
        self.release();
    }

    /// The allocator's sounding note is let go and it goes back to `id`, still held: when the
    /// second string holds it, the two change places (the let-go note's string rings).
    pub fn release_to(&mut self, id: u64) {
        if self.active && self.sec.on && self.sec.held && self.sec.id == id {
            self.swap_roles();
            self.sec.held = false;
            self.orphan = false;
        }
    }

    /// A per-note expression on the double stop's note (only its tuning is its own).
    pub fn expression_second(&mut self, which: u8, value: f32) {
        if which == 0 {
            self.sec.tuning = value;
        }
    }

    /// The just intonation of a double stop, cents on the first string and the second.
    pub(super) fn just_split(&self, s: &ArcusSettings) -> (f64, f64) {
        if s.just <= 0.0 || !self.sec.on || !self.sec.held || self.released {
            return (0.0, 0.0);
        }
        let a = self.note + self.tune + self.tuning;
        let b = self.sec.note + self.sec.tune + self.sec.tuning;
        let (lo_open, hi_open, up) = if a <= b {
            (self.open, self.sec.open, b - a)
        } else {
            (self.sec.open, self.open, a - b)
        };
        let n = (up.round().max(0.0) as usize) % 12;
        let dev = (JUST_CT[n] * s.just.clamp(0.0, 1.0)) as f64;
        // The stopped string moves to the open one; two stopped strings meet halfway.
        let (d_lo, d_hi) = match (lo_open, hi_open) {
            (true, true) => (0.0, 0.0),
            (true, false) => (0.0, dev),
            (false, true) => (-dev, 0.0),
            (false, false) => (-0.5 * dev, 0.5 * dev),
        };
        if a <= b {
            (d_lo, d_hi)
        } else {
            (d_hi, d_lo)
        }
    }

    /// The force shares' step: the first's and the second's follow the tilt (or 1 and 0
    /// when only the first is bowed), glided over `TILT_S`.
    pub(super) fn step_shares(&mut self, s: &ArcusSettings, dt: f32) {
        let both = self.sec.on && self.sec.held && !self.released;
        let (t1, t2) = if both {
            let (lo, hi) = self.tilt_shares(s);
            if self.sec.string_ix < self.string_ix {
                (hi, lo)
            } else {
                (lo, hi)
            }
        } else {
            (1.0, 0.0)
        };
        let c = 1.0 - math::exp(-dt / TILT_S);
        let glide = |w: f32, t: f32| {
            let x = w + (t - w) * c;
            if (x - t).abs() < 1e-4 {
                t
            } else {
                x
            }
        };
        self.w1 = (self.w1.1, glide(self.w1.1, t1));
        self.sec.w = (self.sec.w.1, glide(self.sec.w.1, t2));
    }
}

impl Voice {
    /// The second string's control step (after the first's: the bow's speed and force
    /// buffers are the first's, unscaled): its pitch, its share of the force, its design.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn step_second(
        &mut self,
        sh: &Shared,
        span: usize,
        bw: &Bowing,
        pos: f32,
        spd: f32,
        just: f64,
    ) {
        let s = sh.s;
        let sr = sh.sr;
        let spec = self.spec();
        let c = &mut self.sec;
        let st = spec.strings[c.string_ix];
        if c.glide_step != 0.0 {
            let g = c.glide - c.glide_step * span as f32;
            if g * c.glide <= 0.0 {
                c.glide = 0.0;
                c.glide_step = 0.0;
            } else {
                c.glide = g;
            }
        }
        let now = sh.tuning[(c.key as usize).min(127)];
        if now.is_finite() {
            c.tune = now;
        }
        let mut semis = (c.note + c.glide + sh.ctl.bend * s.bend_st + c.tuning) as f64
            + c.tune as f64
            + 0.01 * just;
        // r42 arcus-classical: its own intonation, as the first string's (`Voice::into`).
        if c.into.0 != 0.0 || c.into.1 != 0.0 {
            let dt = span as f32 / sr;
            c.into.2 += dt;
            let knobs = s.shape_try.unwrap_or(super::super::shape::KNOBS);
            let k = super::super::shape::intonation_at(&knobs, c.into.0, c.into.1, c.into.2);
            let land = (c.into.2 / (0.001 * s.finger_ms.max(1.0))).clamp(0.0, 1.0);
            let land = land * land * (3.0 - 2.0 * land);
            semis += 0.01 * (k * land) as f64;
        }
        // One hand: a stopped second string vibrates with the first; an open one cannot.
        let vib = if c.open { 0.0 } else { self.vib_now };
        let f = 440.0 * math::exp2((semis + 0.01 * vib as f64 - 69.0) / 12.0);
        c.f0 = f;
        c.balance = math::db_to_gain(spec.balance_db_st * (semis as f32 - spec.balance_key));
        let stopped = (st.open_hz() / f).clamp(0.1, 1.0);
        let mut t60 = st.t60_s * s.decay.max(0.05) as f64 * (0.55 + 0.45 * stopped);
        t60 *= steel_t60(s);
        if !c.held || self.released {
            t60 = t60.min(s.ring_s.max(0.05) as f64);
        }
        let bowed = self.bow_on && (c.w.0 > 0.0 || c.w.1 > 0.0);
        c.bowed = bowed;
        let bw2 = bowing(
            spec,
            self.d as f64,
            pos as f64,
            s.bow_force as f64 + super::super::upper::force_trim(self.inst, st.open_hz(), f),
            (s.bow_speed * spd) as f64
                * super::super::upper::speed_scale(self.inst, st.open_hz(), f),
            st.z0(),
            sr as f64 / f,
        );
        // The same bow at the same distance from the bridge: β on this string the first's
        // over the two stopped lengths (a longer string's β is smaller).
        let st1 = spec.strings[self.string_ix];
        let len1 = st1.open_hz() / self.f0.max(1.0);
        let len2 = st.open_hz() / f;
        let beta2 = (bw.beta * len1 / len2).clamp(min_beta(sr as f64 / f), MAX_BETA);
        if bowed {
            // The same bow at the same speed: the force its own stroke's (Schelleng's limit
            // goes with the string's impedance, the speed and 1/β), its share by the tilt.
            let k = ((bw2.force / bw2.speed.max(1e-9)) / (bw.force / bw.speed.max(1e-9))
                * (bw2.beta / beta2)) as f32;
            for j in 0..span {
                let u = (j + 1) as f32 / span as f32;
                let w = c.w.0 + (c.w.1 - c.w.0) * u;
                c.fbuf[j] = self.fbuf[j] * k * w;
            }
        }
        if bowed && sh.table.is_some() {
            if let Some(heard) = c.ear.take() {
                c.trim = (c.trim + (0.6 * heard).clamp(-2.0, 2.0)).clamp(-15.0, 15.0);
            }
            c.ear.aim(f / sr as f64, sr as f64);
        } else {
            c.ear.reset();
        }
        let corr = if bowed {
            sh.table.map_or(0.0, |t| {
                t.cents(self.inst, semis, self.d as f64, pos as f64) + c.trim
            })
        } else {
            0.0
        };
        let step = StringStep {
            f0: f * math::exp2(-corr / 1200.0),
            t60,
            p: st.p * steel_p(s),
            beta: beta2,
            dc: !bowed,
            corner_hz: super::super::upper::corner_hz(self.inst, spec.corner_hz, f)
                * steel_corner(s),
        };
        if c.fresh {
            c.string.start(st.z0() as f32, &step, false);
            c.fresh = false;
        } else {
            c.string.set_z0(st.z0() as f32);
            c.string.design(&step, span as u32);
        }
    }

    /// The second string's samples `a..b` of this step, **added** into `tmp[..b − a]`.
    pub(super) fn render_second(&mut self, sh: &Shared, a: usize, b: usize, grid_end: bool) {
        let k = b - a;
        let friction = self.spec().friction(sh.s.friction);
        let c = &mut self.sec;
        c.out[..k].fill(0.0);
        let bow = if c.bowed {
            Some(Bow {
                v: &self.v[a..b],
                force: &c.fbuf[a..b],
                rough: &self.roughs[a..b],
                friction,
            })
        } else {
            None
        };
        let gain = 2.0 * c.string.z0() * OUT * c.balance;
        c.string
            .render(&ZERO[..k], bow, gain, &mut c.out[..k], None, grid_end);
        if c.bowed {
            c.ear.hear(&c.out[..k]);
        }
        for j in 0..k {
            self.tmp[j] += c.out[j];
        }
        if grid_end && !c.held && !c.string.is_live() {
            c.on = false;
        }
    }
}

/// r42 fiddle — steel strings' factors (`steel` 0: exactly 1): the bridge's corner raised
/// (a steel string's corner is sharper: less bending stiffness and internal loss at the
/// top), the upper partials' loss exponent lowered, the ring longer.
pub(super) fn steel_corner(s: &ArcusSettings) -> f64 {
    if s.steel <= 0.0 {
        1.0
    } else {
        math::exp2(STEEL_CORNER_OCT * s.steel.clamp(0.0, 1.0) as f64)
    }
}
pub(super) fn steel_p(s: &ArcusSettings) -> f64 {
    if s.steel <= 0.0 {
        1.0
    } else {
        1.0 - STEEL_P * s.steel.clamp(0.0, 1.0) as f64
    }
}
pub(super) fn steel_t60(s: &ArcusSettings) -> f64 {
    if s.steel <= 0.0 {
        1.0
    } else {
        1.0 + STEEL_T60 * s.steel.clamp(0.0, 1.0) as f64
    }
}

/// The corner raised by this many octaves at `steel` 1, the loss exponent lowered by this
/// share, the ring lengthened by it.
pub const STEEL_CORNER_OCT: f64 = 1.3;
pub const STEEL_P: f64 = 0.4;
pub const STEEL_T60: f64 = 0.5;
