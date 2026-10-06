//! The engine's dropout blame in words (round 20, `perf_status`): every kind of node is
//! named as the user names it, and a real overrun reaches the status bar.
//!
//! `CANTUS_SHOT_DIR=<dir>` also writes the window with the readout to `<dir>/status.png`.

mod support;
use cantus_core::{EffectKind, Project, Ticks};
use cantus_engine::{Culprit, Dropout, EngineHandle, EngineStats};
use cantus_ui::perf_status::{culprit_text, dropout_text, histogram_text};
use cantus_ui::CantusApp;

fn project() -> Project {
    let mut p = Project::new("perf");
    let mut t = p.make_track("Bass");
    let mut clip = p.make_clip("Bass", Ticks::ZERO, Ticks::beats(8));
    for i in 0..8 {
        let n = p.make_note(Ticks::beats(i), Ticks::QUARTER, 36 + (i as u8 % 3), 100);
        clip.notes.push(n);
    }
    t.clips.push(clip);
    let comp = p.make_effect(EffectKind::Compressor);
    let strip = t.strip.expect("a strip");
    p.strip_mut(strip).unwrap().effects.push(comp);
    p.tracks.push(t);
    let lim = p.make_effect(EffectKind::Limiter);
    p.master.effects.push(lim);
    p
}

#[test]
fn every_node_is_named_as_the_user_names_it() {
    let _app = support::app(Project::new("locale"));
    let p = project();
    let t = &p.tracks[0];
    let strip = t.strip.unwrap();
    let comp = p.strip(strip).unwrap().effects[0].id;
    let lim = p.master.effects[0].id;
    assert_eq!(culprit_text(&p, Culprit::Engine), "the engine");
    assert_eq!(
        culprit_text(&p, Culprit::Instrument(t.id)),
        "Firmus on Bass"
    );
    // An unnamed strip is called after the track that feeds it, as the desk does.
    assert_eq!(culprit_text(&p, Culprit::Strip(strip)), "mixing on Bass");
    assert_eq!(
        culprit_text(
            &p,
            Culprit::Insert {
                strip,
                effect: comp,
                index: 0
            }
        ),
        "Comprimo compressor on Bass"
    );
    assert_eq!(
        culprit_text(
            &p,
            Culprit::MasterInsert {
                effect: lim,
                index: 0
            }
        ),
        "Terminus limiter on Master"
    );
    let d = Dropout {
        culprit: Culprit::MasterInsert {
            effect: lim,
            index: 0,
        },
        culprit_us: 3_100.0,
        block_us: 3_400.0,
        budget_us: 2_666.7,
        block: 12,
    };
    assert_eq!(
        dropout_text(&p, &d),
        "Dropout: Terminus limiter on Master took 3.1 ms of 2.7 ms"
    );
    let mut stats = EngineStats {
        sample_rate: 48_000.0,
        block_frames: 128,
        ..Default::default()
    };
    for _ in 0..99 {
        stats.histogram.record(400.0);
    }
    stats.histogram.record(3_000.0);
    let h = histogram_text(&stats);
    assert!(h.starts_with("Audio callback over 100 blocks"), "{h}");
    assert!(h.contains("budget 2.67 ms"), "{h}");
}

#[test]
fn a_real_overrun_reaches_the_status_bar() {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(project()), engine, false);
    app.sync_engine();
    app.toggle_play();
    // One-frame blocks have 21 µs each: the processor's own work overruns them.
    let mut one = [0f32; 2];
    for _ in 0..2_000 {
        processor.process(&mut one, 2);
        if processor.stats().last_dropout.is_some() {
            break;
        }
    }
    assert!(processor.stats().last_dropout.is_some(), "no block overran");
    let ctx = support::ctx();
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let dir = std::env::var_os("CANTUS_SHOT_DIR").map(std::path::PathBuf::from);
    let path = dir.as_ref().map(|d| d.join("status.png"));
    let mut text = String::new();
    let size = egui::vec2(1600.0, 900.0);
    for i in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(i as f64 * 0.25),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |root| {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(&mut app, root, &mut frame);
        });
        // No renderer applies the frame's textures here (the picker's thumbnails, the
        // verse card…): drop them on purpose, as the other headless tests do.
        out.textures_delta.clear();
        text = support::painted_text(&out.shapes);
    }
    assert!(text.contains("Dropout: "), "the readout is painted: {text}");
    assert!(text.contains(" took "), "{text}");
    if let Some(p) = path {
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        cantus_ui::raster::shoot(&ctx, size, 1.0, 2, Some(&p), |root| {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(&mut app, root, &mut frame);
        });
    }
}
