//! r42:arcus-hands — the realistic violinists' hands against a real hand's proportions (the
//! owner, 2026-11-25: "the musicians can still look much, much better… especially the
//! fingers of the hand"; the fingers read "short and stubby"). Measured on the figures the
//! app draws (`bowed_human::figure`: after its load-time edits), at rest:
//!
//! - each finger's three segments — its base joint to its middle joint, the middle joint to
//!   the last, the last joint to the skin's tip — against the first (a real finger's bones
//!   are about 1 : 0.63 : 0.45; with the pad past the last bone the last segment reads
//!   0.5–0.7 of the first);
//! - the fingers against each other (the middle the longest, the index and the ring about
//!   0.9–0.97 of it, the little finger's tip near the ring finger's last joint) and the
//!   middle finger against the back of the hand (photographs of violinists: the fingers as
//!   long as the back or longer);
//! - each nail's width at its middle against its finger's there (a real nail 0.6–0.7).
//!
//! References: Kapandji, *The Physiology of the Joints* I (the hand); Buryanov & Kotiuk
//! 2010, "Proportions of hand segments" (Int. J. Morphol. 28:755).

use cantus_core::inst::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::scene::bowed_hold::{Hold, Holder};
use cantus_ui::scene::bowed_human::{self as hu, Stuff, Who};
use cantus_ui::scene::{self, SceneBackend};
use cantus_ui::views::inst::arcus::probe;
use cantus_ui::{views, welcome_project, CantusApp};

/// A finger's segments (cm): base to middle joint, middle to last joint, last joint to tip.
fn segments(fig: &hu::Figure, f: usize) -> [f32; 3] {
    let o = hu::side(0);
    let b = |k: usize| &fig.bones[o + hu::finger(f, k)];
    let (mcp, pip, dip) = (b(0).head, b(1).head, b(2).head);
    let ax = (b(2).tail - dip).norm();
    // The skin's tip: the farthest of the last bone's own vertices along it.
    let last = (o + hu::finger(f, 2)) as u8;
    let tip = fig
        .pos
        .iter()
        .zip(&fig.bw)
        .filter(|(_, w)| w.iter().any(|&(bb, x)| bb == last && x > 0.5))
        .map(|(p, _)| (*p - dip).dot(ax))
        .fold(0.0f32, f32::max);
    [(pip - mcp).len(), (dip - pip).len(), tip]
}

/// How far along the hand (wrist to the middle finger's base) a finger's tip and its last
/// joint reach, at rest with the finger straightened along its own bones.
fn reach(fig: &hu::Figure, f: usize) -> (f32, f32) {
    let o = hu::side(0);
    let wr = fig.bones[o + hu::HAND].head;
    let ax = (fig.bones[o + hu::finger(2, 0)].head - wr).norm();
    let s = segments(fig, f);
    let base = (fig.bones[o + hu::finger(f, 0)].head - wr).dot(ax);
    (base + s[0] + s[1] + s[2], base + s[0] + s[1])
}

#[test]
fn the_fingers_have_a_real_hands_proportions() {
    let mut bad = Vec::new();
    let mut report = String::new();
    for who in [Who::Woman, Who::Man] {
        let fig = hu::figure(who);
        let o = hu::side(0);
        let len = |f: usize| segments(fig, f).iter().sum::<f32>();
        for f in 1..5 {
            let s = segments(fig, f);
            let (m, d) = (s[1] / s[0], s[2] / s[0]);
            report += &format!(
                "{who:?} finger {f}: {:.2} {:.2} {:.2} cm, 1 : {m:.2} : {d:.2}\n",
                s[0], s[1], s[2]
            );
            if !(0.52..=0.72).contains(&m) {
                bad.push(format!("{who:?} finger {f}: the middle phalanx {m:.2} of the first"));
            }
            if !(0.45..=0.75).contains(&d) {
                bad.push(format!("{who:?} finger {f}: the last segment {d:.2} of the first"));
            }
        }
        let mid = len(2);
        for (f, lo, hi) in [(1, 0.88, 0.98), (3, 0.90, 0.99)] {
            let r = len(f) / mid;
            report += &format!("{who:?} finger {f} over the middle: {r:.2}\n");
            if !(lo..=hi).contains(&r) {
                bad.push(format!("{who:?} finger {f}: {r:.2} of the middle finger, not {lo}-{hi}"));
            }
        }
        let (little_tip, _) = reach(fig, 4);
        let (_, ring_dip) = reach(fig, 3);
        report += &format!(
            "{who:?} the little finger's tip {:+.2} cm from the ring finger's last joint\n",
            little_tip - ring_dip
        );
        if (little_tip - ring_dip).abs() > 0.6 {
            bad.push(format!(
                "{who:?}: the little finger's tip {:+.2} cm from the ring's last joint",
                little_tip - ring_dip
            ));
        }
        let back = (fig.bones[o + hu::finger(2, 0)].head - fig.bones[o + hu::HAND].head).len();
        let r = mid / back;
        report += &format!("{who:?} the middle finger over the back of the hand: {r:.2}\n");
        if !(1.05..=1.35).contains(&r) {
            bad.push(format!("{who:?}: the middle finger {r:.2} of the back of the hand"));
        }
    }
    eprintln!("{report}");
    assert!(bad.is_empty(), "{}\n{report}", bad.join("\n"));
}

#[test]
fn the_nails_are_a_real_nails_share_of_the_finger() {
    let mut bad = Vec::new();
    let mut report = String::new();
    for who in [Who::Woman, Who::Man] {
        let fig = hu::figure(who);
        let o = hu::side(0);
        let mut nail = vec![false; fig.pos.len()];
        for (t, st) in fig.tris.iter().zip(&fig.stuff) {
            if *st == Stuff::Nail {
                for &i in t {
                    nail[i as usize] = true;
                }
            }
        }
        // The hand's back at rest: across the fingers is square to it and to each finger.
        let kn = |f: usize| fig.bones[o + hu::finger(f, 0)].head;
        let wr = fig.bones[o + hu::HAND].head;
        let back = (kn(1) - kn(4)).cross((kn(2) + kn(3)) * 0.5 - wr).norm();
        for f in 1..5 {
            let last = (o + hu::finger(f, 2)) as u8;
            let bn = &fig.bones[o + hu::finger(f, 2)];
            let ax = (bn.tail - bn.head).norm();
            let side = ax.cross(back).norm();
            let mine: Vec<usize> = (0..fig.pos.len())
                .filter(|&i| fig.bw[i].iter().any(|&(b, x)| b == last && x > 0.5))
                .collect();
            let along = |i: usize| (fig.pos[i] - bn.head).dot(ax);
            let (lo, hi) = mine
                .iter()
                .filter(|&&i| nail[i])
                .fold((f32::MAX, f32::MIN), |(l, h), &i| (l.min(along(i)), h.max(along(i))));
            if lo > hi {
                bad.push(format!("{who:?} finger {f}: no nail"));
                continue;
            }
            let c = (lo + hi) * 0.5;
            let width = |pick: &dyn Fn(usize) -> bool| {
                let xs: Vec<f32> = mine
                    .iter()
                    .copied()
                    .filter(|&i| pick(i) && (along(i) - c).abs() < 0.18)
                    .map(|i| (fig.pos[i] - bn.head).dot(side))
                    .collect();
                xs.iter().copied().fold(f32::MIN, f32::max) - xs.iter().copied().fold(f32::MAX, f32::min)
            };
            let (wn, wf) = (width(&|i| nail[i]), width(&|_| true));
            let share = wn / wf.max(1e-3);
            report += &format!("{who:?} finger {f}: the nail {wn:.2} of {wf:.2} cm = {share:.2}\n");
            if !(0.55..=0.8).contains(&share) {
                bad.push(format!("{who:?} finger {f}: the nail {share:.2} of the finger's width"));
            }
        }
    }
    eprintln!("{report}");
    assert!(bad.is_empty(), "{}\n{report}", bad.join("\n"));
}

/// The Arcus editor shot at `ppp` with `hold` on the violin (B4 on the A string, the sheet's
/// first position), Night, Photographic, High; the shot and the arms as drawn.
fn shot(hold: Hold, ppp: f32) -> (cantus_ui::raster::Canvas, Vec<probe::Arm>) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings.arcus_holder = Holder::of(hold);
    app.settings.figure_quality = cantus_ui::scene::figure::Quality::High;
    app.demo_editor("arcus");
    let t = app.inst.open.expect("an Arcus editor");
    app.set_inst_value(t, "inst", 2.0, false);
    views::inst::arcus::demo_feed(&mut app);
    if let Some(tele) = app.inst.injected.as_mut() {
        tele[cantus_dsp::arcus::tel::STRING] = 2.0;
        tele[cantus_dsp::arcus::tel::KEY] = 71.0;
        tele[cantus_dsp::arcus::tel::VIBRATO] = 0.0;
    }
    let size = views::inst::editor_size(InstrumentKind::BowedString);
    probe::record(true);
    let save = std::env::var_os("CANTUS_HAND_SPECKS_DIR").map(|d| {
        std::path::PathBuf::from(d).join(format!("hands-{hold:?}-{}.png", (ppp * 100.0) as u32))
    });
    let c = cantus_ui::raster::shoot(&ctx, size, ppp, 8, save.as_deref(), |ui| {
        views::inst::window_in(&mut app, ui)
    });
    let arms = probe::arms();
    probe::record(false);
    (c, arms)
}

/// A hand's skin as a mask of the shot's pixels (`ppp`), shrunk by `inset` pixels.
fn hand_mask(a: &probe::Arm, w: usize, h: usize, ppp: f32, inset: usize) -> Vec<bool> {
    let o = hu::side(a.side);
    let handy = |i: u16| {
        let b = a.bone[i as usize] as usize;
        a.skin[i as usize] && (o + hu::HAND..=o + hu::finger(4, 2)).contains(&b)
    };
    let mut m = vec![false; w * h];
    for t in &a.tris {
        if !t.iter().all(|&i| handy(i)) {
            continue;
        }
        let p = t.map(|i| a.screen[i as usize] * ppp);
        let xs = p.iter().map(|q| q.x);
        let ys = p.iter().map(|q| q.y);
        let x0 = xs.clone().fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let x1 = (xs.fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(w);
        let y0 = ys.clone().fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let y1 = (ys.fold(f32::MIN, f32::max).ceil().max(0.0) as usize).min(h);
        let e = |a: egui::Pos2, b: egui::Pos2, x: f32, y: f32| {
            (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
        };
        for y in y0..y1 {
            for x in x0..x1 {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let s = [e(p[0], p[1], fx, fy), e(p[1], p[2], fx, fy), e(p[2], p[0], fx, fy)];
                if s.iter().all(|&v| v >= 0.0) || s.iter().all(|&v| v <= 0.0) {
                    m[y * w + x] = true;
                }
            }
        }
    }
    for _ in 0..inset {
        let prev = m.clone();
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                if prev[y * w + x]
                    && (!prev[y * w + x - 1]
                        || !prev[y * w + x + 1]
                        || !prev[(y - 1) * w + x]
                        || !prev[(y + 1) * w + x])
                {
                    m[y * w + x] = false;
                }
            }
        }
    }
    m
}

/// Dark specks in `region`: groups of at most `most` pixels darker than the median round
/// them by over [`SPECK`] and than every pixel touching them (the neck's stamp cutting a
/// finger along its triangles, a crack between patches, a dark fleck): (x, y, size, by).
fn dark_specks(
    c: &cantus_ui::raster::Canvas,
    region: &[bool],
    most: usize,
) -> Vec<(usize, usize, usize, f32)> {
    let (w, h) = (c.width, c.height);
    let l: Vec<f32> = (0..w * h)
        .map(|i| {
            let p = c.pixel(i % w, i / w);
            (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0
        })
        .collect();
    let mut cand = vec![false; w * h];
    let mut win = [0.0f32; 25];
    for y in 2..h.saturating_sub(2) {
        for x in 2..w.saturating_sub(2) {
            if !region[y * w + x] {
                continue;
            }
            let mut k = 0;
            for dy in 0..5 {
                for dx in 0..5 {
                    win[k] = l[(y + dy - 2) * w + x + dx - 2];
                    k += 1;
                }
            }
            win.sort_by(f32::total_cmp);
            cand[y * w + x] = win[12] - l[y * w + x] > SPECK;
        }
    }
    let near = |j: usize| {
        let (x, y) = (j % w, j / w);
        [
            (x + 1, y),
            (x.wrapping_sub(1), y),
            (x, y + 1),
            (x, y.wrapping_sub(1)),
        ]
        .into_iter()
        .filter(move |&(xx, yy)| xx < w && yy < h)
        .map(move |(xx, yy)| yy * w + xx)
    };
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    for start in 0..w * h {
        if !cand[start] || seen[start] {
            continue;
        }
        let mut group = vec![start];
        seen[start] = true;
        let mut i = 0;
        while i < group.len() {
            for j in near(group[i]) {
                if cand[j] && !seen[j] {
                    seen[j] = true;
                    group.push(j);
                }
            }
            i += 1;
        }
        if group.len() > most {
            continue;
        }
        let v = group.iter().map(|&j| l[j]).sum::<f32>() / group.len() as f32;
        let ring = group
            .iter()
            .flat_map(|&j| near(j))
            .filter(|j| !group.contains(j))
            .map(|j| l[j])
            .fold(f32::MAX, f32::min);
        if ring - v > SPECK * 0.6 {
            out.push((group[0] % w, group[0] / w, group.len(), ring - v));
        }
    }
    out
}

/// How much darker than its surroundings a speck stands (luminance).
const SPECK: f32 = 0.12;

#[test]
fn no_dark_specks_on_the_hands() {
    views::inst::chorda::scene::hybrid::register();
    scene::set_blocking(true);
    scene::backend::set_default(SceneBackend::PerPixel);
    let mut bad = Vec::new();
    let mut report = String::new();
    for hold in [Hold::Woman, Hold::Man] {
        for ppp in [1.0f32, 2.0] {
            let (c, arms) = shot(hold, ppp);
            let pct = (ppp * 100.0) as u32;
            for a in &arms {
                let m = hand_mask(a, c.width, c.height, ppp, 2);
                let n = m.iter().filter(|&&x| x).count();
                let sp = dark_specks(&c, &m, (12.0 * ppp * ppp) as usize);
                report += &format!(
                    "{hold:?} {pct}% hand {}: {n} px, {} specks {:?}\n",
                    a.side,
                    sp.len(),
                    &sp[..sp.len().min(6)]
                );
                if !sp.is_empty() {
                    bad.push(format!("{hold:?} {pct}% hand {}: {} dark specks", a.side, sp.len()));
                }
            }
        }
    }
    scene::backend::set_default(SceneBackend::Vector);
    eprintln!("{report}");
    assert!(bad.is_empty(), "{}\n{report}", bad.join("\n"));
}
