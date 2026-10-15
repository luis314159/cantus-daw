//! Round 27 (ADR 0051, BACKLOG §6g): FL's quick gestures in the arrangement and the
//! inspector, each driven through the whole window with the pointer —
//!
//! - a double-click on a track header opens its instrument (or, for a track whose
//!   instrument has no window, selects it and shows the inspector);
//! - a click on the name's letters renames it in place, with the name selected; a click
//!   anywhere else on the header never does, and typing with a track selected never does;
//! - *Open editor* brings an editor back from the tray and to the front;
//! - a double-click on a clip opens its editor on it: the piano roll, or the channel rack
//!   for a step pattern; an automation clip its editor; an audio clip the pitch editor
//!   (round 28, ADR 0054).

use cantus_core::{
    AudioRegion, AudioSourceId, Command, InstrumentKind, InstrumentSpec, NativeSpec, Project,
    PulsusParams, Ticks, TrackId, TrackInput,
};
use cantus_engine::EngineHandle;
use cantus_ui::app::{CentralView, EditFocus};
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_device as device;
use cantus_ui::CantusApp;
use egui::{pos2, vec2, CursorIcon, Event, Key, Modifiers, Pos2, Rect};

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    size: egui::Vec2,
    frame: u64,
    pointer: Option<Pos2>,
    cursor: CursorIcon,
    /// The words drawn in the last frame, and where.
    texts: Vec<(String, Rect)>,
    _dir: tempfile::TempDir,
}

fn data_dir() {
    static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let d = DIR.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("cantus-gestures-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        d
    });
    std::env::set_var("CANTUS_DATA_DIR", d);
}

impl Rig {
    fn new(project: Project) -> Rig {
        data_dir();
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        let dir = tempfile::tempdir().unwrap();
        let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
        app.settings_path = dir.path().join("settings.json");
        app.layouts_path = dir.path().join("layouts.json");
        let mut r = Rig {
            ctx,
            app,
            size: vec2(1600.0, 900.0),
            frame: 0,
            pointer: None,
            cursor: CursorIcon::Default,
            texts: Vec::new(),
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
        self.cursor = out.platform_output.cursor_icon;
        self.texts.clear();
        for c in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &c.shape {
                self.texts
                    .push((t.galley.text().to_string(), t.visual_bounding_rect()));
            }
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

    fn hover(&mut self, at: Pos2) {
        self.pointer = Some(at);
        self.idle(2);
    }

    fn click(&mut self, at: Pos2) {
        self.idle(30);
        self.hover(at);
        self.frame(vec![Rig::button(at, true)]);
        self.frame(vec![Rig::button(at, false)]);
        self.idle(2);
    }

    /// Two clicks at `at`, quickly: a double-click.
    fn double_click(&mut self, at: Pos2) {
        // Half a second after any earlier click, as a hand would: egui counts two clicks
        // within 0.3 s as a double-click whatever lay between them.
        self.idle(30);
        self.hover(at);
        for _ in 0..2 {
            self.frame(vec![Rig::button(at, true)]);
            self.frame(vec![Rig::button(at, false)]);
        }
        self.idle(3);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        // `RawInput` carries no modifier state: it arrives with the events.
        self.frame(vec![
            Event::ModifiersChanged(modifiers),
            Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ]);
        self.frame(vec![
            Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            },
            Event::ModifiersChanged(Modifiers::NONE),
        ]);
        self.idle(2);
    }

    fn type_text(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.into())]);
        self.idle(1);
    }

    fn keyboard_free(&self) -> bool {
        !self.ctx.egui_wants_keyboard_input() && !cantus_ui::kit::field::active(&self.ctx)
    }

    fn track(&self, i: usize) -> TrackId {
        self.app.project().tracks[i].id
    }

    fn name(&self, i: usize) -> String {
        self.app.project().tracks[i].name.clone()
    }

    /// Where track `i`'s header was drawn, and its name's letters.
    fn header(&self, i: usize) -> (Rect, Rect) {
        let id = self.track(i);
        let h = self
            .app
            .arrangement
            .header_hits()
            .iter()
            .find(|h| h.track == id)
            .copied()
            .expect("the header was drawn");
        (h.row, h.letters.expect("the name was drawn"))
    }

    /// A point on track `i`'s header that is none of its controls and not its name: on the
    /// name line, just right of the letters (an audio track's input chip sits under it).
    fn header_body(&self, i: usize) -> Pos2 {
        let (row, letters) = self.header(i);
        let at = pos2(letters.right() + 8.0, letters.center().y);
        assert!(at.x < row.right() - 70.0, "room right of a short name");
        at
    }

    /// The song in miniature under the name line: also the header, not its name.
    fn header_map(&self, i: usize) -> Pos2 {
        let (row, letters) = self.header(i);
        pos2(
            letters.left() + 4.0,
            (letters.bottom() + row.bottom()) * 0.5 - 2.0,
        )
    }

    /// The middle of clip `c` of track `i` on the timeline.
    fn clip_at(&self, i: usize, c: usize) -> Pos2 {
        let geo = self
            .app
            .arrangement
            .geometry()
            .expect("the timeline was drawn");
        let clip = &self.app.project().tracks[i].clips[c];
        let a = clip.start.as_beats_f64() as f32;
        let b = clip.end().as_beats_f64() as f32;
        pos2(geo.x((a + b) * 0.5), geo.row_y(i))
    }

    fn renaming(&self) -> Option<TrackId> {
        self.app.arrangement.renaming_track()
    }
}

const LEAD: usize = 0;
const DRUMS: usize = 1;
const BASS: usize = 2;
const VOICE: usize = 3;

/// Four tracks: a native instrument with a melody, a kit with a step pattern, Firmus
/// (edited in the inspector) with an automation clip after its notes, and an audio track.
fn band() -> Project {
    let mut p = Project::new("Band");
    let mut lead = p.make_track("Lead");
    lead.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable));
    let mut clip = p.make_clip("Melody", Ticks::beats(8), Ticks::beats(8));
    for (i, pitch) in [60u8, 64, 67, 72].into_iter().enumerate() {
        let n = p.make_note(Ticks::beats(i as i64), Ticks::QUARTER * 3, pitch, 100);
        clip.notes.push(n);
    }
    lead.clips.push(clip);
    p.tracks.push(lead);

    let mut drums = p.make_track("Drums");
    drums.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
    let mut clip = p.make_clip("Beat", Ticks::beats(4), Ticks::beats(8));
    for (at, pitch) in [(0i64, 36u8), (4, 38), (8, 36), (12, 38), (15, 36)] {
        let n = p.make_note(Ticks::SIXTEENTH * at, Ticks::SIXTEENTH, pitch, 110);
        clip.notes.push(n);
    }
    drums.clips.push(clip);
    p.tracks.push(drums);

    let mut bass = p.make_track("Bass");
    let clip = p.make_clip("Line", Ticks::ZERO, Ticks::beats(4));
    bass.clips.push(clip);
    let mut auto = p.make_clip("Level", Ticks::beats(8), Ticks::beats(8));
    auto.automation = Some(cantus_core::Automation::new(
        cantus_core::AutoTarget::TrackGain(bass.id),
        vec![
            cantus_core::AutoPoint::new(Ticks::ZERO, 0.0),
            cantus_core::AutoPoint::new(Ticks::beats(8), 1.0),
        ],
    ));
    bass.clips.push(auto);
    p.tracks.push(bass);

    let mut voice = p.make_track("Voice");
    voice.input = Some(TrackInput::mono(0));
    let mut take = p.make_clip("Take", Ticks::beats(4), Ticks::beats(8));
    take.audio = Some(AudioRegion::of(AudioSourceId(1)));
    voice.clips.push(take);
    p.tracks.push(voice);
    p
}

// ---- 1. a double-click on a header opens the instrument -------------------------------

#[test]
fn a_double_click_on_a_header_opens_the_instrument_and_never_renames() {
    let mut r = Rig::new(band());
    // A native instrument: its editor, a tab of the workspace, on screen.
    let lead = r.track(LEAD);
    r.double_click(r.header_body(LEAD));
    assert_eq!(r.renaming(), None, "a double-click no longer renames");
    assert_eq!(
        r.app.inst.open,
        Some(lead),
        "the instrument's editor opened"
    );
    r.idle(3);
    assert!(r.app.instrument_window_shown(lead), "and it shows");
    let key = r.app.instrument_window_key(lead).unwrap();
    assert!(
        r.app.workspace.is_visible(Pane::Device(key)),
        "its tab in front"
    );
    assert_eq!(
        r.app.selected_track,
        Some(lead),
        "the track is selected too"
    );
    // Closed again: the editor may float over the headers.
    r.app.close_instrument_window(lead);
    r.idle(3);

    // A kit: Pulsus's editor.
    let drums = r.track(DRUMS);
    r.double_click(r.header_body(DRUMS));
    assert_eq!(r.app.pulsus.open, Some(drums), "the kit's editor opened");
    r.app.close_instrument_window(drums);
    r.idle(3);
    assert_eq!(r.renaming(), None);

    // Firmus has no window (it is edited in the inspector), and an audio track's
    // instrument is never played: both select the track and show the inspector.
    for i in [BASS, VOICE] {
        let id = r.track(i);
        r.app.workspace.hide(Pane::Inspector);
        r.idle(2);
        assert!(!r.app.workspace.is_visible(Pane::Inspector));
        r.double_click(r.header_body(i));
        assert_eq!(r.app.selected_track, Some(id), "track {i} selected");
        assert!(
            r.app.workspace.is_visible(Pane::Inspector),
            "track {i}: the inspector shows"
        );
        assert_eq!(r.renaming(), None);
    }
}

// ---- 2. a click on the letters renames; nothing else does ---------------------------

#[test]
fn a_click_on_the_letters_selects_and_renames_with_the_name_selected() {
    let mut r = Rig::new(band());
    r.app.select_track(r.track(LEAD));
    r.idle(2);
    let bass = r.track(BASS);
    let (_, letters) = r.header(BASS);
    r.click(letters.center());
    assert_eq!(
        r.app.selected_track,
        Some(bass),
        "the click selected the track"
    );
    assert_eq!(r.renaming(), Some(bass), "and opened its name");
    assert!(!r.keyboard_free(), "the field holds the keyboard");
    // The name is selected: what is typed replaces it. Enter is one undo step.
    let rev = r.app.bus.revision();
    r.type_text("Sub");
    r.key(Key::Enter, Modifiers::NONE);
    assert_eq!(r.name(BASS), "Sub", "typed over the selected name");
    assert_eq!(r.renaming(), None, "Enter closed it");
    assert!(r.keyboard_free());
    assert_eq!(r.app.bus.revision(), rev + 1, "one command");
    assert_eq!(
        r.app.bus.undo_label_key(),
        Some("cmd.set_track_name"),
        "the rename is the undo step"
    );
    r.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(r.name(BASS), "Bass", "one undo brings the old name back");
}

#[test]
fn a_click_on_the_header_outside_the_letters_never_renames() {
    let mut r = Rig::new(band());
    let drums = r.track(DRUMS);
    // Under the name line, and on the name line right of the letters.
    let (row, letters) = r.header(DRUMS);
    let right_of = pos2(letters.right() + 6.0, letters.center().y);
    assert!(
        right_of.x < row.right() - 60.0,
        "room right of a short name"
    );
    for at in [r.header_map(DRUMS), right_of] {
        r.click(at);
        assert_eq!(r.app.selected_track, Some(drums), "the click selects");
        assert_eq!(r.renaming(), None, "and does not rename ({at:?})");
        assert!(r.keyboard_free());
    }
}

#[test]
fn typing_with_a_track_selected_never_starts_a_rename() {
    let mut r = Rig::new(band());
    r.click(r.header_body(LEAD));
    assert_eq!(r.app.selected_track, Some(r.track(LEAD)));
    r.pointer = None;
    for (k, s) in [(Key::A, "a"), (Key::B, "b"), (Key::X, "x")] {
        r.frame(vec![
            Event::Key {
                key: k,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text(s.into()),
        ]);
        r.frame(vec![Event::Key {
            key: k,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
    }
    r.idle(2);
    assert_eq!(r.renaming(), None, "typing never renames");
    assert_eq!(r.name(LEAD), "Lead");
}

#[test]
fn esc_leaves_the_old_name_and_no_undo_step() {
    let mut r = Rig::new(band());
    let (_, letters) = r.header(LEAD);
    r.click(letters.center());
    assert!(r.renaming().is_some());
    let rev = r.app.bus.revision();
    r.type_text("Wrong");
    r.key(Key::Escape, Modifiers::NONE);
    assert_eq!(r.name(LEAD), "Lead", "the old name");
    assert_eq!(r.renaming(), None, "Esc closed the field");
    assert_eq!(r.app.bus.revision(), rev, "no command, no undo step");
    assert!(r.keyboard_free());
}

#[test]
fn while_the_name_is_open_no_shortcut_fires() {
    let mut r = Rig::new(band());
    let (_, letters) = r.header(LEAD);
    r.click(letters.center());
    let rev = r.app.bus.revision();
    let lead = r.track(LEAD);
    for (k, m) in [
        (Key::Z, Modifiers::COMMAND),
        (Key::M, Modifiers::NONE),
        (Key::Delete, Modifiers::NONE),
        (Key::F8, Modifiers::NONE),
        (Key::Space, Modifiers::NONE),
    ] {
        r.key(k, m);
        assert_eq!(r.renaming(), Some(lead), "{k:?}: still open");
    }
    assert_eq!(r.app.bus.revision(), rev, "no shortcut edited anything");
    assert_eq!(r.app.inst.open, None, "F8 opened nothing");
    assert!(!r.app.engine.is_playing(), "Space played nothing");
    assert!(!r.app.project().tracks[LEAD].muted);
}

#[test]
fn hovering_the_letters_shows_a_text_cursor_and_says_what_a_click_does() {
    let mut r = Rig::new(band());
    let (_, letters) = r.header(LEAD);
    r.hover(letters.center());
    assert_eq!(r.cursor, CursorIcon::Text, "an I-beam over the letters");
    let h = cantus_ui::kit::hint::current(&r.ctx).expect("a hint");
    assert_eq!(
        h.value,
        cantus_ui::i18n::t("track.header.name_tip"),
        "{h:?}"
    );
    r.hover(r.header_body(LEAD));
    assert_ne!(r.cursor, CursorIcon::Text, "no I-beam off the letters");
    let h = cantus_ui::kit::hint::current(&r.ctx).expect("a hint");
    assert_eq!(h.value, cantus_ui::i18n::t("track.header.tip"), "{h:?}");
}

// ---- 4. a double-click on a clip opens its editor on it ------------------------------

#[test]
fn a_double_click_on_a_notes_clip_opens_the_piano_roll_on_it() {
    let mut r = Rig::new(band());
    // The piano roll behind the mixer, the arrangement focused.
    r.app.show_central(CentralView::Mixer);
    r.idle(3);
    assert!(!r.app.workspace.is_visible(Pane::PianoRoll));
    let lead = r.track(LEAD);
    let clip = r.app.project().tracks[LEAD].clips[0].id;
    r.double_click(r.clip_at(LEAD, 0));
    assert!(
        r.app.workspace.is_visible(Pane::PianoRoll),
        "the piano roll in front"
    );
    assert_eq!(r.app.central, CentralView::PianoRoll);
    assert_eq!(r.app.focus, EditFocus::PianoRoll, "and it has the focus");
    assert_eq!(
        (r.app.selected_track, r.app.selected_clip),
        (Some(lead), Some(clip)),
        "on that clip"
    );
    // Shown whole: from the content's start, its eight beats across the grid, and every
    // note's row on screen.
    let v = &r.app.piano_roll;
    let (w, h) = v.grid_size();
    assert!(
        v.scroll_beats <= 0.01,
        "the content's start in view ({})",
        v.scroll_beats
    );
    let beats = w / v.px_per_beat;
    assert!(
        (8.0..=9.0).contains(&beats),
        "the clip across the grid ({beats} beats)"
    );
    let shown = h / v.key_height;
    for n in &r.app.project().tracks[LEAD].clips[0].notes {
        let lane = n.pitch as f32;
        assert!(
            lane < v.top_lane && lane > v.top_lane - shown,
            "pitch {} on screen (top {}, {} rows)",
            n.pitch,
            v.top_lane,
            shown
        );
    }
    assert_eq!(
        r.app.project().tracks[LEAD].clips.len(),
        1,
        "no clip was made"
    );
}

#[test]
fn a_double_click_on_a_step_pattern_opens_the_channel_rack() {
    let mut r = Rig::new(band());
    r.app.show_central(CentralView::PianoRoll);
    r.idle(3);
    assert!(!r.app.workspace.is_visible(Pane::ChannelRack));
    let drums = r.track(DRUMS);
    let clip = r.app.project().tracks[DRUMS].clips[0].id;
    r.double_click(r.clip_at(DRUMS, 0));
    assert!(
        r.app.workspace.is_visible(Pane::ChannelRack),
        "the rack in front"
    );
    assert_eq!(r.app.central, CentralView::ChannelRack);
    assert_eq!(r.app.focus, EditFocus::PianoRoll, "the editors' focus");
    assert_eq!(
        (r.app.selected_track, r.app.selected_clip),
        (Some(drums), Some(clip))
    );
    assert!(r.app.rack.fit, "the whole pattern fits the rack");
}

#[test]
fn an_automation_clip_an_audio_clip_and_an_empty_lane_keep_their_double_clicks() {
    let mut r = Rig::new(band());
    // Automation: its editor (ADR 0027), on that clip.
    r.app.show_central(CentralView::Mixer);
    r.idle(3);
    let bass = r.track(BASS);
    let auto = r.app.project().tracks[BASS].clips[1].id;
    let body = r
        .app
        .arrangement
        .auto
        .drawn
        .iter()
        .find(|d| d.clip == auto)
        .expect("the automation clip drawn in its lane")
        .body;
    // Off the curve (it rises from the bottom left), so no point is grabbed.
    r.double_click(pos2(
        body.left() + body.width() * 0.25,
        body.top() + body.height() * 0.2,
    ));
    assert_eq!(r.app.selected_clip, Some(auto));
    assert_eq!(
        r.app.central,
        CentralView::PianoRoll,
        "the automation editor"
    );
    assert!(r.app.workspace.is_visible(Pane::PianoRoll));
    // Audio: the pitch editor (ADR 0054), in the piano roll's place.
    r.app.show_central(CentralView::Mixer);
    r.idle(2);
    let take = r.app.project().tracks[VOICE].clips[0].id;
    // Low in the clip and off its middle: the gain handle sits at the top centre.
    let geo = r.app.arrangement.geometry().unwrap();
    let row = geo.rows[VOICE];
    let x = r.clip_at(VOICE, 0).x - 40.0;
    r.double_click(pos2(x, row.top + row.height * 0.8));
    assert_eq!(r.app.selected_clip, Some(take));
    assert_eq!(r.app.central, CentralView::PianoRoll, "the pitch editor");
    assert!(
        r.app.workspace.is_visible(Pane::PianoRoll),
        "the pitch editor shows it"
    );
    // An empty lane: a new clip, as before.
    let n = r.app.project().tracks[BASS].clips.len();
    let geo = r.app.arrangement.geometry().unwrap();
    let empty = pos2(geo.x(40.0), geo.row_y(BASS));
    r.double_click(empty);
    assert_eq!(
        r.app.project().tracks[BASS].clips.len(),
        n + 1,
        "a clip made"
    );
    assert_eq!(r.app.selected_track, Some(bass));
}

#[test]
fn the_rack_opens_only_for_step_shaped_content() {
    use cantus_ui::clips::{editor_for, rack_grid, ClipEditor};
    let p = band();
    let grid = Ticks::SIXTEENTH;
    let kit = p.tracks[DRUMS].instrument.lanes();
    let keys = p.tracks[LEAD].instrument.lanes();
    let drums = p.tracks[DRUMS].clips[0].clone();
    assert_eq!(
        editor_for(&drums, kit, grid),
        ClipEditor::ChannelRack,
        "steps"
    );
    let melody = &p.tracks[LEAD].clips[0];
    assert_eq!(
        editor_for(melody, keys, grid),
        ClipEditor::PianoRoll,
        "long notes"
    );
    let empty = &p.tracks[BASS].clips[0];
    assert_eq!(
        editor_for(empty, keys, grid),
        ClipEditor::PianoRoll,
        "unknown: the roll"
    );
    let auto = &p.tracks[BASS].clips[1];
    assert_eq!(editor_for(auto, keys, grid), ClipEditor::Automation);
    let take = &p.tracks[VOICE].clips[0];
    assert_eq!(editor_for(take, keys, grid), ClipEditor::Pitch);
    // One note off every step grid, or one held longer than a step: the piano roll.
    let mut off = drums.clone();
    off.notes[1].start += Ticks(7);
    assert_eq!(
        editor_for(&off, kit, grid),
        ClipEditor::PianoRoll,
        "off the grid"
    );
    let mut held = drums.clone();
    held.notes[2].length = Ticks::QUARTER;
    assert_eq!(editor_for(&held, kit, grid), ClipEditor::PianoRoll, "held");
    // The clip's own grid decides, not the rack's current one: sixteenths with the rack at
    // eighths still open in the rack, at sixteenths.
    assert_eq!(
        rack_grid(&drums, kit, Ticks::SIXTEENTH * 2),
        Some(Ticks::SIXTEENTH)
    );
}

/// Review 5: a channel's row in the rack plays one pitch (a melodic track's one row adds
/// any pitch but shows only on/off; a kit has a row per pad) — FL's rack too, where a
/// melodic pattern opens in the piano roll. So the rack opens only when it shows the clip
/// with nothing lost.
#[test]
fn the_rack_opens_only_when_it_loses_nothing() {
    use cantus_ui::clips::{editor_for, rack_grid, ClipEditor};
    let mut p = band();
    let keys_inst = p.tracks[LEAD].instrument.clone();
    let kit_inst = p.tracks[DRUMS].instrument.clone();
    let keys = keys_inst.lanes();
    let mut line = p.make_clip("Bass line", Ticks::ZERO, Ticks::beats(4));
    for (i, pitch) in [36u8, 36, 43, 48, 39, 41, 36, 46].into_iter().enumerate() {
        let n = p.make_note(
            Ticks::SIXTEENTH * (i as i64 * 2),
            Ticks::SIXTEENTH,
            pitch,
            100,
        );
        line.notes.push(n);
    }
    assert_eq!(
        editor_for(&line, keys, Ticks::SIXTEENTH),
        ClipEditor::PianoRoll,
        "a sixteenth bass line on a synth: the piano roll"
    );
    let mut hats = line.clone();
    for n in &mut hats.notes {
        n.pitch = 42;
    }
    assert_eq!(
        editor_for(&hats, keys, Ticks::SIXTEENTH),
        ClipEditor::ChannelRack,
        "one pitch on a synth: its row shows it"
    );
    // A kit shows a row per pad (and per stray pitch): a pattern across pads, the rack.
    let kit = kit_inst.lanes();
    assert_eq!(
        editor_for(&line, kit, Ticks::SIXTEENTH),
        ClipEditor::ChannelRack,
        "across a kit's rows"
    );
    // Two notes of one pitch on one step are one step in the rack: the piano roll.
    let mut doubled = hats.clone();
    let mut twin = doubled.notes[0].clone();
    twin.id = cantus_core::NoteId(9_999);
    doubled.notes.push(twin);
    assert_eq!(
        editor_for(&doubled, keys, Ticks::SIXTEENTH),
        ClipEditor::PianoRoll
    );
    // Review 7: triplets made in the rack at 1/16T open there — at 1/16T — whatever grid
    // the rack has now.
    let mut trip = p.make_clip("Triplets", Ticks::ZERO, Ticks::beats(2));
    for i in 0..12 {
        let n = p.make_note(Ticks(160 * i), Ticks(160), 42, 90);
        trip.notes.push(n);
    }
    assert_eq!(rack_grid(&trip, kit, Ticks::SIXTEENTH), Some(Ticks(160)));
    assert_eq!(
        editor_for(&trip, kit, Ticks::SIXTEENTH),
        ClipEditor::ChannelRack
    );
    p.tracks.clear();
}

#[test]
fn a_double_click_on_a_sixteenth_line_opens_the_roll_and_on_triplets_the_rack_at_triplets() {
    let mut p = band();
    let mut line = p.make_clip("Bass line", Ticks::beats(16), Ticks::beats(4));
    for (i, pitch) in [36u8, 43, 48, 39].into_iter().enumerate() {
        let n = p.make_note(
            Ticks::SIXTEENTH * (i as i64 * 2),
            Ticks::SIXTEENTH,
            pitch,
            100,
        );
        line.notes.push(n);
    }
    p.tracks[LEAD].clips.push(line);
    let mut trip = p.make_clip("Triplets", Ticks::beats(16), Ticks::beats(2));
    for i in 0..12 {
        let n = p.make_note(Ticks(160 * i), Ticks(160), 36, 90);
        trip.notes.push(n);
    }
    p.tracks[DRUMS].clips.push(trip);
    let mut r = Rig::new(p);
    r.app.show_central(CentralView::Mixer);
    r.idle(3);
    r.double_click(r.clip_at(LEAD, 1));
    assert_eq!(r.app.central, CentralView::PianoRoll, "a melody: the roll");
    assert_eq!(r.app.rack.grid, Ticks::SIXTEENTH);
    r.double_click(r.clip_at(DRUMS, 1));
    assert_eq!(
        r.app.central,
        CentralView::ChannelRack,
        "triplet steps: the rack"
    );
    assert_eq!(r.app.rack.grid, Ticks(160), "at the clip's own grid");
}

/// Review 6: the rack opens with the clip's track in view (and selected), however far down.
#[test]
fn the_rack_opens_with_the_clips_track_in_view() {
    let mut p = Project::new("Many");
    for i in 0..40 {
        let t = p.make_track(format!("Synth {i}"));
        p.tracks.push(t);
    }
    let mut kit = p.make_track("Kit");
    kit.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
    let mut clip = p.make_clip("Four", Ticks::ZERO, Ticks::beats(4));
    for i in 0..4 {
        clip.notes
            .push(p.make_note(Ticks::QUARTER * i, Ticks::SIXTEENTH, 36, 110));
    }
    kit.clips.push(clip);
    p.tracks.push(kit);
    let mut r = Rig::new(p);
    let last = r.app.project().tracks.len() - 1;
    let id = r.track(last);
    let c = r.app.project().tracks[last].clips[0].id;
    r.app.arrangement.scroll_y = 100_000.0;
    r.idle(3);
    r.double_click(r.clip_at(last, 0));
    assert_eq!(r.app.central, CentralView::ChannelRack);
    assert_eq!(
        (r.app.selected_track, r.app.selected_clip),
        (Some(id), Some(c))
    );
    r.idle(3);
    let geo = r.app.rack.geometry().expect("the rack drew");
    let row = geo
        .rows
        .iter()
        .find(|row| row.track == last)
        .expect("the kit's row");
    assert!(
        row.top >= geo.grid_top - 0.5 && row.top + row.height <= r.size.y,
        "the kit's row on screen: {} (grid from {})",
        row.top,
        geo.grid_top
    );
}

/// Review 2 = 10: a docked editor under a floating window cannot be raised above it, so
/// the window over it does not make it "not in front": F8 closes it.
#[test]
fn f8_closes_a_docked_editor_under_a_floating_window() {
    let mut r = Rig::new(band());
    let lead = r.track(LEAD);
    r.app.select_track(lead);
    r.idle(2);
    let key = cantus_ui::views::inst::editor_id(lead).value();
    r.key(Key::F8, Modifiers::NONE);
    r.idle(3);
    r.app.workspace.hide(Pane::Device(key));
    r.app.workspace.reveal(Pane::Device(key));
    r.idle(3);
    assert!(r
        .app
        .workspace
        .find(Pane::Device(key))
        .unwrap()
        .surface
        .is_main());
    let body = device::with_registry(&r.ctx, |g| g.get(key).and_then(|e| e.body))
        .expect("drawn")
        .rect;
    // An EQ's editor floating over the tab's middle.
    let strip = r.app.project().strips[0].id;
    let slot = r.app.bus.new_effect(cantus_core::EffectKind::Eq);
    let fx = slot.id;
    assert!(r.app.exec(Command::InsertEffect {
        strip,
        index: 0,
        slot: Box::new(slot)
    }));
    let fx_key = cantus_ui::views::fx::device_id(fx).value();
    let at = Rect::from_center_size(body.center(), vec2(400.0, 300.0));
    device::remember_place(&r.ctx, fx_key, at);
    r.app.open_effect_editor(strip, fx);
    r.idle(4);
    assert_ne!(
        r.ctx.layer_id_at(body.center()).map(|l| l.order),
        Some(egui::Order::Background)
    );
    r.app.select_track(lead);
    r.key(Key::F8, Modifiers::NONE);
    r.idle(3);
    assert_eq!(r.app.inst.open, None, "F8 closed the docked editor");
}

/// Review 1: on short rows the name's letters reach into the row above's band and this
/// row's resize grip; a click on them still selects this track and opens its name.
#[test]
fn the_letters_win_over_the_neighbours_bands_on_short_rows() {
    for (base, height) in [(40.0f32, 0.34f32), (22.0, 0.6)] {
        let mut p = band();
        for t in &mut p.tracks {
            t.height = height;
        }
        let mut r = Rig::new(p);
        r.app.arrangement.track_height = base;
        r.idle(3);
        for (what, dy) in [("top", 1.0f32), ("bottom", -1.0)] {
            let (_, letters) = r.header(DRUMS);
            let y = if dy > 0.0 {
                letters.top() + dy
            } else {
                letters.bottom() + dy
            };
            let at = pos2(letters.center().x, y);
            r.click(at);
            assert_eq!(
                r.app.selected_track,
                Some(r.track(DRUMS)),
                "{base}×{height} {what}: this track selected"
            );
            assert_eq!(
                r.renaming(),
                Some(r.track(DRUMS)),
                "{base}×{height} {what}: its name opened"
            );
            r.key(Key::Escape, Modifiers::NONE);
            assert_eq!(r.renaming(), None);
            r.hover(at);
            assert_eq!(
                r.cursor,
                CursorIcon::Text,
                "{base}×{height} {what}: an I-beam"
            );
        }
    }
}

// ---- 3. Open editor brings the window back and to the front ---------------------------

/// The device's layer, as the workspace recorded it.
fn layer_of(ctx: &egui::Context, key: u64) -> Option<egui::LayerId> {
    device::with_registry(ctx, |r| r.get(key).and_then(|e| e.body).map(|b| b.layer))
}

#[test]
fn open_editor_restores_a_minimised_instrument_and_brings_it_in_front() {
    let mut r = Rig::new(band());
    let lead = r.track(LEAD);
    r.app.open_inst_editor(lead);
    r.idle(4);
    let key = r.app.instrument_window_key(lead).unwrap();
    assert!(r.app.workspace.is_visible(Pane::Device(key)));
    // Minimised to the tray: gone from the dock, still open.
    device::minimise(&r.ctx, key);
    r.idle(3);
    assert!(!r.app.workspace.contains(Pane::Device(key)));
    assert!(!r.app.instrument_window_shown(lead));
    // The inspector's *Open editor* (and the instrument frame's): back, focused, in front.
    r.app.open_inst_editor(lead);
    r.idle(3);
    assert!(!device::is_minimised(&r.ctx, key), "restored");
    assert!(
        r.app.workspace.is_visible(Pane::Device(key)),
        "its tab in front"
    );
    assert_eq!(
        device::focused(&r.ctx),
        Some(key),
        "focused (Esc closes it)"
    );
    // Docked beside the arrangement, behind it: brought in front. (The editor floats at
    // first when the arrangement's leaf is too short for it; put back in the dock it
    // stays a tab there.)
    r.app.workspace.hide(Pane::Device(key));
    r.app.workspace.reveal(Pane::Device(key));
    r.idle(2);
    let path = r.app.workspace.find(Pane::Device(key)).expect("a tab");
    assert!(path.surface.is_main(), "docked");
    r.app.workspace.reveal(Pane::Arrangement);
    r.idle(2);
    assert!(
        !r.app.workspace.is_visible(Pane::Device(key)),
        "behind the arrangement"
    );
    r.app.open_instrument_window(lead);
    r.idle(3);
    assert!(
        r.app.workspace.is_visible(Pane::Device(key)),
        "in front again"
    );
    // The kit's editor the same way.
    let drums = r.track(DRUMS);
    cantus_ui::views::pulsus_editor::open(&mut r.app, drums);
    r.idle(4);
    let kit = r.app.instrument_window_key(drums).unwrap();
    device::minimise(&r.ctx, kit);
    r.idle(3);
    r.app.open_instrument_window(drums);
    r.idle(3);
    assert!(!device::is_minimised(&r.ctx, kit));
    assert!(r.app.workspace.is_visible(Pane::Device(kit)));
}

#[test]
fn open_editor_raises_a_floating_editor_above_another() {
    let mut r = Rig::new(band());
    let (lead, drums) = (r.track(LEAD), r.track(DRUMS));
    let a = cantus_ui::views::inst::editor_id(lead).value();
    let b = cantus_ui::views::pulsus_editor::editor_id(drums).value();
    // Both float where they floated before, overlapping.
    let place = |x: f32, y: f32| Rect::from_min_size(pos2(x, y), vec2(700.0, 450.0));
    device::remember_place(&r.ctx, a, place(300.0, 200.0));
    device::remember_place(&r.ctx, b, place(420.0, 260.0));
    r.app.open_inst_editor(lead);
    r.idle(4);
    cantus_ui::views::pulsus_editor::open(&mut r.app, drums);
    r.idle(4);
    let overlap = pos2(700.0, 500.0);
    let (la, lb) = (layer_of(&r.ctx, a).unwrap(), layer_of(&r.ctx, b).unwrap());
    assert_ne!(la, lb, "two floating windows");
    assert_eq!(
        r.ctx.layer_id_at(overlap),
        Some(lb),
        "the kit's, opened last, on top"
    );
    r.app.open_inst_editor(lead);
    r.idle(3);
    assert_eq!(
        r.ctx.layer_id_at(overlap),
        layer_of(&r.ctx, a),
        "Open editor raised the instrument's window"
    );
    assert_eq!(device::focused(&r.ctx), Some(a));
}

#[test]
fn open_editor_restores_a_minimised_effect_editor() {
    let mut r = Rig::new(band());
    let strip = r.app.project().strips[0].id;
    let slot = r.app.bus.new_effect(cantus_core::EffectKind::Eq);
    let fx = slot.id;
    assert!(r.app.exec(Command::InsertEffect {
        strip,
        index: 0,
        slot: Box::new(slot)
    }));
    // The mixer's *Open the editor*, and the inspector's slot.
    r.app.open_effect_editor(strip, fx);
    r.idle(4);
    let key = cantus_ui::views::fx::device_id(fx).value();
    device::minimise(&r.ctx, key);
    r.idle(2);
    assert!(device::is_minimised(&r.ctx, key));
    r.app.open_effect_editor(strip, fx);
    r.idle(3);
    assert!(!device::is_minimised(&r.ctx, key), "restored");
    assert_eq!(device::focused(&r.ctx), Some(key));
    assert!(device::shown(&r.ctx, key).is_some(), "drawn");
}

// ---- round 27, the coordinator's two leftovers ----------------------------------------

/// F8 (and the rack's channel button, which runs the same toggle) is a toggle on what the
/// user sees: an editor that is open but not in front — minimised, its tab behind another
/// tab, its window under another — comes forward; only the frontmost one closes.
#[test]
fn f8_brings_an_editor_forward_and_closes_only_the_frontmost() {
    let mut r = Rig::new(band());
    let (lead, drums) = (r.track(LEAD), r.track(DRUMS));
    r.app.select_track(lead);
    r.idle(2);
    let key = cantus_ui::views::inst::editor_id(lead).value();
    // Closed → F8 opens it, in front.
    r.key(Key::F8, Modifiers::NONE);
    r.idle(3);
    assert_eq!(r.app.inst.open, Some(lead), "F8 opened it");
    assert!(r.app.instrument_window_in_front(lead), "in front");
    // Minimised → F8 brings it back (it used to... open it again, minimised).
    device::minimise(&r.ctx, key);
    r.idle(3);
    assert!(!r.app.instrument_window_in_front(lead));
    r.key(Key::F8, Modifiers::NONE);
    r.idle(3);
    assert_eq!(r.app.inst.open, Some(lead), "minimised: still open");
    assert!(!device::is_minimised(&r.ctx, key), "brought back");
    assert!(r.app.instrument_window_in_front(lead));
    // Docked behind the arrangement → F8 brings its tab forward.
    r.app.workspace.hide(Pane::Device(key));
    r.app.workspace.reveal(Pane::Device(key));
    r.idle(2);
    r.app.workspace.reveal(Pane::Arrangement);
    r.idle(3);
    assert!(!r.app.instrument_window_in_front(lead), "behind a tab");
    r.key(Key::F8, Modifiers::NONE);
    r.idle(3);
    assert_eq!(
        r.app.inst.open,
        Some(lead),
        "behind a tab: it used to close"
    );
    assert!(
        r.app.workspace.is_visible(Pane::Device(key)),
        "its tab in front"
    );
    assert!(r.app.instrument_window_in_front(lead));
    // In front → F8 closes it.
    r.key(Key::F8, Modifiers::NONE);
    r.idle(3);
    assert_eq!(r.app.inst.open, None, "the frontmost one closes");

    // Floating under another floating window → F8 raises it; F8 again closes it.
    let kit = cantus_ui::views::pulsus_editor::editor_id(drums).value();
    let place = |x: f32, y: f32| Rect::from_min_size(pos2(x, y), vec2(700.0, 450.0));
    device::remember_place(&r.ctx, key, place(300.0, 200.0));
    device::remember_place(&r.ctx, kit, place(320.0, 220.0));
    r.app.open_inst_editor(lead);
    r.idle(4);
    cantus_ui::views::pulsus_editor::open(&mut r.app, drums);
    r.idle(4);
    r.app.select_track(lead);
    r.idle(2);
    assert!(
        !r.app.instrument_window_in_front(lead),
        "under the kit's window"
    );
    r.key(Key::F8, Modifiers::NONE);
    r.idle(3);
    assert_eq!(
        r.app.inst.open,
        Some(lead),
        "under a window: it used to close"
    );
    assert!(r.app.instrument_window_in_front(lead), "raised");
    r.key(Key::F8, Modifiers::NONE);
    r.idle(3);
    assert_eq!(r.app.inst.open, None, "then F8 closes it");
}

/// With one name open, a click on another track's letters commits the first (one undo
/// step) and opens the second in the same click, its name selected — whichever is above.
#[test]
fn a_click_on_another_name_commits_the_first_and_opens_the_second() {
    for (first, second) in [(LEAD, BASS), (BASS, LEAD)] {
        let mut r = Rig::new(band());
        let (_, letters) = r.header(first);
        r.click(letters.center());
        assert_eq!(r.renaming(), Some(r.track(first)));
        r.type_text("Zed");
        let rev = r.app.bus.revision();
        let (_, letters) = r.header(second);
        r.click(letters.center());
        assert_eq!(r.name(first), "Zed", "{first}: the first rename committed");
        assert_eq!(r.app.bus.revision(), rev + 1, "{first}: one undo step");
        assert_eq!(
            r.renaming(),
            Some(r.track(second)),
            "{first}→{second}: the second field opened in the same click"
        );
        assert_eq!(r.app.selected_track, Some(r.track(second)));
        assert!(!r.keyboard_free(), "it holds the keyboard");
        r.type_text("Wren");
        r.key(Key::Enter, Modifiers::NONE);
        assert_eq!(r.name(second), "Wren", "its name was selected: typed over");
        assert_eq!(r.app.bus.revision(), rev + 2);
    }
}

/// Looked at in the pictures (round 27): on a short row the name's field and its words fit
/// the row — at full size they overflowed into the rows round it.
#[test]
fn on_short_rows_the_name_field_stays_inside_its_row() {
    for (base, height) in [(40.0f32, 0.34f32), (22.0, 0.6)] {
        let mut p = band();
        for t in &mut p.tracks {
            t.height = height;
        }
        let mut r = Rig::new(p);
        r.app.arrangement.track_height = base;
        r.idle(3);
        let (row, letters) = r.header(DRUMS);
        assert!(
            letters.top() >= row.top() - 0.5 && letters.bottom() <= row.bottom() + 0.5,
            "{base}×{height}: the letters inside the row"
        );
        r.click(letters.center());
        assert_eq!(r.renaming(), Some(r.track(DRUMS)));
        let id = cantus_ui::kit::field::edit_id(egui::Id::new(("track-rename", r.track(DRUMS).0)));
        let edit = r.ctx.read_response(id).expect("the field drawn").rect;
        assert!(
            edit.top() >= row.top() - 0.5 && edit.bottom() <= row.bottom() + 0.5,
            "{base}×{height}: the field {edit:?} inside the row {row:?}"
        );
    }
}

/// The coordinator's last leftover: on short rows a clip's label is scaled to the clip
/// and never drawn outside it; below a readable size it is hidden, and its name is in the
/// hover hint.
#[test]
fn on_short_rows_a_clips_label_fits_the_clip_or_hides() {
    for (base, height, shown) in [
        (40.0f32, 0.34f32, true),
        (22.0, 0.6, true),
        (22.0, 0.34, false),
    ] {
        let mut p = band();
        for t in &mut p.tracks {
            t.height = height;
        }
        let mut r = Rig::new(p);
        r.app.arrangement.track_height = base;
        r.idle(3);
        let geo = r.app.arrangement.geometry().unwrap();
        let row = geo.rows[LEAD];
        let clip = &r.app.project().tracks[LEAD].clips[0];
        let (a, b) = (
            geo.x(clip.start.as_beats_f64() as f32),
            geo.x(clip.end().as_beats_f64() as f32),
        );
        // The clip's body, as the arrangement draws it: the row less a small inset.
        let inset = (row.height * 0.04).clamp(1.0, 2.0);
        let body = Rect::from_min_max(
            pos2(a, row.top + inset),
            pos2(b, row.top + row.height - inset),
        );
        let drawn: Vec<Rect> = r
            .texts
            .iter()
            .filter(|(s, _)| s == "Melody")
            .map(|(_, rect)| *rect)
            .collect();
        if shown {
            assert!(!drawn.is_empty(), "{base}×{height}: the label is drawn");
            for d in &drawn {
                assert!(
                    d.top() >= body.top() - 0.5 && d.bottom() <= body.bottom() + 0.5,
                    "{base}×{height}: the label {d:?} inside the clip {body:?}"
                );
            }
        } else {
            assert!(
                drawn.is_empty(),
                "{base}×{height}: too short to read: hidden"
            );
        }
        // Its name is in the hover hint whatever the height.
        r.hover(pos2(a + 6.0, row.top + row.height * 0.5));
        let h = cantus_ui::kit::hint::current(&r.ctx).expect("the clip's hint");
        assert_eq!(h.name, "Melody", "{base}×{height}: {h:?}");
    }
}
