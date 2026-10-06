//! *Get a low-latency driver* (round 20): FlexASIO's latest release page, ASIO4ALL's page,
//! the interface makers' driver pages (the detected maker first), the ASIO drivers
//! already installed, and an obvious *Rescan* — Cantus also rescans when its window gets
//! the focus back after an installer ran outside it.

use super::audio_parts as parts;
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::audio_setup::drivers;
use crate::audio_setup::{self as setup, Hit};
use crate::i18n::{t, tf};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Fill, Style, Tok};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

const SIZE: Vec2 = Vec2::new(600.0, 640.0);

/// The window, when open.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    if !app.audio_setup.drivers {
        return;
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let size = (SIZE * k).min(screen.size() - Vec2::new(40.0, 60.0));
    let rect = Rect::from_center_size(screen.center(), size);
    let mut open = true;
    egui::Area::new(Id::new("cantus-audio-drivers"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 32.0 * k));
            if super::workspace_settings::title(
                ui,
                head,
                &s,
                Glyph::Device,
                "devices.drivers.title",
            ) {
                open = false;
            }
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 18.0 * k, head.bottom() + 10.0 * k),
                Pos2::new(rect.right() - 10.0 * k, rect.bottom() - 12.0 * k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .id_salt("cantus-audio-drivers-body")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(body.width() - 14.0 * k);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0 * k, 7.0 * k);
                    page(app, ui);
                });
        });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.audio_setup.drivers = false;
    }
}

fn page(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    parts::paragraph(ui, &t("devices.drivers.intro"), Role::Body, s.t.text_dim);
    // What is installed now, and whether this build can use it.
    let installed = app
        .audio_setup
        .scan
        .as_ref()
        .map(|s| s.asio.clone())
        .unwrap_or_default();
    let line = if installed.is_empty() {
        t("devices.drivers.none_installed")
    } else {
        tf(
            "devices.drivers.installed",
            &[("list", &installed.join(", "))],
        )
    };
    ui.horizontal(|ui| {
        let r = parts::button(
            ui,
            &t("devices.drivers.rescan"),
            true,
            !app.audio_setup.is_scanning(),
            110.0,
        );
        app.audio_setup.hit(ui.ctx(), Hit::Rescan, r.rect);
        if r.clicked() {
            app.audio_setup.request_scan();
        }
        parts::paragraph(ui, &line, Role::Body, s.t.text);
    });
    if !cantus_engine::devices::asio_built() {
        warn(ui, &t("devices.drivers.no_asio_build"));
    } else if !installed.is_empty() {
        super::workspace_settings::note(ui, &t("devices.drivers.where"), false);
    }
    // 1. FlexASIO.
    w::band(ui, Src::Key("devices.drivers.flex"));
    parts::paragraph(
        ui,
        &t("devices.drivers.flex_body"),
        Role::Body,
        s.t.text_dim,
    );
    ui.horizontal(|ui| {
        let r = parts::button(ui, &t("devices.drivers.flex_open"), true, true, 200.0);
        app.audio_setup.hit(ui.ctx(), Hit::FlexPage, r.rect);
        if r.clicked() {
            drivers::open_url(drivers::FLEXASIO_PAGE);
        }
        parts::paragraph(
            ui,
            &t("devices.drivers.flex_how"),
            Role::Label,
            s.t.text_dim,
        );
    });
    // 2. ASIO4ALL.
    w::band(ui, Src::Key("devices.drivers.a4a"));
    ui.horizontal(|ui| {
        let r = parts::button(ui, &t("devices.drivers.a4a_open"), false, true, 160.0);
        app.audio_setup.hit(ui.ctx(), Hit::Asio4All, r.rect);
        if r.clicked() {
            drivers::open_url(drivers::ASIO4ALL_URL);
        }
        parts::paragraph(
            ui,
            &t("devices.drivers.a4a_body"),
            Role::Label,
            s.t.text_dim,
        );
    });
    // 3. The interface's own driver.
    w::band(ui, Src::Key("devices.drivers.own"));
    let names: Vec<String> = app
        .audio_setup
        .scan
        .as_ref()
        .map(|s| s.interfaces.iter().map(|i| i.key.clone()).collect())
        .unwrap_or_default();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let (makers, detected) = drivers::makers_for(&refs);
    if detected.is_none() {
        super::workspace_settings::note(ui, &t("devices.drivers.own_hint"), false);
    }
    for (i, m) in makers.iter().enumerate() {
        if maker_row(ui, i, m, detected == Some(m.name)) {
            drivers::open_url(m.url);
        }
        let r = app.audio_setup.geo.hits.len();
        let _ = r;
    }
    if detected.is_some() {
        let _ = setup::current_key(app);
    }
}

/// A warning line in the danger colour, wrapped.
fn warn(ui: &mut Ui, s: &str) {
    let st = Style::current();
    parts::paragraph(ui, s, Role::Body, st.t.danger);
}

/// A maker: name, its page's address dimmed, an arrow; the detected one marked.
fn maker_row(ui: &mut Ui, i: usize, m: &drivers::Maker, detected: bool) -> bool {
    let s = Style::current();
    let k = s.k;
    let (r, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0 * k), Sense::click());
    with_brush(ui.painter(), |b| {
        if resp.hovered() || detected {
            let a = if detected { 0.22 } else { 0.10 };
            b.rrect(
                r,
                s.r(3.0),
                Fill::Solid(crate::kit::color::with_alpha(s.t.selection, a)),
            );
        }
        let name = Rect::from_min_max(
            Pos2::new(r.left() + 10.0 * k, r.top()),
            Pos2::new(r.left() + 190.0 * k, r.bottom()),
        );
        let _ = text::paint(
            b,
            name,
            &Text::new(m.name, Role::Body, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        let url = Rect::from_min_max(
            Pos2::new(name.right() + 8.0 * k, r.top()),
            Pos2::new(r.right() - 110.0 * k, r.bottom()),
        );
        let _ = text::paint(
            b,
            url,
            &Text::new(m.url, Role::Label, s.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle)),
            s.tk(),
        );
        if detected {
            let tag = t("devices.drivers.detected");
            let tr = Rect::from_min_max(
                Pos2::new(r.right() - 104.0 * k, r.top()),
                Pos2::new(r.right() - 24.0 * k, r.bottom()),
            );
            let _ = text::paint(
                b,
                tr,
                &Text::new(&tag, Role::Label, s.t.text)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        }
        let g = Rect::from_center_size(
            Pos2::new(r.right() - 12.0 * k, r.center().y),
            Vec2::splat(10.0 * k),
        );
        glyph::paint(
            b,
            g,
            Glyph::Next,
            if resp.hovered() {
                s.t.text
            } else {
                s.t.text_dim
            },
        );
        let _ = Tok::Selection;
    });
    let ctx = ui.ctx().clone();
    let _ = (i, ctx);
    resp.clicked()
}
