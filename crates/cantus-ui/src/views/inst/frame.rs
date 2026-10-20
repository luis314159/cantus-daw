//! The frame every instrument editor shares, on the kit's device shell (ADR 0024 §8): the
//! 28-pt header that is the title bar (preset well, A|B, undo, Play · Edit · Matrix), the
//! body — the instrument's display and cards — the bands at its bottom (macros and
//! modulation sharing a row, the keyboard docked below when shown), and the 24-pt footer
//! with the help line and the chips. Plus the dispatch to each instrument's module.
//!
//! Each instrument fills only the area above the bands: `play` and `body` receive a `ui`
//! whose rect is that area and the shell's [`Style`] (its scale `k` and the device hue).

use super::*;
use crate::kit::knob::KnobSize;
use crate::kit::shell::{self, Ab, Bands, Event, Footer, Header, Preset, Shell};
use crate::kit::text::{self as ktext, Policy, Role, Src, Text};
use crate::kit::widgets::Param;
use crate::kit::{self, Function, Style};
use crate::widgets::Knob;
use cantus_core::inst::modulation::{
    self, connections, Scope, MACROS, MACRO_KEYS, MACRO_NAME_KEYS,
};
use egui::{Pos2, Rect};
use std::fmt::Write as _;

// ---- dispatch ----------------------------------------------------------------------------

/// A kind's factory presets.
pub fn factory_presets(kind: InstrumentKind) -> &'static [FactoryPreset] {
    match kind {
        InstrumentKind::Wavetable => wavetable::FACTORY_PRESETS,
        InstrumentKind::Analog => analog::FACTORY_PRESETS,
        InstrumentKind::Fm => fm::FACTORY_PRESETS,
        InstrumentKind::Sampler => sampler::FACTORY_PRESETS,
        InstrumentKind::Bass => bass::FACTORY_PRESETS,
        InstrumentKind::ElectricPiano => epiano::FACTORY_PRESETS,
        InstrumentKind::Organ => organ::FACTORY_PRESETS,
        InstrumentKind::Chorda => chorda::FACTORY_PRESETS,
    }
}

fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    match spec.kind {
        InstrumentKind::Wavetable => wavetable::body(app, ui, track, spec, s),
        InstrumentKind::Analog => analog::body(app, ui, track, spec, s),
        InstrumentKind::Fm => fm::body(app, ui, track, spec, s),
        InstrumentKind::Sampler => sampler::body(app, ui, track, spec, s),
        InstrumentKind::Bass => bass::body(app, ui, track, spec, s),
        InstrumentKind::ElectricPiano => epiano::body(app, ui, track, spec, s),
        InstrumentKind::Organ => organ::body(app, ui, track, spec, s),
        InstrumentKind::Chorda => chorda::body(app, ui, track, spec, s),
    }
}

fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    match spec.kind {
        InstrumentKind::Wavetable => wavetable::play(app, ui, track, spec, s),
        InstrumentKind::Analog => analog::play(app, ui, track, spec, s),
        InstrumentKind::Fm => fm::play(app, ui, track, spec, s),
        InstrumentKind::Sampler => sampler::play(app, ui, track, spec, s),
        InstrumentKind::Bass => bass::play(app, ui, track, spec, s),
        InstrumentKind::ElectricPiano => epiano::play(app, ui, track, spec, s),
        InstrumentKind::Organ => organ::play(app, ui, track, spec, s),
        InstrumentKind::Chorda => chorda::play(app, ui, track, spec, s),
    }
}

/// The inspector's picture of an instrument.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    match spec.kind {
        InstrumentKind::Wavetable => wavetable::preview(ui, spec),
        InstrumentKind::Analog => analog::preview(ui, spec),
        InstrumentKind::Fm => fm::preview(ui, spec),
        InstrumentKind::Sampler => sampler::preview(ui, spec),
        InstrumentKind::Bass => bass::preview(ui, spec),
        InstrumentKind::ElectricPiano => epiano::preview(ui, spec),
        InstrumentKind::Organ => organ::preview(ui, spec),
        InstrumentKind::Chorda => chorda::preview(ui, spec),
    }
}

/// `--editor <instrument>`: the settings a kind's screenshot is taken with.
pub fn demo_setup(app: &mut CantusApp, track: TrackId, kind: InstrumentKind) {
    match kind {
        InstrumentKind::Wavetable => wavetable::demo_setup(app, track),
        InstrumentKind::Analog => analog::demo_setup(app, track),
        InstrumentKind::Fm => fm::demo_setup(app, track),
        InstrumentKind::Sampler => sampler::demo_setup(app, track),
        InstrumentKind::Bass => bass::demo_setup(app, track),
        InstrumentKind::ElectricPiano => epiano::demo_setup(app, track),
        InstrumentKind::Organ => organ::demo_setup(app, track),
        InstrumentKind::Chorda => chorda::demo_setup(app, track),
    }
}

/// For screenshots and tests with no engine: what a playing engine would report.
pub fn demo_feed(app: &mut CantusApp, kind: InstrumentKind) {
    match kind {
        InstrumentKind::Wavetable => wavetable::demo_feed(app),
        InstrumentKind::Analog => analog::demo_feed(app),
        InstrumentKind::Fm => fm::demo_feed(app),
        InstrumentKind::Sampler => sampler::demo_feed(app),
        InstrumentKind::Bass => bass::demo_feed(app),
        InstrumentKind::ElectricPiano => epiano::demo_feed(app),
        InstrumentKind::Organ => organ::demo_feed(app),
        InstrumentKind::Chorda => chorda::demo_feed(app),
    }
}

/// The device hue: the function of each instrument's main section (ADR 0024 §8).
pub fn hue(kind: InstrumentKind) -> Function {
    match kind {
        InstrumentKind::Analog | InstrumentKind::Organ | InstrumentKind::Chorda => Function::Tone,
        _ => Function::Source,
    }
}

/// The function family of a modulation source, for its chip's bar and the rings it draws.
pub fn source_family(tag: &str) -> Function {
    if tag.starts_with("env") {
        Function::Dynamics
    } else if tag.starts_with("lfo") || tag.starts_with("macro") {
        Function::Mod
    } else if tag.starts_with("mpe") {
        Function::Space
    } else if matches!(tag, "mw" | "at" | "pb" | "expr" | "sus") {
        Function::Output
    } else {
        Function::Source
    }
}

// ---- the window ----------------------------------------------------------------------------

/// The three views, as the header's segmented control names them.
const VIEWS: [Src<'static>; 3] = [
    Src::Key("inst.view.play"),
    Src::Key("inst.view.edit"),
    Src::Key("inst.view.matrix"),
];

fn view_index(v: View) -> usize {
    match v {
        View::Play => 0,
        View::Edit => 1,
        View::Matrix => 2,
    }
}

/// Whether the keyboard card shows: the user's choice, or Play view by default.
pub fn keyboard_shown(app: &CantusApp) -> bool {
    app.inst.keyboard.unwrap_or(app.inst.view == View::Play)
}

/// The open editor hosted in `ui` at its natural size from `ui`'s top-left, standing in for
/// the workspace tab it lives in: what a test or a screenshot draws. The app calls
/// [`window`], and the workspace hosts it.
pub fn window_in(app: &mut CantusApp, ui: &mut egui::Ui) {
    if let Some(track) = app.inst.open {
        let k = Shell::new(("inst", track.0)).scale(ui.ctx());
        host(ui, editor_id(track), shell::INSTRUMENT * k);
    }
    window(app, ui.ctx());
}

/// The instrument editor's device id: its tab in the workspace.
pub fn editor_id(track: TrackId) -> egui::Id {
    egui::Id::new(("inst-editor", track.0))
}

/// Give the device `id` a body of `size` at `ui`'s top-left this frame, as the workspace
/// gives a tab's body: register it, then record where it draws.
pub(crate) fn host(ui: &mut egui::Ui, id: egui::Id, size: egui::Vec2) {
    use crate::views::workspace_device::{record_body, DeviceWindow};
    let ctx = ui.ctx().clone();
    let _ = DeviceWindow::new("").id(id).show(&ctx, |_| ());
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    let body = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    record_body(&ctx, id.value(), &body);
}

/// The window for the open editor, drawn every frame while one is open: the kit's shell in
/// a device tab of the workspace (docked by default, floating by choice).
pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(track) = app.inst.open else {
        return;
    };
    let Some(spec) = app.native_of(track) else {
        app.close_inst_editor();
        return;
    };
    if crate::views::workspace_device::keyboard_toggle_requested(ctx) {
        app.inst.keyboard = Some(!keyboard_shown(app));
    }
    let shell = Shell::new(("inst", track.0));
    let k = shell.scale(ctx);
    let size = shell::INSTRUMENT * k;
    let track_name = app
        .project()
        .track(track)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let name = t(spec.kind.name_key());
    let context = format!("({track_name})");
    let preset_name = app
        .inst
        .preset
        .get(&track)
        .cloned()
        .unwrap_or_else(|| t("inst.preset.init"));
    let on_b = app.inst.on_b(track);
    let undo = (app.bus.can_undo(), app.bus.can_redo());
    let key = editor_id(track).value();
    let header = Header {
        name: &name,
        context: &context,
        hue: hue(spec.kind),
        preset: Some(Preset {
            name: &preset_name,
            modified: false,
            favourite: false,
        }),
        ab: Some(if on_b { Ab::B } else { Ab::A }),
        undo: Some(undo),
        views: Some((&VIEWS, view_index(app.inst.view))),
        bypassed: None,
        help: false,
        window: Some(crate::views::device_chrome::ctl(
            ctx,
            key,
            "swap.tip.instrument",
        )),
    };
    let (chip_strings, chip_tips) = footer_chips(app, &spec);
    let chips: Vec<&str> = chip_strings.iter().map(String::as_str).collect();
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
        .hue(hue(spec.kind))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let out = shell.show(ui, &header, &footer, |ui, s, body_rect| {
                shell_body(app, ui, track, &spec, s, body_rect);
            });
            crate::views::workspace_device::note_shown(ctx, key, rect, out.header, out.window);
            // A chip that explains itself (Exemplar's memory): its tip on the help line.
            let hovered = ui.input(|i| i.pointer.hover_pos());
            if let Some(p) = hovered {
                for (r, tip) in out.chips.iter().zip(&chip_tips) {
                    if let (true, Some(tip)) = (r.contains(p), tip) {
                        crate::kit::hint::set_text(ctx, tip, "", crate::kit::hint::Caps::NONE);
                    }
                }
            }
            events = out.events;
            rect
        });
    if !open {
        app.close_inst_editor();
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
                app.inst.view = match i {
                    1 => View::Edit,
                    2 => View::Matrix,
                    _ => View::Play,
                }
            }
            Event::Ab(ab) => {
                if (ab == Ab::B) != on_b {
                    app.toggle_inst_ab(track);
                }
            }
            Event::Undo => app.undo(),
            Event::Redo => app.redo(),
            Event::PresetPrev => step_preset(app, track, &spec, -1),
            Event::PresetNext => step_preset(app, track, &spec, 1),
            Event::PresetBrowse | Event::Menu => app.inst.popup = Popup::Presets,
            Event::More => app.inst.popup = Popup::More,
            Event::SizeMenu => app.inst.popup = Popup::Sizes,
            Event::Scale(k) => shell.set_scale(ctx, k),
            Event::Favourite | Event::Help | Event::Bypass => {}
            // The window's own buttons (round 20).
            Event::Close | Event::Minimise | Event::Maximise | Event::Swap => {
                use crate::views::device_chrome::{on_event, Asked};
                let swap = crate::picker::PickPurpose::ReplaceInstrument { track };
                if on_event(app, ctx, key, ev, swap) == Asked::Close {
                    app.close_inst_editor();
                    return;
                }
            }
        }
    }
    popups(app, ctx, track, &spec, &shell, rect, k);
    ctx.request_repaint();
}

/// The body: the bands claim the bottom, the view fills the rest.
fn shell_body(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    body_rect: Rect,
) {
    let mut grid = shell::Grid::new(body_rect, s);
    let bands = Bands {
        macros_open: !app.inst.macros_closed,
        mods_open: !app.inst.mods_closed,
        keyboard: keyboard_shown(app),
    };
    let br = shell::layout_bands(&mut grid, s, &bands);
    let area = grid.rest();
    let mut view = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(area)
            .id_salt(("inst-view", track.0)),
    );
    view.set_clip_rect(area);
    match app.inst.view {
        View::Play => play(app, &mut view, track, spec, s),
        View::Edit => {
            egui::ScrollArea::vertical()
                .id_salt(("inst-edit", track.0))
                .show(&mut view, |ui| body(app, ui, track, spec, s));
        }
        View::Matrix => {
            egui::ScrollArea::vertical()
                .id_salt(("inst-matrix", track.0))
                .show(&mut view, |ui| matrix(app, ui, track, spec, s));
        }
    }
    macro_band(app, ui, track, spec, s, br.macros);
    mod_band(app, ui, track, spec, s, br.mods);
    if let Some(kr) = br.keyboard {
        keyboard_band(app, ui, track, s, kr);
    }
}

/// The footer's chips: voices, steals, and what an instrument adds of its own — and, for
/// each, the tip its hover shows on the help line, when it has one.
fn footer_chips(app: &CantusApp, spec: &NativeSpec) -> (Vec<String>, Vec<Option<String>>) {
    let tel = app.inst_telemetry();
    let mut out = vec![tf(
        "inst.voices",
        &[
            ("n", &format!("{}", tel[TEL_VOICES] as u32)),
            ("of", &format!("{}", tel[TEL_POLYPHONY] as u32)),
        ],
    )];
    if tel[TEL_STOLEN] > 0.0 {
        out.push(tf(
            "inst.stolen",
            &[("n", &format!("{}", tel[TEL_STOLEN] as u32))],
        ));
    }
    let mut tips = vec![None; out.len()];
    if spec.kind == InstrumentKind::Sampler {
        sampler::chips(&tel, sampler::loading(app, spec), &mut out, &mut tips);
    }
    (out, tips)
}

/// The track's own output as the footer's meter shows it.
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

/// The factory presets then the user's, as the ‹ › steppers walk them.
fn preset_list(kind: InstrumentKind) -> Vec<(String, NativeSpec)> {
    let mut all: Vec<(String, NativeSpec)> = factory_presets(kind)
        .iter()
        .map(|p| (t(p.label_key), preset_spec(kind, p)))
        .collect();
    all.extend(user_presets(kind));
    all
}

/// Step to the previous or next preset, as one undo step.
fn step_preset(app: &mut CantusApp, track: TrackId, spec: &NativeSpec, by: i32) {
    let list = preset_list(spec.kind);
    if list.is_empty() {
        return;
    }
    let current = app.inst.preset.get(&track);
    let at = current
        .and_then(|c| list.iter().position(|(n, _)| n == c))
        .map(|i| i as i32)
        .unwrap_or(if by > 0 { -1 } else { 0 });
    let n = list.len() as i32;
    let i = (at + by).rem_euclid(n) as usize;
    let (name, preset) = &list[i];
    app.apply_inst_spec(track, preset, "cmd.fx_preset");
    app.inst.preset.insert(track, name.clone());
}

// ---- the header's pop-ups ------------------------------------------------------------------

fn check(on: bool) -> crate::kit::menu::Check {
    if on {
        crate::kit::menu::Check::On
    } else {
        crate::kit::menu::Check::Off
    }
}

fn entry<'a>(label: Src<'a>, check: crate::kit::menu::Check) -> crate::kit::menu::Item<'a> {
    crate::kit::menu::Item::Action {
        label,
        check,
        shortcut: None,
        submenu: false,
        enabled: true,
    }
}

/// The ⋯ menu (the bands), the preset list and the sizes, below the header.
fn popups(
    app: &mut CantusApp,
    ctx: &egui::Context,
    track: TrackId,
    spec: &NativeSpec,
    shell: &Shell,
    rect: Rect,
    k: f32,
) {
    use crate::kit::menu::{Check, Item};
    let popup = app.inst.popup;
    if popup == Popup::None {
        return;
    }
    let at = Pos2::new(rect.right() - 260.0 * k, rect.top() + shell::HEADER * k);
    let presets = preset_list(spec.kind);
    let percents: Vec<String> = shell::SCALES
        .iter()
        .map(|s| format!("{:.0} %", s * 100.0))
        .collect();
    let items: Vec<Item<'_>> = match popup {
        Popup::More => vec![
            entry(Src::Key("inst.more.keyboard"), check(keyboard_shown(app))),
            entry(Src::Key("inst.more.macros"), check(!app.inst.macros_closed)),
            entry(Src::Key("inst.more.mods"), check(!app.inst.mods_closed)),
        ],
        Popup::Presets => {
            let current = app.inst.preset.get(&track).cloned().unwrap_or_default();
            let mut v = vec![Item::Band(Src::Key("preset.factory"))];
            v.extend(
                presets
                    .iter()
                    .map(|(n, _)| entry(Src::Str(n), Check::Radio(*n == current))),
            );
            v
        }
        Popup::Sizes => {
            let now = shell.scale(ctx);
            percents
                .iter()
                .zip(shell::SCALES)
                .map(|(p, s)| entry(Src::Str(p), Check::Radio((s - now).abs() < 0.01)))
                .collect()
        }
        Popup::None => Vec::new(),
    };
    let area = egui::Area::new(egui::Id::new(("inst-popup", track.0)))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            crate::kit::widgets::menu_list(ui, &items, 250.0 * k)
        });
    let (resp, clicks) = area.inner;
    let chosen = clicks.clicked.or(clicks.toggled);
    if let Some(i) = chosen {
        match popup {
            Popup::More => match i {
                0 => app.inst.keyboard = Some(!keyboard_shown(app)),
                1 => app.inst.macros_closed = !app.inst.macros_closed,
                _ => app.inst.mods_closed = !app.inst.mods_closed,
            },
            Popup::Presets => {
                if let Some((name, preset)) = i.checked_sub(1).and_then(|j| presets.get(j)) {
                    app.apply_inst_spec(track, preset, "cmd.fx_preset");
                    app.inst.preset.insert(track, name.clone());
                }
            }
            Popup::Sizes => {
                if let Some(s) = shell::SCALES.get(i) {
                    shell.set_scale(ctx, *s);
                }
            }
            Popup::None => {}
        }
    }
    if clicks.clicked.is_some() || resp.clicked_elsewhere() {
        app.inst.popup = Popup::None;
    }
}

// ---- the bands -----------------------------------------------------------------------------

/// The macro card: eight L knobs, each named and printed, each a modulation source you
/// drag by its name. The card's title collapses it.
fn macro_band(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let open = !app.inst.macros_closed;
    let cr = kit::with_brush(&ui.painter_at(rect), |b| {
        shell::macro_card(b, rect, s, &[], open)
    });
    if ui
        .interact(
            cr.title,
            ui.id().with(("macro-title", track.0)),
            egui::Sense::click(),
        )
        .clicked()
    {
        app.inst.macros_closed = open;
    }
    if !open {
        return;
    }
    let s = &s.function(Function::Mod);
    let k = s.k;
    let body = Rect::from_min_max(
        Pos2::new(rect.left() + 6.0 * k, cr.title.bottom() + 2.0 * k),
        Pos2::new(rect.right() - 6.0 * k, rect.bottom() - 2.0 * k),
    );
    // The macros that mean something — named, moved or connected — or the first four of an
    // untouched instrument: eight L knobs with their names do not fit a card this wide.
    let mut shown: Vec<usize> = (0..MACROS)
        .filter(|&m| {
            spec.text(MACRO_NAME_KEYS[m]).is_some()
                || spec.is_stored(MACRO_KEYS[m])
                || connections(spec).any(|c| c.src == MACRO_KEYS[m])
        })
        .collect();
    if shown.is_empty() {
        shown = (0..4).collect();
    }
    let w = body.width() / shown.len().max(4) as f32;
    let d = KnobSize::L.diameter() * k;
    for (slot, &m) in shown.iter().enumerate() {
        let cell = Rect::from_min_size(
            Pos2::new(body.left() + w * slot as f32, body.top()),
            egui::vec2(w, body.height()),
        );
        let c = Pos2::new(cell.left() + d * 0.5 + 4.0 * k, cell.center().y);
        let knob_rect = Rect::from_center_size(c, egui::Vec2::splat(d + 6.0 * k));
        let key: &'static str = MACRO_KEYS[m];
        let name = spec
            .text(MACRO_NAME_KEYS[m])
            .map(str::to_string)
            .unwrap_or_else(|| tf("inst.macro_n", &[("n", &(m + 1).to_string())]));
        let mut v = spec.get(key);
        let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
        let w = wire(app, track, key, &mut v, (0.0, 1.0, false), s, &mut rings);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(knob_rect));
        let (r, choice) = Param::new(&mut v, 0.0..=1.0, 0.0)
            .size(KnobSize::L)
            .name(&name)
            .style(*s)
            .rings(&rings[..w.n])
            .offers(w.on, w.automated, w.offer)
            .format(|v, b| {
                let _ = write!(b, "{:.0} %", v * 100.0);
            })
            .show_full(&mut child);
        wired(app, &w, choice, r.rect);
        if r.changed() {
            app.set_inst_value(track, key, v, true);
        }
        if r.drag_stopped() || r.double_clicked() {
            app.end_gesture();
        }
        // The name, above the value: drag it to use the macro as a source.
        let tx = c.x + d * 0.5 + 6.0 * k;
        let lr = Rect::from_min_max(
            Pos2::new(tx, c.y - 16.0 * k),
            Pos2::new(cell.right() - 4.0 * k, c.y),
        );
        let vr = Rect::from_min_max(
            Pos2::new(tx, c.y),
            Pos2::new(cell.right() - 4.0 * k, c.y + 16.0 * k),
        );
        let value = format!("{:.0} %", v * 100.0);
        kit::with_brush(&ui.painter_at(cell), |b| {
            let _ = ktext::paint(
                b,
                lr,
                &Text::new(&name, Role::Label, s.t.label)
                    .caps(true)
                    .align(egui::Align2::LEFT_CENTER)
                    .policy(Policy::Abbreviate),
                s.tk(),
            );
            let _ = ktext::paint(
                b,
                vr,
                &Text::new(&value, Role::Value, s.t.text).align(egui::Align2::LEFT_CENTER),
                s.tk(),
            );
        });
        drag_source(ui, lr, key);
    }
}

/// Make `rect` a drag source for the modulation source `tag`.
fn drag_source(ui: &mut egui::Ui, rect: Rect, tag: &'static str) {
    let id = ui.id().with(("mod-src", tag));
    let r = ui.interact(rect, id, egui::Sense::drag());
    if r.drag_started() {
        egui::DragAndDrop::set_payload(ui.ctx(), tag);
    }
    if r.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("mod.drag_tip"),
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
    if r.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
}

/// The modulation card: a chip per source the instrument produces (not the macros, which
/// have their own card), with how many targets each drives. Drag a chip onto a knob.
fn mod_band(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let open = !app.inst.mods_closed;
    let sources: Vec<&'static modulation::Source> = spec
        .kind
        .sources()
        .iter()
        .filter(|tag| !tag.starts_with("macro"))
        .filter_map(|tag| modulation::source(tag))
        .collect();
    let labels: Vec<String> = sources.iter().map(|src| source_label(src.tag)).collect();
    let chips: Vec<shell::Chip<'_>> = sources
        .iter()
        .zip(&labels)
        .map(|(src, label)| shell::Chip {
            tag: label,
            family: source_family(src.tag),
            count: connections(spec)
                .filter(|c| c.src == src.tag)
                .count()
                .min(255) as u8,
        })
        .collect();
    let cr = kit::with_brush(&ui.painter_at(rect), |b| {
        shell::mod_card(b, rect, s, &chips, open)
    });
    if ui
        .interact(
            cr.title,
            ui.id().with(("mod-title", track.0)),
            egui::Sense::click(),
        )
        .clicked()
    {
        app.inst.mods_closed = open;
    }
    if !open {
        return;
    }
    // The chips' places, as `mod_card` lays them out.
    let k = s.k;
    let (cw, ch) = (58.0 * k, 20.0 * k);
    let mut at = Pos2::new(rect.left() + 8.0 * k, cr.title.bottom() + 8.0 * k);
    for src in &sources {
        if at.x + cw > rect.right() - 8.0 * k {
            at = Pos2::new(rect.left() + 8.0 * k, at.y + ch + 6.0 * k);
        }
        if at.y + ch > rect.bottom() - 4.0 * k {
            break;
        }
        drag_source(ui, Rect::from_min_size(at, egui::vec2(cw, ch)), src.tag);
        at.x += cw + 6.0 * k;
    }
    let _ = Scope::Voice;
}

/// The keyboard card: C2–C7, held notes lit in the device hue; click to play (lower on a
/// key is louder), drag across to play a run.
fn keyboard_band(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, s: &Style, rect: Rect) {
    let (lo, hi) = KEYBOARD_RANGE;
    let held = app.inst.held.clone();
    kit::with_brush(&ui.painter_at(rect), |b| {
        shell::keyboard_card(b, rect, s, lo, hi, &held)
    });
    let keys = rect.shrink(4.0 * s.k);
    let resp = ui.interact(
        keys,
        ui.id().with(("inst-keys", track.0)),
        egui::Sense::click_and_drag(),
    );
    let pointer = resp.interact_pointer_pos();
    let down = resp.is_pointer_button_down_on();
    let under = pointer.and_then(|p| key_at(keys, p));
    let playing = app.inst.held.first().copied();
    if down && under != playing {
        if let Some(k) = playing {
            app.engine.note_off(track, k);
            app.inst.held.clear();
        }
        if let (Some(k), Some(p)) = (under, pointer) {
            let frac = ((p.y - keys.top()) / keys.height()).clamp(0.0, 1.0);
            app.engine
                .note_on(track, k, (40.0 + 87.0 * frac).round() as u8);
            app.inst.held.push(k);
        }
    } else if !down {
        for k in std::mem::take(&mut app.inst.held) {
            app.engine.note_off(track, k);
        }
    }
}

fn is_black(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

/// The keys shown: C2 to C7.
pub const KEYBOARD_RANGE: (u8, u8) = (36, 96);

/// The key under a point of the keys' rect, black keys first (they sit on top): the same
/// geometry as `kit::shell::keyboard_card`.
pub fn key_at(rect: Rect, pos: Pos2) -> Option<u8> {
    let (lo, hi) = KEYBOARD_RANGE;
    if !rect.contains(pos) {
        return None;
    }
    let whites: Vec<u8> = (lo..=hi).filter(|k| !is_black(*k)).collect();
    let w = rect.width() / whites.len() as f32;
    if pos.y < rect.top() + rect.height() * 0.6 {
        // A black key is centred on the boundary between two whites, 62 % of a white wide.
        let edge = ((pos.x - rect.left()) / w).round();
        let dx = (pos.x - (rect.left() + edge * w)).abs();
        let e = edge as usize;
        if dx < w * 0.31 && e > 0 && e < whites.len() {
            let left_white = whites[e - 1];
            if left_white < hi && is_black(left_white + 1) {
                return Some(left_white + 1);
            }
        }
    }
    let wi = (((pos.x - rect.left()) / w) as usize).min(whites.len() - 1);
    Some(whites[wi])
}

// ---- controls for the editors ---------------------------------------------------------------

/// Write a value the way a musician reads it into `b`, without allocating: `1.2 kHz`,
/// `350 Hz`, `12 ms`, `-3.5 dB`, `+7 st`, `-12 ct`, `50 %`.
pub fn write_readout(v: f32, unit: &str, b: &mut String) {
    let _ = match unit {
        "Hz" if v >= 1_000.0 => write!(b, "{:.2} kHz", v / 1_000.0),
        "Hz" => write!(b, "{v:.0} Hz"),
        "ms" if v >= 1_000.0 => write!(b, "{:.2} s", v / 1_000.0),
        "ms" if v >= 100.0 => write!(b, "{v:.0} ms"),
        "ms" => write!(b, "{v:.1} ms"),
        "s" if v < 1.0 => write!(b, "{:.0} ms", v * 1_000.0),
        "s" => write!(b, "{v:.2} s"),
        "dB" => write!(b, "{v:+.1} dB"),
        "st" => write!(b, "{v:+.1} st"),
        "ct" => write!(b, "{v:+.0} ct"),
        "%" => write!(b, "{:.0} %", v * 100.0),
        "" if v.abs() >= 100.0 => write!(b, "{v:.0}"),
        "" => write!(b, "{v:.2}"),
        u => write!(b, "{v:.2} {u}"),
    };
}

/// The modulation reaching `key`, as the kit's rings: depth and the source's hue.
fn rings_for(spec: &NativeSpec, key: &str, s: &Style) -> Vec<(f32, egui::Color32)> {
    connections(spec)
        .filter(|c| c.dst == key && c.on)
        .map(|c| (c.amount, s.t.hue(source_family(c.src))))
        .collect()
}

/// One number of the instrument as a kit **cell** (value pill, knob, label — ADR 0024 §4):
/// range, taper, unit and label from its description (the label abbreviates through
/// `<label_key>.short`), a drag is one undo step, a modulation source dropped on it
/// connects it, and what modulates it is drawn as rings.
pub fn cell(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    s: &Style,
    size: KnobSize,
) -> egui::Response {
    param(app, ui, track, spec, key, s, size, true)
}

/// As [`cell`], a bare knob with no pill or label (inside a display, a dense row).
pub fn knob(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    s: &Style,
    size: KnobSize,
) -> egui::Response {
    param(app, ui, track, spec, key, s, size, false)
}

/// What Cantus adds to an instrument's knob on parameter `key` (ADR 0027, 0057): read by
/// [`wire`] before the knob is drawn, answered by [`wired`] after. Every instrument
/// editor's knobs go through the pair (review 1), so none misses *Modulate…*, the dropped
/// handle, the Alt-drag depth or the automation menu.
pub struct Wire {
    pub target: cantus_core::AutoTarget,
    /// The key takes automation and modulation at all (`InstrumentKind::modulatable`):
    /// off, the knob is a plain number — no menu, no rings, no clip's value shown.
    pub on: bool,
    /// A clip drives it.
    pub automated: bool,
    pub offer: crate::kit::automate::ModOffer,
    /// How many of the rings are the modulators'.
    pub n: usize,
}

/// The knob's automation and modulation before it is drawn: `v` becomes what a clip gives
/// it where the song is, `rings` the modulators' rings (`Wire::n` of them).
pub fn wire(
    app: &CantusApp,
    track: TrackId,
    key: &str,
    v: &mut f32,
    (min, max, log): (f32, f32, bool),
    s: &Style,
    rings: &mut [(f32, egui::Color32); crate::views::modulator::MAX_RINGS],
) -> Wire {
    let on = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .is_none_or(|s| s.kind.modulatable(key));
    let target = cantus_core::AutoTarget::Instrument {
        track,
        key: key.to_string(),
    };
    if !on {
        // A clip left on it (an instrument swapped out had the key) is inert: the knob
        // shows its own value (sound review 2).
        return Wire {
            target,
            on,
            automated: false,
            offer: Default::default(),
            n: 0,
        };
    }
    let automated = app.automated_value_where(|x| {
        matches!(x, cantus_core::AutoTarget::Instrument { track: t, key: k } if *t == track && k == key)
    });
    if let Some(Some(av)) = automated {
        *v = av;
    }
    let taper = if log {
        crate::widgets::knob::Taper::Log
    } else {
        crate::widgets::knob::Taper::Linear
    };
    let base = crate::widgets::knob::pos_of(*v, min, max, taper);
    let (offer, n) = app.mod_offer(&target, base, s, rings);
    Wire {
        target,
        on,
        automated: automated.is_some(),
        offer,
        n,
    }
}

/// What the knob's menu or modulation asked, answered.
pub fn wired(
    app: &mut CantusApp,
    w: &Wire,
    choice: Option<crate::kit::automate::Choice>,
    rect: egui::Rect,
) {
    use crate::kit::automate::Choice;
    if !w.on {
        return;
    }
    match choice {
        Some(Choice::Automate) => app.automate(w.target.clone()),
        Some(
            c @ (Choice::Modulate
            | Choice::Depth(_)
            | Choice::DepthDone
            | Choice::Connect(_)
            | Choice::Learn
            | Choice::Forget),
        ) => app.modulation_choice(&w.target, c, rect),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn param(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    s: &Style,
    size: KnobSize,
    as_cell: bool,
) -> egui::Response {
    let Some(info) = spec.kind.param(key) else {
        return ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
    };
    let mut v = spec.get(key);
    // A stepped parameter keeps its fraction while dragged: the project stores the rounded
    // value, and re-reading it each frame would round a slow drag back to where it began.
    let pending = egui::Id::new(("inst-step", track.0, key));
    if info.stepped {
        if let Some(f) = ui.data(|d| d.get_temp::<f32>(pending)) {
            v = f;
        }
    }
    let mut rings = rings_for(spec, key, s);
    let name = t(info.label_key);
    let unit = info.unit;
    // Automated (ADR 0027) and modulated (ADR 0057): the clip's value where the song is,
    // the modulators' rings after the instrument's own, the menu.
    let mut mod_rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let w = wire(
        app,
        track,
        key,
        &mut v,
        (info.min, info.max, info.logarithmic),
        s,
        &mut mod_rings,
    );
    rings.extend_from_slice(&mod_rings[..w.n]);
    let mut p = Param::new(&mut v, info.min..=info.max, info.default)
        .size(size)
        .log(info.logarithmic)
        .name(&name)
        .style(*s)
        .rings(&rings)
        // A key that is set, never automated or modulated (Chorda's tuning) offers neither.
        .offers(w.on, w.automated, w.offer);
    if as_cell {
        p = p.cell(Src::Key(info.label_key));
    }
    let (r, choice) = p
        .format(move |v, b| write_readout(v, unit, b))
        .show_full(ui);
    wired(app, &w, choice, r.rect);
    if info.stepped {
        if r.dragged() {
            ui.data_mut(|d| d.insert_temp(pending, v));
        } else {
            ui.data_mut(|d| d.remove::<f32>(pending));
        }
    }
    if r.changed() {
        let v = if info.stepped { v.round() } else { v };
        if !info.stepped || spec.get(key) != v {
            app.set_inst_value(track, key, v, true);
        }
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    let takes = spec.kind.modulatable(key);
    if takes
        && egui::DragAndDrop::has_payload_of_type::<&'static str>(ui.ctx())
        && r.contains_pointer()
    {
        ui.painter().rect_stroke(
            r.rect,
            4.0 * s.k,
            egui::Stroke::new(1.5, s.hue),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(src) = r.dnd_release_payload::<&'static str>().filter(|_| takes) {
        app.connect_modulation(track, *src, key);
    }
    r
}

/// The colour of a modulation source: its family's hue (the chip's bar, the rings).
pub fn source_color(tag: &str) -> egui::Color32 {
    Style::current().t.hue(source_family(tag))
}

/// A source's short label: the tag, as the chips and rings print it (`LFO1`, `VEL`).
pub fn source_label(tag: &str) -> String {
    tag.to_uppercase()
}

/// The house knob of rounds 15–17, kept while the editors move onto the kit (ADR 0024
/// Phase B): a knob bound to one number, with modulation rings and a drop target.
pub fn param_knob(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
) -> egui::Response {
    param_knob_as(app, ui, track, spec, key, None, grid::GRID * 11.0)
}

/// [`param_knob`] with a label of its own and a diameter.
pub fn param_knob_as(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    label: Option<&str>,
    diameter: f32,
) -> egui::Response {
    let Some(info) = spec.kind.param(key) else {
        return ui.label(key);
    };
    let mut v = spec.get(key);
    let label = label
        .map(str::to_string)
        .unwrap_or_else(|| t(info.label_key));
    let mut knob = Knob::new(&mut v, info.min..=info.max, info.default)
        .diameter(diameter)
        .log(info.logarithmic)
        .stepped(info.stepped)
        .unit(info.unit)
        .label(label);
    for c in connections(spec).filter(|c| c.dst == key && c.on) {
        knob = knob.ring(c.amount, source_color(c.src));
    }
    let r = ui.add(knob);
    if r.changed() {
        app.set_inst_value(track, key, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    if let Some(src) = r.dnd_release_payload::<&'static str>() {
        app.connect_modulation(track, *src, key);
    }
    r
}

/// The generic Edit view: a cell per parameter.
pub fn knobs_for_every_param(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
) {
    ui.horizontal_wrapped(|ui| {
        for info in spec.kind.params() {
            cell(app, ui, track, spec, info.key, s, KnobSize::M);
        }
    });
}

/// The generic Play view: the instrument's first parameters as cells.
pub fn default_play(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
) {
    ui.horizontal_wrapped(|ui| {
        for info in spec.kind.params().iter().take(8) {
            cell(app, ui, track, spec, info.key, s, KnobSize::L);
        }
    });
}

/// The generic inspector picture: the name and how much has moved from the defaults.
pub fn default_preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    ui.label(
        RichText::new(tf(
            "inst.preview",
            &[
                ("name", &t(spec.kind.name_key())),
                ("n", &spec.params.len().to_string()),
            ],
        ))
        .text_style(design::caption()),
    )
}

// ---- the matrix ----------------------------------------------------------------------------

const CURVES: [&str; 5] = [
    "mod.curve.linear",
    "mod.curve.exp",
    "mod.curve.log",
    "mod.curve.s",
    "mod.curve.steps",
];

/// Every connection as a row of kit parts — source ‹ › , target ‹ ›, depth, curve ‹ ›,
/// on, remove — and a button that adds one.
pub fn matrix(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
) {
    use crate::kit::widgets::{led_button, menu_field, readout};
    use modulation::SLOTS;
    let rows: Vec<_> = connections(spec)
        .map(|c| {
            (
                c.slot,
                c.src.to_string(),
                c.dst.to_string(),
                c.amount,
                c.curve as usize,
                c.on,
            )
        })
        .collect();
    let sources = spec.kind.sources();
    // Only what a source can move: Chorda's tuning and its model are no targets
    // (integration review).
    let targets: Vec<&cantus_core::inst::InstParamInfo> = spec
        .kind
        .params()
        .iter()
        .filter(|p| spec.kind.modulatable(p.key))
        .collect();
    let s = &s.function(Function::Mod);
    for (slot, src, dst, amount, curve, on) in rows {
        let k = SLOTS[slot - 1];
        ui.horizontal(|ui| {
            readout(ui, &slot.to_string(), true, &t("mod.slot"), 28.0);
            let (_, c) = menu_field(
                ui,
                &source_label(&src),
                true,
                false,
                &t("mod.source"),
                120.0,
            );
            if c.prev || c.next || c.open {
                let at = sources.iter().position(|x| *x == src).unwrap_or(0) as i32;
                let n = sources.len().max(1) as i32;
                let i = (at + if c.prev { -1 } else { 1 }).rem_euclid(n) as usize;
                if let Some(tag) = sources.get(i) {
                    app.set_inst_text(track, k.src, Some(tag.to_string()));
                }
            }
            let target = spec
                .kind
                .param(&dst)
                .map(|i| t(i.label_key))
                .unwrap_or(dst.clone());
            let (_, c) = menu_field(ui, &target, true, false, &t("mod.target"), 200.0);
            if c.prev || c.next || c.open {
                let at = targets.iter().position(|x| x.key == dst).unwrap_or(0) as i32;
                let n = targets.len().max(1) as i32;
                let i = (at + if c.prev { -1 } else { 1 }).rem_euclid(n) as usize;
                if let Some(info) = targets.get(i) {
                    app.set_inst_text(track, k.dst, Some(info.key.to_string()));
                }
            }
            let mut depth = amount;
            let name = t("mod.amount");
            // A slot's depth is a parameter too: automated and modulated (ADR 0057).
            let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
            let w = wire(
                app,
                track,
                k.amt,
                &mut depth,
                (-1.0, 1.0, false),
                s,
                &mut rings,
            );
            let (r, choice) = Param::new(&mut depth, -1.0..=1.0, 0.0)
                .size(KnobSize::S)
                .name(&name)
                .style(*s)
                .rings(&rings[..w.n])
                .offers(w.on, w.automated, w.offer)
                .format(|v, b| {
                    let _ = write!(b, "{:+.0} %", v * 100.0);
                })
                .show_full(ui);
            wired(app, &w, choice, r.rect);
            if r.changed() {
                app.set_inst_value(track, k.amt, depth, true);
            }
            if r.drag_stopped() || r.double_clicked() {
                app.end_gesture();
            }
            let (cresp, c) = menu_field(
                ui,
                &t(CURVES[curve.min(4)]),
                true,
                false,
                &t("mod.curve"),
                130.0,
            );
            // The slot's curve and switch are parameters: their menus, their marks, the
            // learn mode (round 29, review 2 of ADR 0062, #44).
            let learning = crate::controllers::stepped_inst(app, ui, track, spec, k.curve, &cresp);
            if (c.prev || c.next || c.open) && !learning {
                let i = (curve as i32 + if c.prev { -1 } else { 1 }).rem_euclid(5);
                app.set_inst_value(track, k.curve, i as f32, false);
            }
            let mut live = on;
            let lresp = led_button(ui, &mut live, Src::Key("mod.on"), &t("mod.on"), 56.0);
            let learning = crate::controllers::stepped_inst(app, ui, track, spec, k.on, &lresp);
            if lresp.changed() && !learning {
                app.set_inst_value(track, k.on, if live { 1.0 } else { 0.0 }, false);
            }
            if readout(ui, "\u{d7}", false, &t("mod.remove"), 24.0).clicked() {
                app.remove_modulation(track, slot);
            }
        });
    }
    let first_source = sources.iter().find(|x| !x.starts_with("macro"));
    if let (Some(src), Some(dst)) = (first_source, targets.first()) {
        let mut add = false;
        if led_button(ui, &mut add, Src::Key("mod.add"), &t("mod.add"), 140.0).clicked() {
            app.connect_modulation(track, src, dst.key);
        }
    }
    if connections(spec).next().is_none() {
        ui.label(
            RichText::new(t("mod.empty"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
    }
}

/// The inspector strip for a native instrument: its picture, the button that opens the
/// editor, and the first four macros.
pub fn inspector_strip(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec) {
    preview(ui, spec);
    if ui
        .button(t("inst.open_editor"))
        .on_hover_text(t("inst.open_editor_tip"))
        .clicked()
    {
        app.open_inst_editor(track);
    }
    ui.horizontal_wrapped(|ui| {
        for key in MACRO_KEYS.iter().take(4) {
            param_knob(app, ui, track, spec, key);
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::CantusApp;
    use cantus_core::inst::modulation::{connections, SLOTS};
    use cantus_core::*;
    use cantus_engine::EngineHandle;

    /// Sound review 2: an `a4` automation clip an Organ left on its track is inert once
    /// the track plays Chorda — the A4 knob shows its own value and offers nothing; and a
    /// swap carries no source onto Chorda's tuning.
    #[test]
    fn chordas_tuning_knob_ignores_an_inert_clip_and_a_swap_brings_it_no_source() {
        let mut p = crate::starter_project();
        let track = p.tracks[0].id;
        p.tracks[0].instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda));
        let mut clip = p.make_clip("a4", Ticks::ZERO, Ticks::beats(64));
        clip.automation = Some(Automation::new(
            AutoTarget::Instrument {
                track,
                key: "a4".into(),
            },
            vec![AutoPoint::new(Ticks::ZERO, 1.0)],
        ));
        p.tracks[0].clips.push(clip);
        let a = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
        let mut v = 440.0;
        let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
        let w = super::wire(
            &a,
            track,
            "a4",
            &mut v,
            (415.0, 466.0, false),
            &crate::kit::Style::current(),
            &mut rings,
        );
        assert_eq!(v, 440.0, "the clip's value is not shown");
        assert!(!w.on && !w.automated);
        // An Organ whose LFO drives its A4 (and its drive), swapped to Chorda.
        let mut organ = NativeSpec::new(InstrumentKind::Organ);
        let src = InstrumentKind::Organ.sources()[0];
        for (slot, dst) in [(0usize, "a4"), (1, "volume")] {
            let k = &SLOTS[slot];
            organ.text.insert(k.src.into(), src.into());
            organ.text.insert(k.dst.into(), dst.into());
            organ.set(k.amt, 0.25);
        }
        let (chorda, _) = crate::swap::carry_over(&organ, InstrumentKind::Chorda);
        assert!(
            connections(&chorda).all(|c| c.dst != "a4"),
            "no source on Chorda's A4"
        );
    }
}
