//! Freezing, bouncing in place and low-latency monitoring without a screen (round 29, ADR
//! 0064).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_freeze -- [out-dir]
//! ```
//!
//! A song with a heavy synth frozen with its strip, a bass frozen on its own (it shares its
//! strip with a pad), a singer armed and monitored through Tonus with a linear-phase EQ, and
//! low-latency monitoring on: the arrangement (snowflakes, frosted clips), the mixer with
//! the frozen strip's menu open, the inspector's *Freeze* and *Low latency* rows, the
//! track menu's entries, the corner cards (a freeze rendering with its Stop; an edit
//! refused with its Unfreeze), the audio chip's popover with the bypassed inserts named —
//! in Cantus Night and Paper, English and Spanish, at 75 %, 100 % and 200 %, and a narrow
//! window.

use cantus_core::freeze::{self, FreezeTarget};
use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
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

/// The song (see the module's docs); the synth's strip and the bass's source frozen.
fn song() -> Project {
    let mut p = Project::new("Freeze");
    p.tempo_bpm = 112.0;
    let names = ["Lead synth", "Bass", "Pad", "Voice"];
    for (i, name) in names.iter().enumerate() {
        let mut t = p.make_track(*name);
        if *name != "Voice" {
            let mut c = p.make_clip(*name, Ticks::beats(4 * i as i64 % 8), Ticks::beats(24));
            for k in 0..24 {
                let n = p.make_note(Ticks::beats(k), Ticks::QUARTER, 48 + (k % 12) as u8, 96);
                c.notes.push(n);
            }
            t.clips.push(c);
        }
        p.tracks.push(t);
    }
    // The pad shares the bass's strip: Freeze on the bass freezes its source alone.
    p.tracks[2].strip = p.tracks[1].strip;
    let lead = p.tracks[0].strip.unwrap();
    for kind in [EffectKind::Chorus, EffectKind::Space] {
        let e = p.make_effect(kind);
        p.strip_mut(lead).unwrap().effects.push(e);
    }
    // The singer: armed, monitored, through Tonus and a linear-phase EQ.
    let voice = p.tracks[3].strip.unwrap();
    let mut input = TrackInput::mono(0);
    input.monitor = MonitorMode::Auto;
    p.tracks[3].input = Some(input);
    p.tracks[3].armed = true;
    let tonus = p.make_effect(EffectKind::AutoTune);
    p.strip_mut(voice).unwrap().effects.push(tonus);
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("mode", 2.0).ok();
    p.strip_mut(voice).unwrap().effects.push(eq);
    for target in [
        FreezeTarget::Strip(lead),
        FreezeTarget::Track(p.tracks[1].id),
    ] {
        let cmd = freeze::freeze_commands(&p, target).expect("freezable");
        cmd.apply(&mut p).expect("frozen");
    }
    p
}

/// The app on a manual engine that has played a moment (so the stats name what low-latency
/// monitoring bypasses).
fn app() -> (CantusApp, Processor) {
    let (engine, mut proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(song()), engine, false);
    app.settings.low_latency = true;
    app.arrangement.px_per_beat = 26.0;
    let ctx = egui::Context::default();
    cantus_ui::freeze::apply_low_latency(&mut app);
    app.sync_engine();
    let mut buf = vec![0f32; 256 * 2];
    for _ in 0..8 {
        proc.process(&mut buf, 2);
        app.engine.poll();
    }
    // The renders are not made here: the snowflakes are the model's.
    let _ = ctx;
    let synth = app.project().tracks[0].id;
    app.select_track(synth);
    (app, proc)
}

fn panel(root: &mut egui::Ui, f: impl FnOnce(&mut egui::Ui)) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(root, f);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/freeze"));
    std::fs::create_dir_all(&dir).expect("output dir");
    use cantus_ui::i18n::Locale;
    for dark in [true, false] {
        for lang in [Locale::En, Locale::Es] {
            for k in [0.75f32, 1.0, 2.0] {
                cantus_ui::i18n::set_locale(lang);
                let theme = if dark { "night" } else { "paper" };
                let tag = format!("{theme}-{}-k{}", lang.code(), (k * 100.0) as u32);
                // The arrangement: snowflakes on the headers, the frozen clips frosted.
                let ctx = context(dark);
                let (mut a, _p) = app();
                let out = dir.join(format!("freeze-arrangement-{tag}.png"));
                shoot(&ctx, egui::vec2(1400.0, 420.0), k, 6, Some(&out), |root| {
                    panel(root, |ui| views::arrangement::show(&mut a, ui));
                });
                println!("{}", out.display());
                // The mixer: the frozen strip's head, its menu open.
                let ctx = context(dark);
                let (mut a, _p) = app();
                let lead = a.project().tracks[0].strip;
                views::mixer::open_menu(&mut a, lead, egui::pos2(150.0, 120.0));
                let out = dir.join(format!("freeze-mixer-{tag}.png"));
                shoot(&ctx, egui::vec2(1400.0, 760.0), k, 6, Some(&out), |root| {
                    panel(root, |ui| views::mixer::show(&mut a, ui));
                });
                println!("{}", out.display());
                // The inspector: the singer's channel — Low latency on, what it bypasses —
                // and the frozen synth's Freeze row.
                for (name, track) in [("voice", 3usize), ("synth", 0)] {
                    let ctx = context(dark);
                    let (mut a, _p) = app();
                    let t = a.project().tracks[track].id;
                    a.select_track(t);
                    let out = dir.join(format!("freeze-inspector-{name}-{tag}.png"));
                    shoot(&ctx, egui::vec2(340.0, 1100.0), k, 6, Some(&out), |root| {
                        panel(root, |ui| views::inspector::show(&mut a, ui));
                    });
                    println!("{}", out.display());
                }
                // The track menu's entries.
                let ctx = context(dark);
                let (mut a, _p) = app();
                let bass = a.project().tracks[1].id;
                let out = dir.join(format!("freeze-menu-{tag}.png"));
                shoot(&ctx, egui::vec2(300.0, 180.0), k, 4, Some(&out), |root| {
                    panel(root, |ui| {
                        let _ = views::freeze_view::track_menu(&mut a, ui, bass);
                    });
                });
                println!("{}", out.display());
                // The corner card: an edit the frozen bass refused, with its Unfreeze; the
                // audio chip's popover with low-latency monitoring.
                let ctx = context(dark);
                let (mut a, _p) = app();
                let clip = a.project().tracks[1].clips[0].id;
                a.exec(Command::SetClipMute {
                    track: bass,
                    clip,
                    muted: true,
                });
                a.audio_setup.popover = true;
                let out = dir.join(format!("freeze-cards-{tag}.png"));
                shoot(&ctx, egui::vec2(1200.0, 760.0), k, 6, Some(&out), |root| {
                    panel(root, |_| {});
                    views::audio_setup::frame(&mut a, root.ctx());
                });
                println!("{}", out.display());
            }
        }
    }
    // A freeze rendering: its card with the Stop (the render runs for real, into a scratch
    // folder), and the narrow window.
    for lang in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_locale(lang);
        let ctx = context(true);
        let (mut a, _p) = app();
        let scratch = std::env::temp_dir().join("cantus-shoot-freeze");
        let _ = std::fs::remove_dir_all(&scratch);
        a.engine.frozen.set_app_dir(scratch.clone());
        let pad = a.project().tracks[2].id;
        a.freeze_target(FreezeTarget::Track(pad));
        let out = dir.join(format!("freeze-rendering-night-{}.png", lang.code()));
        shoot(
            &ctx,
            egui::vec2(1200.0, 400.0),
            1.0,
            6,
            Some(&out),
            |root| {
                panel(root, |_| {});
                views::audio_setup::frame(&mut a, root.ctx());
            },
        );
        a.cancel_freeze();
        println!("{}", out.display());
        let ctx = context(true);
        let (mut a, _p) = app();
        let out = dir.join(format!("freeze-narrow-night-{}.png", lang.code()));
        shoot(&ctx, egui::vec2(640.0, 420.0), 1.0, 6, Some(&out), |root| {
            panel(root, |ui| views::arrangement::show(&mut a, ui));
        });
        println!("{}", out.display());
    }
}
