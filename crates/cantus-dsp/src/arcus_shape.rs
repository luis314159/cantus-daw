//! r42 arcus-classical — the stroke's shape (a child of `arcus`, the player's: ADR 0162's layer,
//! `docs/round42/arcus-classical.md`): how a player's bow moves *inside* a held note. Measured on
//! the HSD quartet's stems (`tools/arcus/expr/phrase_study.py`), a player's long note starts
//! 7–15 dB and ends 10–35 dB under its peak, and its peak falls at 0.3–0.5 of its length; Arcus's
//! started 2–5 dB and ended 3–4.5 dB under (an organ's note). The shape is the **bow's**, never
//! an amplitude envelope: a contour in dB moves the bow's speed (the force riding it, so the
//! bowing stays where it was in Schelleng's region: every limit is linear in the speed) and, for
//! a share of it, the dynamic the bowing is read at (the contact point and the force's place:
//! the tone opens as it swells, which is what makes the spectrum settle over the players'
//! 0.15–0.85 s `t_steady` rather than at once).
//!
//! A note's contour (all in dB, 0 the stroke's level):
//!
//! - **the rise** — from where the note begins to 0 over its rise: from silence, the depth and
//!   length the instrument's swell had (`InstrumentSpec::swell_s`, `swell_db`: the players' rise,
//!   pass 4's fit — the bow's now, the output's envelope retired with the player on); a bow
//!   change, a new stroke a few dB under (less in a fast passage); a slur, from where the last
//!   note had sagged to;
//! - **the sag** — then easing a few dB as the stroke goes on (a held bow loses a little as it
//!   travels; Maestre 2010's sustained contours fall after their peak);
//! - **the breath** — a slow wander of about a dB (a seeded walk, glided, one step every
//!   `breath_s`: the 0.5–3 Hz movements of a held bow);
//! - **the taper** — when the key is let go the bow does not lift at once: it slows and lightens
//!   (`taper_db` over a time tied to `bow.release`), then lifts.
//!
//! Every value is a hash of the seed, the key and the note's id (its own generator: the
//! vibrato's walk draws from the voice's), stepped on the control grid: the same bits at every
//! block size. With the player off nothing here runs.

use crate::fides::exciter::Rng;
use crate::math;

/// The shape's values (one set for every instrument; [`KNOBS`] the measured one). The tools may
/// try others (`ArcusSettings::shape_try`, never stored), as the match tool tries a match.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Knobs {
    /// The share of the contour's dB the bow's speed carries (the rest, the dynamic it is read
    /// at: the contact point and the force's place move with it, so the tone opens as it swells).
    pub speed_share: f32,
    /// From silence, the share of the instrument's swell depth (`InstrumentSpec::swell_db`) the
    /// bow starts under, and of its length (`swell_s`) the rise takes.
    pub fresh_db: f32,
    pub fresh_s: f32,
    /// A bow change's new stroke: how far under it starts (dB; × `fast_share` in a fast passage)
    /// and its rise (s).
    pub rebow_db: f32,
    pub rebow_s: f32,
    /// A fast passage's share of a bow change's depth.
    pub fast_share: f32,
    /// A slurred note's rise from where the last one had sagged, s.
    pub slur_s: f32,
    /// The sag after the rise: how far (dB) and its time constant (s).
    pub sag_db: f32,
    pub sag_s: f32,
    /// The breath: its size (dB, a walk of about a unit's spread) and its step (s).
    pub breath_db: f32,
    pub breath_s: f32,
    /// The taper after the key: how far the bow eases (dB) before it lifts, over `taper_k` ×
    /// `bow.release` (at its default 120 ms, 0.18 s).
    pub taper_db: f32,
    pub taper_k: f32,
    /// The vibrato's way back into a note inside a phrase (after a bow change or a slur that is
    /// not a fast passage's): held at `vib_floor` of itself until the first time (s), back in
    /// full by the second. The HSD players' long notes inside a phrase start their vibrato at
    /// 0.04–0.12 s (median; up to 0.25 s); Arcus's ran on from the last note, at 0 s.
    pub vib_back_s: (f32, f32),
    pub vib_floor: f32,
    /// The string's, not the player's (model 2; here so the tools can try it): how the bridge's
    /// corner (`InstrumentSpec::corner_hz`) moves with the bow's force against the same bowing's
    /// mf force — its power above and below (0: the corner fixed, as before).
    pub bright: (f32, f32),
}

/// The measured shape (`docs/round42/arcus-classical.md`).
pub const KNOBS: Knobs = Knobs {
    speed_share: 0.7,
    fresh_db: 1.0,
    fresh_s: 1.0,
    rebow_db: 6.0,
    rebow_s: 0.25,
    fast_share: 0.3,
    slur_s: 0.3,
    sag_db: 6.0,
    sag_s: 1.2,
    breath_db: 1.0,
    breath_s: 0.45,
    taper_db: 12.0,
    taper_k: 1.5,
    vib_back_s: (0.08, 0.38),
    vib_floor: 0.1,
    bright: (0.0, 0.0),
};

impl Knobs {
    /// The vibrato's factor `t` s into a note inside a phrase.
    pub fn vib_back(&self, t: f32) -> f32 {
        let (a, b) = self.vib_back_s;
        let u = smooth((t - a) / (b - a).max(1e-3));
        self.vib_floor + (1.0 - self.vib_floor) * u
    }

    /// A tool's knobs from text, `name=value,…` (unknown names and bad values ignored).
    pub fn parse(mut self, text: &str) -> Knobs {
        for kv in text.split(',') {
            let mut it = kv.splitn(2, '=');
            let (Some(k), Some(v)) = (it.next(), it.next()) else {
                continue;
            };
            let Ok(v) = v.trim().parse::<f32>() else {
                continue;
            };
            match k.trim() {
                "speed_share" => self.speed_share = v,
                "fresh_db" => self.fresh_db = v,
                "fresh_s" => self.fresh_s = v,
                "rebow_db" => self.rebow_db = v,
                "rebow_s" => self.rebow_s = v,
                "fast_share" => self.fast_share = v,
                "slur_s" => self.slur_s = v,
                "sag_db" => self.sag_db = v,
                "sag_s" => self.sag_s = v,
                "breath_db" => self.breath_db = v,
                "breath_s" => self.breath_s = v,
                "taper_db" => self.taper_db = v,
                "taper_k" => self.taper_k = v,
                "vib_back_a" => self.vib_back_s.0 = v,
                "vib_back_b" => self.vib_back_s.1 = v,
                "vib_floor" => self.vib_floor = v,
                "bright_hi" => self.bright.0 = v,
                "bright_lo" => self.bright.1 = v,
                _ => {}
            }
        }
        self
    }
}

/// The seed's twist that gives the shape its own generator.
const SEED_TWIST: u32 = 0x5A3E_0C11;

/// How a note begins.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Begin {
    /// From silence: the instrument's swell, `(depth dB, length s)`.
    Fresh(f32, f32),
    /// A bow change on a ringing string.
    Rebow,
    /// A slur: the bow goes on.
    Slur,
}

/// How much level an instrument's bowing spans from pp to ff, dB (the bridge's velocity goes as
/// `v/β`: the cello's 0.15 → 0.27 m/s and β 0.14 → 0.07, 11 dB).
pub fn span_db(spec: &super::tables::InstrumentSpec) -> f32 {
    let b = &spec.bowing;
    (20.0 * math::log10((b.speed.1 / b.beta.1) / (b.speed.0 / b.beta.0))) as f32
}

/// One voice's stroke shape.
#[derive(Copy, Clone, Debug)]
pub struct Shape {
    /// The contour now, dB (0 the stroke's level), the player's amount in it.
    pub l: f32,
    amount: f32,
    k: Knobs,
    from: f32,
    rise: f32,
    t: f32,
    breath: (f32, f32),
    b_t: f32,
    rng: Rng,
    /// The taper: the level it started from, the time into it and its length (s).
    taper: Option<(f32, f32, f32)>,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            l: 0.0,
            amount: 0.0,
            k: KNOBS,
            from: 0.0,
            rise: 0.0,
            t: 0.0,
            breath: (0.0, 0.0),
            b_t: 0.0,
            rng: Rng(1),
            taper: None,
        }
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Shape {
    /// A note begins (`amount` the player's, 0 none; `fast` a fast passage's).
    #[allow(clippy::too_many_arguments)]
    pub fn begin(
        &mut self,
        how: Begin,
        k: &Knobs,
        amount: f32,
        fast: bool,
        seed: u32,
        key: u8,
        id: u64,
    ) {
        let amount = amount.clamp(0.0, 1.0);
        let was = self.l;
        let fresh = matches!(how, Begin::Fresh(..));
        let breath_was = self.breath_now();
        self.k = *k;
        if fresh || self.amount == 0.0 {
            self.rng = Rng::of(seed ^ SEED_TWIST, key, id);
            self.breath = (0.0, self.rng.bipolar());
            self.b_t = 0.0;
        }
        self.amount = amount;
        self.taper = None;
        self.t = 0.0;
        let (from, rise) = match how {
            Begin::Fresh(depth, len) => (-depth * k.fresh_db, len * k.fresh_s),
            Begin::Rebow => (
                -k.rebow_db * if fast { k.fast_share } else { 1.0 },
                if fast { k.rebow_s * 0.5 } else { k.rebow_s },
            ),
            // A slur from where the last note sagged to (a note from silence never slurs).
            Begin::Slur => ((was / amount.max(1e-6) - breath_was).min(0.0), k.slur_s),
        };
        self.from = from;
        self.rise = rise.max(1e-3);
        self.l = if amount == 0.0 {
            0.0
        } else {
            amount * (from + self.breath_now())
        };
    }

    /// The key is let go: the taper, over `len` s.
    pub fn taper(&mut self, len: f32) {
        if self.amount > 0.0 && self.taper.is_none() {
            self.taper = Some((self.l, 0.0, len.max(1e-3)));
        }
    }

    /// Whether the taper has run its course (the bow lifts now).
    pub fn tapered(&self) -> bool {
        self.taper.is_some_and(|(_, t, len)| t >= len)
    }

    /// The knobs this note was begun with.
    pub fn knobs(&self) -> &Knobs {
        &self.k
    }

    fn breath_now(&self) -> f32 {
        let u = smooth(self.b_t / self.k.breath_s.max(1e-3));
        self.k.breath_db * (self.breath.0 + (self.breath.1 - self.breath.0) * u)
    }

    /// One control step of `dt` seconds: the contour at its end, dB.
    pub fn step(&mut self, dt: f32) -> f32 {
        if self.amount == 0.0 {
            self.l = 0.0;
            return 0.0;
        }
        self.t += dt;
        self.b_t += dt;
        let bs = self.k.breath_s.max(1e-3);
        if self.b_t >= bs {
            self.b_t -= bs;
            // A walk whose steps keep about a unit's spread (uniform ±√3, half kept).
            let next = 0.5 * self.breath.1 + 0.866 * self.rng.bipolar();
            self.breath = (self.breath.1, next.clamp(-1.8, 1.8));
        }
        if let Some((from, t, len)) = self.taper.as_mut() {
            *t += dt;
            self.l = *from - self.amount * self.k.taper_db * smooth(*t / *len);
            return self.l;
        }
        let rise = self.from * (1.0 - smooth(self.t / self.rise));
        let after = (self.t - self.rise).max(0.0);
        let sag = -self.k.sag_db * (1.0 - math::exp(-after / self.k.sag_s.max(1e-3)));
        self.l = self.amount * (rise + sag + self.breath_now());
        self.l
    }

    /// The bow's speed factor and the dynamic's offset for the contour now, on an instrument
    /// whose bowing spans `span_db` of level from pp to ff, at the dynamic `d`.
    pub fn bowing(&self, d: f32, span_db: f32) -> (f32, f32) {
        if self.l == 0.0 {
            return (1.0, 0.0);
        }
        let want = (1.0 - self.k.speed_share) * self.l / span_db.max(1.0);
        let dd = (d + want).clamp(0.0, 1.0) - d;
        let carried = dd * span_db.max(1.0);
        let kv = math::db_to_gain(self.l - carried);
        (kv, dd)
    }
}
