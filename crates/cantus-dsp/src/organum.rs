//! Organum, the tonewheel organ and its rotary speaker: the DSP. Owned by this
//! instrument's builder (ADR 0023).
//!
//! ```text
//!  91 wheels ─ key contacts (9 per key, click, leakage) ─┬ upper ─ level ─┐
//!                                                        ├ lower ─ level ─┼ vibrato/chorus scanner (upper, lower)
//!                                                        ├ pedal ─ level ─┘   │
//!                                                        └ percussion bus ─ single-trigger envelope ─ (joins upper)
//!  ─ swell (with its bass tilt) ─ preamp (asymmetric, ADAA) ─ rotary speaker or direct ─ gain ─ L/R
//! ```
//!
//! **Not voice-based.** The wheels always turn; a key only closes contacts that connect
//! wheels to the manual's output. What sounds is a gain per wheel per output (upper,
//! lower, pedal, percussion) — `WHEELS × 4` multiply-adds a sample whatever the number of
//! keys held — rebuilt from the key state when a key or a drawbar moves. A key whose
//! contacts are still closing (or opening) is handled apart for the few milliseconds it
//! takes: each of its contacts closes at its own seeded moment and bounces, and **that is
//! the key click** — the running sines switched on abruptly — scaled by the click level
//! between a clean (smoothstep) contact and the raw bouncing one.
//!
//! **Deterministic**: the click's timings are seeded from the note (its id, key and
//! manual), every ramp and envelope is stepped per sample, and the wheels', scanner's and
//! rotors' phases are counted from the start of the stream, so the output is the same at
//! every block size. **Exact silence**: once no key is held, no contact moves and the
//! output has stayed below −140 dB for 0.1 s, the audio state is cleared and the output is
//! exact zero (the wheels, scanner and rotors keep turning).

// Parallel per-wheel / per-mode arrays read clearest by index (as in `spatium_*`).
#![allow(clippy::needless_range_loop)]

#[path = "organum_filters.rs"]
pub mod filters;
#[path = "organum_rotary.rs"]
pub mod rotary;
#[path = "organum_scanner.rs"]
pub mod scanner;
#[path = "organum_wheels.rs"]
pub mod wheels;

#[cfg(test)]
#[path = "organum_tests.rs"]
mod tests;

use filters::{Bq, Clipper, Glide};
use rotary::{Rotary, RotarySettings};
use scanner::{Scanner, VibMode};
use wheels::{
    drawbar_gain, leak_partners, wheel_of, Generator, Tuning, BAR_SEMITONES, PEDAL_BARS, WHEELS,
};

/// The three divisions.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Manual {
    Upper = 0,
    Lower = 1,
    Pedal = 2,
}

/// Output columns of the wheel gains: the three manuals and the percussion bus.
const PERC: usize = 3;
/// Keys a manual spans (C2…C7); notes outside fold in by octaves.
pub const MANUAL_LOW: u8 = 36;
pub const MANUAL_HIGH: u8 = 96;
/// The pedals' range (C2…G3 at 8′).
pub const PEDAL_LOW: u8 = 36;
pub const PEDAL_HIGH: u8 = 67;
/// A sine at a full drawbar, into the preamp.
const MANUAL_GAIN: f32 = 0.06;
/// Samples a drawbar (or leakage, brilliance) change glides over: 5 ms at 48 kHz.
const RAMP_MS: f32 = 5.0;
/// Contacts in motion at once (keys × contacts): beyond it the oldest completes at once.
pub const MAX_TRANSITIONS: usize = 64;
/// Notes held at once (two notes on one key are two).
pub const MAX_NOTES: usize = 256;
/// Output below this (−140 dB) for [`SLEEP_MS`] with nothing held: exact silence.
const SILENCE: f32 = 1e-7;
const SLEEP_MS: f32 = 100.0;

/// Everything the organ is set to, in real units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct OrganSettings {
    /// Drawbar positions 0…8 (16′ … 1′).
    pub upper: [f32; 9],
    pub lower: [f32; 9],
    /// 16′ and 8′.
    pub pedal: [f32; 2],
    /// First key of the upper manual (keys below go to the lower manual); 0 = no lower zone.
    pub split_upper: u8,
    /// First key above the pedal zone; 0 = no pedal zone.
    pub split_pedal: u8,
    pub perc_on: bool,
    /// The 3rd harmonic (2⅔′) rather than the 2nd (4′).
    pub perc_third: bool,
    pub perc_slow: bool,
    pub perc_soft: bool,
    /// Decay times to −60 dB, ms.
    pub perc_fast_ms: f32,
    pub perc_slow_ms: f32,
    pub perc_soft_db: f32,
    /// What normal-volume percussion takes off the upper manual, dB (≤ 0).
    pub perc_drop_db: f32,
    /// Percussion on takes the upper manual's 1′ drawbar.
    pub perc_steal: bool,
    pub vib_mode: VibMode,
    pub vib_upper: bool,
    pub vib_lower: bool,
    pub vib_rate_hz: f32,
    /// Key click on press and on release, dB (0 = the raw contact).
    pub click_db: f32,
    pub release_click_db: f32,
    /// Crosstalk between wheels of a compartment, dB.
    pub leak_db: f32,
    /// What a drawbar pushed in still passes, dB.
    pub bar_leak_db: f32,
    /// Tilt of the wheels' levels: dB at the top wheel against 523 Hz.
    pub brilliance_db: f32,
    pub tuning: Tuning,
    pub a4_hz: f32,
    /// Expression pedal, 0…1.
    pub swell: f32,
    /// Preamp drive, 0…1.
    pub drive: f32,
    pub upper_db: f32,
    pub lower_db: f32,
    pub pedal_db: f32,
    /// Through the rotary speaker (or straight out, mono).
    pub rotary_on: bool,
    pub rotary: RotarySettings,
    pub gain_db: f32,
}

impl Default for OrganSettings {
    fn default() -> Self {
        OrganSettings {
            upper: [8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            lower: [0.0, 0.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            pedal: [8.0, 4.0],
            split_upper: 0,
            split_pedal: 0,
            perc_on: false,
            perc_third: true,
            perc_slow: false,
            perc_soft: false,
            perc_fast_ms: 1_000.0,
            perc_slow_ms: 4_000.0,
            perc_soft_db: -6.0,
            perc_drop_db: -3.0,
            perc_steal: true,
            vib_mode: VibMode::C3,
            vib_upper: true,
            vib_lower: false,
            vib_rate_hz: 7.0,
            click_db: -6.0,
            release_click_db: -12.0,
            leak_db: -40.0,
            bar_leak_db: -60.0,
            brilliance_db: 0.0,
            tuning: Tuning::Gear,
            a4_hz: 440.0,
            swell: 1.0,
            drive: 0.2,
            upper_db: 0.0,
            lower_db: -3.0,
            pedal_db: -3.0,
            rotary_on: true,
            rotary: RotarySettings::default(),
            gain_db: 0.0,
        }
    }
}

/// `10^(db/20)`, with anything at or below −120 dB as exact zero.
pub fn db_to_gain(db: f32) -> f32 {
    if db <= -120.0 {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

/// One contact a key closes: which drawbar (9 = the percussion bus), how far above the
/// key's 8′ pitch, and at what level.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct ContactSpec {
    bar: u8,
    semis: i8,
    level: f32,
}

const MAX_CONTACTS: usize = 10;

/// A key's contacts on each division (the upper manual's tenth is the percussion bus, used
/// only while the percussion is on).
fn contacts(manual: Manual, perc_semis: Option<i32>) -> ([ContactSpec; MAX_CONTACTS], usize) {
    let mut out = [ContactSpec::default(); MAX_CONTACTS];
    let mut n = 0;
    match manual {
        Manual::Upper | Manual::Lower => {
            for (b, s) in BAR_SEMITONES.iter().enumerate() {
                out[n] = ContactSpec {
                    bar: b as u8,
                    semis: *s as i8,
                    level: 1.0,
                };
                n += 1;
            }
            if let (Manual::Upper, Some(p)) = (manual, perc_semis) {
                out[n] = ContactSpec {
                    bar: 9,
                    semis: p as i8,
                    level: 1.0,
                };
                n += 1;
            }
        }
        Manual::Pedal => {
            for (b, bar) in PEDAL_BARS.iter().enumerate() {
                for (s, level) in bar {
                    if *level > 0.0 {
                        out[n] = ContactSpec {
                            bar: b as u8,
                            semis: *s as i8,
                            level: *level,
                        };
                        n += 1;
                    }
                }
            }
        }
    }
    (out, n)
}

/// A contact's motion: when it first touches (samples after the key), and when it opens
/// and closes again while it bounces.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Bounce {
    t0: u32,
    toggles: [u32; 6],
    n: u8,
}

impl Bounce {
    /// 0 open, 1 closed, raw.
    #[inline]
    fn raw(&self, t: u32) -> f32 {
        if t < self.t0 {
            return 0.0;
        }
        let dt = t - self.t0;
        let mut passed = 0;
        for k in 0..self.n as usize {
            if dt >= self.toggles[k] {
                passed += 1;
            }
        }
        if passed % 2 == 0 {
            1.0
        } else {
            0.0
        }
    }

    fn end(&self, smooth: u32) -> u32 {
        let last = if self.n > 0 {
            self.toggles[self.n as usize - 1]
        } else {
            0
        };
        self.t0 + last.max(smooth) + 1
    }
}

/// A key whose contacts are moving.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Transition {
    manual: u8,
    key: u8,
    press: bool,
    t: u32,
    len: u32,
    bounce: [Bounce; MAX_CONTACTS],
    spec: [ContactSpec; MAX_CONTACTS],
    wheel: [u8; MAX_CONTACTS],
    n: u8,
}

/// A small, fast, seedable generator (SplitMix64).
#[derive(Copy, Clone, Debug)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Seed for a key's contacts: the note's id, key, division and direction — never a
/// counter, never the clock.
fn note_seed(id: u64, key: u8, manual: Manual, press: bool) -> u64 {
    id.wrapping_mul(0xD6E8_FEB8_6659_FD93)
        ^ ((key as u64) << 8)
        ^ ((manual as u64) << 16)
        ^ if press { 0x5555 } else { 0xAAAA_0000 }
}

/// What the editor draws, read after each block.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OrganMeter {
    pub horn_angle: f32,
    pub drum_angle: f32,
    pub horn_rpm: f32,
    pub drum_rpm: f32,
    /// The vibrato pickup's place on the line, 0…1.
    pub scanner: f32,
    /// The percussion envelope, 0…1.
    pub perc: f32,
    pub peak_l: f32,
    pub peak_r: f32,
    /// Keys down per division.
    pub held: [u32; 3],
}

type Gains = [[f32; 4]; WHEELS];

/// The organ.
#[derive(Clone, Debug)]
pub struct Organum {
    sr: f32,
    s: OrganSettings,
    configured: bool,
    gen: Generator,
    gain: Box<Gains>,
    target: Box<Gains>,
    old: Box<Gains>,
    delta: Box<Gains>,
    ramp_left: u32,
    ramp_len: u32,
    /// Notes holding each key, per division.
    count: [[u8; 128]; 3],
    /// Keys whose contacts are closed and settled (in `target`).
    closed: [[bool; 128]; 3],
    /// Keys down per division.
    down: [u32; 3],
    notes: [(u64, u8, u8); MAX_NOTES],
    nnotes: usize,
    trans: Box<[Transition; MAX_TRANSITIONS]>,
    ntrans: usize,
    /// Drawbar levels per division; index 9 is the percussion bus (1).
    bar: [[f32; 10]; 3],
    taper: [f32; WHEELS],
    partners: [[(u8, f32); 6]; WHEELS],
    perc_semis: Option<i32>,
    click: f32,
    release_click: f32,
    smooth_len: u32,
    perc_env: f32,
    perc_attack: u32,
    perc_attack_len: u32,
    perc_mult: f32,
    perc_gain: Glide,
    drop: Glide,
    lvl: [Glide; 3],
    vib_u: bool,
    vib_l: bool,
    swell: Glide,
    drive: Glide,
    post: Glide,
    out: Glide,
    shelf: Bq,
    clip: Clipper,
    scanner: Scanner,
    rotary: Rotary,
    quiet: u32,
    sleep_after: u32,
    asleep: bool,
    meter: OrganMeter,
}

impl Organum {
    /// Allocates: build it off the audio thread.
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let ms = |v: f32| ((v * 0.001 * sr).round() as u32).max(1);
        let glide = |v: f32| Glide::new(v, ms(10.0));
        let mut o = Organum {
            sr,
            s: OrganSettings::default(),
            configured: false,
            gen: Generator::new(sr),
            gain: Box::new([[0.0; 4]; WHEELS]),
            target: Box::new([[0.0; 4]; WHEELS]),
            old: Box::new([[0.0; 4]; WHEELS]),
            delta: Box::new([[0.0; 4]; WHEELS]),
            ramp_left: 0,
            ramp_len: ms(RAMP_MS),
            count: [[0; 128]; 3],
            closed: [[false; 128]; 3],
            down: [0; 3],
            notes: [(0, 0, 0); MAX_NOTES],
            nnotes: 0,
            trans: Box::new([Transition::default(); MAX_TRANSITIONS]),
            ntrans: 0,
            bar: [[0.0; 10]; 3],
            taper: [1.0; WHEELS],
            partners: [[(0, 0.0); 6]; WHEELS],
            perc_semis: None,
            click: 0.5,
            release_click: 0.25,
            smooth_len: ms(2.0),
            perc_env: 0.0,
            perc_attack: 0,
            perc_attack_len: ms(1.0),
            perc_mult: 1.0,
            perc_gain: Glide::new(1.0, ms(5.0)),
            drop: glide(1.0),
            lvl: [glide(1.0), glide(1.0), glide(1.0)],
            vib_u: true,
            vib_l: false,
            swell: glide(1.0),
            drive: glide(1.0),
            post: glide(1.0),
            out: glide(1.0),
            shelf: Bq::identity(),
            clip: Clipper::new(0.2),
            scanner: Scanner::new(sr),
            rotary: Rotary::new(sr),
            quiet: 0,
            sleep_after: ms(SLEEP_MS),
            asleep: true,
            meter: OrganMeter::default(),
        };
        let s = OrganSettings::default();
        o.set(&s);
        o.configured = false; // the first real configuration snaps again
        o
    }

    pub fn settings(&self) -> &OrganSettings {
        &self.s
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// The wheels' generator (for the editor and the tests).
    pub fn generator(&self) -> &Generator {
        &self.gen
    }

    pub fn meter(&self) -> OrganMeter {
        let mut m = self.meter;
        m.horn_angle = self.rotary.horn.angle as f32;
        m.drum_angle = self.rotary.drum.angle as f32;
        m.horn_rpm = self.rotary.horn.rpm();
        m.drum_rpm = self.rotary.drum.rpm();
        m.scanner = self.scanner.position;
        m.perc = self.perc_env;
        m.held = [self.down[0], self.down[1], self.down[2]];
        m
    }

    /// Keys down across the divisions.
    pub fn keys_down(&self) -> usize {
        (self.down[0] + self.down[1] + self.down[2]) as usize
    }

    /// Keys whose contacts are moving now.
    pub fn contacts_moving(&self) -> usize {
        self.ntrans
    }

    /// New settings. Allocation-free, cheap unless the drawbars, leakage or brilliance
    /// moved (then the wheel gains are rebuilt and glide there over 5 ms).
    pub fn set(&mut self, s: &OrganSettings) {
        let first = !self.configured;
        let old = self.s;
        self.s = *s;
        let sr = self.sr;
        if first || old.tuning != s.tuning || old.a4_hz != s.a4_hz {
            self.gen.tune(s.tuning, s.a4_hz.clamp(300.0, 600.0) as f64);
        }
        let bar_leak = db_to_gain(s.bar_leak_db);
        let level = |p: f32| drawbar_gain(p).max(bar_leak);
        let mut bar = [[0.0f32; 10]; 3];
        for b in 0..9 {
            bar[0][b] = level(s.upper[b]);
            bar[1][b] = level(s.lower[b]);
        }
        bar[2][0] = level(s.pedal[0]);
        bar[2][1] = level(s.pedal[1]);
        if s.perc_on && s.perc_steal {
            bar[0][8] = 0.0;
        }
        for row in &mut bar {
            row[9] = 1.0;
        }
        let perc_semis = s.perc_on.then_some(if s.perc_third { 19 } else { 12 });
        let gains_moved = first
            || bar != self.bar
            || perc_semis != self.perc_semis
            || old.leak_db != s.leak_db
            || old.brilliance_db != s.brilliance_db;
        if first || old.leak_db != s.leak_db {
            let leak = db_to_gain(s.leak_db);
            for w in 0..WHEELS {
                for (i, (p, rel)) in leak_partners(w).iter().enumerate() {
                    self.partners[w][i] = (*p as u8, rel * leak);
                }
            }
        }
        if first || old.brilliance_db != s.brilliance_db {
            let top = (wheels::gear_hz(WHEELS - 1) / 523.25).log2() as f32;
            for w in 0..WHEELS {
                let oct = (wheels::gear_hz(w) / 523.25).log2() as f32;
                self.taper[w] = if s.brilliance_db == 0.0 {
                    1.0
                } else {
                    10f32.powf(s.brilliance_db * oct / top / 20.0)
                };
            }
        }
        self.bar = bar;
        self.perc_semis = perc_semis;
        if gains_moved {
            self.rebuild(first);
        }
        self.click = 10f32.powf(s.click_db.clamp(-120.0, 12.0) / 20.0);
        self.release_click = 10f32.powf(s.release_click_db.clamp(-120.0, 12.0) / 20.0);
        let t60 = if s.perc_slow {
            s.perc_slow_ms
        } else {
            s.perc_fast_ms
        };
        self.perc_mult = 10f32.powf(-3.0 / (t60.max(1.0) * 0.001 * sr));
        self.perc_gain.set(if s.perc_soft {
            db_to_gain(s.perc_soft_db)
        } else {
            1.0
        });
        self.drop.set(if s.perc_on && !s.perc_soft {
            db_to_gain(s.perc_drop_db.min(0.0))
        } else {
            1.0
        });
        self.lvl[0].set(db_to_gain(s.upper_db));
        self.lvl[1].set(db_to_gain(s.lower_db));
        self.lvl[2].set(db_to_gain(s.pedal_db));
        self.scanner.set(s.vib_mode, s.vib_rate_hz);
        self.vib_u = s.vib_upper;
        self.vib_l = s.vib_lower;
        let sw = s.swell.clamp(0.0, 1.0);
        self.swell.set(db_to_gain(-36.0 * (1.0 - sw)));
        self.shelf.set_low_shelf(sr, 150.0, 9.0 * (1.0 - sw));
        let g = 10f32.powf(1.5 * s.drive.clamp(0.0, 1.0));
        self.drive.set(g);
        self.post.set(1.0 / (self.clip.slope() * g.sqrt()));
        self.out.set(db_to_gain(s.gain_db));
        self.rotary.set(&s.rotary);
        if first {
            for gl in [
                &mut self.perc_gain,
                &mut self.drop,
                &mut self.swell,
                &mut self.drive,
                &mut self.post,
                &mut self.out,
            ] {
                let t = gl.target();
                gl.snap(t);
            }
            for gl in &mut self.lvl {
                let t = gl.target();
                gl.snap(t);
            }
            self.rotary.snap_speeds();
            self.configured = true;
        }
    }

    /// Which division a note plays, and the key it closes there.
    pub fn route(&self, key: u8, channel: u8) -> (Manual, u8) {
        let s = &self.s;
        let manual = match channel {
            1 => Manual::Lower,
            2 => Manual::Pedal,
            _ if s.split_pedal > 0 && key < s.split_pedal => Manual::Pedal,
            _ if s.split_upper > 0 && key < s.split_upper => Manual::Lower,
            _ => Manual::Upper,
        };
        let (lo, hi) = match manual {
            Manual::Pedal => (PEDAL_LOW, PEDAL_HIGH),
            _ => (MANUAL_LOW, MANUAL_HIGH),
        };
        let mut k = key;
        while k < lo {
            k += 12;
        }
        while k > hi {
            k -= 12;
        }
        (manual, k)
    }

    pub fn note_on(&mut self, key: u8, id: u64, channel: u8) {
        if self.nnotes == MAX_NOTES {
            return;
        }
        let (manual, k) = self.route(key, channel);
        let m = manual as usize;
        self.notes[self.nnotes] = (id, m as u8, k);
        self.nnotes += 1;
        self.asleep = false;
        self.quiet = 0;
        self.count[m][k as usize] = self.count[m][k as usize].saturating_add(1);
        if self.count[m][k as usize] > 1 {
            return; // the key is already down
        }
        if manual == Manual::Upper && self.down[0] == 0 && self.s.perc_on {
            // Single trigger: only from all keys up.
            self.perc_attack = self.perc_attack_len;
        }
        self.down[m] += 1;
        self.start(manual, k, true, id);
    }

    pub fn note_off(&mut self, id: u64) {
        if let Some(i) = self.notes[..self.nnotes].iter().position(|n| n.0 == id) {
            self.release(i);
        }
    }

    fn release(&mut self, i: usize) {
        let (id, m, k) = self.notes[i];
        self.notes.copy_within(i + 1..self.nnotes, i);
        self.nnotes -= 1;
        let (mi, ki) = (m as usize, k as usize);
        self.count[mi][ki] = self.count[mi][ki].saturating_sub(1);
        if self.count[mi][ki] > 0 {
            return;
        }
        // A press still settling completes before the release begins.
        if let Some(t) = self.trans[..self.ntrans]
            .iter()
            .position(|t| t.press && t.manual == m && t.key == k)
        {
            self.remove_transition(t);
        }
        let was_closed = self.closed[mi][ki];
        self.closed[mi][ki] = false;
        self.down[mi] = self.down[mi].saturating_sub(1);
        let manual = [Manual::Upper, Manual::Lower, Manual::Pedal][mi];
        self.start(manual, k, false, id);
        if was_closed {
            self.rebuild(true);
        }
    }

    /// Release every note (transport stop): the contacts open as a player's would.
    pub fn all_notes_off(&mut self) {
        while self.nnotes > 0 {
            self.release(self.nnotes - 1);
        }
    }

    /// Silence at once and forget everything: back to the state of a new instrument.
    pub fn reset(&mut self) {
        self.count = [[0; 128]; 3];
        self.closed = [[false; 128]; 3];
        self.down = [0; 3];
        self.nnotes = 0;
        self.ntrans = 0;
        *self.gain = [[0.0; 4]; WHEELS];
        *self.target = [[0.0; 4]; WHEELS];
        self.ramp_left = 0;
        self.gen.reset();
        self.scanner.phase = 0;
        self.scanner.clear();
        self.rotary.reset();
        self.shelf.reset();
        self.clip.reset();
        self.perc_env = 0.0;
        self.perc_attack = 0;
        self.asleep = true;
        self.quiet = 0;
        self.meter = OrganMeter::default();
    }

    /// The contacts of a key begin to move.
    fn start(&mut self, manual: Manual, key: u8, press: bool, id: u64) {
        if self.ntrans == MAX_TRANSITIONS {
            self.remove_transition(0);
            self.rebuild(true);
        }
        let (spec, n) = contacts(
            manual,
            if manual == Manual::Upper {
                self.perc_semis
            } else {
                None
            },
        );
        let mut rng = Rng(note_seed(id, key, manual, press));
        let ms = |v: f32| (v * 0.001 * self.sr) as u32;
        let mut tr = Transition {
            manual: manual as u8,
            key,
            press,
            t: 0,
            len: 0,
            bounce: [Bounce::default(); MAX_CONTACTS],
            spec,
            wheel: [0; MAX_CONTACTS],
            n: n as u8,
        };
        let (spread, max_bounces) = if press { (1.5, 4.0) } else { (1.0, 3.0) };
        for c in 0..n {
            tr.wheel[c] = wheel_of(key as i32 + spec[c].semis as i32) as u8;
            let b = &mut tr.bounce[c];
            b.t0 = ms(spread * rng.unit());
            let bounces = ((max_bounces * rng.unit()) as usize).min(3);
            let mut t = 0;
            for _ in 0..bounces {
                t += ms(0.05 + 0.45 * rng.unit()).max(1);
                b.toggles[b.n as usize] = t;
                t += ms(0.03 + 0.27 * rng.unit()).max(1);
                b.toggles[b.n as usize + 1] = t;
                b.n += 2;
            }
            tr.len = tr.len.max(b.end(self.smooth_len));
        }
        self.trans[self.ntrans] = tr;
        self.ntrans += 1;
    }

    /// A transition ends: a press's contacts are settled (they join `target`).
    fn remove_transition(&mut self, i: usize) {
        let tr = self.trans[i];
        self.trans.copy_within(i + 1..self.ntrans, i);
        self.ntrans -= 1;
        let (m, k) = (tr.manual as usize, tr.key as usize);
        if tr.press && self.count[m][k] > 0 {
            self.closed[m][k] = true;
        }
    }

    /// Rebuild the settled gains from the closed keys. `instant`: the change is already
    /// continuous (a contact handed over from its transition), so the gains take the
    /// difference at once and any glide in progress carries on; otherwise glide over 5 ms.
    fn rebuild(&mut self, instant: bool) {
        *self.old = *self.target;
        *self.target = [[0.0; 4]; WHEELS];
        for (m, manual) in [Manual::Upper, Manual::Lower, Manual::Pedal]
            .into_iter()
            .enumerate()
        {
            let (spec, n) = contacts(
                manual,
                if manual == Manual::Upper {
                    self.perc_semis
                } else {
                    None
                },
            );
            for k in 0..128usize {
                if !self.closed[m][k] {
                    continue;
                }
                for c in &spec[..n] {
                    let w = wheel_of(k as i32 + c.semis as i32);
                    if c.bar == 9 {
                        self.target[w][PERC] += self.taper[w];
                        continue;
                    }
                    let lvl = self.bar[m][c.bar as usize] * c.level * self.taper[w];
                    if lvl == 0.0 {
                        continue;
                    }
                    self.target[w][m] += lvl;
                    for (p, rel) in self.partners[w] {
                        if rel != 0.0 {
                            self.target[p as usize][m] += lvl * rel;
                        }
                    }
                }
            }
        }
        if instant {
            for w in 0..WHEELS {
                for o in 0..4 {
                    self.gain[w][o] += self.target[w][o] - self.old[w][o];
                }
            }
            if self.ramp_left == 0 {
                *self.gain = *self.target;
                return;
            }
        } else {
            self.ramp_left = self.ramp_len;
        }
        let left = self.ramp_left as f32;
        for w in 0..WHEELS {
            for o in 0..4 {
                self.delta[w][o] = (self.target[w][o] - self.gain[w][o]) / left;
            }
        }
    }

    /// Render into `left`/`right` (added). Returns whether anything but exact zeros was
    /// written.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let n = left.len().min(right.len());
        if self.asleep && self.nnotes == 0 && self.ntrans == 0 {
            // Silence: time passes — the wheels, the scanner and the rotors turn — and
            // nothing is written.
            self.gen.skip(n);
            self.scanner.skip(n);
            for _ in 0..n {
                self.idle_tick();
            }
            self.meter.peak_l = 0.0;
            self.meter.peak_r = 0.0;
            return false;
        }
        let mut wrote = false;
        for i in 0..n {
            let (l, r) = self.tick();
            if l != 0.0 || r != 0.0 {
                left[i] += l;
                right[i] += r;
                wrote = true;
            }
        }
        wrote
    }

    /// One sample of silence: only what turns, turns.
    #[inline]
    fn idle_tick(&mut self) {
        self.rotary.advance();
        for g in &mut self.lvl {
            g.next();
        }
        self.drop.next();
        self.perc_gain.next();
        self.swell.next();
        self.drive.next();
        self.post.next();
        self.out.next();
    }

    #[inline]
    fn tick(&mut self) -> (f32, f32) {
        if self.asleep {
            if self.nnotes == 0 && self.ntrans == 0 {
                self.gen.skip(1);
                self.scanner.skip(1);
                self.idle_tick();
                return (0.0, 0.0);
            }
            self.asleep = false;
        }
        self.gen.tick();
        if self.ramp_left > 0 {
            self.ramp_left -= 1;
            if self.ramp_left == 0 {
                *self.gain = *self.target;
            } else {
                for w in 0..WHEELS {
                    for o in 0..4 {
                        self.gain[w][o] += self.delta[w][o];
                    }
                }
            }
        }
        let mut acc = [0.0f32; 4];
        {
            let out = &self.gen.out;
            let g = &*self.gain;
            for w in 0..WHEELS {
                let s = out[w];
                let gw = &g[w];
                acc[0] += s * gw[0];
                acc[1] += s * gw[1];
                acc[2] += s * gw[2];
                acc[3] += s * gw[3];
            }
        }
        if self.ntrans > 0 {
            self.run_transitions(&mut acc);
        }
        // Percussion: a 1 ms attack, then an exponential decay to −60 dB in the set time.
        if self.perc_attack > 0 {
            self.perc_env += (1.0 - self.perc_env) / self.perc_attack as f32;
            self.perc_attack -= 1;
        } else if self.perc_env > 0.0 {
            self.perc_env *= self.perc_mult;
            if self.perc_env < 1e-6 {
                self.perc_env = 0.0;
            }
        }
        let lu = self.lvl[0].next();
        let ll = self.lvl[1].next();
        let lp = self.lvl[2].next();
        let drop = self.drop.next();
        let pg = self.perc_gain.next();
        let up = (acc[0] * drop + acc[PERC] * self.perc_env * pg) * lu * MANUAL_GAIN;
        let lo = acc[1] * ll * MANUAL_GAIN;
        let pd = acc[2] * lp * MANUAL_GAIN;
        let (mut vin, mut dry) = (0.0, pd);
        if self.vib_u {
            vin += up;
        } else {
            dry += up;
        }
        if self.vib_l {
            vin += lo;
        } else {
            dry += lo;
        }
        let x = dry + self.scanner.run(vin);
        let x = self.shelf.run(x) * self.swell.next();
        let y = self.clip.run(x * self.drive.next()) * self.post.next();
        let (l, r) = if self.s.rotary_on {
            self.rotary.tick(y)
        } else {
            self.rotary.advance();
            (y, y)
        };
        let o = self.out.next();
        let (l, r) = (l * o, r * o);
        let m = &mut self.meter;
        m.peak_l = (m.peak_l * 0.9999).max(l.abs());
        m.peak_r = (m.peak_r * 0.9999).max(r.abs());
        // Exact silence once nothing is held, nothing moves and the tail has died away.
        let active = self.nnotes > 0 || self.ntrans > 0 || self.ramp_left > 0;
        if !active && l.abs() < SILENCE && r.abs() < SILENCE {
            self.quiet += 1;
            if self.quiet >= self.sleep_after {
                self.sleep();
                return (0.0, 0.0);
            }
        } else {
            self.quiet = 0;
        }
        (l, r)
    }

    fn sleep(&mut self) {
        self.asleep = true;
        self.quiet = 0;
        self.scanner.clear();
        self.shelf.reset();
        self.clip.reset();
        self.rotary.clear();
        self.perc_env = 0.0;
        self.perc_attack = 0;
        self.meter.peak_l = 0.0;
        self.meter.peak_r = 0.0;
    }

    /// The keys whose contacts are moving: each contact's envelope, per sample.
    fn run_transitions(&mut self, acc: &mut [f32; 4]) {
        let out = &self.gen.out;
        let smooth = self.smooth_len as f32;
        let mut finished = false;
        for tr in self.trans[..self.ntrans].iter_mut() {
            let t = tr.t;
            let m = tr.manual as usize;
            let click = if tr.press {
                self.click
            } else {
                self.release_click
            };
            for c in 0..tr.n as usize {
                let b = &tr.bounce[c];
                let x = if t < b.t0 {
                    0.0
                } else {
                    ((t - b.t0) as f32 / smooth).min(1.0)
                };
                let sm = x * x * (3.0 - 2.0 * x);
                let on = sm + click * (b.raw(t) - sm);
                let e = if tr.press { on } else { 1.0 - on };
                if e == 0.0 {
                    continue;
                }
                let spec = tr.spec[c];
                let w = tr.wheel[c] as usize;
                if spec.bar == 9 {
                    acc[PERC] += e * self.taper[w] * out[w];
                    continue;
                }
                let lvl = self.bar[m][spec.bar as usize] * spec.level * self.taper[w];
                if lvl == 0.0 {
                    continue;
                }
                let mut v = out[w];
                for (p, rel) in self.partners[w] {
                    v += rel * out[p as usize];
                }
                acc[m] += e * lvl * v;
            }
            tr.t += 1;
            finished |= tr.t >= tr.len;
        }
        if finished {
            let mut i = 0;
            let mut any_press = false;
            while i < self.ntrans {
                if self.trans[i].t >= self.trans[i].len {
                    any_press |= self.trans[i].press;
                    self.remove_transition(i);
                } else {
                    i += 1;
                }
            }
            if any_press {
                self.rebuild(true);
            }
        }
    }
}
