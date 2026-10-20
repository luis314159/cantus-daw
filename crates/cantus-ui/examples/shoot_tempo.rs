//! The tempo track, the metre changes and the markers without a screen (round 29, ADR 0060).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_tempo -- [out-dir]
//! ```
//!
//! A song with a ramp from 90 to 140 BPM, a bar of 7/8, 3/4 and 4/4 sections and five
//! markers: the arrangement with its tempo track, metre chips and marker lane; the top bar
//! with the playhead inside the ramp; the export dialog between two markers — in Cantus
//! Night and Paper, English and Spanish, at 75 %, 100 % and 200 %, and a narrow window;
//! review 4's scenes (`tempo-r4-*`): the ruler's metre editor, a run of markers on one
//! tick, a take cut in three dragged across the change with a MIDI clip, a reversed take.

use cantus_core::tempo::{Marker, MarkerId, MetreChange, TempoPoint};
use cantus_core::{Color, Project, Ticks, TimeSignature};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, CantusApp};
use std::path::PathBuf;

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme_family::cantus_night()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

/// The song: 90 BPM ramping to 140 over the first two bars, held, then 110 from bar 9; 4/4,
/// a bar of 7/8 at bar 3, 3/4 from bar 4, 4/4 again from bar 8; five markers; a few clips.
fn song() -> Project {
    let mut p = Project::new("Tempo");
    p.tempo_bpm = 90.0;
    let pt = |tick: i64, bpm: f64, ramp: bool| TempoPoint {
        tick: Ticks(tick),
        bpm,
        ramp,
    };
    let bar3 = 2 * 3840;
    let bar4 = bar3 + 3360;
    let bar8 = bar4 + 4 * 2880;
    let bar9 = bar8 + 3840;
    p.tempo_points = vec![
        pt(0, 90.0, true),
        pt(bar3, 140.0, false),
        pt(bar9, 110.0, false),
    ];
    let mc = |tick: i64, n: u8, d: u8| MetreChange {
        tick: Ticks(tick),
        signature: TimeSignature::new(n, d),
    };
    p.metre_changes = vec![mc(0, 4, 4), mc(bar3, 7, 8), mc(bar4, 3, 4), mc(bar8, 4, 4)];
    for (i, (name, tick, color)) in [
        ("Intro", 0, None),
        ("Verse", bar3, Some(Color::PALETTE[1])),
        ("Pre-chorus", bar4 + 2 * 2880, Some(Color::PALETTE[2])),
        ("Chorus", bar8, Some(Color::PALETTE[0])),
        ("Outro", bar9 + 4 * 3840, Some(Color::PALETTE[4])),
    ]
    .into_iter()
    .enumerate()
    {
        p.markers.push(Marker {
            id: MarkerId(10_000 + i as u64),
            tick: Ticks(tick),
            name: name.into(),
            color,
        });
    }
    for (name, start, len) in [("Keys", 0, bar8), ("Bass", bar3, bar9 + 4 * 3840 - bar3)] {
        let mut t = p.make_track(name);
        let mut c = p.make_clip(name, Ticks(start), Ticks(len));
        for k in 0..(len / 960) {
            let n = p.make_note(Ticks(960 * k), Ticks(480), 48 + (k % 7) as u8, 100);
            c.notes.push(n);
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    p
}

fn app() -> CantusApp {
    let mut app = CantusApp::from_parts(Some(song()), EngineHandle::dummy(), false);
    app.arrangement.tempo.shown = Some(true);
    app.arrangement.px_per_beat = 30.0;
    // The playhead inside the ramp.
    app.seek(Ticks(3840 + 1200));
    app
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/tempo"));
    std::fs::create_dir_all(&dir).expect("output dir");
    use cantus_ui::i18n::Locale;
    for dark in [true, false] {
        for lang in [Locale::En, Locale::Es] {
            for k in [0.75f32, 1.0, 2.0] {
                cantus_ui::i18n::set_locale(lang);
                let theme = if dark { "night" } else { "paper" };
                let tag = format!("{theme}-{}-k{}", lang.code(), (k * 100.0) as u32);
                let ctx = context(dark);
                let mut a = app();
                let out = dir.join(format!("tempo-arrangement-{tag}.png"));
                shoot(&ctx, egui::vec2(1400.0, 520.0), k, 6, Some(&out), |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| views::arrangement::show(&mut a, ui));
                });
                println!("{}", out.display());
                let ctx = context(dark);
                let mut a = app();
                let out = dir.join(format!("tempo-topbar-{tag}.png"));
                shoot(&ctx, egui::vec2(1400.0, 60.0), k, 6, Some(&out), |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| views::transport::show(&mut a, ui));
                });
                println!("{}", out.display());
            }
        }
    }
    // The export dialog between two markers.
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_locale(lang);
        let ctx = context(true);
        let mut a = app();
        a.export_between_markers(MarkerId(10_001), MarkerId(10_003));
        let out = dir.join(format!("tempo-export-night-{}.png", lang.code()));
        shoot(
            &ctx,
            egui::vec2(1200.0, 820.0),
            1.0,
            6,
            Some(&out),
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| views::transport::show(&mut a, ui));
                views::export_dialog::show(&mut a, root);
            },
        );
        println!("{}", out.display());
    }
    // Narrow: the lanes keep their words inside their widths.
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_locale(lang);
        let ctx = context(true);
        let mut a = app();
        let out = dir.join(format!("tempo-narrow-night-{}.png", lang.code()));
        shoot(&ctx, egui::vec2(640.0, 480.0), 1.0, 6, Some(&out), |root| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| views::arrangement::show(&mut a, ui));
        });
        println!("{}", out.display());
    }
    review4(&dir);
}

/// A take of a rising tone with a click every beat at 120: 16 s, mono, 48 kHz.
fn take_wav(path: &std::path::Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("wav");
    for i in 0..48_000 * 16 {
        let t = i as f32 / 48_000.0;
        let swell = (t / 16.0).min(1.0);
        let click = if i % 24_000 < 600 { 0.8 } else { 0.0 };
        let v = (swell * 0.5 * (t * 220.0 * std::f32::consts::TAU).sin() + click).clamp(-1.0, 1.0);
        w.write_sample((v * 32767.0) as i16).expect("sample");
    }
    w.finalize().expect("wav");
}

/// Review 4's scenes: the ruler's metre editor (at bar 1, near the right edge, narrow, at
/// 200 %), a run of markers on one tick before a close one, a drag of a take cut in three
/// with a MIDI clip across the tempo change, and a reversed take across it.
fn review4(dir: &std::path::Path) {
    use cantus_core::{AudioSource, AudioSourceId, ClipId, TrackId};
    use cantus_ui::i18n::Locale;
    let bar9 = 2 * 3840 + 3360 + 4 * 2880 + 3840;
    let wav = dir.join("take.wav");
    take_wav(&wav);
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_locale(lang);
        let code = lang.code();
        // The ruler's metre editor: at bar 1 and at bar 12, wide, narrow and at 200 %.
        for (name, size, k, bar) in [
            ("bar1", egui::vec2(1400.0, 520.0), 1.0f32, 0i64),
            ("edge", egui::vec2(1400.0, 520.0), 1.0, 3840 * 12),
            ("narrow", egui::vec2(420.0, 520.0), 1.0, 3840 * 4),
            ("k200", egui::vec2(1400.0, 520.0), 2.0, 3840 * 12),
        ] {
            for dark in [true, false] {
                let theme = if dark { "night" } else { "paper" };
                let ctx = context(dark);
                let mut a = app();
                let out = dir.join(format!("tempo-r4-metre-{name}-{theme}-{code}.png"));
                let mut pass = 0;
                shoot(&ctx, size, k, 6, Some(&out), |root| {
                    pass += 1;
                    if pass == 3 {
                        views::arrangement::tempo::add_metre_change(&mut a, Ticks(bar));
                    }
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| views::arrangement::show(&mut a, ui));
                });
                println!("{}", out.display());
            }
        }
        // A run of markers on one tick, a marker just after it.
        for dark in [true, false] {
            let theme = if dark { "night" } else { "paper" };
            let ctx = context(dark);
            let mut p = song();
            for (i, (name, tick)) in [
                ("Bridge build-up", bar9 + 2 * 3840),
                ("B2", bar9 + 2 * 3840),
                ("Drop", bar9 + 2 * 3840 + 1920),
            ]
            .into_iter()
            .enumerate()
            {
                p.markers.push(Marker {
                    id: MarkerId(20_000 + i as u64),
                    tick: Ticks(tick),
                    name: name.into(),
                    color: Some(Color::PALETTE[3 + i % 3]),
                });
            }
            let mut a = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
            a.arrangement.tempo.shown = Some(true);
            a.arrangement.px_per_beat = 30.0;
            let out = dir.join(format!("tempo-r4-markers-{theme}-{code}.png"));
            shoot(
                &ctx,
                egui::vec2(1400.0, 520.0),
                1.0,
                6,
                Some(&out),
                |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| views::arrangement::show(&mut a, ui));
                },
            );
            println!("{}", out.display());
        }
        // A take cut in three and a MIDI clip, dragged across the change; a reversed take.
        for dark in [true, false] {
            let theme = if dark { "night" } else { "paper" };
            let ctx = context(dark);
            let mut p = song();
            let sid = AudioSourceId(p.alloc_id());
            p.audio_pool.push(AudioSource {
                id: sid,
                name: "take".into(),
                path: wav.to_string_lossy().into_owned(),
                sample_rate: 48_000,
                channels: 1,
                frames: 48_000 * 16,
                sha256: None,
                recorded: true,
            });
            let mut t = p.make_track("Vocal");
            let mut cut = Vec::new();
            for (start, len) in [(0i64, 2400i64), (2400, 2700), (5100, 2580)] {
                let mut c = p.make_audio_clip("take", sid, Ticks(start), Ticks(len));
                c.offset = Ticks(start);
                cut.push(c.id);
                t.clips.push(c);
            }
            let mut r = p.make_audio_clip("reversed", sid, Ticks(bar9 - 3840), Ticks(2 * 3840));
            r.offset = Ticks(3840);
            r.audio.as_mut().unwrap().reverse = true;
            t.clips.push(r);
            let tid: TrackId = t.id;
            p.tracks.push(t);
            let keys = p.tracks[0].id;
            let keys_clip: ClipId = p.tracks[0].clips[0].id;
            let mut a = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
            a.arrangement.tempo.shown = Some(true);
            a.arrangement.px_per_beat = 30.0;
            for s in a.project().audio_pool.clone() {
                let _ = a.peaks.lookup(&s, a.audio_base(), None);
            }
            a.peaks.wait_idle(std::time::Duration::from_secs(30));
            let mut sel: Vec<(TrackId, ClipId)> = cut.iter().map(|c| (tid, *c)).collect();
            sel.push((keys, keys_clip));
            a.set_clip_selection(sel.clone());
            a.arrangement
                .shoot_move(sel[0], Ticks::ZERO, Ticks(3840 * 3));
            let out = dir.join(format!("tempo-r4-drag-{theme}-{code}.png"));
            shoot(
                &ctx,
                egui::vec2(1400.0, 620.0),
                1.0,
                6,
                Some(&out),
                |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| views::arrangement::show(&mut a, ui));
                },
            );
            println!("{}", out.display());
        }
    }
}
