//! Round 20, device-ux: device windows that are easy to close, minimise and swap, and the
//! places that open the device picker. Every gesture goes through published geometry —
//! the window buttons' rects the registry keeps, the spots the views publish, the mixer's
//! `control_rect` — never a copy of the layout.

mod support;
use cantus_core::*;
use cantus_ui::kit::window_ctl::WindowRects;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::views::device_chrome::{self as chrome, Spot};
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_device as device;
use cantus_ui::CantusApp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect};
use support::*;

/// The whole window, frame by frame, with a pointer that stays where it was moved.
struct Win {
    ctx: egui::Context,
    app: CantusApp,
    size: egui::Vec2,
    frame: u64,
    pointer: Option<Pos2>,
    _dir: tempfile::TempDir,
}

impl Win {
    fn new(project: Project) -> Win {
        let mut a = app(project);
        let dir = tempfile::tempdir().unwrap();
        // Never write to the user's real configuration directory from a test.
        a.settings_path = dir.path().join("settings.json");
        a.layouts_path = dir.path().join("layouts.json");
        a.keymap_path = dir.path().join("keymap.toml");
        a.appearance_path = dir.path().join("appearance.json");
        let ctx = ctx();
        a.attach(&ctx);
        let mut w = Win {
            ctx,
            app: a,
            size: egui::vec2(1600.0, 1000.0),
            frame: 0,
            pointer: None,
            _dir: dir,
        };
        w.idle(3);
        w
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.frame += 1;
        let mut ev = Vec::new();
        if let Some(p) = self.pointer {
            ev.push(Event::PointerMoved(p));
        }
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events: ev,
            ..Default::default()
        };
        let app = &mut self.app;
        self.ctx
            .run_ui(input, |ui| app.frame(ui))
            .textures_delta
            .clear();
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn hover(&mut self, p: Pos2) {
        self.pointer = Some(p);
        self.idle(2);
    }

    fn click_with(&mut self, p: Pos2, button: PointerButton) {
        self.hover(p);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: p,
                button,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
        self.idle(2);
    }

    fn click(&mut self, p: Pos2) {
        self.click_with(p, PointerButton::Primary);
    }

    fn key(&mut self, key: Key) {
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        self.idle(2);
    }

    fn buttons(&self, key: u64) -> WindowRects {
        device::buttons(&self.ctx, key).expect("the device drew its window buttons")
    }

    fn spot(&self, s: Spot) -> Rect {
        chrome::spot(&self.ctx, s).unwrap_or_else(|| panic!("{s:?} was not drawn"))
    }
}

/// A window on the welcome project with track 0 hosting Calor, its editor open.
fn calor_open() -> (Win, TrackId, u64) {
    let mut w = Win::new(cantus_ui::welcome_project());
    let track = w.app.project().tracks[0].id;
    w.app.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Analog)),
    );
    w.app.select_track(track);
    w.app.open_inst_editor(track);
    w.idle(4);
    let key = w.app.instrument_window_key(track).unwrap();
    assert!(w.app.workspace.contains(Pane::Device(key)), "the editor has its tab");
    (w, track, key)
}

#[test]
fn an_instrument_window_closes_with_one_click_on_its_cross() {
    let (mut w, _, key) = calor_open();
    let b = w.buttons(key);
    let close = b.close.expect("a close button");
    w.click(close.center());
    assert_eq!(w.app.inst.open, None, "the editor closed");
    w.idle(2);
    assert!(!w.app.workspace.contains(Pane::Device(key)), "its tab went");
}

#[test]
fn minimise_puts_the_device_in_the_tray_and_a_click_brings_it_back() {
    let (mut w, track, key) = calor_open();
    let min = w.buttons(key).minimise.expect("a minimise button");
    w.click(min.center());
    assert_eq!(w.app.inst.open, Some(track), "still open, only minimised");
    assert!(device::is_minimised(&w.ctx, key));
    assert!(!w.app.workspace.contains(Pane::Device(key)), "off the screen");
    // Its chip in the status bar, saying what it is.
    let mut items = Vec::new();
    device::minimised(&w.ctx, &mut items);
    assert_eq!(items.len(), 1);
    assert!(items[0].1.contains("Calor"), "the chip: {}", items[0].1);
    let chip = w.spot(Spot::TrayChip(key));
    assert!(chip.bottom() > w.size.y - 30.0, "in the status bar: {chip:?}");
    w.click(Pos2::new(chip.left() + 12.0, chip.center().y));
    assert!(!device::is_minimised(&w.ctx, key));
    w.idle(2);
    assert!(w.app.workspace.contains(Pane::Device(key)), "back in the dock");
}

#[test]
fn the_tray_cross_closes_a_minimised_device() {
    let (mut w, _, key) = calor_open();
    w.click(w.buttons(key).minimise.unwrap().center());
    w.click(w.spot(Spot::TrayClose(key)).center());
    assert_eq!(w.app.inst.open, None);
}

#[test]
fn an_instrument_too_tall_for_its_leaf_floats_at_its_own_size() {
    let (w, _, key) = calor_open();
    assert!(
        w.app.workspace.is_floating(Pane::Device(key)),
        "the arrangement's half of a 1000-pt window cannot hold an instrument"
    );
    let (whole, _) = device::shown(&w.ctx, key).unwrap();
    let (size, _) = device::sizes(&w.ctx, key).unwrap();
    assert!(
        (whole.height() - size.y).abs() < 60.0,
        "drawn near its own height: {whole:?} for {size:?}"
    );
}

#[test]
fn maximise_fills_the_workspace_and_restore_puts_it_back() {
    let (mut w, _, key) = calor_open();
    let before = device::shown(&w.ctx, key).unwrap().0;
    w.click(w.buttons(key).maximise.unwrap().center());
    w.idle(3);
    assert!(device::is_maximised(&w.ctx, key));
    let big = device::shown(&w.ctx, key).unwrap().0;
    assert!(
        big.width() > before.width() && big.height() > before.height(),
        "{big:?} over {before:?}"
    );
    w.click(w.buttons(key).maximise.unwrap().center());
    w.idle(3);
    assert!(!device::is_maximised(&w.ctx, key));
    let back = device::shown(&w.ctx, key).unwrap().0;
    assert!((back.min - before.min).length() < 2.0, "{back:?} vs {before:?}");
    // Docked by the user, a maximised device docks again when restored.
    w.app.workspace.toggle_float(Pane::Device(key));
    w.idle(3);
    assert!(!w.app.workspace.is_floating(Pane::Device(key)));
    w.click(w.buttons(key).maximise.unwrap().center());
    w.idle(3);
    assert!(w.app.workspace.is_floating(Pane::Device(key)));
    w.click(w.buttons(key).maximise.unwrap().center());
    w.idle(3);
    assert!(
        !w.app.workspace.is_floating(Pane::Device(key)),
        "it was docked: it docks again"
    );
}

#[test]
fn esc_closes_the_device_clicked_last() {
    let (mut w, _, key) = calor_open();
    // A click on the window's own ground: its footer, at the left.
    let (whole, _) = device::shown(&w.ctx, key).unwrap();
    w.click(Pos2::new(whole.left() + 30.0, whole.bottom() - 8.0));
    assert_eq!(device::focused(&w.ctx), Some(key));
    w.key(Key::Escape);
    assert_eq!(w.app.inst.open, None, "Esc closed it");
}

#[test]
fn f8_opens_and_closes_the_selected_tracks_instrument() {
    let mut w = Win::new(cantus_ui::welcome_project());
    // Track 1 of the welcome project is a kit: its window is Pulsus's.
    let kit = w.app.project().tracks[1].id;
    w.app.select_track(kit);
    w.idle(1);
    w.key(Key::F8);
    assert_eq!(w.app.pulsus.open, Some(kit));
    w.key(Key::F8);
    assert_eq!(w.app.pulsus.open, None);
}

/// A window on the starter project with a Comprimo on track 0's strip, its editor open.
fn effect_open(kind: EffectKind) -> (Win, StripId, EffectId, u64) {
    let mut w = Win::new(cantus_ui::starter_project());
    let strip = w.app.project().tracks[0].strip.unwrap();
    w.app.add_effect(strip, kind);
    let fx = w.app.project().strip(strip).unwrap().effects[0].id;
    w.app.open_effect_editor(strip, fx);
    w.idle(4);
    let key = cantus_ui::views::fx::device_id(fx).value();
    (w, strip, fx, key)
}

#[test]
fn an_effect_window_has_the_same_buttons_and_they_work() {
    let (mut w, strip, fx, key) = effect_open(EffectKind::Gate);
    // Minimise: still open, not drawn, a chip in the tray.
    w.click(w.buttons(key).minimise.unwrap().center());
    assert!(w.app.fx.open.is_some());
    assert!(device::is_minimised(&w.ctx, key));
    let chip = w.spot(Spot::TrayChip(key));
    w.click(Pos2::new(chip.left() + 12.0, chip.center().y));
    assert!(!device::is_minimised(&w.ctx, key));
    // Maximise: over the workspace; restore: back where it was.
    let before = device::shown(&w.ctx, key).unwrap().0;
    w.click(w.buttons(key).maximise.unwrap().center());
    let big = device::shown(&w.ctx, key).unwrap().0;
    assert!(big.width() > before.width(), "{big:?} over {before:?}");
    w.click(w.buttons(key).maximise.unwrap().center());
    let back = device::shown(&w.ctx, key).unwrap().0;
    assert!((back.min - before.min).length() < 1.0, "{back:?} vs {before:?}");
    // Close.
    w.click(w.buttons(key).close.unwrap().center());
    assert_eq!(w.app.fx.open, None);
    assert!(w.app.project().effect(strip, fx).is_some(), "closing keeps the effect");
}

#[test]
fn the_swap_button_opens_the_picker_for_the_device_in_place() {
    let (mut w, strip, fx, key) = effect_open(EffectKind::Limiter);
    w.click(w.buttons(key).swap.unwrap().center());
    assert_eq!(
        w.app.picker,
        Some(PickPurpose::ReplaceEffect { strip, effect: fx })
    );
    let (mut w, track, key) = calor_open();
    w.click(w.buttons(key).swap.unwrap().center());
    assert_eq!(w.app.picker, Some(PickPurpose::ReplaceInstrument { track }));
}

#[test]
fn an_effect_reopens_where_it_floated() {
    let (mut w, strip, fx, key) = effect_open(EffectKind::Stereo);
    let (_, header) = device::shown(&w.ctx, key).unwrap();
    // Drag the header's background (left of the name) by (120, 60).
    let from = Pos2::new(header.left() + 60.0, header.center().y);
    let to = from + egui::vec2(120.0, 60.0);
    w.hover(from);
    w.frame(vec![Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    for i in 1..=6 {
        w.pointer = Some(from + (to - from) * (i as f32 / 6.0));
        w.frame(Vec::new());
    }
    w.frame(vec![Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    w.idle(2);
    let moved = device::shown(&w.ctx, key).unwrap().0;
    w.app.close_fx_editor();
    w.idle(4);
    // Another editor elsewhere moves the shared default; this one still knows its place.
    w.app.open_effect_editor(strip, fx);
    w.idle(4);
    let again = device::shown(&w.ctx, key).unwrap().0;
    assert!((again.min - moved.min).length() < 1.0, "{again:?} vs {moved:?}");
}

// ---- apply_pick: what a swap keeps -------------------------------------------------------

fn compiled_pitches(a: &CantusApp, track: TrackId) -> Vec<u8> {
    let snap = Snapshot::compile(a.project(), 0);
    let i = a.project().tracks.iter().position(|t| t.id == track).unwrap();
    let mut v: Vec<u8> = snap.tracks[i].events.iter().map(|e| e.pitch).collect();
    v.sort_unstable();
    v
}

#[test]
fn swapping_an_instrument_keeps_the_track_and_is_one_undo_step() {
    let mut a = app(cantus_ui::welcome_project());
    let t = a.project().tracks[0].clone();
    let before = a.project().clone();
    let pitches = compiled_pitches(&a, t.id);
    let history = a.bus.history().len();
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track: t.id },
        PickChoice::Instrument(InstrumentKind::Wavetable),
    );
    let now = a.project().track(t.id).unwrap().clone();
    assert_eq!(now.instrument.native().map(|n| n.kind), Some(InstrumentKind::Wavetable));
    assert_eq!(now.clips, t.clips, "clips and notes stay");
    assert_eq!((now.strip, now.color), (t.strip, t.color), "strip and colour stay");
    assert_eq!(compiled_pitches(&a, t.id), pitches, "it plays the same notes");
    assert_eq!(a.bus.history().len(), history + 1, "one step");
    // "Keys" is a name the user (or the welcome song) gave it: it stays.
    assert_eq!(now.name, t.name);
    a.undo();
    assert_eq!(*a.project(), before, "one undo puts the old instrument back");
}

#[test]
fn a_default_name_follows_the_new_instrument() {
    let mut a = app(cantus_ui::starter_project());
    let id = a.project().tracks[0].id;
    assert!(a.project().tracks[0].name.starts_with("Track"));
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track: id },
        PickChoice::Instrument(InstrumentKind::Analog),
    );
    assert_eq!(a.project().track(id).unwrap().name, "Calor");
    // Named after its instrument: a further swap renames it again.
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track: id },
        PickChoice::Pulsus,
    );
    assert_eq!(a.project().track(id).unwrap().name, "Pulsus");
}

#[test]
fn a_kit_swapped_for_a_synth_keeps_every_note() {
    let mut a = app(cantus_ui::welcome_project());
    let kit = a.project().tracks[1].id;
    assert!(a.project().track(kit).unwrap().instrument.pulsus().is_some());
    let pitches = compiled_pitches(&a, kit);
    assert!(!pitches.is_empty());
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track: kit },
        PickChoice::Instrument(InstrumentKind::Bass),
    );
    assert_eq!(compiled_pitches(&a, kit), pitches);
}

#[test]
fn matching_settings_come_along_between_native_instruments() {
    use cantus_core::inst::modulation::{MACRO_KEYS, MACRO_NAME_KEYS, SLOTS};
    let mut a = app(cantus_ui::starter_project());
    let id = a.project().tracks[0].id;
    let mut from = NativeSpec::new(InstrumentKind::Analog);
    from.set(MACRO_KEYS[0], 0.7);
    from.text
        .insert(MACRO_NAME_KEYS[0].to_string(), "Bright".to_string());
    // A connection from velocity (every kind has it) to a key both kinds share, if any.
    let shared = InstrumentKind::Analog
        .params()
        .iter()
        .find(|p| InstrumentKind::Wavetable.param(p.key).is_some() && !p.key.starts_with("macro"))
        .map(|p| p.key);
    if let Some(key) = shared {
        from.text.insert(SLOTS[0].src.to_string(), "vel".to_string());
        from.text.insert(SLOTS[0].dst.to_string(), key.to_string());
        from.set(SLOTS[0].amt, 0.5);
        from.set(SLOTS[0].on, 1.0);
    }
    a.set_instrument(id, InstrumentSpec::Native(from));
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track: id },
        PickChoice::Instrument(InstrumentKind::Wavetable),
    );
    let now = a.project().track(id).unwrap().instrument.native().unwrap().clone();
    assert_eq!(now.kind, InstrumentKind::Wavetable);
    assert_eq!(now.get(MACRO_KEYS[0]), 0.7, "the macro's value");
    assert_eq!(now.text(MACRO_NAME_KEYS[0]), Some("Bright"), "and its name");
    if let Some(key) = shared {
        assert_eq!(now.text(SLOTS[0].dst), Some(key), "the connection");
    }
    // Off: a fresh instrument.
    a.swap_keep.matching = false;
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track: id },
        PickChoice::Instrument(InstrumentKind::Fm),
    );
    let fresh = a.project().track(id).unwrap().instrument.native().unwrap().clone();
    assert!(fresh.params.is_empty() && fresh.text.is_empty(), "{fresh:?}");
}

#[test]
fn swapping_an_effect_keeps_its_place_mix_bypass_and_key() {
    let mut a = app(cantus_ui::starter_project());
    let strip = a.project().tracks[0].strip.unwrap();
    let key = a.project().strips[3].id;
    for k in [EffectKind::Eq, EffectKind::Compressor, EffectKind::Reverb] {
        a.add_effect(strip, k);
    }
    let comp = a.project().strip(strip).unwrap().effects[1].id;
    a.exec(Command::SetEffectMix { strip, effect: comp, mix: 0.35 });
    a.exec(Command::SetEffectBypass { strip, effect: comp, bypass: true });
    a.exec(Command::SetEffectSidechain { strip, effect: comp, source: Some(key) });
    let before = a.project().clone();
    let history = a.bus.history().len();
    a.apply_pick(
        PickPurpose::ReplaceEffect { strip, effect: comp },
        PickChoice::Effect(EffectKind::Gate),
    );
    let chain = &a.project().strip(strip).unwrap().effects;
    let kinds: Vec<EffectKind> = chain.iter().map(|e| e.spec.kind).collect();
    assert_eq!(kinds, [EffectKind::Eq, EffectKind::Gate, EffectKind::Reverb]);
    let g = &chain[1];
    assert_eq!((g.mix, g.bypass, g.sidechain), (0.35, true, Some(key)));
    assert_eq!(a.bus.history().len(), history + 1, "one step");
    // A kind with no key drops it; the rest stays.
    let gate = g.id;
    a.apply_pick(
        PickPurpose::ReplaceEffect { strip, effect: gate },
        PickChoice::Effect(EffectKind::Delay),
    );
    let d = &a.project().strip(strip).unwrap().effects[1];
    assert_eq!((d.spec.kind, d.mix, d.sidechain), (EffectKind::Delay, 0.35, None));
    a.undo();
    a.undo();
    // The ids the new slots were given stay handed out; everything else is as it was.
    let mut now = a.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before, "two swaps, two undos");
}

#[test]
fn adding_from_the_picker_names_places_and_opens() {
    let mut a = app(cantus_ui::welcome_project());
    let n = a.project().tracks.len();
    let history = a.bus.history().len();
    a.apply_pick(
        PickPurpose::AddInstrument { index: Some(1) },
        PickChoice::Instrument(InstrumentKind::Organ),
    );
    assert_eq!(a.project().tracks.len(), n + 1);
    let t = &a.project().tracks[1];
    assert_eq!(t.name, "Organum");
    assert_eq!(a.selected_track, Some(t.id));
    assert_eq!(a.inst.open, Some(t.id), "its window opens");
    assert_eq!(a.bus.history().len(), history + 1);
    // A second one is numbered.
    a.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Organ),
    );
    assert_eq!(a.project().tracks.last().unwrap().name, "Organum 2");
    // An effect goes at the end of the chain, its editor open.
    let strip = a.project().tracks[0].strip.unwrap();
    a.apply_pick(
        PickPurpose::AddEffect { strip },
        PickChoice::Effect(EffectKind::Saturator),
    );
    let last = a.project().strip(strip).unwrap().effects.last().unwrap().clone();
    assert_eq!(last.spec.kind, EffectKind::Saturator);
    assert_eq!(a.fx.open.map(|(_, e, _)| e), Some(last.id));
}

#[test]
fn a_choice_that_does_not_fit_changes_nothing() {
    let mut a = app(cantus_ui::welcome_project());
    let before = a.project().clone();
    let track = a.project().tracks[0].id;
    let strip = a.project().tracks[0].strip.unwrap();
    a.open_picker(PickPurpose::ReplaceInstrument { track });
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track },
        PickChoice::Effect(EffectKind::Eq),
    );
    a.apply_pick(
        PickPurpose::AddEffect { strip },
        PickChoice::Instrument(InstrumentKind::Organ),
    );
    let hosted = cantus_host::PluginInfo {
        format: cantus_host::Format::Clap,
        id: "org.example.synth".into(),
        name: "Synth".into(),
        vendor: "Example".into(),
        version: "1".into(),
        path: std::path::PathBuf::new(),
        kind: cantus_host::PluginKind::Instrument,
        main_inputs: 0,
        main_outputs: 2,
        sidechain: false,
        editor: false,
        features: Vec::new(),
    };
    assert!(!cantus_ui::swap::available(
        &PickPurpose::ReplaceInstrument { track },
        &PickChoice::Hosted(Box::new(hosted.clone()))
    ));
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track },
        PickChoice::Hosted(Box::new(hosted)),
    );
    assert_eq!(*a.project(), before);
    assert_eq!(a.picker, None, "the picker closes either way");
}

// ---- entry points: each opens the picker with the right purpose --------------------------

#[test]
fn the_track_headers_instrument_chip_opens_the_picker_to_swap() {
    let mut w = Win::new(cantus_ui::welcome_project());
    let track = w.app.project().tracks[2].id;
    let chip = w.spot(Spot::Chip(track));
    w.click(chip.center());
    assert_eq!(w.app.picker, Some(PickPurpose::ReplaceInstrument { track }));
    assert_eq!(w.app.selected_track, Some(track));
}

#[test]
fn the_add_track_key_opens_the_picker_on_the_instruments() {
    let mut w = Win::new(cantus_ui::welcome_project());
    let n = w.app.project().tracks.len();
    w.click(w.spot(Spot::AddTrack).center());
    assert_eq!(w.app.picker, Some(PickPurpose::AddInstrument { index: None }));
    assert_eq!(w.app.project().tracks.len(), n, "nothing is added until a choice");
}

#[test]
fn the_racks_channel_button_opens_the_instrument_and_its_right_click_swaps_it() {
    let mut w = Win::new(cantus_ui::welcome_project());
    w.app.show_central(cantus_ui::app::CentralView::ChannelRack);
    w.idle(3);
    let kit = w.app.project().tracks[1].id;
    let button = w.spot(Spot::RackButton(kit));
    // Left of centre: the badge sits at the right end while the pointer is on it.
    let at = Pos2::new(button.left() + 10.0, button.center().y);
    w.click(at);
    assert_eq!(w.app.pulsus.open, Some(kit), "a click opens its window (FL)");
    w.click(at);
    assert_eq!(w.app.pulsus.open, None, "and closes it");
    w.click_with(at, PointerButton::Secondary);
    assert_eq!(w.app.picker, Some(PickPurpose::ReplaceInstrument { track: kit }));
    // The badge.
    w.app.picker = None;
    w.hover(at);
    w.click(chrome::badge_rect(button, 1.0).center());
    assert_eq!(w.app.picker, Some(PickPurpose::ReplaceInstrument { track: kit }));
}

#[test]
fn the_inspectors_instrument_field_and_empty_slot_open_the_picker() {
    let mut w = Win::new(cantus_ui::welcome_project());
    let track = w.app.project().tracks[0].id;
    let strip = w.app.project().tracks[0].strip.unwrap();
    w.app.select_track(track);
    w.idle(3);
    w.click(w.spot(Spot::InspectorInstrument(track)).center());
    assert_eq!(w.app.picker, Some(PickPurpose::ReplaceInstrument { track }));
    w.app.picker = None;
    w.app.select_strip(strip);
    w.idle(3);
    w.click(w.spot(Spot::InspectorEmptySlot(0)).center());
    assert_eq!(w.app.picker, Some(PickPurpose::AddEffect { strip }));
}

#[test]
fn the_mixers_slots_open_the_picker_to_add_and_to_swap() {
    use cantus_ui::views::mixer::{self, layout::WidthMode, Control};
    let mut w = Win::new(cantus_ui::welcome_project());
    mixer::set_width_mode(&mut w.app, WidthMode::Xl);
    w.app.show_central(cantus_ui::app::CentralView::Mixer);
    w.idle(3);
    let strip = w.app.project().tracks[0].strip.unwrap();
    let slot = mixer::control_rect(&w.ctx, Some(strip), Control::Slot(0)).expect("slot 1");
    w.click(Pos2::new(slot.left() + 12.0, slot.center().y));
    assert_eq!(w.app.picker, Some(PickPurpose::AddEffect { strip }));
    w.app.picker = None;
    w.app.add_effect(strip, EffectKind::Eq);
    let fx = w.app.project().strip(strip).unwrap().effects[0].id;
    w.idle(2);
    let slot = mixer::control_rect(&w.ctx, Some(strip), Control::Slot(0)).unwrap();
    w.hover(Pos2::new(slot.left() + 20.0, slot.center().y));
    w.click(chrome::badge_rect(slot, 1.0).center());
    assert_eq!(
        w.app.picker,
        Some(PickPurpose::ReplaceEffect { strip, effect: fx })
    );
    assert_eq!(w.app.eq_editor.open, None, "the badge does not open the editor");
}
