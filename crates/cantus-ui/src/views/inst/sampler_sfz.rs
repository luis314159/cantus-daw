//! SFZ: import (the report's tier-1 opcodes and the tier-2 filter, `research/instruments/
//! sampler.md` §5.2) and export. The parser is pure: includes and sample metadata come
//! through closures, so it is tested without files.
//!
//! Headers inherit downward (`<global>` → `<master>` → `<group>` → `<region>`, the most
//! specific winning); `#define $NAME value` and `#include "file"` are honoured; comments
//! are `//` and `/* */`; `sample=` runs to the next opcode, so paths may hold spaces.
//! Every opcode that is not mapped is counted and logged.

use super::import::{parse_note, Import, Meta};
use cantus_core::inst::sampler::{gf, zf, Group, Zone, MAX_GROUPS};
use std::collections::BTreeMap;

type Ops = BTreeMap<String, String>;

/// Strip comments and expand `#define`s and `#include`s (up to a depth of 8).
fn preprocess(
    text: &str,
    include: &dyn Fn(&str) -> Option<String>,
    depth: usize,
    defines: &mut Vec<(String, String)>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_block = false;
    for line in text.lines() {
        let mut l = String::new();
        let mut rest = line;
        loop {
            if in_block {
                match rest.find("*/") {
                    Some(i) => {
                        rest = &rest[i + 2..];
                        in_block = false;
                    }
                    None => break,
                }
            } else {
                let c = rest.find("//");
                let b = rest.find("/*");
                match (c, b) {
                    (Some(ci), Some(bi)) if bi < ci => {
                        l.push_str(&rest[..bi]);
                        rest = &rest[bi + 2..];
                        in_block = true;
                    }
                    (Some(ci), _) => {
                        l.push_str(&rest[..ci]);
                        break;
                    }
                    (None, Some(bi)) => {
                        l.push_str(&rest[..bi]);
                        rest = &rest[bi + 2..];
                        in_block = true;
                    }
                    (None, None) => {
                        l.push_str(rest);
                        break;
                    }
                }
            }
        }
        let trimmed = l.trim_start();
        if let Some(d) = trimmed.strip_prefix("#define") {
            let mut parts = d.split_whitespace();
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                defines.push((k.to_string(), v.to_string()));
            }
            continue;
        }
        if let Some(inc) = trimmed.strip_prefix("#include") {
            let name = inc.trim().trim_matches('"');
            if depth < 8 {
                if let Some(t) = include(name) {
                    out.push_str(&preprocess(&t, include, depth + 1, defines));
                    out.push('\n');
                }
            }
            continue;
        }
        // Longest names first, so $A does not eat $AB.
        let mut sorted: Vec<&(String, String)> = defines.iter().collect();
        sorted.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
        for (k, v) in sorted {
            if l.contains(k.as_str()) {
                l = l.replace(k.as_str(), v);
            }
        }
        out.push_str(&l);
        out.push('\n');
    }
    out
}

/// One element of an SFZ file.
#[derive(Debug, PartialEq)]
enum Tok {
    Header(String),
    Op(String, String),
}

fn is_opcode_start(s: &str) -> bool {
    // `name=` where name is letters, digits and underscores.
    let Some(eq) = s.find('=') else { return false };
    eq > 0
        && s[..eq]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

fn tokenize(text: &str) -> Vec<Tok> {
    let mut toks = Vec::new();
    for line in text.lines() {
        let mut rest = line.trim();
        while !rest.is_empty() {
            if let Some(r) = rest.strip_prefix('<') {
                let end = r.find('>').unwrap_or(r.len());
                toks.push(Tok::Header(r[..end].trim().to_ascii_lowercase()));
                rest = r.get(end + 1..).unwrap_or("").trim_start();
                continue;
            }
            let Some(eq) = rest.find('=') else { break };
            let name = rest[..eq].trim().to_string();
            let after = &rest[eq + 1..];
            // The value runs to the next `name=` or header; most values have no spaces,
            // but `sample` (and labels) may.
            let mut end = after.len();
            let mut i = 0;
            let bytes = after.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'<' {
                    end = i;
                    break;
                }
                if bytes[i].is_ascii_whitespace() {
                    let next = after[i..].trim_start();
                    if next.starts_with('<')
                        || is_opcode_start(next.split_whitespace().next().unwrap_or(""))
                    {
                        end = i;
                        break;
                    }
                }
                i += 1;
            }
            toks.push(Tok::Op(
                name.to_ascii_lowercase(),
                after[..end].trim().to_string(),
            ));
            rest = after[end..].trim_start();
        }
    }
    toks
}

fn num(ops: &Ops, k: &str) -> Option<f32> {
    ops.get(k).and_then(|v| v.trim().parse::<f32>().ok())
}

fn note(ops: &Ops, k: &str, offset: i32) -> Option<u8> {
    ops.get(k)
        .and_then(|v| parse_note(v))
        .map(|n| (n as i32 + offset).clamp(0, 127) as u8)
}

/// The opcodes Exemplar maps (anything else is logged).
const KNOWN: &[&str] = &[
    "sample",
    "default_path",
    "key",
    "lokey",
    "hikey",
    "pitch_keycenter",
    "lovel",
    "hivel",
    "note_offset",
    "octave_offset",
    "offset",
    "end",
    "loop_mode",
    "loopmode",
    "loop_start",
    "loopstart",
    "loop_end",
    "loopend",
    "loop_crossfade",
    "direction",
    "delay",
    "volume",
    "amplitude",
    "pan",
    "tune",
    "transpose",
    "pitch_keytrack",
    "amp_veltrack",
    "bend_up",
    "bend_down",
    "ampeg_delay",
    "ampeg_attack",
    "ampeg_hold",
    "ampeg_decay",
    "ampeg_sustain",
    "ampeg_release",
    "trigger",
    "rt_decay",
    "seq_length",
    "seq_position",
    "lorand",
    "hirand",
    "sw_lokey",
    "sw_hikey",
    "sw_last",
    "sw_default",
    "group",
    "off_by",
    "off_mode",
    "polyphony",
    "xfin_lovel",
    "xfin_hivel",
    "xfout_lovel",
    "xfout_hivel",
    "xfin_lokey",
    "xfin_hikey",
    "xfout_lokey",
    "xfout_hikey",
    "xf_velcurve",
    "xf_keycurve",
    "cutoff",
    "resonance",
    "fil_type",
    "fil_keytrack",
    "fil_veltrack",
    "fileg_depth",
    "fileg_attack",
    "fileg_decay",
    "fileg_sustain",
    "fileg_release",
    "pitchlfo_freq",
    "pitchlfo_depth",
    "pitchlfo_delay",
];

/// Silently ignored: labels, hints and the like, which change no sound.
fn ignorable(k: &str) -> bool {
    k.starts_with("hint_")
        || k.ends_with("_label")
        || k.starts_with("label_cc")
        || k.starts_with("set_cc")
        || k == "sample_quality"
        || k == "image"
}

/// The built-in generators SFZ names (`sample=*sine`) and Exemplar's `gen` for them.
const GENERATORS: [(&str, f32); 5] = [
    ("*sine", 1.0),
    ("*saw", 2.0),
    ("*square", 3.0),
    ("*triangle", 4.0),
    ("*noise", 5.0),
];

/// Parse an SFZ file. `base` is its folder (sample paths are relative to it); `include`
/// reads an `#include`d file by its relative name; `meta` reads a sample's metadata.
pub fn parse(
    text: &str,
    base: &std::path::Path,
    include: &dyn Fn(&str) -> Option<String>,
    meta: &dyn Fn(&str) -> Option<Meta>,
) -> Import {
    let mut defines = Vec::new();
    let text = preprocess(text, include, 0, &mut defines);
    #[derive(PartialEq, Clone, Copy)]
    enum At {
        None,
        Control,
        Global,
        Master,
        Group,
        Region,
        Other,
    }
    let (mut control, mut global, mut master, mut group) =
        (Ops::new(), Ops::new(), Ops::new(), Ops::new());
    let mut region: Option<Ops> = None;
    let mut regions: Vec<(Ops, usize)> = Vec::new();
    let mut sfz_group = 0usize;
    let mut at = At::None;
    let mut out = Import::default();
    let mut other_headers = BTreeMap::<String, usize>::new();
    let flush = |region: &mut Option<Ops>,
                 regions: &mut Vec<(Ops, usize)>,
                 g: &Ops,
                 m: &Ops,
                 gl: &Ops,
                 n: usize| {
        if let Some(r) = region.take() {
            let mut merged = gl.clone();
            merged.extend(m.clone());
            merged.extend(g.clone());
            merged.extend(r);
            regions.push((merged, n));
        }
    };
    for t in tokenize(&text) {
        match t {
            Tok::Header(h) => {
                flush(
                    &mut region,
                    &mut regions,
                    &group,
                    &master,
                    &global,
                    sfz_group,
                );
                at = match h.as_str() {
                    "control" => At::Control,
                    "global" => {
                        global.clear();
                        master.clear();
                        group.clear();
                        At::Global
                    }
                    "master" => {
                        master.clear();
                        group.clear();
                        At::Master
                    }
                    "group" => {
                        group.clear();
                        sfz_group += 1;
                        At::Group
                    }
                    "region" => {
                        region = Some(Ops::new());
                        At::Region
                    }
                    other => {
                        *other_headers.entry(other.to_string()).or_default() += 1;
                        At::Other
                    }
                };
            }
            Tok::Op(k, v) => {
                let target = match at {
                    At::Control => &mut control,
                    At::Global => &mut global,
                    At::Master => &mut master,
                    At::Group => &mut group,
                    At::Region => region.get_or_insert_with(Ops::new),
                    At::None => &mut global,
                    At::Other => continue,
                };
                target.insert(k, v);
            }
        }
    }
    flush(
        &mut region,
        &mut regions,
        &group,
        &master,
        &global,
        sfz_group,
    );
    for (h, n) in other_headers {
        out.log.push(format!("<{h}> ignored ({n})"));
    }
    let offset = num(&control, "note_offset").unwrap_or(0.0) as i32
        + 12 * num(&control, "octave_offset").unwrap_or(0.0) as i32;
    let default_path = control
        .get("default_path")
        .map(|p| p.replace('\\', "/"))
        .unwrap_or_default();
    let mut unsupported = BTreeMap::<String, usize>::new();
    let mut group_of: Vec<(Vec<u32>, usize)> = Vec::new();
    for (ops, sg) in &regions {
        for k in ops.keys() {
            if !KNOWN.contains(&k.as_str()) && !ignorable(k) {
                *unsupported.entry(k.clone()).or_default() += 1;
            }
        }
        let Some(sample) = ops.get("sample") else {
            out.log.push("a region without a sample was skipped".into());
            continue;
        };
        let sample = sample.replace('\\', "/");
        let mut z = Zone::default();
        let mut file_meta = None;
        if let Some(g) = GENERATORS.iter().find(|g| g.0 == sample) {
            z.set(zf::GEN, g.1).set(zf::MODE, 2.0);
        } else if sample.starts_with('*') {
            out.log.push(format!("generator {sample} is not available"));
            continue;
        } else {
            let rel = format!("{default_path}{sample}");
            let path = base.join(&rel);
            let s = path.to_string_lossy().replace('\\', "/");
            file_meta = meta(&s);
            z.sample = Some(s);
        }
        region_to_zone(ops, offset, file_meta.as_ref(), &mut z);
        // Groups: one per SFZ group and envelope, so regions that differ keep their own.
        let env_key: Vec<u32> = std::iter::once(*sg as u32)
            .chain(
                [
                    "ampeg_attack",
                    "ampeg_hold",
                    "ampeg_decay",
                    "ampeg_sustain",
                    "ampeg_release",
                    "amp_veltrack",
                    "polyphony",
                ]
                .iter()
                .map(|k| num(ops, k).map(|v| v.to_bits()).unwrap_or(u32::MAX)),
            )
            .collect();
        let gi = match group_of.iter().find(|(k, _)| *k == env_key) {
            Some((_, i)) => *i,
            None => {
                let i = group_of.len().min(MAX_GROUPS - 1);
                if group_of.len() == MAX_GROUPS {
                    out.log.push(format!(
                        "more than {MAX_GROUPS} groups: merged into the last"
                    ));
                }
                group_of.push((env_key, i));
                let mut g = Group::default();
                let ms = |k: &str, d: f32| (num(ops, k).unwrap_or(d) * 1000.0).max(0.1);
                g.values[gf::ATTACK] = Some(ms("ampeg_attack", 0.0));
                g.values[gf::HOLD] = Some(ms("ampeg_hold", 0.0));
                g.values[gf::DECAY] = Some(ms("ampeg_decay", 0.0).max(1.0));
                g.values[gf::SUSTAIN] =
                    Some(num(ops, "ampeg_sustain").unwrap_or(100.0).clamp(0.0, 100.0));
                g.values[gf::RELEASE] = Some(ms("ampeg_release", 0.001).max(1.0));
                if let Some(v) = num(ops, "amp_veltrack") {
                    g.values[gf::VEL] = Some(v.clamp(-100.0, 100.0));
                }
                if let Some(p) = num(ops, "polyphony") {
                    g.values[gf::POLY] = Some(p.clamp(0.0, 64.0));
                }
                out.groups.push((i, g));
                i
            }
        };
        z.set(zf::GROUP, gi as f32);
        out.zones.push(z);
    }
    for (k, n) in unsupported {
        out.log
            .push(format!("unsupported opcode {k} ({n} regions)"));
    }
    instrument_values(regions.first().map(|r| &r.0), &control, &mut out);
    out.clamp();
    out
}

/// A region's opcodes onto a zone.
fn region_to_zone(ops: &Ops, offset: i32, meta: Option<&Meta>, z: &mut Zone) {
    let key = note(ops, "key", offset);
    let root = match ops.get("pitch_keycenter").map(String::as_str) {
        Some("sample") => meta.and_then(|m| m.root),
        _ => note(ops, "pitch_keycenter", offset),
    }
    .or(key)
    .unwrap_or(60);
    let lo = note(ops, "lokey", offset).or(key).unwrap_or(0);
    let hi = note(ops, "hikey", offset).or(key).unwrap_or(127);
    z.set(zf::LO, lo as f32)
        .set(zf::HI, hi as f32)
        .set(zf::ROOT, root as f32);
    let (lovel, hivel) = (
        num(ops, "lovel").unwrap_or(1.0).max(1.0),
        num(ops, "hivel").unwrap_or(127.0),
    );
    z.set(zf::VLO, lovel).set(zf::VHI, hivel);
    if let Some(v) = num(ops, "offset") {
        z.set(zf::START, v);
    }
    if let Some(v) = num(ops, "end").filter(|v| *v > 0.0) {
        z.set(zf::END, v + 1.0);
    }
    let mode = ops
        .get("loop_mode")
        .or_else(|| ops.get("loopmode"))
        .map(String::as_str);
    let file_loop = meta.and_then(|m| m.sustain_loop);
    let m = match mode {
        Some("one_shot") => 1.0,
        Some("loop_continuous") => 2.0,
        Some("loop_sustain") => 3.0,
        Some(_) => 0.0,
        None if file_loop.is_some() => 2.0,
        None => 0.0,
    };
    z.set(zf::MODE, m);
    if m >= 2.0 {
        let ls = num(ops, "loop_start").or_else(|| num(ops, "loopstart"));
        let le = num(ops, "loop_end").or_else(|| num(ops, "loopend"));
        let (fs, fe) = file_loop
            .map(|(s, e, _)| (s as f32, e as f32))
            .unwrap_or((0.0, 0.0));
        z.set(zf::LS, ls.unwrap_or(fs))
            .set(zf::LE, le.unwrap_or(fe));
        if let Some(xf) = num(ops, "loop_crossfade") {
            z.set(zf::XF, xf * 1000.0);
        }
        if file_loop.is_some_and(|l| l.2) && ls.is_none() {
            z.set(zf::DIR, 1.0);
        }
    }
    if ops.get("direction").is_some_and(|d| d == "reverse") {
        z.set(zf::REV, 1.0);
    }
    if let Some(d) = num(ops, "delay") {
        z.set(zf::DELAY, d * 1000.0);
    }
    let mut gain = num(ops, "volume").unwrap_or(0.0);
    if let Some(a) = num(ops, "amplitude") {
        gain += 20.0 * (a.max(0.01) / 100.0).log10();
    }
    z.set(zf::GAIN, gain);
    if let Some(p) = num(ops, "pan") {
        z.set(zf::PAN, p);
    }
    let tune = num(ops, "tune").unwrap_or(0.0) + 100.0 * num(ops, "transpose").unwrap_or(0.0);
    z.set(zf::TUNE, tune);
    if let Some(k) = num(ops, "pitch_keytrack") {
        z.set(zf::KEYTRACK, k);
    }
    let trig = match ops.get("trigger").map(String::as_str) {
        Some("release") => 1.0,
        Some("first") => 2.0,
        Some("legato") => 3.0,
        Some("release_key") => 4.0,
        _ => 0.0,
    };
    z.set(zf::TRIG, trig);
    if let Some(v) = num(ops, "rt_decay") {
        z.set(zf::RTDECAY, v);
    }
    if let (Some(len), pos) = (num(ops, "seq_length"), num(ops, "seq_position")) {
        z.set(zf::SEQLEN, len).set(zf::SEQ, pos.unwrap_or(1.0));
    }
    if let Some(v) = num(ops, "lorand") {
        z.set(zf::RLO, v);
    }
    if let Some(v) = num(ops, "hirand") {
        z.set(zf::RHI, v);
    }
    if let Some(k) = note(ops, "sw_last", offset) {
        z.set(zf::SW, k as f32);
    }
    if let Some(v) = num(ops, "group").filter(|v| *v > 0.0) {
        z.set(zf::EXCL, v.min(64.0));
    }
    if let Some(v) = num(ops, "off_by").filter(|v| *v > 0.0) {
        z.set(zf::OFFBY, v.min(64.0));
    }
    if ops.get("off_mode").is_some_and(|m| m == "normal") {
        z.set(zf::OFFMODE, 1.0);
    }
    // Velocity crossfades: the zone's range and fade widths from SFZ's four points.
    if let (Some(a), Some(b)) = (num(ops, "xfin_lovel"), num(ops, "xfin_hivel")) {
        z.set(zf::VLO, a.max(lovel)).set(zf::VFIN, b - a);
    }
    if let (Some(a), Some(b)) = (num(ops, "xfout_lovel"), num(ops, "xfout_hivel")) {
        z.set(zf::VHI, b.min(hivel)).set(zf::VFOUT, b - a);
    }
    if let (Some(a), Some(b)) = (
        note(ops, "xfin_lokey", offset),
        note(ops, "xfin_hikey", offset),
    ) {
        z.set(zf::LO, a.max(lo) as f32)
            .set(zf::KFIN, b.saturating_sub(a) as f32);
    }
    if let (Some(a), Some(b)) = (
        note(ops, "xfout_lokey", offset),
        note(ops, "xfout_hikey", offset),
    ) {
        z.set(zf::HI, b.min(hi) as f32)
            .set(zf::KFOUT, b.saturating_sub(a) as f32);
    }
    if ops.get("xf_velcurve").is_some_and(|c| c == "gain")
        || ops.get("xf_keycurve").is_some_and(|c| c == "gain")
    {
        z.set(zf::FC, 1.0);
    }
}

/// The instrument-wide opcodes (as the first region sees them) onto Exemplar's own keys.
fn instrument_values(first: Option<&Ops>, control: &Ops, out: &mut Import) {
    let Some(ops) = first else { return };
    let v = &mut out.values;
    if let Some(b) = num(ops, "bend_up") {
        v.push(("bend".into(), (b / 100.0).round().clamp(0.0, 24.0)));
    }
    if let Some(c) = num(ops, "cutoff") {
        let t = ops.get("fil_type").map(String::as_str).unwrap_or("lpf_2p");
        let kind = if t.starts_with("hpf") {
            3.0
        } else if t.starts_with("bpf") {
            2.0
        } else {
            1.0
        };
        v.push(("filter.type".into(), kind));
        v.push(("filter.cutoff".into(), c.clamp(20.0, 20_000.0)));
        let q = (0.707 * 10f32.powf(num(ops, "resonance").unwrap_or(0.0) / 20.0)).max(0.5);
        v.push((
            "filter.reso".into(),
            ((2.0 - 1.0 / q) / 1.96 * 100.0).clamp(0.0, 100.0),
        ));
        if let Some(k) = num(ops, "fil_keytrack") {
            v.push(("filter.key".into(), k.clamp(0.0, 100.0)));
        }
        if let Some(k) = num(ops, "fil_veltrack") {
            v.push(("filter.vel".into(), (k / 100.0).clamp(0.0, 96.0)));
        }
        if let Some(d) = num(ops, "fileg_depth") {
            v.push(("filter.env".into(), (d / 100.0).clamp(-96.0, 96.0)));
            let ms = |k: &str| num(ops, k).map(|s| (s * 1000.0).max(0.1));
            for (k, key) in [
                ("fileg_attack", "env2.attack"),
                ("fileg_decay", "env2.decay"),
                ("fileg_release", "env2.release"),
            ] {
                if let Some(t) = ms(k) {
                    v.push((key.into(), t.max(1.0)));
                }
            }
            if let Some(s) = num(ops, "fileg_sustain") {
                v.push(("env2.sustain".into(), s.clamp(0.0, 100.0)));
            }
        }
    }
    if let Some(f) = num(ops, "pitchlfo_freq") {
        v.push(("lfo1.rate".into(), f.clamp(0.01, 40.0)));
        if let Some(d) = num(ops, "pitchlfo_depth") {
            v.push(("lfo1.pitch".into(), d.abs().min(100.0)));
        }
        if let Some(d) = num(ops, "pitchlfo_delay") {
            v.push(("lfo1.delay".into(), (d * 1000.0).clamp(0.0, 5_000.0)));
        }
    }
    let offset = num(control, "note_offset").unwrap_or(0.0) as i32
        + 12 * num(control, "octave_offset").unwrap_or(0.0) as i32;
    for (k, key) in [
        ("sw_lokey", "sw.lo"),
        ("sw_hikey", "sw.hi"),
        ("sw_default", "sw.default"),
    ] {
        if let Some(n) = note(ops, k, offset) {
            v.push((key.into(), n as f32));
        }
    }
}

/// Write the instrument as SFZ: one `<group>` per Exemplar group with its effective
/// envelope, one `<region>` per zone. `rel` turns a sample path into what the file names.
pub fn export(spec: &cantus_core::NativeSpec, rel: &dyn Fn(&str) -> String) -> String {
    use cantus_core::inst::sampler::zones;
    use std::fmt::Write;
    let mut s = String::from("// Exported by Cantus Exemplar\n<control>\n<global>\n");
    let g = |k: &str| spec.get(k);
    let bend = g("bend") * 100.0;
    let _ = writeln!(s, "bend_up={bend} bend_down=-{bend}");
    if g("filter.type") >= 1.0 {
        let t = ["lpf_2p", "lpf_2p", "bpf_2p", "hpf_2p"][g("filter.type") as usize];
        let q = 1.0 / (2.0 - 1.96 * g("filter.reso") / 100.0);
        let _ = writeln!(
            s,
            "fil_type={t} cutoff={} resonance={:.2} fil_keytrack={} fil_veltrack={}",
            g("filter.cutoff"),
            (20.0 * (q / 0.707).log10()).max(0.0),
            g("filter.key"),
            g("filter.vel") * 100.0
        );
    }
    if g("sw.lo") >= 0.0 {
        let _ = writeln!(
            s,
            "sw_lokey={} sw_hikey={}",
            g("sw.lo"),
            g("sw.hi").max(g("sw.lo"))
        );
        if g("sw.default") >= 0.0 {
            let _ = writeln!(s, "sw_default={}", g("sw.default"));
        }
    }
    let all = zones(spec);
    let mut groups: Vec<usize> = all.iter().map(|(_, z)| z.get(zf::GROUP) as usize).collect();
    groups.sort_unstable();
    groups.dedup();
    for m in groups {
        let grp = Group::of(spec, m);
        let env = |i: usize, key: &str| grp.values[i].unwrap_or(spec.get(key));
        let _ = writeln!(
            s,
            "\n<group> ampeg_attack={} ampeg_hold={} ampeg_decay={} ampeg_sustain={} ampeg_release={} amp_veltrack={}",
            env(gf::ATTACK, "amp.attack") / 1000.0,
            env(gf::HOLD, "amp.hold") / 1000.0,
            env(gf::DECAY, "amp.decay") / 1000.0,
            env(gf::SUSTAIN, "amp.sustain"),
            env(gf::RELEASE, "amp.release") / 1000.0,
            env(gf::VEL, "vel.amp"),
        );
        if let Some(p) = grp.values[gf::POLY].filter(|p| *p > 0.0) {
            let _ = writeln!(s, "polyphony={p}");
        }
        if let Some(n) = &grp.name {
            let _ = writeln!(s, "group_label={n}");
        }
        for (_, z) in all.iter().filter(|(_, z)| z.get(zf::GROUP) as usize == m) {
            s.push_str(&region_text(z, rel));
        }
    }
    s
}

fn region_text(z: &Zone, rel: &dyn Fn(&str) -> String) -> String {
    use std::fmt::Write;
    let mut r = String::from("<region>");
    let sample = match &z.sample {
        Some(p) => rel(p),
        None => GENERATORS
            .iter()
            .find(|g| g.1 == z.get(zf::GEN))
            .map(|g| g.0.to_string())
            .unwrap_or_else(|| "*sine".into()),
    };
    let _ = write!(
        r,
        " lokey={} hikey={} pitch_keycenter={} lovel={} hivel={}",
        z.lo(),
        z.hi(),
        z.root(),
        z.vlo(),
        z.vhi()
    );
    let mut opt = |cond: bool, text: String| {
        if cond {
            r.push(' ');
            r.push_str(&text);
        }
    };
    let v = |i: usize| z.get(i);
    opt(v(zf::TUNE) != 0.0, format!("tune={}", v(zf::TUNE)));
    opt(v(zf::GAIN) != 0.0, format!("volume={}", v(zf::GAIN)));
    opt(v(zf::PAN) != 0.0, format!("pan={}", v(zf::PAN)));
    opt(
        v(zf::KEYTRACK) != 100.0,
        format!("pitch_keytrack={}", v(zf::KEYTRACK)),
    );
    opt(v(zf::START) > 0.0, format!("offset={}", v(zf::START)));
    opt(v(zf::END) > 0.0, format!("end={}", v(zf::END) - 1.0));
    let mode = v(zf::MODE) as usize;
    let modes = ["no_loop", "one_shot", "loop_continuous", "loop_sustain"];
    opt(true, format!("loop_mode={}", modes[mode.min(3)]));
    if mode >= 2 {
        opt(
            true,
            format!("loop_start={} loop_end={}", v(zf::LS), v(zf::LE)),
        );
        opt(
            v(zf::XF) > 0.0,
            format!("loop_crossfade={}", v(zf::XF) / 1000.0),
        );
    }
    opt(v(zf::REV) >= 0.5, "direction=reverse".into());
    opt(
        v(zf::DELAY) > 0.0,
        format!("delay={}", v(zf::DELAY) / 1000.0),
    );
    let trig = ["attack", "release", "first", "legato", "release_key"];
    opt(
        v(zf::TRIG) > 0.0,
        format!("trigger={}", trig[(v(zf::TRIG) as usize).min(4)]),
    );
    opt(v(zf::RTDECAY) > 0.0, format!("rt_decay={}", v(zf::RTDECAY)));
    opt(
        v(zf::SEQLEN) > 1.0,
        format!("seq_length={} seq_position={}", v(zf::SEQLEN), v(zf::SEQ)),
    );
    opt(v(zf::RLO) > 0.0, format!("lorand={}", v(zf::RLO)));
    opt(v(zf::RHI) < 1.0, format!("hirand={}", v(zf::RHI)));
    opt(v(zf::SW) >= 0.0, format!("sw_last={}", v(zf::SW)));
    opt(v(zf::EXCL) > 0.0, format!("group={}", v(zf::EXCL)));
    opt(v(zf::OFFBY) > 0.0, format!("off_by={}", v(zf::OFFBY)));
    opt(v(zf::OFFMODE) >= 0.5, "off_mode=normal".into());
    opt(
        v(zf::VFIN) > 0.0,
        format!(
            "xfin_lovel={} xfin_hivel={}",
            v(zf::VLO),
            v(zf::VLO) + v(zf::VFIN)
        ),
    );
    opt(
        v(zf::VFOUT) > 0.0,
        format!(
            "xfout_lovel={} xfout_hivel={}",
            v(zf::VHI) - v(zf::VFOUT),
            v(zf::VHI)
        ),
    );
    opt(
        v(zf::KFIN) > 0.0,
        format!(
            "xfin_lokey={} xfin_hikey={}",
            v(zf::LO),
            v(zf::LO) + v(zf::KFIN)
        ),
    );
    opt(
        v(zf::KFOUT) > 0.0,
        format!(
            "xfout_lokey={} xfout_hikey={}",
            v(zf::HI) - v(zf::KFOUT),
            v(zf::HI)
        ),
    );
    opt(v(zf::FC) >= 0.5, "xf_velcurve=gain xf_keycurve=gain".into());
    // The sample last: its value may hold spaces and runs to the end of the line.
    let _ = writeln!(r, " sample={sample}");
    r
}
