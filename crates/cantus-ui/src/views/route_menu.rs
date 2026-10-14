//! **The route menu** — one menu for a route wherever it is shown (round 25, ADR 0041;
//! design §3.3): a cable on the desk, a send knob or a send row, a matrix cell, an
//! inspector row. *Level ▸*, *Switch off/on*, *Post-fader | Pre-fader*, *Make this the
//! output* / *Make this a send* (with the doubled-signal note), *Open the keyed effect*,
//! *Show the destination / the source*, *Select strips routed here / from here*,
//! *Remove*. Also the add-send list (every *Send to ▸*, ending in *New return ▸ Empty ·
//! Reverb · Delay*, design §4.3), the route-here menu (design §4.2), and the two menus a
//! key gesture can end in (which insert to key; a ducking compressor or a gate when
//! nothing on the strip takes a key).
//!
//! Every action goes through `crate::routing_actions`. A view opens a menu under its own
//! `owner` id and calls [`show`] with the same id once a frame, so the desk, the matrix
//! and the inspector each draw only their own. The rows are built while the menu shows,
//! never before (ADR 0024 §13: nothing is worked out for a menu nobody opened).
//!
//! A view that cannot act on the application where it draws (the inspector's sheet
//! borrows it immutably) hands a pick to [`show`] with [`defer`]; it runs in the same
//! frame, when its owner's [`show`] runs.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::menu::{self as kmenu, Check, Item};
use crate::kit::text::{self, Role, Src, Text};
use crate::kit::Style;
use crate::routing_actions::{level_text, strip_name, ReturnKind};
use cantus_core::routing::{self, Route, RouteKind};
use cantus_core::{linear_to_db, Dock, EffectId, SendTap, StripId, TrackId};
use egui::{pos2, Context, Id, Pos2, Rect, Ui};

/// The owner id the inspector opens its route menus under (its sheet's Receives rows and
/// the empty Receives well).
pub fn inspector_owner() -> Id {
    Id::new("inspector.route_menu")
}

/// The id of a strip's *Route here* plate on the desk (tests read where it was drawn).
pub fn plate_id(strip: StripId) -> Id {
    Id::new(("mixer-route-here", strip.0))
}

/// Which menu is open.
#[derive(Clone, Debug, PartialEq)]
enum Kind {
    /// One route's menu.
    Route(Route),
    /// Where `from` (and the selection it is in) can send, ending in *New return*.
    AddSend(StripId),
    /// What can feed a strip nothing feeds yet.
    RouteHere(StripId),
    /// A key gesture onto a strip with several inserts that take a key: which one.
    Key {
        from: StripId,
        host: StripId,
        choices: Vec<EffectId>,
    },
    /// A key gesture onto a strip with nothing that takes a key: a ducking compressor or
    /// a gate keyed from the source.
    NothingToKey { from: StripId, host: StripId },
    /// *Type a level…*: the menu became a field for a send's level in dB (round 26).
    Level { from: StripId, to: StripId },
}

/// A submenu, opened from a row with a chevron (one level, as the desk's menus).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Sub {
    /// A send's level: the usual steps, the current one checked.
    Level(StripId, StripId),
    /// Every strip that could send into this one, by dock.
    SendStrip(StripId),
    /// Every track, the ones going straight to the master first.
    RouteTrack(StripId),
    /// *New return ▸ Empty · Reverb · Delay*, sends from `from` (and its selection).
    NewReturn(StripId),
}

/// What a row does — and what a view can [`defer`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Pick {
    /// A send's level (`from`, `to`, linear).
    Level(StripId, StripId, f32),
    /// Turn the menu into a field to type a send's level (`from`, `to`).
    TypeLevel(StripId, StripId),
    /// A send switched on or off.
    Switch(StripId, StripId),
    Tap(StripId, StripId, SendTap),
    /// *Make this the output* (`from`, `to`).
    MakeOutput(StripId, StripId),
    /// *Make this a send* (`from`).
    MakeSend(StripId),
    /// Open the keyed effect's editor (`host`, `effect`).
    OpenKeyed(StripId, EffectId),
    /// Bring a strip into view on the desk and flash its number, the selection kept.
    Show(StripId),
    /// Select every strip routed or sending into this one.
    SelectInto(StripId),
    /// Select every strip this one routes or sends into.
    SelectFrom(StripId),
    Remove(Route),
    /// Add a send from `from` (and the selection it is in) to `to`, or remove it when
    /// they all send there already.
    ToggleSend(StripId, StripId),
    NewReturn(StripId, ReturnKind),
    /// A track routed onto a strip.
    RouteTrack(TrackId, StripId),
    /// The selection (but the strip itself) routed onto it.
    RouteSelected(StripId),
    /// The selection (but the strip itself) sending into it.
    SendSelected(StripId),
    /// One strip sending into another (`from`, `to`).
    SendStrip(StripId, StripId),
    /// Key `effect` on `host` from `from`.
    Key(StripId, StripId, EffectId),
    /// A ducking compressor on `host` keyed from `from`.
    Duck(StripId, StripId),
    /// A gate on `host` keyed from `from`.
    Gate(StripId, StripId),
    /// Select a strip (a Receives row's name).
    Select(StripId),
}

/// The open menu of one owner, kept in the egui context between frames.
#[derive(Clone, Debug)]
struct Open {
    kind: Kind,
    at: Pos2,
    sub: Option<(Sub, Pos2)>,
    /// Opened this frame: the click that opened it is not a click outside it.
    fresh: bool,
    serial: u64,
    /// Where each row was drawn last frame, by its key or its words (tests click them).
    rows: Vec<(String, Rect)>,
}

fn state_id(owner: Id) -> Id {
    owner.with("route-menu")
}

fn defer_id(owner: Id) -> Id {
    owner.with("route-menu-defer")
}

fn put(ctx: &Context, owner: Id, kind: Kind, at: Pos2) {
    let serial = ctx
        .data(|d| d.get_temp::<Open>(state_id(owner)).map(|o| o.serial))
        .unwrap_or(0)
        .wrapping_add(1);
    let open = Open {
        kind,
        at,
        sub: None,
        fresh: true,
        serial,
        rows: Vec::new(),
    };
    ctx.data_mut(|d| d.insert_temp(state_id(owner), open));
}

/// Open the route menu for `route` at `at`.
pub fn open(ctx: &Context, owner: Id, route: Route, at: Pos2) {
    put(ctx, owner, Kind::Route(route), at);
}

/// Open the add-send list for `from` (the strip's sends, the selection's when `from` is
/// in it), ending in *New return ▸*.
pub fn open_add_send(ctx: &Context, owner: Id, from: StripId, at: Pos2) {
    put(ctx, owner, Kind::AddSend(from), at);
}

/// Open the route-here menu of `strip` (the plate, the inspector's empty Receives).
pub fn open_route_here(ctx: &Context, owner: Id, strip: StripId, at: Pos2) {
    put(ctx, owner, Kind::RouteHere(strip), at);
}

/// A key gesture onto `host` that found several inserts to key: the menu of them.
pub(crate) fn open_key_choice(
    ctx: &Context,
    owner: Id,
    from: StripId,
    host: StripId,
    choices: Vec<EffectId>,
    at: Pos2,
) {
    let kind = Kind::Key {
        from,
        host,
        choices,
    };
    put(ctx, owner, kind, at);
}

/// A key gesture onto `host` with nothing to key: the duck and gate offers.
pub(crate) fn open_nothing_to_key(
    ctx: &Context,
    owner: Id,
    from: StripId,
    host: StripId,
    at: Pos2,
) {
    put(ctx, owner, Kind::NothingToKey { from, host }, at);
}

/// Whether a menu `owner` opened is showing.
pub fn is_open(ctx: &Context, owner: Id) -> bool {
    ctx.data(|d| d.get_temp::<Open>(state_id(owner)).is_some())
}

/// Close what `owner` opened.
pub fn close(ctx: &Context, owner: Id) {
    ctx.data_mut(|d| d.remove::<Open>(state_id(owner)));
}

/// Where the row whose key (or words) is `label` was drawn in `owner`'s menu or its
/// submenu last frame — for tests and tools that click it.
pub fn row_rect(ctx: &Context, owner: Id, label: &str) -> Option<Rect> {
    ctx.data(|d| {
        d.get_temp::<Open>(state_id(owner))
            .and_then(|o| o.rows.iter().find(|(l, _)| l == label).map(|(_, r)| *r))
    })
}

/// Every row label `owner`'s menu drew last frame (a test's failure message).
pub fn row_labels(ctx: &Context, owner: Id) -> Vec<String> {
    ctx.data(|d| {
        d.get_temp::<Open>(state_id(owner))
            .map(|o| o.rows.iter().map(|(l, _)| l.clone()).collect())
            .unwrap_or_default()
    })
}

/// The text typed into the *Type a level…* field `owner`'s menu turned into, while it is
/// open (tests and tools read it).
pub fn typed_level(ctx: &Context, owner: Id) -> Option<String> {
    let open = ctx.data(|d| d.get_temp::<Open>(state_id(owner)))?;
    if !matches!(open.kind, Kind::Level { .. }) {
        return None;
    }
    Some(
        ctx.data(|d| d.get_temp::<Typing>(typing_id(owner)))
            .unwrap_or_default()
            .text,
    )
}

/// Open the *Type a level…* field for the send `from` → `to` at `at` (what *Level ▸ Type
/// a level…* does; tools and pictures call it directly): prefilled with the level now,
/// selected.
pub fn open_level(ctx: &Context, owner: Id, app: &CantusApp, from: StripId, to: StripId, at: Pos2) {
    let now = app
        .project()
        .strip(from)
        .and_then(|x| x.send_to(to))
        .map_or(0.0, |x| x.level);
    let text = if now <= 0.0 {
        "\u{2212}\u{221e}".to_string()
    } else {
        format!("{:.1}", linear_to_db(now)).replace('-', "\u{2212}")
    };
    let typing = Typing {
        prefill: text.clone(),
        text,
        ..Typing::default()
    };
    ctx.data_mut(|d| d.insert_temp(typing_id(owner), typing));
    put(ctx, owner, Kind::Level { from, to }, at);
}

/// The text being typed into a *Type a level…* field, kept by its owner.
#[derive(Clone, Debug, Default)]
struct Typing {
    text: String,
    /// The field has been given the keyboard (and its text selected) already.
    focused: bool,
    /// The last Enter found no level in the words: the caption says what to type.
    bad: bool,
    /// Where the panel was drawn last frame (tests click inside it).
    panel: Option<Rect>,
    /// What the field opened with: Enter on it unchanged sets nothing.
    prefill: String,
}

/// Where the *Type a level…* panel `owner` opened was drawn last frame, while it is open.
pub fn level_panel(ctx: &Context, owner: Id) -> Option<Rect> {
    typed_level(ctx, owner)?;
    ctx.data(|d| d.get_temp::<Typing>(typing_id(owner)))
        .and_then(|t| t.panel)
}

fn typing_id(owner: Id) -> Id {
    owner.with("route-menu-typing")
}

/// A typed level as a send's linear level: a number of decibels — "dB" after it or
/// nothing, a decimal comma or point, a plain or typographic minus — clamped to 0 dB (a
/// send carries at most the whole signal); −∞, or anything at or below the faders' floor
/// (`MIN_GAIN_DB`), is silence. Any other unit ("50%", "-6k", "-6 Hz") is no level: `None`.
pub(crate) fn parse_level(text: &str) -> Option<f32> {
    let lower = text
        .trim()
        .to_lowercase()
        .replace('\u{2212}', "-")
        .replace(',', ".");
    let bare = lower.strip_suffix("db").unwrap_or(&lower).trim();
    let infinite = |s: &str| matches!(s, "inf" | "\u{221e}" | "infinity" | "infinito");
    if let Some(rest) = bare.strip_prefix('-') {
        if infinite(rest.trim()) {
            return Some(0.0);
        }
    }
    if infinite(bare.trim_start_matches('+')) {
        return Some(1.0);
    }
    let db: f32 = bare.parse().ok().filter(|v: &f32| v.is_finite())?;
    Some(cantus_core::db_to_linear(db.min(0.0)))
}

/// Hand `pick` to `owner`'s next [`show`] (this frame's, when it has not run yet).
pub(crate) fn defer(ctx: &Context, owner: Id, pick: Pick) {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<Pick>>(defer_id(owner))
            .push(pick)
    });
}

/// Where the *Show* request lives that the desk answers (`mixer_drag` scrolls the strip
/// into view and flashes its number): the strip and the time it was asked.
pub(crate) fn reveal_id() -> Id {
    Id::new("mixer.reveal")
}

// ---- the rows -----------------------------------------------------------------------

/// A row's words.
enum Label {
    Key(&'static str),
    Own(String),
}

enum Then {
    Pick(Pick),
    Sub(Sub),
    Nothing,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum RowKind {
    Action,
    Band,
    Separator,
}

/// One row of a menu being built.
struct Row {
    kind: RowKind,
    label: Label,
    check: Check,
    /// The right-aligned column: a shortcut, or the current value (a send's level).
    value: Option<String>,
    enabled: bool,
    then: Then,
}

impl Row {
    fn pick(label: Label, pick: Pick) -> Row {
        Row {
            kind: RowKind::Action,
            label,
            check: Check::None,
            value: None,
            enabled: true,
            then: Then::Pick(pick),
        }
    }

    fn sub(label: Label, sub: Sub) -> Row {
        Row {
            then: Then::Sub(sub),
            ..Row::pick(label, Pick::Select(StripId::MASTER))
        }
    }

    fn band(label: Label) -> Row {
        Row {
            kind: RowKind::Band,
            then: Then::Nothing,
            ..Row::pick(label, Pick::Select(StripId::MASTER))
        }
    }

    /// A row that only says something (the doubled-signal note): never clickable.
    fn note(text: String) -> Row {
        Row {
            enabled: false,
            then: Then::Nothing,
            ..Row::pick(Label::Own(text), Pick::Select(StripId::MASTER))
        }
    }

    fn separator() -> Row {
        Row {
            kind: RowKind::Separator,
            then: Then::Nothing,
            ..Row::pick(Label::Key(""), Pick::Select(StripId::MASTER))
        }
    }

    fn check(mut self, c: Check) -> Row {
        self.check = c;
        self
    }

    fn enabled(mut self, on: bool) -> Row {
        self.enabled = on;
        self
    }

    fn value(mut self, v: String) -> Row {
        self.value = Some(v);
        self
    }

    /// What tests find the row by: its key, or its words.
    fn name(&self) -> &str {
        match &self.label {
            Label::Key(k) => k,
            Label::Own(s) => s,
        }
    }
}

fn on(b: bool) -> Check {
    if b {
        Check::On
    } else {
        Check::Off
    }
}

/// A strip as a menu row says it: "3 · Bass", or "3"; the master by its word.
pub(crate) fn strip_label(app: &CantusApp, id: StripId) -> String {
    let p = app.project();
    if id.is_master() {
        return t("mixer.master");
    }
    let n = p.strip_number(id).unwrap_or(0);
    let name = strip_name(p, id);
    if name.is_empty() || name == n.to_string() {
        n.to_string()
    } else {
        format!("{n} \u{b7} {name}")
    }
}

/// An insert as a row names it: "2 · Compressor".
fn effect_label(app: &CantusApp, host: StripId, effect: EffectId) -> String {
    let fx = app.project().effects_of(host);
    let found = fx.and_then(|v| v.iter().position(|e| e.id == effect).map(|i| (i, &v[i])));
    match found {
        Some((i, e)) => format!("{} \u{b7} {}", i + 1, t(e.spec.kind.name_key())),
        None => String::new(),
    }
}

/// The selection, the strip itself left out.
fn others(app: &CantusApp, strip: StripId) -> Vec<StripId> {
    let mut v = app.strip_selection();
    v.retain(|s| *s != strip);
    v
}

fn tap_key(tap: SendTap) -> &'static str {
    match tap {
        SendTap::Post => "routing.menu.post",
        SendTap::Pre => "routing.menu.pre",
    }
}

/// The level steps of a send's *Level ▸* (dB; `None` is silence).
const STEPS: [Option<f32>; 8] = [
    Some(0.0),
    Some(-3.0),
    Some(-6.0),
    Some(-10.0),
    Some(-12.0),
    Some(-18.0),
    Some(-24.0),
    None,
];

/// The rows of a menu, built now that it shows.
fn rows(app: &CantusApp, kind: &Kind) -> Vec<Row> {
    let p = app.project();
    let mut e = Vec::new();
    match kind {
        Kind::Route(r) => route_rows(app, *r, &mut e),
        // A field, not rows (`level_field`).
        Kind::Level { .. } => {}
        Kind::AddSend(from) => {
            let from = *from;
            let sources = app.gesture_sources(from);
            e.push(Row::band(Label::Own(if sources.len() > 1 {
                tf("routing.menu.send_many_to", &[("n", &sources.len())])
            } else {
                tf("routing.menu.send_to", &[("from", &strip_label(app, from))])
            })));
            // First, so it is in reach without scrolling on a 64-strip desk.
            e.push(Row::sub(
                Label::Key("routing.menu.new_return"),
                Sub::NewReturn(from),
            ));
            for dock in Dock::ALL {
                let mut band = false;
                for s in p.strips.iter().filter(|s| s.dock == dock) {
                    if sources.contains(&s.id) {
                        continue;
                    }
                    if !band {
                        e.push(Row::band(Label::Key(dock.label_key())));
                        band = true;
                    }
                    let all = sources
                        .iter()
                        .all(|x| p.strip(*x).and_then(|m| m.send_to(s.id)).is_some());
                    let can = sources
                        .iter()
                        .any(|x| p.route_check(*x, s.id, RouteKind::Send).is_ok());
                    let mut row = Row::pick(
                        Label::Own(strip_label(app, s.id)),
                        Pick::ToggleSend(from, s.id),
                    )
                    .check(on(all))
                    .enabled(all || can);
                    if sources.len() == 1 && routing::doubled(p, from, s.id) {
                        row = row.value(t("routing.menu.doubled_short"));
                    }
                    e.push(row);
                }
            }
        }
        Kind::RouteHere(strip) => here_rows(app, *strip, &mut e),
        Kind::Key {
            from,
            host,
            choices,
        } => {
            e.push(Row::band(Label::Own(tf(
                "routing.menu.key_which",
                &[("host", &strip_label(app, *host))],
            ))));
            for fx in choices {
                e.push(Row::pick(
                    Label::Own(effect_label(app, *host, *fx)),
                    Pick::Key(*from, *host, *fx),
                ));
            }
        }
        Kind::NothingToKey { from, host } => {
            e.push(Row::band(Label::Own(tf(
                "routing.menu.nothing_to_key",
                &[("host", &strip_label(app, *host))],
            ))));
            let src = strip_label(app, *from);
            e.push(Row::pick(
                Label::Own(tf("routing.menu.duck", &[("from", &src)])),
                Pick::Duck(*from, *host),
            ));
            e.push(Row::pick(
                Label::Own(tf("routing.menu.gate", &[("from", &src)])),
                Pick::Gate(*from, *host),
            ));
        }
    }
    e
}

/// One route's rows.
fn route_rows(app: &CantusApp, r: Route, e: &mut Vec<Row>) {
    let p = app.project();
    let (from, to) = (strip_label(app, r.from), strip_label(app, r.to));
    let names: [(&str, &dyn std::fmt::Display); 2] = [("from", &from), ("to", &to)];
    match r.kind {
        RouteKind::Send => {
            let Some(send) = p.strip(r.from).and_then(|s| s.send_to(r.to)).copied() else {
                e.push(Row::band(Label::Key("routing.menu.gone")));
                return;
            };
            e.push(Row::band(Label::Own(tf("routing.menu.send_title", &names))));
            e.push(
                Row::sub(Label::Key("routing.menu.level"), Sub::Level(r.from, r.to))
                    .value(level_text(send.level)),
            );
            e.push(
                Row::pick(
                    Label::Key("routing.menu.send_on"),
                    Pick::Switch(r.from, r.to),
                )
                .check(on(send.enabled)),
            );
            for tap in SendTap::ALL {
                e.push(
                    Row::pick(Label::Key(tap_key(tap)), Pick::Tap(r.from, r.to, tap))
                        .check(Check::Radio(send.tap == tap)),
                );
            }
            e.push(Row::separator());
            if let Some(note) = crate::routing_actions::doubled_text(p, r.from, r.to) {
                e.push(Row::note(note));
            }
            e.push(Row::pick(
                Label::Key("routing.menu.make_output"),
                Pick::MakeOutput(r.from, r.to),
            ));
        }
        RouteKind::Output => {
            e.push(Row::band(Label::Own(tf(
                "routing.menu.output_title",
                &names,
            ))));
            e.push(
                Row::pick(Label::Key("routing.menu.make_send"), Pick::MakeSend(r.from))
                    .enabled(!r.to.is_master()),
            );
        }
        RouteKind::Key(effect) => {
            let fx = effect
                .map(|x| effect_label(app, r.to, x))
                .unwrap_or_default();
            let names: [(&str, &dyn std::fmt::Display); 3] =
                [("from", &from), ("to", &to), ("effect", &fx)];
            e.push(Row::band(Label::Own(tf("routing.menu.key_title", &names))));
            if let Some(x) = effect {
                e.push(Row::pick(
                    Label::Key("routing.menu.open_keyed"),
                    Pick::OpenKeyed(r.to, x),
                ));
            }
        }
    }
    e.push(Row::separator());
    if !r.to.is_master() {
        e.push(Row::pick(
            Label::Key("routing.menu.show_dest"),
            Pick::Show(r.to),
        ));
    }
    e.push(Row::pick(
        Label::Key("routing.menu.show_source"),
        Pick::Show(r.from),
    ));
    e.push(Row::pick(
        Label::Key("routing.menu.select_into"),
        Pick::SelectInto(r.to),
    ));
    e.push(Row::pick(
        Label::Key("routing.menu.select_from"),
        Pick::SelectFrom(r.from),
    ));
    e.push(Row::separator());
    let removable = !(r.kind == RouteKind::Output && r.to.is_master());
    let key = if r.kind == RouteKind::Output {
        "routing.menu.remove_output"
    } else {
        "routing.menu.remove"
    };
    e.push(Row::pick(Label::Key(key), Pick::Remove(r)).enabled(removable));
}

/// The route-here rows, ordered by what the strip probably is (design §4.2): a channel
/// in the middle offers its tracks first, a return or bus in a pinned dock the sends.
fn here_rows(app: &CantusApp, strip: StripId, e: &mut Vec<Row>) {
    let p = app.project();
    let Some(dock) = p.strip(strip).map(|s| s.dock) else {
        return;
    };
    let sel = others(app, strip);
    let n = sel.len();
    let can = |kind| sel.iter().any(|s| p.route_check(*s, strip, kind).is_ok());
    e.push(Row::band(Label::Own(tf(
        "routing.menu.here_title",
        &[("strip", &strip_label(app, strip))],
    ))));
    let route_track = app
        .selected_track
        .and_then(|tr| p.track(tr))
        .filter(|tr| tr.strip != Some(strip))
        .map(|tr| {
            Row::pick(
                Label::Own(tf("routing.menu.route_track", &[("track", &tr.name)])),
                Pick::RouteTrack(tr.id, strip),
            )
        });
    let route_sel = Row::pick(
        Label::Own(tf("routing.menu.route_selected", &[("n", &n)])),
        Pick::RouteSelected(strip),
    )
    .enabled(n > 0 && can(RouteKind::Output));
    let a_track = Row::sub(
        Label::Key("routing.menu.route_a_track"),
        Sub::RouteTrack(strip),
    );
    let a_strip = Row::sub(
        Label::Key("routing.menu.send_a_strip"),
        Sub::SendStrip(strip),
    );
    if dock == Dock::Middle {
        e.extend(route_track);
        e.push(a_track);
        e.push(route_sel);
        e.push(a_strip);
    } else {
        if let Some(one) = app.selected_strip.filter(|s| *s != strip) {
            e.push(
                Row::pick(
                    Label::Own(tf(
                        "routing.menu.send_strip",
                        &[("strip", &strip_label(app, one))],
                    )),
                    Pick::SendStrip(one, strip),
                )
                .enabled(p.route_check(one, strip, RouteKind::Send).is_ok()),
            );
        }
        if n > 1 {
            e.push(
                Row::pick(
                    Label::Own(tf("routing.menu.send_selected", &[("n", &n)])),
                    Pick::SendSelected(strip),
                )
                .enabled(can(RouteKind::Send)),
            );
        }
        e.push(a_strip);
        e.push(Row::separator());
        e.push(route_sel);
        e.extend(route_track);
        e.push(a_track);
    }
}

/// The rows of a submenu.
fn sub_rows(app: &CantusApp, sub: Sub) -> Vec<Row> {
    let p = app.project();
    let mut e = Vec::new();
    match sub {
        Sub::Level(from, to) => {
            let now = p
                .strip(from)
                .and_then(|s| s.send_to(to))
                .map_or(0.0, |s| s.level);
            // First, a level of one's own: the menu becomes a field (round 26).
            e.push(Row::pick(
                Label::Key("routing.menu.level_type"),
                Pick::TypeLevel(from, to),
            ));
            e.push(Row::separator());
            for step in STEPS {
                let level = step.map_or(0.0, cantus_core::db_to_linear);
                let here = match step {
                    Some(db) => now > 0.0 && (linear_to_db(now) - db).abs() < 0.05,
                    None => now <= 0.0,
                };
                e.push(
                    Row::pick(Label::Own(level_text(level)), Pick::Level(from, to, level))
                        .check(Check::Radio(here)),
                );
            }
        }
        Sub::SendStrip(to) => {
            for dock in Dock::ALL {
                let mut band = false;
                for s in p.strips.iter().filter(|s| s.dock == dock && s.id != to) {
                    if !band {
                        e.push(Row::band(Label::Key(dock.label_key())));
                        band = true;
                    }
                    let sends = s.send_to(to).is_some();
                    let ok = p.route_check(s.id, to, RouteKind::Send);
                    e.push(
                        Row::pick(
                            Label::Own(strip_label(app, s.id)),
                            Pick::SendStrip(s.id, to),
                        )
                        .check(on(sends))
                        .enabled(ok.is_ok()),
                    );
                }
            }
        }
        Sub::RouteTrack(to) => {
            // The tracks going straight to the master first: those are the ones looking
            // for a strip.
            for first in [true, false] {
                for tr in p.tracks.iter().filter(|tr| tr.strip.is_none() == first) {
                    let mut row =
                        Row::pick(Label::Own(tr.name.clone()), Pick::RouteTrack(tr.id, to))
                            .check(Check::Radio(tr.strip == Some(to)));
                    if let Some(n) = tr.strip.and_then(|s| p.strip_number(s)) {
                        row = row.value(format!("\u{2192} {n}"));
                    }
                    e.push(row);
                }
            }
            if e.is_empty() {
                e.push(Row::note(t("routing.menu.no_tracks")));
            }
        }
        Sub::NewReturn(from) => {
            for (key, kind) in [
                ("routing.menu.return_empty", ReturnKind::Empty),
                ("routing.menu.return_reverb", ReturnKind::Reverb),
                ("routing.menu.return_delay", ReturnKind::Delay),
            ] {
                e.push(Row::pick(Label::Key(key), Pick::NewReturn(from, kind)));
            }
        }
    }
    e
}

// ---- drawing and acting -------------------------------------------------------------

/// What a list reported this frame.
struct Shown {
    rect: Rect,
    clicked: Option<usize>,
    hovered: Option<(usize, Rect)>,
}

/// How wide a list must be for its longest row.
fn width_of(ctx: &Context, items: &[Item<'_>], s: &Style) -> f32 {
    let mut w: f32 = 170.0;
    for it in items {
        let (label, value, submenu, band) = match it {
            Item::Action {
                label,
                shortcut,
                submenu,
                ..
            } => (*label, *shortcut, *submenu, false),
            Item::Band(label) => (*label, None, false, true),
            Item::Separator => continue,
        };
        // A band is set in capitals in the section role: measured as it is painted.
        let t = Text {
            src: label,
            caps: band,
            ..Text::new("", if band { Role::Section } else { Role::Body }, s.t.text)
        };
        let mut row = 22.0 + text::measure(ctx, &t, s.tk()).x + 20.0;
        if let Some(v) = value {
            row += text::measure(ctx, &Text::new(v, Role::Body, s.t.text), s.tk()).x + 12.0;
        }
        if submenu {
            row += 18.0;
        }
        w = w.max(row);
    }
    w.min(440.0)
}

/// Show `rows` as a kit menu at `at` (moved up to stay on screen, scrolling when taller
/// than the window), and note where each row went into `names`.
fn list(
    ctx: &Context,
    id: Id,
    at: Pos2,
    parent: Option<Rect>,
    rows: &[Row],
    s: &Style,
    names: &mut Vec<(String, Rect)>,
) -> Shown {
    let items: Vec<Item<'_>> = rows
        .iter()
        .map(|r| {
            let src = match &r.label {
                Label::Key(k) => Src::Key(k),
                Label::Own(s) => Src::Str(s),
            };
            match r.kind {
                RowKind::Band => Item::Band(src),
                RowKind::Separator => Item::Separator,
                RowKind::Action => Item::Action {
                    label: src,
                    check: r.check,
                    shortcut: r.value.as_deref(),
                    submenu: matches!(r.then, Then::Sub(_)),
                    enabled: r.enabled,
                },
            }
        })
        .collect();
    let width = width_of(ctx, &items, s);
    let screen = ctx.content_rect();
    let max_h = (screen.height() - 16.0).max(120.0);
    let h = kmenu::height(&items, s.d(1.0)).min(max_h);
    // A submenu that would run off the right goes to its parent's left, never over it.
    let x = match parent {
        Some(p) if at.x + width > screen.right() - 8.0 => p.left() + 4.0 - width,
        _ => at.x,
    };
    let at = pos2(
        x.min(screen.right() - 8.0 - width).max(screen.left() + 8.0),
        at.y.min(screen.bottom() - 8.0 - h).max(screen.top() + 8.0),
    );
    let mut out = Shown {
        rect: Rect::NOTHING,
        clicked: None,
        hovered: None,
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
                    let hover = resp.hover_pos();
                    for (i, row) in rows.iter().enumerate() {
                        let r = kmenu::row(resp.rect, &items, s.d(1.0), i);
                        if row.kind == RowKind::Action {
                            names.push((row.name().to_string(), r));
                        }
                        if hover.is_some_and(|p| r.contains(p)) && items[i].is_action() {
                            out.hovered = Some((i, r));
                        }
                    }
                });
        });
    out.rect = area.response.rect;
    out
}

/// Draw the menu `owner` opened, if any, and act on a pick; run the picks views handed
/// over with [`defer`].
pub fn show(app: &mut CantusApp, ui: &mut Ui, owner: Id) {
    let ctx = ui.ctx().clone();
    let deferred = ctx.data_mut(|d| d.remove_temp::<Vec<Pick>>(defer_id(owner)));
    for pick in deferred.into_iter().flatten() {
        apply(app, &ctx, pick);
    }
    let Some(mut open) = ctx.data(|d| d.get_temp::<Open>(state_id(owner))) else {
        return;
    };
    let s = Style::current();
    if let Kind::Level { from, to } = open.kind {
        level_field(app, &ctx, owner, open, from, to, &s);
        return;
    }
    let base = owner.with(("route-menu-list", open.serial));
    let top_rows = rows(app, &open.kind);
    let mut names = Vec::new();
    let top = list(&ctx, base, open.at, None, &top_rows, &s, &mut names);
    let mut pick: Option<Pick> = None;
    let sub_at = |i: usize, r: Rect| match top_rows[i].then {
        Then::Sub(sub) => Some((sub, pos2(top.rect.right() - 4.0, r.top() - 4.0))),
        _ => None,
    };
    if let Some(i) = top.clicked {
        match top_rows[i].then {
            Then::Pick(p) => pick = Some(p),
            Then::Sub(_) => {
                let r = top.hovered.map_or(top.rect, |(_, r)| r);
                open.sub = sub_at(i, r);
            }
            Then::Nothing => {}
        }
    }
    // A submenu opens on hover and stays while the pointer is on it or on its row.
    if let Some((i, r)) = top.hovered {
        match sub_at(i, r) {
            Some((sub, at)) => {
                if open.sub.is_none_or(|(s0, _)| s0 != sub) {
                    open.sub = Some((sub, at));
                }
            }
            None => open.sub = None,
        }
    }
    let mut sub_rect = Rect::NOTHING;
    if let Some((sub, at)) = open.sub {
        let sub_rows = sub_rows(app, sub);
        let shown = list(
            &ctx,
            base.with("sub"),
            at,
            Some(top.rect),
            &sub_rows,
            &s,
            &mut names,
        );
        sub_rect = shown.rect;
        if let Some(i) = shown.clicked {
            if let Then::Pick(p) = sub_rows[i].then {
                pick = Some(p);
            }
        }
    }
    if let Some(Pick::TypeLevel(from, to)) = pick {
        // The menu becomes the field, where the menu was.
        open_level(&ctx, owner, app, from, to, open.at);
        return;
    }
    if let Some(p) = pick {
        close(&ctx, owner);
        apply(app, &ctx, p);
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
    if escape || (pressed && !inside && !open.fresh) {
        close(&ctx, owner);
        return;
    }
    open.fresh = false;
    open.rows = names;
    ctx.data_mut(|d| d.insert_temp(state_id(owner), open));
}

/// *Type a level…*: the menu turned into a field where it was — the route's band, the
/// kit's numeric entry (a well, the value face) prefilled with the level and selected,
/// and what the keys do. Enter sets the level (one undo step) and closes; words that are
/// not a level keep the field, selected again, and the caption says what to type. Esc
/// cancels; a click outside sets what was typed (words that are not a level set nothing).
fn level_field(
    app: &mut CantusApp,
    ctx: &Context,
    owner: Id,
    mut open: Open,
    from: StripId,
    to: StripId,
    s: &Style,
) {
    use crate::kit::field::{Event, Field};
    use crate::kit::with_brush;
    let tid = typing_id(owner);
    let mut typing = ctx.data(|d| d.get_temp::<Typing>(tid)).unwrap_or_default();
    // Built only while the field shows.
    let (fl, tl) = (strip_label(app, from), strip_label(app, to));
    let title = tf("routing.menu.level_title", &[("from", &fl), ("to", &tl)]);
    let keys = t(if typing.bad {
        "routing.menu.level_bad"
    } else {
        "routing.menu.level_keys"
    });
    let k = s.d(1.0);
    let band = [Item::Band(Src::Str(&title))];
    let width = width_of(
        ctx,
        &[Item::Band(Src::Str(&title)), Item::action(Src::Str(&keys))],
        s,
    )
    .max(220.0 * k);
    let h = (kmenu::BAND + 30.0 + 20.0) * k + 8.0 * k;
    let screen = ctx.content_rect();
    let at = pos2(
        open.at
            .x
            .min(screen.right() - 8.0 - width)
            .max(screen.left() + 8.0),
        open.at
            .y
            .min(screen.bottom() - 8.0 - h)
            .max(screen.top() + 8.0),
    );
    let mut event = None;
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let area = egui::Area::new(owner.with(("route-menu-level", open.serial)))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            // The panel takes the clicks on it, so none reaches what lies under it.
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, h), egui::Sense::click());
            let head = kmenu::row(rect, &band, k, 0);
            let field = Rect::from_min_size(
                pos2(head.left() + 4.0 * k, head.bottom() + 4.0 * k),
                egui::vec2(head.width() - 8.0 * k, 22.0 * k),
            );
            let caption = Rect::from_min_max(
                pos2(head.left() + 8.0 * k, field.bottom() + 2.0 * k),
                pos2(head.right() - 8.0 * k, rect.bottom() - 4.0 * k),
            );
            with_brush(ui.painter(), |b| {
                kmenu::paint(b, rect, s, &band, None);
                let ink = if typing.bad {
                    s.t.danger
                } else {
                    crate::kit::color::with_alpha(s.t.menu_text, 0.72)
                };
                let _ = text::paint(
                    b,
                    caption,
                    &Text::new(&keys, Role::Label, ink)
                        .align(egui::Align2::LEFT_CENTER)
                        .policy(text::Policy::Elide(text::Elide::End)),
                    s.tk(),
                );
            });
            // The kit's field (round 26): the level selected as it opens (and again after
            // words that are not a level); Enter or a click elsewhere sets it, Esc leaves.
            let out = Field::new(tid.with("field"))
                .role(Role::Value)
                .open(!typing.focused)
                .show(ui, field, &mut typing.text);
            typing.focused = true;
            event = out.event;
        });
    let (pressed, pointer) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
    let inside = pointer.is_some_and(|p| area.response.rect.contains(p));
    typing.panel = Some(area.response.rect);
    let done = |ctx: &Context| {
        close(ctx, owner);
        ctx.data_mut(|d| d.remove::<Typing>(tid));
    };
    match event {
        Some(Event::Cancel) => {
            done(ctx);
            return;
        }
        Some(Event::Commit) => {
            if typing.text.trim() == typing.prefill {
                // The prefill untouched: nothing to set (its one decimal may not say the
                // level exactly, and a level under the floor reads as silence).
                done(ctx);
                return;
            }
            match parse_level(&typing.text) {
                Some(level) => {
                    done(ctx);
                    apply(app, ctx, Pick::Level(from, to, level));
                    return;
                }
                None if enter => {
                    // Selected again, and the caption says what a level looks like.
                    typing.bad = true;
                    typing.focused = false;
                }
                // Words that are not a level, left by a click elsewhere: nothing set.
                None => {
                    done(ctx);
                    return;
                }
            }
        }
        None if pressed && !inside && !open.fresh && !typing.focused => {
            done(ctx);
            return;
        }
        None => {}
    }
    open.fresh = false;
    open.rows.clear();
    ctx.data_mut(|d| {
        d.insert_temp(tid, typing);
        d.insert_temp(state_id(owner), open);
    });
}

/// Do what a row (or a deferred pick) says: one undo step at most.
fn apply(app: &mut CantusApp, ctx: &Context, pick: Pick) {
    let send_of = |app: &CantusApp, from: StripId, to: StripId| {
        app.project()
            .strip(from)
            .and_then(|s| s.send_to(to))
            .copied()
    };
    match pick {
        Pick::Level(from, to, level) => {
            if let Some(send) = send_of(app, from, to).filter(|s| s.level != level) {
                app.set_strip_send(from, cantus_core::Send { level, ..send });
                app.end_gesture();
            }
        }
        // Handled by `show`, which owns the menu it turns into a field.
        Pick::TypeLevel(..) => {}
        Pick::Switch(from, to) => {
            if let Some(send) = send_of(app, from, to) {
                let enabled = !send.enabled;
                app.set_strip_send(from, cantus_core::Send { enabled, ..send });
                app.end_gesture();
            }
        }
        Pick::Tap(from, to, tap) => {
            app.set_send_tap(from, to, tap);
        }
        Pick::MakeOutput(from, to) => {
            app.send_to_output(from, to);
        }
        Pick::MakeSend(from) => {
            app.output_to_send(from);
        }
        Pick::OpenKeyed(host, effect) => app.open_effect_editor(host, effect),
        Pick::Show(strip) => {
            // From the matrix or the inspector the desk may be behind another tab: bring
            // it to the front first, or the request would wait unseen.
            app.show_pane(crate::views::workspace::Pane::Mixer);
            let now = ctx.input(|i| i.time);
            ctx.data_mut(|d| d.insert_temp(reveal_id(), (strip, now)));
        }
        Pick::SelectInto(to) => {
            let p = app.project();
            let v: Vec<StripId> = p
                .strips
                .iter()
                .filter(|s| s.output == Some(to) || s.send_to(to).is_some())
                .map(|s| s.id)
                .collect();
            select_all(app, v);
        }
        Pick::SelectFrom(from) => {
            let v: Vec<StripId> = app
                .project()
                .strip(from)
                .map(|s| s.feeds().collect())
                .unwrap_or_default();
            select_all(app, v);
        }
        Pick::Remove(r) => {
            app.remove_route(r);
        }
        Pick::ToggleSend(from, to) => {
            let sources = app.gesture_sources(from);
            let p = app.project();
            let all = sources
                .iter()
                .all(|x| p.strip(*x).and_then(|m| m.send_to(to)).is_some());
            if all {
                let commands: Vec<cantus_core::Command> = sources
                    .iter()
                    .filter_map(|x| routing::plan::remove_route(p, Route::send(*x, to)))
                    .flat_map(|c| match c {
                        cantus_core::Command::Batch { commands, .. } => commands,
                        other => vec![other],
                    })
                    .collect();
                if !commands.is_empty() {
                    app.exec(cantus_core::Command::Batch {
                        label_key: "cmd.remove_route".into(),
                        commands,
                    });
                }
            } else {
                app.add_sends(&sources, to);
            }
        }
        Pick::NewReturn(from, kind) => {
            let sources = app.gesture_sources(from);
            app.new_return(&sources, kind);
        }
        Pick::RouteTrack(track, to) => {
            app.route_tracks_here(&[track], Some(to));
        }
        Pick::RouteSelected(to) => {
            let sel = others(app, to);
            app.replace_output(&sel, to);
        }
        Pick::SendSelected(to) => {
            let sel = others(app, to);
            app.add_sends(&sel, to);
        }
        Pick::SendStrip(from, to) => {
            app.add_sends(&[from], to);
        }
        Pick::Key(from, host, effect) => {
            app.key_sidechain(from, host, effect);
        }
        Pick::Duck(from, host) => {
            if let Some(e) = app.duck_strip_from(host, from) {
                app.open_effect_editor(host, e);
            }
        }
        Pick::Gate(from, host) => {
            if let Some(e) = app.gate_strip_from(host, from) {
                app.open_effect_editor(host, e);
            }
        }
        Pick::Select(strip) => app.select_strip(strip),
    }
}

/// Select `strips` (the first one leading), when there are any.
fn select_all(app: &mut CantusApp, strips: Vec<StripId>) {
    let strips: Vec<StripId> = strips.into_iter().filter(|s| !s.is_master()).collect();
    if let Some(first) = strips.first().copied() {
        app.select_strip(first);
        app.selected_strips = strips;
    }
}

#[cfg(test)]
mod tests {
    use super::parse_level;
    use cantus_core::linear_to_db;

    fn db(text: &str) -> Option<f32> {
        parse_level(text).map(|l| {
            if l <= 0.0 {
                f32::NEG_INFINITY
            } else {
                linear_to_db(l)
            }
        })
    }

    /// Round 26: what a musician types into *Type a level…* — with or without the unit, a
    /// decimal comma or point, a typographic or plain minus — reads as decibels; above
    /// 0 dB clamps to unity; −∞ and the floor are silence; anything else is no level.
    #[test]
    fn a_typed_level_reads_as_decibels_clamped_to_unity() {
        for (text, want) in [
            ("-6", -6.0),
            ("-6 dB", -6.0),
            ("\u{2212}4,5", -4.5),
            ("-4.5dB", -4.5),
            ("  -12,25  db ", -12.25),
            ("0", 0.0),
            ("+3", 0.0),
            ("12 dB", 0.0),
        ] {
            let got = db(text).unwrap_or_else(|| panic!("{text:?} is a level"));
            assert!((got - want).abs() < 0.01, "{text:?} read {got}");
        }
        for text in ["-inf", "\u{2212}\u{221e}", "-\u{221e} dB", "-120", "-96"] {
            assert_eq!(parse_level(text), Some(0.0), "{text:?} is silence");
        }
        assert_eq!(parse_level("inf"), Some(1.0), "+∞ clamps to unity");
        // Review finding 4: only decibels — no other unit the kit's parser knows.
        for text in [
            "",
            "loud",
            "-",
            "dB",
            "6 6",
            "50%",
            "-6k",
            "-6 Hz",
            "-6 db/oct",
            "-6 ms",
            "2 kHz",
            "-6 st",
            "4:1",
        ] {
            assert_eq!(parse_level(text), None, "{text:?} is no level");
        }
    }
}
