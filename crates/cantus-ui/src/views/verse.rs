//! The verse on screen (BACKLOG §6c): a band at the foot of the splash, then a card at the
//! bottom-right of the window while the work begins — the same passage, carried over —
//! and the switch in Settings ▸ General.
//!
//! - **Type.** The passage in the condensed italic (the kit's quotation face), as large as
//!   it can be set in four lines, never below its floor in five; the reference under it in
//!   the brand's gold, in the section caps, with the translation's short name after it.
//! - **Never in the way.** The card is not modal: it takes only the clicks on itself, goes
//!   on ✕ or after [`CARD_SECONDS`], and *Don't show again* turns the switch off (and says
//!   where to turn it back on).
//! - **Follows the language.** The card keeps a reference, not a text: switch the
//!   interface's language and the same passage shows in the other translation.

use super::workspace_settings::LABEL_W;
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::with_alpha;
use crate::kit::glyph::Glyph;
use crate::kit::prose::{self, Lines};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::segmented;
use crate::kit::{with_brush, Brush, Fill, Style, Tokens};
use crate::verse::{Reference, Shown, Spanish, Translation};
use egui::{Align2, Context, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::sync::Arc;
use std::time::Duration;

/// The passage's face.
pub const ROLE: Role = Role::Placeholder;
/// The most lines a passage may take; the tests hold every passage to it.
pub const MAX_LINES: usize = 5;
/// The sizes tried, largest first, over the role's own: four lines at the largest that
/// takes four, else the floor.
const SCALES: [f32; 4] = [1.16, 1.08, 1.0, 0.92];

/// A passage set in a width.
#[derive(Clone, Debug)]
pub struct Fit {
    pub lines: Arc<Lines>,
    /// The size chosen, over the role's own.
    pub scale: f32,
}

impl Fit {
    pub fn fits(&self) -> bool {
        self.lines.lines.len() <= MAX_LINES
    }
}

/// Set `text` in `width` points at the scale `k`.
pub fn fit(ctx: &Context, text: &str, width: f32, k: f32) -> Fit {
    let mut last = None;
    for s in SCALES {
        let lines = prose::wrap(ctx, text, ROLE, k * s, width);
        if lines.lines.len() <= MAX_LINES - 1 {
            return Fit { lines, scale: s };
        }
        last = Some(Fit { lines, scale: s });
    }
    last.unwrap_or(Fit {
        lines: Arc::new(Lines {
            lines: Vec::new(),
            line_h: 0.0,
            widest: 0.0,
        }),
        scale: 1.0,
    })
}

/// Where the parts of a painted verse are: what the tests check.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Painted {
    /// The box the verse was given.
    pub area: Rect,
    /// The passage's lines, as painted.
    pub text: Rect,
    /// The reference line, as painted.
    pub reference: Rect,
    /// *Don't show again*.
    pub hide: Rect,
    pub lines: usize,
    /// Whether any line or the reference had to be shortened.
    pub shortened: bool,
}

/// The reference line: "COLOSSIANS 3:23 · WEB".
fn citation(
    b: &mut Brush,
    rect: Rect,
    s: &str,
    color: egui::Color32,
    k: f32,
    align: Align2,
) -> (Rect, bool) {
    text::paint(
        b,
        rect,
        &Text::new(s, Role::Section, color)
            .align(align)
            .policy(Policy::Elide(Elide::End)),
        k,
    )
    .map_or((Rect::NOTHING, true), |p| (p.rect, p.shortened))
}

/// *Don't show again*, as a quiet link: dim, the text colour and an underline on hover.
fn hide_link(b: &mut Brush, rect: Rect, t: &Tokens, k: f32, hovered: bool, align: Align2) -> Rect {
    let color = if hovered { t.text } else { t.text_dim };
    let p = text::paint(
        b,
        rect,
        &Text::key("verse.hide", Role::Label, color)
            .caps(false)
            .align(align)
            .policy(Policy::Elide(Elide::End)),
        k,
    );
    let r = p.map_or(Rect::NOTHING, |p| p.rect);
    if hovered && r.is_positive() {
        b.hline(
            r.left(),
            r.right(),
            r.bottom() + k,
            b.px(),
            with_alpha(color, 0.7),
        );
    }
    r
}

/// The ornament over the splash's verse: two hairlines and a small gold diamond.
fn ornament(b: &mut Brush, center: Pos2, half: f32, t: &Tokens, k: f32) {
    let y = b.snap_mid(center.y);
    let gap = 9.0 * k;
    let line = with_alpha(t.text, 0.10);
    b.hline(center.x - half, center.x - gap, y, b.px(), line);
    b.hline(center.x + gap, center.x + half, y, b.px(), line);
    let r = 3.2 * k;
    b.polygon(
        &[
            Pos2::new(center.x, y - r),
            Pos2::new(center.x + r, y),
            Pos2::new(center.x, y + r),
            Pos2::new(center.x - r, y),
        ],
        t.brand,
    );
}

// ---- the splash ------------------------------------------------------------------------

/// The splash's verse band, in points at 100 % (under the 520 × 320 composition).
pub const SPLASH_BAND: f32 = 136.0;
/// The band's side margins.
const SPLASH_MARGIN: f32 = 40.0;

/// Paint the splash's verse band in `band` at the splash's scale `k`; `hover` is where
/// the pointer is. Returns where everything went.
pub fn paint_splash(
    b: &mut Brush,
    band: Rect,
    shown: Shown,
    t: &Tokens,
    k: f32,
    hover: Option<Pos2>,
) -> Painted {
    let ctx = b.ctx().clone();
    let inner = Rect::from_min_max(
        Pos2::new(band.left() + SPLASH_MARGIN * k, band.top()),
        Pos2::new(band.right() - SPLASH_MARGIN * k, band.bottom()),
    );
    ornament(
        b,
        Pos2::new(inner.center().x, inner.top() + 6.0 * k),
        inner.width() * 0.5,
        t,
        k,
    );
    let f = fit(&ctx, shown.text(), inner.width(), k);
    let cite_h = 16.0 * k;
    let gap = 8.0 * k;
    let link_h = 16.0 * k;
    // The passage and its reference, centred in what is left above the link.
    let room = Rect::from_min_max(
        Pos2::new(inner.left(), inner.top() + 16.0 * k),
        Pos2::new(inner.right(), inner.bottom() - link_h - 8.0 * k),
    );
    let block = f.lines.height() + gap + cite_h;
    let top = room.top() + ((room.height() - block) * 0.5).max(0.0);
    let text_rect = Rect::from_min_size(
        Pos2::new(inner.left(), top),
        Vec2::new(inner.width(), f.lines.height()),
    );
    let (painted_text, short_text) = prose::paint(
        b,
        text_rect,
        &f.lines,
        ROLE,
        k * f.scale,
        t.text,
        Align2::CENTER_TOP,
    );
    let cite_rect = Rect::from_min_size(
        Pos2::new(inner.left(), text_rect.bottom() + gap),
        Vec2::new(inner.width(), cite_h),
    );
    let (painted_cite, short_cite) = citation(
        b,
        cite_rect,
        &shown.citation(),
        t.brand,
        k,
        Align2::CENTER_CENTER,
    );
    let link_rect = Rect::from_min_max(
        Pos2::new(inner.left(), inner.bottom() - link_h - 6.0 * k),
        Pos2::new(inner.right(), inner.bottom() - 6.0 * k),
    );
    let hovered = hover.is_some_and(|p| splash_hide_rect(band, k).contains(p));
    let hide = hide_link(b, link_rect, t, k, hovered, Align2::RIGHT_CENTER);
    Painted {
        area: band,
        text: painted_text,
        reference: painted_cite,
        hide,
        lines: f.lines.lines.len(),
        shortened: short_text || short_cite || !f.fits(),
    }
}

/// Where *Don't show again* answers the pointer on the splash (a little wider than the
/// words, so it is easy to hit).
pub fn splash_hide_rect(band: Rect, k: f32) -> Rect {
    let right = band.right() - SPLASH_MARGIN * k;
    let bottom = band.bottom() - 6.0 * k;
    Rect::from_min_max(
        Pos2::new(right - 150.0 * k, bottom - 18.0 * k),
        Pos2::new(right + 4.0 * k, bottom + 2.0 * k),
    )
}

// ---- the start card ----------------------------------------------------------------------

/// The card's width, in points.
pub const CARD_W: f32 = 420.0;
/// How long the card stays, in seconds, unless closed first.
pub const CARD_SECONDS: f64 = 45.0;
const PAD: f32 = 16.0;
/// The room the close key takes at the right of the passage.
const KEY_ROOM: f32 = 26.0;

/// The verse the window shows as the work begins: the splash's passage, carried over.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Card {
    pub reference: Reference,
    /// When it first showed (egui's clock), set on the first frame.
    pub since: Option<f64>,
}

impl Card {
    pub fn new(reference: Reference) -> Card {
        Card {
            reference,
            since: None,
        }
    }
}

/// The card's id (its area's), for tests that look for it.
pub fn card_id() -> Id {
    Id::new("cantus-verse-card")
}

/// The card's rect and its passage set, in a window whose content is `screen`: at the
/// bottom-right, clear of the status bar.
pub fn card_layout(ctx: &Context, screen: Rect, shown: Shown) -> (Rect, Fit) {
    let s = Style::current();
    let status_h = (super::workspace_status::HEIGHT * s.t.density).round();
    let margin = 16.0;
    let width = CARD_W.min(screen.width() - 2.0 * margin).max(160.0);
    let f = fit(ctx, shown.text(), width - 2.0 * PAD - KEY_ROOM, s.tk());
    let height = PAD + f.lines.height() + 6.0 + 16.0 + 10.0 + 18.0 + PAD * 0.6;
    let bottom = screen.bottom() - status_h - margin;
    let rect = Rect::from_min_max(
        Pos2::new(screen.right() - margin - width, bottom - height),
        Pos2::new(screen.right() - margin, bottom),
    );
    (rect, f)
}

/// Show the card, if there is one and the switch is on. Called once per frame; returns
/// where it painted.
pub fn frame(app: &mut CantusApp, ctx: &Context) -> Option<Painted> {
    if !app.settings.verse.show {
        app.verse = None;
    }
    let card = app.verse.as_mut()?;
    let now = ctx.input(|i| i.time);
    let since = *card.since.get_or_insert(now);
    let left = CARD_SECONDS - (now - since);
    if left <= 0.0 {
        app.verse = None;
        return None;
    }
    super::workspace_ticker::wake_after(ctx, Duration::from_secs_f64(left));
    let shown = Shown {
        reference: card.reference,
        spanish: app.settings.verse.spanish,
    };
    let s = Style::current();
    let (rect, f) = card_layout(ctx, ctx.content_rect(), shown);
    let mut close = false;
    let mut hide = false;
    let mut painted = None;
    egui::Area::new(card_id())
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
            super::workspace_settings::frame(ui, rect, &s);
            let resp_rect = hide_rect_in_card(rect, &f);
            let resp = ui.interact(resp_rect, card_id().with("hide"), Sense::click());
            if resp.hovered() {
                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            painted = Some(paint_card(ui, rect, shown, &f, &s, resp.hovered()));
            hide = resp.clicked();
            let key = Rect::from_min_size(
                Pos2::new(rect.right() - 30.0, rect.top() + 8.0),
                Vec2::splat(22.0),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(key));
            close =
                w::glyph_key(&mut child, Glyph::Close, false, 22.0, &t("verse.close")).clicked();
        });
    if close {
        app.verse = None;
    }
    if hide {
        turn_off(app);
    }
    painted
}

/// *Don't show again*: the switch off, remembered, and a word on where to turn it back on.
pub fn turn_off(app: &mut CantusApp) {
    app.verse = None;
    app.settings.verse.show = false;
    app.store_settings();
    app.set_status(t("verse.hidden"));
}

/// The card's rows, top to bottom: the passage, the reference, the link.
fn card_rows(rect: Rect, f: &Fit) -> (Rect, Rect, Rect) {
    let left = rect.left() + PAD + 4.0;
    let text = Rect::from_min_size(
        Pos2::new(left, rect.top() + PAD),
        Vec2::new(rect.width() - 2.0 * PAD - KEY_ROOM, f.lines.height()),
    );
    let cite = Rect::from_min_size(
        Pos2::new(left, text.bottom() + 6.0),
        Vec2::new(rect.right() - PAD - left, 16.0),
    );
    let link = Rect::from_min_size(
        Pos2::new(left, cite.bottom() + 10.0),
        Vec2::new(rect.right() - PAD - left, 18.0),
    );
    (text, cite, link)
}

/// Where *Don't show again* answers the pointer on the card.
fn hide_rect_in_card(rect: Rect, f: &Fit) -> Rect {
    let (_, _, link) = card_rows(rect, f);
    Rect::from_min_size(
        link.min - Vec2::new(4.0, 2.0),
        Vec2::new(170.0, link.height() + 4.0),
    )
}

/// Paint the card's face in `rect` (the panel is already drawn): the gold edge, the
/// passage, the reference, *Don't show again*.
fn paint_card(ui: &Ui, rect: Rect, shown: Shown, f: &Fit, s: &Style, hovered: bool) -> Painted {
    let k = s.tk();
    with_brush(ui.painter(), |b| {
        let edge = b.snap_rect(Rect::from_min_size(
            Pos2::new(rect.left() + 1.0, rect.top() + 10.0),
            Vec2::new(3.0, rect.height() - 20.0),
        ));
        b.rrect(edge, 1.5, Fill::Solid(s.t.brand));
        let (text_rect, cite_rect, link_rect) = card_rows(rect, f);
        let (painted, short) = prose::paint(
            b,
            text_rect,
            &f.lines,
            ROLE,
            k * f.scale,
            s.t.text,
            Align2::LEFT_TOP,
        );
        let (cite, short_cite) = citation(
            b,
            cite_rect,
            &shown.citation(),
            s.t.brand,
            k,
            Align2::LEFT_CENTER,
        );
        let hide = hide_link(b, link_rect, &s.t, k, hovered, Align2::LEFT_CENTER);
        Painted {
            area: rect,
            text: painted,
            reference: cite,
            hide,
            lines: f.lines.lines.len(),
            shortened: short || short_cite || !f.fits(),
        }
    })
}

// ---- settings and About --------------------------------------------------------------------

/// Settings ▸ General: the verse switch and the Spanish translation.
pub fn settings(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    w::band(ui, Src::Key("verse.settings.general"));
    ui.horizontal(|ui| {
        w::row_label(ui, "verse.settings.show", LABEL_W, 24.0);
        let mut sel = usize::from(!app.settings.verse.show);
        let labels = [
            Src::Key("verse.settings.on"),
            Src::Key("verse.settings.off"),
        ];
        if segmented(ui, &mut sel, &labels, &t("verse.settings.show"), 160.0).changed() {
            app.settings.verse.show = sel == 0;
            if !app.settings.verse.show {
                app.verse = None;
            }
            app.store_settings();
        }
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "verse.settings.spanish", LABEL_W, 24.0);
        let mut sel = Spanish::ALL
            .iter()
            .position(|x| *x == app.settings.verse.spanish)
            .unwrap_or(0);
        let labels: Vec<Src<'_>> = Spanish::ALL
            .iter()
            .map(|x| Src::Key(x.translation().name_key()))
            .collect();
        let width = (ui.available_width() / k - 8.0).clamp(200.0, 320.0);
        if segmented(ui, &mut sel, &labels, &t("verse.settings.spanish"), width).changed() {
            app.settings.verse.spanish = Spanish::ALL[sel];
            app.store_settings();
        }
    });
}

/// The About box's credit lines: each bundled translation's name and licence.
pub fn credits() -> Vec<(String, String)> {
    Translation::ALL
        .iter()
        .map(|tr| (t(tr.name_key()), t(tr.licence_key())))
        .collect()
}
