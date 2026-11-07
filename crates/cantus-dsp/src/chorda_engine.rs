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
/// would pass full scale. Above `KNEE` the output bends smoothly toward `CEILING`
/// (memoryless, the same at any block size); only fortissimo chords' attacks reach it.
const KNEE: f64 = 0.6;
const CEILING: f64 = 0.92;

/// The ceiling's curve: unity below the knee, a tanh above it, never past the ceiling.
#[inline]
pub fn ceiling(x: f64) -> f64 {
    let a = x.abs();
    if a <= KNEE {
        x
    } else {
        let room = CEILING - KNEE;
        (KNEE + room * math::tanh((a - KNEE) / room)).copysign(x)
    }
}

/// Model 2's room's default level (the `room` setting's default).
pub const ROOM: f64 = 0.35;
/// Model 2's quiet floor (N at the bridge): about −115 dBFS at the staged output (1e-4 N ×
/// the output scale 0.0244), some 20 dB under a 16-bit
/// file's floor — what lets its longer decays (up to four times model 1's in the treble)
/// still end, and the instrument rest, as soon as the family's contract asks.
const MODEL_2_QUIET: f64 = 1.0e-4;

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
    room: Room,
    stage: Stage,
    voicing: Voicing,
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
            body: Body::new(fs, &Voicing::default().body),
            room: Room::new(fs, &Voicing::default().room),
            stage: Stage::new(fs),
            voicing: Voicing::default(),
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
                self.room.reset();
                self.stage.reset();
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
            v.quiet = MODEL_2_QUIET;
            self.sym.quiet_floor = MODEL_2_QUIET;
            self.room.floor = MODEL_2_QUIET;
        } else {
            (v.blow, v.knock, v.landing, v.vary) = (0.0, 0.0, 0.0, 0.0);
            (v.b1_scale, v.b2_scale) = (1.0, 1.0);
            v.quiet = 1.0e-10;
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
            if self.voicing.vmin != 1.0 {
                v.v_min *= self.voicing.vmin;
            }
        }
        v
    }

    /// Model 2's voicing (ADR 0098): the shipped one unless a tool tries another.
    /// Allocates (the body's responses are synthesised): never on the audio thread.
    pub fn set_voicing(&mut self, v: &Voicing) {
        self.voicing = *v;
        self.body = Body::new(self.fs, &v.body);
        self.room = Room::new(self.fs, &v.room);
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

    pub fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            v.kill();
        }
        self.sym.reset();
        self.board.reset();
        self.body.reset();
        self.room.reset();
        self.stage.reset();
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
            && self.room.at_rest()
            && self.stage.at_rest()
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
                let sym = self.sym_out[i];
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
                let (l, r) = if model2 {
                    (ceiling(l * self.gain) as f32, ceiling(r * self.gain) as f32)
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
                    self.stage.flush(FLOOR);
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
