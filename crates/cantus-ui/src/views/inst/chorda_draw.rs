//! Chorda's pictures, drawn with the kit's brush into a card's display from the model's
//! own numbers (`cantus_dsp::chorda::tables`), never from a picture of a piano:
//!
//! - **The piano from above** ([`grand`]): every key's strings at their speaking length (one,
//!   two or three; the wound bass in the copper of the dynamics hue), the long bridge along
//!   their ends, the line where the hammers strike, the dampers (as firmly down as the
//!   sustain pedal leaves them; none above E6), the keyboard under it all. The key last
//!   played is lit, its hammer drawn where it strikes (shifted by the soft pedal).
//! - **The pedals** ([`pedals`]): the sustain's travel with the half-pedal law over it — how
//!   far the dampers are down at each position — and the soft pedal's.
//! - **The tuning** ([`stretch`]): each key's first partial against equal temperament, in
//!   cents: the stretch an aural tuner gives a stiff-stringed piano.
//! - **The felt** ([`felt`]): the shown key's hammer, force against compression, as measured
//!   and as set.
//! - **The ring** ([`ring`]): each key's first partial's T60, free and damped, on a log scale.

use super::super::organ::parts::label;
use crate::i18n::{t, tf, tf_count};
use crate::kit::color::{mix, with_alpha};
use crate::kit::light;
use crate::kit::text::Role;
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::chorda::tables::{Key, KEYS, LOWEST};
use cantus_dsp::chorda::voice::{damped_t60, pedal_contact};
use cantus_dsp::math;
use egui::{Pos2, Rect, Vec2};

/// What the pictures show of the instrument now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Now {
    /// The key shown (the last played, or middle C), and whether it is sounding.
    pub key: u8,
    pub playing: bool,
    pub velocity: f32,
    pub pedal: f32,
    pub soft: f32,
}

/// The display's caption line, and the picture's room under it.
fn caption(d: Rect, s: &Style) -> (Rect, Rect) {
    let line = Rect::from_min_size(
        Pos2::new(d.left() + s.s(10.0), d.top() + s.s(3.0)),
        Vec2::new(d.width() - s.s(20.0), s.s(15.0)),
    );
    let room = Rect::from_min_max(
        Pos2::new(d.left() + s.s(10.0), line.bottom() + s.s(4.0)),
        Pos2::new(d.right() - s.s(10.0), d.bottom() - s.s(6.0)),
    );
    (line, room)
}

fn is_black(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

/// The caption of the piano from above: the shown key's strings.
pub fn grand_caption(k: &Key) -> String {
    tf_count(
        "chorda.legend",
        k.strings as usize,
        &[
            ("key", &cantus_core::pitch_name(k.key)),
            ("hz", &format!("{:.1}", k.f1)),
            ("n", &k.strings),
            ("cm", &format!("{:.0}", k.length * 100.0)),
            ("pct", &format!("{:.0}", k.strike * 100.0)),
        ],
    )
}

/// The piano from above in the display `d`; `busy` (the design's progress) leads its
/// caption while the strings are designed.
pub fn grand(
    b: &mut Brush,
    d: Rect,
    s: &Style,
    keys: &[Key; KEYS],
    now: &Now,
    grip: f32,
    busy: Option<&str>,
) {
    let tk = &s.t;
    let shown = keys.iter().find(|k| k.key == now.key).unwrap_or(&keys[39]);
    let (line, room) = caption(d, s);
    let caption_text = match busy {
        Some(p) => format!("{p} \u{b7} {}", grand_caption(shown)),
        None => grand_caption(shown),
    };
    label(b, line, &caption_text, Role::Label, tk.text, s);
    light::sunken(b, room, s.r(3.0), tk.well, 1.0, tk.depth);
    let inner = room.shrink(s.s(4.0));
    let kb_h = s.s(16.0).min(inner.height() * 0.3);
    let kb = Rect::from_min_max(Pos2::new(inner.left(), inner.bottom() - kb_h), inner.max);
    // The hammers' line sits just behind the keys; the strings run away from the player.
    let base = kb.top() - s.s(6.0);
    let top = inner.top() + s.s(2.0);
    let w = inner.width() / KEYS as f32;
    let longest = keys.iter().fold(0.0f64, |m, k| m.max(k.length));
    let len_px = |k: &Key| ((k.length / longest) as f32 * (base - top)).max(s.s(3.0));
    let x_of = |i: usize| inner.left() + (i as f32 + 0.5) * w;
    let pitch = (w / 3.2).min(s.s(1.6));
    let thin = (w * 0.14).min(s.s(1.2)).max(b.px());
    let steel = tk.text_dim;
    let wound = mix(tk.text_dim, tk.hue_dynamics, 0.55);
    let mut bridge = Vec::with_capacity(KEYS);
    let mut strike = Vec::with_capacity(KEYS);
    let contact = pedal_contact(now.pedal as f64) as f32 * grip;
    for (i, k) in keys.iter().enumerate() {
        let x = x_of(i);
        let y_top = base - len_px(k);
        bridge.push(Pos2::new(x, y_top));
        strike.push(Pos2::new(x, base - k.strike as f32 * len_px(k)));
        let lit = k.key == shown.key;
        let color = if lit {
            with_alpha(
                s.hue,
                if now.playing {
                    0.55 + 0.45 * now.velocity
                } else {
                    0.9
                },
            )
        } else if k.strings < 3 || k.key <= 41 {
            wound
        } else {
            steel
        };
        let n = k.strings as usize;
        for j in 0..n {
            let off = (j as f32 - (n as f32 - 1.0) * 0.5) * pitch;
            let width = if lit { thin * 1.6 } else { thin };
            b.segment(
                Pos2::new(x + off, base),
                Pos2::new(x + off, y_top),
                width,
                color,
                false,
            );
        }
        // The damper, between the strike and the bridge: down as far as the pedal leaves
        // it; lifted off a sounding key; none above E6.
        if k.damper {
            let held = lit && now.playing;
            let c = if held { 0.0 } else { contact };
            let y = base - (k.strike as f32 + 0.1).min(0.9) * len_px(k);
            let r =
                Rect::from_center_size(Pos2::new(x, y), Vec2::new((w * 0.8).max(b.px()), s.s(4.0)));
            b.rect(r, Fill::Solid(with_alpha(tk.label, 0.15 + 0.6 * c)));
        }
    }
    b.polyline(&bridge, s.s(1.5), tk.label);
    b.dashed(
        &strike,
        s.s(1.0),
        with_alpha(s.hue, 0.6),
        s.s(3.0),
        s.s(2.0),
    );
    // The shown key's hammer, under its strings at the strike, shifted by the soft pedal.
    let i = (shown.key - LOWEST) as usize;
    let at = strike[i] + Vec2::new(now.soft * pitch * 1.5, 0.0);
    b.rrect(
        Rect::from_center_size(at, Vec2::new((w * 2.2).max(s.s(5.0)), s.s(7.0))),
        s.r(2.0),
        Fill::Solid(s.hue),
    );
    // The keyboard: one column per key, under its strings — the white keys run the full
    // depth, a black key's front is the white keys' beside it.
    b.rect(kb, Fill::Solid(tk.key_white));
    let front = kb.top() + kb.height() * 0.62;
    for (i, k) in keys.iter().enumerate() {
        let x0 = inner.left() + i as f32 * w;
        let col = Rect::from_min_max(Pos2::new(x0, kb.top()), Pos2::new(x0 + w, kb.bottom()));
        let lit = k.key == shown.key;
        if is_black(k.key) {
            let r = Rect::from_min_max(col.min, Pos2::new(col.right(), front));
            b.rect(r, Fill::Solid(if lit { s.hue } else { tk.key_black }));
        } else {
            if lit {
                b.rect(col, Fill::Solid(s.hue));
            }
            // A gap where two white keys meet without a black key between them.
            if matches!(k.key % 12, 4 | 11) && i + 1 < KEYS {
                b.vline(col.right(), kb.top(), kb.bottom(), b.px(), tk.key_black);
            }
        }
    }
    // Under the black keys, the white keys' fronts divide halfway.
    for (i, k) in keys.iter().enumerate() {
        if is_black(k.key) {
            let x = inner.left() + (i as f32 + 0.5) * w;
            b.vline(x, front, kb.bottom(), b.px(), tk.key_black);
        }
    }
}

/// The pedals in the display `d`: the sustain's travel with the half-pedal law over it,
/// and the soft pedal's.
pub fn pedals(b: &mut Brush, d: Rect, s: &Style, now: &Now, grip: f32) {
    let tk = &s.t;
    // The sustain's law takes the room; the soft pedal is a plain travel.
    let soft_h = s.s(48.0).min(d.height() * 0.4);
    let sus_h = d.height() - s.s(3.0) - soft_h;
    for (row, (value, text)) in [
        (
            now.pedal,
            tf(
                "chorda.sustain_line",
                &[
                    ("p", &format!("{:.0}", now.pedal * 100.0)),
                    (
                        "down",
                        &format!(
                            "{:.0}",
                            pedal_contact(now.pedal as f64) as f32 * grip * 100.0
                        ),
                    ),
                ],
            ),
        ),
        (
            now.soft,
            tf(
                "chorda.soft_line",
                &[("p", &format!("{:.0}", now.soft * 100.0))],
            ),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let band = if row == 0 {
            Rect::from_min_size(
                Pos2::new(d.left(), d.top() + s.s(3.0)),
                Vec2::new(d.width(), sus_h),
            )
        } else {
            Rect::from_min_size(
                Pos2::new(d.left(), d.bottom() - soft_h),
                Vec2::new(d.width(), soft_h),
            )
        };
        let (line, room) = caption(band, s);
        label(b, line, &text, Role::Label, tk.text, s);
        let bar_h = if row == 0 {
            room.height().min(s.s(110.0))
        } else {
            room.height().min(s.s(18.0))
        };
        let bar = Rect::from_min_max(room.min, Pos2::new(room.right(), room.top() + bar_h));
        light::sunken(b, bar, s.r(3.0), tk.well, 1.0, tk.depth);
        let x_of = |p: f32| bar.left() + p.clamp(0.0, 1.0) * bar.width();
        if row == 0 {
            // Half pedalling: where the dampers are neither down nor off.
            let zone = Rect::from_min_max(
                Pos2::new(x_of(0.15), bar.top() + b.px()),
                Pos2::new(x_of(0.65), bar.bottom() - b.px()),
            );
            b.rect(zone, Fill::Solid(with_alpha(s.hue, 0.2)));
            // How far down the dampers are at each position.
            let pts: Vec<Pos2> = (0..=48)
                .map(|i| {
                    let p = i as f32 / 48.0;
                    let c = pedal_contact(p as f64) as f32 * grip;
                    Pos2::new(
                        x_of(p),
                        bar.bottom() - s.s(3.0) - c * (bar.height() - s.s(6.0)),
                    )
                })
                .collect();
            b.polyline(&pts, s.s(1.5), tk.label);
            let c = pedal_contact(value as f64) as f32 * grip;
            let at = Pos2::new(
                x_of(value),
                bar.bottom() - s.s(3.0) - c * (bar.height() - s.s(6.0)),
            );
            b.circle(at, s.s(4.0), Fill::Solid(s.hue));
        }
        if value > 0.0 {
            let fill = Rect::from_min_max(
                Pos2::new(bar.left() + s.s(2.0), bar.bottom() - s.s(6.0)),
                Pos2::new(
                    x_of(value).max(bar.left() + s.s(2.0)),
                    bar.bottom() - s.s(3.0),
                ),
            );
            b.rrect(fill, s.r(1.5), Fill::Solid(s.hue));
        }
        b.segment(
            Pos2::new(x_of(value), bar.top() - s.s(2.0)),
            Pos2::new(x_of(value), bar.bottom() + s.s(2.0)),
            s.s(2.0),
            tk.text,
            true,
        );
    }
}

/// A plot's frame: the sunken well and the room inside it.
fn plot(b: &mut Brush, room: Rect, s: &Style) -> Rect {
    light::sunken(b, room, s.r(3.0), s.t.well, 1.0, s.t.depth);
    room.shrink(s.s(6.0))
}

/// Each key's first partial against equal temperament (cents) in the display `d`.
pub fn stretch(b: &mut Brush, d: Rect, s: &Style, keys: &[Key; KEYS], a4: f64, designing: bool) {
    let tk = &s.t;
    let cents: Vec<f64> = keys
        .iter()
        .map(|k| {
            let et = a4 * math::exp2((k.key as f64 - 69.0) / 12.0);
            1200.0 * math::log2(k.f1 / et)
        })
        .collect();
    let mut text = tf(
        "chorda.tuning_line",
        &[
            ("a4", &format!("{a4:.1}")),
            ("a0", &format!("{:+.0}", cents[0])),
            ("c8", &format!("{:+.0}", cents[KEYS - 1])),
        ],
    );
    if designing {
        text = format!("{text} \u{b7} {}", t("chorda.designing"));
    }
    let (line, room) = caption(d, s);
    label(b, line, &text, Role::Label, tk.text, s);
    let r = plot(b, room, s);
    let span = cents.iter().fold(10.0f64, |m, c| m.max(c.abs())).ceil();
    let y_of = |c: f64| r.center().y - (c / span) as f32 * r.height() * 0.5;
    let x_of = |i: usize| r.left() + i as f32 / (KEYS - 1) as f32 * r.width();
    b.hline(r.left(), r.right(), y_of(0.0), b.px(), tk.grid_strong);
    // Octave lines at every A.
    for (i, k) in keys.iter().enumerate() {
        if k.key % 12 == 9 {
            b.vline(x_of(i), r.top(), r.bottom(), b.px(), tk.grid);
        }
    }
    let pts: Vec<Pos2> = cents
        .iter()
        .enumerate()
        .map(|(i, c)| Pos2::new(x_of(i), y_of(*c)))
        .collect();
    b.polyline(&pts, s.s(2.0), s.hue);
}

/// The felt's force (N) at compression `u` (m): `K·u^p`.
fn felt_force(k: f64, p: f64, u: f64) -> f64 {
    k * math::powf(u, p)
}

/// The shown key's hammer felt in the display `d`: force against compression up to 1 mm,
/// as measured (dim) and as set (the hue).
pub fn felt(b: &mut Brush, d: Rect, s: &Style, key: &Key, hardness: f64) {
    let tk = &s.t;
    let h = hardness.clamp(-1.0, 1.0);
    let k_now = key.hammer_k * math::powf(4.0f64, h);
    let p_now = (key.hammer_p + 0.3 * h).clamp(1.8, 4.0);
    let text = tf(
        "chorda.felt_line",
        &[
            ("key", &cantus_core::pitch_name(key.key)),
            ("g", &format!("{:.1}", key.hammer_mass * 1000.0)),
            ("p", &format!("{p_now:.1}")),
        ],
    );
    let (line, room) = caption(d, s);
    label(b, line, &text, Role::Label, tk.text, s);
    let r = plot(b, room, s);
    let top = felt_force(k_now, p_now, 1e-3)
        .max(felt_force(key.hammer_k, key.hammer_p, 1e-3))
        .max(1.0);
    let curve = |k: f64, p: f64| -> Vec<Pos2> {
        (0..=40)
            .map(|i| {
                let u = i as f64 / 40.0 * 1e-3;
                Pos2::new(
                    r.left() + i as f32 / 40.0 * r.width(),
                    r.bottom() - (felt_force(k, p, u) / top) as f32 * r.height(),
                )
            })
            .collect()
    };
    for q in 1..4 {
        b.vline(
            r.left() + q as f32 * 0.25 * r.width(),
            r.top(),
            r.bottom(),
            b.px(),
            tk.grid,
        );
    }
    b.polyline(&curve(key.hammer_k, key.hammer_p), s.s(1.5), tk.text_dim);
    b.polyline(&curve(k_now, p_now), s.s(2.0), s.hue);
}

/// The caption of the strings' ring: the shown key's first partial's T60.
pub fn ring_caption(key: &Key, decay: f64) -> String {
    tf(
        "chorda.ring_line",
        &[
            ("key", &cantus_core::pitch_name(key.key)),
            ("s", &format!("{:.1}", key.t60(1) * decay)),
        ],
    )
}

/// Each key's first partial's T60 (s) on a log scale in the display `d`: free (the hue)
/// and with the damper down (dim, dashed; none above E6).
pub fn ring(b: &mut Brush, d: Rect, s: &Style, keys: &[Key; KEYS], shown: u8, decay: f64) {
    let tk = &s.t;
    let key = keys.iter().find(|k| k.key == shown).unwrap_or(&keys[39]);
    let (line, room) = caption(d, s);
    label(b, line, &ring_caption(key, decay), Role::Label, tk.text, s);
    let r = plot(b, room, s);
    // 0.05 s … 100 s.
    let (lo, hi) = (math::log10(0.05), math::log10(100.0));
    let y_of = |t: f64| {
        let v = ((math::log10(t.max(0.05)) - lo) / (hi - lo)).clamp(0.0, 1.0);
        r.bottom() - v as f32 * r.height()
    };
    let x_of = |i: usize| r.left() + i as f32 / (KEYS - 1) as f32 * r.width();
    for t in [0.1, 1.0, 10.0] {
        b.hline(r.left(), r.right(), y_of(t), b.px(), tk.grid);
    }
    let free: Vec<Pos2> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| Pos2::new(x_of(i), y_of(k.t60(1) * decay)))
        .collect();
    let damped: Vec<Pos2> = keys
        .iter()
        .enumerate()
        .filter(|(_, k)| k.damper)
        .map(|(i, k)| Pos2::new(x_of(i), y_of(damped_t60(k.key as f64))))
        .collect();
    b.dashed(&damped, s.s(1.5), tk.text_dim, s.s(4.0), s.s(3.0));
    b.polyline(&free, s.s(2.0), s.hue);
    let i = (key.key - LOWEST) as usize;
    b.circle(free[i], s.s(3.5), Fill::Solid(tk.text));
}
