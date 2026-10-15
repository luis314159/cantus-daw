//! The top bar (ADR 0052, BACKLOG §6e; ADR 0024 §4, §9; `fl-studio.md` §3.1): FL's top
//! strip in the kit's look, one group per function, separated by the kit's dividers.
//!
//! From left to right by default: the logo and the menus over the project's name and the
//! hint panel; **PAT / SONG**; the **transport** (to start, play, stop, record and its
//! options, loop); the **tempo** (BPM, tap, the time signature); the **metronome** (its
//! switch, count-in, level and options: voice, accent, when); the **position** in bars and
//! in time; the focused editor's **snap**; **undo**; the **windows**; **CPU / RAM / voices /
//! dropouts**; the **master meter** (peaks or loudness); the **inputs** (typing keyboard,
//! MIDI, armed tracks); the audio **device**; the **tools** (picker, appearance, zoom).
//!
//! The owner shows, hides and reorders the groups (*Customise top bar…*, kept in
//! `Settings::topbar`). When the window is narrow the least important fold — whole — into
//! the **⋯** menu at the right end, where they stay usable; the menus fold into one key
//! before the transport would leave (`layout`). A property test sweeps widths × zooms ×
//! group sets × languages (`tests/topbar_layout.rs`).
//!
//! What a group says is measured once per language, zoom and theme (`WANTS`), so the bar
//! at rest measures nothing and allocates nothing of its own.

#[path = "transport_clock.rs"]
mod clock;
#[path = "transport_keys.rs"]
mod keys;
#[path = "transport_layout.rs"]
pub mod layout;
#[path = "transport_menus.rs"]
mod menus;
#[path = "transport_meter.rs"]
mod meter;
#[path = "transport_metro.rs"]
mod metro;
#[path = "transport_more.rs"]
mod more;
#[path = "transport_pattern.rs"]
pub mod pattern;
#[path = "transport_perf.rs"]
pub mod perf;
#[path = "transport_snap.rs"]
mod snap;
#[path = "transport_tap.rs"]
pub mod tap;
#[path = "transport_tempo.rs"]
mod tempo;
#[path = "transport_title.rs"]
pub mod title;

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::cap::{self, KeyFace};
use crate::kit::color::shade;
use crate::kit::hint::{self, Caps};
use crate::kit::{with_brush, Brush, Fill, State, Style};
use egui::{Id, Pos2, Rect, Response, Sense, Ui, Vec2};
use layout::{Group, Layout, Room, Want, COUNT};
use std::cell::RefCell;

pub use more::open_customise;

/// The toolbar's height at 100 %.
pub const HEIGHT: f32 = 54.0;
/// A toolbar key's side at 100 %.
const KEY: f32 = 28.0;
/// The space between two groups; a divider sits in its middle.
const GAP: f32 = 10.0;
/// The margin at each end of the bar.
const PAD: f32 = 8.0;
/// The space between two keys of a group.
const STEP: f32 = 2.0;
/// The hint panel's least width at 100 %.
const HINT_MIN: f32 = 300.0;

/// The top bar's own state, kept on the application (`CantusApp::topbar`).
#[derive(Debug, Default)]
pub struct TopBar {
    /// Tap tempo: the run of taps and when the clock it is timed by started.
    pub tap: tap::TapTempo,
    pub(crate) tap_origin: Option<std::time::Instant>,
    /// Pattern mode: what was compiled last, and where the song was.
    pub pattern: pattern::State,
    /// The RAM reading, taken about once a second.
    pub(crate) ram: perf::Ram,
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
    /// The menus' and the compact left block's widths, for the same key.
    static LEFT: RefCell<(f32, f32)> = const { RefCell::new((0.0, 0.0)) };
}

/// What the measured widths depend on.
#[derive(Copy, Clone, Debug, PartialEq)]
struct WantsKey {
    locale: crate::i18n::Locale,
    tk: u32,
    ppp: u32,
    groups: [Option<Group>; COUNT],
    chip: u32,
}

/// The groups the owner shows, in their order.
pub fn shown_groups(app: &CantusApp) -> Vec<Group> {
    let mut out = [None; COUNT];
    let n = shown_into(app, &mut out);
    out[..n].iter().flatten().copied().collect()
}

fn shown_into(app: &CantusApp, out: &mut [Option<Group>; COUNT]) -> usize {
    let tb = &app.settings.topbar;
    let mut n = 0;
    // The common case — the default order — without touching the heap.
    let order: std::borrow::Cow<'_, [Group]> = if tb.order.is_empty() {
        std::borrow::Cow::Borrowed(&Group::ALL[..])
    } else {
        std::borrow::Cow::Owned(layout::resolve(&tb.order))
    };
    for g in order.iter() {
        if !tb.hidden.iter().any(|h| h == g.id()) {
            out[n] = Some(*g);
            n += 1;
        }
    }
    n
}

/// Each shown group's width, least and most, measured in the interface's language (the
/// property test reads it).
pub fn wants(app: &CantusApp, ctx: &egui::Context) -> Vec<Want> {
    let (w, n) = wants_cached(app, ctx);
    w[..n].to_vec()
}

fn wants_cached(app: &CantusApp, ctx: &egui::Context) -> ([Want; COUNT], usize) {
    let s = Style::current();
    let mut groups = [None; COUNT];
    let n = shown_into(app, &mut groups);
    let key = WantsKey {
        locale: crate::i18n::locale(),
        tk: s.tk().to_bits(),
        ppp: ctx.pixels_per_point().to_bits(),
        groups,
        chip: super::audio_chip::width(app, ctx).to_bits(),
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
        }; COUNT];
        for (i, g) in groups[..n].iter().flatten().enumerate() {
            w[i] = want_of(*g, app, ctx, &s);
        }
        LEFT.with(|l| *l.borrow_mut() = menus::left_widths(ctx, &s));
        *c = Some((key, w, n));
        (w, n)
    })
}

/// One group's width.
fn want_of(g: Group, app: &CantusApp, ctx: &egui::Context, s: &Style) -> Want {
    let fixed = |w: f32| Want {
        group: g,
        min: w.ceil(),
        max: w.ceil(),
    };
    match g {
        Group::Mode => fixed(keys::mode_width(ctx, s)),
        Group::Transport => fixed(keys::transport_width(s)),
        Group::Tempo => fixed(tempo::width(ctx, s)),
        Group::Metronome => fixed(metro::width(ctx, s)),
        Group::Position => fixed(clock::width(ctx, s)),
        Group::Snap => fixed(snap::width(ctx, s)),
        Group::Undo => fixed(keys::undo_width(s)),
        Group::Windows => fixed(keys::windows_width(s)),
        Group::Performance => fixed(perf::width(ctx, s)),
        Group::Meter => {
            let (min, max) = meter::widths(ctx, s);
            Want {
                group: g,
                min: min.ceil(),
                max: max.ceil(),
            }
        }
        Group::Inputs => fixed(keys::inputs_width(ctx, s)),
        Group::Device => fixed(super::audio_chip::width(app, ctx) + 4.0 * s.k),
        Group::Tools => fixed(keys::tools_width(ctx, s)),
    }
}

/// The room the bar gives its parts at `width` points (the property test reads it).
pub fn room(ctx: &egui::Context, width: f32) -> Room {
    let s = Style::current();
    let k = s.k;
    let (menus_w, compact_w) = LEFT.with(|l| {
        let l = *l.borrow();
        if l.0 > 0.0 {
            l
        } else {
            menus::left_widths(ctx, &s)
        }
    });
    let logo = menus::LOGO_W * k;
    Room {
        width,
        pad: PAD * k,
        gap: GAP * k,
        // The hint panel keeps room for its two lines, as the bar before round 28 gave it at
        // 1280 × 720 (the final review: every hint whole there, at 100 % and 200 %).
        left_min: logo + menus_w.max(HINT_MIN * k),
        left_max: logo + menus_w.max(460.0 * k),
        left_compact: logo + compact_w + HINT_MIN * k,
        more: more::KEY_W * k,
    }
}

/// The bar's layout this frame, and the rect it was laid in (tests and screenshots).
pub fn last_layout(ctx: &egui::Context) -> Option<(Rect, Layout)> {
    ctx.data(|d| d.get_temp::<(Rect, Layout)>(Id::new("cantus-topbar-layout")))
}

/// The toolbar, filling `ui`'s rect.
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let t = s.t;
    let k = s.k;
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
    // a run of taps that ended, the RAM reading.
    pattern::follow(app);
    tempo::tap_tick(app);
    perf::watch(app);

    let (wants, n) = wants_cached(app, &ctx);
    let room = room(&ctx, rect.width());
    let l = layout::layout(&room, &wants[..n]);
    ctx.data_mut(|d| d.insert_temp(Id::new("cantus-topbar-layout"), (rect, l)));
    let at = |sp: layout::Span| {
        Rect::from_min_max(
            Pos2::new(rect.left() + sp.x0, rect.top()),
            Pos2::new(rect.left() + sp.x1, rect.bottom()),
        )
    };

    // The ground: the toolbar token lit from above, a hairline under it, a divider
    // between every two groups.
    let painter = ui.painter_at(rect);
    with_brush(&painter, |b| {
        let r = b.snap_rect(rect);
        b.rect(r, Fill::V(shade(t.toolbar, 0.03), shade(t.toolbar, -0.02)));
        crate::kit::light::sheen(b, r, &t);
        b.hline(
            r.left(),
            r.right(),
            r.bottom() - b.px() * 0.5,
            b.px(),
            shade(t.toolbar, if t.dark { -0.22 } else { -0.16 }),
        );
        for (_, sp) in l.placed() {
            divider(b, rect.left() + sp.x0 - room.gap * 0.5, rect, k, &s);
        }
    });

    menus::left_block(app, ui, at(l.left), l.left_full, &s);
    for (g, sp) in l.placed() {
        draw(*g, app, ui, at(*sp), &s);
    }
    more::key(app, ui, at(l.more), l.folded(), &s);
    // A right-click on the bar's ground: customise it.
    if ground.secondary_clicked() {
        more::open_customise(app);
    }
    texts_last(&ctx, layer, first);
    more::customise_panel(app, &ctx, rect);
}

/// Draw one group in `rect` (on the bar, or in the ⋯ menu).
fn draw(g: Group, app: &mut CantusApp, ui: &mut Ui, rect: Rect, s: &Style) {
    match g {
        Group::Mode => keys::mode(app, ui, rect, s),
        Group::Transport => keys::transport(app, ui, rect, s),
        Group::Tempo => tempo::show(app, ui, rect, s),
        Group::Metronome => metro::show(app, ui, rect, s),
        Group::Position => clock::show(app, ui, rect, s),
        Group::Snap => snap::show(app, ui, rect, s),
        Group::Undo => keys::undo(app, ui, rect, s),
        Group::Windows => keys::windows(app, ui, rect, s),
        Group::Performance => perf::show(app, ui, rect, s),
        Group::Meter => meter::show(app, ui, rect, s),
        Group::Inputs => keys::inputs(app, ui, rect, s),
        Group::Device => {
            let chip = Rect::from_center_size(
                rect.center(),
                Vec2::new(rect.width() - 4.0 * s.k, s.d(22.0)),
            );
            super::audio_chip::show(app, ui, chip);
        }
        Group::Tools => keys::tools(app, ui, rect, s),
    }
}

/// A 1-px divider between two groups.
fn divider(b: &mut Brush, x: f32, rect: Rect, k: f32, s: &Style) {
    b.vline(
        b.snap_mid(x),
        rect.top() + 10.0 * k,
        rect.bottom() - 10.0 * k,
        b.px(),
        shade(s.t.toolbar, if s.t.dark { -0.2 } else { -0.14 }),
    );
}

/// A toolbar key in `r`, reporting a click. The tip is only made when the key is hovered,
/// so a bar at rest allocates nothing.
fn key_in(
    ui: &mut Ui,
    r: Rect,
    s: &Style,
    face: KeyFace<'_>,
    lit: bool,
    id: &str,
    tip: impl FnOnce() -> String,
) -> Response {
    let mut resp = ui.interact(r, Id::new(("toolbar-key", id)), Sense::click());
    let st = State::of(&resp, lit);
    with_brush(ui.painter(), |b| cap::toolbar_key(b, r, s, st, face));
    if st.hovered {
        let tip = tip();
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        resp = resp.on_hover_text(tip);
    }
    resp
}

/// A key's square, left edge at `x`, centred in `r`.
fn key_rect(r: Rect, x: f32, s: &Style) -> Rect {
    Rect::from_min_size(
        Pos2::new(x, r.center().y - KEY * s.k * 0.5),
        Vec2::splat(KEY * s.k),
    )
}

/// A key's tooltip: its name, and its shortcut when it has one.
fn tip(app: &CantusApp, key: &str, action: Option<crate::keymap::Action>) -> String {
    let name = t(key);
    match action.and_then(|a| app.keymap.bindings_for(a).first().copied()) {
        Some(b) => format!("{name} ({})", b.display()),
        None => name,
    }
}

/// A tap of tap tempo (the TAP key and its shortcut, `Action::TapTempo`).
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

/// Open the metronome's options and the overflow menu (screenshots and tests).
pub fn open_metronome_options(ctx: &egui::Context) {
    metro::open_options(ctx);
}

pub fn open_overflow(ctx: &egui::Context) {
    more::open_overflow(ctx);
}

/// The snap group's editor and grid (tests).
pub fn snap_editor(app: &CantusApp) -> (&'static str, cantus_core::Ticks, bool) {
    let e = snap::focused(app);
    let (g, on) = snap::grid_of(app, e);
    let name = match e {
        snap::Editor::Arrangement => "arrangement",
        snap::Editor::PianoRoll => "piano_roll",
        snap::Editor::Rack => "rack",
    };
    (name, g, on)
}

/// Draw one group alone in `rect` (tests: every group keeps to the rect it is given).
pub fn draw_group(g: Group, app: &mut CantusApp, ui: &mut Ui, rect: Rect) {
    let s = Style::current();
    draw(g, app, ui, rect, &s);
}

/// The groups drawn in the open ⋯ menu this frame, and where (tests).
pub fn last_overflow(ctx: &egui::Context) -> Vec<(Group, Rect)> {
    ctx.data(|d| d.get_temp::<Vec<(Group, Rect)>>(Id::new("cantus-topbar-overflow")))
        .unwrap_or_default()
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

/// Open the time signature's editor (screenshots and tests).
pub fn open_metre_editor(ctx: &egui::Context) {
    tempo::open_metre(ctx);
}
