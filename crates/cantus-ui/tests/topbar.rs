//! The top bar's behaviour (round 28, ADR 0052), driven through the whole window: tap
//! tempo, the time signature's editor, PAT / SONG, the refused recording, the project's
//! name and the window title (through the boot path), the snap of the focused editor, the
//! metronome's voice, the overflow menu, customising the bar, the meter's mode, the MIDI
//! lamp, the RAM readout, and what left the status bar.

use cantus_core::{ClickSound, Command, Ticks, TimeSignature};
use cantus_engine::EngineHandle;
use cantus_ui::keymap::{Action, Keymap};
use cantus_ui::views::transport::{self, layout::Group};
use cantus_ui::{welcome_project, CantusApp};
use egui::{pos2, vec2, Event, Modifiers, Pos2, Rect};

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    size: egui::Vec2,
    frame: u64,
    pointer: Option<Pos2>,
    texts: Vec<(String, Rect)>,
    commands: Vec<egui::ViewportCommand>,
    _dir: tempfile::TempDir,
}

impl Rig {
    fn new() -> Rig {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let dir = tempfile::tempdir().unwrap();
        let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
        app.settings_path = dir.path().join("settings.json");
        app.layouts_path = dir.path().join("layouts.json");
        app.keymap_path = dir.path().join("keymap.toml");
        app.appearance_path = dir.path().join("appearance.json");
        let mut r = Rig {
            ctx,
            app,
            size: vec2(2560.0, 900.0),
            frame: 0,
            pointer: None,
            texts: Vec::new(),
            commands: Vec::new(),
            _dir: dir,
        };
        r.idle(3);
        r
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
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        self.texts.clear();
        for c in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &c.shape {
                self.texts
                    .push((t.galley.text().to_string(), t.visual_bounding_rect()));
            }
        }
        for v in out.viewport_output.values() {
            self.commands.extend(v.commands.iter().cloned());
        }
        out.textures_delta.clear();
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn button(at: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn click(&mut self, at: Pos2) {
        self.idle(30);
        self.pointer = Some(at);
        self.idle(2);
        self.frame(vec![Rig::button(at, true)]);
        self.frame(vec![Rig::button(at, false)]);
        self.idle(2);
    }

    /// Where `word` was painted last frame (the first match).
    fn text(&self, word: &str) -> Option<Rect> {
        self.texts.iter().find(|(t, _)| t == word).map(|(_, r)| *r)
    }

    /// The bar's span of a group in screen points.
    fn group(&self, g: Group) -> Option<Rect> {
        let (bar, l) = transport::last_layout(&self.ctx)?;
        let s = l.span_of(g)?;
        Some(Rect::from_min_max(
            pos2(bar.left() + s.x0, bar.top()),
            pos2(bar.left() + s.x1, bar.bottom()),
        ))
    }

    fn painted(&self) -> String {
        self.texts
            .iter()
            .map(|(t, _)| t.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

#[test]
fn tap_tempo_takes_the_average_and_one_run_is_one_undo_step() {
    let mut r = Rig::new();
    let before = r.app.project().tempo_bpm;
    let a = &mut r.app;
    // A run at 100 BPM (0.6 s), ended by a pause.
    for i in 0..4 {
        transport::tap_at(a, 10.0 + 0.6 * i as f64);
    }
    assert_eq!(a.project().tempo_bpm, 100.0);
    transport::tap_tick_at(a, 14.0);
    assert_eq!(a.topbar.tap.taps(), 0, "the pause ended the run");
    // A second run at 150 BPM.
    for i in 0..3 {
        transport::tap_at(a, 20.0 + 0.4 * i as f64);
    }
    transport::tap_tick_at(a, 30.0);
    assert_eq!(a.project().tempo_bpm, 150.0);
    a.undo();
    assert_eq!(a.project().tempo_bpm, 100.0, "the second run was one step");
    a.undo();
    assert_eq!(a.project().tempo_bpm, before, "and the first another");
}

#[test]
fn the_tap_key_and_its_shortcut_tap() {
    let mut r = Rig::new();
    assert_eq!(
        Keymap::default().bindings_for(Action::TapTempo)[0].to_string(),
        "Shift+T"
    );
    assert_eq!(
        Keymap::default().bindings_for(Action::TogglePatternMode)[0].to_string(),
        "L"
    );
    let tap = r.text("TAP").expect("the TAP key is on the bar");
    r.click(tap.center());
    assert_eq!(r.app.topbar.tap.taps(), 1);
    assert!(r.app.status.text.contains("Tap again"), "{}", r.app.status.text);
}

#[test]
fn the_metre_editor_sets_the_time_signature() {
    let mut r = Rig::new();
    let well = r.text("4/4").expect("the metre on the bar");
    r.click(well.center());
    let three = r.text("3/4").expect("the common metres in the editor");
    r.click(three.center());
    assert_eq!(r.app.project().time_signature, TimeSignature::new(3, 4));
    r.app.undo();
    assert_eq!(r.app.project().time_signature, TimeSignature::new(4, 4));
}

/// L switches to pattern mode: the engine gets the edited clip alone, looped, the playhead
/// goes to it; back in song mode the song plays from where it was.
#[test]
fn pattern_mode_plays_the_edited_clip_alone() {
    let mut r = Rig::new();
    let a = &mut r.app;
    let track = a.project().tracks[0].id;
    let clip = a.project().tracks[0].clips[0].id;
    a.selected_track = Some(track);
    a.selected_clip = Some(clip);
    a.seek(Ticks::beats(30));
    a.run_action(Action::TogglePatternMode);
    assert!(a.project().pattern_mode);
    r.idle(2);
    let region = cantus_core::pattern::PatternRegion::of(r.app.project(), track, clip).unwrap();
    let snap = r.app.engine.sent_snapshot().expect("a snapshot sent").clone();
    let lr = snap.loop_region.clone().expect("the pattern loops");
    assert_eq!((lr.start, lr.end), (region.start, region.end()));
    let clips_heard: usize = r
        .app
        .project()
        .tracks
        .iter()
        .filter(|t| region.clip_of(t.id).is_some())
        .count();
    assert!(clips_heard >= 1);
    assert_eq!(r.app.engine.position(), region.start, "from the pattern's top");
    assert!(r.painted().contains("PAT"), "{}", r.painted());
    // Recording is refused, with the way out.
    r.app.run_action(Action::Record);
    assert!(!r.app.is_recording());
    assert!(r.app.status.error && r.app.status.text.contains("song mode"));
    // Undo goes back to song mode and the song's place.
    r.app.undo();
    r.idle(2);
    assert!(!r.app.project().pattern_mode);
    assert_eq!(r.app.engine.position(), Ticks::beats(30));
    let snap = r.app.engine.sent_snapshot().unwrap().clone();
    assert_eq!(snap.loop_enabled, r.app.project().loop_enabled);
}

/// The window's title is the project's from the first frame of the main window (the boot
/// path) and follows a rename; the name is renamed in place in the hint panel.
#[test]
fn the_window_title_follows_the_project_through_the_boot_path() {
    let boot = include_str!("../src/boot.rs");
    let assemble = &boot[boot.find("if let Some(app) = boot.step(&ctx) {").expect("assemble")..];
    let assemble = &assemble[..assemble.find("*self = Shell::Running").unwrap()];
    assert!(
        assemble.contains("transport::title::title_of(&app)")
            && assemble.contains("become_main_window(&ctx, &boot.options, &title)"),
        "boot titles the main window with the project:\n{assemble}"
    );
    let app = include_str!("../src/app.rs");
    let frame = &app[app.find("pub fn frame(&mut self, root: &mut egui::Ui) {").unwrap()..];
    assert!(frame[..2000].contains("views::transport::title::sync(self, ctx);"));
    let mut r = Rig::new();
    let name = r.app.project().name.clone();
    let titled = |r: &Rig, want: &str| {
        r.commands
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::Title(t) if t == want))
    };
    assert!(titled(&r, &format!("{name} \u{2014} Cantus")), "{:?}", r.commands);
    // A click on the name opens the field; typing and Enter rename, one undo step.
    transport::title::start_naming(&mut r.app);
    r.idle(2);
    r.frame(vec![
        Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        },
        Event::Text("Psalm 23".into()),
    ]);
    r.frame(vec![Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    r.idle(2);
    assert_eq!(r.app.project().name, "Psalm 23");
    assert!(titled(&r, "Psalm 23 \u{2022} \u{2014} Cantus"), "{:?}", r.commands);
    r.app.undo();
    assert_eq!(r.app.project().name, name);
}

#[test]
fn the_snap_follows_the_editor_being_worked_in() {
    let mut r = Rig::new();
    r.app.focus = cantus_ui::app::EditFocus::Arrangement;
    let (e, grid, _) = transport::snap_editor(&r.app);
    assert_eq!((e, grid), ("arrangement", r.app.arrangement.grid));
    r.app.focus = cantus_ui::app::EditFocus::PianoRoll;
    r.app.central = cantus_ui::app::CentralView::PianoRoll;
    r.idle(2);
    assert_eq!(transport::snap_editor(&r.app).0, "piano_roll");
    // The SNAP lamp switches the piano roll's snap.
    let before = r.app.piano_roll.snap;
    let lamp = r.group(Group::Snap).expect("the snap group");
    let snap = r
        .texts
        .iter()
        .find(|(t, rr)| t == "SNAP" && lamp.contains_rect(*rr))
        .map(|(_, rr)| *rr)
        .expect("the SNAP lamp");
    r.click(snap.center());
    assert_ne!(r.app.piano_roll.snap, before);
    assert_eq!(r.app.arrangement.snap, true, "the arrangement's is its own");
}

#[test]
fn the_metronome_voice_and_accent_are_set_and_undone() {
    let mut r = Rig::new();
    transport::open_metronome_options(&r.ctx);
    r.idle(2);
    let wood = r.text("Woodblock").expect("the voices in the options");
    r.click(wood.center());
    assert_eq!(r.app.project().recording.click_sound, ClickSound::Wood);
    let accent = r.text("DOWNBEAT").expect("the accent lamp");
    r.click(accent.center());
    assert!(!r.app.project().recording.click_accent);
    r.app.undo();
    r.app.undo();
    assert_eq!(r.app.project().recording.click_sound, ClickSound::Classic);
    assert!(r.app.project().recording.click_accent);
}

/// At 900 px the least important groups fold into ⋯ — and work there.
#[test]
fn a_narrow_window_folds_groups_that_still_work_in_the_overflow() {
    let mut r = Rig::new();
    r.size = vec2(900.0, 700.0);
    r.idle(3);
    let (_, l) = transport::last_layout(&r.ctx).unwrap();
    assert!(l.folded().contains(&Group::Windows), "{l:?}");
    assert!(l.span_of(Group::Transport).is_some());
    transport::open_overflow(&r.ctx);
    r.idle(2);
    let drawn = transport::last_overflow(&r.ctx);
    let (_, windows) = *drawn
        .iter()
        .find(|(g, _)| *g == Group::Windows)
        .expect("the windows in the overflow");
    // The fifth key: the mixer.
    let mixer = pos2(windows.left() + 4.0 * 30.0 + 14.0, windows.center().y);
    let shown = r.app.workspace.is_visible(cantus_ui::views::workspace::Pane::Mixer);
    r.click(mixer);
    assert_ne!(
        r.app.workspace.is_visible(cantus_ui::views::workspace::Pane::Mixer),
        shown
    );
}

#[test]
fn customising_hides_and_reorders_groups_and_keeps_them() {
    let mut r = Rig::new();
    transport::open_customise(&mut r.app);
    r.idle(2);
    let tools = r.text("Tools and zoom").expect("the customise panel lists the groups");
    r.click(tools.center());
    assert!(r.app.settings.topbar.hidden.iter().any(|h| h == "tools"));
    assert!(transport::last_layout(&r.ctx)
        .unwrap()
        .1
        .span_of(Group::Tools)
        .is_none());
    // Drag the first row (PAT / SONG) by its grip below the transport.
    let mode = r.text("Pattern / song").unwrap();
    let transport_row = r.text("Transport").unwrap();
    let grip = pos2(mode.left() - 30.0, mode.center().y);
    let to = pos2(grip.x, transport_row.center().y + 2.0);
    r.idle(30);
    r.pointer = Some(grip);
    r.idle(2);
    r.frame(vec![Rig::button(grip, true)]);
    for i in 1..=6 {
        let p = grip + (to - grip) * (i as f32 / 6.0);
        r.pointer = Some(p);
        r.idle(1);
    }
    r.frame(vec![Rig::button(to, false)]);
    r.idle(2);
    let order = cantus_ui::views::transport::layout::resolve(&r.app.settings.topbar.order);
    assert_eq!(&order[..2], &[Group::Transport, Group::Mode], "{order:?}");
    let saved = std::fs::read_to_string(&r.app.settings_path).unwrap();
    assert!(saved.contains("\"tools\""), "{saved}");
    // Reset: the default bar.
    let reset = r.text("Reset").unwrap();
    r.click(reset.center());
    assert!(r.app.settings.topbar.order.is_empty() && r.app.settings.topbar.hidden.is_empty());
}

#[test]
fn the_meter_shows_loudness_when_asked_and_remembers() {
    let mut r = Rig::new();
    let lufs = r.text("LUFS").expect("the meter's switch");
    r.click(lufs.center());
    assert!(r.app.settings.topbar.meter_lufs);
    assert!(r.painted().contains("TP"), "{}", r.painted());
    let saved = std::fs::read_to_string(&r.app.settings_path).unwrap();
    assert!(saved.contains("\"meter_lufs\": true"), "{saved}");
}

#[test]
fn the_midi_lamp_lights_when_events_come_in() {
    let mut r = Rig::new();
    let n = r.app.engine.midi_sender().received();
    assert!(!r.app.topbar.midi_lamp(n));
    r.app
        .engine
        .midi_sender()
        .send(cantus_engine::MidiEvent::note_off(60));
    let n = r.app.engine.midi_sender().received();
    assert!(r.app.topbar.midi_lamp(n));
}

/// CPU, RAM, voices and drops are on the top bar; the engine's numbers, the MIDI count
/// and the audio chip are no longer on the status bar.
#[test]
fn the_performance_moved_from_the_status_bar_to_the_top_bar() {
    let r = Rig::new();
    let perf = r.group(Group::Performance).expect("the performance group");
    let inside: Vec<&str> = r
        .texts
        .iter()
        .filter(|(_, rr)| perf.contains_rect(*rr))
        .map(|(t, _)| t.as_str())
        .collect();
    for w in ["CPU", "RAM", "VOICES", "DROPS"] {
        assert!(inside.contains(&w), "{inside:?}");
    }
    if cfg!(any(windows, target_os = "linux")) {
        assert!(inside.iter().any(|t| t.ends_with("MB") || t.ends_with("GB")), "{inside:?}");
    }
    let status_top = r.size.y - 30.0;
    let status: Vec<&str> = r
        .texts
        .iter()
        .filter(|(_, rr)| rr.top() > status_top)
        .map(|(t, _)| t.as_str())
        .collect();
    assert!(
        !status.iter().any(|t| t.contains("MIDI") || t.contains("voices")),
        "{status:?}"
    );
}

#[test]
fn the_pattern_switch_is_one_command() {
    let mut r = Rig::new();
    r.app.exec(Command::SetPatternMode { on: true });
    assert_eq!(
        r.app.bus.undo_label_key(),
        Some("cmd.set_pattern_mode"),
        "named in the undo history"
    );
    assert_ne!(
        cantus_ui::i18n::t("cmd.set_pattern_mode"),
        "cmd.set_pattern_mode"
    );
}
