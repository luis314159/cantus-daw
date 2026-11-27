//! r42:arcus-upper — the figure module's contract has a second client (ADR 0159): a figure
//! standing, both hands on a stick held level in front of it, posed from the PlayRig alone
//! (no instrument, no stylised skeleton). Proves the contract is not Arcus-shaped: the feet
//! on the floor, the bones' lengths kept, both hands at their grips on the stick, the head
//! turned to where it looks, the realistic body baked into a picture's mesh.

use cantus_ui::scene::bowed_human::{self as hu, Who};
use cantus_ui::scene::figure::grip::Grip;
use cantus_ui::scene::figure::play::{self, Hand, Head, HeadAt, Legs, PlayRig, Seat};
use cantus_ui::scene::geo::{v, V3};

fn stick_rig(who: Who) -> (PlayRig, V3, V3) {
    let fig = hu::figure(who);
    let eye_h = fig.mark("eye.L").y - fig.mark("sole").y;
    // Facing +z, its right toward -x (the figure's left is +x at rest).
    let (facing, right) = (v(0.0, 0.0, 1.0), v(-1.0, 0.0, 0.0));
    let (a, b) = (v(24.0, 100.0, 32.0), v(-24.0, 100.0, 32.0));
    let up = v(0.0, 1.0, 0.0);
    let grip = |at: f32| Grip::Rod {
        a,
        b,
        at,
        r: 1.4,
        back: up,
        wrap: facing,
    };
    let look = (a + b) * 0.5;
    let face = (look - v(0.0, eye_h, 6.0)).norm();
    let rig = PlayRig {
        facing,
        right,
        head: Head {
            at: HeadAt::Eyes(v(0.0, eye_h, 0.0)),
            up: (up - face * up.dot(face)).norm(),
            face,
        },
        seat: Seat::Standing {
            at: v(0.0, 0.0, -4.0),
            stance: 22.0,
        },
        legs: Legs::Straight,
        hands: [
            Hand::Grip(grip(0.22), v(0.0, -1.0, 0.0) - right * 0.5),
            Hand::Grip(grip(0.78), v(0.0, -1.0, 0.0) + right * 0.5),
        ],
        occluders: Vec::new(),
        shrug: [0.0; 2],
    };
    (rig, a, b)
}

#[test]
fn a_standing_figure_holds_a_stick_from_the_contract_alone() {
    for who in [Who::Woman, Who::Man] {
        let fig = hu::figure(who);
        let (rig, a, b) = stick_rig(who);
        let (posed, seat) = play::pose(fig, &rig);
        assert_eq!(seat, 0.0, "{who:?}: standing has no seat");
        let (pts, _) = hu::skin(fig, &posed);
        // The soles on the floor.
        let low = pts.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(low.abs() < 2.5, "{who:?}: the lowest point at {low:.1} cm");
        // The arms keep their lengths (two bones, shoulder to wrist).
        for sd in 0..2 {
            let o = hu::side(sd);
            let (up, fo) = hu::arm_lengths(fig, sd);
            let sh = hu::shoulder(fig, &posed, sd);
            let el = posed.at(o + hu::FORE1, fig.bones[o + hu::FORE1].head);
            let wr = posed.at(o + hu::HAND, fig.bones[o + hu::HAND].head);
            assert!(
                ((el - sh).len() - up).abs() < 0.5,
                "{who:?} {sd}: upper arm"
            );
            assert!(((wr - el).len() - fo).abs() < 0.5, "{who:?} {sd}: forearm");
            // Each hand on the stick: its middle finger's middle joint wraps over it, its
            // knuckles a hand's thickness from it.
            let ab = b - a;
            let near = |p: V3| {
                let t = ((p - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
                (a + ab * t - p).len()
            };
            let kn = posed.at(o + hu::finger(2, 0), fig.bones[o + hu::finger(2, 0)].head);
            let mid = posed.at(o + hu::finger(2, 1), fig.bones[o + hu::finger(2, 1)].head);
            assert!(
                near(kn) < 6.0,
                "{who:?} {sd}: the knuckles {:.1} cm off",
                near(kn)
            );
            assert!(
                near(mid) < 4.0,
                "{who:?} {sd}: the finger {:.1} cm off",
                near(mid)
            );
        }
        // The head turned toward the stick.
        let eyes = posed.at(hu::HEAD, (fig.mark("eye.L") + fig.mark("eye.R")) * 0.5);
        let nose = posed.at(
            hu::HEAD,
            (fig.mark("eye.L") + fig.mark("eye.R")) * 0.5 + v(0.0, 0.0, 5.0),
        );
        let look = (nose - eyes).norm();
        assert!(
            look.y < -0.2,
            "{who:?}: the head looks down to the stick ({look:?})"
        );
        // Baked as a picture's mesh, as an instrument's figure is.
        let mut m = cantus_ui::scene::bowed_mesh::Mesh::default();
        let (pts, nrm) = hu::skin(fig, &posed);
        hu::emit(&mut m, fig, &pts, &nrm, who, 1, 7, &[]);
        let tris: usize = m.tris.iter().map(|(_, t)| t.idx.len()).sum();
        assert!(tris > 10_000, "{who:?}: {tris} triangles baked");
    }
}
