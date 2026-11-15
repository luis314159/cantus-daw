//! Round 40 (visual): a piano's lid never meets its music desk, nor its prop stick the desk,
//! at any angle the lid can take, for every body — the geometry both pictures (Drawn and
//! Photographic) build from (`scene::piano_geo`). The owner saw the desk poke up through
//! the lid on the parlour, the grand and the white concert grand (2026-11-13).

use cantus_core::inst::chorda_look::Body;
use cantus_ui::scene::geo::{v, V3};
use cantus_ui::scene::piano::{outline, GRAND_LID, RIM_HI, SHORT_STICK, UPRIGHT_LID};
use cantus_ui::scene::piano_geo::*;

const GRANDS: [Body; 3] = [Body::Concert, Body::Grand, Body::Parlour];

/// Every point of the lid's two leaves (the main lid and the flap folded on it), both faces.
fn lid_cloud(body: Body, deg: f32) -> Vec<V3> {
    let ang = deg.to_radians();
    let n = lid_normal(ang);
    let mut p = Vec::new();
    for &(x, z) in &lid_outline(body, 14) {
        let top = lid_point(x, z, ang, body);
        p.push(top);
        p.push(top - n * LID_TH);
    }
    for &(x, z) in &flap_outline(body, 14) {
        let top = lid_point(x, z, ang, body);
        p.push(top + n * LID_TH);
    }
    p
}

fn seg_dist(p: V3, a: V3, b: V3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
    (a + ab * t - p).len()
}

#[test]
fn the_desk_stands_in_front_of_the_lid_at_every_angle() {
    let desk = desk_extent();
    let desk_back = desk.iter().map(|p| p.z).fold(f32::MAX, f32::min);
    for body in GRANDS {
        for deg in lid_angles() {
            let lid = lid_cloud(body, deg);
            // The lid only turns about its hinge (along z): its plan depth never changes,
            // so the whole desk in front of its front edge clears it at any angle.
            let lid_front = lid.iter().map(|p| p.z).fold(f32::MIN, f32::max);
            assert!(
                desk_back > lid_front + CLEARANCE,
                "{body:?} at {deg}°: the desk's back at z {desk_back} reaches the lid's front \
                 edge at {lid_front}"
            );
            // And every point of the lid in the desk's span of depth is above or behind it.
            for p in &desk {
                for q in &lid {
                    if (q.z - p.z).abs() < CLEARANCE {
                        panic!("{body:?} at {deg}°: lid point {q:?} beside the desk's {p:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn the_closed_lid_lies_on_the_rim_and_the_desk_rises_in_front_of_it() {
    for body in GRANDS {
        let lid = lid_cloud(body, 0.0);
        let low = lid.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(
            (low - RIM_HI).abs() < 0.01,
            "{body:?}: the lid's underside at {low}"
        );
        let top = desk_extent().iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(
            top > RIM_HI + 20.0,
            "the desk stands up over the closed lid"
        );
    }
}

#[test]
fn the_prop_stick_stands_on_the_rim_and_holds_the_lid() {
    for body in GRANDS {
        let z = prop_z(body);
        let xr = rim_x(body, z);
        assert!(xr > 40.0, "{body:?}: the bentside at {z} is at {xr}");
        assert!(prop(body, 0.0).is_none(), "no stick under a closed lid");
        for deg in lid_angles().filter(|&d| d > 4.0) {
            let (foot, head) = prop(body, deg).expect("a stick");
            // On the rim's top: inside the outline, within its band.
            assert!(
                (foot.y - RIM_HI).abs() < 0.01 && foot.x < xr - 0.8 && foot.x > xr * 0.97 - 2.0,
                "{body:?} at {deg}°: the stick's foot {foot:?} is not on the rim (x {xr})"
            );
            // On the lid's underside, inside its outline.
            let ang = deg.to_radians();
            let n = lid_normal(ang);
            let under = lid_point(0.0, z, ang, body) - n * LID_TH;
            let off = (head - under).dot(n);
            assert!(
                off.abs() < 0.05,
                "{body:?} at {deg}°: the head is {off} off the lid"
            );
            let o = outline(body, 14);
            let hinge = hinge_x(body);
            let plan_x = hinge + (head + n * LID_TH - v(hinge, RIM_HI + LID_TH, z)).len();
            assert!(
                plan_x < xr - 1.0 && head.z < LID_FRONT,
                "{body:?} at {deg}°: the head lies off the lid ({plan_x} vs {xr})"
            );
            assert!(o.len() > 10);
            // Never near the desk.
            for p in desk_extent() {
                let d = seg_dist(p, foot, head);
                assert!(
                    d > 20.0,
                    "{body:?} at {deg}°: the stick passes {d} cm from the desk"
                );
            }
        }
        for deg in [SHORT_STICK, GRAND_LID] {
            let (foot, head) = prop(body, deg).unwrap();
            assert!(
                head.y > foot.y + 20.0,
                "{body:?}: a stick that stands at {deg}°"
            );
        }
    }
}

#[test]
fn the_uprights_top_folds_back_over_itself_and_never_meets_its_front() {
    let front = upright_front_extent();
    for k in 0..=(UPRIGHT_LID * 4.0) as i32 {
        let deg = k as f32 * 0.25;
        let leaf = upright_leaf(deg);
        for q in &leaf {
            assert!(
                q.y >= UP_H - 0.01,
                "at {deg}°: the top's leaf dips into the carcass at {q:?}"
            );
            for p in &front {
                assert!(
                    p.y < q.y - CLEARANCE,
                    "at {deg}°: the leaf meets the front at {p:?}"
                );
            }
        }
    }
    // Closed: the leaf lies on the carcass; folded: flat on the top's back part.
    let closed = upright_leaf(0.0);
    assert!(closed
        .iter()
        .all(|q| (q.y - UP_H).abs() < 0.01 || (q.y - UP_H - UP_TOP_TH).abs() < 0.01));
    let folded = upright_leaf(UPRIGHT_LID);
    assert!(folded
        .iter()
        .all(|q| q.y > UP_H + UP_TOP_TH - 0.01 && q.y < UP_H + 2.0 * UP_TOP_TH + 0.01));
    assert!(folded
        .iter()
        .all(|q| q.z <= UP_LEAF_HINGE + 0.01 && q.z > -UP_D - 1.0));
}
