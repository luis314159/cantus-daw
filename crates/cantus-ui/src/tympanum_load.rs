//! Tympanum's recorded kits, read from their packs (round 40 phase 3, ADR 0124): the app's
//! side of the kit loading. [`install`] gives the engine a resolver (at start-up,
//! `CantusApp::assemble`); the engine calls it off the audio thread the first time a song
//! names a pack's kit (`cantus_engine::inst::tympanum::cache::recorded`), and keeps what it
//! read for the session.
//!
//! - **SFZ multi-mic kits** ([`read_sfz`]): the regions with their headers inherited
//!   (`sampler_sfz::flatten`), the microphones by ADR 0123 §5's rule
//!   (`sampler_sfz_mics`), every controller at its `set_ccN` (a library's piece and mic
//!   faders, its tuning knob), the hi-hat's levels on CC4, round robins and `lorand` as
//!   takes of one strike, `group`/`off_by` as chokes.
//! - **DrumGizmo kits**: [`dg`] (multichannel hits, `main`, chokes).

use crate::views::inst::sampler::import::meta_of;
use crate::views::inst::sampler::sfz::{self, mics, Flat, Ops};
use cantus_core::inst::tympanum::{KitRef, RoomChoice};
use cantus_core::inst::tympanum_kit::{
    ArtHits, KitDef, KitHits, MicDef, MicTake, RecordedKit, Take, VelLayer,
};
use cantus_core::inst_out::{OutputFamily, OutputId};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

#[path = "tympanum_load_dg.rs"]
pub mod dg;
#[path = "tympanum_load_gm.rs"]
pub mod gm;

/// Where packs are installed (`<data dir>/Content` unless a test says).
static ROOT: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Give the engine the kit loader, reading packs under `content_root`.
pub fn install(content_root: PathBuf) {
    if let Ok(mut r) = ROOT.write() {
        *r = Some(content_root);
    }
    cantus_engine::inst::tympanum::cache::set_resolver(Arc::new(|kit: &KitRef| {
        let root = ROOT
            .read()
            .ok()
            .and_then(|r| r.clone())
            .ok_or_else(|| "no content folder".to_string())?;
        read_pack_kit(&root, kit)
    }));
    // r40:drums-hyb — a modelled kit's hybrid residual, read from its pack's folder (a pack
    // is moved there whole once installed).
    cantus_engine::inst::tympanum_hybrid::set_source(Arc::new(|pack: &str, file: &str| {
        let root = ROOT
            .read()
            .ok()
            .and_then(|r| r.clone())
            .ok_or_else(|| "no content folder".to_string())?;
        std::fs::read(root.join(pack).join(file)).map_err(|e| format!("{pack}: {e}"))
    }));
}

/// A pack's kit: its receipt names the instrument's file (`.sfz` or a DrumGizmo `.xml`).
pub fn read_pack_kit(root: &Path, kit: &KitRef) -> Result<RecordedKit, String> {
    let KitRef::Pack { pack, kit: id } = kit else {
        return Err("not a pack's kit".into());
    };
    let dir = root.join(pack);
    let text = std::fs::read_to_string(dir.join(cantus_content::install::RECEIPT))
        .map_err(|e| format!("{pack} is not installed ({e})"))?;
    let receipt: cantus_content::Receipt =
        serde_json::from_str(&text).map_err(|e| format!("{pack}'s receipt: {e}"))?;
    let inst = receipt
        .instruments
        .iter()
        .find(|i| i.id == *id)
        .ok_or_else(|| format!("{pack} has no kit {id}"))?;
    let file = dir.join(&inst.file);
    let licence = Some(receipt.licence.clone());
    let credit = receipt
        .credit_line()
        .or_else(|| receipt.credits.first().map(|c| c.name.clone()));
    let name = inst.name.en.clone();
    let lower = inst.file.to_lowercase();
    let mut k = if lower.ends_with(".xml") {
        dg::read_drumgizmo(&file, kit.clone(), &name)?
    } else {
        read_sfz(&file, kit.clone(), &name)?
    };
    k.def.licence = licence;
    k.def.credit = credit;
    // The pack's own gain (ADR 0039: where the native instruments sit).
    if inst.gain_db != 0.0 {
        for m in &mut k.def.mics {
            m.level_db += inst.gain_db;
        }
    }
    Ok(k)
}

fn f(ops: &Ops, k: &str) -> Option<f32> {
    ops.get(k).and_then(|v| v.trim().parse::<f32>().ok())
}

/// A controller-driven opcode's value at the controllers' defaults: `base` plus every
/// `<prefix>_onccN` / `<prefix>_ccN` × `cc / 127`.
fn with_ccs(ops: &Ops, base: f32, prefix: &str, cc: &[f32; 128]) -> f32 {
    let mut v = base;
    for (k, x) in ops {
        let n = k
            .strip_prefix(prefix)
            .and_then(|r| r.strip_prefix("_oncc").or_else(|| r.strip_prefix("_cc")))
            .and_then(|n| n.parse::<usize>().ok());
        if let (Some(n), Ok(d)) = (n, x.trim().parse::<f32>()) {
            if n < 128 {
                v += d * curve(ops, prefix, n, cc[n] / 127.0);
            }
        }
    }
    v
}

/// A controller's value through the opcode's curve (`<prefix>_curveccN`): SFZ's predefined
/// curves 0 … 6; a custom `<curve>` (not read) taken as linear.
fn curve(ops: &Ops, prefix: &str, n: usize, x: f32) -> f32 {
    let c = f(ops, &format!("{prefix}_curvecc{n}")).map_or(0, |c| c as i32);
    match c {
        1 => 2.0 * x - 1.0,
        2 => 1.0 - x,
        3 => 1.0 - 2.0 * x,
        4 => x * x,
        5 => x.sqrt(),
        6 => (1.0 - x).sqrt(),
        _ => x,
    }
}

/// ARIA's `amplitude` (percent) times each `amplitude_onccN` (percent at full) — the
/// library's faders at their defaults; negative inverts.
fn amplitude(ops: &Ops, cc: &[f32; 128]) -> f32 {
    let mut a = f(ops, "amplitude").unwrap_or(100.0) / 100.0;
    for (k, x) in ops {
        let n = k
            .strip_prefix("amplitude_oncc")
            .or_else(|| k.strip_prefix("amplitude_cc"))
            .and_then(|n| n.parse::<usize>().ok());
        if let (Some(n), Ok(d)) = (n, x.trim().parse::<f32>()) {
            if n < 128 {
                a *= d / 100.0 * cc[n] / 127.0;
            }
        }
    }
    a
}

/// Whether a region plays with every controller at its default (`loccN`/`hiccN`), but
/// for the hi-hat's `hat_cc`, which Tympanum crossfades.
fn plays_at_defaults(ops: &Ops, cc: &[f32; 128], hat_cc: usize, selectors: &[usize]) -> bool {
    for (k, x) in ops {
        let (lo, n) = if let Some(n) = k.strip_prefix("locc") {
            (true, n)
        } else if let Some(n) = k.strip_prefix("hicc") {
            (false, n)
        } else {
            continue;
        };
        let (Ok(n), Ok(v)) = (n.parse::<usize>(), x.trim().parse::<f32>()) else {
            continue;
        };
        if n == hat_cc || n >= 128 || selectors.contains(&n) {
            continue;
        }
        if (lo && cc[n] < v) || (!lo && cc[n] > v) {
            return false;
        }
    }
    true
}

/// The controllers a library uses to choose microphones (Gogodze's *Mic mix*: `locc105` on
/// every region of its far mics): a condition every region of a mic shares, and only
/// whole mics carry. In Tympanum the mixer chooses the mics, so they gate nothing.
fn mic_selectors(flat: &Flat, of_region: &[u8], hat_cc: usize) -> Vec<usize> {
    // Per controller: per mic, (regions, regions with a condition on it, the conditions).
    let mut seen: Seen = BTreeMap::new();
    let mut per_mic: BTreeMap<u8, usize> = BTreeMap::new();
    for (ri, r) in flat.regions.iter().enumerate() {
        if !r
            .ops
            .get("sample")
            .is_some_and(|s| !s.trim().starts_with('*'))
        {
            continue;
        }
        let m = of_region.get(ri).copied().unwrap_or(0);
        *per_mic.entry(m).or_default() += 1;
        for (k, v) in &r.ops {
            let n = k
                .strip_prefix("locc")
                .or_else(|| k.strip_prefix("hicc"))
                .and_then(|n| n.parse::<usize>().ok());
            if let Some(n) = n.filter(|n| *n != hat_cc && *n < 128) {
                let e = seen.entry(n).or_default().entry(m).or_default();
                e.1 += 1;
                let c = format!("{k}={}", v.trim());
                if !e.2.contains(&c) {
                    e.2.push(c);
                }
            }
        }
    }
    seen.into_iter()
        .filter(|(_, mics)| {
            mics.iter().all(|(m, (_, with, conds))| {
                *with == per_mic.get(m).copied().unwrap_or(0) && conds.len() == 1
            }) && mics.len() < per_mic.len()
        })
        .map(|(n, _)| n)
        .collect()
}

/// The region's range on the hat controller, 0 … 127.
fn hat_range(ops: &Ops, hat_cc: usize) -> Option<(u8, u8)> {
    let lo = f(ops, &format!("locc{hat_cc}"));
    let hi = f(ops, &format!("hicc{hat_cc}"));
    (lo.is_some() || hi.is_some()).then(|| {
        (
            lo.unwrap_or(0.0).clamp(0.0, 127.0) as u8,
            hi.unwrap_or(127.0).clamp(0.0, 127.0) as u8,
        )
    })
}

fn keys(ops: &Ops) -> Option<(u8, u8)> {
    let note = |k: &str| {
        ops.get(k)
            .and_then(|v| crate::views::inst::sampler::import::parse_note(v))
    };
    match note("key") {
        Some(k) => Some((k, k)),
        None => match (note("lokey"), note("hikey")) {
            (Some(a), Some(b)) => Some((a.min(b), a.max(b))),
            (Some(a), None) | (None, Some(a)) => Some((a, a)),
            _ => None,
        },
    }
}

/// An SFZ multi-mic kit.
pub fn read_sfz(path: &Path, kit: KitRef, name: &str) -> Result<RecordedKit, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let include = |n: &str| std::fs::read_to_string(base.join(n.replace('\\', "/"))).ok();
    let flat = sfz::flatten(&text, &include);
    let mut k = kit_from_flat(&flat, &base, kit, name);
    // A DrumGizmo kit built for Tympanum says which mic is which drum's own (its `main`).
    let side = format!("{}.mics.json", path.to_string_lossy());
    if let Ok(t) = std::fs::read_to_string(&side) {
        if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(&t) {
            for (folder, v) in map {
                let keys: Vec<u8> = v["main"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_u64())
                            .map(|x| x as u8)
                            .collect()
                    })
                    .unwrap_or_default();
                let pretty = mics::pretty(&folder);
                let Some(m) = k.def.mics.iter_mut().find(|m| m.name == pretty) else {
                    continue;
                };
                let all = keys.is_empty()
                    || k.def
                        .pieces
                        .iter()
                        .all(|p| p.articulations.iter().any(|a| keys.contains(&a.key)));
                m.main_for = if all {
                    Vec::new()
                } else {
                    k.def
                        .pieces
                        .iter()
                        .filter(|p| p.articulations.iter().any(|a| keys.contains(&a.key)))
                        .map(|p| p.index)
                        .collect()
                };
            }
            k.log
                .push("which mic is each drum's own: from the kit's own `main`".into());
        }
    }
    Ok(k)
}

/// What an articulation is made of while it is read: (key, hat range) → layers (lo, hi)
/// → takes (seq position, lorand) → that take's mic files.
type ArtKey = (u8, Option<(u8, u8)>);
type LayerKey = (u8, u8);
type TakeKey = (u32, u32);
/// An articulation's layers as they are read: each layer's velocity curve and its takes.
type Layers = BTreeMap<LayerKey, (Vec<(u8, f32)>, BTreeMap<TakeKey, Take>)>;
/// A choke group's members: (group, off_by, a hat's CC4 range).
type Group = (i64, u8, Option<(u8, u8)>);
/// Per controller: per mic, (regions, regions with a condition on it, the conditions).
type Seen = BTreeMap<usize, BTreeMap<u8, (usize, usize, Vec<String>)>>;

/// The kit an SFZ file describes (pure but for one header read per mic: its channels).
pub fn kit_from_flat(flat: &Flat, base: &Path, kit: KitRef, name: &str) -> RecordedKit {
    let mut log = Vec::new();
    let (finding, line) = mics::find(flat);
    log.push(line);
    let hat_cc = 4usize;
    let mut cc = [0.0f32; 128];
    for (k, v) in &flat.control {
        if let (Some(n), Ok(x)) = (
            k.strip_prefix("set_cc")
                .and_then(|n| n.parse::<usize>().ok()),
            v.trim().parse::<f32>(),
        ) {
            if n < 128 {
                cc[n] = x.clamp(0.0, 127.0);
            }
        }
    }
    let default_path = flat
        .control
        .get("default_path")
        .map(|p| p.replace('\\', "/"))
        .unwrap_or_default();
    let mut files: Vec<String> = Vec::new();
    let mut file_ix: BTreeMap<String, u32> = BTreeMap::new();
    let mut arts: BTreeMap<ArtKey, Layers> = BTreeMap::new();
    let mut art_meta: BTreeMap<ArtKey, (f32, u8, Vec<i64>, Option<i64>)> = BTreeMap::new();
    // Silencers: (group, key, hat range) of every region (silence included).
    let mut groups: Vec<Group> = Vec::new();
    let mut gated = 0usize;
    let selectors = mic_selectors(flat, &finding.of_zone, hat_cc);
    if !selectors.is_empty() {
        log.push(format!(
            "controllers that choose microphones ({selectors:?}) are the mixer's now: every mic plays"
        ));
    }
    for (ri, r) in flat.regions.iter().enumerate() {
        let ops = &r.ops;
        let Some((lo_k, hi_k)) = keys(ops) else {
            continue;
        };
        let hr = hat_range(ops, hat_cc);
        if let Some(g) = f(ops, "group").or_else(|| f(ops, "polyphony_group")) {
            for k in lo_k..=hi_k {
                groups.push((g as i64, k, hr));
            }
        }
        let Some(sample) = ops.get("sample").map(|s| s.trim().replace('\\', "/")) else {
            continue;
        };
        if sample.starts_with('*') {
            continue;
        }
        if ops.get("trigger").is_some_and(|t| t.trim() != "attack") {
            continue;
        }
        if !plays_at_defaults(ops, &cc, hat_cc, &selectors) {
            gated += 1;
            continue;
        }
        let full = base.join(format!("{default_path}{sample}"));
        let full = normalise(&full);
        let fi = *file_ix.entry(full.clone()).or_insert_with(|| {
            files.push(full.clone());
            (files.len() - 1) as u32
        });
        let mic = finding.of_zone.get(ri).copied().unwrap_or(0);
        let vol = with_ccs(ops, f(ops, "volume").unwrap_or(0.0), "volume", &cc);
        let gain = amplitude(ops, &cc) * cantus_dsp::math::db_to_gain(vol);
        let pan = with_ccs(ops, f(ops, "pan").unwrap_or(0.0), "pan", &cc) / 100.0;
        let cents = with_ccs(ops, f(ops, "tune").unwrap_or(0.0), "tune", &cc)
            + 100.0 * f(ops, "transpose").unwrap_or(0.0);
        let offset = f(ops, "offset").unwrap_or(0.0).max(0.0) as u32;
        let highpass = match ops.get("fil_type").map(|t| t.trim()) {
            Some(t) if t.starts_with("hpf") => f(ops, "cutoff").unwrap_or(0.0).max(0.0),
            _ => 0.0,
        };
        let lovel = f(ops, "lovel").unwrap_or(0.0).clamp(0.0, 127.0) as u8;
        let hivel = f(ops, "hivel").unwrap_or(127.0).clamp(0.0, 127.0) as u8;
        let mut curve: Vec<(u8, f32)> = ops
            .iter()
            .filter_map(|(k, v)| {
                let n = k.strip_prefix("amp_velcurve_")?.parse::<u8>().ok()?;
                Some((n.min(127), v.trim().parse::<f32>().ok()?))
            })
            .collect();
        curve.sort_by_key(|c| c.0);
        let seq = f(ops, "seq_position").unwrap_or(1.0).max(1.0) as u32;
        let rand = (f(ops, "lorand").unwrap_or(0.0) * 1000.0) as u32;
        let release_ms = 1000.0 * f(ops, "ampeg_release").unwrap_or(0.07).max(0.005);
        let fast = ops.get("off_mode").is_some_and(|m| m.trim() == "fast");
        let poly = f(ops, "polyphony").unwrap_or(0.0).clamp(0.0, 64.0) as u8;
        let off_by = f(ops, "off_by").map(|x| x as i64);
        let group = f(ops, "group").map(|x| x as i64);
        for key in lo_k..=hi_k.min(lo_k.saturating_add(24)) {
            let ak = (key, hr);
            let layer = arts
                .entry(ak)
                .or_default()
                .entry((lovel.max(1), hivel))
                .or_insert_with(|| (curve.clone(), BTreeMap::new()));
            layer.1.entry((seq, rand)).or_default().mics.push(MicTake {
                mic,
                file: fi,
                channel: None,
                gain,
                pan,
                offset,
                cents,
                highpass,
            });
            let m = art_meta.entry(ak).or_insert((
                if fast { 5.0 } else { release_ms },
                0,
                Vec::new(),
                None,
            ));
            m.1 = m.1.max(poly);
            if let Some(o) = off_by {
                if !m.2.contains(&o) {
                    m.2.push(o);
                }
            }
            m.3 = m.3.or(group);
        }
    }
    if gated > 0 {
        log.push(format!(
            "{gated} regions left out: a controller at its default does not play them"
        ));
    }
    build(
        flat, finding, files, arts, art_meta, groups, kit, name, log, base,
    )
}

/// Forward slashes, `..` resolved: one spelling per file.
fn normalise(p: &Path) -> String {
    let mut out: Vec<String> = Vec::new();
    let s = p.to_string_lossy().replace('\\', "/");
    for c in s.split('/') {
        match c {
            "." => {}
            ".." if out
                .last()
                .is_some_and(|l| l != ".." && !l.is_empty() && !l.ends_with(':')) =>
            {
                out.pop();
            }
            _ => out.push(c.to_string()),
        }
    }
    out.join("/")
}

#[allow(clippy::too_many_arguments)]
fn build(
    _flat: &Flat,
    finding: cantus_core::inst_out::MicFinding,
    files: Vec<String>,
    arts: BTreeMap<ArtKey, Layers>,
    art_meta: BTreeMap<ArtKey, (f32, u8, Vec<i64>, Option<i64>)>,
    groups: Vec<Group>,
    kit: KitRef,
    name: &str,
    mut log: Vec<String>,
    _base: &Path,
) -> RecordedKit {
    // The pieces, by the General MIDI drum map; the articulations in key order.
    let keys: Vec<ArtKey> = arts.keys().copied().collect();
    let (pieces, art_place) = gm::pieces_for(&keys);
    let art_index: BTreeMap<ArtKey, usize> =
        keys.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let mut out_arts: Vec<ArtHits> = Vec::new();
    for (i, (ak, layers)) in arts.into_iter().enumerate() {
        let (piece, articulation, openness) = art_place[i];
        let meta = art_meta
            .get(&ak)
            .cloned()
            .unwrap_or((70.0, 0, Vec::new(), None));
        let layers = layers
            .into_iter()
            .map(|((lo, hi), (curve, takes))| VelLayer {
                lo,
                hi,
                curve,
                power: None,
                takes: takes.into_values().collect(),
            })
            .collect();
        out_arts.push(ArtHits {
            piece,
            articulation,
            key: ak.0,
            openness,
            layers,
            chokes: Vec::new(),
            choke_ms: meta.0,
            polyphony: meta.1,
        });
    }
    // Chokes: an articulation whose regions say `off_by=g` is silenced by every
    // articulation with a region in group g (a silence region too) on its key and range.
    for (ak, meta) in &art_meta {
        let Some(&victim) = art_index.get(ak) else {
            continue;
        };
        for g in &meta.2 {
            for (gg, key, hr) in &groups {
                if gg != g {
                    continue;
                }
                for (other, &oi) in &art_index {
                    let overlap = match (other.1, hr) {
                        (Some((a, b)), Some((c, d))) => a <= *d && *c <= b,
                        _ => true,
                    };
                    if other.0 == *key && overlap && !out_arts[oi].chokes.contains(&(victim as u16))
                    {
                        out_arts[oi].chokes.push(victim as u16);
                    }
                }
            }
        }
    }
    // The microphones, each with its channels read from one of its files.
    let mut mics: Vec<MicDef> = Vec::new();
    for (i, m) in finding.mics.iter().enumerate() {
        let file = out_arts
            .iter()
            .flat_map(|a| &a.layers)
            .flat_map(|l| &l.takes)
            .flat_map(|t| &t.mics)
            .find(|t| t.mic as usize == i)
            .and_then(|t| files.get(t.file as usize));
        let channels = file
            .and_then(|f| meta_of(Path::new(f)))
            .map_or(m.channels, |x| x.channels.clamp(1, 2) as u8);
        let main_for = pieces
            .iter()
            .filter(|p| gm::close_mic_hears(m.family, p.kind))
            .map(|p| p.index)
            .collect();
        mics.push(MicDef {
            output: OutputId(i as u16),
            name: m.name.clone(),
            family: m.family,
            channels,
            pattern: gm::pattern_of(m.family),
            place: gm::mic_place(m.family, i),
            main_for,
            movable: false,
            pan: 0.0,
            level_db: m.level_db,
            spread: if channels == 2 { 60.0 } else { 0.0 },
        });
    }
    let files_n = files.len();
    log.push(format!(
        "{} pieces, {} articulations, {} microphones, {} files",
        pieces.len(),
        out_arts.len(),
        mics.len(),
        files_n
    ));
    let def = KitDef {
        kit,
        name: name.to_string(),
        modelled: false,
        pieces,
        mics,
        bleed: Default::default(),
        room: gm::studio_room(),
        default_room: RoomChoice::Own,
        licence: None,
        credit: None,
        // A pack says nothing of its shells: the studio kit's maple.
        finish: cantus_core::inst::chorda_look::Finish::Maple,
    };
    let _ = OutputFamily::Other;
    RecordedKit {
        def,
        hits: KitHits {
            files,
            arts: out_arts,
        },
        log,
    }
}
