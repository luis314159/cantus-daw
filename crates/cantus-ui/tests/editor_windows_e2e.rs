//! Editor windows that behave (round 42, ADR 0161), end to end: the app `CantusApp::assemble`
//! puts together, on a real processor driven block by block, frames fed as egui-winit feeds
//! them, every gesture through published geometry — the window buttons the registry keeps,
//! the keyboard cards' keys, the dock's leaves.
//!
//! - Closing an editor brings back the view that was in front before it opened: the
//!   maximised mixer (where egui_dock alone showed the channel rack), the session view
//!   (where it showed the arrangement) — and never yanks the user back to a view they left.
//! - A pinned instrument stays on screen when an effect opens, when the arrangement is
//!   clicked, and through an Esc aimed elsewhere; Esc closes the editor clicked last.
//! - Two instruments open at once each play their own held keys, and each sends its notes
//!   off when it closes — heard on each track's meter.
//! - Deleting a track whose editor is pinned closes it; undoing the deletion brings the track
//!   back and no editor.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec, Project, TrackId};
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::{CentralView, Files};
use cantus_ui::views::device_chrome::{self as chrome, Spot};
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_device as device;
use cantus_ui::CantusApp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, Vec2};
use std::path::Path;

const RATE: f64 = 48_000.0;
/// Ctrl as egui-winit reports it on Windows and Linux: `ctrl` and `command` both.
const CTRL: Modifiers = Modifiers {
    alt: false,
    ctrl: true,
    shift: false,
    mac_cmd: false,
    command: true,
};
/// One frame of the window, in samples: 60 frames a second.
const FRAME: usize = 800;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        // A configured copy: no first-run audio setup.
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
        d
    })
    .path()
}

struct Rig {
    app: CantusApp,
    proc_: Processor,
    ctx: egui::Context,
    size: Vec2,
    t: f64,
    pointer: Option<Pos2>,
    /// The modifiers held, as egui-winit reports them with every frame.
    mods: Modifiers,
}

fn assembled(project: Project, size: Vec2) -> Rig {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, proc_) = EngineHandle::manual(RATE);
    let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
    let mut r = Rig {
        app,
        proc_,
        ctx,
        size,
        t: 0.0,
        pointer: None,
        mods: Modifiers::NONE,
    };
    r.idle(4);
    r
}

impl Rig {
    /// One block of audio and one frame, with `events` after the pointer's place.
    fn frame(&mut self, events: Vec<Event>) {
        let mut buf = vec![0f32; FRAME * 2];
        self.proc_.process(&mut buf, 2);
        self.t += 1.0 / 60.0;
        let mut ev = vec![Event::ModifiersChanged(self.mods)];
        ev.extend(self.pointer.map(Event::PointerMoved));
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.t),
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

    fn button(&mut self, p: Pos2, pressed: bool) {
        self.pointer = Some(p);
        self.frame(vec![Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }]);
    }

    fn click(&mut self, p: Pos2) {
        self.pointer = Some(p);
        self.idle(2);
        self.button(p, true);
        self.button(p, false);
        self.idle(3);
    }

    fn key_with(&mut self, key: Key, modifiers: Modifiers) {
        self.mods = modifiers;
        for pressed in [true, false] {
            self.frame(vec![Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            }]);
        }
        self.mods = Modifiers::NONE;
        self.idle(3);
    }

    fn key(&mut self, key: Key) {
        self.key_with(key, Modifiers::NONE);
    }

    /// The tab in front of the leaf holding `pane`.
    fn front_of(&self, pane: Pane) -> Pane {
        let p = self
            .app
            .workspace
            .find(pane)
            .unwrap_or_else(|| panic!("{pane:?} is in the dock"));
        let leaf = self.app.workspace.dock.leaf(p.node_path()).unwrap();
        leaf.tabs[leaf.active.0]
    }

    fn leaf_rect(&self, pane: Pane) -> Rect {
        let p = self.app.workspace.find(pane).unwrap();
        self.app.workspace.dock.leaf(p.node_path()).unwrap().rect
    }

    /// Whether the device `key` was drawn in the last pass or two, not minimised.
    fn drawn(&self, key: u64) -> bool {
        let pass = self.ctx.cumulative_pass_nr();
        device::with_registry(&self.ctx, |r| {
            r.get(key)
                .is_some_and(|e| !e.minimised && !e.closed && pass.saturating_sub(e.seen) <= 2)
        }) && device::shown(&self.ctx, key).is_some()
    }

    fn buttons(&self, key: u64) -> cantus_ui::kit::window_ctl::WindowRects {
        device::buttons(&self.ctx, key).expect("the device drew its window buttons")
    }

    /// The peak of `track`'s strip as the engine metered it last.
    fn level(&self, track: TrackId) -> f32 {
        let i = self
            .app
            .project()
            .tracks
            .iter()
            .position(|t| t.id == track)
            .unwrap();
        self.app.engine.meters().tracks.get(i).copied().unwrap_or(0.0)
    }

    /// The loudest `track` was over `frames` frames.
    fn loudest(&mut self, track: TrackId, frames: usize) -> f32 {
        let mut most = 0f32;
        for _ in 0..frames {
            self.frame(Vec::new());
            most = most.max(self.level(track));
        }
        most
    }
}

/// The welcome project with Calor on its first track and Modulus on its second.
fn two_synths(size: Vec2) -> (Rig, TrackId, TrackId) {
    let mut r = assembled(cantus_ui::welcome_project(), size);
    let a = r.app.project().tracks[0].id;
    let b = r.app.project().tracks[1].id;
    for (t, kind) in [(a, InstrumentKind::Analog), (b, InstrumentKind::Fm)] {
        r.app
            .set_instrument(t, InstrumentSpec::Native(NativeSpec::new(kind)));
    }
    r.idle(3);
    (r, a, b)
}

fn inst_key(track: TrackId) -> u64 {
    cantus_ui::views::inst::editor_id(track).value()
}

// ---- the view comes back ------------------------------------------------------------------

/// The owner's report: an instrument opened over the mixer, closed, left the channel rack in
/// front (egui_dock shows the tab before the one closed). The mixer maximised (its tab's
/// double-click) holds the lower zone's three tabs; F8 opens the selected track's
/// instrument there, docked; its ✕ brings the mixer back — the same layout, the central view
/// the mixer again.
#[test]
fn closing_an_instrument_opened_over_the_mixer_brings_the_mixer_back() {
    let (mut r, a, _) = two_synths(Vec2::new(1600.0, 1000.0));
    r.app.show_central(CentralView::Mixer);
    r.app.workspace.toggle_maximise(Pane::Mixer);
    r.idle(3);
    assert_eq!(r.front_of(Pane::Mixer), Pane::Mixer);
    let nodes = r.app.workspace.dock.main_surface().len();
    r.app.select_track(a);
    r.key(Key::F8);
    let key = inst_key(a);
    assert_eq!(r.app.inst.open, Some(a), "F8 opened Calor");
    assert!(
        !r.app.workspace.is_floating(Pane::Device(key)),
        "the maximised mixer is tall enough: docked over it"
    );
    assert_eq!(r.front_of(Pane::Mixer), Pane::Device(key));
    let close = r.buttons(key).close.expect("a close button");
    r.click(close.center());
    assert_eq!(r.app.inst.open, None, "closed by its ✕");
    assert!(!r.app.workspace.contains(Pane::Device(key)));
    assert_eq!(r.front_of(Pane::Mixer), Pane::Mixer, "the mixer is back");
    assert_eq!(r.app.central, CentralView::Mixer);
    assert_eq!(r.app.workspace.dock.main_surface().len(), nodes, "the same layout");
    // Esc closes it too, the same way.
    r.key(Key::F8);
    let body = r.leaf_rect(Pane::Device(key));
    r.click(body.center());
    r.key(Key::Escape);
    assert_eq!(r.app.inst.open, None, "Esc closed the editor clicked last");
    assert_eq!(r.front_of(Pane::Mixer), Pane::Mixer);
}

/// From the session view: an instrument that floats (the arrangement's half is too short)
/// leaves the session in front when it opens — egui_dock alone brought the arrangement
/// forward — and when it closes. Opened from the arrangement, the arrangement stays.
#[test]
fn an_instrument_opened_over_the_session_or_the_arrangement_leaves_it_in_front() {
    let (mut r, a, _) = two_synths(Vec2::new(1600.0, 1000.0));
    r.app.toggle_pane(Pane::Session);
    r.idle(3);
    assert_eq!(r.front_of(Pane::Arrangement), Pane::Session);
    r.app.select_track(a);
    r.key(Key::F8);
    let key = inst_key(a);
    assert!(r.app.workspace.is_floating(Pane::Device(key)), "it floats");
    assert_eq!(r.front_of(Pane::Arrangement), Pane::Session, "opening kept it");
    r.click(r.buttons(key).close.unwrap().center());
    assert_eq!(r.front_of(Pane::Arrangement), Pane::Session, "closing too");
    // The arrangement in front: the same.
    r.app.workspace.reveal(Pane::Arrangement);
    r.idle(2);
    r.key(Key::F8);
    assert_eq!(r.front_of(Pane::Arrangement), Pane::Arrangement);
    r.click(r.buttons(key).close.unwrap().center());
    assert_eq!(r.front_of(Pane::Arrangement), Pane::Arrangement);
}

/// The rule's other half: a user who brought another view forward while the editor was open
/// is not pulled back to the one it covered when it closes.
#[test]
fn closing_never_pulls_the_user_back_to_a_view_they_left() {
    let (mut r, a, _) = two_synths(Vec2::new(1600.0, 1000.0));
    r.app.show_central(CentralView::Mixer);
    r.app.workspace.toggle_maximise(Pane::Mixer);
    r.idle(3);
    r.app.select_track(a);
    r.key(Key::F8);
    let key = inst_key(a);
    assert_eq!(r.front_of(Pane::Mixer), Pane::Device(key));
    // F7: the piano roll comes forward in the same leaf, the editor behind it.
    r.key(Key::F7);
    assert_eq!(r.front_of(Pane::Mixer), Pane::PianoRoll);
    // The inspector's *Close instrument* (or the track deleted): the editor closes behind.
    r.app.close_instrument_window(a);
    r.idle(3);
    assert!(!r.app.workspace.contains(Pane::Device(key)));
    assert_eq!(r.front_of(Pane::Mixer), Pane::PianoRoll, "where the user went");
}

// ---- pinned editors -----------------------------------------------------------------------

/// Pin Calor; an effect opens beside it and both are on screen; a click in the arrangement
/// leaves both; Esc aimed at nothing closes nothing; Esc after a click in the effect closes
/// the effect, not the pinned instrument; another instrument opening does not replace it.
#[test]
fn a_pinned_instrument_stays_through_an_effect_a_click_elsewhere_and_esc() {
    let (mut r, a, b) = two_synths(Vec2::new(1600.0, 1000.0));
    r.app.select_track(a);
    r.key(Key::F8);
    let ka = inst_key(a);
    let pin = r.buttons(ka).pin.expect("every device window has its pin");
    r.click(pin.center());
    assert!(device::is_pinned(&r.ctx, ka), "pinned by its key");
    assert!(r.app.status.text.contains("pinned"), "and said: {}", r.app.status.text);
    // An effect opens: the mixer's insert slot calls this.
    let strip = r.app.project().tracks[0].strip.unwrap();
    r.app.add_effect(strip, EffectKind::Limiter);
    let fx = r.app.project().strip(strip).unwrap().effects.last().unwrap().id;
    r.app.open_effect_editor(strip, fx);
    r.idle(4);
    let kf = cantus_ui::views::fx::device_id(fx).value();
    assert!(r.drawn(ka) && r.drawn(kf), "both on screen");
    // The pinned one is above the one that opened after it.
    let order: Vec<egui::LayerId> = r.ctx.memory(|m| m.layer_ids().collect());
    let layer = |k: u64| device::with_registry(&r.ctx, |g| g.get(k).and_then(|e| e.layer));
    let at = |l: Option<egui::LayerId>| order.iter().position(|x| Some(*x) == l).unwrap();
    assert!(at(layer(ka)) > at(layer(kf)), "the pinned window stays above");
    // A click in the arrangement, where no editor is.
    let arr = r.leaf_rect(Pane::Arrangement);
    let spot = [
        arr.left_top() + Vec2::new(60.0, 40.0),
        arr.right_top() + Vec2::new(-60.0, 40.0),
        arr.left_bottom() + Vec2::new(60.0, -20.0),
    ]
    .into_iter()
    .find(|p| {
        [ka, kf]
            .iter()
            .all(|k| !device::shown(&r.ctx, *k).is_some_and(|(w, _)| w.contains(*p)))
    })
    .expect("a free spot in the arrangement");
    r.click(spot);
    assert!(r.drawn(ka) && r.drawn(kf), "a click elsewhere hides neither");
    r.key(Key::Escape);
    assert!(r.drawn(ka) && r.drawn(kf), "Esc aimed at no editor closes none");
    // A click in the effect, then Esc: the effect closes, the pinned instrument stays.
    let (fx_rect, _) = device::shown(&r.ctx, kf).unwrap();
    let (inst_rect, _) = device::shown(&r.ctx, ka).unwrap();
    let in_fx = (1..10)
        .flat_map(|i| (1..10).map(move |j| (i, j)))
        .map(|(i, j)| {
            fx_rect.min + Vec2::new(fx_rect.width() * i as f32, fx_rect.height() * j as f32) / 10.0
        })
        .find(|p| !inst_rect.expand(4.0).contains(*p) && p.y > fx_rect.top() + 40.0)
        .expect("part of the effect is not under the instrument");
    r.click(in_fx);
    r.key(Key::Escape);
    assert!(!r.app.effect_editor_open(fx), "Esc closed the effect");
    assert!(r.drawn(ka), "not the pinned instrument");
    // Another instrument opens (F8 on track b): the pinned one is not replaced.
    r.app.select_track(b);
    r.key(Key::F8);
    r.idle(3);
    assert!(r.drawn(ka) && r.drawn(inst_key(b)), "both instruments on screen");
    assert!(r.app.instrument_window_open(a) && r.app.instrument_window_open(b));
    // Unpinned, the next one opened replaces it again (one at a time): Modulus closed and
    // opened again closes Calor.
    r.click(r.buttons(ka).pin.unwrap().center());
    assert!(!device::is_pinned(&r.ctx, ka));
    r.app.close_instrument_window(b);
    r.idle(3);
    r.app.select_track(b);
    r.key(Key::F8);
    r.idle(3);
    assert!(
        r.app.instrument_window_open(b) && !r.app.instrument_window_open(a),
        "one at a time: Calor, no longer pinned, closed for Modulus"
    );
}

/// Settings ▸ Editing ▸ *Several at once*: nothing closes when another editor opens.
#[test]
fn several_at_once_keeps_every_editor_open_without_a_pin() {
    let (mut r, a, b) = two_synths(Vec2::new(2560.0, 1440.0));
    r.app
        .set_editors_open(cantus_ui::editor_windows::EditorsOpen::Several);
    r.app.select_track(a);
    r.key(Key::F8);
    r.app.select_track(b);
    r.key(Key::F8);
    r.idle(3);
    assert!(r.drawn(inst_key(a)) && r.drawn(inst_key(b)));
    let (wa, _) = device::shown(&r.ctx, inst_key(a)).unwrap();
    let (wb, _) = device::shown(&r.ctx, inst_key(b)).unwrap();
    assert!(
        wa.intersect(wb).area() < wa.area() * 0.25,
        "side by side on a wide screen: {wa:?} {wb:?}"
    );
}

// ---- several instruments, each its own notes ----------------------------------------------

/// Two instruments open at once: a key held on Calor's keyboard sounds on Calor's track and
/// not on Modulus's, and the other way round; Esc on the editor whose key is held closes it
/// and its note stops — heard on the tracks' meters, through the real processor.
#[test]
fn two_instruments_open_play_their_own_held_notes_and_close_with_notes_off() {
    let (mut r, a, b) = two_synths(Vec2::new(2560.0, 1440.0));
    r.app.select_track(a);
    r.key(Key::F8);
    r.click(r.buttons(inst_key(a)).pin.unwrap().center());
    r.app.select_track(b);
    r.key(Key::F8);
    r.idle(4);
    assert!(r.drawn(inst_key(a)) && r.drawn(inst_key(b)), "both open");
    let keys = |r: &Rig, t: TrackId| chrome::spot(&r.ctx, Spot::InstKeys(t)).expect("its keys");
    let (ka, kb) = (keys(&r, a), keys(&r, b));
    assert!(!ka.intersects(kb), "two keyboards apart: {ka:?} {kb:?}");
    let quiet = 1e-4;
    assert!(r.loudest(a, 30) < quiet && r.loudest(b, 30) < quiet, "silence first");
    // Hold a key on Calor's keyboard: Calor sounds, Modulus does not.
    let on_a = Pos2::new(ka.center().x, ka.bottom() - ka.height() * 0.2);
    r.button(on_a, true);
    let (la, lb) = (r.loudest(a, 30), r.level(b));
    assert!(la > 1e-3, "Calor plays its held key: {la}");
    assert!(lb < quiet, "Modulus is silent: {lb}");
    r.button(on_a, false);
    assert!(r.loudest(a, 180) > 0.0);
    assert!(r.level(a) < quiet, "Calor's note ended with the key: {}", r.level(a));
    // Hold a key on Modulus's keyboard: Modulus sounds, Calor does not.
    let on_b = Pos2::new(kb.center().x, kb.bottom() - kb.height() * 0.2);
    r.button(on_b, true);
    let lb = r.loudest(b, 30);
    assert!(lb > 1e-3, "Modulus plays its held key: {lb}");
    assert!(r.level(a) < quiet, "Calor is silent: {}", r.level(a));
    // Esc while the key is still held: the editor clicked last closes, its note goes off.
    r.key(Key::Escape);
    assert!(!r.app.instrument_window_open(b), "Esc closed Modulus");
    assert!(r.app.instrument_window_open(a), "not the pinned Calor");
    let _ = r.loudest(b, 240);
    assert!(r.level(b) < quiet, "its held note went off: {}", r.level(b));
    r.button(on_b, false);
    // And Calor, holding a key, closed by its ✕ from the keyboard: notes off too.
    let on_a = Pos2::new(ka.center().x, ka.bottom() - ka.height() * 0.2);
    r.button(on_a, true);
    assert!(r.loudest(a, 30) > 1e-3);
    r.app.close_instrument_window(a);
    let _ = r.loudest(a, 240);
    assert!(r.level(a) < quiet, "closed with its note off: {}", r.level(a));
    r.button(on_a, false);
}

// ---- deleting the track, and its undo -----------------------------------------------------

/// A track whose editor is pinned (and parked beside another) is deleted: its editor closes
/// with nothing left behind — no tab, no registry entry, nothing parked. Ctrl+Z brings the
/// track back and no editor with it; the other editor is untouched throughout.
#[test]
fn deleting_a_track_closes_its_pinned_editor_and_undo_brings_no_editor_back() {
    let (mut r, a, b) = two_synths(Vec2::new(1600.0, 1000.0));
    r.app.select_track(a);
    r.key(Key::F8);
    let ka = inst_key(a);
    r.click(r.buttons(ka).pin.unwrap().center());
    r.app.select_track(b);
    r.key(Key::F8);
    r.idle(3);
    assert_eq!(r.app.editors.parked_instruments(), vec![a], "Calor parked, pinned");
    // The track header's *Delete track*.
    r.app.delete_track(a);
    r.idle(4);
    assert!(r.app.project().track(a).is_none());
    assert!(!r.app.instrument_window_open(a), "its editor closed");
    assert!(r.app.editors.parked_instruments().is_empty(), "nothing parked");
    assert!(!r.app.workspace.contains(Pane::Device(ka)), "no tab");
    assert!(
        device::with_registry(&r.ctx, |g| g.get(ka).is_none()),
        "no registry entry"
    );
    assert!(r.drawn(inst_key(b)), "the other editor stays");
    // Undo: the track is back, its editor is not.
    let step = r.app.bus.undo_label_key().map(str::to_string);
    r.key_with(Key::Z, CTRL);
    assert!(
        r.app.project().track(a).is_some(),
        "Ctrl+Z undid {step:?}: the track is back"
    );
    r.idle(4);
    assert!(!r.app.instrument_window_open(a), "and no editor with it");
    assert!(!r.app.workspace.contains(Pane::Device(ka)));
    assert!(r.app.editors.parked_instruments().is_empty());
    assert!(r.drawn(inst_key(b)));
    // The same with the pinned editor in its family's slot (not parked).
    r.app.close_instrument_window(b);
    r.app.select_track(a);
    r.key(Key::F8);
    r.click(r.buttons(ka).pin.unwrap().center());
    assert_eq!(r.app.inst.open, Some(a));
    r.app.delete_track(a);
    r.idle(4);
    assert_eq!(r.app.inst.open, None);
    r.key_with(Key::Z, CTRL);
    r.idle(4);
    assert!(r.app.project().track(a).is_some() && r.app.inst.open.is_none());
    assert!(!r.app.workspace.contains(Pane::Device(ka)));
}
