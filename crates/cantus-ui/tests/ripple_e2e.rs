//! r40:ripple — insert and delete time and the section drag end to end (ADR 0116), through
//! `CantusApp::assemble` as the app starts, the whole window drawn, the pointer and the keys
//! where a user puts them: a time selection's menu inserts its time on every track (the
//! sections and markers with it), Ctrl+Del deletes it again and the song is back as it was;
//! Insert alone puts a bar in at the playhead; a section dragged moves its bars in (the song
//! rearranged), replaces where it lands, or copies over it — under both settings of *Section
//! drag* (Insert, the default an old settings file opens with, and Replace: Shift gives the
//! other one); the ripple mode's key and its follow on a delete and a drag; each one undo
//! step, and the song saved and loaded back. The real engine plays the result: a bar
//! inserted from the keys moves the song's sound by its length, to the sample (review
//! mid-round 40, B-4). The time keys are the arrangement's alone: Ctrl+Del over an EQ's band
//! takes the band only, Insert over the mixer inserts nothing (C-3); they reach the
//! arrangement floating or maximised, on its ruler, sections lane and tool row, and wherever
//! the mouse rests once Ctrl+F6 brought the focus there; refused, the status line says why
//! (review 2, R2-B1).
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::arrange::{ArrangeCommand, Section, SectionId};
use cantus_core::{Command, Project, Ticks};
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use egui::{pos2, Modifiers, Pos2};
use std::path::Path;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
        d
    })
    .path()
}

fn bar(n: i64) -> Ticks {
    Ticks(Ticks::beats(4).0 * n)
}

struct W {
    app: CantusApp,
    ctx: egui::Context,
    t: f64,
    texts: Vec<(String, egui::Rect)>,
    /// The engine's processor, driven as the device's callback drives it.
    proc_: Processor,
}

impl W {
    fn new() -> W {
        W::with(cantus_ui::demos::startup_project())
    }

    fn with(project: Project) -> W {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, proc_) = EngineHandle::manual(48_000.0);
        let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
        let mut w = W {
            app,
            ctx,
            t: 0.0,
            texts: Vec::new(),
            proc_,
        };
        for _ in 0..4 {
            w.frame(vec![]);
        }
        w
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.frame_with(events, Modifiers::NONE);
    }

    fn frame_with(&mut self, events: Vec<egui::Event>, modifiers: Modifiers) {
        self.t += 1.0 / 60.0;
        // egui holds the modifiers from the last change it was told of.
        let events: Vec<egui::Event> = std::iter::once(egui::Event::ModifiersChanged(modifiers))
            .chain(events)
            .collect();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1920.0, 1080.0),
            )),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        self.texts.clear();
        for c in &out.shapes {
            walk(&c.shape, &mut self.texts);
        }
    }

    fn word(&self, word: &str) -> Option<egui::Rect> {
        self.texts
            .iter()
            .rev()
            .find(|(t, _)| t.trim() == word)
            .map(|(_, r)| *r)
    }

    fn button(at: Pos2, button: egui::PointerButton, pressed: bool, m: Modifiers) -> egui::Event {
        egui::Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: m,
        }
    }

    fn click_with(&mut self, at: Pos2, button: egui::PointerButton) {
        let m = Modifiers::NONE;
        self.frame(vec![egui::Event::PointerMoved(at)]);
        self.frame(vec![W::button(at, button, true, m)]);
        self.frame(vec![W::button(at, button, false, m)]);
        self.frame(vec![]);
    }

    fn drag_with(&mut self, from: Pos2, to: Pos2, m: Modifiers) {
        let primary = egui::PointerButton::Primary;
        self.frame_with(vec![egui::Event::PointerMoved(from)], m);
        self.frame_with(vec![W::button(from, primary, true, m)], m);
        for i in 1..=8 {
            let p = from + (to - from) * (i as f32 / 8.0);
            self.frame_with(vec![egui::Event::PointerMoved(p)], m);
        }
        self.frame_with(vec![W::button(to, primary, false, m)], m);
        self.frame(vec![]);
    }

    fn key(&mut self, key: egui::Key, m: Modifiers) {
        let ev = |pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: m,
        };
        self.frame_with(vec![ev(true)], m);
        self.frame_with(vec![ev(false)], m);
        self.frame(vec![]);
    }

    fn geo(&self) -> cantus_ui::views::arrangement::TimelineGeometry {
        self.app
            .arrangement
            .geometry()
            .expect("the arrangement was drawn")
    }

    fn x_of(&self, t: Ticks) -> f32 {
        let g = self.geo();
        g.x(t.0 as f32 / Ticks::beats(1).0 as f32)
    }

    fn section_rect(&self, id: SectionId) -> egui::Rect {
        self.app
            .arrangement
            .sections
            .drawn
            .iter()
            .find(|(s, _)| *s == id)
            .map(|(_, r)| *r)
            .expect("the section drawn in its lane")
    }

    fn round_trip(&self) -> Project {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.cantus");
        cantus_project::save(self.app.project(), &path, "test").unwrap();
        cantus_project::load(&path, false).unwrap().project
    }
}

/// Every clip of `before` as `after` holds it once `len` was inserted at `at`: the ones
/// before kept, the ones after moved by `len`, one across cut there with its rest after.
fn holds_the_insert(before: &Project, after: &Project, at: Ticks, len: Ticks) {
    for (tb, ta) in before.tracks.iter().zip(&after.tracks) {
        for c in &tb.clips {
            let now = ta.clip(c.id).expect("the clip kept its id");
            if c.end() <= at {
                assert_eq!((now.start, now.length), (c.start, c.length), "{}", c.name);
            } else if c.start >= at {
                assert_eq!((now.start, now.length), (c.start + len, c.length));
            } else {
                assert_eq!((now.start, now.end()), (c.start, at), "cut at the insert");
                let rest = ta
                    .clips
                    .iter()
                    .find(|r| r.start == at + len && r.name == c.name && r.lane == c.lane)
                    .expect("its rest after the new time");
                assert_eq!(rest.end(), c.end() + len);
            }
        }
    }
}

fn sections(w: &mut W) -> [SectionId; 3] {
    let mut ids = [SectionId(0); 3];
    for (i, (s, l, n)) in [(0, 4, "Intro"), (4, 8, "Drop"), (12, 4, "Outro")]
        .into_iter()
        .enumerate()
    {
        let id = SectionId(w.app.bus.alloc_id());
        ids[i] = id;
        assert!(w.app.exec(Command::Arrange(ArrangeCommand::InsertSection {
            index: i,
            section: Section {
                id,
                start: bar(s),
                length: bar(l),
                name: n.into(),
                color: None,
            },
        })));
    }
    w.frame(vec![]);
    w.frame(vec![]);
    ids
}

#[test]
fn insert_and_delete_time_and_the_section_drag_end_to_end() {
    let mut w = W::new();
    let ids = sections(&mut w);
    time_from_the_menu_and_the_keys(&mut w, ids);
    // Settings ▸ Editing's *Section drag*: an old settings file (this one has no such field)
    // opens with Insert, the default — a plain drag puts the bars in, Ctrl+Shift copies
    // them over.
    use cantus_ui::ripple_ui::SectionDrag;
    assert_eq!(w.app.settings.section_drag, SectionDrag::Insert);
    a_section_dragged_moves_its_bars_in(&mut w, ids, Modifiers::NONE);
    a_section_dragged_copies_its_bars_over(&mut w, ids, Modifiers::COMMAND | Modifiers::SHIFT);
    a_section_dragged_replaces(&mut w, ids, Modifiers::SHIFT);
    // Replace: the other way round — Shift puts the bars in, Ctrl copies them over.
    w.app.set_section_drag(SectionDrag::Replace);
    a_section_dragged_moves_its_bars_in(&mut w, ids, Modifiers::SHIFT);
    a_section_dragged_copies_its_bars_over(&mut w, ids, Modifiers::COMMAND);
    a_section_dragged_replaces(&mut w, ids, Modifiers::NONE);
    let stored = std::fs::read_to_string(data().join("settings.json")).unwrap_or_default();
    let squeezed: String = stored.split_whitespace().collect();
    assert!(
        squeezed.contains("\"section_drag\":\"replace\""),
        "kept in the settings file"
    );
    w.app.set_section_drag(SectionDrag::Insert);
    the_ripple_mode_moves_what_follows(&mut w);
}

fn a_section_dragged_replaces(w: &mut W, ids: [SectionId; 3], m: Modifiers) {
    let before = w.app.project().clone();
    w.frame(vec![]);
    let r = w.section_rect(ids[0]);
    let from = pos2(r.left() + 10.0, r.center().y);
    let to = pos2(w.x_of(bar(12)) + 10.0, r.center().y);
    w.drag_with(from, to, m);
    let p = w.app.project().clone();
    let intro = p.section(ids[0]).unwrap();
    assert_eq!(
        (intro.start, intro.length),
        (bar(12), bar(4)),
        "the Intro landed"
    );
    assert!(p.section(ids[2]).is_none(), "the Outro's bars replaced");
    for t in &p.tracks {
        assert!(
            !t.clips
                .iter()
                .any(|c| c.lane == 0 && c.start < bar(4) && c.end() > Ticks::ZERO),
            "where the Intro was is silent on {}",
            t.name
        );
    }
    w.app.undo();
    let mut now = w.app.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before, "one undo step");
}

/// The first two clips on the playing lane of track `i`, by start.
fn first_two(p: &Project, i: usize) -> (cantus_core::Clip, cantus_core::Clip) {
    let mut c: Vec<_> = p.tracks[i]
        .clips
        .iter()
        .filter(|c| c.lane == 0 && c.automation.is_none())
        .cloned()
        .collect();
    c.sort_by_key(|c| c.start);
    (c[0].clone(), c[1].clone())
}

fn the_ripple_mode_moves_what_follows(w: &mut W) {
    use cantus_core::ripple_mode::RippleMode;
    let original = w.app.project().clone();
    let (a, b) = first_two(&original, 0);
    assert_eq!(w.app.arrangement.ripple, RippleMode::Off, "off at start");
    assert!(w.word("Ripple · this track").is_none(), "no sign while off");
    // Alt+Shift+P: this track. The key says so in the tool row.
    w.key(egui::Key::P, Modifiers::ALT | Modifiers::SHIFT);
    assert_eq!(w.app.arrangement.ripple, RippleMode::Track);
    w.frame(vec![]);
    assert!(
        w.word("Ripple · this track").is_some(),
        "the tool row's sign"
    );
    // A click selects the first clip on the first track; Delete closes its gap there.
    let g = w.geo();
    let y = g.rows[0].top + g.rows[0].height * 0.5;
    w.click_with(
        pos2(w.x_of(a.start + Ticks::beats(2)), y),
        egui::PointerButton::Primary,
    );
    w.key(egui::Key::Delete, Modifiers::NONE);
    let p = w.app.project().clone();
    assert!(p.tracks[0].clip(a.id).is_none(), "deleted");
    assert_eq!(
        p.tracks[0].clip(b.id).unwrap().start,
        b.start - a.length,
        "what followed on its track closed the gap"
    );
    assert_eq!(
        p.tracks[1].clips, original.tracks[1].clips,
        "the other tracks stay"
    );
    w.app.undo();
    let mut back = w.app.project().clone();
    back.next_id = original.next_id;
    assert_eq!(back, original, "one undo step");
    // A clip dragged two bars on: what follows on its track goes with it.
    w.frame(vec![]);
    let g = w.geo();
    let y = g.rows[0].top + g.rows[0].height * 0.5;
    let from = pos2(w.x_of(a.start + Ticks::beats(6)), y);
    let to = pos2(w.x_of(a.start + Ticks::beats(6) + bar(2)), y);
    w.drag_with(from, to, Modifiers::NONE);
    let p = w.app.project().clone();
    assert_eq!(p.tracks[0].clip(a.id).unwrap().start, a.start + bar(2));
    assert_eq!(p.tracks[0].clip(b.id).unwrap().start, b.start + bar(2));
    w.app.undo();
    // A paste goes in: copy the first clip (Ctrl+C), paste it at the playhead (Ctrl+V, the
    // song's start): the clips that were there move on by its length.
    w.frame(vec![]);
    w.click_with(
        pos2(w.x_of(a.start + Ticks::beats(2)), y),
        egui::PointerButton::Primary,
    );
    w.frame_with(vec![egui::Event::Copy], Modifiers::COMMAND);
    w.frame_with(vec![egui::Event::Paste(String::new())], Modifiers::COMMAND);
    w.frame(vec![]);
    let p = w.app.project().clone();
    let pasted = p.tracks[0]
        .clips
        .iter()
        .find(|c| c.start == Ticks::ZERO && c.id != a.id)
        .expect("the copy at the song's start");
    assert_eq!(pasted.length, a.length);
    assert_eq!(p.tracks[0].clip(a.id).unwrap().start, a.start + a.length);
    assert_eq!(p.tracks[0].clip(b.id).unwrap().start, b.start + a.length);
    assert_eq!(
        p.tracks[1].clips, original.tracks[1].clips,
        "this track only"
    );
    w.app.undo();
    // Every track: the marks move too; then off again.
    w.key(egui::Key::P, Modifiers::ALT | Modifiers::SHIFT);
    assert_eq!(w.app.arrangement.ripple, RippleMode::All);
    w.frame(vec![]);
    assert!(w.word("Ripple · every track").is_some());
    w.key(egui::Key::P, Modifiers::ALT | Modifiers::SHIFT);
    assert_eq!(w.app.arrangement.ripple, RippleMode::Off);
    let mut back = w.app.project().clone();
    back.next_id = original.next_id;
    assert_eq!(back, original);
}

fn time_from_the_menu_and_the_keys(w: &mut W, ids: [SectionId; 3]) {
    let original = w.app.project().clone();
    let undo_depth = w.app.bus.history().len();
    // The time selection over bars 5–6 (4 and 5, 0-based), and the empty timeline's menu.
    w.app.set_time_selection(bar(4), bar(6));
    let g = w.geo();
    // Inside the selection, over the first track (a clip there or not: the click is the
    // selection's).
    let at = pos2(w.x_of(bar(5)), g.rows[0].top + g.rows[0].height * 0.5);
    w.click_with(at, egui::PointerButton::Secondary);
    let Some(item) = w.word("Insert time") else {
        let words: Vec<&str> = w.texts.iter().map(|t| t.0.as_str()).collect();
        panic!("the time selection's menu says Insert time; drawn: {words:?}");
    };
    assert!(w.word("Delete time").is_some(), "and Delete time");
    w.click_with(item.center(), egui::PointerButton::Primary);
    holds_the_insert(&original, w.app.project(), bar(4), bar(2));
    let p = w.app.project();
    let drop = p.section(ids[1]).unwrap();
    assert_eq!(
        (drop.start, drop.length),
        (bar(6), bar(8)),
        "the Drop began at the insert: it moved on whole"
    );
    assert_eq!(
        p.section(ids[0]).unwrap().end(),
        bar(4),
        "the Intro ended there: kept"
    );
    assert_eq!(
        p.section(ids[2]).unwrap().start,
        bar(14),
        "the Outro moved on"
    );
    assert_eq!(
        w.app.time_selection,
        Some((bar(4), bar(6))),
        "the new time selected"
    );
    assert_eq!(w.app.bus.history().len(), undo_depth + 1, "one undo step");
    // Ctrl+Del takes it out again: the song as it was, every cut healed.
    w.key(egui::Key::Delete, Modifiers::COMMAND);
    let mut now = w.app.project().clone();
    now.next_id = original.next_id;
    assert_eq!(
        now, original,
        "insert then delete: the song back to the tick"
    );
    assert_eq!(w.app.time_selection, None);
    // Insert alone, with no selection: a bar at the playhead's bar (the song's start).
    w.key(egui::Key::Insert, Modifiers::NONE);
    holds_the_insert(&original, w.app.project(), Ticks::ZERO, bar(1));
    w.app.undo();
    w.app.undo();
    w.app.undo();
    let mut back = w.app.project().clone();
    back.next_id = original.next_id;
    assert_eq!(back, original, "each one undo step");
    assert_eq!(w.round_trip().sections, w.app.project().sections);
}

fn a_section_dragged_moves_its_bars_in(w: &mut W, ids: [SectionId; 3], m: Modifiers) {
    let before = w.app.project().clone();
    w.frame(vec![]);
    let r = w.section_rect(ids[0]);
    let from = pos2(r.left() + 10.0, r.center().y);
    // The Intro's start dropped on the Outro's: its bars go in after the Drop.
    let to = pos2(w.x_of(bar(12)) + 10.0, r.center().y);
    w.drag_with(from, to, m);
    let p = w.app.project();
    let s = |i: usize| {
        let s = p.section(ids[i]).unwrap();
        (s.start, s.length)
    };
    assert_eq!(s(1), (Ticks::ZERO, bar(8)), "the Drop opens the song");
    assert_eq!(s(0), (bar(8), bar(4)), "the Intro after it");
    assert_eq!(s(2), (bar(12), bar(4)), "the Outro where it was");
    // Every clip that lay in the Drop now lies 4 bars earlier.
    for (tb, ta) in before.tracks.iter().zip(&p.tracks) {
        for c in tb
            .clips
            .iter()
            .filter(|c| c.start >= bar(4) && c.end() <= bar(12))
        {
            let now = ta.clip(c.id).expect("moved, not copied");
            assert_eq!(now.start, c.start - bar(4), "{}", c.name);
        }
    }
    w.app.undo();
    assert_eq!(w.app.project().sections, before.sections, "one undo step");
    let mut back = w.app.project().clone();
    back.next_id = before.next_id;
    assert_eq!(back, before);
}

fn a_section_dragged_copies_its_bars_over(w: &mut W, ids: [SectionId; 3], m: Modifiers) {
    let before = w.app.project().clone();
    w.frame(vec![]);
    let r = w.section_rect(ids[0]);
    let from = pos2(r.left() + 10.0, r.center().y);
    let to = pos2(w.x_of(bar(12)) + 10.0, r.center().y);
    w.drag_with(from, to, m);
    let p = w.app.project().clone();
    assert_eq!(
        p.section(ids[0]).unwrap().start,
        Ticks::ZERO,
        "the Intro stays"
    );
    let copy = p
        .sections
        .iter()
        .find(|s| s.name == "Intro" && s.start == bar(12))
        .expect("a copy of the Intro where it landed");
    assert_ne!(copy.id, ids[0]);
    assert!(
        p.section(ids[2]).is_none(),
        "the Outro's bars were replaced, and the Outro with them"
    );
    for (tb, ta) in before.tracks.iter().zip(&p.tracks) {
        let src: Vec<_> = tb.clips.iter().filter(|c| c.end() <= bar(4)).collect();
        for c in src {
            assert!(
                ta.clips.iter().any(|x| x.id != c.id
                    && x.start == c.start + bar(12)
                    && x.notes.len() == c.notes.len()),
                "a copy of {} at bar {}",
                c.name,
                (c.start + bar(12)).0 / bar(1).0
            );
        }
    }
    let back = w.round_trip();
    assert_eq!(back.sections, p.sections);
    w.app.undo();
    let mut now = w.app.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before, "one undo step");
}

/// A kit's groove from the song's start, at 120 BPM in 4/4 (the keys' test's song).
fn kit_song() -> Project {
    use cantus_core::{InstrumentSpec, PulsusParams, PPQ};
    let mut p = Project::new("Ripple kit");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    let pitches: Vec<u8> = t
        .instrument
        .pulsus()
        .unwrap()
        .pads
        .iter()
        .map(|x| x.pitch)
        .collect();
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(8));
    for step in 0..32i64 {
        let at = Ticks(step * PPQ / 4);
        let pitch = pitches[((step * 5) as usize) % pitches.len()];
        c.notes
            .push(p.make_note(at, Ticks(PPQ / 3), pitch, 50 + (step * 13 % 70) as u8));
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// A take of 3.3 s at 48 kHz: struck tones, one every quarter of a second, each its own
/// pitch — audio holds no state from one block to the next, so a bar of silence before it
/// moves it to the sample (an instrument's voices carry their idle state across the bar).
fn take() -> std::path::PathBuf {
    let path = data().join("ripple-take.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    let hit = 12_000usize;
    for i in 0..158_400usize {
        let t = (i % hit) as f32 / 48_000.0;
        let f = 180.0 + 37.0 * (i / hit) as f32;
        let v = 0.6 * (-t * 9.0).exp() * (std::f32::consts::TAU * f * t).sin();
        w.write_sample((v * 30_000.0) as i16).unwrap();
    }
    w.finalize().unwrap();
    path
}

const BAR_FRAMES: usize = 96_000;

/// Play from the top and take `frames` of what the engine hands the device, in blocks of
/// several sizes, as a callback does.
fn play(w: &mut W, frames: usize) -> Vec<f32> {
    w.app.sync_engine();
    w.app.seek(Ticks::ZERO);
    w.app.toggle_play();
    w.app.sync_engine();
    let mut out = Vec::with_capacity(frames * 2);
    let sizes = [512usize, 256, 333, 1024];
    let mut i = 0;
    while out.len() < frames * 2 {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        w.proc_.process(&mut buf, 2);
        out.extend_from_slice(&buf);
    }
    w.app.toggle_play();
    w.app.sync_engine();
    out
}

/// What the export's engine makes of `p`, its first `frames`.
fn offline(w: &mut W, p: &Project, frames: usize) -> Vec<f32> {
    let snap = w.app.engine.compile_for_render(p, 1);
    let mut r = render(
        snap,
        &RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
    )
    .unwrap()
    .samples;
    r.resize(frames * 2, 0.0);
    r
}

/// Over the first track's row, at bar `b` (the arrangement's body).
fn over_the_arrangement(w: &W, b: i64) -> Pos2 {
    let g = w.geo();
    pos2(w.x_of(bar(b)), g.rows[0].top + g.rows[0].height * 0.5)
}

// Review mid-round 40, B-4: the engine plays what the time edit made.
#[test]
fn an_inserted_bar_moves_the_sound_by_its_length_to_the_sample() {
    let mut song = Project::new("Ripple take");
    song.tempo_bpm = 120.0;
    let mut w = W::with(song);
    // The take imported at the song's start, as a file dropped there (one undo step).
    assert_eq!(w.app.import_audio(&[take()], Ticks::ZERO, None), 1);
    for _ in 0..4 {
        w.frame(vec![]);
    }
    let original = w.app.project().clone();
    let clip = &original.tracks[0].clips[0];
    assert!(
        clip.audio.is_some() && clip.start == Ticks::ZERO,
        "the take placed"
    );
    let frames = 5 * 48_000;
    let before = offline(&mut w, &original, frames);
    assert!(before.iter().any(|x| x.abs() > 0.05), "the take sounds");
    // Insert, the pointer over the arrangement: a bar at the playhead's (the song's start).
    w.app.focus_arrangement();
    let at = over_the_arrangement(&w, 3);
    w.frame(vec![egui::Event::PointerMoved(at)]);
    w.key(egui::Key::Insert, Modifiers::NONE);
    holds_the_insert(&original, w.app.project(), Ticks::ZERO, bar(1));
    // Played live by the real engine: a bar of silence, then the song as it was, to the bit.
    let live = play(&mut w, BAR_FRAMES + frames);
    if let Some(i) = live[..BAR_FRAMES * 2].iter().position(|x| *x != 0.0) {
        panic!("the new bar is silent: sample {i} is {}", live[i]);
    }
    let moved = &live[BAR_FRAMES * 2..];
    if let Some(i) = moved
        .iter()
        .zip(&before)
        .position(|(a, b)| a.to_bits() != b.to_bits())
    {
        panic!(
            "moved by a bar, to the sample: {i} is {} live, {} before",
            moved[i], before[i]
        );
    }
    // One undo step gives the song back.
    w.app.undo();
    let mut back = w.app.project().clone();
    back.next_id = original.next_id;
    assert_eq!(back, original);
}

// Review mid-round 40, C-3: one key, one action — the time keys are the arrangement's.
#[test]
fn the_time_keys_act_only_in_the_arrangement() {
    use cantus_ui::views::workspace::Pane;
    let mut w = W::with(kit_song());
    let original = w.app.project().clone();
    let insert = |w: &mut W, at: Pos2| {
        w.frame(vec![egui::Event::PointerMoved(at)]);
        w.key(egui::Key::Insert, Modifiers::NONE);
    };
    // Over the mixer (the arrangement still the keys' focus): Insert inserts nothing.
    w.app.workspace.reveal(Pane::Mixer);
    w.frame(vec![]);
    w.frame(vec![]);
    let mixer = w
        .app
        .workspace
        .areas_in_order()
        .into_iter()
        .find(|(p, _)| *p == Pane::Mixer)
        .map(|(_, r)| r)
        .expect("the mixer on screen");
    w.app.focus_arrangement();
    insert(&mut w, mixer.center());
    assert_eq!(w.app.project(), &original, "Insert over the mixer");
    // The mixer being edited (the keys' focus there), the pointer anywhere: nothing.
    w.app.follow_area_focus(Pane::Mixer);
    let over = over_the_arrangement(&w, 3);
    insert(&mut w, over);
    assert_eq!(w.app.project(), &original, "Insert with the mixer focused");
    // The arrangement's: Insert inserts.
    w.app.focus_arrangement();
    insert(&mut w, over);
    assert_ne!(w.app.project(), &original, "Insert in the arrangement");
    w.app.undo();
    // An EQ's editor open over the song, a band selected, time selected: Ctrl+Del over the
    // band takes the band, and the song's time stays.
    let strip = w.app.project().tracks[0].strip.unwrap();
    w.app.add_effect(strip, cantus_core::EffectKind::Eq);
    let fx = w
        .app
        .project()
        .strip(strip)
        .unwrap()
        .effects
        .last()
        .unwrap()
        .id;
    w.app.open_eq_editor(strip, fx);
    w.app
        .add_eq_band(strip, fx, cantus_dsp::EqShape::Bell, 1_000.0, 3.0);
    w.frame(vec![]);
    w.frame(vec![]);
    let spec = |w: &W| w.app.project().effect(strip, fx).unwrap().spec.clone();
    let used = |s: &cantus_core::EffectSpec| {
        (0..cantus_core::fx::eq::EQ_BANDS)
            .filter(|b| s.get(&format!("b{b}.used")) > 0.5)
            .collect::<Vec<_>>()
    };
    let bands = used(&spec(&w));
    let band = *bands.last().expect("a band");
    let dot = w
        .app
        .eq_editor
        .dot_pos(&spec(&w), band)
        .expect("its dot drawn");
    w.click_with(dot, egui::PointerButton::Primary);
    let song = w.app.project().clone();
    w.app.set_time_selection(bar(1), bar(2));
    w.app.focus_arrangement();
    w.frame(vec![egui::Event::PointerMoved(dot)]);
    w.key(egui::Key::Delete, Modifiers::COMMAND);
    let now = spec(&w);
    assert_eq!(used(&now).len(), bands.len() - 1, "the band went");
    assert_eq!(
        w.app.project().tracks[0].clips,
        song.tracks[0].clips,
        "the time stays"
    );
    assert_eq!(w.app.time_selection, Some((bar(1), bar(2))));
}

// Review 2 of round 40, R2-B1: the time keys reach the arrangement wherever it is — its
// ruler, its sections lane, its tool row; floating in a window of its own; maximised — and
// a keyboard user who moved there with Ctrl+F6 has them wherever the mouse rests. Refused,
// the status line says why and the way there.
#[test]
fn the_time_keys_reach_the_arrangement_wherever_it_is_and_say_why_when_not() {
    use cantus_ui::views::workspace::Pane;
    let mut w = W::with(kit_song());
    // The start page the app opens with closed (over it, the keys are its own).
    w.key(egui::Key::Escape, Modifiers::NONE);
    let ids = sections(&mut w);
    let base = w.app.project().clone();
    // Insert with the pointer at `at`: one bar in, then undone.
    fn inserts(w: &mut W, base: &Project, at: Pos2, what: &str) {
        w.frame(vec![egui::Event::PointerMoved(at)]);
        w.key(egui::Key::Insert, Modifiers::NONE);
        let seen = (w.app.workspace.pane_at(&w.ctx, at), w.ctx.layer_id_at(at));
        assert_ne!(
            w.app.project(),
            base,
            "Insert with the pointer {what}: {seen:?}, {:?}",
            w.app.status.text
        );
        w.app.undo();
        assert_eq!(w.app.project(), base, "{what}: one undo step");
    }
    fn refused(w: &mut W, base: &Project, at: Pos2, what: &str, says: &[&str]) {
        w.app.status.text.clear();
        w.frame(vec![egui::Event::PointerMoved(at)]);
        w.key(egui::Key::Insert, Modifiers::NONE);
        assert_eq!(w.app.project(), base, "Insert {what}");
        for word in says {
            assert!(
                w.app.status.text.contains(word),
                "{what}: the status line says {word:?}: {:?}",
                w.app.status.text
            );
        }
    }
    // The arrangement docked: its ruler, its sections lane, its tool row, its body.
    let spots = |w: &W| {
        let g = w.geo();
        let tools = w.word("Grid").expect("the tool row's Grid");
        [
            (pos2(w.x_of(bar(3)), g.ruler_top + 3.0), "on the ruler"),
            (w.section_rect(ids[1]).center(), "on a section"),
            (tools.center(), "on the tool row"),
            (over_the_arrangement(w, 3), "on a track's row"),
        ]
    };
    w.app.focus_arrangement();
    for (at, what) in spots(&w) {
        inserts(&mut w, &base, at, &format!("docked, {what}"));
    }
    // Floating in a window of its own.
    w.app.workspace.toggle_float(Pane::Arrangement);
    for _ in 0..4 {
        w.frame(vec![]);
    }
    assert!(w.app.workspace.is_floating(Pane::Arrangement));
    w.app.focus_arrangement();
    for (at, what) in spots(&w) {
        inserts(&mut w, &base, at, &format!("floating, {what}"));
    }
    w.app.workspace.toggle_float(Pane::Arrangement);
    for _ in 0..4 {
        w.frame(vec![]);
    }
    // Maximised.
    w.app.workspace.reveal(Pane::Arrangement);
    w.app.workspace.toggle_maximise(Pane::Arrangement);
    for _ in 0..4 {
        w.frame(vec![]);
    }
    w.app.focus_arrangement();
    for (at, what) in spots(&w) {
        inserts(&mut w, &base, at, &format!("maximised, {what}"));
    }
    w.app.workspace.toggle_maximise(Pane::Arrangement);
    w.app.workspace.reveal(Pane::Mixer);
    for _ in 0..4 {
        w.frame(vec![]);
    }
    let mixer = w
        .app
        .workspace
        .areas_in_order()
        .into_iter()
        .find(|(p, _)| *p == Pane::Mixer)
        .map(|(_, r)| r.center())
        .expect("the mixer on screen");
    // The arrangement focused by the mouse, the pointer on the mixer: refused, and said.
    let key = w
        .app
        .keymap
        .display_for(cantus_ui::keymap::Action::NextArea);
    w.app.focus_arrangement();
    refused(
        &mut w,
        &base,
        mixer,
        "over the mixer",
        &["arrangement", "Mixer", &key],
    );
    // The mixer focused: refused, and said.
    w.app.follow_area_focus(Pane::Mixer);
    let body = over_the_arrangement(&w, 3);
    refused(
        &mut w,
        &base,
        body,
        "with the mixer focused",
        &["arrangement", &key],
    );
    // Keyboard only: Ctrl+F6 to the arrangement with the mouse resting on the mixer — the
    // keys act there; a press of the mouse ends it.
    w.frame(vec![egui::Event::PointerMoved(mixer)]);
    for _ in 0..8 {
        if w.app.workspace.focused_area() == Some(Pane::Arrangement) {
            break;
        }
        w.key(egui::Key::F6, Modifiers::COMMAND);
    }
    assert_eq!(w.app.workspace.focused_area(), Some(Pane::Arrangement));
    inserts(
        &mut w,
        &base,
        mixer,
        "resting on the mixer, the focus moved by the keys",
    );
    w.click_with(mixer, egui::PointerButton::Primary);
    w.app.focus_arrangement();
    let base = w.app.project().clone();
    refused(
        &mut w,
        &base,
        mixer,
        "over the mixer once the mouse was pressed",
        &["Mixer"],
    );
}

// Review 2 of round 40, R2-B2: a cue holds until the next one. Ctrl+Del over time whose
// last cue is inside it, with none at its end, keeps that cue's scene for what follows —
// end to end, from the keys, and one undo step.
#[test]
fn deleting_time_from_the_keys_keeps_the_scene_of_what_follows() {
    use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
    use cantus_core::ripple_cues::scene_at;
    let mut p = kit_song();
    let snaps: Vec<MixSnapId> = (0..2).map(|_| MixSnapId(p.alloc_id())).collect();
    for (n, id) in snaps.iter().enumerate() {
        let state = capture(&p, &[], true);
        p.mix.snapshots.push(MixSnapshot {
            id: *id,
            name: format!("Scene {n}"),
            state,
        });
    }
    for (at, snap) in [(bar(1), snaps[0]), (bar(5), snaps[1])] {
        let id = MixCueId(p.alloc_id());
        p.mix.cues.push(MixCue {
            id,
            at,
            snapshot: snap,
            scope: Vec::new(),
        });
    }
    let mut w = W::with(p);
    w.key(egui::Key::Escape, Modifiers::NONE);
    let before = w.app.project().clone();
    w.app.set_time_selection(bar(4), bar(8));
    w.app.focus_arrangement();
    let over = over_the_arrangement(&w, 2);
    w.frame(vec![egui::Event::PointerMoved(over)]);
    w.key(egui::Key::Delete, Modifiers::COMMAND);
    let after = w.app.project().clone();
    assert_ne!(after, before, "Ctrl+Del deleted the time");
    assert_eq!(
        scene_at(&after, bar(4)),
        scene_at(&before, bar(8)),
        "what followed the range keeps its scene"
    );
    assert_eq!(scene_at(&after, bar(3)), scene_at(&before, bar(3)));
    w.app.undo();
    assert_eq!(w.app.project().mix, before.mix, "one undo step");
}
