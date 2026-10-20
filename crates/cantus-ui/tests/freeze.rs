//! Freezing and bouncing in place in the interface (round 29, ADR 0064): Freeze renders on
//! a worker and freezes in one undo step when the render is done; Stop changes nothing; an
//! edit made while it renders is rendered again; an edit a freeze refuses says which and
//! offers Unfreeze; a bounce lays the audio on a new track under the original, which is
//! muted, and the song sounds as it did; low-latency monitoring's switch reaches the
//! engine; the keys and the menus' actions reach all of it.

mod support;

use cantus_core::freeze::FreezeTarget;
use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::bounce::{Plan, Range, Through};
use cantus_ui::keymap::Action;
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};

/// A synth on its own strip with a delay, eight beats; a bass on another; a lonely pad.
fn song() -> Project {
    let mut p = cantus_ui::starter_project();
    p.tracks.clear();
    for (name, notes) in [("Synth", [60u8, 64, 67]), ("Bass", [36, 43, 36])] {
        let mut t = p.make_track(name);
        let mut c = p.make_clip(name, Ticks::ZERO, Ticks::beats(8));
        for (i, k) in notes.iter().enumerate() {
            c.notes
                .push(p.make_note(Ticks::beats(2 * i as i64), Ticks::beats(2), *k, 100));
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    let synth_strip = p.tracks[0].strip.unwrap();
    let delay = p.make_effect(EffectKind::Delay);
    p.strip_mut(synth_strip).unwrap().effects.push(delay);
    p
}

fn app(name: &str) -> (CantusApp, std::path::PathBuf) {
    let dir = std::env::temp_dir()
        .join("cantus-freeze-ui")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = support::app(song());
    app.engine.frozen.set_app_dir(dir.join("renders"));
    // A saved project's folder: its bounces go to `Audio`, its renders to `Freeze`.
    app.path = Some(dir.join("song.cantus"));
    (app, dir)
}

/// Frames until no render runs (or a while has passed).
fn settle(app: &mut CantusApp, ctx: &egui::Context) {
    let t0 = Instant::now();
    loop {
        cantus_ui::freeze::tick(app, ctx);
        if !app.freeze.busy() || t0.elapsed() > Duration::from_secs(120) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    cantus_ui::freeze::tick(app, ctx);
}

/// The song's mix as an export makes it.
fn mix(app: &mut CantusApp) -> Vec<f32> {
    // What a frame does first: the engine resolves the project's own audio files.
    app.sync_engine();
    let project = app.project().clone();
    let snap = app.engine.compile_for_render(&project, 1);
    render(
        snap,
        &RenderOptions {
            tail_seconds: 1.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i}: {} vs {}", a[i], b[i]);
    }
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
}

#[test]
fn freeze_renders_then_freezes_in_one_undo_step_and_the_mix_is_the_live_one() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("freeze");
    let live = mix(&mut app);
    let synth = app.project().tracks[0].id;
    let strip = app.project().tracks[0].strip.unwrap();
    let before = app.project().clone();
    app.select_track(synth);
    app.run_action(Action::FreezeTrack);
    assert!(app.freeze.busy(), "a render on its way");
    assert!(
        app.project().frozen(FreezeTarget::Strip(strip)).is_none(),
        "not yet"
    );
    assert!(
        cantus_ui::views::freeze_view::card(&app).is_some(),
        "the card says so"
    );
    settle(&mut app, &ctx);
    assert!(app.project().frozen(FreezeTarget::Strip(strip)).is_some());
    assert!(app.freeze_holding_track(synth).is_some());
    same_bits(&mix(&mut app), &live, "frozen");
    // An edit inside it is refused, said, and offered Unfreeze.
    let clip = app.project().tracks[0].clips[0].id;
    assert!(!app.exec(Command::SetClipMute {
        track: synth,
        clip,
        muted: true
    }));
    assert!(app.status.error);
    let (title, _, buttons, _) = cantus_ui::views::freeze_view::card(&app).expect("a card");
    assert!(title.contains("Synth"), "{title}");
    assert!(buttons
        .iter()
        .any(|(_, a)| *a
            == cantus_ui::views::freeze_view::Answer::Unfreeze(FreezeTarget::Strip(strip))));
    // Its fader is live.
    assert!(app.exec(Command::SetStripGain {
        id: strip,
        gain_db: -3.0
    }));
    app.undo();
    // One undo step back to before.
    app.undo();
    assert_eq!(app.project(), &before);
    // The key again thaws it.
    app.redo();
    app.run_action(Action::FreezeTrack);
    assert!(app.project().frozen(FreezeTarget::Strip(strip)).is_none());
}

#[test]
fn stop_leaves_the_project_as_it_was() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("stop");
    let strip = app.project().tracks[0].strip.unwrap();
    let before = app.project().clone();
    app.freeze_target(FreezeTarget::Strip(strip));
    app.cancel_freeze();
    settle(&mut app, &ctx);
    assert_eq!(app.project(), &before);
    assert!(!app.bus.can_undo());
}

#[test]
fn an_edit_made_while_it_renders_is_rendered_again() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("edited");
    let synth = app.project().tracks[0].id;
    let strip = app.project().tracks[0].strip.unwrap();
    app.freeze_target(FreezeTarget::Strip(strip));
    assert!(app.exec(Command::SetTrackGain {
        id: synth,
        gain_db: -4.0
    }));
    settle(&mut app, &ctx);
    let p = app.project();
    assert_eq!(
        p.frozen(FreezeTarget::Strip(strip))
            .map(|f| f.content.clone()),
        cantus_core::freeze::content_key(p, FreezeTarget::Strip(strip)),
        "frozen for what it holds now"
    );
    assert!(app
        .engine
        .frozen
        .find(
            &p.frozen(FreezeTarget::Strip(strip))
                .unwrap()
                .content
                .clone(),
            48_000
        )
        .is_some());
}

#[test]
fn a_frozen_project_reopened_without_its_renders_plays_live_and_makes_them() {
    let ctx = support::ctx();
    let (mut app, dir) = app("reopen");
    let strip = app.project().tracks[0].strip.unwrap();
    app.freeze_target(FreezeTarget::Strip(strip));
    settle(&mut app, &ctx);
    let frozen = app.project().clone();
    assert!(frozen.any_frozen());
    // Elsewhere: no renders there.
    let mut other = support::app(frozen);
    other.engine.frozen.set_app_dir(dir.join("elsewhere"));
    let content = other
        .project()
        .frozen(FreezeTarget::Strip(strip))
        .unwrap()
        .content
        .clone();
    assert!(other.engine.frozen.find(&content, 48_000).is_none());
    cantus_ui::freeze::tick(&mut other, &ctx);
    assert!(other.freeze.busy(), "its render is made again");
    settle(&mut other, &ctx);
    assert!(other.engine.frozen.find(&content, 48_000).is_some());
    assert_eq!(
        other
            .project()
            .frozen(FreezeTarget::Strip(strip))
            .unwrap()
            .content,
        content
    );
}

#[test]
fn bounce_in_place_lays_the_audio_under_the_original_and_the_song_sounds_the_same() {
    let ctx = support::ctx();
    for through in [Through::Source, Through::Inserts] {
        let (mut app, _dir) = app(&format!("bounce-{through:?}"));
        let live = mix(&mut app);
        let synth = app.project().tracks[0].id;
        let tracks = app.project().tracks.len();
        app.bounce(Plan {
            track: synth,
            through,
            range: Range::Track,
        });
        settle(&mut app, &ctx);
        let p = app.project();
        assert_eq!(p.tracks.len(), tracks + 1, "{through:?}: a new track");
        let new = &p.tracks[1];
        assert!(
            new.clips.iter().any(|c| c.audio.is_some()),
            "its audio clip"
        );
        assert!(p.tracks[0].muted, "the original muted");
        match through {
            Through::Source => assert_eq!(new.strip, p.tracks[0].strip, "the same strip"),
            _ => assert_ne!(new.strip, p.tracks[0].strip, "a strip of its own"),
        }
        // The bounce keeps the delay's tail on (the clip runs until it rings out), so
        // the song is longer now: what the live song held is the same, to the bit.
        let after = mix(&mut app);
        assert!(after.len() >= live.len());
        same_bits(&after[..live.len()], &live, &format!("{through:?}"));
        // One undo step.
        app.undo();
        assert_eq!(app.project().tracks.len(), tracks);
        assert!(!app.project().tracks[0].muted);
    }
}

#[test]
fn bounce_of_selected_clips_takes_those_and_mutes_them() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("clips");
    let bass = app.project().tracks[1].id;
    let clip = app.project().tracks[1].clips[0].id;
    app.select_clip(bass, clip);
    app.run_action(Action::BounceSource);
    settle(&mut app, &ctx);
    let p = app.project();
    assert!(p.tracks[1].clips[0].muted, "the clip muted");
    assert!(!p.tracks[1].muted, "not its track");
    assert!(p.tracks[2].clips.iter().any(|c| c.audio.is_some()));
}

#[test]
fn low_latency_monitoring_reaches_the_engine_by_its_key() {
    let (engine, mut proc): (EngineHandle, Processor) = EngineHandle::manual(48_000.0);
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts(Some(song()), engine, false);
    let ctx = support::ctx();
    let mut buf = vec![0f32; 256 * 2];
    let mut run = |proc: &mut Processor| {
        proc.process(&mut buf, 2);
        proc.stats().low_latency.on
    };
    cantus_ui::freeze::tick(&mut app, &ctx);
    assert!(!run(&mut proc));
    app.run_action(Action::ToggleLowLatency);
    assert!(app.settings.low_latency);
    cantus_ui::freeze::tick(&mut app, &ctx);
    assert!(run(&mut proc), "on in the engine");
    app.run_action(Action::ToggleLowLatency);
    cantus_ui::freeze::tick(&mut app, &ctx);
    assert!(!run(&mut proc));
}

#[test]
fn every_string_is_in_both_languages() {
    let en = include_str!("../locales/areas/freeze.en.txt");
    let es = include_str!("../locales/areas/freeze.es.txt");
    let keys = |s: &str| -> Vec<String> {
        s.lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim().to_string()))
            .collect()
    };
    assert_eq!(keys(en), keys(es));
    for a in [
        Action::FreezeTrack,
        Action::BounceInPlace,
        Action::BounceSource,
        Action::ToggleLowLatency,
    ] {
        assert!(keys(en).contains(&a.label_key()), "{}", a.label_key());
    }
}
