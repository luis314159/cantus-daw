//! Exemplar — the multisample sampler: the engine's unit (ADR 0023,
//! `docs/instruments/exemplar.md`).
//!
//! `configure` reads the spec's flat keys (`z{n}.*`, `g{m}.*`, the instrument's) into the
//! DSP's fixed tables in one pass, without allocating, and hands each zone its decoded
//! sample and **octave mip-maps**. The mip-maps are built off the audio thread by
//! [`attach_mips`], which `SampleCache::attach` calls for a sampler track: they arrive as
//! extra assets `z{n}.sample~{k}` (level *k*), sized from how far above its root each zone
//! can play.

use super::{Assets, Controller, InstrumentUnit, NoteOn, Telemetry, TELEMETRY_LEN};
use cantus_core::inst::modulation::{self, MACRO_KEYS};
use cantus_core::inst::sampler::{
    self as model, group_field, group_key, zf, zone_field, zone_key, LoopMode, Trigger, MAX_GROUPS,
    MAX_ZONES,
};
use cantus_core::inst_out::output_key;
use cantus_core::{NativeSpec, SampleData};
use cantus_dsp::exemplar::stream::StartPin;
use cantus_dsp::exemplar::{
    self as ex, builtin, Exemplar, Globals, GroupParams, LoopDir, Quality, Region, Source, Trig,
    ZoneParams,
};
use cantus_dsp::math;
use cantus_dsp::modmatrix::{ModCurve, ModRoute};
use cantus_dsp::voices::PlayMode;
use std::collections::HashMap;
use std::sync::Arc;

#[path = "sampler_decode.rs"]
pub mod decode;
// r40:chorda-src — Exemplar's outputs: mix, buses, delay lines, the added room.
#[path = "sampler_outs.rs"]
pub mod outs;
use outs::Outs;

/// Telemetry slots of Exemplar's own (after the frame's 0–2).
pub mod tel {
    /// Layers sounding (a note can sound several zones).
    pub const LAYERS: usize = 8;
    /// Sample memory held by the zones, MiB, mip-maps included.
    pub const MEMORY_MIB: usize = 9;
    pub const ZONES: usize = 10;
    /// Loudest output sample of the last block.
    pub const PEAK: usize = 11;
    /// How many (zone, position) pairs follow.
    pub const PLAYING: usize = 12;
    /// Zones whose sample streams from disk (ADR 0050).
    pub const STREAMED: usize = 13;
    /// Voices ended early by a late block since the unit was made.
    pub const LATE: usize = 14;
    /// Pairs: zone index, position in its file (frames).
    pub const FIRST_PAIR: usize = 16;
    pub const MAX_PAIRS: usize = 16;
    /// The keyswitch in force (−1: none).
    pub const KEYSWITCH: usize = 48;
    // r40:chorda-src
    /// Each output's loudest sample of the last block (after its mix), for the instrument
    /// mixer's meters: outputs 0 … 13 at `OUT_PEAK + n`, *Releases* (16) at the last slot.
    pub const OUT_PEAK: usize = 49;

    /// The telemetry slot of output `id`'s peak, if it has one ([`OUT_PEAK`]).
    pub fn out_peak(id: u16) -> Option<usize> {
        match id {
            0..=13 => Some(OUT_PEAK + id as usize),
            16 => Some(super::TELEMETRY_LEN - 1),
            _ => None,
        }
    }
}

/// fix-e — whole configures Exemplar units have run in this process: what a lane on a
/// global, a group or an output must not cost (`configure_keys`). Diagnostics.
static CONFIGURES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many whole configures Exemplar units have run in this process (fix-e).
pub fn configures() -> u64 {
    CONFIGURES.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Clone, Debug)] // r34:listen: Clone for fork
pub struct Unit {
    ex: Exemplar,
    fs: f32,
    vals: Box<[[f32; zf::COUNT]]>,
    touched: Box<[bool]>,
    has_file: Box<[bool]>,
    srcs: Box<[Source]>,
    used: Vec<u16>,
    prev_used: Vec<u16>,
    groups: Box<[GroupParams]>,
    ptrs: Vec<(usize, usize)>,
    memory_mib: f32,
    stolen: u32,
    peak: f32,
    streamed: usize,
    late_total: u32,
    outs: Outs, // r40:chorda-src
    /// The globals as the last whole configure made them (fix-e: `configure_keys`).
    globals: Globals,
}

impl Unit {
    /// The modulation matrix from the spec's slots (the whole configure's and a lane's on a
    /// slot's amount, fix-e): allocation-free.
    fn matrix(&mut self, spec: &NativeSpec) {
        self.ex.clear_matrix();
        for c in modulation::connections(spec) {
            if !c.on || c.amount == 0.0 {
                continue;
            }
            let (Some(src), Some(dst)) = (ex::source_index(c.src), ex::target_index(c.dst)) else {
                continue;
            };
            let via = c.via.and_then(ex::source_index);
            self.ex.add_route(ModRoute {
                src,
                dst,
                depth: c.amount,
                via,
                curve: ModCurve::from_index(c.curve as usize as f32),
            });
        }
    }

    pub fn new(sample_rate: f32) -> Self {
        Self::with_zones(sample_rate, model::BASE_ZONES)
    }

    // r40:chorda-src
    /// A unit with room for `zones` zones (an instrument with more than every unit holds:
    /// a recorded piano of several images, ADR 0125). Off the audio thread.
    pub fn with_zones(sample_rate: f32, zones: usize) -> Self {
        Self::with(sample_rate, zones, 0, false)
    }

    /// A unit with room for `zones` zones, a delay line for each output in `delays` (a bit
    /// per output) and, if `room`, the added room — exactly what its snapshot asks for
    /// (`UnitKind::Exemplar`). Off the audio thread.
    pub fn with(sample_rate: f32, zones: usize, delays: u32, room: bool) -> Self {
        let cap = zones.clamp(1, MAX_ZONES);
        // The built-in sounds are computed on first use: here, off the audio thread.
        let _ = builtin::builtins();
        let mut defaults = [0.0f32; zf::COUNT];
        for (i, d) in defaults.iter_mut().enumerate() {
            *d = model::ZONE_PARAMS[i].default;
        }
        let ex = Exemplar::with_zones(sample_rate, cap);
        // Its layers say on these boards what they stream and where its zones may start
        // (ADR 0050).
        super::stream::hub().register_board(ex.board());
        super::stream::hub().register_board(ex.starts());
        Unit {
            ex,
            fs: sample_rate,
            vals: vec![defaults; cap].into_boxed_slice(),
            touched: vec![false; cap].into_boxed_slice(),
            has_file: vec![false; cap].into_boxed_slice(),
            srcs: vec![Source::default(); cap].into_boxed_slice(),
            used: Vec::with_capacity(cap),
            prev_used: Vec::with_capacity(cap),
            groups: vec![GroupParams::default(); MAX_GROUPS].into_boxed_slice(),
            ptrs: Vec::with_capacity(cap * ex::MAX_LEVELS),
            memory_mib: 0.0,
            stolen: 0,
            peak: 0.0,
            streamed: 0,
            late_total: 0,
            outs: Outs::new(sample_rate, delays, room), // r40:chorda-src
            globals: Globals::default(),
        }
    }

    // r40:chorda-src
    /// Every output rendered on its lane, mixed, and added into `left`/`right` or handed to
    /// `bus` — output by output, in ascending order (ADR 0123: the split sums to the
    /// unsplit when the strips add in that order).
    fn process_outs(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        bus: Option<&mut cantus_dsp::pulsus::BusOut<'_>>,
    ) -> bool {
        let (wrote, main) = self.outs.run(&mut self.ex, left, right, bus);
        self.stolen += self.ex.take_stolen();
        self.peak = self.ex.take_peak();
        wrote && main
    }

    /// The DSP, for tests.
    pub fn dsp(&self) -> &Exemplar {
        &self.ex
    }

    fn mark(&mut self, n: usize) {
        if !self.touched[n] {
            self.touched[n] = true;
            self.used.push(n as u16);
        }
    }
}

/// The instrument's numbers into [`Globals`].
fn global(g: &mut Globals, key: &str, v: f32) {
    match key {
        "gain" => g.gain_db = v,
        "pan" => g.pan = v / 100.0,
        "transpose" => g.transpose = v,
        "tune" => g.tune_ct = v,
        "vel.amp" => g.vel_amp = v / 100.0,
        "voices" => g.voices = v as usize,
        "mode" => g.mode = PlayMode::from_index(v),
        "glide" => g.glide_ms = v,
        "bend" => g.bend_range = v,
        "quality" => g.quality = Quality::from_index(v),
        "amp.attack" => g.amp[0] = v,
        "amp.hold" => g.amp[1] = v,
        "amp.decay" => g.amp[2] = v,
        "amp.sustain" => g.amp[3] = v / 100.0,
        "amp.release" => g.amp[4] = v,
        "env2.attack" => g.env2[0] = v,
        "env2.decay" => g.env2[1] = v,
        "env2.sustain" => g.env2[2] = v / 100.0,
        "env2.release" => g.env2[3] = v,
        "filter.type" => g.filter = v as u8,
        "filter.cutoff" => g.cutoff = v,
        "filter.reso" => g.reso = v,
        "filter.key" => g.filter_key = v / 100.0,
        "filter.vel" => g.filter_vel = v,
        "filter.env" => g.filter_env = v,
        "lfo1.rate" => g.lfo_rate[0] = v,
        "lfo1.shape" => g.lfo_shape[0] = v as u8,
        "lfo1.delay" => g.lfo1_delay_ms = v,
        "lfo1.pitch" => g.vibrato_ct = v,
        "lfo2.rate" => g.lfo_rate[1] = v,
        "lfo2.shape" => g.lfo_shape[1] = v as u8,
        "start.vel" => g.start_vel_ms = v,
        "sw.lo" => g.sw_lo = v as i16,
        "sw.hi" => g.sw_hi = v as i16,
        "sw.default" => g.sw_default = v as i16,
        "rr.order" => g.rr_in_order = v >= 0.5,
        _ => {
            if let Some(m) = MACRO_KEYS.iter().position(|k| *k == key) {
                g.macros[m] = v;
            }
        }
    }
}

fn group_value(g: &mut GroupParams, field: usize, v: f32) {
    use cantus_core::inst::sampler::gf;
    match field {
        gf::GAIN => g.gain_db = v,
        gf::PAN => g.pan = v / 100.0,
        gf::TUNE => g.tune_ct = v,
        gf::ATTACK => g.amp[0] = Some(v),
        gf::HOLD => g.amp[1] = Some(v),
        gf::DECAY => g.amp[2] = Some(v),
        gf::SUSTAIN => g.amp[3] = Some(v / 100.0),
        gf::RELEASE => g.amp[4] = Some(v),
        gf::VEL => g.vel_amp = Some(v / 100.0),
        gf::POLY => g.poly = v as u8,
        gf::MUTE => g.mute = v >= 0.5,
        gf::OUT => g.out = (v as u8).min(31), // r40:chorda-src
        _ => {}
    }
}

/// A zone's stored numbers as the DSP plays them, for a sample at `rate`.
pub fn zone_params(v: &[f32; zf::COUNT], rate: f32) -> ZoneParams {
    let mode = LoopMode::from_index(v[zf::MODE]);
    let trig = match Trigger::from_index(v[zf::TRIG]) {
        Trigger::Attack => Trig::Attack,
        Trigger::Release => Trig::Release,
        Trigger::First => Trig::First,
        Trigger::Legato => Trig::Legato,
        Trigger::ReleaseKey => Trig::ReleaseKey,
        Trigger::Controller => Trig::Controller,
    };
    let lo = v[zf::LO] as u8;
    let vlo = v[zf::VLO] as u8;
    ZoneParams {
        lo,
        hi: (v[zf::HI] as u8).max(lo),
        root: v[zf::ROOT] as u8,
        vlo,
        vhi: (v[zf::VHI] as u8).max(vlo),
        tune_ct: v[zf::TUNE],
        gain_db: v[zf::GAIN],
        pan: v[zf::PAN] / 100.0,
        keytrack: v[zf::KEYTRACK],
        group: (v[zf::GROUP] as usize).min(MAX_GROUPS - 1) as u8,
        region: Region {
            start: v[zf::START] as i64,
            end: v[zf::END] as i64,
            looping: mode.loops(),
            ls: v[zf::LS] as i64,
            le: v[zf::LE] as i64,
            dir: match v[zf::DIR] as i32 {
                1 => LoopDir::PingPong,
                2 => LoopDir::Backward,
                _ => LoopDir::Forward,
            },
            xf: (v[zf::XF] * rate / 1000.0).round() as i64,
            xf_power: v[zf::LXC] >= 0.5,
            reverse: v[zf::REV] >= 0.5,
        },
        one_shot: mode == LoopMode::OneShot,
        sustain_loop: mode == LoopMode::Sustain,
        vfin: v[zf::VFIN] as u8,
        vfout: v[zf::VFOUT] as u8,
        kfin: v[zf::KFIN] as u8,
        kfout: v[zf::KFOUT] as u8,
        fade_gain: v[zf::FC] >= 0.5,
        seq: v[zf::SEQ] as u8,
        seqlen: v[zf::SEQLEN] as u8,
        rlo: v[zf::RLO],
        rhi: v[zf::RHI],
        trig,
        rt_decay: v[zf::RTDECAY],
        rt_follow: v[zf::RTFOLLOW] >= 0.5,
        excl: v[zf::EXCL] as u8,
        offby: v[zf::OFFBY] as u8,
        off_normal: v[zf::OFFMODE] >= 0.5,
        sw: v[zf::SW] as i16,
        delay_ms: v[zf::DELAY],
        lpf_hz: v[zf::LPF], // r37:piano
        cc: (v[zf::CC] as u8).min(127),
        cclo: (v[zf::CCLO] as u8).min(127),
        cchi: (v[zf::CCHI] as u8).min(127),
    }
}

/// `z{n}.sample` → (n, 0); `z{n}.sample~{k}` → (n, k).
fn asset_level(key: &str) -> Option<(usize, usize)> {
    let (n, field) = zone_key(key)?;
    if field == "sample" {
        return Some((n, 0));
    }
    let k = field.strip_prefix("sample~")?.parse::<usize>().ok()?;
    (k > 0 && k < ex::MAX_LEVELS).then_some((n, k))
}

/// The zone an instrument with nothing loaded plays: the built-in sine over the whole
/// keyboard, so a fresh Exemplar is never a silent track — at −18 dB lifted by the output
/// trim (ADR 0039: +8 dB for a new instance, where the other instruments sit; 0 in a
/// project saved before, so it renders as it did).
fn init_zone(trim_db: f32) -> (ZoneParams, Source) {
    let b = builtin::get(1).expect("the built-in sine");
    let (ls, le) = b.loop_points.unwrap_or((0, 0));
    let z = ZoneParams {
        root: b.root,
        gain_db: -18.0 + trim_db,
        region: Region {
            looping: true,
            ls: ls as i64,
            le: le as i64,
            ..Region::default()
        },
        ..ZoneParams::default()
    };
    (z, b.source())
}

impl InstrumentUnit for Unit {
    /// r42:tuning — the track's tuning (ADR 0155), at the DSP's insertion point.
    fn set_tuning(&mut self, table: &cantus_core::tuning::TuningTable) {
        self.ex.set_tuning(&table.0);
    }

    fn fork(&self) -> Option<Box<dyn InstrumentUnit>> {
        // A zone streaming from disk reads through the reader's boards: not state a copy
        // can own (ADR 0050), so a range render pre-rolls instead.
        (self.streamed == 0).then(|| Box::new(self.clone()) as Box<dyn InstrumentUnit>)
    } // r34:listen
    fn configure(&mut self, spec: &NativeSpec, assets: &Assets) {
        CONFIGURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Forget last time's zones.
        let defaults = &model::ZONE_PARAMS;
        for &n in &self.prev_used {
            let n = n as usize;
            for (i, d) in defaults.iter().enumerate() {
                self.vals[n][i] = d.default;
            }
            self.touched[n] = false;
            self.has_file[n] = false;
            self.srcs[n] = Source::default();
        }
        self.used.clear();
        for g in self.groups.iter_mut() {
            *g = GroupParams::default();
        }
        let mut globals = Globals::default();
        for (k, v) in &spec.params {
            if let Some((n, field)) = zone_key(k) {
                if n >= self.vals.len() {
                    continue; // beyond this unit's room (its spec is sized: never)
                }
                if let Some(i) = zone_field(field) {
                    self.mark(n);
                    self.vals[n][i] = *v;
                }
            } else if let Some((m, field)) = group_key(k) {
                if let Some(i) = group_field(field) {
                    group_value(&mut self.groups[m], i, *v);
                }
            } else {
                global(&mut globals, k, *v);
            }
        }
        for k in spec.text.keys() {
            if let Some((n, "sample")) = zone_key(k).filter(|(n, _)| *n < self.vals.len()) {
                self.mark(n);
                self.has_file[n] = true;
            }
        }
        for (key, data) in assets.iter() {
            let (Some((n, level)), Some(data)) = (asset_level(key), data) else {
                continue;
            };
            if n >= self.vals.len() {
                continue;
            }
            if !self.has_file[n] {
                continue;
            }
            if level == 0 {
                if let Some(st) = super::stream::lookup(data) {
                    // A streamed sample carries every level its file has; the zone keeps
                    // the ones its own range needs, as in memory (ADR 0050, review 5).
                    // r42 fix-engine (review correctness 10): and the highest key the
                    // tuning raises, scaled by the zone's keytrack (`set_tuning` comes before
                    // `configure` for Exemplar; nothing is added at the default).
                    let mut shift = model::max_shift_of(spec, &self.vals[n], 12.0);
                    let up = self.ex.tune_up();
                    if up > 0.0 {
                        shift += up * self.vals[n][zf::KEYTRACK].max(0.0) / 100.0;
                    }
                    let mut src = Source::from_stream(&st);
                    src.limit_levels(1 + levels_for(shift, data.rate));
                    self.srcs[n] = src;
                    continue;
                }
            }
            if self.srcs[n].stream.is_some() {
                continue;
            }
            let s = &mut self.srcs[n];
            if level == 0 {
                s.channels = data.channels.clamp(1, 2) as usize;
                s.rate = data.rate as f32;
                s.frames = data.len_frames();
                s.tune_ct = 0.0;
            }
            s.levels[level] = Some(data.frames.clone());
        }
        self.used.sort_unstable();
        self.ex.clear_zones();
        self.ptrs.clear();
        let mut any = false;
        self.streamed = 0;
        // Taken out and put back (no allocation) so the loop may borrow `self`.
        let used = std::mem::take(&mut self.used);
        for &n in &used {
            let n = n as usize;
            let gen = self.vals[n][zf::GEN] as u8;
            let src = if self.has_file[n] {
                // A file that failed to load is silence, never a fallback sound.
                self.srcs[n].clone()
            } else if let Some(b) = builtin::get(gen) {
                b.source()
            } else {
                continue;
            };
            if src.stream.is_some() {
                self.streamed += 1;
            }
            let z = zone_params(&self.vals[n], src.rate);
            for l in src.levels.iter().flatten() {
                if self.ptrs.len() < self.ptrs.capacity() {
                    self.ptrs.push((l.as_ptr() as usize, l.len() * 4));
                }
            }
            self.ex.put_zone(n, &z, &src);
            any = true;
        }
        self.used = used;
        if !any && !spec.text.keys().any(|k| zone_key(k).is_some()) {
            let (z, src) = init_zone(spec.get(cantus_core::inst::TRIM_KEY));
            self.ex.put_zone(0, &z, &src);
        }
        self.ptrs.sort_unstable();
        self.ptrs.dedup_by_key(|p| p.0);
        self.memory_mib = self.ptrs.iter().map(|p| p.1).sum::<usize>() as f32 / 1_048_576.0;
        self.matrix(spec);
        for (m, g) in self.groups.iter().enumerate() {
            self.ex.set_group(m, g);
        }
        self.ex.set_globals(&globals);
        // What `configure_keys` updates a key at a time (fix-e).
        self.globals = globals;
        let differ = self.ex.has_outputs();
        self.outs.configure(spec, differ); // r40:chorda-src
        self.ex.publish_starts();
        // The reader looks at once (the attach had them read already — second review #16).
        if self.streamed > 0 {
            super::stream::hub().boards_changed();
        }
        std::mem::swap(&mut self.used, &mut self.prev_used);
    }

    /// fix-e — a lane on what the instrument plays *with* sets that alone, to the very
    /// state [`Self::configure`] reaches, never walking a zone (a recorded Chorda holds up
    /// to 8 192): an output's mix, delay or the added room's level (`o{n}.*`,
    /// `room.level`), a global (`gain`, `filter.cutoff`, the envelopes, the LFOs, a
    /// macro…), a group's (`g{m}.gain`, …) and the matrix's amounts. A zone's key, the
    /// trim, and — while a zone streams — `tune`, `transpose` and `bend` (they choose the
    /// levels a stream keeps) go the whole way.
    fn configure_keys(&mut self, spec: &NativeSpec, _assets: &Assets, keys: &[String]) -> bool {
        let (mut glob, mut mix, mut matrix) = (false, false, false);
        let mut groups = 0u64;
        for k in keys {
            let k = k.as_str();
            if zone_key(k).is_some()
                || k == cantus_core::inst::TRIM_KEY
                || k == cantus_core::inst::sampler::CAPACITY_KEY
                || k == outs::ROOM_KEY
                || (self.streamed > 0 && matches!(k, "tune" | "transpose" | "bend"))
            {
                return false;
            }
            if output_key(k).is_some() || k == outs::ROOM_LEVEL_KEY {
                mix = true;
            } else if let Some((m, field)) = group_key(k) {
                if group_field(field).is_some() && m < 64 {
                    groups |= 1 << m;
                }
            } else {
                // What the whole configure hands `global` (a matrix key too: a no-op there).
                glob = true;
                matrix |= modulation::SLOTS
                    .iter()
                    .any(|s| s.amt == k || s.curve == k || s.on == k);
            }
        }
        if groups != 0 {
            for k in keys {
                if let Some((m, field)) = group_key(k) {
                    if let Some(i) = group_field(field) {
                        group_value(&mut self.groups[m], i, spec.get(k));
                    }
                }
            }
            for m in (0..self.groups.len()).filter(|m| groups & (1 << m) != 0) {
                self.ex.set_group(m, &self.groups[m]);
            }
        }
        if glob {
            for k in keys.iter().filter(|k| {
                output_key(k).is_none() && group_key(k).is_none() && *k != outs::ROOM_LEVEL_KEY
            }) {
                global(&mut self.globals, k, spec.get(k));
            }
            if matrix {
                self.matrix(spec);
            }
            self.ex.set_globals(&self.globals);
        }
        if mix {
            self.outs.configure_keys(spec, keys);
        }
        true
    }

    fn note_on(&mut self, note: NoteOn) {
        self.ex.note_on(note.key, note.velocity, note.id);
    }

    fn note_on_seq(&mut self, note: NoteOn, ordinal: u32) {
        self.ex
            .note_on_seq(note.key, note.velocity, note.id, ordinal);
    }

    fn note_off(&mut self, key: u8, id: u64, velocity: f32) {
        self.ex.note_off(key, id, velocity);
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        // Zones a controller starts hear it by its MIDI number (round 26: pedal noises).
        let cc = match controller {
            Controller::ModWheel => Some(1),
            Controller::Expression => Some(11),
            Controller::Sustain => Some(64),
            Controller::Cc(n) => Some(n),
            Controller::Pressure | Controller::PitchBend => None,
        };
        let which = match controller {
            Controller::ModWheel => Some(0),
            Controller::Pressure => Some(1),
            Controller::PitchBend => Some(2),
            Controller::Expression => Some(3),
            Controller::Sustain => Some(4),
            Controller::Cc(_) => None,
        };
        if let Some(w) = which {
            self.ex.controller(w, value);
        }
        if let Some(n) = cc {
            self.ex.cc(n, (value.clamp(0.0, 1.0) * 127.0).round() as u8);
        }
    }

    fn all_notes_off(&mut self) {
        self.ex.all_notes_off();
    }

    fn reset(&mut self) {
        self.ex.reset();
        self.outs.clear(); // r40:chorda-src
    }

    fn at_rest(&self) -> bool {
        // r40:chorda-src — a delay line or the added room still ringing is not at rest.
        self.ex.at_rest() && self.outs.at_rest()
    }

    fn skip(&mut self, frames: usize) {
        self.ex.skip(frames);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.process_par(left, right, &cantus_dsp::par::Serial)
    }

    fn process_par(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        par: &dyn cantus_dsp::par::Parallel,
    ) -> bool {
        if self.outs.by_output {
            return self.process_outs(left, right, None); // r40:chorda-src
        }
        let wrote = self.ex.process_par(left, right, par);
        self.stolen += self.ex.take_stolen();
        self.peak = self.ex.take_peak();
        wrote
    }

    // r40:chorda-src — Exemplar's buses (ADR 0123, 0125).
    fn process_buses(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        out: &mut cantus_dsp::pulsus::BusOut<'_>,
    ) -> bool {
        if !self.outs.by_output {
            return self.process(left, right);
        }
        self.process_outs(left, right, Some(out))
    }

    fn set_offline(&mut self, offline: bool) {
        self.ex.set_blocking(offline);
    }

    fn take_late_reads(&mut self) -> u32 {
        let n = self.ex.take_late();
        self.late_total = self.late_total.saturating_add(n);
        n
    }

    fn take_cut_notes(&mut self) -> u32 {
        self.ex.take_cut()
    }

    fn take_stream_wakes(&mut self) -> u32 {
        self.ex.take_wakes()
    }

    fn splits_voices(&self) -> bool {
        true
    }

    fn active_voices(&self) -> usize {
        self.ex.active_layers()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        t[0] = self.ex.active_notes() as f32;
        t[1] = self.ex.polyphony() as f32;
        t[2] = self.stolen as f32;
        t[tel::LAYERS] = self.ex.active_layers() as f32;
        t[tel::MEMORY_MIB] = self.memory_mib;
        t[tel::ZONES] = self.ex.zone_count() as f32;
        t[tel::PEAK] = self.peak;
        let mut info = [ex::LayerInfo::default(); tel::MAX_PAIRS];
        let n = self.ex.layer_info(&mut info);
        t[tel::PLAYING] = n as f32;
        for (i, l) in info[..n].iter().enumerate() {
            t[tel::FIRST_PAIR + 2 * i] = l.zone as f32;
            t[tel::FIRST_PAIR + 2 * i + 1] = l.position as f32;
        }
        t[tel::KEYSWITCH] = self.ex.keyswitch() as f32;
        t[tel::STREAMED] = self.streamed as f32;
        t[tel::LATE] = self.late_total as f32;
        // r40:chorda-src — each output's peak (only when rendering by output).
        if self.outs.by_output {
            for (n, v) in self.outs.peak.iter().enumerate() {
                if let Some(i) = tel::out_peak(n as u16) {
                    t[i] = *v;
                }
            }
        }
        t
    }
}

// ---- the mip-maps, built off the audio thread ----------------------------------------------

/// Mip-maps already built, by the address of the decoded file they come from (the sample
/// cache never frees one, so the address names it for the session) and whether they were
/// built as a periodic single cycle. Like the sample cache, it never evicts, so the audio
/// thread never drops the last reference to a buffer.
#[derive(Debug, Default)]
pub struct MipCache {
    built: HashMap<(usize, bool), Vec<Arc<SampleData>>>,
}

impl MipCache {
    /// Bytes held.
    pub fn bytes(&self) -> usize {
        self.built
            .values()
            .flatten()
            .map(|d| d.frames.len() * 4)
            .sum()
    }

    fn levels(
        &mut self,
        base: &Arc<SampleData>,
        periodic: bool,
        want: usize,
    ) -> &[Arc<SampleData>] {
        let key = (Arc::as_ptr(base) as usize, periodic);
        let have = self.built.entry(key).or_default();
        let ch = base.channels.max(1) as usize;
        while have.len() < want {
            let prev = have.last().map(|d| &d.frames).unwrap_or(&base.frames);
            if prev.len() / ch < 32 {
                break;
            }
            let next = if periodic {
                ex::resample::halve_periodic(prev, ch)
            } else {
                ex::halve(prev, ch)
            };
            let k = have.len() + 1;
            have.push(Arc::new(SampleData {
                stream: None,
                rate: (base.rate >> k).max(1),
                channels: base.channels,
                frames: next.into(),
            }));
        }
        let n = want.min(have.len());
        &have[..n]
    }
}

/// How many mip levels a zone needs: enough for its highest key plus the bend range and an
/// octave of modulation, at an output rate of 44.1 kHz or more.
pub fn levels_needed(spec: &NativeSpec, zone: &model::Zone, rate: u32) -> usize {
    levels_for(model::max_shift_semitones(spec, zone, 12.0), rate)
}

/// Mip levels a sample at `rate` needs to be played `shift` semitones above its root.
pub fn levels_for(shift: f32, rate: u32) -> usize {
    let r_max = rate as f32 / 44_100.0 * math::exp2(shift / 12.0);
    if r_max < 2.0 {
        0
    } else {
        (math::log2(r_max).floor() as usize).min(ex::MAX_LEVELS - 1)
    }
}

/// The start pins an Exemplar of `spec` playing `assets` publishes as it is configured — a
/// superset: every level its zones keep — computed off the audio thread, so the blocks are
/// read before the snapshot reaches it (second review #16: a note in the very block a
/// zone's start changes in plays as in memory).
pub fn start_pins(spec: &NativeSpec, assets: &Assets) -> Vec<StartPin> {
    let vel_ms = spec.get("start.vel").max(0.0);
    let draft = Quality::from_index(spec.get("quality")) == Quality::Draft;
    let mut out = Vec::new();
    for (key, data) in assets {
        let (Some((n, 0)), Some(d)) = (asset_level(key), data) else {
            continue;
        };
        let Some(st) = super::stream::lookup(d) else {
            continue;
        };
        let mut v = [0.0f32; zf::COUNT];
        for (i, x) in v.iter_mut().enumerate() {
            *x = spec.get(&format!("z{n}.{}", model::zone_field_name(i)));
        }
        let start = v[zf::START].max(0.0) as usize;
        let end = start + (vel_ms * st.rate / 1000.0).ceil() as usize;
        let head = st.head_frames(0);
        let block = ex::stream::BLOCK;
        let shift = model::max_shift_of(spec, &v, 12.0);
        // A zone played fast (more than 8 frames of its file a frame) runs through its head
        // in a few milliseconds: the blocks right after it are kept ready too, so a note's
        // first blocks are there before the reader has seen it (third review #8).
        let fast = d.rate as f32 / 44_100.0 * math::exp2(shift / 12.0) > 8.0;
        if end + 2 * block <= head && !fast {
            continue;
        }
        let lo = start.max(head) / block;
        let hi = ((end + 2 * block).min(st.frames[0].max(1)) - 1) / block;
        // A fast zone's first audio callback runs through its head and on: the blocks one
        // callback reads at its top speed are kept ready after the head (third review #8).
        let ratio_max = d.rate as f64 / 44_100.0 * math::exp2(shift / 12.0) as f64;
        let local = if draft { ratio_max } else { ratio_max.min(2.0) };
        let ahead = if fast {
            ex::stream::Cursor::ahead_for(local.min(ex::head_only_above(44_100.0)))
        } else {
            0
        };
        let hi = hi
            .max(lo + ex::stream::READ_AHEAD - 1 + ahead)
            .min(st.blocks(0).saturating_sub(1));
        out.push(StartPin {
            stream: st.id,
            block: lo,
            count: (hi + 1).saturating_sub(lo).clamp(1, 255),
            // Draft reads level 0 only, whatever the pitch.
            levels: if draft {
                1
            } else {
                (1 + levels_for(shift, d.rate)).min(st.levels())
            },
        });
    }
    out
}

/// For each file an instrument's zones play, whether it may be streamed (ADR 0050 §1):
/// not when some zone loops it or plays it backwards (it stays in memory). Nothing else
/// about a zone changes its stream (review 13).
pub fn stream_files(spec: &NativeSpec) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    for (_, z) in model::zones(spec) {
        let Some(path) = z.sample.clone() else {
            continue;
        };
        let v = &z.values;
        let plain = !LoopMode::from_index(v[zf::MODE]).loops() && v[zf::REV] < 0.5;
        match out.iter_mut().find(|(p, _)| *p == path) {
            Some((_, ok)) => *ok &= plain,
            None => out.push((path, plain)),
        }
    }
    out
}

/// For a sampler track: add every zone's mip levels to its assets, as `z{n}.sample~{k}`.
/// Called by `SampleCache::attach` on the thread that compiles snapshots.
pub fn attach_mips(
    spec: &NativeSpec,
    assets: &mut Vec<(String, Option<Arc<SampleData>>)>,
    cache: &mut MipCache,
) {
    let mut extra = Vec::new();
    for (key, data) in assets.iter() {
        let (Some((n, 0)), Some(data)) = (asset_level(key), data) else {
            continue;
        };
        if data.stream.is_some() {
            continue; // a stream brings its own levels
        }
        let Some(zone) = model::Zone::of(spec, n) else {
            continue;
        };
        let want = levels_needed(spec, &zone, data.rate);
        if want == 0 {
            continue;
        }
        let v = &zone.values;
        let frames = data.len_frames() as f32;
        let periodic = LoopMode::from_index(v[zf::MODE]).loops()
            && v[zf::LS] == 0.0
            && (v[zf::LE] == 0.0 || v[zf::LE] >= frames - 1.0)
            && v[zf::START] == 0.0
            && v[zf::END] == 0.0;
        for (k, level) in cache.levels(data, periodic, want).iter().enumerate() {
            extra.push((format!("{key}~{}", k + 1), Some(level.clone())));
        }
    }
    assets.extend(extra);
}

impl Unit {
    /// The sample rate the unit was built for.
    pub fn sample_rate(&self) -> f32 {
        self.fs
    }
}

/// The stream reader lets go of a dropped instrument's boards (second review #1).
impl Drop for Unit {
    fn drop(&mut self) {
        super::stream::hub().boards_changed();
    }
}
