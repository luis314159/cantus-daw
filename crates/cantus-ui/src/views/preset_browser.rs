//! The preset browser's window (round 37, ADR 0101): FL Studio's browser rows and preview
//! player, in our kit — a search, **style and role chips** that filter in one click (with
//! their counts), rows with a ▶ key, the name, what it is for and the **mini waveform** of
//! what was heard, and at the foot a **player** with the large waveform, what it was heard
//! on, *Sync BPM* and *Load*. Nothing in the project changes until a preset is loaded;
//! loading is one step of undo.
//!
//! Keys while it is open (it reads them, as the device picker does): typing searches,
//! ↑ ↓ choose and hear, Space hears the chosen one again (or stops), Enter loads it, Esc
//! closes.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role as TextRole, Text};
use crate::kit::widgets::LedButton;
use crate::kit::{with_brush, Brush, Fill, State, Style};
use crate::preset_catalog::{Entry, Source, Unit};
use crate::preset_query::counts;
use crate::preset_tags::{Role, Style as Genre};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui, Vec2};

/// The window's size at 100 %.
const SIZE: Vec2 = Vec2::new(600.0, 660.0);
const HEAD: f32 = 32.0;
const CHIP: f32 = 22.0;
const PLAYER: f32 = 96.0;

/// What the window asks for this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) enum Ask {
    #[default]
    None,
    Hear(String),
    Load(String),
    Close,
}

/// Draw the browser when open, and do what it asks.
pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    if !app.presets.open {
        return;
    }
    // r39:ux-b — it closes on its own Esc (`preset_browser_keys`); said open, so nothing
    // under it takes the key (ADR 0110).
    crate::esc::open(ctx, crate::esc::Layer::Presets);
    // A target that went away (its track or insert deleted) closes it.
    if crate::preset_browser::unit_of(app, app.presets.target) != app.presets.unit {
        app.close_preset_browser();
        return;
    }
    let busy = app.poll_preset_audition();
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let size = (SIZE * k).min(screen.size() - vec2(24.0, 48.0));
    let at = app.presets.at.unwrap_or_else(|| {
        pos2(
            screen.right() - size.x - 24.0 * k,
            screen.top() + (screen.height() - size.y) * 0.4,
        )
    });
    let rect = Rect::from_min_size(at, size);
    let mut ask = Ask::None;
    egui::Area::new(Id::new("cantus-preset-browser"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            super::audio_parts::panel(ui, rect, &s);
            ask = body(app, ui, rect, &s);
        });
    if crate::kit::field::active(ctx) {
        super::preset_browser_keys::while_typing(app, ctx);
    }
    if let Some(id) = super::preset_browser_keys::settled(app, std::time::Instant::now()) {
        if ask == Ask::None {
            ask = Ask::Hear(id);
        }
    }
    if app.presets.pending.is_some() {
        ctx.request_repaint_after(super::preset_browser_keys::SETTLE);
    }
    match ask {
        Ask::Hear(id) => app.audition_preset_id(&id),
        Ask::Load(id) => {
            app.apply_preset_id(&id);
        }
        Ask::Close => app.close_preset_browser(),
        Ask::None => {}
    }
    if busy || app.presets.audition.rendering() {
        ctx.request_repaint();
    }
}

/// The window's inside: the title, the search and *Sync BPM*, the chips, the rows, the
/// player.
fn body(app: &mut CantusApp, ui: &mut Ui, rect: Rect, s: &Style) -> Ask {
    let k = s.k;
    let pad = 12.0 * k;
    let unit_name = app
        .presets
        .unit
        .map(|u| t(u.name_key()))
        .unwrap_or_default();
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD * k));
    let mut ask = Ask::None;
    if title(
        ui,
        head,
        s,
        &tf("preset.browser.title", &[("unit", &unit_name)]),
    ) {
        ask = Ask::Close;
    }
    // Drag the title to move the window.
    let grip = Rect::from_min_max(head.min, pos2(head.right() - 40.0 * k, head.bottom()));
    let drag = ui.interact(grip, Id::new("preset-browser-move"), Sense::drag());
    if drag.dragged() {
        app.presets.at = Some(rect.min + drag.drag_delta());
    }
    // The search, and Sync BPM at its right.
    let sync_w = 96.0 * k;
    let search = Rect::from_min_size(
        pos2(rect.left() + pad, head.bottom() + 10.0 * k),
        vec2(rect.width() - 3.0 * pad - sync_w, 26.0 * k),
    );
    let hint = t("preset.browser.search");
    // A printable key typed while the list had the keys starts the search: the field takes
    // the keyboard with the caret after what was typed (not the words selected, or the next
    // key would replace them).
    let focus = std::mem::take(&mut app.presets.focus_search);
    let sid = super::preset_browser_keys::search_id();
    crate::kit::field::Field::new(sid)
        .hint(&hint)
        .pad(20.0, 0.0)
        .open(focus)
        .show(ui, search, &mut app.presets.query.text);
    if focus {
        let eid = crate::kit::field::edit_id(sid);
        if let Some(mut st) = egui::text_edit::TextEditState::load(ui.ctx(), eid) {
            let n = app.presets.query.text.chars().count();
            st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(n),
            )));
            st.store(ui.ctx(), eid);
        }
    }
    with_brush(ui.painter(), |b| {
        let g = Rect::from_center_size(
            pos2(search.left() + 13.0 * k, search.center().y),
            Vec2::splat(12.0 * k),
        );
        super::picker_layout::magnifier(b, g, s.t.text_dim, 1.5 * k);
    });
    let sync = Rect::from_min_size(
        pos2(search.right() + pad, search.top() + 3.0 * k),
        vec2(sync_w, 20.0 * k),
    );
    let tip = t("preset.browser.sync_tip");
    if LedButton::new(
        app.presets.sync_bpm,
        crate::kit::text::Src::Key("preset.browser.sync"),
    )
    .name(&tip)
    .id(Id::new("preset-browser-sync"))
    .caps(crate::kit::hint::Caps::NONE)
    .show_at(ui, sync)
    .clicked()
    {
        app.presets.sync_bpm = !app.presets.sync_bpm;
    }
    // The chips.
    let mut y = search.bottom() + 10.0 * k;
    y = chips(app, ui, rect, y, s);
    let player = Rect::from_min_max(
        pos2(rect.left() + 1.0, rect.bottom() - PLAYER * k),
        rect.max - vec2(1.0, 1.0),
    );
    let list = Rect::from_min_max(
        pos2(rect.left() + 1.0, y + 6.0 * k),
        pos2(rect.right() - 1.0, player.top()),
    );
    with_brush(ui.painter(), |b| {
        b.rect(list, Fill::Solid(s.t.ink));
        b.hline(
            list.left(),
            list.right(),
            b.snap_mid(list.top()),
            b.px(),
            shade(s.t.panel, -0.16),
        );
        b.hline(
            list.left(),
            list.right(),
            b.snap_mid(list.bottom()),
            b.px(),
            shade(s.t.panel, -0.16),
        );
    });
    let rows_ask = super::preset_browser_rows::rows(app, ui, list, s);
    let player_ask = super::preset_browser_rows::player(app, ui, player, s);
    for a in [rows_ask, player_ask] {
        if ask == Ask::None && a != Ask::None {
            ask = a;
        }
    }
    ask
}

/// The title band; returns whether its close key was clicked.
fn title(ui: &mut Ui, rect: Rect, s: &Style, words: &str) -> bool {
    let k = s.k;
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(rect);
        b.hline(
            r.left() + 1.0,
            r.right() - 1.0,
            r.bottom() - b.px() * 0.5,
            b.px(),
            shade(s.t.panel, -0.16),
        );
        let g = Rect::from_center_size(
            pos2(r.left() + 18.0 * k, r.center().y),
            Vec2::splat(14.0 * k),
        );
        glyph::paint(b, g, Glyph::Browser, s.t.text_dim);
        let tr = Rect::from_min_max(
            pos2(g.right() + 8.0 * k, r.top()),
            pos2(r.right() - 44.0 * k, r.bottom()),
        );
        let _ = text::paint(
            b,
            tr,
            &Text::new(words, TextRole::Title, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    let close = Rect::from_center_size(
        pos2(rect.right() - 20.0 * k, rect.center().y),
        Vec2::splat(22.0 * k),
    );
    let resp = ui
        .interact(close, Id::new("preset-browser-close"), Sense::click())
        .on_hover_text(t("preset.browser.close"));
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t("preset.browser.close"))
    });
    with_brush(ui.painter(), |b| {
        let st = State::of(&resp, false);
        crate::kit::cap::toolbar_key(
            b,
            close,
            s,
            st,
            crate::kit::cap::KeyFace::Glyph(Glyph::Close),
        );
    });
    resp.clicked()
}

/// The style chips then the role chips, each line under its label, wrapping; returns the
/// y under them.
fn chips(app: &mut CantusApp, ui: &mut Ui, rect: Rect, top: f32, s: &Style) -> f32 {
    let k = s.k;
    let pad = 12.0 * k;
    let label_w = 64.0 * k;
    let instrument = app.presets.unit.is_some_and(Unit::is_instrument);
    // A chip is there when the unit has a preset of it (the rows do not jump as a search
    // is typed); its number is what a click would answer now.
    let (style_counts, role_counts) = counts(&app.presets.entries);
    let (style_now, role_now) = app.presets.query.facets(&app.presets.entries);
    let styles: Vec<(Genre, usize, usize)> = style_counts
        .into_iter()
        .zip(style_now)
        .map(|((g, all), (_, now))| (g, all, now))
        .collect();
    let roles: Vec<(Role, usize, usize)> = role_counts
        .into_iter()
        .zip(role_now)
        .filter(|((r, _), _)| r.fits(instrument))
        .map(|((r, all), (_, now))| (r, all, now))
        .collect();
    let mut y = top;
    for (band, items) in [
        (
            "preset.browser.styles",
            styles
                .iter()
                .map(|(g, all, now)| (Chip::Style(*g), *all, *now))
                .collect::<Vec<_>>(),
        ),
        (
            "preset.browser.roles",
            roles
                .iter()
                .map(|(r, all, now)| (Chip::Role(*r), *all, *now))
                .collect::<Vec<_>>(),
        ),
    ] {
        // A band with nothing to choose (no preset of this unit names a style yet) is not
        // shown: an empty row would read as a fault.
        if items.iter().all(|(_, all, _)| *all == 0) {
            continue;
        }
        let lr = Rect::from_min_size(pos2(rect.left() + pad, y), vec2(label_w, CHIP * k));
        with_brush(ui.painter(), |b| {
            let _ = text::paint(
                b,
                lr,
                &Text::key(band, TextRole::Micro, s.t.label).align(Align2::LEFT_CENTER),
                s.tk(),
            );
        });
        let mut x = lr.right();
        let right = rect.right() - pad;
        for (chip, all, n) in items {
            let on = match chip {
                Chip::Style(g) => app.presets.query.styles.contains(&g),
                Chip::Role(r) => app.presets.query.roles.contains(&r),
            };
            if all == 0 && !on {
                continue;
            }
            let words = format!("{}  {n}", t(chip.key()));
            let w = text::measure(
                ui.ctx(),
                &Text::new(&words, TextRole::Label, s.t.text),
                s.tk(),
            )
            .x + 18.0 * k;
            if x + w > right && x > lr.right() {
                x = lr.right();
                y += (CHIP + 6.0) * k;
            }
            let r = Rect::from_min_size(pos2(x, y), vec2(w, CHIP * k));
            let resp = ui.interact(r, Id::new(("preset-chip", chip.key())), Sense::click());
            resp.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, t(chip.key()))
            });
            with_brush(ui.painter(), |b| {
                paint_chip(b, r, s, &words, on, resp.hovered())
            });
            if resp.clicked() {
                match chip {
                    Chip::Style(g) => app.presets.query.toggle_style(g),
                    Chip::Role(r) => app.presets.query.toggle_role(r),
                }
                app.presets.scroll = 0.0;
            }
            x += w + 6.0 * k;
        }
        y += (CHIP + 8.0) * k;
    }
    y
}

#[derive(Copy, Clone)]
enum Chip {
    Style(Genre),
    Role(Role),
}

impl Chip {
    fn key(self) -> &'static str {
        match self {
            Chip::Style(g) => g.key(),
            Chip::Role(r) => r.key(),
        }
    }
}

fn paint_chip(b: &mut Brush, r: Rect, s: &Style, words: &str, on: bool, hovered: bool) {
    let base = if on {
        mix(s.t.panel, s.t.brand, 0.42)
    } else if hovered {
        shade(s.t.panel, if s.t.dark { 0.10 } else { -0.07 })
    } else {
        shade(s.t.panel, if s.t.dark { 0.05 } else { -0.04 })
    };
    b.rrect(r, r.height() * 0.5, Fill::Solid(base));
    let edge = if on {
        s.t.brand_bright
    } else {
        with_alpha(s.t.edge, 0.7)
    };
    b.rrect_stroke(r, r.height() * 0.5, b.px(), edge);
    let _ = text::paint(
        b,
        r,
        &Text::new(
            words,
            TextRole::Label,
            if on { s.t.text } else { s.t.text_dim },
        )
        .align(Align2::CENTER_CENTER),
        s.tk(),
    );
}

/// What a row says it is for: its first role, then its styles.
pub(super) fn what_for(e: &Entry) -> String {
    let mut out = t(e.role().key());
    if !e.styles.is_empty() {
        out.push_str(" — ");
        out.push_str(
            &e.styles
                .iter()
                .map(|g| t(g.key()))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if e.source == Source::User {
        out.push_str("  ·  ");
        out.push_str(&t("preset.browser.user"));
    }
    out
}

/// What the player says the audition was heard on.
pub(super) fn heard_on(app: &CantusApp, e: &Entry) -> String {
    if !e.unit.audible() {
        return t("preset.browser.silent");
    }
    let bpm = if app.presets.sync_bpm {
        app.project().tempo_bpm
    } else {
        crate::preset_audition::TEMPO
    };
    use crate::preset_audition::HeardOn;
    // What it was heard on when it was; else what it would be.
    let a = &app.presets.audition;
    let on = match a.on {
        Some(on) if a.last_id.as_deref() == Some(e.id.as_str()) => on,
        _ => match e.unit {
            Unit::Effect(_) => HeardOn::Source(e.role()),
            _ => HeardOn::Phrase(e.role(), bpm),
        },
    };
    use crate::preset_tags::Role;
    match on {
        HeardOn::SelectedClip => t("preset.browser.on_clip"),
        HeardOn::FirstClip => t("preset.browser.on_first_clip"),
        HeardOn::Source(r) => {
            let what = match r {
                Role::Drums => "preset.browser.source.drums",
                Role::Vocal => "preset.browser.source.vocal",
                _ => "preset.browser.source.band",
            };
            tf("preset.browser.on_source", &[("what", &t(what))])
        }
        HeardOn::Phrase(r, bpm) => tf(
            "preset.browser.on_phrase",
            &[("what", &t(phrase_key(r))), ("bpm", &format!("{bpm:.0}"))],
        ),
    }
}

/// The x a fraction `f` of the way across `r` (the player's playhead).
pub(super) fn across(r: Rect, f: f32) -> f32 {
    r.left() + r.width() * f.clamp(0.0, 1.0)
}

/// The name of a role's phrase ("a bass line", "held chords"…), with its article.
pub fn phrase_key(r: crate::preset_tags::Role) -> &'static str {
    use crate::preset_tags::Role;
    match r {
        Role::Bass => "preset.browser.phrase.bass",
        Role::Lead => "preset.browser.phrase.lead",
        Role::Pad => "preset.browser.phrase.pad",
        Role::Pluck => "preset.browser.phrase.pluck",
        Role::Fx => "preset.browser.phrase.fx",
        Role::Drums => "preset.browser.phrase.drums",
        _ => "preset.browser.phrase.keys",
    }
}
