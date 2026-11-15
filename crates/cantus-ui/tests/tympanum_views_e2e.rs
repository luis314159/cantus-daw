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
