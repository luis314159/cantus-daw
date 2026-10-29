//! The frame budget with three units' editors open (round 32, ADR 0074, ADR 0024 §13):
//! Culmen, Spatium and Unda side by side at 100 % (docked) and overlapping at 200 %
//! (floating), at 1 and at 2 physical pixels per point, each in a face of its own — their
//! own sheets, and three distinct rich sheets (wood, vinyl amp covering and carbon; a phosphor, a VFD
//! and a scope display; skirted, chicken-head and dial knobs; every ornament between them),
//! so the cost of three different faces at once is measured, not one face shared.
//!
//! - `three_editors_wear_their_faces_in_one_frame` (every `cargo test`): the three draw
//!   in one frame, each body in its own face, and how much they drew is printed.
//! - `three_editors_frame_time` (`cargo test --release -p cantus-ui --test skins_budget
//!   -- --ignored --nocapture`): build + tessellate over 200 frames; p95 asserted within
//!   the window budget of 4 ms, and the layer atlas never reset in steady state.

use cantus_core::inst::InstrumentKind;
use cantus_core::EffectKind;
use cantus_engine::EngineHandle;
use cantus_ui::kit::color::Hsl;
use cantus_ui::kit::shell::{self, Shell};
use cantus_ui::kit::skin::{
    self, Case, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin, SwitchShape,
    Tint, Unit, Weight,
};
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Pos2, Rect};
use std::time::Instant;

/// The richest face: every costly thing a sheet can ask for at once.
static RICH: Skin = Skin {
    key: "test.rich",
    surface: Tint {
        hue: 28.0,
        sat: 0.35,
        amount: 0.6,
    },
    accent: Some(Pair::both(Hsl {
        h: 32.0,
        s: 0.85,
        l: 0.56,
    })),
    accent2: Some(Pair::both(Hsl {
        h: 190.0,
        s: 0.6,
        l: 0.55,
    })),
    material: Material::Wood,
    display: Display::Phosphor,
    knob: KnobShape::Skirted,
    switch: SwitchShape::Bat,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.12,
        weight: Weight::Medium,
        condensed: true,
        mono_values: true,
    },
    plate: Plate::Badge,
    ornaments: Ornaments {
        screws: true,
        vents: true,
        rules: true,
        rack_ears: true,
    },
};

/// The second rich face (Spatium's here): vinyl amp covering, a VFD, chicken-head knobs, rockers, an
/// engraved plate, screws and vents.
static RICH_B: Skin = Skin {
    key: "test.rich.b",
    surface: Tint {
        hue: 210.0,
        sat: 0.3,
        amount: 0.7,
    },
    accent: Some(Pair::both(Hsl {
        h: 170.0,
        s: 0.7,
        l: 0.5,
    })),
    accent2: None,
    material: Material::Tolex,
    display: Display::Vfd,
    knob: KnobShape::ChickenHead,
    switch: SwitchShape::Rocker,
    lettering: Lettering {
        case: Case::Upper,
        tracking: 0.08,
        weight: Weight::SemiBold,
        condensed: false,
        mono_values: true,
    },
    plate: Plate::Engraved,
    ornaments: Ornaments {
        screws: true,
        vents: true,
        rules: false,
        rack_ears: false,
    },
};

/// The third rich face (Unda's here): carbon, a scope, dial knobs, pills, a stripe, rack
/// ears and rules.
static RICH_C: Skin = Skin {
    key: "test.rich.c",
    surface: Tint {
        hue: 280.0,
        sat: 0.25,
        amount: 0.5,
    },
    accent: Some(Pair::both(Hsl {
        h: 330.0,
        s: 0.75,
        l: 0.58,
    })),
    accent2: Some(Pair::both(Hsl {
        h: 60.0,
        s: 0.7,
        l: 0.55,
    })),
    material: Material::Carbon,
    display: Display::Scope,
    knob: KnobShape::Dial,
    switch: SwitchShape::Pill,
    lettering: Lettering {
        case: Case::AsIs,
        tracking: 0.04,
        weight: Weight::Medium,
        condensed: true,
        mono_values: false,
    },
    plate: Plate::Stripe,
    ornaments: Ornaments {
        screws: false,
        vents: false,
        rules: true,
        rack_ears: true,
    },
};

/// Each of the three units in a rich face of its own.
fn rich_by_unit(u: Unit) -> &'static Skin {
    match u {
        Unit::Effect(EffectKind::Space) => &RICH_B,
        Unit::Instrument(InstrumentKind::Wavetable) => &RICH_C,
        _ => &RICH,
    }
}

/// Which faces a run wears.
#[derive(Copy, Clone, PartialEq)]
enum Faces {
    /// Each unit its own family's sheet.
    Theirs,
    /// Three distinct rich sheets.
    Rich,
}

impl Faces {
    fn wear(self) {
        skin::set_override(None);
        skin::set_override_by(match self {
            Faces::Theirs => None,
            Faces::Rich => Some(rich_by_unit),
        });
    }

    fn name(self) -> &'static str {
        match self {
            Faces::Theirs => "their sheets",
            Faces::Rich => "three rich sheets",
        }
    }
}

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    strip: cantus_core::StripId,
    culmen: cantus_core::EffectId,
    spatium: cantus_core::EffectId,
}

fn rig(k: f32, ppp: f32) -> Rig {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(ppp);
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("culmen");
    let (strip, culmen, _) = app.fx.open.expect("Culmen open");
    app.demo_editor("space");
    let (_, spatium, _) = app.fx.open.expect("Spatium open");
    views::fx::demo_feed(&mut app, EffectKind::Space);
    app.demo_editor("unda");
    views::inst::demo_feed(&mut app, cantus_core::inst::InstrumentKind::Wavetable);
    views::fx::frame::shell_of(EffectKind::Maximizer).set_scale(&ctx, k);
    views::fx::frame::shell_of(EffectKind::Space).set_scale(&ctx, k);
    if let Some(t) = app.inst.open {
        Shell::new(("inst", t.0)).set_scale(&ctx, k);
    }
    Rig {
        app,
        ctx,
        strip,
        culmen,
        spatium,
    }
}

/// The three windows' rects: side by side at 100 %, overlapping at 200 %.
fn places(k: f32) -> [Rect; 3] {
    let c = views::fx::frame::designed_size(EffectKind::Maximizer) * k;
    let s = views::fx::frame::designed_size(EffectKind::Space) * k;
    let u = shell::INSTRUMENT * k;
    if k <= 1.0 {
        [
            Rect::from_min_size(Pos2::ZERO, c),
            Rect::from_min_size(Pos2::new(c.x, 0.0), s),
            Rect::from_min_size(Pos2::new(c.x + s.x, 0.0), u),
        ]
    } else {
        [
            Rect::from_min_size(Pos2::ZERO, c),
            Rect::from_min_size(Pos2::new(240.0, 160.0), s),
            Rect::from_min_size(Pos2::new(480.0, 320.0), u),
        ]
    }
}

thread_local! {
    static PARTS: std::cell::Cell<[f64; 3]> = const { std::cell::Cell::new([0.0; 3]) };
}

/// One frame: (build, tessellate) in ms, vertices, the items' faces.
fn frame(r: &mut Rig, k: f32, i: usize) -> (f64, f64, usize) {
    let p = places(k);
    let screen = p.iter().fold(Rect::NOTHING, |a, b| a.union(*b));
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen.max.to_vec2())),
        time: Some(i as f64 / 60.0),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    let (strip, culmen, spatium) = (r.strip, r.culmen, r.spatium);
    let only = std::env::var("SKINS_BUDGET_ONLY").ok();
    let on = |n: &str| only.as_deref().is_none_or(|o| o.split(',').any(|x| x == n));
    let t0 = Instant::now();
    let mut out = r.ctx.run_ui(input, |ui| {
        let q0 = Instant::now();
        if on("culmen") {
            let mut a = ui.new_child(egui::UiBuilder::new().max_rect(p[0]));
            let _ = views::fx::maximizer::framed(&mut r.app, &mut a, strip, culmen);
        }
        let q1 = Instant::now();
        if on("spatium") {
            let mut b = ui.new_child(egui::UiBuilder::new().max_rect(p[1]));
            let _ = views::fx::space::framed(&mut r.app, &mut b, strip, spatium);
        }
        let q2 = Instant::now();
        if on("unda") {
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(p[2]));
            views::inst::window_in(&mut r.app, &mut c);
        }
        let q3 = Instant::now();
        PARTS.with(|p| {
            let mut v = p.get();
            v[0] += (q1 - q0).as_secs_f64();
            v[1] += (q2 - q1).as_secs_f64();
            v[2] += (q3 - q2).as_secs_f64();
            p.set(v);
        });
    });
    let t1 = Instant::now();
    out.textures_delta.clear();
    let prims = r.ctx.tessellate(out.shapes, out.pixels_per_point);
    let t2 = Instant::now();
    let verts = prims
        .iter()
        .map(|p| match &p.primitive {
            egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
            _ => 0,
        })
        .sum();
    (
        (t1 - t0).as_secs_f64() * 1e3,
        (t2 - t1).as_secs_f64() * 1e3,
        verts,
    )
}

#[test]
fn three_editors_wear_their_faces_in_one_frame() {
    for (faces, k, ppp) in [
        (Faces::Theirs, 1.0, 1.0),
        (Faces::Rich, 1.0, 1.0),
        (Faces::Rich, 2.0, 2.0),
    ] {
        faces.wear();
        let mut r = rig(k, ppp);
        cantus_ui::kit::inspect::set_enabled(&r.ctx, true);
        let mut verts = 0;
        for i in 0..4 {
            verts = frame(&mut r, k, i).2;
        }
        let items = cantus_ui::kit::inspect::items(&r.ctx);
        let mut got: Vec<&str> = items
            .iter()
            .filter(|i| i.part == "device_ground")
            .map(|i| i.skin)
            .collect();
        skin::set_override_by(None);
        let mut want: Vec<&str> = match faces {
            Faces::Rich => vec!["test.rich", "test.rich.b", "test.rich.c"],
            Faces::Theirs => vec![
                skin::of_effect(EffectKind::Maximizer).key,
                skin::of_effect(EffectKind::Space).key,
                skin::of_instrument(InstrumentKind::Wavetable).key,
            ],
        };
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want, "the three bodies, each in its own face");
        eprintln!(
            "three editors ({}, {:.0} %, {ppp} px/pt): {verts} vertices",
            faces.name(),
            k * 100.0
        );
    }
}

/// One run of 200 frames: totals, builds and tessellations (ms, sorted), the vertices.
struct Run {
    ms: Vec<f64>,
    builds: Vec<f64>,
    tess: Vec<f64>,
    verts: usize,
}

/// `SKINS_BUDGET_CASE=0,2` runs only those cases; `SKINS_BUDGET_ONLY=culmen,unda` draws only
/// those editors (to see where a frame's time goes).
#[test]
#[ignore = "timing: run in release with --ignored"]
fn three_editors_frame_time() {
    let mut over = Vec::new();
    for (faces, k, ppp) in [
        (Faces::Theirs, 1.0, 1.0),
        (Faces::Rich, 1.0, 1.0),
        (Faces::Rich, 1.0, 2.0),
        (Faces::Theirs, 2.0, 1.0),
        (Faces::Rich, 2.0, 1.0),
        (Faces::Rich, 2.0, 2.0),
    ]
    .into_iter()
    .enumerate()
    .filter(|(i, _)| {
        std::env::var("SKINS_BUDGET_CASE")
            .ok()
            .is_none_or(|c| c.split(',').any(|x| x == i.to_string()))
    })
    .map(|(_, c)| c)
    {
        faces.wear();
        let mut r = rig(k, ppp);
        for i in 0..60 {
            let _ = frame(&mut r, k, i);
        }
        let before = cantus_ui::kit::cache::with_raw(&r.ctx, |c| c.stats);
        let counts0 = skin::counts();
        PARTS.with(|p| p.set([0.0; 3]));
        // Best of three runs of 200 frames (the CPU tests' rule on a shared machine:
        // round 23): the run with the lowest p95 is the one another process disturbed
        // least.
        let mut best: Option<Run> = None;
        for run in 0..3 {
            let (mut builds, mut tess, mut verts) = (Vec::new(), Vec::new(), 0);
            let mut ms: Vec<f64> = (0..200)
                .map(|i| {
                    let (b, t, v) = frame(&mut r, k, 60 + run * 200 + i);
                    builds.push(b);
                    tess.push(t);
                    verts = v;
                    b + t
                })
                .collect();
            for v in [&mut ms, &mut builds, &mut tess] {
                v.sort_by(f64::total_cmp);
            }
            let p95 = |v: &Vec<f64>| v[v.len() * 95 / 100];
            if best.as_ref().is_none_or(|b| p95(&ms) < p95(&b.ms)) {
                best = Some(Run {
                    ms,
                    builds,
                    tess,
                    verts,
                });
            }
        }
        let Run {
            ms,
            builds,
            tess,
            verts,
        } = best.expect("three runs");
        let after = cantus_ui::kit::cache::with_raw(&r.ctx, |c| c.stats);
        let counts1 = skin::counts();
        let parts = PARTS.with(|p| p.get());
        // Per editor, the mean over all 600 frames, in ms.
        let per = |x: f64| x * 1e3 / 600.0;
        assert_eq!(
            (
                counts1.0 - counts0.0,
                counts1.1 - counts0.1,
                counts1.2 - counts0.2
            ),
            (0, 0, 0),
            "a skin's tokens, the worn style or a display's colours made again in steady state"
        );
        skin::set_override_by(None);
        let p50 = ms[ms.len() / 2];
        let p95 = ms[ms.len() * 95 / 100];
        let name = format!(
            "three editors, {} at {:.0} % ({}), {ppp} px/pt",
            faces.name(),
            k * 100.0,
            if k <= 1.0 { "docked" } else { "floating" }
        );
        eprintln!(
            "{name}: build + tessellate p50 {p50:.2} ms, p95 {p95:.2} ms (build p50 {:.2}, tessellate p50 {:.2}; {verts} vertices; Culmen {:.2}, Spatium {:.2}, Unda {:.2} ms); layers rasterised {}, atlas resets {}",
            builds[builds.len() / 2],
            tess[tess.len() / 2],
            per(parts[0]),
            per(parts[1]),
            per(parts[2]),
            after.layers_rasterised - before.layers_rasterised,
            after.atlas_resets - before.atlas_resets
        );
        assert_eq!(
            after.atlas_resets, before.atlas_resets,
            "{name}: the layer atlas reset in steady state"
        );
        if p95 > 4.0 {
            over.push(format!("{name}: p95 {p95:.2} ms"));
        }
    }
    assert!(over.is_empty(), "over the 4 ms window budget: {over:#?}");
}
