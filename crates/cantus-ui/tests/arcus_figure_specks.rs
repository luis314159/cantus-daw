//! r42:arcus-polish — the stray-pixel gate (the owner, 2026-11-25: specks and shards "have
//! come back three times, so it must be automatic"). The Arcus editor is shot through the
//! app with each realistic violinist (the woman, the man) holding the violin and the viola,
//! from the picture's camera and the earlier one, Night and Paper, Drawn and Photographic.
//! What the picture shows is known: the baked mesh (the instrument, the chair, the body),
//! the live arms as drawn (`views::inst::arcus::probe`) and the bow — rasterised here into
//! a mask of the subject. A speck is a small group of pixels (at most a few) darker or
//! lighter than every pixel round it, on surroundings that are smooth; a shard a larger
//! one of the same kind. The gate fails on
//!
//! - any speck or shard **outside** the subject's mask grown by a few pixels (the room
//!   round a figure is a soft, out-of-focus wall: nothing small and sharp belongs there);
//! - any speck **inside** the figure's skin and clothes (its hair, brows, lashes and eyes
//!   left out: their strands are fine detail by design), shrunk by a few pixels from their
//!   edges.

use cantus_core::inst::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::scene::bowed::Bowed;
use cantus_ui::scene::bowed_hold::{Hold, Holder};
use cantus_ui::scene::bowed_mesh::Role;
use cantus_ui::scene::{self, SceneBackend};
use cantus_ui::views::inst::arcus::probe;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::Pos2;

const PPP: f32 = 1.0;

/// A mask the size of the shot.
struct Mask {
    w: usize,
    h: usize,
    m: Vec<bool>,
}

impl Mask {
    fn new(w: usize, h: usize) -> Mask {
        Mask {
            w,
            h,
            m: vec![false; w * h],
        }
    }

    fn get(&self, x: isize, y: isize) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.w
            && (y as usize) < self.h
            && self.m[y as usize * self.w + x as usize]
    }

    /// A triangle (pixels), filled: every pixel whose centre it holds, and its corners'.
    fn tri(&mut self, p: [Pos2; 3]) {
        let (x0, x1) = (
            p.iter()
                .map(|q| q.x)
                .fold(f32::MAX, f32::min)
                .floor()
                .max(0.0) as usize,
            p.iter()
                .map(|q| q.x)
                .fold(f32::MIN, f32::max)
                .ceil()
                .max(0.0) as usize,
        );
        let (y0, y1) = (
            p.iter()
                .map(|q| q.y)
                .fold(f32::MAX, f32::min)
                .floor()
                .max(0.0) as usize,
            p.iter()
                .map(|q| q.y)
                .fold(f32::MIN, f32::max)
                .ceil()
                .max(0.0) as usize,
        );
        let edge =
            |a: Pos2, b: Pos2, c: Pos2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        let area = edge(p[0], p[1], p[2]);
        for y in y0..=y1.min(self.h.saturating_sub(1)) {
            for x in x0..=x1.min(self.w.saturating_sub(1)) {
                let c = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                let (e0, e1, e2) = (
                    edge(p[1], p[2], c),
                    edge(p[2], p[0], c),
                    edge(p[0], p[1], c),
                );
                let inside = if area >= 0.0 {
                    e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0
                } else {
                    e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0
                };
                if inside {
                    self.m[y * self.w + x] = true;
                }
            }
        }
        for q in p {
            if q.x >= 0.0 && q.y >= 0.0 && (q.x as usize) < self.w && (q.y as usize) < self.h {
                self.m[q.y as usize * self.w + q.x as usize] = true;
            }
        }
    }

    /// A thick segment (pixels).
    fn seg(&mut self, a: Pos2, b: Pos2, r: f32) {
        let d = b - a;
        let n = egui::vec2(-d.y, d.x).normalized() * r;
        self.tri([a - n, a + n, b + n]);
        self.tri([a - n, b + n, b - n]);
    }

    /// Grown (`k` > 0) or shrunk (`k` < 0) by |k| pixels (a square neighbourhood).
    fn grow(&self, k: isize) -> Mask {
        let mut out = Mask::new(self.w, self.h);
        let r = k.abs();
        for y in 0..self.h as isize {
            for x in 0..self.w as isize {
                let mut any = false;
                let mut all = true;
                for dy in -r..=r {
                    for dx in -r..=r {
                        let g = self.get(x + dx, y + dy);
                        any |= g;
                        all &= g;
                    }
                }
                out.m[y as usize * self.w + x as usize] = if k > 0 { any } else { all };
            }
        }
        out
    }
}

/// A stray: where, how many pixels, how far from its ring.
#[derive(Debug)]
struct Stray {
    x: usize,
    y: usize,
    n: usize,
    by: f32,
}

/// The luminance of the shot inside `r` (pixels).
fn luma(c: &cantus_ui::raster::Canvas) -> Vec<f32> {
    let mut out = vec![0.0; c.width * c.height];
    for y in 0..c.height {
        for x in 0..c.width {
            let p = c.pixel(x, y);
            out[y * c.width + x] =
                (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0;
        }
    }
    out
}

/// The strays in `region` (pixels) of the shot: groups of at most `most` pixels standing
/// out from the median round them (by over [`STAND`]), darker or lighter than every pixel
/// that touches them by over [`EDGE`], on surroundings whose spread is under [`SMOOTH`].
fn strays(
    l: &[f32],
    w: usize,
    h: usize,
    region: &dyn Fn(usize, usize) -> bool,
    most: usize,
) -> Vec<Stray> {
    let at = |x: isize, y: isize| {
        l[(y.clamp(0, h as isize - 1) as usize) * w + x.clamp(0, w as isize - 1) as usize]
    };
    let mut cand = vec![false; w * h];
    let mut win = [0.0f32; 25];
    for y in 0..h {
        for x in 0..w {
            if !region(x, y) {
                continue;
            }
            let mut k = 0;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    win[k] = at(x as isize + dx, y as isize + dy);
                    k += 1;
                }
            }
            win.sort_by(f32::total_cmp);
            cand[y * w + x] = (l[y * w + x] - win[12]).abs() > STAND;
        }
    }
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    for start in 0..w * h {
        if !cand[start] || seen[start] {
            continue;
        }
        let mut group = vec![start];
        seen[start] = true;
        let mut i = 0;
        while i < group.len() && group.len() <= most + 1 {
            let (x, y) = ((group[i] % w) as isize, (group[i] / w) as isize);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (xx, yy) = (x + dx, y + dy);
                    if xx < 0 || yy < 0 || xx >= w as isize || yy >= h as isize {
                        continue;
                    }
                    let j = yy as usize * w + xx as usize;
                    if cand[j] && !seen[j] {
                        seen[j] = true;
                        group.push(j);
                    }
                }
            }
            i += 1;
        }
        if group.len() > most {
            continue;
        }
        let v = group.iter().map(|&j| l[j]).sum::<f32>() / group.len() as f32;
        let mut ring = Vec::new();
        let mut wide = Vec::new();
        let (mut cx, mut cy) = (0usize, 0usize);
        for &j in &group {
            cx += j % w;
            cy += j / w;
        }
        let (cx, cy) = (cx / group.len(), cy / group.len());
        for &j in &group {
            let (x, y) = ((j % w) as isize, (j / w) as isize);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (xx, yy) = (x + dx, y + dy);
                    if xx < 0 || yy < 0 || xx >= w as isize || yy >= h as isize {
                        continue;
                    }
                    let k = yy as usize * w + xx as usize;
                    if !group.contains(&k) {
                        ring.push(l[k]);
                    }
                }
            }
        }
        for dy in -3..=3isize {
            for dx in -3..=3isize {
                let (xx, yy) = (cx as isize + dx, cy as isize + dy);
                let k = (yy.clamp(0, h as isize - 1) as usize) * w
                    + xx.clamp(0, w as isize - 1) as usize;
                if !group.contains(&k) {
                    wide.push(l[k]);
                }
            }
        }
        let lo = ring.iter().copied().fold(f32::MAX, f32::min);
        let hi = ring.iter().copied().fold(f32::MIN, f32::max);
        let by = if v < lo - EDGE {
            lo - v
        } else if v > hi + EDGE {
            v - hi
        } else {
            continue;
        };
        let mean = wide.iter().sum::<f32>() / wide.len().max(1) as f32;
        let sd = (wide.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>()
            / wide.len().max(1) as f32)
            .sqrt();
        if sd > SMOOTH {
            continue;
        }
        out.push(Stray {
            x: cx,
            y: cy,
            n: group.len(),
            by,
        });
    }
    out
}

/// How far from the median round it a pixel stands to be looked at, how far past every
/// pixel touching it a group stands to be a stray, and how smooth its surroundings are.
const STAND: f32 = 0.12;
const EDGE: f32 = 0.07;
const SMOOTH: f32 = 0.05;

#[allow(clippy::too_many_arguments)]
fn shot(
    hold: Hold,
    b: Bowed,
    inst: f32,
    dark: bool,
    be: SceneBackend,
    earlier: bool,
    save: Option<std::path::PathBuf>,
) -> (cantus_ui::raster::Canvas, Mask, Mask, egui::Rect) {
    scene::backend::set_default(be);
    cantus_ui::scene::bowed_draw::set_real_view(earlier.then_some((4.0, 7.0)));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::default_theme()
    } else {
        cantus_ui::theme::default_light_theme()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    let h = Holder::of(hold);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings.arcus_holder = h;
    app.settings.figure_quality = cantus_ui::scene::figure::Quality::High;
    app.demo_editor("arcus");
    let t = app.inst.open.expect("an Arcus editor");
    app.set_inst_value(t, "inst", inst, false);
    views::inst::arcus::demo_feed(&mut app);
    let size = views::inst::editor_size(InstrumentKind::BowedString);
    probe::record(true);
    let canvas = cantus_ui::raster::shoot(&ctx, size, PPP, 8, save.as_deref(), |ui| {
        views::inst::window_in(&mut app, ui)
    });
    let arms = probe::arms();
    let bow = probe::bow();
    probe::record(false);
    let sc = &app.inst.arcus_scenes[&t.0].0;
    let built = sc.built().expect("a picture");
    let rect = sc.rect().expect("painted");
    let k = egui::vec2(rect.width() / built.size.x, rect.height() / built.size.y);
    let to_px = |w: cantus_ui::scene::geo::V3| {
        let q = built.cam.p(w);
        let p = rect.min + egui::vec2(q.x * k.x, q.y * k.y);
        Pos2::new(p.x * PPP, p.y * PPP)
    };
    let (w, hh) = (canvas.width, canvas.height);
    let mut subject = Mask::new(w, hh);
    let mut figure = Mask::new(w, hh);
    // Hair, brows, lashes, eyes and jewels: fine detail by design (their strands, lashes
    // and glints), and the skin under them.
    let mut fine = Mask::new(w, hh);
    let mesh = cantus_ui::scene::bowed_mesh::mesh_for(b, h);
    for (_, p) in &mesh.patches {
        for c in p.cells() {
            let q = c.map(|i| to_px(p.pts[i]));
            subject.tri([q[0], q[1], q[2]]);
            subject.tri([q[0], q[2], q[3]]);
        }
    }
    for (_, poly) in &mesh.polys {
        let q: Vec<Pos2> = poly.pts.iter().map(|&p| to_px(p)).collect();
        for i in 1..q.len().saturating_sub(1) {
            subject.tri([q[0], q[i], q[i + 1]]);
        }
    }
    for (_, r) in &mesh.rods {
        let (a, e) = (to_px(r.a), to_px(r.b));
        let cm = (to_px(r.a + cantus_ui::scene::geo::v(1.0, 0.0, 0.0)) - a).length();
        subject.seg(a, e, (r.r * cm).max(1.0) + 1.0);
    }
    for (role, t) in &mesh.tris {
        let skin_or_cloth = matches!(
            role,
            Role::Skin(_) | Role::Velvet | Role::Wool | Role::Shirt | Role::Tie
        );
        let detail = matches!(
            role,
            Role::Card(_)
                | Role::Scalp(_)
                | Role::Shell(_)
                | Role::Hair
                | Role::HairFair
                | Role::Eye
                | Role::Iris
                | Role::Lash
                | Role::Brass
        );
        for tri in &t.idx {
            let q = tri.map(|i| to_px(t.pts[i as usize]));
            subject.tri(q);
            if skin_or_cloth {
                figure.tri(q);
            }
            if detail {
                fine.tri(q);
            }
        }
    }
    for arm in &arms {
        for t in &arm.tris {
            let q = t.map(|i| {
                let p = arm.screen[i as usize];
                Pos2::new(p.x * PPP, p.y * PPP)
            });
            subject.tri(q);
            figure.tri(q);
        }
    }
    if let Some((f, tip, cm)) = bow {
        subject.seg(
            Pos2::new(f.x * PPP, f.y * PPP),
            Pos2::new(tip.x * PPP, tip.y * PPP),
            2.2 * cm * PPP,
        );
    }
    let pic = egui::Rect::from_min_max(
        Pos2::new(rect.min.x * PPP, rect.min.y * PPP),
        Pos2::new(rect.max.x * PPP, rect.max.y * PPP),
    );
    let fine = fine.grow(4);
    for (m, f) in figure.m.iter_mut().zip(&fine.m) {
        *m &= !*f;
    }
    (canvas, subject, figure, pic)
}

#[test]
fn no_stray_pixels_round_or_on_the_violinists() {
    views::inst::chorda::scene::hybrid::register();
    scene::set_blocking(true);
    let mut bad = Vec::new();
    let mut looked = 0;
    for hold in [Hold::Woman, Hold::Man] {
        for (b, inst) in [(Bowed::Violin, 2.0f32), (Bowed::Viola, 3.0)] {
            for earlier in [false, true] {
                for dark in [true, false] {
                    for be in [SceneBackend::Vector, SceneBackend::PerPixel] {
                        // (`CANTUS_SPECKS_DIR=<folder>`: each shot saved there, to look at what
                        // the gate found.)
                        let save = std::env::var_os("CANTUS_SPECKS_DIR").map(|d| {
                            std::path::PathBuf::from(d).join(format!(
                                "{hold:?}-{b:?}-{}-{}-{be:?}.png",
                                if earlier { "earlier" } else { "camera" },
                                if dark { "night" } else { "paper" }
                            ))
                        });
                        let (canvas, subject, figure, pic) =
                            shot(hold, b, inst, dark, be, earlier, save);
                        let (w, h) = (canvas.width, canvas.height);
                        let l = luma(&canvas);
                        let near = subject.grow(3);
                        let deep = figure.grow(-3);
                        // The picture, short of its caption's band at the bottom.
                        let inside_pic = |x: usize, y: usize| {
                            let (x, y) = (x as f32, y as f32);
                            x > pic.min.x + 3.0
                                && x < pic.max.x - 3.0
                                && y > pic.min.y + 3.0
                                && y < pic.min.y + pic.height() * 0.72
                        };
                        let room = |x: usize, y: usize| inside_pic(x, y) && !near.m[y * w + x];
                        let on = |x: usize, y: usize| inside_pic(x, y) && deep.m[y * w + x];
                        let tag = format!(
                            "{hold:?} {b:?} {} {} {be:?}",
                            if earlier { "earlier camera" } else { "camera" },
                            if dark { "Night" } else { "Paper" }
                        );
                        for s in strays(&l, w, h, &room, 30) {
                            bad.push(format!(
                                "{tag}: in the room at ({}, {}), {} px, {:.2}",
                                s.x, s.y, s.n, s.by
                            ));
                        }
                        for s in strays(&l, w, h, &on, 3) {
                            bad.push(format!(
                                "{tag}: on the figure at ({}, {}), {} px, {:.2}",
                                s.x, s.y, s.n, s.by
                            ));
                        }
                        looked += 1;
                    }
                }
            }
        }
    }
    cantus_ui::scene::bowed_draw::set_real_view(None);
    scene::backend::set_default(SceneBackend::Vector);
    assert_eq!(looked, 32);
    assert!(bad.is_empty(), "{} strays:\n{}", bad.len(), bad.join("\n"));
}

/// r42:arcus-polish4 — no skin through her bow sleeve (the coordinator: a skin-coloured
/// diamond under the bow forearm read as the arm through the sleeve — it was her dress's V
/// neckline's point showing under the arm, now higher, `build_figures.py::V_DEPTH`): her
/// bow arm drawn through the app with the bow at the frog, its middle and its tip, both
/// looks; inside the live sleeve's outline, drawn in by 2 px (not its hem, nor the hand, nor
/// the bow over it), no pixel may be skin-coloured — and none in a 12 px band under the
/// forearm's sleeve, where the neckline's point showed.
/// The sleeve test shoots at 200 %: at 100 % the V's point under the arm is a few pixels.
/// r42:arcus-polish5 — and with the bow where a figure starts (0.3, the editor's first
/// picture) and either side of it: there the old neckline (`V_DEPTH` 15 cm, before
/// 982b1dbbc) showed its point under the forearm, while at the frog, the middle and the tip
/// alone the arm covered it or lay far from it — the test passed the old figure. Run with
/// `CANTUS_FIGURE_DIR` holding the old `woman.fig`, it fails. `CANTUS_SLEEVE_DIR=<folder>`
/// saves each shot. (And each shot a new scene and three frames: a later shot of the same
/// scene drew the live arms over an empty picture — a run of frames keeps only the textures
/// it uploaded, and the baked body is a texture uploaded once — so no neckline was ever
/// under the sleeve to find.)
const SLEEVE_PPP: f32 = 2.0;
const SLEEVE_ALONG: [f32; 6] = [0.05, 0.2, 0.3, 0.4, 0.5, 0.92];
/// r42:arcus-polish6 — only islands count (the coordinator: the bug was a skin diamond that
/// read as a hole in the sleeve, an isolated island framed by black cloth; her neckline seen
/// under the forearm, continuous with her chest's visible skin, is the dress's design): a
/// patch of skin under the forearm that is not the picture's chest region — its largest
/// patch of skin, her face, neck and chest — and that dark cloth frames. (Pass 5 counted a
/// patch under 2500 px at 100 %.)
/// r42:arcus-polish6 — a patch counts only when at least this share of the pixels round it
/// (the hand's and the bow's left out) are dark cloth, every channel under `SLEEVE_DARK`.
const SLEEVE_RING: f32 = 0.6;
const SLEEVE_DARK: u8 = 70;

#[test]
fn no_skin_shows_through_or_under_her_bow_sleeve() {
    views::inst::chorda::scene::hybrid::register();
    scene::set_blocking(true);
    cantus_ui::scene::bowed_draw::set_real_view(None);
    let mut bad = Vec::new();
    for (b, inst) in [(Bowed::Violin, 2.0f32), (Bowed::Viola, 3.0)] {
        for be in [SceneBackend::Vector, SceneBackend::PerPixel] {
            scene::backend::set_default(be);
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            app.settings.arcus_holder = Holder::of(Hold::Woman);
            app.settings.figure_quality = cantus_ui::scene::figure::Quality::High;
            app.demo_editor("arcus");
            let t = app.inst.open.expect("an Arcus editor");
            app.set_inst_value(t, "inst", inst, false);
            views::inst::arcus::demo_feed(&mut app);
            let size = views::inst::editor_size(InstrumentKind::BowedString);
            let _ = cantus_ui::raster::shoot(&ctx, size, SLEEVE_PPP, 2, None, |ui| {
                views::inst::window_in(&mut app, ui)
            });
            for along in SLEEVE_ALONG {
                // (A new scene each time, so its picture is drawn and uploaded in this run.)
                app.inst
                    .arcus_scenes
                    .insert(t.0, (Default::default(), along, 0.0));
                probe::record(true);
                let save = std::env::var_os("CANTUS_SLEEVE_DIR").map(|d| {
                    std::path::PathBuf::from(d).join(format!("sleeve-{b:?}-{be:?}-{along}.png"))
                });
                let canvas =
                    cantus_ui::raster::shoot(&ctx, size, SLEEVE_PPP, 3, save.as_deref(), |ui| {
                        views::inst::window_in(&mut app, ui)
                    });
                let arms = probe::arms();
                let bow = probe::bow();
                probe::record(false);
                let (w, h) = (canvas.width, canvas.height);
                let (mut sleeve, mut skin, mut under) =
                    (Mask::new(w, h), Mask::new(w, h), Mask::new(w, h));
                for arm in arms.iter().filter(|a| a.side == 1) {
                    // (r42:arcus-polish5: the band under the forearm's sleeve only — under
                    // the upper arm, at the shoulder, lies her neckline's own skin.)
                    use cantus_ui::scene::bowed_human as hu;
                    let o = hu::side(arm.side);
                    let j = |b: usize| arm.joints.iter().find(|x| x.0 == o + b).map(|x| x.2);
                    let fore = |tr: &[u16; 3]| match (j(hu::FORE1), j(hu::HAND)) {
                        (Some(e), Some(w)) => {
                            let c = tr
                                .iter()
                                .fold(cantus_ui::scene::geo::V3::default(), |a, &i| {
                                    a + arm.pts[i as usize]
                                })
                                * (1.0 / 3.0);
                            let ax = w - e;
                            let t = (c - e).dot(ax) / ax.dot(ax).max(1e-6);
                            (0.15..=1.05).contains(&t)
                        }
                        _ => true,
                    };
                    for tr in &arm.tris {
                        let q = tr.map(|i| {
                            let p = arm.screen[i as usize];
                            Pos2::new(p.x * SLEEVE_PPP, p.y * SLEEVE_PPP)
                        });
                        if tr.iter().all(|&i| !arm.skin[i as usize]) {
                            sleeve.tri(q);
                            // The band under it: the triangle moved 12 px (at 100 %) down.
                            if fore(tr) {
                                let d = egui::vec2(0.0, 12.0 * SLEEVE_PPP);
                                under.tri([q[0] + d, q[1] + d, q[2] + d]);
                            }
                        } else {
                            skin.tri(q);
                        }
                    }
                }
                if let Some((f, tip, cm)) = bow {
                    skin.seg(
                        Pos2::new(f.x * SLEEVE_PPP, f.y * SLEEVE_PPP),
                        Pos2::new(tip.x * SLEEVE_PPP, tip.y * SLEEVE_PPP),
                        2.2 * cm * SLEEVE_PPP,
                    );
                }
                // Drawn in by 2 px: its outline's own antialiasing is not a hole.
                let mut outside = Mask::new(w, h);
                for (o, s) in outside.m.iter_mut().zip(&sleeve.m) {
                    *o = !*s;
                }
                let outside = outside.grow(2);
                let skin = skin.grow(3);
                let (mut inside_n, mut under_n) = (0, 0);
                // (r42:arcus-polish6: where the counted pixels lie, for the report.)
                let mut under_box = (usize::MAX, usize::MAX, 0usize, 0usize);
                // (Skin in the light and in shade — the V's point under her arm was shaded
                // skin, about (83, 67, 61) —; not the velvet's grey sheen, the varnish's
                // orange or the bow's red-brown.)
                let skinny = |x: usize, y: usize| {
                    let p = canvas.pixel(x, y);
                    let (r, g, bl) = (p[0] as f32, p[1] as f32, p[2] as f32);
                    r > 60.0 && (0.55..0.92).contains(&(g / r)) && (0.4..0.85).contains(&(bl / r))
                };
                // r42:arcus-polish5 — under the forearm, only skin cut off by it counts: a
                // small patch (the old V's point poking out below the sleeve), not the
                // neckline that runs on up past the arm (at the frog the forearm crosses her
                // chest over the whole V, which is her own skin, not the arm's).
                let mut open = vec![false; w * h];
                for y in 0..h {
                    for x in 0..w {
                        let k = y * w + x;
                        open[k] = !skin.m[k] && !sleeve.m[k] && skinny(x, y);
                    }
                }
                let mut part = vec![u32::MAX; w * h];
                let mut sizes: Vec<usize> = Vec::new();
                for start in 0..w * h {
                    if !open[start] || part[start] != u32::MAX {
                        continue;
                    }
                    let id = sizes.len() as u32;
                    let mut stack = vec![start];
                    part[start] = id;
                    let mut n = 0;
                    while let Some(k) = stack.pop() {
                        n += 1;
                        let (x, y) = (k % w, k / w);
                        for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                            let (xx, yy) = (x as isize + dx, y as isize + dy);
                            if xx < 0 || yy < 0 || xx >= w as isize || yy >= h as isize {
                                continue;
                            }
                            let j = yy as usize * w + xx as usize;
                            if open[j] && part[j] == u32::MAX {
                                part[j] = id;
                                stack.push(j);
                            }
                        }
                    }
                    sizes.push(n);
                }
                // r42:arcus-polish6 — and only a patch in the dark cloth round it (the forearm's
                // sleeve, her dress): a light speck on the varnish under the forearm (its
                // highlight, its edge's lit line — skin-coloured, orange round it) is the
                // instrument's, not her skin cut off by the arm.
                let mut ring = vec![(0u32, 0u32); sizes.len()];
                for y in 1..h - 1 {
                    for x in 1..w - 1 {
                        let k = y * w + x;
                        if part[k] == u32::MAX {
                            continue;
                        }
                        for j in [k - 1, k + 1, k - w, k + w] {
                            if part[j] == part[k] || skin.m[j] {
                                continue;
                            }
                            let q = canvas.pixel(j % w, j / w);
                            let r = &mut ring[part[k] as usize];
                            r.0 += 1;
                            r.1 += (q[0].max(q[1]).max(q[2]) < SLEEVE_DARK) as u32;
                        }
                    }
                }
                let in_cloth = |id: usize| ring[id].1 as f32 >= ring[id].0 as f32 * SLEEVE_RING;
                // The chest region: the largest patch.
                let chest = (0..sizes.len()).max_by_key(|&i| sizes[i]);
                for y in 0..h {
                    for x in 0..w {
                        let k = y * w + x;
                        if skin.m[k] || !skinny(x, y) {
                            continue;
                        }
                        if !outside.m[k] {
                            inside_n += 1;
                        } else if under.m[k]
                            && !sleeve.m[k]
                            && part[k] != u32::MAX
                            && Some(part[k] as usize) != chest
                            && in_cloth(part[k] as usize)
                        {
                            under_n += 1;
                            under_box = (
                                under_box.0.min(x),
                                under_box.1.min(y),
                                under_box.2.max(x),
                                under_box.3.max(y),
                            );
                        }
                    }
                }
                let area = sleeve.m.iter().filter(|&&x| x).count();
                eprintln!(
                    "{b:?} {be:?} bow at {along}: sleeve {area} px, {inside_n} skin px inside, \
                     {under_n} under{}",
                    if under_n > 0 {
                        format!(
                            " in ({}, {})-({}, {})",
                            under_box.0, under_box.1, under_box.2, under_box.3
                        )
                    } else {
                        String::new()
                    }
                );
                if area < (400.0 * SLEEVE_PPP * SLEEVE_PPP) as usize {
                    bad.push(format!(
                        "{b:?} {be:?} bow at {along}: no bow sleeve drawn ({area} px)"
                    ));
                }
                if inside_n > 0 || under_n > 2 {
                    bad.push(format!(
                        "{b:?} {be:?} bow at {along}: {inside_n} skin px inside the sleeve, \
                         {under_n} under it"
                    ));
                }
            }
        }
    }
    scene::backend::set_default(SceneBackend::Vector);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
