//! Every articulation alone at four velocities through a kit, measured; and the A/B groove.

use super::analyse::{measure, text, Hit};
use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::inst::tympanum::KIT_KEY;
use cantus_core::*;
use cantus_engine::{render, EngineHandle, RenderOptions};
use std::fmt::Write as _;
use std::path::Path;

use super::groove;

/// What is played: a name, the key, the hi-hat's CC4 (0 … 1).
pub const ARTS: [(&str, u8, f32); 13] = [
    ("kick", 36, 0.0),
    ("snare", 38, 0.0),
    ("rimshot", 40, 0.0),
    ("sidestick", 37, 0.0),
    ("tom-high", 45, 0.0),
    ("tom-mid", 43, 0.0),
    ("tom-floor", 41, 0.0),
    ("hat-closed", 42, 0.0),
    ("hat-open", 46, 0.95),
    ("hat-pedal", 44, 0.0),
    ("crash", 49, 0.0),
    ("ride", 51, 0.0),
    ("ride-bell", 53, 0.0),
];
pub const VELS: [u8; 4] = [40, 80, 110, 127];
/// Seconds per hit.
const SLOT: f64 = 4.0;

fn hits_project(kit: &str) -> Project {
    let mut p = Project::new("Tympanum hits");
    p.tempo_bpm = 60.0;
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some(kit.to_string())).unwrap();
    // `TYMP_VOICE=hybrid`: a modelled kit with its recorded residual (r40:drums-hyb).
    if let Ok(v) = std::env::var("TYMP_VOICE") {
        spec.set_text(cantus_core::inst::tympanum::VOICE_KEY, Some(v))
            .unwrap();
    }
    groove::env_spec(&mut spec);
    t.instrument = InstrumentSpec::Native(spec);
    let n = (ARTS.len() * VELS.len()) as i64;
    let mut c = p.make_clip("Hits", Ticks::ZERO, Ticks(n * 4 * PPQ));
    let mut cc4 = Vec::new();
    let mut i = 0i64;
    for (_, key, cc) in ARTS {
        // `TYMP_HAT_OPEN=0.5` plays the open hat half open (the measuring tool's states).
        let cc = match std::env::var("TYMP_HAT_OPEN")
            .ok()
            .and_then(|s| s.parse().ok())
        {
            Some(o) if key == 46 => o,
            _ => cc,
        };
        for v in VELS {
            let at = Ticks(i * 4 * PPQ);
            // (the pedal moves 125 ms before the stroke: a second before, it closed on the
            // open hat's v127 ring at 3 s — a choke every fit took for its tail, the
            // owner's "iuuu")
            cc4.push(automation::AutoPoint::new(
                Ticks((at.0 - PPQ / 8).max(0)),
                cc,
            ));
            cc4.push(automation::AutoPoint::new(
                Ticks(at.0 + PPQ * 3 + PPQ * 3 / 4),
                cc,
            ));
            c.notes.push(p.make_note(at, Ticks(PPQ / 8), key, v));
            i += 1;
        }
    }
    c.lanes.push(ClipLane::new(LaneKind::Cc(4), cc4));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// Every hit of `kit`, mono, each `SLOT` seconds.
fn render_hits(engine: &mut EngineHandle, kit: &str, n: u64) -> Vec<Vec<f64>> {
    let p = hits_project(kit);
    let snap = engine.compile_for_render(&p, n);
    let r = render(
        snap,
        &RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
    )
    .expect("render");
    let slot = (SLOT * 48_000.0) as usize;
    (0..ARTS.len() * VELS.len())
        .map(|i| {
            (i * slot..(i + 1) * slot)
                .map(|f| {
                    let l = r.samples.get(2 * f).copied().unwrap_or(0.0) as f64;
                    let rr = r.samples.get(2 * f + 1).copied().unwrap_or(0.0) as f64;
                    0.5 * (l + rr)
                })
                .collect()
        })
        .collect()
}

fn file_name(kit: &str) -> String {
    kit.replace([':', '/'], "-")
}

pub fn run(out: &Path, kits: &[String]) {
    let (mut engine, _proc) = EngineHandle::manual(48_000.0);
    let mut all: Vec<Vec<Hit>> = Vec::new();
    for (k, kit) in kits.iter().enumerate() {
        let segs = render_hits(&mut engine, kit, k as u64 + 1);
        let hits: Vec<Hit> = segs.iter().map(|s| measure(s)).collect();
        let mut s = format!("# {kit}\n\n");
        // A pack kit: what the reader made of it (its mics, its pieces).
        if let Some(r) = cantus_core::inst::tympanum::KitRef::parse(kit)
            .and_then(|k| cantus_engine::inst::tympanum::cache::recorded(&k))
        {
            for line in &r.log {
                let _ = writeln!(s, "log: {line}");
            }
            let mics: Vec<&str> = r.def.mics.iter().map(|m| m.name.as_str()).collect();
            let _ = writeln!(
                s,
                "mics: {}
",
                mics.join(", ")
            );
        }
        for (a, (name, key, _)) in ARTS.iter().enumerate() {
            for (v, vel) in VELS.iter().enumerate() {
                let h = &hits[a * VELS.len() + v];
                let _ = writeln!(s, "{name} (key {key}) velocity {vel}");
                if h.peak_db == 0.0 && h.rms_db == 0.0 {
                    s += "  silent\n";
                } else {
                    s += &text(h);
                }
            }
            s += "\n";
        }
        std::fs::write(out.join(format!("{}.txt", file_name(kit))), &s).unwrap();
        // The hits as a mono 32-bit float WAV, to listen to and measure (written as it
        // goes: the FLAC encoder's buffers did not fit a machine short of memory).
        let flat: Vec<f32> = segs.iter().flatten().map(|x| *x as f32).collect();
        write_wav(&out.join(format!("{}-hits.wav", file_name(kit))), &flat);
        all.push(hits);
    }
    // Side by side: the summary numbers of every kit, per hit.
    let mut s = String::from("# compare\n\nart vel | ");
    for kit in kits {
        let _ = write!(s, "{kit}: rms att cA cB f0e f0l T20(125,500,2k,8k) | ");
    }
    s += "\n";
    for (a, (name, _, _)) in ARTS.iter().enumerate() {
        for (v, vel) in VELS.iter().enumerate() {
            let _ = write!(s, "{name:<10} {vel:>3} | ");
            for hits in &all {
                let h = &hits[a * VELS.len() + v];
                let _ = write!(
                    s,
                    "{:5.1} {:4.1} {:5.0} {:5.0} {:5.0} {:5.0} {:4.0} {:4.0} {:4.0} {:4.0} | ",
                    h.rms_db,
                    h.attack_ms,
                    h.centroid_attack,
                    h.centroid_body,
                    h.f_early,
                    h.f_late,
                    h.t20_ms[2],
                    h.t20_ms[4],
                    h.t20_ms[6],
                    h.t20_ms[8]
                );
            }
            s += "\n";
        }
    }
    std::fs::write(out.join("compare.txt"), &s).unwrap();
    println!("{s}");
}

/// The groove through the recorded kit and the modelled studio kit, the same loudness.
pub fn ab(out: &Path, kit: &str) {
    let (mut engine, _proc) = EngineHandle::manual(48_000.0);
    let kits = [
        (
            format!("recorded-{}", file_name(kit)),
            format!("pack:{kit}"),
        ),
        // `TYMP_AB_MODEL=jazz`: the A/B against another modelled kit
        {
            let m = std::env::var("TYMP_AB_MODEL").unwrap_or_else(|_| "studio".into());
            (format!("modelled-{m}"), format!("model:{m}"))
        },
    ];
    let mut renders = Vec::new();
    for (i, (_, k)) in kits.iter().enumerate() {
        let mut p = Project::new("Tympanum");
        let p = groove::groove(&mut p, k, None);
        let snap = engine.compile_for_render(&p, i as u64 + 1);
        let r = render(
            snap,
            &RenderOptions {
                tail_seconds: 2.5,
                ..Default::default()
            },
        )
        .expect("render");
        renders.push(r.samples);
    }
    // The same RMS for all, low enough that no peak reaches full scale (−28 dBFS).
    for ((name, _), mut s) in kits.iter().zip(renders) {
        let (_, rms) = groove::stats(&s);
        let g = 10f32.powf((-28.0 - rms) / 20.0);
        s.iter_mut().for_each(|x| *x *= g);
        let (peak, rms) = groove::stats(&s);
        println!(
            "{name}: gain {:+.1} dB, peak {peak:.1}, rms {rms:.1}",
            20.0 * g.log10()
        );
        groove::write_flac(&out.join(format!("{name}.flac")), &s);
    }
}

/// The groove through a modelled kit alone, at the A/B's loudness (−28 dBFS RMS):
/// `<out>/<name>.flac` (`model <out> <root> <studio|jazz> [name]`).
pub fn model_only(out: &Path, model: &str, name: &str) {
    let (mut engine, _proc) = EngineHandle::manual(48_000.0);
    let mut p = Project::new("Tympanum");
    let p = groove::groove(&mut p, &format!("model:{model}"), None);
    let snap = engine.compile_for_render(&p, 1);
    let r = render(
        snap,
        &RenderOptions {
            tail_seconds: 2.5,
            ..Default::default()
        },
    )
    .expect("render");
    let mut s = r.samples;
    let (_, rms) = groove::stats(&s);
    let g = 10f32.powf((-28.0 - rms) / 20.0);
    s.iter_mut().for_each(|x| *x *= g);
    groove::write_flac(&out.join(format!("{name}.flac")), &s);
}

/// The player's phrase through a modelled kit, felt and hard beater, at −28 dBFS RMS.
pub fn phrase(out: &Path, kit: &str) {
    let (mut engine, _proc) = EngineHandle::manual(48_000.0);
    for (i, (name, beater)) in [("felt", 0.0f32), ("plastic", 1.0)].iter().enumerate() {
        let p = groove::phrase(kit, *beater);
        let snap = engine.compile_for_render(&p, i as u64 + 1);
        let r = render(
            snap,
            &RenderOptions {
                tail_seconds: 3.0,
                ..Default::default()
            },
        )
        .expect("render");
        let mut s = r.samples;
        let (_, rms) = groove::stats(&s);
        let g = 10f32.powf((-28.0 - rms) / 20.0);
        s.iter_mut().for_each(|x| *x *= g);
        let (peak, _) = groove::stats(&s);
        println!("phrase-{name}: peak {peak:.1}");
        groove::write_flac(
            &out.join(format!("phrase-{}-{name}.flac", file_name(kit))),
            &s,
        );
    }
}

/// A mono 32-bit float WAV at 48 kHz.
pub fn write_wav(path: &Path, x: &[f32]) {
    use std::io::Write;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).expect("create"));
    let data = (x.len() * 4) as u32;
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&3u16.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&48_000u32.to_le_bytes());
    h.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
    h.extend_from_slice(&4u16.to_le_bytes());
    h.extend_from_slice(&32u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data.to_le_bytes());
    f.write_all(&h).expect("write");
    for v in x {
        f.write_all(&v.to_le_bytes()).expect("write");
    }
}
