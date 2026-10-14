//! The picker's window (ADR 0029 §2): a large panel over a dimmed workspace — header,
//! shelves, gallery, details, footer — composed here; each part is drawn by its module.

use super::picker::{self, PickerView};
use super::picker_keys::{self, Action};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::cap::{self, KeyFace};
use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::Segmented;
use crate::kit::{light, with_brush, Brush, Fill, State, Style, Tok};
use crate::picker::PickPurpose;
use crate::picker_prefs::{Sort, TileSize};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};

pub const HEADER: f32 = 58.0;
pub const FOOTER: f32 = 48.0;
pub const SIDE: f32 = 208.0;
pub const DETAILS: f32 = 316.0;

/// The search field's id (its focus is read before it is drawn).
pub fn search_id() -> Id {
    Id::new("picker-search")
}

/// Draw the whole picker in `screen`; returns what the frame's gestures asked.
pub fn show(app: &mut CantusApp, ui: &mut Ui, screen: Rect, purpose: &PickPurpose) -> Action {
    let s = Style::current();
    let (title, context) = picker::title(app, purpose);
    let verb = picker::verb(purpose);
    let v = &mut app.picker_view;
    let search_focused = ui.memory(|m| m.has_focus(crate::kit::field::edit_id(search_id())));
    let mut action = picker_keys::read(v, ui, search_focused);
    // The scrim: the workspace dimmed; a click on it closes.
    // Allocated, so the overlay's layer covers the window and takes every pointer event.
    let scrim = ui.allocate_rect(screen, Sense::click());
    let panel = picker::panel_rect(screen);
    v.geo.panel = panel;
    with_brush(ui.painter(), |b| {
        b.register(screen, "picker-scrim", &[Tok::MeterWell], &[]);
        // The meters' well is dark in every theme: the workspace sinks under the panel.
        b.rect(
            screen,
            Fill::Solid(with_alpha(
                s.t.meter_well,
                if s.t.dark { 0.6 } else { 0.42 },
            )),
        );
        b.rrect_soft(
            panel.translate(vec2(0.0, s.s(10.0))),
            s.r(12.0),
            Fill::Solid(with_alpha(s.t.meter_well, 0.55)),
            s.s(28.0),
        );
        b.rrect(panel, s.r(10.0), Fill::Solid(s.t.panel));
    });
    // The panel swallows clicks so they do not reach the scrim.
    let _ = ui.interact(panel, Id::new("picker-panel"), Sense::click());
    if scrim.clicked()
        && !scrim
            .interact_pointer_pos()
            .is_some_and(|p| panel.contains(p))
    {
        action = Action::Close;
    }
    let header = Rect::from_min_size(panel.min, vec2(panel.width(), s.s(HEADER)));
    let footer = Rect::from_min_max(pos2(panel.left(), panel.bottom() - s.s(FOOTER)), panel.max);
    let body = Rect::from_min_max(
        pos2(panel.left(), header.bottom()),
        pos2(panel.right(), footer.top()),
    );
    let side = Rect::from_min_size(body.min, vec2(s.s(SIDE), body.height()));
    let details_w = if v.prefs.details && panel.width() > s.s(900.0) {
        s.s(DETAILS)
    } else {
        0.0
    };
    let details = Rect::from_min_max(pos2(body.right() - details_w, body.top()), body.max);
    let grid = Rect::from_min_max(
        pos2(side.right(), body.top()),
        pos2(details.left(), body.bottom()),
    );
    if head(v, ui, header, &s, &title, &context) {
        action = Action::Close;
    }
    super::picker_side::show(v, ui, side, &s);
    let a = super::picker_grid::show(v, ui, grid, &s);
    if a != Action::None {
        action = a;
    }
    if details_w > 0.0 {
        if let Some(a) = super::picker_details::show(v, ui, details, &s, &verb) {
            action = a;
        }
    }
    if let Some(a) = foot(v, ui, footer, &s, &verb) {
        action = a;
    }
    // The panel's rim, over everything.
    with_brush(ui.painter(), |b| {
        b.rrect_stroke(
            panel,
            s.r(10.0),
            b.px(),
            shade(s.t.panel, if s.t.dark { -0.12 } else { -0.2 }),
        );
        b.vline(
            b.snap_mid(side.right()),
            body.top(),
            body.bottom(),
            b.px(),
            shade(s.t.panel, -0.1),
        );
        if details_w > 0.0 {
            b.vline(
                b.snap_mid(details.left()),
                body.top(),
                body.bottom(),
                b.px(),
                shade(s.t.panel, -0.1),
            );
        }
    });
    action
}

/// A push button in `rect`: a cap with its word; `primary` wears the highlight.
pub fn button(
    ui: &Ui,
    rect: Rect,
    id: Id,
    label: &str,
    primary: bool,
    enabled: bool,
) -> egui::Response {
    let s = Style::current();
    let resp = ui.interact(
        rect,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let mut st = State::of(&resp, false);
    st.disabled = !enabled;
    with_brush(ui.painter(), |b| {
        b.register(
            rect,
            "picker-button",
            &[
                Tok::PanelRaised,
                Tok::Highlight,
                Tok::HighlightGlyph,
                Tok::Text,
            ],
            &[],
        );
        let base = if primary {
            s.t.highlight
        } else {
            s.t.panel_raised
        };
        let base = if st.active {
            shade(base, -0.03)
        } else if st.hovered {
            shade(base, 0.05)
        } else {
            base
        };
        light::raised(
            b,
            rect,
            s.r(4.0),
            base,
            light::RaisedOpts {
                drop: !st.active,
                k: s.k,
                alpha: st.alpha(),
                depth: s.t.depth,
                ..Default::default()
            },
        );
        let ink = if primary {
            s.t.highlight_glyph
        } else {
            s.t.text
        };
        let t = Text::new(label, Role::Control, ink.gamma_multiply(st.alpha()));
        let _ = text::paint(b, rect, &t, s.tk());
    });
    resp
}

/// A square key with a glyph (close, the details toggle).
fn key(ui: &Ui, rect: Rect, id: Id, g: Glyph, lit: bool, name: &str) -> egui::Response {
    let s = Style::current();
    let resp = ui.interact(rect, id, Sense::click());
    let st = State::of(&resp, lit);
    if st.hovered {
        crate::kit::hint::set_text(ui.ctx(), name, "", crate::kit::hint::Caps::NONE);
    }
    with_brush(ui.painter(), |b| {
        cap::toolbar_key(b, rect, &s, st, KeyFace::Glyph(g))
    });
    resp.on_hover_text(name)
}

/// The header; returns whether its close key was clicked.
fn head(v: &mut PickerView, ui: &mut Ui, r: Rect, s: &Style, title: &str, context: &str) -> bool {
    let pad = s.s(16.0);
    with_brush(ui.painter(), |b| {
        b.register(
            r,
            "picker-header",
            &[Tok::Toolbar, Tok::Text, Tok::TextDim],
            &[],
        );
        let top = Rect::from_min_max(r.min, pos2(r.right(), r.bottom()));
        b.rrect(
            top,
            s.r(10.0),
            Fill::V(shade(s.t.toolbar, 0.02), s.t.toolbar),
        );
        b.rect(
            Rect::from_min_max(pos2(r.left(), r.bottom() - s.s(10.0)), r.max),
            Fill::Solid(s.t.toolbar),
        );
        b.hline(
            r.left(),
            r.right(),
            b.snap_mid(r.bottom()),
            b.px(),
            shade(s.t.toolbar, -0.14),
        );
        // What is being chosen, set large, and for what, dimmed.
        let tr = Rect::from_min_size(
            pos2(r.left() + pad, r.top() + s.s(8.0)),
            vec2(s.s(300.0), s.s(24.0)),
        );
        let tt = Text::new(title, Role::Title, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, tr, &tt, s.tk() * 1.3);
        let cr = tr.translate(vec2(0.0, s.s(22.0)));
        let ct = Text::new(context, Role::Label, s.t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, cr, &ct, s.tk());
    });
    // Right to left: close, details, size, sort; the search takes what is left.
    let h = s.s(28.0);
    let y = r.center().y - h * 0.5;
    let mut x = r.right() - pad;
    let close = Rect::from_min_size(pos2(x - h, y), vec2(h, h));
    x = close.left() - s.s(8.0);
    let dt = Rect::from_min_size(pos2(x - h, y), vec2(h, h));
    x = dt.left() - s.s(12.0);
    let size = Rect::from_min_size(
        pos2(x - s.s(96.0), y + s.s(2.0)),
        vec2(s.s(96.0), s.s(24.0)),
    );
    x = size.left() - s.s(10.0);
    let sort = Rect::from_min_size(
        pos2(x - s.s(320.0), y + s.s(2.0)),
        vec2(s.s(320.0), s.s(24.0)),
    );
    x = sort.left() - s.s(16.0);
    let left = r.left() + pad + s.s(300.0) + s.s(16.0);
    let search = Rect::from_min_max(pos2(left, y), pos2(x.max(left + s.s(160.0)), y + h));
    v.geo.close = close;
    v.geo.details_toggle = dt;
    v.geo.size = size;
    v.geo.sort = sort;
    v.geo.search = search;
    let closed = key(
        ui,
        close,
        Id::new("picker-close"),
        Glyph::Close,
        false,
        &t("picker.close"),
    )
    .clicked();
    if key(
        ui,
        dt,
        Id::new("picker-details-toggle"),
        Glyph::Inspector,
        v.prefs.details,
        &t("picker.details.toggle"),
    )
    .clicked()
    {
        v.prefs.details = !v.prefs.details;
        v.store_prefs();
    }
    let sizes: Vec<Src> = TileSize::ALL.iter().map(|z| Src::Key(z.key())).collect();
    let cur = TileSize::ALL
        .iter()
        .position(|z| *z == v.prefs.tile)
        .unwrap_or(1);
    let name_size = t("picker.size");
    let (_, pick) = Segmented::new(cur, &sizes)
        .name(&name_size)
        .id(Id::new("picker-size"))
        .show_at(ui, size);
    if let Some(i) = pick {
        v.prefs.tile = TileSize::ALL[i];
        v.reveal = true;
        v.store_prefs();
    }
    let sorts: Vec<Src> = Sort::ALL.iter().map(|z| Src::Key(z.key())).collect();
    let cur = Sort::ALL
        .iter()
        .position(|z| *z == v.query.sort)
        .unwrap_or(0);
    let name_sort = t("picker.sort");
    let (_, pick) = Segmented::new(cur, &sorts)
        .name(&name_sort)
        .id(Id::new("picker-sort"))
        .show_at(ui, sort);
    if let Some(i) = pick {
        v.query.sort = Sort::ALL[i];
        v.prefs.sort = Sort::ALL[i];
        v.reveal = true;
        v.store_prefs();
    }
    search_field(v, ui, search, s);
    closed
}

/// The search: a well with a magnifier, the words, and a clear key.
fn search_field(v: &mut PickerView, ui: &mut Ui, r: Rect, s: &Style) {
    let clear = Rect::from_center_size(
        pos2(r.right() - s.s(14.0), r.center().y),
        egui::Vec2::splat(s.s(18.0)),
    );
    let mut text = v.query.text.clone();
    let n = v
        .catalog
        .iter()
        .filter(|d| v.query.role.is_none_or(|r| d.role == r))
        .count();
    let hint = tf("picker.search", &[("n", &n)]);
    // The kit's field (round 26), with room for the magnifier and the clear key.
    let resp = crate::kit::field::Field::new(search_id())
        .hint(&hint)
        .pad(22.0, (r.right() - clear.left()) / s.k - 2.0)
        .show(ui, r, &mut text)
        .resp;
    with_brush(ui.painter(), |b| {
        let g = Rect::from_center_size(
            pos2(r.left() + s.s(15.0), r.center().y),
            egui::Vec2::splat(s.s(13.0)),
        );
        magnifier(b, g, s.t.text_dim, s.s(1.6));
    });
    if resp.clicked() {
        v.editing = true;
    }
    if !v.editing && resp.has_focus() {
        resp.surrender_focus();
    }
    if text != v.query.text {
        v.set_text(&text);
    }
    if !v.query.text.is_empty() {
        let c = ui.interact(clear, Id::new("picker-search-clear"), Sense::click());
        with_brush(ui.painter(), |b| {
            let ink = if c.hovered() { s.t.text } else { s.t.text_dim };
            crate::kit::glyph::paint(b, clear.shrink(s.s(4.0)), Glyph::Close, ink);
        });
        if c.clicked() {
            v.set_text("");
        }
    }
}

/// ⌕ drawn in lines: a ring and its handle.
pub fn magnifier(b: &mut Brush, r: Rect, c: egui::Color32, w: f32) {
    let rad = r.width() * 0.34;
    let ctr = r.left_top() + vec2(rad + w, rad + w);
    b.circle_stroke(ctr, rad, w, c);
    let d = std::f32::consts::FRAC_1_SQRT_2;
    b.segment(
        ctr + vec2(rad * d, rad * d),
        r.right_bottom() - vec2(w * 0.5, w * 0.5),
        w * 1.2,
        c,
        true,
    );
}

fn foot(v: &mut PickerView, ui: &mut Ui, r: Rect, s: &Style, verb: &str) -> Option<Action> {
    let pad = s.s(16.0);
    let shown = v.geo.tiles.len();
    let count = if shown == 1 {
        t("picker.count.one")
    } else {
        tf("picker.count", &[("n", &shown)])
    };
    with_brush(ui.painter(), |b| {
        b.register(r, "picker-footer", &[Tok::Panel, Tok::TextDim], &[]);
        b.hline(
            r.left(),
            r.right(),
            b.snap_mid(r.top()),
            b.px(),
            shade(s.t.panel, -0.1),
        );
        let cr = Rect::from_min_size(pos2(r.left() + pad, r.top()), vec2(s.s(110.0), r.height()));
        let _ = text::paint(
            b,
            cr,
            &Text::new(&count, Role::Control, s.t.text).align(Align2::LEFT_CENTER),
            s.tk(),
        );
        let hr = Rect::from_min_max(
            pos2(cr.right() + s.s(8.0), r.top()),
            pos2(r.right() - s.s(260.0), r.bottom()),
        );
        let keys = t("picker.hint.keys");
        let _ = text::paint(
            b,
            hr,
            &Text::new(&keys, Role::Label, s.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    let h = s.s(28.0);
    let y = r.center().y - h * 0.5;
    let choose = Rect::from_min_size(pos2(r.right() - pad - s.s(112.0), y), vec2(s.s(112.0), h));
    let cancel = Rect::from_min_size(
        pos2(choose.left() - s.s(10.0) - s.s(92.0), y),
        vec2(s.s(92.0), h),
    );
    v.geo.choose = choose;
    v.geo.cancel = cancel;
    let mut out = None;
    if button(
        ui,
        cancel,
        Id::new("picker-cancel"),
        &t("picker.cancel"),
        false,
        true,
    )
    .clicked()
    {
        out = Some(Action::Close);
    }
    let can = v.selection().is_some();
    if button(ui, choose, Id::new("picker-choose"), verb, true, can).clicked() {
        out = v.selected.clone().map(Action::Choose);
    }
    out
}
