//! The **More** panel (round 39, `dir-a`'s phase B, `overflow2.py`): what the bar folded,
//! laid out as a sheet rather than a list.
//!
//! - Each large folded group is a **section** — Metronome, Windows, Performance, Inputs,
//!   Tools and zoom — with its title (11 SemiBold, ink 3, title case). The small folds
//!   (Pat / Song, the time signature of a compact tempo, the grid of a compact snap, undo,
//!   the master meter, and any other group a very narrow bar folded) are one-line rows
//!   under **Also here**: the label at the left (11, ink 2), the control at 100.
//! - Up to three sections share one column 284 wide; from four on there are two, balanced
//!   by height, 24 apart with a hairline between, so the panel never outgrows the window.
//! - Every shortcut is a keycap — an outline, 9 Medium, ink 3 — under or beside its key.
//! - Performance is four cells, each a small graph with its number: CPU (amber when busy),
//!   memory, voices, dropouts (red ticks, or "None — keeping time").
//! - The foot: *Customise top bar…*, a line, not a button.
//!
//! The panel is `raised`, corner 8, one soft shadow and a nub under the More key.

use super::layout::{Group, Layout};
use super::paint::{self as p, ink1, ink2, ink3};
use super::{keys, meter, metro, perf, snap, tempo, Style};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::keymap::Action;
use crate::kit::color::{mix, opaque};
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src};
use crate::kit::{self, with_brush, Brush, Fill, State};
use egui::{pos2, vec2, Align2, Id, Order, Pos2, Rect, Sense, Ui};
use std::fmt::Write as _;

/// A column's width, the panel's padding, the gutter between columns, a row's pitch.
const CW: f32 = 284.0;
const PAD: f32 = 16.0;
const GUT: f32 = 24.0;
const ROW: f32 = 26.0;
/// The space between two sections (a hairline 15 above the next).
const SEC_GAP: f32 = 29.0;

/// One part of the panel.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Part {
    Section(Group),
    Also,
}

/// A row under *Also here*.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Extra {
    /// A folded group shown whole on its row (Pat / Song, undo, the meter…).
    Group(Group),
    /// The metre of a compact (or folded) tempo.
    Metre,
    /// The grid of a compact snap.
    Grid,
}

/// Whether a folded group is a section of its own.
fn is_section(g: Group) -> bool {
    matches!(
        g,
        Group::Metronome | Group::Windows | Group::Performance | Group::Inputs | Group::Tools
    )
}

/// A section's height at 100 %.
fn height(g: Group) -> f32 {
    match g {
        Group::Metronome => 36.0 + 4.0 * ROW,
        Group::Windows => 18.0 + 54.0 + 16.0,
        Group::Performance => 18.0 + 104.0,
        Group::Inputs => 18.0 + ROW + 22.0,
        _ => 18.0 + 30.0 + 24.0,
    }
}

/// A row's height under *Also here*: a group drawn whole takes the bar's height.
fn extra_h(e: Extra) -> f32 {
    match e {
        Extra::Group(g) if !matches!(g, Group::Mode | Group::Undo | Group::Meter | Group::Snap) => {
            super::HEIGHT
        }
        _ => ROW,
    }
}

/// The rows under *Also here*, in the bar's order, at most `COUNT + 2`.
fn extras(l: &Layout, out: &mut [Option<Extra>; 17]) -> usize {
    let mut n = 0;
    let mut push = |e: Extra, n: &mut usize| {
        if *n < out.len() {
            out[*n] = Some(e);
            *n += 1;
        }
    };
    for g in l.folded() {
        match g {
            g if is_section(*g) => {}
            Group::Tempo => {
                push(Extra::Group(Group::Tempo), &mut n);
            }
            g => push(Extra::Group(*g), &mut n),
        }
    }
    // The pieces the compact forms left: the metre, the grid.
    if l.is_compact(Group::Tempo) {
        push(Extra::Metre, &mut n);
    }
    if l.is_compact(Group::Snap) {
        push(Extra::Grid, &mut n);
    }
    n
}

/// Show the panel under the More key `key`; returns its rect.
pub fn show(app: &mut CantusApp, ctx: &egui::Context, key: Rect, l: &Layout, s: &Style) -> Rect {
    let k = s.k;
    let mut ex = [None; 17];
    let nex = extras(l, &mut ex);
    // The parts and their heights; then the columns, the shorter first.
    let mut parts: [(Part, f32); 8] = [(Part::Also, 0.0); 8];
    let mut np = 0;
    for g in l.folded() {
        if is_section(*g) && np < 7 {
            parts[np] = (Part::Section(*g), height(*g) * k);
            np += 1;
        }
    }
    if nex > 0 {
        let h: f32 = ex[..nex].iter().flatten().map(|e| extra_h(*e)).sum();
        parts[np] = (Part::Also, (18.0 + h) * k);
        np += 1;
    }
    let ncol = if np <= 3 { 1 } else { 2 };
    let mut col_of = [0usize; 8];
    let mut hs = [0f32; 2];
    for i in 0..np {
        let c = if ncol == 2 && hs[1] < hs[0] { 1 } else { 0 };
        col_of[i] = c;
        hs[c] += parts[i].1 + SEC_GAP * k;
    }
    let body_h = hs[0].max(hs[1]).max(SEC_GAP * k);
    let w = (2.0 * PAD + ncol as f32 * CW + (ncol - 1) as f32 * GUT) * k;
    let screen = ctx.content_rect();
    let top = key.bottom() + 9.0 * k;
    let x = (key.right() + 8.0 * k - w).max(screen.left() + 8.0 * k);
    let h = 46.0 * k + body_h - 14.0 * k + 40.0 * k;
    let rect = Rect::from_min_size(pos2(x, top), vec2(w, h));
    let mut drawn: Vec<(Group, Rect)> = Vec::new();
    egui::Area::new(Id::new("toolbar-overflow"))
        // Under the popovers it opens (they are foreground layers), over the workspace.
        .order(Order::Middle)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(rect.size(), Sense::click());
            let _ = crate::views::hint::said_with(resp, || t("topbar.panel.hint"));
            with_brush(ui.painter(), |b| frame(b, rect, key, s));
            // The title and why.
            let head = Rect::from_min_size(
                pos2(rect.left() + PAD * k, rect.top() + 14.0 * k),
                vec2(w - 2.0 * PAD * k, 16.0 * k),
            );
            let n = l.folded().len();
            let why = crate::i18n::tf_count("topbar.panel.why", n, &[]);
            with_brush(ui.painter(), |b| {
                p::words_key(
                    b,
                    head,
                    "topbar.panel.title",
                    Role::Title,
                    13.0,
                    ink1(s),
                    Align2::LEFT_CENTER,
                    s,
                );
                p::words(
                    b,
                    head,
                    &why,
                    Role::Label,
                    11.0,
                    ink3(s),
                    Align2::RIGHT_CENTER,
                    s,
                );
            });
            let y0 = rect.top() + 46.0 * k;
            let mut ys = [y0; 2];
            let line = mix(s.t.menu, p::ink4(s), 0.45);
            for i in 0..np {
                let c = col_of[i];
                let cx = rect.left() + (PAD + c as f32 * (CW + GUT)) * k;
                let y = ys[c];
                if y > y0 {
                    with_brush(ui.painter(), |b| {
                        b.hline(cx, cx + CW * k, b.snap_mid(y - 15.0 * k), b.px(), line)
                    });
                }
                let area = Rect::from_min_size(pos2(cx, y), vec2(CW * k, parts[i].1));
                match parts[i].0 {
                    Part::Section(g) => {
                        section(app, ui, g, area, s);
                        drawn.push((g, area));
                    }
                    Part::Also => also(app, ui, area, &ex[..nex], s, &mut drawn),
                }
                ys[c] += parts[i].1 + SEC_GAP * k;
            }
            if ncol == 2 {
                let gx = rect.left() + (PAD + CW + GUT * 0.5) * k;
                with_brush(ui.painter(), |b| {
                    b.vline(b.snap_mid(gx), y0, y0 + body_h - SEC_GAP * k, b.px(), line)
                });
            }
            // The foot: Customise top bar…, a line.
            let fy = y0 + body_h - 14.0 * k;
            let foot = Rect::from_min_max(pos2(rect.left(), fy), rect.max);
            let fresp = ui.interact(foot, Id::new("toolbar-overflow-customise"), Sense::click());
            with_brush(ui.painter(), |b| {
                b.hline(rect.left(), rect.right(), b.snap_mid(fy), b.px(), line);
                if fresp.hovered() {
                    b.rrect(
                        b.snap_rect(foot.shrink(2.0 * k)),
                        s.r(6.0),
                        Fill::Solid(mix(s.t.menu, s.t.text, 0.05)),
                    );
                }
                let inner = foot.shrink2(vec2(PAD * k, 0.0));
                let g = Rect::from_center_size(
                    pos2(inner.left() + 7.0 * k, foot.center().y),
                    vec2(20.0, 20.0) * k,
                );
                p::icon(
                    b,
                    g,
                    s,
                    Glyph::Sliders,
                    State::of(&fresp, false),
                    s.t.highlight,
                    20.0,
                );
                let wr = Rect::from_min_max(pos2(inner.left() + 22.0 * k, foot.top()), inner.max);
                p::words_key(
                    b,
                    wr,
                    "topbar.customise",
                    Role::Body,
                    12.0,
                    ink1(s),
                    Align2::LEFT_CENTER,
                    s,
                );
                p::words_key(
                    b,
                    wr,
                    "topbar.panel.customise_what",
                    Role::Label,
                    11.0,
                    ink3(s),
                    Align2::RIGHT_CENTER,
                    s,
                );
            });
            if fresp.hovered() {
                hint::set_text(ui.ctx(), &t("topbar.customise"), "", Caps::NONE);
            }
            if fresp.clicked() {
                super::more::open_customise(app);
                app.topbar.customise.overflow = false;
            }
        });
    ctx.data_mut(|d| d.insert_temp(Id::new("cantus-topbar-overflow"), drawn));
    rect
}

/// The panel's ground: `raised`, corner 8, a soft shadow, a lit edge, the nub under `key`.
fn frame(b: &mut Brush, rect: Rect, key: Rect, s: &Style) {
    let k = s.k;
    let r = s.r(8.0);
    b.rrect_soft(
        rect.translate(vec2(0.0, 8.0 * k)),
        r,
        Fill::Solid(s.t.shadow),
        14.0 * k,
    );
    b.rrect(b.snap_rect(rect), r, Fill::Solid(s.t.menu));
    let edge = if s.t.dark {
        mix(s.t.menu, s.t.text, 0.08)
    } else {
        opaque(s.t.inner_shadow)
    };
    b.rrect_stroke(b.snap_rect(rect).shrink(b.px() * 0.5), r, b.px(), edge);
    let kx = key.center().x;
    let nub = [
        pos2(kx - 7.0 * k, rect.top() + 1.0 * k),
        pos2(kx, rect.top() - 6.0 * k),
        pos2(kx + 7.0 * k, rect.top() + 1.0 * k),
    ];
    b.polygon(&nub, s.t.menu);
}

/// A section's title at the top of `area` (11 SemiBold, ink 3), and an optional note or
/// keycap at its right.
fn title(b: &mut Brush, area: Rect, key: &str, right: Option<&str>, s: &Style) {
    let r = Rect::from_min_size(area.min, vec2(area.width(), 13.0 * s.k));
    p::words_key(
        b,
        r,
        key,
        Role::Section,
        11.0,
        ink3(s),
        Align2::LEFT_CENTER,
        s,
    );
    if let Some(w) = right {
        p::words(b, r, w, Role::Label, 10.0, ink3(s), Align2::RIGHT_CENTER, s);
    }
}

/// A keycap: an outline 13 high, the key's words 9 Medium in ink 3, centred on `cx` (or
/// ending at `cx` when `right`).
fn keycap(b: &mut Brush, cx: f32, top: f32, words: &str, right: bool, s: &Style) -> Rect {
    let k = s.k;
    let w = p::measure(b.ctx(), words, Role::Control, 9.0, s) + 8.0 * k;
    let x0 = if right { cx - w } else { cx - w * 0.5 };
    let r = Rect::from_min_size(pos2(x0, top), vec2(w, 13.0 * k));
    b.rrect_stroke(
        b.snap_rect(r).shrink(b.px() * 0.5),
        2.5 * k,
        b.px(),
        mix(s.t.menu, ink3(s), 0.6),
    );
    p::words(
        b,
        r,
        words,
        Role::Control,
        9.0,
        ink3(s),
        Align2::CENTER_CENTER,
        s,
    );
    r
}

/// The first key bound to `action`, as words.
fn binding(app: &CantusApp, action: Action) -> Option<String> {
    app.keymap.bindings_for(action).first().map(|b| b.display())
}

/// A child `Ui` placed at `r` (a kit widget's place in a row).
fn child(ui: &mut Ui, r: Rect) -> Ui {
    ui.new_child(
        egui::UiBuilder::new()
            .max_rect(r)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    )
}

/// One section in `area`.
fn section(app: &mut CantusApp, ui: &mut Ui, g: Group, area: Rect, s: &Style) {
    match g {
        Group::Metronome => metronome(app, ui, area, s),
        Group::Windows => windows(app, ui, area, s),
        Group::Performance => performance(app, ui, area, s),
        Group::Inputs => inputs(app, ui, area, s),
        _ => tools(app, ui, area, s),
    }
}

/// A small switch 24 × 14 at `r`: on, the accent with its knob at the right.
fn switch(b: &mut Brush, r: Rect, on: bool, hovered: bool, s: &Style) {
    let k = s.k;
    let r = b.snap_rect(r);
    if on {
        b.rrect(r, r.height() * 0.5, Fill::Solid(s.t.highlight));
    } else {
        kit::cap::well_body(b, r, r.height() * 0.5, s, s.t.well, 1.0);
    }
    let c = if on {
        pos2(r.right() - 7.0 * k, r.center().y)
    } else {
        pos2(r.left() + 7.0 * k, r.center().y)
    };
    let knob = if on {
        s.t.highlight_glyph
    } else if hovered {
        ink1(s)
    } else {
        ink2(s)
    };
    b.circle(c, 4.5 * k, Fill::Solid(knob));
}

/// The metronome: its key, its name and what it does, its shortcut and its switch; then
/// the count-in, the level, the sound and the accent (quiet while it is off).
fn metronome(app: &mut CantusApp, ui: &mut Ui, area: Rect, s: &Style) {
    let k = s.k;
    let (x, y) = (area.left(), area.top());
    let on = app.project().metronome;
    let kr = Rect::from_min_size(pos2(x, y), vec2(28.0, 24.0) * k);
    let sw = Rect::from_min_size(
        pos2(area.right() - 26.0 * k, y + 5.0 * k),
        vec2(24.0, 14.0) * k,
    );
    let kresp = ui.interact(kr, Id::new("panel-metronome-key"), Sense::click());
    let sresp = ui.interact(sw, Id::new("panel-metronome-switch"), Sense::click());
    let keys_w = binding(app, Action::ToggleMetronome);
    with_brush(ui.painter(), |b| {
        let st = State::of(&kresp, on);
        p::well(b, kr, s, None);
        p::tint(b, kr, s, st, s.t.highlight, 0.2);
        p::icon(b, kr, s, Glyph::Metronome, st, s.t.highlight, 24.0);
        let tr = Rect::from_min_max(
            pos2(x + 38.0 * k, y),
            pos2(sw.left() - 70.0 * k, y + 14.0 * k),
        );
        p::words_key(
            b,
            tr,
            "topbar.group.metronome",
            Role::Control,
            12.0,
            ink1(s),
            Align2::LEFT_CENTER,
            s,
        );
        let sr = Rect::from_min_max(
            pos2(x + 38.0 * k, y + 14.0 * k),
            pos2(sw.left() - 8.0 * k, y + 27.0 * k),
        );
        p::words_key(
            b,
            sr,
            "topbar.panel.metronome_what",
            Role::Label,
            10.0,
            ink3(s),
            Align2::LEFT_CENTER,
            s,
        );
        if let Some(w) = &keys_w {
            keycap(b, sw.left() - 8.0 * k, y + 5.0 * k, w, true, s);
        }
        switch(b, sw, on, sresp.hovered(), s);
    });
    if kresp.hovered() || sresp.hovered() {
        hint::set_text(
            ui.ctx(),
            &super::tip(app, "transport.metronome", Some(Action::ToggleMetronome)),
            "",
            Caps::NONE,
        );
    }
    if kresp.clicked() || sresp.clicked() {
        app.run_action(Action::ToggleMetronome);
    }
    let mut rs = app.project().recording.clone();
    let before = rs.clone();
    let mut done = false;
    let rows = [
        "topbar.panel.count_in",
        "rec.click_level",
        "topbar.click_sound",
        "topbar.accent",
    ];
    for (i, label) in rows.iter().enumerate() {
        let yy = y + 36.0 * k + i as f32 * ROW * k;
        let lr = Rect::from_min_size(pos2(x, yy), vec2(66.0 * k, 22.0 * k));
        let ink = if on { ink2(s) } else { ink3(s) };
        with_brush(ui.painter(), |b| {
            p::words_key(b, lr, label, Role::Label, 11.0, ink, Align2::LEFT_CENTER, s)
        });
        let cr = Rect::from_min_max(pos2(x + 68.0 * k, yy), pos2(area.right(), yy + 22.0 * k));
        let mut c = child(ui, cr);
        if !on {
            c.multiply_opacity(0.45);
        }
        match i {
            0 => {
                let mut sel = metro::COUNT_INS
                    .iter()
                    .position(|(b, _)| *b == rs.count_in_bars)
                    .unwrap_or(0);
                let labels = metro::COUNT_INS.map(|(_, k)| Src::Key(k));
                if kit::widgets::segmented(
                    &mut c,
                    &mut sel,
                    &labels,
                    &t("rec.count_in"),
                    cr.width() - 4.0 * k,
                )
                .changed()
                {
                    rs.count_in_bars = metro::COUNT_INS[sel].0;
                }
            }
            1 => {
                let (v, ended) = crate::views::record_options::level(&mut c, rs.click_db, s);
                rs.click_db = v;
                done |= ended;
            }
            2 => {
                let mut sel = cantus_core::ClickSound::ALL
                    .iter()
                    .position(|c| *c == rs.click_sound)
                    .unwrap_or(0);
                let labels = [
                    Src::Key("topbar.sound.classic"),
                    Src::Key("topbar.sound.wood"),
                    Src::Key("topbar.sound.beep"),
                ];
                if kit::widgets::segmented(
                    &mut c,
                    &mut sel,
                    &labels,
                    &t("topbar.click_sound"),
                    cr.width() - 4.0 * k,
                )
                .changed()
                {
                    rs.click_sound = cantus_core::ClickSound::ALL[sel];
                }
            }
            _ => {
                let mut a = rs.click_accent;
                if kit::widgets::led_button(
                    &mut c,
                    &mut a,
                    Src::Key("topbar.accent_on"),
                    &t("topbar.accent_tip"),
                    cr.width() - 4.0 * k,
                )
                .changed()
                {
                    rs.click_accent = a;
                }
            }
        }
    }
    if rs != before {
        app.set_record_settings(rs);
    }
    if done {
        app.end_gesture();
    }
}

/// The windows' keys, each with its shortcut under it, and the hovered key's words.
fn windows(app: &mut CantusApp, ui: &mut Ui, area: Rect, s: &Style) {
    let k = s.k;
    let (x, y) = (area.left(), area.top());
    with_brush(ui.painter(), |b| {
        title(b, area, "topbar.group.windows", None, s)
    });
    let pitch = (CW + 6.0) / 7.0 * k;
    let kw = (pitch - 6.0 * k).round();
    let top = y + 18.0 * k;
    let cells: [Rect; 7] = std::array::from_fn(|i| {
        Rect::from_min_size(
            pos2((x + i as f32 * pitch).round(), top),
            vec2(kw, 30.0 * k),
        )
    });
    keys::windows_in(app, ui, &cells, 28.0, s);
    let hover = ui
        .ctx()
        .pointer_hover_pos()
        .and_then(|p| cells.iter().position(|c| c.contains(p)));
    with_brush(ui.painter(), |b| {
        for (i, c) in cells.iter().enumerate() {
            let (_, _, action, _) = keys::window_key(app, keys::WINDOWS[i]);
            if let Some(w) = binding(app, action) {
                keycap(b, c.center().x, c.bottom() + 5.0 * k, &w, false, s);
            }
        }
        let line = Rect::from_min_size(pos2(x, top + 54.0 * k), vec2(area.width(), 14.0 * k));
        match hover {
            Some(i) => {
                let (_, _, _, key) = keys::window_key(app, keys::WINDOWS[i]);
                let words = t(key);
                let (name, rest) = words
                    .split_once(": ")
                    .or_else(|| words.split_once(" \u{2014} "))
                    .unwrap_or((&words, ""));
                let nw = p::measure(b.ctx(), name, Role::Control, 11.0, s);
                p::words(
                    b,
                    line,
                    name,
                    Role::Control,
                    11.0,
                    ink1(s),
                    Align2::LEFT_CENTER,
                    s,
                );
                if !rest.is_empty() {
                    let rr =
                        Rect::from_min_max(pos2(line.left() + nw + 6.0 * k, line.top()), line.max);
                    let rest = format!("\u{2014} {rest}");
                    p::words(
                        b,
                        rr,
                        &rest,
                        Role::Label,
                        11.0,
                        ink3(s),
                        Align2::LEFT_CENTER,
                        s,
                    );
                }
            }
            None => p::words_key(
                b,
                line,
                "topbar.panel.windows_idle",
                Role::Label,
                11.0,
                ink3(s),
                Align2::LEFT_CENTER,
                s,
            ),
        }
    });
}

/// A small graph of the last `n` readings of `ring` in `r`: a line over a filled area, or
/// bars; `top` is the reading at the top.
#[allow(clippy::too_many_arguments)]
fn graph(
    b: &mut Brush,
    r: Rect,
    h: &perf::History,
    ring: &[f32; perf::HISTORY],
    top: f32,
    bars: bool,
    ink: egui::Color32,
    s: &Style,
) {
    const N: usize = 40;
    let inner = r.shrink2(vec2(4.0 * s.k, 4.0 * s.k));
    if bars {
        let n = 24usize;
        let w = inner.width() / n as f32;
        for i in 0..n {
            let v = (h.at(ring, n, i) / top.max(1e-3)).clamp(0.0, 1.0);
            let bh = (inner.height() * v).max(1.0 * s.k);
            let br = Rect::from_min_max(
                pos2(inner.left() + i as f32 * w, inner.bottom() - bh),
                pos2(
                    inner.left() + (i as f32 + 1.0) * w - 1.0 * s.k,
                    inner.bottom(),
                ),
            );
            b.rect(b.snap_rect(br), Fill::Solid(ink.gamma_multiply(0.7)));
        }
        return;
    }
    let mut pts = [Pos2::ZERO; N];
    for (i, pt) in pts.iter_mut().enumerate() {
        let v = (h.at(ring, N, i) / top.max(1e-3)).clamp(0.0, 1.0);
        *pt = pos2(
            inner.left() + inner.width() * i as f32 / (N - 1) as f32,
            inner.bottom() - inner.height() * v,
        );
    }
    b.area(&pts, inner.bottom(), ink.gamma_multiply(0.18));
    b.polyline(&pts, 1.1 * s.k, ink);
}

/// The four readings, each a small graph with its number: CPU (amber when busy), memory,
/// voices, dropouts.
fn performance(app: &mut CantusApp, ui: &mut Ui, area: Rect, s: &Style) {
    let k = s.k;
    let stats = *app.engine.stats();
    let live = app.engine.is_live();
    let busy = live && stats.load >= 0.7;
    let drops = if live { stats.overruns } else { 0 };
    let h = &app.topbar.history;
    let ram = app.topbar.ram.bytes().map_or(0, perf::ram_mb);
    let ram_top = h.ram.iter().copied().fold(1.0, f32::max) * 1.25;
    let voices_top = h.voices.iter().copied().fold(8.0, f32::max);
    let last = t("topbar.panel.last_30");
    let cw = (area.width() - 12.0 * k) * 0.5;
    let top = area.top() + 18.0 * k;
    let mut v = String::new();
    with_brush(ui.painter(), |b| {
        title(b, area, "topbar.panel.performance", Some(&last), s);
        for i in 0..4 {
            let cx = area.left() + (i % 2) as f32 * (cw + 12.0 * k);
            let cy = top + (i / 2) as f32 * 52.0 * k;
            let head = Rect::from_min_size(pos2(cx, cy), vec2(cw, 16.0 * k));
            let (label, unit) = match i {
                0 => ("topbar.cpu", "%"),
                1 => ("topbar.ram", ""),
                2 => ("topbar.voices", ""),
                _ => ("topbar.dropouts", ""),
            };
            p::words_key(
                b,
                head,
                label,
                Role::Label,
                11.0,
                ink2(s),
                Align2::LEFT_CENTER,
                s,
            );
            v.clear();
            let (vi, unit) = match i {
                0 => {
                    let _ = write!(
                        v,
                        "{}",
                        if live {
                            (stats.load * 100.0).round() as u32
                        } else {
                            0
                        }
                    );
                    (if busy { p::busy(s) } else { ink1(s) }, unit.to_string())
                }
                1 => {
                    if ram >= 1000 {
                        let _ = write!(v, "{:.1}", ram as f64 / 1024.0);
                        (ink1(s), "GB".to_string())
                    } else {
                        let _ = write!(v, "{ram}");
                        (ink1(s), "MB".to_string())
                    }
                }
                2 => {
                    let _ = write!(v, "{}", if live { stats.active_voices } else { 0 });
                    (ink1(s), String::new())
                }
                _ => {
                    let _ = write!(v, "{drops}");
                    (
                        if drops > 0 { s.t.danger } else { ink1(s) },
                        t("topbar.panel.since_start"),
                    )
                }
            };
            let uw = if unit.is_empty() {
                0.0
            } else {
                p::measure(b.ctx(), &unit, Role::Label, 10.0, s) + 4.0 * k
            };
            p::words(
                b,
                head,
                &unit,
                Role::Label,
                10.0,
                ink3(s),
                Align2::RIGHT_CENTER,
                s,
            );
            let vr = Rect::from_min_max(head.min, pos2(head.right() - uw, head.bottom()));
            p::words(b, vr, &v, Role::Body, 15.0, vi, Align2::RIGHT_CENTER, s);
            let gr = Rect::from_min_size(pos2(cx, cy + 19.0 * k), vec2(cw, 24.0 * k));
            p::well(b, gr, s, None);
            match i {
                0 => graph(
                    b,
                    gr,
                    h,
                    &h.cpu,
                    100.0,
                    false,
                    if busy { p::busy(s) } else { ink3(s) },
                    s,
                ),
                1 => graph(b, gr, h, &h.ram, ram_top, false, ink3(s), s),
                2 => graph(b, gr, h, &h.voices, voices_top, true, ink3(s), s),
                _ => {
                    if drops > 0 {
                        for j in 0..drops.min(3) {
                            let tx = gr.left() + 6.0 * k + j as f32 * 6.0 * k;
                            let tr = Rect::from_min_size(
                                pos2(tx, gr.top() + 4.0 * k),
                                vec2(2.0, 16.0) * k,
                            );
                            b.rrect(b.snap_rect(tr), 1.0 * k, Fill::Solid(s.t.danger));
                        }
                        let wr = gr.shrink2(vec2(6.0 * k, 0.0));
                        p::words_key(
                            b,
                            wr,
                            "topbar.panel.drops_status",
                            Role::Label,
                            10.0,
                            ink2(s),
                            Align2::RIGHT_CENTER,
                            s,
                        );
                    } else {
                        b.circle(
                            pos2(gr.left() + 11.0 * k, gr.center().y),
                            3.0 * k,
                            Fill::Solid(s.t.led),
                        );
                        let wr = Rect::from_min_max(pos2(gr.left() + 19.0 * k, gr.top()), gr.max);
                        p::words_key(
                            b,
                            wr,
                            "topbar.panel.drops_none",
                            Role::Label,
                            10.0,
                            ink2(s),
                            Align2::LEFT_CENTER,
                            s,
                        );
                    }
                }
            }
        }
    });
    let resp = ui.interact(area, Id::new("panel-performance"), Sense::hover());
    if resp.hovered() {
        let tip = if live {
            crate::perf_status::histogram_text(&stats)
        } else {
            t("topbar.no_device")
        };
        hint::set_text(ui.ctx(), &t("topbar.panel.performance"), "", Caps::NONE);
        let _ = resp.on_hover_text(tip);
    }
}

/// The MIDI input (its lamp, the ports, the settings) and the armed tracks.
fn inputs(app: &mut CantusApp, ui: &mut Ui, area: Rect, s: &Style) {
    let k = s.k;
    let (x, y) = (area.left(), area.top() + 18.0 * k);
    let received = app.engine.midi_sender().received();
    let lit = app.topbar.midi_lamp(received);
    let ports = if app.midi_port_names.is_empty() {
        None
    } else {
        Some(app.midi_port_names.join(", "))
    };
    let armed: Vec<&str> = app
        .project()
        .tracks
        .iter()
        .filter(|t| t.armed)
        .map(|t| t.name.as_str())
        .collect();
    let armed_n = armed.len();
    let armed_words = if armed.is_empty() {
        None
    } else {
        Some(armed.join(", "))
    };
    let set_r = Rect::from_min_max(
        pos2(area.right() - 70.0 * k, y),
        pos2(area.right(), y + 22.0 * k),
    );
    let sresp = ui.interact(set_r, Id::new("panel-midi-settings"), Sense::click());
    let arm_r = Rect::from_min_size(pos2(x, y + ROW * k), vec2(area.width(), 22.0 * k));
    let aresp = ui.interact(arm_r, Id::new("panel-armed"), Sense::click());
    with_brush(ui.painter(), |b| {
        title(b, area, "topbar.group.inputs", None, s);
        let lamp = if lit { s.t.led } else { ink3(s) };
        b.circle(pos2(x + 5.0 * k, y + 11.0 * k), 2.5 * k, Fill::Solid(lamp));
        let lr = Rect::from_min_size(pos2(x + 14.0 * k, y), vec2(54.0 * k, 22.0 * k));
        p::words_key(
            b,
            lr,
            "topbar.midi",
            Role::Label,
            11.0,
            ink2(s),
            Align2::LEFT_CENTER,
            s,
        );
        let pr = Rect::from_min_max(
            pos2(x + 68.0 * k, y),
            pos2(set_r.left() - 6.0 * k, y + 22.0 * k),
        );
        match &ports {
            Some(w) => p::words(b, pr, w, Role::Body, 12.0, ink1(s), Align2::LEFT_CENTER, s),
            None => p::words_key(
                b,
                pr,
                "topbar.panel.midi_none",
                Role::Label,
                11.0,
                ink3(s),
                Align2::LEFT_CENTER,
                s,
            ),
        }
        let si = if sresp.hovered() { ink1(s) } else { ink2(s) };
        let sw = Rect::from_min_max(set_r.min, pos2(set_r.right() - 10.0 * k, set_r.bottom()));
        p::words_key(
            b,
            sw,
            "settings.title",
            Role::Label,
            11.0,
            si,
            Align2::RIGHT_CENTER,
            s,
        );
        let c = pos2(set_r.right() - 3.0 * k, set_r.center().y);
        b.polyline(
            &[
                pos2(c.x - 1.5 * k, c.y - 3.0 * k),
                pos2(c.x + 1.5 * k, c.y),
                pos2(c.x - 1.5 * k, c.y + 3.0 * k),
            ],
            1.2 * k,
            ink3(s),
        );
        let ay = y + ROW * k;
        let ring = if armed_words.is_some() {
            s.t.danger
        } else {
            ink3(s)
        };
        b.circle_stroke(pos2(x + 5.0 * k, ay + 11.0 * k), 3.5 * k, 1.2 * k, ring);
        let lr = Rect::from_min_size(pos2(x + 14.0 * k, ay), vec2(54.0 * k, 22.0 * k));
        p::words_key(
            b,
            lr,
            "topbar.armed",
            Role::Label,
            11.0,
            ink2(s),
            Align2::LEFT_CENTER,
            s,
        );
        let nr = Rect::from_min_max(pos2(x + 68.0 * k, ay), pos2(area.right(), ay + 22.0 * k));
        match &armed_words {
            Some(w) => p::words(b, nr, w, Role::Body, 12.0, ink1(s), Align2::LEFT_CENTER, s),
            None => p::words_key(
                b,
                nr,
                "topbar.panel.armed_none",
                Role::Label,
                11.0,
                ink3(s),
                Align2::LEFT_CENTER,
                s,
            ),
        }
    });
    if sresp.hovered() {
        hint::set_text(ui.ctx(), &t("settings.title"), "", Caps::NONE);
    }
    if sresp.clicked() {
        app.show_settings = true;
    }
    if aresp.hovered() {
        let tip = crate::i18n::tf_count("topbar.armed_tip", armed_n, &[]);
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
    }
    if aresp.clicked() {
        app.run_action(Action::ArmTrack);
    }
}

/// The picker, and the interface's zoom with − and +.
fn tools(app: &mut CantusApp, ui: &mut Ui, area: Rect, s: &Style) {
    let k = s.k;
    let (x, y) = (area.left(), area.top() + 18.0 * k);
    with_brush(ui.painter(), |b| {
        title(b, area, "topbar.group.tools", None, s)
    });
    let pr = Rect::from_min_size(pos2(x, y), vec2(28.0, 22.0) * k);
    keys::pick_key(app, ui, pr, s);
    let pick_keys = binding(app, Action::PickElement);
    let zoom_keys = binding(app, Action::ZoomReset);
    with_brush(ui.painter(), |b| {
        let wr = Rect::from_min_max(
            pos2(x + 38.0 * k, y),
            pos2(area.right() - 60.0 * k, y + 22.0 * k),
        );
        p::words_key(
            b,
            wr,
            "topbar.panel.pick",
            Role::Label,
            11.0,
            ink1(s),
            Align2::LEFT_CENTER,
            s,
        );
        if let Some(w) = &pick_keys {
            keycap(b, area.right(), y + 4.0 * k, w, true, s);
        }
        let zy = y + 30.0 * k;
        let lr = Rect::from_min_size(pos2(x, zy), vec2(96.0 * k, 24.0 * k));
        p::words_key(
            b,
            lr,
            "topbar.panel.zoom",
            Role::Label,
            11.0,
            ink2(s),
            Align2::LEFT_CENTER,
            s,
        );
        if let Some(w) = &zoom_keys {
            keycap(b, area.right(), zy + 5.0 * k, w, true, s);
        }
    });
    let zy = y + 30.0 * k;
    let cells = [
        Rect::from_min_size(pos2(x + 100.0 * k, zy + 2.0 * k), vec2(26.0, 20.0) * k),
        Rect::from_min_size(pos2(x + 126.0 * k, zy + 2.0 * k), vec2(54.0, 20.0) * k),
        Rect::from_min_size(pos2(x + 180.0 * k, zy + 2.0 * k), vec2(26.0, 20.0) * k),
    ];
    let minus = ui.interact(cells[0], Id::new("panel-zoom-out"), Sense::click());
    let plus = ui.interact(cells[2], Id::new("panel-zoom-in"), Sense::click());
    let now = app.settings.ui_scale();
    let mut v = String::new();
    let _ = write!(v, "{:.0} %", now * 100.0);
    with_brush(ui.painter(), |b| {
        p::joined(b, &cells, s);
        for (c, r) in [(&minus, cells[0]), (&plus, cells[2])] {
            p::tint(b, r, s, State::of(c, false), s.t.highlight, 0.2);
        }
        p::words(
            b,
            cells[0],
            "\u{2212}",
            Role::Body,
            12.0,
            ink2(s),
            Align2::CENTER_CENTER,
            s,
        );
        p::words(
            b,
            cells[1],
            &v,
            Role::Control,
            11.0,
            ink1(s),
            Align2::CENTER_CENTER,
            s,
        );
        p::words(
            b,
            cells[2],
            "+",
            Role::Body,
            12.0,
            ink2(s),
            Align2::CENTER_CENTER,
            s,
        );
    });
    let steps = crate::settings::UI_SCALES;
    let at = steps.iter().position(|z| (z - now).abs() < 1e-3);
    let step = |d: i32| -> Option<f32> {
        let i = match at {
            Some(i) => i as i32 + d,
            None => steps.iter().position(|z| *z > now).unwrap_or(steps.len()) as i32 + d.min(0),
        };
        (0..steps.len() as i32)
            .contains(&i)
            .then(|| steps[i as usize])
    };
    for (resp, d, key) in [
        (&minus, -1, "topbar.panel.zoom_out"),
        (&plus, 1, "topbar.panel.zoom_in"),
    ] {
        if resp.hovered() {
            hint::set_text(ui.ctx(), &t(key), &v, Caps::NONE);
        }
        if resp.clicked() {
            if let Some(z) = step(d) {
                let ctx = ui.ctx().clone();
                app.apply_ui_scale(&ctx, z);
            }
        }
    }
}

/// *Also here*: the small folds, one row each — the label at the left, the control at 100.
fn also(
    app: &mut CantusApp,
    ui: &mut Ui,
    area: Rect,
    ex: &[Option<Extra>],
    s: &Style,
    drawn: &mut Vec<(Group, Rect)>,
) {
    let k = s.k;
    let x = area.left();
    let mut y = area.top() + 18.0 * k;
    with_brush(ui.painter(), |b| {
        title(b, area, "topbar.panel.also", None, s)
    });
    let ctx = ui.ctx().clone();
    let wants = super::wants(app, &ctx);
    let full = |g: Group| wants.iter().find(|w| w.group == g).map_or(80.0, |w| w.max);
    for e in ex.iter().flatten() {
        let h = extra_h(*e) * k;

        let label = match e {
            Extra::Metre => "topbar.metre",
            Extra::Grid => "topbar.grid",
            Extra::Group(g) => g.name_key(),
        };
        let lr = Rect::from_min_size(pos2(x, y), vec2(96.0 * k, 22.0 * k.max(0.0)));
        let lr = if h > ROW * k {
            lr.translate(vec2(0.0, (h - 22.0 * k) * 0.5))
        } else {
            lr
        };
        with_brush(ui.painter(), |b| {
            p::words_key(
                b,
                lr,
                label,
                Role::Label,
                11.0,
                ink2(s),
                Align2::LEFT_CENTER,
                s,
            )
        });
        let cx = x + 100.0 * k;
        let small = |w: f32| Rect::from_min_size(pos2(cx, y + 1.0 * k), vec2(w, 20.0 * k));
        match e {
            Extra::Metre => tempo::metre_field(app, ui, small(56.0 * k), s),
            Extra::Grid => {
                let r = small(full(Group::Snap));
                snap::show(app, ui, r, s, false);
                drawn.push((Group::Snap, r));
            }
            Extra::Group(g) => {
                let r = match g {
                    Group::Mode | Group::Undo | Group::Meter | Group::Snap => small(full(*g)),
                    _ => Rect::from_min_size(pos2(cx, y), vec2(full(*g), h)),
                };
                match g {
                    Group::Meter => meter::show(app, ui, r, s, false),
                    _ => super::draw(*g, app, ui, r, s, false),
                }
                drawn.push((*g, r));
                let note = match g {
                    Group::Mode => binding(app, Action::TogglePatternMode),
                    Group::Undo => app.bus.undo_label_key().map(t),
                    _ => None,
                };
                if let Some(w) = note {
                    with_brush(ui.painter(), |b| {
                        if *g == Group::Undo {
                            let nr = Rect::from_min_max(
                                pos2(r.right() + 8.0 * k, y),
                                pos2(area.right(), y + 22.0 * k),
                            );
                            p::words(
                                b,
                                nr,
                                &w,
                                Role::Label,
                                11.0,
                                ink3(s),
                                Align2::LEFT_CENTER,
                                s,
                            );
                        } else {
                            keycap(b, area.right(), y + 5.0 * k, &w, true, s);
                        }
                    });
                }
            }
        }
        y += h;
    }
    let _ = tf;
}
