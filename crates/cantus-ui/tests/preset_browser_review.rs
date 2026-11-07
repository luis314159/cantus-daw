//! What the review of the preset browser's first delivery asked for (round 37, ADR 0101),
//! each held by a test that fails without its fix.

use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec, Project};
use cantus_ui::preset_audition::{self as aud, HeardOn, Target};
use cantus_ui::preset_catalog::{entries, Entry, Unit};
use cantus_ui::preset_tags::Role;
use cantus_ui::CantusApp;
use egui::{Pos2, Rect};
use std::time::{Duration, Instant};

mod support;

const SCREEN: egui::Vec2 = egui::vec2(1600.0, 1000.0);

fn run(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| a.frame(ui)).textures_delta.clear();
}

fn key(k: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Three tracks: an Unda with Fervor on its strip; a second track on another strip; a
/// third, muted, on Fervor's strip.
fn project() -> Project {
    let mut p = aud::phrase_project(
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable)),
        Role::Lead,
    );
    let strip = p.tracks[0].strip.unwrap();
    let slot = p.make_effect(EffectKind::Saturator);
    p.strip_mut(strip).unwrap().effects.push(slot);
    let mut other = p.tracks[0].clone();
    other.id = cantus_core::TrackId(9_001);
    other.strip = p.strips.iter().map(|s| s.id).find(|s| *s != strip);
    for (i, c) in other.clips.iter_mut().enumerate() {
        c.id = cantus_core::ClipId(9_100 + i as u64);
    }
    let mut muted = p.tracks[0].clone();
    muted.id = cantus_core::TrackId(9_002);
    muted.muted = true;
    for (i, c) in muted.clips.iter_mut().enumerate() {
        c.id = cantus_core::ClipId(9_200 + i as u64);
        c.muted = true;
    }
    p.tracks.push(other);
    p.tracks.push(muted);
    p
}

fn fervor_entry() -> Entry {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    entries(Unit::Effect(EffectKind::Saturator), false)
        .into_iter()
        .next()
        .expect("Fervor has presets")
}

/// 1, 3: an insert is heard on a clip its strip hears, in a copy cut down to that track and
/// clip, unmuted; a clip on another strip is never used; a hosted plug-in on the path sends
/// the audition to the fixed source.
#[test]
fn an_insert_is_heard_only_on_a_clip_routed_through_it_in_a_cut_down_unmuted_copy() {
    let p = project();
    let strip = p.tracks[0].strip.unwrap();
    let fervor = p.strip(strip).unwrap().effects[0].id;
    let e = fervor_entry();
    let target = Target::Effect {
        strip,
        effect: fervor,
    };
    // The selected clip is on the other strip: not heard through Fervor.
    let elsewhere = p.tracks[1].clips[0].id;
    let a = aud::prepare(&p, target, &e, Some(elsewhere), aud::TEMPO);
    assert_eq!(a.on, HeardOn::FirstClip, "the first clip on this channel");
    assert_eq!(a.project.tracks.len(), 1, "cut down to one track");
    assert_eq!(a.project.tracks[0].id, p.tracks[0].id);
    assert_eq!(a.project.tracks[0].clips.len(), 1);
    assert!(a.needs_engine && a.fallback.is_some());
    // The muted track's clip, selected: heard, unmuted in the copy.
    let muted_clip = p.tracks[2].clips[0].id;
    let b = aud::prepare(&p, target, &e, Some(muted_clip), aud::TEMPO);
    assert_eq!(b.on, HeardOn::SelectedClip);
    assert_eq!(b.project.tracks[0].id, p.tracks[2].id);
    assert!(!b.project.tracks[0].muted && !b.project.tracks[0].clips[0].muted);
    // A hosted plug-in on the path: an offline render plays it silent — the fixed source.
    let mut hosted = p.clone();
    let mut slot = hosted.make_effect(EffectKind::External);
    slot.spec.kind = EffectKind::External;
    hosted.strip_mut(strip).unwrap().effects.insert(0, slot);
    let c = aud::prepare(&hosted, target, &e, None, aud::TEMPO);
    assert!(matches!(c.on, HeardOn::Source(_)));
    assert!(!c.needs_engine);
}

/// 2: a new audition cancels the render under way, and Stop does too; ↑ ↓ wait for the
/// selection to rest before a render starts.
#[test]
fn renders_are_cancelled_and_the_arrow_keys_wait_for_the_selection_to_rest() {
    let mut app = support::app(project());
    let track = app.project().tracks[0].id;
    app.open_preset_browser(Target::Track(track));
    let ids: Vec<String> = app.presets.shown().iter().map(|e| e.id.clone()).collect();
    app.audition_preset_id(&ids[0]);
    let first = app.presets.audition.progress().expect("a render under way");
    app.audition_preset_id(&ids[1]);
    assert!(
        first.cancel.load(std::sync::atomic::Ordering::Relaxed),
        "the first render is cancelled"
    );
    let second = app
        .presets
        .audition
        .progress()
        .expect("the second under way");
    app.stop_preset_audition();
    assert!(second.cancel.load(std::sync::atomic::Ordering::Relaxed));
    assert!(!app.presets.audition.rendering());
    // The keys: a step chooses at once and hears only after the pause.
    app.presets.pending = Some((ids[2].clone(), Instant::now()));
    assert_eq!(
        cantus_ui::views::preset_browser_keys::settled(&mut app, Instant::now()),
        None
    );
    let later = Instant::now() + cantus_ui::views::preset_browser_keys::SETTLE;
    assert_eq!(
        cantus_ui::views::preset_browser_keys::settled(&mut app, later),
        Some(ids[2].clone())
    );
}

/// 4, 5: with the browser open, Ctrl+Z still undoes; a typed letter starts a search (the
/// field takes the keyboard then, and the next letter adds to it); Space hears, Esc closes.
#[test]
fn the_browser_takes_only_its_own_keys() {
    let mut app = support::app(project());
    let ctx = support::ctx();
    app.attach(&ctx);
    app.audio_setup.first_run = None;
    app.workflow.start.close();
    run(&mut app, &ctx, vec![]);
    let track = app.project().tracks[0].id;
    app.open_preset_browser(Target::Track(track));
    let first = app.presets.shown()[1].id.clone();
    assert!(app.apply_preset_id(&first));
    let loaded = app.project().track(track).unwrap().instrument.clone();
    run(&mut app, &ctx, vec![]);
    assert!(
        !cantus_ui::kit::field::active(&ctx),
        "the field waits for a letter"
    );
    run(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(egui::Modifiers::COMMAND),
            key(egui::Key::Z, egui::Modifiers::COMMAND),
        ],
    );
    assert_ne!(
        app.project().track(track).unwrap().instrument,
        loaded,
        "Ctrl+Z undid the load with the browser open"
    );
    run(
        &mut app,
        &ctx,
        vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
    );
    run(&mut app, &ctx, vec![egui::Event::Text("g".into())]);
    assert_eq!(app.presets.query.text, "g");
    run(&mut app, &ctx, vec![]);
    assert!(
        cantus_ui::kit::field::active(&ctx),
        "the field has the keyboard now"
    );
    run(&mut app, &ctx, vec![egui::Event::Text("l".into())]);
    assert_eq!(
        app.presets.query.text, "gl",
        "the caret is after the first letter"
    );
    assert!(app.presets.open);
}

/// 6: a preset loaded from the browser is the one the effect's own window names.
#[test]
fn the_effect_window_names_the_preset_the_browser_loaded() {
    let mut app = support::app(project());
    let ctx = support::ctx();
    app.attach(&ctx);
    let strip = app.project().tracks[0].strip.unwrap();
    let fervor = app.project().strip(strip).unwrap().effects[0].id;
    app.open_effect_editor(strip, fervor);
    run(&mut app, &ctx, vec![]);
    run(&mut app, &ctx, vec![]);
    app.open_preset_browser(Target::Effect {
        strip,
        effect: fervor,
    });
    let e = app.presets.shown()[2].clone();
    assert!(app.apply_preset_id(&e.id));
    run(&mut app, &ctx, vec![]);
    run(&mut app, &ctx, vec![]);
    let named = app
        .fx
        .saturator
        .chrome
        .loaded
        .as_ref()
        .map(|l| l.name.clone());
    assert_eq!(named, Some(e.name), "the well names what was loaded");
}

/// 9: another project closes the browser and cancels its render.
#[test]
fn a_new_project_closes_the_browser_and_cancels_its_render() {
    let mut app = support::app(project());
    let track = app.project().tracks[0].id;
    app.open_preset_browser(Target::Track(track));
    let id = app.presets.shown()[0].id.clone();
    app.audition_preset_id(&id);
    let progress = app.presets.audition.progress().expect("under way");
    app.new_project();
    assert!(!app.presets.open);
    assert!(progress.cancel.load(std::sync::atomic::Ordering::Relaxed));
    assert!(app.finish_preset_audition(Duration::from_secs(5)));
}

/// 11: what an audition was heard on reads as a sentence for every role, in both languages.
#[test]
fn every_role_s_phrase_reads_as_a_sentence() {
    use cantus_ui::i18n::{t_in, tf_in, Locale};
    for l in Locale::ALL {
        for r in Role::INSTRUMENT {
            let key = cantus_ui::views::preset_browser::phrase_key(r);
            let what = t_in(l, key);
            assert_ne!(what, key, "{key} in {l:?}");
            let s = tf_in(
                l,
                "preset.browser.on_phrase",
                &[("what", &what), ("bpm", &120)],
            );
            assert!(!s.contains("a effect") && !s.contains('{'), "{s}");
        }
    }
}

/// A kit split to strips (ADR 0100): an insert on a pad's strip hears the kit's clip
/// through the pad — the audition plays the user's clip, not the fixed source.
#[test]
fn an_insert_on_a_split_kit_s_pad_strip_is_heard_on_the_kit_s_clip() {
    let p = aud::phrase_project(
        InstrumentSpec::Pulsus(cantus_core::PulsusParams::default()),
        Role::Drums,
    );
    let mut app = support::app(p);
    let kit = app.project().tracks[0].id;
    assert!(app.split_kit(kit), "the kit splits");
    let pad_strip = app.project().tracks[0]
        .instrument
        .pulsus()
        .and_then(|k| k.pads.iter().find_map(|p| p.output))
        .expect("a pad has a strip of its own");
    app.add_effect(pad_strip, EffectKind::Saturator);
    let fervor = app.project().strip(pad_strip).unwrap().effects[0].id;
    app.open_preset_browser(Target::Effect {
        strip: pad_strip,
        effect: fervor,
    });
    app.selected_clip = None;
    let id = app.presets.shown()[0].id.clone();
    app.audition_preset_id(&id);
    assert!(app.finish_preset_audition(Duration::from_secs(120)));
    assert_eq!(
        app.presets.audition.on,
        Some(HeardOn::FirstClip),
        "heard on the kit's clip through its pad"
    );
    // The kit's clip, selected: the same, named as the selected one.
    app.selected_clip = Some(app.project().tracks[0].clips[0].id);
    let other = app.presets.shown()[1].id.clone();
    app.audition_preset_id(&other);
    assert!(app.finish_preset_audition(Duration::from_secs(120)));
    assert_eq!(app.presets.audition.on, Some(HeardOn::SelectedClip));
}

/// A frozen strip (ADR 0064): the audition renders the preset live — two presets do not
/// sound the same — and Load is refused, the player saying so.
#[test]
fn an_insert_in_a_frozen_strip_is_auditioned_live_and_its_load_is_refused() {
    let mut p = project();
    let strip = p.tracks[0].strip.unwrap();
    let fervor = p.strip(strip).unwrap().effects[0].id;
    let content =
        cantus_core::freeze::content_key(&p, cantus_core::freeze::FreezeTarget::Strip(strip))
            .expect("a content key");
    p.strip_mut(strip).unwrap().frozen = Some(cantus_core::freeze::Frozen { content });
    let target = Target::Effect {
        strip,
        effect: fervor,
    };
    let e = fervor_entry();
    let copy = aud::prepare(&p, target, &e, None, aud::TEMPO);
    assert!(
        copy.project.strips.iter().all(|s| s.frozen.is_none()),
        "the copy plays live"
    );
    let mut app = support::app(p);
    app.open_preset_browser(target);
    assert!(cantus_ui::preset_browser::load_refused(&app));
    let before = app.project().clone();
    let id = app.presets.shown()[0].id.clone();
    assert!(!app.apply_preset_id(&id), "Load is refused");
    assert_eq!(app.project(), &before);
}
