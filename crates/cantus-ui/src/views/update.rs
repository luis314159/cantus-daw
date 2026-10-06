//! Updates (ADR 0030): the check at start, the notice, the window with the release notes,
//! the Settings rows, and handing over to the installer when the app closes.
//!
//! The work is `cantus_update`'s, on its own threads; this file only reads its
//! [`Status`] each frame and draws it with the kit. A copy built from source never checks
//! by itself (no installer to run, and a developer's machine is not a user's); a portable
//! copy is told and shown the release page; an installed copy downloads, verifies and
//! installs on quit.

use super::workspace_settings::{frame as window_face, note, title, LABEL_W};
use super::workspace_widgets as w;
use crate::app::{config_dir, data_dir, CantusApp, APP_VERSION};
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::led_button;
use crate::kit::{with_brush, Fill, Style};
use crate::views::workspace_ticker::{self as ticker, Reason};
use cantus_update::install::{self, InstallKind};
use cantus_update::notes::LineKind;
use cantus_update::release::Flavor;
use cantus_update::{net, schedule, updater, verify, Config, Status, Updater, Version};
use egui::{Align2, Color32, Context, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::sync::Arc;
use std::time::Duration;

/// The update window's and the notice's state.
pub struct UpdateUi {
    updater: Option<Updater>,
    started: bool,
    pub open: bool,
    /// Run the downloaded installer when Cantus closes (on unless the user unticks it).
    pub install_on_quit: bool,
    /// The notice was closed with ✕ for this session.
    notice_closed: bool,
    launched: bool,
    launch_error: Option<String>,
    kind: Option<InstallKind>,
    /// The proxy field's text while it is edited.
    proxy: Option<String>,
    /// A status to show instead of the updater's (the screenshots, `demo_setup`).
    demo: Option<Status>,
}

impl Default for UpdateUi {
    fn default() -> Self {
        UpdateUi {
            updater: None,
            started: false,
            open: false,
            install_on_quit: true,
            notice_closed: false,
            launched: false,
            launch_error: None,
            kind: None,
            proxy: None,
            demo: None,
        }
    }
}

impl UpdateUi {
    pub fn kind(&mut self) -> InstallKind {
        *self.kind.get_or_insert_with(InstallKind::current)
    }

    pub fn status(&self) -> Status {
        if let Some(d) = &self.demo {
            return d.clone();
        }
        self.updater.as_ref().map_or(Status::Idle, Updater::status)
    }
}

fn flavor() -> Flavor {
    if cantus_engine::devices::ASIO_BUILT {
        Flavor::Asio
    } else {
        Flavor::Standard
    }
}

fn ensure(app: &mut CantusApp) -> &Updater {
    let kind = app.update.kind();
    let proxy = app.settings.updates.proxy.clone();
    app.update.updater.get_or_insert_with(|| {
        let config = Config {
            repo: updater::REPO.into(),
            current: Version::parse(APP_VERSION).unwrap_or(Version {
                major: 0,
                minor: 0,
                patch: 0,
                pre: Vec::new(),
            }),
            flavor: flavor(),
            kind,
            state_path: schedule::state_path(&config_dir()),
            downloads: install::updates_dir(&data_dir()),
            publisher: verify::PUBLISHER.map(str::to_owned),
            held_retry: updater::HELD_RETRY,
        };
        Updater::new(config, Arc::new(net::Https::new(proxy.as_deref())))
    })
}

/// Help → Check for updates, and Settings' *Check now*: ask GitHub now, and show the
/// window.
pub fn check_now(app: &mut CantusApp) {
    ensure(app).check(true);
    app.update.open = true;
    app.update.notice_closed = false;
}

/// Each frame: the check at start, the repaint while something moves, the notice and
/// the window.
pub fn frame(app: &mut CantusApp, ctx: &Context) {
    if !app.update.started {
        app.update.started = true;
        if app.settings.updates.check_daily && app.update.kind() != InstallKind::Development {
            ensure(app).check(false);
        }
    }
    let status = app.update.status();
    match &status {
        Status::Checking | Status::Downloading { .. } => ticker::moving(ctx, Reason::Background),
        Status::Held { .. } => ticker::wake_after(ctx, Duration::from_secs(1)),
        _ => {}
    }
    if !app.update.open && !app.update.notice_closed {
        notice(app, ctx, &status);
    }
    if app.update.open {
        window(app, ctx, &status);
    }
}

/// When the app closes: run the verified installer if the user let it.
pub fn on_quit(app: &mut CantusApp) {
    if app.update.launched || !app.update.install_on_quit {
        return;
    }
    if let Status::Ready { installer, .. } = app.update.status() {
        app.update.launched = true;
        if let Err(e) = install::launch(&installer, false) {
            log::error!("update: the installer did not start: {e}");
        }
    }
}

/// *Restart and update*: run the installer now, asking it to open Cantus again, and close.
fn install_now(app: &mut CantusApp, installer: &std::path::Path) {
    match install::launch(installer, true) {
        Ok(()) => {
            app.update.launched = true;
            app.request_quit = true;
        }
        Err(e) => {
            app.update.launch_error = Some(tf("update.error.launch", &[("error", &e)]));
        }
    }
}

/// Open the release's page in the browser. Only GitHub pages of the project are opened.
fn open_page(url: &str) {
    if !url.starts_with("https://github.com/") {
        return;
    }
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// "5 min ago", "3 h ago", "2 days ago".
pub fn ago(then: u64, now: u64) -> String {
    let secs = now.saturating_sub(then);
    if secs < 3600 {
        tf("update.when.minutes", &[("n", &(secs / 60).max(1))])
    } else if secs < 2 * 86_400 {
        tf("update.when.hours", &[("n", &(secs / 3600))])
    } else {
        tf("update.when.days", &[("n", &(secs / 86_400))])
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

/// The line that says where the updater is.
pub fn status_line(app: &UpdateUi, status: &Status) -> (String, bool) {
    let last = || {
        app.updater
            .as_ref()
            .and_then(Updater::last_checked)
            .map_or_else(
                || t("update.never_checked"),
                |c| tf("update.last_checked", &[("when", &ago(c, schedule::now_unix()))]),
            )
    };
    match status {
        Status::Idle => (last(), false),
        Status::Checking => (t("update.checking"), false),
        Status::UpToDate { .. } => (
            format!("{} {}", tf("update.up_to_date", &[("version", &APP_VERSION)]), last()),
            false,
        ),
        Status::Available(o) => (
            tf(
                "update.available",
                &[("version", &o.version), ("current", &APP_VERSION)],
            ),
            false,
        ),
        Status::Downloading { got, total, .. } => match total {
            Some(total) => (
                tf(
                    "update.downloading",
                    &[("done", &megabytes(*got)), ("total", &megabytes(*total))],
                ),
                false,
            ),
            None => (
                tf("update.downloading.unknown", &[("done", &megabytes(*got))]),
                false,
            ),
        },
        Status::Held { attempt, .. } => (
            tf(
                "update.held",
                &[("attempt", attempt), ("max", &updater::HELD_ATTEMPTS)],
            ),
            false,
        ),
        Status::Ready { .. } => {
            let verified = match verify::PUBLISHER {
                Some(p) => tf("update.verified.signed", &[("publisher", &p)]),
                None => t("update.verified"),
            };
            (format!("{} {verified}", t("update.ready")), false)
        }
        Status::Failed { key, .. } => (t(key), true),
    }
}

/// Break `s` into lines no wider than `width` points in `role`.
fn wrap(ctx: &Context, s: &str, role: Role, width: f32, k: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let cand = if cur.is_empty() {
            word.to_owned()
        } else {
            format!("{cur} {word}")
        };
        let wide = text::measure(ctx, &Text::new(&cand, role, Color32::WHITE), k).x > width;
        if wide && !cur.is_empty() {
            lines.push(std::mem::replace(&mut cur, word.to_owned()));
        } else {
            cur = cand;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// One painted line of text, `indent` points in; returns its rect.
fn line(ui: &mut Ui, s: &str, role: Role, color: Color32, indent: f32, h: f32) -> Rect {
    let st = Style::current();
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), h * st.k), Sense::hover());
    let r = Rect::from_min_max(Pos2::new(rect.left() + indent * st.k, rect.top()), rect.max);
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            r,
            &Text::new(s, role, color)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            st.tk(),
        );
    });
    rect
}

/// The release notes, wrapped to the window's width; bullets get a gold dot.
fn notes(ui: &mut Ui, offer: &cantus_update::Offer) {
    let s = Style::current();
    let ctx = ui.ctx().clone();
    let width = ui.available_width() - 24.0 * s.k;
    if offer.notes.is_empty() {
        note(ui, &t("update.no_notes"), false);
        return;
    }
    for n in &offer.notes {
        match n.kind {
            LineKind::Gap => ui.add_space(4.0 * s.k),
            LineKind::Heading => {
                line(ui, &n.text, Role::Section, s.t.text, 0.0, 22.0);
            }
            LineKind::Bullet => {
                let lines = wrap(&ctx, &n.text, Role::Body, width - 14.0 * s.k, s.tk());
                for (i, l) in lines.iter().enumerate() {
                    let r = line(ui, l, Role::Body, s.t.text, 14.0, 18.0);
                    if i == 0 {
                        let dot = Rect::from_center_size(
                            Pos2::new(r.left() + 5.0 * s.k, r.center().y),
                            Vec2::splat(4.0 * s.k),
                        );
                        with_brush(ui.painter(), |b| {
                            b.rrect(dot, 2.0 * s.k, Fill::Solid(s.t.brand));
                        });
                    }
                }
            }
            LineKind::Text => {
                for l in wrap(&ctx, &n.text, Role::Body, width, s.tk()) {
                    line(ui, &l, Role::Body, s.t.text_dim, 0.0, 18.0);
                }
            }
        }
    }
    if offer.notes.len() >= updater::NOTE_LINES {
        note(ui, &t("update.notes_more"), false);
    }
}

/// What the window's buttons asked for this frame.
#[derive(Default)]
struct Asked {
    check: bool,
    download: bool,
    cancel: bool,
    skip: bool,
    page: Option<String>,
    install: Option<std::path::PathBuf>,
    close: bool,
}

fn buttons(app: &mut CantusApp, ui: &mut Ui, status: &Status) -> Asked {
    let mut a = Asked::default();
    let kind = app.update.kind();
    ui.horizontal(|ui| match status {
        Status::Available(o) | Status::Failed { offer: Some(o), .. } => {
            if o.installer.is_some() {
                a.download = w::button(ui, Src::Key("update.download"), 0.0, true).clicked();
            }
            if w::button(ui, Src::Key("update.open_page"), 0.0, true).clicked() {
                a.page = Some(o.page.clone());
            }
            a.skip = w::button(ui, Src::Key("update.skip"), 0.0, true).clicked();
            a.close = w::button(ui, Src::Key("update.later"), 0.0, true).clicked();
        }
        Status::Checking | Status::Downloading { .. } | Status::Held { .. } => {
            a.cancel = w::button(ui, Src::Key("update.cancel"), 0.0, true).clicked();
        }
        Status::Ready { installer, offer } => {
            if w::button(ui, Src::Key("update.install_now"), 0.0, true).clicked() {
                a.install = Some(installer.clone());
            }
            let mut on = app.update.install_on_quit;
            led_button(
                ui,
                &mut on,
                Src::Key("update.install_on_quit"),
                &t("update.install_on_quit"),
                190.0,
            );
            app.update.install_on_quit = on;
            if w::button(ui, Src::Key("update.open_page"), 0.0, true).clicked() {
                a.page = Some(offer.page.clone());
            }
        }
        _ => {
            a.check = w::button(ui, Src::Key("update.check_now"), 0.0, true).clicked();
            a.close = w::button(ui, Src::Key("workspace.close"), 0.0, true).clicked();
        }
    });
    if status.offer().is_some() && !matches!(status, Status::Ready { .. }) {
        match kind {
            InstallKind::Portable => note(ui, &t("update.portable"), false),
            InstallKind::Development => note(ui, &t("update.dev"), false),
            InstallKind::Installed => {}
        }
    }
    a
}

fn act(app: &mut CantusApp, a: Asked) {
    if a.check {
        check_now(app);
    }
    if let Some(u) = &app.update.updater {
        if a.download {
            u.download();
        }
        if a.cancel {
            u.cancel();
        }
        if a.skip {
            u.skip();
            app.update.open = false;
        }
    }
    if let Some(url) = a.page {
        open_page(&url);
    }
    if let Some(path) = a.install {
        install_now(app, &path);
    }
    if a.close {
        app.update.open = false;
    }
}

/// The update window: where the updater is, what is new, what to do.
fn window(app: &mut CantusApp, ctx: &Context, status: &Status) {
    let s = Style::current();
    let screen = ctx.content_rect();
    let size = Vec2::new(520.0, 520.0).min(screen.size() - Vec2::new(40.0, 100.0));
    let mut asked = Asked::default();
    egui::Window::new("cantus-update")
        .id(Id::new("cantus-update"))
        .title_bar(false)
        .frame(egui::Frame::NONE)
        .resizable(true)
        .default_size(size)
        .min_size(Vec2::new(380.0, 260.0).min(size))
        .default_pos(Pos2::new(
            screen.center().x - size.x * 0.5,
            screen.top() + 80.0,
        ))
        .constrain(true)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            window_face(ui, rect, &s);
            let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 30.0 * s.k));
            let closed = title(ui, header, &s, Glyph::Down, "update.title");
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 12.0 * s.k, header.bottom() + 8.0 * s.k),
                Pos2::new(rect.right() - 12.0 * s.k, rect.bottom() - 10.0 * s.k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            let ui = &mut child;
            ui.spacing_mut().item_spacing = Vec2::new(8.0 * s.k, 6.0 * s.k);
            w::band(ui, Src::Key("update.band.status"));
            let (text_now, warn) = status_line(&app.update, status);
            let width = ui.available_width();
            for l in wrap(ui.ctx(), &text_now, Role::Label, width, s.tk()) {
                note(ui, &l, warn);
            }
            if let Some(e) = app.update.launch_error.clone() {
                note(ui, &e, true);
            }
            if let Some(offer) = status.offer().cloned() {
                w::band(ui, Src::Str(&offer.title));
                let notes_h = (ui.available_height() - 80.0 * s.k).max(60.0 * s.k);
                egui::ScrollArea::vertical()
                    .id_salt("cantus-update-notes")
                    .max_height(notes_h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| notes(ui, &offer));
            }
            ui.add_space(6.0 * s.k);
            asked = buttons(app, ui, status);
            asked.close |= closed || ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
    act(app, asked);
}

/// The notice at the bottom right: a newer Cantus, or one ready to install. Clicking it
/// opens the window; ✕ hides it for this session.
fn notice(app: &mut CantusApp, ctx: &Context, status: &Status) {
    let (key, version) = match status {
        Status::Available(o) => ("update.notice", o.version.clone()),
        Status::Ready { offer, .. } => ("update.notice.ready", offer.version.clone()),
        _ => return,
    };
    let s = Style::current();
    let label = tf(key, &[("version", &version)]);
    let k = s.k;
    let text_w = text::measure(ctx, &Text::new(&label, Role::Body, s.t.text), s.tk()).x;
    let size = Vec2::new(text_w + 74.0 * k, 34.0 * k);
    let status_h = super::workspace_status::HEIGHT * s.t.density;
    let mut open = false;
    let mut close = false;
    egui::Area::new(Id::new("cantus-update-notice"))
        .anchor(
            Align2::RIGHT_BOTTOM,
            Vec2::new(-12.0 * k, -(status_h + 12.0 * k)),
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
            window_face(ui, rect, &s);
            let hover = resp.hovered();
            with_brush(ui.painter(), |b| {
                if hover {
                    b.rrect(
                        rect.shrink(1.0),
                        s.r(6.0),
                        Fill::Solid(crate::kit::color::with_alpha(s.t.selection, 0.12)),
                    );
                }
                let dot = Rect::from_center_size(
                    Pos2::new(rect.left() + 16.0 * k, rect.center().y),
                    Vec2::splat(8.0 * k),
                );
                b.rrect(dot, 4.0 * k, Fill::Solid(s.t.brand));
                let tr = Rect::from_min_max(
                    Pos2::new(dot.right() + 10.0 * k, rect.top()),
                    Pos2::new(rect.right() - 36.0 * k, rect.bottom()),
                );
                let _ = text::paint(
                    b,
                    tr,
                    &Text::new(&label, Role::Body, s.t.text)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
            });
            let keys = Rect::from_min_max(
                Pos2::new(rect.right() - 32.0 * k, rect.top() + 6.0 * k),
                Pos2::new(rect.right() - 6.0 * k, rect.bottom() - 6.0 * k),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(keys)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            close = w::glyph_key(&mut child, Glyph::Close, false, 20.0, &t("workspace.close"))
                .clicked();
            open = resp.clicked() && !close;
        });
    if open {
        app.update.open = true;
    }
    if close {
        app.update.notice_closed = true;
    }
}

/// Settings → Updates: check daily, a proxy of one's own, check now.
pub fn settings_section(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("update.settings.band"));
    ui.horizontal(|ui| {
        w::row_label(ui, "update.settings.check_daily", LABEL_W, 24.0);
        let mut on = app.settings.updates.check_daily;
        if led_button(
            ui,
            &mut on,
            Src::Key("update.settings.check_daily"),
            &t("update.settings.check_daily"),
            120.0,
        )
        .changed()
        {
            app.settings.updates.check_daily = on;
            app.store_settings();
        }
    });
    note(ui, &t("update.settings.check_daily.hint"), false);
    ui.horizontal(|ui| {
        w::row_label(ui, "update.settings.proxy", LABEL_W, 24.0);
        let mut text = app
            .update
            .proxy
            .clone()
            .unwrap_or_else(|| app.settings.updates.proxy.clone().unwrap_or_default());
        let width = (ui.available_width() / Style::current().k - 8.0).max(160.0);
        let resp = w::text_field(ui, &mut text, "update.settings.proxy.hint", width);
        if resp.changed() {
            app.update.proxy = Some(text.clone());
        }
        if resp.lost_focus() {
            let value = text.trim().to_owned();
            app.settings.updates.proxy = (!value.is_empty()).then_some(value);
            app.update.proxy = None;
            app.store_settings();
            // The next check makes its client with the new proxy.
            if !app.update.status().busy() {
                app.update.updater = None;
            }
        }
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 24.0);
        let busy = app.update.status().busy();
        if w::button(ui, Src::Key("update.check_now"), 0.0, !busy).clicked() {
            check_now(app);
        }
        let (line_text, warn) = status_line(&app.update, &app.update.status());
        note(ui, &line_text, warn);
    });
}

/// The screenshots' states (`examples/shoot.rs`): `update` (a release offered),
/// `update-ready` (downloaded and verified), `update-notice` (the notice alone),
/// `update-held` (a proxy holding the download).
pub fn demo_setup(app: &mut CantusApp, kind: &str) {
    let md = "## What is new

* **Automation clips**: draw a curve on any knob, and see it under the clip.
* The device picker shows [every instrument and effect](https://example.com) at once, with drawn thumbnails.
* Swap an instrument for another and keep its notes, its mixer strip and its effects.

## Fixed

- The mixer no longer clicks when a send is muted while playing.
- Recording with ASIO at 32 samples keeps every take aligned.
";
    let offer = cantus_update::Offer {
        version: "0.2.0".into(),
        title: "Cantus 0.2.0".into(),
        notes: cantus_update::notes::read(md, updater::NOTE_LINES),
        page: "https://github.com/luis314159/cantus/releases/tag/v0.2.0".into(),
        installer: Some(cantus_update::release::Asset {
            name: cantus_update::release::installer_name("0.2.0", Flavor::Standard),
            size: 14_200_000,
            browser_download_url: String::new(),
            digest: None,
        }),
        sums_url: None,
    };
    app.update.kind = Some(InstallKind::Installed);
    app.update.started = true;
    app.update.demo = Some(match kind {
        "update-ready" => Status::Ready {
            offer,
            installer: std::path::PathBuf::from("Cantus-0.2.0-windows-x64-setup.exe"),
        },
        "update-held" => Status::Held { offer, attempt: 2 },
        _ => Status::Available(offer),
    });
    app.update.open = kind != "update-notice";
}
