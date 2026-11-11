//! The top bar (ADR 0052, BACKLOG §6e; round 39: `dir-a`'s phase B sheets, approved
//! 2026-11-09): one row 44 high on the bar's surface, one lit top edge, every key a
//! recessed well 24 high on the centre line.
//!
//! The left cluster packs from the left edge: the mark with the menus over the project's
//! name; **Pat / Song**; the **transport** with loop; the **metronome**; the **tempo** and
//! the metre; the **position**; the focused editor's **grid and snap**; **undo**; the
//! **windows**. The right cluster packs from the right edge: **CPU / RAM / voices /
//! dropouts**, the **inputs** (MIDI, armed tracks), the **master meter**, the audio
//! **device**, the **tools** (picker, zoom), **Settings** and **More**.
//!
//! The owner shows, hides and reorders the groups (*Customise top bar…*, kept in
//! `Settings::topbar`). When the window narrows the groups go compact or fold into the
//! **More** panel in one fixed order (`layout::STEPS`), where they stay usable; a property
//! test sweeps widths × zooms × group sets × languages (`tests/topbar_layout.rs`).
//!
//! What a group says is measured once per language, zoom, theme and name (`WANTS`), so the
//! bar at rest measures nothing and allocates nothing of its own.

#[path = "transport_clock.rs"]
pub mod clock;
#[path = "transport_keys.rs"]
mod keys;
#[path = "transport_layout.rs"]
pub mod layout;
#[path = "transport_menus.rs"]
pub(crate) mod menus;
#[path = "transport_meter.rs"]
mod meter;
#[path = "transport_metro.rs"]
mod metro;
#[path = "transport_more.rs"]
mod more;
#[path = "transport_paint.rs"]
pub(crate) mod paint;
#[path = "transport_panel.rs"]
mod panel;
#[path = "transport_pattern.rs"]
pub mod pattern;
#[path = "transport_perf.rs"]
pub mod perf;
#[path = "transport_snap.rs"]
mod snap;
// Link, MIDI clock and time code: the bar's group and the service (round 33). r33:sync
#[path = "transport_sync.rs"]
pub mod sync;
#[path = "../sync_service.rs"]
pub mod sync_service;
#[path = "transport_tap.rs"]
pub mod tap;
#[path = "transport_tempo.rs"]
pub(crate) mod tempo;
#[path = "transport_title.rs"]
pub mod title;

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::mix;
use crate::kit::hint::{self, Caps};
use crate::kit::{with_brush, Fill, Style};
use egui::{Id, Pos2, Rect, Sense, Ui, Vec2};
use layout::{Group, Layout, Room, Want, COUNT};
use std::cell::RefCell;

pub use more::open_customise;

/// The bar's height at 100 % (`dir-a`: 44; keys 24 high at y 10).
pub const HEIGHT: f32 = paint::BAR;
/// The space between two groups, the margin at each end, the least space between the two
/// clusters (`dir-a`: 10, 12, 24).
const GAP: f32 = 10.0;
const EDGE: f32 = 12.0;
const SPLIT: f32 = 24.0;
/// The space between two cells of a group that are not joined (the sync group's).
const STEP: f32 = 2.0;
/// The Settings key's and the More key's widths.
const GEAR_W: f32 = 28.0;
const MORE_W: f32 = 30.0;

/// The style the bar is drawn with on a bar `width` points wide: the window's (round 39:
/// the interface's size follows the screen, ADR 0111 §7, so the bar no longer grows on its
/// own).
pub fn bar_style_at(_width: f32) -> Style {
    Style::current()
}

/// The style group `g` is measured and drawn with on a bar `width` points wide.
pub fn group_style_at(_g: Group, width: f32) -> Style {
    bar_style_at(width)
}

/// The top bar's own state, kept on the application (`CantusApp::topbar`).
#[derive(Debug, Default)]
pub struct TopBar {
    /// Tap tempo: the run of taps and when the clock it is timed by started.
    pub tap: tap::TapTempo,
    /// Where the tempo well was drawn last frame (published geometry: tests aim with it).
    pub tempo_well: Option<egui::Rect>,
    /// What a drag on the tempo moved under a tenth of a BPM so far.
    pub(crate) tempo_rest: f64,
    /// What the wheel turned under a whole notch so far (a touchpad's fractions).
    pub(crate) tempo_notches: f32,
    pub(crate) tap_origin: Option<std::time::Instant>,
    /// Pattern mode: what was compiled last, and where the song was.
    pub pattern: pattern::State,
    /// The RAM reading, taken about once a second, and the last half minute of readings.
    pub(crate) ram: perf::Ram,
    pub(crate) history: perf::History,
    /// The window title last sent, and the project name being typed.
    pub(crate) title: title::State,
    /// The customise panel: open, and the row being dragged.
    pub(crate) customise: more::Customise,
    /// The master meter's peak hold.
    pub(crate) hold: meter::Hold,
    /// The MIDI lamp: the events counted last, and when the count last moved.
    pub(crate) midi: (u64, Option<std::time::Instant>),
}

impl TopBar {
    /// Whether the MIDI-input lamp is lit: an event came in during the last 150 ms.
    pub fn midi_lamp(&mut self, received: u64) -> bool {
        let now = std::time::Instant::now();
        if received != self.midi.0 {
            self.midi = (received, Some(now));
        }
        self.midi
            .1
            .is_some_and(|t| now.duration_since(t) < std::time::Duration::from_millis(150))
    }
}

thread_local! {
    /// The groups' measured widths, and what they were measured for.
    static WANTS: RefCell<Option<(WantsKey, [Want; COUNT], usize)>> = const { RefCell::new(None) };
    /// The menus block's widths in its four forms, for the same key.
    static LEFT: RefCell<[f32; 4]> = const { RefCell::new([0.0; 4]) };
}

/// What the measured widths depend on.
#[derive(Copy, Clone, Debug, PartialEq)]
struct WantsKey {
    locale: crate::i18n::Locale,
    tk: u32,
    ppp: u32,
    groups: [Option<Group>; COUNT],
    chip: u64,
    /// The metre: the clock is as wide as its widest position in it.
    metre: cantus_core::TimeSignature,
    /// The fonts measured with (round 39): a theme's face changes every word's width, and
    /// its fonts arrive a pass after the switch.
    face: u64,
    /// The clock's bar digits and the focused editor's grid (the grid well is as wide as
    /// its value).
    bars: usize,
    grid: i64,
    /// The project's name, its unsaved mark and the facts its summary counts.
    name: u64,
}

/// The groups the owner shows, in their order.
pub fn shown_groups(app: &CantusApp) -> Vec<Group> {
    let mut out = [None; COUNT];
    let n = shown_into(app, &mut out);
    out[..n].iter().flatten().copied().collect()
}

fn shown_into(app: &CantusApp, out: &mut [Option<Group>; COUNT]) -> usize {
    let tb = &app.settings.topbar;
    // Resolved into a fixed array every frame: no allocation, whatever the owner's order.
    let mut order = [None; COUNT];
    let len = layout::resolve_into(&tb.order, &mut order);
    let mut n = 0;
    let in_use = app.settings.sync.source != sync_service::SyncSource::None; // r33:sync
    for g in order[..len].iter().flatten() {
        let in_use = if *g == Group::Ai {
            crate::ai_mcp::chip::in_use(app)
        } else {
            in_use
        }; // r34:mcp
        if g.is_shown(&tb.hidden, &tb.shown, in_use) {
            // r33:sync
            out[n] = Some(*g);
            n += 1;
        }
    }
    n
}

/// Each shown group's widths, compact and full, measured in the interface's language (the
/// property test reads them).
pub fn wants(app: &CantusApp, ctx: &egui::Context) -> Vec<Want> {
    let (w, n) = wants_with(app, ctx);
    w[..n].to_vec()
}

/// [`wants`] for a bar `width` points wide (the bar's widths no longer depend on it).
pub fn wants_at(app: &CantusApp, ctx: &egui::Context, _width: f32) -> Vec<Want> {
    wants(app, ctx)
}

/// A hash of what the menus block's widths depend on: the name, its unsaved mark, the
/// song's summary.
fn name_key(app: &CantusApp) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let p = app.project();
    p.name.hash(&mut h);
    app.bus.is_dirty().hash(&mut h);
    (p.tracks.len(), p.content_end().0).hash(&mut h);
    h.finish()
}

fn wants_with(app: &CantusApp, ctx: &egui::Context) -> ([Want; COUNT], usize) {
    let s = Style::current();
    let mut groups = [None; COUNT];
    let n = shown_into(app, &mut groups);
    let key = WantsKey {
        locale: crate::i18n::locale(),
        tk: s.tk().to_bits(),
        ppp: ctx.pixels_per_point().to_bits(),
        groups,
        chip: super::audio_chip::bar_key(app, ctx),
        metre: crate::views::bars::widest_metre(app.project()),
        face: crate::kit::cache::fonts_seen(ctx),
        name: name_key(app),
        bars: clock::bar_digits(app),
        grid: snap::grid_of(app, snap::focused(app)).0 .0,
    };
    WANTS.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((k, w, n)) = c.as_ref() {
            if *k == key {
                return (*w, *n);
            }
        }
        let mut w = [Want {
            group: Group::Transport,
            min: 0.0,
            max: 0.0,
            tall: true,
        }; COUNT];
        for (i, g) in groups[..n].iter().flatten().enumerate() {
            let (min, max) = widths_of(*g, app, ctx, &s);
            w[i] = Want {
                group: *g,
                min: min.ceil(),
                max: max.ceil(),
                tall: true,
            };
        }
        LEFT.with(|l| *l.borrow_mut() = menus::brand_widths(app, ctx, &s));
        *c = Some((key, w, n));
        (w, n)
    })
}

/// One group's widths, compact and full (the same where it has no compact form).
fn widths_of(g: Group, app: &CantusApp, ctx: &egui::Context, s: &Style) -> (f32, f32) {
    let fixed = |w: f32| (w, w);
    match g {
        Group::Mode => fixed(keys::mode_width(ctx, s)),
        Group::Transport => fixed(keys::transport_width(s)),
        Group::Tempo => tempo::widths(ctx, s),
        Group::Metronome => fixed(metro::width(s)),
        Group::Position => clock::widths(
            ctx,
            s,
            crate::views::bars::widest_metre(app.project()),
            clock::bar_digits(app),
        ),
        Group::Snap => snap::widths(app, ctx, s),
        Group::Undo => fixed(keys::undo_width(s)),
        Group::Windows => fixed(keys::windows_width(s)),
        Group::Performance => fixed(perf::width(ctx, s)),
        Group::Meter => meter::widths(ctx, s),
        Group::Inputs => fixed(keys::inputs_width(ctx, s)),
        Group::Device => super::audio_chip::bar_widths(app, ctx, s),
        Group::Tools => fixed(keys::tools_width(ctx, s)),
        Group::Sync => fixed(sync::width(ctx, s)), // r33:sync
        Group::Ai => fixed(crate::ai_mcp::chip::width(s)), // r34:mcp
    }
}

/// The room the bar gives its parts at `width` points (the property test reads it).
pub fn room(ctx: &egui::Context, width: f32) -> Room {
    let s = Style::current();
    let k = s.k;
    let mut b = LEFT.with(|l| *l.borrow());
    if b[0] <= 0.0 {
        // Not measured yet (a test asking first): the menus alone, no name.
        let menus = menus::left_widths(ctx, &s) + menus::LOGO_W * k;
        b = [menus, menus, 68.0 * k, 60.0 * k];
    }
    Room {
        width,
        pad: EDGE * k,
        gap: GAP * k,
        split: SPLIT * k,
        left_max: b[0],
        left_compact: b[1],
        left_minimal: b[2],
        left_min: b[3],
        more: MORE_W * k,
        gear: GEAR_W * k,
    }
}

/// The bar's layout this frame, and the rect it was laid in (tests and screenshots).
pub fn last_layout(ctx: &egui::Context) -> Option<(Rect, Layout)> {
    ctx.data(|d| d.get_temp::<(Rect, Layout)>(Id::new("cantus-topbar-layout")))
}

/// The toolbar, filling `ui`'s rect.
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    // The bar takes its height even in a panel that did not size it.
    let max = ui.max_rect();
    let rect = Rect::from_min_size(
        max.min,
        Vec2::new(
            max.width(),
            max.height().max((HEIGHT * s.t.density).round()),
        ),
    );
    let ground = ui.allocate_rect(rect, Sense::click());
    let layer = ui.layer_id();
    let first = ui.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    let ctx = ui.ctx().clone();
    // The things the bar keeps up to date whatever it shows: the pattern being played,
    // a run of taps that ended, the RAM reading and the history, the meter's held peaks.
    pattern::follow(app);
    tempo::tap_tick(app);
    perf::watch(app);
    meter::watch(app);

    let (wants, n) = wants_with(app, &ctx);
    let room = room(&ctx, rect.width());
    let l = layout::layout(&room, &wants[..n]);
    ctx.data_mut(|d| d.insert_temp(Id::new("cantus-topbar-layout"), (rect, l)));
    let at = |sp: layout::Span| {
        Rect::from_min_max(
            Pos2::new(rect.left() + sp.x0, rect.top()),
            Pos2::new(rect.left() + sp.x1, rect.bottom()),
        )
    };

    // The ground: the bar's surface, one lit edge along its top, the gutter's line under
    // it (`dir-a`: panels apart by value, never by borders).
    let painter = ui.painter_at(rect);
    with_brush(&painter, |b| {
        let r = b.snap_rect(rect);
        b.rect(r, Fill::Solid(s.t.toolbar));
        let lit = if s.t.dark {
            mix(s.t.toolbar, s.t.text, 0.07)
        } else {
            mix(s.t.toolbar, s.t.light, 0.6)
        };
        b.hline(r.left(), r.right(), r.top() + b.px() * 0.5, b.px(), lit);
        b.hline(
            r.left(),
            r.right(),
            r.bottom() - b.px() * 0.5,
            b.px(),
            s.t.ink,
        );
    });

    let name_line = menus::left_block(app, ui, at(l.left), l.brand, &s);
    for p in l.placed() {
        draw(p.group, app, ui, at(p.span), &s, p.compact);
    }
    gear(app, ui, paint::row(at(l.gear), &s), &s);
    more::key(app, ui, paint::row(at(l.more), &s), &l, &s);
    // A right-click on the bar's ground: customise it — and the help line says so where
    // nothing on it does (round 39, ADR 0109).
    let ground = super::hint::said_with(ground, || crate::i18n::t("ux.hint.topbar"));
    if ground.secondary_clicked() {
        more::open_customise(app);
    }
    // Words after shapes (one draw call fewer per group) — then the name's field, which
    // must cover the bar's words while the project is renamed.
    texts_last(&ctx, layer, first);
    if let Some(line) = name_line {
        title::name_field(app, ui, line, &s);
    }
    more::customise_panel(app, &ctx, rect);
}

/// Draw one group in `rect` (the bar's height on the bar; in the More panel, a row), full or
/// `compact`.
fn draw(g: Group, app: &mut CantusApp, ui: &mut Ui, rect: Rect, s: &Style, compact: bool) {
    match g {
        Group::Mode => keys::mode(app, ui, rect, s),
        Group::Transport => keys::transport(app, ui, rect, s),
        Group::Tempo => tempo::show(app, ui, rect, s, compact),
        Group::Metronome => metro::show(app, ui, rect, s),
        Group::Position => clock::show(app, ui, rect, s, compact),
        Group::Snap => snap::show(app, ui, rect, s, compact),
        Group::Undo => keys::undo(app, ui, rect, s),
        Group::Windows => keys::windows(app, ui, rect, s),
        Group::Performance => perf::show(app, ui, rect, s),
        Group::Meter => meter::show(app, ui, rect, s, compact),
        Group::Inputs => keys::inputs(app, ui, rect, s),
        Group::Device => super::audio_chip::show_bar(app, ui, rect, s, compact),
        Group::Tools => keys::tools(app, ui, rect, s),
        Group::Sync => sync::show(app, ui, rect, s), // r33:sync
        Group::Ai => crate::ai_mcp::chip::show(app, ui, rect, s), // r34:mcp
    }
}

/// The Settings key: the gear, opening Settings.
fn gear(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let resp = ui.interact(r, Id::new(("toolbar-key", "settings")), Sense::click());
    let open = app.show_settings;
    let st = crate::kit::State::of(&resp, false);
    with_brush(ui.painter(), |b| {
        paint::well(b, r, s, None);
        paint::tint(b, r, s, st, s.t.highlight, 0.2);
        let st = if open {
            crate::kit::State {
                hovered: true,
                ..st
            }
        } else {
            st
        };
        paint::icon(
            b,
            r,
            s,
            crate::kit::glyph::Glyph::Gear,
            st,
            s.t.highlight,
            24.0,
        );
    });
    let resp = strip_tip(ui, resp, || {
        tip(
            app,
            "topbar.settings",
            Some(crate::keymap::Action::OpenSettings),
        )
    });
    if resp.clicked() {
        app.show_settings = !open;
    }
}

/// Draw one group alone in `rect` (tests: every group keeps to the rect it is given).
pub fn draw_group(g: Group, app: &mut CantusApp, ui: &mut Ui, rect: Rect) {
    let s = Style::current();
    draw(g, app, ui, rect, &s, false);
}

/// A key's response with its tip, set while it is hovered (the tip is made only then).
fn strip_tip(ui: &Ui, resp: egui::Response, tip: impl FnOnce() -> String) -> egui::Response {
    if resp.hovered() {
        let tip = tip();
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        return resp.on_hover_text(tip);
    }
    resp
}

/// A key's tooltip: its name, and its shortcut when it has one.
fn tip(app: &CantusApp, key: &str, action: Option<crate::keymap::Action>) -> String {
    let name = t(key);
    match action.and_then(|a| app.keymap.bindings_for(a).first().copied()) {
        Some(b) => format!("{name} ({})", b.display()),
        None => name,
    }
}

/// A tap of tap tempo (its shortcut, `Action::TapTempo`).
pub fn tap(app: &mut CantusApp) {
    tempo::tap(app);
}

/// [`tap`] at a given time on the tap clock, and the frame's check that a run ended
/// (tests drive the clock).
pub fn tap_at(app: &mut CantusApp, now: f64) {
    tempo::tap_at(app, now);
}

pub fn tap_tick_at(app: &mut CantusApp, now: f64) {
    tempo::tap_tick_at(app, now);
}

/// Open the metronome's options (screenshots and tests).
pub fn open_metronome_options(ctx: &egui::Context) {
    metro::open_options(ctx);
}

/// Open the menu bar's menu `i` (the screenshots; 4 is Options).
pub fn open_menu(ctx: &egui::Context, i: usize) {
    menus::open_menu(ctx, i);
}

/// Open the count-in's choices (the screenshots): the metronome's options hold them.
pub fn open_count_in(ctx: &egui::Context) {
    metro::open_options(ctx);
}

pub fn open_overflow(app: &mut CantusApp) {
    more::open_overflow(app);
}

/// Whether the overflow panel is open (tests).
pub fn overflow_open(app: &CantusApp) -> bool {
    more::overflow_open(app)
}

/// The snap group's editor and grid (tests).
pub fn snap_editor(app: &CantusApp) -> (&'static str, cantus_core::Ticks, bool) {
    let e = snap::focused(app);
    let (g, on) = snap::grid_of(app, e);
    let name = match e {
        snap::Editor::Arrangement => "arrangement",
        snap::Editor::PianoRoll => "piano_roll",
        snap::Editor::Rack => "rack",
        snap::Editor::Pitch => "pitch",
        snap::Editor::Automation => "automation",
    };
    (name, g, on)
}

/// The groups drawn in the open More panel this frame, and where (tests).
pub fn last_overflow(ctx: &egui::Context) -> Vec<(Group, Rect)> {
    ctx.data(|d| d.get_temp::<Vec<(Group, Rect)>>(Id::new("cantus-topbar-overflow")))
        .unwrap_or_default()
}

/// Open the time signature's editor (screenshots and tests).
pub fn open_metre_editor(ctx: &egui::Context) {
    tempo::open_metre(ctx);
}

thread_local! {
    /// The bar's shapes while they are put in order (reused: no allocation once grown).
    static SHAPES: RefCell<Vec<egui::epaint::ClippedShape>> = const { RefCell::new(Vec::new()) };
}

/// Put the bar's words after its shapes: each group paints a well, then its words, then the
/// next group's well — and every switch between the kit's atlas and the font's is a draw
/// call. The bar's words never lie under one of its shapes (the groups never overlap), so
/// drawing them all last changes no pixel and saves a draw call per group.
fn texts_last(ctx: &egui::Context, layer: egui::LayerId, first: egui::layers::ShapeIdx) {
    SHAPES.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        ctx.graphics_mut(|g| {
            let list = g.entry(layer);
            let end = list.next_idx().0;
            for i in first.0..end {
                list.mutate_shape(egui::layers::ShapeIdx(i), |c| {
                    buf.push(std::mem::replace(
                        c,
                        egui::epaint::ClippedShape {
                            clip_rect: c.clip_rect,
                            shape: egui::Shape::Noop,
                        },
                    ));
                });
            }
            let is_text = |c: &egui::epaint::ClippedShape| matches!(c.shape, egui::Shape::Text(_));
            let mut i = first.0;
            for text in [false, true] {
                for c in buf.iter_mut().filter(|c| is_text(c) == text) {
                    let c = std::mem::replace(
                        c,
                        egui::epaint::ClippedShape {
                            clip_rect: c.clip_rect,
                            shape: egui::Shape::Noop,
                        },
                    );
                    list.set(egui::layers::ShapeIdx(i), c.clip_rect, c.shape);
                    i += 1;
                }
            }
        });
        buf.clear();
    });
}
