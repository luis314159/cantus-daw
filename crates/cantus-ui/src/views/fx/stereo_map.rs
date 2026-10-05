//! Latitudo's map: everything that depends on frequency, on one axis. Above, the width
//! by band — each band's bar, the crossovers, the mono edge with the mono zone hatched,
//! and each band's correlation; below, the mid's and the side's tone with a dot per
//! shelf. The curves are the DSP's responses (`cantus_dsp::latitudo::view`).

use super::State;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use crate::views::fx::canvas::{self, Ink};
use crate::views::fx::controls;
use crate::views::fx::frame;
use crate::views::fx::host::FxHost;
use cantus_dsp::latitudo::view::telemetry;
use cantus_dsp::latitudo::{self, StereoSettings};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};

const F_LO: f32 = 20.0;
const F_HI: f32 = 20_000.0;
/// The width panel's top: 200 %.
const W_TOP: f32 = 2.0;
/// The tone panel's range, ± dB.
const T_DB: f32 = 12.0;
/// Points the curves are drawn with.
const N: usize = 160;

/// What a drag on the map holds.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Grab {
    /// A band's bar (0 low, 1 mid, 2 high).
    Band(usize),
    /// A crossover (0 low, 1 high).
    Cross(usize),
    Mono,
    /// A shelf: 0 mid low, 1 mid high, 2 side low, 3 side high.
    Shelf(usize),
}

const BAND_KEYS: [&str; 3] = ["w_lo", "w_mid", "w_hi"];
const CROSS_KEYS: [&str; 2] = ["x_lo", "x_hi"];
const SHELF_KEYS: [(&str, &str); 4] = [
    ("m_lo_f", "m_lo"),
    ("m_hi_f", "m_hi"),
    ("s_lo_f", "s_lo"),
    ("s_hi_f", "s_hi"),
];

pub fn x_of(r: Rect, hz: f32) -> f32 {
    let t = (hz.clamp(F_LO, F_HI) / F_LO).log2() / (F_HI / F_LO).log2();
    r.left() + t * r.width()
}

fn hz_of(r: Rect, x: f32) -> f32 {
    let t = ((x - r.left()) / r.width().max(1.0)).clamp(0.0, 1.0);
    F_LO * (F_HI / F_LO).powf(t)
}

pub fn wy_of(r: Rect, w: f32) -> f32 {
    r.bottom() - (w / W_TOP).clamp(0.0, 1.0) * r.height()
}

fn w_of(r: Rect, y: f32) -> f32 {
    ((r.bottom() - y) / r.height().max(1.0)).clamp(0.0, 1.0) * W_TOP
}

pub fn ty_of(r: Rect, db: f32) -> f32 {
    r.center().y - (db / T_DB).clamp(-1.0, 1.0) * r.height() * 0.5
}

fn db_of(r: Rect, y: f32) -> f32 {
    ((r.center().y - y) / (r.height() * 0.5).max(1.0)).clamp(-1.0, 1.0) * T_DB
}

/// The width the global controls give every band: width and the side against the mid.
fn global(ss: &StereoSettings) -> f32 {
    ss.width * latitudo::db_to_gain(ss.side_db) / latitudo::db_to_gain(ss.mid_db)
}

/// The bands' spans across the map: (left, right) in Hz.
fn spans(ss: &StereoSettings) -> [(f32, f32); 3] {
    let (lo, hi) = ss.crossovers();
    [(F_LO, lo), (lo, hi), (hi, F_HI)]
}

/// The map in `rect`; returns the width panel and the tone panel.
#[allow(clippy::too_many_arguments)]
pub fn show(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    tone_s: &Style,
    ss: &StereoSettings,
    tel: &[f32; 64],
    sr: f32,
    rect: Rect,
) -> (Rect, Rect) {
    let k = s.k;
    let ink = Ink::of(s);
    let mid_ink = tone_s.hue;
    let side_ink = s.hue;
    let resp = ui.interact(rect, ui.id().with("st-map"), Sense::click_and_drag());
    let pad = 8.0 * k;
    let axis = 16.0 * k;
    let inner = Rect::from_min_max(
        pos2(rect.left() + 34.0 * k, rect.top() + pad),
        pos2(rect.right() - pad, rect.bottom() - axis),
    );
    let split = inner.top() + inner.height() * 0.6;
    let wr = Rect::from_min_max(inner.min, pos2(inner.right(), split - 3.0 * k));
    let tr = Rect::from_min_max(pos2(inner.left(), split + 3.0 * k), inner.max);
    let g = global(ss);
    let sp = spans(ss);
    // The handles where they are now.
    let band_y = |i: usize| wy_of(wr, g * [ss.w_lo, ss.w_mid, ss.w_hi][i]);
    let (x_lo, x_hi) = ss.crossovers();
    let cross_x = [x_of(wr, x_lo), x_of(wr, x_hi)];
    let mono_x = x_of(wr, ss.mono_hz);
    let shelf = |i: usize| {
        let (f, d) = [
            (ss.mid_lo_hz, ss.mid_lo_db),
            (ss.mid_hi_hz, ss.mid_hi_db),
            (ss.side_lo_hz, ss.side_lo_db),
            (ss.side_hi_hz, ss.side_hi_db),
        ][i];
        pos2(x_of(tr, f), ty_of(tr, d))
    };
    let hover = resp.hover_pos();
    let grab_at = |p: Pos2| -> Option<Grab> {
        let near = 7.0 * k;
        if tr.expand(near).contains(p) {
            return (0..4)
                .map(|i| (i, shelf(i).distance(p)))
                .filter(|(_, d)| *d < 2.0 * near)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| Grab::Shelf(i));
        }
        if !wr.expand(near).contains(p) {
            return None;
        }
        if ss.mono_on() && (p.x - mono_x).abs() < near {
            return Some(Grab::Mono);
        }
        if !ss.mono_on() && p.x < wr.left() + near * 2.0 && p.y > wr.bottom() - 3.0 * near {
            return Some(Grab::Mono);
        }
        for (i, x) in cross_x.iter().enumerate() {
            if (p.x - x).abs() < near {
                return Some(Grab::Cross(i));
            }
        }
        let hz = hz_of(wr, p.x);
        let i = sp.iter().position(|(a, b)| hz >= *a && hz <= *b)?;
        ((p.y - band_y(i)).abs() < near).then_some(Grab::Band(i))
    };
    let hot = st.drag.or_else(|| hover.and_then(grab_at));

    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let px = b.px();
        // Frequency grid through both panels.
        for f in [
            50.0f32, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
        ] {
            let x = b.snap_mid(x_of(wr, f));
            b.vline(x, wr.top(), wr.bottom(), px, ink.grid);
            b.vline(x, tr.top(), tr.bottom(), px, ink.grid);
        }
        for w in [0.5f32, 1.5] {
            b.hline(
                wr.left(),
                wr.right(),
                b.snap_mid(wy_of(wr, w)),
                px,
                ink.grid,
            );
        }
        b.hline(
            wr.left(),
            wr.right(),
            b.snap_mid(wy_of(wr, 1.0)),
            px,
            ink.grid_strong,
        );
        for d in [-6.0f32, 6.0] {
            b.hline(
                tr.left(),
                tr.right(),
                b.snap_mid(ty_of(tr, d)),
                px,
                ink.grid,
            );
        }
        b.hline(
            tr.left(),
            tr.right(),
            b.snap_mid(ty_of(tr, 0.0)),
            px,
            ink.grid_strong,
        );
        b.hline(wr.left(), wr.right(), b.snap_mid(split), px, ink.edge);

        // The mono zone, hatched, with its edge.
        if ss.mono_on() {
            let zone = Rect::from_min_max(wr.min, pos2(mono_x, wr.bottom()));
            b.rect(zone, Fill::Solid(Ink::a(ink.dim, 0.10)));
            canvas::hatch(b, zone, 6.0 * k, px, Ink::a(ink.dim, 0.35));
            let w = if hot == Some(Grab::Mono) { 2.5 } else { 1.5 } * k;
            b.vline(mono_x, wr.top(), wr.bottom(), w, ink.text);
        }

        // The effective width over frequency: fill and line.
        let mut pts = [Pos2::ZERO; N];
        for (i, p) in pts.iter_mut().enumerate() {
            let hz = F_LO * (F_HI / F_LO).powf(i as f32 / (N - 1) as f32);
            *p = pos2(x_of(wr, hz), wy_of(wr, latitudo::width_at(ss, sr, hz)));
        }
        b.area(&pts, wr.bottom(), ink.fill(0.9));
        b.polyline(&pts, 2.0 * k, ink.hue);

        // Crossovers: dashed, with a grip at the top.
        for (i, x) in cross_x.iter().enumerate() {
            let on = hot == Some(Grab::Cross(i));
            canvas::dashed(
                b,
                pos2(*x, wr.top()),
                pos2(*x, wr.bottom()),
                if on { 2.0 } else { 1.0 } * k,
                Ink::a(ink.text, if ss.bands_on() || on { 0.8 } else { 0.4 }),
                4.0 * k,
                3.0 * k,
            );
            let grip = Rect::from_center_size(pos2(*x, wr.top() + 4.0 * k), vec2(8.0 * k, 8.0 * k));
            b.rrect(
                grip,
                2.0 * k,
                Fill::Solid(if on { ink.hue } else { ink.text }),
            );
        }
        // The bands' bars.
        for (i, (a, z)) in sp.iter().enumerate() {
            let (xa, xz) = (x_of(wr, *a) + 6.0 * k, x_of(wr, *z) - 6.0 * k);
            if xz <= xa {
                continue;
            }
            let y = band_y(i);
            let on = hot == Some(Grab::Band(i));
            b.segment(
                pos2(xa, y),
                pos2(xz, y),
                if on { 5.0 } else { 3.5 } * k,
                if on { ink.text } else { ink.hue },
                true,
            );
        }

        // Tone: both curves, then the dots.
        let mut mp = [Pos2::ZERO; N];
        let mut sp_ = [Pos2::ZERO; N];
        for i in 0..N {
            let hz = F_LO * (F_HI / F_LO).powf(i as f32 / (N - 1) as f32);
            let (m, sd) = latitudo::tone_db(ss, sr, hz);
            let x = x_of(tr, hz);
            mp[i] = pos2(x, ty_of(tr, m));
            sp_[i] = pos2(x, ty_of(tr, sd));
        }
        b.polyline(&sp_, 2.0 * k, side_ink);
        b.polyline(&mp, 2.0 * k, mid_ink);
        for i in 0..4 {
            let p = shelf(i);
            let col = if i < 2 { mid_ink } else { side_ink };
            let on = hot == Some(Grab::Shelf(i));
            let r = if on { 7.5 } else { 6.5 } * k;
            b.dot(p, r, col);
            if on {
                b.circle_stroke(p, r + 2.0 * k, 1.5 * k, ink.text);
            }
        }

        // Words last.
        words(b, s, &ink, ss, tel, wr, tr, rect, mid_ink, side_ink, &sp);
        for i in 0..4 {
            let p = shelf(i);
            let t = Text::new(
                if i < 2 { "M" } else { "S" },
                Role::Micro,
                contrast_on(&ink, i, mid_ink, side_ink),
            );
            canvas::put(b, p, Align2::CENTER_CENTER, &t, s.tk(), None);
        }
    });

    // The tone slope, in the tone panel's corner.
    let seg_w = 150.0;
    let seg = Rect::from_min_size(
        pos2(tr.right() - seg_w * k - 4.0 * k, tr.top() + 4.0 * k),
        vec2(seg_w * k, controls::ROW * k),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(seg));
    let labels = [
        crate::kit::text::Src::Key("st.slope.0"),
        crate::kit::text::Src::Key("st.slope.1"),
    ];
    let now = host.spec().get("tone_slope").round().clamp(0.0, 1.0) as usize;
    if let Some(i) = controls::segmented(
        &mut child,
        tone_s,
        now,
        &labels,
        frame::tr("st.tone_slope_tip"),
        seg_w,
    ) {
        host.set("tone_slope", i as f32, None);
    }

    gestures(host, st, ui, &resp, ss, wr, tr, grab_at, hot);
    (wr, tr)
}

/// A dot's letter in a colour that reads on it.
fn contrast_on(ink: &Ink, i: usize, mid: Color32, side: Color32) -> Color32 {
    let bg = if i < 2 { mid } else { side };
    crate::kit::color::ensure_contrast(ink.ground, bg, 3.0)
}

/// The map's words: axes, panel names and legend, the values of what can be dragged,
/// each band's correlation.
#[allow(clippy::too_many_arguments)]
fn words(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    ss: &StereoSettings,
    tel: &[f32; 64],
    wr: Rect,
    tr: Rect,
    rect: Rect,
    mid_ink: Color32,
    side_ink: Color32,
    sp: &[(f32, f32); 3],
) {
    let k = s.k;
    let tk = s.tk();
    let plate = Some(ink.plate);
    let gutter = wr.left() - 4.0 * k;
    let axis = |b: &mut Brush, y: f32, args: std::fmt::Arguments<'_>| {
        canvas::putf(
            b,
            pos2(gutter, y),
            Align2::RIGHT_CENTER,
            Role::Micro,
            ink.dim,
            tk,
            None,
            args,
        );
    };
    axis(b, wy_of(wr, 2.0), format_args!("200%"));
    axis(b, wy_of(wr, 1.0), format_args!("100%"));
    axis(b, wy_of(wr, 0.5), format_args!("50%"));
    axis(b, ty_of(tr, 6.0), format_args!("+6"));
    axis(b, ty_of(tr, 0.0), format_args!("0 dB"));
    axis(b, ty_of(tr, -6.0), format_args!("\u{2212}6"));
    for (f, w) in [
        (50.0f32, "50"),
        (100.0, "100"),
        (200.0, "200"),
        (500.0, "500"),
        (1_000.0, "1k"),
        (2_000.0, "2k"),
        (5_000.0, "5k"),
        (10_000.0, "10k"),
    ] {
        let t = Text::new(w, Role::Micro, ink.dim);
        let at = pos2(x_of(tr, f), rect.bottom() - 2.0 * k);
        canvas::put(b, at, Align2::CENTER_BOTTOM, &t, tk, None);
    }
    // The panels' names, with the tone legend.
    let at = pos2(wr.left() + 6.0 * k, wr.top() + 4.0 * k);
    let name = Text::key("st.width_axis", Role::Micro, ink.text);
    canvas::put(b, at, Align2::LEFT_TOP, &name, tk, plate);
    let at = pos2(tr.left() + 6.0 * k, tr.top() + 4.0 * k);
    let name = Text::key("st.tone_axis", Role::Micro, ink.text);
    let r = canvas::put(b, at, Align2::LEFT_TOP, &name, tk, plate);
    let mut x = r.right() + 10.0 * k;
    for (key, col) in [("fx.st.mid", mid_ink), ("fx.st.side", side_ink)] {
        let y = r.center().y;
        b.segment(pos2(x, y), pos2(x + 12.0 * k, y), 2.0 * k, col, true);
        let t = Text::key(key, Role::Micro, ink.text);
        let w = canvas::put(
            b,
            pos2(x + 16.0 * k, r.top()),
            Align2::LEFT_TOP,
            &t,
            tk,
            plate,
        );
        x = w.right() + 10.0 * k;
    }
    // What the handles are at.
    let g = global(ss);
    let playing = tel[telemetry::LEVEL_DB] > -100.0;
    for (i, (a, z)) in sp.iter().enumerate() {
        let (xa, xz) = (x_of(wr, *a), x_of(wr, *z));
        if xz - xa < 40.0 * k {
            continue;
        }
        let w = g * [ss.w_lo, ss.w_mid, ss.w_hi][i];
        let y = wy_of(wr, w);
        let above = y - 5.0 * k > wr.top() + 14.0 * k;
        let (yy, align) = if above {
            (y - 5.0 * k, Align2::CENTER_BOTTOM)
        } else {
            (y + 5.0 * k, Align2::CENTER_TOP)
        };
        canvas::putf(
            b,
            pos2((xa + xz) * 0.5, yy),
            align,
            Role::Value,
            ink.text,
            tk,
            plate,
            format_args!("{:.0}%", w * 100.0),
        );
        if playing {
            let c = tel[telemetry::BAND_CORR + i];
            let col = if c < 0.0 {
                ink.danger
            } else if c > 0.5 {
                ink.ok
            } else {
                ink.text
            };
            canvas::tag(
                b,
                pos2((xa + xz) * 0.5, wr.bottom() - 4.0 * k),
                Align2::CENTER_BOTTOM,
                frame::tr("st.band_corr"),
                col,
                tk,
                plate,
                format_args!("{c:+.2}"),
            );
        }
    }
    let (x_lo, x_hi) = ss.crossovers();
    for hz in [x_lo, x_hi] {
        canvas::putf(
            b,
            pos2(x_of(wr, hz) + 7.0 * k, wr.top() + 1.0 * k),
            Align2::LEFT_TOP,
            Role::Micro,
            ink.dim,
            tk,
            plate,
            format_args!("{}", Hz(hz)),
        );
    }
    if ss.mono_on() {
        let x = x_of(wr, ss.mono_hz);
        let t = Text::key("st.mono_zone", Role::Micro, ink.text);
        let r = canvas::put(
            b,
            pos2(x - 4.0 * k, wr.top() + 18.0 * k),
            Align2::RIGHT_TOP,
            &t,
            tk,
            plate,
        );
        canvas::putf(
            b,
            pos2(x - 4.0 * k, r.bottom() + 2.0 * k),
            Align2::RIGHT_TOP,
            Role::Micro,
            ink.text,
            tk,
            plate,
            format_args!("{}", Hz(ss.mono_hz)),
        );
    } else {
        // The mono edge waits at the left end: a tab to pull.
        let tab = Rect::from_min_size(
            pos2(wr.left(), wr.bottom() - 18.0 * k),
            vec2(5.0 * k, 16.0 * k),
        );
        b.rrect(tab, 2.0 * k, Fill::Solid(Ink::a(ink.text, 0.6)));
    }
}

/// `250 Hz`, `4.0 kHz`.
struct Hz(f32);

impl std::fmt::Display for Hz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= 1_000.0 {
            write!(f, "{:.1} kHz", self.0 / 1_000.0)
        } else {
            write!(f, "{:.0} Hz", self.0)
        }
    }
}

/// The range of one of the effect's parameters.
fn range(key: &str) -> (f32, f32) {
    cantus_core::EffectKind::Stereo
        .param(key)
        .map_or((0.0, 1.0), |i| (i.min, i.max))
}

#[allow(clippy::too_many_arguments)]
fn gestures(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    resp: &egui::Response,
    ss: &StereoSettings,
    wr: Rect,
    tr: Rect,
    grab_at: impl Fn(Pos2) -> Option<Grab>,
    hot: Option<Grab>,
) {
    if resp.drag_started() {
        // Decide by where the button went down: by the time a drag is recognised the
        // pointer has already moved off the handle.
        let at = ui.input(|i| i.pointer.press_origin()).or(resp.hover_pos());
        st.drag = at.and_then(&grab_at);
    }
    if let (Some(g), Some(p)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            drag(host, ss, g, p, wr, tr);
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    if resp.double_clicked() {
        if let Some(g) = hot {
            reset(host, g);
        }
    }
    match hot {
        Some(Grab::Cross(_)) | Some(Grab::Mono) => {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal)
        }
        Some(Grab::Band(_)) => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical),
        Some(Grab::Shelf(_)) => ui.ctx().set_cursor_icon(egui::CursorIcon::Grab),
        None => {}
    }
    if resp.hovered() {
        let name = match hot {
            Some(Grab::Band(i)) => frame::tr(["fx.st.w_lo", "fx.st.w_mid", "fx.st.w_hi"][i]),
            Some(Grab::Cross(i)) => frame::tr(["fx.st.x_lo", "fx.st.x_hi"][i]),
            Some(Grab::Mono) => frame::tr("fx.st.mono"),
            Some(Grab::Shelf(i)) => {
                frame::tr(["fx.st.m_lo", "fx.st.m_hi", "fx.st.s_lo", "fx.st.s_hi"][i])
            }
            None => frame::tr("st.card.map"),
        };
        hint::set_text(ui.ctx(), name, frame::tr("st.map_tip"), Caps::NONE);
    }
}

/// Move what is held to the pointer.
fn drag(host: &mut dyn FxHost, ss: &StereoSettings, g: Grab, p: Pos2, wr: Rect, tr: Rect) {
    match g {
        Grab::Band(i) => {
            let gl = global(ss);
            if gl > 1e-3 {
                let mut w = (w_of(wr, p.y) / gl).clamp(0.0, 2.0);
                // 100 % is a wire: land on it exactly when near.
                if (w - 1.0).abs() < 0.03 {
                    w = 1.0;
                }
                host.set(BAND_KEYS[i], w, Some(BAND_KEYS[i]));
            }
        }
        Grab::Cross(i) => {
            let key = CROSS_KEYS[i];
            let (lo, hi) = range(key);
            let hz = hz_of(wr, p.x);
            let hz = if i == 0 {
                hz.min(ss.x_hi / latitudo::MIN_BAND_RATIO)
            } else {
                hz.max(ss.x_lo * latitudo::MIN_BAND_RATIO)
            };
            host.set(key, hz.clamp(lo, hi), Some(key));
        }
        Grab::Mono => {
            let hz = hz_of(wr, p.x);
            let (lo, hi) = range("mono");
            // Pulled back to the left end, it is off.
            let v = if hz < 25.0 { lo } else { hz.clamp(lo, hi) };
            host.set("mono", v, Some("mono"));
        }
        Grab::Shelf(i) => {
            let (fk, gk) = SHELF_KEYS[i];
            let (flo, fhi) = range(fk);
            let mut db = db_of(tr, p.y);
            // 0 dB is a wire: land on it exactly when near.
            if db.abs() < 0.25 {
                db = 0.0;
            }
            let gesture = ["shelf-0", "shelf-1", "shelf-2", "shelf-3"][i];
            host.set(fk, hz_of(tr, p.x).clamp(flo, fhi), Some(gesture));
            host.set(gk, db, Some(gesture));
        }
    }
}

/// A double-click puts a handle back: a band to 100 %, a crossover where it was made,
/// the mono edge off, a shelf to 0 dB.
fn reset(host: &mut dyn FxHost, g: Grab) {
    let default = |key: &str| {
        cantus_core::EffectKind::Stereo
            .param(key)
            .map_or(0.0, |i| i.default)
    };
    match g {
        Grab::Band(i) => host.set(BAND_KEYS[i], 1.0, None),
        Grab::Cross(i) => host.set(CROSS_KEYS[i], default(CROSS_KEYS[i]), None),
        Grab::Mono => host.set("mono", latitudo::MONO_OFF_HZ, None),
        Grab::Shelf(i) => host.set(SHELF_KEYS[i].1, 0.0, None),
    }
}

/// The inspector's small picture: the width over frequency, the mono zone hatched.
pub fn mini(b: &mut Brush, rect: Rect, ink: &Ink, ss: &StereoSettings) {
    let px = b.px();
    b.hline(
        rect.left(),
        rect.right(),
        b.snap_mid(wy_of(rect, 1.0)),
        px,
        ink.grid_strong,
    );
    if ss.mono_on() {
        let zone = Rect::from_min_max(rect.min, pos2(x_of(rect, ss.mono_hz), rect.bottom()));
        canvas::hatch(b, zone, 4.0, px, Ink::a(ink.dim, 0.35));
    }
    let mut pts = [Pos2::ZERO; 48];
    for (i, p) in pts.iter_mut().enumerate() {
        let hz = F_LO * (F_HI / F_LO).powf(i as f32 / 47.0);
        *p = pos2(
            x_of(rect, hz),
            wy_of(rect, latitudo::width_at(ss, 48_000.0, hz)),
        );
    }
    b.area(&pts, rect.bottom(), ink.fill(0.8));
    b.polyline(&pts, 1.5, ink.hue);
}
