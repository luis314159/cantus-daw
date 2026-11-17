//! Vetus's cassette (ADR 0130): the display's second object, chosen in the ⋯
//! menu beside the colourway (`unit_view`), a tape that ages with the settings as the record
//! does.
//!
//! - **The reels turn** with the song, the tape running from the left pack round the guide
//!   posts, past the head and between the capstan and the pinch roller into the right pack.
//!   **Wow** sways the reels' turn; **flutter** makes the capstan waver.
//! - **The level window** under the reels shows the music of the last second as the tape
//!   holds it: stepped by the **bits**, cut in facets by the **rate**.
//! - **Age** creases the tape across the head and fades the label; **noise** is a grain of
//!   hiss in the window, dust on the shell and a spark at the head when a tick falls;
//!   **tone** prints the medium (or the tape's type) on the label; **Character** its year.
//! - **Dragged** as the record is: turn it back (anticlockwise round the window) to age
//!   everything that is on.

use super::super::frame;
use super::record::{Groove, Look};
use crate::kit::color::with_alpha;
use crate::kit::skin::records as paint;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::modfx_clock::hash64;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};
use std::f32::consts::TAU;

/// The shell in a rect: a cassette's proportions (100 × 63.8 mm), centred.
pub fn shell(rect: Rect) -> Rect {
    let w = (rect.width() - 36.0)
        .min((rect.height() - 40.0) * 1.567)
        .max(40.0);
    Rect::from_center_size(rect.center() + vec2(0.0, 4.0), vec2(w, w / 1.567))
}

/// Where the drag turns round (the window's middle) and how far it reaches.
pub fn grip(rect: Rect) -> (Pos2, f32) {
    let s = shell(rect);
    (s.center() - vec2(0.0, s.height() * 0.06), s.height() * 0.5)
}

fn rgb(h: crate::kit::color::Hsl) -> Color32 {
    h.rgb()
}

fn u(i: u64, salt: u64) -> f32 {
    (hash64(i.wrapping_mul(0x9E37_79B9) ^ salt) >> 40) as f32 / (1u64 << 24) as f32
}

pub fn paint(b: &mut Brush, rect: Rect, s: &Style, look: &Look, groove: &Groove, k: f32) {
    let v = &look.s;
    let accent = s.hue;
    let sh = shell(rect);
    let (w, h) = (sh.width(), sh.height());
    let unit = w / 100.0; // a millimetre of the real thing
                          // The well.
    b.rect(rect, Fill::Solid(rgb(paint::PLINTH)));
    b.radial(
        rect,
        rect.center(),
        rect.size() * 0.75,
        &[
            (0.0, with_alpha(rgb(paint::PLINTH_EDGE), 0.0)),
            (0.6, with_alpha(rgb(paint::PLINTH_EDGE), 0.0)),
            (1.0, with_alpha(rgb(paint::PLINTH_EDGE), 0.9)),
        ],
        16,
    );
    let label = Rect::from_min_max(
        sh.min + vec2(9.0, 7.0) * unit,
        pos2(sh.right() - 9.0 * unit, sh.top() + 45.0 * unit),
    );
    // The window over the reels: the packs and the hubs seen through it, behind the label.
    let win = Rect::from_center_size(
        pos2(sh.center().x, label.top() + 25.0 * unit),
        vec2(62.0 * unit, 16.0 * unit),
    );
    let spin = look.turn * TAU * 1.7 + if v.wob_on { look.speed_ct * 0.01 } else { 0.0 };
    // The packs of tape: the left full early in the side, the right growing.
    let along = 0.35;
    let reel_l = pos2(win.left() + 13.0 * unit, win.center().y);
    let reel_r = pos2(win.right() - 13.0 * unit, win.center().y);
    let (pack_l, pack_r) = (
        (6.0 + 7.0 * (1.0 - along)) * unit,
        (6.0 + 7.0 * along) * unit,
    );
    // The shell's shadow on the well.
    b.rrect_soft(
        sh.translate(vec2(3.0 * k, 5.0 * k)),
        4.0 * unit,
        Fill::Solid(with_alpha(rgb(paint::ARM_SHADOW), 0.55)),
        10.0 * k,
    );
    // Inside the smoky shell: the tape from each pack down round its post and along the
    // bottom, seen faintly through the plastic.
    let y_tape = sh.bottom() - 9.0 * unit;
    for (c, pack, x) in [
        (reel_l, pack_l, sh.left() + 12.0 * unit),
        (reel_r, pack_r, sh.right() - 12.0 * unit),
    ] {
        b.circle(c, pack, Fill::Solid(with_alpha(rgb(paint::TAPE), 0.9)));
        let side = if x < c.x { -1.0 } else { 1.0 };
        b.segment(
            c + vec2(side * pack, 0.0),
            pos2(x + side * 1.4 * unit, y_tape),
            0.8 * k,
            with_alpha(rgb(paint::TAPE_SHINE), 0.9),
            false,
        );
    }
    // The shell: its shadow, the smoked plastic, a hairline of light on its edge.
    b.rrect(
        sh,
        4.0 * unit,
        Fill::V(
            with_alpha(
                rgb(paint::SHELL_LIGHT).lerp_to_gamma(rgb(paint::SHELL), 0.7),
                0.86,
            ),
            with_alpha(rgb(paint::SHELL), 0.84),
        ),
    );
    b.rrect_stroke(
        sh,
        4.0 * unit,
        0.8 * k,
        with_alpha(rgb(paint::SHELL_LIGHT), 0.8),
    );
    // The screws: four corners and the middle of the bottom.
    for p in [
        sh.min + vec2(5.0, 5.0) * unit,
        pos2(sh.right() - 5.0 * unit, sh.top() + 5.0 * unit),
        pos2(sh.left() + 5.0 * unit, sh.bottom() - 5.0 * unit),
        sh.max - vec2(5.0, 5.0) * unit,
        pos2(sh.center().x, sh.bottom() - 4.0 * unit),
    ] {
        b.dot(p, 1.6 * unit, rgb(paint::SCREW));
        b.segment(
            p - vec2(1.0, 1.0) * unit,
            p + vec2(1.0, 1.0) * unit,
            0.6 * k,
            rgb(paint::SHELL),
            false,
        );
    }
    b.rrect(win, 2.0 * unit, Fill::Solid(rgb(paint::WINDOW)));
    // The strand between the packs along the window's foot, a sheen running on it.
    let foot = win.bottom() - 1.6 * unit;
    b.segment(
        pos2(reel_l.x, foot),
        pos2(reel_r.x, foot),
        0.9 * k,
        rgb(paint::TAPE),
        false,
    );
    let gleam = (look.turn * 3.0).fract();
    let gx = reel_l.x + (reel_r.x - reel_l.x) * gleam;
    b.segment(
        pos2((gx - 4.0 * unit).max(reel_l.x), foot),
        pos2((gx + 4.0 * unit).min(reel_r.x), foot),
        0.9 * k,
        with_alpha(rgb(paint::TAPE_SHINE), 0.9),
        false,
    );
    for (c, pack) in [(reel_l, pack_l), (reel_r, pack_r)] {
        b.circle(c, pack, Fill::Solid(rgb(paint::TAPE)));
        // The pack's sheen: the light along its oxide.
        b.arc(
            c,
            pack * 0.78,
            -2.4,
            -1.4,
            pack * 0.3,
            with_alpha(rgb(paint::TAPE_SHINE), 0.22),
            false,
        );
        b.circle_stroke(
            c,
            pack - 0.6 * k,
            0.6 * k,
            with_alpha(rgb(paint::TAPE_SHINE), 0.6),
        );
    }
    // The label over them, cut round the window; faded by wear and Character.
    let age = (0.8 * if v.age_on { v.age.wear } else { 0.0 } + 0.6 * v.character).min(1.0);
    let paper = rgb(paint::LABEL_NEW).lerp_to_gamma(rgb(paint::LABEL_OLD), age);
    let paper_fill = Fill::Solid(paper);
    b.rect(
        Rect::from_min_max(label.min, pos2(label.right(), win.top())),
        paper_fill,
    );
    b.rect(
        Rect::from_min_max(pos2(label.left(), win.bottom()), label.max),
        paper_fill,
    );
    b.rect(
        Rect::from_min_max(
            pos2(label.left(), win.top()),
            pos2(win.left(), win.bottom()),
        ),
        paper_fill,
    );
    b.rect(
        Rect::from_min_max(
            pos2(win.right(), win.top()),
            pos2(label.right(), win.bottom()),
        ),
        paper_fill,
    );
    // What pokes past the label's edges above and below the window stays hidden: the shell.
    let band = Rect::from_min_max(
        pos2(label.left(), label.top() + 9.0 * unit),
        pos2(label.right(), label.top() + 12.0 * unit),
    );
    b.rect(band, Fill::Solid(accent));
    let ink = rgb(paint::LABEL_INK);
    let name = Text::new("VETUS", Role::Section, ink).caps(false);
    super::super::canvas::put(
        b,
        label.min + vec2(3.0, 4.5) * unit,
        Align2::LEFT_CENTER,
        &name,
        k,
        None,
    );
    let side = Text::new("A", Role::Section, ink).caps(false);
    super::super::canvas::put(
        b,
        pos2(label.right() - 3.0 * unit, label.top() + 4.5 * unit),
        Align2::RIGHT_CENTER,
        &side,
        k,
        None,
    );
    let what = if v.tone_on {
        frame::tr(super::MEDIUM_KEYS[v.medium as usize])
    } else {
        frame::tr("vetus.tape_type")
    };
    let year = super::year_text(v.character);
    let mut words = String::from(what);
    words.push_str("  \u{b7}  ");
    words.push_str(&year);
    let t = Text::new(&words, Role::Micro, ink).caps(false);
    super::super::canvas::put(
        b,
        pos2(label.center().x, label.bottom() - 3.5 * unit),
        Align2::CENTER_CENTER,
        &t,
        k,
        None,
    );
    // Creases in an old label.
    if v.age_on {
        for i in 0..(v.age.wear * 6.0) as u64 {
            let y = label.top() + (14.0 + 26.0 * u(i, 21)) * unit;
            if y > win.top() && y < win.bottom() {
                continue;
            }
            let x0 = label.left() + label.width() * u(i, 22);
            let len = (6.0 + 18.0 * u(i, 23)) * unit;
            b.segment(
                pos2(x0, y),
                pos2(
                    (x0 + len).min(label.right()),
                    y + 1.5 * unit * (u(i, 24) - 0.5),
                ),
                0.6 * k,
                with_alpha(ink, 0.18),
                false,
            );
        }
    }
    // The hubs and their six teeth, turning, in the window.
    for c in [reel_l, reel_r] {
        b.circle(c, 4.2 * unit, Fill::Solid(rgb(paint::HUB)));
        b.circle(c, 2.6 * unit, Fill::Solid(rgb(paint::WINDOW)));
        for j in 0..6 {
            let a = spin + j as f32 / 6.0 * TAU;
            let p0 = c + vec2(a.cos(), a.sin()) * (1.6 * unit);
            let p1 = c + vec2(a.cos(), a.sin()) * (2.7 * unit);
            b.segment(p0, p1, 1.1 * k, rgb(paint::HUB), true);
        }
    }
    // Hiss as grain in the window; dust on the shell.
    if v.noise_on {
        for i in 0..(v.noise.hiss * 140.0) as u64 {
            let p = pos2(
                win.left() + win.width() * u(i, 31),
                win.top() + win.height() * u(i, 32),
            );
            b.dot(p, 0.4 * k, with_alpha(rgb(paint::GROOVE), 0.25));
        }
        for i in 0..(v.noise.crackle * 30.0) as u64 {
            let p = pos2(sh.left() + w * u(i, 33), sh.top() + h * u(i, 34));
            if label.contains(p) {
                continue;
            }
            b.dot(
                p,
                (0.5 + 0.7 * u(i, 35)) * k,
                with_alpha(rgb(paint::DUST), 0.5),
            );
        }
    }
    window_glass(b, win, k);
    level(b, sh, unit, groove, look, accent, k);
    path(b, sh, unit, reel_l, reel_r, pack_l, pack_r, look, k);
}

/// The window's glass: a soft reflection across it.
fn window_glass(b: &mut Brush, win: Rect, k: f32) {
    let shine = Rect::from_min_max(win.min, pos2(win.max.x, win.center().y));
    b.rect(
        shine,
        Fill::V(
            with_alpha(rgb(paint::SHEEN), 0.07),
            with_alpha(rgb(paint::SHEEN), 0.0),
        ),
    );
    b.rrect_stroke(
        win,
        2.0 * k,
        0.8 * k,
        with_alpha(rgb(paint::SHELL_LIGHT), 0.7),
    );
}

/// The level window: the last second of the music as the tape holds it.
fn level(
    b: &mut Brush,
    sh: Rect,
    unit: f32,
    groove: &Groove,
    look: &Look,
    accent: Color32,
    k: f32,
) {
    let v = &look.s;
    let r = Rect::from_center_size(
        pos2(sh.center().x, sh.top() + 49.5 * unit),
        vec2(56.0 * unit, 6.0 * unit),
    );
    b.rrect(r, 1.0 * unit, Fill::Solid(rgb(paint::WINDOW)));
    let peaks = groove.peaks();
    let n = peaks.len().min(super::record::PER_TURN / 2);
    if n >= 2 {
        let hold = if v.rate_on {
            ((48_000.0 / v.rate_hz.max(1.0)) * 2.0)
                .round()
                .clamp(1.0, 24.0) as usize
        } else {
            1
        };
        let levels = v
            .bits_on
            .then(|| cantus_dsp::math::exp2(v.bits * 0.5).clamp(2.0, 18.0));
        let mut top = Vec::with_capacity(n);
        let mut bot = Vec::with_capacity(n);
        for j in 0..n {
            let age = j - j % hold;
            let mut a = peaks[peaks.len() - 1 - age.min(peaks.len() - 1)]
                .min(1.0)
                .sqrt();
            if let Some(l) = levels {
                a = (a * l).round() / l;
            }
            let x = r.right() - 1.0 * unit - (r.width() - 2.0 * unit) * j as f32 / (n - 1) as f32;
            let half = a * (r.height() * 0.42);
            top.push(pos2(x, r.center().y - half));
            bot.push(pos2(x, r.center().y + half));
        }
        b.between(&top, &bot, with_alpha(accent, 0.45));
        b.polyline(&top, 0.9 * k, accent);
        b.polyline(&bot, 0.6 * k, with_alpha(accent, 0.6));
    } else {
        b.hline(
            r.left() + unit,
            r.right() - unit,
            r.center().y,
            0.6 * k,
            with_alpha(accent, 0.5),
        );
    }
}

/// The tape's path along the bottom: from the left pack round its post, past the head
/// (creased with age, a spark when a tick falls), between the capstan and its roller, round
/// the right post into the right pack.
#[allow(clippy::too_many_arguments)]
fn path(
    b: &mut Brush,
    sh: Rect,
    unit: f32,
    reel_l: Pos2,
    reel_r: Pos2,
    pack_l: f32,
    pack_r: f32,
    look: &Look,
    k: f32,
) {
    let v = &look.s;
    let y = sh.bottom() - 9.0 * unit;
    let post_l = pos2(sh.left() + 12.0 * unit, y);
    let post_r = pos2(sh.right() - 12.0 * unit, y);
    // The ramp at the bottom the head reaches into: a trapezoid with its four holes.
    let ramp = [
        pos2(sh.left() + 22.0 * unit, y - 1.5 * unit),
        pos2(sh.right() - 22.0 * unit, y - 1.5 * unit),
        pos2(sh.right() - 17.0 * unit, sh.bottom()),
        pos2(sh.left() + 17.0 * unit, sh.bottom()),
    ];
    b.polygon(&ramp, with_alpha(rgb(paint::WINDOW), 0.9));
    b.polyline(
        &[ramp[0], ramp[1]],
        0.6 * k,
        with_alpha(rgb(paint::SHELL_LIGHT), 0.7),
    );
    for dx in [-24.0f32, -13.0, 13.0, 24.0] {
        b.dot(
            pos2(sh.center().x + dx * unit, y + 4.0 * unit),
            1.2 * unit,
            rgb(paint::SHELL),
        );
    }
    let tape = rgb(paint::TAPE_SHINE);
    let _ = (reel_l, reel_r, pack_l, pack_r);
    // Across the bottom: straight when new, creased with age.
    let creases = if v.age_on {
        (v.age.drop * 10.0) as usize
    } else {
        0
    };
    let n = 64;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let x = post_l.x + (post_r.x - post_l.x) * i as f32 / n as f32;
            let mut dy = 0.0;
            for c in 0..creases {
                let at = 0.1 + 0.8 * u(c as u64, 41);
                let d = (i as f32 / n as f32 - at).abs() * n as f32;
                if d < 1.5 {
                    dy += (1.5 - d) * 0.5 * unit * if c % 2 == 0 { 1.0 } else { -1.0 };
                }
            }
            pos2(x, y + dy)
        })
        .collect();
    b.polyline(&pts, 1.0 * k, tape);
    for p in [post_l, post_r] {
        b.circle(p, 1.4 * unit, Fill::Solid(rgb(paint::CAPSTAN)));
    }
    // The head in the middle, the capstan and its roller to the right — the roller
    // wavering with the flutter.
    let head = Rect::from_center_size(
        pos2(sh.center().x, y + 3.2 * unit),
        vec2(8.0 * unit, 4.0 * unit),
    );
    b.rrect(head, 1.0 * unit, Fill::Solid(rgb(paint::CAPSTAN)));
    b.vline(
        head.center().x,
        head.top(),
        head.bottom(),
        0.6 * k,
        rgb(paint::SHELL),
    );
    let waver = if v.wob_on {
        look.flutter * 0.5 * unit
    } else {
        0.0
    };
    let cap = pos2(sh.right() - 26.0 * unit, y + 2.2 * unit);
    b.circle(
        cap + vec2(waver, 0.0),
        2.4 * unit,
        Fill::Solid(rgb(paint::PINCH)),
    );
    b.circle_stroke(
        cap + vec2(waver, 0.0),
        2.4 * unit,
        0.6 * k,
        with_alpha(rgb(paint::SHELL_LIGHT), 0.8),
    );
    b.dot(cap - vec2(0.0, 3.4 * unit), 0.9 * unit, rgb(paint::CAPSTAN));
    if look.clicks > 0.0 && v.noise_on {
        let a = (look.clicks / 6.0).min(1.0);
        let p = pos2(head.center().x, y);
        b.circle_stroke(
            p,
            3.0 * unit,
            0.8 * k,
            with_alpha(rgb(paint::FLASH), 0.6 * a),
        );
        b.dot(p, 0.8 * unit, with_alpha(rgb(paint::FLASH), 0.9 * a));
    }
}
