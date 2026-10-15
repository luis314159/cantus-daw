//! Chorda, the instrument: 88 voices (one per key — a key's strings are always the same
//! strings, so a repeated note strikes the strings that still ring), the pedals, the
//! post-voice stage and the output.
//!
//! **The order of work**, in chunks of [`CHUNK`] samples: every sounding voice renders its
//! bridge force into its own lane, panned by the key's place (bass left, treble right, as
//! the player hears it); the lanes are summed **in key order** (ADR 0038: the same bits on
//! one thread or many); their sum drives the sympathetic strings; the soundboard radiates
//! the voices' force and the strings'. A voice's own clock (its control steps) and the
//! board's quarter-rate steps count from their own start, never from a block's, so any
//! block size gives the same bits.

use super::board::{Soundboard, Sympathetic};
use super::design::Keyboard;
use super::tables::{self, KEYS, LOWEST};
use super::voice::{Voice, VoiceSettings};
use crate::math;
use crate::par::{self, Lanes, Parallel};
use std::sync::Arc;

/// Samples per chunk: the lanes' length.
pub const CHUNK: usize = 64;
/// The bridge force (N) to the output's full scale: a forte middle C peaks near −12 dBFS.
const OUTPUT_SCALE: f64 = 0.03;
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
            },
            sympathetic: 1.0,
            body: 1.0,
            tilt_db: 0.0,
            width: 0.7,
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
    pans: [(f64, f64); KEYS],
    dl: [f64; CHUNK],
    dr: [f64; CHUNK],
    mono: [f64; CHUNK],
    sym_out: [f64; CHUNK],
    sounding: [bool; KEYS],
    sym: Sympathetic,
    board: Soundboard,
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
        let mut c = Chorda {
            fs,
            keyboard: None,
            generation: 0,
            voices,
            hardness: 0.0,
            settings: ChordaSettings::default(),
            lanes: Lanes::new(KEYS, CHUNK),
            scratch: (0..KEYS).map(|_| vec![0.0; CHUNK]).collect(),
            pans: [(0.5, 0.5); KEYS],
            dl: [0.0; CHUNK],
            dr: [0.0; CHUNK],
            mono: [0.0; CHUNK],
            sym_out: [0.0; CHUNK],
            sounding: [false; KEYS],
            sym: Sympathetic::new(&caps, &reference, fs),
            board: Soundboard::new(fs),
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

    pub fn keyboard(&self) -> Option<&Arc<Keyboard>> {
        self.keyboard.as_ref()
    }

    /// The settings, and the felt's hardness (−1…1: softer … harder, 0 the measured felt).
    pub fn set(&mut self, s: &ChordaSettings, hardness: f64) {
        self.settings = *s;
        self.hardness = hardness.clamp(-1.0, 1.0);
        self.gain = OUTPUT_SCALE * math::powf(10.0f64, s.gain_db / 20.0);
        self.sym.level = s.sympathetic;
        self.sym.decay = s.voice.decay;
        self.board.body = s.body;
        self.board.set_tilt(s.tilt_db);
        for (i, p) in self.pans.iter_mut().enumerate() {
            let x = s.width.clamp(0.0, 1.0) * (i as f64 - 43.5) / 43.5;
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
        v
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
        let vs = self.strike_settings(i);
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
        if v.active && v.key_down && v.id == id {
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
            v.release(self.pedal);
        }
    }

    pub fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            v.kill();
        }
        self.sym.reset();
        self.board.reset();
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
    }

    /// Frames skipped while at rest: the board's quarter-rate phase and the sympathetic
    /// strings' control phase move on.
    pub fn skip(&mut self, frames: usize) {
        self.board.skip(frames);
        self.sym.skip(frames);
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
            let n = CHUNK.min(left.len() - done);
            self.chunk(n, par);
            for i in 0..n {
                let (l, r) =
                    self.board
                        .tick(self.dl[i], self.dr[i], self.mono[i] + self.sym_out[i]);
                let (l, r) = ((l * self.gain) as f32, (r * self.gain) as f32);
                left[done + i] += l;
                right[done + i] += r;
                pl = pl.max(l.abs());
                pr = pr.max(r.abs());
            }
            if !self.voices.iter().any(|v| v.active) && self.sym.at_rest() {
                self.board.flush(FLOOR);
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
        for (s, v) in self.sounding.iter_mut().zip(&self.voices) {
            *s = v.active;
        }
        self.sym.set_dampers(self.pedal, &self.sounding);
        let pans = self.pans;
        let active = self.voices.iter().filter(|v| v.active).count();
        let work = self
            .voices
            .iter_mut()
            .zip(self.scratch.iter_mut())
            .zip(self.lanes.split(n))
            .enumerate()
            .filter(|(_, ((v, _), _))| v.active)
            .map(|(i, ((v, buf), (l, r)))| (i, v, buf, l, r));
        let render =
            |(i, v, buf, l, r): (usize, &mut Voice, &mut Vec<f64>, &mut [f32], &mut [f32])| {
                let buf = &mut buf[..n];
                buf.fill(0.0);
                v.render(buf);
                par::clear_lane(l, r);
                let (gl, gr) = pans[i];
                for ((x, a), b) in buf.iter().zip(l.iter_mut()).zip(r.iter_mut()) {
                    *a += (x * gl) as f32;
                    *b += (x * gr) as f32;
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
            if !self.sounding[i] {
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
        self.sym_out[..n].fill(0.0);
        self.sym.process(&self.mono[..n], &mut self.sym_out[..n]);
    }
}
