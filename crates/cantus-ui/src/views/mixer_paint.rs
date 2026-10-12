//! The desk's furniture: the ground, the title row, the legend column, the dock
//! dividers and the scrollbars — and the wheel, which decides between scrolling,
//! reordering and changing the width mode by where the pointer is and what is held.

use super::layout::{self, Fit, Row, WidthMode};
use super::menu::Menu;
use super::{desk, size, Geo, MixerView};
use crate::app::CantusApp;
use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{cap, chrome, hint, led, light, segmented, Brush, Fill, State, Style, Tok};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};

/// The tokens the desk's own surfaces paint with (for inspect mode).
const GROUND_TOKENS: &[Tok] = &[Tok::Panel, Tok::Strip];
const LEGEND_TOKENS: &[Tok] = &[Tok::Panel, Tok::TextDim];

/// The desk's ground under everything, and the title row's band.
pub(super) fn ground(b: &mut Brush, s: &Style, geo: &Geo) {
    let t = &s.t;
    b.register(geo.full, "mixer_ground", GROUND_TOKENS, &[]);
    b.rect(b.snap_rect(geo.full), Fill::Solid(t.panel));
    let r = b.snap_rect(geo.title);
    b.rect(r, Fill::V(shade(t.panel, 0.03), shade(t.panel, -0.01)));
    b.hline(
        r.left(),
        r.right(),
        r.top() + b.px() * 0.5,
        b.px(),
        shade(t.panel, 0.08),
    );
    b.hline(
        r.left(),
        r.right(),
        r.bottom() - b.px() * 0.5,
        b.px(),
        shade(t.panel, -0.15),
    );
}

/// FL's dock divider: four columns of pixels, dark in the middle.
pub(super) fn dividers(b: &mut Brush, s: &Style, geo: &Geo) {
    let p = s.t.panel;
    let px = b.px();
    let left = b.snap(geo.left.right() + size::DIVIDER * 0.5 - 2.0 * px);
    let right = b.snap(geo.right.left() - size::DIVIDER * 0.5 - 2.0 * px);
    let xs = [Some(left), (geo.right.width() > 0.0).then_some(right)];
    for x in xs.into_iter().flatten() {
        for (i, d) in [-0.10, -0.10, -0.18, -0.08].iter().enumerate() {
            b.vline(
                x + i as f32 * px,
                geo.rows.top(),
                geo.rows.bottom() + size::HBAR,
                px,
                shade(p, *d),
            );
        }
    }
}

/// One clickable spot in the title row.
fn key(ui: &Ui, rect: Rect, id: &str) -> egui::Response {
    ui.interact(rect, Id::new(("mixer-title", id)), Sense::click())
}

/// The title row: the desk's ▸ menu, `+`, the width modes, the wave view, the takes; at
/// the right the desk-wide switches that only show when they have something to undo, and
/// FL's "Mixer – <strip>".
pub(super) fn title(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    s: &Style,
    geo: &Geo,
) {
    let t = &s.t;
    let r = geo.title;
    let cy = r.center().y;
    let mut x = r.left() + 4.0;

    // ▸ the desk's menu.
    let mr = Rect::from_min_size(pos2(x, cy - 11.0), vec2(22.0, 22.0));
    let resp = key(ui, mr, "menu");
    if resp.hovered() {
        hint::set_text(ui.ctx(), &v.words.menu, "", hint::Caps::NONE);
        b.rrect(mr, s.r(3.0), Fill::Solid(with_alpha(t.text, 0.08)));
    }
    glyph::paint(b, mr.shrink(5.0), Glyph::Menu, t.text);
    if resp.clicked() {
        v.popups.open(Menu::Desk, pos2(mr.left(), r.bottom()));
    }
    let resp = resp.on_hover_text(&v.words.menu);
    let _ = resp;
    x = mr.right() + 4.0;

    // + a strip at the end of the desk.
    let ar = Rect::from_min_size(pos2(x, cy - 10.0), vec2(20.0, 20.0));
    let resp = key(ui, ar, "add");
    let st = State::of(&resp, false);
    light::raised(
        b,
        ar,
        s.r(3.0),
        shade(t.toolbar, if st.hovered { 0.0 } else { -0.04 }),
        light::RaisedOpts::default(),
    );
    glyph::paint(b, ar.shrink(5.0), Glyph::Plus, t.text);
    if resp.hovered() {
        hint::set_text(ui.ctx(), &v.words.add_strip, "", hint::Caps::NONE);
    }
    if resp.on_hover_text(&v.words.add_strip).clicked() {
        app.add_strip();
    }
    x = ar.right() + 8.0;

    // The width modes.
    let labels = [
        Src::Key(WidthMode::Compact.key()),
        Src::Key(WidthMode::Wide.key()),
        Src::Key(WidthMode::Xl.key()),
    ];
    let wr = Rect::from_min_size(pos2(x, cy - 10.0), vec2(192.0, 20.0));
    let resp = key(ui, wr, "width");
    let hovered = resp
        .hover_pos()
        .and_then(|p| segmented::hit(wr, labels.len(), p.x));
    let mode = v.prefs.current.width;
    if resp.clicked() {
        if let Some(i) = hovered {
            let mut p = v.prefs.current;
            p.width = WidthMode::from_index(i);
            v.prefs.set(p);
        }
    }
    if resp.hovered() {
        hint::set_text(ui.ctx(), &v.words.modes, "", hint::Caps::NONE);
    }
    let _ = resp.on_hover_text(&v.words.modes);
    segmented::paint(
        b,
        wr,
        &s.hue_of(Tok::Highlight),
        State::REST,
        &labels,
        mode.index(),
        hovered,
    );
    x = wr.right() + 8.0;

    // The meter-wave view (FL's Alt+W).
    let kr = Rect::from_min_size(pos2(x, cy - 10.0), vec2(20.0, 20.0));
    let resp = key(ui, kr, "wave");
    let on = app.settings.mixer_meter_wave;
    let st = State::of(&resp, on);
    cap::toolbar_key(b, kr, s, st, cap::KeyFace::Text(""));
    let ink = if on {
        t.highlight_glyph
    } else if st.hovered {
        t.text
    } else {
        t.text_dim
    };
    wave_glyph(b, kr.shrink(5.0), ink);
    if resp.hovered() {
        hint::set_text(ui.ctx(), &v.words.wave, "", hint::Caps::NONE);
    }
    if resp.on_hover_text(&v.words.wave).clicked() {
        app.run_action(crate::keymap::Action::ToggleMeterWave);
    }
    x = kr.right() + 10.0;

    // The cable view (round 25): none, the selection's, all.
    x = cable_view(v, ui, b, s, r, x);

    // Takes: the one running, or the list of those made.
    x = takes(v, app, ui, b, s, r, x);

    // The right side, from the right edge inwards.
    let mut right = r.right() - 10.0;
    right = title_text(v, app, b, s, r, right, x);
    let desk_state = (v.desk.any_solo, v.desk.any_muted);
    if desk_state.0 {
        right = led_key(
            v,
            app,
            ui,
            b,
            s,
            r,
            right,
            x,
            "mixer.solo_off",
            Tok::Highlight,
            true,
        );
    }
    if desk_state.1 {
        let _ = led_key(
            v,
            app,
            ui,
            b,
            s,
            r,
            right,
            x,
            "mixer.unmute_all",
            Tok::Led,
            false,
        );
    }
}

/// A little sine in a square, for the wave view's key.
fn wave_glyph(b: &mut Brush, r: Rect, color: egui::Color32) {
    let mut pts = [egui::Pos2::ZERO; 13];
    for (i, p) in pts.iter_mut().enumerate() {
        let f = i as f32 / 12.0;
        *p = pos2(
            r.left() + r.width() * f,
            r.center().y - (f * std::f32::consts::TAU).sin() * r.height() * 0.38,
        );
    }
    b.polyline(&pts, 1.4, color);
}

/// An LED button in the title row, right-aligned at `right`; returns the new right edge.
#[allow(clippy::too_many_arguments)]
fn led_key(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    s: &Style,
    r: Rect,
    right: f32,
    left: f32,
    label: &'static str,
    hue: Tok,
    solo: bool,
) -> f32 {
    let w = text::measure(
        ui.ctx(),
        &Text::key(label, Role::Label, s.t.text).caps(true),
        s.tk(),
    )
    .x + 30.0;
    let kr = Rect::from_min_max(
        pos2(right - w, r.center().y - 10.0),
        pos2(right, r.center().y + 10.0),
    );
    // No room left of the title: the key gives way (the desk's menu has it too).
    if kr.left() < left + 6.0 {
        return right;
    }
    let resp = key(ui, kr, label);
    led::button(
        b,
        kr,
        &s.hue_of(hue),
        State::of(&resp, true),
        Src::Key(label),
    );
    let tip = if solo {
        &v.words.solo_off
    } else {
        &v.words.unmute_all
    };
    if resp.hovered() {
        hint::set_text(ui.ctx(), tip, "", hint::Caps::NONE);
    }
    if resp.clicked() {
        if solo {
            app.clear_strip_solos();
        } else {
            app.unmute_all_strips();
        }
    }
    kr.left() - 6.0
}

/// "Mixer – <strip>", the context part dimmed, right-aligned at `right` (FL's window
/// title); the multi-selection's count before it. Returns the new right edge.
fn title_text(
    v: &mut MixerView,
    app: &CantusApp,
    b: &mut Brush,
    s: &Style,
    r: Rect,
    right: f32,
    left_limit: f32,
) -> f32 {
    let t = &s.t;
    let ctx = b.ctx().clone();
    let name = match app
        .selected_strip
        .and_then(|id| app.project().strip_index(id))
    {
        Some(i) => v.desk.name(app, i),
        None => "",
    };
    let win = Text::key("kit.mixer.title", Role::Title, t.text_dim).align(Align2::RIGHT_CENTER);
    let ww = text::measure(&ctx, &win, s.tk()).x;
    let room = (right - left_limit - ww - 20.0).max(0.0);
    let mut edge = right;
    if !name.is_empty() && room > 24.0 {
        let subj = Text::new(name, Role::Title, t.text)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Elide(Elide::Middle));
        let sw = text::measure(&ctx, &subj, s.tk()).x.min(room.min(220.0));
        let sr = Rect::from_min_max(pos2(right - sw, r.top()), pos2(right, r.bottom()));
        let _ = text::paint(b, sr, &subj, s.tk());
        let dash = Text::new("\u{2013}", Role::Title, t.text_dim);
        let dr = Rect::from_min_max(
            pos2(sr.left() - 14.0, r.top()),
            pos2(sr.left() - 2.0, r.bottom()),
        );
        let _ = text::paint(b, dr, &dash, s.tk());
        edge = dr.left();
    }
    let wr = Rect::from_min_max(pos2(edge - ww - 1.0, r.top()), pos2(edge, r.bottom()));
    let _ = text::paint(b, wr, &win, s.tk());
    edge = wr.left() - 12.0;
    let multi = v.selection_len(app);
    if multi > 1 {
        use std::fmt::Write as _;
        v.buf.clear();
        let _ = write!(v.buf, "{multi}");
        let chip = Text::new(&v.buf, Role::Value, t.selection).align(Align2::RIGHT_CENTER);
        let cw = text::measure(&ctx, &chip, s.tk()).x + 12.0;
        let cr = Rect::from_min_max(
            pos2(edge - cw, r.center().y - 9.0),
            pos2(edge, r.center().y + 9.0),
        );
        crate::kit::well::paint(b, cr, s, State::REST, None);
        let _ = text::paint(b, cr.shrink2(vec2(6.0, 0.0)), &chip, s.tk());
        edge = cr.left() - 8.0;
    }
    edge
}

/// The take being recorded (its time, in the record colour; a click stops it), or the
/// takes made so far as a menu field. Returns the new left edge.
fn takes(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    s: &Style,
    r: Rect,
    x: f32,
) -> f32 {
    use std::fmt::Write as _;
    let t = &s.t;
    if let Some((label, secs)) = app.mix_recorder.status() {
        v.buf.clear();
        let _ = write!(
            v.buf,
            "{label}  {}:{:04.1}",
            (secs / 60.0).floor() as u64,
            secs % 60.0
        );
        let tt = Text::new(&v.buf, Role::Value, t.danger).align(Align2::LEFT_CENTER);
        let w = text::measure(ui.ctx(), &tt, s.tk()).x + 30.0;
        let kr = Rect::from_min_size(pos2(x, r.center().y - 10.0), vec2(w, 20.0));
        let resp = key(ui, kr, "take");
        crate::kit::well::paint(b, kr, s, State::of(&resp, false), None);
        // A slow pulse: steady enough to read, alive enough to notice.
        let pulse = 0.75 + 0.25 * (ui.input(|i| i.time) as f32 * 3.0).sin();
        b.circle(
            pos2(kr.left() + 11.0, kr.center().y),
            4.0,
            Fill::Solid(with_alpha(t.danger, pulse)),
        );
        let _ = text::paint(
            b,
            kr.shrink2(vec2(10.0, 0.0)).translate(vec2(10.0, 0.0)),
            &tt,
            s.tk(),
        );
        if resp.hovered() {
            hint::set_text(ui.ctx(), &v.words.stop, "", hint::Caps::NONE);
        }
        if resp.on_hover_text(&v.words.stop).clicked() {
            app.stop_capture();
        }
        return kr.right() + 8.0;
    }
    let n = app.mix_recorder.takes.len();
    if n == 0 {
        return x;
    }
    v.buf.clear();
    let _ = write!(v.buf, "{} {n}", v.words.takes);
    let tt = Text::new(&v.buf, Role::Control, t.text);
    let w = text::measure(ui.ctx(), &tt, s.tk()).x + 30.0;
    let kr = Rect::from_min_size(pos2(x, r.center().y - 10.0), vec2(w, 20.0));
    let resp = key(ui, kr, "takes");
    let open = v.popups.is_open(Menu::Takes);
    crate::kit::well::menu_field(b, kr, s, State::of(&resp, open), &v.buf, false);
    if resp.clicked() {
        v.popups.open(Menu::Takes, pos2(kr.left(), r.bottom()));
    }
    kr.right() + 8.0
}

/// The scrollbars: always visible under the middle (6 pt), and at the right when the desk
/// is taller than its panel. Dragging the thumb scrolls; a click on the track pages.
pub(super) fn scrollbars(
    v: &mut MixerView,
    ui: &mut Ui,
    b: &mut Brush,
    s: &Style,
    geo: &Geo,
    fit: &Fit,
) {
    let (max_x, max_y) = v.scroll_max(geo, fit);
    let bars = [
        (
            Some(geo.hbar),
            geo.content_w,
            geo.middle.width(),
            max_x,
            true,
        ),
        (
            geo.vbar,
            fit.height + size::TOP,
            geo.rows.height(),
            max_y,
            false,
        ),
    ];
    for (rect, content, view, max, across) in bars {
        let Some(rect) = rect else {
            continue;
        };
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        let len = (view / content.max(1.0)).clamp(0.0, 1.0);
        let id = Id::new(("mixer-scrollbar", across));
        let resp = ui.interact(rect.expand(2.0), id, Sense::click_and_drag());
        let scroll = if across {
            &mut v.scroll_x
        } else {
            &mut v.scroll_y
        };
        let full = if across { rect.width() } else { rect.height() };
        if resp.dragged() {
            let d = resp.drag_delta();
            let moved = if across { d.x } else { d.y };
            *scroll = (*scroll + moved / full.max(1.0) * content).clamp(0.0, max);
        } else if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let thumb = chrome::thumb(rect, s, *scroll / content.max(1.0), len);
                let before = if across {
                    p.x < thumb.left()
                } else {
                    p.y < thumb.top()
                };
                let page = view * 0.9;
                *scroll = (*scroll + if before { -page } else { page }).clamp(0.0, max);
            }
        }
        let start = *scroll / content.max(1.0);
        chrome::scrollbar(b, rect, s, State::of(&resp, false), start, len);
    }
}

/// The ticks of the fader's scale, as positions on the travel: every 6 dB where the
/// taper is even (+12 … −24), then −30, −40, −60 and silence.
pub(super) fn ticks() -> &'static [f32; 11] {
    static TICKS: std::sync::OnceLock<[f32; 11]> = std::sync::OnceLock::new();
    TICKS.get_or_init(|| {
        let mut out = [0.0; 11];
        for (o, (db, _)) in out.iter_mut().zip(SCALE) {
            *o = crate::widgets::fader::pos_of(db);
        }
        out
    })
}

/// The scale the legend prints beside the faders.
const SCALE: [(f32, &str); 11] = [
    (12.0, "+12"),
    (6.0, "+6"),
    (0.0, "0"),
    (-6.0, "\u{2212}6"),
    (-12.0, "\u{2212}12"),
    (-18.0, "\u{2212}18"),
    (-24.0, "\u{2212}24"),
    (-30.0, "\u{2212}30"),
    (-40.0, "\u{2212}40"),
    (-60.0, "\u{2212}60"),
    (cantus_core::MIN_GAIN_DB, "\u{2212}\u{221e}"),
];

/// The legend column: each row named once for the whole desk, the faders' dB scale
/// beside the fader row, and the 3-dot handle that sets the meter : fader split.
pub(super) fn legend(
    v: &mut MixerView,
    ui: &mut Ui,
    b: &mut Brush,
    s: &Style,
    geo: &Geo,
    fit: &Fit,
) {
    let t = &s.t;
    b.register(geo.legend, "mixer_legend", LEGEND_TOKENS, &[]);
    let l = geo.legend;
    let x0 = l.left() + 8.0;
    let x1 = l.right() - 6.0;
    for r in Row::ALL {
        let Some((a, z)) = fit.row(r) else {
            continue;
        };
        let (a, z) = (geo.y0 + a, geo.y0 + z);
        let box_ = if z - a > 30.0 {
            Rect::from_min_max(pos2(x0, a), pos2(x1, a + 14.0))
        } else {
            Rect::from_center_size(pos2((x0 + x1) * 0.5, (a + z) * 0.5), vec2(x1 - x0, 14.0))
        };
        let tt = Text::key(r.legend_key(), Role::Micro, t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Abbreviate);
        let _ = text::paint(b, box_, &tt, s.tk());
    }
    cables_key(ui, b, s, geo, fit);
    // The dB scale, where it does not collide with itself or with the row's name.
    if let Some((a, z)) = fit.row(Row::Fader) {
        let fr = Rect::from_min_max(pos2(x0, geo.y0 + a), pos2(x1, geo.y0 + z));
        let cap = crate::kit::fader::CAP.y * s.k;
        let travel = (fr.height() - cap).max(1.0);
        let mut last = f32::NEG_INFINITY;
        let name_bottom = fr.top() + 15.0;
        for (i, p) in ticks().iter().enumerate() {
            let y = fr.bottom() - cap * 0.5 - travel * p;
            // Keep the ends, drop what would touch its neighbour.
            let bottom_end = i == SCALE.len() - 1;
            if y - last < 11.0 && !bottom_end {
                continue;
            }
            if bottom_end && y - last < 11.0 {
                continue;
            }
            let lr = Rect::from_min_max(pos2(x1 - 30.0, y - 6.5), pos2(x1, y + 6.5));
            if lr.top() < name_bottom && lr.left() < x0 + 40.0 {
                continue;
            }
            let tt = Text::new(SCALE[i].1, Role::Micro, t.text_dim)
                .caps(false)
                .align(Align2::RIGHT_CENTER);
            let _ = text::paint(b, lr, &tt, s.tk());
            last = y;
        }
    }
    ratio_handle(v, ui, b, s, geo, fit);
    band_handle(v, ui, b, s, geo, fit);
}

/// Under ROUTING, what the band's lines mean, without a hover: a sample of each — an
/// output thick, a send thin, a sidechain key dashed, as the band draws them — and its
/// word; on hover, the whole sentence in the hint bar. Where the band is too short for
/// three lines, only the hover says it.
fn cables_key(ui: &mut Ui, b: &mut Brush, s: &Style, geo: &Geo, fit: &Fit) {
    let Some((a, z)) = fit.row(Row::Cables) else {
        return;
    };
    let t = &s.t;
    let (a, z) = (geo.y0 + a, geo.y0 + z);
    let l = geo.legend;
    let (x0, x1) = (l.left() + 8.0, l.right() - 6.0);
    // Below the row's name (which sits at the top of a tall row, centred in a short one).
    let top = if z - a > 30.0 {
        a + 13.0
    } else {
        (a + z) * 0.5 + 7.0
    };
    // Micro is capitals 8.5 pt (about 6 px tall): 9 px a line keeps them apart, so the
    // name and the three lines fit the band's default 40 px.
    const LINE: f32 = 9.0;
    if z - top >= LINE * 3.0 {
        use crate::kit::cable::{self, CableKind};
        let ink = t.text_dim;
        for (i, (key, kind)) in [
            ("cables.legend.output", CableKind::Output),
            (
                "cables.legend.send",
                CableKind::Send {
                    on: true,
                    pre: false,
                },
            ),
            ("cables.legend.key", CableKind::Key),
        ]
        .into_iter()
        .enumerate()
        {
            let y = top + LINE * (i as f32 + 0.5);
            // As the band strokes it: its width, and a key in the dynamics hue, dashed,
            // ending in its key mark.
            let w = s.s(cable::width(kind));
            if kind == CableKind::Key {
                let hue = t.hue_dynamics;
                for k in 0..2 {
                    let u = x0 + k as f32 * 5.0;
                    b.segment(pos2(u, y), pos2(u + 3.0, y), w, hue, false);
                }
                cable::key_mark(b, s, pos2(x0 + 13.0, y), s.s(8.0), hue);
            } else {
                b.segment(pos2(x0, y), pos2(x0 + 14.0, y), w, ink, false);
            }
            // Taller than the line, so an accent (ENVÍO) is not cut; nothing paints above.
            let tr = Rect::from_min_max(pos2(x0 + 21.0, y - 6.5), pos2(x1, y + 6.5));
            let tt = Text::key(key, Role::Micro, ink)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Abbreviate);
            let _ = text::paint(b, tr, &tt, s.tk());
        }
    }
    let row = Rect::from_min_max(pos2(l.left(), a), pos2(l.right(), z));
    let resp = ui.interact(
        row.intersect(geo.rows),
        Id::new("mixer-cables-key"),
        Sense::hover(),
    );
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            &crate::i18n::t("cables.legend.hint"),
            "",
            hint::Caps::NONE,
        );
    }
}

/// FL's 3-dot handle between the meter and the fader: drag to move height between them,
/// double-click to reset the split.
fn ratio_handle(v: &mut MixerView, ui: &mut Ui, b: &mut Brush, s: &Style, geo: &Geo, fit: &Fit) {
    let (Some((ma, mz)), Some((fa, fz))) = (fit.row(Row::Meter), fit.row(Row::Fader)) else {
        return;
    };
    let flex = (mz - ma) + (fz - fa);
    let y = geo.y0 + mz - 6.0;
    let hr = Rect::from_center_size(pos2(geo.legend.right() - 14.0, y), vec2(22.0, 12.0));
    let resp = ui.interact(
        hr.intersect(geo.rows),
        Id::new("mixer-ratio"),
        Sense::click_and_drag(),
    );
    let st = State::of(&resp, false);
    if st.hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        hint::set_text(ui.ctx(), &v.words.ratio, "", hint::Caps::NONE);
    }
    let mut p = v.prefs.current;
    if resp.dragged() {
        let meter = (mz - ma) + resp.drag_delta().y;
        p.ratio = layout::ratio_for(meter, flex);
        // Kept while dragging (the prefs write only when the value moves).
        v.prefs.current = p;
    }
    if resp.drag_stopped() {
        v.prefs.set(p);
        v.prefs.save();
    }
    if resp.double_clicked() {
        p.ratio = layout::DEFAULT_RATIO;
        v.prefs.set(p);
    }
    let dot = if st.hovered || st.active {
        s.t.text
    } else {
        s.t.text_dim
    };
    for i in -1..=1 {
        b.circle(
            pos2(hr.center().x + i as f32 * 4.0, hr.center().y),
            1.2,
            Fill::Solid(dot),
        );
    }
}

/// The wheel over the desk (ADR 0024 §6): Ctrl steps the width mode (a fader or a knob
/// under the pointer takes it instead), Shift moves the strip under the pointer along the
/// desk, and the plain wheel scrolls — sideways over the numbers, names and meters,
/// vertically elsewhere when the desk is taller than its panel.
pub(super) fn wheel(v: &mut MixerView, app: &mut CantusApp, ui: &mut Ui, geo: &Geo, fit: &Fit) {
    v.ctrl_notches = 0.0;
    let over = ui.rect_contains_pointer(geo.rows.union(geo.hbar.expand(3.0)));
    let (smooth, notches, mods) = ui.input(|i| {
        let mut n = 0.0f32;
        for e in &i.events {
            if let egui::Event::MouseWheel { unit, delta, .. } = e {
                let d = delta.x + delta.y;
                n += match unit {
                    egui::MouseWheelUnit::Line => d,
                    egui::MouseWheelUnit::Point => d / 40.0,
                    egui::MouseWheelUnit::Page => d * 3.0,
                };
            }
        }
        (i.smooth_scroll_delta, n, i.modifiers)
    });
    if !over || (smooth == egui::Vec2::ZERO && notches == 0.0) {
        return;
    }
    let Some(p) = ui.ctx().pointer_hover_pos() else {
        return;
    };
    let row_at = |r: Row| {
        fit.row(r)
            .is_some_and(|(a, z)| p.y >= geo.y0 + a && p.y <= geo.y0 + z)
    };
    let on_control = row_at(Row::Fader)
        || row_at(Row::Pan)
        || row_at(Row::Width)
        || row_at(Row::Eq)
        || row_at(Row::Sends)
        || row_at(Row::SendList);
    let in_legend = geo.legend.contains(p);
    let used;
    if mods.command {
        if on_control && !in_legend {
            // The control under the pointer takes the notches.
            v.ctrl_notches = notches;
        } else if notches != 0.0 {
            v.notch_acc += notches;
            while v.notch_acc.abs() >= 1.0 {
                let dir = v.notch_acc.signum();
                v.notch_acc -= dir;
                let mut pr = v.prefs.current;
                pr.width = pr.width.step(dir as i32);
                v.prefs.set(pr);
            }
        }
        used = true;
    } else if mods.shift {
        if let Some(id) = strip_at(v, geo, p) {
            v.notch_acc += notches;
            while v.notch_acc.abs() >= 1.0 {
                let dir = v.notch_acc.signum();
                v.notch_acc -= dir;
                // The wheel down (towards you) moves the strip right, as a page scrolls.
                app.move_strip(id, if dir < 0.0 { 1 } else { -1 });
            }
        }
        used = true;
    } else {
        let head =
            row_at(Row::Number) || row_at(Row::Name) || row_at(Row::Meter) || row_at(Row::Cables);
        let on_hbar = geo.hbar.expand(3.0).contains(p);
        if fit.scrolls && !head && !on_hbar {
            v.scroll_y -= smooth.y;
            v.scroll_x -= smooth.x;
        } else {
            v.scroll_x -= smooth.y + smooth.x;
        }
        used = smooth != egui::Vec2::ZERO;
    }
    if used {
        ui.ctx()
            .input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
    }
}

/// The strip under `p`, in any dock.
pub(super) fn strip_at(v: &MixerView, geo: &Geo, p: egui::Pos2) -> Option<cantus_core::StripId> {
    if !geo.rows.contains(p) {
        return None;
    }
    let find = |items: &[desk::Item], mut x: f32| {
        for item in items {
            let w = desk::item_width(item, geo.pitch);
            if p.x >= x && p.x < x + w {
                return match item {
                    desk::Item::Strip(id, _) => Some(*id),
                    desk::Item::Separator(_) => None,
                };
            }
            x += w;
        }
        None
    };
    if geo.middle.contains(p) {
        find(&v.desk.middle, geo.middle.left() - v.scroll_x)
    } else if geo.right.contains(p) {
        find(&v.desk.right, geo.right.left())
    } else if geo.left.contains(p) {
        find(&v.desk.left, geo.left.left() + geo.master_w)
    } else {
        None
    }
}

/// What the title row's right side needs: the window's title and the desk-wide keys
/// that show (FL's "Mixer"; *Solo off*, *Unmute all*), so the left side gives way first.
fn right_side_width(v: &MixerView, ui: &Ui, s: &Style) -> f32 {
    let measure = |key: &'static str, role: Role, caps: bool| {
        text::measure(ui.ctx(), &Text::key(key, role, s.t.text).caps(caps), s.tk()).x
    };
    let mut w = 10.0 + measure("kit.mixer.title", Role::Title, false) + 12.0 + 24.0;
    if v.desk.any_solo {
        w += measure("mixer.solo_off", Role::Label, true) + 36.0;
    }
    if v.desk.any_muted {
        w += measure("mixer.unmute_all", Role::Label, true) + 36.0;
    }
    w
}

/// The title row's cable view (round 25, the owner's answer 1): the cables glyph and a
/// three-way switch — *Off · Selected · All*, the selection's by default. Where the row
/// has no room for the switch (a narrow desk, a large UI scale), the glyph alone is the
/// key, and a click steps to the next view. Returns the new left edge.
fn cable_view(v: &mut MixerView, ui: &Ui, b: &mut Brush, s: &Style, r: Rect, x: f32) -> f32 {
    let t = &s.t;
    let cy = r.center().y;
    let view = v.prefs.current.cables;
    let lit = view != layout::CableView::Off;
    let room = r.right() - x - right_side_width(v, ui, s);
    if room < 190.0 {
        let kr = Rect::from_min_size(pos2(x, cy - 10.0), vec2(20.0, 20.0));
        let resp = key(ui, kr, "cables");
        let st = State::of(&resp, lit);
        cap::toolbar_key(b, kr, s, st, cap::KeyFace::Text(""));
        let ink = if lit {
            t.highlight_glyph
        } else if st.hovered {
            t.text
        } else {
            t.text_dim
        };
        glyph::paint(b, kr.shrink(4.0), Glyph::Cables, ink);
        if resp.hovered() {
            let name = crate::i18n::t(view.key());
            hint::set_text(ui.ctx(), &v.words.cable_view, &name, hint::Caps::NONE);
        }
        if resp.clicked() {
            let mut p = v.prefs.current;
            p.cables = layout::CableView::from_index((view.index() + 1) % 3);
            v.prefs.set(p);
        }
        return kr.right() + 8.0;
    }
    let gr = Rect::from_min_size(pos2(x, cy - 8.0), vec2(16.0, 16.0));
    glyph::paint(b, gr, Glyph::Cables, if lit { t.text } else { t.text_dim });
    let labels = [
        Src::Key(layout::CableView::Off.key()),
        Src::Key(layout::CableView::Selected.key()),
        Src::Key(layout::CableView::All.key()),
    ];
    let wr = Rect::from_min_size(pos2(gr.right() + 4.0, cy - 10.0), vec2(168.0, 20.0));
    let resp = key(ui, gr.union(wr), "cables");
    let hovered = resp
        .hover_pos()
        .and_then(|p| segmented::hit(wr, labels.len(), p.x));
    if resp.clicked() {
        if let Some(i) = hovered {
            let mut p = v.prefs.current;
            p.cables = layout::CableView::from_index(i);
            v.prefs.set(p);
        }
    }
    if resp.hovered() {
        hint::set_text(ui.ctx(), &v.words.cable_view, "", hint::Caps::NONE);
    }
    let _ = resp.on_hover_text(&v.words.cable_view);
    segmented::paint(
        b,
        wr,
        &s.hue_of(Tok::Highlight),
        State::REST,
        &labels,
        view.index(),
        hovered,
    );
    wr.right() + 10.0
}

/// The cable band's handle (round 25): a 3-dot grip at the top of its legend cell; drag
/// to make the band taller or shorter (20 to 64 pt), double-click for the default.
fn band_handle(v: &mut MixerView, ui: &mut Ui, b: &mut Brush, s: &Style, geo: &Geo, fit: &Fit) {
    let Some((a, _)) = fit.row(Row::Cables) else {
        return;
    };
    let y = geo.y0 + a + 5.0;
    let hr = Rect::from_center_size(pos2(geo.legend.right() - 14.0, y), vec2(22.0, 10.0));
    let resp = ui.interact(
        hr.intersect(geo.rows),
        Id::new("mixer-band-size"),
        Sense::click_and_drag(),
    );
    let st = State::of(&resp, false);
    if st.hovered || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        hint::set_text(ui.ctx(), &v.words.band, "", hint::Caps::NONE);
    }
    let mut p = v.prefs.current;
    if resp.dragged() {
        // The band is the bottom row: its top edge up makes it taller.
        p.band = (p.band - resp.drag_delta().y).clamp(layout::BAND_MIN, layout::BAND_MAX);
        v.prefs.current = p;
    }
    if resp.drag_stopped() {
        v.prefs.set(p);
        v.prefs.save();
    }
    if resp.double_clicked() {
        p.band = layout::BAND;
        v.prefs.set(p);
    }
    let dot = if st.hovered || st.active {
        s.t.text
    } else {
        s.t.text_dim
    };
    for i in -1..=1 {
        b.circle(
            pos2(hr.center().x + i as f32 * 4.0, hr.center().y),
            1.2,
            Fill::Solid(dot),
        );
    }
}
