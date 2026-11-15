//! A DrumGizmo kit built for **Tympanum** (round 40 phase 3, ADR 0124 §3, `Build::
//! multichannel`): every microphone kept. Each hit's multichannel file is written as one
//! 24-bit mono file per microphone, at the kit's rate (`<folder>/<mic>/<instrument>/
//! NN.wav`), and an SFZ multi-mic kit names them: a `<master>` per mic (its starting level,
//! pan and polarity from the manifest's `mics`), the velocity layers and round robins the
//! stereo build makes (each hit measured on the manifest's mix), the chokes as SFZ groups,
//! and a hi-hat's states laid on key 46 by their openness on CC4 (`KeyMap::openness`), so
//! Tympanum crossfades them continuously.
//!
//! **Why one file per mic, not one multichannel stream per hit.** A mic turned off then
//! loads nothing at all (ADR 0123 §2's `o{n}.on`), and every file is mono or stereo — what
//! ADR 0050's streaming reads — so a 13-mic kit streams as the Salamander does. The memory
//! is the same (a stream's head per channel either way); only the number of files grows.

use super::{gains, layers, level_db, mix, runs, Built, Hit, Reader};
use crate::drumgizmo::{self, Kit};
use crate::manifest::Build;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// A key range (lo, hi) and, for a hi-hat state, its CC4 range.
type KeyRange = (u8, u8, Option<(u8, u8)>);

/// A folder name from a mic's channel name (`OH L` → `OH_L`).
fn safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// One microphone of the build: its folder and its kit channels (one, or a left/right
/// pair heard as one stereo strip: `AmbL`/`AmbR` → `Amb`, ADR 0123 §4).
#[derive(Clone, Debug, PartialEq)]
pub struct MicOut {
    pub name: String,
    pub channels: Vec<String>,
}

/// The kit's channels as microphones: each `…L` with its `…R` one stereo mic.
pub fn mic_outs(channels: &[String]) -> Vec<MicOut> {
    let mut out: Vec<MicOut> = Vec::new();
    let mut used = vec![false; channels.len()];
    for (i, c) in channels.iter().enumerate() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let stem = c.strip_suffix('L').or_else(|| c.strip_suffix('l'));
        let partner = stem.and_then(|st| {
            channels.iter().enumerate().find(|(j, x)| {
                !used[*j] && (x.as_str() == format!("{st}R") || x.as_str() == format!("{st}r"))
            })
        });
        match (stem, partner) {
            (Some(st), Some((j, r))) => {
                used[j] = true;
                let name = st.trim_end_matches(['_', ' ', '-']).to_string();
                out.push(MicOut {
                    name: if name.is_empty() { c.clone() } else { name },
                    channels: vec![c.clone(), r.clone()],
                });
            }
            _ => out.push(MicOut {
                name: c.clone(),
                channels: vec![c.clone()],
            }),
        }
    }
    out
}

/// The microphones a build keeps: the kit's channels the manifest's `mics` list (all, when
/// it lists none — a trigger channel left out), a `…L`/`…R` pair one stereo mic only where
/// the manifest pans them apart (two kicks' mics `KdrumL`/`KdrumR` stay two mics).
pub fn mic_outs_for(b: &Build, channels: &[String]) -> Vec<MicOut> {
    if b.mics.is_empty() {
        return mic_outs(channels);
    }
    let pan = |c: &str| b.mics.iter().find(|m| m.channel == c).map(|m| m.pan);
    let kept: Vec<String> = channels
        .iter()
        .filter(|c| pan(c).is_some())
        .cloned()
        .collect();
    let mut out = Vec::new();
    for mo in mic_outs(&kept) {
        let apart = mo.channels.len() == 2
            && match (pan(&mo.channels[0]), pan(&mo.channels[1])) {
                (Some(l), Some(r)) => l <= -50.0 && r >= 50.0,
                _ => false,
            };
        if mo.channels.len() == 2 && !apart {
            for c in mo.channels {
                out.push(MicOut {
                    name: c.clone(),
                    channels: vec![c],
                });
            }
        } else {
            out.push(mo);
        }
    }
    out
}

/// The sidecar a build writes beside its SFZ (`<sfz>.mics.json`): per mic folder, the keys
/// of the instruments it is the main mic of (DrumGizmo's `main`), which the SFZ cannot say.
pub fn sidecar_path(sfz: &str) -> String {
    format!("{sfz}.mics.json")
}

/// Build the kit in `src` for Tympanum into `out`.
pub fn render_mics(
    b: &Build,
    src: &Path,
    out: &Path,
    credit: &str,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(u64, u64),
) -> Result<Built, String> {
    let kit: Kit = drumgizmo::read_kit(src, &b.kit)?;
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
    report(0, total);
    let out_root = drumgizmo::path_of(out, &b.folder);
    let outs = mic_outs_for(b, &kit.channels);
    let mut built = Built::default();
    let mut reader = Reader {
        path: String::new(),
        audio: None,
    };
    let mut done = 0u64;
    // Per instrument: its regions (files relative to a mic's folder).
    let mut parts: Vec<(usize, Vec<super::Region>)> = Vec::new();
    for (pi, (k, inst)) in plan.iter().enumerate() {
        crate::manifest::check_path(&inst.name).map_err(|e| format!("{}: {e}", inst.name))?;
        let mix_gains = gains(b, &inst.name, k.gain_db);
        let mut hits = Vec::new();
        for (i, s) in inst.samples.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            // Measured on the manifest's mix, as the stereo build measures it.
            let (stereo, rate) = mix(src, s, &mix_gains, &mut reader)?;
            let name = format!("{}/{:02}.wav", inst.name, i + 1);
            for mo in &outs {
                let mut cols: Vec<Vec<f32>> = Vec::new();
                let mut rate = 0;
                for ch in &mo.channels {
                    let Some(a) = s.audio.iter().find(|a| a.channel == *ch) else {
                        continue;
                    };
                    let audio = reader.get(src, &a.file)?;
                    if a.file_channel >= audio.channels {
                        return Err(format!("{}: no channel {}", a.file, a.file_channel + 1));
                    }
                    rate = audio.rate;
                    cols.push(
                        audio
                            .samples
                            .chunks_exact(audio.channels)
                            .map(|f| f[a.file_channel])
                            .collect(),
                    );
                }
                if cols.is_empty() {
                    continue;
                }
                let frames = cols.iter().map(Vec::len).max().unwrap_or(0);
                let ch = cols.len();
                let mut inter = vec![0.0f32; frames * ch];
                for (c, col) in cols.iter().enumerate() {
                    for (i, x) in col.iter().enumerate() {
                        inter[i * ch + c] = *x;
                    }
                }
                let path = out_root.join(safe(&mo.name)).join(&name);
                if let Some(d) = path.parent() {
                    std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
                }
                built.clamped += crate::wav::write_24(&path, rate, ch as u16, &inter)?;
                built.peak = inter.iter().fold(built.peak, |p, x| p.max(x.abs()));
                built.files += 1;
            }
            hits.push(Hit {
                file: name,
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
        for h in &hits {
            if !regions.iter().any(|r| r.file == h.file) {
                for mo in &outs {
                    let _ = std::fs::remove_file(out_root.join(safe(&mo.name)).join(&h.file));
                    built.files = built.files.saturating_sub(1);
                }
            }
        }
        built.regions += regions.len() * outs.len();
        parts.push((pi, regions));
    }
    drop(reader);
    // The hi-hat's states on key 46, by openness: each its range of CC4.
    let mut hats: Vec<(usize, f32)> = plan
        .iter()
        .enumerate()
        .filter_map(|(i, (k, _))| k.openness.map(|o| (i, o.clamp(0.0, 1.0))))
        .collect();
    hats.sort_by(|a, b| a.1.total_cmp(&b.1));
    let cc_range = |i: usize| -> Option<(u8, u8)> {
        let at = hats.iter().position(|h| h.0 == i)?;
        let lo = if at == 0 {
            0.0
        } else {
            (hats[at - 1].1 + hats[at].1) / 2.0 * 127.0 + 0.5
        };
        let hi = if at + 1 == hats.len() {
            127.0
        } else {
            (hats[at].1 + hats[at + 1].1) / 2.0 * 127.0 - 0.5
        };
        Some((
            lo.ceil().clamp(0.0, 127.0) as u8,
            hi.floor().clamp(0.0, 127.0) as u8,
        ))
    };
    use std::fmt::Write;
    let mut s = String::new();
    for line in credit.lines() {
        let _ = writeln!(s, "// {line}");
    }
    let _ = writeln!(
        s,
        "// Every microphone of the kit, one folder each (Cantus's Tympanum build)."
    );
    let _ = writeln!(s, "<control>\ndefault_path={}/", b.folder);
    let _ = writeln!(
        s,
        "<global>\nloop_mode=one_shot amp_veltrack=100 pitch_keytrack=0"
    );
    let mut sidecar = serde_json::Map::new();
    for mo in &outs {
        let ch = &mo.channels[0];
        let m = b.mics.iter().find(|m| m.channel == *ch);
        let gain = m.map_or(0.0, |m| m.gain_db);
        // A pair's own stereo is its two channels: no pan of its own.
        let pan = if mo.channels.len() == 2 {
            0.0
        } else {
            m.map_or(0.0, |m| m.pan)
        };
        let mut main_keys: Vec<u8> = Vec::new();
        for (k, inst) in &plan {
            if inst.main.iter().any(|c| mo.channels.contains(c)) {
                main_keys.extend(&k.keys);
                if k.openness.is_some() {
                    main_keys.push(46);
                }
            }
        }
        main_keys.sort_unstable();
        main_keys.dedup();
        sidecar.insert(safe(&mo.name), serde_json::json!({ "main": main_keys }));
        let amp = if m.is_some_and(|m| m.invert) {
            -100
        } else {
            100
        };
        let _ = writeln!(s, "<master> volume={gain:.2} pan={pan:.1} amplitude={amp}");
        for (pi, regions) in &parts {
            let (k, inst) = plan[*pi];
            let choke = b
                .chokes
                .iter()
                .position(|c| c.instruments.contains(&inst.name))
                .map(|c| (c + 1, b.chokes[c].release_ms));
            // A hat state laid on key 46 by its openness is there once, on its CC4 range.
            let own: Vec<u8> = k
                .keys
                .iter()
                .copied()
                .filter(|key| !(k.openness.is_some() && *key == 46))
                .collect();
            let mut ranges: Vec<KeyRange> =
                runs(&own).into_iter().map(|(a, b)| (a, b, None)).collect();
            if let Some(r) = cc_range(*pi) {
                ranges.push((46, 46, Some(r)));
            }
            for (lo, hi, cc) in ranges {
                let mut g = format!("<group> lokey={lo} hikey={hi}");
                if let Some((a, b)) = cc {
                    let _ = write!(g, " locc4={a} hicc4={b}");
                }
                if let Some((c, rel)) = choke {
                    let _ = write!(
                        g,
                        " group={c} off_by={c} off_mode=normal ampeg_release={:.3}",
                        rel / 1000.0
                    );
                }
                let _ = writeln!(s, "{g}");
                for r in regions {
                    let _ = writeln!(
                        s,
                        "<region> lovel={} hivel={} seq_length={} seq_position={} volume={:.2} sample={}/{}",
                        r.lovel,
                        r.hivel,
                        r.seqlen,
                        r.seq,
                        r.volume,
                        safe(&mo.name),
                        r.file
                    );
                }
            }
        }
    }
    std::fs::write(drumgizmo::path_of(out, &b.sfz), s).map_err(|e| e.to_string())?;
    let side = serde_json::to_string_pretty(&serde_json::Value::Object(sidecar))
        .map_err(|e| e.to_string())?;
    std::fs::write(drumgizmo::path_of(out, &sidecar_path(&b.sfz)), side)
        .map_err(|e| e.to_string())?;
    if let Some(l) = &kit.licence {
        built.log.push(format!("the kit's own licence: {l}"));
    }
    Ok(built)
}
