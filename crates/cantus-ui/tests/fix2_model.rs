//! Round 42's mid-round review, the model side (`docs/round42/fix2-model.md`), through the
//! application — an app made by `CantusApp::assemble` (the boot path) or, for the editors'
//! gestures, the app the piano roll's own tests drive:
//!
//! - **Scala export → import** gives back the same pitches to 0.01 cent for every system and
//!   diapason (review 3), and a reader that follows Surge's `tuning-library`'s documented
//!   rules (the default `KeyboardMapping`: key 60 at `MIDI_0_FREQ * 32`; a `.kbm`'s reference
//!   key at its frequency, degree 0 on its middle key, a size of 0 linear) hears our files
//!   the same;
//! - **a note's articulation** survives *Slice*, *Duplicate* (with its expression and release
//!   velocity) and *Bake to notes* (review 6);
//! - **a quick double-click on empty grid** with the Draw tool draws a note and opens no menu;
//!   a double-click on a note does (review 12);
//! - **exports** carry what their format can: MusicXML's marks, MIDI's tuning when asked
//!   (review 7).
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

mod support;

use cantus_core::note_art::{Articulation, Bow, NoteArt};
use cantus_core::tuning::{
    KeyboardMap, ScalaScale, ScaleDegree, Temperament, Tuning, TuningSystem, TuningTable,
};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::views::tuning::{self, Target};
use cantus_ui::CantusApp;
use std::path::Path;
use std::sync::OnceLock;

fn data() -> &'static Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn assembled(project: Option<Project>) -> CantusApp {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    CantusApp::assemble(&ctx, files, project, engine, None, None, None, false)
}

fn outside(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../cantus-core/tests/data/tuning-library")
        .join(name)
}

/// Each key's pitch as a fractional MIDI key (12-TET at A440), `None` unmapped.
fn pitches(t: &TuningTable) -> Vec<Option<f64>> {
    (0..128u8)
        .map(|k| {
            let o = f64::from(t.at(k));
            (!o.is_nan()).then(|| f64::from(k) + o)
        })
        .collect()
}

/// `MIDI_0_FREQ` of Surge's `tuning-library` (`Tunings.h`).
const MIDI_0_FREQ: f64 = 8.175_798_915_643_71;

/// What Surge's `tuning-library` plays for key `k` from a `.scl` and an optional `.kbm`, by
/// its documented rules (`Tunings.h`): with no map, `KeyboardMapping()` — linear, middle
/// note 60, tuning constant note 60 at `MIDI_0_FREQ * 32`; with one, its middle key holds
/// degree 0, its reference key sounds at its frequency, a size of 0 is linear, a formal
/// octave of 0 is the scale's own period. Pitches as fractional MIDI keys.
fn surge_pitch(scl: &ScalaScale, kbm: Option<&KeyboardMap>, k: u8) -> Option<f64> {
    let cents: Vec<f64> = scl.degrees.iter().map(|d| d.cents()).collect();
    let n = cents.len() as i64;
    let period = cents[cents.len() - 1];
    let at = |d: i64| {
        let (q, r) = (d.div_euclid(n), d.rem_euclid(n));
        q as f64 * period + if r == 0 { 0.0 } else { cents[(r - 1) as usize] }
    };
    let (size, middle, refkey, refhz, octave, mapping) = match kbm {
        None => (0, 60i64, 60i64, MIDI_0_FREQ * 32.0, 0, Vec::new()),
        Some(m) => (
            m.size as i64,
            i64::from(m.middle_key),
            i64::from(m.reference_key),
            m.reference_hz,
            m.octave_degree as i64,
            m.mapping.clone(),
        ),
    };
    let degree = |key: i64| -> Option<i64> {
        let off = key - middle;
        if size == 0 {
            return Some(off);
        }
        let (q, i) = (off.div_euclid(size), off.rem_euclid(size));
        let oct = if octave == 0 { n } else { octave };
        mapping.get(i as usize).copied().flatten().map(|e| i64::from(e) + q * oct)
    };
    let d = degree(i64::from(k))?;
    let r = degree(refkey)?;
    let hz = refhz * 2f64.powf((at(d) - at(r)) / 1200.0);
    Some(69.0 + 12.0 * (hz / 440.0).log2())
}

fn werck(key: u8, a4: f64) -> Tuning {
    Tuning {
        a4_hz: a4,
        system: TuningSystem::Temperament {
            id: Temperament::WerckmeisterIii,
            key,
            follow_song_key: false,
        },
        map: None,
    }
}

/// r42 fix2-model (review 3): export → import gives the same pitches to 0.01 cent — every
/// system, a diapason that is not 440, a map scaled by it — and Surge's rules hear our files
/// as we do.
#[test]
fn a_scala_export_reads_back_as_the_same_pitches_here_and_in_surge() {
    let mut app = assembled(None);
    let edo31 = tuning::io::read_scl(&std::fs::read(outside("31edo.scl")).unwrap()).unwrap();
    let white = tuning::io::read_kbm(&std::fs::read(outside("mapping-a442-7-to-12.kbm")).unwrap())
        .unwrap();
    let ptolemy = ScalaScale {
        description: "!Ptolemy (a description that starts with a bang)".into(),
        degrees: [(9, 8), (5, 4), (4, 3), (3, 2), (5, 3), (15, 8), (2, 1)]
            .map(|(n, d)| ScaleDegree::Ratio(n, d))
            .to_vec(),
    };
    let cases = [
        ("equal at 432", Tuning { a4_hz: 432.0, ..Tuning::default() }),
        ("Werckmeister III in E flat at 415", werck(3, 415.0)),
        ("Werckmeister III in C at 440", werck(0, 440.0)),
        ("31-EDO at 440", Tuning { system: TuningSystem::Scale(edo31.clone()), ..Tuning::default() }),
        ("31-EDO at 415", Tuning { a4_hz: 415.0, system: TuningSystem::Scale(edo31), map: None }),
        (
            "Ptolemy on the white keys at 430",
            Tuning { a4_hz: 430.0, system: TuningSystem::Scale(ptolemy), map: Some(white) },
        ),
    ];
    let dir = tempfile::tempdir().unwrap();
    for (what, tu) in cases {
        assert!(tuning::set(&mut app, Target::Project, tu.clone()), "{what}: set");
        let want = pitches(&tuning::table_of(&app, Target::Project));
        let files = tuning::io::export(&app, Target::Project, &dir.path().join("song")).unwrap();
        let scl_path = files[0].clone();
        let kbm_path = files.get(1).cloned();
        // Back to the default, then imported as a user imports the two files.
        assert!(tuning::set(&mut app, Target::Project, Tuning::default()) || tu.is_default());
        tuning::io::import(&mut app, Target::Project, &scl_path, kbm_path.as_deref())
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        let got = pitches(&tuning::table_of(&app, Target::Project));
        // Surge's reading of the very files written.
        let scl = tuning::io::read_scl(&std::fs::read(&scl_path).unwrap()).unwrap();
        let kbm = kbm_path.map(|p| tuning::io::read_kbm(&std::fs::read(p).unwrap()).unwrap());
        for k in 0..128u8 {
            let (w, g) = (want[usize::from(k)], got[usize::from(k)]);
            match (w, g) {
                (Some(w), Some(g)) => {
                    assert!((w - g).abs() * 100.0 < 0.01, "{what}: key {k}: {w} vs {g}")
                }
                (None, None) => {}
                _ => panic!("{what}: key {k}: {w:?} vs {g:?}"),
            }
            // Inside the range Cantus keeps pitches in (1 Hz – 20 kHz).
            if let (Some(w), Some(s)) = (w, surge_pitch(&scl, kbm.as_ref(), k)) {
                if (-36.0..135.0).contains(&s) {
                    assert!((w - s).abs() * 100.0 < 0.01, "{what}: Surge's key {k}: {w} vs {s}");
                }
            } else {
                assert_eq!(w.is_none(), surge_pitch(&scl, kbm.as_ref(), k).is_none(), "{what}: {k}");
            }
        }
    }
}

// ---- the note's articulation through the editors' edits (review 6) ------------------------

const PIZZ: Option<NoteArt> = Some(NoteArt {
    articulation: Articulation::Pizzicato,
    bow: Some(Bow::Up),
});

/// One track, one clip, one note of two beats: pizzicato, up bow, a release velocity, a bend.
fn marked() -> (CantusApp, TrackId, ClipId, NoteId) {
    use cantus_core::model::note_expr::{ExprKind, ExprPoint, NoteExpr};
    let mut p = Project::new("arts");
    let mut t = p.make_track("Strings");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(8));
    let mut n = p.make_note(Ticks::ZERO, Ticks::beats(2), 60, 100);
    n.art = PIZZ;
    n.release = 33;
    n.expr = NoteExpr {
        bend: vec![ExprPoint(0, 0), ExprPoint(1920, ExprKind::Bend.store(1.0))],
        ..NoteExpr::default()
    }
    .boxed();
    let id = n.id;
    c.notes.push(n);
    let (tid, cid) = (t.id, c.id);
    t.clips.push(c);
    p.tracks.push(t);
    let mut a = support::app(p);
    a.select_clip(tid, cid);
    a.focus_piano_roll();
    (a, tid, cid, id)
}

fn notes(a: &CantusApp, t: TrackId, c: ClipId) -> Vec<Note> {
    a.project().track(t).unwrap().clip(c).unwrap().notes.clone()
}

#[test]
fn slicing_keeps_the_articulation_on_both_halves() {
    let (mut a, t, c, id) = marked();
    a.selected_notes = vec![id];
    assert_eq!(a.slice_at(Ticks::beats(1)), 1);
    let ns = notes(&a, t, c);
    assert_eq!(ns.len(), 2);
    for n in &ns {
        assert_eq!(n.art, PIZZ, "{:?}", n.start);
    }
    let tail = ns.iter().find(|n| n.start == Ticks::beats(1)).unwrap();
    assert_eq!(tail.release, 33, "the tail ends the note: its release velocity");
    a.undo();
    assert_eq!(notes(&a, t, c).len(), 1);
}

#[test]
fn duplicating_keeps_the_articulation_the_expression_and_the_release() {
    let (mut a, t, c, id) = marked();
    a.selected_notes = vec![id];
    a.duplicate_selection();
    let ns = notes(&a, t, c);
    assert_eq!(ns.len(), 2, "Ctrl+D made a copy");
    let (orig, copy) = (&ns[0], &ns[1]);
    assert_ne!(orig.id, copy.id);
    assert_eq!(copy.art, PIZZ);
    assert_eq!(copy.release, 33);
    assert_eq!(copy.expr, orig.expr);
    assert!(copy.expr.is_some());
}

#[test]
fn baking_note_effects_keeps_the_articulation() {
    let (mut a, t, c, _) = marked();
    assert!(a.add_note_fx(t, cantus_core::notefx::NoteFxType::Chord));
    assert!(a.bake_note_fx(t, c));
    let baked: Vec<Note> = a.project().track(t).unwrap().clips[0].notes.clone();
    assert!(baked.len() >= 3, "the chord's voices: {}", baked.len());
    for n in &baked {
        assert_eq!(n.art, PIZZ, "voice {} keeps its note's articulation", n.pitch);
    }
}

// ---- a quick double-click on empty grid (review 12) ---------------------------------------

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    frame: u64,
}

impl Rig {
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.frame += 1;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 800.0),
            )),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let a = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| cantus_ui::views::piano_roll::show(a, ui));
        });
        out.textures_delta.clear();
    }

    fn idle(&mut self, n: usize, at: egui::Pos2) {
        for _ in 0..n {
            self.frame(vec![egui::Event::PointerMoved(at)]);
        }
    }

    fn double_click(&mut self, at: egui::Pos2) {
        self.idle(40, at);
        for _ in 0..2 {
            for pressed in [true, false] {
                self.frame(vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
            }
        }
        self.idle(3, at);
    }

    fn menu_open(&self) -> bool {
        self.ctx
            .data(|d| d.get_temp::<bool>(egui::Id::new("pr-note-art-menu").with("open")))
            .unwrap_or(false)
    }
}

#[test]
fn a_quick_double_click_on_empty_grid_draws_a_note_and_opens_no_menu() {
    let mut p = Project::new("draw");
    let mut t = p.make_track("Strings");
    let c = p.make_clip("c", Ticks::ZERO, Ticks::beats(16));
    let (tid, cid) = (t.id, c.id);
    t.clips.push(c);
    p.tracks.push(t);
    let mut app = support::app(p);
    app.select_clip(tid, cid);
    app.focus_piano_roll();
    app.piano_roll.tool = cantus_ui::views::piano_roll::Tool::Draw;
    let mut r = Rig {
        ctx: support::ctx(),
        app,
        frame: 0,
    };
    let at = egui::pos2(760.0, 330.0);
    r.idle(5, at);
    r.double_click(at);
    let drawn = notes(&r.app, tid, cid);
    assert_eq!(drawn.len(), 1, "the first click drew a note; the second hit it");
    assert!(!r.menu_open(), "a double-click that drew the note opens no menu");
    // A double-click on that note, later, is a request for its menu.
    r.double_click(at);
    assert_eq!(notes(&r.app, tid, cid).len(), 1);
    assert!(r.menu_open(), "the note's menu, as before");
}

// ---- exports (review 7) ---------------------------------------------------------------------

#[test]
fn musicxml_from_the_app_writes_the_notes_marks() {
    let (a, t, _, _) = marked();
    let xml = cantus_ui::score::musicxml(&a, cantus_ui::score::Scope::Track(t));
    assert!(xml.contains(" pizzicato=\"yes\""), "{xml}");
    assert!(xml.contains("<technical><up-bow/></technical>"));
}

#[test]
fn the_midi_export_writes_the_tuning_only_when_asked() {
    use cantus_ui::views::interop::{export_midi_with, MidiScope};
    let mut p = Project::new("tuned");
    p.tuning = werck(3, 415.0);
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    c.notes.push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 63, 100));
    t.clips.push(c);
    p.tracks.push(t);
    let mut a = assembled(Some(p));
    let dir = tempfile::tempdir().unwrap();
    let (plain, tuned) = (dir.path().join("plain.mid"), dir.path().join("tuned.mid"));
    assert!(export_midi_with(&mut a, &plain, MidiScope::Song, 960, true, false));
    assert!(export_midi_with(&mut a, &tuned, MidiScope::Song, 960, true, true));
    let sysex = |p: &Path| {
        let s = cantus_project::midi::smf::read(&std::fs::read(p).unwrap()).unwrap();
        s.tracks
            .iter()
            .flatten()
            .filter_map(|e| match &e.event {
                cantus_project::midi::Event::SysEx(m) => Some(m.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert!(sysex(&plain).is_empty(), "off: no tuning messages");
    let m = sysex(&tuned);
    assert!(m.iter().any(|m| m.len() == 408 && m[4] == 0x01), "a bulk dump");
    // Werckmeister III at A415: each pitch class within ±100 cents? A415 is −101.4 c from
    // 440, so scale/octave cannot hold it: the dump's program is selected instead.
    assert!(!m.iter().any(|m| m.get(4) == Some(&0x09)));
}
