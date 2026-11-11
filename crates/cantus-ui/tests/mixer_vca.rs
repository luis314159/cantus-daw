//! The desk's VCAs and snapshots (round 36, ADR 0093), headless: a VCA is drawn as a strip
//! with a fader, mute and solo and none of the audio rows; the Snapshots menu lists the
//! snapshots and the cues; strips that follow a VCA still show all their rows.

use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
use cantus_core::vca::StripKind;
use cantus_core::{Project, StripId, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::views::mixer::{self, Control};
use cantus_ui::{starter_project, views, CantusApp};
use egui::{vec2, Pos2};

fn project() -> (Project, StripId, StripId) {
    let mut p = starter_project();
    let mut v = p.make_strip("Drums VCA");
    v.kind = StripKind::Vca;
    let vca = v.id;
    p.strips.push(v);
    let member = p.strips[0].id;
    p.strips[0].vcas.push(vca);
    let state = capture(&p, &[], true);
    let sid = MixSnapId(p.alloc_id());
    p.mix.snapshots.push(MixSnapshot {
        id: sid,
        name: "Verse".into(),
        state,
    });
    let cue = MixCueId(p.alloc_id());
    p.mix.cues.push(MixCue {
        id: cue,
        at: Ticks::beats(8),
        snapshot: sid,
        scope: Vec::new(),
    });
    (p, vca, member)
}

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    frame: u64,
}

impl Rig {
    fn new(p: Project) -> Rig {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        Rig {
            ctx,
            app: CantusApp::from_parts(Some(p), EngineHandle::dummy(), false),
            frame: 0,
        }
    }

    fn frames(&mut self, n: usize) -> String {
        let mut text = String::new();
        for _ in 0..n {
            self.frame += 1;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0))),
                time: Some(self.frame as f64 / 60.0),
                predicted_dt: 1.0 / 60.0,
                ..Default::default()
            };
            let app = &mut self.app;
            let mut out = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| views::mixer::show(app, ui));
            });
            out.textures_delta.clear();
            text = painted_text(&out.shapes);
        }
        text
    }
}

fn painted_text(shapes: &[egui::epaint::ClippedShape]) -> String {
    fn walk(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push_str(" | ");
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = String::new();
    for s in shapes {
        walk(&s.shape, &mut out);
    }
    out
}

#[test]
fn a_vca_strip_has_a_fader_and_none_of_the_audio_rows() {
    let (p, vca, member) = project();
    let mut r = Rig::new(p);
    // Polarity, width and the EQ are Wide's and Large's rows (des-mixer).
    mixer::set_width_mode(&mut r.app, mixer::layout::WidthMode::Wide);
    let text = r.frames(6);
    let rect = |s, c| mixer::control_rect(&r.ctx, Some(s), c);
    for c in [Control::Fader, Control::Mute, Control::Solo, Control::Head] {
        assert!(rect(vca, c).is_some(), "the VCA has its {c:?}");
    }
    for c in [Control::Pan, Control::Width, Control::Phase, Control::Eq(0)] {
        assert!(rect(vca, c).is_none(), "the VCA has no {c:?}");
        assert!(rect(member, c).is_some(), "the member keeps its {c:?}");
    }
    assert!(text.contains("VCA"), "the plate says so: {text}");
}

#[test]
fn the_snapshots_menu_lists_the_snapshots_and_the_cues() {
    let (p, _, _) = project();
    let mut r = Rig::new(p);
    r.frames(4);
    mixer::open_snapshots(&mut r.app, Pos2::new(300.0, 60.0));
    let text = r.frames(4);
    assert!(text.contains("Take snapshot"), "{text}");
    assert!(text.contains("Verse"), "{text}");
    assert!(text.contains("Remove cue: bar 3.1"), "{text}");
    // The title row's field says how many there are.
    assert!(text.contains("Snapshots 1"), "{text}");
}

#[test]
fn a_recall_through_the_app_is_one_undo_step() {
    use cantus_core::mix_cmd::MixCommand;
    use cantus_core::Command;
    let (p, _, member) = project();
    let mut r = Rig::new(p);
    let id = r.app.project().mix.snapshots[0].id;
    r.app.exec(Command::SetStripGain {
        id: member,
        gain_db: -12.0,
    });
    r.app.end_gesture();
    let before = r.app.project().clone();
    r.app.exec(Command::Mix(MixCommand::Recall {
        id,
        scope: Vec::new(),
        master: true,
        include_safe: false,
    }));
    assert_eq!(r.app.project().strip(member).unwrap().gain_db, 0.0);
    r.app.undo();
    let mut now = r.app.project().clone();
    now.next_id = before.next_id;
    assert!(
        now == before,
        "one undo step puts back what the recall changed"
    );
}
