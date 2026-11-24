//! What the instrument plays, drawn live over its picture (a child of
//! `views::inst::arcus::draw`; r42:arcus-look). The cello or the bass and its room are the
//! scene engine's picture (`scene::bowed*`), baked and cached; over it, every frame and
//! cheaply, from the same camera and the instrument's own geometry (`scene::bowed::rig`):
//! **the sounding string** lit and swinging as loud as it sounds between the bridge and
//! the finger; **the stopping finger** where the note stops the string, rocking with the
//! vibrato; **the bow** across that string at its real contact point (β of the stopped
//! length from the bridge), tilted over the bridge's arch to reach it alone, its hair
//! pressed into the string and flattened (and a little darker) as hard as it presses, the
//! stick's camber, the frog with its pearl eye, sliding along its length as it moves —
//! a blur behind it as fast — or **pizzicato's finger** pulling the string, or
//! **tremolo's** short strokes.

use super::Now;
use crate::kit::appearance::rgb;
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::scene::bowed::live as pal;
use crate::kit::{Brush, Fill, Style};
use crate::scene::bowed::{Bowed, Rig};
use crate::scene::geo::{Camera, V3};
use cantus_dsp::arcus::{ArcusSettings, Technique};
use egui::{Color32, Pos2, Rect, Vec2};

/// Where the picture's camera puts world points (cm) on the screen.
#[derive(Copy, Clone)]
pub struct View {
    pub cam: Camera,
    /// The picture's corner and its scale from the build's points to the screen's.
    pub origin: Pos2,
    pub k: Vec2,
}

impl View {
    pub fn p(&self, w: V3) -> Pos2 {
        let q = self.cam.p(w);
        self.origin + Vec2::new(q.x * self.k.x, q.y * self.k.y)
    }

    /// How many points a centimetre at `w` spans across the screen.
    pub fn scale(&self, w: V3) -> f32 {
        self.cam.s * self.k.x / self.cam.depth(w).max(1.0)
    }
}

/// What a frame draws: the instrument, what it plays, the bow's place along its hair (0
/// the frog, 1 the tip), the time (vibrato, tremolo), the theme's lit colour for the
/// sounding string.
pub struct Play<'a> {
    pub b: Bowed,
    pub rig: &'a Rig,
    pub fs: &'a ArcusSettings,
    pub n: &'a Now,
    pub travel: f32,
    pub time: f64,
    pub vibrato: f32,
    pub lit: Color32,
    /// What holds the instrument (its figure's right arm is drawn here, to the frog) and
    /// whether the left hand's fingertip shows (this machine's preferences). // r42:arcus-upper
    pub holder: crate::scene::bowed_hold::Holder, // r42:arcus-upper
    pub stop: crate::scene::bowed_hold::StopShow, // r42:arcus-upper
    /// The figure's left hand's position (semitones, glided) and the theme's accent (the
    /// stop's marker). // r42:arcus-upper
    pub hand: f32, // r42:arcus-upper
    pub accent: Color32,                          // r42:arcus-upper
}

fn c(s: [u8; 3]) -> Color32 {
    rgb(s)
}

/// Where the note stops its string (its share of the open length from the bridge).
pub fn stop(fs: &ArcusSettings, n: &Now) -> f32 {
    let open = fs.instrument.spec().strings[n.string.min(3)].open_hz();
    let f = 440.0 * ((n.key as f64 - 69.0) / 12.0).exp2();
    (open / f).clamp(0.08, 1.0) as f32
}

/// Paint what `pl` plays through `v` (clipped by the painter to the picture).
pub fn live(b: &mut Brush, v: &View, pl: &Play, s: &Style) {
    let rig = pl.rig;
    let i = pl.n.string.min(3);
    let [_, bridge, nut] = rig.strings[i];
    let along = nut - bridge;
    let st = stop(pl.fs, pl.n);
    let d = pl.b.dims();
    // The player's left, where the left hand comes from: +x behind a cello, -x under a chin.
    let ex = if d.held { -rig.pose.ex } else { rig.pose.ex }; // r42:arcus-upper
    let ez = rig.pose.ez;
    let k = d.body / 75.7;
    // The finger rocks along the string with the vibrato.
    let wob = (pl.time * 5.5 * std::f64::consts::TAU).sin() as f32;
    let rock = pl.vibrato.clamp(0.0, 1.0) * 0.18 * k * wob;
    let finger = bridge + along * st + along.norm() * rock;
    let pizz = pl.fs.technique == Technique::Pizzicato;
    // The bow at β of the stopped length; a pluck where the settings pluck (over the
    // fingerboard's end by default).
    let tc = if pizz {
        pl.fs.pizz_pos.clamp(0.05, 0.6)
    } else {
        pl.n.beta.clamp(0.02, 0.6)
    };
    let contact = bridge + (finger - bridge) * tc;
    let sounding = pl.n.live && pl.n.level > 0.0;
    // ---- the sounding string, lit, and its swing ----
    let amp = if sounding {
        (0.08 + 0.55 * pl.n.level.sqrt().min(1.0)) * k
    } else {
        0.0
    };
    let side = rig.normal[i].cross(along.norm()).norm();
    let pulled = if pizz && sounding { amp * 0.8 } else { 0.0 };
    let px = b.px();
    let w0 = (rig.gauge[i] * 2.0 * v.scale(contact)).max(px * 1.1);
    let mut line = [Pos2::ZERO; 25];
    let mut lo = [Pos2::ZERO; 25];
    let mut hi = [Pos2::ZERO; 25];
    for (j, ((p, l), h)) in line
        .iter_mut()
        .zip(lo.iter_mut())
        .zip(hi.iter_mut())
        .enumerate()
    {
        let t = j as f32 / 24.0;
        // A pluck pulls the string into a corner at the finger's point.
        let shape = if t < tc {
            t / tc
        } else {
            (1.0 - t) / (1.0 - tc)
        };
        let w = bridge + (finger - bridge) * t - side * (pulled * shape);
        let env = (std::f32::consts::PI * t).sin();
        *p = v.p(w);
        *l = v.p(w + side * (amp * env));
        *h = v.p(w - side * (amp * env));
    }
    if sounding {
        b.between(&lo, &line, with_alpha(pl.lit, 0.16));
        b.between(&line, &hi, with_alpha(pl.lit, 0.16));
        b.polyline(&lo, px, with_alpha(pl.lit, 0.45));
        b.polyline(&hi, px, with_alpha(pl.lit, 0.45));
    }
    let core = if sounding {
        pl.lit
    } else {
        mix(pl.lit, c(pal::SILVER), 0.5)
    };
    let glow = with_alpha(pl.lit, if sounding { 0.5 } else { 0.2 });
    b.glow_polyline(&line, w0, core, glow, s.s(if sounding { 3.0 } else { 1.5 }));
    // ---- the left hand's finger, stopping the string ----
    let up = along.norm();
    // A person's finger whatever the instrument's size: a violinist's is a cellist's. // r42:arcus-upper
    let kf = if d.held { 1.0 } else { k }; // r42:arcus-upper
    use crate::scene::bowed_hold::{self as hold, Hold, StopShow}; // r42:arcus-upper
                                                                  // On a stand nobody plays it: the string sounds, no finger, no bow. // r42:arcus-upper
    if pl.holder.hold == Hold::Stand {
        if st < 0.995 && pl.stop == StopShow::Marker {
            marker_at(b, v, pl, finger + rig.normal[i] * rig.gauge[i], s.s(10.0));
            // r42:arcus-upper
        }
        return; // r42:arcus-upper
    }
    // r42:arcus-upper — a figure's own left hand goes to the stop; otherwise the
    // fingertip, the marker, or nothing.
    if let Some(arm) = hold::left_arm(pl.b, pl.holder) {
        let l = super::hand::Left {
            rig,
            string: i,
            semis: super::hand::semis(st),
            at: pl.hand,
            rock,
            side: ex,
        };
        super::hand::left_arm(b, v, &arm, &l);
    } else if st < 0.995 && pl.stop == StopShow::Finger {
        // Under a chin the finger curls over the fingerboard's edge from the hand behind
        // the neck (its first joint up, the rest down out of sight). // r42:arcus-upper
        let (j1, j2) = if d.held {
            let j1 = finger + ex * (2.0 * kf) + ez * (1.5 * kf) - up * (0.3 * kf);
            (j1, j1 + ex * (2.2 * kf) - ez * (1.8 * kf) - up * (0.5 * kf))
        } else {
            let j1 = finger + ex * (1.5 * kf) + ez * (2.2 * kf) + up * (0.4 * kf);
            (j1, j1 + ex * (2.6 * kf) + ez * (0.9 * kf) + up * (0.6 * kf))
        };
        fingertip(b, v, finger, j1, j2, 0.78 * kf, false);
    }
    if st < 0.995 && pl.stop == StopShow::Marker {
        marker_at(b, v, pl, finger + rig.normal[i] * rig.gauge[i], s.s(10.0)); // r42:arcus-upper
    }
    if pizz {
        let tip = contact - side * pulled;
        let j1 = tip - ex * (1.7 * kf) + ez * (2.0 * kf) - up * (0.5 * kf);
        let j2 = j1 - ex * (2.6 * kf) + ez * (0.7 * kf) - up * (0.8 * kf);
        fingertip(b, v, tip, j1, j2, 0.72 * kf, true);
        return;
    }
    let (frog, dir, n) = bow(b, v, pl, contact, i);
    // The figure's right arm to the frog (a musician's sleeve and hand, the mannequin's
    // wood). // r42:arcus-upper
    if let Some(arm) = crate::scene::bowed_hold::arm(pl.b, pl.holder) {
        super::arm::right_arm(b, v, &arm, frog, dir, n);
    }
}

/// A finger: its tip pressing at `tip`, its joints `j1`, `j2` (its hand beyond, faded
/// out), `r` cm thick, its nail toward the viewer; a soft shadow under it.
fn fingertip(b: &mut Brush, v: &View, tip: V3, j1: V3, j2: V3, r: f32, right: bool) {
    let (pt, p1, p2) = (v.p(tip), v.p(j1), v.p(j2));
    let w = 2.0 * r * v.scale(tip);
    let shade = c(pal::SKIN_SHADE);
    let skin = c(pal::SKIN);
    let off = Vec2::new(w * 0.28, w * 0.34);
    b.segment(
        pt + off,
        p1 + off,
        w * 1.05,
        with_alpha(c(pal::SHADOW), 0.28),
        true,
    );
    // The hand beyond, fading.
    for j in 0..4 {
        let (a, e) = (j as f32 / 4.0, (j + 1) as f32 / 4.0);
        let alpha = 1.0 - e * 0.85;
        let (qa, qe) = (p1 + (p2 - p1) * a, p1 + (p2 - p1) * e);
        b.segment(qa, qe, w * 1.12, with_alpha(shade, alpha), true);
        b.segment(qa, qe, w * 0.8, with_alpha(skin, alpha), true);
    }
    b.segment(pt, p1, w * 1.06, shade, true);
    b.segment(pt, p1, w * 0.78, skin, true);
    let dir = (p1 - pt).normalized();
    let hl = Vec2::new(-dir.y, dir.x) * (w * if right { 0.16 } else { -0.16 });
    let lit = with_alpha(c(pal::NAIL), 0.55);
    b.segment(
        pt + dir * (w * 0.2) + hl,
        p1 - dir * (w * 0.15) + hl,
        w * 0.22,
        lit,
        true,
    );
    // The nail near the tip.
    let nail = pt + dir * (w * 0.55);
    let nc = mix(c(pal::NAIL), skin, 0.25);
    b.segment(
        nail - dir * (w * 0.18),
        nail + dir * (w * 0.22),
        w * 0.5,
        nc,
        true,
    );
}

/// The bow's measures: its hair's length and ribbon's width, cm (French bows for both; a
/// bass's a touch shorter and wider).
fn measures(b: Bowed) -> (f32, f32) {
    match b {
        Bowed::Cello => (60.0, 1.1),
        Bowed::Bass => (56.0, 1.6),
        Bowed::Violin => (65.0, 1.0), // r42:arcus-upper
        Bowed::Viola => (66.0, 1.05), // r42:arcus-upper
    }
}

/// The bow across string `i` at `contact`; where its frog is, its way (frog to tip) and the
/// string's normal (r42:arcus-upper: for the arm that holds it).
fn bow(b: &mut Brush, v: &View, pl: &Play, contact: V3, i: usize) -> (V3, V3, V3) {
    let rig = pl.rig;
    let k = pl.b.dims().body / 75.7;
    let [_, bridge, nut] = rig.strings[i];
    let sdir = (nut - bridge).norm();
    let n = rig.normal[i];
    let mut dir = sdir.cross(n).norm();
    if dir.dot(rig.pose.ex) < 0.0 {
        dir = -dir;
    }
    // The frog toward the player's right: -x behind a cello, +x under a chin. // r42:arcus-upper
    if pl.b.dims().held {
        dir = -dir;
    } // r42:arcus-upper
    let (hair, ribbon) = measures(pl.b);
    let press = (pl.n.force / pl.n.hi.max(1e-3)).clamp(0.0, 1.6);
    let trem = pl.fs.technique == Technique::Tremolo;
    let at = if trem {
        let rate = f64::from(pl.fs.trem_rate.max(2.0));
        0.72 + 0.035 * (pl.time * rate * std::f64::consts::PI).sin() as f32
    } else {
        pl.travel
    }
    .clamp(0.06, 0.94);
    let speed = (pl.n.speed.abs() / 0.35).clamp(0.0, 1.0);
    let sign = if pl.n.speed < 0.0 { -1.0f32 } else { 1.0 };
    let base = contact + n * rig.gauge[i];
    let one = |b: &mut Brush, shift: f32, alpha: f32| {
        let c0 = base + dir * shift;
        let frog = c0 - dir * (at * hair);
        let tip = c0 + dir * ((1.0 - at) * hair);
        let dent = 0.32 * press * k.sqrt();
        let (h0, h1) = (frog + n * dent, tip + n * dent);
        let wv = ribbon * 0.5;
        let a = |p: V3| v.p(p);
        let col = |cc: Color32| with_alpha(cc, alpha);
        // The hair: a ribbon bent at the string.
        let across = |f: f32| [a(h0 + sdir * f), a(c0 + sdir * f), a(h1 + sdir * f)];
        b.between(&across(-wv), &across(wv), col(c(pal::HAIR_SHADE)));
        b.between(&across(-wv * 0.55), &across(wv * 0.2), col(c(pal::HAIR)));
        // Where it grips: wider and darker the harder it presses.
        let grip = (1.0 + 3.0 * press.min(1.2)) * k.sqrt();
        let g = |f: f32| [a(c0 - dir * grip + sdir * f), a(c0 + dir * grip + sdir * f)];
        let gc = with_alpha(c(pal::HAIR_GRIP), 0.35 + 0.4 * press.min(1.0));
        b.between(&g(-wv), &g(wv), col(gc));
        // The stick with its camber, thicker at the frog.
        let mut stick = [Pos2::ZERO; 17];
        for (j, q) in stick.iter_mut().enumerate() {
            let t = j as f32 / 16.0;
            let h = 2.1 + (1.55 - 2.1) * t - 1.45 * (std::f32::consts::PI * t).sin().powf(1.3);
            *q = a(frog.lerp(tip, t) + n * (h + dent * (1.0 - (2.0 * t - 1.0).abs())));
        }
        let ws = 0.85 * v.scale(c0);
        let lift = Vec2::new(0.0, -ws * 0.18);
        for j in 0..16 {
            let w = ws * (1.0 - 0.38 * j as f32 / 16.0);
            b.segment(stick[j], stick[j + 1], w, col(c(pal::STICK)), true);
        }
        for j in 0..16 {
            let w = ws * (1.0 - 0.38 * j as f32 / 16.0) * 0.32;
            let lc = with_alpha(c(pal::STICK_LIT), 0.8);
            b.segment(stick[j] + lift, stick[j + 1] + lift, w, col(lc), true);
        }
        // The head and its white tip plate.
        let hp = |u: f32, w: f32| a(tip + dir * u + n * w);
        let head = [
            hp(-0.4, 0.0),
            hp(0.5, 0.0),
            hp(0.95, 0.55),
            hp(0.85, 1.75),
            hp(-1.4, 1.55),
        ];
        b.polygon(&head, col(c(pal::STICK)));
        b.segment(
            hp(0.5, 0.0),
            hp(0.95, 0.55),
            ws * 0.3,
            col(c(pal::TIP)),
            true,
        );
        b.segment(
            hp(0.95, 0.55),
            hp(0.85, 1.7),
            ws * 0.3,
            col(c(pal::TIP)),
            true,
        );
        // The frog: ebony, its silver ferrule, pearl slide and eye; the button behind.
        let fp = |u: f32, w: f32| a(frog + dir * u + n * w);
        let body = [
            fp(0.4, 0.0),
            fp(-4.3, 0.15),
            fp(-4.6, 1.5),
            fp(-4.1, 2.05),
            fp(0.2, 2.05),
            fp(0.7, 1.3),
        ];
        b.polygon(&body, col(c(pal::FROG)));
        b.segment(
            fp(-4.0, 1.95),
            fp(0.1, 1.95),
            ws * 0.22,
            col(c(pal::FROG_LIT)),
            true,
        );
        b.segment(
            fp(0.25, 0.1),
            fp(0.6, 1.25),
            ws * 0.5,
            col(c(pal::SILVER)),
            true,
        );
        b.segment(
            fp(-4.0, 0.2),
            fp(-0.3, 0.12),
            ws * 0.28,
            col(c(pal::PEARL)),
            true,
        );
        b.circle(fp(-2.4, 1.1), ws * 0.36, Fill::Solid(col(c(pal::PEARL))));
        b.segment(stick[0], fp(-6.4, 2.1), ws * 0.95, col(c(pal::STICK)), true);
        b.segment(
            fp(-6.2, 2.1),
            fp(-7.6, 2.1),
            ws * 0.9,
            col(c(pal::SILVER)),
            true,
        );
    };
    // Its motion: a blur trailing behind it as fast as it moves; tremolo's both ways.
    if trem {
        one(b, -1.2 * k, 0.22);
        one(b, 1.2 * k, 0.22);
    } else if speed > 0.05 && pl.n.live {
        one(b, -sign * 2.6 * speed * k, 0.12);
        one(b, -sign * 1.3 * speed * k, 0.24);
    }
    one(b, 0.0, 1.0);
    (base - dir * (at * hair), dir, n) // r42:arcus-upper
}

/// Where the bow meets the string along its hair after `dt` seconds at `speed` m/s (down
/// toward the tip, up toward the frog), held a hand from either end.
pub fn travel(b: Bowed, at: f32, speed: f32, dt: f32) -> f32 {
    let (hair, _) = measures(b);
    (at + speed * 100.0 * dt.clamp(0.0, 0.1) / hair).clamp(0.08, 0.92)
}

/// The inspector's thumbnail: the instrument's outline upright, small, its strings.
pub fn thumbnail(b: &mut Brush, r: Rect, which: Bowed, s: &Style) {
    let d = which.dims();
    let o = crate::scene::bowed::outline(which, 2);
    let total = d.body + d.neck + d.pegbox + d.scroll_r * 2.0;
    let k = (r.height() / total).min(r.width() / (d.body * 0.62));
    let base = Pos2::new(r.center().x, r.bottom());
    let p = |x: f32, y: f32| Pos2::new(base.x + x * k, base.y - y * k);
    let wood = mix(c(pal::STICK_LIT), s.t.text_dim, 0.25);
    let edge = mix(c(pal::STICK), s.t.text, 0.15);
    let mut ring = [Pos2::ZERO; 256];
    let m = o.len().min(ring.len());
    for (q, &(x, y)) in ring.iter_mut().zip(o.iter()) {
        *q = p(x, y);
    }
    // A fan from the middle (the outline is star-shaped about it).
    let mid = p(0.0, d.body * 0.5);
    for j in 0..m {
        b.between(&[mid, ring[j]], &[mid, ring[(j + 1) % m]], wood);
    }
    b.polyline(&ring[..m], s.s(1.0), edge);
    let (yb, yn) = (d.body - d.stop, d.body + d.neck);
    b.segment(
        p(0.0, yn - d.board),
        p(0.0, yn),
        d.board_w.1 * k,
        c(pal::FROG),
        true,
    );
    b.segment(
        p(0.0, yn),
        p(0.0, yn + d.pegbox),
        d.head_w * 0.7 * k,
        edge,
        true,
    );
    let sc = p(0.0, yn + d.pegbox + d.scroll_r * 0.6);
    b.circle(sc, d.scroll_r * 0.8 * k, Fill::Solid(edge));
    for j in 0..4 {
        let x = (1.5 - j as f32) * d.spacing.1;
        b.segment(
            p(x * 0.6, yb - d.after),
            p(x, yb),
            s.s(0.6),
            c(pal::SILVER),
            false,
        );
        b.segment(p(x, yb), p(x * 0.45, yn), s.s(0.6), c(pal::SILVER), false);
    }
}

/// r42:arcus-upper — the stopped note's marker at `at`, a few points large at any size.
fn marker_at(b: &mut Brush, v: &View, pl: &Play, at: V3, size: f32) {
    super::hand::marker(b, v, at, size, pl.accent, c(pal::FROG));
}
