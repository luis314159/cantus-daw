//! Tympanum's hybrid voice, its fit's renders and its listening (round 40, `drums-hyb`;
//! ADR 0124's third amendment). `tools/tympanum-fit/refit_hybrid.sh` runs the whole fit.
//!
//! ```text
//! cargo run --release -p cantus-ui --example tympanum_hybrid -- takes <out> <content root> <kit> [hybrid]
//! cargo run --release -p cantus-ui --example tympanum_hybrid -- probe <out> <content root> <model id>
//! cargo run --release -p cantus-ui --example tympanum_hybrid -- listen <out> <content root> <model id> <pack/kit>
//! ```
//!
//! - `takes`: every articulation the residual holds × five velocity layers × four takes
//!   (a take: the next ordinal on its key, and the velocity a few units off so a kit with
//!   one sample per dynamic gives four), alone, through the engine's offline render, mono,
//!   `SLOT` seconds each → `<out>/<kit>-takes.wav` and `<out>/schedule.txt`. A modelled kit
//!   plays generated unless `hybrid` is given.
//! - `probe`: the engine's own source → mix response of each articulation's piece: a
//!   residual of unit impulses played hybrid, less the same render generated →
//!   `<out>/probe-<id>.wav` (one `SLOT` per articulation, the schedule's `ARTS` order).
//! - `listen`: the groove and single pieces, generated · hybrid · recorded, at one loudness.

// The listening groove (its `phrase` is the measuring tool's).
#[allow(dead_code)]
#[path = "tympanum_measure/groove.rs"]
mod groove;

use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::inst::tympanum::voice::{hybrid_pack, HYBRID, RESIDUAL_FILE};
use cantus_core::inst::tympanum::{KIT_KEY, VOICE_KEY};
use cantus_core::*;
use cantus_dsp::tympanum_residual::{model_hash, Art, Layer, Residual, Take};
use cantus_engine::{render, EngineHandle, RenderOptions};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What the residual holds: a name, the key, the hi-hat's openness (CC4, 0 … 1; 255/100 =
/// any for the others).
pub const ARTS: [(&str, u8, f32); 15] = [
    ("kick", 36, -1.0),
    ("snare", 38, -1.0),
    ("rimshot", 40, -1.0),
    ("sidestick", 37, -1.0),
    ("tom-high", 45, -1.0),
    ("tom-mid", 43, -1.0),
    ("tom-floor", 41, -1.0),
    ("hat-closed", 42, 0.0),
    ("hat-tight", 46, 0.1),
    ("hat-half", 46, 0.5),
    ("hat-open", 46, 0.95),
    ("hat-pedal", 44, 0.0),
    ("crash", 49, -1.0),
    ("ride", 51, -1.0),
    ("ride-bell", 53, -1.0),
];
/// The velocity layers and each take's offset from its layer's velocity.
pub const LAYERS: [u8; 5] = [30, 55, 80, 105, 127];
pub const JITTER: [i32; 4] = [0, -3, 3, -6];
/// Seconds per hit.
const SLOT: f64 = 2.5;
const FS: f64 = 48_000.0;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let out = PathBuf::from(args.get(2).expect("an output folder"));
    let root = PathBuf::from(args.get(3).expect("a content root"));
    std::fs::create_dir_all(&out).unwrap();
    cantus_ui::tympanum_load::install(root);
    match mode {
        "takes" => {
            let kit = args.get(4).expect("a kit");
            let hybrid = args.get(5).is_some_and(|a| a == "hybrid");
            takes(&out, kit, hybrid);
        }
        "probe" => probe(&out, args.get(4).expect("a model id")),
        "listen" => listen(
            &out,
            args.get(4).expect("a model id"),
            args.get(5).expect("pack/kit"),
        ),
        _ => eprintln!("usage: tympanum_hybrid takes|probe|listen <out> <content root> …"),
    }
}

/// One hit of the schedule: the articulation (index in `ARTS`), its velocity.
type Hit = (usize, u8);

/// A project playing `hits` one per slot (60 / SLOT BPM: a beat a slot).
fn project(kit: &str, hybrid: bool, hits: &[Hit]) -> Project {
    let mut p = Project::new("Tympanum hybrid");
    p.tempo_bpm = 60.0 / SLOT;
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some(kit.to_string())).unwrap();
    if hybrid {
        spec.set_text(VOICE_KEY, Some(HYBRID.to_string())).unwrap();
    }
    t.instrument = InstrumentSpec::Native(spec);
    let n = hits.len() as i64;
    let mut c = p.make_clip("Hits", Ticks::ZERO, Ticks(n * PPQ));
    let mut cc4 = Vec::new();
    for (i, &(a, vel)) in hits.iter().enumerate() {
        let (_, key, open) = ARTS[a];
        let at = Ticks(i as i64 * PPQ);
        let cc = open.max(0.0);
        cc4.push(automation::AutoPoint::new(
            Ticks((at.0 - PPQ / 4).max(0)),
            cc,
        ));
        cc4.push(automation::AutoPoint::new(Ticks(at.0 + PPQ * 3 / 4), cc));
        c.notes.push(p.make_note(at, Ticks(PPQ / 16), key, vel));
    }
    c.lanes.push(ClipLane::new(LaneKind::Cc(4), cc4));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// The render of `p`, mono (L + R) / 2.
fn render_mono(engine: &mut EngineHandle, p: &Project, n: u64) -> Vec<f32> {
    let snap = engine.compile_for_render(p, n);
    let r = render(
        snap,
        &RenderOptions {
            tail_seconds: 0.5,
            ..Default::default()
        },
    )
    .expect("render");
    r.samples
        .chunks_exact(2)
        .map(|f| 0.5 * (f[0] + f[1]))
        .collect()
}

/// The fit's schedule: art × layer × take.
pub fn schedule() -> Vec<(usize, usize, usize, u8)> {
    let mut s = Vec::new();
    for a in 0..ARTS.len() {
        for (l, &v) in LAYERS.iter().enumerate() {
            for (r, j) in JITTER.iter().enumerate() {
                let vel = (i32::from(v) + j).clamp(1, 127) as u8;
                s.push((a, l, r, vel));
            }
        }
    }
    s
}

fn file_name(kit: &str) -> String {
    kit.replace([':', '/'], "-")
}

fn takes(out: &Path, kit: &str, hybrid: bool) {
    let (mut engine, _proc) = EngineHandle::manual(FS);
    let sched = schedule();
    let hits: Vec<Hit> = sched.iter().map(|&(a, _, _, v)| (a, v)).collect();
    let p = project(kit, hybrid, &hits);
    let x = render_mono(&mut engine, &p, 1);
    let mut name = file_name(kit);
    if hybrid {
        name += "-hybrid";
    }
    write_wav(&out.join(format!("{name}-takes.wav")), &x);
    let mut s = format!("# slot {SLOT} s, rate {FS}; art key open layer layer_vel take vel\n");
    for (i, &(a, l, r, v)) in sched.iter().enumerate() {
        let (n, key, open) = ARTS[a];
        let _ = writeln!(s, "{i} {n} {key} {open} {l} {} {r} {v}", LAYERS[l]);
    }
    std::fs::write(out.join("schedule.txt"), s).unwrap();
    println!("{name}: {} hits, {:.0} s", sched.len(), x.len() as f64 / FS);
}

/// A residual of unit impulses, one per articulation at velocity 127.
fn impulses() -> Residual {
    let arts = ARTS
        .iter()
        .enumerate()
        .map(|(i, &(_, key, open))| Art {
            key,
            open: if open < 0.0 {
                255
            } else {
                (open * 100.0).round() as u8
            },
            layers: vec![Layer {
                vel: 127,
                rho: 0.0,
                takes: vec![Take {
                    start: i as u32,
                    len: 1,
                    scale: 1.0 / 32767.0,
                }],
            }],
        })
        .collect();
    let mut r = Residual {
        rate: FS as u32,
        model: model_hash(),
        arts,
        data: vec![32767; ARTS.len()],
        soft: Vec::new(),
    };
    r.finish();
    r
}

fn probe(out: &Path, id: &str) {
    let pack = hybrid_pack(id).expect("a kit with a hybrid");
    let bytes = impulses().encode();
    // The probe in place of the pack (this process only).
    cantus_engine::inst::tympanum_hybrid::set_source(Arc::new(move |p: &str, f: &str| {
        if p == pack && f == RESIDUAL_FILE {
            Ok(bytes.clone())
        } else {
            Err("not the probe".into())
        }
    }));
    let (mut engine, _proc) = EngineHandle::manual(FS);
    let hits: Vec<Hit> = (0..ARTS.len()).map(|a| (a, 127)).collect();
    let kit = format!("model:{id}");
    let on = render_mono(&mut engine, &project(&kit, true, &hits), 1);
    let off = render_mono(&mut engine, &project(&kit, false, &hits), 2);
    let h: Vec<f32> = on.iter().zip(&off).map(|(a, b)| a - b).collect();
    let e: f64 = h.iter().map(|x| f64::from(*x) * f64::from(*x)).sum();
    assert!(e > 0.0, "the probe was not heard: is the hybrid hooked up?");
    write_wav(&out.join(format!("probe-{id}.wav")), &h);
    println!("probe-{id}: energy {e:.3e}");
}

/// The groove and single pieces three ways at one loudness (−24 dBFS RMS for the groove;
/// each piece at its recorded take's loudness).
fn listen(out: &Path, id: &str, pack_kit: &str) {
    let (mut engine, _proc) = EngineHandle::manual(FS);
    let model = format!("model:{id}");
    let rec = format!("pack:{pack_kit}");
    let ways: [(&str, &str, bool); 3] = [
        ("1-generado", &model, false),
        ("2-hibrido", &model, true),
        ("3-grabacion", &rec, false),
    ];
    let mut n = 1;
    for (name, kit, hybrid) in ways {
        let mut p = Project::new("Tympanum");
        let mut p = groove::groove(&mut p, kit, None);
        if hybrid {
            if let Some(InstrumentSpec::Native(s)) = p.tracks.first_mut().map(|t| &mut t.instrument)
            {
                s.set_text(VOICE_KEY, Some(HYBRID.to_string())).unwrap();
            }
        }
        let snap = engine.compile_for_render(&p, n);
        n += 1;
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
        let g = 10f32.powf((-24.0 - rms) / 20.0);
        s.iter_mut().for_each(|x| *x *= g);
        groove::write_flac(&out.join(format!("groove-{name}.flac")), &s);
        println!("groove {name}: {:+.1} dB", 20.0 * g.log10());
    }
    // Single pieces: kick, snare, a tom, the closed and open hat, crash, ride — each at
    // 50, 90 and 127, a second apart, three ways.
    let pieces = [0usize, 1, 5, 7, 10, 12, 13];
    for &a in &pieces {
        let hits: Vec<Hit> = [50u8, 90, 127].iter().map(|&v| (a, v)).collect();
        let mut renders = Vec::new();
        for (_, kit, hybrid) in ways {
            let x = render_mono(&mut engine, &project(kit, hybrid, &hits), n);
            n += 1;
            renders.push(x);
        }
        let rms = |x: &[f32]| {
            (x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
        };
        let target = rms(&renders[2]).max(1e-9);
        for ((name, _, _), mut x) in ways.iter().zip(renders) {
            let g = (target / rms(&x).max(1e-12)) as f32 * 0.5;
            x.iter_mut().for_each(|v| *v *= g);
            let stereo: Vec<f32> = x.iter().flat_map(|v| [*v, *v]).collect();
            groove::write_flac(
                &out.join(format!("pieza-{}-{name}.flac", ARTS[a].0)),
                &stereo,
            );
        }
    }
}

/// A mono 32-bit float WAV at 48 kHz.
fn write_wav(path: &Path, x: &[f32]) {
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
