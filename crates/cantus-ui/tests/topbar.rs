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
    assert!(
        r.app.status.text.contains("Tap again"),
        "{}",
        r.app.status.text
    );
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

/// A note clip far from the edited one, in a pitch nothing else plays: pattern mode must
/// leave it out.
fn far_clip(a: &mut CantusApp) -> cantus_core::ClipId {
    let track = a.project().tracks[0].id;
    let mut p = a.project().clone();
    let mut c = p.make_clip("Far", Ticks::beats(64), Ticks::beats(4));
    let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, 101, 100);
    c.notes.push(n);
    let id = c.id;
    a.exec(Command::InsertClip {
        track,
        clip: Box::new(c),
    });
    id
}

/// The pitches the engine was sent, on every track.
fn sent_pitches(r: &Rig) -> Vec<u8> {
    let snap = r
        .app
        .engine
        .sent_snapshot()
        .expect("a snapshot sent")
        .clone();
    snap.tracks
        .iter()
        .flat_map(|t| t.events.iter().filter(|e| e.is_on()).map(|e| e.pitch))
        .collect()
}

/// L switches to pattern mode: the engine gets the edited clip alone, looped, the playhead
/// goes to it; back in song mode the song plays from where it was.
#[test]
fn pattern_mode_plays_the_edited_clip_alone() {
    let mut r = Rig::new();
    far_clip(&mut r.app);
    r.idle(2);
    assert!(
        sent_pitches(&r).contains(&101),
        "the song plays the far clip"
    );
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
    let snap = r.app.engine.sent_snapshot().unwrap().clone();
    let lr = snap.loop_region.clone().expect("the pattern loops");
    assert_eq!((lr.start, lr.end), (region.start, region.end()));
    let pitches = sent_pitches(&r);
    assert!(
        !pitches.contains(&101),
        "the far clip is left out: {pitches:?}"
    );
    let edited: Vec<u8> = r.app.project().tracks[0].clips[0]
        .notes
        .iter()
        .map(|n| n.pitch)
        .collect();
    assert!(
        edited.iter().all(|p| pitches.contains(p)),
        "the edited clip plays"
    );
    assert_eq!(
        r.app.engine.position(),
        region.start,
        "from the pattern's top"
    );
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
    assert!(sent_pitches(&r).contains(&101), "the song again");
}

/// In pattern mode the playhead stays in the pattern: Home, a second Stop or a seek
/// outside it go to the pattern's top (review 1: they played silence).
#[test]
fn pattern_mode_keeps_the_playhead_in_the_pattern() {
    let mut r = Rig::new();
    let far = far_clip(&mut r.app);
    let track = r.app.project().tracks[0].id;
    r.app.selected_track = Some(track);
    r.app.selected_clip = Some(far);
    r.app.run_action(Action::TogglePatternMode);
    r.idle(2);
    assert_eq!(r.app.engine.position(), Ticks::beats(64));
    for go in [Action::ToStart, Action::Stop] {
        r.app.run_action(go);
        r.idle(2);
        assert_eq!(r.app.engine.position(), Ticks::beats(64), "{go:?}");
    }
    r.app.seek(Ticks::beats(90));
    r.idle(2);
    assert_eq!(r.app.engine.position(), Ticks::beats(64), "past its end");
    r.app.seek(Ticks::beats(66));
    r.idle(2);
    assert_eq!(
        r.app.engine.position(),
        Ticks::beats(66),
        "inside it, left alone"
    );
}

/// Selecting a track, or nothing, does not take the pattern away (review 1): only another
/// note clip chosen does.
#[test]
fn selecting_something_else_keeps_the_pattern() {
    let mut r = Rig::new();
    let far = far_clip(&mut r.app);
    let track = r.app.project().tracks[0].id;
    r.app.selected_track = Some(track);
    r.app.selected_clip = Some(far);
    r.app.run_action(Action::TogglePatternMode);
    r.idle(2);
    r.app.seek(Ticks::beats(65));
    r.app.selected_clip = None;
    r.app.selected_track = Some(r.app.project().tracks[1].id);
    r.idle(2);
    let region = r
        .app
        .topbar
        .pattern
        .region()
        .cloned()
        .expect("still a pattern");
    assert_eq!(region.start, Ticks::beats(64));
    assert_eq!(r.app.engine.position(), Ticks::beats(65));
    assert!(sent_pitches(&r).contains(&101));
}

/// Pattern mode is not switched on in the middle of a take (review 1): the take belongs to
/// the song; the status line says to stop first.
#[test]
fn pattern_mode_waits_for_the_take_to_end() {
    let mut r = Rig::new();
    let keys = r.app.project().tracks[0].id;
    r.app.toggle_arm(keys, false);
    r.app.seek(Ticks::beats(12));
    r.app.start_recording();
    assert!(r.app.is_recording());
    let at = r.app.engine.position();
    r.app.run_action(Action::TogglePatternMode);
    r.idle(1);
    assert!(!r.app.project().pattern_mode);
    assert!(r.app.is_recording());
    assert_eq!(r.app.engine.position(), at);
    assert!(
        r.app.status.text.contains("Stop recording"),
        "{}",
        r.app.status.text
    );
}

/// Opening another song forgets the last one's pattern state (review 1: the new song
/// jumped to the old one's place).
#[test]
fn another_song_starts_without_the_last_ones_pattern_state() {
    let mut r = Rig::new();
    let a = &mut r.app;
    a.selected_track = Some(a.project().tracks[0].id);
    a.selected_clip = Some(a.project().tracks[0].clips[0].id);
    a.seek(Ticks::beats(30));
    a.run_action(Action::TogglePatternMode);
    r.idle(2);
    r.app.replace_project(welcome_project());
    r.idle(3);
    assert!(!r.app.project().pattern_mode);
    assert_eq!(r.app.engine.position(), Ticks::ZERO);
    assert!(r.app.topbar.pattern.region().is_none());
}

/// The window's title is the project's from the first frame of the main window (the boot
/// path) and follows a rename; the name is renamed in place in the hint panel.
#[test]
fn the_window_title_follows_the_project_through_the_boot_path() {
    let boot = include_str!("../src/boot.rs");
    let assemble = &boot[boot
        .find("if let Some(app) = boot.step(&ctx) {")
        .expect("assemble")..];
    let assemble = &assemble[..assemble.find("*self = Shell::Running").unwrap()];
    assert!(
        assemble.contains("transport::title::title_of(&app)")
            && assemble.contains("become_main_window(&ctx, &boot.options, &title)"),
        "boot titles the main window with the project:\n{assemble}"
    );
    let app = include_str!("../src/app.rs");
    let frame = &app[app
        .find("pub fn frame(&mut self, root: &mut egui::Ui) {")
        .unwrap()..];
    assert!(frame[..2000].contains("views::transport::title::sync(self, ctx);"));
    let mut r = Rig::new();
    let name = r.app.project().name.clone();
    let titled = |r: &Rig, want: &str| {
        r.commands
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::Title(t) if t == want))
    };
    assert!(
        titled(&r, &format!("{name} \u{2014} Cantus")),
        "{:?}",
        r.commands
    );
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
    assert!(
        titled(&r, "Psalm 23 \u{2022} \u{2014} Cantus"),
        "{:?}",
        r.commands
    );
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
    assert!(r.app.arrangement.snap, "the arrangement's is its own");
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

/// At 900 px the least important groups fold into ⋯ — and work there, their own menus
/// and popovers included (review 1: opening one closed the overflow and itself).
#[test]
fn a_narrow_window_folds_groups_that_still_work_in_the_overflow() {
    let mut r = Rig::new();
    r.size = vec2(900.0, 700.0);
    r.idle(3);
    let (_, l) = transport::last_layout(&r.ctx).unwrap();
    for g in [Group::Windows, Group::Snap, Group::Metronome] {
        assert!(l.folded().contains(&g), "{g:?}: {l:?}");
    }
    assert!(l.span_of(Group::Transport).is_some());
    transport::open_overflow(&mut r.app);
    r.idle(2);
    let drawn = transport::last_overflow(&r.ctx);
    let at = |g: Group| {
        drawn
            .iter()
            .find(|(x, _)| *x == g)
            .map(|(_, r)| *r)
            .unwrap()
    };
    // The fifth key: the mixer (one strip of 22-pt keys).
    let windows = at(Group::Windows);
    let mixer = pos2(windows.left() + 4.0 * 22.0 + 11.0, windows.center().y);
    let shown = r
        .app
        .workspace
        .is_visible(cantus_ui::views::workspace::Pane::Mixer);
    r.click(mixer);
    assert_ne!(
        r.app
            .workspace
            .is_visible(cantus_ui::views::workspace::Pane::Mixer),
        shown
    );
    assert!(
        transport::overflow_open(&r.app),
        "a click inside keeps it open"
    );
    // The folded snap's grid list opens from the overflow and sets the grid.
    let snap = at(Group::Snap);
    r.click(pos2(snap.left() + 30.0, snap.center().y));
    assert!(
        transport::overflow_open(&r.app),
        "the grid list did not close ⋯"
    );
    let eighth = cantus_ui::i18n::t("grid.eighth");
    let item = r
        .texts
        .iter()
        .rev()
        .filter(|(t, rr)| *t == eighth && rr.left() > snap.left() - 300.0)
        .map(|(_, rr)| *rr)
        .next()
        .expect("the grid list is open");
    r.click(item.center());
    assert_eq!(
        transport::snap_editor(&r.app).1,
        Ticks::EIGHTH,
        "the focused editor's grid"
    );
    // The folded metronome's options open from it too.
    let metro = at(Group::Metronome);
    let down = pos2(metro.right() - 7.0, metro.center().y);
    r.click(down);
    assert!(transport::overflow_open(&r.app));
    assert!(
        r.text("Woodblock").is_some(),
        "the metronome's options are open"
    );
    // A click on the window itself closes it.
    r.click(pos2(450.0, 400.0));
    assert!(!transport::overflow_open(&r.app));
}

#[test]
fn customising_hides_and_reorders_groups_and_keeps_them() {
    let mut r = Rig::new();
    transport::open_customise(&mut r.app);
    r.idle(2);
    let tools = r
        .text("Tools and zoom")
        .expect("the customise panel lists the groups");
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

/// CPU, RAM, voices and drops are on the top bar; the MIDI count and the audio chip —
/// drawn on the status bar before round 28 whatever the device — are no longer there (the
/// engine readout was drawn only with a live device, which no test has).
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
        assert!(
            inside
                .iter()
                .any(|t| t.ends_with("MB") || t.ends_with("GB")),
            "{inside:?}"
        );
    }
    let status_top = r.size.y - 30.0;
    let status: Vec<&str> = r
        .texts
        .iter()
        .filter(|(_, rr)| rr.top() > status_top)
        .map(|(t, _)| t.as_str())
        .collect();
    assert!(
        !status
            .iter()
            .any(|t| t.contains("MIDI") || t.contains("voices")),
        "{status:?}"
    );
    // The chip's words (no device here: "No audio — click to set up") are the top bar's.
    let chip = cantus_ui::i18n::t("devices.chip.none");
    assert!(!status.iter().any(|t| *t == chip), "{status:?}");
    assert!(
        r.texts
            .iter()
            .any(|(t, rr)| *t == chip && rr.bottom() < 60.0),
        "the chip is on the top bar"
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

/// A fine (Shift) drag of the click's level moves it, however slowly the hand goes
/// (review 1: each frame's small move was rounded away).
#[test]
fn a_slow_fine_drag_moves_the_click_level() {
    let mut r = Rig::new();
    let well = r
        .texts
        .iter()
        .find(|(t, _)| t.starts_with('+') && t.ends_with("dB"))
        .map(|(_, rr)| *rr)
        .expect("the click's level on the bar");
    let at = well.center();
    let button = |pos: Pos2, pressed: bool| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::SHIFT,
    };
    r.pointer = Some(at);
    r.idle(2);
    r.frame(vec![
        Event::ModifiersChanged(Modifiers::SHIFT),
        button(at, true),
    ]);
    // One point a frame for 40 frames: 0.03 dB a point with Shift.
    for i in 1..=40 {
        r.pointer = Some(at + vec2(i as f32, 0.0));
        r.idle(1);
    }
    let end = at + vec2(40.0, 0.0);
    r.frame(vec![
        button(end, false),
        Event::ModifiersChanged(Modifiers::NONE),
    ]);
    r.idle(2);
    let db = r.app.project().recording.click_db;
    assert!(
        (0.9..=1.5).contains(&db),
        "{db} dB after a slow fine drag of 40 pt"
    );
}

// ---- fix2 (review 2) ---------------------------------------------------------------------

/// The meter's holds are fed every frame, whatever the bar shows: an over while the meter
/// is folded into ⋯ is in the TP figure when the overflow opens (it was fed only while
/// drawn).
#[test]
fn an_over_while_the_meter_is_folded_is_held() {
    let mut r = Rig::new();
    r.app.settings.topbar.meter_lufs = true;
    r.size = vec2(900.0, 700.0);
    r.idle(3);
    assert!(r.group(Group::Meter).is_none(), "the meter folds at 900");
    let mut m = r.app.engine.meters().clone();
    m.master_loudness.true_peak = 1.2; // +1.6 dBTP
    r.app.engine.set_meters(m.clone());
    r.idle(2);
    m.master_loudness.true_peak = 0.1;
    r.app.engine.set_meters(m);
    r.idle(2);
    transport::open_overflow(&mut r.app);
    r.idle(3);
    let text = r.painted();
    assert!(text.contains("TP 1.6"), "{text}");
}

/// An undo or a redo that brings pattern mode back in the middle of a take ends the take
/// first, in the song (`follow`; review 2: only the key's refusal was tested).
#[test]
fn an_undo_or_redo_that_brings_pattern_mode_back_ends_the_take() {
    for redo in [false, true] {
        let mut r = Rig::new();
        let keys = r.app.project().tracks[0].id;
        r.app.toggle_arm(keys, false);
        r.app.run_action(Action::TogglePatternMode);
        r.idle(1);
        if redo {
            r.app.undo();
        } else {
            r.app.run_action(Action::TogglePatternMode);
        }
        r.idle(1);
        assert!(!r.app.project().pattern_mode, "redo {redo}");
        r.app.start_recording();
        assert!(r.app.is_recording(), "redo {redo}");
        if redo {
            r.app.redo();
        } else {
            r.app.undo();
        }
        r.idle(1);
        assert!(r.app.project().pattern_mode, "redo {redo}: the mode came back");
        assert!(!r.app.is_recording(), "redo {redo}: the take ended");
        assert!(r.app.project().track(keys).unwrap().armed, "redo {redo}");
    }
}

/// Choosing a channel (the rack, Ctrl+Up/Down) in pattern mode keeps the pattern playing
/// when the channel's first clip is elsewhere (review 2: the playhead jumped there).
#[test]
fn choosing_a_channel_keeps_the_pattern() {
    let mut r = Rig::new();
    let far = far_clip(&mut r.app);
    let track = r.app.project().tracks[0].id;
    r.app.selected_track = Some(track);
    r.app.selected_clip = Some(far);
    r.app.run_action(Action::TogglePatternMode);
    r.idle(2);
    r.app.seek(Ticks::beats(65));
    r.idle(1);
    let other = r.app.project().tracks[1].id;
    assert!(
        r.app.project().track(other).unwrap().clips.first().is_some(),
        "the other track has a clip elsewhere"
    );
    r.app.select_track(other);
    r.idle(2);
    let region = r.app.topbar.pattern.region().cloned().expect("a pattern");
    assert_eq!(region.start, Ticks::beats(64));
    assert_eq!(r.app.engine.position(), Ticks::beats(65));
    assert_eq!(r.app.selected_clip, region.clip_of(other));
}
