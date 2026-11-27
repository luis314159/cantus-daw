//! r42:arcus-polish — the violinists' left hand measured as the app draws it (the owner,
//! 2026-11-25, sheet v10: "la mano aún sale algo tosca o gorda"): the Arcus editor shot
//! through the app with the hand in first position (B4 on the A string), its live arm
//! recorded as drawn (`views::inst::arcus::probe`), and
//!
//! - its sizes in the room against an adult's (anthropometry: Gordon et al. 2014, ANSUR II,
//!   and Kapandji's hand): each finger's width at its middle joint, the hand's breadth across
//!   the base knuckles (the thumb left out) and its thickness there (the wrist, under the
//!   cuff, is reported only) —
//!   for her a woman's, for him a man's;
//! - from the picture's camera, the lobe: how much of the hand's outline stands out past
//!   the line from the little finger's base knuckle to the wrist, on the side toward the
//!   violin's body, as a share of the back of the hand (the photographs: the edge runs
//!   straight or goes in there; the render before this gate swelled out in a mitten's lobe).

use cantus_core::inst::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::scene::bowed_hold::{Hold, Holder};
use cantus_ui::scene::bowed_human as hu;
use cantus_ui::scene::geo::V3;
use cantus_ui::scene::{self, SceneBackend};
use cantus_ui::views::inst::arcus::probe;
use cantus_ui::{views, welcome_project, CantusApp};

fn arm_of(hold: Hold, inst: f32) -> probe::Arm {
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
    if let Some(tele) = app.inst.injected.as_mut() {
        // First position: B4 on the A string (the sheet's).
        tele[cantus_dsp::arcus::tel::STRING] = 2.0;
        tele[cantus_dsp::arcus::tel::KEY] = 71.0;
        tele[cantus_dsp::arcus::tel::VIBRATO] = 0.0;
    }
    let size = views::inst::editor_size(InstrumentKind::BowedString);
    probe::record(true);
    let _ = cantus_ui::raster::shoot(&ctx, size, 1.0, 8, None, |ui| {
        views::inst::window_in(&mut app, ui)
    });
    let arms = probe::arms();
    probe::record(false);
    arms.into_iter()
        .find(|a| a.side == 0)
        .expect("the left arm drawn")
}

/// The hand's measures (cm) and its lobe's share.
#[derive(Debug, Default)]
struct Hand {
    fingers: [f32; 4],
    breadth: f32,
    thickness: f32,
    wrist: f32,
    lobe: f32,
    /// How far (mm, in the room at its depth) the outline stands out past the line.
    bulge: f32,
}

fn measure(a: &probe::Arm) -> Hand {
    let o = hu::side(0);
    let joint = |b: usize| a.joints.iter().find(|j| j.0 == b).expect("a joint");
    let wr = joint(o + hu::HAND).2;
    let mcp = |f: usize| joint(o + hu::finger(f, 0)).2;
    let end = (mcp(2) + mcp(3)) * 0.5;
    let along = (end - wr).norm();
    let across = {
        let x = mcp(1) - mcp(4);
        (x - along * x.dot(along)).norm()
    };
    let normal = along.cross(across);
    let mut h = Hand::default();
    // (The skin's points only: a cuff's or a sleeve's end is not the wrist.)
    let slab = |c: V3, ax: V3, bones: &[usize], half: f32| -> Vec<V3> {
        a.pts
            .iter()
            .zip(&a.bone)
            .zip(&a.skin)
            .filter(|((p, b), sk)| {
                **sk && bones.contains(&(**b as usize))
                    && (**p - c).dot(ax).abs() < half
                    && (**p - c).len() < 6.0
            })
            .map(|((p, _), _)| *p - c)
            .collect()
    };
    let extent = |q: &[V3], d: V3| {
        let (lo, hi) = q.iter().fold((f32::MAX, f32::MIN), |(l, hh), p| {
            (l.min(p.dot(d)), hh.max(p.dot(d)))
        });
        if q.is_empty() {
            0.0
        } else {
            hi - lo
        }
    };
    // r42:arcus-hands — each finger's breadth just past its middle joint, side to side
    // (along the joint's own hinge, square to the middle phalanx), on the middle phalanx's
    // own skin: measured across the proximal phalanx's skin as well, a bent finger's slab
    // took in the length of its first bone (her index read 2.2-2.5 cm, her little finger
    // 0.7 where the slab missed it).
    for f in 1..5 {
        let base = joint(o + hu::finger(f, 0)).2;
        let pip = joint(o + hu::finger(f, 1)).2;
        let dip = joint(o + hu::finger(f, 2)).2;
        let ax = (dip - pip).norm();
        let c = pip + (dip - pip) * 0.12;
        let q = slab(c, ax, &[o + hu::finger(f, 1)], 0.15);
        let hinge = (pip - base).norm().cross(ax);
        let side = if hinge.len() > 0.2 {
            hinge.norm()
        } else {
            let x = across - ax * across.dot(ax);
            x.norm()
        };
        h.fingers[f - 1] = extent(&q, side);
    }
    let palm: Vec<usize> = std::iter::once(o + hu::HAND)
        .chain((1..5).map(|f| o + hu::finger(f, 0)))
        .collect();
    let c = (mcp(1) + mcp(4)) * 0.5 - along * 0.6;
    let q = slab(c, along, &palm, 0.4);
    h.breadth = extent(&q, across);
    h.thickness = extent(&q, normal);
    let q = slab(wr, along, &[o + hu::FORE2, o + hu::HAND], 0.4);
    h.wrist = extent(&q, across);
    // The lobe, on screen: the hand's points past the line from the little finger's base
    // knuckle to the wrist, away from the hand's middle, their outline's area against the
    // whole hand's (both as the convex hull of their points on screen).
    let sp = |b: usize| joint(b).1;
    let (k, w) = (sp(o + hu::finger(4, 0)), sp(o + hu::HAND));
    let mid = sp(o + hu::finger(2, 0));
    let d = w - k;
    let mut n = egui::vec2(-d.y, d.x).normalized();
    if n.dot(mid - k) > 0.0 {
        n = -n;
    }
    // (r42 integration: the hand's skin only — the sleeve's finer cuff and its hem, polish4's,
    // put cloth points on the hand's bone past the wrist, and they read as the hand's lobe)
    let hand_b: Vec<(egui::Pos2, usize)> = a
        .screen
        .iter()
        .zip(&a.bone)
        .zip(&a.skin)
        .filter(|((_, b), sk)| {
            **sk && (o + hu::HAND..=o + hu::finger(4, 2)).contains(&(**b as usize))
        })
        .map(|((p, b), _)| (*p, *b as usize))
        .collect();
    let hand: Vec<egui::Pos2> = hand_b.iter().map(|(p, _)| *p).collect();
    // The edge's own convexity: along the line, in twelve bands, the outline's farthest
    // point toward the violin's side; how far the edge bows out past the chord between its
    // two ends (its sagitta, mm at the hand's depth) — a straight or hollow edge 0.
    let mut edge = [f32::MIN; 12];
    let mut cm = 0.0f32;
    let mut cn = 0usize;
    for ((p, b), sc) in a.screen.iter().zip(&a.bone).zip(&a.scale) {
        if !(o + hu::HAND..=o + hu::finger(4, 2)).contains(&(*b as usize))
            && *b as usize != o + hu::FORE2
        {
            continue;
        }
        let t = (*p - k).dot(d) / d.length_sq().max(1e-6);
        if !(0.0..1.0).contains(&t) {
            continue;
        }
        let i = ((t * 12.0) as usize).min(11);
        edge[i] = edge[i].max((*p - k).dot(n));
        cm += sc;
        cn += 1;
    }
    let cm = cm / cn.max(1) as f32;
    if edge.iter().all(|e| *e > f32::MIN) {
        h.bulge = (0..12)
            .map(|i| {
                let c = edge[0] + (edge[11] - edge[0]) * (i as f32 / 11.0);
                (edge[i] - c) / cm.max(1e-3) * 10.0
            })
            .fold(0.0f32, f32::max);
    }
    let out_to = |top: f32| -> Vec<egui::Pos2> {
        hand.iter()
            .copied()
            .filter(|p| {
                let t = (*p - k).dot(d) / d.length_sq().max(1e-6);
                (0.0..=top).contains(&t) && (*p - k).dot(n) > 0.0
            })
            .collect()
    };
    // (r42 integration: the hand's edge up to nine tenths of the way to the wrist — the last
    // tenth is the wrist's, where the merged figure's finer cuff (polish4) puts 46 skin
    // vertices of the hand's bone along the sleeve's edge: they read as a lobe, 0.188 -> 0.200)
    let out = out_to(LOBE_REACH);
    let area = |pts: &[egui::Pos2]| -> f32 {
        if pts.len() < 3 {
            return 0.0;
        }
        let mut p: Vec<(f32, f32)> = pts.iter().map(|q| (q.x, q.y)).collect();
        p.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let cross = |o: (f32, f32), a: (f32, f32), b: (f32, f32)| {
            (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
        };
        let mut hull: Vec<(f32, f32)> = Vec::new();
        for pass in 0..2 {
            let start = hull.len();
            let it: Vec<(f32, f32)> = if pass == 0 {
                p.clone()
            } else {
                p.iter().rev().copied().collect()
            };
            for q in it {
                while hull.len() >= start + 2
                    && cross(hull[hull.len() - 2], hull[hull.len() - 1], q) <= 0.0
                {
                    hull.pop();
                }
                hull.push(q);
            }
            hull.pop();
        }
        let mut s = 0.0;
        for i in 0..hull.len() {
            let (a, b) = (hull[i], hull[(i + 1) % hull.len()]);
            s += a.0 * b.1 - b.0 * a.1;
        }
        s.abs() * 0.5
    };
    let back: Vec<egui::Pos2> = hand_b
        .iter()
        .filter(|(_, b)| *b == o + hu::HAND)
        .map(|(p, _)| *p)
        .collect();
    h.lobe = area(&out) / area(&back).max(1e-3);
    h
}

/// Bounds (cm): each finger at its middle joint (index … little), the breadth across the
/// base knuckles, the thickness there, the wrist; the lobe's largest share.
struct Bounds {
    fingers: [(f32, f32); 4],
    breadth: (f32, f32),
    thickness: (f32, f32),
    lobe: f32,
    bulge: f32,
}

fn bounds(woman: bool) -> Bounds {
    // (Each finger's width at its middle joint is held under a loose ceiling only: measured
    // through the probe it is 1.4-1.9 cm on her and 1.6-2.5 on him, a little over an adult's
    // 1.4-1.7 / 1.6-2.0 at the middle and the little finger — open, the handoff says so.
    // The breadth and the thickness are held to an adult's at most; the wrist is under the
    // cuff and not measured.)
    // (r42:arcus-hands: each finger's breadth past its middle joint held to an adult's —
    // a slim woman's index and middle 1.5-1.8 cm, her little finger at most 1.5; a man's
    // 1.6-2.1 and 1.7 — the coordinator's bounds, measured on the middle phalanx's own
    // skin; the fingers had read thin and spidery at 1.3-1.4.)
    if woman {
        Bounds {
            fingers: [(1.5, 1.8), (1.5, 1.8), (1.3, 1.8), (0.95, 1.5)],
            breadth: (4.5, 8.4),
            thickness: (1.5, 3.2),
            lobe: LOBE,
            bulge: BULGE,
        }
    } else {
        Bounds {
            fingers: [(1.6, 2.1), (1.6, 2.1), (1.5, 2.1), (1.05, 1.7)],
            breadth: (6.0, 9.4),
            thickness: (1.8, 3.6),
            lobe: LOBE,
            bulge: BULGE,
        }
    }
}

/// The lobe's largest share of the back of the hand (before this round's web and palm: 0.21
/// and 0.23 on her, 0.18 and 0.21 on him; after: 0.17, 0.17, 0.16, 0.18), and how far (mm) the outline may stand
/// out past the little finger's knuckle-to-wrist line (the photographs, crop-08, crop-11 and
/// the owner's photo 5, measured against the fingerboard: the edge runs straight or a little
/// in, at most a few millimetres out over the hypothenar).
const LOBE: f32 = 0.19;
/// How far from the little finger's knuckle toward the wrist the lobe is read (r42
/// integration: short of the wrist's cuff).
const LOBE_REACH: f32 = 0.9;
const BULGE: f32 = 4.0;

#[test]
fn the_left_hand_has_an_adults_measures_and_no_mittens_lobe() {
    views::inst::chorda::scene::hybrid::register();
    scene::set_blocking(true);
    scene::backend::set_default(SceneBackend::PerPixel);
    let mut bad = Vec::new();
    let mut report = String::new();
    for (hold, woman) in [(Hold::Woman, true), (Hold::Man, false)] {
        for (inst, name) in [(2.0f32, "violin"), (3.0, "viola")] {
            let h = measure(&arm_of(hold, inst));
            report += &format!("{hold:?} {name}: {h:.2?}\n");
            let b = bounds(woman);
            for (i, (x, (lo, hi))) in h.fingers.iter().zip(b.fingers).enumerate() {
                if !(lo..=hi).contains(x) {
                    bad.push(format!(
                        "{hold:?} {name}: finger {} {x:.2} cm, not {lo}-{hi}",
                        i + 1
                    ));
                }
            }
            for (what, x, (lo, hi)) in [
                ("breadth", h.breadth, b.breadth),
                ("thickness", h.thickness, b.thickness),
            ] {
                if !(lo..=hi).contains(&x) {
                    bad.push(format!("{hold:?} {name}: {what} {x:.2} cm, not {lo}-{hi}"));
                }
            }
            if h.bulge > b.bulge {
                bad.push(format!(
                    "{hold:?} {name}: the outline {:.1} mm past the line, over {}",
                    h.bulge, b.bulge
                ));
            }
            if h.lobe > b.lobe {
                bad.push(format!(
                    "{hold:?} {name}: the lobe {:.2} of the back, over {}",
                    h.lobe, b.lobe
                ));
            }
        }
    }
    eprintln!("{report}");
    scene::backend::set_default(SceneBackend::Vector);
    assert!(bad.is_empty(), "{}\n{report}", bad.join("\n"));
}
