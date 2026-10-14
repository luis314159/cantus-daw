//! Exemplar's engine: configuration, notes, triggers, choke and stealing. Rendering is in
//! `exemplar_render.rs`.

use super::voice::{EnvShape, Layer};
use super::*;
use crate::math;
use crate::modmatrix::ModRoute;
use crate::voices::Action;

#[path = "exemplar_render.rs"]
mod render;

#[path = "exemplar_trigger.rs"]
mod trigger;

pub use trigger::crossfade;

/// Ids at or above this are live input (one per key, `cantus_engine`'s `native_id`).
pub const LIVE_IDS: u64 = u64::MAX - 256;
/// Ids of controller triggers (counted from here), below the live notes'.
const CC_IDS: u64 = LIVE_IDS - (1 << 32);
/// A layer no longer tied to a note (released by all-notes-off, or stolen and fading), or
/// one a controller started.
const ORPHAN: usize = usize::MAX;

/// SplitMix64: a well-mixed number from a note's id and key — the seed of every choice.
pub fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The note's random number in [0, 1), from its id and key (SFZ's `lorand`/`hirand`: one
/// per note, shared by every zone, so layered randoms stay coherent).
pub fn note_random(id: u64, key: u8) -> f32 {
    (mix(id ^ ((key as u64) << 56) ^ 0x5EED) >> 40) as f32 / (1u64 << 24) as f32
}

fn db(x: f32) -> f32 {
    math::powf(10f32, x / 20.0)
}

/// Balance: centre is unity on both sides, never louder than the source.
fn balance(pan: f32) -> [f32; 2] {
    let p = pan.clamp(-1.0, 1.0);
    [1.0 - p.max(0.0), 1.0 + p.min(0.0)]
}

impl Exemplar {
    /// Allocates every table: call off the audio thread.
    pub fn new(fs: f32) -> Exemplar {
        let fs = if fs > 0.0 { fs } else { 48_000.0 };
        let board = std::sync::Arc::new(super::stream::Board::new(MAX_LAYERS));
        let mut layers = vec![Layer::default(); MAX_LAYERS].into_boxed_slice();
        // Each layer's window for a streamed sample: a control step and a 5 ms fade ahead,
        // twice over (a level is read at up to twice the rate), the taps and the decimator.
        let fade = (STEAL_MS * fs / 1000.0).ceil() as usize;
        let ahead = super::voice::CONTROL as usize + fade;
        let frames =
            (2 * ahead + 2 * resample::MAX_TAPS + 4 * super::play::HB_M + 64).next_multiple_of(256);
        for (i, l) in layers.iter_mut().enumerate() {
            l.play.attach_window(frames, fade, board.clone(), i as u16);
            l.play.set_head_only_above(head_only_above(fs));
        }
        Exemplar {
            fs,
            readers: [
                Reader::new(Quality::Draft, fs),
                Reader::new(Quality::Standard, fs),
                Reader::new(Quality::High, fs),
                Reader::new(Quality::Best, fs),
            ],
            globals: Globals::default(),
            groups: vec![GroupParams::default(); MAX_GROUPS].into_boxed_slice(),
            zones: vec![ZoneParams::default(); MAX_ZONES].into_boxed_slice(),
            sources: vec![Source::default(); MAX_ZONES].into_boxed_slice(),
            order: vec![0u16; MAX_ZONES].into_boxed_slice(),
            order_len: 0,
            layers,
            slots: [Slot::default(); MAX_VOICES],
            alloc: Allocator::new(32),
            matrix: ModMatrix::default(),
            mw: 0.0,
            at: 0.0,
            bend: 0.0,
            expr: 1.0,
            sus: 0.0,
            keyswitch: -1,
            live_rr: [0; 128],
            cc_last: [0; 128],
            cc_listen: 0,
            cc_count: 0,
            clock: 0,
            serial: 0,
            stolen: 0,
            keys_down: 0,
            released_buf: [0; MAX_VOICES],
            peak: 0.0,
            lanes: crate::par::Lanes::new(LANES, SPAN),
            lane_end: [u16::MAX; LANES],
            ended: vec![false; MAX_LAYERS].into_boxed_slice(),
            board,
            starts: std::sync::Arc::new(super::stream::Board::new_pins(MAX_ZONES)),
            late: 0,
            cut: 0,
            wakes: 0,
        }
    }

    /// Where its zones say which blocks beyond their heads a note may start in.
    pub fn starts(&self) -> &std::sync::Arc<super::stream::Board> {
        &self.starts
    }

    /// Say, for every zone on a streamed sample, the blocks beyond its head a note may
    /// start in (its start, plus the velocity's), and two more: the reader keeps them in
    /// the pool. Call after the zones and globals are set (configure): allocation-free.
    pub fn publish_starts(&mut self) {
        use super::stream::{StartPin, BLOCK};
        for zi in 0..MAX_ZONES {
            self.starts.set(zi, 0);
        }
        for oi in 0..self.order_len {
            let zi = self.order[oi] as usize;
            let src = &self.sources[zi];
            let Some(st) = src.stream.as_ref() else {
                continue;
            };
            let start = self.zones[zi].region.start.max(0) as usize;
            let vel = (self.globals.start_vel_ms.max(0.0) * src.rate / 1000.0).ceil();
            let end = start + vel as usize;
            let head = st.head_frames(0);
            if end + 2 * BLOCK <= head {
                continue;
            }
            let lo = start.max(head) / BLOCK;
            let hi = ((end + 2 * BLOCK).min(st.frames[0].max(1)) - 1) / BLOCK;
            // The levels a note of this zone can start at: its top key, tunes, transpose and
            // a full bend up (the resampler's choice: level k for a ratio in [2^k, 2^(k+1))),
            // within the levels the zone has.
            let z = &self.zones[zi];
            let g = &self.globals;
            let shift = (z.hi as f32 - z.root as f32).max(0.0) * z.keytrack / 100.0
                + (z.tune_ct + self.groups[z.group as usize].tune_ct + g.tune_ct) / 100.0
                + g.transpose
                + g.bend_range.max(0.0);
            let ratio = src.rate / self.fs * crate::math::exp2(shift / 12.0);
            let reach = if ratio >= 2.0 {
                crate::math::log2(ratio).floor() as usize
            } else {
                0
            };
            let pin = StartPin {
                stream: st.id,
                block: lo,
                count: (hi + 1).saturating_sub(lo).clamp(1, 255),
                levels: reach.min(src.top_level()) + 1,
            };
            self.starts.set(zi, pin.pack());
        }
    }

    /// Where its layers say what they stream (the reader scans it; ADR 0050).
    pub fn board(&self) -> &std::sync::Arc<super::stream::Board> {
        &self.board
    }

    /// Offline: a streamed voice waits for its blocks rather than fade (ADR 0050 §6).
    pub fn set_blocking(&mut self, on: bool) {
        for l in self.layers.iter_mut() {
            l.play.set_blocking(on);
        }
    }

    /// Streamed voices that ended early because a block came late, since the last call.
    pub fn take_late(&mut self) -> u32 {
        std::mem::take(&mut self.late)
    }

    /// Notes that played their head only — read faster than streaming serves (third review
    /// #9) — since the last call. Never late: the same offline and in real time.
    pub fn take_cut(&mut self) -> u32 {
        std::mem::take(&mut self.cut)
    }

    /// Times its voices woke the stream reader — a non-blocking system call on the audio
    /// thread (ADR 0050 §4, fourth review #1) — since the last call.
    pub fn take_wakes(&mut self) -> u32 {
        std::mem::take(&mut self.wakes)
    }

    pub fn sample_rate(&self) -> f32 {
        self.fs
    }

    pub fn globals(&self) -> &Globals {
        &self.globals
    }

    pub fn set_globals(&mut self, g: &Globals) {
        if g.voices != self.globals.voices {
            self.alloc.set_polyphony(g.voices);
        }
        if g.mode != self.alloc.mode() {
            self.alloc.set_mode(g.mode, crate::voices::Priority::Last);
        }
        self.globals.clone_from(g);
    }

    pub fn set_group(&mut self, m: usize, g: &GroupParams) {
        if let Some(slot) = self.groups.get_mut(m) {
            slot.clone_from(g);
        }
    }

    /// Forget the zone list and let go of their sources (second review #2: a removed
    /// zone's stream is released while the instrument lives). Allocation-free, and never
    /// the last drop on the audio thread: the sample cache and the stream hub hold them.
    pub fn clear_zones(&mut self) {
        for oi in 0..self.order_len {
            let zi = self.order[oi] as usize;
            self.sources[zi] = Source::default();
        }
        self.order_len = 0;
        self.cc_listen = 0;
    }

    /// Add zone `n`. Call in ascending `n`; the audio thread may call it (no allocation:
    /// `Source` holds `Arc`s, whose drop here is never the last).
    pub fn put_zone(&mut self, n: usize, z: &ZoneParams, src: &Source) {
        if n >= MAX_ZONES || self.order_len >= MAX_ZONES {
            return;
        }
        self.zones[n].clone_from(z);
        self.sources[n].clone_from(src);
        if z.trig == Trig::Controller {
            self.cc_listen |= 1u128 << z.cc.min(127);
        }
        self.order[self.order_len] = n as u16;
        self.order_len += 1;
    }

    pub fn zone_count(&self) -> usize {
        self.order_len
    }

    pub fn clear_matrix(&mut self) {
        self.matrix.clear();
    }

    pub fn add_route(&mut self, r: ModRoute) -> bool {
        self.matrix.push(r)
    }

    pub fn set_macro(&mut self, m: usize, v: f32) {
        if let Some(x) = self.globals.macros.get_mut(m) {
            *x = v;
        }
    }

    /// Controllers: `which` 0 mod wheel, 1 pressure, 2 bend (semitones at ±2 full scale),
    /// 3 expression, 4 sustain pedal.
    pub fn controller(&mut self, which: u8, v: f32) {
        match which {
            0 => self.mw = v,
            1 => self.at = v,
            2 => self.bend = v,
            3 => self.expr = v,
            4 => {
                self.sus = v;
                let n = self.alloc.set_sustain(v >= 0.5, &mut self.released_buf);
                for i in 0..n {
                    let voice = self.released_buf[i];
                    self.release_slot(voice, 0.0, false);
                }
            }
            _ => {}
        }
    }

    /// A MIDI controller's new value (0…127): the zones it crosses into start on the next
    /// sample, and the looping layers it started whose range it leaves are released.
    pub fn cc(&mut self, n: u8, v: u8) {
        let (n, v) = (n.min(127), v.min(127));
        let prev = std::mem::replace(&mut self.cc_last[n as usize], v);
        if self.cc_listen & (1u128 << n) == 0 || prev == v {
            return;
        }
        self.cc_left(n, v);
        self.cc_entered(n, prev, v);
    }

    fn in_switch_range(&self, key: u8) -> bool {
        let g = &self.globals;
        g.sw_lo >= 0 && (key as i16) >= g.sw_lo && (key as i16) <= g.sw_hi.max(g.sw_lo)
    }

    pub fn note_on(&mut self, key: u8, vel: f32, id: u64) {
        let key = key.min(127);
        if self.in_switch_range(key) {
            self.keyswitch = key as i16;
            return;
        }
        let legato = self.keys_down > 0;
        self.keys_down += 1;
        match self.alloc.note_on(key, id, vel) {
            Action::Start { voice, stolen, .. } => {
                if stolen {
                    self.stolen += 1;
                    self.detach_slot(voice, true);
                }
                let rr = if id >= LIVE_IDS {
                    let c = self.live_rr[key as usize];
                    self.live_rr[key as usize] = c.wrapping_add(1);
                    c
                } else {
                    (mix(id) >> 33) as u32
                };
                self.slots[voice] = Slot {
                    key,
                    id,
                    vel,
                    start: self.clock,
                    rr,
                    rnd: note_random(id, key),
                    key_down: true,
                    released: false,
                    reached: 0.0,
                    layers: 0,
                };
                let first = if legato { Trig::Legato } else { Trig::First };
                self.start_zones(voice, first, 1.0);
            }
            Action::Glide {
                voice,
                key: to,
                id: to_id,
            } => self.glide_slot(voice, to, to_id),
            _ => {}
        }
    }

    pub fn note_off(&mut self, key: u8, id: u64, relvel: f32) {
        if self.in_switch_range(key) {
            return;
        }
        self.keys_down = self.keys_down.saturating_sub(1);
        let mine = (0..MAX_VOICES).find(|v| {
            let s = &self.slots[*v];
            s.key_down && s.key == key && s.id == id
        });
        match self.alloc.note_off(key, id) {
            Action::Release { voice } => {
                self.slots[voice].key_down = false;
                self.release_slot(voice, relvel, true);
            }
            Action::Glide {
                voice,
                key: to,
                id: to_id,
            } => self.glide_slot(voice, to, to_id),
            _ => {
                // Held on by the pedal: the key-up release samples sound now.
                if let Some(v) = mine {
                    self.slots[v].key_down = false;
                    self.start_zones_of(v, Trig::ReleaseKey, relvel);
                }
            }
        }
    }

    /// Release every note without release samples (transport stop, loop wrap).
    pub fn all_notes_off(&mut self) {
        for l in self.layers.iter_mut().filter(|l| l.active) {
            if !l.one_shot {
                l.env.release();
                l.env2.release();
                l.play.release_loop();
            }
            l.released = true;
            l.slot = ORPHAN;
        }
        for s in &mut self.slots {
            *s = Slot::default();
        }
        self.alloc.clear();
        self.keys_down = 0;
        self.cc_count = 0;
    }

    /// Silence at once and forget every tail and every state that depends on the past.
    pub fn reset(&mut self) {
        for l in self.layers.iter_mut() {
            l.active = false;
            l.play.let_go();
        }
        for s in &mut self.slots {
            *s = Slot::default();
        }
        self.alloc.clear();
        self.keys_down = 0;
        self.keyswitch = -1;
        self.live_rr = [0; 128];
        self.cc_last = [0; 128];
        self.cc_count = 0;
        self.mw = 0.0;
        self.at = 0.0;
        self.bend = 0.0;
        self.expr = 1.0;
        self.sus = 0.0;
        self.peak = 0.0;
    }

    /// At rest (ADR 0031 §5.1): no layer sounding, no note or key held, the wheels and
    /// pedals where a reset leaves them, no keyswitch in force (a new unit has none: the
    /// next note would pick other zones) and no live note counted (live notes round-robin
    /// by their count; a clip's notes by their id, which needs no memory), no controller a
    /// zone listens to away from 0 and no controller trigger counted since the last stop
    /// (both choose what the next one starts). The clock only times how long a note was
    /// held, so skipping it is exact.
    pub fn at_rest(&self) -> bool {
        !self.layers.iter().any(|l| l.active)
            && self.alloc.is_clear()
            && self.keys_down == 0
            && self.keyswitch == -1
            && self.live_rr.iter().all(|c| *c == 0)
            && self.cc_count == 0
            && (0..128).all(|n| self.cc_listen & (1u128 << n) == 0 || self.cc_last[n] == 0)
            && self.mw == 0.0
            && self.at == 0.0
            && self.bend == 0.0
            && self.expr == 1.0
            && self.sus == 0.0
    }

    /// Samples skipped while at rest (ADR 0031 §5.1): the clock moves on.
    pub fn skip(&mut self, frames: usize) {
        self.clock += frames as u64;
    }

    /// Voices (notes) sounding, and layers sounding.
    pub fn active_notes(&self) -> usize {
        self.alloc.active()
    }

    pub fn active_layers(&self) -> usize {
        self.layers.iter().filter(|l| l.active).count()
    }

    pub fn polyphony(&self) -> usize {
        self.alloc.polyphony()
    }

    /// The keyswitch in force (−1: none pressed and no default).
    pub fn keyswitch(&self) -> i16 {
        if self.keyswitch >= 0 {
            self.keyswitch
        } else {
            self.globals.sw_default
        }
    }

    pub fn take_stolen(&mut self) -> u32 {
        std::mem::take(&mut self.stolen)
    }

    /// The loudest output sample since the last call.
    pub fn take_peak(&mut self) -> f32 {
        std::mem::take(&mut self.peak)
    }

    /// What each sounding layer is playing, for the editor: into `out`, returns how many.
    pub fn layer_info(&self, out: &mut [LayerInfo]) -> usize {
        let mut n = 0;
        for l in self.layers.iter().filter(|l| l.active) {
            if n == out.len() {
                break;
            }
            out[n] = l.info();
            n += 1;
        }
        n
    }
}

/// An instrument that goes zeroes its boards (second review #1): a copy the stream reader
/// still holds until it next looks wants nothing of it.
impl Drop for Exemplar {
    fn drop(&mut self) {
        for b in [&self.board, &self.starts] {
            for i in 0..b.len() {
                b.set(i, 0);
            }
        }
    }
}

/// The local ratio above which a streamed voice plays its head only (third review #8): one
/// millisecond of its reads at most a quarter of what the reader keeps ready ahead — the
/// reader looks every millisecond while anything streams — 64 at 48 kHz.
pub fn head_only_above(fs: f32) -> f64 {
    let ahead = (super::stream::READ_AHEAD * super::stream::BLOCK) as f64;
    ahead / 4.0 / (fs.max(1.0) as f64 / 1_000.0)
}
