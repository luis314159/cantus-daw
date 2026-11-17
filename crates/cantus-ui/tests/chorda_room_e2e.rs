//! Chorda's room chip end to end (round 40 polish, the owner: "the room card covers the
//! piano"), on the boot path — an app made by `CantusApp::assemble`:
//!
//! - By default nothing lies over the piano (its body, lid, keys and bench; its mics when
//!   shown): the pickers, *Show mics* and the room's chip stand clear of it, for a grand and
//!   an upright, the model and a recorded source, on three screen sizes.
//! - The chip opens the room card under it; a room chosen in it changes the picture (the
//!   look's room, one undo step) and never the sound (the Room is the mixer's); Esc closes
//!   it, a click outside closes it (a click on the lid still opens or closes the lid), and
//!   the chip closes it.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::chorda_look::{self as look, Body};
use cantus_core::inst::room_look;
use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::Files;
use cantus_ui::views::inst::chorda::view as cv;
use cantus_ui::CantusApp;
use std::path::Path;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
        )
        .unwrap();
        d
    })
    .path()
}

struct Rig {
    app: CantusApp,
    proc_: Processor,
    ctx: egui::Context,
    t: f64,
    size: egui::Vec2,
}

impl Rig {
    fn new(project: Project, size: egui::Vec2) -> Rig {
        let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::scene::set_blocking(true);
        let (engine, proc_) = EngineHandle::manual(48_000.0);
        let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
        Rig {
            app,
            proc_,
            ctx,
            t: 0.0,
            size,
        }
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        let mut buf = vec![0f32; 800 * 2];
        self.proc_.process(&mut buf, 2);
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        self.ctx
            .run_ui(input, |ui| app.frame(ui))
            .textures_delta
            .clear();
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn click(&mut self, at: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(at)]);
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        self.idle(1);
    }

    fn spec(&self, track: TrackId) -> NativeSpec {
        self.app.native_of(track).unwrap()
    }
}

/// A song with one Chorda track, Piano, its editor open on the Play view.
fn piano_rig(size: egui::Vec2, body: Body, recorded: bool) -> (Rig, TrackId) {
    let mut p = Project::new("Chorda");
    let mut t = p.make_track("Piano");
    t.strip = Some(p.strips[0].id);
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda));
    let id = t.id;
    p.tracks.push(t);
    let mut r = Rig::new(p, size);
    r.app
        .set_inst_text(id, look::BODY_KEY, Some(body.key().to_string()));
    if recorded {
        r.app.set_inst_text(
            id,
            cantus_core::inst::chorda_source::SOURCE_KEY,
            Some("pack:salamander-grand/grand".into()),
        );
    }
    r.app.open_inst_editor(id);
    r.app.inst.view = cantus_ui::views::inst::View::Play;
    r.idle(4);
    (r, id)
}

fn style() -> cantus_ui::kit::Style {
    cantus_ui::kit::Style::new(cantus_ui::kit::Tokens::slate())
}

#[test]
fn the_piano_shows_whole_with_the_room_card_closed() {
    let mut bad = Vec::new();
    let mut seen = 0;
    for body in [Body::Grand, Body::Upright] {
        for recorded in [false, true] {
            for size in [
                egui::vec2(1280.0, 720.0),
                egui::vec2(1600.0, 900.0),
                egui::vec2(1920.0, 1080.0),
            ] {
                let (mut r, piano) = piano_rig(size, body, recorded);
                let at = format!("{body:?} recorded {recorded} at {size}");
                let Some(pic) = cv::picture_rect(&r.ctx, piano) else {
                    bad.push(format!("{at}: no picture"));
                    continue;
                };
                assert!(
                    cv::room_rects(&r.ctx, piano).1.is_none(),
                    "{at}: closed by default"
                );
                let over = cv::overlay_rects(&r.ctx, piano);
                assert!(
                    cv::room_rects(&r.ctx, piano).0.is_some(),
                    "{at}: the chip is drawn"
                );
                for mics in [false, true] {
                    if mics {
                        // *Show mics*: the mics are the piano's too.
                        let mics_key = over[1];
                        r.click(mics_key.center());
                    }
                    let spec = r.spec(piano);
                    let v = cv::view_of(&spec, &style(), mics, None);
                    let pts = cantus_ui::scene::instrument_points(pic, &v);
                    seen += pts.len();
                    let hit: Vec<_> = pts
                        .iter()
                        .filter(|q| over.iter().any(|o| o.expand(1.0).contains(**q)))
                        .collect();
                    if !hit.is_empty() {
                        bad.push(format!(
                            "{at} mics {mics}: {} of {} points of the piano under an overlay \
                             ({:?}; overlays {over:?})",
                            hit.len(),
                            pts.len(),
                            hit[0]
                        ));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert!(seen > 2000, "{seen} points of pianos seen");
}

#[test]
fn the_room_chip_opens_the_card_and_the_room_changes_through_it() {
    use cantus_ui::views::inst::room_card::probe;
    let (mut r, piano) = piano_rig(egui::vec2(1600.0, 900.0), Body::Grand, false);
    // The lid up on its full stick: a click on it later closes it. The editor maximised:
    // in a small window the open card covers the whole lid.
    r.app.set_inst_value(piano, "lid", 1.0, false);
    let key = r
        .app
        .instrument_window_key(piano)
        .expect("the editor's window");
    cantus_ui::views::workspace_device::toggle_maximise(&r.ctx, key);
    r.idle(4);
    let pic = cv::picture_rect(&r.ctx, piano).unwrap();
    let chip = cv::room_rects(&r.ctx, piano).0.expect("the chip");
    let before = r.spec(piano);
    // Open: the card under the chip, inside the picture.
    r.click(chip.center());
    assert!(cv::room_open(&r.app, piano), "the chip opens it");
    probe::start();
    r.idle(1);
    let seen = probe::take();
    let card = cv::room_rects(&r.ctx, piano).1.expect("the card");
    assert!(pic.expand(0.5).contains_rect(card), "{card:?} in {pic:?}");
    assert!(card.top() >= chip.bottom(), "under the chip");
    let rooms = seen.last().expect("the card was drawn").rooms;
    // Studio, the third of five rooms.
    let studio = egui::pos2(rooms.left() + rooms.width() * 2.5 / 5.0, rooms.center().y);
    r.click(studio);
    let spec = r.spec(piano);
    assert_eq!(
        spec.text(room_look::ROOM_KEY),
        Some("studio"),
        "seen in the studio"
    );
    for k in ["room", "roomsize"] {
        assert_eq!(
            spec.get(k),
            before.get(k),
            "{k}: the picture only, never the sound"
        );
    }
    assert!(cv::room_open(&r.app, piano), "a choice leaves it open");
    let chip = cv::room_rects(&r.ctx, piano).0.unwrap();
    // Esc closes it.
    r.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    r.idle(1);
    assert!(!cv::room_open(&r.app, piano), "Esc closes it");
    assert!(cv::room_rects(&r.ctx, piano).1.is_none());
    // A click outside it, on the empty floor, closes it and changes nothing.
    r.click(chip.center());
    assert!(cv::room_open(&r.app, piano));
    let lid = r.spec(piano).get("lid");
    r.click(pic.left_bottom() + egui::vec2(10.0, -10.0));
    assert!(!cv::room_open(&r.app, piano), "a click outside closes it");
    assert_eq!(r.spec(piano).get("lid"), lid, "and touches nothing");
    // A click on the lid while it is open closes it, and the lid answers as ever.
    r.click(chip.center());
    assert!(cv::room_open(&r.app, piano));
    let sc = r.app.inst.chorda.scene_of(piano).expect("the scene");
    let open_card = cv::room_rects(&r.ctx, piano).1.expect("open");
    // A point of the lid clear of the card (and of every other overlay).
    let over = cv::overlay_rects(&r.ctx, piano);
    let on_lid = (0..240)
        .flat_map(|j| (0..400).map(move |i| (i, j)))
        .map(|(i, j)| {
            egui::pos2(
                pic.left() + (i as f32 + 0.5) / 400.0 * pic.width(),
                pic.top() + (j as f32 + 0.5) / 240.0 * pic.height(),
            )
        })
        .find(|q| {
            !open_card.expand(4.0).contains(*q)
                && !over.iter().any(|o| o.expand(4.0).contains(*q))
                && matches!(sc.pick(pic, *q), Some(cantus_ui::scene::Picked::Lid))
        })
        .expect("a point of the lid clear of the card");
    r.click(on_lid);
    assert!(
        !cv::room_open(&r.app, piano),
        "the click on the lid closes it"
    );
    assert_ne!(r.spec(piano).get("lid"), lid, "and the lid moved");
    // The chip opens and closes it.
    r.click(chip.center());
    assert!(cv::room_open(&r.app, piano));
    r.click(chip.center());
    assert!(!cv::room_open(&r.app, piano), "the chip closes it");
    // The room is the song's look: one undo after the lid's takes it back.
    r.app.undo();
    r.app.undo();
    assert_ne!(r.spec(piano).text(room_look::ROOM_KEY), Some("studio"));
}
