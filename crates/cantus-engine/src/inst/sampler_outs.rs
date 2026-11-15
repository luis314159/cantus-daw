//! Exemplar's outputs (round 40 phase 3, ADR 0123, 0125): each output's internal mix
//! (`o{n}.*`), the bus it plays on, its delay line, and the Cantus room a recorded
//! instrument may add after it (`room.add`).
//!
//! **Nothing allocated for what is not used.** A delay line exists only for an output whose
//! `o{n}.delay` is not 0 or that a lane automates (fix-e: a delay automated up from 0 then
//! delays), and the room only when `room.add` names one: [`delay_mask`],
//! [`automated_delays`] and [`wants_room`] say so from the snapshot (`UnitKind::of_track`),
//! the engine builds a unit with exactly those (off the audio thread, `UnitKind::Exemplar`)
//! and swaps it in with the snapshot. A unit with no delay and no room owns none — it
//! renders to the bit what it did before. [`delay_mask`] | [`automated_delays`] is the
//! shared rule (Tympanum sizes its lines by it, `UnitKind::Drums`).

use cantus_core::inst_out::{bus_of, output_key, OutputField, OutputId, MAX_OUTPUTS};
use cantus_core::NativeSpec;
use cantus_dsp::exemplar::Exemplar;
use cantus_dsp::math;
use cantus_dsp::outmix::OutMix;
use cantus_dsp::par::add_into;
use cantus_dsp::pulsus::BusOut;
use cantus_dsp::tympanum::room::Room;
use cantus_dsp::tympanum::OutDelay;

/// The added room's choice: 0 none, 1 a small room, 2 a studio, 3 a hall (Tympanum's rooms).
pub const ROOM_KEY: &str = "room.add";
/// The added room's level (dB).
pub const ROOM_LEVEL_KEY: &str = "room.level";
/// The frames of a zero-fed delay tail done at once.
const CHUNK: usize = 256;

/// A bit for each output (below 32) whose `o{n}.delay` is not 0 — the delay lines the
/// instrument needs. Read without allocating.
pub fn delay_mask(spec: &NativeSpec) -> u32 {
    let mut m = 0u32;
    for (k, v) in &spec.params {
        if let Some((n, OutputField::Delay)) = output_key(k) {
            if *v != 0.0 && n < 32 {
                m |= 1 << n;
            }
        }
    }
    m
}

/// fix-e (ADR 0123 §3) — a bit for each output (below 32) whose `o{n}.delay` a lane of
/// `automation` drives on `track` (a modulator drives a lane too): its line exists from
/// the start, even at 0, so a delay automated up from 0 delays. Read without allocating.
pub fn automated_delays(
    automation: &[cantus_core::automation::AutoLane],
    track: cantus_core::TrackId,
) -> u32 {
    let mut m = 0u32;
    for l in automation {
        if let cantus_core::automation::AutoTarget::Instrument { track: t, key } = &l.target {
            if *t == track {
                if let Some((n, OutputField::Delay)) = output_key(key) {
                    if n < 32 {
                        m |= 1 << n;
                    }
                }
            }
        }
    }
    m
}

/// Whether the spec adds a room after its outputs.
pub fn wants_room(spec: &NativeSpec) -> bool {
    spec.params.get(ROOM_KEY).is_some_and(|v| *v >= 0.5)
}

/// The room `choice` adds: its mean size (m) and RT60 (s) — Tympanum's small room, studio
/// and hall (`tympanum_unit::room_geom`'s boxes).
pub fn room_of(choice: u8) -> Option<(f32, f32)> {
    let (w, d, h, t60): (f32, f32, f32, f32) = match choice {
        1 => (3.0, 3.5, 2.5, 0.25),
        2 => (6.0, 8.0, 3.4, 0.45),
        3 => (14.0, 20.0, 8.2, 1.6),
        _ => return None,
    };
    Some((math::cbrt(w * d * h), t60))
}

#[derive(Clone, Debug)]
pub(super) struct Outs {
    pub mix: [OutMix; MAX_OUTPUTS],
    pub bus: [u8; MAX_OUTPUTS],
    /// Render output by output: the groups name two or more, a mix is not unity, an output
    /// plays on a strip of its own, a delay or a room. Otherwise the instrument renders
    /// exactly as it always did.
    pub by_output: bool,
    /// Each output's peak of the last block (telemetry).
    pub peak: [f32; MAX_OUTPUTS],
    /// The delay lines this unit was built with: (output, line).
    lines: Vec<(usize, OutDelay)>,
    /// How far into the block each line has been fed.
    fed: Vec<usize>,
    /// Two halves of [`CHUNK`]: the zeros a line is fed while its output is silent.
    scratch: Vec<f32>,
    room: Option<Box<Room>>,
    /// The block's mono sum into the room.
    room_in: Vec<f32>,
    room_gain: f32,
    fs: f32,
    /// The `o{n}.*` keys as the spec stores them (before a solo mutes the others), each
    /// output's delay and soloed: what [`Self::configure_keys`] updates alone.
    keys: [OutMix; MAX_OUTPUTS],
    solo: [bool; MAX_OUTPUTS],
    delay_ms: [f32; MAX_OUTPUTS],
    /// The groups name two or more outputs, or an output plays on a strip.
    split: bool,
    roomed: bool,
}

impl Outs {
    /// Room for the lines in `delays` (a bit per output) and, if `room`, the added room.
    /// Allocates: with the unit, off the audio thread. `Outs::new(fs, 0, false)` holds none.
    pub fn new(fs: f32, delays: u32, room: bool) -> Outs {
        let lines: Vec<(usize, OutDelay)> = (0..32usize)
            .filter(|n| delays & (1 << n) != 0)
            .map(|n| (n, OutDelay::new(fs)))
            .collect();
        let any = !lines.is_empty();
        Outs {
            mix: [OutMix::default(); MAX_OUTPUTS],
            bus: [0; MAX_OUTPUTS],
            by_output: false,
            peak: [0.0; MAX_OUTPUTS],
            fed: vec![0; lines.len()],
            lines,
            scratch: if any {
                vec![0.0; 2 * CHUNK]
            } else {
                Vec::new()
            },
            room: room.then(|| Box::new(Room::new(fs))),
            room_in: if room {
                vec![0.0; crate::messages::MAX_BLOCK_FRAMES]
            } else {
                Vec::new()
            },
            room_gain: 0.0,
            fs,
            keys: [OutMix::default(); MAX_OUTPUTS],
            solo: [false; MAX_OUTPUTS],
            delay_ms: [0.0; MAX_OUTPUTS],
            split: false,
            roomed: false,
        }
    }

    /// From the spec's `o{n}.*` keys, its slots and its room, without allocating.
    pub fn configure(&mut self, spec: &NativeSpec, groups_differ: bool) {
        self.keys = [OutMix::default(); MAX_OUTPUTS];
        self.solo = [false; MAX_OUTPUTS];
        self.delay_ms = [0.0; MAX_OUTPUTS];
        for (k, v) in &spec.params {
            if let Some((n, f)) = output_key(k) {
                self.put(n, f, *v);
            }
        }
        for (i, b) in self.bus.iter_mut().enumerate() {
            *b = bus_of(&spec.outputs, OutputId(i as u16));
        }
        self.room(spec);
        self.split = groups_differ || !spec.outputs.is_empty();
        self.derive();
    }

    /// The added room from the spec (`room.add`, `room.level`).
    fn room(&mut self, spec: &NativeSpec) {
        let choice = spec.params.get(ROOM_KEY).map_or(0, |v| v.round() as u8);
        self.roomed = false;
        if let (Some(r), Some((size, t60))) = (self.room.as_mut(), room_of(choice)) {
            r.set(size, t60, t60 * 0.55);
            let db = spec.params.get(ROOM_LEVEL_KEY).copied().unwrap_or(-12.0);
            self.room_gain = math::db_to_gain(db);
            self.roomed = true;
        } else {
            self.room_gain = 0.0;
        }
    }

    /// fix-e — the automation moved `keys` (each in `spec`): the `o{n}.*` keys and the
    /// added room's level among them are set alone (any other key is the caller's), and
    /// the outputs are what [`Self::configure`] would make of them, to the bit.
    pub fn configure_keys(&mut self, spec: &NativeSpec, keys: &[String]) {
        for k in keys {
            if let Some((n, f)) = output_key(k) {
                self.put(n, f, spec.get(k));
            } else if k == ROOM_LEVEL_KEY {
                self.room(spec);
            }
        }
        self.derive();
    }

    /// One `o{n}.*` key as the spec stores it.
    fn put(&mut self, n: usize, f: OutputField, v: f32) {
        let m = &mut self.keys[n];
        match f {
            OutputField::Level => m.level_db = v,
            OutputField::Pan => m.pan = v,
            OutputField::Width => m.width = v,
            OutputField::Invert => m.invert = v >= 0.5,
            OutputField::Mute => m.mute = v >= 0.5,
            OutputField::Solo => self.solo[n] = v >= 0.5,
            OutputField::Delay => self.delay_ms[n] = v,
            // *On* is the snapshot's: an output turned off has no zones in it at all.
            OutputField::On => {}
        }
    }

    /// The mixes, the lines' delays and whether to render output by output, from the
    /// keys as stored.
    fn derive(&mut self) {
        self.mix = self.keys;
        if self.solo.iter().any(|s| *s) {
            for (m, s) in self.mix.iter_mut().zip(self.solo) {
                m.mute |= !s;
            }
        }
        let fs = self.fs;
        for (n, d) in &mut self.lines {
            d.set_ms(self.delay_ms[*n], fs);
        }
        // A line (its delay set, or a lane driving it) renders output by output from the
        // start: a delay automated up from 0 never switches the render's path.
        self.by_output = self.split
            || self.mix.iter().any(|m| !m.is_unity())
            || !self.lines.is_empty()
            || self.roomed;
    }

    /// Nothing rings: no line holds sound, the room is quiet.
    pub fn at_rest(&self) -> bool {
        self.lines.iter().all(|(_, d)| !d.ringing())
            && self.room.as_ref().is_none_or(|r| r.at_rest())
    }

    /// Forget every tail (a reset).
    pub fn clear(&mut self) {
        for (_, d) in &mut self.lines {
            d.clear();
        }
        if let Some(r) = self.room.as_mut() {
            r.clear();
        }
    }

    /// Render `ex` output by output into `left`/`right` (and `bus`, the outputs on strips
    /// of their own), each output mixed, delayed and summed into the room. Returns (it
    /// wrote, something reached the main output).
    pub fn run(
        &mut self,
        ex: &mut Exemplar,
        left: &mut [f32],
        right: &mut [f32],
        bus: Option<&mut BusOut<'_>>,
    ) -> (bool, bool) {
        let n = left.len().min(right.len());
        let Outs {
            mix,
            bus: route,
            peak,
            lines,
            fed,
            scratch,
            room,
            room_in,
            room_gain,
            ..
        } = self;
        *peak = [0.0; MAX_OUTPUTS];
        fed.iter_mut().for_each(|f| *f = 0);
        let rn = if room.is_some() {
            n.min(room_in.len())
        } else {
            0
        };
        room_in[..rn].fill(0.0);
        let mut main = false;
        let mut sink = Sink {
            route,
            left,
            right,
            bus,
            room_in: &mut room_in[..rn],
            main: &mut main,
        };
        let wrote = ex.process_outputs(n, &mut |k, off, a, b| {
            let k = usize::from(k);
            mix[k].apply(a, b);
            peak[k] = a
                .iter()
                .chain(b.iter())
                .fold(peak[k], |p, x| p.max(x.abs()));
            if let Some(li) = lines.iter().position(|(o, _)| *o == k) {
                let (_, d) = &mut lines[li];
                catch_up(d, &mut fed[li], off, k, scratch, &mut sink);
                d.run(a, b);
                fed[li] = off + a.len();
            }
            sink.put(k, off, a, b);
        });
        // Each line's tail to the block's end.
        for (li, (k, d)) in lines.iter_mut().enumerate() {
            catch_up(d, &mut fed[li], n, *k, scratch, &mut sink);
        }
        let mut sounding = wrote || lines.iter().any(|(_, d)| d.ringing());
        if let Some(r) = room.as_mut() {
            let g = *room_gain;
            if g > 0.0 && !(r.at_rest() && sink.room_in.iter().all(|x| *x == 0.0)) {
                for i in 0..rn {
                    let (tl, tr) = r.tick(sink.room_in[i]);
                    sink.left[i] += tl * g;
                    sink.right[i] += tr * g;
                }
                *sink.main = true;
                sounding = true;
            }
        }
        (sounding, main)
    }
}

/// Where a lane's audio goes: the main output or its strip's bus, and the room's sum.
struct Sink<'a, 'b, 'c> {
    route: &'a [u8; MAX_OUTPUTS],
    left: &'a mut [f32],
    right: &'a mut [f32],
    bus: Option<&'b mut BusOut<'c>>,
    room_in: &'a mut [f32],
    main: &'a mut bool,
}

impl Sink<'_, '_, '_> {
    fn put(&mut self, k: usize, off: usize, a: &[f32], b: &[f32]) {
        let m = a.len().min(b.len());
        if !self.room_in.is_empty() {
            let end = (off + m).min(self.room_in.len());
            for (i, x) in self.room_in[off.min(end)..end].iter_mut().enumerate() {
                *x += 0.5 * (a[i] + b[i]);
            }
        }
        match (self.route[k], self.bus.as_mut()) {
            (0, _) | (_, None) => {
                *self.main = true;
                add_into(&mut self.left[off..off + m], &a[..m]);
                add_into(&mut self.right[off..off + m], &b[..m]);
            }
            (to, Some(out)) => out(to, off, &a[..m], &b[..m]),
        }
    }
}

/// Feed line `d` (output `k`) silence from where it was fed to `to`, its tail delivered
/// while it still rings; once it gives nothing it moves on as if fed silence
/// ([`OutDelay::skip`]: the same line whatever the spans).
fn catch_up(
    d: &mut OutDelay,
    fed: &mut usize,
    to: usize,
    k: usize,
    scratch: &mut [f32],
    sink: &mut Sink<'_, '_, '_>,
) {
    while *fed < to {
        if !d.ringing() {
            d.skip(to - *fed);
            *fed = to;
            return;
        }
        let (sl, sr) = scratch.split_at_mut(CHUNK);
        let m = (to - *fed).min(CHUNK);
        sl[..m].fill(0.0);
        sr[..m].fill(0.0);
        d.run(&mut sl[..m], &mut sr[..m]);
        sink.put(k, *fed, &sl[..m], &sr[..m]);
        *fed += m;
    }
}
