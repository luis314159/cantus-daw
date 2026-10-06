//! **Pulsus II's editor**, on the kit's device shell (ADR 0024 §8): the 28-pt header (the
//! kit as the preset well with ‹ ›, undo, Play · Edit, ⋯, size), the display first — the
//! pad grid beside the selected pad's hit — then a card per section of the pad, and the
//! footer with the help line, the pads and model chips and the output meter.
//!
//! The kit lives in the project, so every edit here is a command (one undo step per
//! gesture) and the editor holds nothing the project does not. What it keeps is view
//! state: which track is open, which pad is selected (by identity, so the selection
//! follows the pad when a lower one is added), the view, where the window is, and the
//! rendered previews.
//!
//! - **The grid** (`pulsus_editor_grid.rs`): every pad as a raised cell with its name, note
//!   and model, its *rendered* hit as a waveform (synth pads included, which no sampler
//!   can show), its choke group as a lettered chip and a link to the pads it chokes. Click
//!   to select and hear (higher is harder).
//! - **The display** (`pulsus_editor_display.rs`): the hit drawn over time with its
//!   amplitude envelope and, on a log-Hz axis, its pitch — Kick 3's idea, for every voice.
//!   The envelope's handles and the pitch curve are the controls.
//! - **The cards** (`pulsus_editor_panel.rs`): voice and model as menu fields, then every
//!   field the pad's voice and model use, in real units, as cells; the layers; the kit.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::menu::{Check, Item};
use crate::kit::shell::{self, Event, Footer, Header, Preset, Shell};
use crate::kit::text::Src;
use crate::kit::{Function, Style};
use cantus_core::{
    Command, DrumPad, InstrumentParam, InstrumentSpec, KitField, MergeKey, PadField, PadId,
    PadVoice, PulsusParams, TrackId,
};
use egui::{Pos2, Rect};
use std::collections::HashMap;

#[path = "pulsus_editor_display.rs"]
mod display;
#[path = "pulsus_editor_grid.rs"]
mod grid;
#[path = "pulsus_editor_panel.rs"]
mod panel;

pub use display::{render_preview, Preview};
pub use grid::{choke_letter, contrast};

/// Which of the two views the editor shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum View {
    /// The grid and the selected pad's display, and the cards that matter live.
    #[default]
    Play,
    /// Everything.
    Edit,
}

/// A pop-up the shell's header opened.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Popup {
    #[default]
    None,
    /// The factory kits (the preset well).
    Kits,
    /// ⋯: add a pad of a voice, close the editor.
    More,
    /// ⤢: the window's scale.
    Sizes,
}

/// The editor's view state (owned by `CantusApp`).
#[derive(Debug, Default)]
pub struct State {
    pub open: Option<TrackId>,
    pub view: View,
    /// The selected pad, by identity.
    pub selected: Option<PadId>,
    pub popup: Popup,
    /// The factory kit each track was last loaded from, for the preset well.
    pub kit_name: HashMap<TrackId, &'static str>,
    /// Rendered hits, by a hash of what makes them sound.
    previews: HashMap<u64, Preview>,
}

impl State {
    /// A pad's rendered preview, rendering it if its sound changed.
    pub fn preview(
        &mut self,
        app_samples: &mut cantus_engine::SampleCache,
        pad: &DrumPad,
        version: u8,
    ) -> &Preview {
        let key = display::preview_key(pad, version);
        if !self.previews.contains_key(&key) {
            if self.previews.len() > 256 {
                self.previews.clear();
            }
            let preview = render_preview(pad, version, app_samples);
            self.previews.insert(key, preview);
        }
        &self.previews[&key]
    }
}

/// Open the editor on a track's kit.
pub fn open(app: &mut CantusApp, track: TrackId) {
    app.pulsus.open = Some(track);
    if let Some(kit) = kit_of(app, track) {
        if app.pulsus.selected.and_then(|id| kit.by_id(id)).is_none() {
            app.pulsus.selected = kit.pads.first().map(|p| p.id);
        }
    }
}

pub fn close(app: &mut CantusApp) {
    app.pulsus.open = None;
    app.pulsus.popup = Popup::None;
}

/// The kit a track hosts, cloned.
pub fn kit_of(app: &CantusApp, track: TrackId) -> Option<PulsusParams> {
    app.project()
        .track(track)
        .and_then(|t| t.instrument.pulsus())
        .cloned()
}

/// Set one field of one pad; a gesture merges frame to frame into one undo step.
pub fn set_field(
    app: &mut CantusApp,
    track: TrackId,
    pad: PadId,
    field: PadField,
    value: f32,
    gesture: bool,
) {
    let cmd = Command::SetInstrumentParam {
        track,
        param: InstrumentParam::Pulsus { pad, field },
        value,
    };
    if gesture {
        app.exec_merging(cmd, MergeKey::of(&("pulsus", track.0, pad.0, field)));
    } else {
        app.exec(cmd);
    }
}

pub fn set_kit_field(app: &mut CantusApp, track: TrackId, field: KitField, value: f32) {
    app.exec(Command::SetInstrumentParam {
        track,
        param: InstrumentParam::PulsusKit(field),
        value,
    });
}

/// Upgrade a model 1 kit to Pulsus II: the version, and the drive model 1 stored where it
/// never sounded. One undo step.
pub fn upgrade(app: &mut CantusApp, track: TrackId) {
    let Some(kit) = kit_of(app, track) else {
        return;
    };
    let mut commands: Vec<Command> = kit
        .upgrade_edits()
        .into_iter()
        .map(|(pad, field, value)| Command::SetInstrumentParam {
            track,
            param: InstrumentParam::Pulsus { pad, field },
            value,
        })
        .collect();
    commands.push(Command::SetInstrumentParam {
        track,
        param: InstrumentParam::PulsusKit(KitField::Version),
        value: cantus_core::PULSUS_MODEL as f32,
    });
    app.exec(Command::Batch {
        label_key: "cmd.pulsus_upgrade".into(),
        commands,
    });
}

/// Load a factory kit onto the track (one undo step), with its pad names in the interface
/// language.
pub fn load_kit(app: &mut CantusApp, track: TrackId, kit: PulsusParams) {
    app.set_instrument(track, InstrumentSpec::Pulsus(kit));
    app.pulsus.selected = kit_of(app, track).and_then(|k| k.pads.first().map(|p| p.id));
}

/// Load factory kit `i` and name it in the preset well.
fn load_factory(app: &mut CantusApp, track: TrackId, i: usize) {
    let factory = PulsusParams::factory();
    if let Some(f) = factory.into_iter().nth(i) {
        app.pulsus.kit_name.insert(track, f.label_key);
        load_kit(app, track, f.kit);
    }
}

/// Names of Pulsus II's shipped pads in the interface language; `None` for a name the user
/// typed or one the classic kit already localises.
pub fn localized_name(name: &str) -> Option<String> {
    let key = match name {
        "808 kick" => "pulsus.name.kick_808",
        "909 kick" => "pulsus.name.kick_909",
        "Clap" => "pulsus.name.clap",
        "Pedal hat" => "pulsus.name.pedal_hat",
        "Mid tom" => "pulsus.name.mid_tom",
        "Crash" => "pulsus.name.crash",
        "Ride" => "pulsus.name.ride",
        "Cowbell" => "pulsus.name.cowbell",
        "Claves" => "pulsus.name.claves",
        "Shaker" => "pulsus.name.shaker",
        "Conga" => "pulsus.name.conga",
        "Bongo" => "pulsus.name.bongo",
        "Bell" => "pulsus.name.bell",
        "Woodblock" => "pulsus.name.woodblock",
        _ => return None,
    };
    Some(t(key))
}

/// Hear a pad now, as the grid's click does.
pub fn audition(app: &mut CantusApp, track: TrackId, pitch: u8, velocity: u8) {
    app.engine.note_on(track, pitch, velocity);
    app.engine.note_off(track, pitch);
}

/// Add a pad of `voice` and select it.
pub fn add_pad(app: &mut CantusApp, track: TrackId, voice: PadVoice) {
    app.add_pad(track, voice);
    app.pulsus.selected = kit_of(app, track).and_then(|k| k.pads.iter().map(|p| p.id).max());
}

/// The two views, as the header's segmented control names them.
const VIEWS: [Src<'static>; 2] = [Src::Key("pulsus.view.play"), Src::Key("pulsus.view.edit")];

/// The device hue: a drum machine's main function is its source.
pub const HUE: Function = Function::Source;

/// The track's own output, as the footer's meter shows it.
fn track_levels(app: &CantusApp, track: TrackId) -> crate::kit::meter::Levels {
    let i = app.project().tracks.iter().position(|t| t.id == track);
    let peak = i
        .map(|i| app.engine.meters().tracks.get(i).copied().unwrap_or(0.0))
        .unwrap_or(0.0);
    let db = cantus_core::linear_to_db(peak.max(1e-6));
    crate::kit::meter::Levels {
        level: [db, db],
        hold: [db, db],
    }
}

/// The open editor hosted in `ui` at its natural size from `ui`'s top-left, standing in for
/// its workspace tab: what a test or a screenshot draws (see `inst::window_in`).
pub fn window_in(app: &mut CantusApp, ui: &mut egui::Ui) {
    if let Some(track) = app.pulsus.open {
        let k = Shell::new(("pulsus", track.0)).scale(ui.ctx());
        crate::views::inst::host(ui, editor_id(track), shell::INSTRUMENT * k);
    }
    window(app, ui.ctx());
}

/// The Pulsus editor's device id: its tab in the workspace.
pub fn editor_id(track: TrackId) -> egui::Id {
    egui::Id::new(("pulsus-editor", track.0))
}

/// The editor window: the kit's shell in a device tab of the workspace (docked by default,
/// floating by choice).
pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(track) = app.pulsus.open else {
        return;
    };
    let Some(kit) = kit_of(app, track) else {
        close(app);
        return;
    };
    let shell = Shell::new(("pulsus", track.0));
    let k = shell.scale(ctx);
    let size = shell::INSTRUMENT * k;
    let track_name = app
        .project()
        .track(track)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let name = t("pulsus.product");
    let context = format!("({track_name})");
    let kit_name = app
        .pulsus
        .kit_name
        .get(&track)
        .map(|k| t(k))
        .unwrap_or_else(|| t("pulsus.kit.custom"));
    let undo = (app.bus.can_undo(), app.bus.can_redo());
    let view = match app.pulsus.view {
        View::Play => 0,
        View::Edit => 1,
    };
    let header = Header {
        name: &name,
        context: &context,
        hue: HUE,
        preset: Some(Preset {
            name: &kit_name,
            modified: false,
            favourite: false,
        }),
        ab: None,
        undo: Some(undo),
        views: Some((&VIEWS, view)),
        bypassed: None,
        help: false,
        window: Some(crate::views::device_chrome::ctl(
            ctx,
            editor_id(track).value(),
            "swap.tip.instrument",
        )),
    };
    let pads = tf("pulsus.chip.pads", &[("n", &kit.pads.len().to_string())]);
    let model = if kit.version < cantus_core::PULSUS_MODEL {
        t("pulsus.chip.model1")
    } else {
        t("pulsus.model2")
    };
    let chips = [pads.as_str(), model.as_str()];
    let footer = Footer {
        chips: &chips,
        meter: Some(track_levels(app, track)),
    };
    let mut open = true;
    let mut events = Vec::new();
    let shown = crate::views::workspace_device::DeviceWindow::new(format!("{name} {context}"))
        .id(editor_id(track))
        .open(&mut open)
        .default_size(size)
        .min_size(size * 0.75)
        .hue(HUE)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let out = shell.show(ui, &header, &footer, |ui, s, body_rect| {
                body(app, ui, track, &kit, s, body_rect);
            });
            let key = editor_id(track).value();
            crate::views::workspace_device::note_shown(ctx, key, rect, out.header, out.window);
            events = out.events;
            rect
        });
    if !open {
        close(app);
        return;
    }
    let Some(rect) = shown else {
        return;
    };
    for ev in events {
        match ev {
            // The tab is the window: the dock moves it, not the header.
            Event::Move(_) => {}
            Event::View(i) => {
                app.pulsus.view = if i == 1 { View::Edit } else { View::Play };
            }
            Event::Undo => app.undo(),
            Event::Redo => app.redo(),
            Event::PresetPrev => step_kit(app, track, -1),
            Event::PresetNext => step_kit(app, track, 1),
            Event::PresetBrowse | Event::Menu => app.pulsus.popup = Popup::Kits,
            Event::More => app.pulsus.popup = Popup::More,
            Event::SizeMenu => app.pulsus.popup = Popup::Sizes,
            Event::Scale(k) => shell.set_scale(ctx, k),
            Event::Ab(_) | Event::Favourite | Event::Help | Event::Bypass => {}
            // The window's own buttons (round 20).
            Event::Close | Event::Minimise | Event::Maximise | Event::Swap => {
                use crate::views::device_chrome::{on_event, Asked};
                let swap = crate::picker::PickPurpose::ReplaceInstrument { track };
                let key = editor_id(track).value();
                if on_event(app, ctx, key, ev, swap) == Asked::Close {
                    close(app);
                    return;
                }
            }
        }
    }
    popups(app, ctx, track, &shell, rect, k);
}

/// Step to the previous or next factory kit, as one undo step.
fn step_kit(app: &mut CantusApp, track: TrackId, by: i32) {
    let factory = PulsusParams::factory();
    let n = factory.len() as i32;
    if n == 0 {
        return;
    }
    let at = app
        .pulsus
        .kit_name
        .get(&track)
        .and_then(|k| factory.iter().position(|f| f.label_key == *k))
        .map(|i| i as i32)
        .unwrap_or(if by > 0 { -1 } else { 0 });
    load_factory(app, track, (at + by).rem_euclid(n) as usize);
}

fn entry(label: Src<'_>, check: Check) -> Item<'_> {
    Item::Action {
        label,
        check,
        shortcut: None,
        submenu: false,
        enabled: true,
    }
}

/// The kits, the ⋯ menu and the sizes, below the header.
fn popups(
    app: &mut CantusApp,
    ctx: &egui::Context,
    track: TrackId,
    shell: &Shell,
    rect: Rect,
    k: f32,
) {
    let popup = app.pulsus.popup;
    if popup == Popup::None {
        return;
    }
    let s = Style::current().scaled(k).function(HUE);
    let at = Pos2::new(rect.right() - 290.0 * k, rect.top() + shell::HEADER * k);
    let factory = PulsusParams::factory();
    let kit_labels: Vec<String> = factory
        .iter()
        .map(|f| {
            let tags: Vec<String> = f.tags.iter().map(|k| t(k)).collect();
            format!("{}  \u{b7}  {}", t(f.label_key), tags.join(", "))
        })
        .collect();
    let percents: Vec<String> = shell::SCALES
        .iter()
        .map(|s| format!("{:.0} %", s * 100.0))
        .collect();
    let current = app.pulsus.kit_name.get(&track).copied();
    let items: Vec<Item<'_>> = match popup {
        Popup::Kits => {
            let mut v = vec![Item::Band(Src::Key("pulsus.kit"))];
            v.extend(
                factory
                    .iter()
                    .zip(&kit_labels)
                    .map(|(f, l)| entry(Src::Str(l), Check::Radio(Some(f.label_key) == current))),
            );
            v
        }
        Popup::More => {
            let mut v = vec![Item::Band(Src::Key("pulsus.pad.add"))];
            v.extend(
                PadVoice::ALL
                    .iter()
                    .map(|voice| entry(Src::Key(voice.label_key()), Check::None)),
            );
            v.push(Item::Separator);
            v.push(entry(Src::Key("pulsus.close"), Check::None));
            v
        }
        Popup::Sizes => {
            let now = shell.scale(ctx);
            percents
                .iter()
                .zip(shell::SCALES)
                .map(|(p, sc)| entry(Src::Str(p), Check::Radio((sc - now).abs() < 0.01)))
                .collect()
        }
        Popup::None => Vec::new(),
    };
    let d = s.d(1.0);
    let w = 280.0 * k;
    let h = crate::kit::menu::height(&items, d);
    let mut chosen = None;
    let area = egui::Area::new(egui::Id::new(("pulsus-popup", track.0)))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
            let hovered = resp.hover_pos().and_then(|p| {
                (0..items.len()).find(|&i| {
                    items[i].is_action() && crate::kit::menu::row(r, &items, d, i).contains(p)
                })
            });
            crate::kit::with_brush(ui.painter(), |b| {
                crate::kit::menu::paint(b, r, &s, &items, hovered)
            });
            if resp.clicked() {
                chosen = hovered;
            }
            resp
        });
    if let Some(i) = chosen {
        match popup {
            Popup::Kits => {
                if i >= 1 {
                    load_factory(app, track, i - 1);
                }
            }
            Popup::More => {
                if let Some(voice) = i.checked_sub(1).and_then(|j| PadVoice::ALL.get(j)) {
                    add_pad(app, track, *voice);
                } else if i == items.len() - 1 {
                    close(app);
                }
            }
            Popup::Sizes => {
                if let Some(sc) = shell::SCALES.get(i) {
                    shell.set_scale(ctx, *sc);
                }
            }
            Popup::None => {}
        }
    }
    if chosen.is_some() || area.inner.clicked_elsewhere() {
        app.pulsus.popup = Popup::None;
    }
}

/// The body: the display row, then the cards; Edit scrolls when it is taller.
fn body(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    kit: &PulsusParams,
    s: &Style,
    body_rect: Rect,
) {
    let area = body_rect.shrink(s.d(8.0));
    let mut view = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(area)
            .id_salt(("pulsus-view", track.0)),
    );
    view.set_clip_rect(area);
    let selected = app
        .pulsus
        .selected
        .and_then(|id| kit.by_id(id))
        .or(kit.pads.first())
        .cloned();
    match app.pulsus.view {
        View::Play => panel::play(app, &mut view, track, kit, selected.as_ref(), s),
        View::Edit => {
            egui::ScrollArea::vertical()
                .id_salt(("pulsus-edit", track.0))
                .show(&mut view, |ui| {
                    panel::edit(app, ui, track, kit, selected.as_ref(), s)
                });
        }
    }
}

/// `shoot … pulsus` (Edit) or `pulsus-play`: the studio kit on the selected track, the
/// kick selected, the editor open.
pub fn demo(app: &mut CantusApp, kind: &str) {
    let Some(track) = app.selected_track else {
        return;
    };
    load_kit(app, track, PulsusParams::studio());
    open(app, track);
    app.pulsus.kit_name.insert(track, "pulsus.kit.studio");
    app.pulsus.view = if kind.ends_with("play") {
        View::Play
    } else {
        View::Edit
    };
    if let Some(kit) = kit_of(app, track) {
        app.pulsus.selected = kit.pads.iter().find(|p| p.pitch == 36).map(|p| p.id);
    }
}
