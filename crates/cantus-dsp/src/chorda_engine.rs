//! Chorda, the instrument: 88 voices (one per key — a key's strings are always the same
//! strings, so a repeated note strikes the strings that still ring), the pedals, the
//! post-voice stage and the output.
//!
//! **The order of work**, in chunks of [`CHUNK`] samples: every sounding voice renders its
//! bridge force into its own lane, panned by the key's place (bass left, treble right, as
//! the player hears it); the lanes are summed **in key order** (ADR 0038: the same bits on
//! one thread or many); their sum drives the sympathetic strings; the soundboard radiates
//! the voices' force and the strings'. Every clock counts from the stream's start, never
//! from a block's: a voice's control steps from its strike, the board's quarter-rate steps
//! and the chunks themselves from the instrument's start (a chunk ends where the stream's
//! grid of [`CHUNK`] does, and only there are the dampers told which keys sound or the
//! board's tail flushed) — so any block size gives the same bits.

use super::board::{Soundboard, Sympathetic};
use super::body::Body;
use super::design::Keyboard;
use super::registers::Registers;
use super::room::Room;
use super::stage::{self, Stage};
use super::tables::{self, KEYS, LOWEST};
use super::voice::{Voice, VoiceSettings};
use super::voicing::Voicing;
use crate::math;
use crate::par::{self, Lanes, Parallel};
use std::sync::Arc;

/// Samples per chunk: the lanes' length.
pub const CHUNK: usize = 64;
/// The bridge force (N) to the output's full scale: both hands fortissimo with the pedal
/// down peak near −3 dBFS, a mezzo-forte middle C near −23 dBFS (`inst_chorda`).
const OUTPUT_SCALE: f64 = 0.0225;
/// Model 2's output ceiling (ADR 0098): a piano's chords peak some 20 dB over their
/// loudness, so at the level every instrument is staged to (ADR 0101) a fortissimo chord
/// would pass full scale. Above the knee the output bends smoothly toward the ceiling
/// (memoryless, the same at any block size); only fortissimo chords' attacks reach it.
/// Beta.14's: unity to 0.6 (−4.4 dBFS), toward 0.92 (`Voicing::knee`, `ceiling`).
pub const KNEE: f64 = 0.6;
pub const CEILING: f64 = 0.92;

/// A second-order low-pass on model 2's output (Butterworth, both channels): the
/// microphones' and the air's roll-off over the top octave (the reference's top phrase holds
/// 25 dB less above 14 kHz). Off when its corner is 0.
#[derive(Clone, Copy, Debug)]
struct TopRoll {
    on: bool,
    c: [f64; 5],
    z: [[f64; 2]; 2],
}

impl TopRoll {
    fn new(fs: f64, hz: f64) -> TopRoll {
        if hz <= 0.0 || hz >= 0.45 * fs {
            return TopRoll {
                on: false,
                c: [0.0; 5],
                z: [[0.0; 2]; 2],
            };
        }
        let w = std::f64::consts::TAU * hz / fs;
        let (s, c) = (math::sin(w), math::cos(w));
        let alpha = s / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        TopRoll {
            on: true,
            c: [
                (1.0 - c) / 2.0 / a0,
                (1.0 - c) / a0,
                (1.0 - c) / 2.0 / a0,
                -2.0 * c / a0,
                (1.0 - alpha) / a0,
            ],
            z: [[0.0; 2]; 2],
        }
    }

    /// The same filter as a high-pass at `hz` (round 40: the sympathetic strings' share of
    /// the bass register's high-pass). Off when its corner is 0.
    fn high_pass(fs: f64, hz: f64) -> TopRoll {
        let mut t = TopRoll::new(fs, hz);
        if t.on {
            let w = std::f64::consts::TAU * hz / fs;
            let (s, c) = (math::sin(w), math::cos(w));
            let a0 = 1.0 + s / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
            t.c[0] = (1.0 + c) / 2.0 / a0;
            t.c[1] = -(1.0 + c) / a0;
            t.c[2] = (1.0 + c) / 2.0 / a0;
        }
        t
    }

    #[inline]
    fn tick(&mut self, ch: usize, x: f64) -> f64 {
        let (c, z) = (&self.c, &mut self.z[ch]);
        let y = c[0] * x + z[0];
        z[0] = c[1] * x - c[3] * y + z[1];
        z[1] = c[2] * x - c[4] * y;
        y
    }
}

/// The ceiling's curve: unity below `knee`, a tanh above it, never past `top`.
#[inline]
pub fn ceiling(x: f64, knee: f64, top: f64) -> f64 {
    let a = x.abs();
    if a <= knee {
        x
    } else {
        let room = top - knee;
        (knee + room * math::tanh((a - knee) / room)).copysign(x)
    }
}

/// Model 2's room's default level (the `room` setting's default).
pub const ROOM: f64 = 0.35;
/// Model 2's quiet floor (N at the bridge): about −115 dBFS at the staged output (1e-4 N ×
/// the output scale 0.0244), some 20 dB under a 16-bit
/// file's floor — what lets its longer decays (up to four times model 1's in the treble)
/// still end, and the instrument rest, as soon as the family's contract asks.
pub const MODEL_2_QUIET: f64 = 1.0e-4;

/// The board's states are flushed below this (N: −230 dBFS out) once nothing drives it.
const FLOOR: f64 = 1.0e-10;

/// The instrument's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChordaSettings {
    pub gain_db: f64,
    pub voice: VoiceSettings,
    /// Levels: the sympathetic strings' drive and the soundboard's body (1: the model's).
    pub sympathetic: f64,
    pub body: f64,
    /// The top raised or lowered (dB), and the stereo spread of the keyboard (0…1).
    pub tilt_db: f64,
    pub width: f64,
    /// The model (ADR 0098): 1, the first (round 28: the board of ADR 0059 §3); 2, the
    /// second (round 37: the body's responses, the room, the hammer's and the action's
    /// noises, the strikes' variation). A song keeps the model it was made with.
    pub model: u8,
    /// Model 2: the room's level (0…1) and the noises' (1: as voiced).
    pub room: f64,
    pub noise: f64,
    /// Model 2's perspective (voicing pass two, `stage`): the microphones (0 over the
    /// hammers … 1 the audience), the lid (0 closed … 1 open), the room's size (0…1).
    pub persp: f64,
    pub lid: f64,
    pub size: f64,
}

impl Default for ChordaSettings {
    fn default() -> Self {
        ChordaSettings {
            gain_db: 0.0,
            voice: VoiceSettings {
                felt: super::string::Felt {
                    k: 1.0,
                    p: 0.0,
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
                blow: 0.0,
                knock: 0.0,
                landing: 0.0,
                vary: 0.0,
                b1_scale: 1.0,
                b2_scale: 1.0,
                quiet: 1.0e-10,
                damped_floor: 0.0,
                damper_t60: 1.0,
                uni_weight: 0.0,
                uni_skew: 0.0,
                uni_rank: 0.0,
                polar: 0.0,
                polar_ct: 0.0,
                polar_yz: 0.0,
                polar_damp: 1.0,
                loose_last: 0.0,
                mass: 1.0,
                partials: [0.0; super::voice::PARTIALS],
                key_return: 0.0,
                rise_ms: 0.0,
                top_damper: 0.0,
                damper_key: 0.0,
                hf_steep: 1.0,
                strike_true: 0.0,
                draw: 0,
                felt_soft: 1.0,
                landing_rise: 0.0,
                level: 1.0,
                thud_level: 1.0,
                bridge_n: 0.0,
                bridge_q: 30.0,
                bridge_g: 0.0,
                bridge_span: 10.0,
                uni_b: 0.0,
                uni_b_hold: 0.0,
                uni_loss: 0.0,
                knock_vel: 0.0,
                landing_tau: 0.0,
                landing_hi: 0.0,
                landing_modes: 1.0,
                landing_follow: 0.0,
                seed: 0,
            },
            sympathetic: 1.0,
            body: 1.0,
            tilt_db: 0.0,
            width: 0.7,
            model: 1,
            room: ROOM,
            noise: 1.0,
            persp: super::stage::PERSP,
            lid: super::stage::LID,
            size: super::stage::SIZE,
        }
    }
}

/// What the instrument shows: voices, pedals, the last strike, the output.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Meter {
    pub voices: usize,
    pub pedal: f32,
    pub soft: f32,
    pub key: u8,
    pub velocity: f32,
    pub peak_l: f32,
    pub peak_r: f32,
}

#[derive(Clone)]
pub struct Chorda {
    fs: f64,
    keyboard: Option<Arc<Keyboard>>,
    /// Bumped whenever the keyboard changes: a voice laid out for an older one keeps it
    /// until it ends, a new strike takes the new one.
    generation: u64,
    voices: Vec<Voice>,
    /// Hardness (×K) and felt exponent offset applied to every key's own hammer.
    hardness: f64,
    settings: ChordaSettings,
    lanes: Lanes,
    scratch: Vec<Vec<f64>>,
    /// Each voice's action noises (model 2), summed into the body after the strings.
    noise: Vec<Vec<f64>>,
    pans: [(f64, f64); KEYS],
    dl: [f64; CHUNK],
    dr: [f64; CHUNK],
    mono: [f64; CHUNK],
    sym_out: [f64; CHUNK],
    /// Which keys sounded at the last grid point (what the sympathetic dampers hear).
    sounding: [bool; KEYS],
    /// Which voices render in the current piece of a chunk (whose lanes are summed).
    rendering: [bool; KEYS],
    /// Samples into the current chunk of the stream's grid.
    pos: usize,
    sym: Sympathetic,
    board: Soundboard,
    /// Model 2's body and room.
    body: Body,
    /// Model 2's voicing by register: the filters, each key's share of the three buses,
    /// and the buses (left, right) for one chunk.
    regs: Registers,
    reg_w: [[f64; 3]; KEYS],
    bus: [[[f64; CHUNK]; 2]; 3],
    room: Room,
    stage: Stage,
    voicing: Voicing,
    /// Model 2 (round 40): the output's top rolled off (`Voicing::top_hz`; off at 0).
    top: TopRoll,
    /// Round 40: the bass register's high-pass on the sympathetic strings' output (its
    /// second section in `z[1]` when the high-pass is of the fourth order) — their bank
    /// passes its drive, the strings' unvoiced sum, straight through as well, and carried
    /// the lowest notes' fundamentals around the bass register's high-pass.
    sym_hp: TopRoll,
    sym_hp4: bool,
    pedal: f64,
    soft: f64,
    gain: f64,
    meter: Meter,
}

impl std::fmt::Debug for Chorda {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chorda")
            .field("fs", &self.fs)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Chorda {
    /// Allocates every voice, lane and string: off the audio thread.
    pub fn new(fs: f32) -> Chorda {
        Chorda::make(fs, false)
    }

    /// [`Self::new`], its body's responses taken if they are made at this rate and else
    /// waited for — never synthesised here ([`Body::waiting`]; round 39, fixes): what a unit
    /// made on the interface's thread is. [`Self::take_body`] gives them when they are made.
    pub fn new_waiting(fs: f32) -> Chorda {
        Chorda::make(fs, true)
    }

    /// The board's power at `f` Hz as it is set, raw and smoothed (tests, tools).
    pub fn body_power(&self, f: f64) -> Option<(f64, f64)> {
        self.body.power_at(f)
    }

    /// Key `key`'s evening as a strike now takes it (tests, tools; 1: none).
    pub fn evening(&self, key: u8) -> f64 {
        let Some(kb) = self.keyboard.as_ref() else {
            return 1.0;
        };
        let i = (key.clamp(LOWEST, tables::HIGHEST) - LOWEST) as usize;
        let partials = self.voicing.partials_of(kb.keys[i].key);
        self.evening_of(i, &kb.keys[i], &partials)
    }

    /// Key index `i`'s evening for `key` with its partial voicing `partials`: the board's
    /// response as the key's pan, its arrival-time difference and the registers deliver it
    /// to the two outputs, at the voiced perspective (the microphones, the width and the lid
    /// at their defaults, where every gain of the stage is 1). A technician evens the
    /// hammers, not the microphones: moving them never re-levels a key (`DSP_VERSION` 7 —
    /// with candidate 16's evening on, the width had moved the outer keys' mono sum).
    /// Allocates nothing.
    fn evening_of(
        &self,
        i: usize,
        key: &tables::Key,
        partials: &[f64; super::voice::PARTIALS],
    ) -> f64 {
        use super::body::Term;
        let (pl, pr) = self.pans[i];
        let (gd, gb, s) = (1.0, 1.0, 1.0);
        let (wa, wb) = (0.5 * (1.0 + s), 0.5 * (1.0 - s));
        let lag = &self.voices[i].lag;
        let dt = if lag.is_empty() {
            0.0
        } else if lag.right {
            lag.len() as f64 / self.fs
        } else {
            -(lag.len() as f64) / self.fs
        };
        let (body, regs, w, fs) = (&self.body, &self.regs, self.reg_w[i], self.fs);
        self.voicing.even_of(key, partials, |f| {
            // Each output's phase taken from its own channel's input: the other's arrives
            // `dt` later (the right) or earlier.
            let ph = std::f64::consts::TAU * f * dt;
            let (c, sn) = (math::cos(ph), math::sin(ph));
            let later = |g: f64| (g * c, -g * sn);
            let sooner = |g: f64| (g * c, g * sn);
            let terms = [
                Term {
                    c: gd * pl,
                    a: (gb * wa * pl, 0.0),
                    b: later(gb * wb * pr),
                },
                Term {
                    c: gd * pr,
                    a: sooner(gb * wb * pl),
                    b: (gb * wa * pr, 0.0),
                },
            ];
            let (p, ps) = body.output_at(f, &terms)?;
            let g = regs.power(&w, f, fs);
            Some((p * g, ps * g))
        })
    }

    /// Whether its body waits for its responses.
    pub fn body_waiting(&self) -> bool {
        self.body.is_waiting()
    }

    /// Its body's responses, if they are made now (lock-free, allocates nothing: the audio
    /// thread). Whether it has them.
    pub fn take_body(&mut self) -> bool {
        self.body.take()
    }

    /// Its body's responses made now if they are not: offline only (allocates).
    pub fn make_body(&mut self) {
        self.body.make();
    }

    fn make(fs: f32, wait: bool) -> Chorda {
        let fs = if fs > 0.0 { fs as f64 } else { 48_000.0 };
        // Room for each key's loop at any tuning the settings allow (A4 415 … 466, the
        // widest stretch): a third more than the reference keyboard's period.
        let reference = tables::keyboard(440.0, 1.0, 1.0);
        let caps: Vec<usize> = reference
            .iter()
            .map(|k| (1.33 * fs / k.partial(1)) as usize + 64)
            .collect();
        let voices = reference
            .iter()
            .zip(&caps)
            .map(|(k, c)| Voice::new(*k, *c, fs))
            .collect();
        let shipped = Voicing::default();
        let mut c = Chorda {
            fs,
            keyboard: None,
            generation: 0,
            voices,
            hardness: 0.0,
            settings: ChordaSettings::default(),
            lanes: Lanes::new(KEYS, CHUNK),
            scratch: (0..KEYS).map(|_| vec![0.0; CHUNK]).collect(),
            noise: (0..KEYS).map(|_| vec![0.0; CHUNK]).collect(),
            pans: [(0.5, 0.5); KEYS],
            dl: [0.0; CHUNK],
            dr: [0.0; CHUNK],
            mono: [0.0; CHUNK],
            sym_out: [0.0; CHUNK],
            sounding: [false; KEYS],
            rendering: [false; KEYS],
            pos: 0,
            sym: Sympathetic::new(&caps, &reference, fs),
            board: Soundboard::new(fs),
            body: if wait {
                Body::waiting(fs)
            } else {
                Body::new(fs, &shipped.body)
            },
            // As `set_voicing` makes them from the shipped voicing (round 40: candidate 16's
            // bass high-pass is fourth order, on the sympathetic strings too).
            regs: Registers::voiced(fs, &shipped.reg, &shipped.reg_level, &shipped.formant)
                .with_bass_hp_order(fs, shipped.bass_hp, shipped.bass_hp_order),
            reg_w: std::array::from_fn(|i| {
                Registers::weights_at(LOWEST + i as u8, &shipped.reg_keys)
            }),
            bus: [[[0.0; CHUNK]; 2]; 3],
            room: Room::new(fs, &shipped.room),
            stage: Stage::new(fs),
            voicing: shipped,
            top: TopRoll::new(fs, shipped.top_hz),
            sym_hp: TopRoll::high_pass(fs, shipped.bass_hp),
            sym_hp4: shipped.bass_hp_order >= 4.0,
            pedal: 0.0,
            soft: 0.0,
            gain: OUTPUT_SCALE,
            meter: Meter::default(),
        };
        c.set(&ChordaSettings::default(), 0.0);
        c
    }

    pub fn sample_rate(&self) -> f64 {
        self.fs
    }

    /// The keyboard (tuning and designs) new strikes take. A keyboard for another sample
    /// rate is refused.
    pub fn set_keyboard(&mut self, keyboard: Arc<Keyboard>) {
        if keyboard.fs != self.fs {
            return;
        }
        if self
            .keyboard
            .as_ref()
            .is_some_and(|k| Arc::ptr_eq(k, &keyboard))
        {
            return;
        }
        self.sym.tune(&keyboard.keys, &keyboard.sympathetic);
        self.keyboard = Some(keyboard);
        self.generation += 1;
    }

    /// No keyboard: new strikes are silent until one is set (a unit waiting for its own
    /// tuning's, ADR 0059 §6). What rings rings on.
    pub fn clear_keyboard(&mut self) {
        if self.keyboard.take().is_some() {
            self.generation += 1;
        }
    }

    pub fn keyboard(&self) -> Option<&Arc<Keyboard>> {
        self.keyboard.as_ref()
    }

    /// The settings, and the felt's hardness (−1…1: softer … harder, 0 the measured felt).
    pub fn set(&mut self, s: &ChordaSettings, hardness: f64) {
        // A model switched on a ringing unit: the side it leaves is never ticked again, so
        // it is cleared (no allocation) — else its frozen state would keep the unit from
        // rest and come back on the way back (ADR 0098, review of the first delivery).
        if (s.model >= 2) != (self.settings.model >= 2) {
            if s.model >= 2 {
                self.board.clear();
            } else {
                self.body.clear();
                self.regs.reset();
                self.room.reset();
                self.stage.reset();
                // Model 2's output roll-off and the sympathetic strings' high-pass (both
                // on since candidate 16) are never ticked by model 1.
                self.top.z = [[0.0; 2]; 2];
                self.sym_hp.z = [[0.0; 2]; 2];
            }
        }
        self.settings = *s;
        // Model 2's action: its noises and its strikes' variation; model 1 has neither.
        let v = &mut self.settings.voice;
        if s.model >= 2 {
            let near = stage::noise_scale(s.persp);
            v.blow = s.noise * near * self.voicing.blow;
            v.knock = s.noise * near * self.voicing.knock;
            v.landing = s.noise * near * self.voicing.landing;
            v.vary = 1.0;
            v.b2_scale = self.voicing.hf_decay;
            v.quiet = self.voicing.quiet;
            v.damped_floor = self.voicing.damped_floor();
            v.damper_t60 = self.voicing.damper_t60;
            v.uni_weight = self.voicing.uni_weight;
            v.uni_skew = self.voicing.uni_skew;
            v.uni_rank = self.voicing.uni_rank;
            v.polar = self.voicing.polar;
            v.polar_ct = self.voicing.polar_ct;
            v.polar_yz = self.voicing.polar_yz;
            v.polar_damp = self.voicing.polar_damp;
            v.loose_last = self.voicing.loose_last;
            v.mass = self.voicing.mass;
            self.sym.quiet_floor = self.voicing.quiet;
            self.room.floor = self.voicing.quiet;
        } else {
            (v.blow, v.knock, v.landing, v.vary) = (0.0, 0.0, 0.0, 0.0);
            (v.b1_scale, v.b2_scale) = (1.0, 1.0);
            v.quiet = 1.0e-10;
            v.damped_floor = 0.0;
            v.damper_t60 = 1.0;
            (v.uni_weight, v.uni_skew, v.uni_rank) = (0.0, 0.0, 0.0);
            (v.polar, v.polar_ct, v.polar_yz, v.polar_damp) = (0.0, 0.0, 0.0, 1.0);
            v.loose_last = 0.0;
            v.mass = 1.0;
            self.sym.quiet_floor = 1.0e-10;
        }
        self.hardness = hardness.clamp(-1.0, 1.0);
        let scale = if s.model >= 2 {
            self.voicing.output
        } else {
            OUTPUT_SCALE
        };
        let spread = if s.model >= 2 {
            self.voicing.spread
        } else {
            1.0
        };
        self.gain = scale * math::powf(10.0f64, s.gain_db / 20.0);
        self.sym.level = s.sympathetic;
        self.sym.set_decay(s.voice.decay);
        self.sym.set_grip(s.voice.damper);
        self.board.body = s.body;
        self.board.set_tilt(s.tilt_db);
        self.body.level = s.body;
        self.body.set_tilt(s.tilt_db);
        self.stage.set(s.persp, s.width, s.lid);
        self.room.set_size(stage::size_scale(s.size));
        if self.at_rest() {
            self.stage.snap();
        }
        // Model 2's width is the microphones' (the stage's gliding mid/side): the keys keep
        // their voiced places, so the width never steps the direct path nor moves the mono
        // sum (review 2 #7). Model 1's spreads the keys, as it always did.
        let width = if s.model >= 2 {
            stage::WIDTH
        } else {
            s.width.clamp(0.0, 1.0)
        };
        for (i, p) in self.pans.iter_mut().enumerate() {
            let x = (spread * width * (i as f64 - 43.5) / 43.5).clamp(-1.0, 1.0);
            *p = (0.5 * (1.0 - x), 0.5 * (1.0 + x));
        }
    }

    /// The voice settings a strike of key index `i` takes: the key's measured felt, made
    /// harder or softer.
    fn strike_settings(&self, i: usize) -> VoiceSettings {
        let k = &self.voices[i].key;
        let mut v = self.settings.voice;
        v.felt.k = k.hammer_k * math::powf(4.0f64, self.hardness);
        v.felt.p = (k.hammer_p + 0.3 * self.hardness).clamp(1.8, 4.0);
        if self.settings.model >= 2 {
            v.felt.k *= self.voicing.felt_of(k.key);
            v.felt.p = (v.felt.p + self.voicing.felt_p).clamp(1.8, 4.0);
            v.b1_scale = self.voicing.decay_of(k.key);
            v.b2_scale = self.voicing.hf_of(k.key);
            let hk = self.voicing.hf_keys[(k.key - LOWEST) as usize];
            if hk != 1.0 {
                v.b2_scale *= hk as f64;
            }
            v.damper_t60 = self.voicing.damper_of(k.key);
            if self.voicing.damper_key != 0.0 {
                // The key's own damper, shaped by register as the pedal's is.
                v.damper_key = self.voicing.damper_key * v.damper_t60 / self.voicing.damper_t60
                    * Voicing::bass_scale(k.key, self.voicing.damper_key_bass);
            }
            v.partials = self.voicing.partials_of(k.key);
            v.key_return = self.voicing.key_return;
            let cs = self.voicing.coupling_of(k.key);
            if cs != 1.0 {
                v.coupling = (v.coupling * cs).clamp(0.0, 0.95);
            }
            if self.voicing.vel_curve != 1.0 {
                v.vel_curve *= self.voicing.vel_curve;
            }
            let kt = self.voicing.knock_of(k.key);
            if kt != 1.0 {
                v.knock *= kt;
            }
            if self.voicing.vmin != 1.0 {
                v.v_min *= self.voicing.vmin;
            }
            if self.voicing.vmin_treble != 1.0 {
                let t = ((k.key as f64 - 60.0) / (96.0 - 60.0)).clamp(0.0, 1.0);
                v.v_min *= math::exp(t * math::ln(self.voicing.vmin_treble));
            }
            if self.voicing.vmin_bass != 1.0 {
                v.v_min *= Voicing::bass_scale(k.key, self.voicing.vmin_bass);
            }
            let [from, range, curve] = self.voicing.vel_top;
            if range != 1.0 {
                // The speed range narrowed toward the top speed: the hardest strike the same.
                let r = Voicing::top_scale(k.key, from, range);
                v.v_min = v.v_max * math::exp(r * math::ln(v.v_min / v.v_max));
            }
            if curve != 1.0 {
                v.vel_curve *= Voicing::top_scale(k.key, from, curve);
            }
            let b = &self.voicing.bridge;
            if b[0] != 0.0 {
                let fade = self.voicing.unison_fade(k.key);
                (v.bridge_n, v.bridge_q, v.bridge_g, v.bridge_span) =
                    (b[0], b[1], b[2] * fade, b[3]);
            }
            v.knock_vel = self.voicing.knock_vel;
            let fade = self.voicing.unison_fade(k.key);
            v.uni_b =
                self.voicing.uni_b * Voicing::bass_scale(k.key, self.voicing.uni_b_bass) * fade;
            v.uni_loss = self.voicing.uni_loss * fade;
            v.uni_b_hold = self.voicing.uni_b_hold;
            if self.voicing.uni_weight_low != 0.0 {
                v.uni_weight = (v.uni_weight + self.voicing.uni_weight_low * fade).min(1.0);
            }
            if self.voicing.uni_weight_bass != 0.0 {
                v.uni_weight = (v.uni_weight + self.voicing.weight_bass(k.key)).min(1.0);
            }
            if self.voicing.detune_bass != 1.0 {
                v.detune_ct *= Voicing::bass_scale(k.key, self.voicing.detune_bass);
            }
            if self.voicing.detune_scale != 1.0 {
                v.detune_ct *= self.voicing.detune_scale;
            }
            let [low, to] = self.voicing.detune_low;
            if low != 1.0 {
                let t = ((to - k.key as f64) / 12.0).clamp(0.0, 1.0);
                if t > 0.0 {
                    v.detune_ct *= math::exp(t * math::ln(low));
                }
            }
            let [from, top] = self.voicing.detune_top;
            if top != 1.0 {
                v.detune_ct *= Voicing::top_scale(k.key, from, top);
            }
            v.rise_ms = self.voicing.rise_of(k.key);
            v.top_damper = self.voicing.top_damper;
            let [from, steep] = self.voicing.hf_top;
            if steep != 1.0 {
                v.hf_steep = Voicing::top_scale(k.key, from, steep);
            }
            v.strike_true = self.voicing.strike_true;
            v.draw = self.voicing.draws[(k.key - LOWEST) as usize];
            let [fs, from, to] = self.voicing.felt_soft;
            if fs != 1.0 {
                let m = k.key as f64;
                let out = (from - m).max(m - to).max(0.0);
                let t = (1.0 - out / 12.0).clamp(0.0, 1.0);
                if t > 0.0 {
                    v.felt_soft = math::exp(t * math::ln(fs));
                }
            }
            v.landing_rise = self.voicing.landing_rise;
            let l = &self.voicing.landing_shape;
            (
                v.landing_tau,
                v.landing_hi,
                v.landing_modes,
                v.landing_follow,
            ) = (l[0], l[1], l[2], l[3]);
        }
        v
    }

    /// Model 2's voicing (ADR 0098): the shipped one unless a tool tries another.
    /// Allocates (the body's responses are synthesised): never on the audio thread.
    pub fn set_voicing(&mut self, v: &Voicing) {
        self.voicing = *v;
        self.body = Body::new(self.fs, &v.body);
        self.regs = Registers::voiced(self.fs, &v.reg, &v.reg_level, &v.formant)
            .with_bass_hp_order(self.fs, v.bass_hp, v.bass_hp_order);
        self.reg_w = std::array::from_fn(|i| Registers::weights_at(LOWEST + i as u8, &v.reg_keys));
        self.room = Room::new(self.fs, &v.room);
        self.top = TopRoll::new(self.fs, v.top_hz);
        self.sym_hp = TopRoll::high_pass(self.fs, v.bass_hp);
        self.sym_hp4 = v.bass_hp_order >= 4.0;
        let (s, h) = (self.settings, self.hardness);
        self.set(&s, h);
    }

    pub fn note_on(&mut self, key: u8, velocity: f32, id: u64) {
        let Some(kb) = self.keyboard.clone() else {
            return;
        };
        if !(LOWEST..=tables::HIGHEST).contains(&key) {
            return;
        }
        let i = (key - LOWEST) as usize;
        let design = &kb.loops[i];
        if !self.voices[i].fits(design) {
            return;
        }
        let mut vs = self.strike_settings(i);
        vs.seed = id;
        if !self.voices[i].active {
            // A silent key's arrival-time difference, laid afresh (model 2's; none in 1).
            let lag = if self.settings.model >= 2 {
                self.voicing.lag_of(key, self.fs)
            } else {
                0
            };
            self.voices[i].lag.set(lag);
        }
        if self.settings.model >= 2 {
            if self.voicing.even != 0.0 {
                vs.level = self.evening_of(i, &kb.keys[i], &vs.partials);
            }
            if self.voicing.levels_amount != 0.0 {
                vs.level *= self.voicing.level_of(key);
            }
            let pg = self.voicing.pp_gain(key, velocity as f64);
            if pg != 1.0 {
                vs.level *= pg;
            }
            if self.voicing.even_thud != 0.0 && vs.level != 1.0 {
                vs.thud_level = math::powf(vs.level, self.voicing.even_thud);
            }
        }
        let gen = self.generation;
        let soft = self.soft;
        self.voices[i].strike(kb.keys[i], design, gen, vs, velocity as f64, soft, id);
        self.voices[i].set_pedal(self.pedal);
        self.meter.key = key;
        self.meter.velocity = velocity;
    }

    pub fn note_off(&mut self, key: u8, id: u64) {
        if !(LOWEST..=tables::HIGHEST).contains(&key) {
            return;
        }
        let i = (key - LOWEST) as usize;
        let v = &mut self.voices[i];
        if v.release_waiting(id, self.pedal) {
            // Its strike waits for the old strings to fade: it lands let go.
        } else if v.active && v.key_down && v.id == id {
            v.release(self.pedal);
        }
    }

    /// The sustain pedal, 0 (up) … 1 (down), continuously.
    pub fn set_pedal(&mut self, value: f32) {
        self.pedal = (value as f64).clamp(0.0, 1.0);
        for v in self.voices.iter_mut() {
            v.set_pedal(self.pedal);
        }
        self.meter.pedal = value;
    }

    /// The soft pedal (una corda), 0 … 1: it shifts the hammers for the next strikes.
    pub fn set_soft(&mut self, value: f32) {
        self.soft = (value as f64).clamp(0.0, 1.0);
        self.meter.soft = value;
    }

    pub fn all_notes_off(&mut self) {
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.release_all_waiting(self.pedal);
            v.release(self.pedal);
        }
    }

    /// fx/final — the settings taken at once (the playhead jumped: a start, a seek, a
    /// loop's wrap): the stage's gains (perspective, width, lid) and the room's size at their
    /// targets, as a unit at rest takes them. The strings, the board, the body and the
    /// room's lines are kept.
    pub fn snap(&mut self) {
        self.stage.snap();
        self.room.snap();
    }

    pub fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            v.kill();
        }
        self.sym.reset();
        self.board.reset();
        self.body.reset();
        self.regs.reset();
        self.room.reset();
        self.stage.reset();
        self.top.z = [[0.0; 2]; 2];
        self.sym_hp.z = [[0.0; 2]; 2];
        self.sounding = [false; KEYS];
        self.pos = 0;
        self.pedal = 0.0;
        self.soft = 0.0;
        self.meter = Meter::default();
    }

    /// Which parts still hold something: (voices, sympathetic, board) — for tests.
    pub fn busy_parts(&self) -> (bool, bool, bool) {
        (
            self.voices.iter().any(|v| v.active),
            !self.sym.at_rest(),
            !self.board.at_rest(),
        )
    }

    /// How firmly key `key`'s damper rests on its strings now (tests).
    #[cfg(test)]
    pub fn damper_of(&self, key: u8) -> f64 {
        self.voices[(key - LOWEST) as usize].contact()
    }

    /// The keyboard's generation now, and the one key `key`'s strings were laid out for
    /// (tests).
    #[cfg(test)]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[cfg(test)]
    pub fn voice_generation(&self, key: u8) -> u64 {
        self.voices[(key - LOWEST) as usize].design_gen
    }

    /// Sympathetic strings still waiting for a new keyboard's layout (tests).
    #[cfg(test)]
    pub fn sympathetic_waiting(&self) -> usize {
        self.sym.waiting()
    }

    pub fn board_largest(&self) -> [f64; 5] {
        self.board.largest()
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    pub fn at_rest(&self) -> bool {
        self.pedal == 0.0
            && self.soft == 0.0
            && !self.voices.iter().any(|v| v.active)
            && self.sym.at_rest()
            && self.board.at_rest()
            && self.body.at_rest()
            && self.regs.at_rest()
            && self.room.at_rest()
            && self.stage.at_rest()
            && self.top.z == [[0.0; 2]; 2]
            && self.sym_hp.z == [[0.0; 2]; 2]
    }

    /// Frames skipped while at rest: the grid, the board's quarter-rate phase and the
    /// sympathetic strings' control phase move on.
    pub fn skip(&mut self, frames: usize) {
        self.board.skip(frames);
        self.body.skip(frames);
        self.sym.skip(frames);
        self.pos = (self.pos + frames % CHUNK) % CHUNK;
    }

    pub fn meter(&self) -> Meter {
        let mut m = self.meter;
        m.voices = self.active_voices();
        m
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.process_par(left, right, &par::Serial)
    }

    /// Render, the voices on `par`'s threads: the same bits as one thread.
    pub fn process_par(&mut self, left: &mut [f32], right: &mut [f32], par: &dyn Parallel) -> bool {
        if self.at_rest() {
            self.skip(left.len());
            return false;
        }
        let mut done = 0;
        let (mut pl, mut pr) = (0.0f32, 0.0f32);
        while done < left.len() {
            let n = (CHUNK - self.pos).min(left.len() - done);
            if self.pos == 0 {
                for (s, v) in self.sounding.iter_mut().zip(&self.voices) {
                    *s = v.active;
                }
                self.sym.set_dampers(self.pedal, &self.sounding);
            }
            self.chunk(n, par);
            let model2 = self.settings.model >= 2;
            let room = self.settings.room;
            for i in 0..n {
                // The sympathetic strings radiate from the middle of the board.
                let mut sym = self.sym_out[i];
                if model2 && self.sym_hp.on {
                    sym = self.sym_hp.tick(0, sym);
                    if self.sym_hp4 {
                        sym = self.sym_hp.tick(1, sym);
                    }
                }
                let (l, r) = if model2 {
                    let (d, b) = self
                        .body
                        .parts(self.dl[i] + 0.5 * sym, self.dr[i] + 0.5 * sym);
                    let mut o = self.stage.mix(d, b);
                    self.body.tilt(&mut o);
                    self.stage.lid(&mut o);
                    let (wl, wr) = self.room.tick(o[0], o[1]);
                    let o = self.stage.wet(o, [wl, wr], room);
                    (o[0], o[1])
                } else {
                    self.board.tick(
                        self.dl[i] + 0.5 * sym,
                        self.dr[i] + 0.5 * sym,
                        self.mono[i] + sym,
                    )
                };
                let (l, r) = if model2 && self.top.on {
                    (self.top.tick(0, l), self.top.tick(1, r))
                } else {
                    (l, r)
                };
                let (l, r) = if model2 {
                    let (k, t) = (self.voicing.knee, self.voicing.ceiling);
                    (
                        ceiling(l * self.gain, k, t) as f32,
                        ceiling(r * self.gain, k, t) as f32,
                    )
                } else {
                    ((l * self.gain) as f32, (r * self.gain) as f32)
                };
                left[done + i] += l;
                right[done + i] += r;
                pl = pl.max(l.abs());
                pr = pr.max(r.abs());
            }
            // The side this model does not play keeps the stream's grid (a switch later
            // finds it where a new unit would be).
            if model2 {
                self.board.skip(n);
            } else {
                self.body.skip(n);
            }
            self.pos += n;
            if self.pos == CHUNK {
                self.pos = 0;
                if !self.voices.iter().any(|v| v.active) && self.sym.at_rest() {
                    self.board.flush(FLOOR);
                    self.body.flush(FLOOR);
                    self.regs.flush(FLOOR);
                    self.stage.flush(FLOOR);
                    for z in self
                        .top
                        .z
                        .iter_mut()
                        .chain(self.sym_hp.z.iter_mut())
                        .flatten()
                    {
                        if z.abs() < FLOOR {
                            *z = 0.0;
                        }
                    }
                    self.room.flush();
                }
            }
            done += n;
        }
        self.meter.peak_l = pl;
        self.meter.peak_r = pr;
        true
    }

    /// One chunk of the voices (in their lanes, summed in key order) and the sympathetic
    /// strings.
    fn chunk(&mut self, n: usize, par: &dyn Parallel) {
        // A voice that ends inside this piece renders zeros after: summed or not, the
        // same bits.
        for (s, v) in self.rendering.iter_mut().zip(&self.voices) {
            *s = v.active;
        }
        let pans = self.pans;
        let active = self.voices.iter().filter(|v| v.active).count();
        let work = self
            .voices
            .iter_mut()
            .zip(self.scratch.iter_mut().zip(self.noise.iter_mut()))
            .zip(self.lanes.split(n))
            .enumerate()
            .filter(|(_, ((v, _), _))| v.active)
            .map(|(i, ((v, (buf, nz)), (l, r)))| (i, v, buf, nz, l, r));
        type Item<'a> = (
            usize,
            &'a mut Voice,
            &'a mut Vec<f64>,
            &'a mut Vec<f64>,
            &'a mut [f32],
            &'a mut [f32],
        );
        let render = |(i, v, buf, nz, l, r): Item| {
            let buf = &mut buf[..n];
            buf.fill(0.0);
            let nz = &mut nz[..n];
            nz.fill(0.0);
            v.render(buf, nz);
            par::clear_lane(l, r);
            let (gl, gr) = pans[i];
            if v.lag.is_empty() {
                for ((x, a), b) in buf.iter().zip(l.iter_mut()).zip(r.iter_mut()) {
                    *a += (x * gl) as f32;
                    *b += (x * gr) as f32;
                }
            } else {
                // Model 2's microphones hear the key at two times (round 40); past the
                // voice's end its later side is dropped too, wherever the piece ends.
                let right = v.lag.right;
                let end = v.ended.unwrap_or(buf.len());
                for (j, ((x, a), b)) in buf.iter().zip(l.iter_mut()).zip(r.iter_mut()).enumerate() {
                    if j >= end {
                        break;
                    }
                    let late = v.lag.tick(*x);
                    let (xl, xr) = if right { (*x, late) } else { (late, *x) };
                    *a += (xl * gl) as f32;
                    *b += (xr * gr) as f32;
                }
            }
        };
        if active > 1 && par.width() > 1 {
            par::run_each(par, work, &render);
        } else {
            for item in work {
                render(item);
            }
        }
        self.dl[..n].fill(0.0);
        self.dr[..n].fill(0.0);
        for i in 0..KEYS {
            if !self.rendering[i] {
                continue;
            }
            let (l, r) = self.lanes.lane(i, n);
            for j in 0..n {
                self.dl[j] += l[j] as f64;
                self.dr[j] += r[j] as f64;
            }
        }
        for j in 0..n {
            self.mono[j] = self.dl[j] + self.dr[j];
        }
        // Model 2 voiced by register: what reaches the body is the three buses, filtered
        // (the sympathetic strings hear the strings themselves, above).
        if self.settings.model >= 2 && self.regs.active() {
            for b in self.bus.iter_mut() {
                b[0][..n].fill(0.0);
                b[1][..n].fill(0.0);
            }
            for i in 0..KEYS {
                if !self.rendering[i] {
                    continue;
                }
                let (l, r) = self.lanes.lane(i, n);
                for (w, b) in self.reg_w[i].iter().zip(self.bus.iter_mut()) {
                    if *w == 0.0 {
                        continue;
                    }
                    for j in 0..n {
                        b[0][j] += w * l[j] as f64;
                        b[1][j] += w * r[j] as f64;
                    }
                }
            }
            self.dl[..n].fill(0.0);
            self.dr[..n].fill(0.0);
            for (k, b) in self.bus.iter_mut().enumerate() {
                let [bl, br] = b;
                self.regs.filter(k, &mut bl[..n], &mut br[..n]);
                for j in 0..n {
                    self.dl[j] += bl[j];
                    self.dr[j] += br[j];
                }
            }
        }
        // Model 2's action noises reach the body beside the strings (never the
        // sympathetic strings: the frame's knock is not the bridge's motion), in key order.
        if self.settings.model >= 2 {
            for ((on, (gl, gr)), noise) in self.rendering.iter().zip(pans).zip(&self.noise) {
                if !on {
                    continue;
                }
                for (j, x) in noise[..n].iter().enumerate() {
                    self.dl[j] += x * gl;
                    self.dr[j] += x * gr;
                }
            }
        }
        self.sym_out[..n].fill(0.0);
        self.sym.process(&self.mono[..n], &mut self.sym_out[..n]);
    }
}
