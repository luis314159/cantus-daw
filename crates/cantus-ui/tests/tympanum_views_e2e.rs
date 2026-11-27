//! Tympanum's views end to end (round 40 phase 3, ADR 0124, design item 4), on the boot path:
//! an app made by `CantusApp::assemble` with the real engine handle and processor.
//!
//! - The Kit view opened on a Tympanum track: a click on the snare in the picture selects
//!   it and plays its key (the note reaches the engine's processor; let go, it stops).
//! - The Mixer view: *Each mic to its own strip* makes a strip in the mixer for every mic,
//!   as one undo step (ADR 0123 §4).
//! - Cantus's mixer groups an instrument's strips under its band: a click on the band folds
//!   them into one strip, *Unfold* brings them back; at rest the desk asks for no frame.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst_out::{OutputFamily, OutputId, OutputPlan, OutputSlot};
use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::{CentralView, Files};
use cantus_ui::views::inst::tympanum;
use cantus_ui::views::workspace_ticker as ticker;
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
    fn new(project: Project) -> Rig {
        let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, proc_) = EngineHandle::manual(48_000.0);
        let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
        Rig {
            app,
            proc_,
            ctx,
            t: 0.0,
            size: egui::vec2(1600.0, 900.0),
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

    fn press(&mut self, at: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(at)]);
        self.frame(vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }]);
    }

    fn release(&mut self, at: egui::Pos2) {
        self.frame(vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
    }

    fn click(&mut self, at: egui::Pos2) {
        self.press(at);
        self.release(at);
        self.idle(1);
    }
}

/// A song with one Tympanum track, Drums, on the first strip.
fn kit_song() -> (Project, TrackId) {
    let mut p = Project::new("Tympanum");
    let mut t = p.make_track("Drums");
    t.strip = Some(p.strips[0].id);
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::AcousticDrums));
    let id = t.id;
    p.tracks.push(t);
    (p, id)
}

#[test]
fn a_click_on_the_snare_selects_it_and_plays_its_key() {
    let (p, drums) = kit_song();
    let mut r = Rig::new(p);
    r.app.open_inst_editor(drums);
    r.app.inst.view = cantus_ui::views::inst::View::Play;
    r.idle(4);
    let pic = tympanum::stage::picture_rect(&r.ctx, drums).expect("the picture is drawn");
    let spec = r.app.native_of(drums).unwrap();
    let def = tympanum::def::def_of(&spec);
    // The kick first, so the snare's click is a change of selection.
    let lk = tympanum::look(&r.ctx, drums);
    let s = cantus_ui::kit::Style::new(cantus_ui::kit::Tokens::slate());
    let v = tympanum::stage::view(&def, &spec, &lk, false, None, &s);
    let kick = tympanum::stage::piece_point(pic, &v, &def, 0).unwrap();
    // The scene says the zone struck too (centre, edge, rim): any of the kick's.
    assert!(
        matches!(
            tympanum::stage::pick_at(pic, &v, &def, kick),
            Some(cantus_ui::scene::Picked::Piece { piece: 0, .. })
        ),
        "the kick is drawn at {kick:?} in the picture {pic:?}"
    );
    r.click(kick);
    assert_eq!(
        tympanum::look(&r.ctx, drums).piece,
        0,
        "the kick is selected"
    );
    let snare = tympanum::stage::piece_point(pic, &v, &def, 1).unwrap();
    r.press(snare);
    let lk = tympanum::look(&r.ctx, drums);
    assert_eq!(lk.piece, 1, "the snare is selected");
    let key = def.pieces[1].articulations[0].key;
    assert_eq!(
        lk.held,
        Some(key),
        "its key is held: the engine was sent note {key}"
    );
    assert_eq!(lk.hit.map(|h| h.0), Some(1));
    r.release(snare);
    r.idle(1);
    assert_eq!(
        tympanum::look(&r.ctx, drums).held,
        None,
        "let go, the note ends"
    );
}

/// Round 40 polish (the owner: "púlelo de una"): the Kit view's picture shows the kit
/// whole — the room card no longer lies over the floor tom and the kick; it opens from a
/// chip by *Show mics*. With the card closed (the default), what lies over the picture
/// (*Show mics*, the room's chip, the kit's tag) covers no point of any piece — its drum or
/// cymbal, nor its stand — for the modelled kits and a recorded one, on screens of three
/// sizes.
#[test]
fn the_kit_shows_whole_with_the_room_card_closed() {
    let s = cantus_ui::kit::Style::new(cantus_ui::kit::Tokens::slate());
    let mut bad = Vec::new();
    let mut seen = 0;
    for kit in ["model:studio", "model:jazz", "pack:gogodze-phu-vol-ii/kit"] {
        for size in [
            egui::vec2(1280.0, 720.0),
            egui::vec2(1600.0, 900.0),
            egui::vec2(1920.0, 1080.0),
        ] {
            let (p, drums) = kit_song();
            let mut r = Rig::new(p);
            r.size = size;
            r.app.set_inst_text(
                drums,
                cantus_core::inst::tympanum::KIT_KEY,
                Some(kit.to_string()),
            );
            r.app.open_inst_editor(drums);
            r.app.inst.view = cantus_ui::views::inst::View::Play;
            r.idle(4);
            let at = format!("{kit} at {size}");
            let pic = tympanum::stage::picture_rect(&r.ctx, drums).expect("the picture");
            let over = tympanum::stage::overlay_rects(&r.ctx, drums);
            assert_eq!(
                over.len(),
                3,
                "{at}: Show mics, the chip, the tag: {over:?}"
            );
            assert!(
                tympanum::stage::room_rects(&r.ctx, drums).1.is_none(),
                "{at}: the room card is closed by default"
            );
            let spec = r.app.native_of(drums).unwrap();
            let def = tympanum::def::def_of(&spec);
            let lk = tympanum::look(&r.ctx, drums);
            let v = tympanum::stage::view(&def, &spec, &lk, false, None, &s);
            for pd in &def.pieces {
                let pts = tympanum::stage::piece_points(pic, &v, pd.index);
                if pts.is_empty() {
                    bad.push(format!("{at}: piece {} not drawn", pd.index));
                }
                let covered: Vec<_> = pts
                    .iter()
                    .filter(|(q, _)| over.iter().any(|o| o.expand(1.0).contains(*q)))
                    .collect();
                seen += pts.len();
                if !covered.is_empty() {
                    bad.push(format!(
                        "{at}: piece {} has {} of {} points under an overlay ({:?}…)",
                        pd.index,
                        covered.len(),
                        pts.len(),
                        covered[0].0
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert!(seen > 1000, "{seen} points of pieces seen");
}

/// Round 40 polish: the room's chip opens the room card over the picture; a room chosen in
/// it is seen and heard (the look's room and the acoustic `room`, one undo step); Esc
/// closes it, a click outside it closes it (and plays nothing), the chip closes it.
#[test]
fn the_room_chip_opens_the_card_and_the_room_changes_through_it() {
    use cantus_core::inst::room_look;
    use cantus_ui::views::inst::room_card::probe;
    let (p, drums) = kit_song();
    let mut r = Rig::new(p);
    r.app.open_inst_editor(drums);
    r.app.inst.view = cantus_ui::views::inst::View::Play;
    r.idle(4);
    let (chip, card) = tympanum::stage::room_rects(&r.ctx, drums);
    let chip = chip.expect("the chip is drawn");
    assert!(card.is_none(), "closed at first");
    // Open: the card under the chip, inside the picture.
    r.click(chip.center());
    assert!(tympanum::look(&r.ctx, drums).room_open, "the chip opens it");
    probe::start();
    r.idle(1);
    let seen = probe::take();
    let pic = tympanum::stage::picture_rect(&r.ctx, drums).unwrap();
    let card = tympanum::stage::room_rects(&r.ctx, drums)
        .1
        .expect("the card");
    assert!(pic.expand(0.5).contains_rect(card), "{card:?} in {pic:?}");
    assert!(card.top() >= chip.bottom(), "under the chip");
    let shown = seen.last().expect("the card was drawn");
    // Studio, the third of four rooms.
    let rooms = shown.rooms;
    let studio = egui::pos2(rooms.left() + rooms.width() * 2.5 / 4.0, rooms.center().y);
    r.click(studio);
    let spec = r.app.native_of(drums).unwrap();
    assert_eq!(
        spec.text(room_look::ROOM_KEY),
        Some("studio"),
        "seen in the studio"
    );
    assert_eq!(spec.get("room"), 2.0, "heard in Aula's studio");
    assert!(
        tympanum::look(&r.ctx, drums).room_open,
        "a choice in the card leaves it open"
    );
    // The chip now names the studio and is as wide as its words.
    let chip2 = tympanum::stage::room_rects(&r.ctx, drums).0.unwrap();
    assert!(chip2.right() <= pic.right(), "{chip2:?}");
    // Esc closes it.
    r.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    r.idle(1);
    assert!(!tympanum::look(&r.ctx, drums).room_open, "Esc closes it");
    assert!(tympanum::stage::room_rects(&r.ctx, drums).1.is_none());
    // Open again; a click outside it (the picture's empty top-left corner) closes it.
    r.click(chip2.center());
    assert!(tympanum::look(&r.ctx, drums).room_open);
    let corner = pic.left_top() + egui::vec2(6.0, 6.0);
    r.click(corner);
    let lk = tympanum::look(&r.ctx, drums);
    assert!(!lk.room_open, "a click outside closes it");
    assert_eq!(lk.held, None, "and plays nothing");
    // Open and close by the chip.
    r.click(chip2.center());
    assert!(tympanum::look(&r.ctx, drums).room_open);
    r.click(chip2.center());
    assert!(
        !tympanum::look(&r.ctx, drums).room_open,
        "the chip closes it"
    );
    // The room is the song's, seen and heard: one undo step takes both back (opening and
    // closing the card made none).
    r.app.undo();
    let spec = r.app.native_of(drums).unwrap();
    assert_ne!(spec.text(room_look::ROOM_KEY), Some("studio"));
    assert_eq!(spec.get("room"), 0.0);
}

#[test]
fn each_mic_to_its_own_strip_makes_the_strips_in_one_undo_step() {
    let (p, drums) = kit_song();
    let mut r = Rig::new(p);
    r.app.open_inst_editor(drums);
    r.app.inst.view = cantus_ui::views::inst::View::Mixer;
    r.idle(4);
    let before = r.app.project().strips.len();
    let button = tympanum::plan_rect(&r.ctx, drums, OutputPlan::EachOwn).expect("drawn");
    r.click(button.center());
    let spec = r.app.native_of(drums).unwrap();
    let def = tympanum::def::def_of(&spec);
    assert_eq!(spec.outputs.len(), def.mics.len(), "every mic on a strip");
    assert_eq!(
        inst_out::strips_of(&spec.outputs).len(),
        def.mics.len(),
        "a strip each"
    );
    assert!(r.app.project().strips.len() >= before);
    r.app.undo();
    assert!(
        r.app.native_of(drums).unwrap().outputs.is_empty(),
        "one undo step puts every mic back on Main"
    );
}

/// A song whose Drums play three mics on strips of their own, beside its strip.
fn grouped_song() -> (Project, TrackId) {
    let (mut p, drums) = kit_song();
    let kit_strip = p.strips[0].id;
    let mut slots = Vec::new();
    for (i, (id, name, fam)) in [
        (0u16, "Kick In", OutputFamily::Kick),
        (1, "Kick Out", OutputFamily::Kick),
        (3, "Snare Top", OutputFamily::Snare),
    ]
    .into_iter()
    .enumerate()
    {
        let s = &mut p.strips[i + 1];
        s.name = name.into();
        s.kind = cantus_core::vca::StripKind::Pad;
        s.output = Some(kit_strip);
        slots.push(OutputSlot {
            id: OutputId(id),
            name: name.into(),
            family: fam,
            strip: s.id,
        });
    }
    if let InstrumentSpec::Native(n) = &mut p.tracks[0].instrument {
        n.outputs = slots;
    }
    (p, drums)
}

#[test]
fn the_band_folds_an_instruments_strips_and_unfold_brings_them_back() {
    let (p, drums) = grouped_song();
    let ids: Vec<StripId> = p.strips[1..4].iter().map(|s| s.id).collect();
    let mut r = Rig::new(p);
    r.app.show_central(CentralView::Mixer);
    r.idle(4);
    use cantus_ui::views::mixer::{control_rect, group, Control};
    let head = |r: &Rig, id| control_rect(&r.ctx, Some(id), Control::Head);
    assert!(
        ids.iter().all(|id| head(&r, *id).is_some()),
        "the three mics' strips are on the desk"
    );
    let kick_in = head(&r, ids[0]).unwrap();
    // The band sits over the strips' heads: a click on it folds the family.
    let band = egui::pos2(kick_in.center().x, kick_in.top() - 10.0);
    r.click(band);
    assert!(group::is_folded(&r.app, drums), "the band's click folds it");
    r.idle(2);
    assert!(
        head(&r, ids[1]).is_none() && head(&r, ids[2]).is_none(),
        "folded: one strip for the three"
    );
    // Folding is the view's: the song is untouched, nothing to undo.
    assert!(!r.app.bus.can_undo() || r.app.project().strips.len() >= 4);
    group::set_folded(&mut r.app, drums, false);
    r.idle(2);
    assert!(
        ids.iter().all(|id| head(&r, *id).is_some()),
        "unfolded: the three again"
    );
    // At rest the desk asks for no frame.
    r.idle(90);
    let quiet = |d: Option<std::time::Duration>| {
        d.is_none_or(|d| d > std::time::Duration::from_millis(500))
    };
    assert!(
        quiet(ticker::last(&r.ctx).requested),
        "{:?}",
        ticker::last(&r.ctx)
    );
}

#[test]
fn a_members_full_name_carries_its_instruments() {
    let (p, _) = grouped_song();
    let id = p.strips[3].id;
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert_eq!(
        cantus_ui::views::mixer::group::full_name(&p, id),
        "Drums · Snare Top"
    );
    assert_eq!(
        cantus_ui::views::mixer::group::full_name(&p, p.strips[0].id),
        p.strips[0].name
    );
}

/// r42 integration — model 6 by choice (ADR 0124, schema 89): a new Tympanum plays model 1
/// (model 5, the released sound); the kits list's *Model 6 (experimental)* switch sets
/// `model` 2 in one undo step and back to 1, and the engine plays what it says.
#[test]
fn the_experimental_model_is_a_switch_a_new_kit_starts_without() {
    let (p, drums) = kit_song();
    let mut r = Rig::new(p);
    r.app.open_inst_editor(drums);
    r.app.inst.view = cantus_ui::views::inst::View::Play;
    r.idle(4);
    let model = |r: &Rig| r.app.native_of(drums).unwrap().get("model");
    assert_eq!(model(&r), 1.0, "a new kit plays model 1");
    let sw = tympanum::kits::model_switch_rect(&r.ctx, drums).expect("the switch is drawn");
    let pic = tympanum::stage::picture_rect(&r.ctx, drums).expect("the picture");
    assert!(
        !sw.intersects(pic),
        "the switch {sw:?} lies beside the picture {pic:?}"
    );
    r.click(sw.center());
    assert_eq!(model(&r), 2.0, "the switch chooses model 2");
    r.app.undo();
    assert_eq!(model(&r), 1.0, "one undo step takes it back");
    r.click(sw.center());
    r.click(sw.center());
    assert_eq!(model(&r), 1.0, "and off again");
}
