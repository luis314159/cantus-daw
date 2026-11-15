//! The microphones of a multi-mic SFZ library (ADR 0123 §5, research-p3 `formats.md`
//! §1.6): which mic each region is, found by the first rule that applies — and logged.
//!
//! 1. **`output=n`** on the regions: each distinct `n` is a microphone.
//! 2. **A level controller per mic**: an ARIA `<master>` (or `<group>`) carries its own
//!    `amplitude_onccN` / `volume_onccN`, named by `label_ccN`. A controller is a mic's
//!    fader when it tells apart the regions *of one key* (the same strike heard by two
//!    mics) and reaches more than one key — a piece's fader (`Kick vol`) or a bleed trim
//!    (`Kick dirt`) does neither. Its `set_ccN` is the mic's starting level.
//! 3. **Folders or name tokens** that differ only by a mic word (`Samples/oh_mic/…`,
//!    `snare_OH_v1.wav`): the position whose values merge most regions into strikes.
//! 4. Otherwise **one** output.
//!
//! Used by Tympanum's kit loader (`crate::tympanum_load`) and by Chorda's multi-mic SFZ
//! pianos (`chorda-src`).

use super::{num, Flat, Ops};
use cantus_core::inst_out::{family_of_name, FoundMic, MicFinding, MicRule, MAX_OUTPUTS};
use std::collections::{BTreeMap, BTreeSet};

/// The level controllers a region carries: `amplitude_ccN`, `amplitude_onccN`,
/// `volume_onccN`, `gain_onccN`, `gain_ccN`.
fn level_ccs(ops: &Ops) -> BTreeSet<u8> {
    let mut out = BTreeSet::new();
    for k in ops.keys() {
        let n = [
            "amplitude_oncc",
            "amplitude_cc",
            "volume_oncc",
            "gain_oncc",
            "gain_cc",
        ]
        .iter()
        .find_map(|p| k.strip_prefix(p));
        if let Some(n) = n.and_then(|n| n.parse::<u8>().ok()) {
            out.insert(n);
        }
    }
    out
}

/// The keys a region plays (`key`, or `lokey` … `hikey`).
fn keys_of(ops: &Ops) -> (u8, u8) {
    let note = |k: &str| {
        ops.get(k)
            .and_then(|v| super::parse_note(v))
            .map(|n| n.min(127))
    };
    match note("key") {
        Some(k) => (k, k),
        None => (note("lokey").unwrap_or(0), note("hikey").unwrap_or(127)),
    }
}

fn is_file(ops: &Ops) -> bool {
    ops.get("sample")
        .is_some_and(|s| !s.trim_start().starts_with('*'))
}

/// A mic's name as a person reads it, from a folder or token (`oh_mic` → `OH`,
/// `wndw_mic` → `Window`, `kick_mic` → `Kick`).
pub fn pretty(raw: &str) -> String {
    let lower = raw.to_lowercase();
    let mut s = lower
        .trim_end_matches("_mic")
        .trim_end_matches(" mic")
        .trim_end_matches("-mic")
        .trim_start_matches("mic_")
        .replace(['_', '-'], " ");
    s = s.trim().to_string();
    let known = [
        ("oh", "OH"),
        ("wndw", "Window"),
        ("amb", "Ambience"),
        ("hh", "Hi-hat"),
        ("bd", "Kick"),
        ("sd", "Snare"),
    ];
    if let Some((_, k)) = known.iter().find(|(a, _)| *a == s) {
        return (*k).to_string();
    }
    let mut out = String::new();
    for (i, w) in s.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        if w.len() <= 2 {
            out.push_str(&w.to_uppercase());
        } else {
            let mut c = w.chars();
            if let Some(f) = c.next() {
                out.extend(f.to_uppercase());
                out.push_str(c.as_str());
            }
        }
    }
    if out.is_empty() {
        raw.to_string()
    } else {
        out
    }
}

fn mic(name: String, family_from: &str) -> FoundMic {
    FoundMic {
        family: family_of_name(family_from),
        name,
        cc: None,
        level_db: 0.0,
        invert: false,
        channels: 1,
    }
}

/// The microphones of `flat`'s regions: the rule, the mics in output order, and each
/// region's mic (in `MicFinding::of_zone`, one per region). The log line says which rule
/// found them.
/// More level controllers than this are per piece, not per microphone.
const MAX_LEVEL_MICS: usize = 12;

pub fn find(flat: &Flat) -> (MicFinding, String) {
    if let Some(f) = by_output(flat) {
        let line = format!("microphones from output= ({} outputs)", f.mics.len());
        return (f, line);
    }
    if let Some(f) = by_level_cc(flat) {
        // r40:tymp-engine — a library whose faders are per piece and per mic (Big Rusty
        // Drums: "Kick OH", "Snare OH", …) is not 30 microphones: its sample folders say
        // which mics there are, when they say a few.
        if f.mics.len() > MAX_LEVEL_MICS {
            if let Some(g) = by_folder(flat).filter(|g| g.mics.len() <= MAX_LEVEL_MICS) {
                let names: Vec<&str> = g.mics.iter().map(|m| m.name.as_str()).collect();
                let line = format!(
                    "microphones from sample folders: {} ({} level controllers, per piece)",
                    names.join(", "),
                    f.mics.len()
                );
                return (g, line);
            }
        }
        let names: Vec<&str> = f.mics.iter().map(|m| m.name.as_str()).collect();
        let line = format!(
            "microphones from their level controllers: {}",
            names.join(", ")
        );
        return (f, line);
    }
    if let Some(f) = by_folder(flat) {
        let names: Vec<&str> = f.mics.iter().map(|m| m.name.as_str()).collect();
        let line = format!("microphones from sample folders: {}", names.join(", "));
        return (f, line);
    }
    let f = MicFinding {
        rule: MicRule::Single,
        mics: vec![mic("Kit".into(), "")],
        of_zone: vec![0; flat.regions.len()],
    };
    (f, "one output: no microphones found".into())
}

/// Each zone's mic, from each zone's region (`parse_flat`'s second value).
pub fn of_zones(f: &MicFinding, zone_region: &[usize]) -> Vec<u8> {
    zone_region
        .iter()
        .map(|r| f.of_zone.get(*r).copied().unwrap_or(0))
        .collect()
}

fn by_output(flat: &Flat) -> Option<MicFinding> {
    let outs: BTreeSet<u32> = flat
        .regions
        .iter()
        .filter_map(|r| num(&r.ops, "output"))
        .map(|v| v.max(0.0) as u32)
        .collect();
    if outs.len() < 2 {
        return None;
    }
    let order: Vec<u32> = outs.into_iter().take(MAX_OUTPUTS).collect();
    let mics = order
        .iter()
        .map(|o| mic(format!("Output {}", o + 1), ""))
        .collect();
    let of_zone = flat
        .regions
        .iter()
        .map(|r| {
            let o = num(&r.ops, "output").map_or(0, |v| v.max(0.0) as u32);
            order.iter().position(|x| *x == o).unwrap_or(0) as u8
        })
        .collect();
    Some(MicFinding {
        rule: MicRule::Outputs,
        mics,
        of_zone,
    })
}

fn by_level_cc(flat: &Flat) -> Option<MicFinding> {
    let regions: Vec<(&Ops, BTreeSet<u8>)> = flat
        .regions
        .iter()
        .filter(|r| is_file(&r.ops))
        .map(|r| (&r.ops, level_ccs(&r.ops)))
        .collect();
    let all: BTreeSet<u8> = regions
        .iter()
        .flat_map(|(_, c)| c.iter().copied())
        .collect();
    let mut mic_ccs: Vec<u8> = Vec::new();
    for cc in all {
        let mut keys_with = BTreeSet::new();
        let mut split_a_key = false;
        let mut by_key: BTreeMap<u8, (bool, bool)> = BTreeMap::new();
        for (ops, ccs) in &regions {
            let (lo, hi) = keys_of(ops);
            for k in lo..=hi.min(lo.saturating_add(16)) {
                let e = by_key.entry(k).or_default();
                if ccs.contains(&cc) {
                    e.0 = true;
                    keys_with.insert(k);
                } else {
                    e.1 = true;
                }
            }
        }
        for (_, (with, without)) in by_key {
            if with && without {
                split_a_key = true;
            }
        }
        if split_a_key && keys_with.len() >= 2 {
            mic_ccs.push(cc);
        }
    }
    if mic_ccs.len() < 2 {
        return None;
    }
    mic_ccs.truncate(MAX_OUTPUTS - 1);
    let label = |cc: u8| flat.control.get(&format!("label_cc{cc}")).cloned();
    let set = |cc: u8| num(&flat.control, &format!("set_cc{cc}")).unwrap_or(0.0);
    let mut mics: Vec<FoundMic> = mic_ccs
        .iter()
        .map(|cc| {
            let name = label(*cc).unwrap_or_else(|| format!("Mic {cc}"));
            let name = name
                .trim()
                .trim_end_matches(" vol")
                .trim_end_matches(" Vol")
                .trim_end_matches(" volume")
                .trim_end_matches(" mic")
                .trim_end_matches(" Mic")
                .to_string();
            let mut m = mic(name.clone(), &name);
            m.cc = Some(*cc);
            let v = set(*cc).clamp(0.0, 127.0) / 127.0;
            m.level_db = if v > 0.0 {
                20.0 * cantus_dsp::math::log10(v)
            } else {
                -96.0
            };
            m
        })
        .collect();
    let mut other = false;
    let of_zone: Vec<u8> = flat
        .regions
        .iter()
        .map(|r| {
            let ccs = level_ccs(&r.ops);
            match mic_ccs.iter().position(|c| ccs.contains(c)) {
                Some(i) => i as u8,
                None => {
                    other = true;
                    mic_ccs.len() as u8
                }
            }
        })
        .collect();
    if other {
        mics.push(mic("Other".into(), ""));
    }
    Some(MicFinding {
        rule: MicRule::LevelCc,
        mics,
        of_zone,
    })
}

/// The pieces of a sample path a mic may be named by: each folder (counted from the
/// file, 1 the folder it is in) and each token of the file's name (negative positions).
fn parts(path: &str) -> (Vec<String>, Vec<String>) {
    let p = path.replace('\\', "/");
    let mut comps: Vec<String> = p.split('/').map(str::to_string).collect();
    let file = comps.pop().unwrap_or_default();
    comps.reverse();
    let stem = file.rsplit_once('.').map_or(file.as_str(), |(s, _)| s);
    let tokens = stem
        .split(['_', ' ', '-', '.'])
        .map(str::to_string)
        .collect();
    (comps, tokens)
}

fn by_folder(flat: &Flat) -> Option<MicFinding> {
    let files: Vec<(usize, String)> = flat
        .regions
        .iter()
        .enumerate()
        .filter(|(_, r)| is_file(&r.ops))
        .filter_map(|(i, r)| r.ops.get("sample").map(|s| (i, s.trim().to_string())))
        .collect();
    // What makes regions one strike: the keys, velocities and round robin they play on —
    // a mic's part of a strike differs from another's only by the mic.
    let strike: Vec<String> = files
        .iter()
        .map(|(i, _)| {
            let o = &flat.regions[*i].ops;
            let g = |k: &str| o.get(k).map(|v| v.trim().to_string()).unwrap_or_default();
            let (lo, hi) = keys_of(o);
            format!(
                "{lo}-{hi}|{}|{}|{}|{}|{}|{}",
                g("lovel"),
                g("hivel"),
                g("seq_position"),
                g("lorand"),
                g("locc4"),
                g("hicc4")
            )
        })
        .collect();
    if files.len() < 2 {
        return None;
    }
    // Every candidate position: a folder (counted from the file) or a token of the name.
    // The one whose values, wildcarded, merge the most regions into strikes wins.
    let split: Vec<(Vec<String>, Vec<String>)> = files.iter().map(|(_, p)| parts(p)).collect();
    let max_dir = split.iter().map(|(d, _)| d.len()).max().unwrap_or(0);
    let max_tok = split.iter().map(|(_, t)| t.len()).max().unwrap_or(0);
    let mut best: Option<(usize, bool, usize, usize)> = None; // (pos, is_dir, merged, values)
    for (is_dir, count) in [(true, max_dir), (false, max_tok)] {
        for pos in 0..count {
            let mut values = BTreeSet::new();
            let mut groups = BTreeSet::new();
            for ((d, t), sk) in split.iter().zip(&strike) {
                let list = if is_dir { d } else { t };
                let v = list.get(pos).cloned().unwrap_or_default();
                let mut rest = list.clone();
                if pos < rest.len() {
                    rest[pos] = "*".into();
                }
                let other = if is_dir { t.join("_") } else { d.join("/") };
                values.insert(v);
                groups.insert((rest.join("|"), other, sk.clone()));
            }
            let merged = split.len() - groups.len();
            let n = values.len();
            if !(2..=MAX_OUTPUTS).contains(&n) || split.len() < n * 2 {
                continue;
            }
            // Strikes must merge: each wildcard group about one region per mic.
            if (split.len() as f32) / (groups.len() as f32) < 1.5 {
                continue;
            }
            if best.is_none_or(|b| merged > b.2) {
                best = Some((pos, is_dir, merged, n));
            }
        }
    }
    let (pos, is_dir, _, _) = best?;
    let value = |path: &str| {
        let (d, t) = parts(path);
        let list = if is_dir { d } else { t };
        list.get(pos).cloned().unwrap_or_default()
    };
    let mut names: Vec<String> = files.iter().map(|(_, p)| value(p)).collect();
    names.sort();
    names.dedup();
    let mics = names.iter().map(|n| mic(pretty(n), n)).collect();
    let of_zone = flat
        .regions
        .iter()
        .map(|r| {
            r.ops
                .get("sample")
                .filter(|_| is_file(&r.ops))
                .and_then(|s| names.iter().position(|n| *n == value(s.trim())))
                .unwrap_or(0) as u8
        })
        .collect();
    Some(MicFinding {
        rule: MicRule::Folders,
        mics,
        of_zone,
    })
}

#[cfg(test)]
mod tests {
    use super::super::flatten;
    use super::*;

    /// sfzformat.com's volume tutorial: a `<master>` per mic, each with its own CC.
    #[test]
    fn a_master_per_mic_with_its_own_cc_is_two_mics_named_by_their_labels() {
        let text = "<control> label_cc80=Close vol label_cc81=Far vol set_cc80=127 set_cc81=64\n\
            <master> amplitude_oncc80=100\n<group> key=36\n<region> sample=close/k1.wav\n\
            <group> key=38\n<region> sample=close/s1.wav\n\
            <master> amplitude_oncc81=100\n<group> key=36\n<region> sample=far/k1.wav\n\
            <group> key=38\n<region> sample=far/s1.wav\n";
        let flat = flatten(text, &|_| None);
        let (f, line) = find(&flat);
        assert_eq!(f.rule, MicRule::LevelCc, "{line}");
        let names: Vec<&str> = f.mics.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Close", "Far"]);
        assert_eq!(f.of_zone, [0, 0, 1, 1]);
        assert_eq!(f.mics[0].level_db, 0.0);
        assert!((f.mics[1].level_db - 20.0 * (64.0f32 / 127.0).log10()).abs() < 1e-3);
    }

    #[test]
    fn output_wins_and_one_folder_per_mic_is_found() {
        let text = "<region> key=36 output=1 sample=a.wav\n<region> key=36 output=0 sample=b.wav\n";
        let (f, _) = find(&flatten(text, &|_| None));
        assert_eq!(f.rule, MicRule::Outputs);
        assert_eq!(f.of_zone, [1, 0]);
        // A piece's fader (cc100 on one key) is not a mic's: Gogodze's folders are.
        let text = "<control> label_cc100=Kick vol set_cc100=100\n\
            <master> amplitude_cc100=100\n<region> key=36 sample=../Samples/kick_mic/ks_vl1_rr1.wav\n\
            <master> amplitude_cc100=60\n<region> key=36 sample=../Samples/oh_mic/ks_vl1_rr1.wav\n\
            <master>\n<region> key=38 sample=../Samples/kick_mic/sn_vl1_rr1.wav\n\
            <region> key=38 sample=../Samples/oh_mic/sn_vl1_rr1.wav\n";
        let (f, line) = find(&flatten(text, &|_| None));
        assert_eq!(f.rule, MicRule::Folders, "{line}");
        let names: Vec<&str> = f.mics.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Kick", "OH"]);
        assert_eq!(f.of_zone, [0, 1, 0, 1]);
        assert_eq!(pretty("wndw_mic"), "Window");
        assert_eq!(pretty("front_mic"), "Front");
    }
}
