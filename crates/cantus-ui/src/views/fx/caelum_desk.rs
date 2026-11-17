//! Caelum's desk (design sheet v2): the sill under the window, where the endless controls
//! are lit pushbuttons in their bezels — **Infinite** (its ∞ lamp), **Freeze** (its crystal
//! lamp), and the two choices an infinite tank makes (the shimmer fades or sustains; the
//! level 8 dB lower or as loud), each a pair of buttons, the chosen one lit — and the rack
//! of eight glass tubes the decay glows in, octave by octave.
//!
//! A click throws a button; its menu offers automation and MIDI learn (the kit's
//! `stepped_offer`); the help line says what it does.

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
    None,
    Infinity,
    Crystal,
}

/// One button on the sill: its place, the parameter it throws (to `value`, or over when a
/// toggle), its word, its lamp.
#[derive(Copy, Clone, Debug)]
pub struct Key {
    pub rect: Rect,
    pub param: &'static str,
    value: Option<f32>,
    word: &'static str,
    glyph: Glyph,
    /// The help line: its name and what it does.
    tip: (&'static str, &'static str),
}

/// The sill's layout: the plate, its title, its captions, its six buttons.
pub struct Sill {
    pub plate: Rect,
    title: Rect,
    captions: [(Rect, &'static str); 2],
    pub keys: [Key; 6],
    /// The two pilot lamps at the sill's end (the input, the ducking): each lamp's centre
    /// and its caption's place.
    lamps: [(egui::Pos2, &'static str); 2],
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
    /// The buttons laid along `plate`: the title, then Infinite and Freeze, then the two
    /// pairs, spread evenly; every word measured, so none is cut.
    pub fn lay(ctx: &egui::Context, s: &Style, plate: Rect) -> Sill {
        let k = s.k;
        let inner = art::plate_inner(plate, k);
        let title = caption(ctx, "caelum.card.endless", k);
        let single = |w: &str| 34.0 * k + word(ctx, w, k) + 14.0 * k;
        let pair = |a: &str, b: &str, cap: &str| {
            let half = word(ctx, a, k).max(word(ctx, b, k)) + 22.0 * k;
            (half * 2.0).max(caption(ctx, cap, k).x + 4.0 * k)
        };
        let widths = [
            single("caelum.l.infinite"),
            single("caelum.l.freeze"),
            pair("caelum.s.fades", "caelum.s.sustains", "caelum.l.climb"),
            pair("caelum.s.under", "caelum.s.full", "caelum.l.level"),
        ];
        // The lamps at the end: each as wide as its caption.
        let lamp_w = |key: &str| caption(ctx, key, k).x.max(22.0 * k);
        let lamps_w = lamp_w("caelum.d.input") + lamp_w("caelum.d.ducking") + 14.0 * k;
        let used = title.x + widths.iter().sum::<f32>() + lamps_w;
        let gap = ((inner.width() - used) / 5.0).clamp(8.0 * k, 64.0 * k);
        let title_r =
            Rect::from_min_size(pos2(inner.left(), inner.center().y - title.y * 0.5), title);
        let mut x = title_r.right() + gap;
        let tall = |x: f32, w: f32| {
            Rect::from_min_max(pos2(x, inner.top() + 1.0 * k), pos2(x + w, inner.bottom()))
        };
        let row_top = inner.top() + 16.0 * k;
        let low =
            |x: f32, w: f32| Rect::from_min_max(pos2(x, row_top), pos2(x + w, inner.bottom()));
        let inf = tall(x, widths[0]);
        x = inf.right() + gap;
        let frz = tall(x, widths[1]);
        x = frz.right() + gap;
        let climb = low(x, widths[2]);
        x = climb.right() + gap;
        let level = low(x, widths[3]);
        let halves = |r: Rect| {
            let m = r.center().x;
            [
                Rect::from_min_max(r.min, pos2(m, r.bottom())),
                Rect::from_min_max(pos2(m, r.top()), r.max),
            ]
        };
        let lx1 = inner.right() - lamp_w("caelum.d.ducking") * 0.5;
        let lx0 =
            inner.right() - lamp_w("caelum.d.ducking") - 14.0 * k - lamp_w("caelum.d.input") * 0.5;
        let ly = inner.top() + 30.0 * k;
        let lamps = [
            (pos2(lx0, ly), "caelum.d.input"),
            (pos2(lx1, ly), "caelum.d.ducking"),
        ];
        let [c0, c1] = halves(climb);
        let [l0, l1] = halves(level);
        let key = |rect, param, value, word, glyph, tip| Key {
            rect,
            param,
            value,
            word,
            glyph,
            tip,
        };
        Sill {
            plate,
            title: title_r,
            lamps,
            captions: [
                (
                    Rect::from_min_max(
                        pos2(climb.left(), inner.top()),
                        pos2(climb.right(), row_top),
                    ),
                    "caelum.l.climb",
                ),
                (
                    Rect::from_min_max(
                        pos2(level.left(), inner.top()),
                        pos2(level.right(), row_top),
                    ),
                    "caelum.l.level",
                ),
            ],
            keys: [
                key(
                    inf,
                    "inf",
                    None,
                    "caelum.l.infinite",
                    Glyph::Infinity,
                    ("caelum.l.infinite", "caelum.tip.inf"),
                ),
                key(
                    frz,
                    "freeze",
                    None,
                    "caelum.l.freeze",
                    Glyph::Crystal,
                    ("caelum.l.freeze", "caelum.tip.freeze"),
                ),
                key(
                    c0,
                    "ishim",
                    Some(0.0),
                    "caelum.s.fades",
                    Glyph::None,
                    ("caelum.l.climb", "fx.caelum.ishim.0"),
                ),
                key(
                    c1,
                    "ishim",
                    Some(1.0),
                    "caelum.s.sustains",
                    Glyph::None,
                    ("caelum.l.climb", "fx.caelum.ishim.1"),
                ),
                key(
                    l0,
                    "ilevel",
                    Some(0.0),
                    "caelum.s.under",
                    Glyph::None,
                    ("caelum.l.level", "fx.caelum.ilevel.0"),
                ),
                key(
                    l1,
                    "ilevel",
                    Some(1.0),
                    "caelum.s.full",
                    Glyph::None,
                    ("caelum.l.level", "fx.caelum.ilevel.1"),
                ),
            ],
        }
    }
}

/// Whether `key` is lit at `spec`.
fn lit(key: &Key, spec: &EffectSpec) -> bool {
    let v = spec.get(key.param);
    match key.value {
        None => v >= 0.5,
        Some(want) => (v.round() - want).abs() < 0.5,
    }
}

/// The buttons answer the pointer: a click throws one (a toggle over, a pair's button to
/// its choice), its menu offers automation and MIDI learn, the help line says what it does.
pub fn edit(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, sill: &Sill) -> [Press; 6] {
    let mut out = [Press::default(); 6];
    for (i, key) in sill.keys.iter().enumerate() {
        let resp = ui.interact(
            key.rect,
            ui.id().with(("caelum-endless", i)),
            Sense::click(),
        );
        let name = tr(match key.param {
            "inf" => "fx.caelum.inf",
            "freeze" => "fx.caelum.freeze",
            "ishim" => "fx.caelum.ishim",
            _ => "fx.caelum.ilevel",
        });
        let learnt = controls::stepped_offer(ui, s, host, key.param, name, &resp);
        let on = lit(key, host.spec());
        if resp.clicked() && !learnt {
            match key.value {
                None => host.set(key.param, if on { 0.0 } else { 1.0 }, None),
                Some(v) if !on => host.set(key.param, v, None),
                _ => {}
            }
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
    out
}

/// The sill: its plate, its engraved title and captions, the buttons lit as the settings
/// stand.
pub fn sill(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    sill: &Sill,
    press: &[Press; 6],
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
    for (r, key) in sill.captions {
        canvas::put(
            b,
            pos2(r.center().x, r.center().y - 1.0 * k),
            Align2::CENTER_CENTER,
            &Text::key(key, Role::Micro, c(m.engrave)),
            k,
            None,
        );
    }
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
            pos2(p.x, sill.captions[0].0.center().y - 1.0 * k),
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
        match key.glyph {
            Glyph::None => {
                canvas::put(b, cap.center(), Align2::CENTER_CENTER, &text, k, None);
            }
            g => {
                let lamp = pos2(cap.left() + 16.0 * k, cap.center().y);
                let col = if on > 0.5 {
                    ink_c
                } else {
                    with_alpha(mix(lens, c(pal::STAR), 0.3), 0.55)
                };
                if g == Glyph::Infinity {
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
        }
    }
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
