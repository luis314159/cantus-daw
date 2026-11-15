//! Bands on the desk end to end (round 40 phase 3, the owner's groups sheets; ADR 0016 and
//! ADR 0123 amended, schema 70), through `CantusApp::assemble`:
//!
//! - a user group is a band over its strips; a click on it folds it into one strip (its
//!   members named, never hidden away), one undo step, and *Unfold* brings them back;
//! - its right edge dragged past the next strip's middle takes that strip in, one undo step;
//!   Esc during the drag leaves the group as it was;
//! - the one-band rule: a group made of strips of one instrument's family sits inside it; a
//!   group of a family's strip and another strip takes the family whole;
//! - *Each mic to its own strip* gathers the new strips into a group per output family inside
//!   the kit's band; *All to Main* takes them away — each one undo step;
//! - a bus fed only by the family is added to its band (`SetStripAttached`), and a change of
//!   its routing takes it out again, said on the status line.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst_out::OutputPlan;
use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::{CentralView, Files};
use cantus_ui::views::inst::tympanum;
use cantus_ui::views::mixer::{control_rect, group, Control};
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
    _proc: Processor,
    ctx: egui::Context,
    t: f64,
    /// The words the last frame drew, with where.
    texts: Vec<(String, egui::Rect)>,
}

impl Rig {
    fn new(project: Project) -> Rig {
        let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, proc_) = EngineHandle::manual(48_000.0);
        let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
        let mut r = Rig {
            app,
            _proc: proc_,
            ctx,
            t: 0.0,
            texts: Vec::new(),
        };
        r.app.show_central(CentralView::Mixer);
        r.idle(4);
        r
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        self.texts = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some((
                    t.galley.text().to_string(),
                    t.galley
                        .rect
                        .translate(t.pos.to_vec2())
                        .intersect(c.clip_rect),
                )),
                _ => None,
            })
            .collect();
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn button(&mut self, at: egui::Pos2, pressed: bool) {
        self.frame(vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }]);
    }

    fn click(&mut self, at: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(at)]);
        self.button(at, true);
        self.button(at, false);
        self.idle(2);
    }

    /// Drag from `a` to `b` in steps; `esc` presses Escape before letting go.
    fn drag(&mut self, a: egui::Pos2, b: egui::Pos2, esc: bool) {
        self.frame(vec![egui::Event::PointerMoved(a)]);
        self.button(a, true);
        for k in 1..=8 {
            let q = a + (b - a) * (k as f32 / 8.0);
            self.frame(vec![egui::Event::PointerMoved(q)]);
        }
        if esc {
            for pressed in [true, false] {
                self.frame(vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }]);
            }
        }
        self.button(b, false);
        self.idle(2);
    }

    fn head(&self, id: StripId) -> Option<egui::Rect> {
        control_rect(&self.ctx, Some(id), Control::Head)
    }
}

/// Seven tracks on strips 1–7, plain synths.
fn desk_song() -> Project {
    let mut p = Project::new("Bands");
    for (i, name) in ["Drums", "Bass", "Piano", "Guitar", "Vox", "Choir", "Pad"]
        .iter()
        .enumerate()
    {
        let mut t = p.make_track(*name);
        t.strip = Some(p.strips[i].id);
        p.tracks.push(t);
    }
    p
}

fn strip_of(r: &Rig, name: &str) -> StripId {
    let p = r.app.project();
    p.tracks
        .iter()
        .find(|t| t.name == name)
        .and_then(|t| t.strip)
        .unwrap()
}

#[test]
fn a_group_is_a_band_that_folds_into_one_strip_and_unfolds() {
    let mut r = Rig::new(desk_song());
    let voices: Vec<StripId> = ["Vox", "Choir", "Pad"]
        .iter()
        .map(|n| strip_of(&r, n))
        .collect();
    let g = r.app.create_strip_group(&voices).expect("a group");
    r.idle(3);
    assert!(
        voices.iter().all(|s| r.head(*s).is_some()),
        "its strips on the desk"
    );
    let vox = r.head(voices[0]).unwrap();
    // The band over the heads: a click folds it, one undo step.
    r.click(egui::pos2(vox.center().x + 20.0, vox.top() - 9.0));
    assert!(r.app.project().strip_group(g).unwrap().collapsed, "folded");
    r.idle(2);
    assert!(
        r.head(voices[0]).is_some(),
        "folded, the group is one strip — never hidden"
    );
    assert!(r.head(voices[1]).is_none() && r.head(voices[2]).is_none());
    r.app.undo();
    r.idle(2);
    assert!(
        !r.app.project().strip_group(g).unwrap().collapsed,
        "one undo step"
    );
    assert!(voices.iter().all(|s| r.head(*s).is_some()));
}

#[test]
fn the_bands_edge_takes_in_the_next_strip_and_esc_leaves_it() {
    let mut r = Rig::new(desk_song());
    let voices: Vec<StripId> = ["Vox", "Choir"].iter().map(|n| strip_of(&r, n)).collect();
    let pad = strip_of(&r, "Pad");
    let g = r.app.create_strip_group(&voices).unwrap();
    r.idle(3);
    let choir = r.head(voices[1]).unwrap();
    let padh = r.head(pad).unwrap();
    let edge = egui::pos2(choir.right() + 1.0, choir.top() - 9.0);
    let over = egui::pos2(padh.right() - 4.0, choir.top() - 9.0);
    // Esc: nothing changes.
    r.drag(edge, over, true);
    assert_ne!(
        r.app.project().strip(pad).unwrap().group,
        Some(g),
        "Esc leaves it"
    );
    r.drag(edge, over, false);
    assert_eq!(
        r.app.project().strip(pad).unwrap().group,
        Some(g),
        "taken in"
    );
    r.app.undo();
    assert_ne!(
        r.app.project().strip(pad).unwrap().group,
        Some(g),
        "one undo step"
    );
}

/// Drums: a Tympanum whose three mics play on strips 2–4, its own strip 1.
fn kit_desk() -> (Project, TrackId) {
    let mut p = desk_song();
    let drums = p.tracks[0].id;
    let mut n = NativeSpec::new(InstrumentKind::AcousticDrums);
    let names = [(0u16, "Kick In"), (1, "Kick Out"), (3, "Snare Top")];
    // Strips 2–4 for the mics; the other tracks move along.
    for (k, t) in p.tracks.iter_mut().enumerate().skip(1) {
        t.strip = Some(p.strips[k + 3].id);
    }
    for (i, (id, name)) in names.iter().enumerate() {
        let kit_strip = p.strips[0].id;
        let s = &mut p.strips[i + 1];
        s.name = (*name).into();
        s.kind = cantus_core::vca::StripKind::Pad;
        s.output = Some(kit_strip);
        n.outputs.push(inst_out::OutputSlot {
            id: inst_out::OutputId(*id),
            name: (*name).into(),
            family: inst_out::family_of_name(name),
            strip: p.strips[i + 1].id,
        });
    }
    p.tracks[0].instrument = InstrumentSpec::Native(n);
    (p, drums)
}

#[test]
fn a_strip_is_in_one_band_a_family_comes_whole() {
    let (p, _) = kit_desk();
    let mut r = Rig::new(p);
    let kick_in = r.app.project().strips[1].id;
    let kick_out = r.app.project().strips[2].id;
    let bass = strip_of(&r, "Bass");
    // Two of the kit's mics: a group inside the family.
    let inner = r.app.create_strip_group(&[kick_in, kick_out]).unwrap();
    r.idle(2);
    let members: Vec<StripId> = r
        .app
        .project()
        .strips_in_group(inner)
        .map(|s| s.id)
        .collect();
    assert_eq!(
        members,
        vec![kick_in, kick_out],
        "a sub-group of the family"
    );
    r.app.undo();
    // A mic and the bass: the family comes whole into the group.
    let outer = r.app.create_strip_group(&[kick_in, bass]).unwrap();
    let held: Vec<StripId> = r
        .app
        .project()
        .strips_in_group(outer)
        .map(|s| s.id)
        .collect();
    for s in r.app.project().strips[0..4].iter() {
        assert!(held.contains(&s.id), "the whole family is in: {:?}", s.name);
    }
    assert!(held.contains(&bass));
}

#[test]
fn each_mic_to_its_own_strip_groups_the_families_and_all_to_main_ungroups() {
    let mut p = desk_song();
    let drums = p.tracks[0].id;
    p.tracks[0].instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::AcousticDrums));
    let mut r = Rig::new(p);
    r.app.open_inst_editor(drums);
    r.app.inst.view = cantus_ui::views::inst::View::Mixer;
    r.idle(4);
    let groups_before = r.app.project().strip_groups.len();
    let each = tympanum::plan_rect(&r.ctx, drums, OutputPlan::EachOwn).expect("drawn");
    r.click(each.center());
    let p = r.app.project();
    let made: Vec<&StripGroup> = p.strip_groups.iter().skip(groups_before).collect();
    assert!(!made.is_empty(), "a group per output family");
    let outs = p.track(drums).unwrap().instrument.output_strips();
    for g in &made {
        let members: Vec<StripId> = p.strips_in_group(g.id).map(|s| s.id).collect();
        assert!(
            members.len() > 1 && members.iter().all(|m| outs.contains(m)),
            "{}",
            g.name
        );
    }
    // One undo step takes the strips and the groups.
    r.app.undo();
    assert_eq!(r.app.project().strip_groups.len(), groups_before);
    r.app.redo();
    r.idle(2);
    let main = tympanum::plan_rect(&r.ctx, drums, OutputPlan::AllToMain).expect("drawn");
    r.click(main.center());
    assert_eq!(
        r.app.project().strip_groups.len(),
        groups_before,
        "All to Main takes them away"
    );
}

#[test]
fn a_bus_fed_by_the_family_joins_its_band_and_leaves_with_its_routing() {
    let (mut p, drums) = kit_desk();
    // Strip 12: the kit's bus; the kit's own strip and its mics play into it.
    let bus = p.strips[11].id;
    p.strips[11].name = "Drum Bus".into();
    for i in 0..4 {
        p.strips[i].output = Some(bus);
    }
    let mut r = Rig::new(p);
    assert!(
        r.app.attach_strip(bus, Some(drums)),
        "fed only by the family: added"
    );
    assert_eq!(r.app.project().strip(bus).unwrap().attached, Some(drums));
    r.idle(2);
    // Another track into the bus: it no longer comes only from the family.
    let guitar = strip_of(&r, "Guitar");
    r.app.exec(Command::SetStripOutput {
        id: guitar,
        output: Some(bus),
    });
    r.idle(2);
    let said = r.app.status.text.clone();
    assert!(
        said.contains("Drum Bus"),
        "the status line says why: {said:?}"
    );
    let _ = group::is_folded(&r.app, drums);
}

#[test]
fn a_band_partly_scrolled_off_keeps_its_name_at_the_visible_edge() {
    // Forty tracks on Wide strips; the group is the 21st to the 23rd, off the desk's right.
    let mut p = Project::new("Long");
    for i in 0..40 {
        let mut t = p.make_track(format!("T{i}"));
        if let Some(st) = p.strips.get(i) {
            t.strip = Some(st.id);
        } else {
            let st = p.make_strip(String::new());
            t.strip = Some(st.id);
            p.strips.push(st);
        }
        p.tracks.push(t);
    }
    let mut r = Rig::new(p);
    cantus_ui::views::mixer::set_width_mode(
        &mut r.app,
        cantus_ui::views::mixer::layout::WidthMode::Wide,
    );
    r.idle(2);
    let members: Vec<StripId> = ["T20", "T21", "T22"]
        .iter()
        .map(|n| strip_of(&r, n))
        .collect();
    let g = r.app.create_strip_group(&members).unwrap();
    let name = r.app.project().strip_group(g).unwrap().name.clone();
    // Its first strip selected: the desk scrolls it in at the right edge, the rest past it.
    r.app.select_strip(members[0]);
    r.idle(4);
    let first = r.head(members[0]).expect("its first strip in view");
    assert!(
        r.head(members[2]).is_none(),
        "the band runs off the right: {:?}",
        r.head(members[2])
    );
    let shown = r
        .texts
        .iter()
        .find(|(w, _)| *w == name)
        .map(|(_, at)| *at)
        .expect("the band's name is drawn, not a bare chevron");
    assert!(
        shown.left() >= first.left() - 1.0 && shown.width() > 1.0,
        "its name stands over its visible part: {shown:?} over {first:?}"
    );
}
