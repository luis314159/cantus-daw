//! Pieces the mock screens share: the title row, FL's dock divider, the label | value
//! row grammar (ADR 0024 §7).

use cantus_ui::kit::color::shade;
use cantus_ui::kit::glyph::{self, Glyph};
use cantus_ui::kit::text::{self, Elide, Policy, Role, Text};
use cantus_ui::kit::{well, Brush, Fill, State, Style};
use egui::{Align2, Pos2, Rect, Vec2};

/// A window's title row: ▸ menu at the left; "<window> – <subject>" with the window part
/// dimmed, at the right when `right`, after the menu otherwise.
pub fn title_bar(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    window_key: &str,
    subject: &str,
    right: bool,
) {
    let t = &s.t;
    b.rect(
        b.snap_rect(rect),
        Fill::V(shade(t.panel, 0.03), shade(t.panel, -0.01)),
    );
    b.hline(
        rect.left(),
        rect.right(),
        rect.top() + b.px() * 0.5,
        b.px(),
        shade(t.panel, 0.08),
    );
    b.hline(
        rect.left(),
        rect.right(),
        rect.bottom() - b.px() * 0.5,
        b.px(),
        shade(t.panel, -0.15),
    );
    glyph::paint(
        b,
        Rect::from_center_size(
            Pos2::new(rect.left() + 14.0, rect.center().y),
            Vec2::splat(14.0),
        ),
        Glyph::Menu,
        t.text,
    );
    let win = Text::key(window_key, Role::Title, t.text_dim).align(Align2::LEFT_CENTER);
    let subj = Text::new(subject, Role::Title, t.text)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::Middle));
    let ww = text::measure(b.ctx(), &win, 1.0).x;
    let dash = Text::new("–", Role::Title, t.text_dim).align(Align2::LEFT_CENTER);
    let sw = text::measure(b.ctx(), &subj, 1.0).x;
    let total = ww + 14.0 + sw;
    let x = if right {
        rect.right() - 12.0 - total
    } else {
        rect.left() + 30.0
    };
    let r = |x0: f32, w: f32| {
        Rect::from_min_size(Pos2::new(x0, rect.top()), Vec2::new(w + 1.0, rect.height()))
    };
    let _ = text::paint(b, r(x, ww), &win, 1.0);
    let _ = text::paint(b, r(x + ww + 4.0, 8.0), &dash, 1.0);
    let _ = text::paint(b, r(x + ww + 14.0, sw), &subj, 1.0);
}

/// The mixer's tools in its title row: add a track, and the width modes.
pub fn mixer_tools(b: &mut Brush, left_mid: Pos2, s: &Style) {
    let t = &s.t;
    let add = Rect::from_center_size(Pos2::new(left_mid.x + 10.0, left_mid.y), Vec2::splat(20.0));
    cantus_ui::kit::light::raised(
        b,
        add,
        3.0,
        shade(t.toolbar, -0.04),
        cantus_ui::kit::light::RaisedOpts::default(),
    );
    glyph::paint(b, add.shrink(5.0), Glyph::Plus, t.text);
    let seg = Rect::from_min_size(
        Pos2::new(add.right() + 8.0, left_mid.y - 10.0),
        Vec2::new(186.0, 20.0),
    );
    let labels = [
        cantus_ui::kit::text::Src::Key("kit.mixer.width.compact"),
        cantus_ui::kit::text::Src::Key("kit.mixer.width.wide"),
        cantus_ui::kit::text::Src::Key("kit.mixer.width.xl"),
    ];
    cantus_ui::kit::segmented::paint(
        b,
        seg,
        &s.hue_of(cantus_ui::kit::Tok::Highlight),
        State::REST,
        &labels,
        0,
        None,
    );
}

/// FL's dock divider: four columns of pixels, dark in the middle.
pub fn dock_divider(b: &mut Brush, x: f32, y0: f32, y1: f32, s: &Style) {
    let p = s.t.panel;
    let px = b.px();
    for (i, d) in [-0.10, -0.10, -0.18, -0.08].iter().enumerate() {
        b.vline(x + i as f32 * px, y0, y1, px, shade(p, *d));
    }
}

/// One label | value row, 24 pt: the label right-aligned in a fixed column, the value in
/// a well. `dim` draws a row that does not apply (dimmed, never hidden).
pub fn row(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    label_key: &str,
    value: &str,
    numeric: bool,
    dim: bool,
) {
    let t = &s.t;
    let col = 92.0;
    let a = if dim { 0.45 } else { 1.0 };
    let lr = Rect::from_min_max(rect.min, Pos2::new(rect.left() + col, rect.bottom()));
    let lt = Text::key(label_key, Role::Body, t.text_dim.gamma_multiply(a))
        .align(Align2::RIGHT_CENTER)
        .policy(Policy::Abbreviate);
    let _ = text::paint(b, lr, &lt, 1.0);
    let vr = Rect::from_min_max(
        Pos2::new(rect.left() + col + 8.0, rect.top() + 2.0),
        Pos2::new(rect.right(), rect.bottom() - 2.0),
    );
    let st = State {
        disabled: dim,
        ..State::REST
    };
    well::readout(b, vr, s, st, value, numeric);
}
