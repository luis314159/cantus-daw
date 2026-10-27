//! **The start page** (round 31, ADR 0072): *File ▸ New from template…*, the palette's
//! *Start page*, Studio One's start page and FL's *New from template* in one panel.
//!
//! At its left, **New from template**: a card for each starter that comes with Cantus
//! (*Empty song*, *Beat*, *Band recording*, *Mastering*) and for each template of the user's,
//! its name over what it holds; a click makes a new project from it (asking first about
//! unsaved changes, as *New* does); a user's card shows a × on hover that deletes the
//! template (after a second click: *Delete?*). At its right, **Recent projects**, newest
//! first, each its name over its folder; a click opens it. Under them, *Open…* and *Save
//! current as template…*. Esc or the close box closes it.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{well, with_brush, Brush, Fill, State, Style};
use crate::templates::{ProjectTemplate, TemplateRef, TrackTemplate};
use crate::views::interop::form;
use crate::views::workspace_settings as ws;
use egui::{pos2, vec2, Align2, Context, Id, Key, Rect, Sense, Ui};
use std::path::PathBuf;

const W: f32 = 860.0;
const H: f32 = 520.0;
const TITLE: f32 = 32.0;
const PAD: f32 = 14.0;
const GAP: f32 = 10.0;
const BAND: f32 = 22.0;
const CARD_H: f32 = 64.0;
const ROW: f32 = 38.0;
const FOOT: f32 = 30.0;
const TRACK_ROW: f32 = 28.0;

/// The page's state.
#[derive(Default)]
pub struct StartPage {
    open: bool,
    fresh: bool,
    /// A user's template asked to be deleted: the next click on its × deletes it.
    confirm: Option<PathBuf>,
    /// Where each card and row was drawn last frame (tests, shots): its name.
    pub rects: Vec<(String, Rect)>,
    pub panel: Option<Rect>,
    /// What the page lists, read from disk when it opens and when a template is saved or
    /// deleted (the workflow's count) — never each frame (review 5: a project on a share
    /// that went away froze the window): the templates, the track templates, the recent
    /// projects and whether each is there.
    pub templates: Vec<ProjectTemplate>,
    pub tracks: Vec<TrackTemplate>,
    pub recent: Vec<(PathBuf, bool)>,
    read_at: Option<u64>,
    /// It opened by itself as Cantus started.
    at_launch: bool,
}

impl StartPage {
    /// Open at launch (`workflow::open_at_start`): as [`Self::open`], and it says so.
    pub fn open_at_launch(&mut self) {
        self.open();
        self.at_launch = true;
    }

    /// Whether it opened by itself as Cantus started.
    pub fn opened_at_launch(&self) -> bool {
        self.open && self.at_launch
    }

    pub fn open(&mut self) {
        *self = StartPage {
            open: true,
            fresh: true,
            ..Default::default()
        };
    }

    pub fn close(&mut self) {
        *self = StartPage::default();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
}

/// What a frame decided.
enum Out {
    Stay,
    Close,
    Template(TemplateRef),
    Recent(PathBuf),
    Delete(PathBuf),
    Open,
    SaveAs,
    /// *Show at start* switched.
    AtStart(bool),
}

/// Draw the page when it is open, and do what is chosen. Once a frame.
pub fn frame(app: &mut CantusApp, ctx: &Context) {
    if !app.workflow.start.is_open() {
        return;
    }
    let rev = app.workflow.templates_rev;
    if app.workflow.start.read_at != Some(rev) {
        let st = &mut app.workflow.start;
        st.templates = app.workflow.store.projects();
        st.tracks = app.workflow.store.tracks();
        // Whether each recent project is there, looked at once.
        st.recent = app
            .workflow
            .recent
            .projects
            .iter()
            .map(|p| (p.clone(), p.exists()))
            .collect();
        st.read_at = Some(rev);
    }
    let mut st = std::mem::take(&mut app.workflow.start);
    let (templates, tracks, recent) = (
        std::mem::take(&mut st.templates),
        std::mem::take(&mut st.tracks),
        std::mem::take(&mut st.recent),
    );
    // The start verse's card (bottom-right, not modal) stays in view: the page keeps clear
    // of it (ADR 0072).
    let avoid = app.verse.filter(|_| app.settings.verse.show).map(|c| {
        let shown = crate::verse::Shown {
            reference: c.reference,
            spanish: app.settings.verse.spanish,
        };
        crate::views::verse::card_layout(ctx, ctx.content_rect(), shown).0
    });
    let at_start = app.settings.start_page_at_start;
    let out = show(
        &mut st,
        ctx,
        (&templates, &tracks, &recent),
        at_start,
        avoid,
    );
    st.templates = templates;
    st.tracks = tracks;
    st.recent = recent;
    st.fresh = false;
    app.workflow.start = st;
    match out {
        Out::Stay => {}
        Out::Close => app.workflow.start.close(),
        Out::Template(tpl) => {
            app.workflow.start.close();
            app.request_template(tpl);
        }
        Out::Recent(p) => {
            app.workflow.start.close();
            app.request_open_path(p);
        }
        Out::Delete(p) => {
            app.workflow.start.confirm = None;
            match app.workflow.store.delete(&p) {
                Ok(()) => {
                    let name = p
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    app.set_status(tf("workflow.status.deleted", &[("name", &name)]));
                    app.workflow.templates_changed();
                }
                Err(e) => app.set_error(e),
            }
        }
        Out::Open => {
            app.workflow.start.close();
            app.request_open();
        }
        Out::SaveAs => {
            app.workflow.start.close();
            crate::views::template_name::ask_project(app);
        }
        Out::AtStart(on) => set_at_start(app, on),
    }
}

/// *Show at start* (the page's switch and Settings ▸ General's), kept in the settings.
pub fn set_at_start(app: &mut CantusApp, on: bool) {
    app.settings.start_page_at_start = on;
    app.store_settings();
}

/// Settings ▸ General's part: the start page's switch.
pub fn settings(app: &mut CantusApp, ui: &mut Ui) {
    use crate::kit::text::Src;
    ui.add_space(8.0);
    crate::views::workspace_widgets::band(ui, Src::Key("workflow.start.settings_band"));
    let mut on = app.settings.start_page_at_start;
    let name = t("workflow.start.at_start");
    if crate::kit::widgets::led_button(
        ui,
        &mut on,
        Src::Key("workflow.start.at_start"),
        &name,
        260.0,
    )
    .changed()
    {
        set_at_start(app, on);
    }
}

type Lists<'a> = (
    &'a [ProjectTemplate],
    &'a [TrackTemplate],
    &'a [(PathBuf, bool)],
);

fn show(
    st: &mut StartPage,
    ctx: &Context,
    lists: Lists<'_>,
    at_start: bool,
    avoid: Option<Rect>,
) -> Out {
    let (templates, tracks, recent) = lists;
    let s = Style::current();
    let k = s.k;
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        return Out::Close;
    }
    let screen = ctx.content_rect();
    let size = vec2(
        (W * k).min(screen.width() - 16.0),
        (H * k).min(screen.height() - 16.0),
    );
    let mut size = size;
    let mut at = screen.center() - size * 0.5;
    if let Some(card) = avoid.filter(|c| c.intersects(Rect::from_min_size(at, size).expand(12.0))) {
        // Beside the verse's card when there is room for the page at its left, else above.
        let room = card.left() - 12.0 * k - (screen.left() + 8.0);
        if room >= 640.0 * k {
            size.x = size.x.min(room);
            at.x = screen.left() + 8.0 + (room - size.x) * 0.5;
        } else {
            at.y = (card.top() - 12.0 * k - size.y).max(screen.top() + 8.0);
        }
    }
    let rect = Rect::from_min_size(at, size);
    st.panel = Some(rect);
    st.rects.clear();
    let mut out = Out::Stay;
    egui::Area::new(Id::new("cantus-start-page"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ws::frame(ui, rect, &s);
            let title = Rect::from_min_size(rect.min, vec2(rect.width(), TITLE * k));
            if ws::title(ui, title, &s, Glyph::Plus, "workflow.start.title") {
                out = Out::Close;
            }
            let body = Rect::from_min_max(
                pos2(rect.left() + PAD * k, title.bottom() + GAP * k),
                pos2(
                    rect.right() - PAD * k,
                    rect.bottom() - (PAD + FOOT + GAP) * k,
                ),
            );
            let split = body.left() + body.width() * 0.6;
            let left = Rect::from_min_max(body.min, pos2(split - GAP * 0.5 * k, body.bottom()));
            let right = Rect::from_min_max(pos2(split + GAP * 0.5 * k, body.top()), body.max);
            if let Some(o) = cards(ui, st, &s, left, templates) {
                out = o;
            }
            // The recent projects over the track templates (listed to delete them; they are
            // inserted from the picker, the devices panel and the palette).
            let cut = right.top() + right.height() * 0.58;
            let top = Rect::from_min_max(right.min, pos2(right.right(), cut - GAP * 0.5 * k));
            let bottom = Rect::from_min_max(pos2(right.left(), cut + GAP * 0.5 * k), right.max);
            if let Some(o) = recents(ui, st, &s, top, recent) {
                out = o;
            }
            if let Some(o) = track_list(ui, st, &s, bottom, tracks) {
                out = o;
            }
            let foot = Rect::from_min_max(
                pos2(body.left(), body.bottom() + GAP * k),
                pos2(body.right(), body.bottom() + (GAP + FOOT) * k),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(foot)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            if form::button(&mut child, &t("workflow.start.open"), false, true) {
                out = Out::Open;
            }
            if form::button(&mut child, &t("workflow.start.save_as"), false, true) {
                out = Out::SaveAs;
            }
            // *Show at start*, at the foot's left (the same switch as Settings ▸ General).
            let sw = Rect::from_min_size(
                pos2(foot.left(), foot.center().y - s.d(10.0)),
                vec2(200.0 * k, s.d(20.0)),
            );
            let r = ui.interact(sw, Id::new("cantus-start-at-start"), Sense::click());
            if r.clicked() {
                out = Out::AtStart(!at_start);
            }
            with_brush(ui.painter(), |b| {
                crate::kit::led::button(
                    b,
                    sw,
                    &s,
                    State::of(&r, at_start),
                    crate::kit::text::Src::Key("workflow.start.at_start"),
                )
            });
            st.rects.push(("at-start".into(), sw));
        });
    let (pressed, pointer) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
    if matches!(out, Out::Stay)
        && pressed
        && !st.fresh
        && !pointer.is_some_and(|p| rect.contains(p) || avoid.is_some_and(|c| c.contains(p)))
    {
        out = Out::Close;
    }
    out
}

/// A section band: the words in capitals on a slightly lifted ground.
fn band(b: &mut Brush, s: &Style, r: Rect, key: &str) {
    b.rect(
        r,
        Fill::Solid(shade(s.t.well, if s.t.dark { 0.04 } else { -0.03 })),
    );
    let t = Text {
        caps: true,
        ..Text::key(key, Role::Section, s.t.label)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End))
    };
    let _ = text::paint(b, r.shrink2(vec2(10.0 * s.k, 0.0)), &t, s.tk());
}

/// The templates as cards, two to a row, in a well that scrolls when they overflow.
fn cards(
    ui: &mut Ui,
    st: &mut StartPage,
    s: &Style,
    area: Rect,
    list: &[ProjectTemplate],
) -> Option<Out> {
    let k = s.k;
    let head = Rect::from_min_size(area.min, vec2(area.width(), BAND * k));
    let well_r = Rect::from_min_max(pos2(area.left(), head.bottom() + 4.0 * k), area.max);
    with_brush(ui.painter(), |b| {
        band(b, s, head, "workflow.start.templates");
        well::paint(b, well_r, s, State::REST, None);
    });
    let inner = well_r.shrink(6.0 * k);
    let cols = 2usize;
    let cw = (inner.width() - GAP * k) / cols as f32;
    let rows = list.len().div_ceil(cols);
    let total = rows as f32 * (CARD_H + GAP) * k;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    child.set_clip_rect(inner.intersect(ui.clip_rect()));
    let mut out = None;
    egui::ScrollArea::vertical()
        .id_salt("cantus-start-cards")
        .auto_shrink([false, false])
        .max_height(inner.height())
        .show(&mut child, |ui| {
            ui.set_height(total.max(inner.height()));
            let (top, left) = (ui.max_rect().top(), ui.max_rect().left());
            let mut paint: Vec<(Rect, usize, bool, bool)> = Vec::new();
            for (i, tpl) in list.iter().enumerate() {
                let (r, c) = (i / cols, i % cols);
                let rect = Rect::from_min_size(
                    pos2(left + c as f32 * (cw + GAP * k), top + r as f32 * (CARD_H + GAP) * k),
                    vec2(cw, CARD_H * k),
                );
                let resp = ui.interact(rect, Id::new(("cantus-start-card", i)), Sense::click());
                let mut on_x = false;
                if let TemplateRef::User(path) = &tpl.template {
                    let x = x_rect(rect, s);
                    let xr = ui.interact(x, Id::new(("cantus-start-del", i)), Sense::click());
                    on_x = xr.hovered();
                    if xr.clicked() {
                        if st.confirm.as_ref() == Some(path) {
                            out = Some(Out::Delete(path.clone()));
                        } else {
                            st.confirm = Some(path.clone());
                        }
                    }
                }
                if resp.clicked() && !on_x && out.is_none() {
                    out = Some(Out::Template(tpl.template.clone()));
                }
                st.rects.push((tpl.name.clone(), rect));
                paint.push((rect, i, resp.hovered(), on_x));
            }
            let painter = ui.painter().clone();
            with_brush(&painter, |b| {
                for &(rect, i, hover, on_x) in &paint {
                    let tpl = &list[i];
                    let confirm = matches!(&tpl.template, TemplateRef::User(p) if st.confirm.as_ref() == Some(p));
                    card(b, rect, s, tpl, hover, on_x, confirm);
                }
            });
        });
    out
}

fn x_rect(card: Rect, s: &Style) -> Rect {
    Rect::from_min_size(
        pos2(card.right() - 22.0 * s.k, card.top() + 4.0 * s.k),
        vec2(18.0 * s.k, 18.0 * s.k),
    )
}

/// One card: a stripe of its hue at the left, the name over what it holds, a user's ×.
fn card(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    tpl: &ProjectTemplate,
    hover: bool,
    on_x: bool,
    confirm: bool,
) {
    let k = s.k;
    let base = if hover {
        mix(s.t.panel_raised, s.t.selection, 0.12)
    } else {
        s.t.panel_raised
    };
    b.rrect(rect, s.r(4.0), Fill::Solid(base));
    let hue = match &tpl.template {
        TemplateRef::Starter(_) => s.t.highlight,
        TemplateRef::User(_) => s.t.led,
    };
    let stripe = Rect::from_min_size(rect.min, vec2(4.0 * k, rect.height()));
    b.rrect(stripe, s.r(2.0), Fill::Solid(hue));
    let body = rect.shrink2(vec2(14.0 * k, 10.0 * k));
    let user = matches!(tpl.template, TemplateRef::User(_));
    let right = if user || confirm {
        x_rect(rect, s).left() - 6.0 * k
    } else {
        body.right()
    };
    let top = Rect::from_min_max(body.min, pos2(right, body.center().y));
    let bottom = Rect::from_min_max(pos2(body.left(), body.center().y), body.max);
    let n = Text::new(&tpl.name, Role::Body, s.t.text)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, top, &n, s.tk());
    let w = if confirm {
        t("workflow.start.delete_confirm")
    } else {
        tpl.what.clone()
    };
    let wc = if confirm { s.t.led } else { s.t.text_dim };
    let wt = Text::new(&w, Role::Label, wc)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, bottom, &wt, s.tk());
    if user && (hover || on_x || confirm) {
        let x = x_rect(rect, s);
        if on_x {
            b.rrect(
                x,
                s.r(3.0),
                Fill::Solid(shade(base, if s.t.dark { 0.08 } else { -0.06 })),
            );
        }
        let ink = if confirm || on_x {
            s.t.text
        } else {
            s.t.text_dim
        };
        crate::kit::glyph::paint(b, x.shrink(3.0 * k), Glyph::Close, ink);
    }
}

/// The recent projects, newest first: the name over its folder.
fn recents(
    ui: &mut Ui,
    st: &mut StartPage,
    s: &Style,
    area: Rect,
    list: &[(PathBuf, bool)],
) -> Option<Out> {
    let k = s.k;
    let head = Rect::from_min_size(area.min, vec2(area.width(), BAND * k));
    let well_r = Rect::from_min_max(pos2(area.left(), head.bottom() + 4.0 * k), area.max);
    with_brush(ui.painter(), |b| {
        band(b, s, head, "workflow.start.recent");
        well::paint(b, well_r, s, State::REST, None);
    });
    let inner = well_r.shrink(k);
    if list.is_empty() {
        with_brush(ui.painter(), |b| {
            let t = Text::key("workflow.start.no_recent", Role::Body, s.t.placeholder)
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, inner.shrink(8.0 * k), &t, s.tk());
        });
        return None;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    child.set_clip_rect(inner.intersect(ui.clip_rect()));
    let mut out = None;
    egui::ScrollArea::vertical()
        .id_salt("cantus-start-recent")
        .auto_shrink([false, false])
        .max_height(inner.height())
        .show(&mut child, |ui| {
            let row_h = ROW * k;
            ui.set_height((list.len() as f32 * row_h).max(inner.height()));
            let (top, left, w) = (
                ui.max_rect().top(),
                ui.max_rect().left(),
                ui.max_rect().width(),
            );
            let mut paint = Vec::new();
            for (i, (p, there)) in list.iter().enumerate() {
                let rect = Rect::from_min_size(pos2(left, top + i as f32 * row_h), vec2(w, row_h));
                let resp = ui.interact(rect, Id::new(("cantus-start-recent", i)), Sense::click());
                if resp.clicked() {
                    out = Some(Out::Recent(p.clone()));
                }
                let name = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                st.rects.push((name.clone(), rect));
                paint.push((rect, name, p, *there, resp.hovered()));
            }
            let painter = ui.painter().clone();
            with_brush(&painter, |b| {
                for (rect, name, p, there, hover) in &paint {
                    if *hover {
                        b.rect(
                            *rect,
                            Fill::Solid(shade(s.t.well, if s.t.dark { 0.05 } else { -0.04 })),
                        );
                    }
                    b.hline(
                        rect.left(),
                        rect.right(),
                        rect.bottom() - b.px() * 0.5,
                        b.px(),
                        shade(s.t.well, if s.t.dark { -0.06 } else { -0.08 }),
                    );
                    let body = rect.shrink2(vec2(10.0 * k, 4.0 * k));
                    let top = Rect::from_min_max(body.min, pos2(body.right(), body.center().y));
                    let bottom = Rect::from_min_max(pos2(body.left(), body.center().y), body.max);
                    let gone = !*there;
                    let ink = if gone {
                        with_alpha(s.t.text, 0.5)
                    } else {
                        s.t.text
                    };
                    let n = Text::new(name, Role::Body, ink)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End));
                    let _ = text::paint(b, top, &n, s.tk());
                    let folder = if gone {
                        t("workflow.start.missing")
                    } else {
                        p.parent()
                            .map(|d| d.display().to_string())
                            .unwrap_or_default()
                    };
                    let f = Text::new(&folder, Role::Label, s.t.text_dim)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::Middle));
                    let _ = text::paint(b, bottom, &f, s.tk());
                }
            });
        });
    out
}

/// The user's track templates, each its name and, on hover, its × (twice to delete).
fn track_list(
    ui: &mut Ui,
    st: &mut StartPage,
    s: &Style,
    area: Rect,
    list: &[TrackTemplate],
) -> Option<Out> {
    let k = s.k;
    let head = Rect::from_min_size(area.min, vec2(area.width(), BAND * k));
    let well_r = Rect::from_min_max(pos2(area.left(), head.bottom() + 4.0 * k), area.max);
    with_brush(ui.painter(), |b| {
        band(b, s, head, "workflow.start.track_templates");
        well::paint(b, well_r, s, State::REST, None);
    });
    let inner = well_r.shrink(k);
    if list.is_empty() {
        with_brush(ui.painter(), |b| {
            let t = Text::key(
                "workflow.start.no_track_templates",
                Role::Label,
                s.t.placeholder,
            )
            .align(Align2::CENTER_CENTER)
            .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, inner.shrink(8.0 * k), &t, s.tk());
        });
        return None;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    child.set_clip_rect(inner.intersect(ui.clip_rect()));
    let mut out = None;
    egui::ScrollArea::vertical()
        .id_salt("cantus-start-tracks")
        .auto_shrink([false, false])
        .max_height(inner.height())
        .show(&mut child, |ui| {
            let row_h = TRACK_ROW * k;
            ui.set_height((list.len() as f32 * row_h).max(inner.height()));
            let (top, left, w) = (
                ui.max_rect().top(),
                ui.max_rect().left(),
                ui.max_rect().width(),
            );
            let mut paint = Vec::new();
            for (i, tpl) in list.iter().enumerate() {
                let rect = Rect::from_min_size(pos2(left, top + i as f32 * row_h), vec2(w, row_h));
                let row = ui.interact(rect, Id::new(("cantus-start-track", i)), Sense::hover());
                let x = Rect::from_center_size(
                    pos2(rect.right() - 14.0 * k, rect.center().y),
                    vec2(18.0 * k, 18.0 * k),
                );
                let xr = ui.interact(x, Id::new(("cantus-start-track-del", i)), Sense::click());
                let confirm = st.confirm.as_ref() == Some(&tpl.path);
                if xr.clicked() {
                    if confirm {
                        out = Some(Out::Delete(tpl.path.clone()));
                    } else {
                        st.confirm = Some(tpl.path.clone());
                    }
                }
                st.rects.push((tpl.name.clone(), rect));
                paint.push((
                    rect,
                    x,
                    tpl,
                    row.hovered() || xr.hovered(),
                    xr.hovered(),
                    confirm,
                ));
            }
            let painter = ui.painter().clone();
            with_brush(&painter, |b| {
                for &(rect, x, tpl, hover, on_x, confirm) in &paint {
                    b.hline(
                        rect.left(),
                        rect.right(),
                        rect.bottom() - b.px() * 0.5,
                        b.px(),
                        shade(s.t.well, if s.t.dark { -0.06 } else { -0.08 }),
                    );
                    let body = Rect::from_min_max(
                        pos2(rect.left() + 10.0 * k, rect.top()),
                        pos2(x.left() - 6.0 * k, rect.bottom()),
                    );
                    let words = if confirm {
                        t("workflow.start.delete_confirm")
                    } else {
                        tpl.name.clone()
                    };
                    let ink = if confirm { s.t.led } else { s.t.text };
                    let n = Text::new(&words, Role::Body, ink)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End));
                    let _ = text::paint(b, body, &n, s.tk());
                    if hover || confirm {
                        if on_x {
                            b.rrect(
                                x,
                                s.r(3.0),
                                Fill::Solid(shade(s.t.well, if s.t.dark { 0.08 } else { -0.06 })),
                            );
                        }
                        let ink = if on_x || confirm {
                            s.t.text
                        } else {
                            s.t.text_dim
                        };
                        crate::kit::glyph::paint(b, x.shrink(3.0 * k), Glyph::Close, ink);
                    }
                }
            });
        });
    out
}
