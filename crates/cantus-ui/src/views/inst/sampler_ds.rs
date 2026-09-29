//! Decent Sampler `.dspreset` import (unencrypted; `research/instruments/sampler.md` §5.3).
//!
//! Attributes inherit from `<groups>` to `<group>` to `<sample>`, the most specific
//! winning — except **volume, which multiplies** across the three levels, and **pan and
//! tuning, which add**. Tags become exclusive classes: a zone tagged `hat` belongs to the
//! class `hat`; one `silencedByTags="hat"` is turned off when a `hat` starts.

use super::import::{parse_note, Import, Meta};
use super::xml::{self as sampler_xml, Element};
use cantus_core::inst::sampler::{gf, zf, Group, Zone, MAX_GROUPS};
use std::path::Path;

/// A volume attribute: `-6dB` or a linear factor (`0.5`), as dB.
fn volume_db(v: &str) -> Option<f32> {
    let t = v.trim();
    let lower = t.to_ascii_lowercase();
    if let Some(db) = lower.strip_suffix("db") {
        return db.trim().parse().ok();
    }
    let lin: f32 = t.parse().ok()?;
    Some(20.0 * lin.max(1e-4).log10())
}

fn f(e: Option<&str>) -> Option<f32> {
    e.and_then(|v| v.trim().parse::<f32>().ok())
}

fn truthy(v: Option<&str>) -> Option<bool> {
    v.map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
}

/// Parse a `.dspreset`. `base` is its folder; `meta` reads a sample's metadata.
pub fn parse(
    text: &str,
    base: &Path,
    meta: &dyn Fn(&str) -> Option<Meta>,
) -> Result<Import, String> {
    let root = sampler_xml::parse(text)?;
    if !root.name.eq_ignore_ascii_case("DecentSampler") {
        return Err(format!("<{}> is not a Decent Sampler preset", root.name));
    }
    let mut out = Import::default();
    let empty = Element::default();
    let groups = root.child("groups").unwrap_or(&empty);
    let mut tags: Vec<String> = Vec::new();
    let class = |name: &str, tags: &mut Vec<String>| -> f32 {
        let name = name.split(',').next().unwrap_or("").trim().to_string();
        if name.is_empty() {
            return 0.0;
        }
        let i = match tags.iter().position(|t| *t == name) {
            Some(i) => i,
            None => {
                tags.push(name);
                tags.len() - 1
            }
        };
        (i + 1).min(64) as f32
    };
    for (gi, group) in groups.children_named("group").enumerate() {
        if truthy(group.attr("enabled")) == Some(false) {
            continue;
        }
        let m = gi.min(MAX_GROUPS - 1);
        if gi == MAX_GROUPS {
            out.log.push(format!(
                "more than {MAX_GROUPS} groups: merged into the last"
            ));
        }
        // What a group inherits from <groups> for its envelope and velocity.
        let gattr = |k: &str| group.attr(k).or_else(|| groups.attr(k));
        let mut g = Group::default();
        let sec = |k: &str| f(gattr(k)).map(|s| (s * 1000.0).max(0.1));
        g.values[gf::ATTACK] = Some(sec("attack").unwrap_or(0.1));
        g.values[gf::DECAY] = Some(sec("decay").unwrap_or(1.0).max(1.0));
        g.values[gf::SUSTAIN] = Some(
            f(gattr("sustain"))
                .map(|s| s * 100.0)
                .unwrap_or(100.0)
                .clamp(0.0, 100.0),
        );
        g.values[gf::RELEASE] = Some(sec("release").unwrap_or(100.0).max(1.0));
        if let Some(v) = f(gattr("ampVelTrack")) {
            g.values[gf::VEL] = Some((v * 100.0).clamp(-100.0, 100.0));
        }
        g.name = group.attr("name").map(str::to_string);
        out.groups.push((m, g));
        let one_shot = truthy(gattr("ampEnvEnabled")) == Some(false);
        let seq_mode = gattr("seqMode").unwrap_or("always").to_ascii_lowercase();
        let samples: Vec<&Element> = group.children_named("sample").collect();
        let seq_len = f(gattr("seqLength"))
            .map(|v| v as usize)
            .unwrap_or_else(|| {
                samples
                    .iter()
                    .filter_map(|s| f(s.attr("seqPosition")))
                    .fold(0.0, f32::max) as usize
            });
        for s in samples {
            let a = |k: &str| {
                s.attr(k)
                    .or_else(|| group.attr(k))
                    .or_else(|| groups.attr(k))
            };
            let Some(path) = s.attr("path") else {
                out.log.push("a sample without a path was skipped".into());
                continue;
            };
            let full = base.join(path.replace('\\', "/"));
            let full = full.to_string_lossy().replace('\\', "/");
            let fm = meta(&full);
            let mut z = Zone::new(Some(full), 0, 127, 60);
            let root = a("rootNote").and_then(parse_note).unwrap_or(60);
            let lo = a("loNote").and_then(parse_note).unwrap_or(root);
            let hi = a("hiNote").and_then(parse_note).unwrap_or(root);
            z.set(zf::ROOT, root as f32)
                .set(zf::LO, lo as f32)
                .set(zf::HI, hi as f32);
            z.set(zf::VLO, f(a("loVel")).unwrap_or(1.0))
                .set(zf::VHI, f(a("hiVel")).unwrap_or(127.0));
            z.set(zf::GROUP, m as f32);
            // Volume multiplies (adds in dB), pan and tuning add, across the levels.
            let vol = [
                groups.attr("volume").or(groups.attr("globalVolume")),
                group.attr("volume").or(group.attr("groupVolume")),
                s.attr("volume"),
            ]
            .iter()
            .filter_map(|v| v.and_then(volume_db))
            .sum::<f32>();
            let pan = [
                groups.attr("globalPan"),
                group.attr("groupPan"),
                s.attr("pan"),
            ]
            .iter()
            .filter_map(|v| f(*v))
            .sum::<f32>();
            let tune = [
                groups.attr("globalTuning"),
                group.attr("groupTuning"),
                s.attr("tuning"),
            ]
            .iter()
            .filter_map(|v| f(*v))
            .sum::<f32>();
            z.set(zf::GAIN, vol)
                .set(zf::PAN, pan)
                .set(zf::TUNE, tune * 100.0);
            if let Some(v) = f(a("start")) {
                z.set(zf::START, v);
            }
            if let Some(v) = f(a("end")).filter(|v| *v > 0.0) {
                z.set(zf::END, v + 1.0);
            }
            if let Some(t) = f(a("pitchKeyTrack")) {
                z.set(zf::KEYTRACK, t * 100.0);
            }
            let file_loop = fm.as_ref().and_then(|m| m.sustain_loop);
            let loop_on = truthy(a("loopEnabled")).unwrap_or(file_loop.is_some());
            if loop_on {
                let (fs, fe) = file_loop
                    .map(|(s, e, _)| (s as f32, e as f32))
                    .unwrap_or((0.0, 0.0));
                z.set(zf::MODE, 2.0)
                    .set(zf::LS, f(a("loopStart")).unwrap_or(fs))
                    .set(zf::LE, f(a("loopEnd")).unwrap_or(fe));
                if let Some(frames) = f(a("loopCrossfade")) {
                    let rate = fm
                        .as_ref()
                        .map(|m| m.rate)
                        .filter(|r| *r > 0)
                        .unwrap_or(44_100);
                    z.set(zf::XF, frames / rate as f32 * 1000.0);
                }
                if a("loopCrossfadeMode").is_some_and(|m| m.eq_ignore_ascii_case("equal_power")) {
                    z.set(zf::LXC, 1.0);
                }
            } else if one_shot {
                z.set(zf::MODE, 1.0);
            }
            let trig = match a("trigger").map(str::to_ascii_lowercase).as_deref() {
                Some("release") => 1.0,
                Some("first") => 2.0,
                Some("legato") => 3.0,
                _ => 0.0,
            };
            z.set(zf::TRIG, trig);
            if let Some(d) = f(a("releaseTriggerDecay")) {
                // Decent gives the decay per second as a linear factor; Exemplar wants dB/s.
                z.set(
                    zf::RTDECAY,
                    if d > 0.0 && d < 1.0 {
                        -20.0 * d.log10()
                    } else {
                        d
                    },
                );
            }
            let pos = f(s.attr("seqPosition")).unwrap_or(1.0);
            match seq_mode.as_str() {
                "round_robin" if seq_len > 1 => {
                    z.set(zf::SEQ, pos).set(zf::SEQLEN, seq_len as f32);
                }
                "random" | "true_random" if seq_len > 1 => {
                    let n = seq_len as f32;
                    z.set(zf::RLO, (pos - 1.0) / n).set(zf::RHI, pos / n);
                }
                _ => {}
            }
            if let Some(t) = a("tags") {
                z.set(zf::EXCL, class(t, &mut tags));
            }
            if let Some(t) = a("silencedByTags") {
                z.set(zf::OFFBY, class(t, &mut tags));
            }
            if a("silencingMode").is_some_and(|m| m.eq_ignore_ascii_case("normal")) {
                z.set(zf::OFFMODE, 1.0);
            }
            out.zones.push(z);
        }
    }
    if let Some(t) = f(groups.attr("glideTime")) {
        out.values
            .push(("glide".into(), (t * 1000.0).clamp(0.0, 5_000.0)));
    }
    for skipped in [
        "ui",
        "effects",
        "modulators",
        "midi",
        "noteSequences",
        "buses",
    ] {
        if root.child(skipped).is_some_and(|e| !e.children.is_empty()) {
            out.log.push(format!("<{skipped}> not carried over"));
        }
    }
    out.clamp();
    Ok(out)
}
