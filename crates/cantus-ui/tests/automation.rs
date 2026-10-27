//! Automation in the application (ADR 0027): making a clip from a parameter and from the
//! last tweaked one, the knobs showing the curve, the clip's points edited in place and in
//! the large editor through published geometry, save and open, split and glue, undo.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::views::arrangement::autolanes;
use cantus_ui::{starter_project, views, CantusApp};

fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

const SIZE: egui::Vec2 = egui::vec2(1200.0, 520.0);

/// One frame of a view with these events.
fn frame(
    a: &mut CantusApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    view: fn(&mut CantusApp, &mut egui::Ui),
) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| view(a, ui));
    })
    .textures_delta
    .clear();
}

fn arrangement(a: &mut CantusApp, ui: &mut egui::Ui) {
    views::arrangement::show(a, ui)
}

fn editor(a: &mut CantusApp, ui: &mut egui::Ui) {
    views::piano_roll::show(a, ui)
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Press at `from`, travel to `to`, release, in `view`.
fn drag(
    a: &mut CantusApp,
    ctx: &egui::Context,
    from: egui::Pos2,
    to: egui::Pos2,
    view: fn(&mut CantusApp, &mut egui::Ui),
) {
    frame(a, ctx, vec![egui::Event::PointerMoved(from)], view);
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
        view,
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frame(a, ctx, vec![egui::Event::PointerMoved(at)], view);
    }
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
        view,
    );
    frame(a, ctx, vec![], view);
}

fn click(
    a: &mut CantusApp,
    ctx: &egui::Context,
    at: egui::Pos2,
    view: fn(&mut CantusApp, &mut egui::Ui),
) {
    frame(a, ctx, vec![egui::Event::PointerMoved(at)], view);
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(at), button(at, true)],
        view,
    );
    frame(a, ctx, vec![button(at, false)], view);
    frame(a, ctx, vec![], view);
}

/// The automation clips of the project, as (track, clip, automation).
fn autos(a: &CantusApp) -> Vec<(TrackId, ClipId, Automation)> {
    a.project()
        .tracks
        .iter()
        .flat_map(|t| {
            t.clips
                .iter()
                .filter_map(move |c| c.automation.clone().map(|x| (t.id, c.id, x)))
        })
        .collect()
}

#[test]
fn a_parameter_makes_a_clip_flat_at_its_value_in_one_undo_step() {
    let mut a = app(starter_project());
    let before = a.project().clone();
    let strip = a.project().tracks[0].strip.unwrap();
    assert!(a.exec(Command::SetStripGain {
        id: strip,
        gain_db: -6.0
    }));
    let rev = a.project().clone();
    a.automate(AutoTarget::StripGain(strip));
    let made = autos(&a);
    assert_eq!(made.len(), 1);
    let (t, c, auto) = &made[0];
    assert_eq!(
        *t,
        a.project().tracks[0].id,
        "under the track that feeds the strip"
    );
    assert_eq!(auto.target, AutoTarget::StripGain(strip));
    let range = automation::range::range_of(a.project(), &auto.target).unwrap();
    for p in &auto.points {
        assert!((range.value(p.value) + 6.0).abs() < 1e-3, "flat at −6 dB");
    }
    assert_eq!(a.selected_clip, Some(*c), "the new clip is selected");
    let clip = a.project().track(*t).unwrap().clip(*c).unwrap();
    assert_eq!(clip.start, Ticks::ZERO);
    assert_eq!(
        clip.length,
        Ticks::beats(16),
        "four bars from the playhead's bar"
    );
    // Asked again, the parameter opens its clip rather than making another.
    a.automate(AutoTarget::StripGain(strip));
    assert_eq!(autos(&a).len(), 1);
    a.undo();
    let mut now = a.project().clone();
    now.next_id = rev.next_id;
    assert_eq!(now, rev, "one undo step");
    a.undo();
    let mut now = a.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before);
}

#[test]
fn the_last_tweaked_parameter_is_any_command_and_makes_its_clip() {
    let mut a = app(starter_project());
    let strip = a.project().tracks[0].strip.unwrap();
    let slot = a.bus.new_effect(EffectKind::Saturator);
    let fx = slot.id;
    assert!(a.exec(Command::InsertEffect {
        strip,
        index: 0,
        slot: Box::new(slot)
    }));
    assert!(!a.automate_last_tweaked(), "nothing tweaked yet");
    assert!(a.exec(Command::SetEffectParam {
        strip,
        effect: fx,
        key: "drive".into(),
        value: 12.0
    }));
    let want = AutoTarget::Effect {
        effect: fx,
        key: "drive".into(),
    };
    assert_eq!(a.automation.last_tweaked, Some(want.clone()));
    a.run_action(cantus_ui::keymap::Action::AutomateLastTweaked);
    let made = autos(&a);
    assert_eq!(made.len(), 1);
    assert_eq!(made[0].2.target, want);
    let name = cantus_ui::automation::target_name(a.project(), &want);
    assert!(name.contains("Drive") || name.contains("drive"), "{name}");
}

/// The knob shows the automated value: the effect editors read their host's spec, the
/// instrument knobs and the mixer the app's automated value — where the song is heard.
#[test]
fn an_automated_knob_shows_the_curve_where_the_song_is() {
    let mut a = app(starter_project());
    let strip = a.project().tracks[0].strip.unwrap();
    let slot = a.bus.new_effect(EffectKind::Saturator);
    let fx = slot.id;
    assert!(a.exec(Command::InsertEffect {
        strip,
        index: 0,
        slot: Box::new(slot)
    }));
    let track = a.project().tracks[0].id;
    let target = AutoTarget::Effect {
        effect: fx,
        key: "drive".into(),
    };
    let mut clip = a.bus.new_clip("drive", Ticks::beats(4), Ticks::beats(4));
    clip.automation = Some(Automation::new(
        target.clone(),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.0),
            AutoPoint::new(Ticks::beats(4), 1.0),
        ],
    ));
    assert!(a.exec(Command::InsertClip {
        track,
        clip: Box::new(clip)
    }));
    assert!(a.is_automated(&target));
    // Before the clip: the knob's own value (the default, 0 dB of drive).
    a.engine.seek(Ticks::beats(1));
    assert_eq!(a.automated_value(&target), Some(0.0));
    // Halfway along the ramp: half of 36 dB.
    a.engine.seek(Ticks::beats(6));
    let v = a.automated_value(&target).unwrap();
    assert!((v - 18.0).abs() < 1e-3, "{v}");
    let host = views::fx::host::DawHost::new(&mut a, strip, fx, "test");
    use views::fx::host::FxHost;
    assert!(
        (host.spec().get("drive") - 18.0).abs() < 1e-3,
        "the editor's knob"
    );
    assert!(host.automated("drive") && !host.automated("tone"));
    drop(host);
    // After it: held at the end.
    a.engine.seek(Ticks::beats(20));
    assert!((a.automated_value(&target).unwrap() - 36.0).abs() < 1e-3);
}

/// A clip with a ramp under the starter track, selected, drawn.
fn ramped() -> (CantusApp, (TrackId, ClipId), egui::Context) {
    let mut a = app(starter_project());
    let strip = a.project().tracks[0].strip.unwrap();
    let track = a.project().tracks[0].id;
    let mut clip = a.bus.new_clip("ride", Ticks::ZERO, Ticks::beats(16));
    clip.automation = Some(Automation::new(
        AutoTarget::StripGain(strip),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.2),
            AutoPoint::new(Ticks::beats(8), 0.8),
            AutoPoint::new(Ticks::beats(16), 0.5),
        ],
    ));
    let id = clip.id;
    assert!(a.exec(Command::InsertClip {
        track,
        clip: Box::new(clip)
    }));
    a.arrangement.track_height = 60.0;
    let ctx = ctx();
    frame(&mut a, &ctx, vec![], arrangement);
    frame(&mut a, &ctx, vec![], arrangement);
    (a, (track, id), ctx)
}

#[test]
fn the_lane_is_laid_out_under_its_track() {
    let (a, r, _) = ramped();
    let geo = a.arrangement.geometry().unwrap();
    assert!(
        geo.rows[0].auto > 0.0,
        "the track's row has a lane under it"
    );
    let p = autolanes::point_pos(&a, r, 1).expect("the clip was drawn");
    assert!(
        p.y > geo.rows[0].top + geo.rows[0].height,
        "in the lane, below the row"
    );
    assert!(p.y < geo.bottom());
}

#[test]
fn dragging_a_point_in_the_clip_moves_it_and_undo_puts_it_back() {
    let (mut a, r, ctx) = ramped();
    let before = a.project().clone();
    let from = autolanes::point_pos(&a, r, 1).unwrap();
    let to = autolanes::content_pos(&a, r, Ticks::beats(12), 0.4).unwrap();
    drag(&mut a, &ctx, from, to, arrangement);
    let auto = autos(&a)[0].2.clone();
    assert_eq!(auto.points[1].tick, Ticks::beats(12), "snapped to the bar");
    assert!(
        (auto.points[1].value - 0.4).abs() < 0.03,
        "{}",
        auto.points[1].value
    );
    assert_eq!(auto.points.len(), 3, "a drag adds nothing");
    a.undo();
    assert_eq!(*a.project(), before, "one undo step");
}

#[test]
fn a_press_on_the_line_adds_a_point_there() {
    let (mut a, r, ctx) = ramped();
    // A quarter of the way along the first ramp, on the line.
    let on = autolanes::content_pos(&a, r, Ticks::beats(2), 0.35).unwrap();
    click(&mut a, &ctx, on, arrangement);
    let auto = autos(&a)[0].2.clone();
    assert_eq!(auto.points.len(), 4, "{:?}", auto.points);
    assert_eq!(auto.points[1].tick, Ticks::beats(2));
}

#[test]
fn the_editor_adds_points_and_delete_removes_them() {
    let (mut a, r, ctx) = ramped();
    a.select_clip(r.0, r.1);
    frame(&mut a, &ctx, vec![], editor);
    frame(&mut a, &ctx, vec![], editor);
    let f = a
        .automation
        .editor
        .frame
        .expect("the editor drew its curve");
    let at = f.point(Ticks::beats(4) + Ticks(240), 0.9);
    click(&mut a, &ctx, at, editor);
    let auto = autos(&a)[0].2.clone();
    assert_eq!(auto.points.len(), 4);
    assert!(auto.points.iter().any(|p| (p.value - 0.9).abs() < 0.02));
    let sel = a.automation.editor.selected.clone();
    assert_eq!(sel.len(), 1, "the new point is selected");
    frame(
        &mut a,
        &ctx,
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        editor,
    );
    assert_eq!(autos(&a)[0].2.points.len(), 3, "Delete removed it");
}

#[test]
fn automation_survives_save_and_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _, _) = ramped();
    let path = dir.path().join("auto.cantus");
    a.save_to_path(&path);
    let saved = a.project().clone();
    a.new_project();
    assert!(autos(&a).is_empty());
    a.open(&path);
    assert_eq!(*a.project(), saved);
    assert_eq!(autos(&a).len(), 1);
}

#[test]
fn split_then_glue_gives_back_the_automation_clip() {
    let (mut a, r, _) = ramped();
    let before = a.project().clone();
    let clip = a.project().track(r.0).unwrap().clip(r.1).unwrap().clone();
    let mut next = a.project().next_id;
    let mut ids = || {
        next += 1;
        next
    };
    let cmds = cantus_ui::clips::split(a.project(), r.0, &clip, Ticks::beats(5), &mut ids).unwrap();
    assert!(a.exec(Command::Batch {
        label_key: "split".into(),
        commands: cmds
    }));
    let halves: Vec<Clip> = a
        .project()
        .track(r.0)
        .unwrap()
        .clips
        .iter()
        .filter(|c| c.automation.is_some())
        .cloned()
        .collect();
    assert_eq!(halves.len(), 2);
    let lane_now =
        automation::compile::lane_for(a.project(), &halves[0].automation.as_ref().unwrap().target)
            .unwrap();
    let lane_was =
        automation::compile::lane_for(&before, &halves[0].automation.as_ref().unwrap().target)
            .unwrap();
    for t in (0..Ticks::beats(16).0).step_by(97) {
        assert_eq!(
            lane_now.pos_at(t as f64),
            lane_was.pos_at(t as f64),
            "tick {t}"
        );
    }
    let refs: Vec<&Clip> = halves.iter().collect();
    let cmds = cantus_ui::clips::glue(r.0, &refs, &mut ids).unwrap();
    assert!(a.exec(Command::Batch {
        label_key: "glue".into(),
        commands: cmds
    }));
    let mut now = a.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before, "split and glue are an exact round trip");
}
