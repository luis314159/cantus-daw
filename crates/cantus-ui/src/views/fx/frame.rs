//! The device frame every effect editor is shown in (ADR 0024 §8): the kit's shell,
//! wired to the editor's host.
//!
//! - **The header is the title bar** — no egui title bar: ▸ the device menu, the name in
//!   the device hue with "(Insert 3 · Keys)" dimmed, the **preset well** (the factory
//!   presets and the user's, ‹ › to step, ▾ to browse and save; italic with a gold dot
//!   once the values drift from the preset), A|B, undo/redo filtered to this device, the
//!   bypass lamp, ⋯ and the size menu. Dragging the header's background moves the window.
//! - **The footer** is the help line of whatever the pointer is on (the kit's hints),
//!   with the editor's chips — latency, oversampling — and its output meter.
//! - **The scale** (75–200 %) is kept per kind; the grip resizes and snaps on release.
//!
//! [`place`] is the one function that puts an editor on screen: today a floating window
//! of its own; when the workspace's docks exist, a dock tab — one line changes there.
//! Everything here asks the editor's [`FxHost`], so the same frame runs inside the
//! exported plug-ins' windows (ADR 0020), where the DAW keeps the undo and the presets
//! are the factory's.

use super::host::FxHost;
use crate::i18n::{t, tf};
use crate::kit::menu::{self, Check, Item};
use crate::kit::shell::{self, Ab, Event, Footer, Header, Preset, Shell};
use crate::kit::text::Src;
use crate::kit::{with_brush, Function, Style};
use cantus_core::EffectKind;
use egui::{Pos2, Rect, Sense, Vec2};
use std::collections::{BTreeMap, BTreeSet};

pub use crate::kit::meter::Levels;
pub use crate::kit::shell::{DISPLAY_EFFECT, EFFECT};

/// A preset as the frame knows it: its name in the interface's language and the values
/// it sets (everything else goes back to its default) — or, for a hosted plug-in, the file
/// of the state it restores.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub params: BTreeMap<String, f32>,
    /// A factory preset (the user's own come after them).
    pub factory: bool,
    /// A saved state the host loads itself (a hosted plug-in's).
    pub file: Option<std::path::PathBuf>,
}

/// The preset last loaded: the well shows its name, italic with a dot once the values
/// drift from it.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    pub index: usize,
    pub name: String,
    pub params: BTreeMap<String, f32>,
}

/// Which of the frame's lists is open.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Popup {
    Device,
    Presets,
    More,
    Size,
}

/// What the frame remembers between frames, per open editor.
#[derive(Debug, Default)]
pub struct Chrome {
    /// The editor this belongs to (a new one starts fresh).
    pub owner: Option<u64>,
    pub loaded: Option<Loaded>,
    /// The factory presets and the user's, read when first needed and when the list
    /// opens (the disk is not read every frame).
    list: Option<Vec<Entry>>,
    /// The open list, where it opens, and the part of the header that opened it.
    popup: Option<(Popup, Pos2, Rect)>,
    /// A name being typed to save the current values as a preset.
    saving: Option<String>,
    favourites: BTreeSet<String>,
    /// Close the window: asked from a menu.
    close: bool,
    /// Where the name box for saving opens: under the header's left end.
    save_at: Pos2,
    /// The header's words, formatted once per change of the host or the language:
    /// (host revision, language, name, context).
    words: Option<(u64, crate::i18n::Locale, String, String)>,
    /// What the name and the context measure at 100 % (`header_name_w`) at the theme's
    /// text size, and that text size's bits: measured again when either changes.
    name_w: f32,
    name_w_at: u32,
    /// Whether the values were looked up among the factory presets when the editor opened
    /// (an insert's starting preset is named as loaded — ADR 0071).
    recognised: bool,
}

impl Chrome {
    /// Start over for another editor.
    pub fn reset_for(&mut self, owner: u64) {
        if self.owner != Some(owner) {
            let favourites = std::mem::take(&mut self.favourites);
            *self = Chrome {
                owner: Some(owner),
                favourites,
                ..Chrome::default()
            };
        }
    }
}

/// The device hue of a kind: the hue of its main function (ADR 0024 §8).
pub fn hue_of(kind: EffectKind) -> Function {
    match kind {
        EffectKind::Eq | EffectKind::DeEsser => Function::Tone,
        EffectKind::Compressor | EffectKind::Limiter | EffectKind::Gate | EffectKind::Multiband => {
            Function::Dynamics
        }
        EffectKind::Space | EffectKind::Delay | EffectKind::Reverb => Function::Space,
        EffectKind::Saturator => Function::Tone,
        EffectKind::Maximizer => Function::Dynamics,
        // Width is space (the kit's own words: delay, reverb, chorus, width).
        EffectKind::Stereo => Function::Space,
        EffectKind::External => Function::Output,
        // Round 28: the modulation units are modulation; the clipper and the transient
        // tools are dynamics; the analyser reads the output; the tuner works on the source.
        EffectKind::Chorus
        | EffectKind::Flanger
        | EffectKind::Phaser
        | EffectKind::Tremolo
        | EffectKind::Vibrato
        | EffectKind::RingMod
        | EffectKind::FreqShift
        | EffectKind::VolumeShaper => Function::Mod,
        EffectKind::Clipper | EffectKind::Transient | EffectKind::TransientSplit => {
            Function::Dynamics
        }
        EffectKind::Analyzer => Function::Output,
        EffectKind::AutoTune => Function::Source,
    }
}

/// The designed size of a kind's window at 100 %: display-first effects are wider.
pub fn designed_size(kind: EffectKind) -> Vec2 {
    match kind {
        // Aequalis: a row of four cards under its display (band, dynamics, analyser, output).
        EffectKind::Eq => Vec2::new(1180.0, 640.0),
        EffectKind::Multiband => DISPLAY_EFFECT,
        // Two displays and a table beside the macros: the display-first width.
        EffectKind::Space => DISPLAY_EFFECT,
        EffectKind::External => Vec2::new(820.0, 480.0),
        EffectKind::Compressor => Vec2::new(1120.0, 780.0),
        // Culmen: gain over time and the loudness, the bands and the curve, two rows of cells.
        EffectKind::Maximizer => Vec2::new(1180.0, 720.0),
        _ => Vec2::new(1000.0, 640.0),
    }
}

/// The buttons an effect's window shows in Cantus (round 20): swap, minimise, maximise
/// (or restore) and close.
pub fn window_ctl(maximised: bool) -> crate::kit::window_ctl::WindowCtl {
    crate::kit::window_ctl::WindowCtl {
        swap: Some("swap.tip.effect"),
        minimise: true,
        maximised: Some(maximised),
        close: true,
    }
}

/// The shell's id for a kind: its scale is kept per kind.
pub fn shell_of(kind: EffectKind) -> Shell {
    Shell::new(("cantus-fx", kind))
}

// ---- the window -------------------------------------------------------------------------

/// Where a floating editor of `size` goes: where it was (`pos`), kept reachable — never
/// above the top, never wholly off a side — or, the first time, centred over the upper
/// part of the window, as the old frame opened.
pub fn position(ctx: &egui::Context, pos: Option<Pos2>, size: Vec2) -> Pos2 {
    let screen = ctx.content_rect();
    let pos = pos.unwrap_or_else(|| {
        Pos2::new(
            (screen.center().x - size.x * 0.5).max(screen.left()),
            (screen.top() + 48.0).min((screen.bottom() - size.y).max(screen.top())),
        )
    });
    Pos2::new(
        pos.x.clamp(
            screen.left() - size.x + 120.0,
            (screen.right() - 120.0).max(screen.left()),
        ),
        pos.y
            .clamp(screen.top(), (screen.bottom() - 28.0).max(screen.top())),
    )
}

/// A floating editor at `pos`, sized `size` (the designed size × its scale), with no egui
/// title bar: the shell's header is the bar, and moves it. Called through
/// [`super::open_window`], the one function that decides where editors go.
pub fn place(
    ctx: &egui::Context,
    id: egui::Id,
    pos: Pos2,
    size: Vec2,
    add: impl FnOnce(&mut egui::Ui),
) {
    egui::Area::new(id)
        .order(egui::Order::Middle)
        .current_pos(pos)
        .movable(false)
        .constrain(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            // The window's own shadow and edge: it floats over the desk.
            let painter = ui.painter().clone();
            with_brush(&painter, |b| {
                let s = Style::current();
                let t = &s.t;
                b.rrect_soft(
                    rect.translate(Vec2::new(0.0, 6.0)),
                    8.0,
                    crate::kit::Fill::Solid(crate::kit::color::with_alpha(
                        t.dark_shade,
                        if t.dark { 0.45 } else { 0.18 },
                    )),
                    16.0,
                );
                b.register(rect, "device_window", &[crate::kit::Tok::Dark], &[]);
            });
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(rect);
            add(&mut child);
        });
}

// ---- popups ---------------------------------------------------------------------------

/// A list under `anchor`, drawn with the kit's menu at the device's scale. Returns the
/// row clicked; `close` is set when the pointer went down outside it — except on
/// `opener`, the control that opened it, whose own click closes it (so a second click
/// on a field does not close and reopen its list).
#[allow(clippy::too_many_arguments)]
pub fn popup_list(
    ctx: &egui::Context,
    id: egui::Id,
    anchor: Pos2,
    opener: Rect,
    s: &Style,
    items: &[Item<'_>],
    width: f32,
    close: &mut bool,
) -> Option<usize> {
    let mut clicked = None;
    let h = menu::height(items, s.d(1.0));
    let area = egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(anchor)
        .constrain(true)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, h), Sense::click());
            let hovered = resp.hover_pos().and_then(|p| {
                (0..items.len()).find(|&i| {
                    items[i].is_action() && menu::row(rect, items, s.d(1.0), i).contains(p)
                })
            });
            if resp.clicked() {
                clicked = hovered;
            }
            with_brush(ui.painter(), |b| menu::paint(b, rect, s, items, hovered));
        });
    let inside = area.response.rect;
    let pressed_outside = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| !inside.contains(p) && !opener.contains(p))
    });
    if pressed_outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *close = true;
    }
    clicked
}

/// A plain action row of a menu.
pub fn action(label: Src<'_>) -> Item<'_> {
    Item::action(label)
}

/// A row with a radio mark.
pub fn radio(label: Src<'_>, on: bool) -> Item<'_> {
    Item::Action {
        label,
        check: Check::Radio(on),
        shortcut: None,
        submenu: false,
        enabled: true,
    }
}

/// A row with a check mark.
pub fn check(label: Src<'_>, on: bool) -> Item<'_> {
    Item::Action {
        label,
        check: if on { Check::On } else { Check::Off },
        shortcut: None,
        submenu: false,
        enabled: true,
    }
}

/// A row that cannot be chosen now.
pub fn disabled(label: Src<'_>) -> Item<'_> {
    Item::Action {
        label,
        check: Check::None,
        shortcut: None,
        submenu: false,
        enabled: false,
    }
}

// ---- presets --------------------------------------------------------------------------

/// A kind's presets: the factory's, then the user's (where the host keeps a folder).
pub fn read_presets(kind: EffectKind, user: bool) -> Vec<Entry> {
    let mut out: Vec<Entry> = super::factory_presets(kind)
        .iter()
        .map(|(label, params)| Entry {
            name: t(label),
            params: params.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            factory: true,
            file: None,
        })
        .collect();
    if user {
        out.extend(
            super::user_presets(kind)
                .into_iter()
                .map(|(name, params)| Entry {
                    name,
                    params,
                    factory: false,
                    file: None,
                }),
        );
    }
    out
}

/// The factory preset of `list` that `spec` holds, key for key — what an editor opened on
/// an insert's starting preset names as loaded (ADR 0071).
pub fn recognise(spec: &cantus_core::EffectSpec, list: &[Entry]) -> Option<Loaded> {
    list.iter()
        .enumerate()
        .filter(|(_, e)| e.factory && !e.params.is_empty())
        .map(|(index, e)| Loaded {
            index,
            name: e.name.clone(),
            params: e.params.clone(),
        })
        .find(|l| !modified(spec, l))
}

/// Whether the effect's values have drifted from the preset that set them: a key it
/// names holds another value, or a key it leaves out is off its default.
pub fn modified(spec: &cantus_core::EffectSpec, loaded: &Loaded) -> bool {
    for info in spec.kind.params() {
        let now = spec.get(info.key);
        let want = loaded.params.get(info.key).copied().unwrap_or(info.default);
        if (now - want).abs() > 1e-6 * want.abs().max(1.0) {
            return true;
        }
    }
    false
}

impl Chrome {
    fn presets(&mut self, host: &dyn FxHost) -> &[Entry] {
        self.list.get_or_insert_with(|| host.presets())
    }

    /// Load preset `i` of the list: one step of the host's undo.
    fn load(&mut self, host: &mut dyn FxHost, i: usize) {
        let Some(e) = self.presets(host).get(i).cloned() else {
            return;
        };
        host.load_preset(&e);
        self.loaded = Some(Loaded {
            index: i,
            name: e.name,
            params: e.params,
        });
    }

    /// Step through the list: ‹ and ›, wrapping.
    fn step(&mut self, host: &mut dyn FxHost, by: isize) {
        let n = self.presets(host).len() as isize;
        if n == 0 {
            return;
        }
        let at = match &self.loaded {
            Some(l) => l.index as isize + by,
            None if by > 0 => 0,
            None => n - 1,
        };
        self.load(host, at.rem_euclid(n) as usize);
    }
}

// ---- the frame --------------------------------------------------------------------------

/// What an editor adds to its frame.
pub struct Look<'a> {
    pub kind: EffectKind,
    /// Pre-formatted chips: "Latency 2.0 ms", "Oversampling 2×" (see [`ChipText`]).
    pub chips: &'a [&'a str],
    /// The output meter at the footer's right.
    pub meter: Option<Levels>,
    /// The editor's own rows in the ⋯ menu, after the frame's.
    pub more: &'a [Item<'a>],
}

/// What happened in the frame this frame.
#[derive(Clone, Debug)]
pub struct Out {
    /// The scale the body was drawn at.
    pub k: f32,
    pub body: Rect,
    /// One of the editor's own ⋯ rows was chosen.
    pub more: Option<usize>,
    /// The header's background was dragged this much: move the window.
    pub moved: Vec2,
    /// Close the window (asked from a menu, or its close button).
    pub close: bool,
    /// Where the window's own buttons were drawn (round 20), when it has them.
    pub window: Option<crate::kit::window_ctl::WindowRects>,
}

impl Default for Out {
    fn default() -> Self {
        Out {
            k: 1.0,
            body: Rect::NOTHING,
            more: None,
            moved: Vec2::ZERO,
            close: false,
            window: None,
        }
    }
}

/// Show an editor in the frame: the header wired to `host`, `body` in the body's rect
/// (with the device's style: its scale and hue), and the footer.
pub fn show(
    ui: &mut egui::Ui,
    host: &mut dyn FxHost,
    chrome: &mut Chrome,
    look: &Look<'_>,
    body: impl FnOnce(&mut egui::Ui, &Style, Rect, &mut dyn FxHost),
) -> Out {
    let kind = look.kind;
    let shell = shell_of(kind);
    let ctx = ui.ctx().clone();
    // The words of the header, kept until the host or the language changes.
    let rev = host.revision();
    let lang = crate::i18n::locale();
    if chrome
        .words
        .as_ref()
        .is_none_or(|w| w.0 != rev || w.1 != lang)
    {
        let context = host.context();
        let context = if context.is_empty() {
            context
        } else {
            format!("({context})")
        };
        let name = host.name().unwrap_or_else(|| t(kind.name_key()));
        // The tray's chip says what the device is (round 20).
        if let Some(key) = host.device_key() {
            let title = if context.is_empty() {
                name.clone()
            } else {
                format!("{name} {context}")
            };
            crate::views::workspace_device::set_title(&ctx, key, &title);
        }
        chrome.words = Some((rev, lang, name, context));
        chrome.name_w_at = u32::MAX;
    }
    // The name's room, at the theme's text size: again when the words or the size change.
    let ts = Style::current().t.text_scale.to_bits();
    if chrome.name_w_at != ts {
        if let Some(w) = &chrome.words {
            chrome.name_w = header_name_w(&ctx, &w.2, &w.3);
            chrome.name_w_at = ts;
        }
    }
    // An editor opened on values a factory preset set (an insert's starting preset, ADR
    // 0071): the well names it, as if it had been loaded here.
    if !chrome.recognised {
        chrome.recognised = true;
        if chrome.loaded.is_none() && host.spec().external.is_none() {
            let list = chrome.presets(host).to_vec();
            chrome.loaded = recognise(host.spec(), &list);
        }
    }
    let spec_modified = chrome
        .loaded
        .as_ref()
        .filter(|l| !l.params.is_empty() || host.spec().external.is_none())
        .map(|l| modified(host.spec(), l));
    let none = tr("shell.preset.none");
    let (name, context) = chrome
        .words
        .as_ref()
        .map_or(("", ""), |w| (w.2.as_str(), w.3.as_str()));
    let preset = Preset {
        name: chrome.loaded.as_ref().map_or(none, |l| l.name.as_str()),
        modified: spec_modified.unwrap_or(false),
        favourite: chrome
            .loaded
            .as_ref()
            .is_some_and(|l| chrome.favourites.contains(&l.name)),
    };
    let ab = host.ab().map(|b| if b { Ab::B } else { Ab::A });
    let header = Header {
        name,
        context,
        hue: hue_of(kind),
        preset: Some(preset),
        ab,
        undo: host.history(),
        views: None,
        bypassed: host.bypassed(),
        help: false,
        name_w: chrome.name_w,
        window: host
            .device_key()
            .map(|key| crate::views::device_chrome::ctl(&ctx, key, host.swap_tip())),
    };
    let footer = Footer {
        chips: look.chips,
        meter: look.meter,
    };
    let mut k = 1.0;
    let mut body_rect = Rect::NOTHING;
    let out = shell.show(ui, &header, &footer, |ui, s, r| {
        k = s.k;
        body_rect = r;
        body(ui, s, r, host)
    });
    let s = Style::current().scaled(out.k).function(hue_of(kind));
    let rects = shell::header_rects(out.header, &s, &header);
    // The bypass lamp is the insert's bypass for MIDI learn (round 29, review 4 of ADR
    // 0062, #21).
    if let Some(br) = rects.bypass {
        host.learn_bypass_spot(ui, br);
    }
    chrome.save_at = out.header.left_bottom() + Vec2::new(24.0, 4.0) * s.k;
    let mut moved = Vec2::ZERO;
    // A part of the header opens its list, or closes it when it is the one open.
    let toggle = |chrome: &mut Chrome, which: Popup, at: Pos2, opener: Rect| {
        chrome.popup = match chrome.popup {
            Some((w, ..)) if w == which => None,
            _ => Some((which, at, opener)),
        };
    };
    for ev in &out.events {
        match *ev {
            Event::Menu => toggle(chrome, Popup::Device, rects.menu.left_bottom(), rects.menu),
            Event::PresetBrowse => {
                // Read the folder again: a preset saved elsewhere shows up.
                chrome.list = None;
                let well = rects.preset.unwrap_or(rects.name);
                toggle(chrome, Popup::Presets, well.left_bottom(), well);
            }
            Event::PresetPrev => chrome.step(host, -1),
            Event::PresetNext => chrome.step(host, 1),
            Event::Favourite => {
                if let Some(l) = &chrome.loaded {
                    if !chrome.favourites.remove(&l.name) {
                        chrome.favourites.insert(l.name.clone());
                    }
                }
            }
            Event::Ab(side) => {
                if host.ab().is_some_and(|on_b| on_b != (side == Ab::B)) {
                    host.toggle_ab();
                }
            }
            Event::Undo => host.undo(),
            Event::Redo => host.redo(),
            Event::Bypass => {
                if let Some(b) = host.bypassed() {
                    host.set_bypassed(!b);
                }
            }
            Event::More => {
                let at = Pos2::new(rects.more.right() - 230.0 * s.k, rects.more.bottom());
                toggle(chrome, Popup::More, at, rects.more);
            }
            Event::SizeMenu => {
                let at = Pos2::new(rects.size.right() - 140.0 * s.k, rects.size.bottom());
                toggle(chrome, Popup::Size, at, rects.size);
            }
            Event::Move(d) => moved += d,
            Event::Scale(_) | Event::Help | Event::View(_) => {}
            // The window's own buttons (round 20): the workspace minimises and maximises
            // the device, the host offers the picker, the editor closes itself.
            Event::Close => chrome.close = true,
            Event::Swap => host.swap(),
            Event::Minimise => {
                if let Some(key) = host.device_key() {
                    crate::views::workspace_device::minimise(&ctx, key);
                }
            }
            Event::Maximise => {
                if let Some(key) = host.device_key() {
                    crate::views::workspace_device::toggle_maximise(&ctx, key);
                }
            }
        }
    }
    let more = popups(&ctx, &shell, host, chrome, &s, look);
    saving(&ctx, &shell, host, chrome, &s);
    Out {
        k: out.k,
        body: out.body,
        more,
        moved,
        close: std::mem::take(&mut chrome.close),
        window: out.window,
    }
}

/// The width the header's name (in capitals) and its context take at 100 %: what the
/// preset well leaves room for before the name is cut (ADR 0073).
pub fn header_name_w(ctx: &egui::Context, name: &str, context: &str) -> f32 {
    use crate::kit::text::{measure, Role, Text};
    let t = Style::current().t;
    // At the theme's text size, as the header paints them (`Style::tk`, at 100 %).
    let ts = t.text_scale;
    let n = measure(ctx, &Text::new(name, Role::Section, t.text).caps(true), ts).x;
    if context.is_empty() {
        return n + 2.0;
    }
    let c = measure(ctx, &Text::new(context, Role::Title, t.text_dim), ts).x;
    n + 6.0 + c + 2.0
}

/// What a row of the frame's own menus does.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Act {
    CopyAb,
    Reset,
    Save,
    Close,
    Editor(usize),
    None,
}

/// The open list, if any: act on what was chosen. Returns the editor's own ⋯ row chosen.
fn popups(
    ctx: &egui::Context,
    shell: &Shell,
    host: &mut dyn FxHost,
    chrome: &mut Chrome,
    s: &Style,
    look: &Look<'_>,
) -> Option<usize> {
    let (which, at, opener) = chrome.popup?;
    let id = shell.id.with("popup");
    let mut close = false;
    let mut more = None;
    match which {
        Popup::Device | Popup::More => {
            let mut items: Vec<Item<'_>> = Vec::with_capacity(8 + look.more.len());
            let mut acts: Vec<Act> = Vec::with_capacity(8 + look.more.len());
            if which == Popup::Device {
                items.push(action(Src::Key("fx.frame.reset")));
                acts.push(Act::Reset);
                if host.user_presets() {
                    items.push(action(Src::Key("fx.frame.save")));
                    acts.push(Act::Save);
                }
            } else {
                if let Some(on_b) = host.ab() {
                    let key = if on_b {
                        "fx.frame.copy_b"
                    } else {
                        "fx.frame.copy_a"
                    };
                    items.push(action(Src::Key(key)));
                    acts.push(Act::CopyAb);
                }
                for (i, it) in look.more.iter().enumerate() {
                    items.push(*it);
                    acts.push(Act::Editor(i));
                }
            }
            items.push(Item::Separator);
            acts.push(Act::None);
            items.push(action(Src::Key("fx.frame.close")));
            acts.push(Act::Close);
            let w = 230.0 * s.k;
            if let Some(i) = popup_list(ctx, id, at, opener, s, &items, w, &mut close) {
                close = true;
                match acts.get(i).copied().unwrap_or(Act::None) {
                    Act::CopyAb => host.copy_ab(),
                    Act::Reset => {
                        host.load(&BTreeMap::new(), "cmd.fx_preset");
                        chrome.loaded = None;
                    }
                    Act::Save => chrome.saving = Some(String::new()),
                    Act::Close => chrome.close = true,
                    Act::Editor(n) => more = Some(n),
                    Act::None => {}
                }
            }
        }
        Popup::Presets => {
            let user = host.user_presets();
            let loaded = chrome.loaded.as_ref().map(|l| l.index);
            let list = chrome.list.get_or_insert_with(|| host.presets());
            let mut items: Vec<Item<'_>> = Vec::with_capacity(list.len() + 5);
            let mut rows: Vec<Option<usize>> = Vec::with_capacity(list.len() + 5);
            let mut band = None;
            for (i, e) in list.iter().enumerate() {
                if band != Some(e.factory) {
                    band = Some(e.factory);
                    items.push(Item::Band(Src::Key(if e.factory {
                        "preset.factory"
                    } else {
                        "preset.user"
                    })));
                    rows.push(None);
                }
                items.push(radio(Src::Str(&e.name), loaded == Some(i)));
                rows.push(Some(i));
            }
            if user {
                items.push(Item::Separator);
                rows.push(None);
                items.push(action(Src::Key("fx.frame.save")));
                rows.push(Some(usize::MAX));
            }
            let w = (shell::PRESET_W * s.k).max(200.0);
            let chosen = popup_list(ctx, id, at, opener, s, &items, w, &mut close);
            drop(items);
            if let Some(i) = chosen {
                close = true;
                match rows.get(i).copied().flatten() {
                    Some(usize::MAX) => chrome.saving = Some(String::new()),
                    Some(n) => chrome.load(host, n),
                    None => {}
                }
            }
        }
        Popup::Size => {
            const LABELS: [&str; 7] = ["75 %", "90 %", "100 %", "125 %", "150 %", "175 %", "200 %"];
            let now = shell.scale(ctx);
            let items: [Item<'_>; 7] = std::array::from_fn(|i| {
                radio(Src::Str(LABELS[i]), (shell::SCALES[i] - now).abs() < 1e-3)
            });
            if let Some(i) = popup_list(ctx, id, at, opener, s, &items, 140.0 * s.k, &mut close) {
                close = true;
                shell.set_scale(ctx, shell::SCALES[i]);
            }
        }
    }
    if close {
        chrome.popup = None;
    }
    more
}

/// The name box for saving the current values as a preset: Enter saves, Esc leaves.
fn saving(
    ctx: &egui::Context,
    shell: &Shell,
    host: &mut dyn FxHost,
    chrome: &mut Chrome,
    s: &Style,
) {
    let Some(name) = chrome.saving.as_mut() else {
        return;
    };
    let at = chrome.save_at;
    let mut done = false;
    let mut save = false;
    egui::Area::new(shell.id.with("save"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(300.0, 34.0) * s.k, Sense::hover());
            with_brush(ui.painter(), |b| {
                b.rrect(
                    b.snap_rect(rect),
                    s.r(6.0),
                    crate::kit::Fill::Solid(s.t.deck),
                );
            });
            // The kit's field (round 26). A preset is a file, so only Enter on a name
            // saves it; a click elsewhere, Esc or Enter on nothing closes the box
            // (final review: it never stays open without the keyboard).
            let hint = t("preset.name");
            let out = crate::kit::field::Field::new(shell.id.with("save-name"))
                .hint(&hint)
                .scale(s.k)
                .opens_when_shown()
                .show(ui, rect.shrink(5.0 * s.k), name);
            match name_box(out.event, out.enter, name) {
                NameBox::Save => save = true,
                NameBox::Close => done = true,
                NameBox::Keep => {}
            }
        });
    if save {
        let params = host.spec().params.clone();
        let saved = name.trim().to_string();
        match host.save_preset(&saved) {
            Ok(()) => {
                host.status(tf("preset.saved", &[("name", &saved)]));
                chrome.list = None;
                // The saved preset is the one loaded now.
                let list = chrome.presets(host);
                if let Some(i) = list.iter().position(|e| !e.factory && e.name == saved) {
                    chrome.loaded = Some(Loaded {
                        index: i,
                        name: saved,
                        params,
                    });
                }
            }
            Err(e) => host.status(e),
        }
        // Saved or not (the status line says why), the session is over: the box closes.
        done = true;
    }
    if done {
        chrome.saving = None;
    }
}

/// What the preset name box does when its field's session ends.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum NameBox {
    Keep,
    Save,
    Close,
}

fn name_box(event: Option<crate::kit::field::Event>, enter: bool, name: &str) -> NameBox {
    use crate::kit::field::Event as F;
    match event {
        None => NameBox::Keep,
        Some(F::Commit) if enter && !name.trim().is_empty() => NameBox::Save,
        Some(_) => NameBox::Close,
    }
}

// ---- words --------------------------------------------------------------------------------

type Words = std::collections::HashMap<(crate::i18n::Locale, &'static str), &'static str>;

thread_local! {
    static WORDS: std::cell::RefCell<Words> = std::cell::RefCell::new(Words::new());
}

/// A translated string that lives for the program, so a frame that names things does not
/// allocate: each (language, key) is looked up once and kept.
pub fn tr(key: &'static str) -> &'static str {
    let lang = crate::i18n::locale();
    WORDS.with(|w| {
        *w.borrow_mut()
            .entry((lang, key))
            .or_insert_with(|| Box::leak(t(key).into_boxed_str()))
    })
}

/// Footer chips, formatted only when what they say changes.
#[derive(Debug, Default)]
pub struct ChipText {
    latency: (Option<(u32, u32, crate::i18n::Locale)>, String),
    oversampling: (Option<(u32, crate::i18n::Locale)>, String),
}

impl ChipText {
    /// "Latency 2.0 ms", or "No latency".
    pub fn latency(&mut self, samples: f32, sample_rate: f32) -> &str {
        let lang = crate::i18n::locale();
        let key = (
            samples.max(0.0).round() as u32,
            sample_rate.round() as u32,
            lang,
        );
        if self.latency.0 != Some(key) {
            self.latency.0 = Some(key);
            self.latency.1 = if key.0 == 0 {
                t("fx.frame.no_latency")
            } else {
                let ms = key.0 as f32 / sample_rate.max(1.0) * 1000.0;
                tf("shell.chip.latency", &[("ms", &format!("{ms:.1}"))])
            };
        }
        &self.latency.1
    }

    /// "Oversampling 2×", or "No oversampling" at 1×.
    pub fn oversampling(&mut self, factor: u32) -> &str {
        let key = (factor, crate::i18n::locale());
        if self.oversampling.0 != Some(key) {
            self.oversampling.0 = Some(key);
            self.oversampling.1 = if factor <= 1 {
                t("fx.frame.no_oversampling")
            } else {
                tf("shell.chip.oversampling", &[("n", &factor)])
            };
        }
        &self.oversampling.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, Event as E, Key, Modifiers, PointerButton};

    /// Final review, fields 4: only Enter on a name saves a preset (a file); a click
    /// elsewhere with a name typed closes the box without writing it.
    #[test]
    fn only_enter_on_a_name_saves_a_preset() {
        use crate::kit::field::Event as F;
        assert_eq!(name_box(Some(F::Commit), true, "Warm"), NameBox::Save);
        assert_eq!(name_box(Some(F::Commit), false, "Warm"), NameBox::Close);
        assert_eq!(name_box(Some(F::Commit), true, "  "), NameBox::Close);
        assert_eq!(name_box(Some(F::Cancel), false, "Warm"), NameBox::Close);
        assert_eq!(name_box(None, false, "Warm"), NameBox::Keep);
    }

    /// Final review, fields 4: the name box for saving a preset, opened and left empty —
    /// a click elsewhere, Esc or Enter — closes and saves nothing; it never stays open
    /// without the keyboard (where Esc could no longer close it).
    #[test]
    fn the_preset_name_box_closes_and_never_saves_an_empty_name() {
        let mut app = crate::CantusApp::from_parts(
            Some(crate::starter_project()),
            cantus_engine::EngineHandle::dummy(),
            false,
        );
        let s0 = app.project().tracks[0].strip.unwrap();
        app.add_effect(s0, EffectKind::Gate);
        let fx = app.project().strip(s0).unwrap().effects[0].id;
        let spec = app.project().strip(s0).unwrap().effects[0].spec.clone();
        let shell = shell_of(EffectKind::Gate);
        let s = Style::current();
        for close in ["away", "esc", "enter"] {
            let ctx = egui::Context::default();
            crate::design::install_fonts(&ctx);
            crate::design::install_style(&ctx);
            let mut chrome = Chrome {
                saving: Some(String::new()),
                save_at: pos2(20.0, 20.0),
                ..Chrome::default()
            };
            let mut frame = 0u64;
            let mut run = |chrome: &mut Chrome, app: &mut crate::CantusApp, events: Vec<E>| {
                frame += 1;
                let input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                    time: Some(frame as f64 / 60.0),
                    events,
                    ..Default::default()
                };
                ctx.run_ui(input, |ui| {
                    let mut host =
                        super::super::host::DawHost::with_spec(app, s0, fx, spec.clone(), "test");
                    saving(ui.ctx(), &shell, &mut host, chrome, &s);
                })
                .textures_delta
                .clear();
            };
            for _ in 0..3 {
                run(&mut chrome, &mut app, Vec::new());
            }
            assert!(
                crate::kit::field::active(&ctx),
                "{close}: the box took the keys"
            );
            let key = |k| E::Key {
                key: k,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            };
            match close {
                "away" => {
                    let at = pos2(700.0, 500.0);
                    run(&mut chrome, &mut app, vec![E::PointerMoved(at)]);
                    for pressed in [true, false] {
                        let b = E::PointerButton {
                            pos: at,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        };
                        run(&mut chrome, &mut app, vec![b]);
                    }
                }
                "esc" => run(&mut chrome, &mut app, vec![key(Key::Escape)]),
                _ => run(&mut chrome, &mut app, vec![key(Key::Enter)]),
            }
            for _ in 0..2 {
                run(&mut chrome, &mut app, Vec::new());
            }
            assert!(chrome.saving.is_none(), "{close}: the box closed");
        }
    }
}
