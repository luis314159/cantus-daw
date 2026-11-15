//! Tympanum's unit (round 40 phase 3, ADR 0124): the acoustic drummer on the audio thread.
//! Owned by `tymp-engine`.
//!
//! A **modelled** kit plays Pulsus's modal voices heard through virtual microphones
//! ([`cantus_dsp::tympanum::model`]); a **recorded** kit plays its pack's strikes, one take
//! drawn per hit for every mic, streamed from disk when Settings stream
//! ([`cantus_dsp::tympanum::rec`]). Either way each microphone is a lane, and the lanes go
//! to their buses in output order ([`InstrumentUnit::process_buses`], ADR 0123): the split
//! sums to the unsplit to the bit.
//!
//! Everything a kit needs is made ready off the audio thread ([`cache`]): `configure` only
//! looks it up (an `Arc` clone) and reads the spec's keys without allocating.

use super::{Controller, InstrumentUnit, NoteOn};
use cantus_core::inst::tympanum::{bleed_key, MicPattern};
use cantus_core::inst::tympanum_kit::{KitDef, MicDef, PieceKind};
use cantus_core::inst_out::{bus_of, output_key, OutputFamily, OutputField};
use cantus_core::{NativeSpec, SampleData};
use cantus_dsp::outmix::OutMix;
use cantus_dsp::tympanum::model::{
    pattern_gain, MicGeom, ModelEngine, PieceGeom, RoomGeom, MAX_PIECES,
};
use cantus_dsp::tympanum::rec::RecEngine;
use cantus_dsp::tympanum::{Outputs, LANE, MAX_MICS};
use std::sync::Arc;

#[path = "tympanum_cache.rs"]
pub mod cache;

/// Files a recorded kit may play (every mic's file of every strike).
pub const MAX_FILES: usize = 32_768;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Mode {
    Silent,
    Modelled,
    Recorded,
}

#[derive(Debug)]
pub struct Unit {
    fs: f32,
    mode: Mode,
    rec: RecEngine,
    model: ModelEngine,
    outs: Outputs,
    /// A recorded kit's files, by index (`t{index}` assets).
    files: Vec<Option<Arc<SampleData>>>,
    /// The modelled kit configured last (its pads and geometry are set when it changes).
    modelled: Option<Arc<cache::Modelled>>,
    /// The recorded kit configured last (fix-e: what `configure_keys` reads its mics from).
    kit_def: Option<Arc<cantus_core::inst::tympanum_kit::RecordedKit>>,
    hat_cc: u8,
    /// A controller arrived (the pedal): not at rest from then on (it holds a value).
    held: bool,
    /// Live hits per key: the ordinal of a hit that has none from the sequencer.
    live: [u32; 128],
    /// The *Feel* card (r40:tymp-engine): each hit's level varied by up to ± `humanize`,
    /// and its time by up to `timing` frames late — seeded by the hit, the same each render.
    humanize: f32,
    timing: f32,
    seed: u32,
    /// Hits waiting for their moment.
    pending: [Pending; MAX_PENDING],
    n_pending: usize,
    /// A recorded kit in one of Cantus's rooms (`room` 1–3): the room, and which mics it
    /// replaces (the kit's own room mics).
    own_room: cantus_dsp::tympanum::room::Room,
    rooms: [bool; cantus_dsp::tympanum::MAX_MICS],
    room_on: bool,
    /// Per key: the player's figure its articulation plays (modelled kits).
    figures: [Figure; 128],
    /// A recorded kit in one of Cantus's rooms (`room` 1–3): Aula's response of that room,
    /// built with the unit off the audio thread (`UnitKind::Drums`), and the room
    /// mics' send.
    aula: Option<Box<cantus_dsp::aula::Aula>>,
    aula_room: u8,
    /// The room the spec chose (`room`), as last configured.
    room_choice: u8,
    send_l: Vec<f32>,
    send_r: Vec<f32>,
}

/// The room of Cantus's a recorded kit plays in (`room` 1–3), `None` for its own room or a
/// modelled kit (whose rooms are its own model). No allocation.
pub fn recorded_room(spec: &NativeSpec) -> Option<u8> {
    let room = spec.params.get("room").copied().unwrap_or(0.0) as u8;
    ((1..=3).contains(&room) && cache::kit_text(spec).starts_with("pack:")).then_some(room)
}

/// fix-e — whole configures Tympanum units have run in this process: what an automated mix
/// must not cost (`configure_keys` sets it alone). Diagnostics, read by the tests.
static CONFIGURES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many whole configures Tympanum units have run in this process (fix-e).
pub fn configures() -> u64 {
    CONFIGURES.load(std::sync::atomic::Ordering::Relaxed)
}

/// Hits the *Feel* card's timing may hold at once.
const MAX_PENDING: usize = 64;
/// A held roll's strokes at most (1.7 s).
const ROLL_STROKES: usize = 32;

/// A hit waiting for its moment.
#[derive(Copy, Clone, Debug, Default)]
struct Pending {
    /// Frames until it sounds.
    left: u32,
    key: u8,
    vel: u8,
    id: u64,
    ordinal: Option<u32>,
    /// A held roll's stroke: what schedules the next when it sounds (fix-e).
    roll: Option<Roll>,
}

/// A held roll, stroke by stroke (fix-e).
#[derive(Copy, Clone, Debug, Default)]
struct Roll {
    /// The stroke this is.
    k: u8,
    /// The note's velocity and ordinal, and the roll's draw.
    velocity: u8,
    ordinal: Option<u32>,
    r: f32,
    /// When this stroke lands, frames after the roll began.
    landed: u32,
}

/// A player's figure an articulation plays (a modelled kit's `flam`, `drag`, `buzz`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Figure {
    None,
    Flam,
    Drag,
    Buzz,
    /// A held roll: strokes alternating hands while the note is held (its note-off ends
    /// the strokes still waiting).
    Roll,
}

/// The buzz's `k`-th bounce's level, `b^k` (deterministic: `cantus_dsp::math`).
fn bounce_gain(b: f32, e: f32) -> f32 {
    cantus_dsp::math::powf(b, e)
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit::with_delays(sample_rate, 0)
    }

    /// A unit whose outputs in `delays` (a bit per output id: ADR 0123 §3's rule, set or
    /// automated) own a delay line; the others own none (allocates: never on the audio
    /// thread).
    pub fn with_delays(sample_rate: f32, delays: u32) -> Self {
        let fs = if sample_rate > 0.0 {
            sample_rate
        } else {
            48_000.0
        };
        let rec = RecEngine::new(fs);
        super::stream::hub().register_board(rec.board());
        Unit {
            fs,
            mode: Mode::Silent,
            rec,
            model: ModelEngine::new(fs),
            outs: Outputs::new(fs, delays),
            files: Vec::with_capacity(MAX_FILES),
            modelled: None,
            kit_def: None,
            hat_cc: 4,
            held: false,
            live: [0; 128],
            humanize: 0.0,
            timing: 0.0,
            seed: 0,
            pending: [Pending::default(); MAX_PENDING],
            n_pending: 0,
            own_room: cantus_dsp::tympanum::room::Room::new(fs),
            rooms: [false; cantus_dsp::tympanum::MAX_MICS],
            room_on: false,
            figures: [Figure::None; 128],
            aula: None,
            aula_room: 0,
            room_choice: 0,
            send_l: vec![0.0; cantus_dsp::tympanum::LANE],
            send_r: vec![0.0; cantus_dsp::tympanum::LANE],
        }
    }

    /// A unit whose recorded kit plays in Cantus's room `room` (1 Small, 2 Studio, 3 Hall):
    /// Aula's response of that room made here, with the unit (allocates: never on the
    /// audio thread).
    pub fn with_room(sample_rate: f32, room: u8) -> Self {
        let mut u = Unit::new(sample_rate);
        let key = match room {
            1 => "small",
            3 => "concert",
            _ => "studio",
        };
        let index = cantus_dsp::aula_gen::FACTORY
            .iter()
            .position(|f| f.key == key)
            .unwrap_or(0);
        let ir = cantus_dsp::aula_gen::factory(index, u.fs);
        let (_, kernel) =
            cantus_dsp::aula::prepare(&ir, u.fs, &cantus_dsp::aula_shape::Shape::default(), None);
        let mut aula = cantus_dsp::aula::Aula::with_kernel(u.fs, std::sync::Arc::new(kernel));
        aula.set(&cantus_dsp::aula::AulaSettings {
            predelay_ms: 0.0,
            width: 1.0,
            mix: 1.0,
            level_db: 0.0,
        });
        aula.reset();
        // Let its mix settle at the wet (its glide), on silence, here.
        let (mut l, mut r) = (vec![0.0; 256], vec![0.0; 256]);
        for _ in 0..40 {
            aula.process(&mut l, &mut r);
        }
        aula.reset();
        u.aula = Some(Box::new(aula));
        u.aula_room = room;
        u
    }

    /// fix-e — the unit `UnitKind::Drums` asks for: its recorded kit in Cantus's room `room`
    /// (0 none) and a delay line for each output in `delays` (a bit per output id; ADR 0123
    /// §3). Off the audio thread.
    pub fn with(sample_rate: f32, room: u8, delays: u32) -> Self {
        let mut u = if room == 0 {
            Unit::new(sample_rate)
        } else {
            Unit::with_room(sample_rate, room)
        };
        if delays != 0 {
            u.outs = Outputs::new(u.fs, delays);
        }
        u
    }

    /// A hit through the *Feel* card: its level varied, its moment perhaps later.
    fn feel(&mut self, key: u8, velocity: u8, id: u64, ordinal: Option<u32>) {
        // A player's figure on its key (a modelled kit's flam, drag, buzz): its strokes,
        // each through the Feel card.
        let fig = self.figures[key as usize & 127];
        if fig != Figure::None {
            let n = ordinal.unwrap_or((id as u32) ^ ((id >> 32) as u32));
            let r = cantus_dsp::tympanum::hit_draw(self.seed ^ 0xF1A3, key, n);
            let ms = |m: f32| (m * 0.001 * self.fs) as u32;
            let mut strokes = [(0u32, 0.0f32); 8];
            let count = match fig {
                // The grace a little before the main stroke, much softer.
                Figure::Flam => {
                    strokes[0] = (0, 0.4);
                    strokes[1] = (ms(18.0 + 10.0 * r), 1.0);
                    2
                }
                // Two graces, then the stroke.
                Figure::Drag => {
                    strokes[0] = (0, 0.36);
                    strokes[1] = (ms(11.0 + 3.0 * r), 0.4);
                    strokes[2] = (ms(36.0 + 6.0 * r), 1.0);
                    3
                }
                // The stick pressed into the head: bounces quicker and softer each.
                Figure::Buzz => {
                    let mut t = 0.0;
                    for (k, s) in strokes.iter_mut().take(8).enumerate() {
                        *s = (ms(t), bounce_gain(0.78, k as f32));
                        t += 21.0 * (1.0 - 0.05 * k as f32) * (0.9 + 0.2 * r);
                    }
                    8
                }
                // A held roll (fix-e): one stroke waits at a time, each scheduling the
                // next as it sounds — overlapping rolls never fill the queue.
                Figure::Roll => {
                    let roll = Roll {
                        k: 0,
                        velocity,
                        ordinal,
                        r,
                        landed: 0,
                    };
                    let (v, o, after) = self.roll_stroke(&roll, 0);
                    self.stroke(key, v, id, o, after, Some(roll));
                    return;
                }
                Figure::None => 0,
            };
            for (k, (at, g)) in strokes.iter().take(count).enumerate() {
                let v = ((velocity as f32 * g).round()).clamp(1.0, 127.0) as u8;
                let o = ordinal.map(|o| o.wrapping_mul(32).wrapping_add(k as u32));
                self.stroke(key, v, id, o, *at, None);
            }
            return;
        }
        self.stroke(key, velocity, id, ordinal, 0, None);
    }

    /// A held roll's stroke `k`: a double-stroke roll, a stroke every 55 ms, each pair's
    /// second a little softer — its velocity, ordinal and frames after the roll began.
    fn roll_stroke(&self, roll: &Roll, k: u8) -> (u8, Option<u32>, u32) {
        let g = if k.is_multiple_of(2) { 0.85 } else { 0.7 };
        let v = ((roll.velocity as f32 * g).round()).clamp(1.0, 127.0) as u8;
        let o = roll
            .ordinal
            .map(|o| o.wrapping_mul(32).wrapping_add(u32::from(k)));
        let after = ((55.0 * k as f32 + 4.0 * roll.r) * 0.001 * self.fs) as u32;
        (v, o, after)
    }

    /// The Feel card on a stroke meant `after` frames from now: its velocity and when it
    /// lands (a pure function of the hit, the same each render).
    fn feel_of(
        &self,
        key: u8,
        velocity: u8,
        id: u64,
        ordinal: Option<u32>,
        after: u32,
    ) -> (u8, u32) {
        if self.humanize <= 0.0 && self.timing <= 0.0 {
            return (velocity, after);
        }
        let n = ordinal.unwrap_or((id as u32) ^ ((id >> 32) as u32));
        let vel = if self.humanize > 0.0 {
            let r = 2.0 * cantus_dsp::tympanum::hit_draw(self.seed ^ 0x5EED_0001, key, n) - 1.0;
            (velocity as f32 * (1.0 + self.humanize * r))
                .round()
                .clamp(1.0, 127.0) as u8
        } else {
            velocity
        };
        let late = after
            + if self.timing > 0.0 {
                (cantus_dsp::tympanum::hit_draw(self.seed ^ 0x5EED_0002, key, n) * self.timing)
                    as u32
            } else {
                0
            };
        (vel, late)
    }

    /// One stroke, `after` frames from now (from the roll's start for a roll's stroke),
    /// through the Feel card. A stroke that cannot wait (the queue full) is **dropped**,
    /// never struck early (fix-e); the hit's own stroke (`after` 0) then sounds at its
    /// written moment, without the card's lateness.
    fn stroke(
        &mut self,
        key: u8,
        velocity: u8,
        id: u64,
        ordinal: Option<u32>,
        after: u32,
        roll: Option<Roll>,
    ) {
        let (vel, late) = self.feel_of(key, velocity, id, ordinal, after);
        let wait = late.saturating_sub(roll.map_or(0, |r| r.landed));
        if wait > 0 && self.n_pending < MAX_PENDING {
            self.pending[self.n_pending] = Pending {
                left: wait,
                key,
                vel,
                id,
                ordinal,
                roll: roll.map(|r| Roll { landed: late, ..r }),
            };
            self.n_pending += 1;
            return;
        }
        if wait > 0 && after > 0 {
            return;
        }
        self.strike(key, vel, id, ordinal);
        if let Some(r) = roll {
            self.next_roll(key, id, Roll { landed: late, ..r });
        }
    }

    /// A roll's stroke `roll.k` has sounded: the next one waits (32 at most, 1.7 s; a
    /// longer roll is written as several notes; the note-off stops what is left).
    fn next_roll(&mut self, key: u8, id: u64, roll: Roll) {
        let k = roll.k + 1;
        if usize::from(k) >= ROLL_STROKES {
            return;
        }
        let next = Roll { k, ..roll };
        let (v, o, after) = self.roll_stroke(&next, k);
        self.stroke(key, v, id, o, after, Some(next));
    }

    /// Strike every waiting hit whose moment has come; frames to the next one.
    fn due(&mut self) -> u32 {
        let mut i = 0;
        let mut next = u32::MAX;
        while i < self.n_pending {
            let p = self.pending[i];
            if p.left == 0 {
                self.pending[i] = self.pending[self.n_pending - 1];
                self.n_pending -= 1;
                self.strike(p.key, p.vel, p.id, p.ordinal);
                if let Some(r) = p.roll {
                    // Its next stroke joins the queue (visited below: it waits).
                    self.next_roll(p.key, p.id, r);
                }
            } else {
                next = next.min(p.left);
                i += 1;
            }
        }
        next
    }

    pub fn sample_rate(&self) -> f32 {
        self.fs
    }

    fn source(files: &[Option<Arc<SampleData>>], i: u32) -> Option<cantus_dsp::exemplar::Source> {
        let d = files.get(i as usize)?.as_ref()?;
        if let Some(st) = super::stream::lookup(d) {
            return Some(cantus_dsp::exemplar::Source::from_stream(&st));
        }
        let mut s = cantus_dsp::exemplar::Source {
            channels: d.channels.clamp(1, 2) as usize,
            rate: d.rate as f32,
            frames: d.len_frames(),
            ..Default::default()
        };
        s.levels[0] = Some(d.frames.clone());
        Some(s)
    }

    fn strike(&mut self, key: u8, velocity: u8, id: u64, ordinal: Option<u32>) {
        #[cfg(test)]
        fixe_tests::STRUCK.with(|s| s.borrow_mut().push((id, key)));
        match self.mode {
            Mode::Silent => {}
            Mode::Modelled => self.model.note_on(key, velocity, id, ordinal),
            Mode::Recorded => {
                let ordinal = ordinal.unwrap_or_else(|| {
                    let k = key as usize & 127;
                    self.live[k] = self.live[k].wrapping_add(1);
                    u32::MAX / 4 + self.live[k]
                });
                let files = &self.files;
                self.rec
                    .note_on(key, velocity, id, ordinal, &|i| Self::source(files, i));
            }
        }
    }
}

/// A mic's level reference (see [`MicGeom::norm`]): the direct gain of its own piece — or
/// of the kit's centre, between its kick and snare — at its default place.
pub fn mic_norm(def: &KitDef, m: &MicDef) -> f32 {
    let target = match m.main_for.first() {
        Some(p) => def
            .pieces
            .iter()
            .find(|x| x.index == *p)
            .map(|x| x.place.at()),
        None => {
            let at = |k: PieceKind| {
                def.pieces
                    .iter()
                    .find(|x| x.kind == k)
                    .map(|x| x.place.at())
            };
            match (at(PieceKind::Kick), at(PieceKind::Snare)) {
                (Some(a), Some(b)) => Some([
                    (a[0] + b[0]) / 2.0,
                    (a[1] + b[1]) / 2.0,
                    (a[2] + b[2]) / 2.0,
                ]),
                (a, b) => a.or(b),
            }
        }
    };
    let Some(t) = target else {
        return 1.0;
    };
    let p = m.place.at();
    let v = [
        (p[0] - t[0]) / 100.0,
        (p[1] - t[1]) / 100.0,
        (p[2] - t[2]) / 100.0,
    ];
    let r = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(0.03);
    let d = m.place.dir();
    let cos = -(d[0] * v[0] + d[1] * v[1] + d[2] * v[2]) / r;
    let g = pattern_index(m.pattern);
    (pattern_gain(g, cos).abs().max(0.2) / r).max(0.05)
}

fn pattern_index(p: MicPattern) -> u8 {
    match p {
        MicPattern::Omni => 0,
        MicPattern::Cardioid => 1,
        MicPattern::Supercardioid => 2,
        MicPattern::Hypercardioid => 3,
        MicPattern::Figure8 => 4,
    }
}

fn db(x: f32) -> f32 {
    if x == 0.0 {
        1.0
    } else {
        cantus_dsp::math::db_to_gain(x)
    }
}

#[path = "tympanum_unit.rs"]
mod unit;
pub use unit::METER_TEL;

#[cfg(test)]
#[path = "tympanum_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tympanum_fixe_tests.rs"]
mod fixe_tests;
