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
    // (His in a jacket: a suit's sleeve stands well off the arm — about 15 cm flat at the
    // upper arm, 13 at the cuff. r42:arcus-polish2, the first measure (the arms' mesh cut by
    // the plane): a wrist is an ellipse, about 5.3 cm across and 3.7 deep for a woman (ANSUR
    // II's wrist), and her left one is seen near edge-on — 3.6 cm; his bent elbow, cut
    // square to the upper arm where the sleeve turns, 7.9. With the violin held as the
    // photographs show it her left elbow bends more: cut there the crook's bulk reads 9.0.)
    if woman {
        [(7.5, 10.5), (6.6, 9.4), (5.4, 9.4), (5.0, 7.6), (3.4, 6.4)]
    } else {
        [
            (9.5, 14.5),
            (9.0, 13.5),
            (7.5, 12.5),
            (7.0, 10.5),
            (6.0, 9.0),
        ]
    }
}

/// The silhouette's width (cm) across the arm at level `t` of segment `seg` (0 the upper
/// arm, 1 the forearm): where the arm's mesh crosses the plane across it there (in the
/// room) within 6 cm of its axis — each triangle edge's crossing, so a thinned forearm whose
/// rings lie a couple of centimetres apart is measured as well as a dense one —, projected;
/// their extent across the segment's way on screen, at the scale there.
fn width(arm: &probe::Arm, seg: usize, t: f32) -> Option<f32> {
    // The joints as the skin was posed (the bones' heads).
    use cantus_ui::scene::bowed_human as hu;
    let o = hu::side(arm.side);
    let j = |b: usize| arm.joints.iter().find(|x| x.0 == o + b).map(|x| x.2);
    let (a3, b3) = if seg == 0 {
        (j(hu::UPPER1)?, j(hu::FORE1)?)
    } else {
        (j(hu::FORE1)?, j(hu::HAND)?)
    };
    let ax = (b3 - a3).norm();
    let c = a3 + (b3 - a3) * t;
    let near = |w: cantus_ui::scene::geo::V3| {
        arm.screen
            .iter()
            .zip(&arm.pts)
            .min_by(|x, y| (*x.1 - w).len().total_cmp(&(*y.1 - w).len()))
            .map(|x| *x.0)
    };
    let (sa, sb) = (near(a3)?, near(b3)?);
    let d = sb - sa;
    if d.length() < 4.0 {
        return None;
    }
    let n = egui::vec2(-d.y, d.x).normalized();
    let (mut lo, mut hi, mut k, mut count) = (f32::MAX, f32::MIN, 0.0, 0);
    let side = |i: usize| (arm.pts[i] - c).dot(ax);
    for t in &arm.tris {
        for e in 0..3 {
            let (i, j) = (t[e] as usize, t[(e + 1) % 3] as usize);
            // Where it crosses the plane (a shared edge twice: the extent is the same).
            let (di, dj) = (side(i), side(j));
            if (di > 0.0) == (dj > 0.0) {
                continue;
            }
            let f = di / (di - dj);
            let p = arm.pts[i] + (arm.pts[j] - arm.pts[i]) * f;
            let q = p - c;
            if (q - ax * q.dot(ax)).len() > 6.0 {
                continue;
            }
            let at = arm.screen[i] + (arm.screen[j] - arm.screen[i]) * f;
            let across = (at - sa).dot(n);
            lo = lo.min(across);
            hi = hi.max(across);
            k += arm.scale[i] + (arm.scale[j] - arm.scale[i]) * f;
            count += 1;
        }
    }
    if count < 4 {
        return None;
    }
    Some((hi - lo) / (k / count as f32).max(1e-3))
}

/// r42:arcus-polish5 — how far the arm's elbow is bent (degrees: 0 straight), from its
/// joints as the skin was posed.
fn bend(arm: &probe::Arm) -> f32 {
    use cantus_ui::scene::bowed_human as hu;
    let o = hu::side(arm.side);
    let j = |b: usize| arm.joints.iter().find(|x| x.0 == o + b).map(|x| x.2);
    match (j(hu::UPPER1), j(hu::FORE1), j(hu::HAND)) {
        (Some(s), Some(e), Some(h)) => {
            let (a, b) = ((e - s).norm(), (h - e).norm());
            a.dot(b).clamp(-1.0, 1.0).acos().to_degrees()
        }
        _ => 0.0,
    }
}

/// r42:arcus-polish5 — past how many degrees an elbow is bent for the taper rule, and how
/// much wider than the upper arm's middle its crook may then measure.
const BENT: f32 = 45.0;
const BENT_MOST: f32 = 1.2;

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
                    .enumerate()
                    .map(|(i, &(lv, seg, t))| {
                        // (His deltoid is the jacket's shoulder, baked with the body: the live
                        // arm's part is cut there, so it is not measured from the arm.)
                        if i == 0 && !woman {
                            return f32::NAN;
                        }
                        width(arm, seg, t)
                            .unwrap_or_else(|| panic!("{hold:?} {name} arm {}: no {lv}", arm.side))
                    })
                    .collect();
                report += &format!(
                    "{hold:?} {name} {} arm ({:.0}° bent): {}\n",
                    if arm.side == 0 { "left" } else { "bow" },
                    bend(arm),
                    LEVELS
                        .iter()
                        .zip(&w)
                        .map(|(l, x)| format!("{} {x:.1}", l.0))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                for ((lv, _, _), (x, (lo, hi))) in LEVELS.iter().zip(w.iter().zip(bounds(woman))) {
                    if !x.is_nan() && !(lo..=hi).contains(x) {
                        bad.push(format!(
                            "{hold:?} {name} arm {}: the {lv} {x:.1} cm, not {lo}-{hi}",
                            arm.side
                        ));
                    }
                }
                // An arm tapers: the elbow under the upper arm's middle, the wrist narrowest.
                // (A jacket's sleeve: the forearm under the upper arm and the cuff no wider
                // than the forearm — it hangs open, and a bent elbow gathers it.)
                // r42:arcus-polish5 — an elbow bent past [`BENT`]: the cut square to the upper
                // arm at the elbow runs into the crook, along the forearm's start, and is
                // wider than the upper arm's middle on a real arm (ref-04's bare bow arm,
                // bent about 75°: cut there it reaches the forearm all the way to the
                // measure's 6 cm, 1.07–1.12 × the middle; her figure's bow elbow 1.07–1.11)
                // — so a bent elbow may be up to [`BENT_MOST`] × the middle, no more (a pad
                // at the joint still fails).
                let elbow_most = if bend(arm) > BENT {
                    w[1] * BENT_MOST
                } else {
                    w[1]
                };
                if woman && w[2] >= elbow_most {
                    bad.push(format!(
                        "{hold:?} {name} arm {}: no elbow ({:.0}° bent)",
                        arm.side,
                        bend(arm)
                    ));
                }
                if woman && (w[4] >= w[3] || w[4] >= w[2]) {
                    bad.push(format!(
                        "{hold:?} {name} arm {}: the wrist not the narrowest",
                        arm.side
                    ));
                }
                if !woman && (w[3] >= w[1] || w[4] > w[3] + 0.2) {
                    bad.push(format!("{hold:?} {name} arm {}: no taper", arm.side));
                }
            }
        }
    }
    eprintln!("{report}");
    scene::backend::set_default(SceneBackend::Vector);
    assert!(bad.is_empty(), "{}\n{report}", bad.join("\n"));
}
