//! Arca's chassis (ADR 0137, provisional): a dark window into the amp. Its valves stand on
//! the chassis in the order the signal meets them — the preamp's (one per circuit the model
//! is cut into), the phase inverter, the power valves — each glowing as hard as it works
//! (the telemetry's swing of each circuit, the inverter's drive, the power valves' approach
//! to grid current), its warm light thrown on the chassis; the transformers at the end. On
//! the left the amp's character: its name, what it sounds like, its power valves, watts,
//! bias and rectifier. On the right the supply's needle, dipping as the supply sags. Along
//! the top the amps as lit legends (a click chooses one) and the oversampling with each
//! choice's latency. Calm at rest: the heaters' glow, the needle at idle.

use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use super::valve2 as tube;
use super::words;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::skin::cabinets as pal;
use crate::kit::text::{self as ktext, Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_dsp::arca::AmpModel;
use cantus_engine::fx::arca as tel;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};

const MAX_PRE: usize = 5;

/// The telemetry, read.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Reading {
    /// Whether a running unit sent it (a sample rate is there).
    pub seen: bool,
    pub stages: usize,
    pub pre: [f32; MAX_PRE],
    pub inverter: f32,
    pub power: f32,
    /// The supply now and at idle (V); zero without an amp.
    pub supply: f32,
    pub idle: f32,
}

impl Reading {
    pub fn of(t: &[f32; 64]) -> Reading {
        let mut pre = [0.0; MAX_PRE];
        pre.copy_from_slice(&t[tel::T_PRE..tel::T_PRE + MAX_PRE]);
        Reading {
            seen: t[tel::T_SAMPLE_RATE] > 0.0,
            stages: (t[tel::T_STAGES].max(0.0) as usize).min(MAX_PRE),
            pre,
            inverter: t[tel::T_INVERTER],
            power: t[tel::T_POWER],
            supply: t[tel::T_SUPPLY],
            idle: t[tel::T_IDLE],
        }
    }
}

/// How lit a preamp circuit's valve is for its swing (≈ 1 where it clips hard).
fn pre_glow(x: f32) -> f32 {
    let x = x.max(0.0);
    x / (x + 0.3)
}

/// How lit the inverter is for its drive (1 where its limit bends).
fn pi_glow(x: f32) -> f32 {
    let x = x.max(0.0);
    x / (x + 0.8)
}

/// What the display remembers between frames: each valve's glow and the needle, eased —
/// quick to light, slow to cool, as a filament does.
#[derive(Debug, Default)]
pub struct View {
    pub pre: [f32; MAX_PRE],
    pub inverter: f32,
    pub power: f32,
    /// The supply's sag below idle (V), and the idle it hangs from.
    pub sag: f32,
    pub idle: f32,
    moving: bool,
}

impl View {
    fn targets(r: &Reading) -> ([f32; MAX_PRE], f32, f32, f32) {
        let mut pre = [0.0; MAX_PRE];
        for (p, x) in pre.iter_mut().zip(r.pre).take(r.stages) {
            *p = pre_glow(x);
        }
        let sag = if r.idle > 0.0 {
            (r.idle - r.supply).max(0.0)
        } else {
            0.0
        };
        (pre, pi_glow(r.inverter), r.power.clamp(0.0, 1.0), sag)
    }

    /// Ease toward the reading over `dt` seconds.
    pub fn update(&mut self, r: &Reading, dt: f32) {
        let (pre, pi, power, sag) = Self::targets(r);
        let mut moving = false;
        let mut ease = |v: &mut f32, to: f32| {
            let tau = if to > *v { 0.035 } else { 0.30 };
            let a = 1.0 - (-dt / tau).exp();
            *v += (to - *v) * a;
            if (to - *v).abs() < 1e-3 {
                *v = to;
            } else {
                moving = true;
            }
        };
        for (v, to) in self.pre.iter_mut().zip(pre) {
            ease(v, to);
        }
        ease(&mut self.inverter, pi);
        ease(&mut self.power, power);
        ease(&mut self.sag, sag);
        if r.idle > 0.0 {
            self.idle = r.idle;
        }
        self.moving = moving;
    }

    /// Start at the reading (a screenshot's first frame).
    pub fn seed(&mut self, r: &Reading) {
        let (pre, pi, power, sag) = Self::targets(r);
        self.pre = pre;
        self.inverter = pi;
        self.power = power;
        self.sag = sag;
        self.idle = r.idle;
        self.moving = false;
    }

    pub fn moving(&self) -> bool {
        self.moving
    }

    /// How hard the amp works as a whole, 0 … 1 (the pilot light's breath).
    pub fn effort(&self) -> f32 {
        self.pre
            .iter()
            .copied()
            .fold(self.power, f32::max)
            .max(self.inverter)
    }
}

/// The display's parts.
struct Parts {
    band: Rect,
    info: Rect,
    valves: Rect,
    meter: Rect,
}

fn parts(r: Rect, k: f32) -> Parts {
    let band = Rect::from_min_size(r.min, vec2(r.width(), 34.0 * k));
    let body = Rect::from_min_max(pos2(r.left(), band.bottom()), r.max);
    let info_w = 232.0 * k;
    let meter_w = 176.0 * k;
    Parts {
        band,
        info: Rect::from_min_size(body.min, vec2(info_w, body.height())),
        valves: Rect::from_min_max(
            pos2(body.left() + info_w, body.top()),
            pos2(body.right() - meter_w, body.bottom()),
        ),
        meter: Rect::from_min_max(pos2(body.right() - meter_w, body.top()), body.max),
    }
}

/// The amps' legends along the band, each as wide as its name (both languages fit).
fn legend_rects(b_ctx: &egui::Context, band: Rect, k: f32) -> [Rect; 6] {
    let mut x = band.left() + 12.0 * k;
    std::array::from_fn(|i| {
        let t = Text::key(words::MODELS[i], Role::Control, Color32::TRANSPARENT);
        let w = ktext::measure(b_ctx, &t, k).x + 18.0 * k;
        let r = Rect::from_min_size(
            pos2(x, band.top() + 3.0 * k),
            vec2(w, band.height() - 6.0 * k),
        );
        x += w + 4.0 * k;
        r
    })
}

/// The oversampling's three places at the band's right.
fn os_rects(band: Rect, k: f32) -> [Rect; 3] {
    let (w, gap) = (84.0 * k, 4.0 * k);
    std::array::from_fn(|i| {
        let x = band.right() - 10.0 * k - (3 - i) as f32 * w - (2 - i) as f32 * gap;
        Rect::from_min_size(
            pos2(x, band.top() + 5.0 * k),
            vec2(w, band.height() - 10.0 * k),
        )
    })
}

/// The chassis in `r`: its legends answer clicks, the rest is drawn.
pub fn show(host: &mut dyn FxHost, v: &View, ui: &mut egui::Ui, s: &Style, r: Rect, rd: &Reading) {
    let k = s.k;
    // The reading has been eased into the view; nothing reads it raw yet.
    let _ = rd;
    let p = parts(r, k);
    let ink = Ink::of(s);
    let model = words::model(host.spec());
    let os_now = (host.spec().get("os").round().max(0.0) as usize).min(2);
    let legends = legend_rects(ui.ctx(), p.band, k);
    legends_edit(host, ui, s, &legends);
    os_edit(host, ui, s, p.band);
    let sr = host.sample_rate();
    let cab_on = host.spec().get("cab").round() > 0.0;
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, s.hue_tok);
        paint_band(b, &ink, &legends, p.band, model, os_now, sr, k);
        paint_valves(b, s, &ink, p.valves, model, v, r.shrink(1.0), k);
        paint_info(b, s, &ink, p.info, model, cab_on, k);
        paint_meter(b, &ink, p.meter, model, v, k);
        b.hline(
            r.left() + 10.0 * k,
            r.right() - 10.0 * k,
            p.band.bottom(),
            0.7 * k,
            ink.grid,
        );
        canvas::edge_b(b, r, &ink);
    });
}

/// The amps' legends: a click chooses one (its menu offers automation and MIDI learn).
fn legends_edit(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rects: &[Rect; 6]) {
    let now = host.spec().get("model").round().max(0.0) as usize;
    let name = frame::tr("fx.arca.model");
    for (i, r) in rects.iter().enumerate() {
        let resp = ui.interact(*r, ui.id().with(("arca-model", i)), Sense::click());
        let learnt = controls::stepped_offer(ui, s, host, "model", name, &resp);
        if resp.clicked() && !learnt && i != now {
            host.set("model", i as f32, None);
        }
        if resp.hovered() {
            let what = match i {
                0 => frame::tr("arca.d.none_tip"),
                i => frame::tr(words::character(AmpModel::from_index(i - 1)).what),
            };
            hint::set_text(ui.ctx(), frame::tr(words::MODELS[i]), what, Caps::NONE);
        }
    }
}

/// The oversampling: a click chooses 2×, 4× or 8×.
fn os_edit(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, band: Rect) {
    let now = (host.spec().get("os").round().max(0.0) as usize).min(2);
    let name = frame::tr("fx.arca.os");
    for (i, r) in os_rects(band, s.k).into_iter().enumerate() {
        let resp = ui.interact(r, ui.id().with(("arca-os", i)), Sense::click());
        let learnt = controls::stepped_offer(ui, s, host, "os", name, &resp);
        if resp.clicked() && !learnt && i != now {
            host.set("os", i as f32, None);
        }
        if resp.hovered() {
            hint::set_text(ui.ctx(), name, frame::tr("arca.d.os_tip"), Caps::NONE);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_band(
    b: &mut Brush,
    ink: &Ink,
    legends: &[Rect; 6],
    band: Rect,
    model: Option<AmpModel>,
    os_now: usize,
    sr: f32,
    k: f32,
) {
    let now = model.map_or(0, |m| m.index() + 1);
    for (i, r) in legends.iter().enumerate() {
        let on = i == now;
        let col = if on { ink.hue } else { ink.dim };
        let t = canvas::put(
            b,
            pos2(r.left() + 9.0 * k, r.center().y),
            Align2::LEFT_CENTER,
            &Text::key(words::MODELS[i], Role::Control, col),
            k,
            None,
        );
        if on {
            let y = r.bottom() - 1.0 * k;
            canvas::trace(
                b,
                &[pos2(t.left() - 2.0 * k, y), pos2(t.right() + 2.0 * k, y)],
                1.4 * k,
                ink.hue,
                ink,
            );
        }
    }
    // The oversampling at the right, each choice with the latency it gives.
    let rects = os_rects(band, k);
    canvas::put(
        b,
        pos2(rects[0].left() - 8.0 * k, band.center().y),
        Align2::RIGHT_CENTER,
        &Text::key("arca.d.os", Role::Micro, ink.dim),
        k,
        None,
    );
    for (i, r) in rects.into_iter().enumerate() {
        let on = i == os_now;
        let col = if on { ink.hue } else { ink.dim };
        if on {
            b.rrect(r, 3.0 * k, Fill::Solid(with_alpha(ink.hue, 0.12)));
        }
        b.rrect_stroke(
            r,
            3.0 * k,
            0.7 * k,
            if on { ink.hue } else { ink.grid_strong },
        );
        canvas::put(
            b,
            pos2(r.left() + 8.0 * k, r.center().y),
            Align2::LEFT_CENTER,
            &Text::key(words::OS[i], Role::Control, col),
            k,
            None,
        );
        let set = cantus_dsp::arca::ArcaSettings {
            model: Some(model.unwrap_or(AmpModel::Clean)),
            oversampling: [2, 4, 8][i],
            ..Default::default()
        };
        let ms = set.latency() as f32 / sr.max(1.0) * 1000.0;
        canvas::putf(
            b,
            pos2(r.right() - 8.0 * k, r.center().y),
            Align2::RIGHT_CENTER,
            Role::Value,
            if on { ink.text } else { ink.dim },
            k,
            None,
            format_args!("{ms:.1} ms"),
        );
    }
}

/// The amp's character: its name large, its sound in a line, then its facts.
fn paint_info(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    model: Option<AmpModel>,
    cab_on: bool,
    k: f32,
) {
    let x = r.left() + 16.0 * k;
    let w = r.width() - 24.0 * k;
    let mut y = r.top() + 16.0 * k;
    let line = |b: &mut Brush, y: f32, t: &Text<'_>| {
        let rr = Rect::from_min_size(pos2(x, y), vec2(w, 16.0 * k));
        let t = Text {
            align: Align2::LEFT_CENTER,
            ..*t
        };
        let _ = ktext::paint(b, rr, &t, k);
    };
    line(b, y, &Text::key("arca.d.amp", Role::Micro, ink.dim));
    y += 22.0 * k;
    let name = model.map_or(words::MODELS[0], |m| words::MODELS[m.index() + 1]);
    let big = Rect::from_min_size(pos2(x, y - 6.0 * k), vec2(w, 30.0 * k));
    let _ = ktext::paint(
        b,
        big,
        &Text::key(
            name,
            Role::Display,
            if model.is_some() { ink.hue } else { ink.dim },
        )
        .align(Align2::LEFT_CENTER),
        k,
    );
    y += 34.0 * k;
    let Some(m) = model else {
        let what = if cab_on {
            "arca.d.none_cab"
        } else {
            "arca.d.none_all"
        };
        line(b, y, &Text::key(what, Role::Body, ink.text));
        return;
    };
    let c = words::character(m);
    line(b, y, &Text::key(c.what, Role::Body, ink.text));
    y += 30.0 * k;
    // The facts, each a caption over its value.
    let fact = |b: &mut Brush, y: &mut f32, cap: &str, value: &str| {
        line(b, *y, &Text::key(cap, Role::Micro, ink.dim));
        line(
            b,
            *y + 15.0 * k,
            &Text::new(value, Role::Value, ink.text).caps(false),
        );
        *y += 38.0 * k;
    };
    let valves = format!(
        "{} \u{d7} {} \u{b7} {:.0} W",
        c.power_count, c.power_valve, c.watts
    );
    fact(b, &mut y, "arca.d.power_valves", &valves);
    let bias = if c.cathode_bias {
        "arca.d.cathode_bias"
    } else {
        "arca.d.fixed_bias"
    };
    fact(b, &mut y, "arca.d.bias", frame::tr(bias));
    let rect = if c.valve_rectifier {
        "arca.d.valve_rectifier"
    } else {
        "arca.d.diode_rectifier"
    };
    fact(b, &mut y, "arca.d.rectifier", frame::tr(rect));
    // What the glow and the bars under the valves mean.
    let y = r.bottom() - 18.0 * k;
    let bar = Rect::from_min_size(pos2(x, y - 1.0 * k), vec2(18.0 * k, 2.0 * k));
    b.rrect(bar, 1.0 * k, Fill::Solid(ink.grid));
    b.rrect(
        Rect::from_min_size(bar.min, vec2(11.0 * k, 2.0 * k)),
        1.0 * k,
        Fill::Solid(ink.hue),
    );
    canvas::put(
        b,
        pos2(x + 24.0 * k, y),
        Align2::LEFT_CENTER,
        &Text::key("arca.d.glow", Role::Micro, ink.dim),
        k,
        None,
    );
    let _ = s;
}

/// A valve on the chassis: where it stands, its heater and its plate, whether a power valve.
struct Bottle {
    r: Rect,
    plate: f32,
    power: bool,
}

/// The chassis seen from the front and a little above (v2, the owner's "a real chassis"):
/// the back plate with its vents, fuse and jacks; the steel deck; along the back the
/// preamp's valves and the inverter in their shielded sockets; along the front the filter
/// capacitors, the power valves, the output and the power transformers. Each valve's heater
/// glows once the amp is on, its plate reddens with the work it does; under each, its bar
/// and its type; under each row, its groups.
#[allow(clippy::too_many_arguments)]
fn paint_valves(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    model: Option<AmpModel>,
    v: &View,
    screen: Rect,
    k: f32,
) {
    let _ = (s, screen);
    let back = Rect::from_min_max(
        pos2(r.left() + 6.0 * k, r.top() + 6.0 * k),
        pos2(r.right() - 6.0 * k, r.top() + 30.0 * k),
    );
    let front_y = r.bottom() - 56.0 * k;
    // The deck: steel, lighter toward its front edge, a lit lip.
    let inset = 12.0 * k;
    let deck = [
        pos2(back.left() + inset, back.bottom()),
        pos2(back.right() - inset, back.bottom()),
        pos2(r.right() - 4.0 * k, front_y),
        pos2(r.left() + 4.0 * k, front_y),
    ];
    b.polygon(&deck, pal::CHASSIS.rgb());
    let lip = Rect::from_min_max(
        pos2(r.left() + 4.0 * k, front_y),
        pos2(r.right() - 4.0 * k, front_y + 9.0 * k),
    );
    b.rect(lip, Fill::V(pal::CHASSIS_TOP.rgb(), pal::CHASSIS.rgb()));
    b.hline(
        lip.left(),
        lip.right(),
        lip.top() + 0.5 * k,
        0.7 * k,
        with_alpha(pal::CHASSIS_EDGE.rgb(), 0.9),
    );
    tube::back_plate(b, k, back);
    let Some(m) = model else {
        canvas::put(
            b,
            pos2(r.center().x, (back.bottom() + front_y) * 0.5),
            Align2::CENTER_CENTER,
            &Text::key("arca.d.cold", Role::Body, ink.dim),
            k,
            None,
        );
        return;
    };
    let ch = words::character(m);
    let n = cantus_dsp::arca::net::split(m.preamp()).1.clamp(1, MAX_PRE);
    let pc = ch.power_count.clamp(2, 4);
    let heater = 0.55;
    // The back row: the preamp's valves and the inverter, a little smaller (further away).
    let back_y = back.bottom() + (front_y - back.bottom()) * 0.44;
    let hb = (back_y - r.top() - 10.0 * k).min(118.0 * k);
    let wb = hb * 0.36;
    let gap_b = (wb * 0.55).max(8.0 * k);
    let row_w = (n + 1) as f32 * wb + n as f32 * gap_b + 14.0 * k;
    let mut x = r.center().x - row_w * 0.5 - r.width() * 0.08;
    let mut bottles: Vec<Bottle> = Vec::with_capacity(n + 1 + pc);
    let mut groups: [(f32, f32, f32); 3] = [(0.0, 0.0, 0.0); 3];
    groups[0] = (x, 0.0, back_y);
    for i in 0..n {
        bottles.push(Bottle {
            r: Rect::from_min_max(pos2(x, back_y - hb), pos2(x + wb, back_y)),
            plate: v.pre[i],
            power: false,
        });
        x += wb + if i + 1 < n { gap_b } else { 0.0 };
    }
    groups[0].1 = x;
    x += gap_b + 14.0 * k;
    groups[1] = (x, x + wb, back_y);
    bottles.push(Bottle {
        r: Rect::from_min_max(pos2(x, back_y - hb), pos2(x + wb, back_y)),
        plate: v.inverter,
        power: false,
    });
    // The front row: the capacitors, the power valves, the transformers.
    let hp = (front_y - back_y - 34.0 * k).min(172.0 * k);
    let wp = hp * 0.40;
    let gap_p = (wp * 0.22).max(6.0 * k);
    let (cap_w, ot_w, pt_w) = (22.0 * k, hp * 0.42, hp * 0.52);
    let front_w = 2.0 * cap_w
        + 6.0 * k
        + pc as f32 * wp
        + (pc - 1) as f32 * gap_p
        + ot_w
        + pt_w
        + 3.0 * 18.0 * k;
    let mut x = r.center().x - front_w * 0.5;
    let caps = [
        Rect::from_min_max(pos2(x, front_y - hp * 0.42), pos2(x + cap_w, front_y)),
        Rect::from_min_max(
            pos2(x + cap_w + 6.0 * k, front_y - hp * 0.34),
            pos2(x + 2.0 * cap_w + 6.0 * k, front_y),
        ),
    ];
    x += 2.0 * cap_w + 6.0 * k + 18.0 * k;
    groups[2] = (x, 0.0, front_y);
    for i in 0..pc {
        bottles.push(Bottle {
            r: Rect::from_min_max(pos2(x, front_y - hp), pos2(x + wp, front_y)),
            plate: v.power,
            power: true,
        });
        x += wp + if i + 1 < pc { gap_p } else { 0.0 };
    }
    groups[2].1 = x;
    x += 18.0 * k;
    let ot = Rect::from_min_max(pos2(x, front_y - hp * 0.44), pos2(x + ot_w, front_y));
    x += ot_w + 18.0 * k;
    let pt = Rect::from_min_max(pos2(x, front_y - hp * 0.56), pos2(x + pt_w, front_y));
    // Sockets, then the valves (the back row first), the capacitors and the iron.
    for t in &bottles {
        let y = t.r.bottom();
        tube::ellipse(
            b,
            pos2(t.r.center().x, y + 1.0 * k),
            t.r.width() * 0.75,
            2.6 * k,
            pal::SOCKET.rgb(),
        );
    }
    for t in &bottles {
        tube::valve(b, k, t.r, heater, t.plate, t.power);
    }
    for cr in caps {
        tube::capacitor(b, k, cr);
    }
    tube::transformer(b, k, ot, true);
    tube::transformer(b, k, pt, true);
    // Under each valve: its bar of work; under each group: its types and its name.
    for t in &bottles {
        let w = t.r.width();
        let y = t.r.bottom() + if t.power { 15.0 } else { 7.0 } * k;
        let bar = Rect::from_min_size(pos2(t.r.left(), y), vec2(w, 1.6 * k));
        b.rrect(bar, 0.8 * k, Fill::Solid(ink.grid));
        let lit = t.plate.clamp(0.0, 1.0);
        if lit > 0.0 {
            b.rrect(
                Rect::from_min_size(bar.min, vec2(w * lit, bar.height())),
                0.8 * k,
                Fill::Solid(mix(ink.hue, ink.text, 0.25 * lit)),
            );
        }
    }
    for (i, t) in bottles.iter().take(n + 1).enumerate() {
        let ty = if i < n {
            words::pre_valve(m, i)
        } else {
            "12AX7"
        };
        canvas::put(
            b,
            pos2(t.r.center().x, back_y + 17.0 * k),
            Align2::CENTER_CENTER,
            &Text::new(ty, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    let types = format!("{} \u{d7} {}", pc, ch.power_valve);
    canvas::put(
        b,
        pos2((groups[2].0 + groups[2].1) * 0.5, front_y + 26.0 * k),
        Align2::CENTER_CENTER,
        &Text::new(&types, Role::Micro, ink.dim).caps(false),
        k,
        None,
    );
    let names = ["arca.d.preamp", "arca.d.inverter", "arca.d.power"];
    for (g, key) in groups.into_iter().zip(names) {
        let (a, z, floor) = g;
        let y = floor + if floor == front_y { 41.0 } else { 30.0 } * k;
        let t = canvas::put(
            b,
            pos2((a + z) * 0.5, y),
            Align2::CENTER_CENTER,
            &Text::key(key, Role::Micro, ink.text),
            k,
            Some(with_alpha(pal::CHASSIS.rgb(), 0.85)),
        );
        let col = ink.grid_strong;
        for (x0, x1) in [(a, t.left() - 5.0 * k), (t.right() + 5.0 * k, z)] {
            if x1 > x0 {
                b.hline(x0, x1, y, 0.6 * k, col);
            }
        }
        b.vline(a, y - 5.0 * k, y, 0.6 * k, col);
        b.vline(z, y - 5.0 * k, y, 0.6 * k, col);
    }
}

/// The supply's meter: a needle over a scale of volts below idle, the supply and the sag
/// read under it.
fn paint_meter(b: &mut Brush, ink: &Ink, r: Rect, model: Option<AmpModel>, v: &View, k: f32) {
    let on = model.is_some();
    let x = r.left() + 8.0 * k;
    canvas::put(
        b,
        pos2(x, r.top() + 16.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("arca.d.supply", Role::Micro, ink.dim),
        k,
        None,
    );
    const SPAN: f32 = 50.0;
    let rad = (r.width() * 0.42).min(78.0 * k);
    let c = pos2(r.center().x, r.top() + 44.0 * k + rad);
    let at = |t: f32| {
        let a = std::f32::consts::PI * (1.18 + 0.64 * t);
        c + vec2(a.cos(), a.sin()) * rad
    };
    // The scale: a fine arc, a tick each 5 V, longer each 25 V with its figure; the first
    // fifth (a supply pulled hard) marked.
    let mut pts = [Pos2::ZERO; 41];
    for (j, p) in pts.iter_mut().enumerate() {
        *p = at(j as f32 / 40.0);
    }
    b.polyline(&pts, 0.8 * k, ink.grid_strong);
    let mut hard = [Pos2::ZERO; 9];
    for (j, p) in hard.iter_mut().enumerate() {
        *p = at(0.2 * j as f32 / 8.0);
    }
    b.polyline(&hard, 1.6 * k, with_alpha(ink.danger, 0.55));
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let p = at(t);
        let d = (p - c).normalized();
        let major = i % 5 == 0;
        let len = if major { 7.0 } else { 4.0 } * k;
        b.segment(p, p + d * len, 0.7 * k, ink.grid_strong, false);
        if major {
            let volts = (-SPAN * (1.0 - t)).abs() * if t < 1.0 { -1.0 } else { 0.0 };
            canvas::putf(
                b,
                p + d * (len + 8.0 * k),
                Align2::CENTER_CENTER,
                Role::Micro,
                ink.dim,
                k,
                None,
                format_args!("{volts:.0}"),
            );
        }
    }
    let t = if on {
        1.0 - (v.sag / SPAN).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let tip = at(t);
    let col = if on { ink.hue } else { ink.dim };
    b.segment(c + (tip - c) * 0.18, tip, 1.2 * k, col, true);
    b.circle(c, 3.2 * k, Fill::Solid(col));
    b.circle(c, 1.4 * k, Fill::Solid(ink.ground));
    canvas::put(
        b,
        pos2(c.x, c.y + 14.0 * k),
        Align2::CENTER_CENTER,
        &Text::key("arca.d.below_idle", Role::Micro, ink.dim),
        k,
        None,
    );
    if !on {
        return;
    }
    // The readouts: the supply now, the sag.
    let y = c.y + 38.0 * k;
    let unit = frame::tr("arca.d.volts");
    if v.idle > 0.0 {
        let now = v.idle - v.sag;
        canvas::tag(
            b,
            pos2(x, y),
            Align2::LEFT_CENTER,
            frame::tr("arca.d.supply_now"),
            ink.text,
            k,
            None,
            format_args!("{now:.0} {unit}"),
        );
    }
    canvas::tag(
        b,
        pos2(x, y + 22.0 * k),
        Align2::LEFT_CENTER,
        frame::tr("arca.d.sag"),
        ink.text,
        k,
        None,
        format_args!("{:.1} {unit}", if v.sag < 0.05 { 0.0 } else { -v.sag }),
    );
}
