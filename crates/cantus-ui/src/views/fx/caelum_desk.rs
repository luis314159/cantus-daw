//! Caelum's desk (design sheet v2, the sill amended with ADR 0138's Sustain): the sill under
//! the window, where the endless controls are lit pushbuttons in their bezels —
//! **Infinite** (its ∞ lamp), **Freeze** (its crystal lamp) — and **Sustain**, a glass gauge
//! with a brass rider (how much of Infinite's wash holds once nothing more comes in); and
//! the rack of eight glass tubes the decay glows in, octave by octave.
//!
//! A click throws a button, a drag moves the rider; each has its menu (automation,
//! modulation, MIDI learn — the kit's `stepped_offer`) and its help line.

use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame::tr;
use super::super::host::FxHost;
use super::art::{self, c, Press};
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::skin::observatory as pal;
use crate::kit::text::{self as ktext, Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::EffectSpec;
use egui::{pos2, vec2, Align2, Color32, Rect, Sense};

/// A button's lamp.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Glyph {
    Infinity,
    Crystal,
}

/// One button on the sill: its place, the parameter it throws over, its word, its lamp.
#[derive(Copy, Clone, Debug)]
pub struct Key {
    pub rect: Rect,
    pub param: &'static str,
    word: &'static str,
    glyph: Glyph,
    /// The help line: its name and what it does.
    tip: (&'static str, &'static str),
}

/// The Sustain gauge on the sill (ADR 0138's amendment): a glass tube lit from its left to
/// the sustain, a brass rider at the value, the word and the figure engraved over it,
/// marks at every quarter under it. Dimmed while Infinite is off (it is Infinite's).
#[derive(Copy, Clone, Debug)]
pub struct Gauge {
    /// Where it answers the pointer: its words and its tube.
    pub rect: Rect,
    /// The tube.
    pub tube: Rect,
    /// The words' row: the word at its left, the figure at its right.
    caption: Rect,
}

impl Gauge {
    /// The rider's travel: 0 % at its left, 100 % at its right.
    fn span(&self) -> (f32, f32) {
        let pad = self.tube.height() * 0.5;
        (self.tube.left() + pad, self.tube.right() - pad)
    }

    /// Where the rider stands for a sustain (0 … 1).
    pub fn x_of(&self, v: f32) -> f32 {
        let (a, b) = self.span();
        a + (b - a) * v.clamp(0.0, 1.0)
    }

    /// The sustain at `x`, to the whole per cent (what the figure shows).
    pub fn value_at(&self, x: f32) -> f32 {
        let (a, b) = self.span();
        (((x - a) / (b - a).max(1.0)).clamp(0.0, 1.0) * 100.0).round() / 100.0
    }
}

/// The sill's layout: the plate, its title, its two buttons, the Sustain gauge.
pub struct Sill {
    pub plate: Rect,
    title: Rect,
    pub keys: [Key; 2],
    pub gauge: Gauge,
    /// The two pilot lamps at the sill's end (the input, the ducking): each lamp's centre
    /// and its caption's place.
    lamps: [(egui::Pos2, &'static str); 2],
    /// The captions' row (the lamps' words stand on it).
    caption_y: f32,
}

fn word(ctx: &egui::Context, key: &str, k: f32) -> f32 {
    ktext::measure(
        ctx,
        &Text::key(key, Role::Control, Color32::TRANSPARENT).caps(false),
        k,
    )
    .x
}

fn caption(ctx: &egui::Context, key: &str, k: f32) -> egui::Vec2 {
    ktext::measure(ctx, &Text::key(key, Role::Micro, Color32::TRANSPARENT), k)
}

impl Sill {
    /// The parts laid along `plate`: the title, Infinite and Freeze, the Sustain gauge as
    /// wide as the room left allows (within 150 … 360 points), the lamps; every word
    /// measured, so none is cut.
    pub fn lay(ctx: &egui::Context, s: &Style, plate: Rect) -> Sill {
        let k = s.k;
        let inner = art::plate_inner(plate, k);
        let title = caption(ctx, "caelum.card.endless", k);
        let single = |w: &str| 34.0 * k + word(ctx, w, k) + 14.0 * k;
        let (w_inf, w_frz) = (single("caelum.l.infinite"), single("caelum.l.freeze"));
        // The gauge's words: the name and the widest figure, apart.
        let words = caption(ctx, "caelum.l.sustain", k).x
            + ktext::measure(
                ctx,
                &Text::new("100 %", Role::Micro, Color32::TRANSPARENT).caps(false),
                k,
            )
            .x
            + 16.0 * k;
        let lamp_w = |key: &str| caption(ctx, key, k).x.max(22.0 * k);
        let lamps_w = lamp_w("caelum.d.input") + lamp_w("caelum.d.ducking") + 14.0 * k;
        let used = title.x + w_inf + w_frz + lamps_w;
        let w_gauge = (inner.width() - used - 4.0 * 24.0 * k)
            .clamp((150.0 * k).max(words), (360.0 * k).max(words));
        let gap = ((inner.width() - used - w_gauge) / 4.0).clamp(8.0 * k, 64.0 * k);
        let title_r =
            Rect::from_min_size(pos2(inner.left(), inner.center().y - title.y * 0.5), title);
        let tall = |x: f32, w: f32| {
            Rect::from_min_max(pos2(x, inner.top() + 1.0 * k), pos2(x + w, inner.bottom()))
        };
        let row_top = inner.top() + 16.0 * k;
        let mut x = title_r.right() + gap;
        let inf = tall(x, w_inf);
        x = inf.right() + gap;
        let frz = tall(x, w_frz);
        x = frz.right() + gap;
        let area = Rect::from_min_max(pos2(x, inner.top()), pos2(x + w_gauge, inner.bottom()));
        let caption_r = Rect::from_min_max(area.min, pos2(area.right(), row_top));
        let th = 13.0 * k;
        let ty = row_top + (inner.bottom() - row_top) * 0.5 - 2.0 * k;
        let tube = Rect::from_min_max(
            pos2(area.left() + 2.0 * k, ty - th * 0.5),
            pos2(area.right() - 2.0 * k, ty + th * 0.5),
        );
        let lx1 = inner.right() - lamp_w("caelum.d.ducking") * 0.5;
        let lx0 =
            inner.right() - lamp_w("caelum.d.ducking") - 14.0 * k - lamp_w("caelum.d.input") * 0.5;
        let ly = inner.top() + 30.0 * k;
        let key = |rect, param, word, glyph, tip| Key {
            rect,
            param,
            word,
            glyph,
            tip,
        };
        Sill {
            plate,
            title: title_r,
            lamps: [
                (pos2(lx0, ly), "caelum.d.input"),
                (pos2(lx1, ly), "caelum.d.ducking"),
            ],
            caption_y: caption_r.center().y,
            gauge: Gauge {
                rect: area,
                tube,
                caption: caption_r,
            },
            keys: [
                key(
                    inf,
                    "inf",
                    "caelum.l.infinite",
                    Glyph::Infinity,
                    ("caelum.l.infinite", "caelum.tip.inf"),
                ),
                key(
                    frz,
                    "freeze",
                    "caelum.l.freeze",
                    Glyph::Crystal,
                    ("caelum.l.freeze", "caelum.tip.freeze"),
                ),
            ],
        }
    }
}

/// Whether `key` is lit at `spec`.
fn lit(key: &Key, spec: &EffectSpec) -> bool {
    spec.get(key.param) >= 0.5
}

/// The sill answers the pointer: a click throws a button, a press or a drag on the gauge
/// puts the sustain at the pointer (one drag, one step of undo; a double click puts it back
/// at 100 %); each has its menu (automation, modulation, MIDI learn) and its help line.
/// The presses: the two buttons', then the gauge's.
pub fn edit(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, sill: &Sill) -> [Press; 3] {
    let mut out = [Press::default(); 3];
    for (i, key) in sill.keys.iter().enumerate() {
        let resp = ui.interact(
            key.rect,
            ui.id().with(("caelum-endless", i)),
            Sense::click(),
        );
        let name = tr(match key.param {
            "inf" => "fx.caelum.inf",
            _ => "fx.caelum.freeze",
        });
        let learnt = controls::stepped_offer(ui, s, host, key.param, name, &resp);
        let on = lit(key, host.spec());
        if resp.clicked() && !learnt {
            host.set(key.param, if on { 0.0 } else { 1.0 }, None);
        }
        if resp.hovered() {
            hint::set_text(ui.ctx(), tr(key.tip.0), tr(key.tip.1), Caps::NONE);
        }
        out[i] = Press {
            lit: 0.0,
            hover: resp.hovered(),
            down: resp.is_pointer_button_down_on(),
        };
    }
    let g = &sill.gauge;
    let resp = ui.interact(
        g.rect,
        ui.id().with("caelum-sustain"),
        Sense::click_and_drag(),
    );
    let learnt = controls::stepped_offer(ui, s, host, "isus", tr("fx.caelum.isus"), &resp);
    if !learnt {
        if resp.double_clicked() {
            host.set("isus", 1.0, None);
        } else if let Some(p) = resp
            .interact_pointer_pos()
            .filter(|_| resp.is_pointer_button_down_on() || resp.clicked())
        {
            let v = g.value_at(p.x);
            if host.spec().get("isus") != v {
                host.set("isus", v, Some("caelum-sustain"));
            }
        }
        if resp.drag_stopped() || resp.clicked() {
            host.end_gesture();
        }
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            tr("caelum.l.sustain"),
            tr("caelum.tip.isus"),
            Caps::NONE,
        );
    }
    out[2] = Press {
        lit: 0.0,
        hover: resp.hovered(),
        down: resp.is_pointer_button_down_on(),
    };
    out
}

/// The sill: its plate, its engraved title, the buttons lit as the settings stand, the
/// gauge, the pilot lamps.
pub fn sill(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    sill: &Sill,
    press: &[Press; 3],
    spec: &EffectSpec,
    now: &super::display::Reading,
) {
    let k = s.k;
    let m = art::metal(s);
    art::plate(b, s, sill.plate);
    canvas::put(
        b,
        sill.title.left_center(),
        Align2::LEFT_CENTER,
        &Text::key("caelum.card.endless", Role::Micro, c(m.engrave)),
        k,
        None,
    );
    let glow = ink.voice(ink.hue, false);
    // The pilot lamps: the input's peak, the ducking's reduction.
    let input = (1.0 + cantus_dsp::math::log10(now.input.max(1e-6)) / 3.0).clamp(0.0, 1.0);
    let duck = if spec.get("duck") > 0.0 {
        (now.duck_db / 6.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    for ((p, key), (lens, lit)) in sill.lamps.iter().zip([(glow, input), (ink.danger, duck)]) {
        canvas::put(
            b,
            pos2(p.x, sill.caption_y - 1.0 * k),
            Align2::CENTER_CENTER,
            &Text::key(key, Role::Micro, c(m.engrave)),
            k,
            None,
        );
        art::pilot(b, s, *p, 5.5 * k, lens, lit);
    }
    for (key, p) in sill.keys.iter().zip(press) {
        let lens = if key.glyph == Glyph::Crystal {
            c(pal::ICE)
        } else {
            glow
        };
        let on = if lit(key, spec) { 1.0 } else { 0.0 };
        let cap = art::button(b, s, key.rect, lens, Press { lit: on, ..*p });
        let ink_c = art::cap_ink(lens, on);
        let text = Text::key(key.word, Role::Control, ink_c).caps(false);
        let lamp = pos2(cap.left() + 16.0 * k, cap.center().y);
        let col = if on > 0.5 {
            ink_c
        } else {
            with_alpha(mix(lens, c(pal::STAR), 0.3), 0.55)
        };
        if key.glyph == Glyph::Infinity {
            art::infinity(b, lamp, 18.0 * k, 1.6 * k, col);
        } else {
            art::crystal(b, lamp, 7.0 * k, 1.1 * k, col);
        }
        canvas::put(
            b,
            pos2(cap.left() + 30.0 * k, cap.center().y),
            Align2::LEFT_CENTER,
            &text,
            k,
            None,
        );
    }
    gauge(b, s, ink, &sill.gauge, press[2], spec);
}

/// The Sustain gauge (see [`Gauge`]).
fn gauge(b: &mut Brush, s: &Style, ink: &Ink, g: &Gauge, p: Press, spec: &EffectSpec) {
    let k = s.k;
    let m = art::metal(s);
    let on = spec.get("inf") >= 0.5;
    let lit = if on { 1.0 } else { 0.45 };
    let v = spec.get("isus").clamp(0.0, 1.0);
    canvas::put(
        b,
        pos2(g.caption.left() + 2.0 * k, g.caption.center().y - 1.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("caelum.l.sustain", Role::Micro, c(m.engrave)),
        k,
        None,
    );
    canvas::putf(
        b,
        pos2(g.caption.right() - 2.0 * k, g.caption.center().y - 1.0 * k),
        Align2::RIGHT_CENTER,
        Role::Micro,
        c(m.engrave),
        k,
        None,
        format_args!("{:.0} %", v * 100.0),
    );
    // The marks at every quarter, engraved under the tube.
    for q in 0..5 {
        let x = g.x_of(q as f32 * 0.25);
        let long = q % 2 == 0;
        b.vline(
            x,
            g.tube.bottom() + 2.0 * k,
            g.tube.bottom() + if long { 6.0 } else { 4.0 } * k,
            0.9 * k,
            with_alpha(c(m.engrave), 0.85),
        );
    }
    let t = g.tube;
    let r = t.height() * 0.5;
    // The tube's dark inside, the gas lit from its left to the sustain.
    b.rrect(t, r, Fill::Solid(mix(ink.ground, c(pal::GASKET), 0.5)));
    let glow = ink.voice(ink.hue, false);
    let x = g.x_of(v);
    let gas = Rect::from_min_max(
        pos2(t.left() + 2.0 * k, t.top() + 2.0 * k),
        pos2(x, t.bottom() - 2.0 * k),
    );
    if gas.width() > 1.0 {
        b.rrect_soft(
            gas.expand(3.0 * k),
            r,
            Fill::Solid(with_alpha(glow, 0.20 * lit)),
            8.0 * k,
        );
        b.rrect(
            gas,
            (r - 2.0 * k).max(1.0),
            Fill::V(
                with_alpha(mix(glow, c(pal::STAR), 0.45), lit),
                with_alpha(glow, 0.55 * lit),
            ),
        );
    }
    // The glass: its rim, a highlight along its top.
    b.rrect_stroke(t, r, 0.8 * k, with_alpha(c(pal::GLASS), 0.35));
    b.hline(
        t.left() + r,
        t.right() - r,
        t.top() + r * 0.45,
        1.1 * k,
        with_alpha(c(pal::GLASS), 0.22),
    );
    // The brass rider: a cap over the tube, its index across it.
    let lift = if p.down {
        0.0
    } else if p.hover {
        0.6
    } else {
        0.3
    };
    let rider = Rect::from_center_size(pos2(x, t.center().y), vec2(9.0 * k, t.height() + 8.0 * k));
    b.rrect_soft(
        rider.translate(vec2(0.0, 1.5 * k)),
        2.5 * k,
        Fill::Solid(with_alpha(c(pal::GASKET), 0.5)),
        3.0 * k,
    );
    b.rrect(rider, 2.5 * k, Fill::V(c(m.top), c(m.foot)));
    b.rrect_stroke(
        rider,
        2.5 * k,
        0.8 * k,
        with_alpha(c(m.edge), 0.6 + 0.4 * lift),
    );
    b.vline(
        x,
        rider.top() + 3.0 * k,
        rider.bottom() - 3.0 * k,
        1.2 * k,
        with_alpha(c(m.engrave), 0.9),
    );
}

/// The rack's parts in the tubes' area `r`.
pub struct Rack {
    pub rack: Rect,
    top_rail: Rect,
    foot_rail: Rect,
    well: Rect,
    /// The scale's span: 0.3 s at its foot, 60 s at its top (log).
    pub plot: Rect,
}

impl Rack {
    pub fn of(r: Rect, k: f32) -> Rack {
        let rack = Rect::from_min_max(
            pos2(r.left() + 36.0 * k, r.top() + 20.0 * k),
            pos2(r.right() - 4.0 * k, r.bottom() - 16.0 * k),
        );
        let (top_h, foot_h) = (8.0 * k, 10.0 * k);
        let top_rail = Rect::from_min_max(rack.min, pos2(rack.right(), rack.top() + top_h));
        let foot_rail = Rect::from_min_max(pos2(rack.left(), rack.bottom() - foot_h), rack.max);
        let well = Rect::from_min_max(
            pos2(rack.left(), top_rail.bottom()),
            pos2(rack.right(), foot_rail.top()),
        );
        Rack {
            rack,
            top_rail,
            foot_rail,
            well,
            plot: well.shrink2(vec2(0.0, 6.0 * k)),
        }
    }
}

/// The height of `t` seconds on the rack's scale.
pub fn decay_y(plot: Rect, t: f32) -> f32 {
    let lo = cantus_dsp::math::log10(0.3f32);
    let hi = cantus_dsp::math::log10(60.0f32);
    let u = (cantus_dsp::math::log10(t.clamp(0.3, 60.0)) - lo) / (hi - lo);
    plot.bottom() - u * plot.height()
}

/// The decay at height `y` of the rack's scale, rounded as the decay knob shows it (a
/// tenth of a second under 10 s, a second above).
pub fn decay_at(plot: Rect, y: f32) -> f32 {
    let lo = cantus_dsp::math::log10(0.3f32);
    let hi = cantus_dsp::math::log10(60.0f32);
    let u = ((plot.bottom() - y) / plot.height()).clamp(0.0, 1.0);
    let t = cantus_dsp::math::powf(10.0f32, lo + u * (hi - lo));
    let t = if t < 10.0 {
        (t * 10.0).round() / 10.0
    } else {
        t.round()
    };
    t.clamp(0.3, 60.0)
}

/// The rack answers the pointer: the dotted line across the tubes is the set decay, and a
/// press or a drag anywhere on the rack puts it at the pointer's height (the tubes then
/// glow to what the tank achieves there, the damping shaping each octave). One drag, one
/// step of undo. While infinite the rack shows ∞ and takes no drag (the decay knob still
/// sets the decay the tank returns to); the damping stays the Damping card's: a tube is the
/// achieved decay of the tank as a whole, not a control of its own octave.
pub fn tubes_edit(host: &mut dyn FxHost, ui: &mut egui::Ui, r: Rect, k: f32) {
    let plot = Rack::of(r, k).plot;
    let id = ui.id().with("caelum-decay");
    let resp = ui.interact(Rack::of(r, k).rack, id, Sense::click_and_drag());
    let inf = host.spec().get("inf") >= 0.5;
    if !inf {
        if let Some(p) = resp
            .interact_pointer_pos()
            .filter(|_| resp.is_pointer_button_down_on() || resp.clicked())
        {
            let t = decay_at(plot, p.y);
            if host.spec().get("decay") != t {
                host.set("decay", t, Some("caelum-decay"));
            }
        }
        if resp.drag_stopped() || resp.clicked() {
            host.end_gesture();
        }
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            tr("caelum.hint.decay"),
            tr(if inf {
                "caelum.hint.decay_inf"
            } else {
                "caelum.hint.decay_tip"
            }),
            Caps::NONE,
        );
    }
}

/// The decay by octave: eight glass tubes in a metal rack, each lit from its foot to the
/// decay the tank achieves at its octave (log, 0.3 … 60 s; to the top while infinite), the
/// figure over its light; the set decay a dotted line across; the scale beside them.
pub fn tubes(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    now: &super::display::Reading,
    spec: &EffectSpec,
) {
    let k = s.k;
    let m = art::metal(s);
    canvas::put(
        b,
        r.min,
        Align2::LEFT_TOP,
        &Text::key("caelum.d.decay", Role::Micro, ink.dim),
        k,
        None,
    );
    let Rack {
        rack,
        top_rail,
        foot_rail,
        well,
        plot,
    } = Rack::of(r, k);
    let y_of = |t: f32| decay_y(plot, t);
    // The scale: a fine line across the rack's back at 1, 10 and 60 s, its figure beside.
    let inf = spec.get("inf") >= 0.5;
    for (sec, label) in [(1.0f32, "1 s"), (10.0, "10 s"), (60.0, "60 s")] {
        let y = y_of(sec);
        b.hline(
            well.left(),
            well.right(),
            y,
            0.6 * k,
            with_alpha(ink.grid, 0.8),
        );
        if inf && sec >= 60.0 {
            // While infinite the top of the scale is ∞.
            continue;
        }
        canvas::put(
            b,
            pos2(rack.left() - 6.0 * k, y),
            Align2::RIGHT_CENTER,
            &Text::new(label, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    let glow = ink.voice(ink.hue, false);
    let w8 = rack.width() / 8.0;
    let tw = (w8 * 0.48).min(18.0 * k);
    let names = [
        "63 Hz", "125 Hz", "250 Hz", "500 Hz", "1 kHz", "2 kHz", "4 kHz", "8 kHz",
    ];
    for (i, a) in now.achieved.iter().enumerate() {
        let x = rack.left() + w8 * (i as f32 + 0.5);
        let tube = Rect::from_min_max(
            pos2(x - tw * 0.5, top_rail.bottom() - 1.0 * k),
            pos2(x + tw * 0.5, foot_rail.top() + 1.0 * k),
        );
        let secs = if inf || !a.is_finite() {
            60.0
        } else if now.seen {
            *a
        } else {
            spec.get("decay")
        };
        let y = if inf { plot.top() } else { y_of(secs) };
        // The tube's dark inside, the gas lit from the foot to its decay.
        b.rrect(
            tube,
            tw * 0.5,
            Fill::Solid(mix(ink.ground, c(pal::GASKET), 0.5)),
        );
        let gas = Rect::from_min_max(
            pos2(tube.left() + 2.0 * k, y),
            pos2(tube.right() - 2.0 * k, tube.bottom() - 2.0 * k),
        );
        if gas.height() > 1.0 {
            b.rrect_soft(
                gas.expand(3.0 * k),
                tw * 0.5,
                Fill::Solid(with_alpha(glow, 0.20)),
                8.0 * k,
            );
            b.rrect(
                gas,
                (tw * 0.5 - 2.0 * k).max(1.0),
                Fill::V(mix(glow, c(pal::STAR), 0.45), with_alpha(glow, 0.55)),
            );
            b.hline(
                gas.left(),
                gas.right(),
                y + 0.6 * k,
                1.2 * k,
                with_alpha(c(pal::STAR), 0.85),
            );
        }
        // The glass: its rim, a highlight down its left, the caps set in the rails.
        b.rrect_stroke(tube, tw * 0.5, 0.8 * k, with_alpha(c(pal::GLASS), 0.35));
        b.vline(
            tube.left() + tw * 0.28,
            tube.top() + tw * 0.4,
            tube.bottom() - tw * 0.4,
            1.2 * k,
            with_alpha(c(pal::GLASS), 0.22),
        );
        // Its figure over its light.
        if !inf {
            let fy = (y - 3.0 * k).max(plot.top() + 10.0 * k);
            canvas::putf(
                b,
                pos2(x, fy),
                Align2::CENTER_BOTTOM,
                Role::Micro,
                ink.text,
                k,
                Some(with_alpha(ink.ground, 0.75)),
                format_args!("{}", Secs(secs)),
            );
        }
        canvas::put(
            b,
            pos2(x, rack.bottom() + 3.0 * k),
            Align2::CENTER_TOP,
            &Text::new(names[i], Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    // The rails over the tubes' ends, a screw at each end.
    for rail in [top_rail, foot_rail] {
        b.rrect(rail, 2.5 * k, Fill::V(c(m.top), c(m.foot)));
        b.hline(
            rail.left() + 2.0 * k,
            rail.right() - 2.0 * k,
            rail.top() + 0.5 * k,
            0.8 * k,
            with_alpha(c(m.edge), 0.9),
        );
        for x in [rail.left() + 6.0 * k, rail.right() - 6.0 * k] {
            art::screw(b, s, pos2(x, rail.center().y), rail.height() * 0.3, 0.3);
        }
    }
    if inf {
        let p = pos2(rack.left() - 16.0 * k, y_of(60.0));
        art::infinity(b, p, 16.0 * k, 1.3 * k, ink.voice(ink.hue, true));
    } else {
        canvas::dashed(
            b,
            pos2(well.left(), y_of(spec.get("decay"))),
            pos2(well.right(), y_of(spec.get("decay"))),
            0.8 * k,
            with_alpha(ink.text, 0.45),
            3.0 * k,
            3.0 * k,
        );
    }
}

/// Seconds as a tube writes them: "0.9", "6.4", "12".
struct Secs(f32);

impl std::fmt::Display for Secs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 < 10.0 {
            write!(f, "{:.1}", self.0)
        } else {
            write!(f, "{:.0}", self.0)
        }
    }
}
