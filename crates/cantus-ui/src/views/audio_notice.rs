//! Cards at the corner of the window (round 20): an interface was plugged in — switch to
//! it? — the one in use was unplugged — keep working, or use the computer's own audio
//! meanwhile — and it came back. They stack above the status bar at the right, under the
//! chip, and wait for an answer (except *reconnected*, which fades on its own).

use super::audio_parts as parts;
use crate::app::CantusApp;
use crate::audio_setup::{self as setup, interfaces, problems::Fix, Hit, Notice};
use crate::i18n::{t, tf};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{led, with_brush, Fill, Style, Tok};
use egui::{Align2, Id, Pos2, Rect, Sense, Vec2};

/// A card's width at 100 %.
const WIDTH: f32 = 340.0;

/// What a card's button does.
enum Act {
    Switch(String),
    Dismiss,
    UseDefault,
}

/// The cards, newest at the bottom.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    if app.audio_setup.notices.is_empty() || app.audio_setup.first_run.is_some() {
        return;
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let status = (super::workspace_status::HEIGHT * s.t.density).round();
    let mut bottom = screen.bottom() - status - 10.0 * k;
    let notices = app.audio_setup.notices.clone();
    let mut act: Option<(usize, Act)> = None;
    for (i, (n, _)) in notices.iter().enumerate().rev() {
        let (title, body, tok, kind, buttons): (
            String,
            String,
            Tok,
            interfaces::Kind,
            Vec<(String, Act)>,
        ) = match n {
            Notice::Connected(key) => (
                tf("devices.notice.connected", &[("name", key)]),
                t("devices.notice.connected_body"),
                Tok::Focus,
                interfaces::kind_of(key),
                vec![
                    (t("devices.notice.switch"), Act::Switch(key.clone())),
                    (t("devices.notice.not_now"), Act::Dismiss),
                ],
            ),
            Notice::Lost(key) => (
                tf("devices.notice.lost", &[("name", key)]),
                t("devices.notice.lost_body"),
                Tok::Danger,
                interfaces::kind_of(key),
                vec![
                    (Fix::UseDefault.label(), Act::UseDefault),
                    (t("devices.notice.ok"), Act::Dismiss),
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
        let g = parts::wrapped(ctx, &body, Role::Label, s.tk(), (WIDTH - 92.0) * k, 3);
        let h = 34.0 * k
            + if body.is_empty() {
                0.0
            } else {
                g.size().y + 4.0 * k
            }
            + if buttons.is_empty() { 0.0 } else { 32.0 * k };
        let h = h.max(58.0 * k);
        let rect = Rect::from_min_size(
            Pos2::new(screen.right() - WIDTH * k - 12.0 * k, bottom - h),
            Vec2::new(WIDTH * k, h),
        );
        bottom = rect.top() - 8.0 * k;
        egui::Area::new(Id::new(("cantus-audio-notice", i)))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let (r, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
                parts::panel(ui, r, &s);
                with_brush(ui.painter(), |b| {
                    let edge = Rect::from_min_size(r.min, Vec2::new(3.0 * k, r.height()))
                        .shrink2(Vec2::new(0.0, 8.0 * k));
                    b.rrect(edge, 1.5 * k, Fill::Solid(s.t.get(tok)));
                    let pic = Rect::from_min_size(
                        r.min + Vec2::new(12.0 * k, 10.0 * k),
                        Vec2::new(60.0 * k, 38.0 * k),
                    );
                    b.rrect(pic, s.r(4.0), Fill::Solid(s.t.well));
                    super::audio_art::paint(b, pic.shrink(4.0 * k), &s, kind, tok != Tok::Danger);
                    led::paint(
                        b,
                        Pos2::new(r.left() + 84.0 * k, r.top() + 19.0 * k),
                        8.0 * k,
                        true,
                        &s,
                        tok,
                        Tok::Panel,
                        1.0,
                    );
                    let tr = Rect::from_min_max(
                        Pos2::new(r.left() + 94.0 * k, r.top() + 8.0 * k),
                        Pos2::new(r.right() - 30.0 * k, r.top() + 30.0 * k),
                    );
                    let _ = text::paint(
                        b,
                        tr,
                        &Text::new(&title, Role::Body, s.t.text)
                            .align(Align2::LEFT_CENTER)
                            .policy(Policy::Elide(Elide::End)),
                        s.tk(),
                    );
                    if !body.is_empty() {
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
                let xr = ui.interact(x, Id::new(("audio-notice-x", i)), Sense::click());
                with_brush(ui.painter(), |b| {
                    let c = if xr.hovered() { s.t.text } else { s.t.text_dim };
                    glyph::paint(b, x.shrink(4.0 * k), Glyph::Close, c);
                });
                if xr.clicked() {
                    act = Some((i, Act::Dismiss));
                }
                let mut bx = r.left() + 84.0 * k;
                for (j, (label, a)) in buttons.into_iter().enumerate() {
                    let tw = text::measure(
                        ui.ctx(),
                        &Text::new(&label, Role::Control, s.t.text),
                        s.tk(),
                    )
                    .x + 24.0 * k;
                    let br = Rect::from_min_size(
                        Pos2::new(bx, r.bottom() - 32.0 * k),
                        Vec2::new(tw.max(70.0 * k), 22.0 * k),
                    );
                    bx = br.right() + 6.0 * k;
                    let resp = ui.interact(br, Id::new(("audio-notice-b", i, j)), Sense::click());
                    parts::paint_button(ui, br, &resp, &label, j == 0, true);
                    app.audio_setup.hit(ui.ctx(), Hit::Notice(i, j), br);
                    if resp.clicked() {
                        act = Some((i, a));
                    }
                }
            });
    }
    match act {
        Some((_, Act::Switch(key))) => setup::choose_interface(app, &key),
        Some((_, Act::UseDefault)) => setup::fix(app, &Fix::UseDefault),
        Some((i, Act::Dismiss)) if i < app.audio_setup.notices.len() => {
            app.audio_setup.notices.remove(i);
        }
        Some((_, Act::Dismiss)) => {}
        None => {}
    }
}
