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
use cantus_core::{NativeSpec, SampleData};
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
    /// Pairs: zone index, position in its file (frames).
    pub const FIRST_PAIR: usize = 16;
    pub const MAX_PAIRS: usize = 16;
    /// The keyswitch in force (−1: none).
    pub const KEYSWITCH: usize = 48;
}

#[derive(Debug)]
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
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        // The built-in sounds are computed on first use: here, off the audio thread.
        let _ = builtin::builtins();
        let mut defaults = [0.0f32; zf::COUNT];
        for (i, d) in defaults.iter_mut().enumerate() {
            *d = model::ZONE_PARAMS[i].default;
        }
        Unit {
            ex: Exemplar::new(sample_rate),
            fs: sample_rate,
            vals: vec![defaults; MAX_ZONES].into_boxed_slice(),
            touched: vec![false; MAX_ZONES].into_boxed_slice(),
            has_file: vec![false; MAX_ZONES].into_boxed_slice(),
            srcs: vec![Source::default(); MAX_ZONES].into_boxed_slice(),
            used: Vec::with_capacity(MAX_ZONES),
            prev_used: Vec::with_capacity(MAX_ZONES),
            groups: vec![GroupParams::default(); MAX_GROUPS].into_boxed_slice(),
            ptrs: Vec::with_capacity(MAX_ZONES * ex::MAX_LEVELS),
            memory_mib: 0.0,
            stolen: 0,
            peak: 0.0,
        }
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
    fn configure(&mut self, spec: &NativeSpec, assets: &Assets) {
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
            if let Some((n, "sample")) = zone_key(k) {
                self.mark(n);
                self.has_file[n] = true;
            }
        }
        for (key, data) in assets.iter() {
            let (Some((n, level)), Some(data)) = (asset_level(key), data) else {
                continue;
            };
            if !self.has_file[n] {
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
        for (m, g) in self.groups.iter().enumerate() {
            self.ex.set_group(m, g);
        }
        self.ex.set_globals(&globals);
        std::mem::swap(&mut self.used, &mut self.prev_used);
    }

    fn note_on(&mut self, note: NoteOn) {
        self.ex.note_on(note.key, note.velocity, note.id);
    }

    fn note_off(&mut self, key: u8, id: u64, velocity: f32) {
        self.ex.note_off(key, id, velocity);
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        let which = match controller {
            Controller::ModWheel => 0,
            Controller::Pressure => 1,
            Controller::PitchBend => 2,
            Controller::Expression => 3,
            Controller::Sustain => 4,
            Controller::Cc(_) => return,
        };
        self.ex.controller(which, value);
    }

    fn all_notes_off(&mut self) {
        self.ex.all_notes_off();
    }

    fn reset(&mut self) {
        self.ex.reset();
    }

    fn at_rest(&self) -> bool {
        self.ex.at_rest()
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
        let wrote = self.ex.process_par(left, right, par);
        self.stolen += self.ex.take_stolen();
        self.peak = self.ex.take_peak();
        wrote
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
    let shift = model::max_shift_semitones(spec, zone, 12.0);
    let r_max = rate as f32 / 44_100.0 * math::exp2(shift / 12.0);
    if r_max < 2.0 {
        0
    } else {
        (math::log2(r_max).floor() as usize).min(ex::MAX_LEVELS - 1)
    }
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
