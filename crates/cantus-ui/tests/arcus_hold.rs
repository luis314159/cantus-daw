//! r42:arcus-upper — what holds Arcus's instruments in their pictures (the owner,
//! 2026-11-24): the preference and its migration through the file the app reads at start,
//! the figure's left hand reaching the stopped note (to the pixel at 100 %), its positions
//! and its glide, and the stop's marker holding 3:1 in every theme.

use cantus_ui::scene::bowed::{rig_for, Bowed};
use cantus_ui::scene::bowed_draw;
use cantus_ui::scene::bowed_hold::{elbow, Hold, Holder, StopShow};
use cantus_ui::scene::Room;
use cantus_ui::settings::Settings;
use cantus_ui::views::inst::arcus::hand;

#[test]
fn the_fingertip_switch_migrates_into_how_the_stop_is_shown() {
    let read = |j: &str| serde_json::from_str::<Settings>(j).unwrap().migrated();
    assert_eq!(
        read(r#"{"arcus_finger": false}"#).arcus_stop,
        StopShow::None
    );
    assert_eq!(
        read(r#"{"arcus_finger": true}"#).arcus_stop,
        StopShow::Finger
    );
    assert_eq!(read("{}").arcus_stop, StopShow::Finger);
    assert_eq!(
        read(r#"{"arcus_stop": "marker"}"#).arcus_stop,
        StopShow::Marker
    );
    assert_eq!(read("{}").arcus_holder, Holder::default());
    // Through the file the app reads at start.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(
        &path,
        r#"{ "arcus_finger": false, "arcus_holder": { "hold": "player", "skin": 2 } }"#,
    )
    .unwrap();
    let s = Settings::load(&path);
    assert_eq!(s.arcus_stop, StopShow::None);
    assert_eq!(s.arcus_holder.hold, Hold::Player);
    assert_eq!(s.arcus_holder.skin, 2);
    let json = serde_json::to_string(&s).unwrap();
    assert!(
        !json.contains("arcus_finger") && json.contains(r#""arcus_stop":"none""#),
        "{json}"
    );
}

#[test]
fn the_hand_keeps_its_position_and_shifts_beyond_it() {
    // First position on the violin's A string (first finger a tone up): up to the fourth
    // finger's E, the hand stays.
    for s in [2.0, 4.0, 5.0, 7.0] {
        assert_eq!(hand::target(2.0, s, 5.0), 2.0, "{s} semitones");
    }
    // An octave up: it shifts so the note falls under a finger.
    let up = hand::target(2.0, 12.0, 5.0);
    assert!((7.0..=11.0).contains(&up), "{up}");
    assert!(12.0 - up <= 5.0);
    // An open string never moves it; a low note brings it back down.
    assert_eq!(hand::target(7.0, 0.0, 5.0), 7.0);
    assert!(hand::target(7.0, 3.0, 5.0) <= 3.0);
    // The shift glides: about a tenth of a second at 60 Hz, never past the target.
    let mut at = 2.0;
    let mut frames = 0;
    while at != 7.0 && frames < 60 {
        let n = hand::glide(at, 7.0, 1.0 / 60.0);
        assert!(n > at && n <= 7.0);
        at = n;
        frames += 1;
    }
    assert!((4..=20).contains(&frames), "{frames} frames");
}

#[test]
fn the_figures_hand_lands_on_the_stopped_point() {
    let size = egui::vec2(590.0, 430.0);
    for b in [Bowed::Violin, Bowed::Viola, Bowed::Cello, Bowed::Bass] {
        for hold in [Hold::Player, Hold::Mannequin, Hold::Woman, Hold::Man] {
            let h = Holder::of(hold);
            let rig = rig_for(b, h);
            let mut cam = bowed_draw::camera(b, h, Room::Studio);
            let fr = bowed_draw::frame_for(size);
            let [l, t, r, bt] = fr.inset;
            let fb = (l, t, size.x - l - r, size.y - t - bt);
            cam.fit(&bowed_draw::points(b, h), fb, fr.margin, fr.anchor);
            for (string, st) in [(1usize, 0.75f32), (2, 0.5), (3, 0.89)] {
                let l = hand::Left {
                    rig: &rig,
                    d: b.dims(),
                    string,
                    semis: hand::semis(st),
                    at: hand::target(1.0, hand::semis(st), hand::span(b.dims().held)),
                    span: hand::span(b.dims().held),
                    rock: 0.0,
                    side: rig.pose.ex,
                };
                let tip = hand::playing_tip(&l).expect("a stopped note");
                let [_, bridge, nut] = rig.strings[string];
                let stop = bridge + (nut - bridge) * st + rig.normal[string] * rig.gauge[string];
                let d = cam.p(tip) - cam.p(stop);
                assert!(
                    d.length() < 1.0,
                    "{b:?} {hold:?}: string {string} at {st}: {d:?} px off"
                );
            }
            // An open string lifts the finger.
            let open = hand::Left {
                rig: &rig,
                d: b.dims(),
                string: 0,
                semis: hand::semis(1.0),
                at: 3.0,
                rock: 0.0,
                side: rig.pose.ex,
                span: 5.0,
            };
            assert!(hand::playing_tip(&open).is_none());
        }
    }
}

#[test]
fn the_arms_elbow_keeps_the_arms_lengths() {
    use cantus_ui::scene::geo::v;
    let s = v(0.0, 100.0, 0.0);
    let w = v(30.0, 80.0, 20.0);
    let e = elbow(s, w, 28.0, 26.0, v(0.0, -1.0, 0.0));
    assert!(((e - s).len() - 28.0).abs() < 0.05);
    assert!(((w - e).len() - 26.0).abs() < 0.05);
    assert!(e.y < s.y.min(w.y) + 5.0, "the elbow falls toward its pole");
}

/// The stop's marker is the theme's focus colour on a halo of its own (as the scene's
/// selection): 3:1 at least in every theme, whatever wood or ebony is under it.
#[test]
fn the_stop_marker_holds_three_to_one_in_every_theme() {
    use cantus_ui::kit::appearance::PRESETS;
    use cantus_ui::kit::color::contrast;
    for (id, make) in PRESETS {
        let t = make().kit_tokens();
        let halo = cantus_ui::scene::halo_for(t.focus);
        let c = contrast(t.focus, halo);
        assert!(c >= 3.0, "{id}: the marker {c:.2}:1 on its halo");
    }
}

/// The owner (2026-11-24): the left hand passes behind the neck. Where the palm, the thumb
/// and the knuckles lie inside the neck's silhouette, the picture shows the neck's own
/// pixels (the baked picture's), not the figure — violin and cello, musician and
/// mannequin, both looks — away from the sounding string and the fingers' arches in front.
#[test]
fn the_left_hand_passes_behind_the_neck() {
    use cantus_core::inst::InstrumentKind;
    use cantus_engine::EngineHandle;
    use cantus_ui::scene::{self, SceneBackend, SceneView, Subject};
    use cantus_ui::{views, welcome_project, CantusApp};
    cantus_ui::views::inst::chorda::scene::hybrid::register();
    scene::set_blocking(true);
    // At twice the scale: a cello's neck is a dozen points wide.
    const PPP: f32 = 2.0;
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
    // (inst, the scene's instrument, the string, the key)
    let cases = [
        (2.0f32, Bowed::Violin, 3usize, 81u8),
        (0.0, Bowed::Cello, 3, 59),
    ];
    for be in [SceneBackend::Vector, SceneBackend::PerPixel] {
        scene::backend::set_default(be);
        for &(inst, b, string, key) in &cases {
            for hold in [Hold::Player, Hold::Mannequin, Hold::Woman, Hold::Man] {
                let h = Holder::of(hold);
                let mut app =
                    CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
                app.settings.arcus_holder = h;
                // Both levels of Figure detail: Light in the Drawn look, High in the
                // Photographic (the hand behind the neck either way).
                app.settings.figure_quality = if be == SceneBackend::Vector {
                    cantus_ui::scene::figure::Quality::Light
                } else {
                    cantus_ui::scene::figure::Quality::High
                };
                app.demo_editor("arcus");
                let t = app.inst.open.expect("an Arcus editor");
                app.set_inst_value(t, "inst", inst, false);
                views::inst::arcus::demo_feed(&mut app);
                if let Some(tele) = app.inst.injected.as_mut() {
                    tele[cantus_dsp::arcus::tel::STRING] = string as f32;
                    tele[cantus_dsp::arcus::tel::KEY] = key as f32;
                    tele[cantus_dsp::arcus::tel::VIBRATO] = 0.0;
                }
                let size = views::inst::editor_size(InstrumentKind::BowedString);
                let canvas = cantus_ui::raster::shoot(&ctx, size, PPP, 8, None, |ui| {
                    views::inst::window_in(&mut app, ui)
                });
                let sc = &app.inst.arcus_scenes[&t.0].0;
                let built = sc.built().expect("a picture");
                let rect = sc.rect().expect("painted");
                let k = egui::vec2(rect.width() / built.size.x, rect.height() / built.size.y);
                let to_px = |w: cantus_ui::scene::geo::V3| {
                    let q = built.cam.p(w);
                    rect.min + egui::vec2(q.x * k.x, q.y * k.y)
                };
                // The baked picture alone, at the same size.
                let view = SceneView {
                    subject: Subject::Bowed {
                        instrument: b,
                        holder: h,
                    },
                    room: scene::Room::Studio,
                    look: Default::default(),
                    dark: true,
                    selected: None,
                    hovered: None,
                    sounding: Vec::new(),
                    accent: egui::Color32::TRANSPARENT,
                    ink: egui::Color32::TRANSPARENT,
                };
                let baked = scene::image(&view, built.size, PPP);
                // The hand as the view placed it.
                let rig = rig_for(b, h);
                let arm = cantus_ui::scene::bowed_hold::left_arm(b, h).expect("a figure");
                let open = b_spec_open(b, string);
                let d = b.dims();
                let l = hand::Left {
                    rig: &rig,
                    d,
                    string,
                    semis: key as f32 - open as f32,
                    at: app.inst.arcus_hands[&t.0].0,
                    rock: 0.0,
                    side: rig.pose.ex,
                    span: hand::span(d.held),
                };
                let lp = hand::left_pose(&arm, &l);
                let eye = built.cam.eye();
                let local = |w: cantus_ui::scene::geo::V3| {
                    let q = w - rig.pose.o;
                    (q.dot(rig.pose.ex), q.dot(rig.pose.ey), q.dot(rig.pose.ez))
                };
                // The grip the owner's photographs show: the knuckles beside the
                // fingerboard on the fingers' side (a violin's E side, a cello's C side);
                // the thumb up the G side under a chin, behind the neck on a cello; from the
                // picture's eye the neck hides the thumb's joint, and the playing finger's
                // arch shows over the fingerboard.
                for f in &lp.fingers {
                    let (x, _, _) = local(f[0]);
                    assert!(
                        x > 1.0,
                        "{b:?} {hold:?}: a knuckle at x {x:.1}, not beside the fingerboard"
                    );
                }
                let (tx, ty, tz) = local(lp.thumb[3]);
                let top = cantus_ui::scene::bowed::board_z(d, ty);
                if d.held {
                    assert!(
                        tx < -1.0,
                        "{b:?} {hold:?}: the thumb at x {tx:.1}, not up the G side"
                    );
                    assert!(
                        tz < top + 1.0,
                        "{b:?} {hold:?}: the thumb's tip over the fingerboard"
                    );
                } else {
                    assert!(
                        tz < top - hand::neck_depth(d) + 0.2,
                        "{b:?} {hold:?}: the thumb at z {tz:.1}, not behind the neck"
                    );
                }
                assert!(
                    hand::hidden_by_neck(lp.thumb[2], eye, d, &rig.pose),
                    "{b:?} {hold:?}: the thumb's joint shows in front of the neck"
                );
                let j = lp.playing.expect("a stopped note");
                assert!(
                    !hand::hidden_by_neck(lp.fingers[j][1], eye, d, &rig.pose),
                    "{b:?} {hold:?}: the playing finger's arch is behind the neck"
                );
                let corners: [egui::Pos2; 8] = {
                    use cantus_ui::scene::bowed::{board_w, board_z, nut_y};
                    let depth = 0.06 * d.body + 0.6;
                    let mut c = [egui::Pos2::ZERO; 8];
                    let mut i = 0;
                    for y in [d.body - 0.5, nut_y(d) + 0.4] {
                        let half = board_w(d, y) * 0.5 + 0.15;
                        let top = board_z(d, y) + 0.5;
                        for x in [-half, half] {
                            c[i] = to_px(rig.pose.at(x, y, top));
                            c[i + 1] = to_px(rig.pose.at(x, y, top - depth));
                            i += 2;
                        }
                    }
                    c
                };
                let (hull, n) = hand::hull8(&corners);
                let in_neck = |p: egui::Pos2| {
                    (0..n).all(|i| {
                        let (a, e) = (hull[i], hull[(i + 1) % n]);
                        (e.x - a.x) * (p.y - a.y) - (e.y - a.y) * (p.x - a.x) >= 1.5
                    })
                };
                // The body's top and back outlines as the eye sees them.
                let o = cantus_ui::scene::bowed::outline(b, 2);
                let top: Vec<egui::Pos2> = o
                    .iter()
                    .map(|&(x, y)| to_px(rig.pose.at(x, y, d.arch)))
                    .collect();
                let back: Vec<egui::Pos2> = o
                    .iter()
                    .map(|&(x, y)| to_px(rig.pose.at(x, y, -cantus_ui::scene::bowed::rib(d, y))))
                    .collect();
                let in_poly = |poly: &[egui::Pos2], p: egui::Pos2| {
                    let mut c = false;
                    for i in 0..poly.len() {
                        let (a, e) = (poly[i], poly[(i + 1) % poly.len()]);
                        if (a.y > p.y) != (e.y > p.y)
                            && p.x < a.x + (p.y - a.y) * (e.x - a.x) / (e.y - a.y)
                        {
                            c = !c;
                        }
                    }
                    c
                };
                let inside = |p: egui::Pos2| in_neck(p) || in_poly(&top, p) || in_poly(&back, p);
                // Away from what is drawn in front: the sounding string, the fingers' arches.
                let [_, bridge, nut] = rig.strings[string];
                let near_seg = |p: egui::Pos2, a: egui::Pos2, e: egui::Pos2, r: f32| {
                    let ab = e - a;
                    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
                    (a + ab * t - p).length() < r
                };
                // A centimetre at the hand, in points: what a finger's or the string's
                // width is kept clear by.
                let cm = (to_px(lp.palm + rig.pose.ex) - to_px(lp.palm)).length();
                // What of the arm is drawn in front of the neck (any piece not wholly hidden
                // by it), with its width: points under it are not the neck's to show.
                let (pieces, np) = hand::left_pieces(&arm, &lp);
                let fronts: Vec<_> = pieces[..np]
                    .iter()
                    .filter(|pc| {
                        ![pc.a, pc.a.lerp(pc.b, 0.5), pc.b]
                            .iter()
                            .all(|&w| hand::hidden_by_neck(w, eye, d, &rig.pose))
                    })
                    .map(|pc| (to_px(pc.a), to_px(pc.b), pc.ra.max(pc.rb)))
                    .collect();
                let clear = |p: egui::Pos2| {
                    !near_seg(p, to_px(bridge), to_px(nut), 1.5 + 0.3 * cm)
                        && fronts
                            .iter()
                            .all(|&(a, e, r)| !near_seg(p, a, e, 1.5 + r * cm))
                };
                // (r42 integration: against the baked picture's nearest pixel within one —
                // the live stamp and the bake are two renderers, and on a thin line (the
                // purfling at the cello's heel, the neck's edge) they stand a pixel apart; the
                // figure showing over the instrument covers more than a pixel.)
                let pixel = |p: egui::Pos2| {
                    let got = canvas.pixel((p.x * PPP) as usize, (p.y * PPP) as usize);
                    let (bx, by) = (
                        ((p.x - rect.min.x) * PPP) as isize,
                        ((p.y - rect.min.y) * PPP) as isize,
                    );
                    let at = |x: isize, y: isize| {
                        let x = x.clamp(0, baked.size[0] as isize - 1) as usize;
                        let y = y.clamp(0, baked.size[1] as isize - 1) as usize;
                        baked.pixels[y * baked.size[0] + x]
                    };
                    let off = |want: egui::Color32| {
                        (got[0] as i32 - want.r() as i32)
                            .abs()
                            .max((got[1] as i32 - want.g() as i32).abs())
                            .max((got[2] as i32 - want.b() as i32).abs())
                    };
                    let want = at(bx, by);
                    let diff = (-1..=1)
                        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
                        .map(|(dx, dy)| off(at(bx + dx, by + dy)))
                        .min()
                        .unwrap_or(i32::MAX);
                    (got, want, diff)
                };
                let mut checked = 0;
                // Along the thumb and the upper arm: where the neck or the body hides
                // them, the picture's own pixels.
                let mut samples = Vec::new();
                // r42:arcus-hands, pass 7 — a realistic figure's own skin in front of the neck
                // (its triangles as the picture's eye sees them, each point at the pixel not
                // hidden by the neck): what it covers is the figure's to show — the wider
                // knuckles' index base over a hidden vertex's pixel (Vector Violin Man) is not
                // the hand showing through the neck. The stylised tubes' clearance (`fronts`)
                // does not reach the skinned hand's own breadth.
                let mut front_tris: Vec<([egui::Pos2; 3], [cantus_ui::scene::geo::V3; 3])> =
                    Vec::new();
                for i in 1..20 {
                    let t = i as f32 / 20.0;
                    for s in 0..3 {
                        samples.push((lp.thumb[s].lerp(lp.thumb[s + 1], t), true));
                    }
                    samples.push((lp.elbow.lerp(arm.shoulder, t), false));
                }
                // A realistic figure's own arm (sheet v7): its skinned vertices, as the live
                // layer poses them (the thumb behind a cello's neck, the upper arm behind its
                // body), every seventh.
                if let Some(who) = cantus_ui::scene::bowed_human::who(hold) {
                    use cantus_ui::scene::bowed_human as hu;
                    let fig = hu::figure(who);
                    let base = hu::rest_body(b, who);
                    let aim = hu::ArmAim {
                        elbow: lp.elbow,
                        wrist: lp.wrist,
                        hand: Some(hu::HandAim {
                            fingers: lp.fingers,
                            thumb: lp.thumb,
                            exact: lp.playing,
                            rots: lp.rots,
                        }),
                    };
                    let posed = hu::repose_arm(fig, &base.0, 0, &aim);
                    for (k, t) in fig.tris.iter().enumerate() {
                        if fig.part[k] & 2 == 0 || fig.part[k] & 1 != 0 {
                            continue;
                        }
                        let w = t.map(|i| posed.vertex(fig, i as usize));
                        if w.iter()
                            .all(|&q| hand::hidden_by_neck(q, eye, d, &rig.pose))
                        {
                            continue;
                        }
                        front_tris.push((w.map(to_px), w));
                    }
                    let mut seen = std::collections::HashSet::new();
                    for (k, t) in fig.tris.iter().enumerate() {
                        if fig.part[k] & 2 == 0 || fig.part[k] & 1 != 0 {
                            continue;
                        }
                        for &i in t {
                            if i % 7 == 0 && seen.insert(i) {
                                let w = posed.vertex(fig, i as usize);
                                let by_neck = hand::hidden_by_neck(w, eye, d, &rig.pose);
                                let (_, y, z) = local(w);
                                // Behind a held instrument's body the bow crosses (its
                                // stick is drawn over the picture): only the neck's there.
                                let behind =
                                    !d.held && z < -cantus_ui::scene::bowed::rib(d, y) - 0.5;
                                if by_neck || behind {
                                    samples.push((w, by_neck));
                                }
                            }
                        }
                    }
                }
                // Whether the figure's own skin in front of the neck covers the pixel `p`.
                let front_skin = |p: egui::Pos2| {
                    front_tris.iter().any(|(q, w)| {
                        let (a, b, c) = (q[0], q[1], q[2]);
                        let den = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
                        if den.abs() < 1e-6 {
                            return false;
                        }
                        let l0 = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / den;
                        let l1 = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / den;
                        let l2 = 1.0 - l0 - l1;
                        l0 >= 0.0
                            && l1 >= 0.0
                            && l2 >= 0.0
                            && !hand::hidden_by_neck(
                                w[0] * l0 + w[1] * l1 + w[2] * l2,
                                eye,
                                d,
                                &rig.pose,
                            )
                    })
                };
                for (w, by_neck) in samples {
                    let p = to_px(w);
                    let hid = if by_neck {
                        hand::hidden_by_neck(w, eye, d, &rig.pose) && in_neck(p)
                    } else {
                        inside(p)
                    };
                    if !hid || !clear(p) {
                        continue;
                    }
                    if front_skin(p) {
                        let (_, _, diff) = pixel(p);
                        if diff > 40 {
                            println!("{be:?} {b:?} {hold:?}: at {p:?} the figure's own front skin");
                        }
                        continue;
                    }
                    let (got, want, diff) = pixel(p);
                    // (r42:arcus-hands, pass 7: CANTUS_HOLD_SHOT=<dir> keeps the shot of a
                    // failing case, to look at what covers the pixel.)
                    if diff > 40 {
                        if let Ok(dir) = std::env::var("CANTUS_HOLD_SHOT") {
                            let _ = canvas.write_png(&std::path::Path::new(&dir).join(format!(
                                "hold-{be:?}-{b:?}-{hold:?}-{:.0}-{:.0}.png",
                                p.x, p.y
                            )));
                            println!(
                                "{be:?} {b:?} {hold:?}: {p:?} by_neck {by_neck}, local {:?}",
                                local(w)
                            );
                        }
                    }
                    assert!(
                        diff <= 40,
                        "{be:?} {b:?} {hold:?}: at {p:?} the figure shows over the instrument (got {got:?}, the picture's {want:?})"
                    );
                    checked += 1;
                }
                // The playing finger's arch is the figure's, over the fingerboard: along it
                // from its knuckle to its tip, the picture shows the finger somewhere.
                let f = lp.fingers[j];
                let shows = (0..3)
                    .flat_map(|s| [0.25f32, 0.5, 0.75].map(move |t| f[s].lerp(f[s + 1], t)))
                    .map(|w| pixel(to_px(w)).2)
                    .max()
                    .unwrap_or(0);
                assert!(
                    shows > 40,
                    "{be:?} {b:?} {hold:?}: the playing finger's arch is hidden ({shows} at most)"
                );
                // Under a chin the hand's back covers the thumb from the picture's eye and
                // the upper arm passes under the body in the open; behind a cello the neck
                // and the body hide the thumb and the arm.
                assert!(
                    checked >= 1 || d.held,
                    "{be:?} {b:?} {hold:?}: no part of the thumb or the arm lies behind the instrument"
                );
            }
        }
    }
    scene::backend::set_default(SceneBackend::Vector);
}

/// The open string's MIDI note.
fn b_spec_open(b: Bowed, string: usize) -> u8 {
    use cantus_dsp::arcus::tables::Instrument;
    let i = match b {
        Bowed::Violin => Instrument::Violin,
        Bowed::Viola => Instrument::Viola,
        Bowed::Bass => Instrument::Bass,
        Bowed::Cello => Instrument::Cello,
    };
    i.spec().strings[string].open
}

/// The left arm's angles against the owner's photographs (2026-11-24; the handoff note
/// lists them): under a chin the elbow straight under the violin's body toward the chest's
/// middle, the forearm and the neck a narrow V, the wrist straight; behind a cello the
/// elbow out to the side a little under the hand, the wrist straight. Printed with
/// `--nocapture` for the note.
#[test]
fn the_left_arm_keeps_the_photographs_angles() {
    use cantus_ui::scene::geo::V3;
    let deg = |a: V3, b: V3| a.norm().dot(b.norm()).clamp(-1.0, 1.0).acos().to_degrees();
    for b in [Bowed::Violin, Bowed::Viola, Bowed::Cello, Bowed::Bass] {
        for hold in [Hold::Player, Hold::Mannequin, Hold::Woman, Hold::Man] {
            let h = Holder::of(hold);
            let rig = rig_for(b, h);
            let d = b.dims();
            let arm = cantus_ui::scene::bowed_hold::left_arm(b, h).expect("a figure");
            for (string, semis) in [(2usize, 7.0f32), (2, 12.0)] {
                let span = hand::span(d.held);
                let l = hand::Left {
                    rig: &rig,
                    d,
                    string,
                    semis,
                    at: hand::target(1.0, semis, span),
                    rock: 0.0,
                    side: rig.pose.ex,
                    span,
                };
                let lp = hand::left_pose(&arm, &l);
                let kn = (lp.fingers[1][0] + lp.fingers[2][0]) * 0.5;
                let fore = lp.wrist - lp.elbow;
                let wrist = deg(fore, kn - lp.wrist);
                let elbow = deg(arm.shoulder - lp.elbow, lp.wrist - lp.elbow);
                let v = deg(kn - lp.elbow, rig.pose.ey);
                let up = V3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                };
                let fore_up = 90.0 - deg(fore, up);
                let upper_down = deg(lp.elbow - arm.shoulder, -up);
                let q = lp.elbow - rig.pose.o;
                let (ex, ey, ez) = (q.dot(rig.pose.ex), q.dot(rig.pose.ey), q.dot(rig.pose.ez));
                let below_hand = kn.y - lp.elbow.y;
                let out = (lp.elbow - arm.shoulder).dot(-arm.right);
                println!(
                    "{b:?} {hold:?} {semis} st: elbow {elbow:.0}°, wrist bend {wrist:.1}°, \
                     forearm/neck V {v:.0}°, forearm rise {fore_up:.0}°, upper arm from down \
                     {upper_down:.0}°, elbow in the instrument's frame ({ex:.1}, {ey:.1}, \
                     {ez:.1}) cm, {below_hand:.0} cm under the knuckles, {out:.0} cm out"
                );
                // `CANTUS_DUMP_HAND=1`: the pose as JSON, for tools/arcus-figures/hand_design.py.
                if std::env::var("CANTUS_DUMP_HAND").is_ok() {
                    let j = |p: V3| format!("[{:.3},{:.3},{:.3}]", p.x, p.y, p.z);
                    let js = |ps: &[V3]| ps.iter().map(|&p| j(p)).collect::<Vec<_>>().join(",");
                    let fs: Vec<String> =
                        lp.fingers.iter().map(|f| format!("[{}]", js(f))).collect();
                    let [_, br, nut] = rig.strings[string];
                    println!(
                        "HAND {{\"b\":\"{b:?}\",\"hold\":\"{hold:?}\",\"semis\":{semis},\"wrist\":{},\"elbow\":{},\"shoulder\":{},\"fingers\":[{}],\"thumb\":[{}],\"playing\":{},\"o\":{},\"ex\":{},\"ey\":{},\"ez\":{},\"bridge\":{},\"nut\":{}}}",
                        j(lp.wrist),
                        j(lp.elbow),
                        j(arm.shoulder),
                        fs.join(","),
                        js(&lp.thumb),
                        lp.playing.map_or(-1, |p| p as i32),
                        j(rig.pose.o),
                        j(rig.pose.ex),
                        j(rig.pose.ey),
                        j(rig.pose.ez),
                        j(br),
                        j(nut)
                    );
                }
                // (r42:arcus-hand: the forearm may turn toward the little finger, as the
                // photographs show it — r42:arcus-hands: they measure 3–23°, median ~10
                // (`left_hand_refs.md`); the table rests at 9 and turns at most 14: within 16°.)
                // (r42:arcus-hands, pass 4: from the 9th semitone 30°, as arcus_hand_table's
                // WRIST_HIGH — high up the hand now shifts to the 9th position for the octave
                // rather than reach it from the 7th with the little finger, and there the
                // wrist bends toward the back beside the body's shoulder, as the owner asked.)
                let most = if semis >= 9.0 && d.held { 30.0 } else { 16.0 };
                assert!(wrist < most, "{b:?} {hold:?}: the wrist bent {wrist:.1}°");
                // (A cellist's hand comes up beside the shoulder: the elbow folds more.)
                assert!(
                    (if d.held { 50.0 } else { 30.0 }..=130.0).contains(&elbow),
                    "{b:?} {hold:?}: elbow {elbow:.0}°"
                );
                if d.held {
                    // Under the body (its length 0..body), below its back, a narrow V.
                    assert!(
                        ey > 0.0 && ey < d.body,
                        "{b:?} {hold:?}: elbow at y {ey:.1}"
                    );
                    assert!(
                        ez < -12.0,
                        "{b:?} {hold:?}: elbow only {ez:.1} cm under the violin"
                    );
                    // (Up the neck toward the body the forearm rises more steeply under it:
                    // a wider V at the octave.)
                    // (r42:arcus-hand: the photographs of professional violinists measure
                    // 50–85° — tools/arcus-figures/left_hand_refs.md.)
                    let v_max = if semis > 9.0 { 88.0 } else { 85.0 };
                    assert!(v < v_max, "{b:?} {hold:?}: forearm and neck {v:.0}° apart");
                } else {
                    assert!(out > 10.0, "{b:?} {hold:?}: the elbow {out:.0} cm out");
                    assert!(
                        (0.0..=30.0).contains(&below_hand),
                        "{b:?} {hold:?}: the elbow {below_hand:.0} cm under the hand"
                    );
                }
            }
        }
    }
}
