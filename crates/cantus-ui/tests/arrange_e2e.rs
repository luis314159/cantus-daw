//! r39:el-arrange — the arrangement's structure end to end (ADR 0113), through
//! `CantusApp::assemble` as the app starts, the whole window drawn: a folder made from a
//! track's menu path, closed by its chevron (its members hidden) and opened again; a
//! section added, dragged along the bars and renamed; an automation lane's own mode, which
//! the write pass honours; a track's own icon picked in the picker, shown before its name,
//! undone; and every one of them saved and loaded back.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::arrange::ArrangeCommand;
use cantus_core::{AutoMode, AutoTarget, Command, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::views::arrangement::{folders, ruler};
use cantus_ui::CantusApp;
use egui::{pos2, Pos2};
use std::path::Path;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
        d
    })
    .path()
}

struct W {
    app: CantusApp,
    ctx: egui::Context,
    t: f64,
    /// The words painted last frame and where.
    texts: Vec<(String, egui::Rect)>,
}

impl W {
    fn new() -> W {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, _proc) = EngineHandle::manual(48_000.0);
        let project = cantus_ui::demos::startup_project();
        let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
        let mut w = W {
            app,
            ctx,
            t: 0.0,
            texts: Vec::new(),
        };
        for _ in 0..4 {
            w.frame(vec![]);
        }
        w
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1920.0, 1080.0),
            )),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        self.texts.clear();
        for c in &out.shapes {
            walk(&c.shape, &mut self.texts);
        }
    }

    /// Where `word` was painted last frame (the last one drawn: a menu over the rest).
    fn word(&self, word: &str) -> Option<egui::Rect> {
        self.texts
            .iter()
            .rev()
            .find(|(t, _)| t.trim() == word)
            .map(|(_, r)| *r)
    }

    fn right_click(&mut self, at: Pos2) {
        let b = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        self.frame(vec![egui::Event::PointerMoved(at)]);
        self.frame(vec![b(true)]);
        self.frame(vec![b(false)]);
        self.frame(vec![]);
    }

    fn button(at: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(at)]);
        self.frame(vec![W::button(at, true)]);
        self.frame(vec![W::button(at, false)]);
        self.frame(vec![]);
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(from)]);
        self.frame(vec![W::button(from, true)]);
        for i in 1..=6 {
            let p = from + (to - from) * (i as f32 / 6.0);
            self.frame(vec![egui::Event::PointerMoved(p)]);
        }
        self.frame(vec![W::button(to, false)]);
        self.frame(vec![]);
    }

    fn geo(&self) -> cantus_ui::views::arrangement::TimelineGeometry {
        self.app
            .arrangement
            .geometry()
            .expect("the arrangement was drawn")
    }

    /// Save the project and load it back.
    fn round_trip(&self) -> cantus_core::Project {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.cantus");
        cantus_project::save(self.app.project(), &path, "test").unwrap();
        cantus_project::load(&path, false).unwrap().project
    }
}

#[test]
fn the_arrangements_structure_end_to_end() {
    let mut w = W::new();
    folders_fold_and_come_back(&mut w);
    sections_are_made_dragged_and_kept(&mut w);
    a_lane_has_its_own_mode_and_the_pass_honours_it(&mut w);
    a_tracks_own_icon_is_picked_shown_undone_and_kept(&mut w);
}

fn folders_fold_and_come_back(w: &mut W) {
    let (a, b) = (w.app.project().tracks[0].id, w.app.project().tracks[1].id);
    let f = folders::new_folder(&mut w.app, &[a, b], "Drums").expect("a folder made");
    w.frame(vec![]);
    let g = w.geo();
    let row = g
        .folders
        .iter()
        .find(|r| r.id == f)
        .copied()
        .expect("its row drawn");
    assert!(
        g.rows[0].top >= row.top + row.height - 0.5,
        "its members under it"
    );
    let hit = |w: &W, i: usize| {
        let id = w.app.project().tracks[i].id;
        w.app
            .arrangement
            .header_hits()
            .iter()
            .find(|h| h.track == id)
            .copied()
    };
    let member = hit(w, 0).expect("a member's header drawn");
    let outside = hit(w, 4).expect("another track's header drawn");
    let (ml, ol) = (
        member.letters.unwrap().left(),
        outside.letters.unwrap().left(),
    );
    assert!(ml > ol + 6.0, "a member is indented a step: {ml} vs {ol}");
    // The chevron closes it: the members hidden, the folder's row a full one.
    let chevron = pos2(row_left(w) + 20.0, row.top + row.height * 0.5);
    w.click(chevron);
    assert!(
        !w.app.project().folder(f).unwrap().open,
        "the chevron closed it"
    );
    let g = w.geo();
    assert_eq!(g.rows[0].height, 0.0, "a closed folder's member is hidden");
    assert!(hit(w, 0).is_none(), "and has no header");
    w.app.undo();
    w.frame(vec![]);
    assert!(w.app.project().folder(f).unwrap().open, "undone");
    let back = w.round_trip();
    assert_eq!(back.folders, w.app.project().folders);
    assert_eq!(back.tracks[0].folder, Some(f));
}

fn row_left(w: &W) -> f32 {
    let g = w.geo();
    g.left - w.app.arrangement.header_shown() - 1.0
}

fn sections_are_made_dragged_and_kept(w: &mut W) {
    let bar = Ticks::beats(4);
    let id = ruler::sections::add_here(&mut w.app, Ticks(bar.0 * 4)).expect("a section");
    w.app.arrangement.sections.rename = None;
    w.frame(vec![]);
    w.frame(vec![]);
    let (_, r) = *w
        .app
        .arrangement
        .sections
        .drawn
        .iter()
        .find(|(s, _)| *s == id)
        .expect("drawn in its lane");
    let s0 = w.app.project().section(id).unwrap().clone();
    assert_eq!((s0.start, s0.length), (Ticks(bar.0 * 4), Ticks(bar.0 * 4)));
    // Dragged two bars right by its body.
    let g = w.geo();
    let from = pos2(r.left() + 12.0, r.center().y);
    let to = from + egui::vec2(g.px_per_beat * 8.0, 0.0);
    w.drag(from, to);
    let s1 = w.app.project().section(id).unwrap().clone();
    assert_eq!(s1.start, Ticks(bar.0 * 6), "moved along the bars");
    assert_eq!(s1.length, s0.length, "its length kept");
    w.app.undo();
    assert_eq!(
        w.app.project().section(id).unwrap().start,
        s0.start,
        "one undo step"
    );
    let back = w.round_trip();
    assert_eq!(back.sections, w.app.project().sections);
}

fn a_lane_has_its_own_mode_and_the_pass_honours_it(w: &mut W) {
    let t = w.app.project().tracks[5].id;
    let target = AutoTarget::TrackGain(t);
    assert_eq!(w.app.own_auto_mode(&target), AutoMode::Read);
    w.app.exec(Command::Arrange(ArrangeCommand::SetLaneMode {
        track: t,
        target: target.clone(),
        mode: Some(AutoMode::Touch),
    }));
    w.frame(vec![]);
    assert_eq!(
        w.app.own_auto_mode(&target),
        AutoMode::Touch,
        "the lane's own"
    );
    assert_eq!(
        w.app.own_auto_mode(&AutoTarget::TrackPan(t)),
        AutoMode::Read,
        "its neighbour reads"
    );
    assert!(w.app.any_writing_mode());
    let back = w.round_trip();
    assert_eq!(
        back.tracks[5].lane_modes,
        w.app.project().tracks[5].lane_modes
    );
    w.app.undo();
    assert_eq!(w.app.own_auto_mode(&target), AutoMode::Read);
}

fn a_tracks_own_icon_is_picked_shown_undone_and_kept(w: &mut W) {
    let i = 6;
    let id = w.app.project().tracks[i].id;
    let h = *w
        .app
        .arrangement
        .header_hits()
        .iter()
        .find(|h| h.track == id)
        .expect("its header drawn");
    let before = h.letters.unwrap().left();
    // The track's menu (a right-click on its header) ▸ Icon… opens the picker; a click on a
    // cell picks.
    let body = pos2(
        h.letters.unwrap().right() + 10.0,
        h.letters.unwrap().center().y,
    );
    w.right_click(body);
    let row = w.word("Icon…").expect("the track's menu offers Icon…");
    w.click(row.center());
    assert!(w.app.arrangement.glyphs.open.is_some(), "the picker opened");
    // r39:integration — set 6's library (316 icons in 17 families): the star sits far down
    // in Marks & shapes, so it is found the way a user finds it, typed in Spanish into the
    // search the picker opens with.
    assert!(w.word("Cymbals").is_some(), "the families are painted");
    assert!(
        w.word("Recent").is_none(),
        "nothing chosen yet: no recent row"
    );
    let field = w.word("Search icons").expect("the search field");
    w.click(field.center());
    w.frame(vec![egui::Event::Text("estrella".into())]);
    w.frame(vec![]);
    assert_eq!(
        w.app.arrangement.glyphs.search, "estrella",
        "the search had the keys"
    );
    let cell = w
        .app
        .arrangement
        .glyphs
        .drawn
        .iter()
        .find(|(g, _)| *g == Some("star"))
        .map(|(_, r)| *r)
        .expect("the picker drew the star");
    w.click(cell.center());
    assert_eq!(
        w.app.project().track(id).unwrap().glyph.as_deref(),
        Some("star")
    );
    assert!(w.app.arrangement.glyphs.open.is_none(), "a pick closes it");
    let h = *w
        .app
        .arrangement
        .header_hits()
        .iter()
        .find(|h| h.track == id)
        .unwrap();
    assert!(
        h.letters.unwrap().left() > before + 8.0,
        "the name gives the icon its place"
    );
    assert!(cantus_ui::views::track_glyphs::track_icon(&w.app, id).is_some());
    // A click on the icon opens the picker again (to change it); Esc-free: a click away.
    w.click(h.glyph.center());
    assert!(
        w.app.arrangement.glyphs.open.is_some(),
        "the icon reopens the picker"
    );
    // The recent row, at the top, holds what was chosen.
    assert!(w.word("Recent").is_some(), "the recent row");
    let first = w
        .app
        .arrangement
        .glyphs
        .drawn
        .iter()
        .find(|(g, _)| g.is_some());
    assert_eq!(
        first.map(|(g, _)| *g),
        Some(Some("star")),
        "the star leads it"
    );
    // r39:fix2 — the click away lands where the desk paints its next empty strips (a
    // click there makes strips, round 39's lo-mixer): it closes the picker and does
    // nothing else, so Undo below takes back the icon, not strips made by that click.
    let strips = w.app.project().strips.len();
    let ghosts = w.app.mixer.next_strips_shown();
    w.click(pos2(1000.0, 700.0));
    assert!(
        w.app.arrangement.glyphs.open.is_none(),
        "a click away closes it"
    );
    assert_eq!(
        w.app.project().strips.len(),
        strips,
        "the click away made nothing beneath it ({ghosts} empty strips were painted)"
    );
    let back = w.round_trip();
    assert_eq!(
        back.tracks[i].glyph.as_deref(),
        Some("star"),
        "saved and loaded"
    );
    let strip = w.app.project().track(id).unwrap().strip.expect("its strip");
    w.frame(vec![]);
    assert_eq!(
        w.app.mixer.head_icon(strip).map(|(_, g)| g),
        Some("star"),
        "the strip's head shows the icon"
    );
    w.app.undo();
    w.frame(vec![]);
    w.frame(vec![]);
    assert_eq!(w.app.project().track(id).unwrap().glyph, None, "undone");
    assert!(
        cantus_ui::views::track_glyphs::track_icon(&w.app, id).is_none(),
        "the header's icon goes with it"
    );
    assert!(
        w.app.mixer.head_icon(strip).map(|(_, g)| g) != Some("star"),
        "and the strip head's"
    );
}
