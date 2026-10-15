//! A pack's build step (round 28, ADR 0042's amendment): a DrumGizmo kit — one file per
//! hit, every microphone a channel — mixed down at install to the stereo instrument
//! Exemplar plays.
//!
//! **The mix.** Each microphone the manifest names is heard at its gain, panned with the
//! equal-power law (−3 dB each side at the centre), its polarity reversed if asked; the
//! sum, times the mix's gain, is written as a 24-bit stereo WAV at the kit's own rate — so
//! one voice plays a hit, not thirteen. Microphones the manifest leaves out are not heard.
//! The sums run in the manifest's order with `cantus_dsp::math`, so every machine writes
//! the same bits.
//!
//! **The instrument.** Each built instrument's hits are measured (the RMS of their first
//! 50 ms, in dB) and sorted. Velocity follows SFZ's default curve (amplitude ∝
//! velocity²): a hit `L` dB under the instrument's loudest one is what velocity
//! `127·10^(L/40)` plays. Hits are grouped, softest first, into layers of `round_robin`
//! (each layer's hits take their turns in order — Exemplar's round robin by the note's
//! ordinal, so a roll never repeats a sample), each layer placed on the velocities around
//! its level, and each hit given the gain that makes every velocity play at that curve
//! whichever hit sounds: the loudness is continuous, the timbre changes from layer to
//! layer as a real drum's does. Chokes (a hi-hat's closed, pedal and open sounds) become
//! exclusive classes that fade over their release.

use crate::drumgizmo::{self, Kit, Sample};
use crate::manifest::Build;
use cantus_dsp::math;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// What a build made.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Built {
    /// Stereo files written.
    pub files: u64,
    /// Regions in the instrument.
    pub regions: usize,
    /// Samples past full scale, clamped (the manifest's mix gain should leave none).
    pub clamped: u64,
    /// The loudest sample of the whole mix, linear.
    pub peak: f32,
    /// Hits left out, softer than the range allows.
    pub dropped: usize,
    /// Notes for the log: what was left out and why.
    pub log: Vec<String>,
}

pub fn db(x: f32) -> f32 {
    math::powf(10f32, x / 20.0)
}

/// Equal-power pan: (left, right) gains for −100 … 100.
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let t = (pan.clamp(-100.0, 100.0) / 100.0 + 1.0) * std::f32::consts::FRAC_PI_4;
    (math::cos(t), math::sin(t))
}

/// The level a hit is measured at: the RMS of its first 50 ms over both channels, dB.
pub fn level_db(stereo: &[f32], rate: u32) -> f32 {
    let n = ((rate as usize) / 20).min(stereo.len() / 2).max(1);
    let mut sum = 0f64;
    for f in stereo.chunks_exact(2).take(n) {
        sum += (f[0] as f64 * f[0] as f64 + f[1] as f64 * f[1] as f64) / 2.0;
    }
    (10.0 * math::log10(sum / n as f64 + 1e-30)) as f32
}

/// A microphone's place in the mix: its kit channel and its two gains.
struct Gains {
    channel: String,
    l: f32,
    r: f32,
}

/// Each microphone's gains on `instrument` (raised or lowered by `inst_db`): at its own
/// level on the drums it faces, at its bleed level on the others.
fn gains(b: &Build, instrument: &str, inst_db: f32) -> Vec<Gains> {
    b.mics
        .iter()
        .map(|m| {
            let faces =
                m.own.is_empty() || m.own.iter().any(|o| instrument.starts_with(o.as_str()));
            let bleed = if faces { 0.0 } else { m.bleed_db };
            let g = db(b.gain_db + inst_db + m.gain_db + bleed) * if m.invert { -1.0 } else { 1.0 };
            let (pl, pr) = pan_gains(m.pan);
            Gains {
                channel: m.channel.clone(),
                l: g * pl,
                r: g * pr,
            }
        })
        .collect()
}

/// One file read at a time: a sample's microphones usually share one.
struct Reader {
    path: String,
    audio: Option<crate::wav::Audio>,
}

impl Reader {
    fn get(&mut self, root: &Path, rel: &str) -> Result<&crate::wav::Audio, String> {
        if self.path != rel || self.audio.is_none() {
            self.audio = Some(crate::wav::read(&drumgizmo::path_of(root, rel))?);
            self.path = rel.to_string();
        }
        Ok(self.audio.as_ref().expect("just read"))
    }
}

/// Mix one hit down to interleaved stereo, and its rate.
fn mix(
    root: &Path,
    s: &Sample,
    mics: &[Gains],
    reader: &mut Reader,
) -> Result<(Vec<f32>, u32), String> {
    let mut out: Vec<f32> = Vec::new();
    let mut rate = 0;
    for m in mics {
        let Some(a) = s.audio.iter().find(|a| a.channel == m.channel) else {
            continue;
        };
        let audio = reader.get(root, &a.file)?;
        if a.file_channel >= audio.channels {
            return Err(format!(
                "{}: channel {} of a {}-channel file",
                a.file,
                a.file_channel + 1,
                audio.channels
            ));
        }
        if rate != 0 && audio.rate != rate {
            return Err(format!("{}: its microphones disagree on the rate", s.name));
        }
        rate = audio.rate;
        let frames = audio.frames();
        if out.len() < frames * 2 {
            out.resize(frames * 2, 0.0);
        }
        let ch = audio.channels;
        for (i, o) in out.chunks_exact_mut(2).take(frames).enumerate() {
            let x = audio.samples[i * ch + a.file_channel];
            o[0] += x * m.l;
            o[1] += x * m.r;
        }
    }
    if rate == 0 {
        return Err(format!("{}: none of its microphones is in the mix", s.name));
    }
    Ok((out, rate))
}

/// One hit of an instrument once mixed: its file (relative to the build's folder) and its
/// level.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub file: String,
    pub level: f32,
}

/// One region of the instrument: a hit on a velocity range, its turn and its gain.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub file: String,
    pub lovel: u8,
    pub hivel: u8,
    /// Turn (1…) of `seqlen`.
    pub seq: u8,
    pub seqlen: u8,
    /// dB.
    pub volume: f32,
}

/// The highest gain a region may carry (Exemplar's zone gain stops at +24 dB).
pub const MAX_VOLUME: f32 = 24.0;

/// The velocity (fractional, 0…127] whose curve level is `l` dB from the loudest hit.
fn velocity_of(l: f32) -> f32 {
    127.0 * math::powf(10f32, l / 40.0)
}

/// An instrument's hits as velocity layers with round robins (see the module's notes).
/// Returns the regions, softest first, and how many hits were left out of the range.
pub fn layers(hits: &[Hit], round_robin: u8, range_db: f32) -> (Vec<Region>, usize) {
    let Some(top) = hits.iter().map(|h| h.level).reduce(f32::max) else {
        return (Vec::new(), 0);
    };
    let mut kept: Vec<&Hit> = hits.iter().filter(|h| h.level >= top - range_db).collect();
    let dropped = hits.len() - kept.len();
    kept.sort_by(|a, b| a.level.total_cmp(&b.level).then(a.file.cmp(&b.file)));
    let n = kept.len();
    let r = (round_robin.max(1) as usize).min(n);
    // As many layers as full turns fit; the hits left over go to the loudest layers.
    let mut sizes = vec![n / (n / r); n / r];
    for s in sizes.iter_mut().rev().take(n % (n / r)) {
        *s += 1;
    }
    let mut groups: Vec<Vec<&Hit>> = Vec::new();
    let mut at = 0;
    for s in sizes {
        groups.push(kept[at..at + s].to_vec());
        at += s;
    }
    // Place each layer on the velocities around its level; a layer that ends up with no
    // velocity of its own joins its louder neighbour.
    loop {
        let centre = |g: &Vec<&Hit>| g.iter().map(|h| h.level).sum::<f32>() / g.len() as f32;
        let bounds: Vec<u8> = (0..groups.len())
            .map(|k| {
                if k + 1 == groups.len() {
                    127
                } else {
                    let mid = (centre(&groups[k]) + centre(&groups[k + 1])) / 2.0 - top;
                    velocity_of(mid).round().clamp(1.0, 127.0) as u8
                }
            })
            .collect();
        let mut lo = 1u8;
        let mut empty = None;
        for (k, &b) in bounds.iter().enumerate() {
            if b < lo {
                empty = Some(k);
                break;
            }
            lo = b.saturating_add(1);
        }
        match empty {
            Some(k) if groups.len() > 1 => {
                let into = if k + 1 < groups.len() { k + 1 } else { k - 1 };
                let moved = std::mem::take(&mut groups[k]);
                groups[into].extend(moved);
                groups[into].sort_by(|a, b| a.level.total_cmp(&b.level).then(a.file.cmp(&b.file)));
                groups.remove(k);
                continue;
            }
            _ => {}
        }
        let mut out = Vec::new();
        let mut lo = 1u8;
        for (g, &hi) in groups.iter().zip(&bounds) {
            let len = g.len().min(16);
            for (i, h) in g.iter().take(16).enumerate() {
                out.push(Region {
                    file: h.file.clone(),
                    lovel: lo,
                    hivel: hi.max(lo),
                    seq: i as u8 + 1,
                    seqlen: len as u8,
                    volume: (top - h.level).min(MAX_VOLUME),
                });
            }
            lo = hi.saturating_add(1);
        }
        let over: usize = groups.iter().map(|g| g.len().saturating_sub(16)).sum();
        return (out, dropped + over);
    }
}

/// Contiguous runs of `keys`: `[47, 48, 50]` → `[(47, 48), (50, 50)]`.
pub fn runs(keys: &[u8]) -> Vec<(u8, u8)> {
    let mut k: Vec<u8> = keys.to_vec();
    k.sort_unstable();
    k.dedup();
    let mut out: Vec<(u8, u8)> = Vec::new();
    for n in k {
        match out.last_mut() {
            Some(r) if r.1 + 1 == n => r.1 = n,
            _ => out.push((n, n)),
        }
    }
    out
}

/// One built instrument for the SFZ: its name, keys, choke class and regions.
pub struct Part {
    pub name: String,
    pub keys: Vec<u8>,
    /// Exclusive class (1…) and its release, ms.
    pub choke: Option<(u8, f32)>,
    pub regions: Vec<Region>,
}

/// The SFZ text of a built kit. `folder` is where the regions' files are, relative to the
/// SFZ; `credit` heads the file.
pub fn sfz(parts: &[Part], folder: &str, credit: &str) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    for line in credit.lines() {
        let _ = writeln!(s, "// {line}");
    }
    let _ = writeln!(
        s,
        "// Velocity layers from each hit's level (SFZ's velocity curve), round robin in order."
    );
    let _ = writeln!(s, "<control>\ndefault_path={folder}/");
    let _ = writeln!(
        s,
        "<global>\nloop_mode=one_shot amp_veltrack=100 pitch_keytrack=0"
    );
    for p in parts {
        let _ = write!(s, "\n<group> group_label={}", p.name);
        if let Some((c, ms)) = p.choke {
            let _ = write!(
                s,
                " group={c} off_by={c} off_mode=normal ampeg_release={}",
                ms / 1000.0
            );
        }
        s.push('\n');
        for (lo, hi) in runs(&p.keys) {
            for r in &p.regions {
                let _ = write!(
                    s,
                    "<region> lokey={lo} hikey={hi} pitch_keycenter={lo} lovel={} hivel={}",
                    r.lovel, r.hivel
                );
                if r.seqlen > 1 {
                    let _ = write!(s, " seq_length={} seq_position={}", r.seqlen, r.seq);
                }
                if r.volume != 0.0 {
                    let _ = write!(s, " volume={:.3}", r.volume);
                }
                let _ = writeln!(s, " sample={}", r.file);
            }
        }
    }
    s
}

/// Build `b` inside `dir` (the pack's unpacked folder): mix, write the instrument, and
/// remove every unpacked file the pack does not keep. `report(done, total)` counts hits.
pub fn build(
    b: &Build,
    dir: &Path,
    credit: &str,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(u64, u64),
) -> Result<Built, String> {
    let built = render(b, dir, dir, credit, cancel, report)?;
    prune(dir, b)?;
    Ok(built)
}

/// Mix the kit in `src` and write the instrument and its files into `out` (the same folder
/// at install; another one to try a mix without touching the kit).
pub fn render(
    b: &Build,
    src: &Path,
    out: &Path,
    credit: &str,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(u64, u64),
) -> Result<Built, String> {
    let dir = src;
    let kit: Kit = drumgizmo::read_kit(dir, &b.kit)?;
    for m in &b.mics {
        if !kit.channels.contains(&m.channel) {
            return Err(format!("microphone {} is not one of the kit's", m.channel));
        }
    }
    let mut plan = Vec::new();
    for k in &b.keys {
        let inst = kit
            .instruments
            .iter()
            .find(|i| i.name == k.instrument)
            .ok_or_else(|| format!("the kit has no instrument {}", k.instrument))?;
        plan.push((k, inst));
    }
    let total: u64 = plan.iter().map(|(_, i)| i.samples.len() as u64).sum();
    let out_root = drumgizmo::path_of(out, &b.folder);
    std::fs::create_dir_all(&out_root).map_err(|e| e.to_string())?;
    let mut built = Built::default();
    let mut reader = Reader {
        path: String::new(),
        audio: None,
    };
    let mut parts = Vec::new();
    let mut done = 0u64;
    for (k, inst) in &plan {
        crate::manifest::check_path(&inst.name).map_err(|e| format!("{}: {e}", inst.name))?;
        let sub = out_root.join(&inst.name);
        std::fs::create_dir_all(&sub).map_err(|e| e.to_string())?;
        let mics = gains(b, &inst.name, k.gain_db);
        let mut hits = Vec::new();
        for (i, s) in inst.samples.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            let (stereo, rate) = mix(dir, s, &mics, &mut reader)?;
            let name = format!("{:02}.wav", i + 1);
            built.clamped += crate::wav::write_stereo_24(&sub.join(&name), rate, &stereo)?;
            built.peak = stereo.iter().fold(built.peak, |p, x| p.max(x.abs()));
            built.files += 1;
            hits.push(Hit {
                file: format!("{}/{name}", inst.name),
                level: level_db(&stereo, rate),
            });
            done += 1;
            report(done, total);
        }
        let (regions, dropped) = layers(&hits, b.round_robin, b.range_db);
        if dropped > 0 {
            built.log.push(format!(
                "{}: {dropped} of {} hits left out (softer than {} dB under the loudest)",
                inst.name,
                hits.len(),
                b.range_db
            ));
        }
        built.dropped += dropped;
        // Hits no region plays are not kept.
        for h in &hits {
            if !regions.iter().any(|r| r.file == h.file) {
                let _ = std::fs::remove_file(drumgizmo::path_of(&out_root, &h.file));
                built.files -= 1;
            }
        }
        let choke = b
            .chokes
            .iter()
            .position(|c| c.instruments.iter().any(|n| *n == inst.name))
            .map(|c| (c as u8 + 1, b.chokes[c].release_ms));
        built.regions += regions.len() * runs(&k.keys).len();
        parts.push(Part {
            name: inst.name.clone(),
            keys: k.keys.clone(),
            choke,
            regions,
        });
    }
    drop(reader);
    let text = sfz(&parts, &b.folder, credit);
    std::fs::write(drumgizmo::path_of(out, &b.sfz), text).map_err(|e| e.to_string())?;
    Ok(built)
}

/// Remove every file under `dir` but the instrument, its folder and the kept files, and
/// the folders left empty.
fn prune(dir: &Path, b: &Build) -> Result<(), String> {
    let folder = format!("{}/", b.folder);
    let keep =
        |rel: &str| rel == b.sfz || rel.starts_with(&folder) || b.keep.iter().any(|k| k == rel);
    walk(dir, dir, &keep).map(|_| ())
}

/// Remove what `keep` refuses under `at`; whether `at` is left empty.
fn walk(root: &Path, at: &Path, keep: &dyn Fn(&str) -> bool) -> Result<bool, String> {
    let mut empty = true;
    for e in std::fs::read_dir(at).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        let rel = p
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if e.file_type().map_err(|e| e.to_string())?.is_dir() {
            if walk(root, &p, keep)? {
                std::fs::remove_dir(&p).map_err(|e| e.to_string())?;
            } else {
                empty = false;
            }
        } else if keep(&rel) {
            empty = false;
        } else {
            std::fs::remove_file(&p).map_err(|e| e.to_string())?;
        }
    }
    Ok(empty)
}
