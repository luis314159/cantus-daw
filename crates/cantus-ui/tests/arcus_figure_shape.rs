//! r42:arcus-polish — the realistic violinists' arms measured as the app draws them (the
//! owner, 2026-11-25: "thick, long, uniform tubes"; a slim adult woman's arms, a man's): the
//! Arcus editor shot through the app (Photographic, High, both cameras' figures with the
//! violin and the viola), the live arms recorded as they are drawn
//! (`views::inst::arcus::probe`), and each arm's silhouette across it — at the deltoid, the
//! upper arm's middle, the elbow, the forearm's middle and the wrist — in centimetres at its
//! own depth. The widths must lie within a woman's or a man's (anthropometry with the
//! clothes on: ANSUR II's girths, Gordon et al. 2014, as diameters, and the sleeve's
//! thickness), and taper as an arm does: the elbow narrower than the upper arm's middle, the
//! wrist the narrowest.

use cantus_core::inst::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::scene::bowed_hold::{Hold, Holder};
use cantus_ui::scene::{self, SceneBackend};
use cantus_ui::views::inst::arcus::probe;
use cantus_ui::{views, welcome_project, CantusApp};

/// (name, segment 0 upper arm / 1 forearm, where along it) — the levels measured.
const LEVELS: [(&str, usize, f32); 5] = [
    ("deltoid", 0, 0.18),
    ("upper arm", 0, 0.5),
    ("elbow", 0, 0.93),
    ("forearm", 1, 0.5),
    ("wrist", 1, 0.9),
];

/// The widths allowed (cm), per level: a slim woman in velvet sleeves, a man in a jacket.
fn bounds(woman: bool) -> [(f32, f32); 5] {
    if woman {
        [(7.5, 10.5), (6.6, 9.0), (5.4, 7.8), (5.0, 7.4), (4.0, 6.0)]
    } else {
        [(9.0, 13.5), (7.8, 11.0), (6.8, 9.6), (6.4, 9.2), (5.2, 7.4)]
    }
}

/// The silhouette's width (cm) across the arm at level `t` of segment `seg` (0 the upper
/// arm, 1 the forearm): the points of the bones about that level within a centimetre of the
/// plane across the arm there (in the room), projected; their extent across the segment's
/// way on screen, at the scale there.
fn width(arm: &probe::Arm, seg: usize, t: f32) -> Option<f32> {
    use cantus_ui::scene::bowed_human as hu;
    let o = hu::side(arm.side);
    let (a3, b3) = if seg == 0 {
        (arm.shoulder, arm.elbow)
    } else {
        (arm.elbow, arm.wrist)
    };
    // The bones that carry the skin about this level (the elbow's both sides, the wrist's
    // the forearm's and the hand's).
    let bones: Vec<usize> = match (seg, t > 0.8) {
        (0, false) => vec![o + hu::UPPER1, o + hu::UPPER2],
        (0, true) => vec![o + hu::UPPER2, o + hu::FORE1],
        (_, false) => vec![o + hu::FORE1, o + hu::FORE2],
        (_, true) => vec![o + hu::FORE2, o + hu::HAND],
    };
    let ax = (b3 - a3).norm();
    let c = a3 + (b3 - a3) * t;
    let sa = arm.screen.iter().zip(&arm.pts).min_by(|x, y| (*x.1 - a3).len().total_cmp(&(*y.1 - a3).len()))?.0;
    let sb = arm.screen.iter().zip(&arm.pts).min_by(|x, y| (*x.1 - b3).len().total_cmp(&(*y.1 - b3).len()))?.0;
    let d = *sb - *sa;
    if d.length() < 4.0 {
        return None;
    }
    let n = egui::vec2(-d.y, d.x).normalized();
    let (mut lo, mut hi, mut k, mut count) = (f32::MAX, f32::MIN, 0.0, 0);
    for (i, &p) in arm.pts.iter().enumerate() {
        if !bones.contains(&(arm.bone[i] as usize)) || (p - c).dot(ax).abs() > 1.0 {
            continue;
        }
        if (p - c - ax * (p - c).dot(ax)).len() > 8.0 {
            continue;
        }
        let across = (arm.screen[i] - *sa).dot(n);
        lo = lo.min(across);
        hi = hi.max(across);
        k += arm.scale[i];
        count += 1;
    }
    if count < 4 {
        return None;
    }
    Some((hi - lo) / (k / count as f32).max(1e-3))
}

/// The arms of `hold` with instrument `inst` (2 violin, 3 viola), as the app draws them.
fn arms_of(hold: Hold, inst: f32) -> Vec<probe::Arm> {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings.arcus_holder = Holder::of(hold);
    app.settings.figure_quality = cantus_ui::scene::figure::Quality::High;
    app.demo_editor("arcus");
    let t = app.inst.open.expect("an Arcus editor");
    app.set_inst_value(t, "inst", inst, false);
    views::inst::arcus::demo_feed(&mut app);
    let size = views::inst::editor_size(InstrumentKind::BowedString);
    probe::record(true);
    let _ = cantus_ui::raster::shoot(&ctx, size, 1.0, 8, None, |ui| {
        views::inst::window_in(&mut app, ui)
    });
    let arms = probe::arms();
    probe::record(false);
    arms
}

#[test]
fn the_violinists_arms_have_a_womans_and_a_mans_proportions() {
    views::inst::chorda::scene::hybrid::register();
    scene::set_blocking(true);
    scene::backend::set_default(SceneBackend::PerPixel);
    let mut report = String::new();
    let mut bad: Vec<String> = Vec::new();
    for (hold, woman) in [(Hold::Woman, true), (Hold::Man, false)] {
        for (inst, name) in [(2.0f32, "violin"), (3.0, "viola")] {
            let arms = arms_of(hold, inst);
            assert_eq!(arms.len(), 2, "{hold:?} {name}: both arms drawn");
            for arm in &arms {
                let w: Vec<f32> = LEVELS
                    .iter()
                    .map(|&(lv, seg, t)| {
                        width(arm, seg, t)
                            .unwrap_or_else(|| panic!("{hold:?} {name} arm {}: no {lv}", arm.side))
                    })
                    .collect();
                report += &format!(
                    "{hold:?} {name} {} arm: {}\n",
                    if arm.side == 0 { "left" } else { "bow" },
                    LEVELS
                        .iter()
                        .zip(&w)
                        .map(|(l, x)| format!("{} {x:.1}", l.0))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                for ((lv, _, _), (x, (lo, hi))) in LEVELS.iter().zip(w.iter().zip(bounds(woman))) {
                    if !(lo..=hi).contains(x) {
                        bad.push(format!(
                            "{hold:?} {name} arm {}: the {lv} {x:.1} cm, not {lo}-{hi}",
                            arm.side
                        ));
                    }
                }
                // An arm tapers: the elbow under the upper arm's middle, the wrist narrowest.
                if w[2] >= w[1] {
                    bad.push(format!("{hold:?} {name} arm {}: no elbow", arm.side));
                }
                if w[4] >= w[3] || w[4] >= w[2] {
                    bad.push(format!("{hold:?} {name} arm {}: the wrist not the narrowest", arm.side));
                }
            }
        }
    }
    eprintln!("{report}");
    scene::backend::set_default(SceneBackend::Vector);
    assert!(bad.is_empty(), "{}\n{report}", bad.join("\n"));
}
