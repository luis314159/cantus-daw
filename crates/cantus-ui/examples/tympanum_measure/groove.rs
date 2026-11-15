//! The listening groove and the FLAC writer, shared by `tympanum_groove` and `tympanum_measure`.

use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::inst::tympanum::KIT_KEY;
use cantus_core::*;

use cantus_export::settings::{Depth, Dither, Tags};
use cantus_export::Audio;
use std::path::Path;

pub const BPM: f64 = 100.0;

fn tick(bar: i64, sixteenth: i64) -> Ticks {
    Ticks(bar * 4 * PPQ + sixteenth * PPQ / 4)
}

/// `TYMP_SPEC=key=value,key=value`: keys set on the kit (a mic soloed, a bleed cell off),
/// to measure one part of the kit alone.
pub fn env_spec(spec: &mut NativeSpec) {
    if let Ok(s) = std::env::var("TYMP_SPEC") {
        for kv in s.split(',') {
            if let Some((k, v)) = kv.split_once('=') {
                if let Ok(v) = v.trim().parse::<f32>() {
                    spec.set(k.trim(), v);
                }
            }
        }
    }
}

/// The groove: five bars, 100 BPM.
pub fn groove(p: &mut Project, kit: &str, solo: Option<u16>) -> Project {
    let mut p = std::mem::replace(p, Project::new("Tympanum"));
    p.tempo_bpm = BPM;
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some(kit.to_string())).unwrap();
    // `TYMP_SEED=n`: the kit's seed (the measures read on several draws).
    if let Some(s) = std::env::var("TYMP_SEED")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
    {
        spec.set("seed", s);
    }
    if let Some(m) = solo {
        spec.set(&format!("o{m}.solo"), 1.0);
    }
    env_spec(&mut spec);
    t.instrument = InstrumentSpec::Native(spec);
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks(5 * 4 * PPQ));
    let mut cc4: Vec<automation::AutoPoint> = Vec::new();
    let mut set_cc = |at: Ticks, v: f32| {
        cc4.push(automation::AutoPoint {
            tick: Ticks(at.0.max(1) - 1),
            value: cc4.last().map_or(v, |p: &automation::AutoPoint| p.value),
            tension: 0.0,
            shape: Default::default(),
        });
        cc4.push(automation::AutoPoint::new(at, v));
    };
    let note = |c: &mut Clip, p: &mut Project, at: Ticks, key: u8, vel: u8| {
        c.notes.push(p.make_note(at, Ticks(PPQ / 8), key, vel));
    };
    for bar in 0..4i64 {
        // Kick: one, the and of two, three.
        for (s, v) in [(0, 112), (6, 92), (8, 108), (11, 70)] {
            note(&mut c, &mut p, tick(bar, s), 36, v);
        }
        // Snare on two and four, ghosts around them.
        for (s, v) in [(4, 110), (12, 114), (7, 28), (10, 24), (14, 32), (15, 22)] {
            note(&mut c, &mut p, tick(bar, s), 38, v);
        }
        // The hats: eighths, accented on the beat.
        for s in (0..16).step_by(2) {
            let v = if s % 4 == 0 { 96 } else { 70 };
            match bar {
                // Closed, then loose.
                0 | 1 => {
                    if s == 0 {
                        set_cc(tick(bar, s), if bar == 0 { 0.06 } else { 0.30 });
                    }
                    note(&mut c, &mut p, tick(bar, s), 46, v);
                }
                // Half open.
                2 => {
                    if s == 0 {
                        set_cc(tick(bar, s), 0.62);
                    }
                    note(&mut c, &mut p, tick(bar, s), 46, v);
                }
                // Open on the ands, the foot closing them on the beats.
                _ => {
                    if s % 4 == 2 {
                        set_cc(tick(bar, s), 0.95);
                        note(&mut c, &mut p, tick(bar, s), 46, v + 10);
                    } else {
                        set_cc(tick(bar, s), 0.02);
                    }
                }
            }
        }
    }
    // A tom fill over the last beat of bar four: high, mid, low, low.
    for (s, key, v) in [(12, 45, 100), (13, 45, 84), (14, 43, 96), (15, 41, 110)] {
        note(&mut c, &mut p, tick(3, s), key, v);
    }
    // Bar five: the kick and a crash, and the hats closed.
    set_cc(tick(4, 0), 0.04);
    note(&mut c, &mut p, tick(4, 0), 36, 118);
    note(&mut c, &mut p, tick(4, 0), 49, 116);
    note(&mut c, &mut p, tick(4, 8), 42, 60);
    // `TYMP_GROOVE_KEYS=42,46`: only those keys play (the groove's parts measured apart).
    if let Ok(keys) = std::env::var("TYMP_GROOVE_KEYS") {
        let keep: Vec<u8> = keys
            .split(',')
            .filter_map(|k| k.trim().parse().ok())
            .collect();
        c.notes.retain(|n| keep.contains(&n.pitch));
    }
    c.lanes.push(ClipLane::new(LaneKind::Cc(4), cc4));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

pub fn write_flac(path: &Path, samples: &[f32]) {
    let a = Audio {
        rate: 48_000,
        channels: 2,
        samples: samples.to_vec(),
    };
    let mut f = std::fs::File::create(path).expect("create");
    let mut progress = |_: f32| true;
    cantus_export::encode::flac(
        &mut f,
        &a,
        Depth::Int24,
        Dither::Off,
        &Tags::default(),
        "",
        &mut progress,
    )
    .expect("flac");
}

pub fn stats(samples: &[f32]) -> (f32, f32) {
    let peak = samples.iter().fold(0f32, |m, x| m.max(x.abs()));
    let rms = (samples.iter().map(|x| (x * x) as f64).sum::<f64>() / samples.len().max(1) as f64)
        .sqrt() as f32;
    let db = |x: f32| 20.0 * x.max(1e-9).log10();
    (db(peak), db(rms))
}

/// A player's phrase for a modelled kit (round 40 phase 3, the third listen): ghost notes
/// around the backbeat, flams and a drag, a buzz, a held roll into a crash, the ride's ping,
/// the foot's splashes on the hats. Four bars at 92 BPM; `beater` the kick's (0 felt).
pub fn phrase(kit: &str, beater: f32) -> Project {
    let mut p = Project::new("Tympanum phrase");
    p.tempo_bpm = 92.0;
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some(kit.to_string())).unwrap();
    spec.set("humanize", 0.06);
    spec.set("timing", 3.0);
    spec.set("beater", beater);
    t.instrument = InstrumentSpec::Native(spec);
    let mut c = p.make_clip("Phrase", Ticks::ZERO, Ticks(4 * 4 * PPQ));
    let mut n = |at: Ticks, len: Ticks, key: u8, vel: u8| {
        let note = p.make_note(at, len, key, vel);
        c.notes.push(note);
    };
    let s16 = Ticks(PPQ / 8);
    for bar in 0..4i64 {
        // Kick: one and the and of three; ride on quarters with the bell on four.
        for (s, v) in [(0, 112), (10, 96)] {
            n(tick(bar, s), s16, 36, v);
        }
        for s in [0, 4, 8, 12] {
            let (key, v) = if s == 12 {
                (53, 92)
            } else {
                (51, 84 + (s as u8 % 8) * 2)
            };
            n(tick(bar, s), s16, key, v);
        }
        // The hats with the foot on two and four, a splash on the last bar's and of four.
        for s in [4, 12] {
            n(tick(bar, s), s16, 44, 70);
        }
        // Snare: the backbeat (a flam on bar two's four), ghosts between.
        let four = if bar == 1 { 91 } else { 38 };
        n(tick(bar, 4), s16, 38, 110);
        n(tick(bar, 12), s16, four, 114);
        for (s, v) in [(2, 26), (7, 30), (9, 22), (14, 34), (15, 24)] {
            n(tick(bar, s), s16, 38, v);
        }
    }
    // Bar three: a drag into the backbeat and a buzz on its last eighth.
    n(tick(2, 11), s16, 92, 96);
    n(tick(2, 14), s16, 93, 88);
    // Bar four: a held roll over beats three and four into the crash, the foot's splash.
    n(tick(3, 8), Ticks(PPQ * 2 - PPQ / 8), 94, 92);
    n(tick(3, 14), s16, 95, 90);
    n(tick(4, 0), s16, 49, 118);
    n(tick(4, 0), s16, 36, 118);
    t.clips.push(c);
    p.tracks.push(t);
    p
}
