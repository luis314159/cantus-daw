//! *Help ▸ About Cantus* (round 22), drawn with the kit in place of egui's stock window:
//! the logo, the name and version, who made it, the motto in the reader's translation,
//! the licence with the notices, privacy statement and licence text one click away, the
//! Bible translations' credits, the trademark notices, and the manual, release notes, bug
//! form and source.

use super::audio_parts as parts;
use super::workspace_widgets as w;
use crate::app::{CantusApp, APP_VERSION};
use crate::docs::{self, Doc};
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Style};
use egui::{Align2, Color32, Id, Pos2, Rect, Sense, Ui, Vec2};

const SIZE: Vec2 = Vec2::new(580.0, 700.0);
/// The logo's side in the hero, in points at 100 %.
const LOGO: f32 = 96.0;

/// The motto, Colossians 3:23, in the translation the verse at start uses.
pub fn motto(spanish: crate::verse::Spanish) -> (String, String) {
    let shown = crate::verse::Shown {
        reference: crate::verse::Reference::parse("COL 3:23").expect("a reference"),
        spanish,
    };
    let text = shown.text().trim_end_matches([',', ';', ':', ' ']);
    (format!("“{text}…”"), shown.citation())
}

/// What is clicked in the window.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Link {
    Doc(Doc),
}

/// The window, when open. Returns the link clicked this frame and opens nothing, so a
/// test can click every link.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) -> Option<Link> {
    if !app.show_about {
        return None;
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let size = (SIZE * k).min(screen.size() - Vec2::new(40.0, 60.0));
    let rect = Rect::from_center_size(screen.center(), size);
    let mut open = true;
    let mut clicked = None;
    egui::Area::new(Id::new("cantus-about"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 32.0 * k));
            if super::workspace_settings::title(ui, head, &s, Glyph::Help, "about.title") {
                open = false;
            }
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 22.0 * k, head.bottom() + 14.0 * k),
                Pos2::new(rect.right() - 12.0 * k, rect.bottom() - 14.0 * k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .id_salt("cantus-about-body")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(body.width() - 14.0 * k);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0 * k, 7.0 * k);
                    clicked = page(app, ui);
                });
        });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.show_about = false;
    }
    clicked
}

/// The window in the application: what is clicked opens in the browser.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    if let Some(Link::Doc(d)) = show(app, ctx) {
        docs::open(d);
    }
}

/// A paragraph wrapped to the width, as many lines as it needs.
fn para(ui: &mut Ui, s: &str, role: Role, color: Color32) {
    let st = Style::current();
    let w = ui.available_width();
    let g = parts::wrapped(ui.ctx(), s, role, st.tk(), w, 16);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y), Sense::hover());
    with_brush(ui.painter(), |b| {
        b.text(Pos2::new(b.snap(rect.left()), b.snap(rect.top())), g, color, 0.0);
    });
}

/// One line of text in a box of the given role.
fn line(b: &mut crate::kit::Brush, rect: Rect, s: &str, role: Role, color: Color32, k: f32) {
    let _ = text::paint(
        b,
        rect,
        &Text::new(s, role, color)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End)),
        k,
    );
}

fn page(app: &mut CantusApp, ui: &mut Ui) -> Option<Link> {
    let s = Style::current();
    let k = s.k;
    let mut clicked = None;
    // The hero: the logo, the name in the brand's gold, the version, what it is.
    let (hero, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), LOGO * k + 8.0 * k), Sense::hover());
    let logo = Rect::from_min_size(hero.min, Vec2::splat(LOGO * k));
    w::paint_logo(ui.painter(), logo, app.appearance.logo, &s.t);
    let x = logo.right() + 18.0 * k;
    let row = |y: f32, h: f32| {
        Rect::from_min_max(Pos2::new(x, hero.top() + y * k), Pos2::new(hero.right(), hero.top() + (y + h) * k))
    };
    with_brush(ui.painter(), |b| {
        line(b, row(4.0, 34.0), "Cantus", Role::Title, s.t.brand, s.tk() * 2.2);
        line(
            b,
            row(40.0, 18.0),
            &tf("about.version", &[("version", APP_VERSION)]),
            Role::Value,
            s.t.text_dim,
            s.tk(),
        );
        line(b, row(62.0, 18.0), &t("about.tagline"), Role::Body, s.t.text, s.tk());
        line(b, row(82.0, 18.0), &t("about.made_with"), Role::Label, s.t.text_dim, s.tk());
    });
    // Who made it.
    ui.add_space(4.0 * k);
    ui.horizontal(|ui| {
        para_inline(ui, &t("about.created"), Role::Body, s.t.text);
        if parts::button(ui, "lalmazan.com", false, true, 110.0).clicked() {
            clicked = Some(Link::Doc(Doc::Site));
        }
    });
    let (verse, citation) = motto(app.settings.verse.spanish);
    para(ui, &verse, Role::Placeholder, s.t.text);
    para(ui, &citation, Role::Label, s.t.brand);

    // The licence, and the documents behind it.
    w::band(ui, Src::Key("about.band.licence"));
    para(ui, &t("about.licence"), Role::Body, s.t.text);
    ui.horizontal_wrapped(|ui| {
        for (key, doc) in [
            ("about.open.licence", Doc::Licence),
            ("about.open.notices", Doc::Notices),
            ("about.open.privacy", Doc::Privacy),
        ] {
            if parts::button(ui, &t(key), false, true, 120.0).clicked() {
                clicked = Some(Link::Doc(doc));
            }
        }
    });
    para(ui, &t("about.privacy"), Role::Label, s.t.text_dim);

    // The verse at start: each translation and its licence (the RVR1960's word for word).
    w::band(ui, Src::Key("verse.about.title"));
    for (name, licence) in super::verse::credits() {
        para(ui, &format!("{name} — {licence}"), Role::Label, s.t.text_dim);
    }

    w::band(ui, Src::Key("about.band.trademarks"));
    para(ui, &t("about.trademarks"), Role::Label, s.t.text_dim);

    // Help and feedback.
    w::band(ui, Src::Key("about.band.help"));
    ui.horizontal_wrapped(|ui| {
        for (key, doc, primary) in [
            ("about.open.manual", Doc::Manual, true),
            ("about.open.notes", Doc::ReleaseNotes, false),
            ("about.open.bug", Doc::ReportBug, false),
            ("about.open.source", Doc::Source, false),
        ] {
            if parts::button(ui, &t(key), primary, true, 120.0).clicked() {
                clicked = Some(Link::Doc(doc));
            }
        }
    });
    para(ui, &t("about.beta"), Role::Label, s.t.text_dim);
    clicked
}

/// A one-line label that takes only its own width (beside a button).
fn para_inline(ui: &mut Ui, s: &str, role: Role, color: Color32) {
    let st = Style::current();
    let tx = Text::new(s, role, color).align(Align2::LEFT_CENTER);
    let w = text::measure(ui.ctx(), &tx, st.tk()).x + 4.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, st.d(24.0)), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(b, rect, &tx, st.tk());
    });
}
