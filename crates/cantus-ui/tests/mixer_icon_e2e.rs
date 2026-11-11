//! r39:lo-mixer — a track's own icon on the mixer's strip head (ADR 0113 §4, the icons
//! sheet's "Mixer strip head" row), end to end through `CantusApp::assemble` as the app
//! starts: the icon is chosen as a user chooses it — the track's menu in the arrangement ▸
//! *Icon…*, typed into the picker's search, a click on its cell — and the mixer docked
//! below then paints it on that track's strip head, in every density, inside the head;
//! undone, it is gone from the head too.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_engine::EngineHandle;
use cantus_ui::app::{CentralView, Files};
use cantus_ui::views::mixer::{self, layout::WidthMode, Control};
use cantus_ui::CantusApp;
use egui::{pos2, Pos2, Rect};

struct W {
    app: CantusApp,
    ctx: egui::Context,
    t: f64,
    texts: Vec<(String, Rect)>,
    /// Every white-tinted vertex painted last frame: (where, its atlas coordinate).
    quads: Vec<(Pos2, egui::Pos2)>,
    _dir: tempfile::TempDir,
}

impl W {
    fn new() -> W {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
        std::env::set_var("CANTUS_DATA_DIR", dir.path());
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let files = Files::read(dir.path(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, _proc) = EngineHandle::manual(48_000.0);
        let project = cantus_ui::demos::startup_project();
        let mut app =
            CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
        // The arrangement above, the mixer docked below it: the window a user works in.
        app.show_central(CentralView::Mixer);
        let mut w = W {
            app,
            ctx,
            t: 0.0,
            texts: Vec::new(),
            quads: Vec::new(),
            _dir: dir,
        };
        for _ in 0..4 {
            w.frame(vec![]);
        }
        w
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1920.0, 1080.0))),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        fn walk(
            shape: &egui::Shape,
            texts: &mut Vec<(String, Rect)>,
            quads: &mut Vec<(Pos2, egui::Pos2)>,
        ) {
            match shape {
                egui::Shape::Text(t) => texts.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Mesh(m) => {
                    // A layer quad's corners: white-tinted vertices with atlas coordinates
                    // (a solid fill's vertices all sit on the atlas's one white texel).
                    for v in &m.vertices {
                        if v.color.r() == v.color.a() && v.color.a() > 0 {
                            quads.push((v.pos, v.uv));
                        }
                    }
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, texts, quads)),
                _ => {}
            }
        }
        self.texts.clear();
        self.quads.clear();
        for c in &out.shapes {
            walk(&c.shape, &mut self.texts, &mut self.quads);
        }
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(vec![]);
        }
    }

    fn word(&self, word: &str) -> Option<Rect> {
        self.texts
            .iter()
            .rev()
            .find(|(t, _)| t.trim() == word)
            .map(|(_, r)| *r)
    }

    fn press(&mut self, at: Pos2, button: egui::PointerButton) {
        self.frame(vec![egui::Event::PointerMoved(at)]);
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerButton {
                pos: at,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        self.frame(vec![]);
    }

    fn click(&mut self, at: Pos2) {
        self.press(at, egui::PointerButton::Primary);
    }

    /// The head's icon of `strip` as the desk drew it, and a textured quad painted there.
    fn painted(&self, strip: cantus_core::StripId) -> Option<(Rect, &'static str)> {
        let (at, id) = self.app.mixer.head_icon(strip)?;
        let inside: Vec<egui::Pos2> = self
            .quads
            .iter()
            .filter(|(p, _)| at.expand(1.0).contains(*p))
            .map(|(_, uv)| *uv)
            .collect();
        let spread = inside
            .iter()
            .any(|uv| inside.iter().any(|w| (uv.x - w.x).abs() > 1e-4));
        let quad = inside.len() >= 4 && spread;
        assert!(quad, "a textured quad painted in the icon's place {at:?}");
        Some((at, id))
    }
}

#[test]
fn an_icon_chosen_in_the_picker_is_painted_on_the_strip_head() {
    let mut w = W::new();
    let i = w
        .app
        .project()
        .tracks
        .iter()
        .position(|t| t.name == "Bass")
        .expect("the demo's Bass");
    let id = w.app.project().tracks[i].id;
    let strip = w.app.project().tracks[i]
        .strip
        .expect("Bass plays into a strip");
    assert!(
        w.app.mixer.head_icon(strip).is_none(),
        "no icon before one is chosen"
    );

    // The track's menu (a right-click on its header) ▸ Icon… opens the picker.
    let h = *w
        .app
        .arrangement
        .header_hits()
        .iter()
        .find(|h| h.track == id)
        .expect("its header drawn");
    let name = h.letters.expect("its name drawn");
    w.press(
        pos2(name.right() + 10.0, name.center().y),
        egui::PointerButton::Secondary,
    );
    let row = w.word("Icon…").expect("the track's menu offers Icon…");
    w.click(row.center());
    assert!(w.app.arrangement.glyphs.open.is_some(), "the picker opened");
    // Typed into its search, as a user finds it.
    let field = w.word("Search icons").expect("the search field");
    w.click(field.center());
    w.frame(vec![egui::Event::Text("piano".into())]);
    w.frames(1);
    let cell = w
        .app
        .arrangement
        .glyphs
        .drawn
        .iter()
        .find(|(g, _)| *g == Some("piano"))
        .map(|(_, r)| *r)
        .expect("the picker drew the piano");
    w.click(cell.center());
    assert_eq!(
        w.app.project().track(id).unwrap().glyph.as_deref(),
        Some("piano"),
        "picked"
    );
    w.frames(2);

    // On the strip head, in each density, inside the head, beside its name.
    for mode in [WidthMode::Compact, WidthMode::Wide, WidthMode::Xl] {
        let mut prefs = w.app.mixer.prefs();
        prefs.width = mode;
        w.app.mixer.set_prefs(prefs);
        w.frames(3);
        let (at, g) = w
            .painted(strip)
            .unwrap_or_else(|| panic!("{mode:?}: the head drew the icon"));
        assert_eq!(g, "piano", "{mode:?}");
        let head = mixer::control_rect(&w.ctx, Some(strip), Control::Head)
            .unwrap_or_else(|| panic!("{mode:?}: the head"));
        assert!(
            head.expand(0.5).contains_rect(at),
            "{mode:?}: the icon {at:?} inside the head {head:?}"
        );
        let side = if mode == WidthMode::Compact {
            12.0
        } else {
            14.0
        };
        assert!((at.width() - side).abs() < 0.01, "{mode:?}: {side} across");
        // Nothing else on the desk wears it.
        let others = w
            .app
            .project()
            .strips
            .iter()
            .filter(|s| s.id != strip && w.app.mixer.head_icon(s.id).is_some())
            .count();
        assert_eq!(others, 0, "{mode:?}: only Bass's strip");
        if mode != WidthMode::Compact {
            // The name stays whole beside it, and does not run into it.
            let words = w
                .texts
                .iter()
                .filter(|(t, r)| t == "Bass" && head.contains(r.center()))
                .map(|(_, r)| *r)
                .next()
                .unwrap_or_else(|| panic!("{mode:?}: the name on the head"));
            assert!(
                words.left() >= at.right(),
                "{mode:?}: the icon before the name"
            );
        }
    }

    // Undone, it leaves the head as it leaves the header.
    w.app.undo();
    w.frames(3);
    assert!(w.app.mixer.head_icon(strip).is_none(), "undone");
}

/// r39:lo-mixer — des-mixer's header shares the workspace's tab row: docked under the
/// arrangement, the density's words sit on the line of the Mixer tab, right of the tabs,
/// and the desk's first row starts right under the bar (no row of its own); a click on
/// *Large* there still sets the density, and the tab still drags (panels.rs holds moving,
/// folding and maximising the areas).
#[test]
fn the_header_shares_the_tab_row() {
    let mut w = W::new();
    let compact = w.word("Compact").expect("the density");
    let tab = w
        .texts
        .iter()
        .filter(|(t, r)| t == "Mixer" && r.right() < compact.left())
        .map(|(_, r)| *r)
        .min_by(|a, b| {
            (a.center().y - compact.center().y)
                .abs()
                .total_cmp(&(b.center().y - compact.center().y).abs())
        })
        .expect("the Mixer tab");
    assert!(
        (compact.center().y - tab.center().y).abs() < 3.0,
        "the density {compact:?} on the tab's line {tab:?}"
    );
    assert!(compact.left() > tab.right(), "right of the tabs");
    let large = w.word("Large").expect("Large");
    w.click(large.center());
    w.frames(2);
    assert_eq!(w.app.mixer.prefs().width, WidthMode::Xl, "the click set it");
}

/// r39:lo-mixer — the inspector's Automation ▸ *Show in the arrangement*, end to end: a
/// strip chosen on the desk whose track (the last, out of sight below the arrangement's
/// rows) has a closed automation lane; the link opens the track's lanes (one undo step),
/// scrolls the arrangement until the track and its lanes are in view, and gives the
/// arrangement the dock's focus and the keys'.
#[test]
fn show_in_the_arrangement_reveals_the_tracks_lanes() {
    use cantus_core::{AutoTarget, Command};
    let mut w = W::new();
    let i = w.app.project().tracks.len() - 1;
    let track = w.app.project().tracks[i].id;
    let strip = w.app.project().tracks[i].strip.expect("its strip");
    w.app.select_track(track);
    assert!(w.app.create_automation_clip(AutoTarget::StripGain(strip)));
    w.app.exec(Command::SetTrackAutomationOpen {
        id: track,
        open: false,
    });
    w.app.arrangement.scroll_y = 0.0;
    w.app.select_strip(strip);
    w.app.focus = cantus_ui::app::EditFocus::Mixer;
    cantus_ui::views::inspector::unfold(&w.ctx, "inspector.automation");
    w.frames(3);
    let geo = w.app.arrangement.geometry().expect("the arrangement drawn");
    let row = geo.rows[i];
    assert!(
        row.top >= geo.lanes_bottom,
        "the track starts out of sight: {} >= {}",
        row.top,
        geo.lanes_bottom
    );
    let steps = w.app.bus.history().len();
    let link = w
        .word("Show in the arrangement")
        .expect("Automation offers the link");
    w.click(link.center());
    w.frames(3);
    let p = w.app.project();
    assert!(p.track(track).unwrap().automation_open, "its lanes open");
    assert_eq!(w.app.bus.history().len(), steps + 1, "one undo step");
    let geo = w.app.arrangement.geometry().expect("the arrangement drawn");
    let row = geo.rows[i];
    assert!(row.auto > 0.0, "its lanes have room");
    assert!(
        row.top >= geo.body_top - 0.5 && row.top + row.height <= geo.lanes_bottom + 0.5,
        "the track in view: {:?} in {}..{}",
        row,
        geo.body_top,
        geo.lanes_bottom
    );
    assert_eq!(
        w.app.workspace.focused_area(),
        Some(cantus_ui::views::workspace::Pane::Arrangement),
        "the arrangement has the dock's focus"
    );
    assert_eq!(w.app.focus, cantus_ui::app::EditFocus::Arrangement);
    w.app.undo();
    assert!(
        !w.app.project().track(track).unwrap().automation_open,
        "undone"
    );
}
