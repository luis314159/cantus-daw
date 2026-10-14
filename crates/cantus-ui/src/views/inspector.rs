//! The inspector (ADR 0024 §7): the selected channel's whole strip as a **signal-flow
//! sheet**, in the kit's look.
//!
//! - **It follows what was clicked last**: a strip on the desk shows that channel; a
//!   track header shows the track with its channel below; a clip or notes show them,
//!   above their track's channel. The title row says which — "Mixer – Keys", the context
//!   part dimmed (FL).
//! - **Top to bottom, the signal's own order**: input → inserts as tiles (▸ opens a mini
//!   view of the effect without its editor) → the strip EQ as a display that is a
//!   control → sends → routing and delay compensation → output; the channel's fader,
//!   pan and meter pinned at the bottom (Bitwig), so a channel can be mixed without the
//!   desk.
//! - **One row grammar** (`inspector_sheet`): label | value rows of 24 pt, labels
//!   right-aligned in a fixed column, every value in a well; what does not apply is
//!   dimmed, never hidden.
//! - **Resizable and collapsible**: 280 pt by default, 220–420 by dragging its edge, a
//!   snap at 280, right-click resets, double-click (or the chevron) collapses it to a
//!   24-pt rail. The host (a dock) reads [`desired_width`] to size itself.
//!
//! Performance: rows are painted through one brush; values are formatted when they
//! change (`Fmt`), lists are built when their menu opens, curves when their settings
//! move.

#[path = "inspector_audio.rs"]
pub mod audio;
#[path = "inspector_channel.rs"]
mod channel;
#[path = "inspector_eq.rs"]
mod eq;
#[path = "inspector_hosted.rs"]
mod hosted;
#[path = "inspector_menus.rs"]
mod menus;
#[path = "inspector_mini.rs"]
mod mini;
#[path = "inspector_pads.rs"]
mod pads;
#[path = "inspector_pinned.rs"]
mod pinned;
#[path = "inspector_receives.rs"]
mod receives;
#[path = "inspector_record.rs"]
mod record;
#[path = "inspector_sends.rs"]
mod sends;
#[path = "inspector_sheet.rs"]
mod sheet;
#[path = "inspector_track.rs"]
mod track;

use crate::app::CantusApp;
use crate::kit::color::shade;
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{cache, Brush, Fill, State, Style};
use cantus_core::{ClipId, Command, EffectId, MergeKey, StripId, TrackId};
use egui::{pos2, Align2, Id, Rect, Sense, Vec2};
pub use menus::MenuId;
use sheet::{Fmt, Sheet};

/// The default width, and where a drag snaps.
pub const WIDTH: f32 = 280.0;
pub const MIN_WIDTH: f32 = 220.0;
pub const MAX_WIDTH: f32 = 420.0;
/// The collapsed panel.
pub const RAIL: f32 = 24.0;
/// How close a drag must come to [`WIDTH`] to land on it.
const SNAP: f32 = 10.0;
const TITLE: f32 = 28.0;
const PINNED: f32 = 150.0;

/// What the inspector is about.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Subject {
    /// A strip chosen on the desk: its channel alone.
    Strip(StripId),
    /// A track header: the track, then its channel.
    Track(TrackId),
    /// A clip: the clip, then its track's channel.
    Clip(TrackId, ClipId),
    /// Notes in the piano roll: them, then their track's channel.
    Notes(TrackId, ClipId),
    /// Nothing selected: the master.
    Master,
}

/// A list the inspector has open, and the field it hangs from.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MenuOpen {
    pub id: MenuId,
    pub anchor: Rect,
}

/// What a row asks for; run after painting, when the project is no longer borrowed.
#[derive(Clone, Debug)]
pub enum Act {
    Exec(Command),
    Merge(Command, MergeKey),
    EndGesture,
    StripGain(StripId, f32),
    ToggleSolo(StripId),
    SelectTrack(TrackId),
    OpenEditor(StripId, EffectId),
    /// An instrument's window: opened, or brought back and to the front (ADR 0051).
    OpenInstrument(TrackId),
    OpenPulsus(TrackId, cantus_core::PadId),
    /// Open the device picker (round 20): swap the instrument, add or swap an insert.
    Pick(crate::picker::PickPurpose),
    SelectPad(usize),
    LoadSample(TrackId, usize),
    RemovePad(TrackId, usize),
    Flat(StripId),
    /// The audio clip section's buttons (locate, show, normalise).
    Audio(audio::AudioAct),
    /// The clip section's link row (round 26): select the copies, make unique, link to….
    Link(TrackId, ClipId, track::LinkAct),
}

/// The inspector's own state: what it follows, its size, and what it keeps between
/// frames so that nothing is rebuilt that has not changed.
pub struct Inspector {
    /// What was clicked last, as the selection calls report it ([`Inspector::follow`]).
    follow: Option<Subject>,
    /// The notes selection last seen (count, first id), to notice a new one.
    notes_seen: (usize, u64),
    /// The subject shown last frame (the text cache is cleared when it changes).
    shown: Option<Subject>,
    /// The width asked for (points); the host sizes its dock from [`desired_width`].
    pub width: f32,
    pub collapsed: bool,
    /// The host's width last frame: a change there is the dock's divider being dragged.
    host_seen: f32,
    brush: Brush,
    fmt: Fmt,
    acts: Vec<Act>,
    menu: Option<MenuOpen>,
    /// The list open last frame, and the frame it opened in (the click that opened it
    /// is not a click outside it).
    menu_seen: Option<MenuId>,
    menu_pass: u64,
    /// A native instrument's spec, copied when the project changes (its strip is drawn
    /// by the instrument's editor, which needs the application too).
    native: Option<(u64, TrackId, cantus_core::NativeSpec)>,
    /// Where the scrollbar asked the sheet to be.
    scroll_to: Option<f32>,
    chan: channel::Chan,
    pin: pinned::Meter,
}

impl Default for Inspector {
    fn default() -> Self {
        Inspector {
            follow: None,
            notes_seen: (0, 0),
            shown: None,
            width: WIDTH,
            collapsed: false,
            host_seen: f32::NAN,
            brush: Brush::default(),
            fmt: Fmt::default(),
            acts: Vec::new(),
            menu: None,
            menu_seen: None,
            menu_pass: 0,
            native: None,
            scroll_to: None,
            chan: channel::Chan::default(),
            pin: pinned::Meter::default(),
        }
    }
}

impl std::fmt::Debug for Inspector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inspector")
            .field("follow", &self.follow)
            .field("width", &self.width)
            .field("collapsed", &self.collapsed)
            .finish_non_exhaustive()
    }
}

impl Inspector {
    /// Say what was clicked last. The selection calls (`select_strip`, `select_track`,
    /// `select_clip`) report here, so the inspector knows a strip click from a track
    /// click even when both leave the same things selected.
    pub fn follow(&mut self, s: Subject) {
        self.follow = Some(s);
    }

    /// Open or close an insert's mini view (its tile's ▸).
    pub fn toggle_mini(&mut self, e: EffectId) {
        match self.chan.open.iter().position(|x| *x == e) {
            Some(i) => {
                self.chan.open.remove(i);
            }
            None => self.chan.open.push(e),
        }
    }

    /// Collapse to the rail, or open again (the I key's action).
    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
    }

    /// Set the width as a drag would: kept in 220–420, snapped to 280 when near it.
    pub fn set_width(&mut self, w: f32) {
        let w = w.clamp(MIN_WIDTH, MAX_WIDTH);
        self.width = if (w - WIDTH).abs() < SNAP { WIDTH } else { w };
    }

    /// A sheet over `ui` that paints through this inspector's brush.
    fn sheet<'a>(&'a mut self, ui: &'a mut egui::Ui, s: &Style, col: f32) -> Sheet<'a> {
        Sheet {
            ui,
            b: &mut self.brush,
            fmt: &mut self.fmt,
            acts: &mut self.acts,
            menu: &mut self.menu,
            s: *s,
            col,
        }
    }
}

/// The width the host should give the inspector: its own, or the rail's.
pub fn desired_width(app: &CantusApp) -> f32 {
    if app.inspector.collapsed {
        RAIL
    } else {
        app.inspector.width
    }
}

/// The subject: what was clicked last, when the selection still agrees with it; else
/// what the selection says.
pub fn subject(app: &CantusApp) -> Subject {
    let p = app.project();
    let track_ok = |t: TrackId| app.selected_track == Some(t) && p.track(t).is_some();
    let clip_ok = |t: TrackId, c: ClipId| {
        track_ok(t)
            && app.selected_clip == Some(c)
            && p.track(t).is_some_and(|x| x.clip(c).is_some())
    };
    let notes = !app.selected_notes.is_empty();
    match app.inspector.follow {
        Some(Subject::Strip(s)) if app.selected_strip == Some(s) && p.strip(s).is_some() => {
            return Subject::Strip(s)
        }
        Some(Subject::Track(t)) if track_ok(t) => return Subject::Track(t),
        // The master chosen on the desk (ADR 0026).
        Some(Subject::Master) => return Subject::Master,
        Some(Subject::Clip(t, c) | Subject::Notes(t, c)) if clip_ok(t, c) => {
            return if notes {
                Subject::Notes(t, c)
            } else {
                Subject::Clip(t, c)
            };
        }
        _ => {}
    }
    if let (Some(t), Some(c)) = (app.selected_track, app.selected_clip) {
        if notes && clip_ok(t, c) {
            return Subject::Notes(t, c);
        }
    }
    // A strip that the selected track does not feed was chosen on its own.
    if let Some(s) = app.selected_strip.filter(|s| p.strip(*s).is_some()) {
        let fed = app
            .selected_track
            .and_then(|t| p.track(t))
            .is_some_and(|t| t.strip == Some(s));
        if !fed {
            return Subject::Strip(s);
        }
    }
    if let Some(t) = app.selected_track.filter(|t| p.track(*t).is_some()) {
        return Subject::Track(t);
    }
    Subject::Master
}

/// The strip a subject's channel sheet is about, if it has one.
pub fn strip_of(app: &CantusApp, s: Subject) -> Option<StripId> {
    let p = app.project();
    match s {
        Subject::Strip(id) => Some(id),
        Subject::Track(t) | Subject::Clip(t, _) | Subject::Notes(t, _) => {
            p.track(t).and_then(|t| t.strip)
        }
        Subject::Master => None,
    }
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    let mut st = std::mem::take(&mut app.inspector);
    // A new notes selection is a click on notes.
    let sig = (
        app.selected_notes.len(),
        app.selected_notes.first().map_or(0, |n| n.0),
    );
    if sig != st.notes_seen {
        st.notes_seen = sig;
        if sig.0 > 0 {
            if let (Some(t), Some(c)) = (app.selected_track, app.selected_clip) {
                st.follow = Some(Subject::Notes(t, c));
            }
        }
    }
    app.inspector.follow = st.follow;
    st.frame(app, ui);
    // A selection call made while the rows acted landed on the placeholder: keep it.
    let late = app.inspector.follow;
    app.inspector = st;
    if late.is_some() && late != app.inspector.follow {
        app.inspector.follow = late;
    }
}

impl Inspector {
    fn frame(&mut self, app: &mut CantusApp, ui: &mut egui::Ui) {
        let s = Style::current();
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        // The host's divider moved (or this is the first frame): take its width, within
        // the range and the snap. A host narrower than the rail is the rail's own.
        let host = full.width();
        if self.host_seen.is_nan() || (host - self.host_seen).abs() > 0.5 {
            if !self.collapsed && host > RAIL + 1.0 {
                self.set_width(host);
            }
            self.host_seen = host;
        }
        let w = if self.collapsed {
            RAIL * s.k
        } else {
            self.width.min(full.width())
        };
        let rect = Rect::from_min_max(pos2(full.right() - w, full.top()), full.max);
        let painter = ui.painter_at(full);
        self.brush.begin(ui.ctx());
        if rect.left() > full.left() + 0.5 {
            // Room the host gave beyond the panel's width: the workspace's ground.
            let gap = Rect::from_min_max(full.min, pos2(rect.left(), full.bottom()));
            self.brush.rect(gap, Fill::Solid(s.t.ink));
        }
        let subject = subject(app);
        if self.shown != Some(subject) {
            self.shown = Some(subject);
            self.fmt.clear();
            self.menu = None;
        }
        if self.collapsed {
            self.rail(app, ui, rect, &s, subject);
            self.brush.flush(&painter);
            return;
        }
        self.brush.rect(rect, Fill::Solid(s.t.panel));
        self.edge(ui, rect, &s);
        let title = Rect::from_min_size(rect.min, Vec2::new(rect.width(), s.s(TITLE)));
        self.title(app, ui, title, &s, subject);
        let pin_h = s.s(PINNED).min((rect.height() - title.height()) * 0.5);
        let pin = Rect::from_min_max(pos2(rect.left(), rect.bottom() - pin_h), rect.max);
        let body = Rect::from_min_max(
            pos2(rect.left(), title.bottom()),
            pos2(rect.right(), pin.top()),
        );
        // FL's inspector column: the tiles sit on the lighter ground behind them.
        self.brush.rect(body, Fill::Solid(s.t.panel_light));
        self.brush.flush(&painter);

        self.acts.clear();
        // The sheet scrolls under a kit scrollbar that is always there (ADR 0024 §10).
        let lane = Rect::from_min_max(
            pos2(body.right() - s.s(7.0), body.top() + s.s(2.0)),
            pos2(body.right() - s.s(1.0), body.bottom() - s.s(2.0)),
        );
        let area = Rect::from_min_max(body.min, pos2(lane.left() - s.s(1.0), body.bottom()));
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(area)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(area.intersect(ui.clip_rect()));
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("inspector-sheet")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
        if let Some(y) = self.scroll_to.take() {
            scroll = scroll.vertical_scroll_offset(y);
        }
        let out = scroll.show(&mut child, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            self.body(app, ui, &s, subject);
        });
        let content = out.content_size.y.max(1.0);
        let len = (out.inner_rect.height() / content).min(1.0);
        let mut start = out.state.offset.y / content;
        let bar =
            crate::kit::widgets::scrollbar(ui, lane, Id::new("inspector-scroll"), &mut start, len);
        if bar.dragged() {
            self.scroll_to = Some(start * content);
        }
        self.brush.begin(ui.ctx());
        pinned::show(self, app, ui, pin, &s, subject);
        self.brush.flush(&painter);
        menus::show(self, app, ui, &s);
        let acts = std::mem::take(&mut self.acts);
        for a in acts.iter().cloned() {
            apply(app, a);
        }
        self.acts = acts;
        // Round 25: the route menus the Receives rows open, and what they hand over.
        super::route_menu::show(app, ui, super::route_menu::inspector_owner());
    }

    /// Everything between the title and the pinned strip, in the signal's order.
    fn body(&mut self, app: &mut CantusApp, ui: &mut egui::Ui, s: &Style, subject: Subject) {
        let painter = ui.painter().clone();
        self.brush.begin(ui.ctx());
        let w = ui.available_width();
        let col = (w * 0.36).clamp(84.0, 120.0);
        let strip = strip_of(app, subject);
        // What sits above the channel: the track, the clip or the notes.
        {
            let mut sh = self.sheet(ui, s, col);
            match subject {
                Subject::Track(t) => {
                    track::rows(&mut sh, app, t);
                    // An audio track: its audio clip's section where an instrument's
                    // would be (its input and recording are in the channel below).
                    if audio_track(app, t) {
                        if let Some(c) = crate::record_session::inspected_audio_clip(app, t) {
                            audio::show(&mut sh, app, t, c);
                        }
                    }
                }
                Subject::Clip(t, c) => {
                    track::clip(&mut sh, app, t, c);
                    audio::show(&mut sh, app, t, c);
                }
                Subject::Notes(t, c) => track::notes(&mut sh, app, t, c),
                _ => {}
            }
        }
        self.brush.flush(&painter);
        if let Some(t) = match subject {
            Subject::Track(t) if !audio_track(app, t) => Some(t),
            _ => None,
        } {
            // The instrument's section may hold the instrument editor's own widgets:
            // it flushes the brush around them so they paint under nothing of ours.
            track::instrument(self, app, ui, s, col, t);
        }
        self.brush.begin(ui.ctx());
        let mut chan = std::mem::take(&mut self.chan);
        {
            let mut sh = self.sheet(ui, s, col);
            channel::show(&mut sh, &mut chan, app, subject, strip);
            sh.ui.add_space(s.d(10.0));
        }
        self.chan = chan;
        self.brush.flush(&painter);
    }

    /// The panel's left edge: drag to resize (220–420, snapping at 280), right-click to
    /// reset, double-click to collapse.
    fn edge(&mut self, ui: &mut egui::Ui, rect: Rect, s: &Style) {
        let grip = Rect::from_min_max(rect.min, pos2(rect.left() + s.s(5.0), rect.bottom()));
        let r = ui.interact(grip, Id::new("inspector-edge"), Sense::click_and_drag());
        if r.drag_started() {
            ui.data_mut(|d| d.insert_temp(Id::new("inspector-edge-raw"), self.width));
        }
        if r.dragged() {
            // The raw width follows the pointer; the snap applies to what is shown, so
            // a drag passes through 280 instead of sticking to it.
            let dx = r.drag_delta().x;
            let width = self.width;
            let raw = ui.data_mut(|d| {
                let v = d.get_temp_mut_or_insert_with(Id::new("inspector-edge-raw"), || width);
                *v -= dx;
                *v
            });
            self.set_width(raw);
        }
        if r.secondary_clicked() {
            self.width = WIDTH;
        }
        if r.double_clicked() {
            self.collapsed = true;
        }
        if r.hovered() || r.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            let name = self.fmt.tr("inspector.edge_tip");
            hint::set_text(ui.ctx(), name, "", Caps::NONE);
        }
        let st = State::of(&r, false);
        let line = if st.hovered || st.active {
            s.t.focus
        } else {
            shade(s.t.panel, -0.16)
        };
        let px = self.brush.px();
        self.brush
            .vline(rect.left() + px * 0.5, rect.top(), rect.bottom(), px, line);
    }

    /// The title row: ▸ (the panel's menu), "Mixer – Keys" with the context dimmed, and a
    /// chevron that collapses the panel to its rail.
    fn title(
        &mut self,
        app: &CantusApp,
        ui: &mut egui::Ui,
        rect: Rect,
        s: &Style,
        subject: Subject,
    ) {
        let b = &mut self.brush;
        let t = &s.t;
        b.register(
            rect,
            "title_row",
            &[
                crate::kit::Tok::Panel,
                crate::kit::Tok::Text,
                crate::kit::Tok::TextDim,
                crate::kit::Tok::Sheen,
            ],
            &[],
        );
        b.rect(
            b.snap_rect(rect),
            Fill::V(shade(t.panel, 0.03), shade(t.panel, -0.01)),
        );
        crate::kit::light::sheen(b, rect, t);
        b.hline(
            rect.left(),
            rect.right(),
            rect.bottom() - b.px() * 0.5,
            b.px(),
            shade(t.panel, -0.15),
        );
        let menu = Rect::from_center_size(
            pos2(rect.left() + s.s(14.0), rect.center().y),
            Vec2::splat(s.s(20.0)),
        );
        let mr = ui.interact(menu, Id::new("inspector-title-menu"), Sense::click());
        glyph::paint(
            b,
            menu.shrink(s.s(4.0)),
            Glyph::Menu,
            if mr.hovered() { t.text } else { t.text_dim },
        );
        if mr.clicked() {
            self.menu = match self.menu {
                Some(m) if m.id == MenuId::Panel => None,
                _ => Some(MenuOpen {
                    id: MenuId::Panel,
                    anchor: menu,
                }),
            };
        }
        let fold = Rect::from_center_size(
            pos2(rect.right() - s.s(14.0), rect.center().y),
            Vec2::splat(s.s(20.0)),
        );
        let fr = ui.interact(fold, Id::new("inspector-title-fold"), Sense::click());
        glyph::paint(
            b,
            fold.shrink(s.s(5.0)),
            Glyph::Next,
            if fr.hovered() { t.text } else { t.text_dim },
        );
        if fr.hovered() {
            let name = self.fmt.tr("inspector.collapse");
            hint::set_text(ui.ctx(), name, "", Caps::NONE);
        }
        if fr.clicked() {
            self.collapsed = true;
        }
        let area = Rect::from_min_max(
            pos2(menu.right() + s.s(4.0), rect.top()),
            pos2(fold.left() - s.s(4.0), rect.bottom()),
        );
        let ctx_key = context_key(subject);
        let name = subject_name(&mut self.fmt, app, subject);
        let ctx_t = Text::key(ctx_key, Role::Title, t.text_dim).align(Align2::LEFT_CENTER);
        let cw = text::measure(b.ctx(), &ctx_t, s.tk())
            .x
            .min(area.width() * 0.45);
        let _ = text::paint(
            b,
            Rect::from_min_size(area.min, Vec2::new(cw + 1.0, area.height())),
            &ctx_t,
            s.tk(),
        );
        let dash = Text::new("\u{2013}", Role::Title, t.text_dim).align(Align2::CENTER_CENTER);
        let dx = area.left() + cw + s.s(4.0);
        let _ = text::paint(
            b,
            Rect::from_min_size(pos2(dx, area.top()), Vec2::new(s.s(8.0), area.height())),
            &dash,
            s.tk(),
        );
        let nt = Text::new(name, Role::Title, t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::Middle));
        let _ = text::paint(
            b,
            Rect::from_min_max(pos2(dx + s.s(12.0), area.top()), area.max),
            &nt,
            s.tk(),
        );
    }

    /// The collapsed panel: a 24-pt rail with the chevron that opens it and the subject's
    /// name on its side. A click anywhere on it opens the panel again.
    fn rail(
        &mut self,
        app: &CantusApp,
        ui: &mut egui::Ui,
        rect: Rect,
        s: &Style,
        subject: Subject,
    ) {
        let r = ui.interact(rect, Id::new("inspector-rail"), Sense::click());
        let b = &mut self.brush;
        let t = &s.t;
        b.register(
            rect,
            "rail",
            &[crate::kit::Tok::Panel, crate::kit::Tok::Text],
            &[],
        );
        b.rect(
            rect,
            Fill::Solid(if r.hovered() {
                shade(t.panel, 0.03)
            } else {
                t.panel
            }),
        );
        b.vline(
            rect.left() + b.px() * 0.5,
            rect.top(),
            rect.bottom(),
            b.px(),
            shade(t.panel, -0.16),
        );
        let chev = Rect::from_center_size(
            pos2(rect.center().x, rect.top() + s.s(14.0)),
            Vec2::splat(s.s(12.0)),
        );
        glyph::paint(
            b,
            chev,
            Glyph::Prev,
            if r.hovered() { t.text } else { t.text_dim },
        );
        let name = subject_name(&mut self.fmt, app, subject);
        let label = Rect::from_min_max(
            pos2(rect.left(), chev.bottom() + s.s(8.0)),
            pos2(rect.right(), rect.bottom() - s.s(8.0)),
        );
        let nt = Text::new(name, Role::Label, t.text_dim)
            .align(Align2::CENTER_TOP)
            .policy(Policy::Rotate90(Elide::End));
        let _ = text::paint(b, label, &nt, s.tk());
        if r.hovered() {
            let name = self.fmt.tr("inspector.expand");
            hint::set_text(ui.ctx(), name, "", Caps::NONE);
        }
        if r.clicked() {
            self.collapsed = false;
        }
    }
}

/// The dimmed first half of the title: which kind of thing is shown.
/// Whether a track is an audio track (an input, no note clips): no instrument section.
fn audio_track(app: &CantusApp, t: cantus_core::TrackId) -> bool {
    app.project()
        .track(t)
        .is_some_and(crate::record_session::is_audio_track)
}

fn context_key(s: Subject) -> &'static str {
    match s {
        Subject::Strip(_) | Subject::Master => "kit.mixer.title",
        Subject::Track(_) => "inspector.ctx.track",
        Subject::Clip(..) => "inspector.ctx.clip",
        Subject::Notes(..) => "inspector.ctx.notes",
    }
}

/// The subject's name: a strip's own, else the first track feeding it, else its number.
fn subject_name<'f>(fmt: &'f mut Fmt, app: &CantusApp, s: Subject) -> &'f str {
    let p = app.project();
    let stamp = app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60);
    let key = cache::key(("insp-subject", s));
    match s {
        Subject::Strip(id) => fmt.get(key, stamp, |b| strip_label(app, id, b)),
        Subject::Track(t) => fmt.get(key, stamp, |b| {
            if let Some(t) = p.track(t) {
                b.push_str(&t.name)
            }
        }),
        Subject::Clip(t, c) | Subject::Notes(t, c) => fmt.get(key, stamp, |b| {
            if let Some(c) = p.track(t).and_then(|t| t.clip(c)) {
                b.push_str(&c.name)
            }
        }),
        Subject::Master => fmt.get(key, stamp, |b| b.push_str(&crate::i18n::t("mixer.master"))),
    }
}

/// How a strip is named: its own name, else the first track feeding it, else "Strip n".
pub(crate) fn strip_label(app: &CantusApp, id: StripId, b: &mut String) {
    let p = app.project();
    let Some(strip) = p.strip(id) else {
        return;
    };
    if !strip.name.is_empty() {
        b.push_str(&strip.name);
    } else if let Some(t) = p.tracks_on_strip(id).next() {
        b.push_str(&t.name);
    } else {
        let n = p.strip_number(id).unwrap_or(0);
        b.push_str(&crate::i18n::tf("inspector.strip_n", &[("n", &n)]));
    }
}

/// Run what the rows asked for.
fn apply(app: &mut CantusApp, a: Act) {
    match a {
        Act::Exec(c) => {
            app.exec(c);
        }
        Act::Merge(c, k) => {
            app.exec_merging(c, k);
        }
        Act::EndGesture => app.end_gesture(),
        Act::StripGain(id, db) => app.set_strip_gain(id, db),
        Act::ToggleSolo(id) => app.toggle_strip_solo(id),
        Act::SelectTrack(t) => app.select_track(t),
        Act::OpenEditor(s, e) => app.open_effect_editor(s, e),
        Act::OpenInstrument(t) => app.open_inst_editor(t),
        Act::Pick(purpose) => app.open_picker(purpose),
        Act::OpenPulsus(t, pad) => {
            app.pulsus.selected = Some(pad);
            crate::views::pulsus_editor::open(app, t);
        }
        Act::SelectPad(i) => app.rack.selected_pad = i,
        Act::LoadSample(t, pad) => app.load_pad_sample(t, pad),
        Act::RemovePad(t, pad) => app.remove_pad(t, pad),
        Act::Audio(a) => audio::apply(app, a),
        Act::Link(t, c, a) => track::apply_link(app, t, c, a),
        Act::Flat(id) => {
            let commands = (0..3u8)
                .map(|band| Command::SetStripEq {
                    id,
                    band,
                    field: cantus_core::EqField::Gain,
                    value: 0.0,
                })
                .collect();
            app.exec(Command::Batch {
                label_key: "cmd.set_strip_eq".into(),
                commands,
            });
        }
    }
}

/// Which strip a track feeds, as a menu field (the channel rack's row uses it too).
pub fn route_picker(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, current: Option<StripId>) {
    menus::route_picker(app, ui, id, current);
}

/// The strip EQ's picture for another view: the same display the inspector draws.
pub fn eq_curve(ui: &mut egui::Ui, eq: &cantus_core::StripEq) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 112.0), Sense::hover());
    let s = Style::current().function(crate::kit::Function::Tone);
    let mut curve = eq::EqCurve::default();
    crate::kit::with_brush(ui.painter(), |b| {
        eq::eq_display(b, rect, &s, eq, &mut curve, None, None)
    });
}
