//! The desk's furniture: the ground, the title row, the legend column, the dock
//! dividers and the scrollbars — and the wheel, which decides between scrolling,
//! reordering and changing the width mode by where the pointer is and what is held.

use super::layout::{self, Fit, Row, WidthMode};
use super::look::{self, W};
use super::menu::Menu;
use super::{desk, size, Geo, MixerView};
use crate::app::CantusApp;
use crate::kit::color::{mix, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::glyph_mixer as mg;
use crate::kit::text::{self, Role, Src, Text};
use crate::kit::{cap, hint, segmented, Brush, Fill, State, Style, Tok};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};

/// The tokens the desk's own surfaces paint with (for inspect mode).
const GROUND_TOKENS: &[Tok] = &[Tok::Panel, Tok::Strip, Tok::Sheen, Tok::Glow, Tok::GlowCool];

/// The desk's ground (des-mixer): `base` under the strips, the header a `surface` band
/// with its 1-px lit edge — panels separated by value, not by borders.
pub(super) fn ground(b: &mut Brush, s: &Style, geo: &Geo) {
    let t = &s.t;
    let p = look::Pal::of(s);
    b.register(geo.full, "mixer_ground", GROUND_TOKENS, &[]);
    b.rect(b.snap_rect(geo.full), Fill::Solid(p.base));
    if geo.merged {
        // The tab row is the dock's: its ground stays.
        return;
    }
    let r = b.snap_rect(geo.title);
    b.rect(r, Fill::Solid(p.surface));
    let lit = if p.dark {
        mix(p.surface, p.ink1, 0.07)
    } else {
        mix(p.surface, t.light, 0.6)
    };
    b.hline(r.left(), r.right(), r.top() + b.px() * 0.5, b.px(), lit);
}

/// The right dock's edge: a 1-px seam `mix(base, edge, .6)` 6 pt left of it.
pub(super) fn dividers(b: &mut Brush, s: &Style, geo: &Geo) {
    if geo.right.width() <= 0.0 {
        return;
    }
    let p = look::Pal::of(s);
    let x = b.snap(geo.right.left() - 6.0);
    b.vline(
        x + b.px() * 0.5,
        geo.strips.top() + size::TOP,
        geo.strips.bottom() - size::TOP,
        b.px(),
        mix(p.base, p.edge, 0.6),
    );
}

/// One clickable spot in the title row.
fn key(ui: &Ui, rect: Rect, id: &str) -> egui::Response {
    ui.interact(rect, Id::new(("mixer-title", id)), Sense::click())
}

/// A header key drawn as Slate's well, 20 high.
fn well_key(b: &mut Brush, r: Rect, p: &look::Pal, hot: bool) {
    look::well(b, r, p, hot);
}

/// A small chevron down at `c` (2.5 across, 1.2 wide strokes).
fn chevron(b: &mut Brush, c: egui::Pos2, ink: egui::Color32) {
    b.segment(
        pos2(c.x - 2.5, c.y - 1.25),
        pos2(c.x, c.y + 1.25),
        1.2,
        ink,
        true,
    );
    b.segment(
        pos2(c.x, c.y + 1.25),
        pos2(c.x + 2.5, c.y - 1.25),
        1.2,
        ink,
        true,
    );
}

/// The header (des-mixer, 28 high): the density (Compact · Wide · Large), the **Rows**
/// menu (the desk's menu: which rows show, the colours, the pan law…), the solo chip while
/// anything is soloed ("1 soloed · Clear"), a take being recorded; at the right the cable
/// view's key and its options, *Snapshots ▾* and `+` (add a strip).
pub(super) fn title(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    s: &Style,
    geo: &Geo,
) {
    let p = look::Pal::of(s);
    let r = geo.title;
    let cy = r.center().y;
    let ctx = ui.ctx().clone();
    let mut x = r.left() + 12.0;

    // The density.
    let labels = [
        Src::Key(WidthMode::Compact.key()),
        Src::Key(WidthMode::Wide.key()),
        Src::Key(WidthMode::Xl.key()),
    ];
    // Equal cells (the kit's segmented control), each wide enough for the longest word.
    let cell = WidthMode::ALL
        .iter()
        .map(|m| text::measure(&ctx, &Text::key(m.key(), Role::Control, p.ink1), s.tk()).x)
        .fold(0.0f32, f32::max);
    // On a narrow desk the cells give way first (the + key, Rows and the cable view's key
    // keep their room).
    let rows_w = (look::width_of(&ctx, s, &v.words.rows, 11.0, W::Regular) + 22.0).round();
    let room = r.width() - 12.0 - 12.0 - rows_w - 8.0 - 24.0 - 8.0 - 10.0 - 24.0 - 8.0;
    let seg_w = (3.0 * (cell + 20.0).max(44.0).round())
        .min(room.max(90.0))
        .floor();
    let wr = Rect::from_min_size(pos2(x, cy - 10.0), vec2(seg_w, 20.0));
    let resp = key(ui, wr, "width");
    let hovered = resp
        .hover_pos()
        .and_then(|q| segmented::hit(wr, labels.len(), q.x));
    let mode = v.prefs.current.width;
    if resp.clicked() {
        if let Some(i) = hovered {
            let mut pr = v.prefs.current;
            pr.width = WidthMode::from_index(i);
            v.prefs.set(pr);
        }
    }
    if resp.hovered() {
        hint::set_text(&ctx, &v.words.modes, "", hint::Caps::NONE);
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
    x = wr.right() + 12.0;

    // Rows ▾: the desk's menu.
    let rw = (look::width_of(&ctx, s, &v.words.rows, 11.0, W::Regular) + 22.0).round();
    let kr = Rect::from_min_size(pos2(x, cy - 10.0), vec2(rw, 20.0));
    let resp = key(ui, kr, "menu");
    let open = v.popups.is_open(Menu::Desk);
    well_key(b, kr, &p, resp.hovered() || open);
    look::say(
        b,
        s,
        &v.words.rows,
        kr.left() + 8.0,
        cy + 4.0,
        Align2::LEFT_BOTTOM,
        rw - 16.0,
        11.0,
        W::Regular,
        p.ink2,
    );
    chevron(b, pos2(kr.right() - 9.0, cy), p.ink3);
    if resp.hovered() {
        hint::set_text(&ctx, &v.words.rows_tip, "", hint::Caps::NONE);
    }
    if resp.on_hover_text(&v.words.rows_tip).clicked() {
        v.popups.open(Menu::Desk, pos2(kr.left(), r.bottom()));
    }
    x = kr.right() + 8.0;

    // The Cables key (the owner's option C with B on demand): off at rest; on, the
    // selection's cables under the strips; "Show all" in its options menu.
    {
        let kx = x;
        let view = v.prefs.current.cables;
        let lit = view != layout::CableView::Off;
        let tw = look::width_of(&ctx, s, &v.words.cables, 11.0, W::Regular);
        let narrow = r.width() < 480.0;
        let kw = if narrow { 24.0 } else { (tw + 30.0).round() };
        let kr = Rect::from_min_size(pos2(kx, cy - 10.0), vec2(kw, 20.0));
        let resp = key(ui, kr, "cables");
        let tint = mix(p.well, p.accent, 0.2);
        if lit {
            b.rrect(b.snap_rect(kr), 3.0, Fill::Solid(tint));
        } else {
            well_key(b, kr, &p, resp.hovered());
        }
        let ink = if lit {
            mg::Ink::On(p.accent)
        } else if resp.hovered() {
            mg::Ink::Hover
        } else {
            mg::Ink::Rest
        };
        look::glyph(
            b,
            s,
            pos2(kr.left() + 12.0, cy),
            16.0,
            mg::MGlyph::Routing,
            ink,
            if lit { tint } else { p.well },
        );
        if !narrow {
            let wi = if lit { p.accent } else { p.ink2 };
            look::say(
                b,
                s,
                &v.words.cables,
                kr.left() + 23.0,
                cy + 4.0,
                Align2::LEFT_BOTTOM,
                tw + 2.0,
                11.0,
                W::Regular,
                wi,
            );
        }
        if resp.hovered() {
            hint::set_text(&ctx, &v.words.cables_tip, "", hint::Caps::NONE);
        }
        if resp.clicked() {
            let mut pr = v.prefs.current;
            pr.cables = if lit {
                layout::CableView::Off
            } else {
                layout::CableView::Selected
            };
            v.prefs.set(pr);
        }
        // Its options where the row has room before the + key at the right (the Rows
        // menu has them too).
        x = if kr.right() + 2.0 + OPTIONS_KEY + 8.0 <= r.right() - 10.0 - 24.0 - 8.0 {
            cable_options(v, ui, b, s, r, kr.right() + 2.0) + 8.0
        } else {
            kr.right() + 8.0
        };
    }

    // The solo chip: "1 soloed · Clear".
    let soloed = app.project().strips.iter().filter(|s| s.soloed).count();
    if soloed > 0 {
        let mut buf = std::mem::take(&mut v.buf);
        buf.clear();
        super::rows::fill_n(&mut buf, &v.words.soloed, soloed);
        let sw = look::width_of(&ctx, s, &buf, 11.0, W::Medium);
        let cw = look::width_of(&ctx, s, &v.words.clear, 11.0, W::Regular);
        let chip = Rect::from_min_size(pos2(x, cy - 10.0), vec2((sw + cw + 26.0).round(), 20.0));
        // Only where it fits before the + key at the right (the Rows menu has *Solo off*).
        if chip.right() <= r.right() - 42.0 {
            let resp = key(ui, chip, "mixer.solo_off");
            let fill = mix(p.surface, p.solo, if resp.hovered() { 0.24 } else { 0.16 });
            b.rrect(b.snap_rect(chip), 4.0, Fill::Solid(fill));
            look::say(
                b,
                s,
                &buf,
                chip.left() + 8.0,
                cy + 4.0,
                Align2::LEFT_BOTTOM,
                sw + 2.0,
                11.0,
                W::Medium,
                p.solo,
            );
            look::say(
                b,
                s,
                &v.words.clear,
                chip.right() - 8.0,
                cy + 4.0,
                Align2::RIGHT_BOTTOM,
                cw + 2.0,
                11.0,
                W::Regular,
                p.ink2,
            );
            if resp.hovered() {
                hint::set_text(&ctx, &v.words.clear_tip, "", hint::Caps::NONE);
            }
            if resp.clicked() {
                app.clear_strip_solos();
            }
            x = chip.right() + 8.0;
        }
        v.buf = buf;
    }

    // Takes: the one running, or the list of those made.
    x = takes(v, app, ui, b, s, r, x);

    // The right side, from the edge in: + (add a strip), Snapshots ▾, the cable view.
    let mut right = r.right() - 10.0;
    let ar = Rect::from_min_max(pos2(right - 24.0, cy - 10.0), pos2(right, cy + 10.0));
    let resp = key(ui, ar, "add");
    well_key(b, ar, &p, resp.hovered());
    look::plus(b, ar.center(), if resp.hovered() { p.ink1 } else { p.ink2 });
    if resp.hovered() {
        hint::set_text(&ctx, &v.words.add_strip, "", hint::Caps::NONE);
    }
    if resp.on_hover_text(&v.words.add_strip).clicked() {
        app.add_strip();
    }
    right = ar.left() - 8.0;
    let n = app.project().mix.snapshots.len();
    let snap_w = {
        v.buf.clear();
        use std::fmt::Write as _;
        let _ = if n == 0 {
            write!(v.buf, "{}", v.words.snap)
        } else {
            write!(v.buf, "{} {n}", v.words.snap)
        };
        text::measure(&ctx, &Text::new(&v.buf, Role::Control, p.ink1), s.tk()).x + 30.0
    };
    if right - snap_w > x + 80.0 {
        let _ = super::vca_ui::snap_key(v, app, ui, b, s, r, right - snap_w, 0.0);
    }
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

/// The scrollbars (des-mixer): 3 pt at the desk's foot under the middle, and at the
/// right when the desk is taller than its panel — only while there is something to
/// scroll; the track `mix(base, edge, .3)`, the thumb ink 4. Dragging the thumb scrolls;
/// a click on the track pages.
pub(super) fn scrollbars(
    v: &mut MixerView,
    ui: &mut Ui,
    b: &mut Brush,
    s: &Style,
    geo: &Geo,
    fit: &Fit,
) {
    let p = look::Pal::of(s);
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
        if rect.width() <= 0.0 || rect.height() <= 0.0 || max <= 0.5 {
            continue;
        }
        let len = (view / content.max(1.0)).clamp(0.0, 1.0);
        let id = Id::new(("mixer-scrollbar", across));
        let resp = ui.interact(rect.expand(3.0), id, Sense::click_and_drag());
        let scroll = if across {
            &mut v.scroll_x
        } else {
            &mut v.scroll_y
        };
        let full = if across { rect.width() } else { rect.height() };
        let thumb_at = |scroll: f32| {
            let start = scroll / content.max(1.0);
            if across {
                Rect::from_min_size(
                    pos2(rect.left() + full * start, rect.top()),
                    vec2((full * len).max(12.0), rect.height()),
                )
            } else {
                Rect::from_min_size(
                    pos2(rect.left(), rect.top() + full * start),
                    vec2(rect.width(), (full * len).max(12.0)),
                )
            }
        };
        if resp.dragged() {
            let d = resp.drag_delta();
            let moved = if across { d.x } else { d.y };
            *scroll = (*scroll + moved / full.max(1.0) * content).clamp(0.0, max);
        } else if resp.clicked() {
            if let Some(q) = resp.interact_pointer_pos() {
                let thumb = thumb_at(*scroll);
                let before = if across {
                    q.x < thumb.left()
                } else {
                    q.y < thumb.top()
                };
                let page = view * 0.9;
                *scroll = (*scroll + if before { -page } else { page }).clamp(0.0, max);
            }
        }
        let hot = resp.hovered() || resp.dragged();
        b.rrect(
            b.snap_rect(rect),
            1.5,
            Fill::Solid(mix(p.base, p.edge, 0.3)),
        );
        b.rrect(
            b.snap_rect(thumb_at(*scroll)),
            1.5,
            Fill::Solid(if hot { p.ink3 } else { p.ink4 }),
        );
        if hot {
            // r39:ux-a — the help line (ADR 0109).
            let words = crate::i18n::t("ux.hint.scrollbar_page");
            crate::kit::hint::fallback(ui.ctx(), &words, "", crate::kit::hint::Caps::NONE);
        }
    }
}

/// FL's next inserts (round 39, lo-mixer): where the desk ends before the panel does, the
/// room up to its edge holds the next strips' places — numbered on from the last strip, in
/// the look of an empty strip (the quiet body, the number in ink 4, a groove, the cap's
/// ghost at 0 dB). They are only painted: nothing in the project, nothing in the engine,
/// nothing that repaints. A click on one makes the strips up to it (one undo step) and
/// selects it, ready to be routed to. Returns how many were painted.
pub(super) fn ghosts(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &super::strip::Pass,
    from: f32,
    to: f32,
) -> usize {
    let pitch = pass.prefs.width.pitch();
    let have = app.project().strips.len();
    let room = cantus_engine::MAX_STRIPS.saturating_sub(have);
    let n = (((to - from) / pitch).floor().max(0.0) as usize).min(room);
    let Some((na, _)) = pass.row(Row::Number) else {
        return 0;
    };
    let p = pass.pal;
    let s = pass.s;
    let mode = pass.mode();
    let g = look::geom(mode, false);
    let travel = pass.fit.travel().map(|(a, z)| (pass.y0 + a, pass.y0 + z));
    let bw = pitch - 2.0;
    let mut clicked = None;
    for k in 0..n {
        let x = from + k as f32 * pitch;
        let body = Rect::from_min_max(pos2(x, pass.clip.top()), pos2(x + bw, pass.clip.bottom()))
            .intersect(pass.body(x, bw));
        let resp = ui.interact(body, Id::new(("mixer-next-strip", k)), Sense::click());
        look::body(b, body, &p, Some(p.empty));
        if resp.hovered() {
            b.rrect(
                b.snap_rect(body),
                3.0,
                Fill::Solid(with_alpha(p.ink1, 0.03)),
            );
            let words = &v.words.next_strip;
            hint::set_text(ui.ctx(), words, "", hint::Caps::NONE);
        }
        v.buf.clear();
        {
            use std::fmt::Write as _;
            let _ = write!(v.buf, "{}", have + k + 1);
        }
        let base = na
            + if mode == WidthMode::Compact {
                14.0
            } else {
                15.0
            };
        look::say(
            b,
            s,
            &v.buf,
            x + bw * 0.5,
            base,
            Align2::CENTER_BOTTOM,
            bw,
            10.0,
            W::Regular,
            p.ink4,
        );
        if let Some((top, bot)) = travel {
            let gx = x + g.groove_x;
            look::groove(b, gx, top, bot, &p);
            let cy = top + (bot - top) * look::law::frac(0.0);
            look::cap_ghost(b, gx, cy, g.cap.x - 2.0, &p);
        }
        if resp.clicked() {
            clicked = Some(have + k + 1);
        }
    }
    if let Some(upto) = clicked {
        app.add_strips_up_to(upto);
    }
    n
}

/// The middle's overflow (des-mixer): a 20-pt fade into `base` at each edge with strips
/// beyond it, and a pill with how many ("‹ 6", "6 ›"), vertically centred in its slot
/// beside the middle (r39:lo-mixer: it covered the last strip's meter); a click on the pill
/// scrolls a page towards them.
pub(super) fn overflow(v: &mut MixerView, ui: &mut Ui, b: &mut Brush, s: &Style, geo: &Geo) {
    let p = look::Pal::of(s);
    let mid = geo.middle.intersect(geo.strips);
    if mid.width() < 60.0 {
        return;
    }
    let (mut left, mut right) = (0usize, 0usize);
    let mut x = -v.scroll_x;
    for item in &v.desk.middle {
        let w = desk::item_width(item, geo.pitch);
        if matches!(item, desk::Item::Strip(..)) {
            if x + w <= 1.0 {
                left += 1;
            } else if x >= mid.width() - 1.0 {
                right += 1;
            }
        }
        x += w;
    }
    let ctx = ui.ctx().clone();
    for (n, right_side) in [(left, false), (right, true)] {
        if n == 0 {
            continue;
        }
        let fade = if right_side {
            Rect::from_min_max(pos2(mid.right() - 20.0, mid.top()), mid.max)
        } else {
            Rect::from_min_max(mid.min, pos2(mid.left() + 20.0, mid.bottom()))
        };
        // Ten slices of 2 pt, from clear to 85 % of the base, towards the edge.
        for k in 0..10 {
            let f = (k as f32 + 0.5) / 10.0;
            let a = 0.85 * if right_side { f } else { 1.0 - f };
            let sx = fade.left() + 2.0 * k as f32;
            b.rect(
                Rect::from_min_max(pos2(sx, fade.top()), pos2(sx + 2.0, fade.bottom())),
                Fill::Solid(with_alpha(p.base, a)),
            );
        }
        v.buf.clear();
        {
            use std::fmt::Write as _;
            let _ = write!(v.buf, "{n}");
        }
        let lw = look::width_of(&ctx, s, &v.buf, 10.0, W::SemiBold);
        let bw = (lw + 20.0).round();
        let by = (mid.center().y - 9.0).round();
        // r39:lo-mixer — in the slot beside the middle where there is one (never over a
        // strip's meter), else at the middle's edge.
        // r39:fix2 — each side's own slot (the right one may be wider: what is left of
        // the room after the whole strips).
        let bx = match right_side {
            true if geo.slot_r > 0.0 => (mid.right() + (geo.slot_r - bw) * 0.5).round(),
            false if geo.slot_l > 0.0 => {
                (mid.left() - geo.slot_l + (geo.slot_l - bw) * 0.5).round()
            }
            true => mid.right() - bw - 2.0,
            false => mid.left() + 2.0,
        };
        let pill = Rect::from_min_size(pos2(bx, by), vec2(bw, 18.0));
        let resp = key(
            ui,
            pill,
            if right_side {
                "more-right"
            } else {
                "more-left"
            },
        );
        let fill = if resp.hovered() {
            mix(p.raised, p.ink1, 0.06)
        } else {
            p.raised
        };
        b.rrect(b.snap_rect(pill), 9.0, Fill::Solid(fill));
        b.hline(
            pill.left() + 6.0,
            pill.right() - 6.0,
            pill.top() + b.px() * 0.5,
            b.px(),
            mix(p.raised, p.ink1, 0.08),
        );
        let buf = std::mem::take(&mut v.buf);
        let cy = pill.center().y;
        if right_side {
            look::say(
                b,
                s,
                &buf,
                pill.left() + 7.0,
                cy + 3.5,
                Align2::LEFT_BOTTOM,
                lw + 2.0,
                10.0,
                W::SemiBold,
                p.ink2,
            );
            let c = pos2(pill.right() - 7.0, cy);
            b.segment(pos2(c.x - 1.25, c.y - 2.5), c, 1.2, p.ink2, true);
            b.segment(c, pos2(c.x - 1.25, c.y + 2.5), 1.2, p.ink2, true);
        } else {
            look::say(
                b,
                s,
                &buf,
                pill.right() - 7.0,
                cy + 3.5,
                Align2::RIGHT_BOTTOM,
                lw + 2.0,
                10.0,
                W::SemiBold,
                p.ink2,
            );
            let c = pos2(pill.left() + 7.0, cy);
            b.segment(pos2(c.x + 1.25, c.y - 2.5), c, 1.2, p.ink2, true);
            b.segment(c, pos2(c.x + 1.25, c.y + 2.5), 1.2, p.ink2, true);
        }
        v.buf = buf;
        if resp.hovered() {
            let words = if right_side {
                &v.words.more_right
            } else {
                &v.words.more_left
            };
            let mut line = String::new();
            super::rows::fill_n(&mut line, words, n);
            hint::set_text(&ctx, &line, "", hint::Caps::NONE);
        }
        if resp.clicked() {
            let page = ((mid.width() / geo.pitch).floor().max(1.0) - 1.0).max(1.0) * geo.pitch;
            v.scroll_x = if right_side {
                v.scroll_x + page
            } else {
                (v.scroll_x - page).max(0.0)
            };
        }
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
    // Over a pinned band the band is what is under the pointer, whatever row lies hidden
    // beneath it.
    let in_band = geo.pinned && row_at(Row::Cables);
    let on_control = !in_band
        && (row_at(Row::Fader)
            || row_at(Row::Pan)
            || row_at(Row::Width)
            || row_at(Row::Eq)
            || row_at(Row::Sends)
            || row_at(Row::SendList));
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
        let head = in_band
            || row_at(Row::Number)
            || row_at(Row::Name)
            || row_at(Row::Meter)
            || row_at(Row::Cables);
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

/// The options key's width.
const OPTIONS_KEY: f32 = 20.0;

/// The cable view's options key beside the switch: a funnel, lit while anything differs
/// from the default (so a hidden kind is never forgotten); a click opens the options
/// menu under it, a second click closes it. Returns its right edge.
fn cable_options(v: &mut MixerView, ui: &Ui, b: &mut Brush, s: &Style, r: Rect, x: f32) -> f32 {
    let t = &s.t;
    let kr = Rect::from_min_size(pos2(x, r.center().y - 10.0), vec2(OPTIONS_KEY, OPTIONS_KEY));
    let resp = key(ui, kr, "cable-options");
    let open = v.popups.is_open(Menu::Cables);
    let filtered = v.prefs.current.cable_opts.filtered();
    let st = State::of(&resp, open || filtered);
    cap::toolbar_key(b, kr, s, st, cap::KeyFace::Text(""));
    let ink = if filtered {
        t.highlight_glyph
    } else if st.hovered || open {
        t.text
    } else {
        t.text_dim
    };
    glyph::paint(b, kr.shrink(5.0), Glyph::Filter, ink);
    if resp.hovered() {
        hint::set_text(ui.ctx(), &v.words.cable_opts, "", hint::Caps::NONE);
    }
    if resp.clicked() {
        if open {
            v.popups.close();
        } else {
            v.popups
                .open_from(Menu::Cables, pos2(kr.left(), r.bottom()), Some(kr));
        }
    }
    kr.right()
}
