//! The ARA section of an audio clip's menu (round 36, ADR 0097) and what a bound clip shows
//! on its body. Unbound: *Bind to* each ARA plug-in found (and *with its previous edits* when
//! it had some). Bound: what the plug-in does with the clip, *Open* its editor, *Retry* a
//! plug-in that stopped, *Let go of* it. Always: *Play sources only*, and that ARA plug-ins
//! run inside Cantus, not isolated.

use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::{t, tf};
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, Align2, Rect};

/// The menu's width (the arrangement's clip menu).
const MENU_W: f32 = 240.0;

#[derive(Clone, Copy)]
enum Pick {
    Bind(usize),
    Previous,
    Open,
    Retry,
    Unbind,
    Toggle,
}

/// One row: its words, whether it can be clicked, its check, what it does.
type Row = (String, bool, Check, Option<Pick>);

/// The section's rows for clip `r`.
fn rows(app: &CantusApp, r: ClipRef) -> Vec<Row> {
    let on = app.plugins.ara.enabled();
    let mut out: Vec<Row> = Vec::new();
    if let Some(name) = app.ara_binding(r).map(|p| p.name.clone()) {
        let live = matches!(
            app.plugins.ara.status(r.1),
            Some(cantus_host::ara::AraStatus::Live)
        );
        if let Some(st) = app.ara_status_text(r) {
            out.push((st, false, Check::None, None));
        }
        if !live {
            out.push((t("ara.menu.source"), false, Check::None, None));
        }
        if app.ara_retryable(r) {
            out.push((
                tf("ara.menu.retry", &[("name", &name)]),
                true,
                Check::None,
                Some(Pick::Retry),
            ));
        } else {
            out.push((
                tf("ara.menu.open", &[("name", &name)]),
                live,
                Check::None,
                Some(Pick::Open),
            ));
        }
        out.push((
            tf("ara.menu.unbind", &[("name", &name)]),
            true,
            Check::None,
            Some(Pick::Unbind),
        ));
    } else if !app.ara_can_bind(r) {
        out.push((t("ara.menu.blocked"), false, Check::None, None));
    } else {
        let plugins = app.ara_plugins();
        let previous = app
            .plugins
            .ara
            .previous_binding(r.1)
            .map(|p| p.plugin.name.clone());
        if plugins.is_empty() && previous.is_none() {
            out.push((t("ara.menu.none"), false, Check::None, None));
        }
        for (i, p) in plugins.iter().enumerate() {
            out.push((
                tf("ara.menu.bind", &[("name", &p.name)]),
                on,
                Check::None,
                Some(Pick::Bind(i)),
            ));
        }
        if let Some(n) = previous {
            out.push((
                tf("ara.menu.restore", &[("name", &n)]),
                on,
                Check::None,
                Some(Pick::Previous),
            ));
        }
    }
    out.push((
        t("ara.menu.off"),
        true,
        if on { Check::Off } else { Check::On },
        Some(Pick::Toggle),
    ));
    out.push((t("ara.menu.in_process"), false, Check::None, None));
    out
}

pub fn clip_menu(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef) {
    if crate::audio_clips::region(app.project(), r).is_none() {
        return;
    }
    let rows = rows(app, r);
    let mut items = vec![Item::Band(Src::Key("ara.menu.band"))];
    items.extend(rows.iter().map(|(label, enabled, check, _)| Item::Action {
        label: Src::Str(label),
        check: *check,
        shortcut: None,
        submenu: false,
        enabled: *enabled,
    }));
    let Some(i) = crate::kit::widgets::menu_list(ui, &items, MENU_W).1.clicked else {
        return;
    };
    let Some(pick) = i.checked_sub(1).and_then(|i| rows.get(i)).and_then(|r| r.3) else {
        return;
    };
    match pick {
        Pick::Bind(k) => {
            if let Some(p) = app.ara_plugins().get(k).cloned() {
                app.bind_ara(r, &p);
            }
        }
        Pick::Previous => {
            app.bind_ara_previous(r);
        }
        Pick::Open => {
            app.open_ara_editor(r);
        }
        Pick::Retry => app.retry_ara(r),
        Pick::Unbind => {
            app.unbind_ara(r);
        }
        Pick::Toggle => {
            let on = app.plugins.ara.enabled();
            app.set_ara_enabled(!on);
        }
    }
    ui.close();
}

/// On a bound clip's body: the plug-in's name and how the clip stands, and while its render
/// is on its way, how far (a bar along the foot and the words). Drawn over the waveform.
pub fn paint_clip(b: &mut Brush, app: &CantusApp, r: ClipRef, inner: Rect, s: &Style) {
    let Some(name) = app.ara_binding(r).map(|p| p.name.clone()) else {
        return;
    };
    let k = s.k;
    if inner.height() < 16.0 * k || inner.width() < 40.0 * k {
        return;
    }
    let line = Rect::from_min_max(
        pos2(inner.left() + 4.0 * k, inner.bottom() - 15.0 * k),
        pos2(inner.right() - 4.0 * k, inner.bottom() - 3.0 * k),
    );
    let words = match app.plugins.ara.progress(r.1) {
        Some(p) => {
            let bar = Rect::from_min_max(
                pos2(inner.left(), inner.bottom() - 2.0 * k),
                pos2(
                    inner.left() + inner.width() * p.clamp(0.0, 1.0),
                    inner.bottom(),
                ),
            );
            b.rect(bar, Fill::Solid(s.t.highlight));
            let pct = (p * 100.0).round() as i32;
            tf("ara.clip.rendering", &[("name", &name), ("pct", &pct)])
        }
        None => match app.plugins.ara.status(r.1) {
            Some(cantus_host::ara::AraStatus::Live) => tf("ara.clip.live", &[("name", &name)]),
            _ => tf("ara.clip.source", &[("name", &name)]),
        },
    };
    let text = Text::new(&words, Role::Micro, s.t.text_dim)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, line, &text, s.tk());
}
