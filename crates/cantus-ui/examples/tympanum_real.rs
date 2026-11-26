//! Tympanum against the drums library (round 42, `tympanum-real`): any kit — a modelled one
//! or a recorded pack read by the app's own loader — played the same way, through the
//! engine's offline render, for `tools/tympanum-real`'s measures.
//!
//! ```text
//! cargo run --release -p cantus-ui --example tympanum_real -- info <content root> <kit> [<kit> …]
//! cargo run --release -p cantus-ui --example tympanum_real -- hits <content root> <out.wav> <kit>
//!     [--vels 20,40,…] [--takes 3] [--slot 5] [--slot-cym 6] [--kinds kick,snare,…] [--set k=v,…]
//! cargo run --release -p cantus-ui --example tympanum_real -- notes <content root> <out.wav> <kit>
//!     <notes.csv> [--set k=v,…]
//! ```
//!
//! A kit is `model:<id>` or `pack:<pack>/<kit>` (a pack under `<content root>` with a receipt).
//! `dg-build <root> <drumkit.xml> <out dir>` builds a DrumGizmo kit into an SFZ multi-mic kit
//! as the app's install does (`cantus_content::user_pack::dg`). `info` prints each kit's pieces, articulations (key, openness) and mics as JSON lines.
//! `hits` plays every articulation alone at each velocity, `takes` times (consecutive hits
//! draw different takes), each in a `slot`-second window, and writes a 32-bit float stereo
//! WAV and beside it `<out>.csv` (one row per hit: its start frame, piece, kind, articulation,
//! key, openness, velocity, take). `notes` plays a CSV of `seconds,key,velocity[,cc4]` (a
//! transcription, a MIDI file read by `tools/tympanum-real/midi.py`). `--set` sets the kit's
//! numeric keys (`seed=3,p2.pos=0.5`; `voice=hybrid` sets the text key).

use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::inst::tympanum::{KitRef, KIT_KEY};
use cantus_core::inst::tympanum_kit::{modelled_kit, KitDef};
use cantus_core::*;
use cantus_engine::{render, EngineHandle, RenderOptions};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const RATE: f64 = 48_000.0;
static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
const BPM: f64 = 60.0;

fn kit_def(kit: &str) -> KitDef {
    let r = KitRef::parse(kit).unwrap_or_else(|| panic!("not a kit: {kit}"));
    if let Some(id) = kit.strip_prefix("model:") {
        return modelled_kit(id).unwrap_or_else(|| panic!("no modelled kit {id}"));
    }
    let rec = cantus_engine::inst::tympanum::cache::recorded(&r).unwrap_or_else(|| {
        let why = ROOT
            .get()
            .map(|root| cantus_ui::tympanum_load::read_pack_kit(root, &r).err())
            .unwrap_or_default();
        panic!("{kit}: not readable ({why:?})")
    });
    for l in &rec.log {
        eprintln!("{kit}: {l}");
    }
    rec.def.clone()
}

fn spec_for(kit: &str, sets: &str) -> NativeSpec {
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some(kit.to_string())).unwrap();
    for kv in sets.split(',').filter(|s| !s.is_empty()) {
        let (k, v) = kv.split_once('=').expect("k=v");
        match v.trim().parse::<f32>() {
            Ok(x) => {
                spec.set(k.trim(), x);
            }
            Err(_) => {
                spec.set_text(k.trim(), Some(v.trim().to_string())).unwrap();
            }
        }
    }
    spec
}

fn ticks(sec: f64) -> Ticks {
    Ticks((sec * BPM / 60.0 * PPQ as f64).round() as i64)
}

/// One note: seconds, key, velocity, the hat's openness (CC4, 0 … 1) at the stroke.
struct Hit {
    at: f64,
    key: u8,
    vel: u8,
    cc4: Option<f32>,
}

fn render_notes(kit: &str, sets: &str, notes: &[Hit], tail: f64) -> Vec<f32> {
    let mut p = Project::new("tympanum-real");
    p.tempo_bpm = BPM;
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Native(spec_for(kit, sets));
    let end = notes.iter().map(|n| n.at).fold(0.0, f64::max) + tail;
    let mut c = p.make_clip("Hits", Ticks::ZERO, ticks(end));
    let mut cc4 = Vec::new();
    for n in notes {
        let at = ticks(n.at);
        if let Some(o) = n.cc4 {
            // the pedal set a little before the stroke (ADR 0124: the openness at the hit)
            cc4.push(automation::AutoPoint::new(
                Ticks((at.0 - PPQ / 16).max(0)),
                o,
            ));
        }
        c.notes.push(p.make_note(at, Ticks(PPQ / 8), n.key, n.vel));
    }
    if !cc4.is_empty() {
        c.lanes.push(ClipLane::new(LaneKind::Cc(4), cc4));
    }
    t.clips.push(c);
    p.tracks.push(t);
    let (mut engine, _proc) = EngineHandle::manual(RATE);
    let snap = engine.compile_for_render(&p, 1);
    let r = render(
        snap,
        &RenderOptions {
            tail_seconds: tail,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("render");
    r.samples
}

fn write_wav(path: &Path, stereo: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("wav");
    for x in stereo {
        w.write_sample(*x).unwrap();
    }
    w.finalize().unwrap();
}

fn arg<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn kind_name(k: cantus_core::inst::tympanum_kit::PieceKind) -> String {
    format!("{k:?}").to_lowercase()
}

fn info(kits: &[String]) {
    for kit in kits {
        let d = kit_def(kit);
        let mut s = format!("{{\"kit\":\"{kit}\",\"name\":{:?},\"pieces\":[", d.name);
        for (i, p) in d.pieces.iter().enumerate() {
            let arts: Vec<String> = p
                .articulations
                .iter()
                .map(|a| {
                    format!(
                        "{{\"id\":{:?},\"key\":{},\"open\":{}}}",
                        a.id,
                        a.key,
                        a.openness.map(|o| o.at).unwrap_or(-1.0)
                    )
                })
                .collect();
            let _ = write!(
                s,
                "{}{{\"index\":{},\"name\":{:?},\"kind\":\"{}\",\"diameter\":{},\"arts\":[{}]}}",
                if i > 0 { "," } else { "" },
                p.index,
                p.name,
                kind_name(p.kind),
                p.diameter,
                arts.join(",")
            );
        }
        let mics: Vec<String> = d.mics.iter().map(|m| format!("{:?}", m.name)).collect();
        let _ = write!(s, "],\"mics\":[{}]}}", mics.join(","));
        println!("{s}");
    }
}

fn hits(out: &Path, kit: &str, args: &[String]) {
    let vels: Vec<u8> = arg(args, "--vels")
        .unwrap_or("16,32,48,64,80,96,112,127")
        .split(',')
        .map(|v| v.parse().unwrap())
        .collect();
    let takes: usize = arg(args, "--takes").map_or(3, |t| t.parse().unwrap());
    let slot: f64 = arg(args, "--slot").map_or(5.0, |t| t.parse().unwrap());
    let kinds: Option<Vec<String>> =
        arg(args, "--kinds").map(|k| k.split(',').map(str::to_string).collect());
    let slot_cym: f64 = arg(args, "--slot-cym").map_or(6.0, |t| t.parse().unwrap());
    let sets = arg(args, "--set").unwrap_or("");
    let d = kit_def(kit);
    let mut notes = Vec::new();
    let mut next = 0.5;
    let mut csv = String::from("start,piece,kind,art,key,open,vel,take\n");
    // takes vary fastest so each velocity's takes sit side by side
    for p in &d.pieces {
        let kind = kind_name(p.kind);
        if kinds.as_ref().is_some_and(|k| !k.contains(&kind)) {
            continue;
        }
        // cymbals ring past a drum's window: theirs is `--slot-cym` (6 s)
        let cymbal = matches!(
            kind.as_str(),
            "crash" | "ride" | "splash" | "china" | "stack"
        );
        let len = if cymbal { slot_cym } else { slot };
        for a in &p.articulations {
            for &v in &vels {
                for take in 0..takes {
                    let at = next;
                    next += len;
                    // a closed hat as a song without a pedal plays it (CC4 at 0): a kit's
                    // nearly-closed articulation (Tympanum's 0.1) is closed here
                    let open = a.openness.map(|o| o.at).map(|o| if o <= 0.15 { 0.0 } else { o });
                    let _ = writeln!(
                        csv,
                        "{},{:?},{},{},{},{},{},{}",
                        (at * RATE).round() as i64,
                        p.name.replace(',', " "),
                        kind,
                        a.id,
                        a.key,
                        open.unwrap_or(-1.0),
                        v,
                        take
                    );
                    notes.push(Hit {
                        at,
                        key: a.key,
                        vel: v,
                        cc4: Some(open.unwrap_or(0.0)),
                    });
                }
            }
        }
    }
    eprintln!("{kit}: {} hits", notes.len());
    let y = render_notes(kit, sets, &notes, slot_cym);
    write_wav(out, &y);
    std::fs::write(out.with_extension("csv"), csv).unwrap();
}

fn notes_csv(out: &Path, kit: &str, file: &Path, args: &[String]) {
    let text = std::fs::read_to_string(file).expect("notes csv");
    let mut notes = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        let Ok(at) = f[0].parse::<f64>() else {
            continue; // a header
        };
        notes.push(Hit {
            at: at + 0.5,
            key: f[1].parse().unwrap(),
            vel: f[2].parse::<u8>().unwrap().clamp(1, 127),
            cc4: f.get(3).and_then(|x| x.parse().ok()),
        });
    }
    let y = render_notes(kit, arg(args, "--set").unwrap_or(""), &notes, 6.0);
    write_wav(out, &y);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let root = PathBuf::from(args.get(2).expect("a content root"));
    ROOT.set(root.clone()).unwrap();
    cantus_ui::tympanum_load::install(root);
    match mode {
        "info" => info(&args[3..]),
        // A DrumGizmo kit built for Tympanum as the app's *Install from a file…* builds it.
        "dg-build" => {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let name = cantus_content::user_pack::dg::build(
                Path::new(&args[3]),
                Path::new(&args[4]),
                &cancel,
                &mut |_| {},
            )
            .expect("the DrumGizmo build");
            println!("{name}");
        }
        "hits" => hits(Path::new(&args[3]), &args[4], &args[5..]),
        "notes" => notes_csv(Path::new(&args[3]), &args[4], Path::new(&args[5]), &args[6..]),
        _ => eprintln!("modes: info, hits, notes (see the file's header)"),
    }
}
