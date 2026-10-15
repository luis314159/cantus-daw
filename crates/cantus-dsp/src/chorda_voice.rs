//! One sounding key of Chorda: its strings struck, its damper rising and falling with the
//! key and the pedal, and the phantom partials of its strings' tension.
//!
//! **The damper** rests on the strings with a contact between 0 (lifted) and 1 (down),
//! which the key (held: lifted) and the sustain pedal set: the pedal lifts the dampers
//! progressively over its travel (`1 − smoothstep(0.15, 0.65, pedal)`), so half-pedalling
//! is continuous. The contact follows its target over ~8 ms (felt does not land in a
//! sample) and adds `contact · σ_d` to every partial's decay rate, `σ_d` the felt's grip:
//! a damped string dies in about 1.5 s at A0, 0.3 s at C4 and 0.15 s at E6. Keys above E6
//! have no damper, as on a grand.
//!
//! **Phantom partials** (Bank & Sujbert 2005): a string's tension rises with the square of
//! its slope, `ΔT ≈ (EA / 2c²) v²` for the transverse velocity `v`; the square holds the
//! sums and differences of the partials, which the tension carries to the bridge, through
//! the string's first longitudinal mode (`c_L / 2L`, steel's `c_L` = 5 100 m/s) and
//! directly. `EA` from the wire's diameter, itself from `B = π³ E d⁴ / (64 T L²)`.

use super::design::LoopDesign;
use super::string::{loss, Felt, KeyStrings, MAX_STRINGS};
use super::tables::Key;
use crate::math;

/// The control step (samples): the damper and the loss filter move at this rate, counted
/// from the voice's own start.
pub const CONTROL: u32 = 32;
/// Control steps of near-silence (and no hammer) after which a voice ends.
const QUIET_STEPS: u32 = 64;
/// Below this force (N) a step is quiet.
const QUIET_N: f64 = 1.0e-10;
/// Steel: Young's modulus (Pa) and longitudinal wave speed (m/s).
const STEEL_E: f64 = 2.0e11;
const STEEL_CL: f64 = 5_100.0;
/// How much of the tension's swing reaches the bridge as a vertical force (the bridge's
/// rocking under the strings' pull): a tenth.
const LONGITUDINAL_COUPLING: f64 = 0.1;

/// What a strike and a voice's life take from the instrument's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceSettings {
    pub felt: Felt,
    /// Strike velocity range (m/s) and the curve (1: exponential in the MIDI velocity).
    pub v_min: f64,
    pub v_max: f64,
    pub vel_curve: f64,
    /// The decay against the measured one (> 1: longer), the bridge's share of `b₁`.
    pub decay: f64,
    pub coupling: f64,
    /// The unison strings' spread (cents) and the damper's grip (0…1, 1: a grand's).
    pub detune_ct: f64,
    pub damper: f64,
    /// The phantom partials' level (1: the physics').
    pub phantom: f64,
    /// The soft pedal's reach (0…1) and how far it is pressed now (0…1).
    pub una_depth: f64,
}

/// A damped string's T60 at its first partial (s), by key: interpolated on a log scale.
pub fn damped_t60(m: f64) -> f64 {
    let pts = [(21.0, 1.5), (60.0, 0.3), (88.0, 0.15)];
    let i = if m < 60.0 { 0 } else { 1 };
    let (x0, y0) = pts[i];
    let (x1, y1) = pts[i + 1];
    let t = ((m - x0) / (x1 - x0)).clamp(-1.0, 2.0);
    math::exp(math::ln(y0) + t * (math::ln(y1) - math::ln(y0)))
}

/// The damper's contact a pedal position leaves on a released key (1: fully down).
pub fn pedal_contact(pedal: f64) -> f64 {
    let t = ((pedal - 0.15) / 0.5).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// A small, fixed variation per key (−1…1): hashed from the key, never random.
fn key_hash(key: u8) -> f64 {
    let mut z = (key as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xC40D_A5EED;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

#[derive(Clone, Debug)]
pub struct Voice {
    pub key: Key,
    strings: KeyStrings,
    pub active: bool,
    pub key_down: bool,
    pub id: u64,
    /// The damper's contact now and where it goes.
    contact: f64,
    target: f64,
    /// The loss filter's grip as last set (to skip setting it again).
    set_contact: f64,
    fs: f64,
    /// The damper's glide per control step (~8 ms).
    glide: f64,
    settings: VoiceSettings,
    /// The phantom path: the tension's coefficient, its DC blocker and the longitudinal
    /// mode's resonator.
    tension: f64,
    dc_x: f64,
    dc_y: f64,
    res: [f64; 2],
    res_b: [f64; 3],
    phase: u32,
    quiet: u32,
    step_peak: f64,
    /// The design the strings were laid out for (its generation in the instrument).
    pub design_gen: u64,
    /// A strike for a newer keyboard while the strings still ring for an older one: they
    /// fade out over [`RETUNE_FADE`] samples, then the new strings are laid and struck
    /// (sound review 2: a key struck again while it rang kept the old tuning for good).
    retune: Option<Retune>,
}

/// Samples a ringing voice takes to fade out before a strike for a newer keyboard.
pub const RETUNE_FADE: u32 = 128;

/// A strike waiting for the old strings to fade (everything [`Voice::strike`] takes).
#[derive(Clone, Copy, Debug)]
struct Retune {
    key: Key,
    design: LoopDesign,
    gen: u64,
    settings: VoiceSettings,
    vel: f64,
    soft: f64,
    id: u64,
    /// Samples of the fade left.
    left: u32,
    /// Let go before the strike landed: the pedal it was let go at.
    released: Option<f64>,
}

impl Voice {
    /// A voice for `key`, with strings for loops up to `capacity` samples. Allocates.
    pub fn new(key: Key, capacity: usize, fs: f64) -> Voice {
        let d = ((64.0 * key.impedance * key.speed * key.length * key.length * key.b)
            / (std::f64::consts::PI * std::f64::consts::PI * std::f64::consts::PI * STEEL_E))
            .max(0.0);
        let diameter = math::powf(d, 0.25).max(2.0e-4);
        let area = std::f64::consts::PI * diameter * diameter / 4.0;
        let tension = STEEL_E * area / (2.0 * key.speed * key.speed);
        // The first longitudinal mode, a resonator with Q 30.
        let fl = (STEEL_CL / (2.0 * key.length)).min(0.45 * fs);
        let w = std::f64::consts::TAU * fl / fs;
        let r = math::exp(-w / (2.0 * 30.0));
        let res_b = [-2.0 * r * math::cos(w), r * r, 1.0 - r];
        Voice {
            key,
            strings: KeyStrings::new(capacity, fs),
            active: false,
            key_down: false,
            id: 0,
            contact: 1.0,
            target: 1.0,
            set_contact: -1.0,
            fs,
            glide: 1.0 - math::exp(-(CONTROL as f64) / (0.008 * fs)),
            settings: VoiceSettings {
                felt: Felt {
                    k: key.hammer_k,
                    p: key.hammer_p,
                    eps: 0.9,
                    tau: 1.0e-5,
                },
                v_min: 0.35,
                v_max: 5.5,
                vel_curve: 1.0,
                decay: 1.0,
                coupling: 0.5,
                detune_ct: 1.0,
                damper: 1.0,
                phantom: 1.0,
                una_depth: 1.0,
            },
            tension,
            dc_x: 0.0,
            dc_y: 0.0,
            res: [0.0; 2],
            res_b,
            phase: 0,
            quiet: 0,
            step_peak: 0.0,
            design_gen: 0,
            retune: None,
        }
    }

    /// Whether this voice's strings can hold `design`'s loop.
    pub fn fits(&self, design: &LoopDesign) -> bool {
        self.strings.fits(design)
    }

    /// Strike the key at MIDI velocity `vel` (0…1), the soft pedal at `soft`.
    #[allow(clippy::too_many_arguments)]
    pub fn strike(
        &mut self,
        key: Key,
        design: &LoopDesign,
        design_gen: u64,
        settings: VoiceSettings,
        vel: f64,
        soft: f64,
        id: u64,
    ) {
        if self.active && self.design_gen != design_gen {
            // Strings ringing for an older keyboard: they fade, then this strike lands on
            // the new ones (the newest strike wins if several come meanwhile).
            let left = self.retune.map_or(RETUNE_FADE, |r| r.left);
            self.retune = Some(Retune {
                key,
                design: *design,
                gen: design_gen,
                settings,
                vel,
                soft,
                id,
                left,
                released: None,
            });
            return;
        }
        self.settings = settings;
        // The settings may have moved (decay, coupling): the loss follows at the next step.
        self.set_contact = -1.0;
        // A ringing voice keeps its strings (a new keyboard waits until it has ended: never
        // a cut); a silent one is laid out afresh, its damper already lifted by the key's
        // travel before the hammer lands.
        if !self.active {
            self.key = key;
            let spread = settings.detune_ct * (0.7 + 0.3 * key_hash(key.key));
            let detune = match key.strings {
                3 => [-spread, 0.0, spread],
                2 => [-0.5 * spread, 0.5 * spread, 0.0],
                _ => [0.0; MAX_STRINGS],
            };
            self.strings.tune(&key, design, detune);
            self.design_gen = design_gen;
            self.dc_x = 0.0;
            self.dc_y = 0.0;
            self.res = [0.0; 2];
            self.phase = 0;
            self.contact = 0.0;
            self.target = 0.0;
        }
        let soft = (soft * settings.una_depth).clamp(0.0, 1.0);
        let mut felt = settings.felt;
        felt.k *= 1.0 - 0.4 * soft;
        let x = ((vel - 1.0 / 127.0) / (1.0 - 1.0 / 127.0)).clamp(0.0, 1.0);
        let shaped = math::powf(x, settings.vel_curve.max(0.1));
        let v = settings.v_min * math::powf(settings.v_max / settings.v_min, shaped);
        let weight = match key.strings {
            3 => [1.0, 1.0, 1.0 - soft],
            2 => [1.0, 1.0 - 0.5 * soft, 0.0],
            _ => [1.0, 0.0, 0.0],
        };
        self.strings.strike(felt, key.hammer_mass, v, weight);
        self.active = true;
        self.key_down = true;
        self.id = id;
        self.quiet = 0;
    }

    /// How firmly the damper rests on the strings now (0 off … 1 down).
    pub fn contact(&self) -> f64 {
        self.contact
    }

    /// Where the damper goes, for the key's state and the pedal (0…1).
    pub fn set_pedal(&mut self, pedal: f64) {
        if let Some(r) = self.retune.as_mut().filter(|r| r.released.is_some()) {
            r.released = Some(pedal);
        }
        self.target = if self.key_down || !self.key.damper {
            0.0
        } else {
            pedal_contact(pedal) * self.settings.damper
        };
    }

    pub fn release(&mut self, pedal: f64) {
        self.key_down = false;
        self.set_pedal(pedal);
    }

    /// Let go of any strike waiting for the old strings to fade (all notes off).
    pub fn release_all_waiting(&mut self, pedal: f64) {
        if let Some(r) = self.retune.as_mut() {
            r.released = Some(pedal);
        }
    }

    /// Let go of the strike waiting for the old strings to fade, if it is `id`'s: it
    /// lands released. Whether it was.
    pub fn release_waiting(&mut self, id: u64, pedal: f64) -> bool {
        match self.retune.as_mut() {
            Some(r) if r.id == id => {
                r.released = Some(pedal);
                true
            }
            _ => false,
        }
    }

    /// Stop at once (a reset).
    pub fn kill(&mut self) {
        self.retune = None;
        if self.active {
            self.strings.silence();
        }
        self.active = false;
        self.key_down = false;
        self.contact = 1.0;
        self.target = 1.0;
        self.quiet = 0;
        self.phase = 0;
        self.dc_x = 0.0;
        self.dc_y = 0.0;
        self.res = [0.0; 2];
        self.design_gen = 0;
    }

    fn control(&mut self) {
        // The damper glides to its target over ~8 ms.
        let k = self.glide;
        self.contact += (self.target - self.contact) * k;
        if (self.contact - self.target).abs() < 1e-4 {
            self.contact = self.target;
        }
        if self.contact != self.set_contact {
            let grip = 6.907_755 / damped_t60(self.key.key as f64);
            let (g, a, yz) = loss(
                &self.key,
                self.fs,
                self.settings.coupling,
                self.settings.decay,
                self.contact * grip,
                0.0,
            );
            self.strings.set_loss(g, a, yz);
            self.set_contact = self.contact;
        }
    }

    /// Render `out.len()` samples of the bridge force (N) into `out` (added).
    pub fn render(&mut self, out: &mut [f64]) {
        if let Some(r) = self.retune {
            // The old strings fade out; then the waiting strike lands on the new ones.
            let n = (r.left as usize).min(out.len());
            self.render_gain(&mut out[..n], true);
            if self.retune.is_some_and(|r| r.left == 0) {
                let r = self.retune.take().unwrap_or(r);
                self.kill();
                self.strike(r.key, &r.design, r.gen, r.settings, r.vel, r.soft, r.id);
                if let Some(pedal) = r.released {
                    self.release(pedal);
                }
            }
            self.render_gain(&mut out[n..], false);
            return;
        }
        self.render_gain(out, false);
    }

    /// [`Self::render`]'s samples; `fading`, each scaled by what is left of the fade.
    fn render_gain(&mut self, out: &mut [f64], fading: bool) {
        if !self.active {
            return;
        }
        let ph = self.settings.phantom * self.tension * LONGITUDINAL_COUPLING;
        for o in out.iter_mut() {
            if self.phase == 0 {
                self.control();
            }
            let f = self.strings.tick();
            let mut y = f;
            if ph != 0.0 {
                let mut sq = 0.0;
                for i in 0..self.strings.strings() {
                    let a = self.strings.arrived(i);
                    sq += a * a;
                }
                // DC blocker, then half straight to the bridge, half through the
                // longitudinal mode.
                let x = sq * ph;
                let dc = x - self.dc_x + 0.9995 * self.dc_y;
                self.dc_x = x;
                self.dc_y = dc;
                let r =
                    self.res_b[2] * dc - self.res_b[0] * self.res[0] - self.res_b[1] * self.res[1];
                self.res[1] = self.res[0];
                self.res[0] = r;
                y += 0.5 * dc + 0.5 * r;
            }
            if fading {
                if let Some(r) = self.retune.as_mut() {
                    y *= f64::from(r.left) / f64::from(RETUNE_FADE);
                    r.left = r.left.saturating_sub(1);
                }
            }
            *o += y;
            self.step_peak = self.step_peak.max(f.abs());
            self.phase += 1;
            if self.phase == CONTROL {
                self.phase = 0;
                if self.step_peak < QUIET_N
                    && !self.key_down
                    && !self.strings.hammering()
                    && self.retune.is_none()
                {
                    self.quiet += 1;
                    if self.quiet >= QUIET_STEPS {
                        self.kill();
                        return;
                    }
                } else {
                    self.quiet = 0;
                }
                self.step_peak = 0.0;
            }
        }
    }
}
