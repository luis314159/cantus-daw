//! The top bar's agent chip (ADR 0081 §10, *visible agent mode*): shown while an agent is
//! connected — its name ("AI · Claude Code", or how many), a lamp lit while one of its calls
//! waits on the card. A click opens the AI activity (where it is disconnected and its
//! changes undone); a right-click opens Settings ▸ AI. Hidden while the AI area is off.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::shade;
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{led, with_brush, State, Style, Tok};
use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};

/// The chip's width at 100 %: room for "AI · " and a client's name, elided in its middle.
pub const WIDTH: f32 = 150.0;

/// Its group's width on the bar.
pub fn width(s: &Style) -> f32 {
    (WIDTH * s.k).ceil()
}

/// Whether the bar shows the chip: the AI area on and an agent connected.
pub fn in_use(app: &CantusApp) -> bool {
    !super::connected(app).is_empty()
}

/// What the chip says.
pub fn words(app: &CantusApp) -> String {
    let agents = super::connected(app);
    match agents.as_slice() {
        [] => t("ai.chip.none"),
        [one] => tf("ai.chip.one", &[("name", &one.name)]),
        many => tf("ai.chip.many", &[("n", &many.len())]),
    }
}

/// Draw the chip in `rect`.
pub fn show(app: &mut CantusApp, ui: &mut Ui, rect: Rect, s: &Style) {
    let chip = Rect::from_center_size(
        rect.center(),
        Vec2::new(
            rect.width() - 4.0 * s.k,
            s.d(22.0).min(rect.height() - 2.0 * s.k),
        ),
    );
    let resp = ui.interact(chip, ui.id().with("ai-chip"), Sense::click());
    if resp.clicked() {
        app.ai.activity_open = !app.ai.activity_open;
    }
    if resp.secondary_clicked() {
        crate::views::workspace_settings::open(
            app,
            crate::views::workspace_settings::SettingsPage::Ai,
        );
    }
    let waiting = app.ai.agents().is_some_and(|a| a.asks().next().is_some());
    if resp.hovered() {
        let names: Vec<String> = super::connected(app).into_iter().map(|c| c.name).collect();
        let body = tf("ai.chip.tip", &[("names", &names.join(", "))]);
        hint::set_text(
            ui.ctx(),
            &t("ai.chip.title"),
            &body,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    let label = words(app);
    let st = State::of(&resp, app.ai.activity_open);
    with_brush(ui.painter(), |b| {
        b.register(chip, "ai-chip", &[Tok::Panel, Tok::Text, Tok::Led], &[]);
        let base = if st.lit {
            shade(s.t.panel_raised, 0.07)
        } else if st.hovered {
            shade(s.t.panel_raised, 0.04)
        } else {
            shade(s.t.panel, 0.03)
        };
        light::raised(
            b,
            chip,
            s.r(4.0),
            base,
            RaisedOpts {
                drop: false,
                k: s.k,
                depth: s.t.depth * 0.7,
                ..Default::default()
            },
        );
        let lamp = Pos2::new(chip.left() + s.s(11.0), chip.center().y);
        let tok = if waiting { Tok::Focus } else { Tok::Led };
        led::paint(b, lamp, s.s(8.0), true, s, tok, Tok::Panel, 1.0);
        let g = Rect::from_center_size(
            Pos2::new(chip.left() + s.s(26.0), chip.center().y),
            Vec2::splat(s.s(13.0)),
        );
        glyph::paint(b, g, Glyph::Agent, s.t.highlight);
        let tr = Rect::from_min_max(
            Pos2::new(chip.left() + s.s(36.0), chip.top()),
            Pos2::new(chip.right() - s.s(6.0), chip.bottom()),
        );
        let _ = text::paint(
            b,
            tr,
            &Text::new(&label, Role::Label, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle)),
            s.tk(),
        );
    });
    if waiting {
        crate::views::workspace_ticker::wake_after(ui.ctx(), std::time::Duration::from_millis(500));
    }
}
