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
        // What a frame does first: files loaded in the background taken in.
        app.sync_engine();
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

/// Within −100 dB of each other: a time range's cut sums a note's tail before it and the
/// bounce after it apart, where the instrument summed them (float order).
fn close(a: &[f32], b: &[f32], what: &str) {
    within(a, b, 1e-5, what);
}

/// Within `limit` of each other, sample for sample.
fn within(a: &[f32], b: &[f32], limit: f32, what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    let worst = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0f32, f32::max);
    assert!(worst < limit, "{what}: {worst}");
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
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

// ---- review 1 ---------------------------------------------------------------------------

/// Bounce `plan` and let it render.
fn bounce(app: &mut CantusApp, ctx: &egui::Context, plan: Plan) {
    app.bounce(plan);
    settle(app, ctx);
}

#[test]
fn a_time_range_inside_one_clip_is_cut_at_both_edges_and_sounds_the_same() {
    let ctx = support::ctx();
    for through in [Through::Source, Through::Inserts] {
        let (mut app, _dir) = app(&format!("time-{through:?}"));
        let live = mix(&mut app);
        let synth = app.project().tracks[0].id;
        // Beats 2 to 4: the clip runs 0 to 8, its second note sounds 2 to 4.
        let (a, b) = (Ticks::beats(2), Ticks::beats(4));
        bounce(
            &mut app,
            &ctx,
            Plan {
                track: synth,
                through,
                range: Range::Time(a, b),
            },
        );
        let p = app.project();
        let orig = &p.tracks[0];
        let inside: Vec<&Clip> = orig
            .clips
            .iter()
            .filter(|c| c.start >= a && c.end() <= b)
            .collect();
        assert_eq!(inside.len(), 1, "{through:?}: one piece inside the range");
        assert!(inside[0].muted, "the piece inside is muted");
        assert!(
            orig.clips
                .iter()
                .filter(|c| !c.muted)
                .all(|c| c.end() <= a || c.start >= b),
            "{through:?}: nothing of the original plays inside the range (review 1: doubled)"
        );
        let new = &p.tracks[1];
        assert_eq!(new.clips[0].start, a, "the bounce starts at the range");
        let after = mix(&mut app);
        // Through the inserts, the chain is split at the cut too: the echoes of what played
        // before it ring on the old strip, the bounce's after it on its own (−60 dB).
        let limit = if through == Through::Source {
            1e-5
        } else {
            1e-3
        };
        within(
            &after[..live.len()],
            &live,
            limit,
            &format!("{through:?}: the song"),
        );
    }
}

#[test]
fn a_bounce_through_a_latent_chain_keeps_the_song_to_the_bit() {
    let ctx = support::ctx();
    for through in [Through::Source, Through::Inserts] {
        let (mut app, _dir) = app(&format!("latent-{through:?}"));
        let strip = app.project().tracks[0].strip.unwrap();
        let mut p = app.project().clone();
        let mut eq = p.make_effect(EffectKind::Eq);
        eq.spec.set("mode", 2.0).unwrap();
        p.strip_mut(strip).unwrap().effects.push(eq);
        app.bus = CommandBus::new(p);
        let live = mix(&mut app);
        let synth = app.project().tracks[0].id;
        bounce(
            &mut app,
            &ctx,
            Plan {
                track: synth,
                through,
                range: Range::Track,
            },
        );
        assert_eq!(app.project().tracks.len(), 3, "{through:?}: bounced");
        let after = mix(&mut app);
        same_bits(&after[..live.len()], &live, &format!("{through:?}: latent"));
    }
}

#[test]
fn a_solo_elsewhere_is_not_in_a_bounce_and_a_soloed_original_hands_its_solo_over() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("solo");
    let live = mix(&mut app);
    let synth = app.project().tracks[0].id;
    let bass = app.project().tracks[1].id;
    // The bass soloed while the synth is bounced with its inserts.
    assert!(app.exec(Command::SetTrackSolo {
        id: bass,
        soloed: true
    }));
    bounce(
        &mut app,
        &ctx,
        Plan {
            track: synth,
            through: Through::Inserts,
            range: Range::Track,
        },
    );
    assert!(app.exec(Command::SetTrackSolo {
        id: bass,
        soloed: false
    }));
    let after = mix(&mut app);
    same_bits(
        &after[..live.len()],
        &live,
        "bounced under a solo, unsoloed",
    );
    // A soloed original: its bounce takes the solo.
    let (mut app, _dir) = self::app("solo-own");
    let synth = app.project().tracks[0].id;
    assert!(app.exec(Command::SetTrackSolo {
        id: synth,
        soloed: true
    }));
    bounce(
        &mut app,
        &ctx,
        Plan {
            track: synth,
            through: Through::Source,
            range: Range::Track,
        },
    );
    let p = app.project();
    assert!(p.tracks[1].soloed && !p.tracks[0].soloed, "the solo moved");
    assert!(p.track_audible(&p.tracks[1]), "the bounce is heard");
}

#[test]
fn an_edit_made_while_a_bounce_renders_is_bounced_again() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("bounce-edited");
    let synth = app.project().tracks[0].id;
    app.bounce(Plan {
        track: synth,
        through: Through::Source,
        range: Range::Track,
    });
    assert!(app.freeze.busy());
    // A note moved while it renders.
    let clip = app.project().tracks[0].clips[0].clone();
    let mut notes = clip.notes.clone();
    notes[0].pitch = 72;
    assert!(app.exec(Command::SetNotes {
        track: synth,
        clip: clip.id,
        notes: notes.clone(),
    }));
    settle(&mut app, &ctx);
    let p = app.project();
    assert_eq!(p.tracks[0].clips[0].notes[0].pitch, 72, "the edit is kept");
    assert_eq!(p.tracks.len(), 3, "bounced once, from the edited notes");
    // The bounce plays the edit: muted original, the mix is the edited song's.
    let mut live = p.clone();
    live.tracks.remove(1);
    live.tracks[0].muted = false;
    let edited = {
        let (mut other, _d) = self::app("bounce-edited-live");
        other.bus = CommandBus::new(live);
        mix(&mut other)
    };
    let after = mix(&mut app);
    same_bits(&after[..edited.len()], &edited, "the bounce is of the edit");
}

#[test]
fn a_bounce_asked_twice_while_it_renders_is_made_once() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("twice");
    let synth = app.project().tracks[0].id;
    let plan = Plan {
        track: synth,
        through: Through::Inserts,
        range: Range::Track,
    };
    app.bounce(plan.clone());
    app.bounce(plan.clone());
    settle(&mut app, &ctx);
    assert_eq!(app.project().tracks.len(), 3, "one bounce");
    // And once bounced (the original muted), it is not bounced again.
    app.bounce(plan);
    assert!(app.status.error);
    settle(&mut app, &ctx);
    assert_eq!(app.project().tracks.len(), 3);
}

#[test]
fn a_bounce_carries_the_rides_of_what_it_replaces() {
    let ctx = support::ctx();
    for through in [Through::Source, Through::Inserts] {
        let (mut app, _dir) = app(&format!("rides-{through:?}"));
        let synth = app.project().tracks[0].id;
        let strip = app.project().tracks[0].strip.unwrap();
        // A level ride on the synth track and a fader ride on its strip.
        let mut p = app.project().clone();
        for target in [AutoTarget::TrackGain(synth), AutoTarget::StripGain(strip)] {
            let ride = p.make_automation_clip(
                "ride",
                target,
                Ticks::ZERO,
                Ticks::beats(8),
                vec![
                    automation::AutoPoint::new(Ticks::ZERO, 0.9),
                    automation::AutoPoint::new(Ticks::beats(8), 0.2),
                ],
            );
            p.tracks[1].clips.push(ride);
        }
        app.bus = CommandBus::new(p);
        let live = mix(&mut app);
        settings_remove(&mut app, true);
        bounce(
            &mut app,
            &ctx,
            Plan {
                track: synth,
                through,
                range: Range::Track,
            },
        );
        let p = app.project();
        assert!(p.track(synth).is_none(), "{through:?}: removed");
        let after = mix(&mut app);
        same_bits(
            &after[..live.len()],
            &live,
            &format!("{through:?}: the rides"),
        );
    }
}

/// The *Remove the original* setting.
fn settings_remove(app: &mut CantusApp, on: bool) {
    app.settings.bounce_removes = on;
}

#[test]
fn a_strip_bounced_whole_hands_its_key_over() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("key");
    let synth_strip = app.project().tracks[0].strip.unwrap();
    let bass_strip = app.project().tracks[1].strip.unwrap();
    // The bass's compressor keyed from the synth's strip.
    let mut p = app.project().clone();
    let mut comp = p.make_effect(EffectKind::Compressor);
    comp.sidechain = Some(synth_strip);
    let comp_id = comp.id;
    p.strip_mut(bass_strip).unwrap().effects.push(comp);
    app.bus = CommandBus::new(p);
    let live = mix(&mut app);
    app.bounce_strip(synth_strip);
    settle(&mut app, &ctx);
    let p = app.project();
    let new = p.tracks[1].strip.unwrap();
    assert_ne!(new, synth_strip);
    assert_eq!(
        p.strip(bass_strip)
            .unwrap()
            .effects
            .iter()
            .find(|e| e.id == comp_id)
            .unwrap()
            .sidechain,
        Some(new),
        "the key follows the new strip"
    );
    let after = mix(&mut app);
    same_bits(&after[..live.len()], &live, "keyed from the bounce");
}

#[test]
fn a_time_range_takes_in_the_notes_held_across_its_edges() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("held");
    let live = mix(&mut app);
    let synth = app.project().tracks[0].id;
    // The synth's notes sound 0–2, 2–4, 4–6: a range from beat 3 to beat 5 cuts two.
    bounce(
        &mut app,
        &ctx,
        Plan {
            track: synth,
            through: Through::Source,
            range: Range::Time(Ticks::beats(3), Ticks::beats(5)),
        },
    );
    let p = app.project();
    assert_eq!(
        p.tracks[1].clips[0].start,
        Ticks::beats(2),
        "moved out to the note held across the start"
    );
    let after = mix(&mut app);
    close(&after[..live.len()], &live, "no gap");
}

#[test]
fn removing_the_original_of_a_time_range_keeps_a_muted_clip_it_never_rendered() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("muted-clip");
    let synth = app.project().tracks[0].id;
    let mut p = app.project().clone();
    let mut alt = p.make_clip("alt", Ticks::beats(2), Ticks::beats(1));
    alt.muted = true;
    let alt_id = alt.id;
    p.tracks[0].clips.push(alt);
    app.bus = CommandBus::new(p);
    settings_remove(&mut app, true);
    bounce(
        &mut app,
        &ctx,
        Plan {
            track: synth,
            through: Through::Source,
            range: Range::Time(Ticks::beats(2), Ticks::beats(4)),
        },
    );
    let t = app.project().track(synth).unwrap();
    assert!(t.clip(alt_id).is_some_and(|c| c.muted), "kept, still muted");
}

#[test]
fn a_render_asked_in_one_project_is_never_laid_into_another() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("epoch");
    let synth = app.project().tracks[0].id;
    let strip = app.project().tracks[0].strip.unwrap();
    app.bounce(Plan {
        track: synth,
        through: Through::Source,
        range: Range::Track,
    });
    app.freeze_target(FreezeTarget::Strip(strip));
    // Another project opened meanwhile, with the same ids.
    let other = song();
    app.replace_project(other.clone());
    settle(&mut app, &ctx);
    assert_eq!(
        app.project().tracks.len(),
        other.tracks.len(),
        "nothing bounced into it"
    );
    assert!(!app.project().any_frozen(), "nothing frozen in it");
}

#[test]
fn freeze_track_leaves_the_tracks_mute_and_solo_live() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("header");
    let synth = app.project().tracks[0].id;
    app.select_track(synth);
    app.run_action(Action::FreezeTrack);
    settle(&mut app, &ctx);
    assert!(app.freeze_holding_track(synth).is_some());
    for cmd in [
        Command::SetTrackMute {
            id: synth,
            muted: true,
        },
        Command::SetTrackSolo {
            id: synth,
            soloed: true,
        },
    ] {
        assert!(app.exec(cmd), "the header's M and S work");
    }
}

#[test]
fn saving_a_frozen_take_into_its_folder_keeps_it_frozen_and_its_render() {
    let ctx = support::ctx();
    let (mut app, dir) = app("collect");
    app.path = None;
    // A recorded take outside any project folder, on the synth's track.
    let take = dir.join("scratch").join("take 1.wav");
    std::fs::create_dir_all(take.parent().unwrap()).unwrap();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&take, spec).unwrap();
    for i in 0..48_000 {
        w.write_sample(if i % 100 == 0 { 0.5f32 } else { 0.0 })
            .unwrap();
    }
    w.finalize().unwrap();
    let mut p = app.project().clone();
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take 1".into(),
        path: take.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000,
        sha256: None,
        recorded: true,
    });
    let clip = p.make_audio_clip("take 1", id, Ticks::ZERO, Ticks::beats(2));
    p.tracks[0].clips.push(clip);
    app.bus = CommandBus::new(p);
    let strip = app.project().tracks[0].strip.unwrap();
    let target = FreezeTarget::Strip(strip);
    app.freeze_target(target);
    settle(&mut app, &ctx);
    assert!(app.project().frozen(target).is_some());
    let file = dir.join("saved").join("song.cantus");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    app.collect_audio(&file);
    let p = app.project();
    assert_eq!(p.audio_source(id).unwrap().path, "Audio/take 1.wav");
    assert!(file.with_file_name("Audio").join("take 1.wav").is_file());
    let f = p.frozen(target).expect("still frozen");
    assert_eq!(
        Some(f.content.clone()),
        cantus_core::freeze::content_key(p, target)
    );
    let render =
        cantus_engine::freeze::file_name(&cantus_engine::freeze::render_key(&f.content, 48_000));
    assert!(
        file.with_file_name("Freeze").join(render).is_file(),
        "its render is beside the project under its new key"
    );
}

#[test]
fn stop_stops_a_background_refresh_and_an_export_does_not_wait_for_one() {
    let ctx = support::ctx();
    let (mut app, dir) = app("refresh-stop");
    let strip = app.project().tracks[0].strip.unwrap();
    app.freeze_target(FreezeTarget::Strip(strip));
    settle(&mut app, &ctx);
    let frozen = app.project().clone();
    // Elsewhere, its render missing: a refresh starts by itself.
    let mut other = support::app(frozen);
    other.engine.frozen.set_app_dir(dir.join("elsewhere"));
    cantus_ui::freeze::tick(&mut other, &ctx);
    assert!(other.freeze.refreshing());
    // An export does not wait for it: it gives way.
    assert!(cantus_ui::freeze::yield_refresh(&mut other));
    assert!(other.freeze.job.is_none());
    // Stop: it is not made again until the user freezes or unfreezes.
    cantus_ui::freeze::tick(&mut other, &ctx);
    std::thread::sleep(Duration::from_millis(600));
    cantus_ui::freeze::tick(&mut other, &ctx);
    assert!(other.freeze.refreshing(), "asked again after the export");
    other.cancel_freeze();
    settle(&mut other, &ctx);
    std::thread::sleep(Duration::from_millis(600));
    cantus_ui::freeze::tick(&mut other, &ctx);
    assert!(!other.freeze.busy(), "Stop stops it");
}

#[test]
fn the_freeze_key_acts_on_what_was_chosen_last() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("key-strip");
    let synth = app.project().tracks[0].id;
    let bus = app.project().strips[6].id;
    // The bass plays into a bus.
    let mut p = app.project().clone();
    let bass_strip = p.tracks[1].strip.unwrap();
    p.strip_mut(bass_strip).unwrap().output = Some(bus);
    app.bus = CommandBus::new(p);
    app.select_track(synth);
    // A bus chosen on the desk after the track: the key freezes the bus.
    app.select_strip(bus);
    app.run_action(Action::FreezeTrack);
    settle(&mut app, &ctx);
    assert!(app.project().frozen(FreezeTarget::Strip(bus)).is_some());
    assert!(app
        .project()
        .frozen(FreezeTarget::Strip(app.project().tracks[0].strip.unwrap()))
        .is_none());
}

#[test]
fn the_bounce_keys_take_the_time_selection_made_last_and_every_tracks_clips() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("selection");
    let synth = app.project().tracks[0].id;
    let bass = app.project().tracks[1].id;
    // A clip clicked, then a range dragged in the ruler: the range is bounced.
    let clip = app.project().tracks[0].clips[0].id;
    app.select_clip(synth, clip);
    app.set_time_selection(Ticks::beats(4), Ticks::beats(6));
    app.run_action(Action::BounceSource);
    settle(&mut app, &ctx);
    let p = app.project();
    assert_eq!(
        p.tracks[1].clips[0].start,
        Ticks::beats(4),
        "the range, not the clip"
    );
    // Clips chosen on two tracks: both bounced.
    let (mut app, _dir) = self::app("selection-two");
    let a = app.project().tracks[0].clips[0].id;
    let b = app.project().tracks[1].clips[0].id;
    app.select_clip(synth, a);
    app.toggle_clip_selection(bass, b);
    app.run_action(Action::BounceSource);
    settle(&mut app, &ctx);
    let p = app.project();
    assert_eq!(p.tracks.len(), 4, "a bounce for each track");
    assert!(p.track(synth).unwrap().clips[0].muted && p.track(bass).unwrap().clips[0].muted);
}

#[test]
fn a_freeze_that_cannot_land_says_why() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("say");
    let synth = app.project().tracks[0].id;
    let strip = app.project().tracks[0].strip.unwrap();
    app.freeze_target(FreezeTarget::Strip(strip));
    // While it renders, the track is set to monitor its input always.
    let mut input = TrackInput::mono(0);
    input.monitor = MonitorMode::On;
    assert!(app.exec(Command::SetTrackInput {
        id: synth,
        input: Some(input)
    }));
    settle(&mut app, &ctx);
    assert!(app.project().frozen(FreezeTarget::Strip(strip)).is_none());
    assert!(app.status.error, "said, not dropped in silence");
}
