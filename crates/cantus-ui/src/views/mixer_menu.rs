//! The desk's menus, drawn with the kit's `Menu` (FL 2026's rows, bands, check column,
//! chevrons) in a floating area, with one level of submenus and a swatch grid for colours.
//!
//! **A list is built when its menu opens, never before**: the old desk worked out every
//! strip's "send to" targets every frame for a `+` nobody had pressed (O(N³), 2.8 ms and
//! 8,064 allocations at 64 strips). Here the rows exist only while the menu shows.

use super::layout::{Colourful, Row, WidthMode};
use super::{in_selection, MixerView};
use crate::app::CantusApp;
use crate::keymap::Action;
use crate::kit::color::{safe_track, FillSize};
use crate::kit::menu::{self as kmenu, Check, Item};
use crate::kit::text::{self, Role, Src, Text};
use crate::kit::{Fill, Style};
use cantus_core::{
    Color, Command, Dock, EffectId, EffectKind, GroupId, PanLaw, Send, StripId, TrackId,
    MAX_INSERTS,
};
use egui::{pos2, vec2, Id, Pos2, Rect, Sense};

/// Which menu is open.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Menu {
    /// The desk's ▸ menu: width, rows, view, mix.
    Desk,
    /// A strip's menu (right-click on its number or name).
    Strip(StripId),
    /// A group separator's menu.
    Group(GroupId),
    /// Where the selected strip goes: its output and its sends.
    Route(StripId),
    /// The route from the selected strip to this one (right-click on a send arrow).
    Arrow(StripId),
    PanLaw,
    Takes,
    /// What to put in an empty insert slot.
    AddEffect(StripId),
    /// A loaded insert slot.
    Slot(StripId, EffectId),
    /// The master's menu (right-click on its number or name): its chain (ADR 0026).
    Master,
}

/// A submenu, opened from a row with a chevron.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Sub {
    Width,
    Rows,
    Colourful,
    PanLaw,
    Colors(ColorOf),
    Output(StripId),
    SendTo(StripId),
    Duck(StripId),
    Dock(StripId),
    Group(StripId),
    AddEffect(StripId),
    /// The device input of the strip's recording track (FL's mixer input; ADR 0025).
    Input(StripId),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ColorOf {
    Strip(StripId),
    Group(GroupId),
}

/// What a row does.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Act {
    /// Automate a strip's (or the master's) volume (0), pan (1) or width (2) (ADR 0027).
    Automate(StripId, u8),
    Rename(StripId),
    RenameGroup(GroupId),
    Color(ColorOf, Option<Color>),
    Solo(StripId),
    Mute(StripId),
    Mono(StripId),
    RouteHere(StripId),
    SendHere(StripId),
    Submix(StripId),
    RouteTrack(TrackId, StripId),
    Duck(StripId, StripId),
    Dock(StripId, Dock),
    GroupNew(StripId),
    GroupJoin(StripId, GroupId),
    GroupLeave(StripId),
    Move(StripId, i32),
    Remove(StripId),
    GroupSelect(GroupId),
    GroupDissolve(GroupId),
    Output(StripId, Option<StripId>),
    ToggleSend(StripId, StripId),
    SendOnOff(StripId, StripId),
    Width(WidthMode),
    Row(Row),
    Colourful(Colourful),
    Wave,
    ResetRatio,
    PanLaw(PanLaw),
    UnmuteAll,
    SoloOff,
    AddStrip,
    AddEffect(StripId, EffectKind),
    OpenEffect(StripId, EffectId),
    Bypass(StripId, EffectId, bool),
    MoveEffect(StripId, EffectId, usize),
    RemoveEffect(StripId, EffectId),
    /// The master's whole chain in or out.
    MasterFx(bool),
    /// A strip's whole chain in or out.
    FxOn(StripId, bool),
    Reveal(usize),
    StopTake,
    /// The strip's recording track takes this input (first channel, stereo), or none.
    Input(StripId, Option<(u16, bool)>),
    /// Arm or disarm the strip's tracks that have an input.
    Arm(StripId),
    /// Record what leaves the strip's fader (the take recorder), for a strip whose record
    /// dot arms its input.
    Capture(StripId),
}

/// A row's words.
enum Label {
    Key(&'static str),
    Own(String),
}

enum Then {
    Act(Act),
    Sub(Sub),
    Nothing,
}

enum Kind {
    Action,
    Band,
    Separator,
}

/// One row of a menu being built.
struct Entry {
    kind: Kind,
    label: Label,
    check: Check,
    shortcut: Option<String>,
    enabled: bool,
    then: Then,
}

impl Entry {
    fn act(label: Label, act: Act) -> Entry {
        Entry {
            kind: Kind::Action,
            label,
            check: Check::None,
            shortcut: None,
            enabled: true,
            then: Then::Act(act),
        }
    }

    fn sub(label: Label, sub: Sub) -> Entry {
        Entry {
            then: Then::Sub(sub),
            ..Entry::act(label, Act::AddStrip)
        }
    }

    fn band(key: &'static str) -> Entry {
        Entry {
            kind: Kind::Band,
            then: Then::Nothing,
            ..Entry::act(Label::Key(key), Act::AddStrip)
        }
    }

    fn separator() -> Entry {
        Entry {
            kind: Kind::Separator,
            then: Then::Nothing,
            ..Entry::act(Label::Key(""), Act::AddStrip)
        }
    }

    fn check(mut self, c: Check) -> Entry {
        self.check = c;
        self
    }

    fn enabled(mut self, on: bool) -> Entry {
        self.enabled = on;
        self
    }

    fn shortcut(mut self, s: String) -> Entry {
        if !s.is_empty() {
            self.shortcut = Some(s);
        }
        self
    }
}

fn on(b: bool) -> Check {
    if b {
        Check::On
    } else {
        Check::Off
    }
}

fn radio(b: bool) -> Check {
    Check::Radio(b)
}

/// The open menu and submenu.
#[derive(Debug, Default)]
pub(super) struct Popups {
    open: Option<(Menu, Pos2)>,
    sub: Option<(Sub, Pos2, usize)>,
    /// Opened this frame: the click that opened it must not close it.
    fresh: bool,
    serial: u64,
}

impl Popups {
    pub fn open(&mut self, m: Menu, at: Pos2) {
        self.open = Some((m, at));
        self.sub = None;
        self.fresh = true;
        self.serial = self.serial.wrapping_add(1);
    }

    pub fn is_open(&self, m: Menu) -> bool {
        self.open.is_some_and(|(o, _)| o == m)
    }

    pub fn close(&mut self) {
        self.open = None;
        self.sub = None;
    }
}

/// A strip as a menu row says it: "3 · Bass", or "3".
fn strip_label(v: &MixerView, app: &CantusApp, index: usize) -> String {
    let name = v.desk.name(app, index);
    if name.is_empty() {
        (index + 1).to_string()
    } else {
        format!("{} \u{b7} {name}", index + 1)
    }
}

/// The strips a strip-level operation acts on: the selection when the strip is in it.
fn targets(app: &CantusApp, id: StripId) -> Vec<StripId> {
    if in_selection(app, id) {
        app.strip_selection()
    } else {
        vec![id]
    }
}

/// The rows of a menu, built now that it is open.
fn entries(v: &MixerView, app: &CantusApp, m: Menu) -> Vec<Entry> {
    let p = app.project();
    let mut e = Vec::new();
    match m {
        Menu::Desk => {
            e.push(Entry::sub(Label::Key("mixer.menu.width"), Sub::Width));
            e.push(Entry::sub(Label::Key("mixer.menu.rows"), Sub::Rows));
            e.push(Entry::sub(
                Label::Key("mixer.menu.colourful"),
                Sub::Colourful,
            ));
            e.push(Entry::sub(Label::Key("mixer.pan_law"), Sub::PanLaw));
            e.push(Entry::separator());
            e.push(
                Entry::act(Label::Key("mixer.wave"), Act::Wave)
                    .check(on(app.settings.mixer_meter_wave))
                    .shortcut(app.keymap.display_for(Action::ToggleMeterWave)),
            );
            e.push(Entry::act(
                Label::Key("mixer.menu.reset_ratio"),
                Act::ResetRatio,
            ));
            e.push(Entry::separator());
            e.push(Entry::act(Label::Key("mixer.add_strip"), Act::AddStrip));
            e.push(
                Entry::act(Label::Key("mixer.unmute_all"), Act::UnmuteAll)
                    .enabled(v.desk.any_muted),
            );
            e.push(Entry::act(Label::Key("mixer.solo_off"), Act::SoloOff).enabled(v.desk.any_solo));
        }
        Menu::Strip(id) => {
            let Some(index) = p.strip_index(id) else {
                return e;
            };
            let s = &p.strips[index];
            e.push(Entry::act(Label::Key("mixer.menu.rename"), Act::Rename(id)));
            automate_entries(id, &mut e);
            e.push(Entry::sub(
                Label::Key("mixer.menu.color"),
                Sub::Colors(ColorOf::Strip(id)),
            ));
            e.push(Entry::separator());
            e.push(Entry::act(Label::Key("track.solo_tip"), Act::Solo(id)).check(on(s.soloed)));
            e.push(Entry::act(Label::Key("track.mute_tip"), Act::Mute(id)).check(on(s.muted)));
            e.push(Entry::act(Label::Key("track.mono"), Act::Mono(id)).check(on(s.width <= 0.005)));
            if p.tracks_on_strip(id).next().is_some() {
                // Recording (ADR 0025): the input, arm, and the mix take for a strip whose
                // record dot now arms.
                let (inputs, armed) = rec_state(app, id);
                e.push(Entry::band("rec.recording"));
                e.push(Entry::sub(Label::Key("rec.input"), Sub::Input(id)));
                e.push(
                    Entry::act(Label::Key("rec.arm"), Act::Arm(id))
                        .check(on(armed))
                        .enabled(inputs),
                );
                if inputs {
                    e.push(
                        Entry::act(Label::Key("rec.capture_strip"), Act::Capture(id))
                            .check(on(app.capturing(cantus_engine::CaptureTarget::Strip(id)))),
                    );
                }
            }
            e.push(Entry::band("mixer.menu.routing"));
            e.push(Entry::sub(Label::Key("mixer.menu.output"), Sub::Output(id)));
            e.push(Entry::sub(
                Label::Key("mixer.menu.send_to"),
                Sub::SendTo(id),
            ));
            let selection = app.strip_selection();
            let others = selection.iter().any(|s| *s != id);
            e.push(
                Entry::act(Label::Key("mixer.menu.route_here"), Act::RouteHere(id)).enabled(others),
            );
            e.push(
                Entry::act(Label::Key("mixer.menu.send_here"), Act::SendHere(id)).enabled(others),
            );
            e.push(
                Entry::act(Label::Key("mixer.menu.submix"), Act::Submix(id))
                    .enabled(selection.contains(&id)),
            );
            if let Some(track) = app.selected_track.and_then(|t| p.track(t)) {
                e.push(
                    Entry::act(
                        Label::Own(crate::i18n::tf(
                            "mixer.menu.route_track",
                            &[("track", &track.name)],
                        )),
                        Act::RouteTrack(track.id, id),
                    )
                    .enabled(track.strip != Some(id)),
                );
            }
            e.push(Entry::sub(Label::Key("comp.duck.menu"), Sub::Duck(id)));
            e.push(Entry::band("master.menu.effects"));
            e.push(Entry::sub(Label::Key("effect.add"), Sub::AddEffect(id)));
            e.push(
                Entry::act(
                    Label::Key("master.menu.fx_on"),
                    Act::FxOn(id, !s.fx_enabled),
                )
                .check(on(s.fx_enabled))
                .enabled(!s.effects.is_empty()),
            );
            e.push(Entry::band("mixer.menu.desk"));
            e.push(Entry::sub(Label::Key("mixer.menu.dock"), Sub::Dock(id)));
            e.push(Entry::sub(Label::Key("mixer.menu.group"), Sub::Group(id)));
            let last = p.strips.len().saturating_sub(1);
            e.push(
                Entry::act(Label::Key("mixer.menu.move_left"), Act::Move(id, -1))
                    .enabled(index > 0)
                    .shortcut(app.keymap.display_for(Action::MoveStripLeft)),
            );
            e.push(
                Entry::act(Label::Key("mixer.menu.move_right"), Act::Move(id, 1))
                    .enabled(index < last)
                    .shortcut(app.keymap.display_for(Action::MoveStripRight)),
            );
            let in_use = p.tracks_on_strip(id).next().is_some()
                || p.strips.iter().any(|s| s.feeds().any(|to| to == id));
            e.push(Entry::act(Label::Key("mixer.menu.remove"), Act::Remove(id)).enabled(!in_use));
        }
        Menu::Group(g) => {
            e.push(Entry::act(
                Label::Key("mixer.menu.rename"),
                Act::RenameGroup(g),
            ));
            e.push(Entry::sub(
                Label::Key("mixer.menu.color"),
                Sub::Colors(ColorOf::Group(g)),
            ));
            e.push(Entry::separator());
            e.push(Entry::act(
                Label::Key("mixer.group.select"),
                Act::GroupSelect(g),
            ));
            e.push(Entry::act(
                Label::Key("mixer.group.dissolve"),
                Act::GroupDissolve(g),
            ));
        }
        Menu::Route(id) => {
            sub_entries(v, app, Sub::Output(id), &mut e);
            sub_entries(v, app, Sub::SendTo(id), &mut e);
        }
        Menu::Arrow(to) => {
            let Some(sel) = app.selected_strip.and_then(|s| p.strip(s)) else {
                return e;
            };
            let loops = p
                .strip_index(to)
                .and_then(|i| v.desk.would_cycle.get(i).copied())
                .unwrap_or(true);
            let send = sel.send_to(to);
            e.push(
                Entry::act(
                    Label::Key("mixer.menu.send_here_one"),
                    Act::ToggleSend(sel.id, to),
                )
                .check(on(send.is_some()))
                .enabled(send.is_some() || !loops),
            );
            if let Some(send) = send {
                e.push(
                    Entry::act(Label::Key("mixer.menu.send_on"), Act::SendOnOff(sel.id, to))
                        .check(on(send.enabled)),
                );
            }
            let here = sel.output == Some(to);
            e.push(
                Entry::act(
                    Label::Key("mixer.menu.route_only"),
                    Act::Output(sel.id, Some(to)),
                )
                .check(radio(here))
                .enabled(here || !loops),
            );
            e.push(
                Entry::act(
                    Label::Key("mixer.menu.route_master"),
                    Act::Output(sel.id, None),
                )
                .check(radio(sel.output.is_none())),
            );
        }
        Menu::PanLaw => sub_entries(v, app, Sub::PanLaw, &mut e),
        Menu::Takes => {
            if app.mix_recorder.status().is_some() {
                e.push(Entry::act(Label::Key("take.stop_tip"), Act::StopTake));
                e.push(Entry::separator());
            }
            for (i, take) in app.mix_recorder.takes.iter().enumerate().rev() {
                e.push(Entry::act(
                    Label::Own(format!(
                        "{}  \u{b7}  {:.1} s  \u{b7}  {:.1} dB",
                        take.label,
                        take.seconds,
                        cantus_core::linear_to_db(take.peak)
                    )),
                    Act::Reveal(i),
                ));
            }
        }
        Menu::AddEffect(id) => sub_entries(v, app, Sub::AddEffect(id), &mut e),
        Menu::Master => {
            let m = &p.master;
            automate_entries(StripId::MASTER, &mut e);
            e.push(Entry::band("master.menu.effects"));
            e.push(Entry::sub(
                Label::Key("effect.add"),
                Sub::AddEffect(StripId::MASTER),
            ));
            e.push(
                Entry::act(
                    Label::Key("master.menu.fx_on"),
                    Act::MasterFx(!m.fx_enabled),
                )
                .check(on(m.fx_enabled))
                .enabled(!m.effects.is_empty()),
            );
            e.push(Entry::separator());
            e.push(Entry::sub(Label::Key("mixer.pan_law"), Sub::PanLaw));
        }
        Menu::Slot(id, fx) => {
            let Some(s) = p.effects_of(id) else {
                return e;
            };
            let Some(i) = s.iter().position(|x| x.id == fx) else {
                return e;
            };
            let bypass = s[i].bypass;
            e.push(Entry::act(
                Label::Key("mixer.menu.open_editor"),
                Act::OpenEffect(id, fx),
            ));
            e.push(
                Entry::act(Label::Key("effect.bypass"), Act::Bypass(id, fx, !bypass))
                    .check(on(bypass)),
            );
            e.push(
                Entry::act(
                    Label::Key("effect.up"),
                    Act::MoveEffect(id, fx, i.saturating_sub(1)),
                )
                .enabled(i > 0),
            );
            e.push(
                Entry::act(Label::Key("effect.down"), Act::MoveEffect(id, fx, i + 1))
                    .enabled(i + 1 < s.len()),
            );
            e.push(Entry::separator());
            e.push(Entry::act(
                Label::Key("effect.remove"),
                Act::RemoveEffect(id, fx),
            ));
        }
    }
    e
}

/// The rows of a submenu (and of the menus made of them).
fn sub_entries(v: &MixerView, app: &CantusApp, sub: Sub, e: &mut Vec<Entry>) {
    let p = app.project();
    let prefs = v.prefs.current;
    match sub {
        Sub::Colors(_) => {}
        Sub::Width => {
            for mode in WidthMode::ALL {
                e.push(
                    Entry::act(Label::Key(mode.key()), Act::Width(mode))
                        .check(radio(prefs.width == mode)),
                );
            }
        }
        Sub::Rows => {
            for r in Row::TOGGLES {
                e.push(
                    Entry::act(Label::Key(r.legend_key()), Act::Row(r))
                        .check(on(prefs.rows.has(r))),
                );
            }
        }
        Sub::Colourful => {
            for c in Colourful::ALL {
                e.push(
                    Entry::act(Label::Key(c.key()), Act::Colourful(c))
                        .check(radio(prefs.colourful == c)),
                );
            }
        }
        Sub::PanLaw => {
            for law in PanLaw::ALL {
                e.push(
                    Entry::act(Label::Key(law.label_key()), Act::PanLaw(law))
                        .check(radio(p.pan_law == law)),
                );
            }
        }
        Sub::Input(id) => {
            let Some(track) = rec_track(app, id).and_then(|t| p.track(t)) else {
                return;
            };
            e.push(Entry::band("rec.input"));
            let current = track.input.as_ref().map(|i| (i.first, i.stereo));
            e.push(
                Entry::act(Label::Key("rec.no_input_choice"), Act::Input(id, None))
                    .check(radio(current.is_none())),
            );
            let channels = app.input_channels();
            if channels == 0 {
                e.push(
                    Entry::act(Label::Key("rec.no_device"), Act::Input(id, current)).enabled(false),
                );
            }
            for c in crate::record_session::input_choices(channels) {
                let mut word = String::new();
                crate::record_session::input_label(&c, &mut word);
                let key = (c.first, c.stereo);
                e.push(
                    Entry::act(Label::Own(word), Act::Input(id, Some(key)))
                        .check(radio(current == Some(key))),
                );
            }
        }
        Sub::Output(id) => {
            let Some(s) = p.strip(id) else {
                return;
            };
            e.push(Entry::band("mixer.menu.output"));
            e.push(
                Entry::act(Label::Key("mixer.master"), Act::Output(id, None))
                    .check(radio(s.output.is_none())),
            );
            for (i, other) in p.strips.iter().enumerate() {
                if other.id == id {
                    continue;
                }
                let loops = p.routing_would_cycle(id, Some(other.id));
                e.push(
                    Entry::act(
                        Label::Own(strip_label(v, app, i)),
                        Act::Output(id, Some(other.id)),
                    )
                    .check(radio(s.output == Some(other.id)))
                    .enabled(!loops),
                );
            }
        }
        Sub::SendTo(id) => {
            let Some(s) = p.strip(id) else {
                return;
            };
            e.push(Entry::band("mixer.menu.send_to"));
            for (i, other) in p.strips.iter().enumerate() {
                if other.id == id {
                    continue;
                }
                let sends = s.send_to(other.id).is_some();
                let loops = !sends && p.routing_would_cycle(id, Some(other.id));
                e.push(
                    Entry::act(
                        Label::Own(strip_label(v, app, i)),
                        Act::ToggleSend(id, other.id),
                    )
                    .check(on(sends))
                    .enabled(!loops),
                );
            }
        }
        Sub::Duck(id) => {
            e.push(Entry::band("comp.duck.hint"));
            for (i, other) in p.strips.iter().enumerate() {
                if other.id == id || p.routing_would_cycle(other.id, Some(id)) {
                    continue;
                }
                e.push(Entry::act(
                    Label::Own(strip_label(v, app, i)),
                    Act::Duck(id, other.id),
                ));
            }
        }
        Sub::Dock(id) => {
            let dock = p.strip(id).map(|s| s.dock);
            for d in Dock::ALL {
                e.push(
                    Entry::act(Label::Key(d.label_key()), Act::Dock(id, d))
                        .check(radio(dock == Some(d))),
                );
            }
        }
        Sub::Group(id) => {
            let current = p.strip(id).and_then(|s| s.group);
            e.push(Entry::act(
                Label::Key("mixer.menu.group_new"),
                Act::GroupNew(id),
            ));
            if !p.strip_groups.is_empty() {
                e.push(Entry::separator());
            }
            for g in &p.strip_groups {
                e.push(
                    Entry::act(Label::Own(g.name.clone()), Act::GroupJoin(id, g.id))
                        .check(radio(current == Some(g.id))),
                );
            }
            if current.is_some() {
                e.push(Entry::separator());
                e.push(Entry::act(
                    Label::Key("mixer.menu.group_leave"),
                    Act::GroupLeave(id),
                ));
            }
        }
        Sub::AddEffect(id) => {
            let full = p.effects_of(id).is_none_or(|s| s.len() >= MAX_INSERTS);
            e.push(Entry::band("effect.add"));
            for kind in EffectKind::ALL {
                e.push(
                    Entry::act(Label::Key(kind.name_key()), Act::AddEffect(id, kind))
                        .enabled(!full),
                );
            }
        }
    }
}

/// What a list reported this frame.
struct Shown {
    rect: Rect,
    clicked: Option<usize>,
    toggled: Option<usize>,
    hovered: Option<usize>,
    /// Each row's rect, for anchoring a submenu.
    row_of_hovered: Option<Rect>,
}

/// How wide a list must be for its longest row.
fn width_of(ctx: &egui::Context, items: &[Item<'_>], s: &Style) -> f32 {
    let mut w: f32 = 150.0;
    for it in items {
        if let Item::Action {
            label,
            shortcut,
            submenu,
            ..
        } = it
        {
            let t = Text {
                src: *label,
                role: Role::Body,
                caps: false,
                align: egui::Align2::LEFT_CENTER,
                policy: text::Policy::default(),
                color: s.t.text,
            };
            let mut row = 22.0 + text::measure(ctx, &t, s.tk()).x + 20.0;
            if let Some(sc) = shortcut {
                row += text::measure(ctx, &Text::new(sc, Role::Body, s.t.text), s.tk()).x + 12.0;
            }
            if *submenu {
                row += 18.0;
            }
            w = w.max(row);
        }
    }
    w.min(420.0)
}

/// Show `entries` as a kit menu at `at`, scrolling when taller than the window.
fn list(ctx: &egui::Context, id: Id, at: Pos2, entries: &[Entry], s: &Style) -> Shown {
    let items: Vec<Item<'_>> = entries
        .iter()
        .map(|e| {
            let src = match &e.label {
                Label::Key(k) => Src::Key(k),
                Label::Own(s) => Src::Str(s),
            };
            match e.kind {
                Kind::Band => Item::Band(src),
                Kind::Separator => Item::Separator,
                Kind::Action => Item::Action {
                    label: src,
                    check: e.check,
                    shortcut: e.shortcut.as_deref(),
                    submenu: matches!(e.then, Then::Sub(_)),
                    enabled: e.enabled,
                },
            }
        })
        .collect();
    let width = width_of(ctx, &items, s);
    // A menu that would run off the bottom of the window moves up (a context menu opens
    // where the pointer is), and scrolls only when taller than the window.
    let screen = ctx.content_rect();
    let max_h = (screen.height() - 16.0).max(120.0);
    let h = kmenu::height(&items, s.d(1.0)).min(max_h);
    let at = pos2(
        at.x,
        at.y.min(screen.bottom() - 8.0 - h).max(screen.top() + 8.0),
    );
    let mut out = Shown {
        rect: Rect::NOTHING,
        clicked: None,
        toggled: None,
        hovered: None,
        row_of_hovered: None,
    };
    let area = egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .show(ui, |ui| {
                    let (resp, clicks) = crate::kit::widgets::menu_list(ui, &items, width);
                    out.clicked = clicks.clicked;
                    out.toggled = clicks.toggled;
                    if let Some(p) = resp.hover_pos() {
                        for i in 0..items.len() {
                            let r = kmenu::row(resp.rect, &items, s.d(1.0), i);
                            if r.contains(p) && items[i].is_action() {
                                out.hovered = Some(i);
                                out.row_of_hovered = Some(r);
                            }
                        }
                    }
                });
        });
    out.rect = area.response.rect;
    out
}

/// The colour swatches of a strip or a group, in a small floating grid.
fn swatches(
    v: &MixerView,
    app: &CantusApp,
    ctx: &egui::Context,
    id: Id,
    at: Pos2,
    of: ColorOf,
    s: &Style,
) -> (Rect, Option<Act>) {
    let t = &s.t;
    let current = match of {
        ColorOf::Strip(sid) => app.project().strip(sid).and_then(|s| s.color),
        ColorOf::Group(g) => app.project().strip_group(g).map(|g| g.color),
    };
    let follow = matches!(of, ColorOf::Strip(_)) && current.is_some();
    let _ = v;
    let mut act = None;
    let cols = 4;
    let cell = 22.0;
    let rows = Color::PALETTE.len().div_ceil(cols);
    let extra = if follow { kmenu::ROW + 4.0 } else { 0.0 };
    let size = vec2(cols as f32 * cell + 12.0, rows as f32 * cell + 12.0 + extra);
    let area = egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let items: [Item<'_>; 0] = [];
            let _ = items;
            crate::kit::with_brush(ui.painter(), |b| {
                kmenu::paint(b, rect, s, &[], None);
            });
            for (i, c) in Color::PALETTE.iter().enumerate() {
                let r = Rect::from_min_size(
                    pos2(
                        rect.left() + 6.0 + (i % cols) as f32 * cell,
                        rect.top() + 6.0 + (i / cols) as f32 * cell,
                    ),
                    vec2(cell - 4.0, cell - 4.0),
                );
                let resp = ui.interact(r, id.with(i), Sense::click());
                let fill = safe_track(crate::theme::track_color(*c), t.dark, FillSize::Small);
                crate::kit::with_brush(ui.painter(), |b| {
                    b.rrect(r, s.r(3.0), Fill::Solid(fill));
                    if resp.hovered() || current == Some(*c) {
                        b.rrect_stroke(r.shrink(b.px()), s.r(3.0), 1.5, t.menu_text);
                    }
                });
                if resp.clicked() {
                    act = Some(Act::Color(of, Some(*c)));
                }
            }
            if follow {
                let r = Rect::from_min_size(
                    pos2(rect.left() + 4.0, rect.bottom() - 6.0 - kmenu::ROW),
                    vec2(rect.width() - 8.0, kmenu::ROW),
                );
                let resp = ui.interact(r, id.with("follow"), Sense::click());
                crate::kit::with_brush(ui.painter(), |b| {
                    if resp.hovered() {
                        b.rrect(r, s.s(2.0), Fill::Solid(t.menu_hover));
                    }
                    let ink = if resp.hovered() {
                        t.menu_text_hover
                    } else {
                        t.menu_text
                    };
                    let tt = Text::key("mixer.menu.color_track", Role::Body, ink)
                        .align(egui::Align2::LEFT_CENTER)
                        .policy(text::Policy::Elide(text::Elide::End));
                    let _ = text::paint(b, r.shrink2(vec2(8.0, 0.0)), &tt, s.tk());
                });
                if resp.clicked() {
                    act = Some(Act::Color(of, None));
                }
            }
        });
    (area.response.rect, act)
}

/// Show the open menu and its submenu, act on a choice, close on a click elsewhere or
/// Escape.
pub(super) fn show(v: &mut MixerView, app: &mut CantusApp, ui: &mut egui::Ui) {
    let Some((menu, at)) = v.popups.open else {
        return;
    };
    let ctx = ui.ctx().clone();
    let s = Style::current();
    let base = Id::new(("mixer-menu", v.popups.serial));
    let rows = entries(v, app, menu);
    let top = list(&ctx, base, at, &rows, &s);
    let mut act: Option<Act> = None;
    let mut keep_open = false;
    if let Some(i) = top.clicked.or(top.toggled) {
        match rows[i].then {
            Then::Act(a) => {
                act = Some(a);
                keep_open = top.toggled.is_some();
            }
            Then::Sub(sub) => {
                let r = top.row_of_hovered.unwrap_or(top.rect);
                v.popups.sub = Some((sub, pos2(top.rect.right() - 4.0, r.top() - 4.0), i));
            }
            Then::Nothing => {}
        }
    }
    // A submenu opens on hover, and stays while the pointer is on it or on its row.
    if let (Some(i), Some(r)) = (top.hovered, top.row_of_hovered) {
        if let Then::Sub(sub) = rows[i].then {
            if v.popups.sub.is_none_or(|(s0, _, _)| s0 != sub) {
                v.popups.sub = Some((sub, pos2(top.rect.right() - 4.0, r.top() - 4.0), i));
            }
        } else {
            v.popups.sub = None;
        }
    }
    let mut sub_rect = Rect::NOTHING;
    if let Some((sub, sat, _)) = v.popups.sub {
        let sid = base.with("sub");
        match sub {
            Sub::Colors(of) => {
                let (r, a) = swatches(v, app, &ctx, sid, sat, of, &s);
                sub_rect = r;
                act = act.or(a);
            }
            _ => {
                let mut sub_rows = Vec::new();
                sub_entries(v, app, sub, &mut sub_rows);
                let shown = list(&ctx, sid, sat, &sub_rows, &s);
                sub_rect = shown.rect;
                if let Some(i) = shown.clicked.or(shown.toggled) {
                    if let Then::Act(a) = sub_rows[i].then {
                        act = Some(a);
                        keep_open = shown.toggled.is_some();
                    }
                }
            }
        }
    }
    let fresh = std::mem::take(&mut v.popups.fresh);
    if let Some(a) = act {
        apply(v, app, a);
        if !keep_open {
            v.popups.close();
        }
        return;
    }
    let (pressed, escape, pointer) = ctx.input(|i| {
        (
            i.pointer.any_pressed(),
            i.key_pressed(egui::Key::Escape),
            i.pointer.interact_pos(),
        )
    });
    let inside = pointer.is_some_and(|p| top.rect.contains(p) || sub_rect.contains(p));
    if escape || (pressed && !inside && !fresh) {
        v.popups.close();
    }
}

/// Do what a row says.
fn apply(v: &mut MixerView, app: &mut CantusApp, act: Act) {
    let mut prefs = v.prefs.current;
    match act {
        Act::Rename(id) => {
            if let Some(s) = app.project().strip(id) {
                let name = s.name.clone();
                v.rename = Some((super::Renaming::Strip(id), name));
            }
        }
        Act::RenameGroup(g) => v.begin_group_rename(app, g),
        Act::Color(ColorOf::Strip(id), color) => {
            app.exec(Command::SetStripColor { id, color });
        }
        Act::Color(ColorOf::Group(id), color) => {
            if let Some(color) = color {
                app.exec(Command::SetStripGroupColor { id, color });
            }
        }
        Act::Solo(id) => app.toggle_strip_solo(id),
        Act::Mute(id) => app.toggle_strip_mute(id),
        Act::Mono(id) => {
            if let Some(w) = app.project().strip(id).map(|s| s.width) {
                app.exec(Command::SetStripWidth {
                    id,
                    width: if w <= 0.005 { 1.0 } else { 0.0 },
                });
            }
        }
        Act::RouteHere(id) => {
            let others: Vec<StripId> = app
                .strip_selection()
                .into_iter()
                .filter(|s| *s != id)
                .collect();
            app.route_strips_to(&others, Some(id));
        }
        Act::SendHere(id) => {
            let others: Vec<StripId> = app
                .strip_selection()
                .into_iter()
                .filter(|s| *s != id)
                .collect();
            app.send_strips_to(&others, id);
        }
        Act::Submix(_) => {
            let members = app.strip_selection();
            app.create_submix(&members);
        }
        Act::RouteTrack(track, id) => app.route_track(track, Some(id)),
        Act::Duck(strip, key) => {
            if let Some(effect) = app.duck_strip_from(strip, key) {
                app.open_effect_editor(strip, effect);
            }
        }
        Act::Dock(id, d) => {
            let targets = targets(app, id);
            app.set_strips_dock(&targets, d);
        }
        Act::GroupNew(id) => {
            let targets = targets(app, id);
            app.create_strip_group(&targets);
        }
        Act::GroupJoin(id, g) => {
            let targets = targets(app, id);
            app.add_strips_to_group(&targets, g);
        }
        Act::GroupLeave(id) => {
            let targets = targets(app, id);
            app.remove_strips_from_group(&targets);
        }
        Act::Move(id, d) => app.move_strip(id, d),
        Act::Remove(id) => app.remove_strip(id),
        Act::GroupSelect(g) => {
            let members: Vec<StripId> = app.project().strips_in_group(g).map(|s| s.id).collect();
            if let Some(first) = members.first().copied() {
                app.select_strip(first);
                app.selected_strips = members;
            }
        }
        Act::GroupDissolve(g) => app.dissolve_strip_group(g),
        Act::Output(id, output) => {
            if app.project().strip(id).map(|s| s.output) != Some(output) {
                app.exec(Command::SetStripOutput { id, output });
            }
        }
        Act::ToggleSend(id, to) => {
            if app
                .project()
                .strip(id)
                .and_then(|s| s.send_to(to))
                .is_some()
            {
                app.remove_strip_send(id, to);
            } else {
                app.set_strip_send(id, Send::new(to));
                app.end_gesture();
            }
        }
        Act::SendOnOff(id, to) => {
            if let Some(send) = app.project().strip(id).and_then(|s| s.send_to(to)).copied() {
                app.set_strip_send(
                    id,
                    Send {
                        enabled: !send.enabled,
                        ..send
                    },
                );
                app.end_gesture();
            }
        }
        Act::Width(mode) => {
            prefs.width = mode;
            v.prefs.set(prefs);
        }
        Act::Row(r) => {
            prefs.rows.toggle(r);
            v.prefs.set(prefs);
        }
        Act::Colourful(c) => {
            prefs.colourful = c;
            v.prefs.set(prefs);
        }
        Act::Automate(id, what) => {
            use cantus_core::AutoTarget as A;
            let target = match (id.is_master(), what) {
                (true, 0) => A::MasterGain,
                (true, 1) => A::MasterPan,
                (true, _) => A::MasterWidth,
                (false, 0) => A::StripGain(id),
                (false, 1) => A::StripPan(id),
                (false, _) => A::StripWidth(id),
            };
            app.automate(target);
        }
        Act::ResetRatio => {
            prefs.ratio = super::layout::DEFAULT_RATIO;
            v.prefs.set(prefs);
        }
        Act::Wave => app.run_action(Action::ToggleMeterWave),
        Act::PanLaw(law) => {
            if app.project().pan_law != law {
                app.exec(Command::SetPanLaw { law });
            }
        }
        Act::UnmuteAll => app.unmute_all_strips(),
        Act::SoloOff => app.clear_strip_solos(),
        Act::AddStrip => app.add_strip(),
        Act::AddEffect(id, kind) => app.add_effect(id, kind),
        Act::OpenEffect(id, fx) => app.open_effect_editor(id, fx),
        Act::Bypass(strip, effect, bypass) => {
            app.exec(Command::SetEffectBypass {
                strip,
                effect,
                bypass,
            });
        }
        Act::MoveEffect(strip, effect, index) => {
            app.exec(Command::MoveEffect {
                strip,
                effect,
                index,
            });
        }
        Act::RemoveEffect(id, fx) => app.remove_effect(id, fx),
        Act::MasterFx(enabled) => {
            app.exec(Command::SetMasterFxEnabled { enabled });
        }
        Act::FxOn(id, enabled) => {
            app.exec(Command::SetStripFxEnabled { id, enabled });
        }
        Act::Reveal(i) => {
            if let Some(take) = app.mix_recorder.takes.get(i) {
                reveal(&take.path);
            }
        }
        Act::StopTake => app.stop_capture(),
        Act::Input(id, input) => {
            if let Some(track) = rec_track(app, id) {
                let input = input.map(|(first, stereo)| {
                    if stereo {
                        cantus_core::TrackInput::stereo(first)
                    } else {
                        cantus_core::TrackInput::mono(first)
                    }
                });
                app.set_track_input(track, input);
            }
        }
        Act::Arm(id) => arm_strip(app, id),
        Act::Capture(id) => app.toggle_capture(cantus_engine::CaptureTarget::Strip(id)),
    }
}

/// The track a strip records: the first on it with an input, else the first on it.
pub(super) fn rec_track(app: &CantusApp, id: StripId) -> Option<TrackId> {
    let p = app.project();
    p.tracks_on_strip(id)
        .find(|t| t.input.is_some())
        .or_else(|| p.tracks_on_strip(id).next())
        .map(|t| t.id)
}

/// Whether any track on the strip has an input, and whether all of those are armed.
/// Allocates nothing (the desk asks every frame).
pub(super) fn rec_state(app: &CantusApp, id: StripId) -> (bool, bool) {
    let mut any = false;
    let mut all = true;
    for t in app
        .project()
        .tracks_on_strip(id)
        .filter(|t| t.input.is_some())
    {
        any = true;
        all &= t.armed;
    }
    (any, any && all)
}

/// Arm the strip's input tracks, or disarm them when all are armed: one undo step.
pub(super) fn arm_strip(app: &mut CantusApp, id: StripId) {
    let (any, all) = rec_state(app, id);
    if !any {
        return;
    }
    let commands: Vec<Command> = app
        .project()
        .tracks_on_strip(id)
        .filter(|t| t.input.is_some() && t.armed == all)
        .map(|t| Command::SetTrackArmed {
            id: t.id,
            armed: !all,
        })
        .collect();
    if !commands.is_empty() {
        app.exec(Command::Batch {
            label_key: "cmd.set_track_armed".into(),
            commands,
        });
    }
}

/// Show a file in the system's file browser.
fn reveal(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(dir) = path.parent() {
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}

/// A strip's (or the master's) automation rows: its volume, pan and width (ADR 0027).
fn automate_entries(id: StripId, e: &mut Vec<Entry>) {
    e.push(Entry::band("auto.menu.band"));
    e.push(Entry::act(
        Label::Key("auto.track.volume"),
        Act::Automate(id, 0),
    ));
    e.push(Entry::act(
        Label::Key("auto.track.pan"),
        Act::Automate(id, 1),
    ));
    e.push(Entry::act(
        Label::Key("auto.strip.width"),
        Act::Automate(id, 2),
    ));
}
