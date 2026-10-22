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
#[path = "transport_pattern.rs"]
pub mod pattern;
#[path = "transport_perf.rs"]
pub mod perf;
#[path = "transport_snap.rs"]
mod snap;
#[path = "transport_tap.rs"]
pub mod tap;
#[path = "transport_tempo.rs"]
pub(crate) mod tempo;
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
/// A key's side at 100 % in a group as tall as the bar.
const KEY: f32 = 28.0;
/// A key's side at 100 % in a group one row high (two rows to the bar, as FL's).
const ROW_KEY: f32 = 22.0;
/// The space between two columns of groups; a divider sits in its middle.
const GAP: f32 = 8.0;
/// The margin at each end of the bar.
const PAD: f32 = 6.0;
/// The space between two keys of a group.
const STEP: f32 = 2.0;
/// The hint panel's least width at 100 %.
const HINT_MIN: f32 = 300.0;
/// The audio chip's least on the bar at 100 % while it reads a running device (its line is
/// shortened in its middle, the whole of it in the chip's hint): what keeps every group on
/// the bar at 1920 px with a running interface's chip (review 2). A sentence (no audio,
/// lost) is read whole, up to [`DEVICE_SENTENCE`].
pub const DEVICE_MIN: f32 = 150.0;
/// The most a sentence on the chip takes before it is shortened.
const DEVICE_SENTENCE: f32 = 200.0;
/// The audio chip's widest on the bar at 100 %, when the bar has the room (it grows after
/// the meter).
pub const DEVICE_MAX: f32 = 320.0;

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
    /// The menus' width, for the same key.
    static LEFT: RefCell<f32> = const { RefCell::new(0.0) };
}

/// What the measured widths depend on.
#[derive(Copy, Clone, Debug, PartialEq)]
struct WantsKey {
    locale: crate::i18n::Locale,
    tk: u32,
    ppp: u32,
    groups: [Option<Group>; COUNT],
    chip: u32,
    /// The metre: the clock is as wide as its widest position in it.
    metre: cantus_core::TimeSignature,
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
    for g in order[..len].iter().flatten() {
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
        metre: crate::views::bars::widest_metre(app.project()),
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
            tall: false,
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
        tall: g.tall(),
    };
    match g {
        Group::Mode => fixed(keys::mode_width(ctx, s)),
        Group::Transport => fixed(keys::transport_width(s)),
        Group::Tempo => fixed(tempo::width(ctx, s)),
        Group::Metronome => fixed(metro::width(ctx, s)),
        Group::Position => fixed(clock::width(
            ctx,
            s,
            crate::views::bars::widest_metre(app.project()),
        )),
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
                tall: true,
            }
        }
        Group::Inputs => fixed(keys::inputs_width(ctx, s)),
        // The chip's own width, at least up to 150 pt (a sentence: 200) and at most 320
        // as room allows: a long device line is shortened in the chip (its whole text is
        // in its hint), never the bar's room for the other groups.
        Group::Device => {
            let chip = super::audio_chip::width(app, ctx);
            let least = if super::audio_chip::reads_a_device(app, ctx) {
                DEVICE_MIN
            } else {
                DEVICE_SENTENCE
            };
            Want {
                group: g,
                min: (chip.min(least * s.k) + 4.0 * s.k).ceil(),
                max: (chip.min(DEVICE_MAX * s.k) + 4.0 * s.k).ceil(),
                tall: false,
            }
        }
        Group::Tools => fixed(keys::tools_width(ctx, s)),
    }
}

/// The room the bar gives its parts at `width` points (the property test reads it).
pub fn room(ctx: &egui::Context, width: f32) -> Room {
    let s = Style::current();
    let k = s.k;
    let menus_w = LEFT.with(|l| {
        let l = *l.borrow();
        if l > 0.0 {
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
    // a run of taps that ended, the RAM reading, the meter's held peaks.
    pattern::follow(app);
    tempo::tap_tick(app);
    perf::watch(app);
    meter::watch(app);

    let (wants, n) = wants_cached(app, &ctx);
    let room = room(&ctx, rect.width());
    let l = layout::layout(&room, &wants[..n]);
    ctx.data_mut(|d| d.insert_temp(Id::new("cantus-topbar-layout"), (rect, l)));
    let at = |sp: layout::Span, slot: layout::Slot| {
        let (top, bottom) = slot_y(rect, slot, k);
        Rect::from_min_max(
            Pos2::new(rect.left() + sp.x0, top),
            Pos2::new(rect.left() + sp.x1, bottom),
        )
    };

    // The ground: the toolbar token lit from above, a hairline under it, a divider
    // between every two columns of groups.
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
        for x in l.columns() {
            divider(b, rect.left() + x - room.gap * 0.5, rect, k, &s);
        }
    });

    let full = |sp| at(sp, layout::Slot::Full);
    let hint = menus::left_block(app, ui, full(l.left), &s);
    for p in l.placed() {
        draw(p.group, app, ui, at(p.span, p.slot), &s);
    }
    more::key(app, ui, full(l.more), l.folded(), &s);
    // A right-click on the bar's ground: customise it.
    if ground.secondary_clicked() {
        more::open_customise(app);
    }
    // Words after shapes (one draw call fewer per group) — then the name's field, which
    // must cover the panel's words while the project is renamed.
    texts_last(&ctx, layer, first);
    if let Some(hint) = hint {
        title::name_field(app, ui, hint, &s);
    }
    more::customise_panel(app, &ctx, rect);
}

/// A slot's top and bottom in the bar `rect`: the whole height, or one of its two rows.
fn slot_y(rect: Rect, slot: layout::Slot, k: f32) -> (f32, f32) {
    let mid = rect.center().y;
    match slot {
        layout::Slot::Full => (rect.top(), rect.bottom()),
        layout::Slot::Top => (rect.top() + 2.0 * k, mid - 1.0 * k),
        layout::Slot::Bottom => (mid + 1.0 * k, rect.bottom() - 2.0 * k),
    }
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
                Vec2::new(
                    rect.width() - 4.0 * s.k,
                    s.d(22.0).min(rect.height() - 2.0 * s.k),
                ),
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

/// A key's square, left edge at `x`, centred in `r`: the bar's key in a group as tall as
/// the bar, a row's key in a group one row high.
fn key_rect(r: Rect, x: f32, s: &Style) -> Rect {
    let side = key_side(r, s);
    Rect::from_min_size(Pos2::new(x, r.center().y - side * 0.5), Vec2::splat(side))
}

/// A key's side in `r`: the bar's key, or a row's when `r` is one row high — never taller
/// than the row (review 2: in the compact density the rows are 20 pt, and 22-pt keys stuck
/// out of them and touched).
fn key_side(r: Rect, s: &Style) -> f32 {
    if r.height() < KEY * s.k + 4.0 * s.k {
        (ROW_KEY * s.k).min(r.height())
    } else {
        KEY * s.k
    }
}

#[cfg(test)]
mod row_tests {
    use super::*;

    /// At every density the two rows' keys stay inside their rows, and apart.
    #[test]
    fn row_keys_fit_their_rows_at_every_density() {
        for density in [0.85f32, 1.0, 1.15] {
            let mut s = Style::current();
            s.k = 1.0;
            s.t.density = density;
            let bar = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, (HEIGHT * density).round()));
            let mut keys = Vec::new();
            for slot in [layout::Slot::Top, layout::Slot::Bottom] {
                let (top, bottom) = slot_y(bar, slot, s.k);
                let row = Rect::from_min_max(Pos2::new(0.0, top), Pos2::new(400.0, bottom));
                let key = key_rect(row, 10.0, &s);
                assert!(row.contains_rect(key), "{density}: {key:?} out of {row:?}");
                keys.push(key);
            }
            assert!(
                keys[0].bottom() < keys[1].top(),
                "{density}: {keys:?} touch"
            );
        }
    }
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

/// Open the count-in chip's menu (the screenshots).
pub fn open_count_in(ctx: &egui::Context) {
    metro::open_count_in(ctx);
}

pub fn open_overflow(app: &mut CantusApp) {
    more::open_overflow(app);
}

/// Whether the overflow menu is open (tests).
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

/// A row of keys on one cap — FL's button strips: one raised face, a hairline between the
/// keys, a lit key in the highlight, a hovered one brightened. Cheaper than a cap per key
/// (review 1: the two-row bar shows every group at 1920 px within the window's triangle
/// budget) and FL's own look. `x` is the strip's left edge; each key is a key's side wide.
fn key_strip<const N: usize>(
    ui: &mut Ui,
    x: f32,
    r: Rect,
    s: &Style,
    keys: [(KeyFace<'_>, bool, &str); N],
) -> [Response; N] {
    use crate::kit::color::with_alpha;
    use crate::kit::light::{self, RaisedOpts};
    let k = s.k;
    let side = key_side(r, s);
    let top = r.center().y - side * 0.5;
    let cells: [Rect; N] = std::array::from_fn(|i| {
        Rect::from_min_size(Pos2::new(x + i as f32 * side, top), Vec2::splat(side))
    });
    let resps: [Response; N] = std::array::from_fn(|i| {
        ui.interact(
            cells[i],
            Id::new(("toolbar-key", keys[i].2)),
            Sense::click(),
        )
    });
    let strip = Rect::from_min_max(cells[0].min, cells[N - 1].max);
    with_brush(ui.painter(), |b| {
        b.register(strip, "toolbar_key", cap::KEY_TOKENS, &[]);
        let rad = s.s(s.t.radius);
        let base = if s.t.dark {
            shade(s.t.toolbar, -0.05)
        } else {
            shade(s.t.toolbar, 0.06)
        };
        light::raised(
            b,
            strip,
            rad,
            base,
            RaisedOpts {
                drop: true,
                k,
                alpha: 1.0,
                depth: s.t.depth,
                ..Default::default()
            },
        );
        for (i, c) in cells.iter().enumerate() {
            let st = State::of(&resps[i], keys[i].1);
            let face = b.snap_rect(*c).shrink(b.px());
            let ink = if st.lit {
                let stops = cap::highlight_stops(s);
                b.rrect(face, (rad - b.px()).max(0.0), Fill::Stops(&stops));
                s.t.highlight_glyph
            } else if st.hovered {
                let a = if st.active { 0.14 } else { 0.08 };
                b.rrect(
                    face,
                    (rad - b.px()).max(0.0),
                    Fill::Solid(with_alpha(s.t.text, a)),
                );
                s.t.text
            } else {
                s.t.text_dim
            };
            if i > 0 {
                b.vline(
                    b.snap_mid(c.left()),
                    c.top() + 3.0 * k,
                    c.bottom() - 3.0 * k,
                    b.px(),
                    shade(base, -0.22),
                );
            }
            match keys[i].0 {
                KeyFace::Glyph(g) => {
                    crate::kit::glyph::paint(b, c.shrink(5.5 * k), g, ink);
                }
                KeyFace::Text(t) => {
                    let _ = crate::kit::text::paint(
                        b,
                        c.shrink(2.0 * k),
                        &crate::kit::text::Text::new(t, crate::kit::text::Role::Micro, ink),
                        s.tk(),
                    );
                }
            }
        }
    });
    resps
}

/// A strip key's tip, set while it is hovered (the tip is made only then).
fn strip_tip(ui: &Ui, resp: Response, tip: impl FnOnce() -> String) -> Response {
    if resp.hovered() {
        let tip = tip();
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        return resp.on_hover_text(tip);
    }
    resp
}
