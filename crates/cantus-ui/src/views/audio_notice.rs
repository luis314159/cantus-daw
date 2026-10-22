//! Cards at the corner of the window, in **one stack** (round 20; shared since round 29):
//! an interface was plugged in — switch to it? — the one in use was unplugged — keep
//! working, or use the computer's own audio meanwhile — and it came back; and, since round
//! 29 (ADR 0063), a hosted plug-in that stopped working — *Reload* or *Keep it off*
//! (`plugin_notice`). They stack above the status bar at the right, under the chip, each
//! above the one before, so two never overlap, and wait for an answer (except
//! *reconnected*, which fades on its own).

use super::audio_parts as parts;
use super::plugin_notice;
use crate::app::CantusApp;
use crate::audio_setup::{self as setup, interfaces, problems::Fix, Hit, Notice};
use crate::i18n::{t, tf};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{led, with_brush, Fill, Style, Tok};
use egui::{Align2, Id, Pos2, Rect, Sense, Vec2};

/// A card's width at 100 %.
const WIDTH: f32 = 340.0;

/// What a card's button does.
#[derive(Clone)]
enum Act {
    Switch(String),
    Dismiss(usize),
    UseDefault,
    Plugin(plugin_notice::Answer),
    /// A plug-in waiting on a window of its own (round 29, review isolation-4 #54).
    Waiting(plugin_notice::Target, plugin_notice::WaitAnswer),
    /// A render's Stop, a refused edit's Unfreeze (round 29, ADR 0064).
    Freeze(super::freeze_view::Answer),
}

/// The picture at a card's left.
#[derive(Copy, Clone)]
enum Art {
    Interface(interfaces::Kind, bool),
    /// A plug-in that stopped working.
    Plugin,
    /// A freeze: a render on its way, an edit it refused.
    Frozen,
}

/// One card of the stack.
struct Card {
    id: (&'static str, usize),
    /// A name drawn before the title, shortened in its middle when long (a plug-in card).
    name: Option<String>,
    title: String,
    body: String,
    tok: Tok,
    art: Art,
    buttons: Vec<(String, Act)>,
    /// What the ✕ does.
    close: Act,
    /// A hint on the whole card (the reasons a plug-in stopped), and its heading.
    tip: Option<(String, String)>,
    /// The audio setup's hit areas (its tests press the buttons through them).
    audio: Option<usize>,
}

/// The audio notices' cards, oldest first.
fn audio_cards(app: &CantusApp) -> Vec<Card> {
    if app.audio_setup.first_run.is_some() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, (n, _)) in app.audio_setup.notices.iter().enumerate() {
        let (title, body, tok, kind, buttons) = match n {
            Notice::Connected(key) => (
                tf("devices.notice.connected", &[("name", key)]),
                t("devices.notice.connected_body"),
                Tok::Focus,
                interfaces::kind_of(key),
                vec![
                    (t("devices.notice.switch"), Act::Switch(key.clone())),
                    (t("devices.notice.not_now"), Act::Dismiss(i)),
                ],
            ),
            Notice::Lost(key) => (
                tf("devices.notice.lost", &[("name", key)]),
                t("devices.notice.lost_body"),
                Tok::Danger,
                interfaces::kind_of(key),
                vec![
                    (Fix::UseDefault.label(), Act::UseDefault),
                    (t("devices.notice.ok"), Act::Dismiss(i)),
                ],
            ),
            Notice::Reconnected(key) => (
                tf("devices.notice.reconnected", &[("name", key)]),
                String::new(),
                Tok::Led,
                interfaces::kind_of(key),
                Vec::new(),
            ),
        };
        out.push(Card {
            id: ("audio", i),
            name: None,
            title,
            body,
            tok,
            art: Art::Interface(kind, tok != Tok::Danger),
            buttons,
            close: Act::Dismiss(i),
            tip: None,
            audio: Some(i),
        });
    }
    out
}

/// The plug-in card, when one waits.
fn plugin_card(app: &CantusApp) -> Option<Card> {
    let c = plugin_notice::card(app)?;
    let sentence = c.sentence();
    Some(Card {
        id: ("plugin", 0),
        name: c.name,
        title: c.title,
        body: c.body,
        tok: Tok::Danger,
        art: Art::Plugin,
        buttons: c
            .buttons
            .into_iter()
            .map(|(l, a)| (l, Act::Plugin(a)))
            .collect(),
        close: Act::Plugin(plugin_notice::Answer::KeepOff),
        tip: Some((sentence, c.reasons)),
        audio: None,
    })
}

/// The card of a plug-in waiting on a window of its own, while one does.
fn waiting_card(app: &CantusApp) -> Option<Card> {
    let c = plugin_notice::waiting_card(app)?;
    let target = c.target;
    Some(Card {
        id: ("waiting", 0),
        name: Some(c.name),
        title: c.title,
        body: c.body,
        tok: Tok::Focus,
        art: Art::Plugin,
        buttons: c
            .buttons
            .into_iter()
            .map(|(l, a)| (l, Act::Waiting(target, a)))
            .collect(),
        close: Act::Waiting(target, plugin_notice::WaitAnswer::Dismiss),
        tip: Some(c.tip),
        audio: None,
    })
}

/// The freeze card (round 29, ADR 0064), when a render runs or an edit was refused.
fn freeze_card(app: &CantusApp) -> Option<Card> {
    let (title, body, buttons, close) = super::freeze_view::card(app)?;
    Some(Card {
        id: ("freeze", 0),
        name: None,
        title,
        body,
        tok: Tok::Focus,
        art: Art::Frozen,
        buttons: buttons
            .into_iter()
            .map(|(l, a)| (l, Act::Freeze(a)))
            .collect(),
        close: Act::Freeze(close),
        tip: None,
        audio: None,
    })
}

/// The stack: the plug-in card at the bottom (it is the one that needs an answer now),
/// the freeze card, the audio cards above them, newest lowest; each card above the last,
/// never over it.
fn stack(app: &CantusApp, ctx: &egui::Context, s: &Style) -> Vec<(Card, Rect)> {
    let mut cards: Vec<Card> = plugin_card(app).into_iter().collect();
    cards.extend(waiting_card(app));
    cards.extend(freeze_card(app));
    cards.extend(audio_cards(app).into_iter().rev());
    let k = s.k;
    let screen = ctx.content_rect();
    let status = (super::workspace_status::HEIGHT * s.t.density).round();
    let mut bottom = screen.bottom() - status - 10.0 * k;
    cards
        .into_iter()
        .map(|card| {
            let rect = place(ctx, &card, s, screen.right(), bottom);
            bottom = rect.top() - 8.0 * k;
            (card, rect)
        })
        .collect()
}

/// Where each card of the stack goes, bottom first (for tests: no two overlap).
pub fn card_rects(app: &CantusApp, ctx: &egui::Context) -> Vec<Rect> {
    stack(app, ctx, &Style::current())
        .into_iter()
        .map(|(_, r)| r)
        .collect()
}

/// Each card's rectangle and its buttons' (for tests: every button inside its card).
pub fn card_layout(app: &CantusApp, ctx: &egui::Context) -> Vec<(Rect, Vec<Rect>)> {
    let s = Style::current();
    stack(app, ctx, &s)
        .into_iter()
        .map(|(card, r)| {
            let b = buttons_at(ctx, &card, r, &s)
                .into_iter()
                .map(|(_, br)| br)
                .collect();
            (r, b)
        })
        .collect()
}

/// Where a card's buttons go, by index: rows that fit, from the card's bottom up.
fn buttons_at(ctx: &egui::Context, card: &Card, r: Rect, s: &Style) -> Vec<(usize, Rect)> {
    let k = s.k;
    let rows = button_rows(ctx, card, s);
    let n = rows.len();
    let mut out = Vec::new();
    for (row, buttons) in rows.into_iter().enumerate() {
        let top = r.bottom() - (n - row) as f32 * ROW * k - 6.0 * k;
        let mut bx = r.left() + 84.0 * k;
        for (j, w) in buttons {
            let br = Rect::from_min_size(Pos2::new(bx, top + 3.0 * k), Vec2::new(w, 22.0 * k));
            bx = br.right() + 6.0 * k;
            out.push((j, br));
        }
    }
    out
}

pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    let s = Style::current();
    let cards = stack(app, ctx, &s);
    if cards.is_empty() {
        return;
    }
    let mut act: Option<Act> = None;
    for (card, rect) in &cards {
        if let Some(a) = draw(app, ctx, card, *rect, &s) {
            act = Some(a);
        }
    }
    match act {
        Some(Act::Switch(key)) => setup::choose_interface(app, &key),
        Some(Act::UseDefault) => setup::fix(app, &Fix::UseDefault),
        Some(Act::Dismiss(i)) if i < app.audio_setup.notices.len() => {
            app.audio_setup.notices.remove(i);
        }
        Some(Act::Dismiss(_)) => {}
        Some(Act::Plugin(a)) => plugin_notice::answer(app, a),
        Some(Act::Waiting(target, a)) => plugin_notice::answer_waiting(app, target, a),
        Some(Act::Freeze(a)) => super::freeze_view::answer(app, a),
        None => {}
    }
}

/// The buttons in rows that fit the card (a Spanish *Ejecutar por separado* goes to a
/// second row rather than past the card's edge): each row's buttons, by index and width.
fn button_rows(ctx: &egui::Context, card: &Card, s: &Style) -> Vec<Vec<(usize, f32)>> {
    let k = s.k;
    let room = (WIDTH - 84.0 - 12.0) * k;
    let mut rows: Vec<Vec<(usize, f32)>> = Vec::new();
    let mut used = room;
    for (j, (label, _)) in card.buttons.iter().enumerate() {
        let w = (text::measure(ctx, &Text::new(label, Role::Control, s.t.text), s.tk()).x
            + 24.0 * k)
            .max(70.0 * k)
            .min(room);
        if rows.is_empty() || used + 6.0 * k + w > room {
            rows.push(Vec::new());
            used = 0.0;
        } else {
            used += 6.0 * k;
        }
        used += w;
        if let Some(r) = rows.last_mut() {
            r.push((j, w));
        }
    }
    rows
}

/// The height of a row of buttons.
const ROW: f32 = 28.0;

/// The card's rectangle, its bottom at `bottom`, its right edge 12 points in from `right`.
fn place(ctx: &egui::Context, card: &Card, s: &Style, right: f32, bottom: f32) -> Rect {
    let k = s.k;
    let g = parts::wrapped(ctx, &card.body, Role::Label, s.tk(), (WIDTH - 92.0) * k, 3);
    let rows = button_rows(ctx, card, s).len();
    let h = 34.0 * k
        + if card.body.is_empty() {
            0.0
        } else {
            g.size().y + 4.0 * k
        }
        + if rows == 0 {
            0.0
        } else {
            4.0 * k + ROW * k * rows as f32
        };
    let h = h.max(58.0 * k);
    Rect::from_min_size(
        Pos2::new(right - WIDTH * k - 12.0 * k, bottom - h),
        Vec2::new(WIDTH * k, h),
    )
}

/// Draws one card; returns what was pressed.
fn draw(
    app: &mut CantusApp,
    ctx: &egui::Context,
    card: &Card,
    rect: Rect,
    s: &Style,
) -> Option<Act> {
    let k = s.k;
    let g = parts::wrapped(ctx, &card.body, Role::Label, s.tk(), (WIDTH - 92.0) * k, 3);
    let mut act = None;
    egui::Area::new(Id::new(("cantus-corner-card", card.id)))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (r, whole) = ui.allocate_exact_size(rect.size(), Sense::hover());
            parts::panel(ui, r, s);
            // The reasons, wherever the pointer is on the card (its buttons say their own).
            if let Some((heading, tip)) = &card.tip {
                if whole.hovered() {
                    hint::set_text(ui.ctx(), heading, tip, Caps::NONE);
                }
            }
            let pic = Rect::from_min_size(
                r.min + Vec2::new(12.0 * k, 10.0 * k),
                Vec2::new(60.0 * k, 38.0 * k),
            );
            let ui_ctx = &ui.ctx().clone();
            with_brush(ui.painter(), |b| {
                let edge = Rect::from_min_size(r.min, Vec2::new(3.0 * k, r.height()))
                    .shrink2(Vec2::new(0.0, 8.0 * k));
                b.rrect(edge, 1.5 * k, Fill::Solid(s.t.get(card.tok)));
                b.rrect(pic, s.r(4.0), Fill::Solid(s.t.well));
                match card.art {
                    Art::Interface(kind, lit) => {
                        super::audio_art::paint(b, pic.shrink(4.0 * k), s, kind, lit)
                    }
                    Art::Plugin => {
                        let g = Rect::from_center_size(pic.center(), Vec2::splat(26.0 * k));
                        glyph::paint(b, g, Glyph::PluginStopped, s.t.danger);
                    }
                    Art::Frozen => {
                        let g = Rect::from_center_size(pic.center(), Vec2::splat(28.0 * k));
                        glyph::paint(b, g, Glyph::Frozen, s.t.focus);
                    }
                }
                led::paint(
                    b,
                    Pos2::new(r.left() + 84.0 * k, r.top() + 19.0 * k),
                    8.0 * k,
                    true,
                    s,
                    card.tok,
                    Tok::Panel,
                    1.0,
                );
                let tr = Rect::from_min_max(
                    Pos2::new(r.left() + 94.0 * k, r.top() + 8.0 * k),
                    Pos2::new(r.right() - 30.0 * k, r.top() + 30.0 * k),
                );
                // A name first (shortened in its middle when long), then what happened, at
                // full width: "Spectra…Omnisphere stopped working" (#36).
                let mut title_rect = tr;
                if let Some(name) = &card.name {
                    let rest = text::measure(
                        ui_ctx,
                        &Text::new(&card.title, Role::Body, s.t.text),
                        s.tk(),
                    )
                    .x;
                    let room = (tr.width() - rest - 4.0 * k).max(24.0 * k);
                    let nr = Rect::from_min_size(tr.min, Vec2::new(room, tr.height()));
                    let used = text::paint(
                        b,
                        nr,
                        &Text::new(name, Role::Body, s.t.text)
                            .align(Align2::LEFT_CENTER)
                            .policy(Policy::Elide(Elide::Middle)),
                        s.tk(),
                    )
                    .map_or(0.0, |p| p.rect.width());
                    title_rect =
                        Rect::from_min_max(Pos2::new(tr.left() + used + 3.0 * k, tr.top()), tr.max);
                }
                let _ = text::paint(
                    b,
                    title_rect,
                    &Text::new(&card.title, Role::Body, s.t.text)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
                if !card.body.is_empty() {
                    b.text(
                        Pos2::new(b.snap(r.left() + 84.0 * k), b.snap(tr.bottom() + 2.0 * k)),
                        g.clone(),
                        s.t.text_dim,
                        0.0,
                    );
                }
            });

            // ✕ at the top right.
            let x = Rect::from_center_size(
                Pos2::new(r.right() - 16.0 * k, r.top() + 18.0 * k),
                Vec2::splat(18.0 * k),
            );
            let xr = ui.interact(x, Id::new(("corner-card-x", card.id)), Sense::click());
            with_brush(ui.painter(), |b| {
                let c = if xr.hovered() { s.t.text } else { s.t.text_dim };
                glyph::paint(b, x.shrink(4.0 * k), Glyph::Close, c);
            });
            if xr.clicked() {
                act = Some(card.close.clone());
            }
            for (j, br) in buttons_at(ui.ctx(), card, r, s) {
                let (label, a) = &card.buttons[j];
                let resp = ui.interact(br, Id::new(("corner-card-b", card.id, j)), Sense::click());
                parts::paint_button(ui, br, &resp, label, j == 0, true);
                if let Some(i) = card.audio {
                    app.audio_setup.hit(ui.ctx(), Hit::Notice(i, j), br);
                }
                if resp.clicked() {
                    act = Some(a.clone());
                }
            }
        });
    act
}
