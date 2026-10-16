//! No two widgets share an egui id (round 28, fix4): egui reports a clash in debug builds
//! by painting "First use of … ID" / "Second use of … ID" over the screen, and a clash
//! means one widget answers for another (a drag, a focus, a popup). The inspector with
//! several modulators on a track, a strip with several sends and an instrument with
//! several modulated knobs are drawn here, two frames each, and none may clash.

use cantus_core::modulator::*;
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::{starter_project, CantusApp};

fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// Every id clash egui reported while `draw` ran (two frames, the second one's).
fn clashes(
    ctx: &egui::Context,
    size: egui::Vec2,
    mut draw: impl FnMut(&mut egui::Ui),
) -> Vec<String> {
    assert!(
        ctx.options(|o| o.warn_on_id_clash),
        "egui checks ids in debug builds"
    );
    let mut found = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| draw(ui));
        });
        out.textures_delta.clear();
        found = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .filter(|t| t.contains(" use of ") && t.contains("ID"))
            .collect();
    }
    found
}

/// A modulator of `kind` named `name` driving `target`.
fn modulator(p: &mut Project, kind: ModKind, name: &str, target: AutoTarget) -> Modulator {
    let mut m = Modulator::new(ModulatorId(p.alloc_id()), kind);
    m.name = name.into();
    m.routes.push(ModRoute { target, depth: 0.3 });
    m
}

#[test]
fn the_inspector_with_several_modulators_has_no_id_clash() {
    let mut p = starter_project();
    let t = p.tracks[0].id;
    let s = p.tracks[0].strip.unwrap();
    for (k, kind) in [
        ModKind::Lfo,
        ModKind::Envelope,
        ModKind::Follower,
        ModKind::Step,
    ]
    .into_iter()
    .enumerate()
    {
        let target = if k % 2 == 0 {
            AutoTarget::StripPan(s)
        } else {
            AutoTarget::StripGain(s)
        };
        let m = modulator(&mut p, kind, &format!("Mod {k}"), target);
        p.modulators_mut(ModHost::Track(t)).unwrap().push(m);
    }
    for kind in [ModKind::Random, ModKind::Lfo] {
        let m = modulator(&mut p, kind, "On the strip", AutoTarget::StripPan(s));
        p.modulators_mut(ModHost::Strip(s)).unwrap().push(m);
    }
    let mut a = app(p);
    let ctx = ctx();
    a.select_track(t);
    let size = egui::vec2(360.0, 3000.0);
    let found = clashes(&ctx, size, |ui| {
        cantus_ui::views::inspector::show(&mut a, ui)
    });
    assert!(found.is_empty(), "the track's inspector: {found:?}");
    a.select_master();
    let found = clashes(&ctx, size, |ui| {
        cantus_ui::views::inspector::show(&mut a, ui)
    });
    assert!(found.is_empty(), "the master's inspector: {found:?}");
}

/// A channel with several sends — each send's level modulated — in the inspector (its
/// send rows, its pinned strip), each return's received rows, and the mixer desk.
#[test]
fn strips_with_several_sends_have_no_id_clash() {
    let mut a = app(starter_project());
    let track = a.project().tracks[0].id;
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_strip();
    let s1 = a.project().strips.last().unwrap().id;
    let mut returns = Vec::new();
    for _ in 0..3 {
        a.add_strip();
        let ret = a.project().strips.last().unwrap().id;
        assert!(a.add_sends(&[s0, s1], ret));
        returns.push(ret);
    }
    let mut p = a.project().clone();
    for (k, ret) in returns.iter().enumerate() {
        let m = modulator(
            &mut p,
            ModKind::Lfo,
            &format!("Send {k}"),
            AutoTarget::Send {
                strip: s0,
                to: *ret,
            },
        );
        p.modulators_mut(ModHost::Track(track)).unwrap().push(m);
    }
    let mut a = app(p);
    let ctx = ctx();
    let size = egui::vec2(360.0, 3000.0);
    a.select_track(track);
    let found = clashes(&ctx, size, |ui| {
        cantus_ui::views::inspector::show(&mut a, ui)
    });
    assert!(found.is_empty(), "the channel: {found:?}");
    for ret in &returns {
        a.select_strip(*ret);
        let found = clashes(&ctx, size, |ui| {
            cantus_ui::views::inspector::show(&mut a, ui)
        });
        assert!(found.is_empty(), "a return: {found:?}");
    }
    let found = clashes(&ctx, egui::vec2(1600.0, 800.0), |ui| {
        cantus_ui::views::mixer::show(&mut a, ui)
    });
    assert!(found.is_empty(), "the desk: {found:?}");
}

/// Every native instrument's editor — each view — with several of its knobs modulated,
/// in the channel's inspector too.
#[test]
fn an_instrument_with_several_modulated_knobs_has_no_id_clash() {
    use cantus_ui::views::inst::View;
    let ctx = ctx();
    let mut bad = Vec::new();
    for kind in InstrumentKind::ALL {
        let mut a = app(starter_project());
        let track = a.project().tracks[0].id;
        a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
        cantus_ui::views::inst::demo_setup(&mut a, track, kind);
        let mut p = a.project().clone();
        let keys: Vec<String> = kind
            .params()
            .iter()
            .filter(|d| kind.modulatable(d.key))
            .take(4)
            .map(|d| d.key.to_string())
            .collect();
        for (k, key) in keys.iter().enumerate() {
            let m = modulator(
                &mut p,
                if k % 2 == 0 {
                    ModKind::Lfo
                } else {
                    ModKind::Random
                },
                &format!("Knob {k}"),
                AutoTarget::Instrument {
                    track,
                    key: key.clone(),
                },
            );
            p.modulators_mut(ModHost::Track(track)).unwrap().push(m);
        }
        let mut a = app(p);
        a.open_inst_editor(track);
        for view in [View::Play, View::Edit, View::Matrix] {
            a.inst.view = view;
            let found = clashes(&ctx, egui::vec2(1400.0, 900.0), |ui| {
                cantus_ui::views::inst::window_in(&mut a, ui)
            });
            bad.extend(found.into_iter().map(|f| format!("{kind:?} {view:?}: {f}")));
        }
        a.select_track(track);
        let found = clashes(&ctx, egui::vec2(360.0, 3000.0), |ui| {
            cantus_ui::views::inspector::show(&mut a, ui)
        });
        bad.extend(
            found
                .into_iter()
                .map(|f| format!("{kind:?} inspector: {f}")),
        );
    }
    bad.dedup();
    assert!(bad.is_empty(), "id clashes: {bad:#?}");
}
