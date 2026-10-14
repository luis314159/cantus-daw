//! The clip browser (ADR 0037, round 26): FL Studio's playlist picker panel in Cantus — a
//! pane listing the link groups and clips of their own as tiles (name, kind, ×N, a
//! miniature), filtered by name or track; dragging a tile onto the arrangement places a
//! linked copy where it is dropped, snapped, in one undo step (`Linker::copy`), and the
//! copy is in the group.

mod support;

use cantus_core::*;
use cantus_ui::keymap::{Action, Binding, Keymap};
use cantus_ui::views::workspace::Pane;
use cantus_ui::CantusApp;
use support::{app, ctx, painted_text};

/// Drums: "Beat" linked twice (bars 1–2) and a level curve; Bass: "Riff" of its own.
struct Scene {
    a: CantusApp,
    drums: TrackId,
    bass: TrackId,
    beats: [ClipId; 2],
    riff: ClipId,
    level: ClipId,
}

fn scene() -> Scene {
    let mut p = Project::new("browser");
    let mut drums = p.make_track("Drums");
    let mut bass = p.make_track("Bass");
    let link = LinkId(p.alloc_id());
    let mut beat = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(4));
    for i in 0..4 {
        let n = p.make_note(Ticks::beats(i), Ticks::QUARTER, 36 + i as u8, 90);
        beat.notes.push(n);
    }
    beat.link = Some(link);
    let mut beats = Vec::new();
    for bar in 0..2 {
        let mut c = beat.clone();
        c.id = ClipId(p.alloc_id());
        c.start = Ticks::beats(4 * bar);
        beats.push(c.id);
        drums.clips.push(c);
    }
    let mut level = p.make_clip("Level", Ticks::beats(8), Ticks::beats(4));
    level.automation = Some(Automation::new(AutoTarget::TrackGain(drums.id), Vec::new()));
    let level_id = level.id;
    drums.clips.push(level);
    let mut riff = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(4));
    let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, 40, 90);
    riff.notes.push(n);
    let riff_id = riff.id;
    bass.clips.push(riff);
    let ids = (drums.id, bass.id);
    p.tracks = vec![drums, bass];
    let mut a = app(p);
    a.show_pane(Pane::Clips);
    Scene {
        a,
        drums: ids.0,
        bass: ids.1,
        beats: [beats[0], beats[1]],
        riff: riff_id,
        level: level_id,
    }
}

fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| a.frame(ui));
    out.textures_delta.clear();
    out
}

fn normalised(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

fn button(at: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Press on the tile of `entry`, move in steps to `to`, let go there: a real drag, frame
/// by frame.
fn drag(a: &mut CantusApp, ctx: &egui::Context, entry: (TrackId, ClipId), to: egui::Pos2) {
    let from = a
        .clip_browser
        .tile_rects()
        .iter()
        .find(|(r, _)| *r == entry)
        .unwrap_or_else(|| panic!("{entry:?} has no tile"))
        .1
        .center();
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=12 {
        let p = from + (to - from) * (i as f32 / 12.0);
        frame(a, ctx, vec![egui::Event::PointerMoved(p)]);
    }
    frame(a, ctx, vec![button(to, false)]);
    frame(a, ctx, Vec::new());
}

fn ready(s: &mut Scene) -> egui::Context {
    let ctx = ctx();
    s.a.attach(&ctx);
    s.a.focus_arrangement();
    for _ in 0..4 {
        frame(&mut s.a, &ctx, Vec::new());
    }
    ctx
}

#[test]
fn the_pane_lists_groups_then_clips_of_their_own_as_tiles() {
    let mut s = scene();
    let ctx = ready(&mut s);
    let tiles: Vec<_> =
        s.a.clip_browser
            .tile_rects()
            .iter()
            .map(|(r, _)| *r)
            .collect();
    assert_eq!(
        tiles,
        vec![(s.drums, s.beats[0]), (s.drums, s.level), (s.bass, s.riff)]
    );
    let text = painted_text(&frame(&mut s.a, &ctx, Vec::new()).shapes);
    assert!(text.contains("×2"), "{text}");
    assert!(
        text.contains(&cantus_ui::i18n::t("links.browser.title")),
        "{text}"
    );
    // The filter narrows the tiles, by name or track.
    s.a.clip_browser.filter = "bass".into();
    frame(&mut s.a, &ctx, Vec::new());
    let tiles: Vec<_> =
        s.a.clip_browser
            .tile_rects()
            .iter()
            .map(|(r, _)| *r)
            .collect();
    assert_eq!(tiles, vec![(s.bass, s.riff)]);
}

/// The property the owner asked for: the dropped clip is in the entry's group, holds its
/// content, starts where it was dropped (snapped to the grid), on the row it was dropped
/// on — and one undo takes it all back, the group as it was.
#[test]
fn dragging_a_tile_onto_the_arrangement_places_a_linked_copy_in_one_undo_step() {
    let mut s = scene();
    let ctx = ready(&mut s);
    let before = normalised(s.a.project());
    let g =
        s.a.arrangement
            .geometry()
            .expect("the arrangement was drawn");
    // Bar 4 and a bit, on the bass row: snapped down to the bar.
    let to = egui::pos2(g.x(12.6), g.row_y(1));
    drag(&mut s.a, &ctx, (s.drums, s.beats[0]), to);
    let bass = s.a.project().track(s.bass).unwrap();
    assert_eq!(bass.clips.len(), 2, "a copy landed on the bass track");
    let copy = bass.clips.iter().find(|c| c.id != s.riff).unwrap().clone();
    let beat =
        s.a.project()
            .track(s.drums)
            .unwrap()
            .clip(s.beats[0])
            .unwrap()
            .clone();
    assert_eq!(copy.link, beat.link, "the copy is in the group");
    assert!(links::same_content(&copy, &beat));
    assert_eq!(copy.start, Ticks::beats(12), "dropped at bar 4, snapped");
    assert_eq!(links::count(s.a.project(), beat.link.unwrap()), 3);
    assert!(links::check(s.a.project()).is_ok());
    assert_eq!(
        s.a.clip_selection(),
        vec![(s.bass, copy.id)],
        "the copy is selected"
    );
    assert_eq!(s.a.bus.undo_label_key(), Some("cmd.place_linked_copy"));
    s.a.undo();
    assert_eq!(normalised(s.a.project()), before);
}

/// A clip of its own dropped becomes the first link of a new group; an automation clip
/// stays under the track it automates, wherever its row was dropped.
#[test]
fn a_clip_of_its_own_starts_a_group_and_automation_stays_on_its_track() {
    let mut s = scene();
    let ctx = ready(&mut s);
    let g = s.a.arrangement.geometry().unwrap();
    drag(
        &mut s.a,
        &ctx,
        (s.bass, s.riff),
        egui::pos2(g.x(8.2), g.row_y(1)),
    );
    let riff =
        s.a.project()
            .track(s.bass)
            .unwrap()
            .clip(s.riff)
            .unwrap()
            .clone();
    assert!(riff.link.is_some());
    assert_eq!(links::count(s.a.project(), riff.link.unwrap()), 2);
    let rev = s.a.bus.revision();
    let g = s.a.arrangement.geometry().unwrap();
    drag(
        &mut s.a,
        &ctx,
        (s.drums, s.level),
        egui::pos2(g.x(16.0), g.row_y(1)),
    );
    assert_eq!(s.a.bus.revision(), rev + 1, "one step");
    assert_eq!(s.a.bus.undo_label_key(), Some("cmd.place_linked_copy"));
    let drums = s.a.project().track(s.drums).unwrap();
    let copies: Vec<_> = drums
        .clips
        .iter()
        .filter(|c| c.automation.is_some())
        .collect();
    assert_eq!(copies.len(), 2, "the curve's copy is under Drums");
    assert!(copies
        .iter()
        .all(|c| c.link.is_some() && c.link == copies[0].link));
}

#[test]
fn a_drop_outside_the_arrangement_places_nothing() {
    let mut s = scene();
    let ctx = ready(&mut s);
    let before = normalised(s.a.project());
    let rev = s.a.bus.revision();
    // Over the inspector at the right edge.
    drag(
        &mut s.a,
        &ctx,
        (s.drums, s.beats[0]),
        egui::pos2(1390.0, 300.0),
    );
    assert_eq!(normalised(s.a.project()), before);
    assert_eq!(s.a.bus.revision(), rev);
    assert!(s.a.clip_browser.dragging().is_none());
}

/// Alt+F5 (F5 is the arrangement, FL's playlist) toggles the pane; a keymap saved before
/// it gets it; its names are in both languages.
#[test]
fn alt_f5_toggles_the_clip_browser() {
    let map = Keymap::default();
    assert_eq!(
        map.bindings_for(Action::ToggleClips),
        &[Binding::alt(egui::Key::F5)]
    );
    assert!(map.conflicts().is_empty());
    let mut s = scene();
    let _ctx = ready(&mut s);
    assert!(s.a.workspace.contains(Pane::Clips));
    s.a.run_action(Action::ToggleClips);
    assert!(!s.a.workspace.contains(Pane::Clips));
    let text: String = map
        .to_toml()
        .lines()
        .filter(|l| !l.trim_start().starts_with("toggle-clips"))
        .map(|l| {
            if l.starts_with("version") {
                "version = 6".to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let old = Keymap::from_toml(&text).unwrap();
    assert_eq!(
        old.bindings_for(Action::ToggleClips),
        map.bindings_for(Action::ToggleClips)
    );
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::default_thread_locale(locale);
        for key in [
            Action::ToggleClips.label_key(),
            "links.browser.title".into(),
        ] {
            let t = cantus_ui::i18n::t(&key);
            assert_ne!(t, key, "{key} ({locale:?})");
        }
    }
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
}

/// No two texts overlap in the pane, English and Spanish, 75 % and 200 %.
#[test]
fn no_two_texts_collide_in_the_pane() {
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for zoom in [0.75f32, 2.0] {
            let mut s = scene();
            let ctx = ctx();
            cantus_ui::i18n::default_thread_locale(locale);
            ctx.set_pixels_per_point(zoom);
            let mut shapes = Vec::new();
            for _ in 0..3 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(220.0, 600.0),
                    )),
                    time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
                    ..Default::default()
                };
                let a = &mut s.a;
                let mut out = ctx.run_ui(input, |ui| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ui, |ui| cantus_ui::views::clip_browser::show(a, ui));
                });
                out.textures_delta.clear();
                shapes = out.shapes;
            }
            let mut rects = Vec::new();
            fn walk(sh: &egui::Shape, clip: egui::Rect, out: &mut Vec<(egui::Rect, String)>) {
                match sh {
                    egui::Shape::Text(t) if t.angle == 0.0 && !t.galley.text().is_empty() => {
                        let r = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                        if r.is_positive() {
                            out.push((r, t.galley.text().to_string()));
                        }
                    }
                    egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, clip, out)),
                    _ => {}
                }
            }
            for c in &shapes {
                walk(&c.shape, c.clip_rect, &mut rects);
            }
            assert!(rects.len() >= 6, "{rects:?}");
            for (i, (r, t)) in rects.iter().enumerate() {
                for (q, u) in &rects[i + 1..] {
                    let o = r.intersect(*q);
                    assert!(
                        o.width() <= 1.0 || o.height() <= 1.0,
                        "{t:?} and {u:?} collide ({zoom}×, {locale:?})"
                    );
                }
            }
        }
    }
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
}

// ---- painting with the chosen tile (FL's paint tool, B) -------------------------------------

fn tile_at(a: &CantusApp, entry: (TrackId, ClipId)) -> egui::Pos2 {
    a.clip_browser
        .tile_rects()
        .iter()
        .find(|(r, _)| *r == entry)
        .unwrap_or_else(|| panic!("{entry:?} has no tile"))
        .1
        .center()
}

fn press(at: egui::Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: at,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// A click on `at` (pressed and released in one frame), then a frame.
fn click_at(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    use egui::PointerButton::Primary;
    frame(
        a,
        ctx,
        vec![
            egui::Event::PointerMoved(at),
            press(at, Primary, true),
            press(at, Primary, false),
        ],
    );
    frame(a, ctx, Vec::new());
}

/// A stroke with `button` from `from` to `to`, in steps, let go at `to`.
fn stroke(
    a: &mut CantusApp,
    ctx: &egui::Context,
    button: egui::PointerButton,
    from: egui::Pos2,
    to: egui::Pos2,
) {
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), press(from, button, true)],
    );
    for i in 1..=16 {
        let p = from + (to - from) * (i as f32 / 16.0);
        frame(a, ctx, vec![egui::Event::PointerMoved(p)]);
    }
    frame(a, ctx, vec![press(to, button, false)]);
    frame(a, ctx, Vec::new());
}

/// Choose `entry`'s tile and arm the arrangement's paint tool (B, the arrangement focused).
fn paint_with(a: &mut CantusApp, ctx: &egui::Context, entry: (TrackId, ClipId)) {
    let at = tile_at(a, entry);
    click_at(a, ctx, at);
    assert_eq!(
        a.clip_browser.brush(),
        Some(entry),
        "the tile is the paint source"
    );
    a.focus_arrangement();
    if !a.clip_browser.painting() {
        a.run_action(Action::ToolPaint);
    }
    assert!(
        a.clip_browser.painting(),
        "B arms the arrangement's paint tool"
    );
    frame(a, ctx, Vec::new());
}

fn starts(a: &CantusApp, t: TrackId, link: Option<LinkId>) -> Vec<Ticks> {
    let mut v: Vec<Ticks> = a
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .filter(|c| c.link.is_some() && c.link == link)
        .map(|c| c.start)
        .collect();
    v.sort();
    v
}

#[test]
fn a_click_with_the_paint_tool_places_one_linked_copy_where_it_snaps() {
    let mut s = scene();
    let ctx = ready(&mut s);
    paint_with(&mut s.a, &ctx, (s.drums, s.beats[0]));
    let before = normalised(s.a.project());
    let g = s.a.arrangement.geometry().unwrap();
    click_at(&mut s.a, &ctx, egui::pos2(g.x(12.6), g.row_y(1)));
    let link =
        s.a.project()
            .track(s.drums)
            .unwrap()
            .clip(s.beats[0])
            .unwrap()
            .link;
    assert_eq!(starts(&s.a, s.bass, link), vec![Ticks::beats(12)]);
    assert_eq!(s.a.bus.undo_label_key(), Some("cmd.paint_linked_copies"));
    s.a.undo();
    assert_eq!(normalised(s.a.project()), before);
}

/// A drag paints copies end to end along the row from where it was pressed (snapped), one
/// clip length apart, never overlapping each other or a copy already there — one undo step.
#[test]
fn a_paint_stroke_places_copies_end_to_end_in_one_undo_step() {
    let mut s = scene();
    let ctx = ready(&mut s);
    paint_with(&mut s.a, &ctx, (s.drums, s.beats[0]));
    let before = normalised(s.a.project());
    let rev = s.a.bus.revision();
    let g = s.a.arrangement.geometry().unwrap();
    let row = g.row_y(1);
    stroke(
        &mut s.a,
        &ctx,
        egui::PointerButton::Primary,
        egui::pos2(g.x(12.3), row),
        egui::pos2(g.x(28.5), row),
    );
    assert_eq!(s.a.bus.revision(), rev + 1, "one step");
    let link =
        s.a.project()
            .track(s.drums)
            .unwrap()
            .clip(s.beats[0])
            .unwrap()
            .link;
    let got = starts(&s.a, s.bass, link);
    assert_eq!(
        got,
        (3..=7).map(|b| Ticks::beats(4 * b)).collect::<Vec<_>>()
    );
    assert!(links::check(s.a.project()).is_ok());
    // Painting over the same stretch again adds nothing where a copy already starts.
    let rev = s.a.bus.revision();
    stroke(
        &mut s.a,
        &ctx,
        egui::PointerButton::Primary,
        egui::pos2(g.x(12.3), row),
        egui::pos2(g.x(20.5), row),
    );
    assert_eq!(s.a.bus.revision(), rev, "nothing to paint, nothing done");
    s.a.undo();
    assert_eq!(normalised(s.a.project()), before);
}

/// A right-drag erases — only the copies of the paint source's group (FL's paint tool
/// erases any clip; Cantus erases only the clip it paints, so a stroke never takes a
/// neighbour along) — in one undo step.
#[test]
fn a_right_drag_erases_only_copies_of_the_paint_source() {
    let mut s = scene();
    let ctx = ready(&mut s);
    // A beat painted onto the bass row at bar 3, then the riff at bar 4.
    paint_with(&mut s.a, &ctx, (s.drums, s.beats[0]));
    let g = s.a.arrangement.geometry().unwrap();
    click_at(&mut s.a, &ctx, egui::pos2(g.x(8.5), g.row_y(1)));
    paint_with(&mut s.a, &ctx, (s.bass, s.riff));
    click_at(&mut s.a, &ctx, egui::pos2(g.x(12.5), g.row_y(1)));
    let before = normalised(s.a.project());
    let bass = |a: &CantusApp| a.project().track(s.bass).unwrap().clips.len();
    assert_eq!(bass(&s.a), 3);
    let row = g.row_y(1);
    stroke(
        &mut s.a,
        &ctx,
        egui::PointerButton::Secondary,
        egui::pos2(g.x(0.5), row),
        egui::pos2(g.x(15.5), row),
    );
    let left: Vec<String> =
        s.a.project()
            .track(s.bass)
            .unwrap()
            .clips
            .iter()
            .map(|c| c.name.clone())
            .collect();
    assert_eq!(
        left,
        vec!["Beat".to_string()],
        "the beat stays; both riffs went"
    );
    assert_eq!(s.a.bus.undo_label_key(), Some("cmd.erase_linked_copies"));
    assert_eq!(
        s.a.clip_browser.brush(),
        None,
        "its group is gone: no source left"
    );
    s.a.undo();
    assert_eq!(normalised(s.a.project()), before);
}

/// The chosen tile is marked in the browser and named in the hint bar over the lanes; Esc
/// clears it and puts the tool away; a double-click on a tile selects its copies.
#[test]
fn the_source_is_shown_and_esc_clears_it() {
    let mut s = scene();
    let ctx = ready(&mut s);
    paint_with(&mut s.a, &ctx, (s.drums, s.beats[0]));
    let g = s.a.arrangement.geometry().unwrap();
    frame(
        &mut s.a,
        &ctx,
        vec![egui::Event::PointerMoved(egui::pos2(g.x(20.0), g.row_y(1)))],
    );
    let text = painted_text(&frame(&mut s.a, &ctx, Vec::new()).shapes);
    let hint = cantus_ui::i18n::tf("links.paint.hint", &[("name", &"Beat")]);
    assert!(text.contains(&hint), "{hint:?} not in {text}");
    let ev = |pressed| egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    frame(&mut s.a, &ctx, vec![ev(true), ev(false)]);
    assert_eq!(s.a.clip_browser.brush(), None);
    assert!(!s.a.clip_browser.painting());
    // A double-click selects the group's copies.
    let at = tile_at(&s.a, (s.drums, s.beats[0]));
    use egui::PointerButton::Primary;
    frame(
        &mut s.a,
        &ctx,
        vec![
            egui::Event::PointerMoved(at),
            press(at, Primary, true),
            press(at, Primary, false),
            press(at, Primary, true),
            press(at, Primary, false),
        ],
    );
    frame(&mut s.a, &ctx, Vec::new());
    let mut sel = s.a.clip_selection();
    sel.sort();
    let mut want = vec![(s.drums, s.beats[0]), (s.drums, s.beats[1])];
    want.sort();
    assert_eq!(sel, want);
}

// ---- the second review of the paint tool and the browser --------------------------------------

/// The copies of `link` on track `t` never overlap each other.
fn no_overlap(a: &CantusApp, t: TrackId, link: Option<LinkId>) {
    let mut spans: Vec<(Ticks, Ticks)> = a
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .filter(|c| c.link.is_some() && c.link == link)
        .map(|c| (c.start, c.end()))
        .collect();
    spans.sort();
    for w in spans.windows(2) {
        assert!(w[0].1 <= w[1].0, "{:?} overlaps {:?}", w[0], w[1]);
    }
}

/// On a one-beat grid a stroke started one beat later than another, over the same ground,
/// paints nothing that overlaps; a click inside a copy places nothing.
#[test]
fn painting_never_stacks_copies_on_copies() {
    let mut s = scene();
    let ctx = ready(&mut s);
    s.a.arrangement.grid = Ticks::beats(1);
    paint_with(&mut s.a, &ctx, (s.bass, s.riff));
    let g = s.a.arrangement.geometry().unwrap();
    let row = g.row_y(1);
    let p = |b: f32| egui::pos2(g.x(b), row);
    stroke(
        &mut s.a,
        &ctx,
        egui::PointerButton::Primary,
        p(12.3),
        p(20.5),
    );
    let link =
        s.a.project()
            .track(s.bass)
            .unwrap()
            .clip(s.riff)
            .unwrap()
            .link;
    assert_eq!(
        starts(&s.a, s.bass, link),
        [0, 12, 16, 20].map(Ticks::beats).to_vec()
    );
    let rev = s.a.bus.revision();
    stroke(
        &mut s.a,
        &ctx,
        egui::PointerButton::Primary,
        p(14.2),
        p(22.5),
    );
    assert_eq!(
        s.a.bus.revision(),
        rev,
        "every place overlaps a copy: nothing painted"
    );
    click_at(&mut s.a, &ctx, p(1.5));
    assert_eq!(
        s.a.bus.revision(),
        rev,
        "a click inside the source places nothing"
    );
    // Further right the stroke paints again — from where it no longer overlaps.
    stroke(
        &mut s.a,
        &ctx,
        egui::PointerButton::Primary,
        p(22.2),
        p(30.5),
    );
    no_overlap(&s.a, s.bass, link);
    assert_eq!(
        starts(&s.a, s.bass, link),
        [0, 12, 16, 20, 26, 30].map(Ticks::beats).to_vec(),
        "22 and 26… would overlap the copy at 20: the lattice from 22 keeps 26 and 30"
    );
}

/// A leftward stroke places the copy under the pointer, as a rightward one does.
#[test]
fn a_leftward_stroke_reaches_the_pointer() {
    let mut s = scene();
    let ctx = ready(&mut s);
    paint_with(&mut s.a, &ctx, (s.bass, s.riff));
    let g = s.a.arrangement.geometry().unwrap();
    let row = g.row_y(1);
    stroke(
        &mut s.a,
        &ctx,
        egui::PointerButton::Primary,
        egui::pos2(g.x(18.5), row),
        egui::pos2(g.x(9.0), row),
    );
    let link =
        s.a.project()
            .track(s.bass)
            .unwrap()
            .clip(s.riff)
            .unwrap()
            .link;
    assert_eq!(
        starts(&s.a, s.bass, link),
        [0, 8, 12, 16].map(Ticks::beats).to_vec()
    );
}

/// A stroke and a tile chosen in the browser give the arrangement the edit focus, so B and
/// Delete then act on it.
#[test]
fn painting_and_choosing_a_tile_focus_the_arrangement() {
    let mut s = scene();
    let ctx = ready(&mut s);
    paint_with(&mut s.a, &ctx, (s.bass, s.riff));
    s.a.focus_piano_roll();
    let g = s.a.arrangement.geometry().unwrap();
    click_at(&mut s.a, &ctx, egui::pos2(g.x(20.5), g.row_y(1)));
    s.a.run_action(Action::ToolPaint);
    assert!(
        !s.a.clip_browser.painting(),
        "B put the arrangement's tool away"
    );
    s.a.focus_piano_roll();
    let at = tile_at(&s.a, (s.drums, s.beats[0]));
    click_at(&mut s.a, &ctx, at);
    s.a.run_action(Action::ToolPaint);
    assert!(
        s.a.clip_browser.painting(),
        "B brought the arrangement's tool out"
    );
}

/// Esc meant for something else — the piano roll's selection — leaves the tool and its
/// source alone; with the arrangement focused it puts them away.
#[test]
fn esc_puts_the_tool_away_only_from_the_arrangement() {
    let mut s = scene();
    let ctx = ready(&mut s);
    paint_with(&mut s.a, &ctx, (s.bass, s.riff));
    let esc = || {
        let ev = |pressed| egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        vec![ev(true), ev(false)]
    };
    s.a.focus_piano_roll();
    frame(&mut s.a, &ctx, esc());
    assert!(s.a.clip_browser.painting());
    assert_eq!(s.a.clip_browser.brush(), Some((s.bass, s.riff)));
    s.a.focus_arrangement();
    frame(&mut s.a, &ctx, esc());
    assert!(!s.a.clip_browser.painting());
    assert_eq!(s.a.clip_browser.brush(), None);
}

/// An automation source paints where its copy lands — under the track it automates — and
/// its ghost is drawn there, in the paint tool and in a drag from the browser; erasing
/// over the row it was painted from reaches it.
#[test]
fn an_automation_source_shows_and_erases_where_it_lands() {
    let mut s = scene();
    let ctx = ready(&mut s);
    paint_with(&mut s.a, &ctx, (s.drums, s.level));
    let g = s.a.arrangement.geometry().unwrap();
    let drums = g.rows[0];
    let inside =
        |r: egui::Rect| r.center().y > drums.top && r.center().y < drums.top + drums.height;
    // Held on the bass row: the ghost is on the drums row.
    let from = egui::pos2(g.x(20.5), g.row_y(1));
    use egui::PointerButton::Primary;
    frame(
        &mut s.a,
        &ctx,
        vec![egui::Event::PointerMoved(from), press(from, Primary, true)],
    );
    for i in 1..6 {
        let p = from + egui::vec2(i as f32 * 4.0, 0.0);
        frame(&mut s.a, &ctx, vec![egui::Event::PointerMoved(p)]);
    }
    let ghosts = s.a.clip_browser.ghosts().to_vec();
    assert!(
        !ghosts.is_empty() && ghosts.iter().all(|r| inside(*r)),
        "{ghosts:?}"
    );
    frame(&mut s.a, &ctx, vec![press(from, Primary, false)]);
    frame(&mut s.a, &ctx, Vec::new());
    let level_copies = |a: &CantusApp| {
        a.project()
            .track(s.drums)
            .unwrap()
            .clips
            .iter()
            .filter(|c| c.automation.is_some())
            .count()
    };
    assert_eq!(level_copies(&s.a), 2);
    // A right-drag over the same stretch of the bass row erases it.
    stroke(
        &mut s.a,
        &ctx,
        egui::PointerButton::Secondary,
        egui::pos2(g.x(19.0), g.row_y(1)),
        egui::pos2(g.x(26.0), g.row_y(1)),
    );
    assert_eq!(level_copies(&s.a), 1);
    // The browser's drag: the ghost is on the drums row too.
    s.a.run_action(Action::ToolPaint);
    let tile = tile_at(&s.a, (s.drums, s.level));
    let to = egui::pos2(g.x(14.5), g.row_y(1));
    frame(
        &mut s.a,
        &ctx,
        vec![egui::Event::PointerMoved(tile), press(tile, Primary, true)],
    );
    for i in 1..=12 {
        let p = tile + (to - tile) * (i as f32 / 12.0);
        frame(&mut s.a, &ctx, vec![egui::Event::PointerMoved(p)]);
    }
    let ghosts = s.a.clip_browser.ghosts().to_vec();
    assert!(ghosts.len() == 1 && inside(ghosts[0]), "{ghosts:?}");
    frame(&mut s.a, &ctx, vec![press(to, Primary, false)]);
}

/// The browser's scrollbar moves the tiles.
#[test]
fn the_browser_scrollbar_scrolls() {
    let mut p = Project::new("many");
    let mut t = p.make_track("T");
    for i in 0..40 {
        let mut c = p.make_clip(format!("Clip {i}"), Ticks::beats(4 * i), Ticks::beats(4));
        let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 90);
        c.notes.push(n);
        t.clips.push(c);
    }
    p.tracks.push(t);
    let mut a = app(p);
    let ctx = ctx();
    let pane = |a: &mut CantusApp, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(220.0, 600.0),
            )),
            events,
            time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| cantus_ui::views::clip_browser::show(a, ui));
        });
        out.textures_delta.clear();
    };
    for _ in 0..3 {
        pane(&mut a, Vec::new());
    }
    let before = a.clip_browser.tile_rects()[0];
    let lane = a.clip_browser.scroll_lane().expect("the list overflows");
    let from = egui::pos2(lane.center().x, lane.top() + 10.0);
    let to = egui::pos2(lane.center().x, lane.top() + 200.0);
    use egui::PointerButton::Primary;
    pane(
        &mut a,
        vec![egui::Event::PointerMoved(from), press(from, Primary, true)],
    );
    for i in 1..=10 {
        let p = from + (to - from) * (i as f32 / 10.0);
        pane(&mut a, vec![egui::Event::PointerMoved(p)]);
    }
    pane(&mut a, vec![press(to, Primary, false)]);
    pane(&mut a, Vec::new());
    assert_ne!(a.clip_browser.tile_rects()[0], before, "the list moved");
}
