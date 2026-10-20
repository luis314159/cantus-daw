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
    within(a, b, 1e-6, what);
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
        // A note's tail before the cut and the bounce after it are summed apart (float
        // order: within −120 dB). Through the inserts too (review 2: the −60 dB this was
        // held to was the transport stopped at the end, now played on).
        let limit = 1e-6;
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
    assert!(app.freeze.job.is_some());
    assert_eq!(app.freeze.queued(), 0, "not asked again while it renders");
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
    assert!(
        app.status.error && app.status.text.contains("monitors its input"),
        "said, not dropped in silence: {}",
        app.status.text
    );
}

#[test]
fn a_render_that_needs_a_plugin_not_loaded_is_refused_and_says_which() {
    let ctx = support::ctx();
    let (mut app, dir) = app("missing-plugin");
    let strip = app.project().tracks[0].strip.unwrap();
    let mut p = app.project().clone();
    let mut e = p.make_effect(EffectKind::Delay);
    e.spec = EffectSpec::external(ExternalPlugin::new(
        PluginFormat::Clap,
        "org.nowhere.reverb",
        "Nowhere Reverb",
    ));
    p.strip_mut(strip).unwrap().effects.push(e);
    app.bus = CommandBus::new(p);
    app.freeze_target(FreezeTarget::Strip(strip));
    settle(&mut app, &ctx);
    assert!(app.project().frozen(FreezeTarget::Strip(strip)).is_none());
    assert!(
        app.status.error && app.status.text.contains("Nowhere Reverb"),
        "{}",
        app.status.text
    );
    // No render written under the true key.
    let written = |d: &std::path::Path| {
        std::fs::read_dir(d)
            .map(|r| {
                r.flatten()
                    .any(|e| e.path().extension().is_some_and(|x| x == "cfrz"))
            })
            .unwrap_or(false)
    };
    assert!(!written(&dir.join("renders")) && !written(&dir.join("Freeze")));
    // On a strip the render does not run (the bass's), it is not needed — nor made
    // (review 1: every plug-in of the project was instantiated for one freeze).
    let (mut app, _dir) = self::app("missing-elsewhere");
    let strip = app.project().tracks[0].strip.unwrap();
    let bass = app.project().tracks[1].strip.unwrap();
    let mut p = app.project().clone();
    let mut e = p.make_effect(EffectKind::Delay);
    e.spec = EffectSpec::external(ExternalPlugin::new(
        PluginFormat::Clap,
        "org.nowhere.reverb",
        "Nowhere Reverb",
    ));
    p.strip_mut(bass).unwrap().effects.push(e);
    app.bus = CommandBus::new(p);
    app.freeze_target(FreezeTarget::Strip(strip));
    settle(&mut app, &ctx);
    assert!(app.project().frozen(FreezeTarget::Strip(strip)).is_some());
}

#[test]
fn a_render_older_than_a_file_it_plays_is_made_again() {
    let ctx = support::ctx();
    let (mut app, dir) = app("older");
    let take = dir.join("take.wav");
    let write = |level: f32| {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut w = hound::WavWriter::create(&take, spec).unwrap();
        for i in 0..48_000 {
            w.write_sample(if i % 100 == 0 { level } else { 0.0 })
                .unwrap();
        }
        w.finalize().unwrap();
    };
    write(0.5);
    let mut p = app.project().clone();
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: take.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000,
        sha256: None,
        recorded: false,
    });
    let clip = p.make_audio_clip("take", id, Ticks::ZERO, Ticks::beats(2));
    p.tracks[0].clips.push(clip);
    app.bus = CommandBus::new(p);
    let strip = app.project().tracks[0].strip.unwrap();
    let target = FreezeTarget::Strip(strip);
    app.freeze_target(target);
    settle(&mut app, &ctx);
    app.unfreeze_target(target);
    // Edited elsewhere (louder), then frozen again: the render is not the file's now.
    std::thread::sleep(Duration::from_millis(1100));
    write(0.9);
    app.freeze_target(target);
    assert!(app.freeze.job.is_some(), "rendered again, not reused");
    settle(&mut app, &ctx);
    assert!(app.project().frozen(target).is_some());
}

#[test]
fn the_renders_folder_follows_the_project_before_its_first_compile() {
    let (mut app, dir) = app("follow");
    app.path = Some(dir.join("other").join("song.cantus"));
    app.sync_engine();
    assert_eq!(
        app.engine.frozen.project_dir(),
        Some(dir.join("other").join("Freeze").as_path())
    );
}

#[test]
fn half_written_files_are_tidied_and_the_cache_kept_to_its_size() {
    let dir = std::env::temp_dir()
        .join("cantus-freeze-ui")
        .join(format!("tidy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    for (name, len) in [
        ("a.raw.part", 10usize),
        ("b.part1234", 10),
        ("1.cfrz", 300),
        ("2.cfrz", 300),
        ("3.cfrz", 300),
    ] {
        let p = dir.join(name);
        std::fs::write(&p, vec![0u8; len]).unwrap();
        let f = std::fs::File::options().write(true).open(&p).unwrap();
        f.set_modified(old + Duration::from_secs(len as u64 + name.len() as u64))
            .unwrap();
    }
    cantus_ui::freeze::tidy(&dir);
    assert!(!dir.join("a.raw.part").exists() && !dir.join("b.part1234").exists());
    cantus_ui::freeze::prune(&dir, 650);
    let left = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(left, 2, "the oldest render went");
}

#[test]
fn a_modulator_bake_waits_for_a_freeze_rendering() {
    let (mut app, _dir) = app("bake");
    let synth_strip = app.project().tracks[0].strip.unwrap();
    let bass_strip = app.project().tracks[1].strip.unwrap();
    let mut p = app.project().clone();
    let mut f = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Follower);
    if let ModSource::Follower(src) = &mut f.source {
        src.listen = cantus_core::modulator::Listen::Strip(synth_strip);
    }
    let target = AutoTarget::StripPan(bass_strip);
    f.routes.push(ModRoute {
        target: target.clone(),
        depth: 0.4,
    });
    let id = f.id;
    p.strip_mut(bass_strip).unwrap().modulators.push(f);
    app.bus = CommandBus::new(p);
    app.freeze_target(FreezeTarget::Strip(synth_strip));
    assert!(app.freeze.job.is_some());
    assert!(!app.bake_route(id, &target), "it waits");
    assert!(app.status.error);
}

/// The samples of the bounce on track `index`.
fn bounced(app: &CantusApp, index: usize) -> Vec<f32> {
    let p = app.project();
    let a = p.tracks[index].clips[0]
        .audio
        .as_ref()
        .expect("an audio clip");
    let s = p.audio_source(a.source).unwrap();
    let path = cantus_ui::waveform::resolve(app.audio_base(), &s.path);
    hound::WavReader::open(path)
        .unwrap()
        .samples::<f32>()
        .map(|x| x.unwrap())
        .collect()
}

#[test]
fn a_track_that_shares_the_strip_for_its_notes_does_not_leak_into_a_bounce() {
    let ctx = support::ctx();
    let make = |name: &str, on_strip: bool| {
        let (mut app, dir) = app(name);
        let strip = app.project().tracks[0].strip.unwrap();
        let mut p = app.project().clone();
        // An insert on the synth's strip hears the bass's notes; the bass plays into the
        // synth's strip too, or to the master.
        let bass = p.tracks[1].id;
        p.strip_mut(strip).unwrap().effects[0].midi_from = Some(bass);
        p.tracks[1].strip = if on_strip { Some(strip) } else { None };
        app.bus = CommandBus::new(p);
        let synth = app.project().tracks[0].id;
        bounce(
            &mut app,
            &ctx,
            Plan {
                track: synth,
                through: Through::Inserts,
                range: Range::Track,
            },
        );
        (bounced(&app, 1), dir)
    };
    let (shared, _a) = make("leak-shared", true);
    let (apart, _b) = make("leak-apart", false);
    same_bits(&shared, &apart, "the synth through its inserts alone");
}

// ---- review 2 ---------------------------------------------------------------------------

/// The render files in `dir`.
fn renders_in(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "cfrz"))
                .collect()
        })
        .unwrap_or_default()
}

/// A track playing an audio file of its own, written at `path`.
fn with_take(app: &mut CantusApp, path: &std::path::Path) -> AudioSourceId {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..48_000 {
        w.write_sample(if i % 100 == 0 { 0.5f32 } else { 0.0 })
            .unwrap();
    }
    w.finalize().unwrap();
    let mut p = app.project().clone();
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000,
        sha256: None,
        recorded: false,
    });
    let clip = p.make_audio_clip("take", id, Ticks::ZERO, Ticks::beats(2));
    p.tracks[0].clips.push(clip);
    app.bus = CommandBus::new(p);
    id
}

#[test]
fn a_missing_file_refuses_a_freeze_and_a_refresh_waits_for_it() {
    let ctx = support::ctx();
    let (mut app, dir) = app("missing-file");
    let take = dir.join("drive").join("take.wav");
    with_take(&mut app, &take);
    let strip = app.project().tracks[0].strip.unwrap();
    let target = FreezeTarget::Strip(strip);
    let kept = dir.join("kept.wav");
    std::fs::rename(&take, &kept).unwrap();
    app.freeze_target(target);
    settle(&mut app, &ctx);
    assert!(app.project().frozen(target).is_none());
    assert!(
        app.status.error && app.status.text.contains("take.wav"),
        "{}",
        app.status.text
    );
    assert!(
        renders_in(&dir.join("Freeze")).is_empty() && renders_in(&dir.join("renders")).is_empty()
    );
    // Frozen with the file there; opened elsewhere without it: it waits, never failed.
    std::fs::rename(&kept, &take).unwrap();
    app.freeze_target(target);
    settle(&mut app, &ctx);
    assert!(
        app.project().frozen(target).is_some(),
        "{}",
        app.status.text
    );
    let mut other = support::app(app.project().clone());
    other.engine.frozen.set_app_dir(dir.join("elsewhere"));
    std::fs::rename(&take, &kept).unwrap();
    settle(&mut other, &ctx);
    assert!(
        renders_in(&dir.join("elsewhere")).is_empty(),
        "no render of silence"
    );
    std::fs::rename(&kept, &take).unwrap();
    std::thread::sleep(Duration::from_millis(600));
    settle(&mut other, &ctx);
    assert_eq!(
        renders_in(&dir.join("elsewhere")).len(),
        1,
        "made once the file is back"
    );
}

#[test]
fn freezing_an_armed_track_renders_once_under_the_key_it_keeps() {
    let ctx = support::ctx();
    let (mut app, dir) = app("armed");
    let synth = app.project().tracks[0].id;
    let strip = app.project().tracks[0].strip.unwrap();
    assert!(app.exec(Command::SetTrackArmed {
        id: synth,
        armed: true
    }));
    app.freeze_target(FreezeTarget::Strip(strip));
    settle(&mut app, &ctx);
    let content = app
        .project()
        .frozen(FreezeTarget::Strip(strip))
        .unwrap()
        .content
        .clone();
    let renders = renders_in(&dir.join("Freeze"));
    assert_eq!(renders.len(), 1, "{renders:?}");
    assert!(app.engine.frozen.find(&content, 48_000).is_some());
}

#[test]
fn a_strip_resting_inside_a_playing_render_is_not_refreshed_until_it_wakes() {
    let ctx = support::ctx();
    let (mut app, dir) = app("nested-refresh");
    // The synth's strip plays into the bass's strip, a bus; both frozen.
    let inner = app.project().tracks[0].strip.unwrap();
    let outer = app.project().tracks[1].strip.unwrap();
    let mut p = app.project().clone();
    p.strip_mut(inner).unwrap().output = Some(outer);
    p.strip_mut(inner).unwrap().sends.clear();
    app.bus = CommandBus::new(p);
    app.freeze_target(FreezeTarget::Strip(inner));
    settle(&mut app, &ctx);
    app.freeze_target(FreezeTarget::Strip(outer));
    settle(&mut app, &ctx);
    // Elsewhere only the bus's render is at hand.
    let elsewhere = dir.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let content = |p: &Project, s| p.frozen(FreezeTarget::Strip(s)).unwrap().content.clone();
    let from = app
        .engine
        .frozen
        .path_of(&content(app.project(), outer), 48_000)
        .unwrap();
    std::fs::copy(&from, elsewhere.join(from.file_name().unwrap())).unwrap();
    let mut other = support::app(app.project().clone());
    other.engine.frozen.set_app_dir(elsewhere.clone());
    settle(&mut other, &ctx);
    assert_eq!(
        renders_in(&elsewhere).len(),
        1,
        "the strip resting inside is not made"
    );
    // Thawed outside, it wakes: its render is made.
    other.unfreeze_target(FreezeTarget::Strip(outer));
    std::thread::sleep(Duration::from_millis(600));
    settle(&mut other, &ctx);
    assert!(other
        .engine
        .frozen
        .find(&content(other.project(), inner), 48_000)
        .is_some());
}

#[test]
fn saving_elsewhere_brings_the_renders_and_keeps_only_those_wanted() {
    let ctx = support::ctx();
    let (mut app, dir) = app("save-as");
    let strip = app.project().tracks[0].strip.unwrap();
    let target = FreezeTarget::Strip(strip);
    app.freeze_target(target);
    settle(&mut app, &ctx);
    let first = app.project().frozen(target).unwrap().content.clone();
    // A render nothing wants, beside the project.
    std::fs::write(
        dir.join("Freeze").join(format!("{}.cfrz", "0".repeat(64))),
        b"x",
    )
    .unwrap();
    let file = dir.join("moved").join("song.cantus");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    app.start_save(&file);
    app.poll_save(true);
    let there = renders_in(&file.with_file_name("Freeze"));
    assert_eq!(there.len(), 1, "its render came along: {there:?}");
    // Saved where it was: the stray render goes, the frozen one stays.
    let again = dir.join("song.cantus");
    app.start_save(&again);
    app.poll_save(true);
    let kept = renders_in(&dir.join("Freeze"));
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(app.engine.frozen.path_of(&first, 48_000).is_some());
}

#[test]
fn half_written_bounce_files_are_tidied() {
    let dir = std::env::temp_dir()
        .join("cantus-freeze-ui")
        .join(format!("tidy-bounce-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    for name in [
        "Synth bounce.wav.part",
        "Synth bounce.raw.part",
        "take.wav.part",
        "Synth bounce.wav",
    ] {
        let p = dir.join(name);
        std::fs::write(&p, b"x").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(old)
            .unwrap();
    }
    cantus_ui::freeze::tidy_bounces(&dir);
    let mut left: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, vec!["Synth bounce.wav", "take.wav.part"]);
    // And where the session does it: the project's `Audio` folder, when its path is taken.
    let ctx = support::ctx();
    let (mut app, project) = self::app("tidy-bounce-session");
    let audio = project.join("Audio");
    std::fs::create_dir_all(&audio).unwrap();
    let part = audio.join("Bass bounce.wav.part");
    std::fs::write(&part, b"x").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&part)
        .unwrap()
        .set_modified(old)
        .unwrap();
    cantus_ui::freeze::tick(&mut app, &ctx);
    assert!(
        !part.exists(),
        "a session cut short leaves no partial bounce"
    );
}

#[test]
fn a_strip_bounce_refuses_a_solo_inside_it() {
    let (mut app, _dir) = app("bounce-refuses-solo");
    let bass = app.project().tracks[1].id;
    let bass_strip = app.project().tracks[1].strip.unwrap();
    assert!(app.exec(Command::SetTrackSolo {
        id: bass,
        soloed: true
    }));
    app.bounce_strip(bass_strip);
    assert!(
        app.status.error && app.status.text.contains("Bass"),
        "{}",
        app.status.text
    );
    assert!(!app.freeze.busy());
}

#[test]
fn a_bounce_refuses_a_live_input() {
    let (mut app, _dir) = app("bounce-refuses-input");
    let synth = app.project().tracks[0].id;
    assert!(app.exec(Command::SetTrackArmed {
        id: synth,
        armed: true
    }));
    app.bounce(Plan {
        track: synth,
        through: Through::Source,
        range: Range::Track,
    });
    assert!(
        app.status.error && !app.freeze.busy(),
        "{}",
        app.status.text
    );
}

#[test]
fn a_range_bounce_refuses_clips_whose_notes_are_heard() {
    let (mut app, _dir) = app("bounce-refuses-notes");
    let synth = app.project().tracks[0].id;
    let bass_strip = app.project().tracks[1].strip.unwrap();
    // Its clips' notes heard by an insert elsewhere: a range would mute them.
    let mut p = app.project().clone();
    let mut gate = p.make_effect(EffectKind::Gate);
    gate.midi_from = Some(synth);
    p.strip_mut(bass_strip).unwrap().effects.push(gate);
    app.bus = CommandBus::new(p);
    app.bounce(Plan {
        track: synth,
        through: Through::Source,
        range: Range::Time(Ticks::beats(2), Ticks::beats(4)),
    });
    assert!(
        app.status.error && !app.freeze.busy(),
        "{}",
        app.status.text
    );
}

#[test]
fn a_bounce_through_latency_keeps_its_rides_and_keys_in_time() {
    let ctx = support::ctx();
    for strip_mode in [false, true] {
        let (mut app, _dir) = app(&format!("late-ride-{strip_mode}"));
        let synth = app.project().tracks[0].id;
        let strip = app.project().tracks[0].strip.unwrap();
        let bass_strip = app.project().tracks[1].strip.unwrap();
        let mut p = app.project().clone();
        // A linear-phase EQ on the synth's strip, a fader ride on it, and the bass's
        // compressor keyed from it.
        let mut eq = p.make_effect(EffectKind::Eq);
        eq.spec.set("mode", 2.0).unwrap();
        p.strip_mut(strip).unwrap().effects.push(eq);
        let ride = p.make_automation_clip(
            "ride",
            AutoTarget::StripGain(strip),
            Ticks::ZERO,
            Ticks::beats(8),
            vec![
                automation::AutoPoint::new(Ticks::ZERO, 0.9),
                automation::AutoPoint::new(Ticks::beats(8), 0.2),
            ],
        );
        p.tracks[1].clips.push(ride);
        let mut comp = p.make_effect(EffectKind::Compressor);
        comp.sidechain = Some(strip);
        p.strip_mut(bass_strip).unwrap().effects.push(comp);
        app.bus = CommandBus::new(p);
        let live = mix(&mut app);
        if strip_mode {
            app.bounce_strip(strip);
        } else {
            app.bounce(Plan {
                track: synth,
                through: Through::Inserts,
                range: Range::Track,
            });
        }
        settle(&mut app, &ctx);
        assert_eq!(
            app.project().tracks.len(),
            3,
            "bounced: {}",
            app.status.text
        );
        assert!(
            app.project().tracks[1].lateness.is_some(),
            "it keeps the chain's latency"
        );
        let after = mix(&mut app);
        same_bits(&after[..live.len()], &live, &format!("strip {strip_mode}"));
    }
}

#[test]
fn a_bounce_of_a_clip_late_in_the_song_plays_a_free_lfo_as_the_song_does() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("free-lfo");
    let synth = app.project().tracks[0].id;
    let delay = app.project().strips[0].effects[0].id;
    let strip = app.project().tracks[0].strip.unwrap();
    let mut p = app.project().clone();
    // The synth's clip starts at beat 16; a free LFO (in hertz) moves its strip's delay.
    p.tracks[0].clips[0].start = Ticks::beats(16);
    let mut lfo = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
    if let ModSource::Lfo(l) = &mut lfo.source {
        l.rate = cantus_core::modulator::ModRate::Hz { hz: 0.3 };
    }
    lfo.routes.push(ModRoute {
        target: AutoTarget::Effect {
            effect: delay,
            key: "feedback".into(),
        },
        depth: 0.5,
    });
    p.strip_mut(strip).unwrap().modulators.push(lfo);
    app.bus = CommandBus::new(p);
    let live = mix(&mut app);
    bounce(
        &mut app,
        &ctx,
        Plan {
            track: synth,
            through: Through::Inserts,
            range: Range::Track,
        },
    );
    let after = mix(&mut app);
    same_bits(&after[..live.len()], &live, "the LFO's phase");
}

#[test]
fn an_edit_elsewhere_while_a_bounce_renders_does_not_make_it_again() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("edit-elsewhere");
    let synth = app.project().tracks[0].id;
    let bass_strip = app.project().tracks[1].strip.unwrap();
    app.bounce(Plan {
        track: synth,
        through: Through::Source,
        range: Range::Track,
    });
    assert!(app.exec(Command::SetStripGain {
        id: bass_strip,
        gain_db: -4.0
    }));
    // Every frame watched: a bounce made again says so when its first render lands (and
    // the second one's "done" would hide it after).
    let again = cantus_ui::i18n::t("bounce.again");
    let t0 = Instant::now();
    let mut said = Vec::new();
    while app.freeze.busy() && t0.elapsed() < Duration::from_secs(120) {
        app.sync_engine();
        cantus_ui::freeze::tick(&mut app, &ctx);
        said.push(app.status.text.clone());
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(&mut app, &ctx);
    assert_eq!(app.project().tracks.len(), 3);
    assert!(!said.contains(&again), "made again for an edit elsewhere");
}

#[test]
fn removing_an_original_whose_notes_are_heard_says_why_with_the_result() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("kept-why");
    let synth = app.project().tracks[0].id;
    let bass_strip = app.project().tracks[1].strip.unwrap();
    let mut p = app.project().clone();
    let mut gate = p.make_effect(EffectKind::Gate);
    gate.midi_from = Some(synth);
    p.strip_mut(bass_strip).unwrap().effects.push(gate);
    app.bus = CommandBus::new(p);
    app.settings.bounce_removes = true;
    bounce(
        &mut app,
        &ctx,
        Plan {
            track: synth,
            through: Through::Source,
            range: Range::Track,
        },
    );
    assert!(app.project().track(synth).is_some_and(|t| t.muted));
    assert!(app.status.text.contains("notes"), "{}", app.status.text);
}

#[test]
fn the_bounce_keys_take_a_chosen_track_whole() {
    let ctx = support::ctx();
    // A track header chosen: the whole track (its two clips).
    let (mut app, _dir) = app("key-track");
    let synth = app.project().tracks[0].id;
    let mut p = app.project().clone();
    let extra = p.make_clip("more", Ticks::beats(8), Ticks::beats(4));
    p.tracks[0].clips.push(extra);
    app.bus = CommandBus::new(p);
    app.select_track(synth);
    app.run_action(Action::BounceSource);
    settle(&mut app, &ctx);
    assert!(app.project().track(synth).unwrap().muted, "the whole track");
}

#[test]
fn the_bounce_keys_take_a_chosen_strip() {
    let ctx = support::ctx();
    // A strip chosen on the desk (after a track): that strip.
    let (mut app, _dir) = self::app("key-strip-bounce");
    let bass_strip = app.project().tracks[1].strip.unwrap();
    app.select_track(app.project().tracks[0].id);
    app.select_strip(bass_strip);
    app.run_action(Action::BounceInPlace);
    settle(&mut app, &ctx);
    assert!(
        app.project().strip(bass_strip).unwrap().muted,
        "the strip bounced"
    );
}

#[test]
fn the_freeze_and_bounce_keys_never_take_the_master() {
    let ctx = support::ctx();
    // The master chosen after a track: nothing frozen or bounced (review 2: the track
    // chosen before was frozen).
    let (mut app, _dir) = self::app("key-master");
    app.select_track(app.project().tracks[0].id);
    app.select_master();
    let before = app.project().clone();
    app.run_action(Action::FreezeTrack);
    app.run_action(Action::BounceInPlace);
    settle(&mut app, &ctx);
    assert_eq!(app.project(), &before);
}

#[test]
fn stop_pressed_while_its_file_is_written_commits_nothing() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("stop-late");
    let synth = app.project().tracks[0].id;
    app.bounce(Plan {
        track: synth,
        through: Through::Source,
        range: Range::Track,
    });
    // The render done, its file written: then Stop, before the frame collects it.
    let t0 = Instant::now();
    while app
        .freeze
        .job
        .as_ref()
        .is_some_and(|j| j.progress.fraction() < 1.0)
        && t0.elapsed() < Duration::from_secs(120)
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(300));
    app.cancel_freeze();
    settle(&mut app, &ctx);
    assert_eq!(app.project().tracks.len(), 2, "nothing laid back");
}

#[test]
fn nothing_starts_once_the_session_closes() {
    let ctx = support::ctx();
    let (mut app, dir) = app("closing");
    let strip = app.project().tracks[0].strip.unwrap();
    app.freeze_target(FreezeTarget::Strip(strip));
    settle(&mut app, &ctx);
    let mut other = support::app(app.project().clone());
    other.engine.frozen.set_app_dir(dir.join("elsewhere"));
    cantus_ui::freeze::close(&mut other);
    std::thread::sleep(Duration::from_millis(600));
    cantus_ui::freeze::tick(&mut other, &ctx);
    assert!(!other.freeze.busy(), "no refresh started while closing");
}

#[test]
fn a_bypassed_missing_plugin_does_not_stop_a_freeze() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("bypassed-missing");
    let strip = app.project().tracks[0].strip.unwrap();
    let mut p = app.project().clone();
    let mut e = p.make_effect(EffectKind::Delay);
    e.spec = EffectSpec::external(ExternalPlugin::new(
        PluginFormat::Clap,
        "org.nowhere.reverb",
        "Nowhere Reverb",
    ));
    e.bypass = true;
    p.strip_mut(strip).unwrap().effects.push(e);
    app.bus = CommandBus::new(p);
    app.freeze_target(FreezeTarget::Strip(strip));
    settle(&mut app, &ctx);
    assert!(
        app.project().frozen(FreezeTarget::Strip(strip)).is_some(),
        "{}",
        app.status.text
    );
}

#[test]
fn a_freeze_waiting_behind_a_bake_shows_its_card_and_can_be_stopped() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("waiting");
    let synth_strip = app.project().tracks[0].strip.unwrap();
    let bass_strip = app.project().tracks[1].strip.unwrap();
    let mut p = app.project().clone();
    let mut f = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Follower);
    if let ModSource::Follower(src) = &mut f.source {
        src.listen = cantus_core::modulator::Listen::Strip(synth_strip);
    }
    let target = AutoTarget::StripPan(bass_strip);
    f.routes.push(ModRoute {
        target: target.clone(),
        depth: 0.4,
    });
    let id = f.id;
    p.strip_mut(bass_strip).unwrap().modulators.push(f);
    app.bus = CommandBus::new(p);
    assert!(app.bake_route(id, &target), "a bake runs");
    app.freeze_target(FreezeTarget::Strip(synth_strip));
    cantus_ui::freeze::tick(&mut app, &ctx);
    let (title, _, buttons, _) = cantus_ui::views::freeze_view::card(&app).expect("a card");
    assert!(title.contains("Synth"), "{title}");
    assert!(buttons
        .iter()
        .any(|(_, a)| *a == cantus_ui::views::freeze_view::Answer::Stop));
    cantus_ui::views::freeze_view::answer(&mut app, cantus_ui::views::freeze_view::Answer::Stop);
    assert!(!app.freeze.busy(), "stopped");
}

#[test]
fn the_words_say_what_stays_live() {
    let mut p = song();
    let strip = p.tracks[0].strip.unwrap();
    let delay = p.strips[0].effects[0].id;
    let plain =
        cantus_ui::views::freeze_view::live_words(&p, FreezeTarget::Strip(strip), "freeze.done");
    assert_eq!(plain, "freeze.done");
    let t = p.tracks[0].id;
    assert_eq!(
        cantus_ui::views::freeze_view::live_words(&p, FreezeTarget::Track(t), "freeze.done"),
        "freeze.done_track"
    );
    // A follower of its own strip: its fader is part of its sound.
    let mut f = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Follower);
    if let ModSource::Follower(src) = &mut f.source {
        src.listen = cantus_core::modulator::Listen::Host;
    }
    f.routes.push(ModRoute {
        target: AutoTarget::Effect {
            effect: delay,
            key: "feedback".into(),
        },
        depth: 0.4,
    });
    p.strip_mut(strip).unwrap().modulators.push(f);
    assert_eq!(
        cantus_ui::views::freeze_view::live_words(
            &p,
            FreezeTarget::Strip(strip),
            "freeze.card.refused_body"
        ),
        "freeze.card.refused_body_heard"
    );
}

#[test]
fn rides_and_modulators_on_the_original_go_with_its_bounce() {
    let ctx = support::ctx();
    for through in [Through::Source, Through::Inserts] {
        for remove in [false, true] {
            let what = format!("{through:?}, remove {remove}");
            let (mut app, _dir) = app(&format!("own-rides-{through:?}-{remove}"));
            let synth = app.project().tracks[0].id;
            let strip = app.project().tracks[0].strip.unwrap();
            let delay = app.project().strips[0].effects[0].id;
            let mut p = app.project().clone();
            // On the synth's own lanes: a level ride and a fader ride.
            for target in [AutoTarget::TrackGain(synth), AutoTarget::StripGain(strip)] {
                let ride = p.make_automation_clip(
                    "ride",
                    target,
                    Ticks::ZERO,
                    Ticks::beats(8),
                    vec![
                        automation::AutoPoint::new(Ticks::ZERO, 0.9),
                        automation::AutoPoint::new(Ticks::beats(8), 0.3),
                    ],
                );
                p.tracks[0].clips.push(ride);
            }
            // Hosted on the synth: an LFO on its strip's pan and one on the delay.
            for target in [
                AutoTarget::StripPan(strip),
                AutoTarget::Effect {
                    effect: delay,
                    key: "feedback".into(),
                },
            ] {
                let mut lfo = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
                lfo.routes.push(ModRoute { target, depth: 0.3 });
                p.tracks[0].modulators.push(lfo);
            }
            app.bus = CommandBus::new(p);
            let live = mix(&mut app);
            app.settings.bounce_removes = remove;
            bounce(
                &mut app,
                &ctx,
                Plan {
                    track: synth,
                    through,
                    range: Range::Track,
                },
            );
            assert!(
                app.freeze.job.is_none() && app.project().tracks.len() >= 2,
                "{what}"
            );
            assert_eq!(app.project().track(synth).is_none(), remove, "{what}");
            let after = mix(&mut app);
            same_bits(&after[..live.len()], &live, &what);
        }
    }
}

#[test]
fn a_damaged_render_is_made_again_in_the_session() {
    let ctx = support::ctx();
    let (mut app, dir) = app("damaged-ui");
    let strip = app.project().tracks[0].strip.unwrap();
    let target = FreezeTarget::Strip(strip);
    app.freeze_target(target);
    settle(&mut app, &ctx);
    let content = app.project().frozen(target).unwrap().content.clone();
    let path = app.engine.frozen.path_of(&content, 48_000).unwrap();
    // Elsewhere, its render damaged two seconds in.
    let elsewhere = dir.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() > 900_000);
    for b in &mut bytes[800_000..804_096] {
        *b ^= 0x5a;
    }
    std::fs::write(elsewhere.join(path.file_name().unwrap()), &bytes).unwrap();
    let mut other = support::app(app.project().clone());
    other.engine.frozen.set_app_dir(elsewhere);
    other.sync_engine();
    // An export reads the damaged blocks: it fails, the render is missing from then on, and
    // the session makes it again.
    let snap = other.engine.compile_for_render(&other.project().clone(), 1);
    assert!(snap.strips[0].frozen.as_ref().unwrap().plays());
    assert!(render(
        snap,
        &RenderOptions {
            tail_seconds: 0.2,
            ..Default::default()
        },
    )
    .is_err());
    std::thread::sleep(Duration::from_millis(600));
    cantus_ui::freeze::tick(&mut other, &ctx);
    assert!(other.freeze.refreshing(), "made again");
}

#[test]
fn the_chip_counts_the_input_only_for_an_audio_input() {
    let device = cantus_engine::DeviceLatency {
        input: 300,
        output: 280,
    };
    let mut ll = cantus_engine::LowLatency {
        on: true,
        monitoring: true,
        audio: true,
        monitored_samples: 128,
        ..Default::default()
    };
    let ms = |ll: &cantus_engine::LowLatency| {
        cantus_ui::views::freeze_monitor::monitored_figure(device, 40.0, ll, 48_000.0)
    };
    assert_eq!(
        ms(&ll),
        Some((300.0 + 280.0 + 40.0 + 128.0) * 1000.0 / 48_000.0)
    );
    ll.audio = false;
    assert_eq!(
        ms(&ll),
        Some((280.0 + 128.0) * 1000.0 / 48_000.0),
        "notes go out only"
    );
}
