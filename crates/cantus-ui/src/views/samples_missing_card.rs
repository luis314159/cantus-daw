//! The missing-pack cards (see `samples_missing.rs`): at the lower left, above the status
//! bar, in the middle layer — so a window opened over them (*Sound packs…* above all,
//! which their own button opens) sits above them and takes its clicks. While *Sound
//! packs…* is open they step aside altogether.

use super::{dismiss, refresh, relink, relink_to, shown, Why};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{led, with_brush, Fill, Style, Tok};
use crate::views::audio_parts as parts;
use egui::{Align2, Id, Pos2, Rect, Sense, Vec2};

/// A card's width at 100 %.
const WIDTH: f32 = 380.0;

/// What a card's button does.
enum Act {
    Install(String),
    Relink(String),
    Locate(String),
    Dismiss(String),
}

/// Scan when due, and draw the cards.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    refresh(app);
    if app.samples.window.open {
        return;
    }
    let cards = shown(app);
    if cards.is_empty() {
        return;
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let status = (crate::views::workspace_status::HEIGHT * s.t.density).round();
    let mut bottom = screen.bottom() - status - 10.0 * k;
    let width = (WIDTH * k).min(screen.width() - 24.0 * k);
    let mut act: Option<Act> = None;
    for (i, m) in cards.iter().enumerate() {
        let tracks = m
            .labels
            .iter()
            .map(|n| tf("samples.missing.track", &[("name", n)]))
            .collect::<Vec<_>>()
            .join(", ");
        let words: [(&str, &dyn std::fmt::Display); 2] = [("pack", &m.name), ("tracks", &tracks)];
        let (title, body, tok, buttons) = match m.why {
            Why::NotInstalled => (
                t("samples.missing.title"),
                tf("samples.missing.body", &words),
                Tok::Danger,
                vec![
                    (t("samples.missing.install"), Act::Install(m.pack.clone())),
                    (t("samples.missing.locate"), Act::Locate(m.pack.clone())),
                ],
            ),
            Why::Incomplete => (
                t("samples.missing.title_incomplete"),
                tf("samples.missing.incomplete", &words),
                Tok::Danger,
                vec![
                    (t("samples.missing.reinstall"), Act::Install(m.pack.clone())),
                    (t("samples.missing.locate"), Act::Locate(m.pack.clone())),
                ],
            ),
            Why::Elsewhere(_) => (
                t("samples.missing.title_elsewhere"),
                tf("samples.missing.elsewhere", &words),
                Tok::Focus,
                vec![(t("samples.missing.relink"), Act::Relink(m.pack.clone()))],
            ),
        };
        let mut buttons = buttons;
        buttons.push((t("samples.missing.not_now"), Act::Dismiss(m.pack.clone())));
        // The buttons flow onto as many rows as the card's width needs.
        let inner = width - 32.0 * k;
        let mut rows: Vec<Vec<(f32, String, Act)>> = vec![Vec::new()];
        let mut x = 0.0;
        for (label, a) in buttons {
            let w = (text::measure(ctx, &Text::new(&label, Role::Control, s.t.text), s.tk()).x
                + 24.0 * k)
                .max(70.0 * k)
                .min(inner);
            if x > 0.0 && x + w > inner {
                rows.push(Vec::new());
                x = 0.0;
            }
            x += w + 6.0 * k;
            rows.last_mut().expect("a row").push((w, label, a));
        }
        let g = parts::wrapped(ctx, &body, Role::Label, s.tk(), width - 40.0 * k, 8);
        let rows_h = rows.len() as f32 * 28.0 * k;
        let h = 34.0 * k + g.size().y + 8.0 * k + rows_h + 6.0 * k;
        let rect = Rect::from_min_size(
            Pos2::new(screen.left() + 12.0 * k, bottom - h),
            Vec2::new(width, h),
        );
        bottom = rect.top() - 8.0 * k;
        egui::Area::new(Id::new(("cantus-missing-pack", i)))
            .order(egui::Order::Middle)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let (r, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
                parts::panel(ui, r, &s);
                with_brush(ui.painter(), |b| {
                    let edge = Rect::from_min_size(r.min, Vec2::new(3.0 * k, r.height()))
                        .shrink2(Vec2::new(0.0, 8.0 * k));
                    b.rrect(edge, 1.5 * k, Fill::Solid(s.t.get(tok)));
                    led::paint(
                        b,
                        Pos2::new(r.left() + 20.0 * k, r.top() + 19.0 * k),
                        8.0 * k,
                        true,
                        &s,
                        tok,
                        Tok::Panel,
                        1.0,
                    );
                    let tr = Rect::from_min_max(
                        Pos2::new(r.left() + 32.0 * k, r.top() + 8.0 * k),
                        Pos2::new(r.right() - 12.0 * k, r.top() + 30.0 * k),
                    );
                    let _ = text::paint(
                        b,
                        tr,
                        &Text::new(&title, Role::Body, s.t.text)
                            .align(Align2::LEFT_CENTER)
                            .policy(Policy::Elide(Elide::End)),
                        s.tk(),
                    );
                    b.text(
                        Pos2::new(b.snap(r.left() + 20.0 * k), b.snap(tr.bottom() + 4.0 * k)),
                        g.clone(),
                        s.t.text_dim,
                        0.0,
                    );
                });
                let mut y = r.bottom() - 6.0 * k - rows_h;
                for (row_i, row) in rows.into_iter().enumerate() {
                    let mut bx = r.left() + 20.0 * k;
                    for (j, (w, label, a)) in row.into_iter().enumerate() {
                        let br =
                            Rect::from_min_size(Pos2::new(bx, y + 3.0 * k), Vec2::new(w, 22.0 * k));
                        bx = br.right() + 6.0 * k;
                        let id = Id::new(("missing-pack-b", i, row_i, j));
                        let resp = ui.interact(br, id, Sense::click());
                        parts::paint_button(ui, br, &resp, &label, row_i == 0 && j == 0, true);
                        if resp.clicked() {
                            act = Some(a);
                        }
                    }
                    y += 28.0 * k;
                }
            });
    }
    match act {
        Some(Act::Install(pack)) => {
            app.samples.window.open = true;
            app.samples.window.selected = Some(pack);
            app.samples.window.confirm_remove = None;
        }
        Some(Act::Relink(pack)) => {
            relink(app, &pack);
        }
        Some(Act::Locate(pack)) => {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                if !relink_to(app, &pack, &folder) {
                    app.set_status(t("samples.missing.not_there"));
                }
            }
        }
        Some(Act::Dismiss(pack)) => dismiss(app, &pack),
        None => {}
    }
}
