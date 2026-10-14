//! Linking by name (ADR 0037, round 26): the *Link to…* picker lists the project's link
//! groups and clips of their own by name — with the count, the kind and where each sits,
//! a filter, refused targets dimmed with their reason — and choosing one links the
//! selection in one undo step; the inspector's clip section names a linked clip's group
//! with its count and its actions, and renames the whole group from there.

mod support;

use cantus_core::links::{Kind, Refusal};
use cantus_core::*;
use cantus_ui::keymap::Action;
use cantus_ui::links::{picker_rows, PickRow};
use cantus_ui::views::inspector;
use cantus_ui::CantusApp;
use std::alloc::{GlobalAlloc, Layout, System};
use support::{app, ctx, painted_text};

/// Drums: "Beat" linked three times (bars 1–3), "Melodía" of its own at bar 5.
/// Bass: "Riff" of its own. Vocal: "Take" (audio) linked twice, and a "Beat" of its own
/// (an audio clip that happens to share the drum group's name).
struct Scene {
    a: CantusApp,
    drums: TrackId,
    bass: TrackId,
    vocal: TrackId,
    beats: [ClipId; 3],
    melodia: ClipId,
    riff: ClipId,
    takes: [ClipId; 2],
    vocal_beat: ClipId,
}

fn notes(p: &mut Project, c: &mut Clip, pitch: u8) {
    for i in 0..4 {
        let n = p.make_note(Ticks::beats(i), Ticks::QUARTER, pitch + i as u8, 90);
        c.notes.push(n);
    }
}

fn scene() -> Scene {
    let mut p = Project::new("picker");
    let mut drums = p.make_track("Drums");
    let mut bass = p.make_track("Bass");
    let mut vocal = p.make_track("Vocal");
    let link = LinkId(p.alloc_id());
    let mut beat = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(4));
    notes(&mut p, &mut beat, 36);
    beat.link = Some(link);
    let mut beats = Vec::new();
    for bar in 0..3 {
        let mut c = beat.clone();
        c.id = ClipId(p.alloc_id());
        c.start = Ticks::beats(4 * bar);
        beats.push(c.id);
        drums.clips.push(c);
    }
    let mut melodia = p.make_clip("Melodía", Ticks::beats(16), Ticks::beats(4));
    notes(&mut p, &mut melodia, 72);
    let melodia_id = melodia.id;
    drums.clips.push(melodia);
    let mut riff = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(4));
    notes(&mut p, &mut riff, 40);
    let riff_id = riff.id;
    bass.clips.push(riff);
    let src = AudioSource {
        id: AudioSourceId(p.alloc_id()),
        name: "take".into(),
        path: "C:/nowhere/take.wav".into(),
        sample_rate: 48_000,
        channels: 2,
        frames: 48_000 * 4,
        sha256: None,
        recorded: false,
    };
    let take_link = LinkId(p.alloc_id());
    let mut takes = Vec::new();
    for bar in 0..2 {
        let mut c = p.make_clip("Take", Ticks::beats(4 * bar), Ticks::beats(4));
        c.audio = Some(AudioRegion::of(src.id));
        c.link = Some(take_link);
        takes.push(c.id);
        vocal.clips.push(c);
    }
    let mut vb = p.make_clip("Beat", Ticks::beats(12), Ticks::beats(4));
    vb.audio = Some(AudioRegion::of(src.id));
    let vocal_beat = vb.id;
    vocal.clips.push(vb);
    p.audio_pool.push(src);
    let ids = (drums.id, bass.id, vocal.id);
    p.tracks = vec![drums, bass, vocal];
    assert!(links::check(&p).is_ok());
    Scene {
        a: app(p),
        drums: ids.0,
        bass: ids.1,
        vocal: ids.2,
        beats: [beats[0], beats[1], beats[2]],
        melodia: melodia_id,
        riff: riff_id,
        takes: [takes[0], takes[1]],
        vocal_beat,
    }
}

fn clip_of(a: &CantusApp, r: (TrackId, ClipId)) -> Clip {
    a.project().track(r.0).unwrap().clip(r.1).unwrap().clone()
}

fn normalised(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

fn row(rows: &[PickRow], first: (TrackId, ClipId)) -> &PickRow {
    rows.iter()
        .find(|r| r.group.first == first)
        .unwrap_or_else(|| panic!("{first:?} not offered: {rows:#?}"))
}

use cantus_core::links;

#[test]
fn the_rows_name_each_group_with_its_count_kind_place_and_reason() {
    let s = scene();
    let p = s.a.project();
    let rows = picker_rows(p, &[(s.bass, s.riff)], "");
    assert_eq!(rows.len(), 5, "{rows:#?}");
    let beat = row(&rows, (s.drums, s.beats[0]));
    assert_eq!((beat.group.count, beat.group.kind), (3, Kind::Notes));
    assert_eq!(beat.refusal, None);
    assert_eq!(beat.hint, "Drums · 1.1");
    let take = row(&rows, (s.vocal, s.takes[0]));
    assert_eq!(take.refusal, Some(Refusal::Kind), "audio is another kind");
    let own = row(&rows, (s.bass, s.riff));
    assert_eq!(own.refusal, Some(Refusal::Already), "the selection itself");
    assert_eq!(own.hint, "Bass · 1.1");
    // The two "Beat"s: one name, told apart by where they sit and what they hold.
    let vb = row(&rows, (s.vocal, s.vocal_beat));
    assert_eq!(vb.group.name, beat.group.name);
    assert_eq!(vb.hint, "Vocal · 4.1");
    assert_ne!(vb.hint, beat.hint);
    assert_eq!(vb.group.kind, Kind::Audio);
    // Groups first.
    assert_eq!(rows[0].group.first, (s.drums, s.beats[0]));
    assert_eq!(rows[1].group.first, (s.vocal, s.takes[0]));
}

#[test]
fn the_filter_ignores_case_and_accents_and_also_finds_a_track() {
    let s = scene();
    let p = s.a.project();
    let sel = [(s.bass, s.riff)];
    let names = |f: &str| -> Vec<String> {
        picker_rows(p, &sel, f)
            .into_iter()
            .map(|r| r.group.name)
            .collect()
    };
    assert_eq!(names("MELODIA"), vec!["Melodía"]);
    assert_eq!(names("melodía"), vec!["Melodía"]);
    assert_eq!(names("  beat "), vec!["Beat", "Beat"]);
    // The track's name finds what sits on it.
    assert_eq!(names("vocal"), vec!["Take", "Beat"]);
    assert!(names("nothing like this").is_empty());
}

#[test]
fn with_nothing_selected_link_to_says_so_and_opens_nothing() {
    let mut s = scene();
    s.a.set_clip_selection(Vec::new());
    s.a.selected_clip = None;
    assert!(s.a.clip_selection().is_empty());
    s.a.run_action(Action::LinkTo);
    assert!(!s.a.link_picker.is_open());
    assert_eq!(
        s.a.status.text,
        cantus_ui::i18n::t("links.status.select_first")
    );
}

/// One frame of the whole application, with `events`.
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

fn key(k: egui::Key) -> Vec<egui::Event> {
    let ev = |pressed| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    vec![ev(true), ev(false)]
}

fn click(at: egui::Pos2) -> Vec<egui::Event> {
    let b = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    vec![egui::Event::PointerMoved(at), b(true), b(false)]
}

#[test]
fn typing_a_name_and_enter_links_the_selection_in_one_undo_step() {
    let mut s = scene();
    let ctx = ctx();
    s.a.attach(&ctx);
    let (melodia, beat) = ((s.drums, s.melodia), (s.drums, s.beats[0]));
    s.a.focus_arrangement();
    s.a.select_clip(melodia.0, melodia.1);
    let before = normalised(s.a.project());
    frame(&mut s.a, &ctx, Vec::new());
    s.a.run_action(Action::LinkTo);
    assert!(s.a.link_picker.is_open());
    frame(&mut s.a, &ctx, Vec::new());
    frame(&mut s.a, &ctx, vec![egui::Event::Text("bea".into())]);
    assert_eq!(s.a.link_picker.filter, "bea");
    let firsts: Vec<_> =
        s.a.link_picker
            .rows()
            .iter()
            .map(|r| r.group.first)
            .collect();
    assert_eq!(firsts, vec![beat, (s.vocal, s.vocal_beat)]);
    frame(&mut s.a, &ctx, key(egui::Key::Enter));
    assert!(!s.a.link_picker.is_open(), "a choice closes the picker");
    let (m, b) = (clip_of(&s.a, melodia), clip_of(&s.a, beat));
    assert!(m.link.is_some() && m.link == b.link);
    assert_eq!(m.notes, b.notes);
    assert_eq!(m.start, Ticks::beats(16), "it keeps its place");
    assert_eq!(links::count(s.a.project(), b.link.unwrap()), 4);
    assert_eq!(s.a.bus.undo_label_key(), Some("cmd.link_clips"));
    s.a.undo();
    assert_eq!(normalised(s.a.project()), before);
}

#[test]
fn the_arrows_skip_refused_rows_and_a_refused_row_cannot_be_clicked() {
    let mut s = scene();
    let ctx = ctx();
    s.a.attach(&ctx);
    let riff = (s.bass, s.riff);
    s.a.select_clip(riff.0, riff.1);
    s.a.run_action(Action::LinkTo);
    for _ in 0..2 {
        frame(&mut s.a, &ctx, Vec::new());
    }
    let rows: Vec<PickRow> = s.a.link_picker.rows().into_iter().cloned().collect();
    let rects = s.a.link_picker.row_rects().to_vec();
    assert_eq!(rects.len(), rows.len(), "every row was drawn");
    // A click on the audio group (another kind) does nothing and keeps the picker open.
    let take = rects
        .iter()
        .find(|(r, _)| *r == (s.vocal, s.takes[0]))
        .unwrap()
        .1;
    frame(&mut s.a, &ctx, click(take.center()));
    assert!(s.a.link_picker.is_open());
    assert_eq!(clip_of(&s.a, riff).link, None);
    // Down from the first row: past the takes and the vocal beat (refused) to "Melodía";
    // again, and it stays there (the riff is the selection itself).
    assert_eq!(
        s.a.link_picker.highlighted(),
        Some((s.drums, s.beats[0])),
        "the first row that links is ready for Enter"
    );
    for _ in 0..2 {
        frame(&mut s.a, &ctx, key(egui::Key::ArrowDown));
        assert_eq!(
            s.a.link_picker.highlighted(),
            Some((s.drums, s.melodia)),
            "{rows:#?}"
        );
    }
    frame(&mut s.a, &ctx, key(egui::Key::ArrowUp));
    assert_eq!(s.a.link_picker.highlighted(), Some((s.drums, s.beats[0])));
    // A click on the beat group links the riff to it.
    let beat = rects
        .iter()
        .find(|(r, _)| *r == (s.drums, s.beats[0]))
        .unwrap()
        .1;
    frame(&mut s.a, &ctx, click(beat.center()));
    assert!(!s.a.link_picker.is_open());
    assert_eq!(
        clip_of(&s.a, riff).link,
        clip_of(&s.a, (s.drums, s.beats[0])).link
    );
}

#[test]
fn escape_and_a_click_outside_close_the_picker_and_change_nothing() {
    let mut s = scene();
    let ctx = ctx();
    s.a.attach(&ctx);
    s.a.select_clip(s.bass, s.riff);
    let before = normalised(s.a.project());
    for close in [key(egui::Key::Escape), click(egui::pos2(4.0, 890.0))] {
        s.a.run_action(Action::LinkTo);
        frame(&mut s.a, &ctx, Vec::new());
        frame(&mut s.a, &ctx, Vec::new());
        assert!(s.a.link_picker.is_open());
        frame(&mut s.a, &ctx, close);
        assert!(!s.a.link_picker.is_open());
        assert_eq!(normalised(s.a.project()), before);
    }
}

// ---- the inspector's clip section ---------------------------------------------------------

/// Frames of the inspector alone in a panel `w` wide, with `events` in the first (what they
/// open shows in the next: a new menu is laid out unseen once); the shapes
/// the last one painted.
fn inspect(
    ctx: &egui::Context,
    a: &mut CantusApp,
    w: f32,
    n: usize,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut last = Vec::new();
    for i in 0..n {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(w, 1400.0),
            )),
            events: if i == 0 { events.clone() } else { Vec::new() },
            time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| inspector::show(a, ui));
        });
        out.textures_delta.clear();
        last = out.shapes;
    }
    last
}

/// The visible part of every unrotated text the frame painted.
fn text_rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<(egui::Rect, String)> {
    fn walk(s: &egui::Shape, clip: egui::Rect, out: &mut Vec<(egui::Rect, String)>) {
        match s {
            egui::Shape::Text(t) if t.angle == 0.0 && !t.galley.text().is_empty() => {
                let r = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if r.is_positive() {
                    out.push((r, t.galley.text().to_string()))
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in shapes {
        walk(&s.shape, s.clip_rect, &mut out);
    }
    out
}

/// Where the text `t` was painted (the last place, the topmost).
fn text_at(shapes: &[egui::epaint::ClippedShape], t: &str) -> egui::Pos2 {
    text_rects(shapes)
        .into_iter()
        .rev()
        .find(|(_, s)| s == t)
        .unwrap_or_else(|| panic!("{t:?} not painted: {}", painted_text(shapes)))
        .0
        .center()
}

fn tr(key: &str) -> String {
    cantus_ui::i18n::t(key)
}

#[test]
fn the_inspector_names_a_linked_clips_group_and_offers_link_to_on_a_clip_of_its_own() {
    let mut s = scene();
    let ctx = ctx();
    s.a.select_clip(s.drums, s.beats[1]);
    let text = painted_text(&inspect(&ctx, &mut s.a, 300.0, 3, Vec::new()));
    assert!(text.contains(&tr("links.inspector.row")), "{text}");
    assert!(text.contains("×3"), "{text}");
    let copies = cantus_ui::i18n::tf("links.inspector.copies", &[("n", &3)]);
    assert!(text.contains(&copies), "{text}");
    assert!(text.contains("Beat"), "{text}");
    s.a.select_clip(s.drums, s.melodia);
    let text = painted_text(&inspect(&ctx, &mut s.a, 300.0, 3, Vec::new()));
    assert!(text.contains(&tr("links.inspector.link_to")), "{text}");
    assert!(!text.contains('×'), "{text}");
}

#[test]
fn renaming_from_the_inspector_renames_every_copy_in_one_undo_step() {
    let mut s = scene();
    let ctx = ctx();
    s.a.select_clip(s.drums, s.beats[1]);
    let shapes = inspect(&ctx, &mut s.a, 300.0, 3, Vec::new());
    let at = text_at(&shapes, "Beat");
    inspect(&ctx, &mut s.a, 300.0, 2, click(at));
    // The whole name is selected: typing replaces it.
    inspect(
        &ctx,
        &mut s.a,
        300.0,
        1,
        vec![egui::Event::Text("Groove".into())],
    );
    inspect(&ctx, &mut s.a, 300.0, 2, key(egui::Key::Enter));
    for c in s.beats {
        assert_eq!(clip_of(&s.a, (s.drums, c)).name, "Groove");
    }
    assert_eq!(s.a.bus.undo_label_key(), Some("cmd.set_clip_name"));
    s.a.undo();
    for c in s.beats {
        assert_eq!(clip_of(&s.a, (s.drums, c)).name, "Beat");
    }
    // Escape leaves the name as it was, with nothing to undo.
    let label = s.a.bus.undo_label_key().map(str::to_string);
    let shapes = inspect(&ctx, &mut s.a, 300.0, 2, Vec::new());
    inspect(&ctx, &mut s.a, 300.0, 2, click(text_at(&shapes, "Beat")));
    inspect(
        &ctx,
        &mut s.a,
        300.0,
        1,
        vec![egui::Event::Text("X".into())],
    );
    inspect(&ctx, &mut s.a, 300.0, 2, key(egui::Key::Escape));
    assert_eq!(clip_of(&s.a, (s.drums, s.beats[1])).name, "Beat");
    assert_eq!(s.a.bus.undo_label_key().map(str::to_string), label);
}

#[test]
fn the_inspector_link_menu_selects_the_copies_makes_unique_and_opens_the_picker() {
    let mut s = scene();
    let ctx = ctx();
    let copies = cantus_ui::i18n::tf("links.inspector.copies", &[("n", &3)]);
    // Select linked copies.
    s.a.select_clip(s.drums, s.beats[0]);
    let shapes = inspect(&ctx, &mut s.a, 300.0, 3, Vec::new());
    let shapes = inspect(&ctx, &mut s.a, 300.0, 2, click(text_at(&shapes, &copies)));
    let at = text_at(&shapes, &tr("clip.select_linked"));
    inspect(&ctx, &mut s.a, 300.0, 2, click(at));
    let mut sel = s.a.clip_selection();
    sel.sort();
    let mut all: Vec<_> = s.beats.iter().map(|c| (s.drums, *c)).collect();
    all.sort();
    assert_eq!(sel, all);
    // Make unique, on the one clip the inspector shows.
    s.a.select_clip(s.drums, s.beats[1]);
    let shapes = inspect(&ctx, &mut s.a, 300.0, 3, Vec::new());
    let shapes = inspect(&ctx, &mut s.a, 300.0, 2, click(text_at(&shapes, &copies)));
    let at = text_at(&shapes, &tr("clip.make_unique"));
    inspect(&ctx, &mut s.a, 300.0, 2, click(at));
    assert_eq!(clip_of(&s.a, (s.drums, s.beats[1])).link, None);
    assert_eq!(s.a.bus.undo_label_key(), Some("cmd.make_unique"));
    let two = cantus_ui::i18n::tf("links.inspector.copies", &[("n", &2)]);
    // Link to…, from the menu of a linked clip and from a clip of its own.
    s.a.select_clip(s.drums, s.beats[0]);
    let shapes = inspect(&ctx, &mut s.a, 300.0, 3, Vec::new());
    let shapes = inspect(&ctx, &mut s.a, 300.0, 2, click(text_at(&shapes, &two)));
    let at = text_at(&shapes, &tr("links.inspector.link_to"));
    inspect(&ctx, &mut s.a, 300.0, 2, click(at));
    assert!(s.a.link_picker.is_open());
    assert_eq!(s.a.clip_selection(), vec![(s.drums, s.beats[0])]);
    s.a.link_picker.close();
    s.a.select_clip(s.bass, s.riff);
    let shapes = inspect(&ctx, &mut s.a, 300.0, 3, Vec::new());
    let at = text_at(&shapes, &tr("links.inspector.link_to"));
    inspect(&ctx, &mut s.a, 300.0, 2, click(at));
    assert!(s.a.link_picker.is_open());
    assert_eq!(s.a.clip_selection(), vec![(s.bass, s.riff)]);
}

/// No two texts overlap, in the clip section and in the picker, in English and Spanish, at
/// 75 % and 200 %; the picker stays on screen at 1280×720.
#[test]
fn no_two_texts_collide_in_either_language_at_75_and_200_percent() {
    let collide = |rects: &[(egui::Rect, String)], what: &str| {
        for (i, (r, t)) in rects.iter().enumerate() {
            for (q, u) in &rects[i + 1..] {
                let o = r.intersect(*q);
                assert!(
                    o.width() <= 1.0 || o.height() <= 1.0,
                    "{t:?} {r:?} and {u:?} {q:?} collide ({what})"
                );
            }
        }
    };
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for zoom in [0.75f32, 2.0] {
            for width in [220.0f32, 280.0, 420.0] {
                for linked in [true, false] {
                    let mut s = scene();
                    let ctx = ctx();
                    cantus_ui::i18n::default_thread_locale(locale);
                    ctx.set_pixels_per_point(zoom);
                    let c = if linked { s.beats[1] } else { s.melodia };
                    s.a.select_clip(s.drums, c);
                    let shapes = inspect(&ctx, &mut s.a, width, 3, Vec::new());
                    let what = format!("inspector {width} pt, {zoom}×, {locale:?}, {linked}");
                    collide(&text_rects(&shapes), &what);
                }
            }
            let mut s = scene();
            let ctx = ctx();
            cantus_ui::i18n::default_thread_locale(locale);
            ctx.set_pixels_per_point(zoom);
            s.a.attach(&ctx);
            s.a.select_clip(s.bass, s.riff);
            s.a.run_action(Action::LinkTo);
            let screen = egui::vec2(1280.0, 720.0) / zoom.max(1.0);
            let mut shapes = Vec::new();
            for _ in 0..3 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
                    time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
                    ..Default::default()
                };
                let a = &mut s.a;
                let mut out = ctx.run_ui(input, |ui| {
                    cantus_ui::views::link_picker::frame(a, ui.ctx());
                });
                out.textures_delta.clear();
                shapes = out.shapes;
            }
            let rects = text_rects(&shapes);
            assert!(rects.len() > 8, "the picker painted its rows");
            let what = format!("picker {zoom}×, {locale:?}");
            collide(&rects, &what);
            let panel = s.a.link_picker.panel_rect();
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, screen);
            assert!(
                screen.contains_rect(panel),
                "{panel:?} off {screen:?} ({what})"
            );
        }
    }
}

// ---- the clip's right-click menu ------------------------------------------------------------

fn right_click(at: egui::Pos2) -> Vec<egui::Event> {
    let b = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    vec![egui::Event::PointerMoved(at), b(true), b(false)]
}

/// Every clip's menu offers *Link to…* (a clip of its own as much as a linked one), and it
/// opens the picker for the clip right-clicked.
#[test]
fn the_clip_menu_offers_link_to_and_opens_the_picker_for_the_clip_right_clicked() {
    let mut s = scene();
    let ctx = ctx();
    s.a.attach(&ctx);
    s.a.focus_arrangement();
    s.a.select_clip(s.drums, s.melodia);
    for _ in 0..3 {
        frame(&mut s.a, &ctx, Vec::new());
    }
    let g =
        s.a.arrangement
            .geometry()
            .expect("the arrangement was drawn");
    // The riff: the bass track's first clip, beats 0–4.
    let at = egui::pos2(g.x(2.0), g.rows[1].top + 8.0);
    frame(&mut s.a, &ctx, right_click(at));
    let mut shapes = Vec::new();
    for _ in 0..2 {
        shapes = frame(&mut s.a, &ctx, Vec::new()).shapes;
    }
    let word = tr("clip.link_to");
    // The menu's row, not the inspector's link row of the same words: the nearest.
    let item = text_rects(&shapes)
        .into_iter()
        .filter(|(_, t)| *t == word)
        .map(|(r, _)| r.center())
        .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)))
        .expect("the menu offers Link to…");
    assert!(
        item.distance(at) < 400.0,
        "{item:?} is not in the menu at {at:?}"
    );
    assert_eq!(s.a.clip_selection(), vec![(s.bass, s.riff)]);
    frame(&mut s.a, &ctx, click(item));
    assert!(s.a.link_picker.is_open(), "{}", painted_text(&shapes));
    frame(&mut s.a, &ctx, Vec::new());
    let firsts: Vec<_> =
        s.a.link_picker
            .rows()
            .iter()
            .map(|r| r.group.first)
            .collect();
    assert!(firsts.contains(&(s.drums, s.beats[0])));
}

// ---- a still frame -----------------------------------------------------------------------

struct Counting;
thread_local! {
    // Per thread (round 26): tests measuring at once never mix their numbers.
    static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ALLOCS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static THIS_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNTING.try_with(|c| c.get()).unwrap_or_default() && THIS_THREAD.with(|t| t.get()) {
            {
                let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
            };
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if COUNTING.try_with(|c| c.get()).unwrap_or_default() && THIS_THREAD.with(|t| t.get()) {
            {
                let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
            };
        }
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

/// The clip section of a linked clip — its name with ×N, its link row — formats nothing in
/// a still frame: the inspector's whole frame stays within the bound its own test holds
/// for egui's bookkeeping (`tests/inspector.rs`), and no more than a clip of its own costs.
#[test]
fn a_still_frame_of_a_linked_clip_allocates_no_more_than_a_clip_of_its_own() {
    let mut s = scene();
    let ctx = ctx();
    let run = |a: &mut CantusApp, count: bool| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 1400.0),
            )),
            ..Default::default()
        };
        let mut n = 0;
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    THIS_THREAD.with(|t| t.set(true));
                    ALLOCS.with(|c| c.set(0));
                    COUNTING.with(|c| c.set(count));
                    inspector::show(a, ui);
                    COUNTING.with(|c| c.set(false));
                    THIS_THREAD.with(|t| t.set(false));
                    n = ALLOCS.try_with(|c| c.get()).unwrap_or_default();
                });
        });
        out.textures_delta.clear();
        n
    };
    let still = |a: &mut CantusApp| {
        for _ in 0..8 {
            run(a, false);
        }
        run(a, true)
    };
    s.a.select_clip(s.drums, s.melodia);
    let own = still(&mut s.a);
    s.a.select_clip(s.drums, s.beats[1]);
    let linked = still(&mut s.a);
    eprintln!("inspector, still: {own} allocations (clip of its own), {linked} (linked)");
    assert!(
        linked <= 40,
        "a still frame of a linked clip allocated {linked} times"
    );
    assert!(
        linked <= own + 2,
        "the link rows allocate: {linked} vs {own}"
    );
}

// ---- the key ------------------------------------------------------------------------------

/// *Link to…* has a key in every preset (every clip action does): Alt+N — the link *by
/// name*, Alt and a letter so the typing piano never takes it — and a keymap saved before
/// it (file version 6) gets it where the key is free. Its label is in both languages.
#[test]
fn link_to_has_alt_n_and_an_older_keymap_gets_it() {
    use cantus_ui::keymap::{Binding, Keymap};
    let map = Keymap::default();
    assert_eq!(
        map.bindings_for(Action::LinkTo),
        &[Binding::alt(egui::Key::N)]
    );
    assert!(map.conflicts().is_empty());
    let text: String = map
        .to_toml()
        .lines()
        .filter(|l| !l.trim_start().starts_with("link-to"))
        .map(|l| {
            if l.starts_with("version") {
                "version = 6".to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("version = 6") && !text.contains("link-to"));
    let old = Keymap::from_toml(&text).unwrap();
    assert_eq!(
        old.bindings_for(Action::LinkTo),
        map.bindings_for(Action::LinkTo)
    );
    assert!(old.conflicts().is_empty());
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::default_thread_locale(locale);
        let label = cantus_ui::i18n::t(&Action::LinkTo.label_key());
        assert!(!label.starts_with("action."), "{label} ({locale:?})");
    }
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
}

// ---- round 26's review, and the groups-only switch ------------------------------------------

/// Frames of the picker alone (no app around it) on a 1400×900 window; the last shapes.
fn picker_frames(
    ctx: &egui::Context,
    a: &mut CantusApp,
    n: usize,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut last = Vec::new();
    for i in 0..n {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events: if i == 0 { events.clone() } else { Vec::new() },
            time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            cantus_ui::views::link_picker::frame(a, ui.ctx());
        });
        out.textures_delta.clear();
        last = out.shapes;
    }
    last
}

fn rename(a: &mut CantusApp, r: (TrackId, ClipId), name: &str) {
    a.exec(Command::SetClipName {
        track: r.0,
        clip: r.1,
        name: name.into(),
    });
}

/// A name stored decomposed (an "í" as "i" and a combining accent, as macOS file names
/// are) is found by "melodia" and by "melodía", both ways.
#[test]
fn the_filter_finds_a_decomposed_name_and_a_decomposed_filter() {
    let mut s = scene();
    rename(&mut s.a, (s.drums, s.melodia), "Melodi\u{301}a");
    let sel = [(s.bass, s.riff)];
    for f in ["melodia", "MELODÍA", "melodi\u{301}a"] {
        let names: Vec<_> = picker_rows(s.a.project(), &sel, f)
            .into_iter()
            .map(|r| r.group.first)
            .collect();
        assert_eq!(names, vec![(s.drums, s.melodia)], "{f:?}");
    }
}

/// A clip with no name reads "Untitled clip" in the lead line and the hint bar, as its row
/// does — never “”.
#[test]
fn an_unnamed_clip_is_untitled_in_the_lead_line_too() {
    let mut s = scene();
    let ctx = ctx();
    s.a.attach(&ctx);
    rename(&mut s.a, (s.bass, s.riff), "");
    // The command refuses nothing: an empty name can come from a file too.
    s.a.select_clip(s.bass, s.riff);
    s.a.run_action(Action::LinkTo);
    let text = painted_text(&picker_frames(&ctx, &mut s.a, 3, Vec::new()));
    let untitled = tr("links.picker.unnamed");
    let lead = cantus_ui::i18n::tf("links.picker.lead_one", &[("name", &untitled)]);
    assert!(text.contains(&lead), "{text}");
    assert!(!text.contains("“”"), "{text}");
}

/// A long name is cut, the verb never: the lead line always ends with what will happen.
#[test]
fn a_long_name_is_cut_in_the_lead_line_and_the_verb_stays() {
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        let mut s = scene();
        let ctx = ctx();
        cantus_ui::i18n::default_thread_locale(locale);
        s.a.attach(&ctx);
        let long = "Beat A with an exceptionally long clip name that must be cut somewhere";
        rename(&mut s.a, (s.bass, s.riff), long);
        s.a.select_clip(s.bass, s.riff);
        s.a.run_action(Action::LinkTo);
        let shapes = picker_frames(&ctx, &mut s.a, 3, Vec::new());
        let template = tr("links.picker.lead_one");
        let tail = template.split("{name}").nth(1).unwrap().to_string();
        let lead = text_rects(&shapes)
            .into_iter()
            .map(|(_, t)| t)
            .find(|t| t.ends_with(&tail))
            .unwrap_or_else(|| panic!("no lead ending {tail:?}: {}", painted_text(&shapes)));
        assert!(lead.contains('…'), "{lead}");
        assert!(lead.starts_with(template.split("{name}").next().unwrap()));
        cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    }
}

/// The filter's ring is the well's: the field the focus ring outlines is the whole well,
/// and the text sits inside it with the kit's margin, not against its edge.
#[test]
fn the_filter_field_is_one_frame_with_its_text_inset() {
    let mut s = scene();
    let ctx = ctx();
    s.a.attach(&ctx);
    s.a.select_clip(s.bass, s.riff);
    s.a.run_action(Action::LinkTo);
    let shapes = picker_frames(&ctx, &mut s.a, 3, Vec::new());
    let focused = ctx
        .memory(|m| m.focused())
        .expect("the filter has the focus");
    let field = ctx.read_response(focused).expect("drawn").rect;
    let panel = s.a.link_picker.panel_rect();
    // The well starts at the panel's inner edge and is a whole row high (24 pt).
    assert!(
        (field.left() - (panel.left() + 12.0)).abs() < 0.5
            && field.height() >= 23.5
            && field.width() > 200.0,
        "the focused rect {field:?} is the well, in {panel:?}"
    );
    let hint = tr("links.picker.filter");
    let at = text_rects(&shapes)
        .into_iter()
        .find(|(_, t)| *t == hint)
        .expect("the placeholder")
        .0;
    assert!(at.left() - field.left() >= 5.0, "{at:?} against {field:?}");
    assert!(field.contains_rect(at.shrink(0.5)));
}

/// *Linked only* narrows the list to the link groups; it stays as set for the next
/// opening (for the session); Enter still links the first row that links.
#[test]
fn linked_only_shows_the_groups_and_is_kept_for_the_session() {
    let mut s = scene();
    let ctx = ctx();
    s.a.attach(&ctx);
    s.a.select_clip(s.drums, s.melodia);
    s.a.run_action(Action::LinkTo);
    let shapes = picker_frames(&ctx, &mut s.a, 3, Vec::new());
    assert_eq!(s.a.link_picker.rows().len(), 5);
    let word = tr("links.picker.groups_only");
    let at = text_rects(&shapes)
        .into_iter()
        .find(|(_, t)| t.to_uppercase() == word.to_uppercase())
        .unwrap_or_else(|| panic!("{word:?} not painted: {}", painted_text(&shapes)))
        .0
        .center();
    picker_frames(&ctx, &mut s.a, 2, click(at));
    assert!(s.a.link_picker.groups_only());
    let firsts: Vec<_> =
        s.a.link_picker
            .rows()
            .iter()
            .map(|r| r.group.first)
            .collect();
    assert_eq!(firsts, vec![(s.drums, s.beats[0]), (s.vocal, s.takes[0])]);
    s.a.link_picker.close();
    s.a.run_action(Action::LinkTo);
    picker_frames(&ctx, &mut s.a, 2, Vec::new());
    assert!(s.a.link_picker.groups_only(), "kept while the program runs");
    assert_eq!(s.a.link_picker.rows().len(), 2);
    picker_frames(&ctx, &mut s.a, 1, key(egui::Key::Enter));
    let m = clip_of(&s.a, (s.drums, s.melodia));
    assert_eq!(m.link, clip_of(&s.a, (s.drums, s.beats[0])).link);
}

/// The inspector's link menu shows each row's key, as the arrangement's menus do (a row
/// that runs a keymap action shows its binding); and the menu is wide enough for both.
#[test]
fn the_inspector_link_menu_shows_the_keys() {
    let mut s = scene();
    let ctx = ctx();
    s.a.select_clip(s.drums, s.beats[0]);
    let copies = cantus_ui::i18n::tf("links.inspector.copies", &[("n", &3)]);
    let shapes = inspect(&ctx, &mut s.a, 300.0, 3, Vec::new());
    let shapes = inspect(&ctx, &mut s.a, 300.0, 2, click(text_at(&shapes, &copies)));
    let text = painted_text(&shapes);
    for (action, label) in [
        (Action::SelectLinked, "clip.select_linked"),
        (Action::MakeUnique, "clip.make_unique"),
        (Action::LinkTo, "links.inspector.link_to"),
    ] {
        let key = s.a.keymap.bindings_for(action)[0].display();
        assert!(
            text.contains(&format!("| {key} |")),
            "{key} missing: {text}"
        );
        let rects = text_rects(&shapes);
        let row = rects.iter().find(|(_, t)| *t == tr(label)).unwrap().0;
        let k = rects.iter().find(|(_, t)| *t == key).unwrap().0;
        assert!(!row.intersects(k), "{label} and {key} overlap");
    }
}
