//! SoundFont 2 (and SF3) import (`research/instruments/sampler.md` §5.4): the RIFF hydra
//! read into presets → instruments → zones → samples; one preset becomes an Exemplar
//! instrument, its samples written out as WAV files (SF3's Ogg Vorbis samples as `.ogg`)
//! under the app's data folder.
//!
//! Instrument-level generators are absolute, preset-level ones add to them; key and
//! velocity ranges intersect; instrument-only generators are ignored at the preset level.
//! Units: timecents, centibels, absolute cents, 0.1 % pan — converted to ms, dB, Hz, %.

use super::import::Import;
use cantus_core::inst::sampler::{gf, zf, Group, Zone};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct SampleHeader {
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub loop_start: u32,
    pub loop_end: u32,
    pub rate: u32,
    pub pitch: u8,
    pub correction: i8,
    pub kind: u16,
}

impl SampleHeader {
    /// SF3: the sample is an Ogg Vorbis stream (start and end are byte offsets).
    pub fn compressed(&self) -> bool {
        self.kind & 0x10 != 0
    }
}

/// A zone's generators: `Some(amount)` where set.
pub type Gens = [Option<i16>; 64];

#[derive(Clone, Debug)]
pub struct ZoneGens {
    pub gens: Gens,
}

#[derive(Clone, Debug)]
pub struct Preset {
    pub name: String,
    pub bank: u16,
    pub program: u16,
    pub zones: Vec<ZoneGens>,
}

#[derive(Clone, Debug)]
pub struct Instrument {
    pub name: String,
    pub zones: Vec<ZoneGens>,
}

/// A parsed SoundFont.
#[derive(Clone, Debug, Default)]
pub struct SoundFont {
    pub name: String,
    pub presets: Vec<Preset>,
    pub instruments: Vec<Instrument>,
    pub samples: Vec<SampleHeader>,
    /// The `smpl` chunk's bytes (16-bit little-endian PCM, or Ogg streams for SF3).
    pub data: Vec<u8>,
}

const INSTRUMENT: usize = 41;
const KEY_RANGE: usize = 43;
const VEL_RANGE: usize = 44;
const SAMPLE_ID: usize = 53;
/// Generators that are only meaningful at the instrument level.
const INSTRUMENT_ONLY: [usize; 12] = [0, 1, 2, 3, 4, 12, 45, 46, 47, 50, 54, 57];

fn name20(b: &[u8]) -> String {
    let end = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim().to_string()
}

fn u16le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// The chunks of a LIST body: (id, body).
fn chunks(b: &[u8]) -> Vec<(&[u8], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= b.len() {
        let size = u32le(b, at + 4) as usize;
        let end = (at + 8 + size).min(b.len());
        out.push((&b[at..at + 4], &b[at + 8..end]));
        at += 8 + size + (size & 1);
    }
    out
}

/// Zones from bag and generator records, for records `first..last` of the bags.
fn zones_of(bags: &[u8], gens: &[u8], first: usize, last: usize) -> Vec<ZoneGens> {
    let mut out = Vec::new();
    for b in first..last {
        if (b + 1) * 4 + 2 > bags.len() {
            break;
        }
        let (g0, g1) = (
            u16le(bags, b * 4) as usize,
            u16le(bags, (b + 1) * 4) as usize,
        );
        let mut z = ZoneGens { gens: [None; 64] };
        for g in g0..g1 {
            if (g + 1) * 4 > gens.len() {
                break;
            }
            let oper = u16le(gens, g * 4) as usize;
            let amount = u16le(gens, g * 4 + 2) as i16;
            if oper < 64 {
                z.gens[oper] = Some(amount);
            }
        }
        out.push(z);
    }
    out
}

/// Read a SoundFont's structure (the sample data is kept as bytes).
pub fn parse(b: &[u8]) -> Result<SoundFont, String> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"sfbk" {
        return Err("not a SoundFont (RIFF sfbk)".into());
    }
    let mut sf = SoundFont::default();
    let mut hydra: Vec<(&[u8], &[u8])> = Vec::new();
    for (id, body) in chunks(&b[12..]) {
        if id != b"LIST" || body.len() < 4 {
            continue;
        }
        let kind = &body[..4];
        for (cid, cbody) in chunks(&body[4..]) {
            match (kind, cid) {
                (b"INFO", b"INAM") => sf.name = name20(cbody),
                (b"sdta", b"smpl") => sf.data = cbody.to_vec(),
                (b"pdta", _) => hydra.push((cid, cbody)),
                _ => {}
            }
        }
    }
    let get = |id: &[u8]| {
        hydra
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, b)| *b)
            .unwrap_or(&[])
    };
    let (phdr, pbag, pgen) = (get(b"phdr"), get(b"pbag"), get(b"pgen"));
    let (inst, ibag, igen, shdr) = (get(b"inst"), get(b"ibag"), get(b"igen"), get(b"shdr"));
    if phdr.len() < 76 || inst.len() < 44 || shdr.len() < 92 {
        return Err("the SoundFont has no presets, instruments or samples".into());
    }
    let np = phdr.len() / 38;
    for p in 0..np - 1 {
        let r = &phdr[p * 38..p * 38 + 38];
        let next = &phdr[(p + 1) * 38..(p + 1) * 38 + 38];
        sf.presets.push(Preset {
            name: name20(&r[..20]),
            program: u16le(r, 20),
            bank: u16le(r, 22),
            zones: zones_of(pbag, pgen, u16le(r, 24) as usize, u16le(next, 24) as usize),
        });
    }
    let ni = inst.len() / 22;
    for i in 0..ni - 1 {
        let r = &inst[i * 22..i * 22 + 22];
        let next = &inst[(i + 1) * 22..(i + 1) * 22 + 22];
        sf.instruments.push(Instrument {
            name: name20(&r[..20]),
            zones: zones_of(ibag, igen, u16le(r, 20) as usize, u16le(next, 20) as usize),
        });
    }
    let ns = shdr.len() / 46;
    for s in 0..ns - 1 {
        let r = &shdr[s * 46..s * 46 + 46];
        sf.samples.push(SampleHeader {
            name: name20(&r[..20]),
            start: u32le(r, 20),
            end: u32le(r, 24),
            loop_start: u32le(r, 28),
            loop_end: u32le(r, 32),
            rate: u32le(r, 36),
            pitch: r[40],
            correction: r[41] as i8,
            kind: u16le(r, 44),
        });
    }
    sf.presets.sort_by_key(|p| (p.bank, p.program));
    Ok(sf)
}

fn tc_ms(tc: i16) -> f32 {
    (1000.0 * (tc as f32 / 1200.0).exp2()).clamp(0.1, 30_000.0)
}

fn abs_cents_hz(c: i16) -> f32 {
    8.176 * (c as f32 / 1200.0).exp2()
}

/// A file name safe on every system.
fn clean(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "sample".into()
    } else {
        s
    }
}

/// Write sample `i` into `dir` (once) and return its path.
fn extract(sf: &SoundFont, i: usize, dir: &Path) -> Result<PathBuf, String> {
    let s = &sf.samples[i];
    if s.compressed() {
        let path = dir.join(format!("{i:04}_{}.ogg", clean(&s.name)));
        if !path.exists() {
            let bytes = sf
                .data
                .get(s.start as usize..s.end as usize)
                .ok_or("an SF3 sample lies outside the data")?;
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        }
        return Ok(path);
    }
    let path = dir.join(format!("{i:04}_{}.wav", clean(&s.name)));
    if !path.exists() {
        let (a, b) = (s.start as usize * 2, s.end as usize * 2);
        let bytes = sf.data.get(a..b).ok_or("a sample lies outside the data")?;
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: s.rate.max(1),
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).map_err(|e| e.to_string())?;
        for c in bytes.chunks_exact(2) {
            w.write_sample(i16::from_le_bytes([c[0], c[1]]))
                .map_err(|e| e.to_string())?;
        }
        w.finalize().map_err(|e| e.to_string())?;
    }
    Ok(path)
}

/// Preset `preset` (an index into `sf.presets`) as an Exemplar instrument, its samples
/// written into `dir`.
pub fn to_import(sf: &SoundFont, preset: usize, dir: &Path) -> Result<Import, String> {
    let p = sf.presets.get(preset).ok_or("no such preset")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut out = Import {
        name: Some(p.name.clone()),
        ..Import::default()
    };
    let merge = |base: Option<Gens>, over: &Gens| {
        let mut g = base.unwrap_or([None; 64]);
        for (i, v) in over.iter().enumerate() {
            if v.is_some() {
                g[i] = *v;
            }
        }
        g
    };
    let pglobal = p
        .zones
        .first()
        .filter(|z| z.gens[INSTRUMENT].is_none())
        .map(|z| z.gens);
    let mut group_of: Vec<(Vec<i32>, usize)> = Vec::new();
    let mut filter_done = false;
    let mut rom = 0;
    for pz in p.zones.iter().filter(|z| z.gens[INSTRUMENT].is_some()) {
        let pg = merge(pglobal, &pz.gens);
        let Some(inst) = sf
            .instruments
            .get(pg[INSTRUMENT].unwrap_or(0) as u16 as usize)
        else {
            continue;
        };
        let iglobal = inst
            .zones
            .first()
            .filter(|z| z.gens[SAMPLE_ID].is_none())
            .map(|z| z.gens);
        for iz in inst.zones.iter().filter(|z| z.gens[SAMPLE_ID].is_some()) {
            let ig = merge(iglobal, &iz.gens);
            let si = ig[SAMPLE_ID].unwrap_or(0) as u16 as usize;
            let Some(s) = sf.samples.get(si) else {
                continue;
            };
            if s.kind & 0x8000 != 0 {
                rom += 1;
                continue;
            }
            let (pk, ik) = (range(pg[KEY_RANGE]), range(ig[KEY_RANGE]));
            let (pv, iv) = (range(pg[VEL_RANGE]), range(ig[VEL_RANGE]));
            let (lo, hi) = (pk.0.max(ik.0), pk.1.min(ik.1));
            let (vlo, vhi) = (pv.0.max(iv.0).max(1), pv.1.min(iv.1));
            if lo > hi || vlo > vhi {
                continue;
            }
            let val = |op: usize, default: i32| -> i32 {
                let i = ig[op].map(|v| v as i32).unwrap_or(default);
                let add = if INSTRUMENT_ONLY.contains(&op) || op == 58 {
                    0
                } else {
                    pg[op].map(|v| v as i32).unwrap_or(0)
                };
                i + add
            };
            let path = extract(sf, si, dir)?;
            let root = match ig[58] {
                Some(k) if (0..=127).contains(&k) => k as u8,
                _ if s.pitch <= 127 => s.pitch,
                _ => 60,
            };
            let mut z = Zone::new(
                Some(path.to_string_lossy().replace('\\', "/")),
                lo,
                hi,
                root,
            );
            z.set(zf::VLO, vlo as f32).set(zf::VHI, vhi as f32);
            z.set(
                zf::TUNE,
                (100 * val(51, 0) + val(52, 0) + s.correction as i32) as f32,
            );
            z.set(zf::KEYTRACK, val(56, 100) as f32);
            // FluidSynth scales attenuation by 0.4 (hardware-era banks were voiced so).
            z.set(zf::GAIN, -0.04 * val(48, 0) as f32);
            z.set(zf::PAN, val(17, 0) as f32 / 5.0);
            let start = val(0, 0) + 32_768 * val(4, 0);
            let end_off = val(1, 0) + 32_768 * val(12, 0);
            let ls_off = val(2, 0) + 32_768 * val(45, 0);
            let le_off = val(3, 0) + 32_768 * val(50, 0);
            let base = if s.compressed() { 0 } else { s.start as i64 };
            z.set(zf::START, start.max(0) as f32);
            if end_off != 0 && !s.compressed() {
                z.set(
                    zf::END,
                    (s.end as i64 - base + end_off as i64).max(1) as f32,
                );
            }
            let mode = match ig[54].unwrap_or(0) {
                1 => 2.0,
                3 => 3.0,
                _ => 0.0,
            };
            if mode > 0.0 {
                let ls = s.loop_start as i64 - base + ls_off as i64;
                let le = s.loop_end as i64 - base + le_off as i64 - 1;
                if le > ls && ls >= 0 {
                    z.set(zf::MODE, mode)
                        .set(zf::LS, ls as f32)
                        .set(zf::LE, le as f32);
                }
            }
            if let Some(c) = ig[57].filter(|c| *c > 0) {
                z.set(zf::EXCL, (c as f32).min(64.0))
                    .set(zf::OFFBY, (c as f32).min(64.0));
            }
            let delay = val(33, -12_000);
            if delay > -12_000 {
                z.set(zf::DELAY, tc_ms(delay as i16));
            }
            let env = vec![
                val(34, -12_000),
                val(35, -12_000),
                val(36, -12_000),
                val(37, 0),
                val(38, -12_000),
            ];
            let gi = match group_of.iter().find(|(e, _)| *e == env) {
                Some((_, i)) => *i,
                None => {
                    let i = group_of.len().min(63);
                    let mut g = Group::default();
                    g.values[gf::ATTACK] = Some(tc_ms(env[0].clamp(-32_768, 32_767) as i16));
                    g.values[gf::HOLD] = Some(tc_ms(env[1].clamp(-32_768, 32_767) as i16));
                    g.values[gf::DECAY] =
                        Some(tc_ms(env[2].clamp(-32_768, 32_767) as i16).max(1.0));
                    g.values[gf::SUSTAIN] = Some(
                        (10f32.powf(-(env[3].max(0) as f32) / 200.0) * 100.0).clamp(0.0, 100.0),
                    );
                    g.values[gf::RELEASE] =
                        Some(tc_ms(env[4].clamp(-32_768, 32_767) as i16).max(1.0));
                    group_of.push((env, i));
                    out.groups.push((i, g));
                    i
                }
            };
            z.set(zf::GROUP, gi as f32);
            if !filter_done {
                filter_done = true;
                let fc = val(8, 13_500);
                if fc < 13_500 {
                    let q = (10f32.powf(val(9, 0) as f32 / 200.0) * 0.707).max(0.5);
                    out.values.push(("filter.type".into(), 1.0));
                    out.values.push((
                        "filter.cutoff".into(),
                        abs_cents_hz(fc as i16).clamp(20.0, 20_000.0),
                    ));
                    out.values.push((
                        "filter.reso".into(),
                        ((2.0 - 1.0 / q) / 1.96 * 100.0).clamp(0.0, 100.0),
                    ));
                }
                let vib = val(6, 0);
                if vib != 0 {
                    out.values
                        .push(("lfo1.pitch".into(), (vib.unsigned_abs() as f32).min(100.0)));
                    out.values.push((
                        "lfo1.rate".into(),
                        abs_cents_hz(val(24, 0) as i16).clamp(0.01, 40.0),
                    ));
                }
            }
            out.zones.push(z);
        }
    }
    if rom > 0 {
        out.log
            .push(format!("{rom} zones use ROM samples and were skipped"));
    }
    out.log
        .push("attenuation scaled by 0.4, as FluidSynth does".into());
    out.log.push(
        "modulators: velocity follows Exemplar's curve; the rest are not carried over".into(),
    );
    out.clamp();
    Ok(out)
}

fn range(g: Option<i16>) -> (u8, u8) {
    match g {
        Some(v) => {
            let [lo, hi] = (v as u16).to_le_bytes();
            (lo.min(127), hi.min(127))
        }
        None => (0, 127),
    }
}
